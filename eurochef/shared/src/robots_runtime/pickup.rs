use serde::Serialize;

use super::player_items::{ROBOTS_PLAYER_ITEM_GATE_FIRST, ROBOTS_PLAYER_ITEM_GATE_LAST};

pub const ROBOTS_PICKUP_SERIALIZED_TYPES: [u32; 15] = [
    0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x29, 0x3E, 0x3F, 0x40, 0x41, 0x43, 0x47, 0x52, 0x53,
];

pub const ROBOTS_PICKUP_TRICKCHIP_SERIALIZED_TYPE: u32 = 0x1C;
pub const ROBOTS_PICKUP_DEFAULT_REGISTRATION_FLAGS: u32 = 0x1010;
pub const ROBOTS_PICKUP_SCRAP_REGISTRATION_FLAGS: u32 = 0x1410;
pub const ROBOTS_PICKUP_INITIAL_LIFETIME_SECONDS: f32 = 5.0;
pub const ROBOTS_PICKUP_SPIN_STEP_RADIANS: f32 = -0.034_906_585;
pub const ROBOTS_PICKUP_BROAD_RANGE_SQUARED: f32 = 6.25;
pub const ROBOTS_PICKUP_MODE2_COLLECT_RANGE_SQUARED: f32 = 1.959_999_9;
pub const ROBOTS_PICKUP_DEFAULT_COLLECT_RANGE_SQUARED: f32 = 1.0;
pub const ROBOTS_PICKUP_MODE34_COLLECT_RANGE_SQUARED: f32 = 9.0;
pub const ROBOTS_PICKUP_FAR_CADENCE_RANGE_SQUARED: f32 = 100.0;
pub const ROBOTS_PICKUP_COLLECTED_FLAG: u8 = 0x02;
pub const ROBOTS_PICKUP_PERIODIC_FLAG: u8 = 0x20;
pub const ROBOTS_PICKUP_KILL_ON_EXPIRY_FLAG: u8 = 0x10;
/// Player vslot +0x174 (`0x004B0D50`) returned by the common attraction wrapper
/// `0x004AFF20` when callers request a smaller radius.
pub const ROBOTS_PICKUP_ATTRACT_DEFAULT_RANGE: f32 = 2.0;
/// Player vslot +0x178 (`0x004B0D60`) target offset used by
/// `XItemPhysics_PickupAttract`.
pub const ROBOTS_PICKUP_ATTRACT_TARGET_OFFSET_XYZ: [f32; 3] = [0.0, 0.5, 0.0];
/// `0x004DBF90` commits only while target displacement squared is strictly below 0.4.
pub const ROBOTS_PICKUP_ATTRACT_COLLECT_RANGE_SQUARED: f32 = 0.4;
const ROBOTS_PICKUP_ATTRACT_FIXED_STEP_SECONDS: f32 = 1.0 / 60.0;
const ROBOTS_PICKUP_ATTRACT_NORMAL_FORCE_LIMIT: f32 = 2.0;
const ROBOTS_PICKUP_ATTRACT_MODE234_FORCE_LIMIT: f32 = 81.0;
const ROBOTS_PICKUP_ATTRACT_NORMAL_FORCE_THRESHOLD_SQUARED: f32 = 4.0;
const ROBOTS_PICKUP_ATTRACT_MODE234_FORCE_THRESHOLD_SQUARED: f32 = 81.0;
const ROBOTS_PICKUP_ATTRACT_NORMALIZE_EPSILON: f32 = 0.000_01;

/// Engine-neutral state for `XItemPhysics_PickupAttract` (`0x004DA9D0`).
/// Native replaces Projectile Physics with this class; both integrators never run together.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsPickupAttractPhysicsState {
    pub position_xyz: [f32; 3],
    /// Native Physics+0x230..+0x238. These are per-tick displacement lanes,
    /// not world-units-per-second projectile velocity.
    pub local_velocity_xyz: [f32; 3],
}

impl RobotsPickupAttractPhysicsState {
    pub const fn new(position_xyz: [f32; 3]) -> Self {
        Self {
            position_xyz,
            local_velocity_xyz: [0.0; 3],
        }
    }

    /// Spatial lanes of `XItemPhysics_PickupAttract::Update 0x004DA9D0`.
    /// This producer path has a zero W delta: generic XItem creation clears +0xDC,
    /// fragment generation preserves it, and the Player target offset W is zero.
    pub fn advance_fixed(
        &mut self,
        player_position_xyz: [f32; 3],
        game_control_mode: u8,
        runtime_rate_scale: f32,
    ) -> bool {
        if !self.position_xyz.iter().all(|value| value.is_finite())
            || !player_position_xyz.iter().all(|value| value.is_finite())
            || !runtime_rate_scale.is_finite()
        {
            return false;
        }
        let rate = runtime_rate_scale.max(0.0);
        let target = [
            player_position_xyz[0] + ROBOTS_PICKUP_ATTRACT_TARGET_OFFSET_XYZ[0],
            player_position_xyz[1] + ROBOTS_PICKUP_ATTRACT_TARGET_OFFSET_XYZ[1],
            player_position_xyz[2] + ROBOTS_PICKUP_ATTRACT_TARGET_OFFSET_XYZ[2],
        ];
        let mut delta = [
            target[0] - self.position_xyz[0],
            target[1] - self.position_xyz[1],
            target[2] - self.position_xyz[2],
        ];
        let distance_squared = delta.iter().map(|value| value * value).sum::<f32>();
        let mode234 = matches!(game_control_mode, 2 | 3 | 4);
        let threshold_squared = if mode234 {
            ROBOTS_PICKUP_ATTRACT_MODE234_FORCE_THRESHOLD_SQUARED
        } else {
            ROBOTS_PICKUP_ATTRACT_NORMAL_FORCE_THRESHOLD_SQUARED
        };
        if distance_squared > threshold_squared {
            let length = distance_squared.max(0.0).sqrt();
            if length > ROBOTS_PICKUP_ATTRACT_NORMALIZE_EPSILON {
                let scale = (if mode234 {
                    ROBOTS_PICKUP_ATTRACT_MODE234_FORCE_LIMIT
                } else {
                    ROBOTS_PICKUP_ATTRACT_NORMAL_FORCE_LIMIT
                }) / length;
                delta = delta.map(|value| value * scale);
            }
        }

        let acceleration_scale = rate * ROBOTS_PICKUP_ATTRACT_FIXED_STEP_SECONDS;
        for axis in 0..3 {
            self.local_velocity_xyz[axis] += delta[axis] * acceleration_scale;
        }

        // Prevent one native tick from overshooting the remaining XYZ target distance.
        let velocity_squared = self
            .local_velocity_xyz
            .iter()
            .map(|value| value * value)
            .sum::<f32>();
        if distance_squared < velocity_squared && velocity_squared > 0.0 {
            let velocity_length = velocity_squared.sqrt();
            if velocity_length > ROBOTS_PICKUP_ATTRACT_NORMALIZE_EPSILON {
                let distance = distance_squared.max(0.0).sqrt();
                let scale = distance / velocity_length;
                self.local_velocity_xyz = self.local_velocity_xyz.map(|value| value * scale);
            }
        }

        for axis in 0..3 {
            self.position_xyz[axis] += rate * self.local_velocity_xyz[axis];
        }

        let damping = match game_control_mode {
            3 | 4 => 0.03,
            6 | 7 => 0.0,
            _ => 0.07,
        };
        let damping_scale = 1.0 - rate * damping;
        self.local_velocity_xyz = self.local_velocity_xyz.map(|value| value * damping_scale);
        true
    }

    pub fn target_distance_squared(self, player_position_xyz: [f32; 3]) -> Option<f32> {
        if !self.position_xyz.iter().all(|value| value.is_finite())
            || !player_position_xyz.iter().all(|value| value.is_finite())
        {
            return None;
        }
        let delta = [
            player_position_xyz[0] + ROBOTS_PICKUP_ATTRACT_TARGET_OFFSET_XYZ[0]
                - self.position_xyz[0],
            player_position_xyz[1] + ROBOTS_PICKUP_ATTRACT_TARGET_OFFSET_XYZ[1]
                - self.position_xyz[1],
            player_position_xyz[2] + ROBOTS_PICKUP_ATTRACT_TARGET_OFFSET_XYZ[2]
                - self.position_xyz[2],
        ];
        Some(delta.iter().map(|value| value * value).sum())
    }
}

const ROBOTS_PICKUP_BLUEPRINT_UIDS: [u32; 8] = [
    0x4700_000E,
    0x4700_000F,
    0x4700_0010,
    0x4700_0011,
    0x4700_0012,
    0x4700_0014,
    0x4700_0013,
    0x4700_0015,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsPickupSpawnLayout {
    pub serialized_trigger_type: u32,
    pub native_runtime_type: u32,
    pub pickup_uid: u32,
    pub handler_datum_uid: u32,
    pub quantity: i32,
    pub registration_flags: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsPickupSpawnPlan {
    pub layout: RobotsPickupSpawnLayout,
    /// XItem `+0xE4` written by `0x0048A1E0`: native unit RNG multiplied by
    /// `360.0 * pi/180`, i.e. one full turn in radians.
    pub initial_spin_radians: f32,
}

impl RobotsPickupSpawnLayout {
    pub fn with_random_unit(self, random_unit: f32) -> Option<RobotsPickupSpawnPlan> {
        if !random_unit.is_finite() || !(0.0..1.0).contains(&random_unit) {
            return None;
        }
        Some(RobotsPickupSpawnPlan {
            layout: self,
            initial_spin_radians: random_unit * std::f32::consts::TAU,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsPickupCreateGate {
    Ready(RobotsPickupSpawnLayout),
    Suppressed,
    UnresolvedPlayerItemPreflight,
    InvalidLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsPickupCollectionRoute {
    HandlerFallback,
    PlayerItemOwner,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsPickupProximityPlan {
    OutsideBroadRange,
    WaitForCollectRange {
        collect_range_squared: f32,
        allow_proximity_probe: bool,
    },
    Collect {
        route: RobotsPickupCollectionRoute,
    },
    InventoryRejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsPickupLifetimeStep {
    pub spin_delta_radians: f32,
    pub fade_scalar: Option<f32>,
    pub collect_on_expiry: bool,
    pub destroy_on_expiry: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsPickupPeriodicServiceStep {
    pub candidate_ordinal: Option<u32>,
    pub play_pickup_sound_uid: Option<u32>,
    pub owner_runtime_bit_0x02: bool,
    pub collected: bool,
}

/// Process-global Pickup scheduling state. Native globals are
/// `0x007B25B4/B8/BC/C0` and are owned by the frame scheduler, not by one Pickup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct RobotsPickupGlobalSchedulerState {
    pub cadence_phase_b4: u8,
    pub periodic_candidate_count_b8: u32,
    pub selected_periodic_ordinal_bc: u32,
    pub pickup_sound_cooldown_c0: i8,
}

impl RobotsPickupGlobalSchedulerState {
    pub fn next_handler_constructor_phase(&mut self) -> u8 {
        self.cadence_phase_b4 = self.cadence_phase_b4.wrapping_add(1);
        if self.cadence_phase_b4 > 10 {
            self.cadence_phase_b4 = 0;
        }
        self.cadence_phase_b4
    }

    pub fn begin_fixed_update(&mut self) {
        self.periodic_candidate_count_b8 = 0;
    }

    pub fn finish_fixed_update(&mut self, random_u32: Option<u32>) -> bool {
        self.pickup_sound_cooldown_c0 = self.pickup_sound_cooldown_c0.wrapping_sub(1);
        if self.periodic_candidate_count_b8 == 0 {
            self.selected_periodic_ordinal_bc = 0;
            return false;
        }
        let Some(random_u32) = random_u32 else {
            return false;
        };
        self.selected_periodic_ordinal_bc = random_u32 % self.periodic_candidate_count_b8;
        true
    }

    pub fn next_proximity_cadence_phase(&mut self, player_mode: u8) -> u8 {
        if matches!(player_mode, 3 | 4) {
            self.cadence_phase_b4 = 0;
            return 0;
        }
        let limit = if player_mode == 2 { 3 } else { 10 };
        self.cadence_phase_b4 = self.cadence_phase_b4.wrapping_add(1);
        if self.cadence_phase_b4 > limit {
            self.cadence_phase_b4 = 0;
        }
        self.cadence_phase_b4
    }
}

/// Handler-owned state from `XItemHandler_Pickup` constructor `0x00413A70`.
/// Trigger `+0xE8` suppression remains in `RobotsPickupRuntimeState` below.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsPickupHandlerRuntimeState {
    pub lifetime_seconds_3c0: f32,
    pub lifetime_limit_seconds_3c4: f32,
    pub pickup_uid_3c8: u32,
    pub quantity_3cc: i32,
    pub cadence_3d0: u8,
    pub flags_3d1: u8,
    pub proximity_cadence_enabled_3d2: bool,
    pub periodic_sound_cooldown_3d3: i8,
}

impl RobotsPickupHandlerRuntimeState {
    pub fn new(
        pickup_uid: u32,
        quantity: i32,
        global: &mut RobotsPickupGlobalSchedulerState,
    ) -> Self {
        Self {
            lifetime_seconds_3c0: 0.0,
            lifetime_limit_seconds_3c4: ROBOTS_PICKUP_INITIAL_LIFETIME_SECONDS,
            pickup_uid_3c8: pickup_uid,
            quantity_3cc: quantity,
            cadence_3d0: global.next_handler_constructor_phase(),
            flags_3d1: 0,
            proximity_cadence_enabled_3d2: true,
            periodic_sound_cooldown_3d3: 30,
        }
    }

    pub fn collected(&self) -> bool {
        self.flags_3d1 & ROBOTS_PICKUP_COLLECTED_FLAG != 0
    }

    pub fn advance_lifetime(
        &mut self,
        scaled_fixed_delta_seconds: f32,
        spin_enabled: bool,
    ) -> RobotsPickupLifetimeStep {
        let mut step = RobotsPickupLifetimeStep {
            spin_delta_radians: if spin_enabled {
                ROBOTS_PICKUP_SPIN_STEP_RADIANS
            } else {
                0.0
            },
            fade_scalar: None,
            collect_on_expiry: false,
            destroy_on_expiry: false,
        };
        if self.flags_3d1 & 0x01 == 0 {
            return step;
        }

        self.lifetime_seconds_3c0 += scaled_fixed_delta_seconds;
        if self.lifetime_limit_seconds_3c4 - 1.0 < self.lifetime_seconds_3c0 {
            let remaining = self.lifetime_limit_seconds_3c4 - self.lifetime_seconds_3c0;
            if remaining < 0.0 {
                if self.flags_3d1 & ROBOTS_PICKUP_KILL_ON_EXPIRY_FLAG == 0 {
                    step.collect_on_expiry = true;
                } else {
                    step.destroy_on_expiry = true;
                }
            }
            step.fade_scalar = Some(remaining.clamp(0.0, 1.0));
        }
        step
    }

    pub fn proximity_plan(
        &self,
        distance_squared: f32,
        player_interaction_range_squared: f32,
        player_mode: u8,
        player_item_policy_allows: bool,
        player_item_preflight_result: Option<u32>,
    ) -> RobotsPickupProximityPlan {
        let broad_range_squared =
            player_interaction_range_squared.max(ROBOTS_PICKUP_BROAD_RANGE_SQUARED);
        if !(distance_squared < broad_range_squared) {
            return RobotsPickupProximityPlan::OutsideBroadRange;
        }

        let route = if player_item_policy_allows {
            Some(RobotsPickupCollectionRoute::PlayerItemOwner)
        } else if matches!(player_item_preflight_result, Some(1 | 4)) {
            Some(RobotsPickupCollectionRoute::PlayerItemOwner)
        } else if self.pickup_uid_3c8 == 0x4700_0001 {
            Some(RobotsPickupCollectionRoute::HandlerFallback)
        } else {
            None
        };
        let Some(route) = route else {
            return RobotsPickupProximityPlan::InventoryRejected;
        };

        let (collect_range_squared, allow_proximity_probe) = match player_mode {
            2 => (ROBOTS_PICKUP_MODE2_COLLECT_RANGE_SQUARED, true),
            3 | 4 => (ROBOTS_PICKUP_MODE34_COLLECT_RANGE_SQUARED, false),
            _ => (ROBOTS_PICKUP_DEFAULT_COLLECT_RANGE_SQUARED, true),
        };
        if distance_squared < collect_range_squared {
            RobotsPickupProximityPlan::Collect { route }
        } else {
            RobotsPickupProximityPlan::WaitForCollectRange {
                collect_range_squared,
                allow_proximity_probe,
            }
        }
    }

    pub fn finish_proximity_cadence(
        &mut self,
        distance_squared: f32,
        player_mode: u8,
        global: &mut RobotsPickupGlobalSchedulerState,
    ) {
        if !self.proximity_cadence_enabled_3d2 {
            self.cadence_3d0 = 0;
        } else if ROBOTS_PICKUP_FAR_CADENCE_RANGE_SQUARED < distance_squared {
            self.cadence_3d0 = 20;
        } else {
            self.cadence_3d0 = global.next_proximity_cadence_phase(player_mode);
        }
    }

    pub fn tick_cadence_gate(&mut self) -> bool {
        if self.cadence_3d0 == 0 {
            false
        } else {
            self.cadence_3d0 = self.cadence_3d0.wrapping_sub(1);
            true
        }
    }

    pub fn service_periodic(
        &mut self,
        owner_runtime_type: u32,
        owner_animation_suppressed: bool,
        global: &mut RobotsPickupGlobalSchedulerState,
    ) -> RobotsPickupPeriodicServiceStep {
        let periodic = self.flags_3d1 & ROBOTS_PICKUP_PERIODIC_FLAG != 0;
        let mut step = RobotsPickupPeriodicServiceStep {
            candidate_ordinal: None,
            play_pickup_sound_uid: None,
            owner_runtime_bit_0x02: periodic,
            collected: self.collected(),
        };
        if periodic {
            if self.periodic_sound_cooldown_3d3 > 0 {
                self.periodic_sound_cooldown_3d3 -= 1;
            }
            let ordinal = global.periodic_candidate_count_b8;
            step.candidate_ordinal = Some(ordinal);
            if global.selected_periodic_ordinal_bc == ordinal
                && global.pickup_sound_cooldown_c0 < 1
                && self.periodic_sound_cooldown_3d3 < 1
            {
                if !owner_animation_suppressed {
                    step.play_pickup_sound_uid = Some(if owner_runtime_type == 0x2D {
                        0x0400_02FC
                    } else {
                        0x0400_02FB
                    });
                }
                self.periodic_sound_cooldown_3d3 = 30;
                global.pickup_sound_cooldown_c0 = 5;
            }
            global.periodic_candidate_count_b8 = global.periodic_candidate_count_b8.wrapping_add(1);
        }
        step
    }

    pub fn mark_collected(&mut self) {
        self.flags_3d1 |= ROBOTS_PICKUP_COLLECTED_FLAG;
    }
}

/// Pickup-local persistent state. `0x00489EB0` initializes `+0xE8=0` and
/// `0x0048A7B0` restores the saved byte, while service event `0x1000` clears it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct RobotsPickupRuntimeState {
    pub suppressed_e8: bool,
}

impl RobotsPickupRuntimeState {
    pub fn from_loaded_suppressed(suppressed_e8: bool) -> Self {
        Self { suppressed_e8 }
    }

    pub fn dispatch_event(&mut self, event_mask: u32) {
        if event_mask & 0x1000 != 0 {
            self.suppressed_e8 = false;
        }
    }

    /// Mirrors the class-specific front of `0x0048A180`.
    ///
    /// Runtime type `0x30` (serialized `0x1C`) calls `0x0048A080` only while
    /// `+0xE8==0`. That preflight uses Player-item UID `0x4800000E + data[0]`;
    /// native result `1/4` permits creation, every other concrete result latches
    /// `+0xE8=1`. The caller supplies that already-resolved native preflight
    /// result so this reducer stays independent of any engine/database owner.
    pub fn prepare_create(
        &mut self,
        serialized_trigger_type: u32,
        data: &[Option<u32>],
        player_item_preflight_result: Option<u32>,
    ) -> RobotsPickupCreateGate {
        if self.suppressed_e8 {
            return RobotsPickupCreateGate::Suppressed;
        }

        if serialized_trigger_type == ROBOTS_PICKUP_TRICKCHIP_SERIALIZED_TYPE {
            if robots_pickup_special_player_item_uid(serialized_trigger_type, data).is_none() {
                return RobotsPickupCreateGate::InvalidLayout;
            }
            let Some(result) = player_item_preflight_result else {
                return RobotsPickupCreateGate::UnresolvedPlayerItemPreflight;
            };
            if result != 1 && result != 4 {
                self.suppressed_e8 = true;
                return RobotsPickupCreateGate::Suppressed;
            }
        }

        robots_pickup_spawn_layout(serialized_trigger_type, data)
            .map(RobotsPickupCreateGate::Ready)
            .unwrap_or(RobotsPickupCreateGate::InvalidLayout)
    }
}

pub fn is_robots_pickup_serialized_type(serialized_trigger_type: u32) -> bool {
    ROBOTS_PICKUP_SERIALIZED_TYPES.contains(&serialized_trigger_type)
}

pub fn robots_pickup_native_runtime_type(serialized_trigger_type: u32) -> Option<u32> {
    Some(match serialized_trigger_type {
        0x1D => 0x29,
        0x1E => 0x2A,
        0x29 => 0x2B,
        0x19 => 0x2C,
        0x1B => 0x2D,
        0x53 => 0x2E,
        0x1A => 0x2F,
        0x1C => 0x30,
        0x3E => 0x31,
        0x3F => 0x32,
        0x40 => 0x33,
        0x41 => 0x34,
        0x43 => 0x35,
        0x47 => 0x36,
        0x52 => 0x37,
        _ => return None,
    })
}

pub fn robots_pickup_special_player_item_uid(
    serialized_trigger_type: u32,
    data: &[Option<u32>],
) -> Option<u32> {
    if serialized_trigger_type != ROBOTS_PICKUP_TRICKCHIP_SERIALIZED_TYPE {
        return None;
    }
    let index = data.first().copied().flatten()?;
    if index >= 16 {
        return None;
    }
    let uid = ROBOTS_PLAYER_ITEM_GATE_FIRST + index;
    (uid <= ROBOTS_PLAYER_ITEM_GATE_LAST).then_some(uid)
}

fn raw_i32(data: &[Option<u32>], index: usize) -> i32 {
    data.get(index).copied().flatten().unwrap_or(u32::MAX) as i32
}

pub fn robots_pickup_spawn_layout(
    serialized_trigger_type: u32,
    data: &[Option<u32>],
) -> Option<RobotsPickupSpawnLayout> {
    let native_runtime_type = robots_pickup_native_runtime_type(serialized_trigger_type)?;
    let (pickup_uid, handler_datum_uid, quantity, registration_flags) = match native_runtime_type {
        0x29 => {
            let family = data.first().copied().flatten()? as usize;
            let pickup_uid = *ROBOTS_PICKUP_BLUEPRINT_UIDS.get(family)?;
            (
                pickup_uid,
                pickup_uid,
                raw_i32(data, 1),
                ROBOTS_PICKUP_DEFAULT_REGISTRATION_FLAGS,
            )
        }
        0x2A => (
            0x4700_000C,
            0x4700_000C,
            raw_i32(data, 0),
            ROBOTS_PICKUP_DEFAULT_REGISTRATION_FLAGS,
        ),
        0x2B => (
            0x4700_0016,
            0x4700_0016,
            raw_i32(data, 0),
            ROBOTS_PICKUP_DEFAULT_REGISTRATION_FLAGS,
        ),
        0x2C => (
            0x4700_0001,
            0x4700_0001,
            raw_i32(data, 0),
            ROBOTS_PICKUP_SCRAP_REGISTRATION_FLAGS,
        ),
        0x2D => (
            0x4700_0002,
            0x4700_0002,
            raw_i32(data, 0),
            ROBOTS_PICKUP_DEFAULT_REGISTRATION_FLAGS,
        ),
        0x2E => (
            0x4700_0001,
            0x4700_0001,
            25,
            ROBOTS_PICKUP_SCRAP_REGISTRATION_FLAGS,
        ),
        0x2F => (
            0x4700_0023,
            0x4700_0023,
            1,
            ROBOTS_PICKUP_DEFAULT_REGISTRATION_FLAGS,
        ),
        0x30 => {
            let player_item_uid =
                robots_pickup_special_player_item_uid(serialized_trigger_type, data)?;
            (
                0x4700_000D,
                player_item_uid,
                1,
                ROBOTS_PICKUP_DEFAULT_REGISTRATION_FLAGS,
            )
        }
        0x31 => (
            0x4700_0025,
            0x4700_0025,
            1,
            ROBOTS_PICKUP_DEFAULT_REGISTRATION_FLAGS,
        ),
        0x32 => (
            0x4700_0026,
            0x4700_0026,
            1,
            ROBOTS_PICKUP_DEFAULT_REGISTRATION_FLAGS,
        ),
        0x33 => (
            0x4700_0027,
            0x4700_0027,
            1,
            ROBOTS_PICKUP_DEFAULT_REGISTRATION_FLAGS,
        ),
        0x34 => (
            0x4700_0028,
            0x4700_0028,
            1,
            ROBOTS_PICKUP_DEFAULT_REGISTRATION_FLAGS,
        ),
        0x35 => (
            0x4700_0029,
            0x4700_0029,
            1,
            ROBOTS_PICKUP_DEFAULT_REGISTRATION_FLAGS,
        ),
        0x36 => (
            0x4700_002A,
            0x4700_002A,
            1,
            ROBOTS_PICKUP_DEFAULT_REGISTRATION_FLAGS,
        ),
        0x37 => (
            0x4700_002C,
            0x4700_002C,
            1,
            ROBOTS_PICKUP_DEFAULT_REGISTRATION_FLAGS,
        ),
        _ => return None,
    };

    Some(RobotsPickupSpawnLayout {
        serialized_trigger_type,
        native_runtime_type,
        pickup_uid,
        handler_datum_uid,
        quantity,
        registration_flags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialized_layouts_map_to_exact_native_runtime_types_and_pickup_uids() {
        let expected = [
            (0x19, 0x2C, 0x4700_0001),
            (0x1A, 0x2F, 0x4700_0023),
            (0x1B, 0x2D, 0x4700_0002),
            (0x1C, 0x30, 0x4700_000D),
            (0x1D, 0x29, 0x4700_000E),
            (0x1E, 0x2A, 0x4700_000C),
            (0x29, 0x2B, 0x4700_0016),
            (0x3E, 0x31, 0x4700_0025),
            (0x3F, 0x32, 0x4700_0026),
            (0x40, 0x33, 0x4700_0027),
            (0x41, 0x34, 0x4700_0028),
            (0x43, 0x35, 0x4700_0029),
            (0x47, 0x36, 0x4700_002A),
            (0x52, 0x37, 0x4700_002C),
            (0x53, 0x2E, 0x4700_0001),
        ];
        for (serialized, runtime, pickup_uid) in expected {
            let data = if serialized == 0x1C || serialized == 0x1D {
                vec![Some(0), Some(7)]
            } else {
                vec![Some(7)]
            };
            let layout = robots_pickup_spawn_layout(serialized, &data).unwrap();
            assert_eq!(
                layout.native_runtime_type, runtime,
                "serialized=0x{serialized:02X}"
            );
            assert_eq!(
                layout.pickup_uid, pickup_uid,
                "serialized=0x{serialized:02X}"
            );
        }
    }

    #[test]
    fn blueprint_family_and_quantity_match_native_tables() {
        let expected_uids = [
            0x4700_000E,
            0x4700_000F,
            0x4700_0010,
            0x4700_0011,
            0x4700_0012,
            0x4700_0014,
            0x4700_0013,
            0x4700_0015,
        ];
        for (family, expected_uid) in expected_uids.into_iter().enumerate() {
            let layout =
                robots_pickup_spawn_layout(0x1D, &[Some(family as u32), Some(19)]).unwrap();
            assert_eq!(layout.pickup_uid, expected_uid);
            assert_eq!(layout.handler_datum_uid, expected_uid);
            assert_eq!(layout.quantity, 19);
        }
        assert!(robots_pickup_spawn_layout(0x1D, &[Some(8), Some(1)]).is_none());
    }

    #[test]
    fn trickchip_uses_same_exact_sixteen_player_item_table_for_gate_and_handler_datum() {
        for index in 0..16u32 {
            let data = [Some(index)];
            let expected = 0x4800_000E + index;
            assert_eq!(
                robots_pickup_special_player_item_uid(0x1C, &data),
                Some(expected)
            );
            let layout = robots_pickup_spawn_layout(0x1C, &data).unwrap();
            assert_eq!(layout.pickup_uid, 0x4700_000D);
            assert_eq!(layout.handler_datum_uid, expected);
            assert_eq!(layout.quantity, 1);
        }
        assert_eq!(
            robots_pickup_special_player_item_uid(0x1C, &[Some(16)]),
            None
        );
    }

    #[test]
    fn eligibility_latch_matches_initialize_service_and_trickchip_preflight_order() {
        let mut state = RobotsPickupRuntimeState::default();
        assert!(!state.suppressed_e8);

        assert_eq!(
            state.prepare_create(0x1C, &[Some(2)], Some(3)),
            RobotsPickupCreateGate::Suppressed
        );
        assert!(state.suppressed_e8);
        assert_eq!(
            state.prepare_create(0x1C, &[Some(2)], Some(4)),
            RobotsPickupCreateGate::Suppressed
        );
        state.dispatch_event(0x1000);
        assert!(!state.suppressed_e8);

        assert!(matches!(
            state.prepare_create(0x1C, &[Some(2)], Some(4)),
            RobotsPickupCreateGate::Ready(_)
        ));
        assert!(!state.suppressed_e8);
        assert_eq!(
            state.prepare_create(0x1C, &[Some(2)], None),
            RobotsPickupCreateGate::UnresolvedPlayerItemPreflight
        );
    }

    #[test]
    fn spawn_payload_preserves_native_quantity_registration_and_one_turn_rng_scale() {
        let scrap = robots_pickup_spawn_layout(0x19, &[Some(50)]).unwrap();
        assert_eq!(scrap.quantity, 50);
        assert_eq!(
            scrap.registration_flags,
            ROBOTS_PICKUP_SCRAP_REGISTRATION_FLAGS
        );

        let fixed = robots_pickup_spawn_layout(0x53, &[]).unwrap();
        assert_eq!(fixed.quantity, 25);
        assert_eq!(
            fixed.registration_flags,
            ROBOTS_PICKUP_SCRAP_REGISTRATION_FLAGS
        );

        let plan = fixed.with_random_unit(0.5).unwrap();
        assert!((plan.initial_spin_radians - std::f32::consts::PI).abs() <= 1.0e-6);
        assert!(fixed.with_random_unit(1.0).is_none());
        assert!(fixed.with_random_unit(f32::NAN).is_none());
    }

    #[test]
    fn handler_constructor_keeps_trigger_state_separate_and_staggers_phase() {
        let mut global = RobotsPickupGlobalSchedulerState::default();
        let first = RobotsPickupHandlerRuntimeState::new(0x4700_0001, 7, &mut global);
        let second = RobotsPickupHandlerRuntimeState::new(0x4700_0002, 3, &mut global);
        assert_eq!(first.cadence_3d0, 1);
        assert_eq!(second.cadence_3d0, 2);
        assert_eq!(first.lifetime_limit_seconds_3c4, 5.0);
        assert_eq!(first.periodic_sound_cooldown_3d3, 30);
    }

    #[test]
    fn proximity_plan_preserves_native_mode_ranges_and_scrap_fallback() {
        let mut global = RobotsPickupGlobalSchedulerState::default();
        let scrap = RobotsPickupHandlerRuntimeState::new(0x4700_0001, 5, &mut global);
        assert_eq!(
            scrap.proximity_plan(0.5, 1.0, 0, false, None),
            RobotsPickupProximityPlan::Collect {
                route: RobotsPickupCollectionRoute::HandlerFallback
            }
        );
        assert_eq!(
            scrap.proximity_plan(2.0, 1.0, 2, true, Some(1)),
            RobotsPickupProximityPlan::WaitForCollectRange {
                collect_range_squared: ROBOTS_PICKUP_MODE2_COLLECT_RANGE_SQUARED,
                allow_proximity_probe: true,
            }
        );
        assert_eq!(
            scrap.proximity_plan(8.0, 9.0, 3, true, Some(4)),
            RobotsPickupProximityPlan::Collect {
                route: RobotsPickupCollectionRoute::PlayerItemOwner
            }
        );

        let ordinary = RobotsPickupHandlerRuntimeState::new(0x4700_0002, 1, &mut global);
        assert_eq!(
            ordinary.proximity_plan(0.5, 1.0, 0, false, Some(3)),
            RobotsPickupProximityPlan::InventoryRejected
        );
    }

    #[test]
    fn lifetime_step_uses_native_last_second_fade_and_expiry_route() {
        let mut global = RobotsPickupGlobalSchedulerState::default();
        let mut pickup = RobotsPickupHandlerRuntimeState::new(0x4700_0002, 1, &mut global);
        pickup.flags_3d1 |= 0x01;
        pickup.lifetime_seconds_3c0 = 4.25;
        let fade = pickup.advance_lifetime(0.25, true);
        assert_eq!(fade.spin_delta_radians, ROBOTS_PICKUP_SPIN_STEP_RADIANS);
        assert_eq!(fade.fade_scalar, Some(0.5));
        assert!(!fade.collect_on_expiry);
        assert!(!fade.destroy_on_expiry);

        let expired = pickup.advance_lifetime(0.6, true);
        assert_eq!(expired.fade_scalar, Some(0.0));
        assert!(expired.collect_on_expiry);
        assert!(!expired.destroy_on_expiry);

        pickup.flags_3d1 |= ROBOTS_PICKUP_KILL_ON_EXPIRY_FLAG;
        let killed = pickup.advance_lifetime(0.1, false);
        assert!(!killed.collect_on_expiry);
        assert!(killed.destroy_on_expiry);
    }

    #[test]
    fn pickup_attract_replays_native_default_offset_force_limits_and_damping() {
        let mut ordinary = RobotsPickupAttractPhysicsState::new([0.0, 0.0, 0.0]);
        assert!(ordinary.advance_fixed([1.2, 0.0, 0.0], 0, 1.0));
        assert!((ordinary.position_xyz[0] - 0.02).abs() < 1.0e-7);
        assert!((ordinary.position_xyz[1] - (0.5 / 60.0)).abs() < 1.0e-7);
        assert!((ordinary.local_velocity_xyz[0] - 0.02 * 0.93).abs() < 1.0e-7);
        assert!((ordinary.local_velocity_xyz[1] - (0.5 / 60.0) * 0.93).abs() < 1.0e-7);

        let mut far_default = RobotsPickupAttractPhysicsState::new([0.0, 0.0, 0.0]);
        assert!(far_default.advance_fixed([10.0, -0.5, 0.0], 0, 1.0));
        assert!((far_default.position_xyz[0] - (2.0 / 60.0)).abs() < 1.0e-7);

        let mut far_mode3 = RobotsPickupAttractPhysicsState::new([0.0, 0.0, 0.0]);
        assert!(far_mode3.advance_fixed([10.0, -0.5, 0.0], 3, 1.0));
        assert!((far_mode3.position_xyz[0] - (81.0 / 60.0)).abs() < 1.0e-6);
        assert!((far_mode3.local_velocity_xyz[0] - (81.0 / 60.0) * 0.97).abs() < 1.0e-6);

        let near_target = RobotsPickupAttractPhysicsState::new([0.0, 0.0, 0.0]);
        assert_eq!(
            near_target.target_distance_squared([0.0, 0.0, 0.0]),
            Some(0.25)
        );
        assert!(near_target
            .target_distance_squared([f32::NAN, 0.0, 0.0])
            .is_none());
    }

    #[test]
    fn periodic_service_registers_global_ordinal_and_applies_exact_sound_cooldowns() {
        let mut global = RobotsPickupGlobalSchedulerState {
            selected_periodic_ordinal_bc: 0,
            pickup_sound_cooldown_c0: 0,
            ..Default::default()
        };
        let mut pickup = RobotsPickupHandlerRuntimeState::new(0x4700_0002, 1, &mut global);
        pickup.flags_3d1 |= ROBOTS_PICKUP_PERIODIC_FLAG;
        pickup.periodic_sound_cooldown_3d3 = 0;
        global.begin_fixed_update();
        let step = pickup.service_periodic(0x2D, false, &mut global);
        assert_eq!(step.candidate_ordinal, Some(0));
        assert_eq!(step.play_pickup_sound_uid, Some(0x0400_02FC));
        assert!(step.owner_runtime_bit_0x02);
        assert_eq!(pickup.periodic_sound_cooldown_3d3, 30);
        assert_eq!(global.pickup_sound_cooldown_c0, 5);
        assert_eq!(global.periodic_candidate_count_b8, 1);

        assert!(global.finish_fixed_update(Some(17)));
        assert_eq!(global.selected_periodic_ordinal_bc, 0);
        assert_eq!(global.pickup_sound_cooldown_c0, 4);
    }
}
