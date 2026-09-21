use super::{
    sweeper_boss::{
        NativeSweeperBossEyeCommandStep, NativeSweeperBossEyeLifecycle,
        NativeSweeperBossEyeLifecycleStep, EYE_COUNT,
    },
    sweeper_boss_script::{
        NativeSweeperStandaloneScriptRuntime, NativeSweeperStandaloneScriptStep,
        RobotsSweeperEyeScripts,
    },
};

/// Composes the native Eye handler state machine with its independently updated standalone
/// `EXItemAnimator_Script`. Keeping these as two owned sub-runtimes mirrors Robots' XItem layout:
/// handler first, attached animator second.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct NativeSweeperBossEyeRuntime {
    pub lifecycle: NativeSweeperBossEyeLifecycle,
    pub script: NativeSweeperStandaloneScriptRuntime,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct NativeSweeperBossEyeRuntimeStep {
    pub handler: NativeSweeperBossEyeLifecycleStep,
    pub animator: NativeSweeperStandaloneScriptStep,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct NativeSweeperBossOwnedEyeXItemPhaseStep {
    pub eye_steps: [NativeSweeperBossEyeRuntimeStep; EYE_COUNT],
    pub command_steps: [NativeSweeperBossEyeCommandStep; EYE_COUNT],
    pub random_draws_used: u8,
    pub transporter_activation_requests: u32,
}

impl NativeSweeperBossEyeRuntime {
    pub(crate) fn request_open(&mut self) {
        self.lifecycle.request_open();
    }

    /// One priority-0x14 Eye XItem update: Handler `0x004CE920 -> 0x004CEAF0` first, then the
    /// attached standalone Script animator `0x004FA5A8`. A SetScriptValue emitted by the animator
    /// therefore becomes visible to the *next* Handler update, exactly like native `+0x3D0`.
    #[cfg(test)]
    pub(crate) fn step_fixed_update(
        &mut self,
        scripts: &RobotsSweeperEyeScripts,
    ) -> NativeSweeperBossEyeRuntimeStep {
        self.step_fixed_update_interleaved(scripts, || ()).0
    }

    pub(crate) fn step_fixed_update_interleaved<T>(
        &mut self,
        scripts: &RobotsSweeperEyeScripts,
        middle: impl FnOnce() -> T,
    ) -> (NativeSweeperBossEyeRuntimeStep, T) {
        let handler = self.lifecycle.step();
        if let Some(script) = handler.script {
            self.script.restart(script);
        }

        let middle_result = middle();

        let animator = self.script.step(scripts);
        if let Some(value) = animator.last_script_value {
            self.lifecycle.script_signal = value.trunc() as i32;
        }

        (
            NativeSweeperBossEyeRuntimeStep { handler, animator },
            middle_result,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maps::triggers::sweeper_boss::{
        EYE_SCRIPT_OPEN, EYE_SCRIPT_OPENING, EYE_STATE_OPEN, EYE_STATE_OPENING,
    };
    use crate::maps::triggers::sweeper_boss_script::{
        NativeSweeperStandaloneScriptProfile, NativeSweeperStandaloneScriptValueEvent,
    };

    fn scripts() -> RobotsSweeperEyeScripts {
        RobotsSweeperEyeScripts::from_profiles_for_test([
            NativeSweeperStandaloneScriptProfile {
                script: EYE_SCRIPT_OPENING,
                frame_rate: 30.0,
                length: 11,
                events: vec![NativeSweeperStandaloneScriptValueEvent {
                    start_frame: 10,
                    length: 1,
                    value: 1.0,
                }],
            },
            NativeSweeperStandaloneScriptProfile {
                script: EYE_SCRIPT_OPEN,
                frame_rate: 30.0,
                length: 36,
                events: vec![],
            },
        ])
    }

    #[test]
    fn eye_handler_consumes_standalone_script_value_on_following_xitem_update() {
        let scripts = scripts();
        let mut eye = NativeSweeperBossEyeRuntime::default();
        eye.request_open();

        let activate = eye.step_fixed_update(&scripts);
        assert_eq!(activate.handler.script, Some(EYE_SCRIPT_OPENING));
        assert_eq!(eye.lifecycle.state, EYE_STATE_OPENING);
        assert_eq!(eye.script.current_frame, 1.0);
        assert_eq!(eye.lifecycle.script_signal, 0);

        for _ in 0..9 {
            let quiet = eye.step_fixed_update(&scripts);
            assert_eq!(quiet.animator.last_script_value, None);
            assert_eq!(eye.lifecycle.state, EYE_STATE_OPENING);
        }

        let event = eye.step_fixed_update(&scripts);
        assert_eq!(event.animator.frame_before, 10.0);
        assert_eq!(event.animator.last_script_value, Some(1.0));
        assert_eq!(eye.lifecycle.state, EYE_STATE_OPENING);
        assert_eq!(eye.lifecycle.script_signal, 1);

        let consume = eye.step_fixed_update(&scripts);
        assert_eq!(consume.handler.script, Some(EYE_SCRIPT_OPEN));
        assert_eq!(eye.lifecycle.state, EYE_STATE_OPEN);
        assert_eq!(eye.lifecycle.script_signal, 0);
        assert_eq!(eye.script.active_script, Some(EYE_SCRIPT_OPEN));
        assert_eq!(eye.script.current_frame, 1.0);
    }
}
