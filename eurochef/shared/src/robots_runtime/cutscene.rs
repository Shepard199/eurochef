use serde::Serialize;

use crate::script::{UXGeoScript, UXGeoScriptCommandData};

use super::{
    events::{
        event_type, RobotsCutsceneCommandKind, RobotsHandlerScriptCommandSemantic,
        RobotsScriptEventView,
    },
    script_host::native_script_ftol,
};

pub const ROBOTS_CUTSCENE_NATIVE_FADE_UPDATES: u32 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsCutsceneStateMarkerEntry {
    pub frame: i16,
    pub marker_uid: Option<u32>,
}

/// Native `0x00417600` builds this table once from Script Event commands whose
/// type is `0x1600000D`. Each row is exactly `{command.start, command+0x14}`.
pub fn build_cutscene_state_marker_table(
    script: &UXGeoScript,
) -> Vec<RobotsCutsceneStateMarkerEntry> {
    script
        .commands
        .iter()
        .filter_map(|command| {
            let event = RobotsScriptEventView::from_command(command)?;
            (event.event_type == event_type::STATE_MARKER).then_some(
                RobotsCutsceneStateMarkerEntry {
                    frame: command.start,
                    marker_uid: event.native_arg_word(0),
                },
            )
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsCutsceneStateMarkerEventResult {
    Continue,
    Hold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsCutsceneHandlerHostInput {
    /// `XItemHandler_Cutscene +0xDC = 0x00409B70` returns creator Cutscene
    /// `trigger+0x70`, i.e. serialized `data[1]`.
    pub trigger_flags: u32,
    pub fade_available: bool,
    /// Exact native comparisons are performed by the host against the shared
    /// GameWnd fade state: low means progress <= 0.001, high >= 0.999.
    pub fade_low_reached: bool,
    pub fade_high_reached: bool,
    /// `0x00407420` takes the ordinary production teardown path when the live
    /// creator owns an EXItemAnimator_Script at `XItem+0x144`.
    pub creator_script_present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RobotsCutsceneHandlerHostEffect {
    RequestFadeOut { duration_updates: u32 },
    RequestFadeIn { duration_updates: u32 },
    FinalizeCutscene,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsCutsceneHandlerStep {
    pub effect: Option<RobotsCutsceneHandlerHostEffect>,
    pub blocked_on_fade: bool,
    pub unsupported_missing_creator_script: bool,
}

/// Cutscene-local portion of `XItemHandler_Cutscene` used by StateMarker.
/// Field names retain the proven native offsets because neighboring commands
/// share these latches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RobotsCutsceneStateMarkerRuntime {
    pub marker_table: Vec<RobotsCutsceneStateMarkerEntry>,
    pub bound_script_uid: Option<u32>,
    /// Serialized Script FPS retained as bits so the runtime remains Eq/portable.
    pub bound_script_framerate_bits: u32,
    /// Native Handler `+0x3A0`: StateMarker-local Script time stored as f32 bits.
    pub marker_time_bits_3a0: u32,
    /// Native ScriptLifecycle marker guard `+0x3A4`; ctor `0x00416AF0` sets -1.
    pub marker_guard_3a4: i16,
    pub handler_state_16b0: u32,
    pub script_complete_16b8: bool,
    pub fade_direction_in_16b9: bool,
    pub teardown_complete_16ba: bool,
    pub marker_started_16bb: bool,
    pub marker_active_16bc: bool,
}

impl Default for RobotsCutsceneStateMarkerRuntime {
    fn default() -> Self {
        Self {
            marker_table: Vec::new(),
            bound_script_uid: None,
            bound_script_framerate_bits: 30.0f32.to_bits(),
            marker_time_bits_3a0: 0.0f32.to_bits(),
            marker_guard_3a4: -1,
            handler_state_16b0: 0,
            script_complete_16b8: false,
            fade_direction_in_16b9: false,
            teardown_complete_16ba: false,
            marker_started_16bb: false,
            marker_active_16bc: false,
        }
    }
}

impl RobotsCutsceneStateMarkerRuntime {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn bind_script(&mut self, script: &UXGeoScript) {
        if self.bound_script_uid == Some(script.hashcode) {
            return;
        }
        *self = Self {
            marker_table: build_cutscene_state_marker_table(script),
            bound_script_uid: Some(script.hashcode),
            bound_script_framerate_bits: script.timeline_framerate().to_bits(),
            ..Self::default()
        };
    }

    /// Native `SetAlternateState 0x1600000E` in `0x00416D10` is not a host
    /// presentation request. While marker guard `+0x3A4 == -1`, it calls the
    /// current `EXItemAnimator_Script +0x20 = 0x004FB462` with creator
    /// `XTrigger_Cutscene +0x40` seconds. That vslot multiplies by Script FPS
    /// and seeks through `0x004FA74C`.
    pub fn set_alternate_state_seek_frame_bits(
        &self,
        creator_saved_time_bits_40: u32,
    ) -> Option<u32> {
        if self.marker_guard_3a4 != -1 {
            return None;
        }
        let fps = f32::from_bits(self.bound_script_framerate_bits);
        let saved_seconds = f32::from_bits(creator_saved_time_bits_40);
        Some((saved_seconds * fps).to_bits())
    }

    /// Exact terminal/non-terminal split from `0x00407E40 -> 0x00416AB0`.
    pub fn on_state_marker(&mut self, start: i16) -> RobotsCutsceneStateMarkerEventResult {
        // On the first visit native stores `short(start) / ScriptFPS` at
        // Handler+0x3A0 before asking 0x00416AB0 whether a later marker exists.
        if !self.marker_active_16bc {
            let fps = f32::from_bits(self.bound_script_framerate_bits);
            self.marker_time_bits_3a0 = ((start as f32) / fps).to_bits();
        }
        if self.marker_table.iter().any(|entry| entry.frame > start) {
            return RobotsCutsceneStateMarkerEventResult::Continue;
        }
        if self.teardown_complete_16ba {
            self.script_complete_16b8 = true;
            return RobotsCutsceneStateMarkerEventResult::Continue;
        }
        if self.marker_active_16bc {
            return RobotsCutsceneStateMarkerEventResult::Hold;
        }
        self.marker_started_16bb = true;
        self.handler_state_16b0 = 1;
        self.marker_active_16bc = true;
        RobotsCutsceneStateMarkerEventResult::Hold
    }

    /// Native Handler tick `0x00407420` for the ordinary live-script branch.
    pub fn advance_handler(
        &mut self,
        input: RobotsCutsceneHandlerHostInput,
    ) -> RobotsCutsceneHandlerStep {
        let mut step = RobotsCutsceneHandlerStep::default();
        match self.handler_state_16b0 {
            1 => {
                self.fade_direction_in_16b9 = false;
                if input.trigger_flags & 2 == 0 && input.fade_available {
                    step.effect = Some(RobotsCutsceneHandlerHostEffect::RequestFadeOut {
                        duration_updates: ROBOTS_CUTSCENE_NATIVE_FADE_UPDATES,
                    });
                }
                self.handler_state_16b0 = 3;
            }
            2 => {
                self.fade_direction_in_16b9 = true;
                if input.trigger_flags & 2 == 0 && input.fade_available {
                    step.effect = Some(RobotsCutsceneHandlerHostEffect::RequestFadeIn {
                        duration_updates: ROBOTS_CUTSCENE_NATIVE_FADE_UPDATES,
                    });
                }
                self.handler_state_16b0 = 3;
            }
            3 => {
                if input.trigger_flags & 2 == 0 && input.fade_available {
                    let threshold_reached = if self.fade_direction_in_16b9 {
                        input.fade_high_reached
                    } else {
                        input.fade_low_reached
                    };
                    if !threshold_reached {
                        step.blocked_on_fade = true;
                        return step;
                    }
                }
                if !input.creator_script_present {
                    step.unsupported_missing_creator_script = true;
                    return step;
                }
                if self.fade_direction_in_16b9 {
                    if self.marker_started_16bb {
                        self.teardown_complete_16ba = true;
                    } else {
                        self.handler_state_16b0 = 4;
                    }
                } else {
                    // `0x004075C0` decides whether the live Handler is released
                    // immediately (ChangeLevel/linked Cutscene/special Player) or
                    // retained with +0x16BA=1/state6 (ordinary branch). Do not
                    // pre-commit the ordinary latch before the host resolves it.
                    step.effect = Some(RobotsCutsceneHandlerHostEffect::FinalizeCutscene);
                }
            }
            _ => {}
        }
        step
    }

    /// Ordinary `0x004075C0` tail only. Routed/special branches release the
    /// Handler instead and therefore never execute this latch/state write.
    pub fn complete_ordinary_finalize(&mut self) {
        self.teardown_complete_16ba = true;
        self.handler_state_16b0 = 6;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsCutsceneFinalizeDispatchTarget {
    ChangeLevel,
    Cutscene,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsCutsceneFinalizeBranch {
    ChangeLevel,
    LinkedCutscene,
    SpecialPlayerState,
    Ordinary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsCutsceneFinalizeInput {
    /// Handler `+0x3A0`, set by StateMarker before the terminal handshake.
    pub marker_time_bits_3a0: u32,
    /// Handler `+0x16C3`: when set, finalization stops/clears Cutscene audio state.
    pub audio_active_16c3: bool,
    /// Handler `+0x16C4`: Script audio scan found at least one music (`0x1B`) command.
    pub music_audio_present_16c4: bool,
    /// Creator `XTrigger_Cutscene +0xE8`, read by native `0x004075C0`.
    pub creator_flags_e8: u8,
    /// Player Handler `+0x6DE`. Only 0x1D/0x3E/0x2F select the special branch.
    pub player_handler_state_6de: Option<u8>,
    /// Handler `+0x16B4` remembered HUD/display mask.
    pub saved_display_mask_16b4: u32,
    /// Native global `DAT_007B3174`; only saved bits still present here are restored.
    pub active_display_mask: u32,
    /// SwapCharacter arg1 path sets Handler `+0x16BD`; finalization consumes it.
    pub pending_swap_character_16bd: bool,
    /// Native global Player mode byte `DAT_007B3207`, preserved raw for the host
    /// because `0x004B0120/0x004B02A0` ownership is a separate Player boundary.
    pub player_mode: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RobotsCutsceneFinalizeEffect {
    /// Exact `0x004078D0` + Script animator vslot `+0x20(marker_time_bits)` preamble.
    /// `music_audio_present_16c4` preserves its optional `0x00407960` music-range branch.
    FinalizeScriptAnimatorTail {
        marker_time_bits_3a0: u32,
        music_audio_present_16c4: bool,
    },
    /// Handler `+0x16C3` branch: native conditionally stops active audio through
    /// the audio manager and always clears the Cutscene-local audio owner state.
    StopAndClearCutsceneAudio,
    SetMusicVolumePercent {
        percent: u32,
    },
    /// Exact `0x00433280(state)`: replace the current UI/game-state stack top
    /// with `state`, or push it when the stack is empty.
    SetGameStateTop {
        state: u32,
    },
    /// Exact `0x004D10D0(1)`: refresh the Player-side audio/UI parameters and,
    /// when requested, copy Player Handler maxHealth `+0x388` into
    /// currentHealth `+0x384`.
    RefreshPlayerAudioAndRestoreHealth {
        restore_full_health: bool,
    },
    /// Handler vslot `+0x90 = 0x00408790` runs common Handler cleanup, which
    /// restores remembered display bits, removes UI/game state 3, reaches
    /// `0x00443EE0`, marks the owning XItem for removal and calls creator +0x84.
    ReleaseCutsceneHandler {
        restore_display_mask: u32,
    },
    /// Creator Cutscene vslot `+0x88 = 0x0048B2D0`: invoke creator vslot
    /// `+0xE0(-1)` and then restore trigger `+0xE4` alternate override to -1.
    ResetCreatorActivationOverride,
    DispatchFirstTriggerType {
        target: RobotsCutsceneFinalizeDispatchTarget,
        event_mask: u32,
    },
    /// Exact `0x004D1040` boundary. Its internal continue-point/player behavior
    /// remains a host operation instead of leaking XApp globals into shared runtime.
    RestoreGameplayAfterSpecialPlayerState,
    /// Exact ordinary `0x00409810` call arguments reached from `0x004075C0`.
    ApplyCreatorCutsceneEntityPolicy {
        arg0: u32,
        arg1: u32,
        arg2: u32,
    },
    DestroyDisplayMask {
        mask: u32,
    },
    RestoreDisplayMask {
        mask: u32,
    },
    ClearSavedDisplayMask,
    /// Native `DAT_007B31CC` is the current Mission/UI owner. Ordinary Cutscene
    /// teardown clears `DAT_007B25A8` only when no such owner remains.
    ClearGlobalCutsceneActiveIfNoMissionOwner,
    ConsumePendingSwapCharacter {
        player_mode: Option<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RobotsCutsceneFinalizePlan {
    pub branch: RobotsCutsceneFinalizeBranch,
    pub effects: Vec<RobotsCutsceneFinalizeEffect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsCutscenePlayerHealthState {
    pub current: f32,
    pub max: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RobotsCutsceneHostRuntimeState {
    pub music_volume_percent: u32,
    pub game_state_stack: Vec<u32>,
    pub display_mask: u32,
    pub global_cutscene_active: bool,
    pub mission_owner_present: bool,
    pub player_health: Option<RobotsCutscenePlayerHealthState>,
    pub player_audio_refresh_requests: u32,
    pub last_creator_entity_policy: Option<[u32; 3]>,
    pub special_gameplay_restore_requests: u32,
    pub pending_swap_character_consumptions: u32,
    pub last_consumed_player_mode: Option<u8>,
    pub player_action_requests: u32,
    pub last_player_action: Option<RobotsCutscenePlayerAction>,
    pub alternate_state_requests: u32,
}

impl Default for RobotsCutsceneHostRuntimeState {
    fn default() -> Self {
        Self {
            music_volume_percent: 100,
            game_state_stack: Vec::new(),
            display_mask: 0,
            global_cutscene_active: false,
            mission_owner_present: false,
            player_health: None,
            player_audio_refresh_requests: 0,
            last_creator_entity_policy: None,
            special_gameplay_restore_requests: 0,
            pending_swap_character_consumptions: 0,
            last_consumed_player_mode: None,
            player_action_requests: 0,
            last_player_action: None,
            alternate_state_requests: 0,
        }
    }
}

impl RobotsCutsceneHostRuntimeState {
    /// Apply one engine-neutral host effect in native order. This owns only
    /// globals already proved by `0x004075C0`/`0x00408790`; map/XItem/link
    /// ownership remains in the GUI/UE host adapter.
    pub fn apply_finalize_effect(&mut self, effect: RobotsCutsceneFinalizeEffect) {
        match effect {
            RobotsCutsceneFinalizeEffect::FinalizeScriptAnimatorTail { .. }
            | RobotsCutsceneFinalizeEffect::StopAndClearCutsceneAudio
            | RobotsCutsceneFinalizeEffect::ResetCreatorActivationOverride
            | RobotsCutsceneFinalizeEffect::DispatchFirstTriggerType { .. }
            | RobotsCutsceneFinalizeEffect::ClearSavedDisplayMask => {}
            RobotsCutsceneFinalizeEffect::SetMusicVolumePercent { percent } => {
                self.music_volume_percent = percent;
            }
            RobotsCutsceneFinalizeEffect::SetGameStateTop { state } => {
                if let Some(top) = self.game_state_stack.last_mut() {
                    *top = state;
                } else {
                    self.game_state_stack.push(state);
                }
            }
            RobotsCutsceneFinalizeEffect::RefreshPlayerAudioAndRestoreHealth {
                restore_full_health,
            } => {
                self.player_audio_refresh_requests =
                    self.player_audio_refresh_requests.wrapping_add(1);
                if restore_full_health {
                    if let Some(health) = self.player_health.as_mut() {
                        health.current = health.max;
                    }
                }
            }
            RobotsCutsceneFinalizeEffect::ReleaseCutsceneHandler {
                restore_display_mask,
            } => {
                self.display_mask &= !0x8;
                self.display_mask |= restore_display_mask;
                if self.game_state_stack.last().copied() == Some(3) {
                    self.game_state_stack.pop();
                }
            }
            RobotsCutsceneFinalizeEffect::RestoreGameplayAfterSpecialPlayerState => {
                self.special_gameplay_restore_requests =
                    self.special_gameplay_restore_requests.wrapping_add(1);
            }
            RobotsCutsceneFinalizeEffect::ApplyCreatorCutsceneEntityPolicy { arg0, arg1, arg2 } => {
                self.last_creator_entity_policy = Some([arg0, arg1, arg2]);
            }
            RobotsCutsceneFinalizeEffect::DestroyDisplayMask { mask } => {
                self.display_mask &= !mask;
            }
            RobotsCutsceneFinalizeEffect::RestoreDisplayMask { mask } => {
                self.display_mask |= mask;
            }
            RobotsCutsceneFinalizeEffect::ClearGlobalCutsceneActiveIfNoMissionOwner => {
                if !self.mission_owner_present {
                    self.global_cutscene_active = false;
                }
            }
            RobotsCutsceneFinalizeEffect::ConsumePendingSwapCharacter { player_mode } => {
                self.pending_swap_character_consumptions =
                    self.pending_swap_character_consumptions.wrapping_add(1);
                self.last_consumed_player_mode = player_mode;
            }
        }
    }

    pub fn apply_script_effect(&mut self, effect: RobotsCutsceneEffect) {
        match effect {
            RobotsCutsceneEffect::PerformActionCutscene {
                action: Some(action),
                ..
            } => {
                self.player_action_requests = self.player_action_requests.wrapping_add(1);
                self.last_player_action = Some(action);
            }
            RobotsCutsceneEffect::SetAlternateState => {
                self.alternate_state_requests = self.alternate_state_requests.wrapping_add(1);
            }
            _ => {}
        }
    }
}

fn cutscene_special_player_state(state: Option<u8>) -> bool {
    matches!(state, Some(0x1d | 0x3e | 0x2f))
}

/// Engine-neutral projection of native `XItemHandler_Cutscene` finalizer
/// `0x004075C0`. Branch ordering is exact: creator bit1 (ChangeLevel), creator
/// bit2 (linked Cutscene), special Player state, ordinary teardown.
pub fn plan_cutscene_finalize(input: RobotsCutsceneFinalizeInput) -> RobotsCutsceneFinalizePlan {
    let player_special = cutscene_special_player_state(input.player_handler_state_6de);
    let mut effects = vec![RobotsCutsceneFinalizeEffect::FinalizeScriptAnimatorTail {
        marker_time_bits_3a0: input.marker_time_bits_3a0,
        music_audio_present_16c4: input.music_audio_present_16c4,
    }];
    if input.audio_active_16c3 {
        effects.push(RobotsCutsceneFinalizeEffect::StopAndClearCutsceneAudio);
    }
    effects.extend([
        RobotsCutsceneFinalizeEffect::SetMusicVolumePercent { percent: 100 },
        RobotsCutsceneFinalizeEffect::SetGameStateTop { state: 3 },
    ]);

    let branch = if input.creator_flags_e8 & 0x02 != 0 {
        if player_special {
            effects.push(
                RobotsCutsceneFinalizeEffect::RefreshPlayerAudioAndRestoreHealth {
                    restore_full_health: true,
                },
            );
        }
        effects.push(RobotsCutsceneFinalizeEffect::ReleaseCutsceneHandler {
            restore_display_mask: input.saved_display_mask_16b4 & input.active_display_mask,
        });
        effects.push(RobotsCutsceneFinalizeEffect::ResetCreatorActivationOverride);
        effects.push(RobotsCutsceneFinalizeEffect::DispatchFirstTriggerType {
            target: RobotsCutsceneFinalizeDispatchTarget::ChangeLevel,
            event_mask: 0x100,
        });
        RobotsCutsceneFinalizeBranch::ChangeLevel
    } else if input.creator_flags_e8 & 0x04 != 0 {
        effects.push(RobotsCutsceneFinalizeEffect::ReleaseCutsceneHandler {
            restore_display_mask: input.saved_display_mask_16b4 & input.active_display_mask,
        });
        effects.push(RobotsCutsceneFinalizeEffect::ResetCreatorActivationOverride);
        effects.push(RobotsCutsceneFinalizeEffect::DispatchFirstTriggerType {
            target: RobotsCutsceneFinalizeDispatchTarget::Cutscene,
            event_mask: 0x1,
        });
        RobotsCutsceneFinalizeBranch::LinkedCutscene
    } else if player_special {
        effects.push(RobotsCutsceneFinalizeEffect::ReleaseCutsceneHandler {
            restore_display_mask: input.saved_display_mask_16b4 & input.active_display_mask,
        });
        effects.push(RobotsCutsceneFinalizeEffect::ResetCreatorActivationOverride);
        effects.push(RobotsCutsceneFinalizeEffect::RestoreGameplayAfterSpecialPlayerState);
        RobotsCutsceneFinalizeBranch::SpecialPlayerState
    } else {
        effects.push(
            RobotsCutsceneFinalizeEffect::ApplyCreatorCutsceneEntityPolicy {
                arg0: 1,
                arg1: 0,
                arg2: 0,
            },
        );
        effects.push(RobotsCutsceneFinalizeEffect::DestroyDisplayMask { mask: 0x8 });
        effects.push(RobotsCutsceneFinalizeEffect::RestoreDisplayMask {
            mask: input.saved_display_mask_16b4 & input.active_display_mask,
        });
        effects.push(RobotsCutsceneFinalizeEffect::ClearSavedDisplayMask);
        effects.push(RobotsCutsceneFinalizeEffect::ClearGlobalCutsceneActiveIfNoMissionOwner);
        if input.pending_swap_character_16bd {
            effects.push(RobotsCutsceneFinalizeEffect::ConsumePendingSwapCharacter {
                player_mode: input.player_mode,
            });
        }
        RobotsCutsceneFinalizeBranch::Ordinary
    };

    RobotsCutsceneFinalizePlan { branch, effects }
}

/// Exact native Script resource selection performed for `XItemHandler_Cutscene`.
///
/// Native ownership chain:
/// - `XTrigger_Cutscene::CreateItem 0x00488340` copies trigger `+0xE4` into
///   handler `+0x3B8` and clears the trigger field back to `-1`.
/// - `0x00409730` uses handler `+0x3B8` when it is not `-1`; otherwise it uses
///   trigger `+0x54` (the serialized default Script UID).
/// - trigger `+0x50` is the serialized external owner-file UID when present;
///   otherwise native uses the current/default resource owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsCutsceneScriptSelection {
    pub owner_file_uid: u32,
    pub default_script_uid: Option<u32>,
    pub selected_script_uid: u32,
    pub alternate_applied: bool,
}

pub const fn select_cutscene_script(
    current_file_uid: u32,
    explicit_file_uid: Option<u32>,
    default_script_uid: Option<u32>,
    alternate_script_uid: Option<u32>,
) -> Option<RobotsCutsceneScriptSelection> {
    let selected_script_uid = match alternate_script_uid {
        Some(uid) => uid,
        None => match default_script_uid {
            Some(uid) => uid,
            None => return None,
        },
    };
    Some(RobotsCutsceneScriptSelection {
        owner_file_uid: match explicit_file_uid {
            Some(uid) => uid,
            None => current_file_uid,
        },
        default_script_uid,
        selected_script_uid,
        alternate_applied: alternate_script_uid.is_some(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsCutsceneToggle {
    Keep,
    Disabled,
    Enabled,
    Unsupported,
}

impl RobotsCutsceneToggle {
    pub fn from_native(raw: Option<u32>) -> Self {
        match raw.map(|value| value as i32) {
            None | Some(-1) => Self::Keep,
            Some(0) => Self::Disabled,
            Some(1) => Self::Enabled,
            Some(_) => Self::Unsupported,
        }
    }
}

/// The finalizer-relevant subset of native `XTrigger_Cutscene +0xE8` initialized
/// by `0x00488100`: bit1 when a linked ChangeLevel exists; bit2 when a linked
/// Cutscene exists and creator serialized `data[3] & 2` does not suppress it.
pub const fn initial_cutscene_finalize_route_flags(
    has_change_level_link: bool,
    has_cutscene_link: bool,
    suppress_linked_cutscene: bool,
) -> u8 {
    (if has_change_level_link { 0x02 } else { 0 })
        | (if has_cutscene_link && !suppress_linked_cutscene {
            0x04
        } else {
            0
        })
}

/// `SetPropertiesCutscene` arg0 reaches `0x004881C0`. Disable always clears the
/// ChangeLevel route bit; enable sets it only when the creator actually has a
/// linked ChangeLevel. Keep/unsupported do not invent state.
pub const fn apply_cutscene_change_level_route_toggle(
    flags: u8,
    toggle: RobotsCutsceneToggle,
    has_change_level_link: bool,
) -> u8 {
    match toggle {
        RobotsCutsceneToggle::Disabled => flags & !0x02,
        RobotsCutsceneToggle::Enabled if has_change_level_link => flags | 0x02,
        RobotsCutsceneToggle::Enabled
        | RobotsCutsceneToggle::Keep
        | RobotsCutsceneToggle::Unsupported => flags,
    }
}

/// `SetPropertiesCutscene` arg1 owns Handler byte `+0x16BE`. Disable always
/// clears it. Enable is accepted only when creator `XTrigger_Cutscene data[0]`
/// bit0 is clear, matching `0x00407E40` without inventing a broader message policy.
pub const fn apply_cutscene_message_toggle(
    current: bool,
    toggle: RobotsCutsceneToggle,
    creator_data0: u32,
) -> bool {
    match toggle {
        RobotsCutsceneToggle::Disabled => false,
        RobotsCutsceneToggle::Enabled if creator_data0 & 1 == 0 => true,
        RobotsCutsceneToggle::Enabled
        | RobotsCutsceneToggle::Keep
        | RobotsCutsceneToggle::Unsupported => current,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsCutsceneAudioScan {
    pub streamed_sfx_present_16c3: bool,
    pub music_present_16c4: bool,
}

/// Native `0x0040A110` audio precache scan. It walks serialized Script commands
/// only until opcode18, marks `+0x16C4` for music-family `0x1B` Sound commands,
/// and marks `+0x16C3` when any non-music SoundDetails row has byte `+0x0A`
/// (`sample_streamed`) set. Missing SoundDetails behaves like native fallback: false.
pub fn scan_cutscene_script_audio<F>(
    script: &UXGeoScript,
    mut sample_streamed: F,
) -> RobotsCutsceneAudioScan
where
    F: FnMut(u32) -> Option<bool>,
{
    let mut scan = RobotsCutsceneAudioScan::default();
    for command in &script.commands {
        if command.opcode == 18 {
            break;
        }
        let UXGeoScriptCommandData::Sound { hashcode } = &command.data else {
            continue;
        };
        if hashcode & 0x7f00_0000 == 0x1b00_0000 {
            scan.music_present_16c4 = true;
        } else if sample_streamed(*hashcode).unwrap_or(false) {
            scan.streamed_sfx_present_16c3 = true;
        }
    }
    scan
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsCutscenePlayerAction {
    Action48000000,
    Action48000003,
    Action48000004,
    Action48000005,
    Action48000008,
    Action4800000b,
    SetHandlerFlag4000,
}

pub fn cutscene_player_action(mode: Option<u32>) -> Option<RobotsCutscenePlayerAction> {
    match mode.map(|value| value as i32) {
        Some(0) => Some(RobotsCutscenePlayerAction::Action48000000),
        Some(1) => Some(RobotsCutscenePlayerAction::Action48000003),
        Some(2) => Some(RobotsCutscenePlayerAction::Action48000004),
        Some(3) => Some(RobotsCutscenePlayerAction::Action48000005),
        Some(4) => Some(RobotsCutscenePlayerAction::Action48000008),
        Some(5) => Some(RobotsCutscenePlayerAction::Action4800000b),
        Some(6) => Some(RobotsCutscenePlayerAction::SetHandlerFlag4000),
        _ => None,
    }
}

/// Engine-neutral host effects for the Cutscene-specific `Handler +0x5C`
/// command family (`0x00407E40`). These plans deliberately preserve raw native
/// arguments whenever the host-side meaning is not yet fully reconstructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RobotsCutsceneEffect {
    StateMarker {
        args: [Option<u32>; 4],
        start: Option<i16>,
    },
    SwapCharacter {
        mode: Option<i32>,
    },
    ShowMessage {
        message_uid: Option<u32>,
        parameter_08: Option<i32>,
        use_script_length: Option<bool>,
        parameter_10: Option<i32>,
        parameter_14: Option<i32>,
        start: Option<i16>,
        length: Option<u16>,
    },
    MessageRelaySpecific {
        event_mask: Option<u32>,
        link_mask: Option<u8>,
    },
    ShakeCamera {
        magnitude: Option<i32>,
        duration_updates: Option<i32>,
        primary_channel: Option<bool>,
        player_channel: Option<bool>,
        raw_flags: Option<u32>,
    },
    SetAlternateState,
    WaitForText {
        args: [Option<u32>; 4],
        start: Option<i16>,
    },
    ExitScript,
    PositionCharacter {
        owner_selector: Option<i32>,
        position_selector: Option<i32>,
    },
    SetProperties {
        args: [Option<u32>; 4],
        start: Option<i16>,
    },
    SetPropertiesCutscene {
        cutscene_enabled: RobotsCutsceneToggle,
        message_enabled: RobotsCutsceneToggle,
        raw_property: Option<u32>,
        music_volume_percent: Option<u32>,
    },
    MissionCheck {
        args: [Option<u32>; 4],
        start: Option<i16>,
    },
    AnimationControl {
        link_index: Option<i32>,
        action: Option<i32>,
        enabled: Option<bool>,
    },
    SetPropertiesPlayer {
        mode: Option<i32>,
    },
    PerformActionCutscene {
        raw_mode: Option<i32>,
        action: Option<RobotsCutscenePlayerAction>,
    },
}

fn signed(value: Option<u32>) -> Option<i32> {
    value.map(|value| value as i32)
}

/// `0x00407E40` compares SetPropertiesCutscene arg2 against native float
/// constants 0.0 and 100.0, then converts the accepted value to an integer
/// before calling music-volume setter `0x004385B0`.
pub fn cutscene_music_volume_percent(value: Option<u32>) -> Option<u32> {
    let value = signed(value)?;
    (0 < value && value < 100).then_some(value as u32)
}

pub fn plan_cutscene_effect(
    semantic: &RobotsHandlerScriptCommandSemantic,
) -> Option<RobotsCutsceneEffect> {
    match semantic {
        RobotsHandlerScriptCommandSemantic::MessageRelaySpecific { arg0, arg1 } => {
            Some(RobotsCutsceneEffect::MessageRelaySpecific {
                event_mask: *arg0,
                link_mask: arg1.map(|mask| mask as u8),
            })
        }
        RobotsHandlerScriptCommandSemantic::ShakeCamera { arg0, arg1, flags } => {
            Some(RobotsCutsceneEffect::ShakeCamera {
                magnitude: arg0.map(native_script_ftol),
                duration_updates: arg1.map(native_script_ftol),
                primary_channel: flags.map(|flags| flags & 0x2 == 0),
                player_channel: flags.map(|flags| flags & 0x1 == 0),
                raw_flags: *flags,
            })
        }
        RobotsHandlerScriptCommandSemantic::SetAlternateState => {
            Some(RobotsCutsceneEffect::SetAlternateState)
        }
        RobotsHandlerScriptCommandSemantic::CutsceneCommand {
            command,
            args,
            start,
            length,
        } => {
            let args4 = [args[0], args[1], args[2], args[3]];
            Some(match command {
                RobotsCutsceneCommandKind::StateMarker => RobotsCutsceneEffect::StateMarker {
                    args: args4,
                    start: *start,
                },
                RobotsCutsceneCommandKind::SwapCharacter => RobotsCutsceneEffect::SwapCharacter {
                    mode: signed(args[0]),
                },
                RobotsCutsceneCommandKind::ShowMessage => RobotsCutsceneEffect::ShowMessage {
                    message_uid: args[0],
                    parameter_08: args[1].map(f32::from_bits).map(native_script_ftol),
                    use_script_length: args[1].map(|raw| raw as u8 & 1 == 0),
                    parameter_10: signed(args[4]),
                    parameter_14: signed(args[5]),
                    start: *start,
                    length: *length,
                },
                RobotsCutsceneCommandKind::WaitForText => RobotsCutsceneEffect::WaitForText {
                    args: args4,
                    start: *start,
                },
                RobotsCutsceneCommandKind::ExitScript => RobotsCutsceneEffect::ExitScript,
                RobotsCutsceneCommandKind::PositionCharacter => {
                    RobotsCutsceneEffect::PositionCharacter {
                        owner_selector: signed(args[0]),
                        position_selector: signed(args[1]),
                    }
                }
                RobotsCutsceneCommandKind::SetProperties => RobotsCutsceneEffect::SetProperties {
                    args: args4,
                    start: *start,
                },
                RobotsCutsceneCommandKind::SetPropertiesCutscene => {
                    RobotsCutsceneEffect::SetPropertiesCutscene {
                        cutscene_enabled: RobotsCutsceneToggle::from_native(args[0]),
                        message_enabled: RobotsCutsceneToggle::from_native(args[1]),
                        raw_property: args[2],
                        music_volume_percent: cutscene_music_volume_percent(args[2]),
                    }
                }
                RobotsCutsceneCommandKind::MissionCheck => RobotsCutsceneEffect::MissionCheck {
                    args: args4,
                    start: *start,
                },
                RobotsCutsceneCommandKind::AnimationControl => {
                    RobotsCutsceneEffect::AnimationControl {
                        link_index: signed(args[0]),
                        action: signed(args[1]),
                        enabled: args[2].map(|value| value & 1 != 0),
                    }
                }
                RobotsCutsceneCommandKind::SetPropertiesPlayer => {
                    RobotsCutsceneEffect::SetPropertiesPlayer {
                        mode: signed(args[0]),
                    }
                }
                RobotsCutsceneCommandKind::PerformActionCutscene => {
                    RobotsCutsceneEffect::PerformActionCutscene {
                        raw_mode: signed(args[0]),
                        action: cutscene_player_action(args[0]),
                    }
                }
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        robots_runtime::events::RobotsHandlerScriptCommandSemantic,
        script::{UXGeoScriptCommand, UXGeoScriptCommandData},
    };

    fn state_marker(start: i16, marker_uid: u32) -> UXGeoScriptCommand {
        let mut data = 0u32.to_le_bytes().to_vec();
        data.extend_from_slice(&marker_uid.to_le_bytes());
        UXGeoScriptCommand {
            opcode: 11,
            start,
            length: 1,
            controller_header_index: 0,
            controller_index: 0,
            parent_controller_index: 0xff,
            data: UXGeoScriptCommandData::Event {
                event_type: event_type::STATE_MARKER,
                data,
            },
        }
    }

    fn marker_script(commands: Vec<UXGeoScriptCommand>) -> UXGeoScript {
        UXGeoScript {
            hashcode: 0x0400_028E,
            framerate: 30.0,
            length: 321,
            num_threads: 1,
            commands,
            serialized_controller_count: 0,
            controller_record_metadata: vec![],
            controllers: vec![],
            controller_group_indices: vec![],
            controller_groups: vec![],
        }
    }

    fn sound_command(start: i16, hashcode: u32) -> UXGeoScriptCommand {
        UXGeoScriptCommand {
            opcode: 5,
            start,
            length: 1,
            controller_header_index: 0,
            controller_index: 0,
            parent_controller_index: 0xff,
            data: UXGeoScriptCommandData::Sound { hashcode },
        }
    }

    #[test]
    fn audio_scan_matches_native_streamed_sfx_music_and_opcode18_cutoff() {
        let mut commands = vec![sound_command(1, 0x1AF0_0123), sound_command(2, 0x1B00_0042)];
        commands.push(UXGeoScriptCommand {
            opcode: 18,
            start: 0,
            length: 0,
            controller_header_index: 0,
            controller_index: 0,
            parent_controller_index: 0,
            data: UXGeoScriptCommandData::Unknown {
                cmd: 18,
                data: vec![],
            },
        });
        commands.push(sound_command(3, 0x1AF0_0999));
        let script = marker_script(commands);
        let mut visited = Vec::new();
        let scan = scan_cutscene_script_audio(&script, |uid| {
            visited.push(uid);
            Some(uid == 0x1AF0_0123 || uid == 0x1AF0_0999)
        });
        assert!(scan.streamed_sfx_present_16c3);
        assert!(scan.music_present_16c4);
        assert_eq!(visited, vec![0x1AF0_0123]);
    }

    #[test]
    fn state_marker_table_is_exact_frame_uid_projection_of_script_events() {
        let script = marker_script(vec![
            state_marker(20, 0x5600_0001),
            state_marker(245, u32::MAX),
        ]);
        assert_eq!(
            build_cutscene_state_marker_table(&script),
            vec![
                RobotsCutsceneStateMarkerEntry {
                    frame: 20,
                    marker_uid: Some(0x5600_0001),
                },
                RobotsCutsceneStateMarkerEntry {
                    frame: 245,
                    marker_uid: Some(u32::MAX),
                },
            ]
        );
    }

    #[test]
    fn set_alternate_state_projects_creator_saved_seconds_to_script_frame_only_when_guard_is_clear()
    {
        let script = marker_script(vec![state_marker(20, 0x5600_0001)]);
        let mut runtime = RobotsCutsceneStateMarkerRuntime::default();
        runtime.bind_script(&script);
        assert_eq!(runtime.marker_guard_3a4, -1);
        assert_eq!(
            runtime.set_alternate_state_seek_frame_bits((2.5f32).to_bits()),
            Some(75.0f32.to_bits())
        );
        runtime.marker_guard_3a4 = 4;
        assert_eq!(
            runtime.set_alternate_state_seek_frame_bits((2.5f32).to_bits()),
            None
        );
    }

    #[test]
    fn terminal_state_marker_holds_until_native_fade_teardown_latch_then_releases() {
        let script = marker_script(vec![
            state_marker(20, 0x5600_0001),
            state_marker(245, u32::MAX),
        ]);
        let mut runtime = RobotsCutsceneStateMarkerRuntime::default();
        runtime.bind_script(&script);

        assert_eq!(
            runtime.on_state_marker(20),
            RobotsCutsceneStateMarkerEventResult::Continue
        );
        assert_eq!(runtime.marker_time_bits_3a0, (20.0f32 / 30.0).to_bits());
        assert_eq!(
            runtime.on_state_marker(245),
            RobotsCutsceneStateMarkerEventResult::Hold
        );
        assert_eq!(runtime.marker_time_bits_3a0, (245.0f32 / 30.0).to_bits());
        assert_eq!(runtime.handler_state_16b0, 1);
        assert!(runtime.marker_started_16bb);
        assert!(runtime.marker_active_16bc);

        let fade_out = runtime.advance_handler(RobotsCutsceneHandlerHostInput {
            trigger_flags: 0,
            fade_available: true,
            fade_low_reached: false,
            fade_high_reached: true,
            creator_script_present: true,
        });
        assert_eq!(runtime.handler_state_16b0, 3);
        assert_eq!(
            fade_out.effect,
            Some(RobotsCutsceneHandlerHostEffect::RequestFadeOut {
                duration_updates: 30,
            })
        );

        let blocked = runtime.advance_handler(RobotsCutsceneHandlerHostInput {
            trigger_flags: 0,
            fade_available: true,
            fade_low_reached: false,
            fade_high_reached: false,
            creator_script_present: true,
        });
        assert!(blocked.blocked_on_fade);
        assert!(!runtime.teardown_complete_16ba);

        let finalized = runtime.advance_handler(RobotsCutsceneHandlerHostInput {
            trigger_flags: 0,
            fade_available: true,
            fade_low_reached: true,
            fade_high_reached: false,
            creator_script_present: true,
        });
        assert_eq!(
            finalized.effect,
            Some(RobotsCutsceneHandlerHostEffect::FinalizeCutscene)
        );
        assert_eq!(runtime.handler_state_16b0, 3);
        assert!(!runtime.teardown_complete_16ba);
        runtime.complete_ordinary_finalize();
        assert_eq!(runtime.handler_state_16b0, 6);
        assert!(runtime.teardown_complete_16ba);
        assert_eq!(
            runtime.on_state_marker(245),
            RobotsCutsceneStateMarkerEventResult::Continue
        );
        assert!(runtime.script_complete_16b8);
    }

    #[test]
    fn trigger_flag_bit_one_skips_gamewnd_fade_but_keeps_one_handler_tick_boundary() {
        let script = marker_script(vec![state_marker(7, u32::MAX)]);
        let mut runtime = RobotsCutsceneStateMarkerRuntime::default();
        runtime.bind_script(&script);
        assert_eq!(
            runtime.on_state_marker(7),
            RobotsCutsceneStateMarkerEventResult::Hold
        );
        let first = runtime.advance_handler(RobotsCutsceneHandlerHostInput {
            trigger_flags: 2,
            fade_available: true,
            fade_low_reached: false,
            fade_high_reached: true,
            creator_script_present: true,
        });
        assert_eq!(first.effect, None);
        assert_eq!(runtime.handler_state_16b0, 3);
        let second = runtime.advance_handler(RobotsCutsceneHandlerHostInput {
            trigger_flags: 2,
            fade_available: true,
            fade_low_reached: false,
            fade_high_reached: true,
            creator_script_present: true,
        });
        assert_eq!(
            second.effect,
            Some(RobotsCutsceneHandlerHostEffect::FinalizeCutscene)
        );
    }

    fn finalize_input() -> RobotsCutsceneFinalizeInput {
        RobotsCutsceneFinalizeInput {
            marker_time_bits_3a0: (245.0f32 / 30.0).to_bits(),
            audio_active_16c3: false,
            music_audio_present_16c4: false,
            creator_flags_e8: 0,
            player_handler_state_6de: None,
            saved_display_mask_16b4: 0x35,
            active_display_mask: 0x17,
            pending_swap_character_16bd: false,
            player_mode: None,
        }
    }

    #[test]
    fn finalize_preamble_preserves_marker_time_and_optional_cutscene_audio_stop() {
        let marker_time_bits = (245.0f32 / 30.0).to_bits();
        let plan = plan_cutscene_finalize(RobotsCutsceneFinalizeInput {
            marker_time_bits_3a0: marker_time_bits,
            audio_active_16c3: true,
            music_audio_present_16c4: true,
            ..finalize_input()
        });
        assert_eq!(
            plan.effects[0],
            RobotsCutsceneFinalizeEffect::FinalizeScriptAnimatorTail {
                marker_time_bits_3a0: marker_time_bits,
                music_audio_present_16c4: true,
            }
        );
        assert_eq!(
            plan.effects[1],
            RobotsCutsceneFinalizeEffect::StopAndClearCutsceneAudio
        );
        assert_eq!(
            plan.effects[2],
            RobotsCutsceneFinalizeEffect::SetMusicVolumePercent { percent: 100 }
        );
        assert_eq!(
            plan.effects[3],
            RobotsCutsceneFinalizeEffect::SetGameStateTop { state: 3 }
        );
    }

    #[test]
    fn finalize_change_level_branch_has_native_priority_and_exact_dispatch() {
        let plan = plan_cutscene_finalize(RobotsCutsceneFinalizeInput {
            creator_flags_e8: 0x06,
            player_handler_state_6de: Some(0x1d),
            ..finalize_input()
        });
        assert_eq!(plan.branch, RobotsCutsceneFinalizeBranch::ChangeLevel);
        assert!(plan.effects.contains(
            &RobotsCutsceneFinalizeEffect::RefreshPlayerAudioAndRestoreHealth {
                restore_full_health: true,
            }
        ));
        assert_eq!(
            plan.effects.last(),
            Some(&RobotsCutsceneFinalizeEffect::DispatchFirstTriggerType {
                target: RobotsCutsceneFinalizeDispatchTarget::ChangeLevel,
                event_mask: 0x100,
            })
        );
        assert!(!plan.effects.iter().any(|effect| matches!(
            effect,
            RobotsCutsceneFinalizeEffect::DispatchFirstTriggerType {
                target: RobotsCutsceneFinalizeDispatchTarget::Cutscene,
                ..
            }
        )));
    }

    #[test]
    fn finalize_linked_cutscene_and_special_player_branches_stay_distinct() {
        let linked = plan_cutscene_finalize(RobotsCutsceneFinalizeInput {
            creator_flags_e8: 0x04,
            player_handler_state_6de: Some(0x3e),
            ..finalize_input()
        });
        assert_eq!(linked.branch, RobotsCutsceneFinalizeBranch::LinkedCutscene);
        assert_eq!(
            linked.effects.last(),
            Some(&RobotsCutsceneFinalizeEffect::DispatchFirstTriggerType {
                target: RobotsCutsceneFinalizeDispatchTarget::Cutscene,
                event_mask: 0x1,
            })
        );
        assert!(!linked
            .effects
            .contains(&RobotsCutsceneFinalizeEffect::RestoreGameplayAfterSpecialPlayerState));

        let player = plan_cutscene_finalize(RobotsCutsceneFinalizeInput {
            player_handler_state_6de: Some(0x2f),
            ..finalize_input()
        });
        assert_eq!(
            player.branch,
            RobotsCutsceneFinalizeBranch::SpecialPlayerState
        );
        assert_eq!(
            player.effects.last(),
            Some(&RobotsCutsceneFinalizeEffect::RestoreGameplayAfterSpecialPlayerState)
        );
    }

    #[test]
    fn finalize_ordinary_branch_restores_only_saved_active_display_bits_and_consumes_swap() {
        let plan = plan_cutscene_finalize(RobotsCutsceneFinalizeInput {
            pending_swap_character_16bd: true,
            player_mode: Some(1),
            ..finalize_input()
        });
        assert_eq!(plan.branch, RobotsCutsceneFinalizeBranch::Ordinary);
        assert!(plan
            .effects
            .contains(&RobotsCutsceneFinalizeEffect::DestroyDisplayMask { mask: 0x8 }));
        assert!(plan
            .effects
            .contains(&RobotsCutsceneFinalizeEffect::RestoreDisplayMask { mask: 0x15 }));
        assert_eq!(
            plan.effects.last(),
            Some(&RobotsCutsceneFinalizeEffect::ConsumePendingSwapCharacter {
                player_mode: Some(1),
            })
        );
    }

    #[test]
    fn finalize_host_runtime_applies_routed_release_without_leaking_state3_or_message_box() {
        let input = RobotsCutsceneFinalizeInput {
            creator_flags_e8: 0x02,
            player_handler_state_6de: Some(0x1d),
            ..finalize_input()
        };
        let plan = plan_cutscene_finalize(input);
        let mut host = RobotsCutsceneHostRuntimeState {
            music_volume_percent: 25,
            game_state_stack: vec![7],
            display_mask: 0x1f,
            global_cutscene_active: true,
            mission_owner_present: false,
            player_health: Some(RobotsCutscenePlayerHealthState {
                current: 25.0,
                max: 100.0,
            }),
            ..RobotsCutsceneHostRuntimeState::default()
        };
        for effect in plan.effects.iter().copied() {
            host.apply_finalize_effect(effect);
        }
        assert_eq!(host.music_volume_percent, 100);
        assert_eq!(host.game_state_stack, Vec::<u32>::new());
        assert_eq!(host.display_mask, 0x17);
        assert_eq!(host.player_health.unwrap().current, 100.0);
        assert_eq!(host.player_audio_refresh_requests, 1);
    }

    #[test]
    fn finalize_host_runtime_keeps_ordinary_state3_and_applies_display_restore() {
        let plan = plan_cutscene_finalize(finalize_input());
        let mut host = RobotsCutsceneHostRuntimeState {
            game_state_stack: vec![9],
            display_mask: 0x1f,
            global_cutscene_active: true,
            mission_owner_present: false,
            ..RobotsCutsceneHostRuntimeState::default()
        };
        for effect in plan.effects.iter().copied() {
            host.apply_finalize_effect(effect);
        }
        assert_eq!(host.game_state_stack, vec![3]);
        assert_eq!(host.display_mask, 0x17);
        assert!(!host.global_cutscene_active);
        assert_eq!(host.last_creator_entity_policy, Some([1, 0, 0]));
    }

    #[test]
    fn native_script_selection_prefers_handler_alternate_without_changing_owner() {
        let selection = select_cutscene_script(
            0x0100_00BB,
            Some(0x0100_00C9),
            Some(0x0400_028E),
            Some(0x0400_0291),
        )
        .unwrap();
        assert_eq!(selection.owner_file_uid, 0x0100_00C9);
        assert_eq!(selection.default_script_uid, Some(0x0400_028E));
        assert_eq!(selection.selected_script_uid, 0x0400_0291);
        assert!(selection.alternate_applied);
    }

    #[test]
    fn native_script_selection_falls_back_to_default_and_current_file() {
        let selection = select_cutscene_script(0x0100_00BB, None, Some(0x0400_028E), None).unwrap();
        assert_eq!(selection.owner_file_uid, 0x0100_00BB);
        assert_eq!(selection.selected_script_uid, 0x0400_028E);
        assert!(!selection.alternate_applied);
        assert!(select_cutscene_script(1, None, None, None).is_none());
    }

    #[test]
    fn setproperties_cutscene_music_volume_uses_strict_native_zero_to_hundred_bounds() {
        assert_eq!(cutscene_music_volume_percent(Some(0)), None);
        assert_eq!(cutscene_music_volume_percent(Some(1)), Some(1));
        assert_eq!(cutscene_music_volume_percent(Some(99)), Some(99));
        assert_eq!(cutscene_music_volume_percent(Some(100)), None);
        assert_eq!(cutscene_music_volume_percent(Some(u32::MAX)), None);
    }

    #[test]
    fn effect_plan_decodes_exact_perform_action_mapping() {
        let semantic = RobotsHandlerScriptCommandSemantic::CutsceneCommand {
            command: RobotsCutsceneCommandKind::PerformActionCutscene,
            args: [Some(4), None, None, None, None, None, None, None],
            start: Some(12),
            length: None,
        };
        assert_eq!(
            plan_cutscene_effect(&semantic),
            Some(RobotsCutsceneEffect::PerformActionCutscene {
                raw_mode: Some(4),
                action: Some(RobotsCutscenePlayerAction::Action48000008),
            })
        );
    }

    #[test]
    fn effect_plan_preserves_native_minus_one_toggle_semantics() {
        let semantic = RobotsHandlerScriptCommandSemantic::CutsceneCommand {
            command: RobotsCutsceneCommandKind::SetPropertiesCutscene,
            args: [
                Some(u32::MAX),
                Some(1),
                Some(7),
                None,
                None,
                None,
                None,
                None,
            ],
            start: None,
            length: None,
        };
        assert_eq!(
            plan_cutscene_effect(&semantic),
            Some(RobotsCutsceneEffect::SetPropertiesCutscene {
                cutscene_enabled: RobotsCutsceneToggle::Keep,
                message_enabled: RobotsCutsceneToggle::Enabled,
                raw_property: Some(7),
                music_volume_percent: Some(7),
            })
        );
    }

    #[test]
    fn m10_delegated_effects_decode_exact_relay_and_camera_shake_contracts() {
        assert_eq!(
            plan_cutscene_effect(&RobotsHandlerScriptCommandSemantic::MessageRelaySpecific {
                arg0: Some(1),
                arg1: Some(0xFC),
            }),
            Some(RobotsCutsceneEffect::MessageRelaySpecific {
                event_mask: Some(1),
                link_mask: Some(0xFC),
            })
        );
        assert_eq!(
            plan_cutscene_effect(&RobotsHandlerScriptCommandSemantic::ShakeCamera {
                arg0: Some(10.0),
                arg1: Some(20.0),
                flags: Some(1),
            }),
            Some(RobotsCutsceneEffect::ShakeCamera {
                magnitude: Some(10),
                duration_updates: Some(20),
                primary_channel: Some(true),
                player_channel: Some(false),
                raw_flags: Some(1),
            })
        );
    }

    #[test]
    fn m10_show_message_preserves_length_gate_and_extended_native_arguments() {
        let semantic = RobotsHandlerScriptCommandSemantic::CutsceneCommand {
            command: RobotsCutsceneCommandKind::ShowMessage,
            args: [
                Some(0x4500_034E),
                Some(0.0f32.to_bits()),
                Some(0),
                Some(0),
                Some(u32::MAX),
                Some(u32::MAX),
                None,
                None,
            ],
            start: Some(200),
            length: Some(152),
        };
        assert_eq!(
            plan_cutscene_effect(&semantic),
            Some(RobotsCutsceneEffect::ShowMessage {
                message_uid: Some(0x4500_034E),
                parameter_08: Some(0),
                use_script_length: Some(true),
                parameter_10: Some(-1),
                parameter_14: Some(-1),
                start: Some(200),
                length: Some(152),
            })
        );
    }

    #[test]
    fn m10_perform_action_modes_zero_and_three_map_to_native_player_actions() {
        for (mode, expected) in [
            (0, RobotsCutscenePlayerAction::Action48000000),
            (3, RobotsCutscenePlayerAction::Action48000005),
        ] {
            let semantic = RobotsHandlerScriptCommandSemantic::CutsceneCommand {
                command: RobotsCutsceneCommandKind::PerformActionCutscene,
                args: [Some(mode), None, None, None, None, None, None, None],
                start: Some(0),
                length: Some(0x4B),
            };
            assert_eq!(
                plan_cutscene_effect(&semantic),
                Some(RobotsCutsceneEffect::PerformActionCutscene {
                    raw_mode: Some(mode as i32),
                    action: Some(expected),
                })
            );
        }
    }
}
