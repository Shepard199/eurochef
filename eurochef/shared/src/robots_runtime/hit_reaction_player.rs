use serde::Serialize;

pub const ROBOTS_PLAYER_HIT_EPSILON: f32 = 0.001;
pub const ROBOTS_PLAYER_HIT_HEALTH_FRACTION: f32 = 0.2;
pub const ROBOTS_PLAYER_HIT_WINDOW: f32 = 5.0;
pub const ROBOTS_PLAYER_BALL_HIT_WINDOW: f32 = 3.0;
pub const ROBOTS_PLAYER_REACTION_FIXED_STEP_SECONDS: f32 = 1.0 / 60.0;
pub const ROBOTS_PLAYER_DEFAULT_MAX_HEALTH: f32 = 100.0;

pub const ROBOTS_PLAYER_HIT_FLAG_MODE_1: u32 = 0x0000_0002;
pub const ROBOTS_PLAYER_HIT_FLAG_MODE_2: u32 = 0x0000_0004;
pub const ROBOTS_PLAYER_HIT_FLAG_PLAYER_REJECT: u32 = 0x0000_0008;
pub const ROBOTS_PLAYER_HIT_FLAG_MODE_4: u32 = 0x0000_1000;

pub const ROBOTS_PLAYER_STATE_REACTION_EXEMPT_A: u8 = 0x1D;
pub const ROBOTS_PLAYER_STATE_REACTION_EXEMPT_B: u8 = 0x3E;

pub const ROBOTS_PLAYER_BALL_FLAG_1000_EFFECT_SCRIPT_UID: u32 = 0x0400_02F7;
pub const ROBOTS_PLAYER_BALL_FLAG_1000_EFFECT_DATUM_UID: u32 = 0x1000_004A;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsPlayerHitContext250 {
    /// Native `DAT_007B2C80 +0x250`.
    pub mode_250: i32,
    /// Native `DAT_007B2C80 +0xD4`.
    pub uid_d4: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsPlayerBaseHitGateInput {
    /// Top value of the process game-state stack (`+0x3B4/+0x3B8`), or 0 when empty.
    pub top_game_state: u32,
    /// `GameWnd +0x50F / DAT_007B3207`.
    pub game_control_mode_50f: u8,
    /// Raw `DAT_007B3021` branch used by `0x004B0B10`.
    pub raw_gate_7b3021: bool,
    /// Optional global context read by `0x004B0B10`.
    pub context_7b2c80: Option<RobotsPlayerHitContext250>,
    /// Player Handler `+0x460` fallback blocker used by `0x004B0B10`.
    pub field_460_nonzero: bool,
    /// Player Handler `+0x6DE`.
    pub player_state_6de: u8,
    /// Player Handler `+0x6E8` transient veto in `0x004BA750`.
    pub field_6e8_nonzero: bool,
}

/// Exact boolean portion of Player vslot `+0x134 = 0x004BA750`, including its
/// helper `0x004B0B10`. The unrelated `0x0042A670(3)` registration-side effect
/// is intentionally host-owned and does not affect this admission result.
pub fn robots_player_base_hit_gate(input: RobotsPlayerBaseHitGateInput) -> bool {
    if matches!(input.top_game_state, 1 | 2 | 3) {
        return false;
    }
    if input.game_control_mode_50f == 1 {
        return false;
    }

    let b0b10_blocked = if matches!(input.game_control_mode_50f, 2 | 3 | 4) || input.raw_gate_7b3021
    {
        matches!(input.top_game_state, 1 | 2 | 3)
    } else {
        matches!(input.top_game_state, 2 | 3)
    } || matches!(input.top_game_state, 0x0b | 0x0f)
        || input.context_7b2c80.is_some_and(|context| {
            !matches!(context.mode_250, 2 | 7) || context.uid_d4 == 0x0400_006c
        })
        || input.field_460_nonzero;
    if b0b10_blocked {
        return false;
    }

    if matches!(
        input.player_state_6de,
        0x01 | 0x1a | 0x1d | 0x36 | 0x37 | 0x38 | 0x3a | 0x3c | 0x3d | 0x3e
    ) {
        return false;
    }
    !input.field_6e8_nonzero
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsPlayerHitRuntimeState {
    pub current_health: f32,
    pub max_health: f32,
    pub reaction_window: f32,
    /// Native debug/god-mode byte `DAT_007B31EF`.
    pub suppress_health_damage_7b31ef: bool,
}

impl Default for RobotsPlayerHitRuntimeState {
    fn default() -> Self {
        Self {
            // `0x00411F90` seeds +0x384=0 and +0x388=100; Player Initialise
            // `0x004AD6E0` copies max -> current before gameplay begins.
            current_health: ROBOTS_PLAYER_DEFAULT_MAX_HEALTH,
            max_health: ROBOTS_PLAYER_DEFAULT_MAX_HEALTH,
            reaction_window: 0.0,
            suppress_health_damage_7b31ef: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsPlayerHitReactionMode {
    None,
    Mode1,
    Mode2,
    Mode4,
}

impl RobotsPlayerHitReactionMode {
    pub fn from_query_flags(flags: u32) -> Self {
        if flags & ROBOTS_PLAYER_HIT_FLAG_MODE_1 != 0 {
            Self::Mode1
        } else if flags & ROBOTS_PLAYER_HIT_FLAG_MODE_2 != 0 {
            Self::Mode2
        } else if flags & ROBOTS_PLAYER_HIT_FLAG_MODE_4 != 0 {
            Self::Mode4
        } else {
            Self::None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsPlayerHitInput {
    /// Exact result of Player vslot +0x134. Its higher-level semantic name is
    /// intentionally left to the future Player subsystem; false rejects +0xC8.
    pub base_hit_gate: bool,
    pub query_flags: u32,
    /// Native global DAT_007B31EF. When true, health subtraction is skipped.
    pub suppress_health_damage: bool,
    pub current_health: f32,
    pub max_health: f32,
    /// Native Handler +0x6E4.
    pub reaction_window: f32,
    /// Native Player state byte +0x6DE.
    pub player_state: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsPlayerHitResult {
    pub reaction_accepted: bool,
    pub query_hit_still_commits: bool,
    pub current_health: f32,
    pub reaction_window: f32,
    pub reaction_mode: RobotsPlayerHitReactionMode,
    pub reaction_window_rearmed: bool,
    pub request_death: bool,
}

/// Player fixed service `0x004B7A00` owns Handler `+0x6E4`. While positive it
/// subtracts `runtime_rate_scale * 1/60`; once the result reaches the native
/// epsilon, Player Handler `+0x37C` is reset to query serial 0.
///
/// Returns true when the host must perform that serial reset. Native leaves the
/// post-subtraction float as-is, including a small negative value, so this helper
/// deliberately does not clamp it to zero.
pub fn advance_player_reaction_window_fixed(
    reaction_window: &mut f32,
    runtime_rate_scale: f32,
) -> bool {
    if *reaction_window <= 0.0 {
        return false;
    }
    *reaction_window -= runtime_rate_scale * ROBOTS_PLAYER_REACTION_FIXED_STEP_SECONDS;
    *reaction_window <= ROBOTS_PLAYER_HIT_EPSILON
}

/// `XItemHandler_Player::+0xC8 = 0x004BA8E0`.
///
/// Native health damage is exactly 0.2 * maxHealth and is only applied while the
/// existing reaction window is <= 0.001. The outer hit-query still commits when
/// this callback rejects because 0x00425C70 ignores the callback return value.
pub fn robots_apply_player_hit(input: RobotsPlayerHitInput) -> RobotsPlayerHitResult {
    if !input.base_hit_gate || input.query_flags & ROBOTS_PLAYER_HIT_FLAG_PLAYER_REJECT != 0 {
        return RobotsPlayerHitResult {
            reaction_accepted: false,
            query_hit_still_commits: true,
            current_health: input.current_health,
            reaction_window: input.reaction_window,
            reaction_mode: RobotsPlayerHitReactionMode::None,
            reaction_window_rearmed: false,
            request_death: false,
        };
    }

    let window_ready = input.reaction_window <= ROBOTS_PLAYER_HIT_EPSILON;
    let mut current_health = input.current_health;
    if !input.suppress_health_damage && window_ready {
        current_health -= input.max_health * ROBOTS_PLAYER_HIT_HEALTH_FRACTION;
    }
    let request_death = current_health < ROBOTS_PLAYER_HIT_EPSILON;

    let player_state_allows_reaction = input.player_state != ROBOTS_PLAYER_STATE_REACTION_EXEMPT_A
        && input.player_state != ROBOTS_PLAYER_STATE_REACTION_EXEMPT_B;
    let reaction_window_rearmed = window_ready && player_state_allows_reaction;
    let (reaction_window, reaction_mode) = if reaction_window_rearmed {
        (
            ROBOTS_PLAYER_HIT_WINDOW,
            RobotsPlayerHitReactionMode::from_query_flags(input.query_flags),
        )
    } else {
        (input.reaction_window, RobotsPlayerHitReactionMode::None)
    };

    RobotsPlayerHitResult {
        reaction_accepted: true,
        query_hit_still_commits: true,
        current_health,
        reaction_window,
        reaction_mode,
        reaction_window_rearmed,
        request_death,
    }
}

impl RobotsPlayerHitRuntimeState {
    pub fn advance_fixed(&mut self, runtime_rate_scale: f32) -> bool {
        advance_player_reaction_window_fixed(&mut self.reaction_window, runtime_rate_scale)
    }

    pub fn apply_hit(
        &mut self,
        base_hit_gate: bool,
        query_flags: u32,
        player_state: u8,
    ) -> RobotsPlayerHitResult {
        let result = robots_apply_player_hit(RobotsPlayerHitInput {
            base_hit_gate,
            query_flags,
            suppress_health_damage: self.suppress_health_damage_7b31ef,
            current_health: self.current_health,
            max_health: self.max_health,
            reaction_window: self.reaction_window,
            player_state,
        });
        self.current_health = result.current_health;
        self.reaction_window = result.reaction_window;
        result
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsPlayerBallHitBranch {
    Death,
    Flag2Vslot158Zero,
    Flag4Vslot15cZero,
    Flag8Vslot168,
    Flag1000EffectThenVslot164,
    DefaultVslot160,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsPlayerBallHitInput {
    pub base_hit_gate: bool,
    pub query_flags: u32,
    pub suppress_health_damage: bool,
    pub current_health: f32,
    pub max_health: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsPlayerBallHitResult {
    pub reaction_accepted: bool,
    pub query_hit_still_commits: bool,
    pub current_health: f32,
    pub reaction_window: f32,
    pub reaction_mode: RobotsPlayerHitReactionMode,
    pub branch: Option<RobotsPlayerBallHitBranch>,
    pub effect_script_uid: Option<u32>,
    pub effect_datum_uid: Option<u32>,
}

/// PlayerBall family `+0xC8 = 0x004B4A60`.
///
/// Unlike normal Player, PlayerBall always arms a fresh 3.0 reaction window
/// before damage and does not gate the 20% health subtraction on that window.
pub fn robots_apply_player_ball_hit(input: RobotsPlayerBallHitInput) -> RobotsPlayerBallHitResult {
    if !input.base_hit_gate {
        return RobotsPlayerBallHitResult {
            reaction_accepted: false,
            query_hit_still_commits: true,
            current_health: input.current_health,
            reaction_window: 0.0,
            reaction_mode: RobotsPlayerHitReactionMode::None,
            branch: None,
            effect_script_uid: None,
            effect_datum_uid: None,
        };
    }

    let reaction_mode = RobotsPlayerHitReactionMode::from_query_flags(input.query_flags);
    let mut current_health = input.current_health;
    if !input.suppress_health_damage {
        current_health -= input.max_health * ROBOTS_PLAYER_HIT_HEALTH_FRACTION;
    }

    let (branch, effect_script_uid, effect_datum_uid) =
        if current_health < ROBOTS_PLAYER_HIT_EPSILON {
            (RobotsPlayerBallHitBranch::Death, None, None)
        } else if input.query_flags & ROBOTS_PLAYER_HIT_FLAG_MODE_1 != 0 {
            (RobotsPlayerBallHitBranch::Flag2Vslot158Zero, None, None)
        } else if input.query_flags & ROBOTS_PLAYER_HIT_FLAG_MODE_2 != 0 {
            (RobotsPlayerBallHitBranch::Flag4Vslot15cZero, None, None)
        } else if input.query_flags & ROBOTS_PLAYER_HIT_FLAG_PLAYER_REJECT != 0 {
            (RobotsPlayerBallHitBranch::Flag8Vslot168, None, None)
        } else if input.query_flags & ROBOTS_PLAYER_HIT_FLAG_MODE_4 != 0 {
            (
                RobotsPlayerBallHitBranch::Flag1000EffectThenVslot164,
                Some(ROBOTS_PLAYER_BALL_FLAG_1000_EFFECT_SCRIPT_UID),
                Some(ROBOTS_PLAYER_BALL_FLAG_1000_EFFECT_DATUM_UID),
            )
        } else {
            (RobotsPlayerBallHitBranch::DefaultVslot160, None, None)
        };

    RobotsPlayerBallHitResult {
        reaction_accepted: true,
        query_hit_still_commits: true,
        current_health,
        reaction_window: ROBOTS_PLAYER_BALL_HIT_WINDOW,
        reaction_mode,
        branch: Some(branch),
        effect_script_uid,
        effect_datum_uid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player() -> RobotsPlayerHitInput {
        RobotsPlayerHitInput {
            base_hit_gate: true,
            query_flags: 0,
            suppress_health_damage: false,
            current_health: 100.0,
            max_health: 100.0,
            reaction_window: 0.0,
            player_state: 0,
        }
    }

    #[test]
    fn player_reaction_window_fixed_decay_matches_native_and_requests_serial_zero() {
        let mut window =
            ROBOTS_PLAYER_REACTION_FIXED_STEP_SECONDS + ROBOTS_PLAYER_HIT_EPSILON * 0.5;
        assert!(advance_player_reaction_window_fixed(&mut window, 1.0));
        assert!(window <= ROBOTS_PLAYER_HIT_EPSILON);

        assert!(advance_player_reaction_window_fixed(&mut window, 1.0));
        assert!(window <= 0.0);
        let expired = window;
        assert!(!advance_player_reaction_window_fixed(&mut window, 1.0));
        assert_eq!(window.to_bits(), expired.to_bits());
    }

    #[test]
    fn player_reject_gates_do_not_mutate_health_or_window_but_query_still_commits() {
        let mut input = player();
        input.base_hit_gate = false;
        let result = robots_apply_player_hit(input);
        assert!(!result.reaction_accepted);
        assert!(result.query_hit_still_commits);
        assert_eq!(result.current_health, 100.0);

        let mut input = player();
        input.query_flags = ROBOTS_PLAYER_HIT_FLAG_PLAYER_REJECT;
        assert!(!robots_apply_player_hit(input).reaction_accepted);
    }

    #[test]
    fn player_ready_window_takes_exact_twenty_percent_max_health_and_rearms_five() {
        let result = robots_apply_player_hit(player());
        assert!(result.reaction_accepted);
        assert_eq!(result.current_health, 80.0);
        assert_eq!(result.reaction_window, ROBOTS_PLAYER_HIT_WINDOW);
        assert!(result.reaction_window_rearmed);
        assert_eq!(result.reaction_mode, RobotsPlayerHitReactionMode::None);
    }

    #[test]
    fn player_active_window_suppresses_repeat_damage_and_rearm() {
        let mut input = player();
        input.reaction_window = 1.0;
        let result = robots_apply_player_hit(input);
        assert_eq!(result.current_health, 100.0);
        assert_eq!(result.reaction_window, 1.0);
        assert!(!result.reaction_window_rearmed);
    }

    #[test]
    fn player_global_suppression_skips_health_but_not_reaction_window() {
        let mut input = player();
        input.suppress_health_damage = true;
        input.query_flags = ROBOTS_PLAYER_HIT_FLAG_MODE_2;
        let result = robots_apply_player_hit(input);
        assert_eq!(result.current_health, 100.0);
        assert_eq!(result.reaction_window, 5.0);
        assert_eq!(result.reaction_mode, RobotsPlayerHitReactionMode::Mode2);
    }

    #[test]
    fn player_exempt_states_do_not_rearm_reaction_window() {
        for player_state in [
            ROBOTS_PLAYER_STATE_REACTION_EXEMPT_A,
            ROBOTS_PLAYER_STATE_REACTION_EXEMPT_B,
        ] {
            let mut input = player();
            input.player_state = player_state;
            let result = robots_apply_player_hit(input);
            assert_eq!(result.current_health, 80.0);
            assert_eq!(result.reaction_window, 0.0);
            assert!(!result.reaction_window_rearmed);
        }
    }

    #[test]
    fn player_death_is_requested_after_native_float_subtraction() {
        let mut input = player();
        input.current_health = 10.0;
        let result = robots_apply_player_hit(input);
        assert_eq!(result.current_health, -10.0);
        assert!(result.request_death);
    }

    fn ball(flags: u32) -> RobotsPlayerBallHitInput {
        RobotsPlayerBallHitInput {
            base_hit_gate: true,
            query_flags: flags,
            suppress_health_damage: false,
            current_health: 100.0,
            max_health: 100.0,
        }
    }

    #[test]
    fn player_ball_always_arms_three_and_takes_twenty_percent_when_damage_enabled() {
        let result = robots_apply_player_ball_hit(ball(0));
        assert!(result.reaction_accepted);
        assert_eq!(result.current_health, 80.0);
        assert_eq!(result.reaction_window, ROBOTS_PLAYER_BALL_HIT_WINDOW);
        assert_eq!(
            result.branch,
            Some(RobotsPlayerBallHitBranch::DefaultVslot160)
        );
    }

    #[test]
    fn player_ball_flag_priority_and_mode_match_native_order() {
        let flags = ROBOTS_PLAYER_HIT_FLAG_MODE_1
            | ROBOTS_PLAYER_HIT_FLAG_MODE_2
            | ROBOTS_PLAYER_HIT_FLAG_PLAYER_REJECT
            | ROBOTS_PLAYER_HIT_FLAG_MODE_4;
        let result = robots_apply_player_ball_hit(ball(flags));
        assert_eq!(result.reaction_mode, RobotsPlayerHitReactionMode::Mode1);
        assert_eq!(
            result.branch,
            Some(RobotsPlayerBallHitBranch::Flag2Vslot158Zero)
        );
    }

    #[test]
    fn player_ball_flag1000_emits_exact_effect_plan() {
        let result = robots_apply_player_ball_hit(ball(ROBOTS_PLAYER_HIT_FLAG_MODE_4));
        assert_eq!(
            result.branch,
            Some(RobotsPlayerBallHitBranch::Flag1000EffectThenVslot164)
        );
        assert_eq!(
            result.effect_script_uid,
            Some(ROBOTS_PLAYER_BALL_FLAG_1000_EFFECT_SCRIPT_UID)
        );
        assert_eq!(
            result.effect_datum_uid,
            Some(ROBOTS_PLAYER_BALL_FLAG_1000_EFFECT_DATUM_UID)
        );
    }

    #[test]
    fn player_ball_death_preempts_class_specific_response_branch() {
        let mut input = ball(ROBOTS_PLAYER_HIT_FLAG_MODE_1);
        input.current_health = 10.0;
        let result = robots_apply_player_ball_hit(input);
        assert_eq!(result.current_health, -10.0);
        assert_eq!(result.branch, Some(RobotsPlayerBallHitBranch::Death));
    }
}
