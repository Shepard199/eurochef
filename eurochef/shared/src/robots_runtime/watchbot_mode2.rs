use serde::Serialize;

use super::{
    locomotion::shortest_yaw_delta,
    watchbot::ROBOTS_WATCHBOT_TARGET_FOLLOW_COMPONENT_MODE,
    watchbot_component::{
        service_watchbot_component_recovery, watchbot_component_idle_timer_from_draw,
        watchbot_component_stop_predicate, RobotsWatchbotComponentRecoveryContact,
        RobotsWatchbotComponentStopInput, ROBOTS_WATCHBOT_COMPONENT_ACCELERATION_SCALE,
        ROBOTS_WATCHBOT_COMPONENT_EPSILON, ROBOTS_WATCHBOT_COMPONENT_MAX_HORIZONTAL_SPEED,
        ROBOTS_WATCHBOT_COMPONENT_MOVING_DAMPING, ROBOTS_WATCHBOT_COMPONENT_STEERING_ROLL_SCALE,
        ROBOTS_WATCHBOT_COMPONENT_YAW_INPUT_SCALE,
    },
};

pub const ROBOTS_WATCHBOT_MODE2_STATE_IDLE: u32 = 1;
pub const ROBOTS_WATCHBOT_MODE2_STATE_IDLE_VARIANT: u32 = 2;
pub const ROBOTS_WATCHBOT_MODE2_STATE_DRIVE: u32 = 3;
pub const ROBOTS_WATCHBOT_MODE2_STATE_HIT: u32 = 7;
pub const ROBOTS_WATCHBOT_MODE2_STATE_RECOVERY: u32 = 8;
pub const ROBOTS_WATCHBOT_MODE2_STATE_SEQUENCE: u32 = 11;
pub const ROBOTS_WATCHBOT_MODE2_STATE_SPECIAL: u32 = 12;

pub const ROBOTS_WATCHBOT_MODE2_MOVE_ANIM_MODE: u32 = 0x0900_0003;
pub const ROBOTS_WATCHBOT_MODE2_VERTICAL_POSITIVE_ANIM_MODE: u32 = 0x0900_00DF;
pub const ROBOTS_WATCHBOT_MODE2_VERTICAL_NEGATIVE_ANIM_MODE: u32 = 0x0900_00E1;
pub const ROBOTS_WATCHBOT_MODE2_SPECIAL_ANIM_MODE: u32 = 0x0900_00A9;
pub const ROBOTS_WATCHBOT_MODE2_SEQUENCE_START_ANIM_MODE: u32 = 0x0900_0071;
pub const ROBOTS_WATCHBOT_MODE2_SEQUENCE_LATE_ANIM_MODE: u32 = 0x0900_0072;
pub const ROBOTS_WATCHBOT_MODE2_IDLE_ANIM_MODES: [u32; 8] = [
    0x0900_0007,
    0x0900_0008,
    0x0900_0009,
    0x0900_000A,
    0x0900_0034,
    0x0900_006F,
    0x0900_0070,
    0x0900_007A,
];
pub const ROBOTS_WATCHBOT_MODE2_ENTRY_PROTECTED_ANIM_MODES: [u32; 2] = [0x0900_0071, 0x0900_0072];
pub const ROBOTS_WATCHBOT_MODE2_STATE8_RESOURCE_ACTIVATIONS: [u32; 2] = [0x1900_0003, 0x1900_0004];
pub const ROBOTS_WATCHBOT_MODE2_IDLE_TIMER_MODULUS: u32 = 0xF0;
pub const ROBOTS_WATCHBOT_MODE2_IDLE_TIMER_BASE: u16 = 0x3C;

pub const ROBOTS_WATCHBOT_MODE2_HANDOFF_UPDATES: u32 = 20;
pub const ROBOTS_WATCHBOT_MODE2_HANDOFF_SCALAR_74: f32 = f32::from_bits(0x3D4C_CCCD); // 0.05
pub const ROBOTS_WATCHBOT_MODE2_SPIN_TARGET_LOW: f32 = 2.0;
pub const ROBOTS_WATCHBOT_MODE2_SPIN_TARGET_DEFAULT: f32 = 4.0;
pub const ROBOTS_WATCHBOT_MODE2_SPIN_TARGET_FAST: f32 = 10.0;
pub const ROBOTS_WATCHBOT_MODE2_SPIN_RAMP_SCALE: f32 = f32::from_bits(0x3DCC_CCCD); // 0.1
pub const ROBOTS_WATCHBOT_MODE2_IDLE_STATE1_THRESHOLD: i16 = 200;
pub const ROBOTS_WATCHBOT_MODE2_SEQUENCE_LATE_ANIM_AFTER: u32 = 300;
pub const ROBOTS_WATCHBOT_MODE2_SEQUENCE_HIT_AT: u32 = 0x259; // 601
pub const ROBOTS_WATCHBOT_MODE2_COMPONENT_MODE_EXIT_REQUEST: u32 =
    ROBOTS_WATCHBOT_TARGET_FOLLOW_COMPONENT_MODE;
pub const ROBOTS_WATCHBOT_MODE2_ENTRY_FORWARD_OFFSET: f32 = 1.5;
pub const ROBOTS_WATCHBOT_MODE2_ENTRY_VERTICAL_OFFSET: f32 = 2.25;

pub type RobotsWatchbotMode2StopInput = RobotsWatchbotComponentStopInput;
pub type RobotsWatchbotMode2RecoveryContact = RobotsWatchbotComponentRecoveryContact;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotMode2EntryPlacementPlan {
    /// Request-time snapshot rotation Y copied to owner +0xE4.
    pub owner_yaw_radians: f32,
    /// `0x00495490` offsets snapshot position by 1.5 along yaw and +2.25 Y.
    pub owner_position_xyzw: [f32; 4],
    pub mark_owner_transform_dirty: bool,
    /// `0x004936D0(...,1)` mirrors the placed owner position into Handler +0x4DC..+0x4E8.
    pub sync_handler_target_position: bool,
}

/// Mode2 vslot +0x10 (`0x00495490`) used on entry from every component except mode3.
pub fn watchbot_mode2_entry_placement_plan(
    snapshot_position_xyzw: [f32; 4],
    snapshot_rotation_xyzw: [f32; 4],
) -> RobotsWatchbotMode2EntryPlacementPlan {
    let yaw = snapshot_rotation_xyzw[1];
    RobotsWatchbotMode2EntryPlacementPlan {
        owner_yaw_radians: yaw,
        owner_position_xyzw: [
            snapshot_position_xyzw[0]
                + (yaw as f64).sin() as f32 * ROBOTS_WATCHBOT_MODE2_ENTRY_FORWARD_OFFSET,
            snapshot_position_xyzw[1] + ROBOTS_WATCHBOT_MODE2_ENTRY_VERTICAL_OFFSET,
            snapshot_position_xyzw[2]
                + (yaw as f64).cos() as f32 * ROBOTS_WATCHBOT_MODE2_ENTRY_FORWARD_OFFSET,
            snapshot_position_xyzw[3],
        ],
        mark_owner_transform_dirty: true,
        sync_handler_target_position: true,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotMode2InputSample {
    /// Native axis pair read through input vslot +0x38 IDs 0x12..0x15.
    pub axis_xz: [f32; 2],
    /// Native action +0x24(0x0C): toggles internal state3/state8.
    pub action_0c: bool,
    /// Native action +0x24(0x0D): drives vertical +0x90 toward -1.
    pub action_0d: bool,
    /// Native action +0x24(0x0E): drives vertical +0x90 toward +1.
    pub action_0e: bool,
    /// Native action +0x2C(0x0F): requests WatchBot component mode1.
    pub action_0f: bool,
    /// Native DAT_00620034.
    pub runtime_rate_scale: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotMode2InputStep {
    pub requested_internal_state: Option<u32>,
    pub requested_component_mode: Option<u32>,
    pub requested_anim_mode: Option<u32>,
    pub control_magnitude: f32,
    pub control_yaw_radians: f32,
    pub vertical_input: f32,
    /// Component +0x78. Native vslot +0x6C consumes it for the attached WatchBot
    /// child orientation; locomotion itself does not.
    pub attachment_spin_scalar: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotMode2StateEntryPlan {
    pub state: u32,
    pub requested_anim_mode: Option<u32>,
    /// Native component +0x8E after state-entry timer seeding.
    pub idle_timer_8e: Option<u16>,
    /// State8 `0x00495910` activates these two native resource UIDs.
    pub resource_activation_uids: [Option<u32>; 2],
    /// State12 entry calls vslot +0x78, resetting component motion/input and
    /// synchronizing Handler target lanes to the live owner transform.
    pub reset_control_to_owner_transform: bool,
    pub rng_draws_consumed: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotMode2FixedInput {
    pub owner_rotation_euler4: [f32; 4],
    pub controller_yaw_radians: f32,
    pub runtime_rate_scale: f32,
    pub stop: RobotsWatchbotMode2StopInput,
    /// Only consumed by native state8 before the shared locomotion callback.
    pub recovery_contact: Option<RobotsWatchbotMode2RecoveryContact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsWatchbotMode2MotionStep {
    pub requested_internal_state: Option<u32>,
    /// Tick-time AnimMode request. State11 starts with 0x71 on entry, then requests
    /// 0x72 every update once native +0x84 is greater than 300.
    pub requested_anim_mode: Option<u32>,
    pub owner_rotation_euler4: [f32; 4],
    /// Component-local +0x30..+0x3C after acceleration/damping.
    pub local_velocity_xyzw: [f32; 4],
    /// World-space motion vector after controller-yaw rotation and native vslot
    /// +0x20 mode3->mode2 handoff interpolation. Host physics adds this to the
    /// real body accumulator.
    pub world_motion_xyzw: [f32; 4],
    pub handoff_updates_remaining: u32,
}

/// Engine-neutral XItemHandler_WatchBot component mode2 runtime.
///
/// Native owner: ctor `0x00495330`, vtable `0x005ECEB8`, input `0x00496460`,
/// locomotion `0x00495ED0`, motion accumulator `0x00493800`.
#[derive(Debug, Clone, PartialEq)]
pub struct RobotsWatchbotMode2Runtime {
    pub internal_state: u32,
    pub velocity_xyzw_30: [f32; 4],
    pub control_magnitude_60: f32,
    pub control_yaw_64: f32,
    pub state8_boost_70: f32,
    pub handoff_scalar_74: f32,
    pub attachment_spin_78: f32,
    /// State11 sequence counter at native +0x84. This is not the state1/2 i16
    /// counter at +0x8C and must remain a separate lane.
    pub sequence_counter_84: u32,
    pub state_counter_8c: i16,
    /// Base component/state-entry random timer at native +0x8E. None means the
    /// host has no proven global RNG anchor for this editor/session boundary.
    pub idle_timer_8e: Option<u16>,
    pub vertical_input_90: f32,
    pub recovery_latched_94: bool,
    pub recovery_updates_95: u8,
    pub handoff_velocity_xyzw_50: [f32; 4],
    pub handoff_updates_88: u32,
}

impl Default for RobotsWatchbotMode2Runtime {
    fn default() -> Self {
        Self {
            internal_state: ROBOTS_WATCHBOT_MODE2_STATE_DRIVE,
            velocity_xyzw_30: [0.0; 4],
            control_magnitude_60: 0.0,
            control_yaw_64: 0.0,
            state8_boost_70: 1.0,
            handoff_scalar_74: 0.0,
            attachment_spin_78: 0.0,
            sequence_counter_84: 0,
            state_counter_8c: 0,
            idle_timer_8e: None,
            vertical_input_90: 0.0,
            recovery_latched_94: false,
            recovery_updates_95: 0,
            handoff_velocity_xyzw_50: [0.0; 4],
            handoff_updates_88: 0,
        }
    }
}

impl RobotsWatchbotMode2Runtime {
    /// Common component setup `0x004935A0` followed by mode2 setup `0x00495390`.
    /// Base setup always clears mode2 local velocity. Only a previous component
    /// mode3 seeds the separate 20-update motion blend lane.
    pub fn enter_component(
        &mut self,
        previous_component_mode: u32,
        previous_mode3_velocity_xyzw: [f32; 4],
    ) {
        self.velocity_xyzw_30 = [0.0; 4];
        self.control_magnitude_60 = 0.0;
        self.control_yaw_64 = 0.0;
        self.state8_boost_70 = 1.0;
        self.vertical_input_90 = 0.0;
        self.recovery_latched_94 = false;
        self.recovery_updates_95 = 0;
        self.sequence_counter_84 = 0;
        self.state_counter_8c = 0;
        self.idle_timer_8e = None;
        self.internal_state = ROBOTS_WATCHBOT_MODE2_STATE_DRIVE;
        self.handoff_velocity_xyzw_50 = [0.0; 4];
        self.handoff_updates_88 = 0;
        self.handoff_scalar_74 = 0.0;

        if previous_component_mode == 3 {
            self.handoff_velocity_xyzw_50 = previous_mode3_velocity_xyzw;
            self.handoff_updates_88 = ROBOTS_WATCHBOT_MODE2_HANDOFF_UPDATES;
            self.handoff_scalar_74 = ROBOTS_WATCHBOT_MODE2_HANDOFF_SCALAR_74;
        }
    }

    /// Same component setup with the common `0x004935A0` RNG timer side effect.
    pub fn enter_component_with_setup_draw(
        &mut self,
        previous_component_mode: u32,
        previous_mode3_velocity_xyzw: [f32; 4],
        setup_rng_draw: Option<u32>,
    ) {
        self.enter_component(previous_component_mode, previous_mode3_velocity_xyzw);
        self.idle_timer_8e = setup_rng_draw.map(mode2_idle_timer_from_draw);
    }

    /// Number of process-global `FUN_00509C48` draws consumed by the native state
    /// entry handler. None means this state-entry handler is not recovered here.
    pub fn state_entry_rng_draw_count(state: u32, current_anim_mode_uid: u32) -> Option<usize> {
        match state {
            ROBOTS_WATCHBOT_MODE2_STATE_IDLE
            | ROBOTS_WATCHBOT_MODE2_STATE_HIT
            | ROBOTS_WATCHBOT_MODE2_STATE_RECOVERY
            | ROBOTS_WATCHBOT_MODE2_STATE_SEQUENCE
            | ROBOTS_WATCHBOT_MODE2_STATE_SPECIAL => Some(0),
            ROBOTS_WATCHBOT_MODE2_STATE_IDLE_VARIANT => Some(
                1 + usize::from(
                    !ROBOTS_WATCHBOT_MODE2_ENTRY_PROTECTED_ANIM_MODES
                        .contains(&current_anim_mode_uid),
                ),
            ),
            ROBOTS_WATCHBOT_MODE2_STATE_DRIVE => Some(1),
            _ => None,
        }
    }

    /// Exact recovered state-entry side effects for dispatcher states 1/2/3/7/8/12.
    /// The caller supplies raw global RNG draws so RNG ownership remains outside the
    /// component and UE/MapFrame can preserve the process-wide stream.
    pub fn enter_internal_state(
        &mut self,
        state: u32,
        current_anim_mode_uid: u32,
        rng_draws: &[u32],
    ) -> Option<RobotsWatchbotMode2StateEntryPlan> {
        let required = Self::state_entry_rng_draw_count(state, current_anim_mode_uid)?;
        if rng_draws.len() < required {
            return None;
        }

        let mut plan = RobotsWatchbotMode2StateEntryPlan {
            state,
            requested_anim_mode: None,
            idle_timer_8e: None,
            resource_activation_uids: [None; 2],
            reset_control_to_owner_transform: false,
            rng_draws_consumed: required as u8,
        };

        match state {
            ROBOTS_WATCHBOT_MODE2_STATE_IDLE => {
                if !ROBOTS_WATCHBOT_MODE2_ENTRY_PROTECTED_ANIM_MODES
                    .contains(&current_anim_mode_uid)
                {
                    plan.requested_anim_mode = Some(ROBOTS_WATCHBOT_MODE2_MOVE_ANIM_MODE);
                }
            }
            ROBOTS_WATCHBOT_MODE2_STATE_IDLE_VARIANT => {
                self.state_counter_8c = 0;
                let timer = mode2_idle_timer_from_draw(rng_draws[0]);
                self.idle_timer_8e = Some(timer);
                plan.idle_timer_8e = Some(timer);
                if !ROBOTS_WATCHBOT_MODE2_ENTRY_PROTECTED_ANIM_MODES
                    .contains(&current_anim_mode_uid)
                {
                    plan.requested_anim_mode =
                        Some(ROBOTS_WATCHBOT_MODE2_IDLE_ANIM_MODES[(rng_draws[1] & 7) as usize]);
                }
            }
            ROBOTS_WATCHBOT_MODE2_STATE_DRIVE => {
                self.state_counter_8c = 0;
                self.recovery_updates_95 = 0;
                let timer = mode2_idle_timer_from_draw(rng_draws[0]);
                self.idle_timer_8e = Some(timer);
                plan.idle_timer_8e = Some(timer);
                plan.requested_anim_mode = Some(ROBOTS_WATCHBOT_MODE2_MOVE_ANIM_MODE);
            }
            ROBOTS_WATCHBOT_MODE2_STATE_HIT => {
                plan.requested_anim_mode = Some(0x0900_00A8);
            }
            ROBOTS_WATCHBOT_MODE2_STATE_RECOVERY => {
                plan.resource_activation_uids =
                    ROBOTS_WATCHBOT_MODE2_STATE8_RESOURCE_ACTIVATIONS.map(Some);
            }
            ROBOTS_WATCHBOT_MODE2_STATE_SEQUENCE => {
                self.sequence_counter_84 = 0;
                plan.requested_anim_mode = Some(ROBOTS_WATCHBOT_MODE2_SEQUENCE_START_ANIM_MODE);
            }
            ROBOTS_WATCHBOT_MODE2_STATE_SPECIAL => {
                self.velocity_xyzw_30 = [0.0; 4];
                self.control_magnitude_60 = 0.0;
                self.vertical_input_90 = 0.0;
                plan.reset_control_to_owner_transform = true;
                plan.requested_anim_mode = Some(ROBOTS_WATCHBOT_MODE2_SPECIAL_ANIM_MODE);
            }
            _ => unreachable!("state was validated above"),
        }

        self.internal_state = state;
        Some(plan)
    }

    pub fn set_internal_state(&mut self, state: u32) {
        self.internal_state = state;
        if state == ROBOTS_WATCHBOT_MODE2_STATE_SEQUENCE {
            self.sequence_counter_84 = 0;
        }
        if state == ROBOTS_WATCHBOT_MODE2_STATE_DRIVE {
            self.state_counter_8c = 0;
            self.recovery_updates_95 = 0;
        }
        if state == ROBOTS_WATCHBOT_MODE2_STATE_SPECIAL {
            self.velocity_xyzw_30 = [0.0; 4];
            self.control_magnitude_60 = 0.0;
            self.vertical_input_90 = 0.0;
        }
    }

    /// Native input override `0x00496460`. State7/state12 and an active recovery
    /// latch suppress ordinary input exactly like the executable.
    pub fn apply_input(
        &mut self,
        input: RobotsWatchbotMode2InputSample,
    ) -> RobotsWatchbotMode2InputStep {
        if self.recovery_latched_94
            || self.internal_state == ROBOTS_WATCHBOT_MODE2_STATE_HIT
            || self.internal_state == ROBOTS_WATCHBOT_MODE2_STATE_SPECIAL
        {
            return self.input_step(None, None, None);
        }

        let axis_x = input.axis_xz[0];
        let axis_z = input.axis_xz[1];
        self.control_magnitude_60 = (axis_x * axis_x + axis_z * axis_z).sqrt();
        self.control_yaw_64 = axis_x.atan2(axis_z);

        let requested_internal_state = if input.action_0c {
            if self.internal_state != ROBOTS_WATCHBOT_MODE2_STATE_RECOVERY {
                self.internal_state = ROBOTS_WATCHBOT_MODE2_STATE_RECOVERY;
                Some(ROBOTS_WATCHBOT_MODE2_STATE_RECOVERY)
            } else {
                None
            }
        } else if self.internal_state == ROBOTS_WATCHBOT_MODE2_STATE_RECOVERY {
            self.internal_state = ROBOTS_WATCHBOT_MODE2_STATE_DRIVE;
            Some(ROBOTS_WATCHBOT_MODE2_STATE_DRIVE)
        } else {
            None
        };

        let old_vertical = self.vertical_input_90;
        let vertical_target = if input.action_0d {
            -1.0
        } else if input.action_0e {
            1.0
        } else {
            0.0
        };
        let requested_anim_mode = if !input.action_0d
            && !input.action_0e
            && old_vertical != 0.0
            && self.velocity_xyzw_30[0] * self.velocity_xyzw_30[0]
                + self.velocity_xyzw_30[2] * self.velocity_xyzw_30[2]
                < ROBOTS_WATCHBOT_COMPONENT_EPSILON
        {
            Some(if old_vertical < 0.0 {
                ROBOTS_WATCHBOT_MODE2_VERTICAL_NEGATIVE_ANIM_MODE
            } else {
                ROBOTS_WATCHBOT_MODE2_VERTICAL_POSITIVE_ANIM_MODE
            })
        } else {
            None
        };
        self.vertical_input_90 =
            native_ramp(old_vertical, vertical_target, input.runtime_rate_scale);

        let spin_target = if input.action_0c || input.action_0e {
            ROBOTS_WATCHBOT_MODE2_SPIN_TARGET_FAST
        } else if input.action_0d {
            ROBOTS_WATCHBOT_MODE2_SPIN_TARGET_LOW
        } else {
            ROBOTS_WATCHBOT_MODE2_SPIN_TARGET_DEFAULT
        };
        self.attachment_spin_78 = native_ramp(
            self.attachment_spin_78,
            spin_target,
            input.runtime_rate_scale * ROBOTS_WATCHBOT_MODE2_SPIN_RAMP_SCALE,
        );

        self.input_step(
            requested_internal_state,
            input
                .action_0f
                .then_some(ROBOTS_WATCHBOT_MODE2_COMPONENT_MODE_EXIT_REQUEST),
            requested_anim_mode,
        )
    }

    fn input_step(
        &self,
        requested_internal_state: Option<u32>,
        requested_component_mode: Option<u32>,
        requested_anim_mode: Option<u32>,
    ) -> RobotsWatchbotMode2InputStep {
        RobotsWatchbotMode2InputStep {
            requested_internal_state,
            requested_component_mode,
            requested_anim_mode,
            control_magnitude: self.control_magnitude_60,
            control_yaw_radians: self.control_yaw_64,
            vertical_input: self.vertical_input_90,
            attachment_spin_scalar: self.attachment_spin_78,
        }
    }

    fn stop_predicate(&self, input: RobotsWatchbotMode2StopInput) -> bool {
        watchbot_component_stop_predicate(self.velocity_xyzw_30, input)
    }

    fn service_recovery(&mut self, contact: Option<RobotsWatchbotMode2RecoveryContact>) {
        service_watchbot_component_recovery(
            &mut self.recovery_latched_94,
            &mut self.recovery_updates_95,
            &mut self.control_magnitude_60,
            &mut self.control_yaw_64,
            contact,
        );
    }

    /// State-machine update plus native locomotion vslot +0x7C. Animation requests
    /// on state entry stay outside this physics runtime; the returned state edge is
    /// enough for the host animation owner to apply them in native order.
    pub fn fixed_pre_physics(
        &mut self,
        input: RobotsWatchbotMode2FixedInput,
    ) -> Option<RobotsWatchbotMode2MotionStep> {
        let mut requested_internal_state = None;
        let mut requested_anim_mode = None;
        let should_move = match self.internal_state {
            ROBOTS_WATCHBOT_MODE2_STATE_IDLE => {
                if !self.stop_predicate(input.stop) {
                    self.internal_state = ROBOTS_WATCHBOT_MODE2_STATE_DRIVE;
                    requested_internal_state = Some(ROBOTS_WATCHBOT_MODE2_STATE_DRIVE);
                    false
                } else {
                    let old_counter = self.state_counter_8c;
                    self.state_counter_8c = self.state_counter_8c.wrapping_add(1);
                    if old_counter > ROBOTS_WATCHBOT_MODE2_IDLE_STATE1_THRESHOLD {
                        self.internal_state = ROBOTS_WATCHBOT_MODE2_STATE_IDLE_VARIANT;
                        requested_internal_state = Some(ROBOTS_WATCHBOT_MODE2_STATE_IDLE_VARIANT);
                    }
                    true
                }
            }
            ROBOTS_WATCHBOT_MODE2_STATE_IDLE_VARIANT => {
                if self.stop_predicate(input.stop)
                    && input.stop.current_anim_mode_uid != ROBOTS_WATCHBOT_MODE2_MOVE_ANIM_MODE
                {
                    true
                } else {
                    self.internal_state = ROBOTS_WATCHBOT_MODE2_STATE_DRIVE;
                    requested_internal_state = Some(ROBOTS_WATCHBOT_MODE2_STATE_DRIVE);
                    false
                }
            }
            ROBOTS_WATCHBOT_MODE2_STATE_DRIVE => {
                if self.stop_predicate(input.stop) {
                    self.internal_state = ROBOTS_WATCHBOT_MODE2_STATE_IDLE;
                    requested_internal_state = Some(ROBOTS_WATCHBOT_MODE2_STATE_IDLE);
                }
                self.service_recovery(input.recovery_contact);
                true
            }
            ROBOTS_WATCHBOT_MODE2_STATE_RECOVERY => {
                self.service_recovery(input.recovery_contact);
                true
            }
            ROBOTS_WATCHBOT_MODE2_STATE_SEQUENCE => {
                let old_counter = self.sequence_counter_84;
                if old_counter < ROBOTS_WATCHBOT_MODE2_SEQUENCE_HIT_AT {
                    if old_counter > ROBOTS_WATCHBOT_MODE2_SEQUENCE_LATE_ANIM_AFTER {
                        requested_anim_mode = Some(ROBOTS_WATCHBOT_MODE2_SEQUENCE_LATE_ANIM_MODE);
                    }
                } else if self.internal_state != ROBOTS_WATCHBOT_MODE2_STATE_HIT {
                    self.internal_state = ROBOTS_WATCHBOT_MODE2_STATE_HIT;
                    requested_internal_state = Some(ROBOTS_WATCHBOT_MODE2_STATE_HIT);
                }
                self.sequence_counter_84 = self.sequence_counter_84.wrapping_add(1);
                true
            }
            ROBOTS_WATCHBOT_MODE2_STATE_SPECIAL => true,
            _ => false,
        };

        if !should_move {
            return None;
        }

        let mut effective_magnitude = self.control_magnitude_60;
        if self.internal_state == ROBOTS_WATCHBOT_MODE2_STATE_RECOVERY && self.state8_boost_70 > 0.0
        {
            effective_magnitude += effective_magnitude;
        }

        let owner_yaw = input.owner_rotation_euler4[1];
        let controller_frame_yaw =
            owner_yaw + shortest_yaw_delta(owner_yaw, input.controller_yaw_radians);
        let local_velocity_yaw = self.velocity_xyzw_30[0].atan2(self.velocity_xyzw_30[2]);
        let yaw_error = shortest_yaw_delta(owner_yaw, local_velocity_yaw + controller_frame_yaw);
        let horizontal_speed = (self.velocity_xyzw_30[0] * self.velocity_xyzw_30[0]
            + self.velocity_xyzw_30[2] * self.velocity_xyzw_30[2])
            .sqrt()
            .clamp(0.0, ROBOTS_WATCHBOT_COMPONENT_MAX_HORIZONTAL_SPEED);

        let mut owner_rotation_euler4 = input.owner_rotation_euler4;
        owner_rotation_euler4[1] = owner_yaw
            + yaw_error
                * effective_magnitude
                * ROBOTS_WATCHBOT_COMPONENT_YAW_INPUT_SCALE
                * input.runtime_rate_scale;
        owner_rotation_euler4[2] =
            -(horizontal_speed * ROBOTS_WATCHBOT_COMPONENT_STEERING_ROLL_SCALE * yaw_error);

        let acceleration = effective_magnitude * ROBOTS_WATCHBOT_COMPONENT_ACCELERATION_SCALE;
        self.velocity_xyzw_30[0] += self.control_yaw_64.sin() * acceleration;
        self.velocity_xyzw_30[1] +=
            self.vertical_input_90 * ROBOTS_WATCHBOT_COMPONENT_ACCELERATION_SCALE;
        self.velocity_xyzw_30[2] += self.control_yaw_64.cos() * acceleration;
        for component in &mut self.velocity_xyzw_30 {
            *component *= ROBOTS_WATCHBOT_COMPONENT_MOVING_DAMPING;
        }

        let sin_frame = controller_frame_yaw.sin();
        let cos_frame = controller_frame_yaw.cos();
        let raw_world_motion = [
            cos_frame * self.velocity_xyzw_30[0] + sin_frame * self.velocity_xyzw_30[2],
            self.velocity_xyzw_30[1],
            cos_frame * self.velocity_xyzw_30[2] - sin_frame * self.velocity_xyzw_30[0],
            self.velocity_xyzw_30[3],
        ];
        let world_motion_xyzw =
            self.apply_handoff_blend(raw_world_motion, input.runtime_rate_scale);

        if self.internal_state == ROBOTS_WATCHBOT_MODE2_STATE_RECOVERY && self.recovery_latched_94 {
            self.internal_state = ROBOTS_WATCHBOT_MODE2_STATE_DRIVE;
            requested_internal_state = Some(ROBOTS_WATCHBOT_MODE2_STATE_DRIVE);
        }

        Some(RobotsWatchbotMode2MotionStep {
            requested_internal_state,
            requested_anim_mode,
            owner_rotation_euler4,
            local_velocity_xyzw: self.velocity_xyzw_30,
            world_motion_xyzw,
            handoff_updates_remaining: self.handoff_updates_88,
        })
    }

    fn apply_handoff_blend(
        &mut self,
        requested_world_motion: [f32; 4],
        runtime_rate_scale: f32,
    ) -> [f32; 4] {
        if self.handoff_updates_88 <= 1 {
            return requested_world_motion;
        }

        self.handoff_updates_88 -= 1;
        let factor = runtime_rate_scale / self.handoff_updates_88 as f32;
        if factor >= 1.0 {
            return requested_world_motion;
        }

        let mut blended = [0.0; 4];
        for index in 0..4 {
            blended[index] = self.handoff_velocity_xyzw_50[index]
                + (requested_world_motion[index] - self.handoff_velocity_xyzw_50[index]) * factor;
        }
        blended
    }
}

fn mode2_idle_timer_from_draw(draw: u32) -> u16 {
    watchbot_component_idle_timer_from_draw(draw)
}

fn native_ramp(current: f32, target: f32, factor: f32) -> f32 {
    if factor < 1.0 {
        current + (target - current) * factor
    } else {
        target
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stop_input(current_anim_mode_uid: u32) -> RobotsWatchbotMode2StopInput {
        RobotsWatchbotMode2StopInput {
            owner_position_xyz: [0.0, 0.0, 0.0],
            handler_target_xz: [100.0, 100.0],
            current_anim_mode_uid,
        }
    }

    fn fixed_input(current_anim_mode_uid: u32) -> RobotsWatchbotMode2FixedInput {
        RobotsWatchbotMode2FixedInput {
            owner_rotation_euler4: [0.0; 4],
            controller_yaw_radians: 0.0,
            runtime_rate_scale: 1.0,
            stop: stop_input(current_anim_mode_uid),
            recovery_contact: None,
        }
    }

    #[test]
    fn non_mode3_entry_placement_uses_request_snapshot_yaw_forward_and_vertical_offsets() {
        let yaw = core::f32::consts::FRAC_PI_2;
        let plan =
            watchbot_mode2_entry_placement_plan([10.0, 20.0, 30.0, 40.0], [99.0, yaw, 77.0, 66.0]);
        assert_eq!(plan.owner_yaw_radians, yaw);
        assert!((plan.owner_position_xyzw[0] - 11.5).abs() < 1.0e-6);
        assert!((plan.owner_position_xyzw[1] - 22.25).abs() < 1.0e-6);
        assert!((plan.owner_position_xyzw[2] - 30.0).abs() < 1.0e-6);
        assert_eq!(plan.owner_position_xyzw[3], 40.0);
        assert!(plan.mark_owner_transform_dirty);
        assert!(plan.sync_handler_target_position);
    }

    #[test]
    fn mode3_to_mode2_handoff_clears_local_velocity_but_seeds_twenty_update_blend() {
        let mut runtime = RobotsWatchbotMode2Runtime::default();
        runtime.enter_component(3, [2.0, 3.0, 4.0, 5.0]);
        assert_eq!(runtime.velocity_xyzw_30, [0.0; 4]);
        assert_eq!(runtime.handoff_velocity_xyzw_50, [2.0, 3.0, 4.0, 5.0]);
        assert_eq!(
            runtime.handoff_updates_88,
            ROBOTS_WATCHBOT_MODE2_HANDOFF_UPDATES
        );
        assert_eq!(
            runtime.handoff_scalar_74,
            ROBOTS_WATCHBOT_MODE2_HANDOFF_SCALAR_74
        );
    }

    #[test]
    fn input_axes_vertical_ramp_state8_toggle_and_mode1_request_match_native_order() {
        let mut runtime = RobotsWatchbotMode2Runtime::default();
        let first = runtime.apply_input(RobotsWatchbotMode2InputSample {
            axis_xz: [1.0, 0.0],
            action_0c: true,
            action_0d: false,
            action_0e: true,
            action_0f: true,
            runtime_rate_scale: 0.5,
        });
        assert_eq!(
            first.requested_internal_state,
            Some(ROBOTS_WATCHBOT_MODE2_STATE_RECOVERY)
        );
        assert_eq!(first.requested_component_mode, Some(1));
        assert!((first.control_magnitude - 1.0).abs() < 1.0e-6);
        assert!((first.control_yaw_radians - core::f32::consts::FRAC_PI_2).abs() < 1.0e-6);
        assert_eq!(first.vertical_input, 0.5);
        assert_eq!(first.attachment_spin_scalar, 0.5);

        let second = runtime.apply_input(RobotsWatchbotMode2InputSample {
            axis_xz: [0.0, 1.0],
            action_0c: false,
            action_0d: false,
            action_0e: false,
            action_0f: false,
            runtime_rate_scale: 1.0,
        });
        assert_eq!(
            second.requested_internal_state,
            Some(ROBOTS_WATCHBOT_MODE2_STATE_DRIVE)
        );
        assert_eq!(second.vertical_input, 0.0);
        assert!((second.attachment_spin_scalar - 0.85).abs() < 1.0e-6);
    }

    #[test]
    fn free_flight_accelerates_in_control_space_then_rotates_motion_by_controller_yaw() {
        let mut runtime = RobotsWatchbotMode2Runtime::default();
        runtime.enter_component(0, [0.0; 4]);
        runtime.apply_input(RobotsWatchbotMode2InputSample {
            axis_xz: [0.0, 1.0],
            action_0c: false,
            action_0d: false,
            action_0e: false,
            action_0f: false,
            runtime_rate_scale: 1.0,
        });
        let mut input = fixed_input(ROBOTS_WATCHBOT_MODE2_MOVE_ANIM_MODE);
        input.controller_yaw_radians = core::f32::consts::FRAC_PI_2;
        let step = runtime.fixed_pre_physics(input).unwrap();
        assert!((step.local_velocity_xyzw[2] - 0.2375).abs() < 1.0e-6);
        assert!((step.world_motion_xyzw[0] - 0.2375).abs() < 1.0e-6);
        assert!(step.world_motion_xyzw[2].abs() < 1.0e-6);
    }

    #[test]
    fn state3_stop_requests_state1_but_still_runs_same_tick_motion() {
        let mut runtime = RobotsWatchbotMode2Runtime::default();
        runtime.enter_component(0, [0.0; 4]);
        runtime.control_magnitude_60 = 1.0;
        runtime.control_yaw_64 = 0.0;
        let mut input = fixed_input(ROBOTS_WATCHBOT_MODE2_MOVE_ANIM_MODE);
        input.stop.owner_position_xyz = [1.0, 0.0, 2.0];
        input.stop.handler_target_xz = [1.0, 2.0];
        let step = runtime.fixed_pre_physics(input).unwrap();
        assert_eq!(
            step.requested_internal_state,
            Some(ROBOTS_WATCHBOT_MODE2_STATE_IDLE)
        );
        assert_eq!(runtime.internal_state, ROBOTS_WATCHBOT_MODE2_STATE_IDLE);
        assert!(step.local_velocity_xyzw[2] > 0.0);
    }

    #[test]
    fn recovery_contact_overrides_control_and_returns_state8_to_state3_after_motion() {
        let mut runtime = RobotsWatchbotMode2Runtime::default();
        runtime.enter_component(0, [0.0; 4]);
        runtime.set_internal_state(ROBOTS_WATCHBOT_MODE2_STATE_RECOVERY);
        runtime.control_magnitude_60 = 0.1;
        runtime.control_yaw_64 = 0.0;
        let mut input = fixed_input(ROBOTS_WATCHBOT_MODE2_MOVE_ANIM_MODE);
        input.recovery_contact = Some(RobotsWatchbotMode2RecoveryContact {
            special_contact: true,
            recovery_yaw_radians: 0.0,
        });
        let step = runtime.fixed_pre_physics(input).unwrap();
        assert!(runtime.recovery_latched_94);
        assert_eq!(
            step.requested_internal_state,
            Some(ROBOTS_WATCHBOT_MODE2_STATE_DRIVE)
        );
        assert_eq!(runtime.internal_state, ROBOTS_WATCHBOT_MODE2_STATE_DRIVE);
        assert!((step.local_velocity_xyzw[2] - 0.2375).abs() < 1.0e-6);
    }

    #[test]
    fn handoff_motion_uses_decremented_counter_and_fixed_seed_like_native_vslot20() {
        let mut runtime = RobotsWatchbotMode2Runtime::default();
        runtime.enter_component(3, [1.0, 2.0, 3.0, 4.0]);
        let blended = runtime.apply_handoff_blend([0.0; 4], 1.0);
        assert_eq!(runtime.handoff_updates_88, 19);
        let factor = 1.0 / 19.0;
        assert!((blended[0] - (1.0 - factor)).abs() < 1.0e-6);
        assert!((blended[2] - 3.0 * (1.0 - factor)).abs() < 1.0e-6);
    }

    #[test]
    fn component_setup_rng_seeds_native_60_to_299_timer_without_inventing_unknown_seed() {
        let mut runtime = RobotsWatchbotMode2Runtime::default();
        runtime.enter_component_with_setup_draw(0, [0.0; 4], Some(0x1234_5678));
        assert_eq!(
            runtime.idle_timer_8e,
            Some(mode2_idle_timer_from_draw(0x1234_5678))
        );
        runtime.enter_component_with_setup_draw(0, [0.0; 4], None);
        assert_eq!(runtime.idle_timer_8e, None);
    }

    #[test]
    fn state2_entry_consumes_second_draw_only_when_idle_animation_is_not_protected() {
        assert_eq!(
            RobotsWatchbotMode2Runtime::state_entry_rng_draw_count(
                ROBOTS_WATCHBOT_MODE2_STATE_IDLE_VARIANT,
                0x0900_0071,
            ),
            Some(1)
        );
        assert_eq!(
            RobotsWatchbotMode2Runtime::state_entry_rng_draw_count(
                ROBOTS_WATCHBOT_MODE2_STATE_IDLE_VARIANT,
                ROBOTS_WATCHBOT_MODE2_MOVE_ANIM_MODE,
            ),
            Some(2)
        );

        let mut runtime = RobotsWatchbotMode2Runtime::default();
        let protected = runtime
            .enter_internal_state(ROBOTS_WATCHBOT_MODE2_STATE_IDLE_VARIANT, 0x0900_0072, &[17])
            .unwrap();
        assert_eq!(protected.rng_draws_consumed, 1);
        assert_eq!(protected.requested_anim_mode, None);
        assert_eq!(protected.idle_timer_8e, Some(77));

        let selected = runtime
            .enter_internal_state(
                ROBOTS_WATCHBOT_MODE2_STATE_IDLE_VARIANT,
                ROBOTS_WATCHBOT_MODE2_MOVE_ANIM_MODE,
                &[17, 5],
            )
            .unwrap();
        assert_eq!(selected.rng_draws_consumed, 2);
        assert_eq!(selected.requested_anim_mode, Some(0x0900_006F));
        assert_eq!(selected.idle_timer_8e, Some(77));
    }

    #[test]
    fn state3_state8_and_state12_entry_plans_preserve_exact_rng_and_effect_boundaries() {
        let mut runtime = RobotsWatchbotMode2Runtime::default();
        let drive = runtime
            .enter_internal_state(ROBOTS_WATCHBOT_MODE2_STATE_DRIVE, 0x0900_0071, &[239])
            .unwrap();
        assert_eq!(drive.rng_draws_consumed, 1);
        assert_eq!(drive.idle_timer_8e, Some(299));
        assert_eq!(
            drive.requested_anim_mode,
            Some(ROBOTS_WATCHBOT_MODE2_MOVE_ANIM_MODE)
        );

        let recovery = runtime
            .enter_internal_state(
                ROBOTS_WATCHBOT_MODE2_STATE_RECOVERY,
                ROBOTS_WATCHBOT_MODE2_MOVE_ANIM_MODE,
                &[],
            )
            .unwrap();
        assert_eq!(recovery.rng_draws_consumed, 0);
        assert_eq!(
            recovery.resource_activation_uids,
            ROBOTS_WATCHBOT_MODE2_STATE8_RESOURCE_ACTIVATIONS.map(Some)
        );

        runtime.velocity_xyzw_30 = [1.0, 2.0, 3.0, 4.0];
        runtime.control_magnitude_60 = 1.0;
        runtime.vertical_input_90 = -1.0;
        let special = runtime
            .enter_internal_state(
                ROBOTS_WATCHBOT_MODE2_STATE_SPECIAL,
                ROBOTS_WATCHBOT_MODE2_MOVE_ANIM_MODE,
                &[],
            )
            .unwrap();
        assert_eq!(special.rng_draws_consumed, 0);
        assert_eq!(
            special.requested_anim_mode,
            Some(ROBOTS_WATCHBOT_MODE2_SPECIAL_ANIM_MODE)
        );
        assert!(special.reset_control_to_owner_transform);
        assert_eq!(runtime.velocity_xyzw_30, [0.0; 4]);
        assert_eq!(runtime.control_magnitude_60, 0.0);
        assert_eq!(runtime.vertical_input_90, 0.0);
    }

    #[test]
    fn state11_sequence_preserves_native_300_301_600_601_boundaries_and_same_tick_motion() {
        let mut runtime = RobotsWatchbotMode2Runtime::default();
        let entry = runtime
            .enter_internal_state(
                ROBOTS_WATCHBOT_MODE2_STATE_SEQUENCE,
                ROBOTS_WATCHBOT_MODE2_MOVE_ANIM_MODE,
                &[],
            )
            .unwrap();
        assert_eq!(entry.rng_draws_consumed, 0);
        assert_eq!(
            entry.requested_anim_mode,
            Some(ROBOTS_WATCHBOT_MODE2_SEQUENCE_START_ANIM_MODE)
        );
        assert_eq!(runtime.sequence_counter_84, 0);

        runtime.sequence_counter_84 = 300;
        let at_300 = runtime
            .fixed_pre_physics(fixed_input(ROBOTS_WATCHBOT_MODE2_SEQUENCE_START_ANIM_MODE))
            .unwrap();
        assert_eq!(at_300.requested_anim_mode, None);
        assert_eq!(at_300.requested_internal_state, None);
        assert_eq!(runtime.sequence_counter_84, 301);

        let at_301 = runtime
            .fixed_pre_physics(fixed_input(ROBOTS_WATCHBOT_MODE2_SEQUENCE_START_ANIM_MODE))
            .unwrap();
        assert_eq!(
            at_301.requested_anim_mode,
            Some(ROBOTS_WATCHBOT_MODE2_SEQUENCE_LATE_ANIM_MODE)
        );
        assert_eq!(at_301.requested_internal_state, None);
        assert_eq!(runtime.sequence_counter_84, 302);

        runtime.sequence_counter_84 = 600;
        let at_600 = runtime
            .fixed_pre_physics(fixed_input(ROBOTS_WATCHBOT_MODE2_SEQUENCE_LATE_ANIM_MODE))
            .unwrap();
        assert_eq!(
            at_600.requested_anim_mode,
            Some(ROBOTS_WATCHBOT_MODE2_SEQUENCE_LATE_ANIM_MODE)
        );
        assert_eq!(at_600.requested_internal_state, None);
        assert_eq!(runtime.sequence_counter_84, 601);

        let at_601 = runtime
            .fixed_pre_physics(fixed_input(ROBOTS_WATCHBOT_MODE2_SEQUENCE_LATE_ANIM_MODE))
            .unwrap();
        assert_eq!(at_601.requested_anim_mode, None);
        assert_eq!(
            at_601.requested_internal_state,
            Some(ROBOTS_WATCHBOT_MODE2_STATE_HIT)
        );
        assert_eq!(runtime.internal_state, ROBOTS_WATCHBOT_MODE2_STATE_HIT);
        assert_eq!(runtime.sequence_counter_84, 602);
        // Native executes vslot +0x7C before the counter-601 transition to state7.
        assert!(at_601
            .local_velocity_xyzw
            .iter()
            .all(|value| value.is_finite()));
    }
}
