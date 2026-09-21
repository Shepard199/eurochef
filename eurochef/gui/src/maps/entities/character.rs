use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, Seek, SeekFrom},
    sync::Arc,
};

use anyhow::{bail, Context};
use eurochef_edb::{
    anim::EXGeoBaseAnimSkin, binrw::BinReaderExt, edb::EdbFile, versions::Platform, Hashcode,
    HashcodeUtils,
};
use eurochef_shared::{
    robots_runtime::{
        ai_character::{
            ai_handler_class_for_runtime_selector, ai_runtime_type_for_serialized_trigger_type,
        },
        events::{event_type, RobotsScriptEventView},
        projectile::RobotsCreateProjectileRequest,
    },
    script::{UXGeoScript, UXGeoScriptCommandData},
    spreadsheets::UXGeoSpreadsheet,
};
use glam::{Quat, Vec3};
use nohash_hasher::IntMap;
use tracing::warn;

use super::super::{
    ProcessedCharacterAnimDatum, ProcessedCharacterAnimationBonePose,
    ProcessedCharacterAnimationTrack, ProcessedCharacterCollisionProfile,
    ProcessedCharacterCollisionShape, ProcessedCharacterRootMotionSample, ProcessedCharacterVisual,
    ProcessedMap,
};

pub const ROBOTS_MONSTER_DATABASE_FILE: Hashcode = 0x0100_0023;
const ROBOTS_MONSTER_DATABASE_SHEET: Hashcode = 0x1400_0005;
const ROBOTS_CHARACTER_INITIAL_ANIMATION: Hashcode = 0x0300_000E;
pub(crate) const ROBOTS_ANIM_MODE_DEFAULT: Hashcode = 0x0900_0002;
const ROBOTS_ANIM_MODE_MOVE: Hashcode = 0x0900_0003;
const ROBOTS_ANIM_MODE_TURN_ON_SPOT_L: Hashcode = 0x0900_0035;
const ROBOTS_ANIM_MODE_TURN_ON_SPOT_R: Hashcode = 0x0900_0036;
pub(crate) const ROBOTS_ANIM_DATUM_SOLID_COLLISION: Hashcode = 0x1000_0001;

type RobotsCharacterVisualMetadata = (
    Option<Hashcode>,
    Option<ProcessedCharacterCollisionProfile>,
    Option<ProcessedCharacterCollisionProfile>,
    Option<Arc<ProcessedCharacterAnimationTrack>>,
    BTreeMap<Hashcode, Arc<ProcessedCharacterAnimationTrack>>,
    BTreeMap<Hashcode, Arc<UXGeoScript>>,
    BTreeMap<Hashcode, ProcessedCharacterAnimDatum>,
    BTreeMap<(Hashcode, Hashcode), Arc<ProcessedCharacterAnimationTrack>>,
);

/// The exact eight-sheet layout consumed by XTrigger_AI_Character's native
/// resource selector at 0x0047E9E0. Monster rows are 24 bytes and NPC rows are
/// 8 bytes; the first dword of every row is the external character EDB UID.
#[derive(Clone, Copy, Debug)]
struct RobotsCharacterDatabaseRow {
    file: Hashcode,
    /// Native Handler+0x628 seed. `0x0046B8DE..0x0046B919` reads the second
    /// dword of the selected MonsterDatabase row; NPC rows are exactly 8 bytes.
    handler_flags_628: u32,
    /// Native Monster Handler+0x62E initial byte health, loaded by
    /// `0x0045AEB0` from MonsterDatabase row +0x08. NPC sheet 11 rows are only
    /// eight bytes and therefore have no Monster health byte here.
    initial_health: Option<u8>,
    /// MonsterDatabase row +0x09 -> Handler+0x638. Native AI global manager
    /// `0x004563C0` compares this byte through handler vslot +0x14C when choosing
    /// the process-wide current-attacker owner.
    attacker_priority_638: Option<u8>,
    /// MonsterDatabase row +0x0C -> Handler+0x634. Common Monster action
    /// `0x004550A0` uses this explosion UID while Handler+0x628 bit0x40000 is clear.
    explosion_uid_634: Option<u32>,
    /// MonsterDatabase row +0x10 -> Handler+0x630. Common Monster action
    /// `0x004550A0` selects this explosion UID while Handler+0x628 bit0x40000 is set.
    explosion_uid_630: Option<u32>,
    /// MonsterDatabase row +0x14 -> Handler+0x63C. `AI_MagneticHit`
    /// `0x004587E0` consumes this byte as its 0..100 mass coefficient.
    magnetic_mass: Option<u8>,
    /// MonsterDatabase row +0x15 -> Handler+0x63D. This is the existing drop
    /// counter; `AI_MagneticHit` also decrements it while emitting 0x47000001.
    pickup_drop_count: Option<u8>,
}

#[derive(Clone, Debug)]
pub struct RobotsCharacterDatabase {
    rows_by_runtime_type: BTreeMap<u32, Vec<RobotsCharacterDatabaseRow>>,
}

impl RobotsCharacterDatabase {
    pub fn read(edb: &mut EdbFile) -> anyhow::Result<Self> {
        let spreadsheets = UXGeoSpreadsheet::read_all(edb)?;
        let (_, UXGeoSpreadsheet::Data(sheets)) = spreadsheets
            .into_iter()
            .find(|(hashcode, _)| *hashcode == ROBOTS_MONSTER_DATABASE_SHEET)
            .context("HT_SpreadSheet_MonsterDatabase is missing")?
        else {
            bail!("HT_SpreadSheet_MonsterDatabase is not a data spreadsheet");
        };

        if sheets.len() != 8 {
            bail!(
                "unexpected MonsterDatabase sheet count: expected 8, got {}",
                sheets.len()
            );
        }

        let mut rows_by_runtime_type = BTreeMap::new();
        for (sheet_index, sheet) in sheets.into_iter().enumerate() {
            let runtime_type = sheet_index as u32 + 5;
            let row_size = if runtime_type == 11 { 8u64 } else { 24u64 };
            let mut rows = Vec::with_capacity(sheet.row_count as usize);
            for row_index in 0..sheet.row_count {
                let row_address = sheet.address as u64 + row_index as u64 * row_size;
                edb.seek(SeekFrom::Start(row_address))?;
                let file = edb.read_type::<u32>(edb.endian)?;
                let handler_flags_628 = edb.read_type::<u32>(edb.endian)?;
                let initial_health = if row_size >= 0x09 {
                    edb.seek(SeekFrom::Start(row_address + 0x08))?;
                    Some(edb.read_type::<u8>(edb.endian)?)
                } else {
                    None
                };
                let attacker_priority_638 = if row_size >= 0x0a {
                    edb.seek(SeekFrom::Start(row_address + 0x09))?;
                    Some(edb.read_type::<u8>(edb.endian)?)
                } else {
                    None
                };
                let explosion_uid_634 = if row_size >= 0x10 {
                    edb.seek(SeekFrom::Start(row_address + 0x0c))?;
                    Some(edb.read_type::<u32>(edb.endian)?)
                } else {
                    None
                };
                let explosion_uid_630 = if row_size >= 0x14 {
                    edb.seek(SeekFrom::Start(row_address + 0x10))?;
                    Some(edb.read_type::<u32>(edb.endian)?)
                } else {
                    None
                };
                let magnetic_mass = if row_size >= 0x15 {
                    edb.seek(SeekFrom::Start(row_address + 0x14))?;
                    Some(edb.read_type::<u8>(edb.endian)?)
                } else {
                    None
                };
                let pickup_drop_count = if row_size >= 0x16 {
                    edb.seek(SeekFrom::Start(row_address + 0x15))?;
                    Some(edb.read_type::<u8>(edb.endian)?)
                } else {
                    None
                };
                rows.push(RobotsCharacterDatabaseRow {
                    file,
                    handler_flags_628,
                    initial_health,
                    attacker_priority_638,
                    explosion_uid_634,
                    explosion_uid_630,
                    magnetic_mass,
                    pickup_drop_count,
                });
            }
            rows_by_runtime_type.insert(runtime_type, rows);
        }

        Ok(Self {
            rows_by_runtime_type,
        })
    }

    pub fn file_for_trigger(&self, serialized_type: u32, data: &[Option<u32>]) -> Option<Hashcode> {
        let runtime_type = robots_character_runtime_type(serialized_type)?;
        let config_index = data.first().copied().flatten()? as usize;
        self.file_for_runtime_selector(runtime_type, config_index)
    }

    pub(crate) fn file_for_runtime_selector(
        &self,
        runtime_type: u32,
        config_index: usize,
    ) -> Option<Hashcode> {
        self.rows_by_runtime_type
            .get(&runtime_type)?
            .get(config_index)
            .map(|row| row.file)
            .filter(|file| file.base() == 0x0100_0000 && *file != 0x0100_0000)
    }

    pub(crate) fn handler_flags_for_runtime_selector(
        &self,
        runtime_type: u32,
        config_index: usize,
    ) -> Option<u32> {
        self.rows_by_runtime_type
            .get(&runtime_type)?
            .get(config_index)
            .map(|row| row.handler_flags_628)
    }

    pub(crate) fn initial_health_for_runtime_selector(
        &self,
        runtime_type: u32,
        config_index: usize,
    ) -> Option<u8> {
        self.rows_by_runtime_type
            .get(&runtime_type)?
            .get(config_index)?
            .initial_health
    }

    pub(crate) fn attacker_priority_for_runtime_selector(
        &self,
        runtime_type: u32,
        config_index: usize,
    ) -> Option<u8> {
        self.rows_by_runtime_type
            .get(&runtime_type)?
            .get(config_index)?
            .attacker_priority_638
    }

    pub(crate) fn explosion_uids_for_runtime_selector(
        &self,
        runtime_type: u32,
        config_index: usize,
    ) -> (Option<u32>, Option<u32>) {
        self.rows_by_runtime_type
            .get(&runtime_type)
            .and_then(|rows| rows.get(config_index))
            .map(|row| (row.explosion_uid_634, row.explosion_uid_630))
            .unwrap_or((None, None))
    }

    pub(crate) fn magnetic_mass_for_runtime_selector(
        &self,
        runtime_type: u32,
        config_index: usize,
    ) -> Option<u8> {
        self.rows_by_runtime_type
            .get(&runtime_type)?
            .get(config_index)?
            .magnetic_mass
    }

    pub(crate) fn pickup_drop_count_for_runtime_selector(
        &self,
        runtime_type: u32,
        config_index: usize,
    ) -> Option<u8> {
        self.rows_by_runtime_type
            .get(&runtime_type)?
            .get(config_index)?
            .pickup_drop_count
    }
}

/// Exact serialized EXGeoTriggerType -> runtime AI-character family bridge.
/// Runtime types 5..12 index the eight MonsterDatabase sheets.
pub fn robots_character_runtime_type(serialized_type: u32) -> Option<u32> {
    ai_runtime_type_for_serialized_trigger_type(serialized_type)
}

/// Native `XTrigger_AI_Character::Create` (`0x0047E740`) indexes the
/// 16-byte table at `0x0061F380` by serialized trigger type and copies the
/// first dword to `XItem+0x260`. All eight shipped AI-character trigger
/// families map to raw group 1. Keep this tied to the already-proven family
/// bridge so unrelated trigger classes can never be promoted accidentally.
pub(crate) fn robots_character_hit_query_raw_group(serialized_type: u32) -> Option<i32> {
    robots_character_runtime_type(serialized_type).map(|_| 1)
}

fn preview_script(edb: &mut EdbFile) -> anyhow::Result<Option<Hashcode>> {
    let saved_internal_references = edb.internal_references.clone();
    let saved_external_references = edb.external_references.clone();
    let scripts = UXGeoScript::read_all(edb)?;
    edb.internal_references = saved_internal_references;
    edb.external_references = saved_external_references;

    Ok(scripts
        .iter()
        .filter(|script| script.hashcode.is_local())
        .find(|script| {
            script
                .commands
                .iter()
                .any(|command| matches!(command.data, UXGeoScriptCommandData::Animation { .. }))
        })
        .or_else(|| {
            scripts.iter().find(|script| {
                script
                    .commands
                    .iter()
                    .any(|command| matches!(command.data, UXGeoScriptCommandData::Animation { .. }))
            })
        })
        .map(|script| script.hashcode))
}

fn preview_collision_profile(
    edb: &mut EdbFile,
) -> anyhow::Result<Option<ProcessedCharacterCollisionProfile>> {
    if edb.header.version != 248 || edb.platform != Platform::Pc {
        return Ok(None);
    }
    let headers = edb.header.animskin_list.data().clone();
    let mut selected: Option<ProcessedCharacterCollisionProfile> = None;
    for header in headers {
        edb.seek(SeekFrom::Start(header.common.address as u64))?;
        let skin = edb.read_type_args::<EXGeoBaseAnimSkin>(edb.endian, (248,))?;
        let Some(datum) = skin
            .robots_animdatum_section
            .as_ref()
            .and_then(|section| section.find(0x1000_0004))
        else {
            continue;
        };
        let Some(radius) = datum.map_collision_radius() else {
            continue;
        };
        let shape = if datum.header.shape_mode == 3 {
            ProcessedCharacterCollisionShape::Capsule {
                half_segment: datum.map_collision_half_segment().unwrap_or_default(),
                radius,
            }
        } else {
            ProcessedCharacterCollisionShape::Sphere { radius }
        };
        let profile = ProcessedCharacterCollisionProfile {
            animskin: header.common.hashcode,
            shape,
            local_center: Vec3::from_array(datum.local_center),
            local_orientation: datum.local_orientation,
            transform_selector: datum.transform_selector,
        };
        if let Some(existing) = selected {
            if existing.shape != profile.shape
                || existing.local_center != profile.local_center
                || existing.local_orientation != profile.local_orientation
                || existing.transform_selector != profile.transform_selector
            {
                bail!(
                    "character EDB contains differing searchable MapCollision datums across AnimSkin variations"
                );
            }
        } else {
            selected = Some(profile);
        }
    }
    Ok(selected)
}

pub(crate) fn preview_anim_datum_collision_profile(
    edb: &mut EdbFile,
    animskin: Hashcode,
    datum_hashcode: Hashcode,
) -> anyhow::Result<Option<ProcessedCharacterCollisionProfile>> {
    if edb.header.version != 248 || edb.platform != Platform::Pc {
        return Ok(None);
    }
    let Some(header) = edb
        .header
        .animskin_list
        .data()
        .iter()
        .find(|header| header.common.hashcode == animskin)
        .cloned()
    else {
        return Ok(None);
    };
    edb.seek(SeekFrom::Start(header.common.address as u64))?;
    let skin = edb.read_type_args::<EXGeoBaseAnimSkin>(edb.endian, (248,))?;
    let Some(datum) = skin
        .robots_animdatum_section
        .as_ref()
        .and_then(|section| section.find(datum_hashcode))
    else {
        return Ok(None);
    };
    let shape = if datum.header.shape_mode == 3 {
        ProcessedCharacterCollisionShape::Capsule {
            half_segment: datum.shape_scalars[0],
            radius: datum.shape_scalars[1],
        }
    } else {
        ProcessedCharacterCollisionShape::Sphere {
            radius: datum.shape_scalars[0],
        }
    };
    Ok(Some(ProcessedCharacterCollisionProfile {
        animskin,
        shape,
        local_center: Vec3::from_array(datum.local_center),
        local_orientation: datum.local_orientation,
        transform_selector: datum.transform_selector,
    }))
}

pub(crate) fn preview_hit_area_profile(
    edb: &mut EdbFile,
    animskin: Hashcode,
) -> anyhow::Result<Option<ProcessedCharacterCollisionProfile>> {
    preview_anim_datum_collision_profile(edb, animskin, 0x1000_0010)
}

fn preview_anim_datums(
    edb: &mut EdbFile,
    animskin: Hashcode,
) -> anyhow::Result<BTreeMap<Hashcode, ProcessedCharacterAnimDatum>> {
    if edb.header.version != 248 || edb.platform != Platform::Pc {
        return Ok(BTreeMap::new());
    }
    let Some(header) = edb
        .header
        .animskin_list
        .data()
        .iter()
        .find(|header| header.common.hashcode == animskin)
        .cloned()
    else {
        return Ok(BTreeMap::new());
    };
    edb.seek(SeekFrom::Start(header.common.address as u64))?;
    let skin = edb.read_type_args::<EXGeoBaseAnimSkin>(edb.endian, (248,))?;
    let mut datums = BTreeMap::new();
    if let Some(section) = skin.robots_animdatum_section.as_ref() {
        for entry in section.searchable_entries() {
            let datum = entry.datum();
            datums.insert(
                entry.hashcode,
                ProcessedCharacterAnimDatum {
                    hashcode: entry.hashcode,
                    animskin,
                    shape: if datum.header.shape_mode == 3 {
                        ProcessedCharacterCollisionShape::Capsule {
                            half_segment: datum.shape_scalars[0],
                            radius: datum.shape_scalars[1],
                        }
                    } else {
                        ProcessedCharacterCollisionShape::Sphere {
                            radius: datum.shape_scalars[0],
                        }
                    },
                    local_center: Vec3::from_array(datum.local_center),
                    local_orientation: datum.local_orientation,
                    transform_selector: datum.transform_selector,
                },
            );
        }
    }
    Ok(datums)
}

fn preview_animation_track(
    edb: &mut EdbFile,
    collision: &ProcessedCharacterCollisionProfile,
    animation_hashcode: Hashcode,
    transition_fixed_ticks: u16,
) -> anyhow::Result<Option<ProcessedCharacterAnimationTrack>> {
    preview_animation_track_for_selector(
        edb,
        collision.animskin,
        collision.transform_selector,
        animation_hashcode,
        transition_fixed_ticks,
    )
}

fn preview_animation_track_for_selector(
    edb: &mut EdbFile,
    animskin: Hashcode,
    transform_selector: u8,
    animation_hashcode: Hashcode,
    transition_fixed_ticks: u16,
) -> anyhow::Result<Option<ProcessedCharacterAnimationTrack>> {
    if edb.header.version != 248 || edb.platform != Platform::Pc {
        return Ok(None);
    }

    let Some(animation) = edb
        .header
        .anim_list
        .data()
        .iter()
        .find(|animation| animation.common.hashcode == animation_hashcode)
        .cloned()
    else {
        return Ok(None);
    };
    let Some(skin_header) = edb
        .header
        .animskin_list
        .data()
        .iter()
        .find(|skin| skin.common.hashcode == animskin)
        .cloned()
    else {
        return Ok(None);
    };
    if animation.skin_num != u32::MAX && skin_header.base_skin_num != animation.skin_num {
        return Ok(None);
    }

    edb.seek(SeekFrom::Start(skin_header.common.address as u64))?;
    let skin = edb.read_type_args::<EXGeoBaseAnimSkin>(edb.endian, (248,))?;
    let selector = usize::from(transform_selector);
    let bone_count = skin.bone_count as usize;
    if selector >= bone_count
        || skin.relative_bind_positions.len() < bone_count
        || skin.hier_data.len() < bone_count
    {
        return Ok(None);
    }

    let mut bone_chain = Vec::new();
    let mut bone = selector;
    loop {
        bone_chain.push(bone);
        let parent = usize::from(skin.hier_data[bone].link_index);
        if parent >= bone || parent >= bone_count {
            break;
        }
        bone = parent;
        if bone_chain.len() > bone_count {
            bail!("AnimSkin hierarchy contains a cycle while resolving MapCollision selector");
        }
    }
    bone_chain.reverse();

    let serialized = animation.read_robots_v248_exgeoanim(edb, edb.endian)?;
    if serialized.bone_count as usize != bone_count || serialized.frame_count == 0 {
        return Ok(None);
    }
    let motion = animation.read_robots_v248_motion_stream(edb)?;
    let block_table = animation.read_robots_v248_motion_block_table(edb, edb.endian)?;
    let root_correction = animation.read_robots_v248_root_correction_transform(edb, edb.endian)?;
    let frame_count = usize::from(serialized.frame_count);
    let mut poses = Vec::with_capacity(frame_count.saturating_mul(bone_chain.len()));
    let mut root_motion_samples = Vec::with_capacity(frame_count);
    for frame_index in 0..frame_count {
        let raw = match block_table.as_ref() {
            Some(table) => serialized.decode_skeletal_frame_with_block_table(
                &motion,
                frame_index as u16,
                table,
            )?,
            None => serialized.decode_skeletal_frame(&motion, frame_index as u16)?,
        };
        let root_motion = serialized.extract_root_motion_transform(&raw)?;
        root_motion_samples.push(ProcessedCharacterRootMotionSample {
            position: Vec3::from_array(root_motion.position),
            rotation: Quat::from_array(root_motion.rotation).normalize(),
        });
        let assembled = serialized.assemble_same_skin_pose_with_root_correction(
            &raw,
            &skin.relative_bind_positions,
            root_correction.as_ref(),
        )?;
        for &bone_index in &bone_chain {
            let pose = &assembled.bones[bone_index];
            poses.push(ProcessedCharacterAnimationBonePose {
                position: Vec3::from_array(pose.position),
                rotation: Quat::from_xyzw(
                    pose.rotation[0],
                    pose.rotation[1],
                    pose.rotation[2],
                    pose.rotation[3],
                ),
            });
        }
    }

    Ok(Some(ProcessedCharacterAnimationTrack {
        animation: animation_hashcode,
        animskin,
        clip_rate: serialized.raw_byte_0c,
        transition_fixed_ticks,
        frame_count,
        root_motion_samples,
        bone_chain,
        poses,
    }))
}

fn preview_animation_track_for_binding(
    edb: &mut EdbFile,
    collision: &ProcessedCharacterCollisionProfile,
    animation_hashcode: Hashcode,
    transition_fixed_ticks: u16,
    explicit_animskin: Option<Hashcode>,
) -> anyhow::Result<Option<ProcessedCharacterAnimationTrack>> {
    let Some(animation) = edb
        .header
        .anim_list
        .data()
        .iter()
        .find(|animation| animation.common.hashcode == animation_hashcode)
        .cloned()
    else {
        return Ok(None);
    };

    if let Some(animskin_reference) = explicit_animskin {
        let Some(animskin) = resolve_preview_animskin_reference(edb, animskin_reference) else {
            return Ok(None);
        };
        return preview_animation_track_for_selector(
            edb,
            animskin,
            collision.transform_selector,
            animation_hashcode,
            transition_fixed_ticks,
        );
    }

    let mut candidate_animskins = Vec::new();
    if let Some(collision_skin) = edb
        .header
        .animskin_list
        .data()
        .iter()
        .find(|skin| skin.common.hashcode == collision.animskin)
    {
        if animation.skin_num == u32::MAX || collision_skin.base_skin_num == animation.skin_num {
            candidate_animskins.push(collision.animskin);
        }
    }
    if animation.skin_num != u32::MAX {
        for skin in edb.header.animskin_list.data() {
            if skin.base_skin_num == animation.skin_num
                && !candidate_animskins.contains(&skin.common.hashcode)
            {
                candidate_animskins.push(skin.common.hashcode);
            }
        }
    }

    for animskin in candidate_animskins {
        if let Some(track) = preview_animation_track_for_selector(
            edb,
            animskin,
            collision.transform_selector,
            animation_hashcode,
            transition_fixed_ticks,
        )? {
            return Ok(Some(track));
        }
    }
    Ok(None)
}

/// Mirrors the native opcode-2 target-skin resource semantics already used by
/// `AnimationRuntime::resolve_animskin_reference`: a serialized reference may
/// name an AnimSkin directly or an Animation whose bound skin is selected via
/// that Animation's `skin_num`. Local `0x8...` references are indices within
/// the corresponding typed list, not interchangeable local-object namespaces.
fn resolve_preview_animskin_reference(edb: &EdbFile, reference: Hashcode) -> Option<Hashcode> {
    if reference & 0x7f00_0000 == 0x0300_0000 {
        let animations = edb.header.anim_list.data();
        let animation = if reference.is_local() {
            animations.get(reference.index() as usize)
        } else {
            animations
                .iter()
                .find(|animation| animation.common.hashcode == reference)
        }?;
        return edb
            .header
            .animskin_list
            .data()
            .iter()
            .find(|skin| skin.base_skin_num == animation.skin_num)
            .map(|skin| skin.common.hashcode);
    }

    let skins = edb.header.animskin_list.data();
    if reference.is_local() {
        skins
            .get(reference.index() as usize)
            .map(|skin| skin.common.hashcode)
    } else {
        skins
            .iter()
            .find(|skin| skin.common.hashcode == reference)
            .map(|skin| skin.common.hashcode)
    }
}

struct PreviewAnimModeBinding {
    track: ProcessedCharacterAnimationTrack,
    script: Option<UXGeoScript>,
}

fn preview_anim_mode_binding(
    edb: &mut EdbFile,
    collision: &ProcessedCharacterCollisionProfile,
    target_mode_hashcode: Hashcode,
) -> anyhow::Result<Option<PreviewAnimModeBinding>> {
    if edb.header.version != 248 || edb.platform != Platform::Pc {
        return Ok(None);
    }

    let modes = edb.header.animmode_list.data().clone();
    let sets = edb.header.animset_list.data().clone();
    let Some(default_mode) = modes
        .iter()
        .find(|mode| mode.common.hashcode == ROBOTS_ANIM_MODE_DEFAULT)
        .cloned()
    else {
        return Ok(None);
    };
    let Some(target_mode_index) = modes
        .iter()
        .position(|mode| mode.common.hashcode == target_mode_hashcode)
    else {
        return Ok(None);
    };
    let transitions = default_mode.read_robots_v248_transitions(edb, edb.endian)?;
    let Some(transition) = transitions
        .iter()
        .find(|transition| transition.new_mode_index as usize == target_mode_index)
    else {
        return Ok(None);
    };
    if transition.controls.len() != 1 || transition.controls[0].opcode != 0x0C00_0002 {
        return Ok(None);
    }
    let control = &transition.controls[0];
    let Some(anim_set) = sets.get(control.resource_key as usize) else {
        return Ok(None);
    };
    let groups = anim_set.read_robots_v248_groups(edb, edb.endian)?;
    if groups.len() != 1 || groups[0].contributions.len() != 1 {
        return Ok(None);
    }
    let contribution = groups[0].contributions[0];
    let resource = contribution.resource_hashcode;
    let family = resource >> 24 & 0x7F;
    let (animation_hashcode, script, explicit_animskin) = match family {
        3 => (resource, None, None),
        4 => {
            let Some(header) = edb
                .header
                .animscript_list
                .iter()
                .find(|header| header.hashcode == resource)
                .cloned()
            else {
                return Ok(None);
            };
            let saved_internal_references = edb.internal_references.clone();
            let saved_external_references = edb.external_references.clone();
            let script = UXGeoScript::read(&header, edb)?;
            edb.internal_references = saved_internal_references;
            edb.external_references = saved_external_references;
            let animations = script
                .commands
                .iter()
                .filter_map(|command| match command.data {
                    UXGeoScriptCommandData::Animation {
                        skin_file,
                        skin_hashcode,
                        anim_file,
                        anim_hashcode,
                    } => Some((skin_file, skin_hashcode, anim_file, anim_hashcode)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if animations.len() != 1 {
                return Ok(None);
            }
            let (skin_file, skin_hashcode, anim_file, anim_hashcode) = animations[0];
            if anim_file != u32::MAX && anim_file != edb.header.hashcode {
                return Ok(None);
            }
            if skin_hashcode != u32::MAX
                && skin_file != u32::MAX
                && skin_file != edb.header.hashcode
            {
                return Ok(None);
            }
            (
                anim_hashcode,
                Some(script),
                (skin_hashcode != u32::MAX).then_some(skin_hashcode),
            )
        }
        _ => return Ok(None),
    };

    Ok(preview_animation_track_for_binding(
        edb,
        collision,
        animation_hashcode,
        contribution.raw_u16_06,
        explicit_animskin,
    )?
    .map(|track| PreviewAnimModeBinding { track, script }))
}

fn preview_anim_mode_animation_track(
    edb: &mut EdbFile,
    collision: &ProcessedCharacterCollisionProfile,
    target_mode_hashcode: Hashcode,
) -> anyhow::Result<Option<ProcessedCharacterAnimationTrack>> {
    Ok(
        preview_anim_mode_binding(edb, collision, target_mode_hashcode)?
            .map(|binding| binding.track),
    )
}

fn preview_anim_mode_bindings(
    edb: &mut EdbFile,
    collision: &ProcessedCharacterCollisionProfile,
) -> anyhow::Result<(
    BTreeMap<Hashcode, Arc<ProcessedCharacterAnimationTrack>>,
    BTreeMap<Hashcode, Arc<UXGeoScript>>,
)> {
    let mode_hashcodes = edb
        .header
        .animmode_list
        .data()
        .iter()
        .map(|mode| mode.common.hashcode)
        .filter(|mode| *mode != ROBOTS_ANIM_MODE_DEFAULT)
        .collect::<Vec<_>>();
    let mut tracks = BTreeMap::new();
    let mut scripts = BTreeMap::new();
    for mode in mode_hashcodes {
        if let Some(binding) = preview_anim_mode_binding(edb, collision, mode)? {
            tracks.insert(mode, Arc::new(binding.track));
            if let Some(script) = binding.script {
                scripts.insert(mode, Arc::new(script));
            }
        }
    }
    Ok((tracks, scripts))
}

fn preview_anim_mode_datum_tracks(
    edb: &mut EdbFile,
    animation_modes: &BTreeMap<Hashcode, Arc<ProcessedCharacterAnimationTrack>>,
    animation_mode_scripts: &BTreeMap<Hashcode, Arc<UXGeoScript>>,
    anim_datums: &BTreeMap<Hashcode, ProcessedCharacterAnimDatum>,
) -> anyhow::Result<BTreeMap<(Hashcode, Hashcode), Arc<ProcessedCharacterAnimationTrack>>> {
    let mut tracks = BTreeMap::new();
    for (&anim_mode, script) in animation_mode_scripts {
        let Some(base_track) = animation_modes.get(&anim_mode) else {
            continue;
        };
        for command in &script.commands {
            let Some(event) = RobotsScriptEventView::from_command(command) else {
                continue;
            };
            let datum_hashcode = match event.event_type {
                event_type::CREATE_PROJECTILE => {
                    let Some(request) = RobotsCreateProjectileRequest::from_event(event) else {
                        continue;
                    };
                    request.launch_datum
                }
                event_type::HIT_CHECK => {
                    let Some(selector) = event.native_arg_word(0) else {
                        continue;
                    };
                    selector
                }
                _ => continue,
            };
            let Some(datum) = anim_datums.get(&datum_hashcode) else {
                continue;
            };
            let key = (anim_mode, datum_hashcode);
            if tracks.contains_key(&key) {
                continue;
            }
            if let Some(track) = preview_animation_track_for_selector(
                edb,
                datum.animskin,
                datum.transform_selector,
                base_track.animation,
                base_track.transition_fixed_ticks,
            )? {
                tracks.insert(key, Arc::new(track));
            }
        }
    }

    // Native global XItem collision `0x004D5750 -> 0x004D5B80` queries
    // HT_AnimDatum_SolidCollision on every collidable character independently of
    // AnimScript events. Predecode that selector for every resolved AnimMode so the
    // common physics/contact host does not depend on a coincidental HitCheck event.
    if let Some(datum) = anim_datums.get(&ROBOTS_ANIM_DATUM_SOLID_COLLISION) {
        for (&anim_mode, base_track) in animation_modes {
            let key = (anim_mode, ROBOTS_ANIM_DATUM_SOLID_COLLISION);
            if tracks.contains_key(&key) {
                continue;
            }
            if let Some(track) = preview_animation_track_for_selector(
                edb,
                datum.animskin,
                datum.transform_selector,
                base_track.animation,
                base_track.transition_fixed_ticks,
            )? {
                tracks.insert(key, Arc::new(track));
            }
        }

        // Fresh XItems already participate in the global collision pass before any
        // behavior node is required to own an AnimMode. Native layer0 starts on
        // HT_Animation_Idle_Attack; keep the SolidCollision selector for that pose
        // under the Default-mode key so the host can resolve the pre-behavior body
        // without inventing a root-only fallback.
        let default_key = (ROBOTS_ANIM_MODE_DEFAULT, ROBOTS_ANIM_DATUM_SOLID_COLLISION);
        if !tracks.contains_key(&default_key) {
            if let Some(track) = preview_animation_track_for_selector(
                edb,
                datum.animskin,
                datum.transform_selector,
                ROBOTS_CHARACTER_INITIAL_ANIMATION,
                0,
            )? {
                tracks.insert(default_key, Arc::new(track));
            }
        }
    }
    Ok(tracks)
}

fn preview_anim_mode_animation_tracks(
    edb: &mut EdbFile,
    collision: &ProcessedCharacterCollisionProfile,
) -> anyhow::Result<BTreeMap<Hashcode, Arc<ProcessedCharacterAnimationTrack>>> {
    preview_anim_mode_bindings(edb, collision).map(|(tracks, _)| tracks)
}

fn preview_move_animation_track(
    edb: &mut EdbFile,
    collision: &ProcessedCharacterCollisionProfile,
) -> anyhow::Result<Option<ProcessedCharacterAnimationTrack>> {
    preview_anim_mode_animation_track(edb, collision, ROBOTS_ANIM_MODE_MOVE)
}

fn preview_turn_on_spot_l_animation_track(
    edb: &mut EdbFile,
    collision: &ProcessedCharacterCollisionProfile,
) -> anyhow::Result<Option<ProcessedCharacterAnimationTrack>> {
    preview_anim_mode_animation_track(edb, collision, ROBOTS_ANIM_MODE_TURN_ON_SPOT_L)
}

fn preview_turn_on_spot_r_animation_track(
    edb: &mut EdbFile,
    collision: &ProcessedCharacterCollisionProfile,
) -> anyhow::Result<Option<ProcessedCharacterAnimationTrack>> {
    preview_anim_mode_animation_track(edb, collision, ROBOTS_ANIM_MODE_TURN_ON_SPOT_R)
}

fn preview_initial_animation_track(
    edb: &mut EdbFile,
    collision: &ProcessedCharacterCollisionProfile,
) -> anyhow::Result<Option<ProcessedCharacterAnimationTrack>> {
    preview_animation_track(edb, collision, ROBOTS_CHARACTER_INITIAL_ANIMATION, 0)
}

fn load_robots_character_visual_metadata(
    file: Hashcode,
    runtime_type: u32,
    config_index: u32,
    path_cache: &IntMap<Hashcode, String>,
    platform: Platform,
    metadata_by_file: &mut BTreeMap<Hashcode, RobotsCharacterVisualMetadata>,
) -> anyhow::Result<RobotsCharacterVisualMetadata> {
    if let Some(metadata) = metadata_by_file.get(&file) {
        return Ok(metadata.clone());
    }

    let metadata = match path_cache.get(&file) {
        Some(path) => {
            let external_file =
                File::open(path).with_context(|| format!("open character EDB {path}"))?;
            let mut external_edb = EdbFile::new(Box::new(BufReader::new(external_file)), platform)?;
            let script = preview_script(&mut external_edb)?;
            let collision = preview_collision_profile(&mut external_edb)?;
            let hit_area = match collision.as_ref() {
                Some(profile) => preview_hit_area_profile(&mut external_edb, profile.animskin)?,
                None => None,
            };
            let initial_animation = match collision.as_ref() {
                Some(profile) => {
                    preview_initial_animation_track(&mut external_edb, profile)?.map(Arc::new)
                }
                None => None,
            };
            let (animation_modes, animation_mode_scripts) = match collision.as_ref() {
                Some(profile) => preview_anim_mode_bindings(&mut external_edb, profile)?,
                None => (BTreeMap::new(), BTreeMap::new()),
            };
            let anim_datums = match collision.as_ref() {
                Some(profile) => preview_anim_datums(&mut external_edb, profile.animskin)?,
                None => BTreeMap::new(),
            };
            let animation_mode_datum_tracks = preview_anim_mode_datum_tracks(
                &mut external_edb,
                &animation_modes,
                &animation_mode_scripts,
                &anim_datums,
            )?;
            (
                script,
                collision,
                hit_area,
                initial_animation,
                animation_modes,
                animation_mode_scripts,
                anim_datums,
                animation_mode_datum_tracks,
            )
        }
        None => {
            warn!(
                "Character EDB 0x{file:08X} selected by runtime type {runtime_type} config {config_index} is absent from path_cache"
            );
            (
                None,
                None,
                None,
                None,
                BTreeMap::new(),
                BTreeMap::new(),
                BTreeMap::new(),
                BTreeMap::new(),
            )
        }
    };
    metadata_by_file.insert(file, metadata.clone());
    Ok(metadata)
}

fn build_robots_character_visual(
    current_edb: &mut EdbFile,
    database: &RobotsCharacterDatabase,
    runtime_type: u32,
    config_index: u32,
    path_cache: &IntMap<Hashcode, String>,
    platform: Platform,
    metadata_by_file: &mut BTreeMap<Hashcode, RobotsCharacterVisualMetadata>,
) -> anyhow::Result<Option<ProcessedCharacterVisual>> {
    let Some(file) = database.file_for_runtime_selector(runtime_type, config_index as usize) else {
        return Ok(None);
    };
    let Some(handler_class) = ai_handler_class_for_runtime_selector(runtime_type, config_index)
    else {
        warn!("No native AI handler class for runtime type {runtime_type} config {config_index}");
        return Ok(None);
    };
    let handler_flags_628 = database
        .handler_flags_for_runtime_selector(runtime_type, config_index as usize)
        .unwrap_or(0);
    let initial_health =
        database.initial_health_for_runtime_selector(runtime_type, config_index as usize);
    let attacker_priority_638 =
        database.attacker_priority_for_runtime_selector(runtime_type, config_index as usize);
    let (explosion_uid_634, explosion_uid_630) =
        database.explosion_uids_for_runtime_selector(runtime_type, config_index as usize);
    let magnetic_mass =
        database.magnetic_mass_for_runtime_selector(runtime_type, config_index as usize);
    let pickup_drop_count =
        database.pickup_drop_count_for_runtime_selector(runtime_type, config_index as usize);
    let (
        script,
        collision,
        hit_area,
        initial_animation,
        animation_modes,
        animation_mode_scripts,
        anim_datums,
        animation_mode_datum_tracks,
    ) = load_robots_character_visual_metadata(
        file,
        runtime_type,
        config_index,
        path_cache,
        platform,
        metadata_by_file,
    )?;
    let Some(script) = script else {
        return Ok(None);
    };
    let move_animation = animation_modes.get(&ROBOTS_ANIM_MODE_MOVE).cloned();
    let turn_on_spot_l_animation = animation_modes
        .get(&ROBOTS_ANIM_MODE_TURN_ON_SPOT_L)
        .cloned();
    let turn_on_spot_r_animation = animation_modes
        .get(&ROBOTS_ANIM_MODE_TURN_ON_SPOT_R)
        .cloned();
    current_edb.add_external_reference(file, script);
    Ok(Some(ProcessedCharacterVisual {
        file,
        script,
        runtime_type,
        config_index,
        handler_class,
        handler_flags_628,
        initial_health,
        attacker_priority_638,
        explosion_uid_634,
        explosion_uid_630,
        magnetic_mass,
        pickup_drop_count,
        collision,
        hit_area,
        initial_animation,
        animation_modes,
        animation_mode_scripts,
        anim_datums,
        animation_mode_datum_tracks,
        move_animation,
        turn_on_spot_l_animation,
        turn_on_spot_r_animation,
    }))
}

pub(crate) fn resolve_robots_character_visual_catalog(
    current_edb: &mut EdbFile,
    path_cache: &IntMap<Hashcode, String>,
    platform: Platform,
    selectors: &[(u32, u32)],
) -> anyhow::Result<BTreeMap<(u32, u32), ProcessedCharacterVisual>> {
    let Some(database_path) = path_cache.get(&ROBOTS_MONSTER_DATABASE_FILE) else {
        return Ok(BTreeMap::new());
    };
    let database_file = File::open(database_path)
        .with_context(|| format!("open MonsterDatabase EDB {database_path}"))?;
    let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), platform)?;
    let database = RobotsCharacterDatabase::read(&mut database_edb)?;
    let mut metadata_by_file = BTreeMap::<Hashcode, RobotsCharacterVisualMetadata>::new();
    let mut catalog = BTreeMap::new();
    for &(runtime_type, config_index) in selectors {
        if catalog.contains_key(&(runtime_type, config_index)) {
            continue;
        }
        if let Some(visual) = build_robots_character_visual(
            current_edb,
            &database,
            runtime_type,
            config_index,
            path_cache,
            platform,
            &mut metadata_by_file,
        )? {
            catalog.insert((runtime_type, config_index), visual);
        }
    }
    Ok(catalog)
}

/// Resolves runtime-created Monster/NPC/Fish models before the normal external
/// reference closure is loaded. The game creates these XItems from data[0] and
/// d00_mons.edb rather than serializing visual_object on the trigger.
pub fn resolve_robots_character_visuals(
    current_edb: &mut EdbFile,
    maps: &mut [ProcessedMap],
    path_cache: &IntMap<Hashcode, String>,
    platform: Platform,
) -> anyhow::Result<usize> {
    let Some(database_path) = path_cache.get(&ROBOTS_MONSTER_DATABASE_FILE) else {
        warn!(
            "Robots MonsterDatabase EDB 0x{:08X} is absent from path_cache",
            ROBOTS_MONSTER_DATABASE_FILE
        );
        return Ok(0);
    };

    let database_file = File::open(database_path)
        .with_context(|| format!("open MonsterDatabase EDB {database_path}"))?;
    let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), platform)?;
    let database = RobotsCharacterDatabase::read(&mut database_edb)?;

    let mut metadata_by_file = BTreeMap::<Hashcode, RobotsCharacterVisualMetadata>::new();
    let mut resolved = 0usize;
    for map in maps {
        let (triggers, runtime_character_visuals) =
            (&mut map.triggers, &mut map.runtime_character_visuals);
        for trigger in triggers {
            let Some(runtime_type) = robots_character_runtime_type(trigger.ttype) else {
                continue;
            };
            let Some(config_index) = trigger.data.first().copied().flatten() else {
                continue;
            };
            let Some(visual) = build_robots_character_visual(
                current_edb,
                &database,
                runtime_type,
                config_index,
                path_cache,
                platform,
                &mut metadata_by_file,
            )?
            else {
                continue;
            };
            runtime_character_visuals.insert((runtime_type, config_index), visual.clone());
            trigger.character_visual = Some(visual);
            resolved += 1;
        }
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use std::{
        fs::File,
        io::{BufReader, Seek, SeekFrom},
        path::Path,
    };

    use eurochef_edb::{
        anim::EXGeoBaseAnimSkin, binrw::BinReaderExt, edb::EdbFile, versions::Platform,
    };
    use eurochef_shared::{
        robots_runtime::events::{event_type, RobotsScriptEventView},
        script::UXGeoScriptCommandData,
    };
    use glam::Mat4;
    use nohash_hasher::IntMap;

    use super::{
        preview_anim_datums, preview_anim_mode_bindings, preview_anim_mode_datum_tracks,
        preview_collision_profile, preview_move_animation_track,
        preview_turn_on_spot_l_animation_track, preview_turn_on_spot_r_animation_track,
        resolve_robots_character_visuals, robots_character_hit_query_raw_group,
        robots_character_runtime_type, RobotsCharacterDatabase, ROBOTS_ANIM_MODE_MOVE,
        ROBOTS_CHARACTER_INITIAL_ANIMATION, ROBOTS_MONSTER_DATABASE_FILE,
    };
    use crate::maps::ProcessedCharacterCollisionShape;

    #[test]
    fn serialized_character_types_use_the_exact_runtime_database_sheets() {
        assert_eq!(robots_character_runtime_type(10), Some(5));
        assert_eq!(robots_character_runtime_type(11), Some(6));
        assert_eq!(robots_character_runtime_type(18), Some(7));
        assert_eq!(robots_character_runtime_type(33), Some(8));
        assert_eq!(robots_character_runtime_type(74), Some(9));
        assert_eq!(robots_character_runtime_type(3), Some(10));
        assert_eq!(robots_character_runtime_type(48), Some(11));
        assert_eq!(robots_character_runtime_type(70), Some(12));
        assert_eq!(robots_character_runtime_type(4), None);
    }

    #[test]
    fn serialized_character_types_use_native_raw_hit_query_group_one() {
        for serialized_type in [3, 10, 11, 18, 33, 48, 70, 74] {
            assert_eq!(
                robots_character_hit_query_raw_group(serialized_type),
                Some(1)
            );
        }
        assert_eq!(robots_character_hit_query_raw_group(4), None);
    }

    #[test]
    fn real_robots_v248_ef01_solid_collision_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/ef01_min.edb");
        let file = File::open(&path).expect("open ef01_min.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse ef01_min.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse EF01 collision")
            .expect("EF01 MapCollision profile");
        let datums =
            preview_anim_datums(&mut edb, collision.animskin).expect("parse EF01 AnimDatums");
        let solid = datums
            .get(&super::ROBOTS_ANIM_DATUM_SOLID_COLLISION)
            .expect("EF01 SolidCollision datum");
        assert_eq!(solid.animskin, 0x8D00_0001);
        assert_eq!(solid.transform_selector, 26);
        assert_eq!(
            solid.shape,
            ProcessedCharacterCollisionShape::Capsule {
                half_segment: 0.56324387,
                radius: 0.83675605,
            }
        );
        assert!((solid.local_center.x - 0.010869006).abs() <= 1.0e-6);
        assert!((solid.local_center.y - 1.4166412).abs() <= 1.0e-6);
        assert!((solid.local_center.z - 0.0028536883).abs() <= 1.0e-6);

        let (animation_modes, animation_mode_scripts) =
            preview_anim_mode_bindings(&mut edb, &collision)
                .expect("resolve EF01 AnimMode bindings");
        let tracks = preview_anim_mode_datum_tracks(
            &mut edb,
            &animation_modes,
            &animation_mode_scripts,
            &datums,
        )
        .expect("resolve EF01 SolidCollision tracks");
        let default_solid = tracks
            .get(&(
                super::ROBOTS_ANIM_MODE_DEFAULT,
                super::ROBOTS_ANIM_DATUM_SOLID_COLLISION,
            ))
            .expect("EF01 Default SolidCollision sampled track");
        assert_eq!(default_solid.animation, ROBOTS_CHARACTER_INITIAL_ANIMATION);
        assert_eq!(default_solid.animskin, solid.animskin);
        assert_eq!(default_solid.bone_chain.last().copied(), Some(26));
        for &anim_mode in animation_modes.keys() {
            let track = tracks
                .get(&(anim_mode, super::ROBOTS_ANIM_DATUM_SOLID_COLLISION))
                .unwrap_or_else(|| {
                    panic!("EF01 AnimMode 0x{anim_mode:08X} missing SolidCollision track")
                });
            assert_eq!(track.animskin, solid.animskin);
            assert_eq!(track.bone_chain.last().copied(), Some(26));
        }
    }

    #[test]
    fn real_robots_v248_dog_default_move_track_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/eq01_dog.edb");
        let file = File::open(&path).expect("open eq01_dog.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse eq01_dog.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse DogBot collision")
            .expect("DogBot MapCollision profile");
        let track = preview_move_animation_track(&mut edb, &collision)
            .expect("resolve DogBot Default -> Move track")
            .expect("DogBot Move track");
        assert_eq!(track.animation, 0x8300_0016);
        assert_eq!(track.animskin, collision.animskin);
        assert!(track.frame_count > 0 && !track.bone_chain.is_empty());
        assert_eq!(
            track.poses.len(),
            track.frame_count * track.bone_chain.len()
        );
        let move_max_frame_translation = track
            .root_motion_samples
            .windows(2)
            .map(|samples| (samples[1].position - samples[0].position).length())
            .fold(0.0_f32, f32::max);
        let move_total_translation = track
            .root_motion_samples
            .first()
            .zip(track.root_motion_samples.last())
            .map(|(first, last)| (last.position - first.position).length())
            .unwrap_or_default();
        eprintln!(
            "DogBot Move root-motion animation=0x{:08X} frames={} rate={} max_frame={move_max_frame_translation:.6} total={move_total_translation:.6}",
            track.animation, track.frame_count, track.clip_rate
        );
        assert!(move_max_frame_translation > 1.0e-6);

        let endian = edb.endian;
        let modes = edb.header.animmode_list.data().clone();
        let sets = edb.header.animset_list.data().clone();
        let default_mode = modes
            .iter()
            .find(|mode| mode.common.hashcode == super::ROBOTS_ANIM_MODE_DEFAULT)
            .expect("DogBot Default AnimMode");
        let move_mode_index = modes
            .iter()
            .position(|mode| mode.common.hashcode == ROBOTS_ANIM_MODE_MOVE)
            .expect("DogBot Move AnimMode index");
        let transition = default_mode
            .read_robots_v248_transitions(&mut edb, endian)
            .expect("DogBot Default transitions")
            .into_iter()
            .find(|transition| transition.new_mode_index as usize == move_mode_index)
            .expect("DogBot Default -> Move transition");
        assert_eq!(transition.controls.len(), 1);
        let control = &transition.controls[0];
        let anim_set = sets
            .get(control.resource_key as usize)
            .expect("DogBot Move AnimSet");
        let move_groups = anim_set
            .read_robots_v248_groups(&mut edb, endian)
            .expect("DogBot Move AnimSet groups");
        eprintln!(
            "DogBot Move AnimSet=0x{:08X} groups={move_groups:?}",
            anim_set.common.hashcode,
        );
        assert_eq!(move_groups.len(), 1);
        assert_eq!(move_groups[0].contributions.len(), 1);
        assert_eq!(move_groups[0].weight, 0.0);
        let move_contribution = move_groups[0].contributions[0];
        assert_eq!(move_contribution.resource_hashcode, 0x8400_0016);
        assert_eq!(move_contribution.raw_u16_04, 11);
        assert_eq!(move_contribution.raw_u16_06, 5);
        assert_eq!(move_contribution.raw_u32_08, 0);

        let (animation_modes, animation_mode_scripts) =
            preview_anim_mode_bindings(&mut edb, &collision)
                .expect("decode DogBot AnimMode bindings");
        let move_script = animation_mode_scripts
            .get(&ROBOTS_ANIM_MODE_MOVE)
            .expect("DogBot Move AnimMode must retain its native AnimScript");
        eprintln!(
            "DogBot Move script=0x{:08X} fps={} len={} commands={:?}",
            move_script.hashcode,
            move_script.framerate,
            move_script.length,
            move_script
                .commands
                .iter()
                .map(|command| (command.opcode, command.start, command.length, &command.data))
                .collect::<Vec<_>>()
        );
        assert_eq!(move_script.hashcode, 0x8400_0016);
        let attack_mode = 0x0900_0027;
        let attack_track = animation_modes
            .get(&attack_mode)
            .expect("DogBot Attack2 AnimMode must resolve");
        let attack_script = animation_mode_scripts
            .get(&attack_mode)
            .expect("DogBot Attack2 AnimMode must retain its native AnimScript");
        let attack_events = attack_script
            .commands
            .iter()
            .filter_map(|command| match &command.data {
                UXGeoScriptCommandData::Event { event_type, .. } => {
                    let view = eurochef_shared::robots_runtime::events::RobotsScriptEventView::from_command(command)?;
                    Some((command.start, *event_type, view.words::<5>(), view.native_args::<4>()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let attack_timeline = attack_script
            .commands
            .iter()
            .map(|command| (command.opcode, command.start, command.length))
            .collect::<Vec<_>>();
        eprintln!(
            "DogBot Attack2 animation=0x{:08X} frames={} rate={} script=0x{:08X} fps={} len={} commands={attack_timeline:?} events={attack_events:?}",
            attack_track.animation,
            attack_track.frame_count,
            attack_track.clip_rate,
            attack_script.hashcode,
            attack_script.framerate,
            attack_script.length,
        );
        assert!(!attack_events.is_empty());
        let anim_datums = preview_anim_datums(&mut edb, collision.animskin)
            .expect("decode DogBot searchable AnimDatums");
        let attack_point = anim_datums
            .get(&0x1000_0009)
            .expect("DogBot HT_AnimDatum_AttackPoint");
        let datum_tracks = preview_anim_mode_datum_tracks(
            &mut edb,
            &animation_modes,
            &animation_mode_scripts,
            &anim_datums,
        )
        .expect("decode DogBot gameplay datum tracks");
        let attack_point_track = datum_tracks
            .get(&(attack_mode, 0x1000_0009))
            .expect("DogBot Attack2/AttackPoint sampled track");
        eprintln!(
            "DogBot AttackPoint shape={:?} selector={} sampled_animation=0x{:08X}",
            attack_point.shape, attack_point.transform_selector, attack_point_track.animation,
        );
        assert_eq!(attack_point_track.animation, attack_track.animation);

        for death_mode in [0x0900_0033, 0x0900_0032] {
            assert!(
                animation_modes.contains_key(&death_mode),
                "DogBot HitDeath AnimMode 0x{death_mode:08X} must resolve"
            );
            let script = animation_mode_scripts
                .get(&death_mode)
                .expect("DogBot HitDeath AnimMode must retain its native AnimScript");
            let explosion_events = script
                .commands
                .iter()
                .filter_map(|command| match &command.data {
                    UXGeoScriptCommandData::Event { event_type, .. }
                        if *event_type == 0x1600_0039 =>
                    {
                        let view = eurochef_shared::robots_runtime::events::RobotsScriptEventView::from_command(command)?;
                        Some((command.start, view.words::<5>(), view.native_args::<4>()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            eprintln!(
                "DogBot HitDeath mode=0x{death_mode:08X} script=0x{:08X} MonsterExplosion={explosion_events:?}",
                script.hashcode
            );
            assert!(!explosion_events.is_empty());
            assert_eq!(
                explosion_events.last().and_then(|(_, _, args)| args[0]),
                Some(0),
                "DogBot HitDeath must retain its guaranteed late MonsterExplosion"
            );
        }
    }

    #[test]
    fn real_robots_v248_shunt_family_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/ew03_shu.edb");
        let file = File::open(&path).expect("open ew03_shu.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse ew03_shu.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse ShuntBotBoss collision")
            .expect("ShuntBotBoss MapCollision profile");
        let (_, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode Shunt family AnimMode bindings");

        for mode in [
            0x0900_002A,
            0x0900_003C,
            0x0900_003D,
            0x0900_0032,
            0x0900_003A,
            0x0900_003B,
        ] {
            assert!(
                scripts.contains_key(&mode),
                "ew03_shu must retain normal/fatal Shunt AnimMode 0x{mode:08X}"
            );
        }
        for mode in [0x0900_002A, 0x0900_003C, 0x0900_003D] {
            let script = &scripts[&mode];
            assert!(
                script.commands.iter().any(|command| matches!(
                    &command.data,
                    UXGeoScriptCommandData::Event { event_type, .. }
                        if *event_type == event_type::SETUP_IDLE
                )),
                "normal Shunt hit mode 0x{mode:08X} must terminate through SetupIdle"
            );
        }
        for mode in [0x0900_0032, 0x0900_003A, 0x0900_003B] {
            let script = &scripts[&mode];
            assert!(
                script.commands.iter().any(|command| matches!(
                    &command.data,
                    UXGeoScriptCommandData::Event { event_type, .. }
                        if *event_type == event_type::MONSTER_EXPLOSION
                )),
                "Shunt fatal mode 0x{mode:08X} must retain MonsterExplosion"
            );
        }

        for mode in [0x0900_0027, 0x0900_0038, 0x0900_0039, 0x0900_0037] {
            let script = scripts
                .get(&mode)
                .unwrap_or_else(|| panic!("ShuntBotBoss AnimMode 0x{mode:08X} script"));
            let events = script
                .commands
                .iter()
                .filter_map(|command| match &command.data {
                    UXGeoScriptCommandData::Event { event_type, .. } => {
                        let view = RobotsScriptEventView::from_command(command)?;
                        Some((command.start, *event_type, view.native_args::<5>()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            eprintln!(
                "ShuntBotBoss mode=0x{mode:08X} script=0x{:08X} events={events:?}",
                script.hashcode
            );
            let expected = match mode {
                0x0900_0027 => vec![(35, event_type::SETUP_IDLE, [None; 5])],
                0x0900_0038 => vec![(
                    0,
                    event_type::HIT_CHECK,
                    [Some(0x1000_0009), Some(26.0f32.to_bits()), None, None, None],
                )],
                0x0900_0039 => vec![(25, event_type::SETUP_IDLE, [None; 5])],
                0x0900_0037 => vec![(20, event_type::SETUP_IDLE, [None; 5])],
                _ => unreachable!(),
            };
            assert_eq!(events, expected);
        }
    }

    #[test]
    fn real_robots_v248_sawbot_family_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/ew02_saw.edb");
        let file = File::open(&path).expect("open ew02_saw.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse ew02_saw.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse SawBot collision")
            .expect("SawBot MapCollision profile");
        let (_, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode SawBot AnimMode bindings");

        for mode in [
            0x0900_0006,
            0x0900_0025,
            0x0900_0027,
            0x0900_0037,
            0x0900_0029,
            0x0900_002A,
            0x0900_0033,
            0x0900_0032,
        ] {
            assert!(
                scripts.contains_key(&mode),
                "SawBot AnimMode 0x{mode:08X} must resolve"
            );
        }

        for mode in [0x0900_0029, 0x0900_002A] {
            let script = &scripts[&mode];
            assert!(
                script.commands.iter().any(|command| matches!(
                    &command.data,
                    UXGeoScriptCommandData::Event { event_type, .. }
                        if *event_type == event_type::SETUP_IDLE
                )),
                "SawBot hit mode 0x{mode:08X} must terminate through SetupIdle"
            );
        }
        for mode in [0x0900_0033, 0x0900_0032] {
            let script = &scripts[&mode];
            assert!(
                script.commands.iter().any(|command| matches!(
                    &command.data,
                    UXGeoScriptCommandData::Event { event_type, .. }
                        if *event_type == event_type::MONSTER_EXPLOSION
                )),
                "SawBot fatal mode 0x{mode:08X} must retain MonsterExplosion"
            );
        }

        for mode in [0x0900_0025, 0x0900_0027, 0x0900_0037] {
            let script = &scripts[&mode];
            let events = script
                .commands
                .iter()
                .filter_map(|command| match &command.data {
                    UXGeoScriptCommandData::Event { event_type, .. } => {
                        let view = RobotsScriptEventView::from_command(command)?;
                        Some((command.start, *event_type, view.native_args::<5>()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let expected = match mode {
                0x0900_0025 => vec![
                    (
                        5,
                        event_type::HIT_CHECK,
                        [Some(0x1000_0009), Some(60.0f32.to_bits()), None, None, None],
                    ),
                    (35, event_type::SETUP_IDLE, [None; 5]),
                ],
                0x0900_0027 => vec![
                    (
                        8,
                        event_type::HIT_CHECK,
                        [Some(0x1000_0009), Some(94.0f32.to_bits()), None, None, None],
                    ),
                    (74, event_type::SETUP_IDLE, [None; 5]),
                ],
                0x0900_0037 => vec![
                    (
                        7,
                        0x1600_0021,
                        [Some(0x1900_0018), Some(u32::MAX), None, None, None],
                    ),
                    (
                        16,
                        event_type::HIT_CHECK,
                        [Some(0x1000_0009), Some(18.0f32.to_bits()), None, None, None],
                    ),
                    (35, event_type::SETUP_IDLE, [None; 5]),
                ],
                _ => unreachable!(),
            };
            assert_eq!(
                events, expected,
                "SawBot attack 0x{mode:08X} event contract"
            );
        }
    }

    #[test]
    fn real_robots_v248_spikebot_family_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/eb01_spi.edb");
        let file = File::open(&path).expect("open eb01_spi.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse eb01_spi.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse SpikeBot collision")
            .expect("SpikeBot MapCollision profile");
        let (_, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode SpikeBot AnimMode bindings");

        for mode in [
            0x0900_0006,
            0x0900_0007,
            0x0900_0029,
            0x0900_002A,
            0x0900_0033,
            0x0900_0032,
            0x0900_002F,
            0x0900_0031,
            0x0900_0030,
            0x0900_00EA,
        ] {
            assert!(
                scripts.contains_key(&mode),
                "SpikeBot AnimMode 0x{mode:08X} must resolve"
            );
        }

        for mode in [0x0900_002F, 0x0900_0031, 0x0900_0030, 0x0900_00EA] {
            let events = scripts[&mode]
                .commands
                .iter()
                .filter_map(|command| match &command.data {
                    UXGeoScriptCommandData::Event { event_type, .. } => {
                        let view = RobotsScriptEventView::from_command(command)?;
                        Some((command.start, *event_type, view.native_args::<5>()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let expected = match mode {
                0x0900_002F => vec![(30, event_type::SETUP_IDLE, [None; 5])],
                0x0900_0031 => vec![(31, event_type::SETUP_IDLE, [None; 5])],
                0x0900_0030 => vec![(
                    0,
                    event_type::HIT_CHECK,
                    [Some(0x1000_0001), Some(31.0f32.to_bits()), None, None, None],
                )],
                0x0900_00EA => vec![(
                    0,
                    event_type::HIT_CHECK,
                    [Some(0x1000_0001), Some(45.0f32.to_bits()), None, None, None],
                )],
                _ => unreachable!(),
            };
            assert_eq!(
                events, expected,
                "SpikeBot attack 0x{mode:08X} event contract"
            );
        }
        let mut set_script_value_modes = scripts
            .iter()
            .filter_map(|(mode, script)| {
                script
                    .commands
                    .iter()
                    .any(|command| {
                        matches!(
                            &command.data,
                            UXGeoScriptCommandData::Event { event_type, .. }
                                if *event_type == event_type::SET_SCRIPT_VALUE
                        )
                    })
                    .then_some(*mode)
            })
            .collect::<Vec<_>>();
        set_script_value_modes.sort_unstable();
        assert_eq!(set_script_value_modes, vec![0x0900_0026]);
    }

    #[test]
    fn real_robots_v248_magnabot_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/eb11_mag.edb");
        let file = File::open(&path).expect("open eb11_mag.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse eb11_mag.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse MagnaBot collision")
            .expect("MagnaBot MapCollision profile");
        let (tracks, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode MagnaBot AnimMode bindings");

        // EB11 deliberately lacks IdleAttack 04 and generic TurnOnSpot 28. Native
        // still has behavior code that can request them, but 0x004051E0 only writes
        // the pending AnimMode; missing resource bindings do not become a live pose.
        assert!(!tracks.contains_key(&0x0900_0004));
        assert!(!tracks.contains_key(&0x0900_0028));

        for mode in [
            0x0900_0003,
            0x0900_0025,
            0x0900_0029,
            0x0900_002A,
            0x0900_002D,
            0x0900_0032,
            0x0900_0033,
            0x0900_0035,
            0x0900_0036,
            0x0900_0076,
            0x0900_0077,
            0x0900_0078,
            0x0900_007D,
        ] {
            assert!(
                tracks.contains_key(&mode),
                "MagnaBot shipped AnimMode 0x{mode:08X} must resolve to an animation track"
            );
            assert!(
                scripts.contains_key(&mode),
                "MagnaBot shipped AnimMode 0x{mode:08X} must retain its AnimScript"
            );
        }

        let database_path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d00_mons.edb");
        let database_file = File::open(&database_path).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        let flags = database
            .handler_flags_for_runtime_selector(6, 9)
            .expect("EB11 MagnaBot MonsterDatabase row");
        assert_ne!(
            flags & 0x4,
            0,
            "MagnaBot must use directional TurnOnSpotL/R"
        );
    }

    #[test]
    fn real_robots_v248_eb12_evilbot_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/eb12_evi.edb");
        let file = File::open(&path).expect("open eb12_evi.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse eb12_evi.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse EB12 EvilBot collision")
            .expect("EB12 EvilBot MapCollision profile");
        let (tracks, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode EB12 EvilBot AnimMode bindings");

        for mode in [
            0x0900_0003,
            0x0900_0025,
            0x0900_0027,
            0x0900_0029,
            0x0900_002A,
            0x0900_002D,
            0x0900_0032,
            0x0900_0033,
            0x0900_0076,
            0x0900_0077,
            0x0900_0078,
            0x0900_007D,
            0x0900_00E9,
        ] {
            assert!(
                tracks.contains_key(&mode),
                "EB12 EvilBot native builder mode 0x{mode:08X} must resolve"
            );
        }

        for mode in [0x0900_0025, 0x0900_0027] {
            let events = scripts[&mode]
                .commands
                .iter()
                .filter_map(|command| match &command.data {
                    UXGeoScriptCommandData::Event { event_type, .. } => {
                        let view = RobotsScriptEventView::from_command(command)?;
                        Some((command.start, *event_type, view.native_args::<5>()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let expected = match mode {
                0x0900_0025 => vec![
                    (
                        2,
                        event_type::ATTACH_SWOOSH,
                        [Some(0x1900_0017), Some(u32::MAX), None, None, None],
                    ),
                    (
                        5,
                        event_type::HIT_CHECK,
                        [Some(0x1000_000B), Some(6.0f32.to_bits()), None, None, None],
                    ),
                    (42, event_type::SETUP_IDLE, [None; 5]),
                ],
                0x0900_0027 => vec![
                    (
                        2,
                        event_type::ATTACH_SWOOSH,
                        [Some(0x1900_0018), Some(u32::MAX), None, None, None],
                    ),
                    (
                        8,
                        event_type::HIT_CHECK,
                        [Some(0x1000_000C), Some(6.0f32.to_bits()), None, None, None],
                    ),
                    (38, event_type::SETUP_IDLE, [None; 5]),
                ],
                _ => unreachable!(),
            };
            assert_eq!(
                events, expected,
                "EB12 attack mode 0x{mode:08X} event contract"
            );
        }

        let database_path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d00_mons.edb");
        let database_file = File::open(&database_path).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        assert_eq!(database.file_for_runtime_selector(5, 12), Some(0x0100_004C));
    }

    #[test]
    fn real_robots_v248_ew09_armoured_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/ew09_arm.edb");
        let file = File::open(&path).expect("open ew09_arm.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse ew09_arm.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse EW09 Armoured collision")
            .expect("EW09 Armoured MapCollision profile");
        let (tracks, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode EW09 Armoured AnimMode bindings");
        let attack = tracks.get(&0x0900_0025).expect("EW09 Attack25 track");
        assert_eq!(attack.animation, 0x8300_0003);
        assert_eq!(attack.frame_count, 26);
        assert_eq!(attack.clip_rate, 30);
        let total_translation = attack
            .root_motion_samples
            .first()
            .zip(attack.root_motion_samples.last())
            .map(|(first, last)| (last.position - first.position).length())
            .unwrap_or_default();
        let max_frame_translation = attack
            .root_motion_samples
            .windows(2)
            .map(|pair| (pair[1].position - pair[0].position).length())
            .fold(0.0f32, f32::max);
        assert!(total_translation <= 1.0e-6);
        assert!(max_frame_translation <= 1.0e-6);

        let attack_script = scripts.get(&0x0900_0025).expect("EW09 Attack25 script");
        let attack_events = attack_script
            .commands
            .iter()
            .filter_map(|command| match &command.data {
                UXGeoScriptCommandData::Event { event_type, .. } => {
                    let view = RobotsScriptEventView::from_command(command)?;
                    Some((command.start, *event_type, view.native_args::<5>()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            attack_events,
            vec![
                (
                    7,
                    event_type::HIT_CHECK,
                    [Some(0x1000_0009), Some(8.0f32.to_bits()), None, None, None],
                ),
                (25, event_type::SETUP_IDLE, [None; 5]),
            ]
        );
        let database_path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d00_mons.edb");
        let database_file = File::open(&database_path).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        assert_eq!(database.file_for_runtime_selector(5, 8), Some(0x0100_0045));
        assert_eq!(database.handler_flags_for_runtime_selector(5, 8), Some(1));
    }

    #[test]
    fn real_robots_v248_eq04_mine_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/eq04_min.edb");
        let file = File::open(&path).expect("open eq04_min.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse eq04_min.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse EQ04 Mine collision")
            .expect("EQ04 Mine MapCollision profile");
        let (tracks, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode EQ04 Mine AnimMode bindings");

        eprintln!("EQ04 modes: {:?}", tracks.keys().collect::<Vec<_>>());
        if let Some(track) = tracks.get(&0x0900_0003) {
            let total_translation = track
                .root_motion_samples
                .first()
                .zip(track.root_motion_samples.last())
                .map(|(first, last)| (last.position - first.position).length())
                .unwrap_or_default();
            let max_frame_translation = track
                .root_motion_samples
                .windows(2)
                .map(|pair| (pair[1].position - pair[0].position).length())
                .fold(0.0f32, f32::max);
            eprintln!(
                "EQ04 Move frames={} rate={} total_translation={} max_frame_translation={}",
                track.frame_count, track.clip_rate, total_translation, max_frame_translation
            );
        }
        for (mode, script) in &scripts {
            let events = script
                .commands
                .iter()
                .filter_map(|command| match &command.data {
                    UXGeoScriptCommandData::Event { event_type, .. } => {
                        Some((command.start, *event_type))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            eprintln!("EQ04 mode 0x{mode:08X} events={events:?}");
        }

        assert!(tracks.contains_key(&0x0900_0003));
        assert!(scripts.contains_key(&0x0900_0003));
    }

    #[test]
    fn real_robots_v248_ew09_armoured_shipped_path_bindings_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let root =
            Path::new(&game_root).join("_eurotools_out/extracted_main/robots/binary/_bin_pc");
        let mut armoured = Vec::new();
        for entry in std::fs::read_dir(&root).expect("scan Robots EDB folder") {
            let path = entry.expect("bad EDB directory entry").path();
            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if path.extension().and_then(|value| value.to_str()) != Some("edb")
                || !file_name.starts_with('m')
            {
                continue;
            }
            let file = File::open(&path).expect("open candidate map EDB");
            let Ok(mut edb) = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc) else {
                continue;
            };
            let maps = crate::maps::read_from_file(&mut edb);
            for map in &maps {
                for (trigger_index, trigger) in map.triggers.iter().enumerate() {
                    if robots_character_runtime_type(trigger.ttype) != Some(5)
                        || trigger.data.first().copied().flatten() != Some(8)
                    {
                        continue;
                    }
                    let path_uid = trigger.data.get(2).copied().flatten();
                    let resolved_path = path_uid.and_then(|uid| {
                        map.paths.iter().find(|candidate| candidate.hashcode == uid)
                    });
                    armoured.push((
                        file_name.to_owned(),
                        trigger_index,
                        path_uid,
                        resolved_path.map(|path| path.path_type),
                        resolved_path.map(|path| path.nodes.len()),
                        trigger.position,
                    ));
                }
            }
        }
        assert_eq!(armoured.len(), 1);
        assert_eq!(armoured[0].0, "m99_enem.edb");
        assert_eq!(armoured[0].1, 25);
        assert_eq!(armoured[0].2, Some(0x0B00_0000));
        assert_eq!(armoured[0].3, None);
        assert_eq!(armoured[0].4, None);
    }

    #[test]
    fn real_robots_v248_ew04_securitybot_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/ew04_sec.edb");
        let file = File::open(&path).expect("open ew04_sec.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse ew04_sec.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse EW04 SecurityBot collision")
            .expect("EW04 SecurityBot MapCollision profile");
        let (tracks, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode EW04 SecurityBot AnimMode bindings");
        for mode in [
            0x0900_0003,
            0x0900_0006,
            0x0900_0007,
            0x0900_0025,
            0x0900_0027,
            0x0900_0029,
            0x0900_002A,
            0x0900_002D,
            0x0900_0032,
            0x0900_0033,
            0x0900_0076,
            0x0900_0077,
            0x0900_0078,
            0x0900_007D,
            0x0900_00E9,
        ] {
            assert!(
                tracks.contains_key(&mode),
                "EW04 SecurityBot native builder/runtime mode 0x{mode:08X} must resolve"
            );
        }
        for mode in [0x0900_0025, 0x0900_0027] {
            let events = scripts[&mode]
                .commands
                .iter()
                .filter_map(|command| match &command.data {
                    UXGeoScriptCommandData::Event { event_type, .. } => {
                        let view = RobotsScriptEventView::from_command(command)?;
                        Some((command.start, *event_type, view.native_args::<5>()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let expected = match mode {
                0x0900_0025 => vec![
                    (
                        0,
                        event_type::ATTACH_SWOOSH,
                        [Some(0x1900_0018), Some(u32::MAX), None, None, None],
                    ),
                    (
                        0,
                        event_type::ATTACH_SWOOSH,
                        [Some(0x1900_0017), Some(u32::MAX), None, None, None],
                    ),
                    (
                        4,
                        event_type::HIT_CHECK,
                        [Some(0x1000_000B), Some(8.0f32.to_bits()), None, None, None],
                    ),
                    (
                        4,
                        event_type::HIT_CHECK,
                        [Some(0x1000_000C), Some(8.0f32.to_bits()), None, None, None],
                    ),
                    (14, 0, [None; 5]),
                    (50, event_type::SETUP_IDLE, [None; 5]),
                ],
                0x0900_0027 => vec![
                    (
                        3,
                        event_type::HIT_CHECK,
                        [Some(0x1000_000B), Some(17.0f32.to_bits()), None, None, None],
                    ),
                    (
                        3,
                        event_type::ATTACH_SWOOSH,
                        [Some(0x1900_0018), Some(u32::MAX), None, None, None],
                    ),
                    (
                        3,
                        event_type::ATTACH_SWOOSH,
                        [Some(0x1900_0017), Some(u32::MAX), None, None, None],
                    ),
                    (
                        3,
                        event_type::HIT_CHECK,
                        [Some(0x1000_000C), Some(17.0f32.to_bits()), None, None, None],
                    ),
                    (38, event_type::SETUP_IDLE, [None; 5]),
                ],
                _ => unreachable!(),
            };
            assert_eq!(
                events, expected,
                "EW04 attack mode 0x{mode:08X} event contract"
            );
        }
        let database_path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d00_mons.edb");
        let database_file = File::open(&database_path).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        assert_eq!(database.file_for_runtime_selector(5, 5), Some(0x0100_002D));
        let flags = database
            .handler_flags_for_runtime_selector(5, 5)
            .expect("EW04 SecurityBot MonsterDatabase row");
        assert_eq!(flags, 0);
        assert_eq!(flags & 0x4, 0, "EW04 does not use directional TurnOnSpot");
    }

    #[test]
    fn real_robots_v248_eb05_guardbot_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/eb05_gua.edb");
        let file = File::open(&path).expect("open eb05_gua.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse eb05_gua.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse EB05 GuardBot collision")
            .expect("EB05 GuardBot MapCollision profile");
        let (tracks, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode EB05 GuardBot AnimMode bindings");
        for mode in [
            0x0900_0003,
            0x0900_0025,
            0x0900_0029,
            0x0900_002A,
            0x0900_0032,
            0x0900_0033,
            0x0900_0076,
            0x0900_0077,
            0x0900_0078,
            0x0900_007D,
            0x0900_00E9,
        ] {
            assert!(
                tracks.contains_key(&mode),
                "EB05 GuardBot native builder/runtime mode 0x{mode:08X} must resolve"
            );
        }
        let attack_script = scripts
            .get(&0x0900_0025)
            .expect("EB05 GuardBot Attack25 AnimScript");
        assert!(
            attack_script.commands.iter().any(|command| matches!(
                &command.data,
                UXGeoScriptCommandData::Event { event_type, .. }
                    if *event_type == event_type::SETUP_IDLE
            )),
            "GuardBot TurnAttack relies on Attack25 SetupIdle to complete"
        );

        let database_path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d00_mons.edb");
        let database_file = File::open(&database_path).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        assert_eq!(database.file_for_runtime_selector(6, 2), Some(0x0100_0029));
    }

    #[test]
    fn real_robots_v248_ep02_turret_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/ep02_tur.edb");
        let file = File::open(&path).expect("open ep02_tur.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse ep02_tur.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse EP02 Turret collision")
            .expect("EP02 Turret MapCollision profile");
        let (tracks, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode EP02 Turret AnimMode bindings");
        let yaw = tracks
            .get(&0x0900_0003)
            .expect("EP02 Turret yaw mode03 must resolve");
        assert_eq!(yaw.animation, 0x8300_0007);
        assert_eq!(yaw.animskin, 0x0D00_0000);
        let attack = tracks
            .get(&0x0900_0025)
            .expect("EP02 Turret Attack25 must resolve");
        assert_eq!(attack.animation, 0x8300_0003);
        assert_eq!(attack.animskin, 0x8D00_0001);
        assert!(!tracks.contains_key(&0x0900_0001));
        assert!(!tracks.contains_key(&0x0900_0033));
        let attack_events = scripts[&0x0900_0025]
            .commands
            .iter()
            .filter_map(|command| match &command.data {
                UXGeoScriptCommandData::Event { event_type, .. } => {
                    Some((command.start, *event_type))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            attack_events,
            vec![
                (3, event_type::CREATE_PROJECTILE),
                (15, event_type::SETUP_IDLE),
            ]
        );

        let database_path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d00_mons.edb");
        let database_file = File::open(&database_path).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        assert_eq!(database.file_for_runtime_selector(6, 5), Some(0x0100_0036));
    }

    #[test]
    fn real_robots_v248_ep04_turret_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/ep04_tur.edb");
        let file = File::open(&path).expect("open ep04_tur.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse ep04_tur.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse EP04 Turret collision")
            .expect("EP04 Turret MapCollision profile");
        let (tracks, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode EP04 Turret AnimMode bindings");
        let mut resolved_modes = tracks.keys().copied().collect::<Vec<_>>();
        resolved_modes.sort_unstable();
        assert_eq!(resolved_modes, vec![0x0900_0025, 0x0900_0029]);
        let attack = tracks
            .get(&0x0900_0025)
            .expect("EP04 Turret Attack25 must resolve");
        assert_eq!(attack.animation, 0x8300_0006);
        assert_eq!(attack.animskin, 0x0D00_001A);
        assert!(!tracks.contains_key(&0x0900_0003));
        let attack_events = scripts[&0x0900_0025]
            .commands
            .iter()
            .filter_map(|command| match &command.data {
                UXGeoScriptCommandData::Event { event_type, .. } => {
                    Some((command.start, *event_type))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            attack_events,
            vec![
                (2, event_type::CREATE_PROJECTILE),
                (2, event_type::CREATE_PROJECTILE),
                (56, event_type::SETUP_IDLE),
            ]
        );
        let hit_events = scripts[&0x0900_0029]
            .commands
            .iter()
            .filter_map(|command| match &command.data {
                UXGeoScriptCommandData::Event { event_type, .. } => {
                    Some((command.start, *event_type))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(hit_events, vec![(9, event_type::SETUP_IDLE)]);

        let database_path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d00_mons.edb");
        let database_file = File::open(&database_path).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        assert_eq!(database.file_for_runtime_selector(6, 6), Some(0x0100_003e));
    }

    #[test]
    fn real_robots_v248_ep05_turret_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/ep05_tur.edb");
        let file = File::open(&path).expect("open ep05_tur.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse ep05_tur.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse EP05 Turret collision")
            .expect("EP05 Turret MapCollision profile");
        let (tracks, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode EP05 Turret AnimMode bindings");
        let mut resolved_modes = tracks.keys().copied().collect::<Vec<_>>();
        resolved_modes.sort_unstable();
        println!("EP05_MODES={resolved_modes:08X?}");
        for mode in resolved_modes {
            let track = &tracks[&mode];
            let events = scripts
                .get(&mode)
                .map(|script| {
                    script
                        .commands
                        .iter()
                        .filter_map(|command| match &command.data {
                            UXGeoScriptCommandData::Event { event_type, .. } => {
                                Some((command.start, *event_type))
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            println!(
                "EP05_MODE=0x{mode:08X} animation=0x{:08X} animskin=0x{:08X} events={events:08X?}",
                track.animation, track.animskin
            );
        }

        let database_path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d00_mons.edb");
        let database_file = File::open(&database_path).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        assert_eq!(database.file_for_runtime_selector(6, 7), Some(0x0100_0035));
    }

    #[test]
    fn real_robots_v248_ep06_turret_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/ep06_tur.edb");
        let file = File::open(&path).expect("open ep06_tur.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse ep06_tur.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse EP06 Turret collision")
            .expect("EP06 Turret MapCollision profile");
        let (tracks, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode EP06 Turret AnimMode bindings");
        let mut resolved_modes = tracks.keys().copied().collect::<Vec<_>>();
        resolved_modes.sort_unstable();
        println!("EP06_MODES={resolved_modes:08X?}");
        for mode in resolved_modes {
            let track = &tracks[&mode];
            let events = scripts
                .get(&mode)
                .map(|script| {
                    script
                        .commands
                        .iter()
                        .filter_map(|command| match &command.data {
                            UXGeoScriptCommandData::Event { event_type, .. } => {
                                Some((command.start, *event_type))
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            println!(
                "EP06_MODE=0x{mode:08X} animation=0x{:08X} animskin=0x{:08X} events={events:08X?}",
                track.animation, track.animskin
            );
        }

        let database_path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d00_mons.edb");
        let database_file = File::open(&database_path).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        assert_eq!(database.file_for_runtime_selector(6, 8), Some(0x0100_003a));
    }

    #[test]
    fn real_robots_v248_eb06_shieldbot_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/eb06_shi.edb");
        let file = File::open(&path).expect("open eb06_shi.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse eb06_shi.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse EB06 ShieldBot collision")
            .expect("EB06 ShieldBot MapCollision profile");
        let (tracks, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode EB06 ShieldBot AnimMode bindings");
        for mode in [
            0x0900_0003,
            0x0900_0006,
            0x0900_0007,
            0x0900_0025,
            0x0900_0027,
            0x0900_002D,
            0x0900_0029,
            0x0900_002A,
            0x0900_0032,
            0x0900_0033,
            0x0900_0076,
            0x0900_0077,
            0x0900_0078,
            0x0900_007D,
            0x0900_00E9,
        ] {
            assert!(
                tracks.contains_key(&mode),
                "EB06 ShieldBot native builder/runtime mode 0x{mode:08X} must resolve"
            );
        }
        for mode in [0x0900_0025, 0x0900_0027] {
            let attack_script = scripts
                .get(&mode)
                .unwrap_or_else(|| panic!("EB06 ShieldBot attack AnimScript 0x{mode:08X}"));
            assert!(
                attack_script.commands.iter().any(|command| matches!(
                    &command.data,
                    UXGeoScriptCommandData::Event { event_type, .. }
                        if *event_type == event_type::SETUP_IDLE
                )),
                "ShieldBot attack mode 0x{mode:08X} requires SetupIdle completion"
            );
        }

        let database_path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d00_mons.edb");
        let database_file = File::open(&database_path).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        assert_eq!(database.file_for_runtime_selector(6, 3), Some(0x0100_002A));
    }

    #[test]
    fn real_robots_v248_eb07_minebot_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/eb07_min.edb");
        let file = File::open(&path).expect("open eb07_min.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse eb07_min.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse EB07 MineBot collision")
            .expect("EB07 MineBot MapCollision profile");
        let (tracks, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode EB07 MineBot AnimMode bindings");
        for mode in [
            0x0900_0003,
            0x0900_0006,
            0x0900_0007,
            0x0900_0025,
            0x0900_0029,
            0x0900_002A,
            0x0900_0032,
            0x0900_0033,
            0x0900_003E,
            0x0900_003F,
            0x0900_0040,
            0x0900_0076,
            0x0900_0077,
            0x0900_0078,
            0x0900_007D,
            0x0900_00E9,
        ] {
            assert!(
                tracks.contains_key(&mode),
                "EB07 MineBot native builder/runtime mode 0x{mode:08X} must resolve"
            );
        }

        for mode in [0x0900_0025, 0x0900_003E, 0x0900_003F, 0x0900_0040] {
            let events = scripts[&mode]
                .commands
                .iter()
                .filter_map(|command| match &command.data {
                    UXGeoScriptCommandData::Event { event_type, .. } => {
                        Some((command.start, *event_type))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let expected = match mode {
                0x0900_0025 => vec![
                    (23, event_type::CREATE_PROJECTILE),
                    (32, event_type::SETUP_IDLE),
                ],
                0x0900_003E => vec![(10, event_type::SETUP_IDLE)],
                0x0900_003F => vec![
                    (6, event_type::CREATE_PROJECTILE),
                    (14, event_type::SETUP_IDLE),
                ],
                0x0900_0040 => vec![(5, event_type::SETUP_IDLE)],
                _ => unreachable!(),
            };
            assert_eq!(events, expected, "EB07 mode 0x{mode:08X} event contract");
        }

        let database_path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d00_mons.edb");
        let database_file = File::open(&database_path).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        let eb07_row = database
            .rows_by_runtime_type
            .iter()
            .find_map(|(runtime_type, rows)| {
                rows.iter()
                    .position(|row| row.file == 0x0100_0030)
                    .map(|index| (*runtime_type, index))
            });
        assert_eq!(eb07_row, Some((6, 4)));
        assert_eq!(database.file_for_runtime_selector(6, 4), Some(0x0100_0030));
    }

    #[test]
    fn real_robots_v248_eb04_constructionbot_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/eb04_con.edb");
        let file = File::open(&path).expect("open eb04_con.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse eb04_con.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse EB04 ConstructionBot collision")
            .expect("EB04 ConstructionBot MapCollision profile");
        let (tracks, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode EB04 ConstructionBot AnimMode bindings");
        for mode in [
            0x0900_0003,
            0x0900_0006,
            0x0900_0007,
            0x0900_0025,
            0x0900_0027,
            0x0900_0029,
            0x0900_002A,
            0x0900_002D,
            0x0900_0032,
            0x0900_0033,
            0x0900_0035,
            0x0900_0036,
            0x0900_0037,
            0x0900_0076,
            0x0900_0077,
            0x0900_0078,
            0x0900_007D,
            0x0900_00E9,
        ] {
            assert!(
                tracks.contains_key(&mode),
                "EB04 ConstructionBot native builder/runtime mode 0x{mode:08X} must resolve"
            );
        }
        for mode in [0x0900_0025, 0x0900_0027] {
            let events = scripts[&mode]
                .commands
                .iter()
                .filter_map(|command| match &command.data {
                    UXGeoScriptCommandData::Event { event_type, .. } => {
                        let view = RobotsScriptEventView::from_command(command)?;
                        Some((command.start, *event_type, view.native_args::<5>()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let expected = match mode {
                0x0900_0025 => vec![
                    (
                        9,
                        event_type::HIT_CHECK,
                        [Some(0x1000_000C), Some(14.0f32.to_bits()), None, None, None],
                    ),
                    (23, event_type::SETUP_IDLE, [None; 5]),
                ],
                0x0900_0027 => vec![
                    (
                        0,
                        event_type::ATTACH_SWOOSH,
                        [Some(0x1900_0018), Some(u32::MAX), None, None, None],
                    ),
                    (
                        6,
                        event_type::HIT_CHECK,
                        [Some(0x1000_000C), Some(10.0f32.to_bits()), None, None, None],
                    ),
                    (21, event_type::SETUP_IDLE, [None; 5]),
                ],
                _ => unreachable!(),
            };
            assert_eq!(
                events, expected,
                "EB04 attack mode 0x{mode:08X} event contract"
            );
        }
        let database_path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d00_mons.edb");
        let database_file = File::open(&database_path).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        assert_eq!(database.file_for_runtime_selector(5, 4), Some(0x0100_0028));
        let flags = database
            .handler_flags_for_runtime_selector(5, 4)
            .expect("EB04 ConstructionBot MonsterDatabase row");
        assert_eq!(flags, 0);
        assert_eq!(flags & 0x4, 0, "EB04 does not use directional TurnOnSpot");
    }

    #[test]
    fn real_robots_v248_ef03_evilbot_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/ef03_evi.edb");
        let file = File::open(&path).expect("open ef03_evi.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse ef03_evi.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse EF03 EvilBot collision")
            .expect("EF03 EvilBot MapCollision profile");
        let (tracks, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode EF03 EvilBot AnimMode bindings");
        for mode in [
            0x0900_0003,
            0x0900_0006,
            0x0900_0007,
            0x0900_0025,
            0x0900_0027,
            0x0900_0028,
            0x0900_0029,
            0x0900_002A,
            0x0900_002D,
            0x0900_0032,
            0x0900_0033,
            0x0900_0076,
            0x0900_0077,
            0x0900_0078,
            0x0900_007D,
            0x0900_00E9,
        ] {
            assert!(
                tracks.contains_key(&mode),
                "EF03 EvilBot native builder/runtime mode 0x{mode:08X} must resolve"
            );
        }

        for mode in [0x0900_0025, 0x0900_0027] {
            let events = scripts[&mode]
                .commands
                .iter()
                .filter_map(|command| match &command.data {
                    UXGeoScriptCommandData::Event { event_type, .. } => {
                        let view = RobotsScriptEventView::from_command(command)?;
                        Some((command.start, *event_type, view.native_args::<5>()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let expected = match mode {
                0x0900_0025 => vec![
                    (
                        2,
                        event_type::ATTACH_SWOOSH,
                        [Some(0x1900_0018), Some(u32::MAX), None, None, None],
                    ),
                    (
                        2,
                        event_type::ATTACH_SWOOSH,
                        [Some(0x1900_0017), Some(u32::MAX), None, None, None],
                    ),
                    (
                        3,
                        event_type::HIT_CHECK,
                        [Some(0x1000_0009), Some(10.0f32.to_bits()), None, None, None],
                    ),
                    (30, event_type::SETUP_IDLE, [None; 5]),
                ],
                0x0900_0027 => vec![
                    (
                        6,
                        event_type::HIT_CHECK,
                        [Some(0x1000_0009), Some(12.0f32.to_bits()), None, None, None],
                    ),
                    (41, event_type::SETUP_IDLE, [None; 5]),
                ],
                _ => unreachable!(),
            };
            assert_eq!(
                events, expected,
                "EF03 attack mode 0x{mode:08X} event contract"
            );
        }

        let sounds = crate::sound_native::NativeSoundCatalog::load_pc_robots(Path::new(&game_root))
            .expect("load Robots native sound catalog");
        assert!(
            sounds.wave(0x1AF0_0155, 0).is_some(),
            "EF03 permanent SoundTag 0x1AF00155 must resolve to a playable native source"
        );

        let database_path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d00_mons.edb");
        let database_file = File::open(&database_path).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        assert_eq!(database.file_for_runtime_selector(5, 19), Some(0x0100_0057));
        let flags = database
            .handler_flags_for_runtime_selector(5, 19)
            .expect("EF03 EvilBot MonsterDatabase row");
        assert_eq!(flags, 0);
        assert_eq!(flags & 0x4, 0, "EF03 does not use directional TurnOnSpot");
    }

    #[test]
    fn real_robots_v248_ew11_fatbot_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/ew11_fat.edb");
        let file = File::open(&path).expect("open ew11_fat.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse ew11_fat.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse EW11 FatBot collision")
            .expect("EW11 FatBot MapCollision profile");
        let (tracks, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode EW11 FatBot AnimMode bindings");
        for mode in [
            0x0900_0003,
            0x0900_0006,
            0x0900_0007,
            0x0900_0025,
            0x0900_0029,
            0x0900_002A,
            0x0900_0032,
            0x0900_0033,
            0x0900_0076,
            0x0900_0077,
            0x0900_0078,
            0x0900_007D,
        ] {
            assert!(
                tracks.contains_key(&mode),
                "EW11 FatBot native builder mode 0x{mode:08X} must resolve"
            );
        }
        assert!(tracks.contains_key(&0x0900_0028));
        assert!(!tracks.contains_key(&0x0900_002D));
        assert!(!tracks.contains_key(&0x0900_00E9));

        let events = scripts[&0x0900_0025]
            .commands
            .iter()
            .filter_map(|command| match &command.data {
                UXGeoScriptCommandData::Event { event_type, .. } => {
                    let view = RobotsScriptEventView::from_command(command)?;
                    Some((command.start, *event_type, view.native_args::<5>()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            events,
            vec![
                (
                    11,
                    event_type::HIT_CHECK,
                    [Some(0x1000_0009), Some(20.0f32.to_bits()), None, None, None],
                ),
                (35, event_type::SETUP_IDLE, [None; 5]),
            ],
            "EW11 Attack25 event contract"
        );

        let database_path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d00_mons.edb");
        let database_file = File::open(&database_path).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        assert_eq!(database.file_for_runtime_selector(5, 18), Some(0x0100_005A));
        let flags = database
            .handler_flags_for_runtime_selector(5, 18)
            .expect("EW11 FatBot MonsterDatabase row");
        assert_eq!(flags, 0);
        assert_eq!(flags & 0x4, 0, "EW11 does not use directional TurnOnSpot");
    }

    #[test]
    fn real_robots_v248_eb16_knucklebot_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/eb16_knu.edb");
        let file = File::open(&path).expect("open eb16_knu.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse eb16_knu.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse EB16 KnuckleBot collision")
            .expect("EB16 KnuckleBot MapCollision profile");
        let (tracks, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode EB16 KnuckleBot AnimMode bindings");
        for mode in [
            0x0900_0003,
            0x0900_0006,
            0x0900_0007,
            0x0900_0025,
            0x0900_0027,
            0x0900_0029,
            0x0900_002A,
            0x0900_002D,
            0x0900_0032,
            0x0900_0033,
            0x0900_0076,
            0x0900_0077,
            0x0900_0078,
            0x0900_007D,
            0x0900_00E9,
        ] {
            assert!(
                tracks.contains_key(&mode),
                "EB16 KnuckleBot native builder mode 0x{mode:08X} must resolve"
            );
        }
        assert!(!tracks.contains_key(&0x0900_0035));
        assert!(!tracks.contains_key(&0x0900_0036));

        for mode in [0x0900_0025, 0x0900_0027] {
            let events = scripts[&mode]
                .commands
                .iter()
                .filter_map(|command| match &command.data {
                    UXGeoScriptCommandData::Event { event_type, .. } => {
                        let view = RobotsScriptEventView::from_command(command)?;
                        Some((command.start, *event_type, view.native_args::<5>()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let expected = match mode {
                0x0900_0025 => vec![
                    (
                        3,
                        event_type::ATTACH_SWOOSH,
                        [Some(0x1900_0017), Some(u32::MAX), None, None, None],
                    ),
                    (
                        7,
                        event_type::HIT_CHECK,
                        [Some(0x1000_000B), Some(4.0f32.to_bits()), None, None, None],
                    ),
                    (30, event_type::SETUP_IDLE, [None; 5]),
                ],
                0x0900_0027 => vec![
                    (
                        6,
                        event_type::ATTACH_SWOOSH,
                        [Some(0x1900_0017), Some(u32::MAX), None, None, None],
                    ),
                    (
                        6,
                        event_type::ATTACH_SWOOSH,
                        [Some(0x1900_0018), Some(u32::MAX), None, None, None],
                    ),
                    (
                        16,
                        event_type::HIT_CHECK,
                        [Some(0x1000_000B), Some(4.0f32.to_bits()), None, None, None],
                    ),
                    (
                        16,
                        event_type::HIT_CHECK,
                        [Some(0x1000_000C), Some(4.0f32.to_bits()), None, None, None],
                    ),
                    (43, event_type::SETUP_IDLE, [None; 5]),
                ],
                _ => unreachable!(),
            };
            assert_eq!(
                events, expected,
                "EB16 attack mode 0x{mode:08X} event contract"
            );
        }

        let database_path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d00_mons.edb");
        let database_file = File::open(&database_path).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        assert_eq!(database.file_for_runtime_selector(5, 17), Some(0x0100_005F));
        let flags = database
            .handler_flags_for_runtime_selector(5, 17)
            .expect("EB16 KnuckleBot MonsterDatabase row");
        assert_eq!(flags, 0);
        assert_eq!(flags & 0x4, 0, "EB16 does not use directional TurnOnSpot");
    }

    #[test]
    fn real_robots_v248_eb15_launcher_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/eb15_lau.edb");
        let file = File::open(&path).expect("open eb15_lau.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse eb15_lau.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse EB15 Launcher collision")
            .expect("EB15 Launcher MapCollision profile");
        let (tracks, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode EB15 Launcher AnimMode bindings");
        for mode in [
            0x0900_0003,
            0x0900_0006,
            0x0900_0007,
            0x0900_0025,
            0x0900_0027,
            0x0900_0029,
            0x0900_002A,
            0x0900_002D,
            0x0900_0032,
            0x0900_0033,
            0x0900_0076,
            0x0900_0077,
            0x0900_0078,
            0x0900_007D,
            0x0900_00E9,
        ] {
            assert!(
                tracks.contains_key(&mode),
                "EB15 Launcher native builder mode 0x{mode:08X} must resolve"
            );
        }
        assert!(!tracks.contains_key(&0x0900_0035));
        assert!(!tracks.contains_key(&0x0900_0036));

        for mode in [0x0900_0025, 0x0900_0027] {
            let events = scripts[&mode]
                .commands
                .iter()
                .filter_map(|command| match &command.data {
                    UXGeoScriptCommandData::Event { event_type, .. } => {
                        let view = RobotsScriptEventView::from_command(command)?;
                        Some((command.start, *event_type, view.native_args::<5>()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let expected = match mode {
                0x0900_0025 => vec![
                    (
                        22,
                        event_type::CREATE_PROJECTILE,
                        [
                            Some(0x5700_0002),
                            Some(0x1000_0011),
                            Some(0x1000_0017),
                            Some(0),
                            Some(0),
                        ],
                    ),
                    (60, event_type::SETUP_IDLE, [None; 5]),
                ],
                0x0900_0027 => vec![
                    (
                        1,
                        event_type::ATTACH_SWOOSH,
                        [Some(0x1900_0017), Some(u32::MAX), None, None, None],
                    ),
                    (
                        9,
                        event_type::HIT_CHECK,
                        [Some(0x1000_000B), Some(5.0f32.to_bits()), None, None, None],
                    ),
                    (40, event_type::SETUP_IDLE, [None; 5]),
                ],
                _ => unreachable!(),
            };
            assert_eq!(
                events, expected,
                "EB15 attack mode 0x{mode:08X} event contract"
            );
        }

        let database_path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d00_mons.edb");
        let database_file = File::open(&database_path).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        assert_eq!(database.file_for_runtime_selector(5, 16), Some(0x0100_0059));
        let flags = database
            .handler_flags_for_runtime_selector(5, 16)
            .expect("EB15 Launcher MonsterDatabase row");
        assert_eq!(flags, 0x0000_0001);
        assert_eq!(
            flags & 0x4,
            0,
            "EB15 uses direct yaw writes, not directional TurnOnSpot"
        );
    }

    #[test]
    fn real_robots_v248_eb13_knightbot_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/eb13_kni.edb");
        let file = File::open(&path).expect("open eb13_kni.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse eb13_kni.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse EB13 KnightBot collision")
            .expect("EB13 KnightBot MapCollision profile");
        let (tracks, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode EB13 KnightBot AnimMode bindings");
        assert_eq!(tracks[&0x0900_0027].animskin, collision.animskin);
        assert_ne!(tracks[&0x0900_003E].animskin, collision.animskin);
        assert_eq!(tracks[&0x0900_003E].animskin, tracks[&0x0900_003F].animskin);
        assert_eq!(tracks[&0x0900_003E].animskin, tracks[&0x0900_0040].animskin);
        for mode in [
            0x0900_0003,
            0x0900_0006,
            0x0900_0007,
            0x0900_0025,
            0x0900_0027,
            0x0900_0029,
            0x0900_002A,
            0x0900_002D,
            0x0900_0032,
            0x0900_0033,
            0x0900_0035,
            0x0900_0036,
            0x0900_0037,
            0x0900_003E,
            0x0900_003F,
            0x0900_0040,
            0x0900_0076,
            0x0900_0077,
            0x0900_0078,
            0x0900_007D,
            0x0900_0094,
            0x0900_0095,
            0x0900_0096,
        ] {
            assert!(
                tracks.contains_key(&mode),
                "EB13 shipped mode 0x{mode:08X} must resolve"
            );
        }

        for mode in [
            0x0900_0025,
            0x0900_0027,
            0x0900_0037,
            0x0900_003E,
            0x0900_003F,
            0x0900_0040,
            0x0900_0094,
            0x0900_0095,
            0x0900_0096,
        ] {
            let events = scripts[&mode]
                .commands
                .iter()
                .filter_map(|command| match &command.data {
                    UXGeoScriptCommandData::Event { event_type, .. } => {
                        let view = RobotsScriptEventView::from_command(command)?;
                        Some((command.start, *event_type, view.native_args::<5>()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let expected = match mode {
                0x0900_0025 => vec![
                    (
                        7,
                        event_type::ATTACH_SWOOSH,
                        [Some(0x1900_0017), Some(u32::MAX), None, None, None],
                    ),
                    (
                        14,
                        event_type::HIT_CHECK,
                        [Some(0x1000_000B), Some(8.0f32.to_bits()), None, None, None],
                    ),
                    (40, event_type::SETUP_IDLE, [None; 5]),
                ],
                0x0900_0027 => vec![
                    (
                        14,
                        event_type::CREATE_PROJECTILE,
                        [
                            Some(0x5700_000B),
                            Some(0x1000_0011),
                            Some(0x1000_0017),
                            Some(0),
                            Some(0),
                        ],
                    ),
                    (40, event_type::SETUP_IDLE, [None; 5]),
                ],
                0x0900_0037 => vec![
                    (
                        6,
                        event_type::ATTACH_SWOOSH,
                        [Some(0x1900_0017), Some(u32::MAX), None, None, None],
                    ),
                    (
                        15,
                        event_type::HIT_CHECK,
                        [Some(0x1000_000B), Some(8.0f32.to_bits()), None, None, None],
                    ),
                    (40, event_type::SETUP_IDLE, [None; 5]),
                ],
                0x0900_003E => vec![(7, event_type::SETUP_IDLE, [None; 5])],
                0x0900_003F => Vec::new(),
                0x0900_0040 => vec![(15, event_type::SETUP_IDLE, [None; 5])],
                0x0900_0094 => vec![(25, event_type::SETUP_IDLE, [None; 5])],
                0x0900_0095 => vec![(30, event_type::SETUP_IDLE, [None; 5])],
                0x0900_0096 => Vec::new(),
                _ => unreachable!(),
            };
            assert_eq!(
                events, expected,
                "EB13 shipped mode 0x{mode:08X} event contract"
            );
        }

        let database_path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d00_mons.edb");
        let database_file = File::open(&database_path).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        assert_eq!(database.file_for_runtime_selector(5, 15), Some(0x0100_004E));
    }

    #[test]
    fn real_robots_v248_jailbot_large_database_mapping_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/d00_mons.edb");
        let file = File::open(&path).expect("open d00_mons.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse d00_mons.edb");
        let database = RobotsCharacterDatabase::read(&mut edb).expect("MonsterDatabase failed");
        assert_eq!(database.file_for_trigger(74, &[Some(0)]), Some(0x0100_009C));
    }

    #[test]
    fn real_robots_v248_jailbot_large_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/eb17_jai.edb");
        let file = File::open(&path).expect("open eb17_jai.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse eb17_jai.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse JailBotLarge collision")
            .expect("JailBotLarge MapCollision profile");
        if let ProcessedCharacterCollisionShape::Capsule {
            half_segment,
            radius,
        } = collision.shape
        {
            assert!((half_segment - 1.25).abs() < 0.001);
            assert!((radius - 1.20).abs() < 0.001);
        } else {
            panic!("JailBotLarge MapCollision must be a capsule");
        }
        let (_, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode JailBotLarge AnimMode bindings");
        let events = scripts[&0x0900_0025]
            .commands
            .iter()
            .filter_map(|command| match &command.data {
                UXGeoScriptCommandData::Event { event_type, .. } => {
                    let view = RobotsScriptEventView::from_command(command)?;
                    Some((command.start, *event_type, view.native_args::<5>()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            events,
            vec![
                (
                    11,
                    event_type::ATTACH_SWOOSH,
                    [Some(0x1900_0018), Some(u32::MAX), None, None, None],
                ),
                (
                    12,
                    event_type::HIT_CHECK,
                    [Some(0x1000_000C), Some(6.0f32.to_bits()), None, None, None],
                ),
                (36, event_type::SETUP_IDLE, [None; 5]),
            ],
            "JailBotLarge mode25 event contract",
        );
    }

    #[test]
    fn real_robots_v248_jailbot_normal_family_animation_contract_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/eb03_jai.edb");
        let file = File::open(&path).expect("open eb03_jai.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse eb03_jai.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse JailBot collision")
            .expect("JailBot MapCollision profile");
        if let ProcessedCharacterCollisionShape::Capsule {
            half_segment,
            radius,
        } = collision.shape
        {
            assert!((half_segment - 0.628).abs() < 0.001);
            assert!((radius - 0.600).abs() < 0.001);
        } else {
            panic!("JailBotNormal MapCollision must be a capsule");
        }
        let (_, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("decode JailBot AnimMode bindings");

        for mode in [
            0x0900_0006,
            0x0900_0007,
            0x0900_0008,
            0x0900_0009,
            0x0900_0029,
            0x0900_002A,
            0x0900_0033,
            0x0900_0032,
            0x0900_002D,
            0x0900_0025,
            0x0900_0027,
            0x0900_0076,
            0x0900_0077,
            0x0900_0078,
            0x0900_007D,
            0x0900_00E9,
        ] {
            assert!(
                scripts.contains_key(&mode),
                "JailBot AnimMode 0x{mode:08X} must resolve"
            );
        }

        for mode in [0x0900_0025, 0x0900_0027] {
            let events = scripts[&mode]
                .commands
                .iter()
                .filter_map(|command| match &command.data {
                    UXGeoScriptCommandData::Event { event_type, .. } => {
                        let view = RobotsScriptEventView::from_command(command)?;
                        Some((command.start, *event_type, view.native_args::<5>()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let expected = match mode {
                0x0900_0025 => vec![
                    (
                        11,
                        event_type::ATTACH_SWOOSH,
                        [Some(0x1900_0018), Some(u32::MAX), None, None, None],
                    ),
                    (
                        12,
                        event_type::HIT_CHECK,
                        [Some(0x1000_000C), Some(6.0f32.to_bits()), None, None, None],
                    ),
                    (36, event_type::SETUP_IDLE, [None; 5]),
                ],
                0x0900_0027 => vec![
                    (
                        10,
                        event_type::ATTACH_SWOOSH,
                        [Some(0x1900_0018), Some(u32::MAX), None, None, None],
                    ),
                    (
                        18,
                        event_type::HIT_CHECK,
                        [Some(0x1000_000C), Some(6.0f32.to_bits()), None, None, None],
                    ),
                    (35, event_type::SETUP_IDLE, [None; 5]),
                ],
                _ => unreachable!(),
            };
            assert_eq!(
                events, expected,
                "JailBot attack 0x{mode:08X} event contract"
            );
        }
    }

    #[test]
    fn real_robots_v248_ew10_eb14_attack_events_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let root =
            Path::new(&game_root).join("_eurotools_out/extracted_main/robots/binary/_bin_pc");
        let mut path_cache = IntMap::default();
        for entry in std::fs::read_dir(&root).expect("could not scan Robots EDB folder") {
            let path = entry.expect("bad EDB directory entry").path();
            if path.extension().and_then(|value| value.to_str()) != Some("edb") {
                continue;
            }
            let file = File::open(&path).expect("could not open indexed EDB");
            let edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .expect("indexed EDB header did not parse");
            path_cache.insert(edb.header.hashcode, path);
        }

        for (label, file_uid) in [("EW10", 0x0100_004F), ("EB14", 0x0100_0056)] {
            let path = path_cache
                .get(&file_uid)
                .unwrap_or_else(|| panic!("{label} EDB 0x{file_uid:08X} was not indexed"));
            let file = File::open(path).unwrap_or_else(|_| panic!("open {label} EDB"));
            let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .unwrap_or_else(|_| panic!("parse {label} EDB"));
            let collision = preview_collision_profile(&mut edb)
                .unwrap_or_else(|_| panic!("parse {label} collision"))
                .unwrap_or_else(|| panic!("{label} MapCollision profile"));
            let (_, scripts) = preview_anim_mode_bindings(&mut edb, &collision)
                .unwrap_or_else(|_| panic!("decode {label} AnimMode bindings"));

            for mode in [0x0900_0025, 0x0900_0027] {
                let script = scripts
                    .get(&mode)
                    .unwrap_or_else(|| panic!("{label} AnimMode 0x{mode:08X} script"));
                let events = script
                    .commands
                    .iter()
                    .filter_map(|command| {
                        let view = RobotsScriptEventView::from_command(command)?;
                        Some((command.start, view.event_type, view.native_args::<5>()))
                    })
                    .collect::<Vec<_>>();
                eprintln!(
                    "{label} mode=0x{mode:08X} script=0x{:08X} events={events:?}",
                    script.hashcode
                );
                let expected = match (label, mode) {
                    ("EW10", 0x0900_0025) => vec![
                        (
                            1,
                            event_type::ATTACH_SWOOSH,
                            [Some(0x1900_0017), Some(u32::MAX), None, None, None],
                        ),
                        (
                            5,
                            event_type::HIT_CHECK,
                            [Some(0x1000_000B), Some(6.0f32.to_bits()), None, None, None],
                        ),
                        (
                            10,
                            event_type::ATTACH_SWOOSH,
                            [Some(0x1900_0018), Some(u32::MAX), None, None, None],
                        ),
                        (
                            13,
                            event_type::HIT_CHECK,
                            [Some(0x1000_000C), Some(15.0f32.to_bits()), None, None, None],
                        ),
                        (45, event_type::SETUP_IDLE, [None; 5]),
                    ],
                    ("EW10", 0x0900_0027) => vec![
                        (
                            3,
                            event_type::ATTACH_SWOOSH,
                            [Some(0x1900_0018), Some(u32::MAX), None, None, None],
                        ),
                        (
                            7,
                            event_type::HIT_CHECK,
                            [Some(0x1000_000C), Some(11.0f32.to_bits()), None, None, None],
                        ),
                        (
                            21,
                            event_type::ATTACH_SWOOSH,
                            [Some(0x1900_0017), Some(u32::MAX), None, None, None],
                        ),
                        (
                            27,
                            event_type::HIT_CHECK,
                            [Some(0x1000_000B), Some(9.0f32.to_bits()), None, None, None],
                        ),
                        (65, event_type::SETUP_IDLE, [None; 5]),
                    ],
                    ("EB14", 0x0900_0025) => vec![
                        (
                            4,
                            event_type::ATTACH_SWOOSH,
                            [Some(0x1900_0018), Some(u32::MAX), None, None, None],
                        ),
                        (
                            9,
                            event_type::HIT_CHECK,
                            [Some(0x1000_000C), Some(33.0f32.to_bits()), None, None, None],
                        ),
                        (55, event_type::SETUP_IDLE, [None; 5]),
                    ],
                    ("EB14", 0x0900_0027) => vec![
                        (
                            0,
                            event_type::ATTACH_SWOOSH,
                            [Some(0x1900_0017), Some(u32::MAX), None, None, None],
                        ),
                        (
                            8,
                            event_type::HIT_CHECK,
                            [Some(0x1000_000B), Some(6.0f32.to_bits()), None, None, None],
                        ),
                        (
                            14,
                            event_type::ATTACH_SWOOSH,
                            [Some(0x1900_0018), Some(u32::MAX), None, None, None],
                        ),
                        (
                            19,
                            event_type::HIT_CHECK,
                            [Some(0x1000_000C), Some(4.0f32.to_bits()), None, None, None],
                        ),
                        (62, event_type::SETUP_IDLE, [None; 5]),
                    ],
                    _ => unreachable!(),
                };
                assert_eq!(events, expected);
            }
        }
    }

    #[test]
    fn real_robots_v248_sweeper_rollerbot_collision_profile_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let root =
            Path::new(&game_root).join("_eurotools_out/extracted_main/robots/binary/_bin_pc");
        let mut path_cache = IntMap::default();
        for entry in std::fs::read_dir(&root).expect("could not scan Robots EDB folder") {
            let path = entry.expect("bad EDB directory entry").path();
            if path.extension().and_then(|value| value.to_str()) != Some("edb") {
                continue;
            }
            let file = File::open(&path).expect("could not open indexed EDB");
            let edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .expect("indexed EDB header did not parse");
            path_cache.insert(edb.header.hashcode, path.to_string_lossy().into_owned());
        }
        let database_file = File::open(root.join("d00_mons.edb")).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        let minebot_file = database
            .file_for_runtime_selector(5, 6)
            .expect("EQ02 selector6 MineBot file");
        assert_eq!(minebot_file, 0x0100_002F);
        let minebot_health = database
            .initial_health_for_runtime_selector(5, 6)
            .expect("EQ02 selector6 MineBot health");
        eprintln!("EQ02 MineBot initial health: {minebot_health}");
        assert_eq!(minebot_health, 3);
        assert_eq!(database.initial_health_for_runtime_selector(11, 0), None);

        let roller_file = database
            .file_for_runtime_selector(5, 9)
            .expect("Sweeper selector9 RollerBot file");
        assert_eq!(roller_file, 0x0100_0046);
        assert_eq!(
            database.pickup_drop_count_for_runtime_selector(5, 7),
            Some(0)
        );
        assert_eq!(
            database.pickup_drop_count_for_runtime_selector(5, 9),
            Some(0)
        );
        let roller_path = path_cache
            .get(&roller_file)
            .expect("selector9 RollerBot EDB was not indexed");
        let file = File::open(roller_path).expect("open selector9 RollerBot EDB");
        let mut roller_edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
            .expect("parse selector9 RollerBot EDB");
        let collision = preview_collision_profile(&mut roller_edb)
            .expect("parse selector9 RollerBot collision")
            .expect("selector9 RollerBot MapCollision profile");
        assert_eq!(collision.animskin, 0x0D00_0001);
        assert_eq!(collision.local_orientation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(collision.transform_selector, 18);
        assert!((collision.local_center.x - 0.0).abs() <= 1.0e-6);
        assert!((collision.local_center.y - 0.922_545_8).abs() <= 1.0e-6);
        assert!((collision.local_center.z - 0.0).abs() <= 1.0e-6);
        let super::ProcessedCharacterCollisionShape::Capsule {
            half_segment,
            radius,
        } = collision.shape
        else {
            panic!("selector9 RollerBot MapCollision must remain a capsule");
        };
        assert!((half_segment - 0.524_026_6).abs() <= 1.0e-6);
        assert!((radius - 0.4).abs() <= 1.0e-6);
        let owner_y_on_flat_floor = half_segment + radius - collision.local_center.y;
        assert!((owner_y_on_flat_floor - 0.001_480_8).abs() <= 1.0e-5);
    }

    #[test]
    fn real_robots_v248_eq02_minebot_move_root_motion_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/eq02_min.edb");
        let file = File::open(&path).expect("open eq02_min.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse eq02_min.edb");
        let collision = preview_collision_profile(&mut edb)
            .expect("decode EQ02 MineBot collision")
            .expect("EQ02 MineBot collision");
        let skin_header = edb
            .header
            .animskin_list
            .data()
            .iter()
            .find(|header| header.common.hashcode == collision.animskin)
            .cloned()
            .expect("EQ02 MineBot AnimSkin header");
        edb.seek(SeekFrom::Start(skin_header.common.address as u64))
            .expect("seek EQ02 MineBot AnimSkin");
        let skin = edb
            .read_type_args::<EXGeoBaseAnimSkin>(edb.endian, (248,))
            .expect("decode EQ02 MineBot AnimSkin");
        let missile_datum = skin
            .robots_animdatum_section
            .as_ref()
            .and_then(|section| section.find(0x1000_0011))
            .expect("EQ02 MineBot HT_AnimDatum_MissilePosition");
        eprintln!(
            "EQ02 MineBot MissilePosition selector={} local_center={:?} orientation={:?}; MapCollision selector={}",
            missile_datum.transform_selector,
            missile_datum.local_center,
            missile_datum.local_orientation,
            collision.transform_selector,
        );
        let (animation_modes, animation_mode_scripts) =
            preview_anim_mode_bindings(&mut edb, &collision)
                .expect("decode EQ02 MineBot AnimMode bindings");
        let anim_datums = preview_anim_datums(&mut edb, collision.animskin)
            .expect("decode EQ02 MineBot AnimDatums");
        let datum_tracks = preview_anim_mode_datum_tracks(
            &mut edb,
            &animation_modes,
            &animation_mode_scripts,
            &anim_datums,
        )
        .expect("decode EQ02 MineBot gameplay datum tracks");
        let missile_track = datum_tracks
            .get(&(0x0900_003F, 0x1000_0011))
            .expect("EQ02 MineBot Active/MissilePosition track");
        assert_eq!(missile_track.bone_chain.last().copied(), Some(21));
        let track = animation_modes
            .get(&ROBOTS_ANIM_MODE_MOVE)
            .expect("EQ02 MineBot Move track");
        for attack_mode in [0x0900_003E, 0x0900_003F, 0x0900_0040] {
            assert!(
                animation_modes.contains_key(&attack_mode),
                "EQ02 MineBot native Attack AnimMode 0x{attack_mode:08X} must resolve"
            );
            let attack_track = animation_modes
                .get(&attack_mode)
                .expect("MineBot Attack AnimMode track");
            let attack_total_translation = attack_track
                .root_motion_samples
                .first()
                .zip(attack_track.root_motion_samples.last())
                .map(|(first, last)| (last.position - first.position).length())
                .unwrap_or_default();
            let attack_max_rotation = attack_track
                .root_motion_samples
                .windows(2)
                .map(|samples| {
                    let dot = samples[0]
                        .rotation
                        .dot(samples[1].rotation)
                        .abs()
                        .clamp(0.0, 1.0);
                    2.0 * dot.acos()
                })
                .fold(0.0_f32, f32::max);
            let script = animation_mode_scripts
                .get(&attack_mode)
                .expect("MineBot Attack AnimMode must retain its native AnimScript");
            let events = script
                .commands
                .iter()
                .filter_map(|command| match &command.data {
                    UXGeoScriptCommandData::Event { event_type, .. } => {
                        let view = eurochef_shared::robots_runtime::events::RobotsScriptEventView::from_command(command)?;
                        Some((
                            command.start,
                            *event_type,
                            view.words::<5>(),
                            view.native_args::<4>(),
                        ))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let command_timeline = script
                .commands
                .iter()
                .map(|command| (command.opcode, command.start, command.length))
                .collect::<Vec<_>>();
            eprintln!(
                "EQ02 MineBot Attack mode=0x{attack_mode:08X} script=0x{:08X} fps={} len={} root_translation={attack_total_translation:.6} max_root_rot={attack_max_rotation:.9} commands={command_timeline:?} events={events:?}",
                script.hashcode, script.framerate, script.length
            );
        }
        for death_mode in [0x0900_0033, 0x0900_0032] {
            let death_track = animation_modes
                .get(&death_mode)
                .expect("MineBot HitDeath AnimMode track");
            let script = animation_mode_scripts
                .get(&death_mode)
                .expect("MineBot HitDeath AnimMode must retain its native AnimScript");
            let explosion_events = script
                .commands
                .iter()
                .filter_map(|command| match &command.data {
                    UXGeoScriptCommandData::Event { event_type, .. }
                        if *event_type == 0x1600_0039 =>
                    {
                        let view = eurochef_shared::robots_runtime::events::RobotsScriptEventView::from_command(command)?;
                        Some((command.start, view.words::<5>(), view.native_args::<4>()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let death_total_translation = death_track
                .root_motion_samples
                .first()
                .zip(death_track.root_motion_samples.last())
                .map(|(first, last)| (last.position - first.position).length())
                .unwrap_or_default();
            let death_max_rotation = death_track
                .root_motion_samples
                .windows(2)
                .map(|samples| {
                    let dot = samples[0]
                        .rotation
                        .dot(samples[1].rotation)
                        .abs()
                        .clamp(0.0, 1.0);
                    2.0 * dot.acos()
                })
                .fold(0.0_f32, f32::max);
            eprintln!(
                "EQ02 MineBot HitDeath mode=0x{death_mode:08X} animation=0x{:08X} frames={} rate={} root_translation={death_total_translation:.6} max_root_rot={death_max_rotation:.9} script=0x{:08X} fps={} len={} MonsterExplosion frames={explosion_events:?}",
                death_track.animation,
                death_track.frame_count,
                death_track.clip_rate,
                script.hashcode,
                script.framerate,
                script.length,
            );
            assert_eq!(explosion_events.len(), 1);
        }
        assert_eq!(track.root_motion_samples.len(), track.frame_count);
        let max_frame_translation = track
            .root_motion_samples
            .windows(2)
            .map(|samples| (samples[1].position - samples[0].position).length())
            .fold(0.0_f32, f32::max);
        let total_translation = track
            .root_motion_samples
            .first()
            .zip(track.root_motion_samples.last())
            .map(|(first, last)| (last.position - first.position).length())
            .unwrap_or_default();
        let max_frame_rotation_radians = track
            .root_motion_samples
            .windows(2)
            .map(|samples| {
                let dot = samples[0]
                    .rotation
                    .dot(samples[1].rotation)
                    .abs()
                    .clamp(0.0, 1.0);
                2.0 * dot.acos()
            })
            .fold(0.0_f32, f32::max);
        eprintln!(
            "EQ02 MineBot Move root-motion: animation=0x{:08X} frames={} rate={} transition_ticks={} max_frame={:.6} total={:.6} max_rot_rad={:.9}",
            track.animation,
            track.frame_count,
            track.clip_rate,
            track.transition_fixed_ticks,
            max_frame_translation,
            total_translation,
            max_frame_rotation_radians,
        );
        assert!(max_frame_translation > 1.0e-6);
    }

    #[test]
    fn real_robots_v248_eq02_minebot_attack_animscript_events_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let path = Path::new(&game_root)
            .join("_eurotools_out/extracted_main/robots/binary/_bin_pc/eq02_min.edb");
        let file = File::open(&path).expect("open eq02_min.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse eq02_min.edb");
        let endian = edb.endian;
        let modes = edb.header.animmode_list.data().clone();
        let sets = edb.header.animset_list.data().clone();
        let default_mode = modes
            .iter()
            .find(|mode| mode.common.hashcode == super::ROBOTS_ANIM_MODE_DEFAULT)
            .cloned()
            .expect("EQ02 Default AnimMode");

        for attack_mode in [0x0900_003E, 0x0900_003F, 0x0900_0040] {
            let target_mode_index = modes
                .iter()
                .position(|mode| mode.common.hashcode == attack_mode)
                .expect("EQ02 MineBot Attack AnimMode index");
            let transition = default_mode
                .read_robots_v248_transitions(&mut edb, endian)
                .expect("EQ02 MineBot Attack transitions")
                .into_iter()
                .find(|transition| transition.new_mode_index as usize == target_mode_index)
                .expect("Default -> MineBot Attack transition");
            assert_eq!(transition.controls.len(), 1);
            let control = &transition.controls[0];
            assert_eq!(control.opcode, 0x0C00_0002);
            let anim_set = sets
                .get(control.resource_key as usize)
                .expect("EQ02 MineBot Attack AnimSet");
            let groups = anim_set
                .read_robots_v248_groups(&mut edb, endian)
                .expect("EQ02 MineBot Attack groups");
            assert_eq!(groups.len(), 1);
            assert_eq!(groups[0].contributions.len(), 1);
            let resource = groups[0].contributions[0].resource_hashcode;
            let family = resource >> 24 & 0x7F;
            eprintln!(
                "EQ02 MineBot Attack mode 0x{attack_mode:08X}: resource=0x{resource:08X} family={family}"
            );
            if family != 4 {
                continue;
            }
            let header = edb
                .header
                .animscript_list
                .iter()
                .find(|header| header.hashcode == resource)
                .cloned()
                .expect("EQ02 MineBot Attack AnimScript header");
            let saved_internal_references = edb.internal_references.clone();
            let saved_external_references = edb.external_references.clone();
            let script = super::UXGeoScript::read(&header, &mut edb)
                .expect("EQ02 MineBot Attack AnimScript");
            edb.internal_references = saved_internal_references;
            edb.external_references = saved_external_references;
            for command in &script.commands {
                eprintln!(
                    "  cmd opcode={} start={} len={} data={:?}",
                    command.opcode, command.start, command.length, command.data
                );
            }
        }
    }

    #[test]
    fn real_robots_v248_sweeper_malfbot_hit_area_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let root =
            Path::new(&game_root).join("_eurotools_out/extracted_main/robots/binary/_bin_pc");
        let mut path_cache = IntMap::default();
        for entry in std::fs::read_dir(&root).expect("could not scan Robots EDB folder") {
            let path = entry.expect("bad EDB directory entry").path();
            if path.extension().and_then(|value| value.to_str()) != Some("edb") {
                continue;
            }
            let file = File::open(&path).expect("could not open indexed EDB");
            let edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .expect("indexed EDB header did not parse");
            path_cache.insert(edb.header.hashcode, path.to_string_lossy().into_owned());
        }
        let database_file = File::open(root.join("d00_mons.edb")).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        let malf_file = database
            .file_for_runtime_selector(5, 7)
            .expect("Sweeper selector7 MalfBot file");
        assert_eq!(malf_file, 0x0100_0039);
        let malf_path = path_cache
            .get(&malf_file)
            .expect("selector7 MalfBot EDB was not indexed");
        let file = File::open(malf_path).expect("open selector7 MalfBot EDB");
        let mut malf_edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
            .expect("parse selector7 MalfBot EDB");
        let headers = malf_edb.header.animskin_list.data().clone();
        let mut hit_areas = Vec::new();
        for header in headers {
            malf_edb
                .seek(SeekFrom::Start(header.common.address as u64))
                .expect("seek MalfBot AnimSkin");
            let skin = malf_edb
                .read_type_args::<EXGeoBaseAnimSkin>(malf_edb.endian, (248,))
                .expect("parse MalfBot AnimSkin");
            let Some(hit_area) = skin
                .robots_animdatum_section
                .as_ref()
                .and_then(|section| section.find(0x1000_0010))
            else {
                continue;
            };
            hit_areas.push((
                header.common.hashcode,
                hit_area.header.shape_mode,
                hit_area.shape_scalars,
                hit_area.local_center,
                hit_area.local_orientation,
                hit_area.transform_selector,
                hit_area.hierarchy_chain.clone(),
            ));
        }
        eprintln!("Sweeper selector7 MalfBot HitArea profiles: {hit_areas:?}");
        assert!(
            !hit_areas.is_empty(),
            "selector7 MalfBot searchable HitArea missing"
        );
    }

    #[test]
    fn real_robots_v248_sweeper_malfbot_move_root_motion_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let root =
            Path::new(&game_root).join("_eurotools_out/extracted_main/robots/binary/_bin_pc");
        let mut path_cache = IntMap::default();
        for entry in std::fs::read_dir(&root).expect("could not scan Robots EDB folder") {
            let path = entry.expect("bad EDB directory entry").path();
            if path.extension().and_then(|value| value.to_str()) != Some("edb") {
                continue;
            }
            let file = File::open(&path).expect("could not open indexed EDB");
            let edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .expect("indexed EDB header did not parse");
            path_cache.insert(edb.header.hashcode, path.to_string_lossy().into_owned());
        }
        let database_file = File::open(root.join("d00_mons.edb")).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        let malf_file = database
            .file_for_runtime_selector(5, 7)
            .expect("Sweeper selector7 MalfBot file");
        eprintln!(
            "MalfBot MonsterDatabase row: magnetic_mass={:?} pickup_drop_count={:?}",
            database.magnetic_mass_for_runtime_selector(5, 7),
            database.pickup_drop_count_for_runtime_selector(5, 7),
        );
        let malf_path = path_cache
            .get(&malf_file)
            .expect("selector7 MalfBot EDB was not indexed");
        let file = File::open(malf_path).expect("open selector7 MalfBot EDB");
        let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
            .expect("parse selector7 MalfBot EDB");
        let collision = preview_collision_profile(&mut edb)
            .expect("parse MalfBot collision")
            .expect("MalfBot MapCollision profile");
        let track = preview_move_animation_track(&mut edb, &collision)
            .expect("resolve MalfBot Default -> Move track")
            .expect("MalfBot Move track");
        let max_frame_translation = track
            .root_motion_samples
            .windows(2)
            .map(|samples| (samples[1].position - samples[0].position).length())
            .fold(0.0_f32, f32::max);
        let total_translation = track
            .root_motion_samples
            .first()
            .zip(track.root_motion_samples.last())
            .map(|(first, last)| (last.position - first.position).length())
            .unwrap_or_default();
        eprintln!(
            "MalfBot Move root-motion animation=0x{:08X} frames={} rate={} max_frame={max_frame_translation:.6} total={total_translation:.6}",
            track.animation, track.frame_count, track.clip_rate
        );
        assert_eq!(track.animation, 0x8300_000E);
        assert_eq!(track.clip_rate, 30);
        assert!(track.frame_count > 0);
        assert!(max_frame_translation > 1.0e-6);
        assert!(total_translation > 1.0e-6);

        let (status_tracks, status_scripts) = preview_anim_mode_bindings(&mut edb, &collision)
            .expect("resolve MalfBot status-hit AnimModes");
        for mode in [
            0x0900_0076,
            0x0900_0078,
            0x0900_0077,
            0x0900_007d,
            0x0900_00e9,
        ] {
            assert!(
                status_tracks.contains_key(&mode),
                "MalfBot status-hit AnimMode 0x{mode:08X} missing track"
            );
        }
        let magnetic_track = status_tracks
            .get(&0x0900_00e9)
            .expect("MalfBot MagneticHit track");
        let magnetic_total_translation = magnetic_track
            .root_motion_samples
            .first()
            .zip(magnetic_track.root_motion_samples.last())
            .map(|(first, last)| (last.position - first.position).length())
            .unwrap_or_default();
        eprintln!(
            "MalfBot MagneticHit root-motion animation=0x{:08X} frames={} rate={} total={magnetic_total_translation:.6}",
            magnetic_track.animation, magnetic_track.frame_count, magnetic_track.clip_rate
        );
        assert_eq!(magnetic_track.animation, 0x8300_000A);
        assert_eq!(magnetic_track.clip_rate, 30);
        assert_eq!(magnetic_track.frame_count, 41);
        assert!(magnetic_total_translation <= 1.0e-6);
        for mode in [0x0900_0076, 0x0900_0077, 0x0900_007d] {
            let script = status_scripts.get(&mode).unwrap_or_else(|| {
                panic!("MalfBot status-hit AnimMode 0x{mode:08X} missing script")
            });
            assert!(
                script.commands.iter().any(|command| {
                    RobotsScriptEventView::from_command(command)
                        .is_some_and(|event| event.event_type == event_type::SETUP_IDLE)
                }),
                "MalfBot status-hit AnimMode 0x{mode:08X} missing SetupIdle"
            );
        }
    }

    #[test]
    fn real_robots_npc_handler_flags_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let root =
            Path::new(&game_root).join("_eurotools_out/extracted_main/robots/binary/_bin_pc");
        let database_file = File::open(root.join("d00_mons.edb")).expect("open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("parse d00_mons.edb");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        let dogbot_flags = database
            .handler_flags_for_runtime_selector(5, 0)
            .expect("DogBot runtime5 selector0 flags");
        eprintln!("DogBot handler_flags_628=0x{dogbot_flags:08X}");
        let npc_rows = database
            .rows_by_runtime_type
            .get(&11)
            .expect("NPC MonsterDatabase sheet");
        assert_eq!(
            npc_rows
                .iter()
                .map(|row| row.handler_flags_628)
                .collect::<Vec<_>>(),
            vec![
                5, 5, 5, 5, 5, 1, 1, 5, 5, 5, 5, 5, 5, 5, 1, 1, 1, 1, 5, 5, 5, 5, 5, 1, 5, 5, 1, 1,
            ]
        );

        let npc_file = File::open(root.join("m98_npcs.edb")).expect("open m98_npcs.edb");
        let mut npc_edb = EdbFile::new(Box::new(BufReader::new(npc_file)), Platform::Pc)
            .expect("parse m98_npcs.edb");
        let maps = crate::maps::read_from_file(&mut npc_edb);
        let selectors = maps
            .iter()
            .flat_map(|map| map.triggers.iter())
            .filter(|trigger| trigger.ttype == 48)
            .map(|trigger| trigger.data[0].expect("NPC data0 selector"))
            .collect::<Vec<_>>();
        assert_eq!(
            selectors,
            vec![2, 1, 3, 5, 7, 6, 8, 9, 14, 15, 16, 11, 12, 13, 10, 8, 17, 18, 19, 20, 21]
        );
        let directional = selectors
            .iter()
            .filter(|selector| npc_rows[**selector as usize].handler_flags_628 & 0x4 != 0)
            .count();
        assert_eq!(directional, 15);
        assert_eq!(selectors.len() - directional, 6);
        let diner_trigger = maps
            .iter()
            .flat_map(|map| map.triggers.iter())
            .find(|trigger| {
                trigger.ttype == 48
                    && trigger.data[0] == Some(8)
                    && trigger.data[2].is_some_and(|flags| flags & 0x20 != 0)
            })
            .expect("m98 selector8 diner NPC with flag0x20");
        assert_eq!(diner_trigger.data[1], Some(0x0B00_0000));
        assert_eq!(diner_trigger.data[2], Some(0x22));

        let mut path_cache = IntMap::default();
        for entry in std::fs::read_dir(&root).expect("could not scan Robots EDB folder") {
            let path = entry.expect("bad EDB directory entry").path();
            if path.extension().and_then(|value| value.to_str()) != Some("edb") {
                continue;
            }
            let file = File::open(&path).expect("could not open indexed EDB");
            let edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .expect("indexed EDB header did not parse");
            path_cache.insert(edb.header.hashcode, path.to_string_lossy().into_owned());
        }
        let diner_file = database
            .file_for_runtime_selector(11, 8)
            .expect("NPC selector8 diner file");
        let file = File::open(
            path_cache
                .get(&diner_file)
                .expect("NPC selector8 diner EDB was not indexed"),
        )
        .expect("open NPC selector8 diner EDB");
        let diner_edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
            .expect("parse NPC selector8 diner EDB");
        let diner_modes = diner_edb
            .header
            .animmode_list
            .data()
            .iter()
            .map(|mode| mode.common.hashcode)
            .collect::<Vec<_>>();
        for mode in [0x0900_00EB, 0x0900_00EC, 0x0900_00ED] {
            assert!(
                diner_modes.contains(&mode),
                "selector8 missing diner AnimMode 0x{mode:08X}"
            );
        }

        let directional_file = database
            .file_for_runtime_selector(11, 2)
            .expect("NPC selector2 file");
        let file = File::open(
            path_cache
                .get(&directional_file)
                .expect("NPC selector2 EDB was not indexed"),
        )
        .expect("open NPC selector2 EDB");
        let mut character_edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
            .expect("parse NPC selector2 EDB");
        let collision = preview_collision_profile(&mut character_edb)
            .expect("parse NPC selector2 collision")
            .expect("NPC selector2 MapCollision profile");
        let turn_l = preview_turn_on_spot_l_animation_track(&mut character_edb, &collision)
            .expect("resolve TurnOnSpotL")
            .expect("NPC selector2 TurnOnSpotL track");
        let turn_r = preview_turn_on_spot_r_animation_track(&mut character_edb, &collision)
            .expect("resolve TurnOnSpotR")
            .expect("NPC selector2 TurnOnSpotR track");
        for (label, mode_hashcode, track) in [
            ("L", super::ROBOTS_ANIM_MODE_TURN_ON_SPOT_L, &turn_l),
            ("R", super::ROBOTS_ANIM_MODE_TURN_ON_SPOT_R, &turn_r),
        ] {
            let chain_len = track.bone_chain.len();
            let first = track.poses[0].rotation;
            let last = track.poses[(track.frame_count - 1) * chain_len].rotation;
            let delta = last * first.conjugate();
            let (_, skeletal_angle) = delta.to_axis_angle();
            assert!(skeletal_angle < 0.01);

            let endian = character_edb.endian;
            let modes = character_edb.header.animmode_list.data().clone();
            let sets = character_edb.header.animset_list.data().clone();
            let default_mode = modes
                .iter()
                .find(|mode| mode.common.hashcode == super::ROBOTS_ANIM_MODE_DEFAULT)
                .cloned()
                .expect("Default AnimMode");
            let target_mode_index = modes
                .iter()
                .position(|mode| mode.common.hashcode == mode_hashcode)
                .expect("TurnOnSpot AnimMode index");
            let transition = default_mode
                .read_robots_v248_transitions(&mut character_edb, endian)
                .expect("TurnOnSpot transitions")
                .into_iter()
                .find(|transition| transition.new_mode_index as usize == target_mode_index)
                .expect("Default -> TurnOnSpot transition");
            let control = transition.controls.first().expect("TurnOnSpot control");
            let anim_set = sets
                .get(control.resource_key as usize)
                .expect("TurnOnSpot AnimSet");
            let groups = anim_set
                .read_robots_v248_groups(&mut character_edb, endian)
                .expect("TurnOnSpot groups");
            let resource = groups[0].contributions[0].resource_hashcode;
            assert_eq!(
                resource >> 24 & 0x7f,
                4,
                "TurnOnSpot{label} must use AnimScript"
            );
            let header = character_edb
                .header
                .animscript_list
                .iter()
                .find(|header| header.hashcode == resource)
                .cloned()
                .expect("TurnOnSpot AnimScript header");
            let saved_internal_references = character_edb.internal_references.clone();
            let saved_external_references = character_edb.external_references.clone();
            let script = super::UXGeoScript::read(&header, &mut character_edb)
                .expect("TurnOnSpot AnimScript");
            character_edb.internal_references = saved_internal_references;
            character_edb.external_references = saved_external_references;
            let command = script
                .commands
                .iter()
                .find(|command| {
                    matches!(
                        command.data,
                        super::UXGeoScriptCommandData::Animation { .. }
                    )
                })
                .expect("TurnOnSpot Animation command");
            let controller = script
                .controllers
                .get(command.controller_header_index as usize)
                .expect("TurnOnSpot command controller");
            assert_eq!(controller.ctrl_mask, 0);
            assert_eq!(controller.ctrl_channel_mask, 0);
            assert!(controller.channels.vector_0.is_empty());
            assert!(controller.channels.quat_0.is_empty());

            let animation = character_edb
                .header
                .anim_list
                .data()
                .iter()
                .find(|animation| animation.common.hashcode == track.animation)
                .cloned()
                .expect("TurnOnSpot Animation header");
            let serialized = animation
                .read_robots_v248_exgeoanim(&mut character_edb, endian)
                .expect("TurnOnSpot EXGeoAnim");
            assert_eq!(serialized.raw_10, 0x1500);
            assert_eq!(track.transition_fixed_ticks, 5);
            assert_eq!(track.root_motion_samples.len(), track.frame_count);
            assert!(track.frame_count >= 2);
            let q0 = track.root_motion_samples[0].rotation;
            let mut q1 = track.root_motion_samples[1].rotation;
            if q0.dot(q1) < 0.0 {
                q1 = -q1;
            }
            let half_tick = q0.slerp(q1, 0.5);
            let first_tick_delta = (half_tick * q0.conjugate()).normalize();
            let first_tick_yaw = 2.0 * first_tick_delta.y.atan2(first_tick_delta.w);
            assert!(
                (0.04..0.05).contains(&first_tick_yaw.abs()),
                "TurnOnSpot{label} unexpected first fixed-tick yaw {first_tick_yaw}"
            );
            if label == "L" {
                assert!(first_tick_yaw < 0.0);
            } else {
                assert!(first_tick_yaw > 0.0);
            }
        }
    }

    #[test]
    fn real_m02_city_resolves_every_runtime_character_visual_when_requested() {
        let Ok(city_path) = std::env::var("EUROCHEF_REAL_M02_CITY_EDB") else {
            return;
        };
        let city_path = Path::new(&city_path);
        let root = city_path.parent().expect("m02_city fixture has no parent");

        let mut path_cache = IntMap::default();
        for entry in std::fs::read_dir(root).expect("could not scan Robots EDB folder") {
            let path = entry.expect("bad EDB directory entry").path();
            if path.extension().and_then(|value| value.to_str()) != Some("edb") {
                continue;
            }
            let file = File::open(&path).expect("could not open indexed EDB");
            let edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .expect("indexed EDB header did not parse");
            path_cache.insert(edb.header.hashcode, path.to_string_lossy().into_owned());
        }
        assert!(path_cache.contains_key(&ROBOTS_MONSTER_DATABASE_FILE));

        let database_file = File::open(
            path_cache
                .get(&ROBOTS_MONSTER_DATABASE_FILE)
                .expect("d00_mons.edb was not indexed"),
        )
        .expect("could not open d00_mons.edb");
        let mut database_edb = EdbFile::new(Box::new(BufReader::new(database_file)), Platform::Pc)
            .expect("d00_mons.edb did not parse");
        let database =
            RobotsCharacterDatabase::read(&mut database_edb).expect("MonsterDatabase failed");
        assert_eq!(database.file_for_trigger(10, &[Some(0)]), Some(0x0100_0007));
        assert_eq!(database.file_for_trigger(10, &[Some(1)]), Some(0x0100_001F));
        assert_eq!(database.file_for_trigger(10, &[Some(3)]), Some(0x0100_0027));
        assert_eq!(database.file_for_trigger(10, &[Some(6)]), Some(0x0100_002F));
        assert_eq!(database.file_for_trigger(10, &[Some(7)]), Some(0x0100_0039));
        assert_eq!(
            database.explosion_uids_for_runtime_selector(5, 10),
            (Some(0x5200_0006), Some(0x5200_002E))
        );
        assert_eq!(
            database.explosion_uids_for_runtime_selector(5, 11),
            (Some(0x5200_003A), Some(0x5200_002C))
        );
        assert_eq!(database.handler_flags_for_runtime_selector(5, 10), Some(0));
        assert_eq!(database.handler_flags_for_runtime_selector(5, 11), Some(0));
        assert_eq!(
            eurochef_shared::robots_runtime::hit_reaction::robots_monster_action_explosion_uid(
                database.handler_flags_for_runtime_selector(5, 11).unwrap(),
                0x5200_003A,
                0x5200_002C,
            ),
            0x5200_003A
        );
        assert_eq!(database.file_for_trigger(11, &[Some(1)]), Some(0x0100_0024));
        assert_eq!(database.file_for_trigger(18, &[Some(2)]), Some(0x0100_002E));
        assert_eq!(
            database.file_for_trigger(48, &[Some(10)]),
            Some(0x0100_0098)
        );
        assert_eq!(database.file_for_trigger(70, &[Some(0)]), Some(0x0100_0066));

        let city_file = File::open(city_path).expect("could not open m02_city.edb");
        let mut city_edb = EdbFile::new(Box::new(BufReader::new(city_file)), Platform::Pc)
            .expect("m02_city.edb did not parse");
        let mut maps = crate::maps::read_from_file(&mut city_edb);
        let resolved =
            resolve_robots_character_visuals(&mut city_edb, &mut maps, &path_cache, Platform::Pc)
                .expect("City character visuals did not resolve");
        assert_eq!(resolved, 77);

        let character_triggers = maps
            .iter()
            .flat_map(|map| &map.triggers)
            .filter(|trigger| robots_character_runtime_type(trigger.ttype).is_some())
            .collect::<Vec<_>>();
        assert_eq!(character_triggers.len(), 77);
        assert!(character_triggers
            .iter()
            .all(|trigger| trigger.character_visual.is_some()));
        let engine_visual_character_triggers = character_triggers
            .iter()
            .filter(|trigger| trigger.engine_options.visual_object.is_some())
            .count();
        eprintln!(
            "m02_city runtime characters with serialized engine visual_object: {engine_visual_character_triggers}/{}",
            character_triggers.len()
        );
        assert_eq!(
            engine_visual_character_triggers, 0,
            "AI character visuals must stay owned by the runtime character branch so live body poses are not shadowed by a serialized trigger visual"
        );
        let initial_animation_tracks = character_triggers
            .iter()
            .filter_map(|trigger| {
                trigger
                    .character_visual
                    .as_ref()
                    .and_then(|visual| visual.initial_animation.as_ref())
            })
            .collect::<Vec<_>>();
        assert_eq!(initial_animation_tracks.len(), 77);
        assert!(initial_animation_tracks
            .iter()
            .all(|track| track.animation == ROBOTS_CHARACTER_INITIAL_ANIMATION));
        let move_animation_tracks = character_triggers
            .iter()
            .filter_map(|trigger| {
                trigger
                    .character_visual
                    .as_ref()
                    .and_then(|visual| visual.move_animation.as_ref())
            })
            .collect::<Vec<_>>();
        let missing_move_files = character_triggers
            .iter()
            .filter_map(|trigger| trigger.character_visual.as_ref())
            .filter(|visual| visual.move_animation.is_none())
            .map(|visual| visual.file)
            .collect::<std::collections::BTreeSet<_>>();
        eprintln!(
            "m02_city runtime character Default->Move tracks: {}/{} missing_files={missing_move_files:08X?}",
            move_animation_tracks.len(),
            character_triggers.len()
        );
        assert_eq!(move_animation_tracks.len(), 65);
        assert_eq!(
            missing_move_files,
            std::collections::BTreeSet::from([0x0100_0024, 0x0100_0066])
        );
        let collision_profiles = character_triggers
            .iter()
            .filter(|trigger| {
                trigger
                    .character_visual
                    .as_ref()
                    .and_then(|visual| visual.collision)
                    .is_some()
            })
            .count();
        let missing_collision_files = character_triggers
            .iter()
            .filter_map(|trigger| trigger.character_visual.as_ref())
            .filter(|visual| visual.collision.is_none())
            .map(|visual| visual.file)
            .collect::<std::collections::BTreeSet<_>>();
        eprintln!(
            "m02_city runtime character native collision profiles: {collision_profiles}/{} missing_files={missing_collision_files:08X?}",
            character_triggers.len()
        );
        let hit_area_profiles = character_triggers
            .iter()
            .filter_map(|trigger| trigger.character_visual.as_ref())
            .filter(|visual| visual.hit_area.is_some())
            .count();
        let missing_hit_area_files = character_triggers
            .iter()
            .filter_map(|trigger| trigger.character_visual.as_ref())
            .filter(|visual| visual.hit_area.is_none())
            .map(|visual| visual.file)
            .collect::<std::collections::BTreeSet<_>>();
        let incompatible_hit_area_profiles = character_triggers
            .iter()
            .filter_map(|trigger| trigger.character_visual.as_ref())
            .filter_map(|visual| {
                let collision = visual.collision?;
                let hit_area = visual.hit_area?;
                (collision.animskin != hit_area.animskin
                    || collision.transform_selector != hit_area.transform_selector)
                    .then_some((
                        visual.file,
                        collision.animskin,
                        collision.transform_selector,
                        hit_area.animskin,
                        hit_area.transform_selector,
                    ))
            })
            .collect::<std::collections::BTreeSet<_>>();
        eprintln!(
            "m02_city runtime character HitArea profiles: {hit_area_profiles}/{} missing_files={missing_hit_area_files:08X?} incompatible={incompatible_hit_area_profiles:08X?}",
            character_triggers.len()
        );
        assert_eq!(hit_area_profiles, 73);
        assert_eq!(
            missing_hit_area_files,
            std::collections::BTreeSet::from([0x0100_0066])
        );
        assert_eq!(
            incompatible_hit_area_profiles,
            std::collections::BTreeSet::from([(0x0100_0021, 0x8D00_0001, 18, 0x8D00_0001, 2)])
        );
        let mut gameplay_bodies = character_triggers
            .iter()
            .filter_map(|trigger| {
                crate::map_runtime::RuntimeCharacterBodyState::from_trigger(trigger)
            })
            .collect::<Vec<_>>();
        assert_eq!(gameplay_bodies.len(), 77);
        assert!(gameplay_bodies.iter().all(|body| {
            body.registration_mask
                == crate::map_runtime::ROBOTS_GAMEPLAY_COLLISION_REGISTRATION_MASK
        }));
        let moved_by_real_move_track = gameplay_bodies.iter_mut().any(|body| {
            let Some(track) = body.move_animation.clone() else {
                return false;
            };
            let start = body.owner_position;
            let ticks = track.frame_count.saturating_mul(2).max(1);
            for tick in 0..ticks {
                let seconds = tick as f32 / 60.0;
                let Some(delta) = crate::map_runtime::runtime_character_track_root_motion_delta(
                    &track,
                    seconds,
                    1.0 / 60.0,
                    1.0,
                ) else {
                    continue;
                };
                if delta.native_translation.length_squared() <= 1.0e-12 {
                    continue;
                }
                body.apply_local_root_motion_delta(delta);
                return body.owner_position.distance_squared(start) > 1.0e-12;
            }
            false
        });
        assert!(
            moved_by_real_move_track,
            "at least one shipped m02_city AI body must project decoded Move root motion into its live owner position"
        );
        for body in &mut gameplay_bodies {
            body.initial_animation_seconds = 0.5;
        }
        let animated_bone_matrices = gameplay_bodies
            .iter()
            .filter(|body| body.initial_animation_bone_matrix() != Mat4::IDENTITY)
            .count();
        assert!(
            animated_bone_matrices > 0,
            "real m02_city initial Idle_Attack must reach at least one non-identity MapCollision bone matrix"
        );
        eprintln!(
            "m02_city runtime character initial animation: tracks=77 animated_bone_matrices@0.5s={animated_bone_matrices}"
        );
        assert!(character_triggers.iter().all(|trigger| {
            let visual = trigger.character_visual.as_ref().unwrap();
            city_edb
                .external_references
                .iter()
                .any(|(file, object)| *file == visual.file && *object == visual.script)
        }));
    }
}
