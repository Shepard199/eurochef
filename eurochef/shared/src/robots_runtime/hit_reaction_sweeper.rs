use serde::Serialize;

pub const ROBOTS_SWEEPER_EYE_STATE_OPEN: u8 = 3;
pub const ROBOTS_SWEEPER_EYE_STATE_HIT: u8 = 5;
pub const ROBOTS_SWEEPER_EYE_STATE_DESTROYED: u8 = 7;
pub const ROBOTS_SWEEPER_EYE_HIT_SCRIPT_UID: u32 = 0x0400_025C;
pub const ROBOTS_SWEEPER_EYE_DESTROYED_SCRIPT_UID: u32 = 0x0400_025D;
pub const ROBOTS_SWEEPER_EYE_DESTROY_EFFECT_UID: u32 = 0x1AF0_044A;

pub const ROBOTS_SWEEPER_RATCHET_STATE_HIT_RECOVERY: u8 = 9;
pub const ROBOTS_SWEEPER_RATCHET_STATE_DEATH: u8 = 10;
pub const ROBOTS_SWEEPER_RATCHET_HIT_FORWARD_THRESHOLD: u8 = 70;
pub const ROBOTS_SWEEPER_RATCHET_HIT_FORWARD_ANIM_MODE: u32 = 0x0900_0029;
pub const ROBOTS_SWEEPER_RATCHET_HIT_BACK_ANIM_MODE: u32 = 0x0900_002A;
pub const ROBOTS_SWEEPER_RATCHET_DEATH_ANIM_MODE: u32 = 0x0900_0065;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsSweeperEyeHitInput {
    pub state: u8,
    pub hit_points: u8,
    pub difficulty: u8,
    pub max_difficulty: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsSweeperEyeHitResult {
    pub reaction_accepted: bool,
    pub query_hit_still_commits: bool,
    pub hit_points: u8,
    pub state: u8,
    pub script_uid: Option<u32>,
    pub reset_script_signal: bool,
    pub effect_uid: Option<u32>,
    pub destroyed: bool,
    pub difficulty: u8,
}

/// Exact Sweeper Eye accepted-hit reducer `0x004CF410` after geometric contact.
/// Only state 3 (Open) accepts damage. The native byte HP decrement is wrapping;
/// on an accepted hit the eye switches to Hit or Destroyed, resets its script
/// signal, and increments controller difficulty with the native byte clamp.
pub fn robots_apply_sweeper_eye_hit(input: RobotsSweeperEyeHitInput) -> RobotsSweeperEyeHitResult {
    if input.state != ROBOTS_SWEEPER_EYE_STATE_OPEN {
        return RobotsSweeperEyeHitResult {
            reaction_accepted: false,
            query_hit_still_commits: true,
            hit_points: input.hit_points,
            state: input.state,
            script_uid: None,
            reset_script_signal: false,
            effect_uid: None,
            destroyed: false,
            difficulty: input.difficulty,
        };
    }

    let hit_points = input.hit_points.wrapping_sub(1);
    let destroyed = hit_points == 0;
    let (state, script_uid, effect_uid) = if destroyed {
        (
            ROBOTS_SWEEPER_EYE_STATE_DESTROYED,
            ROBOTS_SWEEPER_EYE_DESTROYED_SCRIPT_UID,
            Some(ROBOTS_SWEEPER_EYE_DESTROY_EFFECT_UID),
        )
    } else {
        (
            ROBOTS_SWEEPER_EYE_STATE_HIT,
            ROBOTS_SWEEPER_EYE_HIT_SCRIPT_UID,
            None,
        )
    };

    RobotsSweeperEyeHitResult {
        reaction_accepted: true,
        query_hit_still_commits: true,
        hit_points,
        state,
        script_uid: Some(script_uid),
        reset_script_signal: true,
        effect_uid,
        destroyed,
        difficulty: input.difficulty.wrapping_add(1).min(input.max_difficulty),
    }
}

/// Native lethal Ratchet path only resolves Player link0 when both the live
/// Player XItem and its creator trigger are present. If neither is present the
/// callback simply skips the dispatch and continues. If a creator exists but
/// link0 is missing, native returns 0 after HP/state/script/AI-cleanup side
/// effects but before difficulty is incremented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsSweeperRatchetPlayerLink0 {
    NoPlayerOrCreator,
    Resolved,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsSweeperRatchetHitInput {
    pub damage_phase_armed: bool,
    pub top_state: u8,
    pub damage_accept_latch: bool,
    pub hit_points: u8,
    /// Native nonlethal path consumes exactly one global RNG draw and uses `%100`.
    /// `None` lets a host query whether that draw is required before mutating state.
    pub hit_random_mod100: Option<u8>,
    pub difficulty: u8,
    pub max_difficulty: u8,
    pub player_link0: RobotsSweeperRatchetPlayerLink0,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsSweeperRatchetHitResult {
    pub reaction_accepted: bool,
    pub query_hit_still_commits: bool,
    pub needs_direction_rng: bool,
    pub destroyed: bool,
    pub hit_points: u8,
    pub top_state: u8,
    pub requested_anim_mode: Option<u32>,
    pub random_draws_used: u8,
    pub cleanup_live_ai_characters: bool,
    pub dispatch_player_trigger_link0_event: bool,
    pub player_link0_resolution_failed: bool,
    pub difficulty: u8,
}

impl RobotsSweeperRatchetHitResult {
    fn rejected(input: RobotsSweeperRatchetHitInput) -> Self {
        Self {
            reaction_accepted: false,
            query_hit_still_commits: true,
            needs_direction_rng: false,
            destroyed: false,
            hit_points: input.hit_points,
            top_state: input.top_state,
            requested_anim_mode: None,
            random_draws_used: 0,
            cleanup_live_ai_characters: false,
            dispatch_player_trigger_link0_event: false,
            player_link0_resolution_failed: false,
            difficulty: input.difficulty,
        }
    }
}

/// Exact state transition core of `XItemHandler_Sweeper_Boss_Ratchet::ApplyHit`
/// (`0x004CFB60`). This reducer deliberately keeps world-list AI cleanup and
/// trigger dispatch as host-visible plans. The outer geometric query remains a
/// hit even if the callback returns 0.
pub fn robots_apply_sweeper_ratchet_hit(
    input: RobotsSweeperRatchetHitInput,
) -> RobotsSweeperRatchetHitResult {
    if !input.damage_phase_armed
        || input.top_state == ROBOTS_SWEEPER_RATCHET_STATE_HIT_RECOVERY
        || !input.damage_accept_latch
    {
        return RobotsSweeperRatchetHitResult::rejected(input);
    }

    let hit_points = input.hit_points.wrapping_sub(1);
    let destroyed = (hit_points as i8) <= 0;

    if !destroyed {
        let Some(hit_random_mod100) = input.hit_random_mod100 else {
            let mut result = RobotsSweeperRatchetHitResult::rejected(input);
            result.needs_direction_rng = true;
            return result;
        };
        let requested_anim_mode =
            if hit_random_mod100 % 100 < ROBOTS_SWEEPER_RATCHET_HIT_FORWARD_THRESHOLD {
                ROBOTS_SWEEPER_RATCHET_HIT_FORWARD_ANIM_MODE
            } else {
                ROBOTS_SWEEPER_RATCHET_HIT_BACK_ANIM_MODE
            };
        return RobotsSweeperRatchetHitResult {
            reaction_accepted: true,
            query_hit_still_commits: true,
            needs_direction_rng: false,
            destroyed: false,
            hit_points,
            top_state: ROBOTS_SWEEPER_RATCHET_STATE_HIT_RECOVERY,
            requested_anim_mode: Some(requested_anim_mode),
            random_draws_used: 1,
            cleanup_live_ai_characters: false,
            dispatch_player_trigger_link0_event: false,
            player_link0_resolution_failed: false,
            difficulty: input.difficulty.wrapping_add(1).min(input.max_difficulty),
        };
    }

    let mut result = RobotsSweeperRatchetHitResult {
        reaction_accepted: true,
        query_hit_still_commits: true,
        needs_direction_rng: false,
        destroyed: true,
        hit_points,
        top_state: ROBOTS_SWEEPER_RATCHET_STATE_DEATH,
        requested_anim_mode: Some(ROBOTS_SWEEPER_RATCHET_DEATH_ANIM_MODE),
        random_draws_used: 0,
        cleanup_live_ai_characters: true,
        dispatch_player_trigger_link0_event: false,
        player_link0_resolution_failed: false,
        difficulty: input.difficulty,
    };

    match input.player_link0 {
        RobotsSweeperRatchetPlayerLink0::NoPlayerOrCreator => {}
        RobotsSweeperRatchetPlayerLink0::Resolved => {
            result.dispatch_player_trigger_link0_event = true;
        }
        RobotsSweeperRatchetPlayerLink0::Missing => {
            result.reaction_accepted = false;
            result.player_link0_resolution_failed = true;
            return result;
        }
    }

    result.difficulty = input.difficulty.wrapping_add(1).min(input.max_difficulty);
    result
}

pub const ROBOTS_RATCHET_MISSILE_HIT_EFFECT_SCRIPT_UID: u32 = 0x0400_02B1;
pub const ROBOTS_RATCHET_MISSILE_HIT_EFFECT_DATUM_UID: u32 = 0x1000_00BC;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsRatchetMissileHitResult {
    pub reaction_accepted: bool,
    pub query_hit_still_commits: bool,
    /// Native `0x00443EE0` marks the missile owner XItem pending destroy (bit0x10)
    /// and notifies its creator through vslot +0x84 when present.
    pub mark_owner_pending_destroy: bool,
    /// Native `0x00404360` receives owner position +0xD0/+0xE0 and these exact
    /// resources. Position resolution remains host-side for UE5.8.
    pub effect_script_uid: u32,
    pub effect_datum_uid: u32,
}

/// `XItemHandler_RatchetMissile::ApplyHit` (`0x004D01C0`). Native always marks
/// the live owner pending destroy, emits the fixed hit effect at owner position,
/// and returns 1.
pub fn robots_apply_ratchet_missile_hit() -> RobotsRatchetMissileHitResult {
    RobotsRatchetMissileHitResult {
        reaction_accepted: true,
        query_hit_still_commits: true,
        mark_owner_pending_destroy: true,
        effect_script_uid: ROBOTS_RATCHET_MISSILE_HIT_EFFECT_SCRIPT_UID,
        effect_datum_uid: ROBOTS_RATCHET_MISSILE_HIT_EFFECT_DATUM_UID,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eye_only_accepts_open_and_clamps_controller_difficulty() {
        let rejected = robots_apply_sweeper_eye_hit(RobotsSweeperEyeHitInput {
            state: 2,
            hit_points: 3,
            difficulty: 14,
            max_difficulty: 15,
        });
        assert!(!rejected.reaction_accepted);
        assert!(rejected.query_hit_still_commits);
        assert_eq!(rejected.hit_points, 3);
        assert_eq!(rejected.difficulty, 14);

        let accepted = robots_apply_sweeper_eye_hit(RobotsSweeperEyeHitInput {
            state: ROBOTS_SWEEPER_EYE_STATE_OPEN,
            hit_points: 3,
            difficulty: 15,
            max_difficulty: 15,
        });
        assert!(accepted.reaction_accepted);
        assert_eq!(accepted.hit_points, 2);
        assert_eq!(accepted.state, ROBOTS_SWEEPER_EYE_STATE_HIT);
        assert_eq!(accepted.script_uid, Some(ROBOTS_SWEEPER_EYE_HIT_SCRIPT_UID));
        assert!(accepted.reset_script_signal);
        assert_eq!(accepted.difficulty, 15);
    }

    #[test]
    fn eye_final_hit_switches_to_destroyed_and_requests_native_effect() {
        let result = robots_apply_sweeper_eye_hit(RobotsSweeperEyeHitInput {
            state: ROBOTS_SWEEPER_EYE_STATE_OPEN,
            hit_points: 1,
            difficulty: 7,
            max_difficulty: 15,
        });
        assert!(result.reaction_accepted);
        assert!(result.destroyed);
        assert_eq!(result.hit_points, 0);
        assert_eq!(result.state, ROBOTS_SWEEPER_EYE_STATE_DESTROYED);
        assert_eq!(
            result.script_uid,
            Some(ROBOTS_SWEEPER_EYE_DESTROYED_SCRIPT_UID)
        );
        assert_eq!(
            result.effect_uid,
            Some(ROBOTS_SWEEPER_EYE_DESTROY_EFFECT_UID)
        );
        assert_eq!(result.difficulty, 8);
    }

    fn ratchet(hit_points: u8) -> RobotsSweeperRatchetHitInput {
        RobotsSweeperRatchetHitInput {
            damage_phase_armed: true,
            top_state: 6,
            damage_accept_latch: true,
            hit_points,
            hit_random_mod100: Some(12),
            difficulty: 10,
            max_difficulty: 15,
            player_link0: RobotsSweeperRatchetPlayerLink0::Resolved,
        }
    }

    #[test]
    fn ratchet_rejects_unarmed_state9_and_closed_damage_latch_without_rng() {
        let mut unarmed = ratchet(5);
        unarmed.damage_phase_armed = false;
        let result = robots_apply_sweeper_ratchet_hit(unarmed);
        assert!(!result.reaction_accepted);
        assert_eq!(result.random_draws_used, 0);

        let mut state9 = ratchet(5);
        state9.top_state = ROBOTS_SWEEPER_RATCHET_STATE_HIT_RECOVERY;
        assert!(!robots_apply_sweeper_ratchet_hit(state9).reaction_accepted);

        let mut closed = ratchet(5);
        closed.damage_accept_latch = false;
        assert!(!robots_apply_sweeper_ratchet_hit(closed).reaction_accepted);
    }

    #[test]
    fn ratchet_nonlethal_requires_one_roll_and_uses_native_70_30_split() {
        let mut input = ratchet(5);
        input.hit_random_mod100 = None;
        let needs_rng = robots_apply_sweeper_ratchet_hit(input);
        assert!(needs_rng.needs_direction_rng);
        assert!(!needs_rng.reaction_accepted);
        assert_eq!(needs_rng.hit_points, 5);

        let mut forward = ratchet(5);
        forward.hit_random_mod100 = Some(69);
        let result = robots_apply_sweeper_ratchet_hit(forward);
        assert!(result.reaction_accepted);
        assert_eq!(result.hit_points, 4);
        assert_eq!(result.top_state, ROBOTS_SWEEPER_RATCHET_STATE_HIT_RECOVERY);
        assert_eq!(
            result.requested_anim_mode,
            Some(ROBOTS_SWEEPER_RATCHET_HIT_FORWARD_ANIM_MODE)
        );
        assert_eq!(result.random_draws_used, 1);
        assert_eq!(result.difficulty, 11);

        let mut back = ratchet(5);
        back.hit_random_mod100 = Some(70);
        assert_eq!(
            robots_apply_sweeper_ratchet_hit(back).requested_anim_mode,
            Some(ROBOTS_SWEEPER_RATCHET_HIT_BACK_ANIM_MODE)
        );
    }

    #[test]
    fn ratchet_lethal_hit_uses_no_direction_rng_and_plans_cleanup_and_link0_event() {
        let result = robots_apply_sweeper_ratchet_hit(ratchet(1));
        assert!(result.reaction_accepted);
        assert!(result.destroyed);
        assert_eq!(result.hit_points, 0);
        assert_eq!(result.top_state, ROBOTS_SWEEPER_RATCHET_STATE_DEATH);
        assert_eq!(
            result.requested_anim_mode,
            Some(ROBOTS_SWEEPER_RATCHET_DEATH_ANIM_MODE)
        );
        assert_eq!(result.random_draws_used, 0);
        assert!(result.cleanup_live_ai_characters);
        assert!(result.dispatch_player_trigger_link0_event);
        assert_eq!(result.difficulty, 11);
    }

    #[test]
    fn ratchet_missing_player_link0_returns_zero_after_lethal_side_effects_before_difficulty_increment(
    ) {
        let mut input = ratchet(1);
        input.player_link0 = RobotsSweeperRatchetPlayerLink0::Missing;
        let result = robots_apply_sweeper_ratchet_hit(input);
        assert!(!result.reaction_accepted);
        assert!(result.query_hit_still_commits);
        assert!(result.destroyed);
        assert_eq!(result.hit_points, 0);
        assert_eq!(result.top_state, ROBOTS_SWEEPER_RATCHET_STATE_DEATH);
        assert!(result.cleanup_live_ai_characters);
        assert!(!result.dispatch_player_trigger_link0_event);
        assert!(result.player_link0_resolution_failed);
        assert_eq!(result.difficulty, 10);
    }

    #[test]
    fn ratchet_lethal_without_player_creator_skips_event_but_still_increments_difficulty() {
        let mut input = ratchet(1);
        input.player_link0 = RobotsSweeperRatchetPlayerLink0::NoPlayerOrCreator;
        let result = robots_apply_sweeper_ratchet_hit(input);
        assert!(result.reaction_accepted);
        assert!(!result.dispatch_player_trigger_link0_event);
        assert!(!result.player_link0_resolution_failed);
        assert_eq!(result.difficulty, 11);
    }

    #[test]
    fn ratchet_missile_hit_marks_owner_pending_destroy_and_emits_exact_effect() {
        let result = robots_apply_ratchet_missile_hit();
        assert!(result.reaction_accepted);
        assert!(result.query_hit_still_commits);
        assert!(result.mark_owner_pending_destroy);
        assert_eq!(result.effect_script_uid, 0x0400_02B1);
        assert_eq!(result.effect_datum_uid, 0x1000_00BC);
    }
}
