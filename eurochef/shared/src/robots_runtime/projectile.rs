use std::io::{Seek, SeekFrom};

use anyhow::Context;
use eurochef_edb::{binrw::BinReaderExt, edb::EdbFile};
use serde::Serialize;

use super::events::RobotsScriptEventView;
use crate::spreadsheets::UXGeoSpreadsheet;

pub const ROBOTS_MISSILE_DATABASE_FILE_UID: u32 = 0x0100_00AA;
pub const ROBOTS_MISSILE_SPREADSHEET_UID: u32 = 0x1400_000F;
pub const ROBOTS_MISSILE_ROW_SIZE: usize = 0x38;
pub const ROBOTS_MISSILE_ROW_WORDS: usize = ROBOTS_MISSILE_ROW_SIZE / 4;
pub const ROBOTS_PROJECTILE_FIXED_STEP_SECONDS: f32 = 1.0 / 60.0;
pub const ROBOTS_PROJECTILE_GRAVITY: f32 = 9.8;
pub const ROBOTS_PROJECTILE_NATIVE_EPSILON: f32 = 0.001;
pub const ROBOTS_PROJECTILE_NORMALIZE_EPSILON: f32 = 0.000_01;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsCreateProjectileRequest {
    /// Raw typed-argument ordinal serialized by AnimScript Event. Native script
    /// loading relocates this to the HT_Missile row key before `0x004DFEB0`.
    pub missile_index: u32,
    pub launch_datum: u32,
    pub source_override: u32,
    pub launch_scalar_bits: Option<u32>,
}

impl RobotsCreateProjectileRequest {
    /// Native AI CreateProjectile event arguments consumed by the common
    /// producer at `0x0044F1CC -> 0x004DFEB0`.
    pub fn from_event(event: RobotsScriptEventView<'_>) -> Option<Self> {
        Some(Self {
            missile_index: event.native_arg_word(0)?,
            launch_datum: event.native_arg_word(1)?,
            source_override: event.native_arg_word(2)?,
            launch_scalar_bits: event.native_arg_word(3),
        })
    }

    pub fn launch_scalar(self) -> Option<f32> {
        self.launch_scalar_bits.map(f32::from_bits)
    }
}

/// Exact fixed-size row consumed by native common projectile producer
/// `0x004DFEB0`. Only fields with a proven downstream consumer receive named
/// accessors; the remaining words deliberately stay raw until their native role
/// is needed by the runtime host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsProjectileHandlerKind {
    Projectile,
    Mine,
    Homing,
}

impl RobotsProjectileHandlerKind {
    /// `XItemHandler_Mine` replaces the inherited physical-contact alias at
    /// vslot +0xCC with a no-op. Its accepted hit-query and lifetime terminal
    /// paths remain inherited from XItemHandler_Projectile.
    pub const fn suppresses_physical_contact_terminal_path(self) -> bool {
        matches!(self, Self::Mine)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsMissileDefinition {
    pub selector: u32,
    pub words_04_to_34: [u32; ROBOTS_MISSILE_ROW_WORDS - 1],
}

impl RobotsMissileDefinition {
    pub const fn from_native_words(words: [u32; ROBOTS_MISSILE_ROW_WORDS]) -> Self {
        Self {
            selector: words[0],
            words_04_to_34: [
                words[1], words[2], words[3], words[4], words[5], words[6], words[7], words[8],
                words[9], words[10], words[11], words[12], words[13],
            ],
        }
    }

    /// `0x004DFEB0 +0x4/+0x8` are passed as the two resource identifiers to
    /// `0x00414D90` when choosing the concrete projectile factory.
    pub const fn factory_resource_pair(self) -> [u32; 2] {
        [self.words_04_to_34[0], self.words_04_to_34[1]]
    }

    /// D04 row +0x0C is copied by `0x00414EF0` into Handler+0x388. The inherited
    /// `+0xD8=0x004152A0` terminal generator consumes that slot; GenericMine maps
    /// it to `HT_Script_MissileExplosionTest (0x0400004C)`.
    pub const fn primary_terminal_resource(self) -> u32 {
        self.words_04_to_34[2]
    }

    /// Native row +0x14 is the default scalar passed to projectile construction;
    /// the final CreateProjectile Event argument can override it when positive.
    pub fn default_launch_scalar(self) -> f32 {
        f32::from_bits(self.words_04_to_34[4])
    }

    /// Handler+0x390 lifetime seed passed by `0x004DFEB0 -> 0x00414EF0` from
    /// row +0x18. Base projectile Handler update subtracts one native fixed step
    /// and enters its terminal +0xD4 path once this value becomes negative.
    pub fn lifetime_seconds(self) -> f32 {
        f32::from_bits(self.words_04_to_34[5])
    }

    /// D04 row +0x1C is passed to `XItemPhysics_Projectile` initializer
    /// `0x0041A2D0`, which seeds the embedded source shape at Physics+0x1D8.
    /// For ordinary projectile hit queries this is the local sphere radius.
    pub fn hit_query_radius(self) -> f32 {
        f32::from_bits(self.words_04_to_34[6])
    }

    /// Common producer sets XItemPhysics_Projectile+0x128 to 9.8 only when
    /// row +0x20 bit0 is present; otherwise the gravity contribution is zero.
    pub fn gravity_acceleration(self) -> f32 {
        if self.flags() & 0x1 != 0 {
            ROBOTS_PROJECTILE_GRAVITY
        } else {
            0.0
        }
    }

    /// The optional Event scalar replaces row +0x14 only when positive.
    pub fn resolved_launch_scalar(self, event_scalar: Option<f32>) -> f32 {
        match event_scalar {
            Some(value) if value > 0.0 => value,
            _ => self.default_launch_scalar(),
        }
    }

    /// Native row +0x20 is tested bitwise by `0x004DFEB0` to select projectile
    /// physics/factory variants and post-spawn effects.
    pub const fn flags(self) -> u32 {
        self.words_04_to_34[7]
    }

    pub const fn handler_kind(self) -> RobotsProjectileHandlerKind {
        if self.flags() & 0x2 != 0 {
            RobotsProjectileHandlerKind::Homing
        } else if self.flags() & 0x8 != 0 {
            RobotsProjectileHandlerKind::Mine
        } else {
            RobotsProjectileHandlerKind::Projectile
        }
    }

    pub const fn registration_mask(self) -> u32 {
        if self.flags() & 0x10 != 0 {
            0x0020_2000
        } else {
            0x0000_2000
        }
    }

    /// Native row +0x24 is converted to an integer symmetric range and multiplied
    /// by the global degree-to-radian constant before perturbing launch yaw.
    pub fn yaw_scatter_steps(self) -> i32 {
        f32::from_bits(self.words_04_to_34[8]) as i32
    }

    pub const fn raw_words(self) -> [u32; ROBOTS_MISSILE_ROW_WORDS] {
        [
            self.selector,
            self.words_04_to_34[0],
            self.words_04_to_34[1],
            self.words_04_to_34[2],
            self.words_04_to_34[3],
            self.words_04_to_34[4],
            self.words_04_to_34[5],
            self.words_04_to_34[6],
            self.words_04_to_34[7],
            self.words_04_to_34[8],
            self.words_04_to_34[9],
            self.words_04_to_34[10],
            self.words_04_to_34[11],
            self.words_04_to_34[12],
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsProjectilePhysicsRuntimeState {
    pub position_xyz: [f32; 3],
    /// XItemPhysics_Projectile +0x2C/+0x30/+0x34 initialized by `0x00414EF0`.
    pub linear_velocity_xyz: [f32; 3],
    /// Native Physics +0x124. `0x0041EB10` accumulates gravity here instead of
    /// mutating the base Y velocity stored at +0x30.
    pub gravity_velocity_y: f32,
    /// Native Physics +0x128, seeded by the D04 row bit0 policy.
    pub gravity_acceleration: f32,
    pub updates_completed: u64,
}

/// Shared no-extra-scatter ballistic setup from native `0x0041EC80`.
/// Explosion fragments call that helper with param5=0, so its optional internal
/// process-RNG perturbation is not involved. The returned state feeds the same
/// `advance_physics_fixed()` integrator used by missiles.
pub fn robots_projectile_ballistic_state_from_target(
    source_xyz: [f32; 3],
    target_xyz: [f32; 3],
    vertical_bias: f32,
    gravity_acceleration: f32,
) -> Option<RobotsProjectilePhysicsRuntimeState> {
    if !source_xyz.iter().all(|value| value.is_finite())
        || !target_xyz.iter().all(|value| value.is_finite())
        || !vertical_bias.is_finite()
        || !gravity_acceleration.is_finite()
    {
        return None;
    }

    let delta = [
        target_xyz[0] - source_xyz[0],
        target_xyz[1] - source_xyz[1],
        target_xyz[2] - source_xyz[2],
    ];

    if gravity_acceleration < ROBOTS_PROJECTILE_NATIVE_EPSILON {
        let length_squared = delta.iter().map(|value| value * value).sum::<f32>();
        let linear_velocity_xyz = if length_squared >= 0.0 {
            let length = length_squared.sqrt();
            if length > ROBOTS_PROJECTILE_NORMALIZE_EPSILON {
                delta.map(|value| value / length)
            } else {
                delta
            }
        } else {
            delta
        };
        return Some(RobotsProjectilePhysicsRuntimeState {
            position_xyz: source_xyz,
            linear_velocity_xyz,
            gravity_velocity_y: 0.0,
            gravity_acceleration,
            updates_completed: 0,
        });
    }

    let horizontal_length_squared = delta[0] * delta[0] + delta[2] * delta[2];
    let horizontal_length = horizontal_length_squared.max(0.0).sqrt();
    let horizontal_direction = if horizontal_length > ROBOTS_PROJECTILE_NORMALIZE_EPSILON {
        [delta[0] / horizontal_length, delta[2] / horizontal_length]
    } else {
        [delta[0], delta[2]]
    };

    let launch_height = delta[1] + vertical_bias;
    let gravity_velocity_y = if launch_height < 0.0 {
        0.0
    } else {
        (2.0 * launch_height * gravity_acceleration).sqrt()
            * (1.0 + ROBOTS_PROJECTILE_FIXED_STEP_SECONDS)
    };

    let half_negative_gravity = gravity_acceleration * -0.5;
    let discriminant =
        gravity_velocity_y * gravity_velocity_y - (-delta[1]) * half_negative_gravity * 4.0;
    let root = discriminant.sqrt();
    let denominator = half_negative_gravity + half_negative_gravity;
    let time_a = (root - gravity_velocity_y) / denominator;
    let time_b = (-gravity_velocity_y - root) / denominator;
    let flight_time = time_a.max(time_b);
    let horizontal_speed = if flight_time.abs() > f32::EPSILON {
        horizontal_length / flight_time
    } else {
        0.0
    };

    Some(RobotsProjectilePhysicsRuntimeState {
        position_xyz: source_xyz,
        linear_velocity_xyz: [
            horizontal_direction[0] * horizontal_speed,
            0.0,
            horizontal_direction[1] * horizontal_speed,
        ],
        gravity_velocity_y,
        gravity_acceleration,
        updates_completed: 0,
    })
}

impl RobotsProjectilePhysicsRuntimeState {
    /// Shared fixed-step integrator for native `XItemPhysics_Projectile::Update`
    /// (`0x0041EB10`). Explosion fragments own the same physics object, so they
    /// call this directly instead of fabricating a missile Handler lifetime.
    pub fn advance_fixed(&mut self, runtime_rate_scale: f32, gravity_suppressed: bool) {
        let rate = if runtime_rate_scale.is_finite() {
            runtime_rate_scale
        } else {
            0.0
        };
        if gravity_suppressed {
            self.gravity_velocity_y = 0.0;
        } else {
            self.gravity_velocity_y -=
                rate * self.gravity_acceleration * ROBOTS_PROJECTILE_FIXED_STEP_SECONDS;
        }
        let dt = rate * ROBOTS_PROJECTILE_FIXED_STEP_SECONDS;
        self.position_xyz[0] += self.linear_velocity_xyz[0] * dt;
        self.position_xyz[1] += (self.gravity_velocity_y + self.linear_velocity_xyz[1]) * dt;
        self.position_xyz[2] += self.linear_velocity_xyz[2] * dt;
        self.updates_completed = self.updates_completed.saturating_add(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsProjectileHandlerRuntimeState {
    /// Native Handler+0x390 countdown. `0x004150E0` enters the terminal +0xD4
    /// path only after this becomes strictly negative.
    pub lifetime_seconds: f32,
    pub updates_completed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsProjectileRuntimeState {
    pub physics: RobotsProjectilePhysicsRuntimeState,
    pub handler: RobotsProjectileHandlerRuntimeState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsProjectileHandlerFixedStep {
    pub lifetime_expired: bool,
}

impl RobotsProjectileRuntimeState {
    pub fn from_launch(
        definition: RobotsMissileDefinition,
        request: RobotsCreateProjectileRequest,
        position_xyz: [f32; 3],
        direction_xyz: [f32; 3],
    ) -> Option<Self> {
        if !position_xyz.iter().all(|value| value.is_finite())
            || !direction_xyz.iter().all(|value| value.is_finite())
        {
            return None;
        }
        let length_squared = direction_xyz.iter().map(|value| value * value).sum::<f32>();
        let normalized = if length_squared > f32::EPSILON {
            let inv_length = length_squared.sqrt().recip();
            direction_xyz.map(|value| value * inv_length)
        } else {
            [0.0, 0.0, 0.0]
        };
        let launch_scalar = definition.resolved_launch_scalar(request.launch_scalar());
        let lifetime_seconds = definition.lifetime_seconds();
        if !launch_scalar.is_finite() || !lifetime_seconds.is_finite() {
            return None;
        }
        Some(Self {
            physics: RobotsProjectilePhysicsRuntimeState {
                position_xyz,
                linear_velocity_xyz: normalized.map(|value| value * launch_scalar),
                gravity_velocity_y: 0.0,
                gravity_acceleration: definition.gravity_acceleration(),
                updates_completed: 0,
            },
            handler: RobotsProjectileHandlerRuntimeState {
                lifetime_seconds,
                updates_completed: 0,
            },
        })
    }

    /// Replays `XItemPhysics_Projectile::Update 0x0041EB10`. Collision/contact
    /// resolution stays in the host. `gravity_suppressed` corresponds to native
    /// Physics+0x08 bit0, which resets the accumulated +0x124 gravity lane.
    pub fn advance_physics_fixed(&mut self, runtime_rate_scale: f32, gravity_suppressed: bool) {
        self.physics
            .advance_fixed(runtime_rate_scale, gravity_suppressed);
    }

    /// Replays the Handler+0x390 lifetime tail from `0x004150E0` independently
    /// from Physics so the host can preserve native component/scheduler order.
    pub fn advance_handler_fixed(
        &mut self,
        runtime_rate_scale: f32,
    ) -> RobotsProjectileHandlerFixedStep {
        let rate = if runtime_rate_scale.is_finite() {
            runtime_rate_scale
        } else {
            0.0
        };
        self.handler.lifetime_seconds -= rate * ROBOTS_PROJECTILE_FIXED_STEP_SECONDS;
        self.handler.updates_completed = self.handler.updates_completed.saturating_add(1);
        RobotsProjectileHandlerFixedStep {
            lifetime_expired: self.handler.lifetime_seconds < 0.0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RobotsMissileDatabase {
    pub rows: Vec<RobotsMissileDefinition>,
}

impl RobotsMissileDatabase {
    pub fn definition_by_index(&self, missile_index: u32) -> Option<&RobotsMissileDefinition> {
        self.rows.get(usize::try_from(missile_index).ok()?)
    }

    pub fn definition(&self, selector: u32) -> Option<&RobotsMissileDefinition> {
        self.rows.iter().find(|row| row.selector == selector)
    }
}

/// Loads the exact `D04_Missiles / HT_SpreadSheet_Missiles` sheet bound by
/// Robots.exe around `0x004D409C` to the common projectile runtime at global
/// state +0x424. `0x004DFEB0` walks sheet 0 with a fixed 0x38-byte stride.
pub fn read_robots_missile_database(edb: &mut EdbFile) -> anyhow::Result<RobotsMissileDatabase> {
    anyhow::ensure!(
        edb.header.hashcode == ROBOTS_MISSILE_DATABASE_FILE_UID,
        "expected Robots missile EDB 0x{ROBOTS_MISSILE_DATABASE_FILE_UID:08X}, got 0x{:08X}",
        edb.header.hashcode
    );
    let spreadsheets = UXGeoSpreadsheet::read_all(edb)?;
    let sheets = spreadsheets
        .iter()
        .find_map(|(uid, spreadsheet)| {
            (*uid == ROBOTS_MISSILE_SPREADSHEET_UID)
                .then_some(spreadsheet)
                .and_then(|spreadsheet| match spreadsheet {
                    UXGeoSpreadsheet::Data(sheets) => Some(sheets.as_slice()),
                    UXGeoSpreadsheet::Text(_) => None,
                })
        })
        .context("missing HT_SpreadSheet_Missiles")?;
    let sheet = sheets
        .first()
        .context("HT_SpreadSheet_Missiles has no sheet 0")?;

    edb.seek(SeekFrom::Start(sheet.address as u64))?;
    let mut rows = Vec::with_capacity(sheet.row_count as usize);
    for _ in 0..sheet.row_count {
        let mut words = [0u32; ROBOTS_MISSILE_ROW_WORDS];
        for word in &mut words {
            *word = edb.read_type(edb.endian)?;
        }
        rows.push(RobotsMissileDefinition::from_native_words(words));
    }
    Ok(RobotsMissileDatabase { rows })
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::Path};

    use eurochef_edb::versions::Platform;

    use super::*;

    #[test]
    fn create_projectile_event_keeps_native_argument_roles() {
        let mut data = 0u32.to_le_bytes().to_vec();
        data.extend_from_slice(&3u32.to_le_bytes());
        data.extend_from_slice(&0x1000_0011u32.to_le_bytes());
        data.extend_from_slice(&u32::MAX.to_le_bytes());
        let event = RobotsScriptEventView {
            event_type: 0x1600_001F,
            data: &data,
            start: Some(6),
            length: Some(1),
        };
        assert_eq!(
            RobotsCreateProjectileRequest::from_event(event),
            Some(RobotsCreateProjectileRequest {
                missile_index: 3,
                launch_datum: 0x1000_0011,
                source_override: u32::MAX,
                launch_scalar_bits: None,
            })
        );
    }

    #[test]
    fn native_row_stride_is_0x38() {
        assert_eq!(ROBOTS_MISSILE_ROW_WORDS, 14);
        let words = std::array::from_fn(|index| index as u32);
        let row = RobotsMissileDefinition::from_native_words(words);
        assert_eq!(row.selector, 0);
        assert_eq!(row.raw_words(), words);
    }

    #[test]
    fn shared_ballistic_target_setup_matches_native_gravity_lane() {
        let state = robots_projectile_ballistic_state_from_target(
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            0.25,
            ROBOTS_PROJECTILE_GRAVITY,
        )
        .unwrap();
        let expected_up = (2.0 * 0.25 * ROBOTS_PROJECTILE_GRAVITY).sqrt()
            * (1.0 + ROBOTS_PROJECTILE_FIXED_STEP_SECONDS);
        let expected_time = (2.0 * expected_up) / ROBOTS_PROJECTILE_GRAVITY;
        assert!((state.gravity_velocity_y - expected_up).abs() < 1.0e-6);
        assert!((state.linear_velocity_xyz[0] - expected_time.recip()).abs() < 1.0e-5);
        assert_eq!(state.linear_velocity_xyz[1], 0.0);
        assert_eq!(state.linear_velocity_xyz[2], 0.0);
        assert_eq!(state.gravity_acceleration, ROBOTS_PROJECTILE_GRAVITY);
    }

    #[test]
    fn real_robots_missile_index3_resolves_generic_mine_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d04_miss.edb");
        let file = File::open(&path).expect("open d04_miss.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse d04_miss.edb");
        let database = read_robots_missile_database(&mut edb).expect("decode missile database");
        for (index, row) in database.rows.iter().enumerate() {
            eprintln!(
                "Robots missile raw row{index} words={:08X?}",
                row.raw_words()
            );
        }
        let row = database
            .definition_by_index(3)
            .expect("native missile event index 3");
        assert_eq!(row.selector, 0x5700_0008);
        assert_eq!(row.handler_kind(), RobotsProjectileHandlerKind::Mine);
        assert_eq!(row.registration_mask(), 0x0000_2000);
        assert_eq!(row.factory_resource_pair(), [0x0100_0038, 0x0200_000E]);
        assert_eq!(row.primary_terminal_resource(), 0x0400_004C);
        assert_eq!(row.default_launch_scalar().to_bits(), 0);
        assert_eq!(row.lifetime_seconds().to_bits(), 5.0_f32.to_bits());
        assert_eq!(row.hit_query_radius().to_bits(), 0.2_f32.to_bits());
        assert_eq!(row.gravity_acceleration().to_bits(), 0);
    }

    #[test]
    fn projectile_handler_five_second_lifetime_expires_on_native_update_300() {
        let definition = RobotsMissileDefinition::from_native_words([
            0x5700_0008,
            0,
            0,
            0,
            0,
            0,
            5.0_f32.to_bits(),
            0,
            0x8,
            0,
            0,
            0,
            0,
            0,
        ]);
        let request = RobotsCreateProjectileRequest {
            missile_index: 3,
            launch_datum: 0,
            source_override: u32::MAX,
            launch_scalar_bits: None,
        };
        let mut runtime =
            RobotsProjectileRuntimeState::from_launch(definition, request, [0.0; 3], [0.0; 3])
                .unwrap();
        for update in 1..=300 {
            let step = runtime.advance_handler_fixed(1.0);
            assert_eq!(step.lifetime_expired, update == 300, "update {update}");
        }
        assert_eq!(runtime.handler.updates_completed, 300);
        assert!(runtime.handler.lifetime_seconds < 0.0);
        assert!(RobotsProjectileHandlerKind::Mine.suppresses_physical_contact_terminal_path());
    }

    #[test]
    fn projectile_ballistics_keep_base_velocity_and_accumulate_gravity_separately() {
        let definition = RobotsMissileDefinition::from_native_words([
            0x5700_0000,
            0,
            0,
            0,
            0,
            10.0_f32.to_bits(),
            5.0_f32.to_bits(),
            0,
            1,
            0,
            0,
            0,
            0,
            0,
        ]);
        let request = RobotsCreateProjectileRequest {
            missile_index: 0,
            launch_datum: 0,
            source_override: u32::MAX,
            launch_scalar_bits: None,
        };
        let mut runtime = RobotsProjectileRuntimeState::from_launch(
            definition,
            request,
            [1.0, 2.0, 3.0],
            [1.0, 0.0, 0.0],
        )
        .unwrap();
        runtime.advance_physics_fixed(1.0, false);
        let handler_step = runtime.advance_handler_fixed(1.0);
        assert!(!handler_step.lifetime_expired);
        assert!((runtime.physics.linear_velocity_xyz[0] - 10.0).abs() <= 1.0e-6);
        assert!((runtime.physics.gravity_velocity_y + 9.8 / 60.0).abs() <= 1.0e-6);
        assert!((runtime.physics.position_xyz[0] - (1.0 + 10.0 / 60.0)).abs() <= 1.0e-6);
        assert!((runtime.physics.position_xyz[1] - (2.0 - 9.8 / 3600.0)).abs() <= 1.0e-6);
        assert_eq!(runtime.physics.updates_completed, 1);
        assert_eq!(runtime.handler.updates_completed, 1);
    }
}
