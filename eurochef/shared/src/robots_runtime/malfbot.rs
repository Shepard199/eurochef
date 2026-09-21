use serde::Serialize;

use super::{
    ai_character::RobotsAiPatrolConfig,
    locomotion::{shortest_yaw_delta, RobotsAiTurnRateInput},
    pursue_navmesh::ROBOTS_PURSUE_NAV_PRIORITY,
    scrambled_hit::RobotsScrambledHitRuntimeState,
};

pub const ROBOTS_MALFBOT_PERIODIC_IDLE_BASE_DELAY_TICKS: i32 = 180;
pub const ROBOTS_MALFBOT_PERIODIC_IDLE_ANIM_MODES: [u32; 2] = [0x0900_0006, 0x0900_0007];

pub const ROBOTS_MALFBOT_SCRAMBLED_HIT_PRIORITY: u8 = 0x55;
pub const ROBOTS_MALFBOT_SCRAMBLED_HIT_FLAG: u32 = 0x0000_0008;
pub const ROBOTS_MALFBOT_SCRAMBLED_HIT_START_ANIM_MODE: u32 = 0x0900_0076;
pub const ROBOTS_MALFBOT_SCRAMBLED_HIT_LOOP_ANIM_MODE: u32 = 0x0900_0078;
pub const ROBOTS_MALFBOT_SCRAMBLED_HIT_END_ANIM_MODE: u32 = 0x0900_0077;
pub const ROBOTS_MALFBOT_SCRAMBLED_HIT_LOOP_TICKS: i32 = 180;

pub const ROBOTS_MALFBOT_ELECTRO_HIT_PRIORITY: u8 = 0xc8;
pub const ROBOTS_MALFBOT_ELECTRO_HIT_FLAG: u32 = 0x0000_0010;
pub const ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE: u32 = 0x0900_007d;

pub const ROBOTS_MALFBOT_MAGNETIC_HIT_PRIORITY: u8 = 0x69;
pub const ROBOTS_MALFBOT_MAGNETIC_HIT_FLAG: u32 = 0x0000_4000;
pub const ROBOTS_MALFBOT_MAGNETIC_HIT_ANIM_MODE: u32 = 0x0900_00e9;
pub const ROBOTS_MALFBOT_MAGNETIC_RELEASE_HEIGHT: f32 = 0.1;
pub const ROBOTS_MALFBOT_MAGNETIC_TIMEOUT_SECONDS: f32 = 2.0;
pub const ROBOTS_MALFBOT_MAGNETIC_TARGET_HEIGHT: f32 = 2.0;
pub const ROBOTS_MALFBOT_MAGNETIC_NEAR_HEIGHT: f32 = 0.25;
pub const ROBOTS_MALFBOT_MAGNETIC_DEFAULT_MASS: u8 = 40;
pub const ROBOTS_MALFBOT_MAGNETIC_SPRING_SCALE: f32 = 0.1;
pub const ROBOTS_MALFBOT_MAGNETIC_MASS_DAMPING_SCALE: f32 = 0.001;
pub const ROBOTS_MALFBOT_MAGNETIC_WOBBLE_SCALE: f32 = 0.5;
pub const ROBOTS_MALFBOT_MAGNETIC_PHASE_DEGREES_SCALE: f32 = 1.0 / 12.0;
pub const ROBOTS_MALFBOT_MAGNETIC_DROP_INTERVAL_INITIAL: u32 = 20;
pub const ROBOTS_MALFBOT_MAGNETIC_DROP_INTERVAL_MIN: u32 = 5;

pub const fn malfbot_patrol_config() -> RobotsAiPatrolConfig {
    RobotsAiPatrolConfig {
        base_yaw_radians: 0.0,
        interval_seconds: 5,
        target_locomotion_scalar: 0.0,
        turn_rate: RobotsAiTurnRateInput::Default,
    }
}

fn status_hit_priority(
    active: bool,
    completed: bool,
    got_hit_latch: bool,
    query_flags: u32,
    required_flag: u32,
    priority: u8,
) -> u8 {
    if active && !completed {
        return priority;
    }
    if got_hit_latch && query_flags & required_flag != 0 {
        priority
    } else {
        1
    }
}

pub type RobotsMalfBotScrambledHitRuntimeState = RobotsScrambledHitRuntimeState;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsMalfBotElectroHitRuntimeState {
    pub active: bool,
    pub completed: bool,
}

impl RobotsMalfBotElectroHitRuntimeState {
    pub fn priority(&self, got_hit_latch: bool, query_flags: u32) -> u8 {
        status_hit_priority(
            self.active,
            self.completed,
            got_hit_latch,
            query_flags,
            ROBOTS_MALFBOT_ELECTRO_HIT_FLAG,
            ROBOTS_MALFBOT_ELECTRO_HIT_PRIORITY,
        )
    }

    pub fn enter(&mut self) {
        self.active = true;
        self.completed = false;
    }

    pub fn leave(&mut self) {
        self.active = false;
    }

    pub const fn requested_anim_mode(&self) -> u32 {
        ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE
    }

    /// Native `0x004584A0 -> 0x004550A0(...,1)` routes through MalfBot vslot
    /// +0x12C (`0x00453640`), i.e. the existing natural-death/creator-latch path.
    pub fn setup_idle(&mut self) -> RobotsMalfBotStatusHitHostPlan {
        self.completed = true;
        RobotsMalfBotStatusHitHostPlan {
            request_natural_death: true,
            ..RobotsMalfBotStatusHitHostPlan::default()
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub enum RobotsMalfBotMagneticHitPhase {
    #[default]
    Inactive,
    Attached,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsMalfBotMagneticHitRuntimeState {
    pub active: bool,
    pub completed: bool,
    pub phase: RobotsMalfBotMagneticHitPhase,
    pub initial_owner_y: f32,
    pub elapsed_seconds: f32,
    /// Native node +0x30, added into Character Physics +0x30 every attached tick.
    pub spring_accumulator: f32,
    /// Native node +0x34. `0x004587E0` advances and wraps this before the next tick.
    pub wobble_phase_radians: f32,
    /// Native node +0x3C. This is latched on the first |height error| < 0.25
    /// crossing before the optional visual/effect lookup and later requests death.
    pub near_height_latched: bool,
    /// Native node +0x40 / +0x44 adaptive interval used only while +0x63D is nonzero.
    pub drop_interval: u32,
    pub drop_interval_counter: u32,
}

impl Default for RobotsMalfBotMagneticHitRuntimeState {
    fn default() -> Self {
        Self {
            active: false,
            completed: false,
            phase: RobotsMalfBotMagneticHitPhase::Inactive,
            initial_owner_y: 0.0,
            elapsed_seconds: 0.0,
            spring_accumulator: 0.0,
            wobble_phase_radians: 0.0,
            near_height_latched: false,
            drop_interval: ROBOTS_MALFBOT_MAGNETIC_DROP_INTERVAL_INITIAL,
            drop_interval_counter: ROBOTS_MALFBOT_MAGNETIC_DROP_INTERVAL_INITIAL,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct RobotsMalfBotStatusHitHostPlan {
    pub requested_anim_mode: Option<u32>,
    /// Host seam for `0x004587E0`; this owns native Physics/effect work and may
    /// later report that natural death should be requested on Magnetic finish.
    pub service_magnetic_effect: bool,
    /// `0x00455E30(1)` when Magnetic state1 loses the attachment relation.
    pub set_handler_606: Option<bool>,
    pub request_natural_death: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsMalfBotMagneticEffectInput {
    pub owner_y: f32,
    /// MonsterDatabase row +0x14 for the concrete Malf family. `None` means the
    /// class-chain did not match and native uses the default mass 40; a present
    /// value above 100 makes `0x004587E0` return immediately without servicing.
    pub magnetic_mass: Option<u8>,
    /// Handler +0x63D. The native helper decrements this only when its adaptive
    /// drop interval fires.
    pub drop_charge_count: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct RobotsMalfBotMagneticEffectPlan {
    /// False only for the concrete Malf class case where Handler+0x63C > 100;
    /// native `0x004587E0` returns before touching Physics/effect/drop state.
    pub serviced: bool,
    /// Delta added to Character Physics +0x30 this fixed tick.
    pub physics_vertical_velocity_delta: f32,
    /// First crossing of |target_height - owner_y| < 0.25 requests the native
    /// datum/effect lookup and latches node +0x3C.
    pub request_near_height_effect: bool,
    /// Native `0x47000001` drop-object request from the +0x63D cadence branch.
    pub request_drop_object: bool,
    pub next_drop_charge_count: u8,
}

impl RobotsMalfBotMagneticHitRuntimeState {
    pub fn priority(&self, got_hit_latch: bool, query_flags: u32) -> u8 {
        status_hit_priority(
            self.active,
            self.completed,
            got_hit_latch,
            query_flags,
            ROBOTS_MALFBOT_MAGNETIC_HIT_FLAG,
            ROBOTS_MALFBOT_MAGNETIC_HIT_PRIORITY,
        )
    }

    pub fn enter(&mut self, owner_y: f32) {
        self.active = true;
        self.completed = false;
        self.phase = RobotsMalfBotMagneticHitPhase::Attached;
        self.initial_owner_y = owner_y;
        self.elapsed_seconds = 0.0;
        self.near_height_latched = false;
        self.spring_accumulator = 0.0;
        self.wobble_phase_radians = 0.0;
        self.drop_interval = ROBOTS_MALFBOT_MAGNETIC_DROP_INTERVAL_INITIAL;
        self.drop_interval_counter = ROBOTS_MALFBOT_MAGNETIC_DROP_INTERVAL_INITIAL;
    }

    pub fn leave(&mut self) {
        self.active = false;
        self.phase = RobotsMalfBotMagneticHitPhase::Inactive;
    }

    pub fn apply_effect_host_result(&mut self, near_height_latched: bool) {
        self.near_height_latched |= near_height_latched;
    }

    /// Engine-neutral body of native `0x004587E0`. This deliberately returns
    /// Physics/effect requests instead of owning a GUI/UE physics object.
    pub fn step_attached_effect(
        &mut self,
        input: RobotsMalfBotMagneticEffectInput,
    ) -> RobotsMalfBotMagneticEffectPlan {
        if input.magnetic_mass.is_some_and(|mass| mass > 100) {
            return RobotsMalfBotMagneticEffectPlan::default();
        }
        let mass = input
            .magnetic_mass
            .unwrap_or(ROBOTS_MALFBOT_MAGNETIC_DEFAULT_MASS) as f32;
        let height_error =
            (self.initial_owner_y + ROBOTS_MALFBOT_MAGNETIC_TARGET_HEIGHT) - input.owner_y;
        let damping = 1.0 - mass * ROBOTS_MALFBOT_MAGNETIC_MASS_DAMPING_SCALE;
        self.spring_accumulator = (height_error * ROBOTS_MALFBOT_MAGNETIC_SPRING_SCALE
            + self.spring_accumulator)
            * damping;

        let wobble_delta = self.wobble_phase_radians.sin() * ROBOTS_MALFBOT_MAGNETIC_WOBBLE_SCALE;
        let phase_step_degrees = (100.0 - mass) * ROBOTS_MALFBOT_MAGNETIC_PHASE_DEGREES_SCALE;
        self.wobble_phase_radians = shortest_yaw_delta(
            0.0,
            self.wobble_phase_radians + phase_step_degrees.to_radians(),
        );

        let request_near_height_effect =
            !self.near_height_latched && height_error.abs() < ROBOTS_MALFBOT_MAGNETIC_NEAR_HEIGHT;
        self.near_height_latched |= request_near_height_effect;

        let mut next_drop_charge_count = input.drop_charge_count;
        let mut request_drop_object = false;
        if next_drop_charge_count != 0 {
            let old_counter = self.drop_interval_counter;
            self.drop_interval_counter = self.drop_interval_counter.wrapping_add(1);
            if self.drop_interval < old_counter {
                self.drop_interval_counter = 0;
                self.drop_interval = self.drop_interval.saturating_sub(3).clamp(
                    ROBOTS_MALFBOT_MAGNETIC_DROP_INTERVAL_MIN,
                    ROBOTS_MALFBOT_MAGNETIC_DROP_INTERVAL_INITIAL,
                );
                next_drop_charge_count = next_drop_charge_count.saturating_sub(1);
                request_drop_object = true;
            }
        }

        RobotsMalfBotMagneticEffectPlan {
            serviced: true,
            physics_vertical_velocity_delta: self.spring_accumulator + wobble_delta,
            request_near_height_effect,
            request_drop_object,
            next_drop_charge_count,
        }
    }

    pub fn step_with_anim_mode(
        &mut self,
        got_hit_latch: &mut bool,
        owner_y: f32,
        attachment_relation_matches: bool,
        delta_seconds: f32,
        requested_anim_mode: u32,
    ) -> RobotsMalfBotStatusHitHostPlan {
        match self.phase {
            RobotsMalfBotMagneticHitPhase::Inactive => RobotsMalfBotStatusHitHostPlan::default(),
            RobotsMalfBotMagneticHitPhase::Attached => {
                if !attachment_relation_matches {
                    self.phase = RobotsMalfBotMagneticHitPhase::Released;
                    return RobotsMalfBotStatusHitHostPlan {
                        requested_anim_mode: Some(requested_anim_mode),
                        set_handler_606: Some(true),
                        ..RobotsMalfBotStatusHitHostPlan::default()
                    };
                }
                RobotsMalfBotStatusHitHostPlan {
                    requested_anim_mode: Some(requested_anim_mode),
                    service_magnetic_effect: true,
                    set_handler_606: Some(false),
                    ..RobotsMalfBotStatusHitHostPlan::default()
                }
            }
            RobotsMalfBotMagneticHitPhase::Released => {
                self.elapsed_seconds += delta_seconds.max(0.0);
                if owner_y - self.initial_owner_y < ROBOTS_MALFBOT_MAGNETIC_RELEASE_HEIGHT
                    || self.elapsed_seconds > ROBOTS_MALFBOT_MAGNETIC_TIMEOUT_SECONDS
                {
                    self.phase = RobotsMalfBotMagneticHitPhase::Inactive;
                    self.completed = true;
                    *got_hit_latch = false;
                    return RobotsMalfBotStatusHitHostPlan {
                        request_natural_death: self.near_height_latched,
                        ..RobotsMalfBotStatusHitHostPlan::default()
                    };
                }
                RobotsMalfBotStatusHitHostPlan {
                    requested_anim_mode: Some(requested_anim_mode),
                    ..RobotsMalfBotStatusHitHostPlan::default()
                }
            }
        }
    }

    pub fn step(
        &mut self,
        got_hit_latch: &mut bool,
        owner_y: f32,
        attachment_relation_matches: bool,
        delta_seconds: f32,
    ) -> RobotsMalfBotStatusHitHostPlan {
        self.step_with_anim_mode(
            got_hit_latch,
            owner_y,
            attachment_relation_matches,
            delta_seconds,
            ROBOTS_MALFBOT_MAGNETIC_HIT_ANIM_MODE,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsMalfBotBehaviorWinner {
    Patrol,
    PeriodicIdle,
    PatrolNavMesh2,
    PursueNavMesh,
    ScrambledHit,
    ElectroHit,
    MagneticHit,
    AttackGroup,
}

/// Native `0x00457140` low-byte priority selection for the complete MalfBot
/// behavior list. Status-hit nodes use the same active-node retention rule as
/// every other node: an active incomplete node keeps returning its configured
/// priority even after the original Handler+0x608/+0x378 admission edge changes.
pub fn malfbot_behavior_winner(
    attack_group_priority: u8,
    pursue_ready: bool,
    periodic_idle_priority: u8,
    patrol_navmesh2_priority: u8,
    patrol_priority: u8,
    scrambled_hit_priority: u8,
    electro_hit_priority: u8,
    magnetic_hit_priority: u8,
) -> Option<RobotsMalfBotBehaviorWinner> {
    let candidates = [
        (patrol_priority, RobotsMalfBotBehaviorWinner::Patrol),
        (
            periodic_idle_priority,
            RobotsMalfBotBehaviorWinner::PeriodicIdle,
        ),
        (
            patrol_navmesh2_priority,
            RobotsMalfBotBehaviorWinner::PatrolNavMesh2,
        ),
        (
            if pursue_ready {
                ROBOTS_PURSUE_NAV_PRIORITY
            } else {
                1
            },
            RobotsMalfBotBehaviorWinner::PursueNavMesh,
        ),
        (
            scrambled_hit_priority,
            RobotsMalfBotBehaviorWinner::ScrambledHit,
        ),
        (
            electro_hit_priority,
            RobotsMalfBotBehaviorWinner::ElectroHit,
        ),
        (
            magnetic_hit_priority,
            RobotsMalfBotBehaviorWinner::MagneticHit,
        ),
        (
            attack_group_priority,
            RobotsMalfBotBehaviorWinner::AttackGroup,
        ),
    ];
    let mut best_priority = 1u8;
    let mut selected = None;
    for (priority, winner) in candidates {
        if best_priority < priority {
            best_priority = priority;
            selected = Some(winner);
        }
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_malfbot_normal_priorities_order_attack_pursue_idle_nav_patrol() {
        assert_eq!(
            crate::robots_runtime::ai_character::ROBOTS_AI_PATROL_PRIORITY,
            10
        );
        assert_eq!(
            crate::robots_runtime::npc_behavior::ROBOTS_PATROL_NAVMESH2_PRIORITY,
            11
        );
        assert_eq!(
            crate::robots_runtime::npc_behavior::ROBOTS_PERIODIC_IDLE_PRIORITY,
            0x16
        );
        assert_eq!(ROBOTS_PURSUE_NAV_PRIORITY, 0x1f);

        assert_eq!(
            malfbot_behavior_winner(0x32, true, 0x16, 0x0b, 0x0a, 1, 1, 1),
            Some(RobotsMalfBotBehaviorWinner::AttackGroup)
        );
        assert_eq!(
            malfbot_behavior_winner(1, true, 0x16, 0x0b, 0x0a, 1, 1, 1),
            Some(RobotsMalfBotBehaviorWinner::PursueNavMesh)
        );
        assert_eq!(
            malfbot_behavior_winner(1, false, 0x16, 0x0b, 0x0a, 1, 1, 1),
            Some(RobotsMalfBotBehaviorWinner::PeriodicIdle)
        );
        assert_eq!(
            malfbot_behavior_winner(1, false, 1, 0x0b, 0x0a, 1, 1, 1),
            Some(RobotsMalfBotBehaviorWinner::PatrolNavMesh2)
        );
        assert_eq!(
            malfbot_behavior_winner(1, false, 1, 1, 0x0a, 1, 1, 1),
            Some(RobotsMalfBotBehaviorWinner::Patrol)
        );
        assert_eq!(malfbot_behavior_winner(1, false, 1, 1, 1, 1, 1, 1), None);
    }

    #[test]
    fn status_hit_priorities_match_builder_and_preempt_normal_nodes() {
        assert_eq!(ROBOTS_MALFBOT_SCRAMBLED_HIT_PRIORITY, 0x55);
        assert_eq!(ROBOTS_MALFBOT_MAGNETIC_HIT_PRIORITY, 0x69);
        assert_eq!(ROBOTS_MALFBOT_ELECTRO_HIT_PRIORITY, 0xc8);
        assert_eq!(
            malfbot_behavior_winner(0x32, true, 0x16, 0x0b, 0x0a, 0x55, 1, 1),
            Some(RobotsMalfBotBehaviorWinner::ScrambledHit)
        );
        assert_eq!(
            malfbot_behavior_winner(0x32, true, 0x16, 0x0b, 0x0a, 0x55, 1, 0x69),
            Some(RobotsMalfBotBehaviorWinner::MagneticHit)
        );
        assert_eq!(
            malfbot_behavior_winner(0x32, true, 0x16, 0x0b, 0x0a, 0x55, 0xc8, 0x69),
            Some(RobotsMalfBotBehaviorWinner::ElectroHit)
        );
    }

    #[test]
    fn scrambled_hit_preserves_setup_idle_phases_and_negative_counter_boundary() {
        let mut state = RobotsMalfBotScrambledHitRuntimeState::default();
        let mut got_hit = true;
        assert_eq!(
            state.priority(got_hit, ROBOTS_MALFBOT_SCRAMBLED_HIT_FLAG),
            0x55
        );
        state.enter();
        assert_eq!(
            state.step(&mut got_hit),
            ROBOTS_MALFBOT_SCRAMBLED_HIT_START_ANIM_MODE
        );
        state.setup_idle(&mut got_hit);
        assert_eq!(
            state.current_anim_mode,
            ROBOTS_MALFBOT_SCRAMBLED_HIT_LOOP_ANIM_MODE
        );
        for _ in 0..ROBOTS_MALFBOT_SCRAMBLED_HIT_LOOP_TICKS {
            assert_eq!(
                state.step(&mut got_hit),
                ROBOTS_MALFBOT_SCRAMBLED_HIT_LOOP_ANIM_MODE
            );
        }
        assert_eq!(state.remaining_loop_ticks, 0);
        assert_eq!(
            state.step(&mut got_hit),
            ROBOTS_MALFBOT_SCRAMBLED_HIT_END_ANIM_MODE
        );
        assert!(got_hit);
        state.setup_idle(&mut got_hit);
        assert!(state.completed);
        assert!(!got_hit);
    }

    #[test]
    fn electro_setup_idle_requests_existing_natural_death_path() {
        let mut state = RobotsMalfBotElectroHitRuntimeState::default();
        state.enter();
        assert_eq!(
            state.requested_anim_mode(),
            ROBOTS_MALFBOT_ELECTRO_HIT_ANIM_MODE
        );
        let plan = state.setup_idle();
        assert!(plan.request_natural_death);
        assert!(state.completed);
    }

    #[test]
    fn magnetic_attached_effect_matches_native_spring_wobble_near_height_and_drop_cadence() {
        let mut state = RobotsMalfBotMagneticHitRuntimeState::default();
        state.enter(10.0);
        let first = state.step_attached_effect(RobotsMalfBotMagneticEffectInput {
            owner_y: 10.0,
            magnetic_mass: Some(40),
            drop_charge_count: 2,
        });
        assert!((first.physics_vertical_velocity_delta - 0.192).abs() < 1.0e-6);
        assert!((state.wobble_phase_radians - 5.0_f32.to_radians()).abs() < 1.0e-6);
        assert!(!first.request_near_height_effect);
        assert!(!first.request_drop_object);
        assert_eq!(first.next_drop_charge_count, 2);

        let second = state.step_attached_effect(RobotsMalfBotMagneticEffectInput {
            owner_y: 11.9,
            magnetic_mass: Some(40),
            drop_charge_count: first.next_drop_charge_count,
        });
        let expected_spring = (0.1 * 0.1 + 0.192) * 0.96;
        let expected_wobble = 5.0_f32.to_radians().sin() * 0.5;
        assert!(
            (second.physics_vertical_velocity_delta - (expected_spring + expected_wobble)).abs()
                < 1.0e-6
        );
        assert!(second.request_near_height_effect);
        assert!(state.near_height_latched);
        assert!(second.request_drop_object);
        assert_eq!(second.next_drop_charge_count, 1);
        assert_eq!(state.drop_interval, 17);
        assert_eq!(state.drop_interval_counter, 0);

        let mut fallback = RobotsMalfBotMagneticHitRuntimeState::default();
        fallback.enter(10.0);
        let fallback_plan = fallback.step_attached_effect(RobotsMalfBotMagneticEffectInput {
            owner_y: 10.0,
            magnetic_mass: None,
            drop_charge_count: 0,
        });
        assert!(fallback_plan.serviced);
        assert!((fallback_plan.physics_vertical_velocity_delta - 0.192).abs() < 1.0e-6);

        let mut rejected = RobotsMalfBotMagneticHitRuntimeState::default();
        rejected.enter(10.0);
        let rejected_plan = rejected.step_attached_effect(RobotsMalfBotMagneticEffectInput {
            owner_y: 10.0,
            magnetic_mass: Some(101),
            drop_charge_count: 2,
        });
        assert!(!rejected_plan.serviced);
        assert_eq!(rejected.spring_accumulator, 0.0);
        assert_eq!(rejected.wobble_phase_radians, 0.0);
        assert_eq!(
            rejected.drop_interval_counter,
            ROBOTS_MALFBOT_MAGNETIC_DROP_INTERVAL_INITIAL
        );
    }

    #[test]
    fn magnetic_release_uses_point_one_height_and_strict_two_second_timeout() {
        let mut state = RobotsMalfBotMagneticHitRuntimeState::default();
        let mut got_hit = true;
        state.enter(10.0);
        let attached = state.step(&mut got_hit, 10.0, true, 1.0 / 60.0);
        assert!(attached.service_magnetic_effect);
        assert_eq!(attached.set_handler_606, Some(false));
        let released = state.step(&mut got_hit, 10.2, false, 1.0 / 60.0);
        assert_eq!(state.phase, RobotsMalfBotMagneticHitPhase::Released);
        assert_eq!(released.set_handler_606, Some(true));
        let still_active = state.step(&mut got_hit, 10.2, false, 2.0);
        assert!(!state.completed);
        assert_eq!(
            still_active.requested_anim_mode,
            Some(ROBOTS_MALFBOT_MAGNETIC_HIT_ANIM_MODE)
        );
        let timed_out = state.step(&mut got_hit, 10.2, false, 1.0 / 60.0);
        assert!(state.completed);
        assert!(!got_hit);
        assert_eq!(timed_out.requested_anim_mode, None);

        let mut low = RobotsMalfBotMagneticHitRuntimeState::default();
        got_hit = true;
        low.enter(10.0);
        let _ = low.step(&mut got_hit, 10.0, false, 1.0 / 60.0);
        let done = low.step(&mut got_hit, 10.05, false, 1.0 / 60.0);
        assert!(low.completed);
        assert!(!got_hit);
        assert_eq!(done.requested_anim_mode, None);
    }

    #[test]
    fn shipped_periodic_idle_and_patrol_config_match_builder() {
        assert_eq!(ROBOTS_MALFBOT_PERIODIC_IDLE_BASE_DELAY_TICKS, 180);
        assert_eq!(
            ROBOTS_MALFBOT_PERIODIC_IDLE_ANIM_MODES,
            [0x0900_0006, 0x0900_0007]
        );
        let patrol = malfbot_patrol_config();
        assert_eq!(patrol.interval_ticks(), 300);
        assert_eq!(patrol.target_locomotion_scalar, 0.0);
        assert_eq!(patrol.turn_rate, RobotsAiTurnRateInput::Default);
    }
}
