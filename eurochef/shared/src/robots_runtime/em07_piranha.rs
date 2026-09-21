use serde::Serialize;

use super::{
    generic_attack::{RobotsGenericAttackConfig, ROBOTS_GENERIC_ATTACK_PRIORITY},
    script_host::native_script_ftol,
};

pub const ROBOTS_EM07_PIRANHA_FILE_UID: u32 = 0x0100_0066;
pub const ROBOTS_EM07_PIRANHA_ATTACK_ANIM_MODE: u32 = 0x0900_0027;
pub const ROBOTS_EM07_PIRANHA_ATTACK_OUTER_RADIUS: f32 = 8.0;
pub const ROBOTS_EM07_PIRANHA_ATTACK_YAW_TOLERANCE_RADIANS: f32 = std::f32::consts::TAU;
pub const ROBOTS_EM07_PIRANHA_ATTACK_VERTICAL_LIMIT: f32 = 1000.0;
pub const ROBOTS_EM07_PIRANHA_ATTACHMENT_ENTITY_UID: u32 = 0x0200_0039;
pub const ROBOTS_EM07_PIRANHA_ATTACHMENT_DATUM_UID: u32 = 0x0E00_0070;
pub const ROBOTS_EM07_PIRANHA_ATTACHMENT_ROTATION_RADIANS_PER_SECOND: f32 =
    -std::f32::consts::TAU * 3.0;
pub const ROBOTS_EM07_PIRANHA_LINK_FIRST_INDEX: usize = 4;
pub const ROBOTS_EM07_PIRANHA_LINK_END_INDEX: usize = 8;
pub const ROBOTS_EM07_PIRANHA_PERMANENT_SOUND_UID: u32 = 0x1AF0_0388;
pub const ROBOTS_EM07_PIRANHA_PERMANENT_SOUND_PARAMETER: u32 = 100;
pub const ROBOTS_EM07_PIRANHA_ABOVE_SOUND_UID: u32 = 0x1AF0_0387;
pub const ROBOTS_EM07_PIRANHA_TRANSITION_SOUND_UID: u32 = 0x1AF0_0389;
pub const ROBOTS_EM07_PIRANHA_TRANSITION_EFFECT_UID_1: u32 = 0x0400_029C;
pub const ROBOTS_EM07_PIRANHA_TRANSITION_EFFECT_UID_2: u32 = 0x0400_029D;
pub const ROBOTS_EM07_PIRANHA_FIRST_UPDATE_COMPONENT_SCALAR: f32 =
    f32::from_bits(0x414B_D70A);
pub const ROBOTS_EM07_PIRANHA_RANDOM_ANGLE_MODULUS: u32 = 0x274;
pub const ROBOTS_EM07_PIRANHA_RANDOM_SCALE: f32 = 0.01;
pub const ROBOTS_EM07_PIRANHA_DISTANCE_RADIUS_SCALE: f32 = 0.1;
pub const ROBOTS_EM07_PIRANHA_RANDOM_RADIUS_MODULUS_SCALE: f32 = 100.0;
pub const ROBOTS_EM07_PIRANHA_HEIGHT_OFFSET: f32 = 1.0;

pub fn em07_piranha_distance_radius_from_raw(raw: u32) -> f32 {
    ((raw as i32) as f32 * ROBOTS_EM07_PIRANHA_DISTANCE_RADIUS_SCALE).abs()
}

pub fn em07_piranha_random_radius_modulus(radius: f32) -> u32 {
    (radius.max(0.0) * ROBOTS_EM07_PIRANHA_RANDOM_RADIUS_MODULUS_SCALE).trunc() as u32
}

pub fn em07_piranha_random_xz_offset(
    angle_draw: u32,
    radius_draw: u32,
    radius: f32,
) -> [f32; 2] {
    let angle = (angle_draw % ROBOTS_EM07_PIRANHA_RANDOM_ANGLE_MODULUS) as f32
        * ROBOTS_EM07_PIRANHA_RANDOM_SCALE;
    let modulus = em07_piranha_random_radius_modulus(radius);
    let sampled_radius = if modulus == 0 {
        0.0
    } else {
        (radius_draw % modulus) as f32 * ROBOTS_EM07_PIRANHA_RANDOM_SCALE
    };
    [angle.sin() * sampled_radius, angle.cos() * sampled_radius]
}

pub fn em07_piranha_cached_target_from_draws(
    points: &[RobotsEm07PiranhaCachedPoint],
    point_draw: u32,
    angle_draw: u32,
    radius_draw: u32,
) -> Option<[f32; 4]> {
    if points.is_empty() {
        return None;
    }
    let point = points[point_draw as usize % points.len()];
    let offset =
        em07_piranha_random_xz_offset(angle_draw, radius_draw, point.native_radius);
    let mut target = point.position_xyzw;
    target[0] += offset[0];
    target[2] += offset[1];
    Some(target)
}

pub fn em07_piranha_creator_target_radius(raw_bits: u32) -> f32 {
    f32::from_bits(raw_bits).max(0.0)
}

pub fn em07_piranha_creator_retarget_ticks(raw_bits: u32) -> i32 {
    native_script_ftol(f32::from_bits(raw_bits))
}

pub fn em07_piranha_creator_spawn_position_from_draws(
    creator_position_xyzw: [f32; 4],
    proximity_radius: f32,
    angle_draw: u32,
    radius_draw: u32,
) -> [f32; 4] {
    let offset = em07_piranha_random_xz_offset(angle_draw, radius_draw, proximity_radius);
    [
        creator_position_xyzw[0] + offset[0],
        creator_position_xyzw[1] - ROBOTS_EM07_PIRANHA_HEIGHT_OFFSET,
        creator_position_xyzw[2] + offset[1],
        creator_position_xyzw[3],
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsEm07PiranhaFlightCycleState {
    pub retarget_period_ticks: i32,
    pub retarget_countdown_ticks: i32,
    pub flight_active: bool,
}

impl RobotsEm07PiranhaFlightCycleState {
    pub const fn new(retarget_period_ticks: i32) -> Self {
        Self {
            retarget_period_ticks,
            retarget_countdown_ticks: retarget_period_ticks,
            flight_active: false,
        }
    }

    /// Native `0x004676F0`: while +0x654 is clear, subtract exactly one
    /// fixed update from +0x644. Crossing <=0 reloads +0x640 and starts a flight.
    pub fn tick_waiting(&mut self) -> bool {
        if self.flight_active {
            return false;
        }
        self.retarget_countdown_ticks = self.retarget_countdown_ticks.wrapping_sub(1);
        if self.retarget_countdown_ticks > 0 {
            return false;
        }
        self.retarget_countdown_ticks = self.retarget_period_ticks;
        true
    }

    pub fn mark_flight_started(&mut self) {
        self.flight_active = true;
    }

    pub fn mark_flight_completed(&mut self) {
        self.flight_active = false;
    }
}

pub const fn em07_piranha_attack_config() -> RobotsGenericAttackConfig {
    RobotsGenericAttackConfig {
        primary_anim_mode: ROBOTS_EM07_PIRANHA_ATTACK_ANIM_MODE,
        secondary_anim_mode: None,
        inner_radius: 0.0,
        outer_radius: ROBOTS_EM07_PIRANHA_ATTACK_OUTER_RADIUS,
        yaw_tolerance_radians: ROBOTS_EM07_PIRANHA_ATTACK_YAW_TOLERANCE_RADIANS,
        vertical_limit: ROBOTS_EM07_PIRANHA_ATTACK_VERTICAL_LIMIT,
        reentry_delay_ticks: 0,
        secondary_repeat_count: 0,
        sticky_while_active: false,
        priority: ROBOTS_GENERIC_ATTACK_PRIORITY,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsEm07PiranhaBehaviorWinner {
    CommonIdle,
    Attack,
}

pub const fn em07_piranha_behavior_winner(
    common_idle_priority: u8,
    attack_priority: u8,
) -> Option<RobotsEm07PiranhaBehaviorWinner> {
    let mut best_priority = 1u8;
    let mut selected = None;
    if best_priority < common_idle_priority {
        best_priority = common_idle_priority;
        selected = Some(RobotsEm07PiranhaBehaviorWinner::CommonIdle);
    }
    if best_priority < attack_priority {
        selected = Some(RobotsEm07PiranhaBehaviorWinner::Attack);
    }
    selected
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsEm07PiranhaCachedPoint {
    pub position_xyzw: [f32; 4],
    pub native_radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsEm07PiranhaHeightTransition {
    EnteredAbove,
    EnteredBelow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsEm07PiranhaHeightTransitionPlan {
    pub stop_loop_sound_uid: u32,
    pub start_loop_sound_uid: u32,
    pub one_shot_sound_uid: u32,
    pub script_uid: Option<u32>,
}

pub const fn em07_piranha_height_transition_plan(
    transition: RobotsEm07PiranhaHeightTransition,
    splash_selector: u32,
) -> RobotsEm07PiranhaHeightTransitionPlan {
    let (stop_loop_sound_uid, start_loop_sound_uid) = match transition {
        RobotsEm07PiranhaHeightTransition::EnteredAbove => (
            ROBOTS_EM07_PIRANHA_PERMANENT_SOUND_UID,
            ROBOTS_EM07_PIRANHA_ABOVE_SOUND_UID,
        ),
        RobotsEm07PiranhaHeightTransition::EnteredBelow => (
            ROBOTS_EM07_PIRANHA_ABOVE_SOUND_UID,
            ROBOTS_EM07_PIRANHA_PERMANENT_SOUND_UID,
        ),
    };
    let script_uid = match splash_selector {
        1 => Some(ROBOTS_EM07_PIRANHA_TRANSITION_EFFECT_UID_1),
        2 => Some(ROBOTS_EM07_PIRANHA_TRANSITION_EFFECT_UID_2),
        _ => None,
    };
    RobotsEm07PiranhaHeightTransitionPlan {
        stop_loop_sound_uid,
        start_loop_sound_uid,
        one_shot_sound_uid: ROBOTS_EM07_PIRANHA_TRANSITION_SOUND_UID,
        script_uid,
    }
}

pub const fn em07_piranha_attachment_rotation_per_fixed_tick() -> f32 {
    ROBOTS_EM07_PIRANHA_ATTACHMENT_ROTATION_RADIANS_PER_SECOND / 60.0
}

pub fn em07_piranha_owner_pitch_yaw_from_physics(
    velocity_xyz: [f32; 3],
    gravity_velocity_y: f32,
) -> [f32; 2] {
    let horizontal = (velocity_xyz[0] * velocity_xyz[0]
        + velocity_xyz[2] * velocity_xyz[2])
        .max(0.0)
        .sqrt();
    let yaw = velocity_xyz[0].atan2(velocity_xyz[2]);
    let pitch = horizontal.atan2(gravity_velocity_y + velocity_xyz[1])
        - std::f32::consts::FRAC_PI_2;
    [pitch, yaw]
}

pub fn em07_piranha_flight_reset_ready(owner_y: f32, creator_y: f32) -> bool {
    owner_y - creator_y < -ROBOTS_EM07_PIRANHA_HEIGHT_OFFSET
}

pub const fn em07_piranha_height_transition(
    was_above: bool,
    relative_height: f32,
) -> Option<RobotsEm07PiranhaHeightTransition> {
    if relative_height >= 0.0 {
        if was_above {
            None
        } else {
            Some(RobotsEm07PiranhaHeightTransition::EnteredAbove)
        }
    } else if was_above {
        Some(RobotsEm07PiranhaHeightTransition::EnteredBelow)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctor_builder_and_first_update_constants_match_00467580_00467600_00467c60() {
        let attack = em07_piranha_attack_config();
        assert_eq!(attack.primary_anim_mode, 0x0900_0027);
        assert_eq!(attack.outer_radius.to_bits(), 8.0f32.to_bits());
        assert_eq!(
            attack.yaw_tolerance_radians.to_bits(),
            std::f32::consts::TAU.to_bits()
        );
        assert_eq!(attack.vertical_limit.to_bits(), 1000.0f32.to_bits());
        assert_eq!(attack.reentry_delay_ticks, 0);
        assert!(!attack.sticky_while_active);
        assert_eq!(attack.priority, 0x32);
        assert_eq!(ROBOTS_EM07_PIRANHA_FILE_UID, 0x0100_0066);
        assert_eq!(ROBOTS_EM07_PIRANHA_ATTACHMENT_ENTITY_UID, 0x0200_0039);
        assert_eq!(ROBOTS_EM07_PIRANHA_ATTACHMENT_DATUM_UID, 0x0E00_0070);
        assert_eq!(
            ROBOTS_EM07_PIRANHA_FIRST_UPDATE_COMPONENT_SCALAR.to_bits(),
            12.74f32.to_bits()
        );
        assert_eq!(ROBOTS_EM07_PIRANHA_PERMANENT_SOUND_UID, 0x1AF0_0388);
        assert_eq!(ROBOTS_EM07_PIRANHA_LINK_FIRST_INDEX, 4);
        assert_eq!(ROBOTS_EM07_PIRANHA_LINK_END_INDEX, 8);
    }

    #[test]
    fn selector_and_attachment_rotation_preserve_native_order_and_fixed_step() {
        assert_eq!(
            em07_piranha_behavior_winner(2, 0x32),
            Some(RobotsEm07PiranhaBehaviorWinner::Attack)
        );
        assert_eq!(
            em07_piranha_behavior_winner(2, 2),
            Some(RobotsEm07PiranhaBehaviorWinner::CommonIdle)
        );
        assert_eq!(
            em07_piranha_attachment_rotation_per_fixed_tick().to_bits(),
            (-std::f32::consts::PI / 10.0).to_bits()
        );
    }

    #[test]
    fn owner_pitch_yaw_matches_004679c0_00467a09_x87_operand_order() {
        let [pitch, yaw] = em07_piranha_owner_pitch_yaw_from_physics([1.0, 0.0, 0.0], 0.0);
        assert_eq!(pitch.to_bits(), 0.0f32.to_bits());
        assert!((yaw - std::f32::consts::FRAC_PI_2).abs() < 1.0e-6);

        let [pitch, yaw] = em07_piranha_owner_pitch_yaw_from_physics([0.0, 0.0, 1.0], 0.0);
        assert_eq!(pitch.to_bits(), 0.0f32.to_bits());
        assert_eq!(yaw.to_bits(), 0.0f32.to_bits());

        let [pitch, _] = em07_piranha_owner_pitch_yaw_from_physics([0.0, 0.0, 0.0], 1.0);
        assert!((pitch + std::f32::consts::FRAC_PI_2).abs() < 1.0e-6);
    }

    #[test]
    fn radius_modulus_preserves_native_conditional_rng_gate() {
        assert_eq!(em07_piranha_random_radius_modulus(0.0), 0);
        assert_eq!(em07_piranha_random_radius_modulus(-3.0), 0);
        assert_eq!(em07_piranha_random_radius_modulus(2.5), 250);
    }

    #[test]
    fn flight_reset_is_strictly_below_creator_minus_one_like_00467a22() {
        assert!(!em07_piranha_flight_reset_ready(9.0, 10.0));
        assert!(em07_piranha_flight_reset_ready(8.999, 10.0));
        assert!(!em07_piranha_flight_reset_ready(10.0, 10.0));
    }

    #[test]
    fn height_crossing_latch_is_edge_triggered_like_004676f0_00467b50() {
        assert_eq!(
            em07_piranha_height_transition(false, 0.0),
            Some(RobotsEm07PiranhaHeightTransition::EnteredAbove)
        );
        assert_eq!(em07_piranha_height_transition(true, 1.0), None);
        assert_eq!(
            em07_piranha_height_transition(true, -0.01),
            Some(RobotsEm07PiranhaHeightTransition::EnteredBelow)
        );
        assert_eq!(em07_piranha_height_transition(false, -1.0), None);
    }

    #[test]
    fn height_transition_plan_matches_00467b50_loop_switch_one_shot_and_splash_table() {
        let above = em07_piranha_height_transition_plan(
            RobotsEm07PiranhaHeightTransition::EnteredAbove,
            1,
        );
        assert_eq!(above.stop_loop_sound_uid, 0x1AF0_0388);
        assert_eq!(above.start_loop_sound_uid, 0x1AF0_0387);
        assert_eq!(above.one_shot_sound_uid, 0x1AF0_0389);
        assert_eq!(above.script_uid, Some(0x0400_029C));

        let below = em07_piranha_height_transition_plan(
            RobotsEm07PiranhaHeightTransition::EnteredBelow,
            2,
        );
        assert_eq!(below.stop_loop_sound_uid, 0x1AF0_0387);
        assert_eq!(below.start_loop_sound_uid, 0x1AF0_0388);
        assert_eq!(below.one_shot_sound_uid, 0x1AF0_0389);
        assert_eq!(below.script_uid, Some(0x0400_029D));

        assert_eq!(
            em07_piranha_height_transition_plan(
                RobotsEm07PiranhaHeightTransition::EnteredAbove,
                0,
            )
            .script_uid,
            None
        );
        assert_eq!(
            em07_piranha_height_transition_plan(
                RobotsEm07PiranhaHeightTransition::EnteredAbove,
                3,
            )
            .script_uid,
            None
        );
    }

    #[test]
    fn distance_radius_and_cached_target_rng_match_00467c60_004676f0() {
        assert_eq!(
            em07_piranha_distance_radius_from_raw(25).to_bits(),
            2.5f32.to_bits()
        );
        assert_eq!(
            em07_piranha_distance_radius_from_raw((-25i32) as u32).to_bits(),
            2.5f32.to_bits()
        );

        let point = RobotsEm07PiranhaCachedPoint {
            position_xyzw: [10.0, 20.0, 30.0, 0.0],
            native_radius: 2.5,
        };
        let offset = em07_piranha_random_xz_offset(0, 149, point.native_radius);
        assert_eq!(offset[0].to_bits(), 0.0f32.to_bits());
        assert_eq!(offset[1].to_bits(), 1.49f32.to_bits());

        let target = em07_piranha_cached_target_from_draws(&[point], 123, 0, 149).unwrap();
        assert_eq!(target[0].to_bits(), 10.0f32.to_bits());
        assert_eq!(target[1].to_bits(), 20.0f32.to_bits());
        assert_eq!(target[2].to_bits(), 31.49f32.to_bits());
        assert_eq!(target[3].to_bits(), 0.0f32.to_bits());
    }

    #[test]
    fn creator_fields_spawn_and_cycle_match_monster_fish_vslots_and_004676f0() {
        assert_eq!(
            em07_piranha_creator_target_radius(3.5f32.to_bits()).to_bits(),
            3.5f32.to_bits()
        );
        assert_eq!(
            em07_piranha_creator_target_radius((-3.5f32).to_bits()).to_bits(),
            0.0f32.to_bits()
        );
        assert_eq!(
            em07_piranha_creator_retarget_ticks(12.9f32.to_bits()),
            12
        );

        let spawn =
            em07_piranha_creator_spawn_position_from_draws([5.0, 7.0, 11.0, 0.0], 2.0, 0, 99);
        assert_eq!(spawn[0].to_bits(), 5.0f32.to_bits());
        assert_eq!(spawn[1].to_bits(), 6.0f32.to_bits());
        assert_eq!(spawn[2].to_bits(), 11.99f32.to_bits());

        let mut cycle = RobotsEm07PiranhaFlightCycleState::new(2);
        assert!(!cycle.tick_waiting());
        assert!(cycle.tick_waiting());
        assert_eq!(cycle.retarget_countdown_ticks, 2);
        cycle.mark_flight_started();
        assert!(!cycle.tick_waiting());
        assert_eq!(cycle.retarget_countdown_ticks, 2);
        cycle.mark_flight_completed();
        assert!(!cycle.tick_waiting());
        assert!(cycle.tick_waiting());
    }
}
