use std::io::{Seek, SeekFrom};

use anyhow::Context;
use eurochef_edb::{binrw::BinReaderExt, edb::EdbFile};
use serde::Serialize;

use crate::{
    script::{
        robots_script_payload_diagnostic, RobotsScriptPayloadDiagnostic, UXGeoScript,
        UXGeoScriptCommandData,
    },
    spreadsheets::UXGeoSpreadsheet,
};

use super::{
    hit_candidate_policy::ROBOTS_HIT_QUERY_EXCLUDE_RAW_GROUP1_FLAG,
    hit_query::RobotsHitQueryInitPlan,
    process_rng::{robots_process_lcg_next_u31, ROBOTS_PROCESS_LCG_UNIT_FLOAT_SCALE},
    projectile::{
        robots_projectile_ballistic_state_from_target, RobotsProjectilePhysicsRuntimeState,
        ROBOTS_PROJECTILE_GRAVITY,
    },
};

pub const ROBOTS_EXPLOSION_DATABASE_FILE_UID: u32 = 0x0100_0038;
pub const ROBOTS_EXPLOSION_SPREADSHEET_UID: u32 = 0x1400_000A;
pub const ROBOTS_EXPLOSION_GENERATION_ANIM_DATUM: u32 = 0x1000_002C;
pub const ROBOTS_EXPLOSION_FRAGMENT_ANIM_DATUM: u32 = 0x1000_002D;
pub const ROBOTS_EXPLOSION_ROW_SIZE: usize = 0x44;
pub const ROBOTS_EXPLOSION_ROW_WORDS: usize = ROBOTS_EXPLOSION_ROW_SIZE / 4;
pub const ROBOTS_EXPLOSION_FRAGMENT_COUNT: usize = 10;
/// `XExplosionFragment::+0x0C 0x004DB6B0` increments Handler+0x3E4 once per
/// Handler service and enters terminal teardown when the count reaches 0x4B0.
pub const ROBOTS_EXPLOSION_FRAGMENT_MAX_HANDLER_UPDATES: u32 = 0x4B0;
pub const ROBOTS_EXPLOSION_FRAGMENT_ROW_SIZE: usize = 0x20;
pub const ROBOTS_EXPLOSION_FRAGMENT_ROW_WORDS: usize = ROBOTS_EXPLOSION_FRAGMENT_ROW_SIZE / 4;
pub const ROBOTS_EXPLOSION_FRAGMENT_RANDOM_SCALAR_FLAG: u32 = 0x0000_0001;
pub const ROBOTS_EXPLOSION_FRAGMENT_ATTACHMENT_SETUP_FLAG: u32 = 0x0000_0002;
pub const ROBOTS_EXPLOSION_FRAGMENT_SCRIPT_STATE_MARKER_FLAG: u32 = 0x0000_0004;
/// Low raw FX03 flag serviced by `XExplosionFragment::Update 0x004DB6B0` through
/// the explicit-shape HitQuery helper `0x00425800`.
pub const ROBOTS_EXPLOSION_FRAGMENT_HIT_QUERY_FLAG: u32 = 0x0000_0008;
/// Raw FX03 bit paired with raw bit 0x4 by `0x004DC0F0`: once the fragment has
/// fallen more than two world units below its spawn Y, native emits its pickup at
/// the original position before teardown.
pub const ROBOTS_EXPLOSION_FRAGMENT_DROP_PICKUP_ON_FALL_FLAG: u32 = 0x0000_0010;
pub const ROBOTS_EXPLOSION_FRAGMENT_DIRECTIONAL_FLAG: u32 = 0x0000_0100;
pub const ROBOTS_EXPLOSION_FRAGMENT_RANDOM_SCALE_FLAG: u32 = 0x0000_0200;
/// `0x004DD1EC..0x004DD23A`: this raw fragment bit maps to common HitQuery flag
/// 0x20000 (exclude raw group1 candidates) when `0x004DC2E0` initializes +0x3E8.
pub const ROBOTS_EXPLOSION_FRAGMENT_QUERY_EXCLUDE_GROUP1_SEED_FLAG: u32 = 0x0000_0400;
pub const ROBOTS_EXPLOSION_FRAGMENT_HIT_QUERY_RADIUS: f32 = 0.2;
pub const ROBOTS_EXPLOSION_FRAGMENT_HIT_QUERY_MIN_SPEED_SQUARED: f32 = 1.0;
/// `XExplosionFragment::0x004DB8B0` reflects Projectile physics on contact when
/// raw fragment bit 0x20 is present. This is the dominant shipped FX03 mode.
pub const ROBOTS_EXPLOSION_FRAGMENT_REBOUND_FLAG: u32 = 0x0000_0020;
/// Base Physics+0x138 is initialized to 0.98 by `0x004191A0`; fragment rebound
/// multiplies the effective velocity by this value before reflection.
pub const ROBOTS_EXPLOSION_FRAGMENT_REBOUND_DAMPING: f32 = 0.98;
/// Native rebound folds 70% of Physics+0x124 gravity velocity into the effective
/// Y component before damping/reflection.
pub const ROBOTS_EXPLOSION_FRAGMENT_REBOUND_GRAVITY_MIX: f32 = 0.7;
/// Reflected motion is kept when the three-axis speed squared is >= 1.0.
pub const ROBOTS_EXPLOSION_FRAGMENT_REBOUND_MIN_SPEED_SQUARED: f32 = 1.0;
/// Runtime-only Handler+0x3C0 bit set by `0x004DBCC0` after the first physical
/// rebound for ordinary fragments. While bit 0x20000 is clear it drives fade-out.
pub const ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_FADE_OUT_FLAG: u32 = 0x0001_0000;
/// Runtime-only Handler+0x3C0 bit consumed by `0x004DBCC0`; when paired with the
/// fade-out latch it routes the fragment back through the +0.06 fade-in branch.
pub const ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_FADE_REVERSE_FLAG: u32 = 0x0002_0000;
/// `0x004DBF90` sets this after the fragment pickup value has been committed so
/// teardown does not emit a duplicate pickup object.
pub const ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_PICKUP_HANDLED_FLAG: u32 = 0x0004_0000;
/// `0x004DBEF0` sets this player-proximity/attraction latch. `0x004DB8B0` then
/// suppresses the ordinary physical-contact teardown while the latch is active.
pub const ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_PLAYER_PICKUP_LATCH: u32 = 0x0008_0000;
/// Terminal latch written by `0x004DBB20` / `0x004DC0F0` after XItem teardown.
pub const ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_TEARDOWN_FLAG: u32 = 0x0010_0000;
/// Runtime-only branch flag seeded during fragment creation from the global
/// fragment-chain condition; `0x004DC0F0` consumes it together with raw bit 0x4.
pub const ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_EARLY_PICKUP_FLAG: u32 = 0x0020_0000;
pub const ROBOTS_EXPLOSION_FRAGMENT_VISIBILITY_STEP: f32 = 0.06;
pub const ROBOTS_EXPLOSION_FRAGMENT_MAX_FALL_DISTANCE: f32 = 2.0;
/// `0x004DB6CF` divides the process-wide engine frame counter `0x008CB338` by 30
/// and services the staggered fragment lane only when the remainder equals +0x48C.
pub const ROBOTS_EXPLOSION_FRAGMENT_CADENCE_PERIOD: u32 = 30;
pub const ROBOTS_EXPLOSION_FRAGMENT_PICKUP_RANDOM_MASK: u32 = 0x0000_0084;
pub const ROBOTS_EXPLOSION_FRAGMENT_PICKUP_RANDOM_VALUE: u32 = 0x0000_0004;
pub const ROBOTS_EXPLOSION_FRAGMENT_AZIMUTH_U31_SCALE: f32 = f32::from_bits(0x3149_0FDB);
pub const ROBOTS_EXPLOSION_FRAGMENT_RANDOM_SCALAR_U31_SCALE: f32 = f32::from_bits(0x2E56_7750);
pub const ROBOTS_EXPLOSION_FRAGMENT_MIN_HORIZONTAL_DISTANCE: f32 = 0.3;
pub const ROBOTS_EXPLOSION_FRAGMENT_MIN_VERTICAL_BIAS: f32 = 0.25;
pub const ROBOTS_EXPLOSION_FRAGMENT_VERTICAL_RANGE_ADD: f32 = 0.4;
pub const ROBOTS_EXPLOSION_FRAGMENT_RANDOM_SCALE_MIN: f32 = 0.4;
pub const ROBOTS_EXPLOSION_FRAGMENT_RANDOM_SCALE_RANGE: f32 = 0.6;
pub const ROBOTS_EXPLOSION_FRAGMENT_GENERATION_NORMALIZE_EPSILON: f32 = 0.000_01;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsExplosionCollisionVectorKey {
    pub frame_bits: u32,
    pub xyz_bits: [u32; 3],
}

impl RobotsExplosionCollisionVectorKey {
    fn from_native(frame: f32, xyz: [f32; 3]) -> Self {
        Self {
            frame_bits: frame.to_bits(),
            xyz_bits: xyz.map(f32::to_bits),
        }
    }

    pub fn frame(self) -> f32 {
        f32::from_bits(self.frame_bits)
    }

    pub fn xyz(self) -> [f32; 3] {
        self.xyz_bits.map(f32::from_bits)
    }
}

/// Direct opcode-13 `EXItemAnimator_Collision` owned by an FX03 main explosion Script.
/// The shipped corpus uses sphere mode 1 only. Float payloads stay bit-exact so the
/// database remains Eq/serializable without rounding away source data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RobotsExplosionCollisionProfile {
    pub explosion_selector: u32,
    pub script_uid: u32,
    pub command_start: i16,
    pub command_length: u16,
    pub datum_shape_mode: u8,
    pub serialized_shape_scalar_bits: [u32; 3],
    pub position_keys: Vec<RobotsExplosionCollisionVectorKey>,
    pub scale_keys: Vec<RobotsExplosionCollisionVectorKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsExplosionCollisionSample {
    pub local_center_xyz: [f32; 3],
    pub shape_scalar_xyz: [f32; 3],
}

fn sample_linear_vector_keys(
    keys: &[RobotsExplosionCollisionVectorKey],
    frame: f32,
    default: [f32; 3],
) -> [f32; 3] {
    let Some(first) = keys.first().copied() else {
        return default;
    };
    if frame <= first.frame() {
        return first.xyz();
    }
    for window in keys.windows(2) {
        let start = window[0];
        let end = window[1];
        if frame <= end.frame() {
            let start_frame = start.frame();
            let end_frame = end.frame();
            if end_frame <= start_frame {
                return end.xyz();
            }
            let t = ((frame - start_frame) / (end_frame - start_frame)).clamp(0.0, 1.0);
            let a = start.xyz();
            let b = end.xyz();
            return std::array::from_fn(|index| a[index] + (b[index] - a[index]) * t);
        }
    }
    keys.last().copied().map(|key| key.xyz()).unwrap_or(default)
}

impl RobotsExplosionCollisionProfile {
    pub fn command_end_frame(&self) -> f32 {
        f32::from(self.command_start) + f32::from(self.command_length)
    }

    pub fn active_at_frame(&self, frame: f32) -> bool {
        frame >= f32::from(self.command_start) && frame < self.command_end_frame()
    }

    pub fn sample_at_frame(&self, frame: f32) -> Option<RobotsExplosionCollisionSample> {
        if !frame.is_finite() || !self.active_at_frame(frame) {
            return None;
        }
        let serialized_shape = self.serialized_shape_scalar_bits.map(f32::from_bits);
        Some(RobotsExplosionCollisionSample {
            local_center_xyz: sample_linear_vector_keys(&self.position_keys, frame, [0.0; 3]),
            shape_scalar_xyz: sample_linear_vector_keys(&self.scale_keys, frame, serialized_shape),
        })
    }
}

/// Minimal native Script/Collision clock for the main Explosion XItem. The XItem Handler
/// runs before its attached Script animator. A newly created priority-0x14 Explosion is
/// not serviced on its spawn frame; on its first later XItem tick the Handler therefore
/// sees no opcode-13 child yet, then the Script creates/applies the current-frame child.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsExplosionCollisionRuntimeState {
    pub next_script_frame_bits: u32,
    pub applied_collision_frame_bits: Option<u32>,
}

impl Default for RobotsExplosionCollisionRuntimeState {
    fn default() -> Self {
        Self {
            next_script_frame_bits: 0.0f32.to_bits(),
            applied_collision_frame_bits: None,
        }
    }
}

impl RobotsExplosionCollisionRuntimeState {
    pub fn handler_sample(
        self,
        profile: &RobotsExplosionCollisionProfile,
    ) -> Option<RobotsExplosionCollisionSample> {
        profile.sample_at_frame(f32::from_bits(self.applied_collision_frame_bits?))
    }

    /// Replay the attached Script animator after the Handler phase. Native standalone
    /// Explosion Scripts advance by exactly one serialized Script frame per XItem tick.
    pub fn advance_animator(&mut self, profile: &RobotsExplosionCollisionProfile) {
        let frame = f32::from_bits(self.next_script_frame_bits);
        self.applied_collision_frame_bits = profile.active_at_frame(frame).then(|| frame.to_bits());
        self.next_script_frame_bits = (frame + 1.0).to_bits();
    }
}

/// Native 0x44-byte explosion definition consumed by `0x004DC510 -> 0x004DC6A0`.
///
/// Only fields with an independently proven consumer are named. The tail remains
/// raw instead of inventing semantics from one call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsExplosionDefinition {
    pub selector: u32,
    /// FX03 row +0x04. `0x004DC510` passes this as the main explosion Script/XItem
    /// resource to the generic XItem factory.
    pub main_script_uid: u32,
    /// FX03 row +0x08. Companion EDB/file UID passed beside `main_script_uid`.
    pub main_file_uid: u32,
    pub fragment_uids: [u32; ROBOTS_EXPLOSION_FRAGMENT_COUNT],
    /// FX03 row +0x34. `0x004DD332..0x004DD39C` scales the ordinary shipped
    /// fragment horizontal and vertical scatter ranges with this float.
    pub scatter_extent_bits_34: u32,
    /// FX03 row +0x38 -> HitQuery +0x8C initial flags.
    pub hit_query_initial_flags_38: u32,
    /// FX03 row +0x3C. Low two bytes are consumed by the optional init-side service
    /// at `0x004DC730`; keep raw until that service is identified.
    pub word_3c: u32,
    /// FX03 row +0x40 -> common HitQuery selector; -1 disables the query.
    pub hit_query_selector_40: u32,
}

impl RobotsExplosionDefinition {
    pub const fn from_native_words(words: [u32; ROBOTS_EXPLOSION_ROW_WORDS]) -> Self {
        Self {
            selector: words[0],
            main_script_uid: words[1],
            main_file_uid: words[2],
            fragment_uids: [
                words[3], words[4], words[5], words[6], words[7], words[8], words[9], words[10],
                words[11], words[12],
            ],
            scatter_extent_bits_34: words[13],
            hit_query_initial_flags_38: words[14],
            word_3c: words[15],
            hit_query_selector_40: words[16],
        }
    }

    pub const fn raw_words(self) -> [u32; ROBOTS_EXPLOSION_ROW_WORDS] {
        [
            self.selector,
            self.main_script_uid,
            self.main_file_uid,
            self.fragment_uids[0],
            self.fragment_uids[1],
            self.fragment_uids[2],
            self.fragment_uids[3],
            self.fragment_uids[4],
            self.fragment_uids[5],
            self.fragment_uids[6],
            self.fragment_uids[7],
            self.fragment_uids[8],
            self.fragment_uids[9],
            self.scatter_extent_bits_34,
            self.hit_query_initial_flags_38,
            self.word_3c,
            self.hit_query_selector_40,
        ]
    }

    /// FX03 row +0x40. `0x004DC6DA` passes this directly as arg0 to the common
    /// HitQuery initializer `0x00425A70`; -1 disables the embedded query.
    pub const fn scatter_extent(self) -> f32 {
        f32::from_bits(self.scatter_extent_bits_34)
    }

    pub const fn hit_query_selector(self) -> u32 {
        self.hit_query_selector_40
    }

    /// FX03 row +0x38. `0x004DC6EF..0x004DC704` passes this directly as the
    /// common HitQuery initializer's initial +0x8C flags word.
    pub const fn hit_query_initial_flags(self) -> u32 {
        self.hit_query_initial_flags_38
    }

    /// Explosion requests FLT_MAX from the common HitQuery initializer. Query
    /// lifetime/flags therefore stay in the shared HitQuery runtime, not here.
    pub fn hit_query_plan(self) -> RobotsHitQueryInitPlan {
        RobotsHitQueryInitPlan::direct(
            self.hit_query_selector(),
            f32::MAX,
            self.hit_query_initial_flags(),
        )
    }
}

/// Native 0x20-byte fragment row from FX03 sheet 1. Only the selector is named
/// until the child-XItem consumers are closed; the remaining words stay lossless.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsExplosionFragmentDefinition {
    pub selector: u32,
    /// Fragment Script/XItem resource pair consumed by `0x004DCC20 -> 0x004DCCE0`.
    pub script_uid: u32,
    pub file_uid: u32,
    /// Shipped FX03 keeps both words at -1 for all 161 rows. Preserve them losslessly
    /// without inventing semantics until a native consumer appears.
    pub reserved_0c: u32,
    pub reserved_10: u32,
    /// Optional pickup emitted/owned by the fragment. `0x00410BE0` explicitly tests
    /// this lane for HT_Pickup_Scrap and sums the signed low-16 quantity below.
    pub pickup_uid: u32,
    /// Native reads this with `movsx word [row+0x18]`; upper 16 bits are retained raw.
    pub pickup_quantity_word_18: u32,
    /// Raw behavior/physics flags copied to child Handler+0x3C0 and consumed by the
    /// projectile-physics setup in `0x004DCCE0`.
    pub flags: u32,
}

impl RobotsExplosionFragmentDefinition {
    pub const fn from_native_words(words: [u32; ROBOTS_EXPLOSION_FRAGMENT_ROW_WORDS]) -> Self {
        Self {
            selector: words[0],
            script_uid: words[1],
            file_uid: words[2],
            reserved_0c: words[3],
            reserved_10: words[4],
            pickup_uid: words[5],
            pickup_quantity_word_18: words[6],
            flags: words[7],
        }
    }

    pub const fn raw_words(self) -> [u32; ROBOTS_EXPLOSION_FRAGMENT_ROW_WORDS] {
        [
            self.selector,
            self.script_uid,
            self.file_uid,
            self.reserved_0c,
            self.reserved_10,
            self.pickup_uid,
            self.pickup_quantity_word_18,
            self.flags,
        ]
    }

    pub const fn pickup_quantity(self) -> i16 {
        self.pickup_quantity_word_18 as u16 as i16
    }

    /// `0x004DD5AD` initializes the child attachment container at Handler+0x13C
    /// and routes its resource through the shared `0x0042A180` attachment factory.
    pub const fn uses_attachment_setup(self) -> bool {
        self.flags & ROBOTS_EXPLOSION_FRAGMENT_ATTACHMENT_SETUP_FLAG != 0
    }

    /// `0x004DD5DA` runs the common Script StateMarker cache builder `0x00417600`
    /// and resets its current marker through `0x00417510`.
    pub const fn uses_script_state_markers(self) -> bool {
        self.flags & ROBOTS_EXPLOSION_FRAGMENT_SCRIPT_STATE_MARKER_FLAG != 0
    }

    /// `0x004DCEA6..0x004DCF02`: one gameplay-RNG draw produces a uniform XYZ
    /// child scale in `[0.4, 1.0)`. Shipped FX03 uses this only for row 0x53000057.
    pub const fn uses_random_uniform_scale(self) -> bool {
        self.flags & ROBOTS_EXPLOSION_FRAGMENT_RANDOM_SCALE_FLAG != 0
    }

    /// `0x004DCF06..0x004DCF92`: the pickup branch is selected by `(flags & 0x84)==4`.
    /// Native consumes its gameplay-RNG draw only after resolving `pickup_uid` in the
    /// pickup table; the host preserves that distinction rather than keying on bit4 alone.
    pub const fn uses_pickup_randomization(self) -> bool {
        self.flags & ROBOTS_EXPLOSION_FRAGMENT_PICKUP_RANDOM_MASK
            == ROBOTS_EXPLOSION_FRAGMENT_PICKUP_RANDOM_VALUE
    }

    /// Shipped FX03 has exactly three pickup-randomized rows and all three resolve
    /// through the native pickup-variant table before `0x004DA710` consumes its
    /// gameplay-RNG input: Scrap, GoldenScrap and SilverScrap.
    pub const fn shipped_pickup_variant_consumes_rng(self) -> bool {
        self.uses_pickup_randomization()
            && matches!(self.pickup_uid, 0x4700_0001 | 0x4700_0002 | 0x4700_002B)
    }
}

/// Source-independent ordinary shipped fragment trajectory plan from
/// `0x004DD2EE..0x004DD55B`. The host still owns the actual child-XItem position;
/// once that position is known this plan feeds the shared projectile physics seam.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsExplosionFragmentBallisticPlan {
    pub azimuth_radians: f32,
    pub horizontal_distance: f32,
    pub vertical_bias: f32,
    pub gravity_acceleration: f32,
    /// Fragment flag bit0 consumes two additional process-LCG steps and writes the
    /// second draw-derived scalar to Handler+0x3C4/+0x3C8.
    pub random_scalar: Option<f32>,
}

impl RobotsExplosionFragmentBallisticPlan {
    pub fn target_position_xyz(self, source_xyz: [f32; 3]) -> [f32; 3] {
        let (sin_azimuth, cos_azimuth) = self.azimuth_radians.sin_cos();
        [
            source_xyz[0] + sin_azimuth * self.horizontal_distance,
            source_xyz[1],
            source_xyz[2] + cos_azimuth * self.horizontal_distance,
        ]
    }

    pub fn projectile_physics(
        self,
        source_xyz: [f32; 3],
    ) -> Option<RobotsProjectilePhysicsRuntimeState> {
        robots_projectile_ballistic_state_from_target(
            source_xyz,
            self.target_position_xyz(source_xyz),
            self.vertical_bias,
            self.gravity_acceleration,
        )
    }
}

/// Replays the `HT_AnimDatum_ExplosionFragmentGeneration` random offset used by
/// `0x004DCD77..0x004DCE92`. Native draws three gameplay-RNG unit floats, forms
/// `(draw2, draw1, draw3)`, normalizes that positive-octant vector, and scales it
/// by the animated datum radius before adding the world center.
pub fn robots_explosion_fragment_generation_position(
    center_xyz: [f32; 3],
    radius: f32,
    gameplay_unit_draws: [f32; 3],
) -> Option<[f32; 3]> {
    if !center_xyz.iter().all(|value| value.is_finite())
        || !radius.is_finite()
        || !gameplay_unit_draws.iter().all(|value| value.is_finite())
    {
        return None;
    }
    let mut direction = [
        gameplay_unit_draws[1],
        gameplay_unit_draws[0],
        gameplay_unit_draws[2],
    ];
    let length_squared = direction.iter().map(|value| value * value).sum::<f32>();
    if length_squared >= 0.0 {
        let length = length_squared.sqrt();
        if length > ROBOTS_EXPLOSION_FRAGMENT_GENERATION_NORMALIZE_EPSILON {
            let inverse = length.recip();
            direction = direction.map(|value| value * inverse);
        }
    }
    Some([
        center_xyz[0] + direction[0] * radius,
        center_xyz[1] + direction[1] * radius,
        center_xyz[2] + direction[2] * radius,
    ])
}

/// `0x004DCECE..0x004DCF02`: fragment flag 0x200 scales all XYZ axes by
/// `0.4 + gameplay_rng_unit * 0.6`. Keep this pure so the host, not the shared
/// definition layer, owns the process-global gameplay RNG stream.
pub fn robots_explosion_fragment_uniform_scale(gameplay_unit_draw: f32) -> Option<f32> {
    gameplay_unit_draw.is_finite().then_some(
        ROBOTS_EXPLOSION_FRAGMENT_RANDOM_SCALE_MIN
            + gameplay_unit_draw * ROBOTS_EXPLOSION_FRAGMENT_RANDOM_SCALE_RANGE,
    )
}

/// `0x004DD1EC..0x004DD2BA -> 0x004DC2E0`: every fragment constructs an embedded
/// common HitQuery with selector 0 and requested budget 0 (native clamps it to 1.0).
/// Only raw flag 0x8 later services the explicit Physics-shape path at 0x00425800.
pub fn robots_explosion_fragment_hit_query_plan(
    fragment: RobotsExplosionFragmentDefinition,
) -> RobotsHitQueryInitPlan {
    let flags = if fragment.flags & ROBOTS_EXPLOSION_FRAGMENT_QUERY_EXCLUDE_GROUP1_SEED_FLAG != 0 {
        ROBOTS_HIT_QUERY_EXCLUDE_RAW_GROUP1_FLAG
    } else {
        0
    };
    RobotsHitQueryInitPlan::direct(0, 0.0, flags)
}

pub fn robots_explosion_fragment_services_hit_query(
    fragment: RobotsExplosionFragmentDefinition,
) -> bool {
    fragment.flags & ROBOTS_EXPLOSION_FRAGMENT_HIT_QUERY_FLAG != 0
}

/// Generic `0x004DBB20` teardown emits the fragment's pickup/resource only for raw
/// bit 0x4 fragments that have not already been consumed through runtime bit 0x40000.
/// The 30-tick `0x004DC0F0` vertical-destroy path has stricter rules and stays separate.
pub fn robots_explosion_fragment_teardown_emits_pickup(handler_flags: u32) -> bool {
    handler_flags & ROBOTS_EXPLOSION_FRAGMENT_SCRIPT_STATE_MARKER_FLAG != 0
        && handler_flags & ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_PICKUP_HANDLED_FLAG == 0
}

/// Allocate Handler+0x48C exactly like `0x004DD603..0x004DD625`.
/// Native increments the low byte at process-global `0x007B3248`, then stores
/// `(new_value & 0xff) % 30`; byte overflow therefore wraps naturally at 255 -> 0.
pub fn robots_explosion_fragment_allocate_cadence_phase(global_counter: &mut u8) -> u8 {
    *global_counter = global_counter.wrapping_add(1);
    (*global_counter as u32 % ROBOTS_EXPLOSION_FRAGMENT_CADENCE_PERIOD) as u8
}

/// Exact `0x004DB6CF..0x004DB6E4` stagger gate. `0x008CB338` is the shared
/// engine frame counter: independent native consumers use it as a frame cache stamp
/// and as a `%60==0` cadence source, so it must stay host-owned rather than Handler-owned.
pub fn robots_explosion_fragment_cadence_due(engine_frame_counter: u32, phase_48c: u8) -> bool {
    engine_frame_counter % ROBOTS_EXPLOSION_FRAGMENT_CADENCE_PERIOD == u32::from(phase_48c)
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsExplosionFragmentVerticalService {
    Continue,
    Teardown {
        pickup_position_xyz: Option<[f32; 3]>,
    },
}

/// Exact position/flag reducer for staggered helper `0x004DC0F0`.
///
/// The host owns the 30-frame cadence. Once called, native tears a fragment down when
/// it has fallen strictly more than 2.0 units below its spawn Y. Raw bits 0x10|0x4
/// emit the pickup at the original spawn position first. Separately, runtime bit
/// 0x200000 paired with raw bit 0x4 emits at current X/Z with spawn Y, then tears down.
pub fn robots_explosion_fragment_vertical_service(
    handler_flags: u32,
    initial_position_xyz: [f32; 3],
    current_position_xyz: [f32; 3],
) -> RobotsExplosionFragmentVerticalService {
    if !initial_position_xyz.iter().all(|value| value.is_finite())
        || !current_position_xyz.iter().all(|value| value.is_finite())
    {
        return RobotsExplosionFragmentVerticalService::Continue;
    }

    let fallen_distance = initial_position_xyz[1] - current_position_xyz[1];
    if fallen_distance > ROBOTS_EXPLOSION_FRAGMENT_MAX_FALL_DISTANCE {
        let emit_pickup = handler_flags
            & (ROBOTS_EXPLOSION_FRAGMENT_DROP_PICKUP_ON_FALL_FLAG
                | ROBOTS_EXPLOSION_FRAGMENT_SCRIPT_STATE_MARKER_FLAG)
            == (ROBOTS_EXPLOSION_FRAGMENT_DROP_PICKUP_ON_FALL_FLAG
                | ROBOTS_EXPLOSION_FRAGMENT_SCRIPT_STATE_MARKER_FLAG);
        return RobotsExplosionFragmentVerticalService::Teardown {
            pickup_position_xyz: emit_pickup.then_some(initial_position_xyz),
        };
    }

    if handler_flags
        & (ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_EARLY_PICKUP_FLAG
            | ROBOTS_EXPLOSION_FRAGMENT_SCRIPT_STATE_MARKER_FLAG)
        == (ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_EARLY_PICKUP_FLAG
            | ROBOTS_EXPLOSION_FRAGMENT_SCRIPT_STATE_MARKER_FLAG)
    {
        return RobotsExplosionFragmentVerticalService::Teardown {
            pickup_position_xyz: Some([
                current_position_xyz[0],
                initial_position_xyz[1],
                current_position_xyz[2],
            ]),
        };
    }

    RobotsExplosionFragmentVerticalService::Continue
}

/// Handler-side `0x004DB805..0x004DB83D` services the explicit sphere query only
/// while the post-Physics velocity magnitude is strictly greater than 1.0.
pub fn robots_explosion_fragment_hit_query_speed_gate(
    physics: RobotsProjectilePhysicsRuntimeState,
) -> bool {
    let velocity_y = physics.linear_velocity_xyz[1] + physics.gravity_velocity_y;
    let speed_squared = physics.linear_velocity_xyz[0] * physics.linear_velocity_xyz[0]
        + velocity_y * velocity_y
        + physics.linear_velocity_xyz[2] * physics.linear_velocity_xyz[2];
    speed_squared.is_finite()
        && speed_squared > ROBOTS_EXPLOSION_FRAGMENT_HIT_QUERY_MIN_SPEED_SQUARED
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsExplosionFragmentContactStep {
    /// True only when the native bit-0x20 rebound branch had a usable contact normal.
    pub rebound_applied: bool,
    /// `0x004DB8B0` keeps the fragment alive through this contact only when the
    /// reflected three-axis speed squared is >= 1.0.
    pub motion_survives_contact: bool,
}

/// Pure Physics transition for `XExplosionFragment::0x004DB8B0`.
///
/// The native handler starts from Projectile `[vx, vy, vz]`, folds 70% of the
/// accumulated gravity lane into Y, damps the effective velocity by Physics+0x138
/// (0.98 from the base ctor), and reflects it around the contact normal. Base Y is
/// then cleared and the reflected Y is stored back in the gravity lane. Negative
/// reflected Y is clamped to zero. If reflected speed squared is below 1.0 the
/// complete motion state is zeroed and the caller proceeds to native teardown.
pub fn robots_explosion_fragment_apply_contact_rebound(
    handler_flags: u32,
    physics: &mut RobotsProjectilePhysicsRuntimeState,
    contact_normal_xyz: [f32; 3],
) -> RobotsExplosionFragmentContactStep {
    if handler_flags & ROBOTS_EXPLOSION_FRAGMENT_REBOUND_FLAG == 0
        || !contact_normal_xyz.iter().all(|value| value.is_finite())
    {
        return RobotsExplosionFragmentContactStep {
            rebound_applied: false,
            motion_survives_contact: false,
        };
    }

    let normal_length_squared = contact_normal_xyz
        .iter()
        .map(|value| value * value)
        .sum::<f32>();
    if !normal_length_squared.is_finite() || normal_length_squared <= f32::EPSILON {
        return RobotsExplosionFragmentContactStep {
            rebound_applied: false,
            motion_survives_contact: false,
        };
    }
    let inverse_normal_length = normal_length_squared.sqrt().recip();
    let normal = contact_normal_xyz.map(|value| value * inverse_normal_length);

    let effective_velocity = [
        physics.linear_velocity_xyz[0] * ROBOTS_EXPLOSION_FRAGMENT_REBOUND_DAMPING,
        (physics.linear_velocity_xyz[1]
            + physics.gravity_velocity_y * ROBOTS_EXPLOSION_FRAGMENT_REBOUND_GRAVITY_MIX)
            * ROBOTS_EXPLOSION_FRAGMENT_REBOUND_DAMPING,
        physics.linear_velocity_xyz[2] * ROBOTS_EXPLOSION_FRAGMENT_REBOUND_DAMPING,
    ];
    let dot = effective_velocity[0] * normal[0]
        + effective_velocity[1] * normal[1]
        + effective_velocity[2] * normal[2];
    let reflected = [
        effective_velocity[0] - 2.0 * dot * normal[0],
        effective_velocity[1] - 2.0 * dot * normal[1],
        effective_velocity[2] - 2.0 * dot * normal[2],
    ];
    let speed_squared = reflected.iter().map(|value| value * value).sum::<f32>();
    let survives = speed_squared.is_finite()
        && speed_squared >= ROBOTS_EXPLOSION_FRAGMENT_REBOUND_MIN_SPEED_SQUARED;

    if survives {
        physics.linear_velocity_xyz = [reflected[0], 0.0, reflected[2]];
        physics.gravity_velocity_y = reflected[1].max(0.0);
    } else {
        physics.linear_velocity_xyz = [0.0; 3];
        physics.gravity_velocity_y = 0.0;
    }

    RobotsExplosionFragmentContactStep {
        rebound_applied: true,
        motion_survives_contact: survives,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsExplosionFragmentVisibilityState {
    /// Native XItem+0x258. Generic XItem construction initializes this to 1.0.
    pub target_258: f32,
    /// Native XItem+0x25C. `0x004DD16D` explicitly resets fragment current visibility to 0.
    pub current_25c: f32,
}

impl Default for RobotsExplosionFragmentVisibilityState {
    fn default() -> Self {
        Self {
            target_258: 1.0,
            current_25c: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsExplosionFragmentVisibilityStep {
    /// False exactly when `0x004DBCC0` would tail into `0x004DBB20` because the
    /// fade-out candidate crossed strictly below zero.
    pub survives: bool,
    /// Mirrors XItem+0x26D bit0 becoming dirty because the newly written visibility
    /// differs from the previous target value.
    pub render_dirty: bool,
}

/// Exact scalar/latch part of `XExplosionFragment::0x004DBCC0`.
///
/// Native calls this after physical-contact service and before the optional fragment
/// HitQuery in the same Handler update. After the first rebound, ordinary fragments
/// (raw bit 0x4 clear) latch runtime bit 0x10000 and fade by 0.06 per service. With
/// runtime bit 0x20000 set, or before fade-out begins, the same scalar ramps upward.
pub fn robots_explosion_fragment_advance_visibility(
    handler_flags: &mut u32,
    physical_contact_responses_completed: u32,
    visibility: &mut RobotsExplosionFragmentVisibilityState,
    runtime_rate_scale: f32,
) -> RobotsExplosionFragmentVisibilityStep {
    if physical_contact_responses_completed > 0
        && *handler_flags & ROBOTS_EXPLOSION_FRAGMENT_SCRIPT_STATE_MARKER_FLAG == 0
    {
        *handler_flags |= ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_FADE_OUT_FLAG;
    }

    if !runtime_rate_scale.is_finite()
        || !visibility.current_25c.is_finite()
        || !visibility.target_258.is_finite()
    {
        return RobotsExplosionFragmentVisibilityStep {
            survives: true,
            render_dirty: false,
        };
    }

    let delta = runtime_rate_scale * ROBOTS_EXPLOSION_FRAGMENT_VISIBILITY_STEP;
    let fading_out = *handler_flags & ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_FADE_OUT_FLAG != 0
        && *handler_flags & ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_FADE_REVERSE_FLAG == 0;
    let candidate = if fading_out {
        visibility.current_25c - delta
    } else {
        visibility.current_25c + delta
    };

    if fading_out && candidate < 0.0 {
        return RobotsExplosionFragmentVisibilityStep {
            survives: false,
            render_dirty: false,
        };
    }

    let next = candidate.clamp(0.0, 1.0);
    let render_dirty = next != visibility.target_258;
    visibility.current_25c = next;
    visibility.target_258 = next;
    RobotsExplosionFragmentVisibilityStep {
        survives: true,
        render_dirty,
    }
}

/// Consume the exact process-global `DAT_007BE1E8` draws for the ordinary shipped
/// fragment branch. The unshipped directional flag 0x100 has a separate native
/// vector-relative algorithm and therefore fails closed without advancing RNG.
pub fn robots_explosion_fragment_ballistic_plan(
    explosion: RobotsExplosionDefinition,
    fragment: RobotsExplosionFragmentDefinition,
    process_lcg_seed: &mut Option<u32>,
) -> Option<RobotsExplosionFragmentBallisticPlan> {
    if fragment.flags & ROBOTS_EXPLOSION_FRAGMENT_DIRECTIONAL_FLAG != 0 {
        return None;
    }
    let scatter_extent = explosion.scatter_extent();
    if !scatter_extent.is_finite() {
        return None;
    }

    // Work transactionally: an unknown seed must not partially mutate the shared
    // process-global stream while the GUI is failing closed.
    let mut seed = *process_lcg_seed;
    let azimuth_raw = robots_process_lcg_next_u31(&mut seed)?;
    let horizontal_raw = robots_process_lcg_next_u31(&mut seed)?;
    let vertical_raw = robots_process_lcg_next_u31(&mut seed)?;

    let azimuth_radians = azimuth_raw as f32 * ROBOTS_EXPLOSION_FRAGMENT_AZIMUTH_U31_SCALE;
    let horizontal_unit = horizontal_raw as f32 * ROBOTS_PROCESS_LCG_UNIT_FLOAT_SCALE;
    let vertical_unit = vertical_raw as f32 * ROBOTS_PROCESS_LCG_UNIT_FLOAT_SCALE;
    let horizontal_distance =
        horizontal_unit * scatter_extent + ROBOTS_EXPLOSION_FRAGMENT_MIN_HORIZONTAL_DISTANCE;
    let vertical_range = (scatter_extent + ROBOTS_EXPLOSION_FRAGMENT_VERTICAL_RANGE_ADD)
        - ROBOTS_EXPLOSION_FRAGMENT_MIN_VERTICAL_BIAS;
    let vertical_bias =
        vertical_unit * vertical_range + ROBOTS_EXPLOSION_FRAGMENT_MIN_VERTICAL_BIAS;

    let random_scalar = if fragment.flags & ROBOTS_EXPLOSION_FRAGMENT_RANDOM_SCALAR_FLAG != 0 {
        let _ = robots_process_lcg_next_u31(&mut seed)?;
        let scalar_raw = robots_process_lcg_next_u31(&mut seed)?;
        Some(scalar_raw as f32 * ROBOTS_EXPLOSION_FRAGMENT_RANDOM_SCALAR_U31_SCALE)
    } else {
        None
    };

    *process_lcg_seed = seed;
    Some(RobotsExplosionFragmentBallisticPlan {
        azimuth_radians,
        horizontal_distance,
        vertical_bias,
        gravity_acceleration: ROBOTS_PROJECTILE_GRAVITY,
        random_scalar,
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RobotsExplosionDatabase {
    pub definitions: Vec<RobotsExplosionDefinition>,
    pub fragments: Vec<RobotsExplosionFragmentDefinition>,
    pub collision_profiles: Vec<RobotsExplosionCollisionProfile>,
}

impl RobotsExplosionDatabase {
    pub fn definition(&self, selector: u32) -> Option<&RobotsExplosionDefinition> {
        self.definitions.iter().find(|row| row.selector == selector)
    }

    pub fn fragment(&self, selector: u32) -> Option<&RobotsExplosionFragmentDefinition> {
        self.fragments.iter().find(|row| row.selector == selector)
    }

    pub fn collision_profile(&self, selector: u32) -> Option<&RobotsExplosionCollisionProfile> {
        self.collision_profiles
            .iter()
            .find(|profile| profile.explosion_selector == selector)
    }
}

fn collision_vector_keys(values: &[(f32, [f32; 3])]) -> Vec<RobotsExplosionCollisionVectorKey> {
    values
        .iter()
        .copied()
        .map(|(frame, xyz)| RobotsExplosionCollisionVectorKey::from_native(frame, xyz))
        .collect()
}

fn build_direct_collision_profile(
    definition: RobotsExplosionDefinition,
    script: &UXGeoScript,
) -> Option<RobotsExplosionCollisionProfile> {
    let command = script
        .commands
        .iter()
        .find(|command| command.opcode == 13)?;
    let UXGeoScriptCommandData::Unknown { data, .. } = &command.data else {
        return None;
    };
    let RobotsScriptPayloadDiagnostic::Collision {
        datum_shape_mode,
        serialized_shape_scalar_0,
        serialized_shape_scalar_1,
        serialized_shape_scalar_2,
        ..
    } = robots_script_payload_diagnostic(13, data)?
    else {
        return None;
    };
    let controller = script
        .controllers
        .get(command.controller_header_index as usize);
    Some(RobotsExplosionCollisionProfile {
        explosion_selector: definition.selector,
        script_uid: script.hashcode,
        command_start: command.start,
        command_length: command.length,
        datum_shape_mode,
        serialized_shape_scalar_bits: [
            serialized_shape_scalar_0.to_bits(),
            serialized_shape_scalar_1.to_bits(),
            serialized_shape_scalar_2.to_bits(),
        ],
        position_keys: controller
            .map(|controller| collision_vector_keys(&controller.channels.vector_0))
            .unwrap_or_default(),
        scale_keys: controller
            .map(|controller| collision_vector_keys(&controller.channels.vector_1))
            .unwrap_or_default(),
    })
}

/// Load FX03_Explosion (0x01000038), spreadsheet 0x1400000A.
/// Sheet 0 is the 66-row 0x44 explosion table consumed through DAT_007B3240/44;
/// sheet 1 is the 161-row 0x20 fragment table consumed through DAT_007B3230/34.
pub fn read_robots_explosion_database(
    edb: &mut EdbFile,
) -> anyhow::Result<RobotsExplosionDatabase> {
    anyhow::ensure!(
        edb.header.hashcode == ROBOTS_EXPLOSION_DATABASE_FILE_UID,
        "expected Robots explosion EDB 0x{ROBOTS_EXPLOSION_DATABASE_FILE_UID:08X}, got 0x{:08X}",
        edb.header.hashcode
    );
    let spreadsheets = UXGeoSpreadsheet::read_all(edb)?;
    let sheets = spreadsheets
        .iter()
        .find_map(|(uid, spreadsheet)| {
            (*uid == ROBOTS_EXPLOSION_SPREADSHEET_UID)
                .then_some(spreadsheet)
                .and_then(|spreadsheet| match spreadsheet {
                    UXGeoSpreadsheet::Data(sheets) => Some(sheets.as_slice()),
                    UXGeoSpreadsheet::Text(_) => None,
                })
        })
        .context("missing Robots explosion spreadsheet 0x1400000A")?;
    let definition_sheet = sheets
        .first()
        .context("missing explosion definition sheet 0")?;
    let fragment_sheet = sheets
        .get(1)
        .context("missing explosion fragment sheet 1")?;

    edb.seek(SeekFrom::Start(definition_sheet.address as u64))?;
    let mut definitions = Vec::with_capacity(definition_sheet.row_count as usize);
    for _ in 0..definition_sheet.row_count {
        let mut words = [0u32; ROBOTS_EXPLOSION_ROW_WORDS];
        for word in &mut words {
            *word = edb.read_type(edb.endian)?;
        }
        definitions.push(RobotsExplosionDefinition::from_native_words(words));
    }

    edb.seek(SeekFrom::Start(fragment_sheet.address as u64))?;
    let mut fragments = Vec::with_capacity(fragment_sheet.row_count as usize);
    for _ in 0..fragment_sheet.row_count {
        let mut words = [0u32; ROBOTS_EXPLOSION_FRAGMENT_ROW_WORDS];
        for word in &mut words {
            *word = edb.read_type(edb.endian)?;
        }
        fragments.push(RobotsExplosionFragmentDefinition::from_native_words(words));
    }

    let scripts = UXGeoScript::read_all(edb)?;
    let mut collision_profiles = Vec::new();
    for definition in definitions.iter().copied() {
        if let Some(script) = scripts
            .iter()
            .find(|script| script.hashcode == definition.main_script_uid)
        {
            if let Some(profile) = build_direct_collision_profile(definition, script) {
                collision_profiles.push(profile);
            }
        }
    }

    Ok(RobotsExplosionDatabase {
        definitions,
        fragments,
        collision_profiles,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsExplosionFragmentAttempt {
    pub ordinal: u8,
    pub fragment_uid: u32,
}

/// Class-independent state owned by `XItemHandler_Explosion`.
///
/// Native fields:
/// - `+0x4B8`: next fragment ordinal
/// - `+0x4BC`: periodic service counter
/// - `+0x4E8`: continue-next-service latch
///
/// `0x004DC6A0` performs one fragment attempt immediately after init. The vslot
/// service at `0x004DCBA0` then attempts normally on `counter % 3 == 2`, while a
/// failed in-range attempt arms the continue latch so the next service consumes the
/// next slot immediately. `0x004DCC6C` advances +0x4B8 before lookup, so the failed
/// slot itself is never retried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsExplosionRuntimeState {
    pub next_fragment_ordinal: u8,
    pub service_counter: u32,
    pub continue_next_service: bool,
    pub definition_attached: bool,
    pub source_attached: bool,
}

impl Default for RobotsExplosionRuntimeState {
    fn default() -> Self {
        Self {
            next_fragment_ordinal: 0,
            service_counter: 0,
            continue_next_service: false,
            definition_attached: true,
            source_attached: true,
        }
    }
}

impl RobotsExplosionRuntimeState {
    /// Native `0x004DCC20`: consume one of the ten definition fragment slots.
    /// Reaching ordinal 10 detaches definition/source and produces no attempt.
    pub fn take_fragment_attempt(
        &mut self,
        definition: RobotsExplosionDefinition,
    ) -> Option<RobotsExplosionFragmentAttempt> {
        if !self.definition_attached || !self.source_attached {
            return None;
        }
        let ordinal = usize::from(self.next_fragment_ordinal);
        if ordinal >= ROBOTS_EXPLOSION_FRAGMENT_COUNT {
            self.definition_attached = false;
            self.source_attached = false;
            return None;
        }
        self.next_fragment_ordinal = self.next_fragment_ordinal.saturating_add(1);
        Some(RobotsExplosionFragmentAttempt {
            ordinal: ordinal as u8,
            fragment_uid: definition.fragment_uids[ordinal],
        })
    }

    /// Caller reports whether the native fragment XItem factory actually succeeded.
    /// A failed in-range attempt arms Handler+0x4E8; the already-advanced cursor means
    /// the following service continues with the next slot rather than retrying this one.
    pub fn record_fragment_factory_result(&mut self, succeeded: bool) {
        self.continue_next_service = !succeeded;
    }

    /// `0x004DCBA0` cadence gate. Returns true when this fixed service invocation
    /// must call `0x004DCC20`.
    pub fn advance_service(&mut self) -> bool {
        self.service_counter = self.service_counter.wrapping_add(1);
        if self.continue_next_service {
            self.continue_next_service = false;
            return true;
        }
        self.service_counter % 3 == 2
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::Path};

    use eurochef_edb::versions::Platform;

    use super::*;

    fn definition() -> RobotsExplosionDefinition {
        RobotsExplosionDefinition::from_native_words([
            0x5200_003A,
            0x0100_0001,
            0x0200_0002,
            0xA0,
            0xA1,
            0xA2,
            0xA3,
            0xA4,
            0xA5,
            0xA6,
            0xA7,
            0xA8,
            0xA9,
            0x34,
            0x38,
            0x3C,
            0x40,
        ])
    }

    #[test]
    fn native_row_is_exactly_0x44_and_preserves_unknown_tail() {
        assert_eq!(ROBOTS_EXPLOSION_ROW_WORDS, 17);
        let words = std::array::from_fn(|index| index as u32);
        assert_eq!(
            RobotsExplosionDefinition::from_native_words(words).raw_words(),
            words
        );
    }

    #[test]
    fn fragment_sequence_consumes_exactly_ten_slots_then_detaches() {
        let definition = definition();
        let mut state = RobotsExplosionRuntimeState::default();
        for ordinal in 0..ROBOTS_EXPLOSION_FRAGMENT_COUNT {
            let attempt = state.take_fragment_attempt(definition).unwrap();
            assert_eq!(usize::from(attempt.ordinal), ordinal);
            assert_eq!(attempt.fragment_uid, 0xA0 + ordinal as u32);
            state.record_fragment_factory_result(true);
        }
        assert!(state.definition_attached);
        assert!(state.source_attached);
        assert_eq!(state.take_fragment_attempt(definition), None);
        assert!(!state.definition_attached);
        assert!(!state.source_attached);
    }

    #[test]
    fn periodic_service_matches_modulo_three_and_immediate_continue_lane() {
        let mut state = RobotsExplosionRuntimeState::default();
        assert!(!state.advance_service());
        assert!(state.advance_service());
        state.record_fragment_factory_result(false);
        assert!(state.advance_service());
        state.record_fragment_factory_result(true);
        assert!(!state.advance_service());
        assert!(state.advance_service());
    }

    #[test]
    fn fragment_generation_uses_native_positive_octant_draw_order() {
        assert_eq!(
            robots_explosion_fragment_generation_position([10.0, 20.0, 30.0], 2.0, [0.0, 1.0, 0.0],),
            Some([12.0, 20.0, 30.0])
        );
        assert_eq!(
            robots_explosion_fragment_generation_position([10.0, 20.0, 30.0], 2.0, [1.0, 0.0, 0.0],),
            Some([10.0, 22.0, 30.0])
        );
        assert_eq!(
            robots_explosion_fragment_generation_position([10.0, 20.0, 30.0], 2.0, [0.0, 0.0, 0.0],),
            Some([10.0, 20.0, 30.0])
        );
    }

    #[test]
    fn fragment_hit_query_seed_flag_and_speed_gate_match_native_paths() {
        let mut words = [u32::MAX; ROBOTS_EXPLOSION_FRAGMENT_ROW_WORDS];
        words[0] = 0x5300_0001;
        words[7] = ROBOTS_EXPLOSION_FRAGMENT_HIT_QUERY_FLAG
            | ROBOTS_EXPLOSION_FRAGMENT_QUERY_EXCLUDE_GROUP1_SEED_FLAG;
        let fragment = RobotsExplosionFragmentDefinition::from_native_words(words);
        let plan = robots_explosion_fragment_hit_query_plan(fragment);
        assert_eq!(plan.selector, 0);
        assert_eq!(plan.remaining_budget, 1.0);
        assert_eq!(plan.initial_flags, ROBOTS_HIT_QUERY_EXCLUDE_RAW_GROUP1_FLAG);
        assert!(robots_explosion_fragment_services_hit_query(fragment));

        let physics = |linear_velocity_xyz: [f32; 3], gravity_velocity_y: f32| {
            RobotsProjectilePhysicsRuntimeState {
                position_xyz: [0.0; 3],
                linear_velocity_xyz,
                gravity_velocity_y,
                gravity_acceleration: 0.0,
                updates_completed: 0,
            }
        };
        assert!(!robots_explosion_fragment_hit_query_speed_gate(physics(
            [1.0, 0.0, 0.0],
            0.0,
        )));
        assert!(robots_explosion_fragment_hit_query_speed_gate(physics(
            [1.01, 0.0, 0.0],
            0.0,
        )));
        assert!(robots_explosion_fragment_hit_query_speed_gate(physics(
            [0.0, 0.25, 0.0],
            0.8,
        )));
    }

    #[test]
    fn fragment_contact_rebound_matches_native_damping_reflection_and_terminal_threshold() {
        let mut words = [u32::MAX; ROBOTS_EXPLOSION_FRAGMENT_ROW_WORDS];
        words[7] = ROBOTS_EXPLOSION_FRAGMENT_REBOUND_FLAG;
        let fragment = RobotsExplosionFragmentDefinition::from_native_words(words);

        let mut physics = RobotsProjectilePhysicsRuntimeState {
            position_xyz: [0.0; 3],
            linear_velocity_xyz: [2.0, 0.0, 0.0],
            gravity_velocity_y: 0.0,
            gravity_acceleration: 9.8,
            updates_completed: 0,
        };
        let step = robots_explosion_fragment_apply_contact_rebound(
            fragment.flags,
            &mut physics,
            [1.0, 0.0, 0.0],
        );
        assert!(step.rebound_applied);
        assert!(step.motion_survives_contact);
        assert!((physics.linear_velocity_xyz[0] + 1.96).abs() < 1.0e-6);
        assert_eq!(physics.linear_velocity_xyz[1], 0.0);
        assert_eq!(physics.linear_velocity_xyz[2], 0.0);
        assert_eq!(physics.gravity_velocity_y, 0.0);

        let mut falling = RobotsProjectilePhysicsRuntimeState {
            position_xyz: [0.0; 3],
            linear_velocity_xyz: [0.0, 0.0, 0.0],
            gravity_velocity_y: -4.0,
            gravity_acceleration: 9.8,
            updates_completed: 0,
        };
        let step = robots_explosion_fragment_apply_contact_rebound(
            fragment.flags,
            &mut falling,
            [0.0, 1.0, 0.0],
        );
        assert!(step.rebound_applied);
        assert!(step.motion_survives_contact);
        assert!(falling.gravity_velocity_y > 0.0);
        assert_eq!(falling.linear_velocity_xyz[1], 0.0);

        let mut slow = RobotsProjectilePhysicsRuntimeState {
            position_xyz: [0.0; 3],
            linear_velocity_xyz: [0.5, 0.0, 0.0],
            gravity_velocity_y: 0.0,
            gravity_acceleration: 9.8,
            updates_completed: 0,
        };
        let step = robots_explosion_fragment_apply_contact_rebound(
            fragment.flags,
            &mut slow,
            [1.0, 0.0, 0.0],
        );
        assert!(step.rebound_applied);
        assert!(!step.motion_survives_contact);
        assert_eq!(slow.linear_velocity_xyz, [0.0; 3]);
        assert_eq!(slow.gravity_velocity_y, 0.0);
    }

    #[test]
    fn fragment_without_rebound_flag_fails_closed_without_mutating_physics() {
        let fragment = RobotsExplosionFragmentDefinition::from_native_words(
            [0; ROBOTS_EXPLOSION_FRAGMENT_ROW_WORDS],
        );
        let mut physics = RobotsProjectilePhysicsRuntimeState {
            position_xyz: [1.0, 2.0, 3.0],
            linear_velocity_xyz: [4.0, 5.0, 6.0],
            gravity_velocity_y: 7.0,
            gravity_acceleration: 9.8,
            updates_completed: 8,
        };
        let before = physics;
        let step = robots_explosion_fragment_apply_contact_rebound(
            fragment.flags,
            &mut physics,
            [0.0, 1.0, 0.0],
        );
        assert!(!step.rebound_applied);
        assert!(!step.motion_survives_contact);
        assert_eq!(physics, before);
    }

    #[test]
    fn fragment_cadence_phase_uses_process_byte_counter_and_engine_frame_remainder() {
        let mut global_counter = 0u8;
        let first = robots_explosion_fragment_allocate_cadence_phase(&mut global_counter);
        let second = robots_explosion_fragment_allocate_cadence_phase(&mut global_counter);
        assert_eq!(first, 1);
        assert_eq!(second, 2);
        assert!(robots_explosion_fragment_cadence_due(31, first));
        assert!(!robots_explosion_fragment_cadence_due(32, first));
        assert!(robots_explosion_fragment_cadence_due(32, second));

        global_counter = u8::MAX;
        let wrapped = robots_explosion_fragment_allocate_cadence_phase(&mut global_counter);
        assert_eq!(global_counter, 0);
        assert_eq!(wrapped, 0);
        assert!(robots_explosion_fragment_cadence_due(60, wrapped));
    }

    #[test]
    fn fragment_vertical_service_matches_native_fall_and_early_pickup_positions() {
        let initial = [1.0, 5.0, 3.0];
        let current = [9.0, 2.9, 7.0];
        assert_eq!(
            robots_explosion_fragment_vertical_service(0, initial, current),
            RobotsExplosionFragmentVerticalService::Teardown {
                pickup_position_xyz: None,
            }
        );

        let drop_flags = ROBOTS_EXPLOSION_FRAGMENT_DROP_PICKUP_ON_FALL_FLAG
            | ROBOTS_EXPLOSION_FRAGMENT_SCRIPT_STATE_MARKER_FLAG;
        assert_eq!(
            robots_explosion_fragment_vertical_service(drop_flags, initial, current),
            RobotsExplosionFragmentVerticalService::Teardown {
                pickup_position_xyz: Some(initial),
            }
        );

        let boundary = [9.0, 3.0, 7.0];
        assert_eq!(
            robots_explosion_fragment_vertical_service(drop_flags, initial, boundary),
            RobotsExplosionFragmentVerticalService::Continue
        );

        let early_flags = ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_EARLY_PICKUP_FLAG
            | ROBOTS_EXPLOSION_FRAGMENT_SCRIPT_STATE_MARKER_FLAG;
        let near = [9.0, 4.5, 7.0];
        assert_eq!(
            robots_explosion_fragment_vertical_service(early_flags, initial, near),
            RobotsExplosionFragmentVerticalService::Teardown {
                pickup_position_xyz: Some([9.0, 5.0, 7.0]),
            }
        );
    }

    #[test]
    fn fragment_visibility_starts_at_zero_and_fades_in_by_native_point_zero_six() {
        let mut flags = 0u32;
        let mut visibility = RobotsExplosionFragmentVisibilityState::default();
        let step =
            robots_explosion_fragment_advance_visibility(&mut flags, 0, &mut visibility, 1.0);
        assert!(step.survives);
        assert!(step.render_dirty);
        assert_eq!(visibility.current_25c.to_bits(), 0.06_f32.to_bits());
        assert_eq!(visibility.target_258, visibility.current_25c);
        assert_eq!(flags & ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_FADE_OUT_FLAG, 0);
    }

    #[test]
    fn fragment_first_rebound_latches_fade_out_and_tears_down_only_below_zero() {
        let mut flags = ROBOTS_EXPLOSION_FRAGMENT_REBOUND_FLAG;
        let mut visibility = RobotsExplosionFragmentVisibilityState {
            target_258: 0.12,
            current_25c: 0.12,
        };
        let step =
            robots_explosion_fragment_advance_visibility(&mut flags, 1, &mut visibility, 1.0);
        assert!(step.survives);
        assert_eq!(
            flags & ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_FADE_OUT_FLAG,
            ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_FADE_OUT_FLAG
        );
        assert!((visibility.current_25c - 0.06).abs() < 1.0e-7);

        visibility.target_258 = 0.0;
        visibility.current_25c = 0.0;
        let before = visibility;
        let step =
            robots_explosion_fragment_advance_visibility(&mut flags, 1, &mut visibility, 1.0);
        assert!(!step.survives);
        assert_eq!(visibility, before);
    }

    #[test]
    fn fragment_raw_bit4_blocks_contact_fade_and_runtime_reverse_routes_upward() {
        let mut pickup_flags = ROBOTS_EXPLOSION_FRAGMENT_SCRIPT_STATE_MARKER_FLAG;
        let mut pickup_visibility = RobotsExplosionFragmentVisibilityState::default();
        let step = robots_explosion_fragment_advance_visibility(
            &mut pickup_flags,
            4,
            &mut pickup_visibility,
            1.0,
        );
        assert!(step.survives);
        assert_eq!(
            pickup_flags & ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_FADE_OUT_FLAG,
            0
        );
        assert_eq!(pickup_visibility.current_25c.to_bits(), 0.06_f32.to_bits());

        let mut reverse_flags = ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_FADE_OUT_FLAG
            | ROBOTS_EXPLOSION_FRAGMENT_RUNTIME_FADE_REVERSE_FLAG;
        let mut reverse_visibility = RobotsExplosionFragmentVisibilityState {
            target_258: 0.5,
            current_25c: 0.5,
        };
        robots_explosion_fragment_advance_visibility(
            &mut reverse_flags,
            1,
            &mut reverse_visibility,
            1.0,
        );
        assert!((reverse_visibility.current_25c - 0.56).abs() < 1.0e-6);
    }

    #[test]
    fn ordinary_fragment_ballistics_consume_three_or_five_shared_process_draws() {
        let mut explosion_words = definition().raw_words();
        explosion_words[13] = 2.6_f32.to_bits();
        let explosion = RobotsExplosionDefinition::from_native_words(explosion_words);
        let mut fragment_words = [u32::MAX; ROBOTS_EXPLOSION_FRAGMENT_ROW_WORDS];
        fragment_words[0] = 0x5300_0037;
        fragment_words[7] = 0;

        let mut three_draw_seed = Some(1);
        let plan = robots_explosion_fragment_ballistic_plan(
            explosion,
            RobotsExplosionFragmentDefinition::from_native_words(fragment_words),
            &mut three_draw_seed,
        )
        .unwrap();
        assert_eq!(three_draw_seed, Some(0x8116_017E));
        assert_eq!(plan.gravity_acceleration, ROBOTS_PROJECTILE_GRAVITY);
        assert!(plan.horizontal_distance >= ROBOTS_EXPLOSION_FRAGMENT_MIN_HORIZONTAL_DISTANCE);
        assert!(plan.vertical_bias >= ROBOTS_EXPLOSION_FRAGMENT_MIN_VERTICAL_BIAS);
        assert_eq!(plan.random_scalar, None);
        assert!(plan.projectile_physics([1.0, 2.0, 3.0]).is_some());

        fragment_words[7] = ROBOTS_EXPLOSION_FRAGMENT_RANDOM_SCALAR_FLAG;
        let mut five_draw_seed = Some(1);
        let plan = robots_explosion_fragment_ballistic_plan(
            explosion,
            RobotsExplosionFragmentDefinition::from_native_words(fragment_words),
            &mut five_draw_seed,
        )
        .unwrap();
        assert_eq!(five_draw_seed, Some(0x0CF0_6D60));
        assert_eq!(
            plan.random_scalar,
            Some((0x0CF0_6D60_u32 >> 1) as f32 * ROBOTS_EXPLOSION_FRAGMENT_RANDOM_SCALAR_U31_SCALE)
        );
    }

    #[test]
    fn fragment_uniform_scale_matches_native_04_plus_u_times_06() {
        assert_eq!(robots_explosion_fragment_uniform_scale(0.0), Some(0.4));
        assert!((robots_explosion_fragment_uniform_scale(0.5).unwrap() - 0.7).abs() < 1.0e-6);
        assert!((robots_explosion_fragment_uniform_scale(1.0).unwrap() - 1.0).abs() < 1.0e-6);
        assert_eq!(robots_explosion_fragment_uniform_scale(f32::NAN), None);
    }

    #[test]
    fn unsupported_directional_fragment_fails_closed_without_consuming_process_rng() {
        let mut fragment_words = [u32::MAX; ROBOTS_EXPLOSION_FRAGMENT_ROW_WORDS];
        fragment_words[7] = ROBOTS_EXPLOSION_FRAGMENT_DIRECTIONAL_FLAG;
        let mut seed = Some(0x1234_5678);
        assert!(robots_explosion_fragment_ballistic_plan(
            definition(),
            RobotsExplosionFragmentDefinition::from_native_words(fragment_words),
            &mut seed,
        )
        .is_none());
        assert_eq!(seed, Some(0x1234_5678));
    }

    #[test]
    fn real_fx03_database_has_native_definition_and_fragment_counts_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/fx03_exp.edb");
        let file = File::open(&path).expect("open fx03_exp.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse fx03_exp.edb");
        let database = read_robots_explosion_database(&mut edb).expect("decode explosion database");
        assert_eq!(database.definitions.len(), 66);
        assert_eq!(database.fragments.len(), 161);
        assert_eq!(database.collision_profiles.len(), 12);
        assert!(database
            .collision_profiles
            .iter()
            .all(|profile| profile.datum_shape_mode == 1));
        let collision_selectors = database
            .collision_profiles
            .iter()
            .map(|profile| profile.explosion_selector)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            collision_selectors,
            std::collections::BTreeSet::from([
                0x5200_0002,
                0x5200_0004,
                0x5200_0005,
                0x5200_000D,
                0x5200_0022,
                0x5200_0024,
                0x5200_0025,
                0x5200_002C,
                0x5200_002D,
                0x5200_0033,
                0x5200_003A,
                0x5200_003C,
            ])
        );
        let offset_profile = database
            .collision_profile(0x5200_0002)
            .expect("offset Collision profile");
        assert!(offset_profile
            .position_keys
            .iter()
            .any(|key| key.xyz()[0] != 0.0));

        let mut hit_flag_counts = std::collections::BTreeMap::<u32, usize>::new();
        for definition in database
            .definitions
            .iter()
            .filter(|definition| definition.hit_query_selector() != u32::MAX)
        {
            *hit_flag_counts
                .entry(definition.hit_query_initial_flags())
                .or_default() += 1;
        }
        assert_eq!(
            hit_flag_counts,
            std::collections::BTreeMap::from([
                (0x0000_0000, 43),
                (0x0000_0008, 1),
                (0x0000_0020, 7),
                (0x0002_0000, 3),
                (0x0004_0020, 1),
            ])
        );

        let mut flag_counts = std::collections::BTreeMap::<u32, usize>::new();
        for fragment in &database.fragments {
            *flag_counts.entry(fragment.flags).or_default() += 1;
        }
        assert_eq!(
            flag_counts,
            std::collections::BTreeMap::from([
                (0x0000_0000, 2),
                (0x0000_0001, 21),
                (0x0000_0021, 18),
                (0x0000_0023, 116),
                (0x0000_0064, 3),
                (0x0000_0609, 1),
            ])
        );

        let ef01 = database
            .definition(0x5200_003A)
            .expect("EF01 common explosion");
        assert_eq!(ef01.main_script_uid, 0x0400_01C5);
        assert_eq!(ef01.main_file_uid, 0x0100_0038);
        assert_eq!(
            ef01.fragment_uids[..5],
            [
                0x5300_007C,
                0x5300_007D,
                0x5300_007D,
                0x5300_0001,
                0x5300_0001
            ]
        );
        assert_eq!(ef01.scatter_extent_bits_34, 0x4026_6666);
        assert_eq!(ef01.scatter_extent().to_bits(), 0x4026_6666);
        assert_eq!(ef01.hit_query_initial_flags_38, 0);
        assert_eq!(ef01.word_3c, 0);
        assert_eq!(ef01.hit_query_selector_40, 0x1000_0001);

        let ef01_collision = database
            .collision_profile(0x5200_003A)
            .expect("EF01 main Collision profile");
        assert_eq!(ef01_collision.script_uid, 0x0400_01C5);
        assert_eq!(ef01_collision.command_start, 0);
        assert_eq!(ef01_collision.command_length, 52);
        assert_eq!(ef01_collision.datum_shape_mode, 1);
        let frame0 = ef01_collision
            .sample_at_frame(0.0)
            .expect("frame0 Collision");
        assert!((frame0.local_center_xyz[1] - -0.015791323).abs() < 1.0e-7);
        assert!((frame0.shape_scalar_xyz[0] - 0.56079364).abs() < 1.0e-7);
        let frame1 = ef01_collision
            .sample_at_frame(1.0)
            .expect("frame1 Collision");
        let expected_frame1 = 0.56079364 + (2.2134306 - 0.56079364) / 3.0;
        assert!((frame1.shape_scalar_xyz[0] - expected_frame1).abs() < 1.0e-6);
        let mut collision_runtime = RobotsExplosionCollisionRuntimeState::default();
        assert_eq!(collision_runtime.handler_sample(ef01_collision), None);
        collision_runtime.advance_animator(ef01_collision);
        assert_eq!(
            collision_runtime.handler_sample(ef01_collision),
            Some(frame0)
        );

        let eq04 = database
            .definition(0x5200_0006)
            .expect("EQ04 common explosion");
        assert_eq!(eq04.main_script_uid, 0x0400_0087);
        assert_eq!(eq04.main_file_uid, 0x0100_0038);
        assert!(database.collision_profile(0x5200_0006).is_none());

        let scrap = database.fragment(0x5300_0001).expect("scrap fragment");
        assert_eq!(scrap.script_uid, 0x0400_0024);
        assert_eq!(scrap.file_uid, 0x0100_0003);
        assert_eq!(scrap.reserved_0c, u32::MAX);
        assert_eq!(scrap.reserved_10, u32::MAX);
        assert_eq!(scrap.pickup_uid, 0x4700_0001);
        assert_eq!(scrap.pickup_quantity(), 1);
        assert_eq!(scrap.flags, 0x64);
        assert!(!scrap.uses_attachment_setup());
        assert!(scrap.uses_script_state_markers());

        let common_fragment = database
            .fragment(0x5300_0037)
            .expect("common monster fragment");
        assert_eq!(common_fragment.flags, 0x23);
        assert!(common_fragment.uses_attachment_setup());
        assert!(!common_fragment.uses_script_state_markers());

        assert!(database
            .fragments
            .iter()
            .all(|row| row.reserved_0c == u32::MAX));
        assert!(database
            .fragments
            .iter()
            .all(|row| row.reserved_10 == u32::MAX));
        assert_eq!(
            database
                .fragments
                .iter()
                .filter(|row| row.pickup_uid != u32::MAX)
                .count(),
            3
        );
        assert!(database
            .fragments
            .iter()
            .all(|row| row.flags & ROBOTS_EXPLOSION_FRAGMENT_DIRECTIONAL_FLAG == 0));
        assert_eq!(
            database
                .fragments
                .iter()
                .filter(|row| row.uses_random_uniform_scale())
                .count(),
            1
        );
        assert_eq!(
            database
                .fragments
                .iter()
                .filter(|row| row.shipped_pickup_variant_consumes_rng())
                .count(),
            3
        );
        assert!(database.fragment(0x5300_0037).is_some());
    }
}
