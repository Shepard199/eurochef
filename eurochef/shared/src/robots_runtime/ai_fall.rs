use serde::Serialize;

pub const ROBOTS_AI_FALL_PRIORITY: u8 = 0x82;
pub const ROBOTS_AI_FALL_FALLBACK_HEIGHT: f32 = 99_999.0;
pub const ROBOTS_AI_FALL_ANIM_MODE_MOVE: u32 = 0x0900_0003;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsAiFallConfig {
    pub anim_mode: u32,
    pub height_threshold: f32,
    pub priority: u8,
}

impl RobotsAiFallConfig {
    /// EQ04 builder `0x00463390 -> 0x0046B000`.
    pub const fn eq04_mine() -> Self {
        Self {
            anim_mode: ROBOTS_AI_FALL_ANIM_MODE_MOVE,
            height_threshold: 0.5,
            priority: ROBOTS_AI_FALL_PRIORITY,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsAiFallRuntimeState {
    /// Native node+0x50. `0x0046B0F0` sets it on leave, making the behavior one-shot.
    pub completed_once: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsAiFallGateInput {
    pub owner_y: f32,
    /// Native Handler+0x2C0 bit0 selects Handler+0x1C0 as the reference Y.
    /// `None` represents bit0 clear and therefore the exact 99999.0 fallback.
    pub reference_y: Option<f32>,
}

pub fn ai_fall_height(input: RobotsAiFallGateInput) -> f32 {
    input
        .reference_y
        .map_or(ROBOTS_AI_FALL_FALLBACK_HEIGHT, |reference_y| {
            input.owner_y - reference_y
        })
}

/// Native gate `0x0046B070`. The threshold is inclusive; after the node has left
/// once, node+0x50 suppresses it permanently by returning base priority 1.
pub fn ai_fall_priority(
    state: RobotsAiFallRuntimeState,
    config: RobotsAiFallConfig,
    input: RobotsAiFallGateInput,
) -> u8 {
    if state.completed_once {
        return 1;
    }
    if ai_fall_height(input) >= config.height_threshold {
        config.priority
    } else {
        1
    }
}

/// Native leave `0x0046B0F0 -> 0x00456F10` plus node+0x50=1.
pub fn leave_ai_fall(state: &mut RobotsAiFallRuntimeState) {
    state.completed_once = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fall_gate_keeps_native_fallback_threshold_and_one_shot_latch() {
        let config = RobotsAiFallConfig::eq04_mine();
        let mut state = RobotsAiFallRuntimeState::default();
        assert_eq!(
            ai_fall_priority(
                state,
                config,
                RobotsAiFallGateInput {
                    owner_y: 10.0,
                    reference_y: None,
                },
            ),
            0x82
        );
        assert_eq!(
            ai_fall_priority(
                state,
                config,
                RobotsAiFallGateInput {
                    owner_y: 10.5,
                    reference_y: Some(10.0),
                },
            ),
            0x82
        );
        assert_eq!(
            ai_fall_priority(
                state,
                config,
                RobotsAiFallGateInput {
                    owner_y: 10.499,
                    reference_y: Some(10.0),
                },
            ),
            1
        );
        leave_ai_fall(&mut state);
        assert_eq!(
            ai_fall_priority(
                state,
                config,
                RobotsAiFallGateInput {
                    owner_y: 100.0,
                    reference_y: None,
                },
            ),
            1
        );
    }
}
