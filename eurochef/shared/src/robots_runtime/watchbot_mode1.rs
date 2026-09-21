use serde::Serialize;

use super::watchbot_component::{
    watchbot_component_idle_timer_from_draw, watchbot_component_steering_step,
    RobotsWatchbotComponentSteeringInput, RobotsWatchbotComponentSteeringStep,
    ROBOTS_WATCHBOT_COMPONENT_EPSILON, ROBOTS_WATCHBOT_COMPONENT_FIXED_STEP_SECONDS,
};

pub const ROBOTS_WATCHBOT_MODE1_STATE_DRIVE: u32 = 3;
pub const ROBOTS_WATCHBOT_MODE1_STATE_FAR: u32 = 7;
pub const ROBOTS_WATCHBOT_MODE1_STATE_COMPLETION_BYPASS: u32 = 9;

pub const ROBOTS_WATCHBOT_MODE1_ORBIT_RADIUS: f32 = 1.5;
pub const ROBOTS_WATCHBOT_MODE1_ORBIT_VERTICAL_OFFSET_LOW: f32 = 1.5;
pub const ROBOTS_WATCHBOT_MODE1_ORBIT_VERTICAL_OFFSET_MID: f32 = 1.875;
pub const ROBOTS_WATCHBOT_MODE1_ORBIT_VERTICAL_OFFSET_HIGH: f32 = 2.25;
pub const ROBOTS_WATCHBOT_MODE1_TEMPORARY_TARGET_SECONDS: f32 = 10.0;
pub const ROBOTS_WATCHBOT_MODE1_TEMPORARY_TARGET_Y_OFFSET: f32 = 0.25;
pub const ROBOTS_WATCHBOT_MODE1_TEMPORARY_TARGET_CLEAR_DISTANCE_SQUARED: f32 = 0.5;
pub const ROBOTS_WATCHBOT_MODE1_FAR_DISTANCE_SQUARED: f32 = 100.0;
pub const ROBOTS_WATCHBOT_MODE1_COMPLETION_XZ_DISTANCE_SQUARED: f32 = 7.25;
pub const ROBOTS_WATCHBOT_MODE1_STEERING_DAMPING: f32 = f32::from_bits(0x3F73_3333); // 0.95
pub const ROBOTS_WATCHBOT_MODE1_STEERING_STRENGTH: f32 = f32::from_bits(0x3E99_999A); // 0.3
pub const ROBOTS_WATCHBOT_MODE1_FALLBACK_Y_OFFSET: f32 = 1.0;
pub const ROBOTS_WATCHBOT_MODE1_COMPLETION_BLOCKED_ANIM_MODES: [u32; 3] =
    [0x0900_0071, 0x0900_0072, 0x0900_00A8];

const ROBOTS_WATCHBOT_MODE1_ORBIT_ANGLES: [f32; 8] = [
    f32::from_bits(0xC049_0FDB), // -pi
    f32::from_bits(0x401A_A9BD),
    f32::from_bits(0xBFC9_0FDB), // -pi/2
    f32::from_bits(0x3F9A_A9BD),
    0.0,
    f32::from_bits(0xBF9A_A9BD),
    f32::from_bits(0x3FC9_0FDB), // +pi/2
    f32::from_bits(0xC01A_A9BD),
];

const ROBOTS_WATCHBOT_MODE1_ORBIT_VERTICAL_OFFSETS: [f32; 8] = [
    ROBOTS_WATCHBOT_MODE1_ORBIT_VERTICAL_OFFSET_LOW,
    ROBOTS_WATCHBOT_MODE1_ORBIT_VERTICAL_OFFSET_LOW,
    ROBOTS_WATCHBOT_MODE1_ORBIT_VERTICAL_OFFSET_MID,
    ROBOTS_WATCHBOT_MODE1_ORBIT_VERTICAL_OFFSET_MID,
    ROBOTS_WATCHBOT_MODE1_ORBIT_VERTICAL_OFFSET_HIGH,
    ROBOTS_WATCHBOT_MODE1_ORBIT_VERTICAL_OFFSET_LOW,
    ROBOTS_WATCHBOT_MODE1_ORBIT_VERTICAL_OFFSET_MID,
    ROBOTS_WATCHBOT_MODE1_ORBIT_VERTICAL_OFFSET_HIGH,
];

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotMode1PrepareInput {
    /// Live controlled WatchBot XItem +0xD0..+0xDC.
    pub owner_position_xyzw: [f32; 4],
    /// Handler-bound target XItem +0xD0..+0xDC.
    pub target_position_xyzw: [f32; 4],
    /// Optional point exposed by target Handler +0x6BC. The pointed native payload
    /// starts at +4 and contains the four values copied into component +0xA0..+0xAC.
    pub target_temporary_point_xyzw: Option<[f32; 4]>,
    /// Native DAT_00620034 used to age the temporary-target lane.
    pub runtime_rate_scale: f32,
}

/// Pure phase of mode1 locomotion `0x00492C90`: chooses the orbit/temporary point
/// before the host performs native world query `0x004940F0`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotMode1PreparedTarget {
    pub owner_position_xyzw: [f32; 4],
    pub target_position_xyzw: [f32; 4],
    pub visibility_target_xyzw: [f32; 4],
    pub used_temporary_target: bool,
    pub temporary_target_seconds_remaining: f32,
    pub orbit_index: u8,
    #[serde(skip)]
    next_temporary_target_xyzw_a0: Option<[f32; 4]>,
    #[serde(skip)]
    next_temporary_target_seconds_ec: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotMode1VisibilityInput {
    /// Result of host world/collision query equivalent to `0x004940F0`.
    pub path_clear: bool,
    /// Controlled WatchBot XItem +0x161 bit 1 (mask 2).
    pub owner_flag_161_bit2: bool,
    /// Host projection of DAT_007B2BE0..DAT_007B2BEC. Native adds +1.0 to Y
    /// before the special blocked-path snap.
    pub fallback_position_xyzw: [f32; 4],
    /// True when the current GameWnd/state-stack mode suppresses the native
    /// distance>100 transition to state7.
    pub far_transition_suppressed: bool,
    /// Raw process-global `FUN_00509C48` draw. Required exactly once when
    /// `path_clear == false`; ignored otherwise.
    pub blocked_path_rng_draw: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotMode1SteeringInput {
    pub owner_position_xyzw: [f32; 4],
    pub owner_rotation_xyzw: [f32; 4],
    pub steering_target_xyzw: [f32; 4],
    pub bound_target_position_xyzw: [f32; 4],
    pub internal_state: u32,
    pub player_yaw_radians: Option<f32>,
    pub global_override_position_xyzw: Option<[f32; 4]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotMode1Step {
    pub next_internal_state: u32,
    /// Point passed to common steering helper `0x004943B0` when `run_steering` is true.
    pub steering_target_xyzw: [f32; 4],
    pub run_steering: bool,
    /// Special blocked-path branch writes this transform through `0x004936D0`.
    pub snap_owner_position_xyzw: Option<[f32; 4]>,
    pub steering_damping: f32,
    pub steering_strength: f32,
    pub orbit_index: u8,
    pub rng_draws_consumed: u8,
}

/// Engine-neutral state owned by WatchBot component mode1 (`0x00490D90`,
/// vtable `0x005ECCF0`). World traces and the common body/physics accumulator stay
/// host-owned; this runtime owns only deterministic target/orbit/timer decisions.
#[derive(Debug, Clone, PartialEq)]
pub struct RobotsWatchbotMode1Runtime {
    /// Native component +0xB0. Only low three bits are currently decoded as the
    /// orbit selector; higher bits are preserved across rerolls.
    pub orbit_flags_b0: u32,
    pub idle_timer_8e: Option<u16>,
    /// Common component steering velocity at native +0x30..+0x3C.
    pub velocity_xyzw_30: [f32; 4],
    temporary_target_xyzw_a0: Option<[f32; 4]>,
    temporary_target_seconds_ec: f32,
}

impl Default for RobotsWatchbotMode1Runtime {
    fn default() -> Self {
        Self {
            orbit_flags_b0: 0,
            idle_timer_8e: None,
            velocity_xyzw_30: [0.0; 4],
            temporary_target_xyzw_a0: None,
            temporary_target_seconds_ec: 0.0,
        }
    }
}

impl RobotsWatchbotMode1Runtime {
    /// Common component setup `0x004935A0` followed by mode1 setup `0x00490ED0`.
    /// Re-entry clears the temporary-target lane but does not overwrite +0xB0;
    /// the native mode1 setup likewise leaves the orbit selector untouched.
    pub fn enter_component_with_setup_draw(&mut self, setup_rng_draw: Option<u32>) {
        self.idle_timer_8e = setup_rng_draw.map(watchbot_component_idle_timer_from_draw);
        self.velocity_xyzw_30 = [0.0; 4];
        self.temporary_target_xyzw_a0 = None;
        self.temporary_target_seconds_ec = 0.0;
    }

    /// Phase 1 of native `0x00492C90`/`0x00492B40`. It is intentionally pure so a
    /// host with no known global RNG seed can fail a later blocked visibility result
    /// without partially advancing this component runtime.
    pub fn prepare_target(
        &self,
        input: RobotsWatchbotMode1PrepareInput,
    ) -> RobotsWatchbotMode1PreparedTarget {
        let orbit_index = (self.orbit_flags_b0 & 7) as u8;
        let mut visibility_target_xyzw =
            mode1_orbit_target(input.target_position_xyzw, orbit_index);
        let mut next_temporary_target = self.temporary_target_xyzw_a0;
        let mut next_temporary_seconds = self.temporary_target_seconds_ec;
        let mut used_temporary_target = false;

        if let Some(source) = input.target_temporary_point_xyzw {
            if next_temporary_seconds < ROBOTS_WATCHBOT_COMPONENT_EPSILON {
                next_temporary_target = Some(source);
                next_temporary_seconds = ROBOTS_WATCHBOT_MODE1_TEMPORARY_TARGET_SECONDS;
            }
        }

        if ROBOTS_WATCHBOT_COMPONENT_EPSILON < next_temporary_seconds {
            next_temporary_seconds -=
                input.runtime_rate_scale * ROBOTS_WATCHBOT_COMPONENT_FIXED_STEP_SECONDS;
            if let Some(mut temporary_target) = next_temporary_target {
                temporary_target[1] =
                    input.target_position_xyzw[1] + ROBOTS_WATCHBOT_MODE1_TEMPORARY_TARGET_Y_OFFSET;
                visibility_target_xyzw = temporary_target;
                used_temporary_target = true;

                let distance_squared =
                    squared_distance4(visibility_target_xyzw, input.owner_position_xyzw);
                if distance_squared < ROBOTS_WATCHBOT_MODE1_TEMPORARY_TARGET_CLEAR_DISTANCE_SQUARED
                    || next_temporary_seconds < ROBOTS_WATCHBOT_COMPONENT_EPSILON
                {
                    next_temporary_target = None;
                    next_temporary_seconds = 0.0;
                }
            }
        }

        RobotsWatchbotMode1PreparedTarget {
            owner_position_xyzw: input.owner_position_xyzw,
            target_position_xyzw: input.target_position_xyzw,
            visibility_target_xyzw,
            used_temporary_target,
            temporary_target_seconds_remaining: next_temporary_seconds,
            orbit_index,
            next_temporary_target_xyzw_a0: next_temporary_target,
            next_temporary_target_seconds_ec: next_temporary_seconds,
        }
    }

    /// Phase 2 of `0x00492C90`, after the host has run world query `0x004940F0`.
    /// A blocked path consumes exactly one global RNG draw before either native
    /// fallback branch. The common steering math `0x004943B0` remains a separate
    /// shared/host boundary and is not approximated here.
    pub fn resolve_visibility(
        &mut self,
        prepared: RobotsWatchbotMode1PreparedTarget,
        input: RobotsWatchbotMode1VisibilityInput,
    ) -> Option<RobotsWatchbotMode1Step> {
        let blocked_draw = if input.path_clear {
            None
        } else {
            Some(input.blocked_path_rng_draw?)
        };

        self.temporary_target_xyzw_a0 = prepared.next_temporary_target_xyzw_a0;
        self.temporary_target_seconds_ec = prepared.next_temporary_target_seconds_ec;

        if input.path_clear {
            return Some(mode1_step(
                ROBOTS_WATCHBOT_MODE1_STATE_DRIVE,
                prepared.visibility_target_xyzw,
                true,
                None,
                (self.orbit_flags_b0 & 7) as u8,
                0,
            ));
        }

        let draw = blocked_draw.expect("blocked path draw was checked above");
        self.orbit_flags_b0 = (self.orbit_flags_b0 & !7) | (draw & 7);
        let orbit_index = (self.orbit_flags_b0 & 7) as u8;

        if input.owner_flag_161_bit2 {
            let mut snapped = input.fallback_position_xyzw;
            snapped[1] += ROBOTS_WATCHBOT_MODE1_FALLBACK_Y_OFFSET;
            return Some(mode1_step(
                ROBOTS_WATCHBOT_MODE1_STATE_DRIVE,
                prepared.visibility_target_xyzw,
                false,
                Some(snapped),
                orbit_index,
                1,
            ));
        }

        let distance_squared =
            squared_distance3(prepared.owner_position_xyzw, prepared.target_position_xyzw);
        if !input.far_transition_suppressed
            && ROBOTS_WATCHBOT_MODE1_FAR_DISTANCE_SQUARED < distance_squared
        {
            return Some(mode1_step(
                ROBOTS_WATCHBOT_MODE1_STATE_FAR,
                prepared.owner_position_xyzw,
                false,
                None,
                orbit_index,
                1,
            ));
        }

        Some(mode1_step(
            ROBOTS_WATCHBOT_MODE1_STATE_DRIVE,
            prepared.owner_position_xyzw,
            true,
            None,
            orbit_index,
            1,
        ))
    }
}

/// Common wrapper `0x00494390` / steering helper `0x004943B0` for mode1.
/// The real owner transform/body writes stay host-owned; runtime retains only
/// component +0x30..+0x3C velocity between fixed updates.
pub fn advance_watchbot_mode1_steering(
    runtime: &mut RobotsWatchbotMode1Runtime,
    input: RobotsWatchbotMode1SteeringInput,
) -> RobotsWatchbotComponentSteeringStep {
    let step = watchbot_component_steering_step(
        runtime.velocity_xyzw_30,
        RobotsWatchbotComponentSteeringInput {
            owner_position_xyzw: input.owner_position_xyzw,
            owner_rotation_xyzw: input.owner_rotation_xyzw,
            desired_target_xyzw: input.steering_target_xyzw,
            bound_target_position_xyzw: input.bound_target_position_xyzw,
            internal_state: input.internal_state,
            player_yaw_radians: input.player_yaw_radians,
            global_override_position_xyzw: input.global_override_position_xyzw,
            damping: ROBOTS_WATCHBOT_MODE1_STEERING_DAMPING,
            strength: ROBOTS_WATCHBOT_MODE1_STEERING_STRENGTH,
            suppress_close_target_avoidance: false,
        },
    );
    runtime.velocity_xyzw_30 = step.velocity_xyzw_30;
    step
}

/// Mode1 vslot +0x84 (`0x00493180`). Internal state9 bypasses the remaining
/// checks. Otherwise the target handler must be state2/3, the current AnimMode
/// must not be one of 0x71/0x72/0xA8, and target-owner XZ distance² must be <=7.25.
pub fn watchbot_mode1_completion_predicate(
    current_internal_state: u32,
    target_handler_state_6de: u8,
    current_anim_mode_uid: u32,
    owner_position_xyz: [f32; 3],
    target_position_xyz: [f32; 3],
) -> bool {
    if current_internal_state == ROBOTS_WATCHBOT_MODE1_STATE_COMPLETION_BYPASS {
        return true;
    }
    if !matches!(target_handler_state_6de, 2 | 3) {
        return false;
    }
    if ROBOTS_WATCHBOT_MODE1_COMPLETION_BLOCKED_ANIM_MODES.contains(&current_anim_mode_uid) {
        return false;
    }
    let dx = target_position_xyz[0] - owner_position_xyz[0];
    let dz = target_position_xyz[2] - owner_position_xyz[2];
    dx * dx + dz * dz <= ROBOTS_WATCHBOT_MODE1_COMPLETION_XZ_DISTANCE_SQUARED
}

pub fn mode1_orbit_target(target_position_xyzw: [f32; 4], orbit_index: u8) -> [f32; 4] {
    let index = (orbit_index & 7) as usize;
    let angle = ROBOTS_WATCHBOT_MODE1_ORBIT_ANGLES[index];
    let sin = (angle as f64).sin() as f32;
    let cos = (angle as f64).cos() as f32;
    [
        target_position_xyzw[0] - sin * ROBOTS_WATCHBOT_MODE1_ORBIT_RADIUS,
        target_position_xyzw[1] + ROBOTS_WATCHBOT_MODE1_ORBIT_VERTICAL_OFFSETS[index],
        target_position_xyzw[2] - cos * ROBOTS_WATCHBOT_MODE1_ORBIT_RADIUS,
        target_position_xyzw[3],
    ]
}

fn mode1_step(
    next_internal_state: u32,
    steering_target_xyzw: [f32; 4],
    run_steering: bool,
    snap_owner_position_xyzw: Option<[f32; 4]>,
    orbit_index: u8,
    rng_draws_consumed: u8,
) -> RobotsWatchbotMode1Step {
    RobotsWatchbotMode1Step {
        next_internal_state,
        steering_target_xyzw,
        run_steering,
        snap_owner_position_xyzw,
        steering_damping: ROBOTS_WATCHBOT_MODE1_STEERING_DAMPING,
        steering_strength: ROBOTS_WATCHBOT_MODE1_STEERING_STRENGTH,
        orbit_index,
        rng_draws_consumed,
    }
}

fn squared_distance3(a: [f32; 4], b: [f32; 4]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}

fn squared_distance4(a: [f32; 4], b: [f32; 4]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    let dw = a[3] - b[3];
    dx * dx + dy * dy + dz * dz + dw * dw
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepare_input() -> RobotsWatchbotMode1PrepareInput {
        RobotsWatchbotMode1PrepareInput {
            owner_position_xyzw: [0.0, 0.0, 0.0, 0.0],
            target_position_xyzw: [10.0, 5.0, 20.0, 0.0],
            target_temporary_point_xyzw: None,
            runtime_rate_scale: 1.0,
        }
    }

    #[test]
    fn orbit_table_keeps_native_non_uniform_angles_and_vertical_levels() {
        let target = [10.0, 5.0, 20.0, 7.0];
        let zero = mode1_orbit_target(target, 0);
        assert!((zero[0] - 10.0).abs() < 1.0e-6);
        assert!((zero[1] - 6.5).abs() < 1.0e-6);
        assert!((zero[2] - 21.5).abs() < 1.0e-6);
        assert_eq!(zero[3], 7.0);

        let four = mode1_orbit_target(target, 4);
        assert!((four[0] - 10.0).abs() < 1.0e-6);
        assert!((four[1] - 7.25).abs() < 1.0e-6);
        assert!((four[2] - 18.5).abs() < 1.0e-6);

        let three = mode1_orbit_target(target, 3);
        let five = mode1_orbit_target(target, 5);
        assert!((three[0] + five[0] - 20.0).abs() < 1.0e-5);
        assert!((three[1] - 6.875).abs() < 1.0e-6);
        assert!((five[1] - 6.5).abs() < 1.0e-6);
    }

    #[test]
    fn temporary_target_is_latched_for_ten_seconds_and_uses_target_y_plus_quarter() {
        let runtime = RobotsWatchbotMode1Runtime::default();
        let mut input = prepare_input();
        input.target_temporary_point_xyzw = Some([30.0, 99.0, 40.0, 0.0]);
        let prepared = runtime.prepare_target(input);
        assert!(prepared.used_temporary_target);
        assert_eq!(prepared.visibility_target_xyzw, [30.0, 5.25, 40.0, 0.0]);
        assert!((prepared.temporary_target_seconds_remaining - (10.0 - 1.0 / 60.0)).abs() < 1.0e-6);
    }

    #[test]
    fn clear_path_uses_no_rng() {
        let mut runtime = RobotsWatchbotMode1Runtime::default();
        let prepared = runtime.prepare_target(prepare_input());
        let step = runtime
            .resolve_visibility(
                prepared,
                RobotsWatchbotMode1VisibilityInput {
                    path_clear: true,
                    owner_flag_161_bit2: false,
                    fallback_position_xyzw: [0.0; 4],
                    far_transition_suppressed: false,
                    blocked_path_rng_draw: None,
                },
            )
            .unwrap();
        assert_eq!(step.next_internal_state, ROBOTS_WATCHBOT_MODE1_STATE_DRIVE);
        assert!(step.run_steering);
        assert_eq!(step.rng_draws_consumed, 0);
    }

    #[test]
    fn blocked_path_consumes_one_draw_and_keeps_strict_100_boundary() {
        let mut runtime = RobotsWatchbotMode1Runtime::default();
        let mut input = prepare_input();
        input.target_position_xyzw = [10.0, 0.0, 0.0, 0.0];
        let boundary = runtime.prepare_target(input);
        let boundary_step = runtime
            .resolve_visibility(
                boundary,
                RobotsWatchbotMode1VisibilityInput {
                    path_clear: false,
                    owner_flag_161_bit2: false,
                    fallback_position_xyzw: [0.0; 4],
                    far_transition_suppressed: false,
                    blocked_path_rng_draw: Some(5),
                },
            )
            .unwrap();
        assert_eq!(
            boundary_step.next_internal_state,
            ROBOTS_WATCHBOT_MODE1_STATE_DRIVE
        );
        assert!(boundary_step.run_steering);
        assert_eq!(boundary_step.orbit_index, 5);
        assert_eq!(boundary_step.rng_draws_consumed, 1);

        let mut input = prepare_input();
        input.target_position_xyzw = [10.01, 0.0, 0.0, 0.0];
        let far = runtime.prepare_target(input);
        let far_step = runtime
            .resolve_visibility(
                far,
                RobotsWatchbotMode1VisibilityInput {
                    path_clear: false,
                    owner_flag_161_bit2: false,
                    fallback_position_xyzw: [0.0; 4],
                    far_transition_suppressed: false,
                    blocked_path_rng_draw: Some(2),
                },
            )
            .unwrap();
        assert_eq!(
            far_step.next_internal_state,
            ROBOTS_WATCHBOT_MODE1_STATE_FAR
        );
        assert!(!far_step.run_steering);
        assert_eq!(far_step.orbit_index, 2);
    }

    #[test]
    fn blocked_special_flag_rerolls_then_returns_native_fallback_snap() {
        let mut runtime = RobotsWatchbotMode1Runtime::default();
        let prepared = runtime.prepare_target(prepare_input());
        let step = runtime
            .resolve_visibility(
                prepared,
                RobotsWatchbotMode1VisibilityInput {
                    path_clear: false,
                    owner_flag_161_bit2: true,
                    fallback_position_xyzw: [1.0, 2.0, 3.0, 4.0],
                    far_transition_suppressed: false,
                    blocked_path_rng_draw: Some(7),
                },
            )
            .unwrap();
        assert_eq!(step.orbit_index, 7);
        assert_eq!(step.rng_draws_consumed, 1);
        assert!(!step.run_steering);
        assert_eq!(step.snap_owner_position_xyzw, Some([1.0, 3.0, 3.0, 4.0]));
    }

    #[test]
    fn blocked_path_without_global_rng_fails_before_runtime_mutation() {
        let mut runtime = RobotsWatchbotMode1Runtime::default();
        runtime.orbit_flags_b0 = 0x18 | 3;
        let prepared = runtime.prepare_target(prepare_input());
        assert!(runtime
            .resolve_visibility(
                prepared,
                RobotsWatchbotMode1VisibilityInput {
                    path_clear: false,
                    owner_flag_161_bit2: false,
                    fallback_position_xyzw: [0.0; 4],
                    far_transition_suppressed: false,
                    blocked_path_rng_draw: None,
                }
            )
            .is_none());
        assert_eq!(runtime.orbit_flags_b0, 0x1B);
    }

    #[test]
    fn steering_wrapper_keeps_native_mode1_constants_velocity_and_reentry_reset() {
        let mut runtime = RobotsWatchbotMode1Runtime::default();
        let input = RobotsWatchbotMode1SteeringInput {
            owner_position_xyzw: [0.0; 4],
            owner_rotation_xyzw: [0.0; 4],
            steering_target_xyzw: [0.0, 0.0, 10.0, 0.0],
            bound_target_position_xyzw: [100.0, 0.0, 100.0, 0.0],
            internal_state: ROBOTS_WATCHBOT_MODE1_STATE_DRIVE,
            player_yaw_radians: None,
            global_override_position_xyzw: None,
        };
        let first = advance_watchbot_mode1_steering(&mut runtime, input);
        assert!((first.velocity_xyzw_30[2] - 0.285).abs() < 1.0e-6);
        let second = advance_watchbot_mode1_steering(&mut runtime, input);
        assert!(second.velocity_xyzw_30[2] > first.velocity_xyzw_30[2]);
        assert_eq!(runtime.velocity_xyzw_30, second.velocity_xyzw_30);

        runtime.enter_component_with_setup_draw(Some(0));
        assert_eq!(runtime.velocity_xyzw_30, [0.0; 4]);
    }

    #[test]
    fn completion_predicate_keeps_state9_bypass_anim_blocks_and_inclusive_threshold() {
        assert!(watchbot_mode1_completion_predicate(
            9,
            0,
            0x0900_0071,
            [0.0; 3],
            [100.0, 0.0, 100.0],
        ));
        assert!(!watchbot_mode1_completion_predicate(
            3,
            4,
            0x0900_0003,
            [0.0; 3],
            [0.0; 3],
        ));
        assert!(!watchbot_mode1_completion_predicate(
            3,
            2,
            0x0900_0072,
            [0.0; 3],
            [0.0; 3],
        ));
        assert!(watchbot_mode1_completion_predicate(
            3,
            2,
            0x0900_0003,
            [0.0; 3],
            [
                ROBOTS_WATCHBOT_MODE1_COMPLETION_XZ_DISTANCE_SQUARED.sqrt(),
                0.0,
                0.0
            ],
        ));
    }
}
