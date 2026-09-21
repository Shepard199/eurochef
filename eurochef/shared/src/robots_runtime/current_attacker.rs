use serde::Serialize;

pub const ROBOTS_CURRENT_ATTACKER_DISTANCE_SWITCH_MARGIN: f32 = 1.0;
pub const ROBOTS_CURRENT_ATTACKER_PLAYER_MAX_FLOOR_HEIGHT: f32 = 3.0;
pub const ROBOTS_CURRENT_ATTACKER_PLAYER_STRICT_FLOOR_HEIGHT: f32 = 1.0;
pub const ROBOTS_CURRENT_ATTACKER_WATCHBOT_BLOCK_ENTER: f32 = 0.6;
pub const ROBOTS_CURRENT_ATTACKER_WATCHBOT_BLOCK_EXIT: f32 = 0.3;
pub const ROBOTS_CURRENT_ATTACKER_MISSING_FLOOR_SENTINEL: f32 = 99_999.0;
pub const ROBOTS_CURRENT_ATTACKER_WATCHBOT_DATUM_UID: u32 = 0x1000_0027;

pub const fn robots_current_attacker_claim_active(
    handler_flags_628: u32,
    is_current_owner: bool,
) -> bool {
    handler_flags_628 & 0x0010_0000 != 0 || is_current_owner
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsCurrentAttackerEligibilityInput {
    /// Handler+0x628 bit0. Native +0x15C rejects while this bit is set.
    pub handler_flags_628: u32,
    /// Handler+0x61C navigation-region ownership.
    pub owner_region_ordinal: Option<usize>,
    /// Handler+0x610 group flags/lane selected for the current face.
    pub owner_group_flags0: Option<u8>,
    /// Handler+0x605 copied from creator +0x104. Nonzero permits cross-group.
    pub allow_cross_group: bool,
    /// Process-global target navigation region, DAT_007B2A04.
    pub target_region_ordinal: Option<usize>,
    /// Process-global target group, DAT_007B2A0C.
    pub target_group_flags0: Option<u8>,
    /// Exact result of native 0x00455560: true means attack candidacy is blocked.
    pub environment_blocked: bool,
}

/// Exact common AI candidate vslot 0x00455510, projected to stable host identifiers.
pub const fn robots_current_attacker_candidate_eligible(
    input: RobotsCurrentAttackerEligibilityInput,
) -> bool {
    if input.handler_flags_628 & 1 != 0 || input.environment_blocked {
        return false;
    }
    let (Some(owner_region), Some(target_region)) =
        (input.owner_region_ordinal, input.target_region_ordinal)
    else {
        return false;
    };
    if owner_region != target_region {
        return false;
    }
    if input.allow_cross_group {
        return true;
    }
    matches!(
        (input.owner_group_flags0, input.target_group_flags0),
        (Some(owner_group), Some(target_group)) if owner_group == target_group
    )
}

/// Normal Player-target environment branch of native 0x00455560.
pub fn robots_current_attacker_player_environment_blocked(
    game_control_mode: u8,
    player_state: u8,
    target_height_above_floor: Option<f32>,
) -> bool {
    if game_control_mode == 6 {
        return false;
    }
    let height =
        target_height_above_floor.unwrap_or(ROBOTS_CURRENT_ATTACKER_MISSING_FLOOR_SENTINEL);
    if height > ROBOTS_CURRENT_ATTACKER_PLAYER_MAX_FLOOR_HEIGHT {
        return true;
    }
    if matches!(player_state, 5 | 6 | 7 | 9 | 11) {
        return false;
    }
    height > ROBOTS_CURRENT_ATTACKER_PLAYER_STRICT_FLOOR_HEIGHT
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsCurrentAttackerWatchBotGateState {
    /// Candidate Handler+0x5FB.
    pub blocked_5fb: bool,
}

impl RobotsCurrentAttackerWatchBotGateState {
    /// Special WatchBot-target branch of native 0x00455560.
    pub fn advance(
        &mut self,
        watchbot_height_above_floor: Option<f32>,
        candidate_datum_relative_y: Option<f32>,
    ) -> bool {
        let watchbot_height = watchbot_height_above_floor
            .unwrap_or(ROBOTS_CURRENT_ATTACKER_MISSING_FLOOR_SENTINEL);
        let datum_relative_y = candidate_datum_relative_y.unwrap_or(1.0);
        let delta = watchbot_height - datum_relative_y;
        if self.blocked_5fb {
            if delta < ROBOTS_CURRENT_ATTACKER_WATCHBOT_BLOCK_EXIT {
                self.blocked_5fb = false;
            }
        } else if delta > ROBOTS_CURRENT_ATTACKER_WATCHBOT_BLOCK_ENTER {
            self.blocked_5fb = true;
        }
        self.blocked_5fb
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsCurrentAttackerCandidate {
    pub owner_key: u64,
    pub eligible: bool,
    pub base_priority: u8,
    pub position_xyz: [f32; 3],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsCurrentAttackerRuntimeState {
    pub current_owner_key: Option<u64>,
}

impl RobotsCurrentAttackerRuntimeState {
    pub fn clear_if_owner(&mut self, owner_key: u64) {
        if self.current_owner_key == Some(owner_key) {
            self.current_owner_key = None;
        }
    }

    /// Exact process-global current-attacker selection core from native 0x004563C0.
    ///
    /// Host responsibilities stay outside this reducer: vslot +0x15C eligibility
    /// including 0x00455560, manager target navigation context, manager +0x34
    /// bonus owner, stable owner keys, and world positions.
    ///
    /// The RNG callback is invoked only on the native random-selection branch.
    pub fn advance<F>(
        &mut self,
        candidates: &[RobotsCurrentAttackerCandidate],
        bonus_owner_key: Option<u64>,
        target_position_xyz: [f32; 3],
        mut next_gameplay_rng: F,
    ) where
        F: FnMut() -> u32,
    {
        if self.current_owner_key.is_some_and(|current| {
            !candidates
                .iter()
                .any(|candidate| candidate.owner_key == current && candidate.eligible)
        }) {
            self.current_owner_key = None;
        }

        let mut best_effective_priority = 0u16;
        let mut best = Vec::<&RobotsCurrentAttackerCandidate>::new();
        for candidate in candidates.iter().filter(|candidate| candidate.eligible) {
            let effective = u16::from(candidate.base_priority)
                + if bonus_owner_key == Some(candidate.owner_key) { 2 } else { 0 };
            if effective > best_effective_priority {
                best_effective_priority = effective;
                best.clear();
                best.push(candidate);
            } else if effective == best_effective_priority {
                best.push(candidate);
            }
        }

        let Some(current_key) = self.current_owner_key else {
            if best.is_empty() {
                return;
            }
            let draw = next_gameplay_rng();
            self.current_owner_key = Some(best[draw as usize % best.len()].owner_key);
            return;
        };

        let current_base_priority = candidates
            .iter()
            .find(|candidate| candidate.owner_key == current_key && candidate.eligible)
            .map(|candidate| u16::from(candidate.base_priority))
            .unwrap_or(0);

        if current_base_priority < best_effective_priority {
            if best.is_empty() {
                self.current_owner_key = None;
                return;
            }
            let draw = next_gameplay_rng();
            self.current_owner_key = Some(best[draw as usize % best.len()].owner_key);
            return;
        }

        let Some(current_candidate) = candidates
            .iter()
            .find(|candidate| candidate.owner_key == current_key && candidate.eligible)
        else {
            self.current_owner_key = None;
            return;
        };
        let current_distance = distance(current_candidate.position_xyz, target_position_xyz);

        let nearest = best.iter().copied().min_by(|a, b| {
            distance_squared(a.position_xyz, target_position_xyz)
                .total_cmp(&distance_squared(b.position_xyz, target_position_xyz))
        });
        let Some(nearest) = nearest else {
            return;
        };
        if nearest.owner_key == current_key {
            return;
        }

        let nearest_distance = distance(nearest.position_xyz, target_position_xyz);
        if nearest_distance
            < current_distance - ROBOTS_CURRENT_ATTACKER_DISTANCE_SWITCH_MARGIN
        {
            self.current_owner_key = Some(nearest.owner_key);
        }
    }
}

fn distance_squared(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    distance_squared(a, b).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(key: u64, eligible: bool, priority: u8, x: f32) -> RobotsCurrentAttackerCandidate {
        RobotsCurrentAttackerCandidate {
            owner_key: key,
            eligible,
            base_priority: priority,
            position_xyz: [x, 0.0, 0.0],
        }
    }

    #[test]
    fn missing_current_uses_one_rng_draw_across_max_priority_ties() {
        let mut state = RobotsCurrentAttackerRuntimeState::default();
        let candidates = [
            c(10, true, 3, 1.0),
            c(11, true, 3, 2.0),
            c(12, true, 2, 0.0),
        ];
        let mut draws = 0;
        state.advance(&candidates, None, [0.0; 3], || {
            draws += 1;
            1
        });
        assert_eq!(state.current_owner_key, Some(11));
        assert_eq!(draws, 1);
    }

    #[test]
    fn bonus_owner_adds_two_only_to_candidate_competition() {
        let mut state = RobotsCurrentAttackerRuntimeState {
            current_owner_key: Some(10),
        };
        let candidates = [c(10, true, 4, 1.0), c(11, true, 3, 2.0)];
        let mut draws = 0;
        state.advance(&candidates, Some(11), [0.0; 3], || {
            draws += 1;
            0
        });
        assert_eq!(state.current_owner_key, Some(11));
        assert_eq!(draws, 1);
    }

    #[test]
    fn competitive_current_switches_only_when_best_candidate_is_over_one_unit_closer() {
        let mut state = RobotsCurrentAttackerRuntimeState {
            current_owner_key: Some(10),
        };
        let candidates = [c(10, true, 5, 5.0), c(11, true, 5, 4.1)];
        let mut draws = 0;
        state.advance(&candidates, None, [0.0; 3], || {
            draws += 1;
            0
        });
        assert_eq!(state.current_owner_key, Some(10));
        assert_eq!(draws, 0);

        let candidates = [c(10, true, 5, 5.0), c(11, true, 5, 3.9)];
        state.advance(&candidates, None, [0.0; 3], || {
            draws += 1;
            0
        });
        assert_eq!(state.current_owner_key, Some(11));
        assert_eq!(draws, 0);
    }

    #[test]
    fn ineligible_current_is_cleared_before_native_random_reselection() {
        let mut state = RobotsCurrentAttackerRuntimeState {
            current_owner_key: Some(10),
        };
        let candidates = [c(10, false, 9, 1.0), c(11, true, 2, 2.0)];
        let mut draws = 0;
        state.advance(&candidates, None, [0.0; 3], || {
            draws += 1;
            7
        });
        assert_eq!(state.current_owner_key, Some(11));
        assert_eq!(draws, 1);
    }

    #[test]
    fn no_eligible_candidates_clear_stale_current_without_rng() {
        let mut state = RobotsCurrentAttackerRuntimeState {
            current_owner_key: Some(10),
        };
        let candidates = [c(10, false, 9, 1.0)];
        let mut draws = 0;
        state.advance(&candidates, None, [0.0; 3], || {
            draws += 1;
            0
        });
        assert_eq!(state.current_owner_key, None);
        assert_eq!(draws, 0);
    }

    #[test]
    fn claim_455da0_is_local_flag_or_process_global_owner() {
        assert!(!robots_current_attacker_claim_active(0, false));
        assert!(robots_current_attacker_claim_active(0x0010_0000, false));
        assert!(robots_current_attacker_claim_active(0, true));
        assert!(robots_current_attacker_claim_active(0x0010_0000, true));
    }

    #[test]
    fn candidate_vslot_455510_keeps_region_group_and_environment_gates_exact() {
        let base = RobotsCurrentAttackerEligibilityInput {
            handler_flags_628: 0,
            owner_region_ordinal: Some(2),
            owner_group_flags0: Some(7),
            allow_cross_group: false,
            target_region_ordinal: Some(2),
            target_group_flags0: Some(7),
            environment_blocked: false,
        };
        assert!(robots_current_attacker_candidate_eligible(base));
        assert!(!robots_current_attacker_candidate_eligible(
            RobotsCurrentAttackerEligibilityInput {
                handler_flags_628: 1,
                ..base
            }
        ));
        assert!(!robots_current_attacker_candidate_eligible(
            RobotsCurrentAttackerEligibilityInput {
                target_region_ordinal: Some(3),
                ..base
            }
        ));
        assert!(!robots_current_attacker_candidate_eligible(
            RobotsCurrentAttackerEligibilityInput {
                target_group_flags0: Some(8),
                ..base
            }
        ));
        assert!(robots_current_attacker_candidate_eligible(
            RobotsCurrentAttackerEligibilityInput {
                target_group_flags0: Some(8),
                allow_cross_group: true,
                ..base
            }
        ));
        assert!(!robots_current_attacker_candidate_eligible(
            RobotsCurrentAttackerEligibilityInput {
                environment_blocked: true,
                ..base
            }
        ));
    }

    #[test]
    fn normal_player_floor_gate_matches_455560_boundaries_and_mode6_bypass() {
        assert!(robots_current_attacker_player_environment_blocked(0, 2, None));
        assert!(!robots_current_attacker_player_environment_blocked(
            0,
            2,
            Some(1.0)
        ));
        assert!(robots_current_attacker_player_environment_blocked(
            0,
            2,
            Some(1.0001)
        ));
        assert!(!robots_current_attacker_player_environment_blocked(
            0,
            5,
            Some(3.0)
        ));
        assert!(robots_current_attacker_player_environment_blocked(
            0,
            5,
            Some(3.0001)
        ));
        assert!(!robots_current_attacker_player_environment_blocked(
            6,
            2,
            None
        ));
    }

    #[test]
    fn watchbot_gate_keeps_native_point6_point3_hysteresis_and_fallbacks() {
        let mut state = RobotsCurrentAttackerWatchBotGateState::default();
        assert!(!state.advance(Some(1.6), Some(1.0)));
        assert!(state.advance(Some(1.6001), Some(1.0)));
        assert!(state.advance(Some(1.3001), Some(1.0)));
        assert!(!state.advance(Some(1.2999), Some(1.0)));

        let mut missing_floor = RobotsCurrentAttackerWatchBotGateState::default();
        assert!(missing_floor.advance(None, Some(1.0)));

        let mut missing_datum = RobotsCurrentAttackerWatchBotGateState::default();
        assert!(missing_datum.advance(Some(1.61), None));
    }
}
