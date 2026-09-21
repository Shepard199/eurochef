use std::{
    collections::{BTreeMap, BTreeSet},
    io::Seek,
    sync::Arc,
};

use anyhow::Context;

use egui::mutex::{Mutex, RwLock};
use eurochef_edb::{
    binrw::BinReaderExt,
    edb::EdbFile,
    entity::{read_robots_v248_entity_anim_datums, EXGeoEntity, EXGeoMapZoneEntity},
    map::{
        EXGeoBaseDatum, EXGeoBspNode, EXGeoMap, EXGeoMapZone, EXGeoPlacement,
        EXGeoTriggerEngineOptions,
    },
    robots_ball_track::{
        read_robots_ball_track_pair, read_robots_ball_track_schedule, RobotsBallTrackPair,
        RobotsBallTrackSchedule, ROBOTS_HUB_TRACK_FILE,
    },
    robots_trigger_db::{
        read_robots_pattern_groups, RobotsPatternGroup, ROBOTS_TRIGGER_DATABASE_FILE,
    },
    versions::Platform,
    Hashcode, HashcodeUtils,
};
use eurochef_shared::{
    robots_runtime::{
        ai_character::RobotsAiHandlerClass,
        explosion::{
            read_robots_explosion_database, RobotsExplosionDatabase,
            ROBOTS_EXPLOSION_DATABASE_FILE_UID,
        },
        inventory::{
            read_robots_inventory_definitions, RobotsInventoryDefinition, ROBOTS_INVENTORY_FILE_UID,
        },
        mission::{
            read_robots_mission_definitions, RobotsMissionDefinition, ROBOTS_MISSIONS_FILE_UID,
        },
        npc_text::{read_robots_text_groups, RobotsTextGroupCatalog, ROBOTS_TEXT_FILE_UID},
        projectile::{
            read_robots_missile_database, RobotsMissileDatabase, ROBOTS_MISSILE_DATABASE_FILE_UID,
        },
        shop::{read_robots_shop_database, RobotsShopDatabase, ROBOTS_SHOP_FILE_UID},
    },
    script::{UXGeoScript, UXGeoScriptCommandData},
    IdentifiableResult,
};
use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use nohash_hasher::IntMap;

use crate::{
    entities::{
        ProcessedEntityMesh, ProcessedNavMesh, RobotsRaycastTriangle, RobotsSurfaceTriangle,
    },
    map_frame::MapFrame,
    map_zone::robots_map_zone_index_by_bsp,
    render::{entity::EntityRenderer, viewer::CameraType, NativeLightingTriangle, RenderStore},
    sound_preview::SharedSoundPreview,
};

mod dev_map;
mod entities;
mod triggers;

pub(crate) use dev_map::robots_dev_map_info;
pub use entities::{resolve_robots_character_visuals, robots_pickup_visual};
pub(crate) use entities::{
    robots_character_hit_query_raw_group, robots_character_runtime_type, RobotsCharacterDatabase,
    ROBOTS_ANIM_DATUM_SOLID_COLLISION, ROBOTS_ANIM_MODE_DEFAULT, ROBOTS_MONSTER_DATABASE_FILE,
};
#[cfg(test)]
pub(crate) use triggers::NativeSweeperBossTriggerEvent;
pub(crate) use triggers::{
    resolve_sweeper_boss_map_bindings, robots_fluid_initial_shared_rng_draw_count,
    robots_monster_transporter_route_distance_squared, robots_sweeper_boss_spawn_selection,
    robots_sweeper_boss_spawn_transform, robots_sweeper_ratchet_anchor,
    sweeper_health_pickup_player_contact_guaranteed_miss, NativeMonsterTransporterFixedStep,
    NativeMonsterTransporterPathEvent, NativeMonsterTransporterRuntime,
    NativeSweeperBossControllerSnapshot, NativeSweeperBossGenericAiBootstrap,
    NativeSweeperBossLiveAiSource, NativeSweeperBossMapBindings,
    NativeSweeperBossOwnedControllerPhaseInput, NativeSweeperBossOwnedXItemPhaseInput,
    NativeSweeperBossReplayRuntime, NativeSweeperBossSpawnSelection,
    NativeSweeperBossSpawnTransform, NativeSweeperBossTransporterEvent, RobotsSweeperBossPatterns,
    RobotsSweeperEyeScripts, RobotsSweeperRatchetScripts, ROBOTS_FLUID_TYPE,
    ROBOTS_SWEEPER_APPEAR_ANIM_MODE, ROBOTS_SWEEPER_ATTACK_ANIMATION,
    ROBOTS_SWEEPER_ATTACK_ANIM_MODE, ROBOTS_SWEEPER_ATTACK_ANIM_SET, ROBOTS_SWEEPER_ATTACK_SCRIPT,
    ROBOTS_SWEEPER_BOSS_ANIM_MODE, ROBOTS_SWEEPER_CONTROLLER_RUNTIME_CLASS_CODE,
    ROBOTS_SWEEPER_CONTROLLER_SAVE_SIZE, ROBOTS_SWEEPER_CONTROLLER_SERVICE_RADIUS,
    ROBOTS_SWEEPER_CONTROLLER_TYPE, ROBOTS_SWEEPER_EYE_COUNT,
    ROBOTS_SWEEPER_EYE_INITIAL_HIT_POINTS, ROBOTS_SWEEPER_EYE_OPEN_SECONDS,
    ROBOTS_SWEEPER_EYE_PRESSURE_THRESHOLD, ROBOTS_SWEEPER_EYE_TYPE,
    ROBOTS_SWEEPER_HEALTH_PICKUP_INVENTORY_ADD_EVENT, ROBOTS_SWEEPER_HEALTH_PICKUP_ITEM,
    ROBOTS_SWEEPER_HEALTH_PICKUP_LIFETIME_SECONDS, ROBOTS_SWEEPER_HEALTH_PICKUP_REGISTRATION_MASK,
    ROBOTS_SWEEPER_HEALTH_PICKUP_ROTATION_PER_UPDATE, ROBOTS_SWEEPER_HEALTH_PICKUP_SCRIPT,
    ROBOTS_SWEEPER_HEALTH_PICKUP_SPAWN_X_BIAS, ROBOTS_SWEEPER_HEALTH_PICKUP_SPAWN_X_SCALE,
    ROBOTS_SWEEPER_HEALTH_PICKUP_SPAWN_Z, ROBOTS_SWEEPER_HEALTH_PICKUP_TIMER_INITIAL_SECONDS,
    ROBOTS_SWEEPER_HEALTH_PICKUP_TIMER_JITTER_SECONDS,
    ROBOTS_SWEEPER_HEALTH_PICKUP_WAIT_FOR_HIT_EVENT, ROBOTS_SWEEPER_INITIAL_DIFFICULTY,
    ROBOTS_SWEEPER_JUMP_LEFT_ANIM_MODE, ROBOTS_SWEEPER_JUMP_RIGHT_ANIM_MODE,
    ROBOTS_SWEEPER_MAX_DIFFICULTY, ROBOTS_SWEEPER_MISSILE_EVENT,
    ROBOTS_SWEEPER_MISSILE_LAUNCH_BONE, ROBOTS_SWEEPER_MISSILE_LIVE_BONE_FRAME,
    ROBOTS_SWEEPER_MISSILE_RESOURCE_FILE, ROBOTS_SWEEPER_MISSILE_SCRIPT,
    ROBOTS_SWEEPER_MONSTER_CREATE_RESOURCE, ROBOTS_SWEEPER_MONSTER_PHYSICS_DESCRIPTOR,
    ROBOTS_SWEEPER_MONSTER_RUNTIME_TYPE, ROBOTS_SWEEPER_MONSTER_UPDATE_REGISTRATION_MASK,
    ROBOTS_SWEEPER_MONSTER_UPDATE_REGISTRATION_PRIORITY,
    ROBOTS_SWEEPER_MONSTER_XITEM_REGISTRATION_FLAGS, ROBOTS_SWEEPER_PATTERN_FILE,
    ROBOTS_SWEEPER_PATTERN_ROW_COUNT, ROBOTS_SWEEPER_RAT_ARRIVAL_YAW,
    ROBOTS_SWEEPER_RAT_BASE_SCRIPT, ROBOTS_SWEEPER_RAT_DAMAGE_PHASE_DIFFICULTY,
    ROBOTS_SWEEPER_RAT_DEATH_ANIM_MODE, ROBOTS_SWEEPER_RAT_DEATH_ANIM_SET,
    ROBOTS_SWEEPER_RAT_DEATH_SCRIPT, ROBOTS_SWEEPER_RAT_DEATH_SIGNAL_FRAME,
    ROBOTS_SWEEPER_RAT_ENTITY, ROBOTS_SWEEPER_RAT_FACE_PLAYER_ALPHA,
    ROBOTS_SWEEPER_RAT_HIT_BACK_ANIM_MODE, ROBOTS_SWEEPER_RAT_HIT_BACK_ANIM_SET,
    ROBOTS_SWEEPER_RAT_HIT_BACK_SCRIPT, ROBOTS_SWEEPER_RAT_HIT_BACK_SIGNAL_FRAME,
    ROBOTS_SWEEPER_RAT_HIT_FORWARD_ANIM_MODE, ROBOTS_SWEEPER_RAT_HIT_FORWARD_ANIM_SET,
    ROBOTS_SWEEPER_RAT_HIT_FORWARD_SCRIPT, ROBOTS_SWEEPER_RAT_HIT_FORWARD_SIGNAL_FRAME,
    ROBOTS_SWEEPER_RAT_HIT_FORWARD_THRESHOLD, ROBOTS_SWEEPER_RAT_INITIAL_HIT_POINTS,
    ROBOTS_SWEEPER_RAT_JUMP_LEFT_YAW, ROBOTS_SWEEPER_RAT_JUMP_RIGHT_YAW,
    ROBOTS_SWEEPER_RAT_POSITION_DATUM, ROBOTS_SWEEPER_RAT_RESOURCE_FILE,
    ROBOTS_SWEEPER_RAT_SCRIPT_VALUE_EVENT, ROBOTS_SWEEPER_RAT_SCRIPT_VALUE_SIGNAL_1,
    ROBOTS_SWEEPER_RAT_SCRIPT_VALUE_SIGNAL_2,
};
pub use triggers::{
    robots_camera_controller_plan, robots_camera_flags, robots_camera_marker_scaled_data0,
    robots_camera_mode, robots_camera_scaled_data4, robots_camera_scaled_data5,
    robots_camera_viewport_runtime, robots_direct_object_audio_profile,
    robots_monster_data15_value, robots_monster_data4_value, robots_monster_flags,
    robots_monster_is_family, robots_monster_proximity_radius, robots_monster_runtime_selector,
    robots_monster_test_runtime_value, robots_monster_transporter_path_speed,
    robots_monster_transporter_secondary_path_hash, robots_npc_alternate_cutscenes,
    robots_npc_cutscene_is_null, robots_npc_flags, robots_npc_runtime_selector,
    robots_npc_runtime_uid, robots_npc_text_group, robots_object_audio_is_consumer,
    robots_object_audio_is_enabled, robots_object_audio_profile_for_source,
    robots_trigger_path_data_slot, robots_trigger_path_hash, robots_trigger_path_is_proven,
    robots_trigger_platform_angular_velocity, robots_trigger_runtime_path_acceleration,
    robots_trigger_runtime_path_speed, robots_watchbot_enter_distance, robots_watchbot_flags,
    robots_watchbot_leave_distance, robots_watchbot_mode, NativeCameraFadeTransitionRuntime,
    NativeCameraOwnershipRuntime, NativeCameraSequenceRuntime, NativeCameraShakeRuntime,
    NativeCameraViewportPose, NativeCameraViewportRuntime, NativeDefaultPlayerCameraRuntime,
    ObjectAudioProfile, NATIVE_FADE_HIGH_THRESHOLD, NATIVE_FADE_IN_STATE,
    NATIVE_FADE_LOW_THRESHOLD, NATIVE_FADE_OUT_STATE,
};

const ROBOTS_SWEEPER_TRANSPORTER_RESOURCE_FILE: u32 = 0x0100_007B;
const ROBOTS_SWEEPER_TRANSPORTER_ENTITY: u32 = 0x0200_0039;
const ROBOTS_SWEEPER_TRANSPORTER_SPAWN_DATUM: u32 = 0x1000_0011;

pub struct MapViewerPanel {
    maps: Vec<ProcessedMap>,

    // TODO(cohae): Replace so we can do funky stuff
    frame: MapFrame,
}

#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct ProcessedMap {
    pub hashcode: u32,
    pub bsp_nodes: Vec<EXGeoBspNode>,
    pub mapzone_entities: Vec<EXGeoMapZoneEntity>,
    pub zones: Vec<EXGeoMapZone>,
    pub skies: Vec<Hashcode>,
    pub placements: Vec<EXGeoPlacement>,
    pub placement_group_count: usize,
    pub cameras: Vec<ProcessedCamera>,
    pub portals: Vec<ProcessedPortal>,
    pub isounds: Vec<u16>,
    pub lights: Vec<ProcessedLight>,
    pub sounds: Vec<ProcessedSound>,
    pub lighting_triangles: Vec<NativeLightingTriangle>,
    /// Robots v248 native per-face surface metadata aggregated from each MapZone ref mesh.
    pub zone_surface_mask_counts: Vec<BTreeMap<u16, usize>>,
    /// Exact nonzero native surface triangles for each MapZone ref mesh.
    pub zone_surface_triangles: Vec<Vec<RobotsSurfaceTriangle>>,
    /// Complete native `EXGeoEntity::DoRayCast` face stream for each MapZone root entity.
    pub zone_raycast_triangles: Vec<Option<Vec<RobotsRaycastTriangle>>>,
    /// Native 0x607 navigation topology reached through each MapZone ref-entity.
    /// A zone may expose multiple NavMesh children through a Split hierarchy.
    pub zone_navmeshes: Vec<Vec<ProcessedNavMesh>>,
    /// World-space native raycast faces for each serialized placement. `None`
    /// means the referenced entity could not be resolved; `Some(empty)` means
    /// it resolved and genuinely has no native raycast faces.
    pub placement_raycast_triangles: Vec<Option<Vec<RobotsRaycastTriangle>>>,
    pub paths: Vec<ProcessedPath>,
    pub triggers: Vec<ProcessedTrigger>,
    /// Runtime-selector visual catalog for AI XItems that are created dynamically
    /// and therefore have no serialized trigger record to own `character_visual`.
    /// Key is `(MonsterDatabase runtime_type, config_index)` and the payload is
    /// exactly the same decoded AnimSkin/AnimMode/HitArea contract used by
    /// serialized Monster/NPC/Fish triggers.
    pub runtime_character_visuals: BTreeMap<(u32, u32), ProcessedCharacterVisual>,
    /// Per-trigger process-global RNG draws consumed by a successfully validated native Fluid setup.
    /// `None` means either non-Fluid or the class-specific mesh/grid setup could not be proven.
    pub fluid_initial_shared_rng_draws: Vec<Option<u32>>,
    /// True only when shipped Sweeper data proves no serialized/static source can create a
    /// candidate for the native post-XItem Pickup ordinal tail (`0x00444E0A`). Dynamic runtime
    /// producers are checked separately each fixed frame.
    pub sweeper_post_xitem_pickup_static_zero: bool,
    pub trigger_collisions: Vec<EXGeoBaseDatum>,
    /// Global Robots D02 trigger-database PatternGroup resources used by XTrigger_Pattern.
    pub pattern_groups: IntMap<Hashcode, RobotsPatternGroup>,
    /// Native T00 HubTrack spreadsheet pairs requested by serialized XTrigger_BallTrack records.
    pub ball_track_pairs: IntMap<u32, RobotsBallTrackPair>,
    /// Native T00 HubTrack alternate-scheduler sheets (`0x1400000E`) keyed by
    /// serialized BallTrack data[3]. Only data[1] > 0 triggers request these rows.
    pub ball_track_schedules: IntMap<u32, RobotsBallTrackSchedule>,
    /// Immutable D03_Missions / HT_SpreadSheet_Missions records used by Mission/NPC runtime.
    pub mission_definitions: Vec<RobotsMissionDefinition>,
    /// Immutable O01_PickUps / HT_SpreadSheet_Inventory records backing family-0x47 progress.
    pub inventory_definitions: Vec<RobotsInventoryDefinition>,
    /// Immutable D04_Missiles / HT_SpreadSheet_Missiles catalog used by the common
    /// AI/player projectile producer at native `0x004DFEB0`.
    pub missile_database: Option<RobotsMissileDatabase>,
    /// Immutable FX03_Explosion / spreadsheet 0x1400000A catalog used by
    /// `0x004DC510 -> 0x004DC6A0 -> XItemHandler_Explosion`.
    pub explosion_database: Option<RobotsExplosionDatabase>,
    /// Rodney's native candidate-side HT_AnimDatum_HitArea from P01_Rodney.
    /// Player Handler ownership remains separate from AI-character bodies.
    pub player_hit_area: Option<ProcessedCharacterCollisionProfile>,
    /// Rodney HT_AnimDatum_SolidCollision (0x10000001), consumed by the native
    /// global XItem collision pair pass before Handler vslot +0x64 callbacks.
    pub player_solid_collision: Option<ProcessedCharacterCollisionProfile>,
    /// Immutable H05_Shop / HT_SpreadSheet_Shops catalog used by the native Shop host.
    pub shop_database: Option<RobotsShopDatabase>,
    /// Immutable D01_Text / HT_SpreadSheet_TextGroups catalog used by native
    /// XTextManager group selection for NPC/simple-message presentation.
    pub text_groups: RobotsTextGroupCatalog,
    /// Native Bo5_Final Sweeper-boss 70×5 monster spawn pattern spreadsheet.
    pub sweeper_boss_patterns: Option<RobotsSweeperBossPatterns>,
    /// Standalone Eye ScriptValue profiles from FinalBoss.edb (0x010000BC).
    pub sweeper_eye_scripts: Option<RobotsSweeperEyeScripts>,
    /// Relevant nb11_rat Script/Event profiles used by the native Ratchet fixed-tick reducer.
    pub sweeper_ratchet_scripts: Option<RobotsSweeperRatchetScripts>,
    /// Static HT_AnimDatum_RatchetPosition local center from HT_Entity_Sweeper in Bo5_Final.
    pub sweeper_ratchet_position_local: Option<[f32; 3]>,
    /// Exact previous-live HT_AnimBone_R_Hand point sampled from the native Attack animation at frame 16.5.
    pub sweeper_ratchet_missile_hand_local: Option<[f32; 3]>,
    /// Static ef02_dro HT_AnimDatum_MissilePosition used by native MonsterTransporter 0x00469450
    /// as the spawned monster's initial world position before its own Handler takes ownership.
    pub sweeper_transporter_spawn_position_local: Option<[f32; 3]>,
}

impl ProcessedMap {
    pub fn native_zone_index(&self, point: Vec3) -> Option<usize> {
        robots_map_zone_index_by_bsp(&self.bsp_nodes, self.zones.len(), point)
    }

    /// Mirrors the dynamic-light zone broad phase at Robots.exe 0x00523105 ->
    /// 0x0055475E. The containing BSP zone is always first. Ordinary lights may
    /// additionally spill into directly connected portal neighbours when the
    /// light-to-portal geometric distance is below the serialized radius; mode
    /// bit 1 keeps the light in the containing zone only.
    pub fn native_dynamic_light_zone_indices(
        &self,
        point: Vec3,
        radius: f32,
        containing_zone_only: bool,
    ) -> Vec<usize> {
        let Some(root_zone) = self.native_zone_index(point) else {
            return Vec::new();
        };
        let mut zones = vec![root_zone];
        if containing_zone_only || !radius.is_finite() || radius <= 0.0 {
            return zones;
        }

        for portal in &self.portals {
            let Some(neighbour) = robots_portal_neighbor_zone(portal, root_zone, self.zones.len())
            else {
                continue;
            };
            if robots_portal_distance_to_point(portal, point) < radius
                && !zones.contains(&neighbour)
            {
                zones.push(neighbour);
            }
        }
        zones
    }

    /// Reproduces the local-map streaming request traversal used by
    /// Robots.exe 0x004EEAB3. This is the transient +0x6E request set, not the
    /// renderer's +0x6A activated-resource state and not the visual portal list
    /// consumed by 0x004EC2AA.
    pub fn native_streaming_request_zone_indices(&self, point: Vec3) -> Vec<usize> {
        let Some(root_zone) = self.native_zone_index(point) else {
            return Vec::new();
        };

        let mut requested = vec![false; self.zones.len()];
        self.mark_native_streaming_zone(root_zone, 0, &mut requested);
        requested
            .into_iter()
            .enumerate()
            .filter_map(|(index, active)| active.then_some(index))
            .collect()
    }

    fn mark_native_streaming_zone(&self, zone_index: usize, depth: usize, requested: &mut [bool]) {
        // 0x004EEAB3 returns before marking when param_3 >= 4, so the root and
        // at most three portal transitions are included.
        if depth >= 4 || zone_index >= self.zones.len() {
            return;
        }
        requested[zone_index] = true;

        let Some(portal_infos) = self.zones[zone_index].unk18.as_ref() else {
            return;
        };
        let portal_infos = portal_infos.data();
        let mut group_index = 0usize;
        while group_index < portal_infos.len() {
            let info = &portal_infos[group_index];
            let next_group =
                group_index.saturating_add(1usize.saturating_add(info.portal_count as usize));

            // Native uses 0xFFFF as the external/no-local-target sentinel.
            if info.map_to != u16::MAX {
                let target_zone = (info.map_to & 0x00ff) as usize;
                let portal_index = usize::try_from(info.index).ok();
                let portal_is_open = portal_index
                    .and_then(|index| self.portals.get(index))
                    .is_some_and(|portal| portal.flags & 1 == 0);

                if portal_is_open && target_zone < requested.len() && !requested[target_zone] {
                    // 0x004EEAB3 is depth-first and marks before recursion.
                    self.mark_native_streaming_zone(target_zone, depth + 1, requested);
                }
            }

            if next_group <= group_index {
                break;
            }
            group_index = next_group;
        }
    }

    /// Builds the visual MapZone list that feeds render record 0 at +0x20.
    /// Robots.exe 0x004ED203 inserts the BSP root first; 0x004ED920 then walks
    /// visible portal groups, permits shallower/equal-depth revisits, reparents
    /// the embedded runtime-zone node, keeps sibling order by portal depth and
    /// recurses while the next depth is < 16. 0x004EDDDB(..., 0) later flattens
    /// that final tree into the +0x28/+0x2C array consumed by 0x004EC2AA.
    #[cfg(test)]
    pub fn native_visual_zone_indices(&self, point: Vec3, view_projection: Mat4) -> Vec<usize> {
        self.native_visual_zone_frame(point, view_projection)
            .ordered
    }

    pub(crate) fn native_visual_zone_frame(
        &self,
        point: Vec3,
        view_projection: Mat4,
    ) -> NativeVisualZoneFrame {
        let Some(root_zone) = self.native_zone_index(point) else {
            return NativeVisualZoneFrame::empty(self.zones.len());
        };

        // 0x004ED203 primes runtime bitset +0x1F4 from the BSP root zone's
        // serialized +0x40 mask through 0x0053BC13(..., 1), which also sets
        // runtime state +0x214 to 2. Every recursive 0x004ED920 entry calls
        // 0x0053BC13(current_zone_mask, 0) again, but state 2 makes that branch
        // a no-op. The entire traversal therefore keeps the BSP-root mask.
        let root_exclusion_mask = self.zones[root_zone].visual_zone_exclusion_mask;
        let mut traversal = NativeVisualTraversal::new(self.zones.len());
        self.collect_native_visual_zone(
            root_zone,
            1,
            NativePortalClip::FULL,
            view_projection,
            &root_exclusion_mask,
            &mut traversal,
        );

        let mut ordered = Vec::new();
        traversal.flatten(root_zone, &mut ordered);
        NativeVisualZoneFrame {
            ordered,
            frame_depths: traversal.frame_depths,
            blocked_depth_writes: traversal.blocked_depth_writes,
        }
    }

    fn collect_native_visual_zone(
        &self,
        zone_index: usize,
        depth: usize,
        parent_clip: NativePortalClip,
        view_projection: Mat4,
        root_exclusion_mask: &[u32; 8],
        traversal: &mut NativeVisualTraversal,
    ) {
        if depth >= 16 || zone_index >= self.zones.len() {
            return;
        }
        traversal.frame_depths[zone_index] = depth as i8;
        traversal.clips[zone_index] = parent_clip;

        let Some(portal_infos) = self.zones[zone_index].unk18.as_ref() else {
            return;
        };
        let portal_infos = portal_infos.data();
        let mut group_index = 0usize;
        while group_index < portal_infos.len() {
            let leader = &portal_infos[group_index];
            let group_len = 1usize.saturating_add(leader.portal_count as usize);
            let next_group = group_index.saturating_add(group_len);

            if leader.map_to != u16::MAX {
                // 0x004ED920 treats non-FFFF map_to as a direct local-zone
                // index. Before touching that runtime zone it tests the bit in
                // the root zone's serialized +0x40 256-bit exclusion mask.
                let target_zone = leader.map_to as usize;
                let target_excluded = target_zone < 256
                    && (root_exclusion_mask[target_zone / 32] & (1u32 << (target_zone & 31))) != 0;
                if target_excluded {
                    group_index = next_group;
                    continue;
                }
                if target_zone < self.zones.len() {
                    let next_depth = depth.saturating_add(1);
                    let previous_frame_depth = traversal.frame_depths[target_zone];
                    let skip_open_portals = previous_frame_depth != 0
                        && previous_frame_depth.unsigned_abs() < next_depth as u8;
                    let mut group_clip: Option<NativePortalClip> = None;

                    for info in portal_infos
                        .iter()
                        .skip(group_index)
                        .take(group_len.min(portal_infos.len() - group_index))
                    {
                        let Some(portal_index) = usize::try_from(info.index).ok() else {
                            continue;
                        };
                        let Some(portal) = self.portals.get(portal_index) else {
                            continue;
                        };

                        if portal.flags & 1 == 0 {
                            if skip_open_portals {
                                continue;
                            }
                            let Some(clip) = native_portal_clip(
                                portal,
                                info.flipped,
                                parent_clip,
                                view_projection,
                            ) else {
                                continue;
                            };
                            group_clip = Some(match group_clip {
                                Some(existing) => existing.union(clip),
                                None => clip,
                            });
                        } else if previous_frame_depth == 0 {
                            // 0x004ED920 writes -(next depth) directly into the
                            // persistent +0x6B byte when a disabled portal sees
                            // a target without a transient +0x6D depth.
                            traversal.blocked_depth_writes[target_zone] = Some(-(next_depth as i8));
                            if let Some(clip) = native_portal_clip(
                                portal,
                                info.flipped,
                                parent_clip,
                                view_projection,
                            ) {
                                traversal.clips[target_zone] = clip;
                            }
                        } else if previous_frame_depth > 0 {
                            // Native merges only the four projected screen bounds
                            // here; the target depth interval remains unchanged.
                            traversal.clips[target_zone] =
                                traversal.clips[target_zone].union_screen(parent_clip);
                        }
                    }

                    if let Some(clip) = group_clip {
                        traversal.reparent_sorted(zone_index, target_zone, clip);
                        // A depth-16 child is inserted into the tree but native
                        // does not recurse into it, so it can appear in the final
                        // render list without receiving a +0x6D depth value.
                        if next_depth < 16 {
                            self.collect_native_visual_zone(
                                target_zone,
                                next_depth,
                                clip,
                                view_projection,
                                root_exclusion_mask,
                                traversal,
                            );
                        }
                    }
                }
            }

            if next_group <= group_index {
                break;
            }
            group_index = next_group;
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct NativeVisualZoneFrame {
    pub ordered: Vec<usize>,
    pub frame_depths: Vec<i8>,
    pub blocked_depth_writes: Vec<Option<i8>>,
}

impl NativeVisualZoneFrame {
    fn empty(zone_count: usize) -> Self {
        Self {
            ordered: Vec::new(),
            frame_depths: vec![0; zone_count],
            blocked_depth_writes: vec![None; zone_count],
        }
    }
}

#[derive(Debug)]
struct NativeVisualTraversal {
    frame_depths: Vec<i8>,
    blocked_depth_writes: Vec<Option<i8>>,
    clips: Vec<NativePortalClip>,
    parent: Vec<Option<usize>>,
    children: Vec<Vec<usize>>,
}

impl NativeVisualTraversal {
    fn new(zone_count: usize) -> Self {
        Self {
            frame_depths: vec![0; zone_count],
            blocked_depth_writes: vec![None; zone_count],
            clips: vec![NativePortalClip::FULL; zone_count],
            parent: vec![None; zone_count],
            children: vec![Vec::new(); zone_count],
        }
    }

    fn reparent_sorted(&mut self, parent_zone: usize, target_zone: usize, clip: NativePortalClip) {
        if let Some(old_parent) = self.parent[target_zone] {
            self.children[old_parent].retain(|child| *child != target_zone);
        }
        self.parent[target_zone] = Some(parent_zone);
        self.clips[target_zone] = clip;

        let target_depth = clip.depth_min;
        let insert_at = self.children[parent_zone]
            .iter()
            .position(|child| target_depth < self.clips[*child].depth_min)
            .unwrap_or(self.children[parent_zone].len());
        self.children[parent_zone].insert(insert_at, target_zone);
    }

    fn flatten(&self, zone_index: usize, output: &mut Vec<usize>) {
        output.push(zone_index);
        for child in &self.children[zone_index] {
            self.flatten(*child, output);
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct NativePortalClip {
    min: Vec2,
    max: Vec2,
    depth_min: f32,
    depth_max: f32,
}

impl NativePortalClip {
    const FULL: Self = Self {
        min: Vec2::splat(-1.0),
        max: Vec2::splat(1.0),
        depth_min: 0.0,
        depth_max: 1.0,
    };

    fn union(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
            depth_min: self.depth_min.min(other.depth_min),
            depth_max: self.depth_max.max(other.depth_max),
        }
    }

    fn union_screen(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
            depth_min: self.depth_min,
            depth_max: self.depth_max,
        }
    }
}

fn native_clip_plane_value(vertex: Vec4, plane: usize) -> f32 {
    match plane {
        0 => vertex.x + vertex.w,
        1 => vertex.w - vertex.x,
        2 => vertex.y + vertex.w,
        3 => vertex.w - vertex.y,
        4 => vertex.z,
        5 => vertex.w - vertex.z,
        _ => vertex.w,
    }
}

fn native_clip_polygon_to_frustum(mut polygon: Vec<Vec4>) -> Vec<Vec4> {
    for plane in 0..6 {
        if polygon.is_empty() {
            break;
        }
        let input = std::mem::take(&mut polygon);
        let mut previous = *input.last().unwrap();
        let mut previous_distance = native_clip_plane_value(previous, plane);
        for current in input {
            let current_distance = native_clip_plane_value(current, plane);
            let previous_inside = previous_distance >= 0.0;
            let current_inside = current_distance >= 0.0;
            if previous_inside != current_inside {
                let denominator = previous_distance - current_distance;
                if denominator.abs() > f32::EPSILON {
                    let t = previous_distance / denominator;
                    polygon.push(previous + (current - previous) * t);
                }
            }
            if current_inside {
                polygon.push(current);
            }
            previous = current;
            previous_distance = current_distance;
        }
    }
    polygon
}

fn native_portal_clip(
    portal: &ProcessedPortal,
    flipped: u8,
    parent_clip: NativePortalClip,
    view_projection: Mat4,
) -> Option<NativePortalClip> {
    let homogeneous = portal
        .vertices
        .map(|vertex| view_projection * vertex.extend(1.0));

    // 0x004ED38B rejects the portal according to its serialized flipped bit and
    // projected winding before intersecting it with the parent portal window.
    // Keep the same inequality when all source vertices can be projected
    // directly. Near-plane crossings are clipped below and stay conservative.
    if homogeneous.iter().all(|vertex| vertex.w.abs() > 1.0e-6) {
        let projected = homogeneous.map(|vertex| vertex.truncate() / vertex.w);
        let winding = (projected[1].x - projected[2].x) * (projected[1].y - projected[0].y)
            <= (projected[1].x - projected[0].x) * (projected[1].y - projected[2].y);
        if (flipped == 0) != winding {
            return None;
        }
    }

    let polygon = native_clip_polygon_to_frustum(homogeneous.to_vec());
    if polygon.len() < 3 {
        return None;
    }

    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    let mut depth_min = f32::INFINITY;
    let mut depth_max = f32::NEG_INFINITY;
    for vertex in polygon {
        if vertex.w.abs() <= 1.0e-6 {
            continue;
        }
        let ndc = vertex.truncate() / vertex.w;
        min = min.min(ndc.truncate());
        max = max.max(ndc.truncate());
        depth_min = depth_min.min(ndc.z);
        depth_max = depth_max.max(ndc.z);
    }
    if !min.is_finite() || !max.is_finite() || !depth_min.is_finite() || !depth_max.is_finite() {
        return None;
    }

    min = min.max(parent_clip.min);
    max = max.min(parent_clip.max);
    depth_min = depth_min.max(parent_clip.depth_min);
    depth_max = depth_max.min(parent_clip.depth_max);
    (min.x <= max.x && min.y <= max.y && depth_min <= depth_max).then_some(NativePortalClip {
        min,
        max,
        depth_min,
        depth_max,
    })
}

fn map_editor_start_position(map: &ProcessedMap) -> Option<Vec3> {
    let mut bounds_min = Vec3::splat(f32::INFINITY);
    let mut bounds_max = Vec3::splat(f32::NEG_INFINITY);

    for zone in &map.zones {
        let a = Vec3::from(zone.bounds_box[0]);
        let b = Vec3::from(zone.bounds_box[1]);
        bounds_min = bounds_min.min(a.min(b));
        bounds_max = bounds_max.max(a.max(b));
    }

    if !bounds_min.is_finite() || !bounds_max.is_finite() {
        return None;
    }

    let center = (bounds_min + bounds_max) * 0.5;
    let y = if bounds_min.y <= 0.0 && bounds_max.y >= 0.0 {
        0.0
    } else {
        center.y
    };
    Some(Vec3::new(center.x, y, center.z))
}

#[derive(Debug, Clone)]
pub struct ProcessedCamera {
    pub hashcode: u32,
    pub position: Vec3,
    pub flags: u32,
    pub look: Vec3,
    pub focal_length: f32,
    pub aperture_width: f32,
    pub aperture_height: f32,
}

#[derive(Debug, Clone)]
pub struct ProcessedPortal {
    pub map_a: u16,
    pub map_b: u16,
    pub flags: u16,
    pub distance: f32,
    pub vertices: [Vec3; 4],
    pub face_common: u32,
    pub face_texture_ref: u32,
    pub face_flags: u32,
    pub face_vertices: Vec<Vec3>,
}

pub fn robots_portal_neighbor_zone(
    portal: &ProcessedPortal,
    zone_index: usize,
    zone_count: usize,
) -> Option<usize> {
    let zone_a = portal.map_a as usize;
    let zone_b = portal.map_b as usize;
    if zone_a >= zone_count || zone_b >= zone_count || zone_a == zone_b {
        return None;
    }
    if zone_index == zone_a {
        Some(zone_b)
    } else if zone_index == zone_b {
        Some(zone_a)
    } else {
        None
    }
}

fn point_triangle_distance_squared(point: Vec3, a: Vec3, b: Vec3, c: Vec3) -> f32 {
    let ab = b - a;
    let ac = c - a;
    let ap = point - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return ap.length_squared();
    }

    let bp = point - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return bp.length_squared();
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return (point - (a + ab * v)).length_squared();
    }

    let cp = point - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return cp.length_squared();
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return (point - (a + ac * w)).length_squared();
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 >= d3 && d5 >= d6 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (point - (b + (c - b) * w)).length_squared();
    }

    let denominator = 1.0 / (va + vb + vc);
    let v = vb * denominator;
    let w = vc * denominator;
    (point - (a + ab * v + ac * w)).length_squared()
}

pub fn robots_portal_distance_to_point(portal: &ProcessedPortal, point: Vec3) -> f32 {
    let [a, b, c, d] = portal.vertices;
    point_triangle_distance_squared(point, a, b, c)
        .min(point_triangle_distance_squared(point, a, c, d))
        .max(0.0)
        .sqrt()
}

#[derive(Debug, Clone)]
pub struct ProcessedSound {
    pub hashcode: u32,
    pub position: Vec3,
    pub flags: u32,
    pub sound_ref: u32,
    pub color: [u8; 4],
    pub volume: u8,
    pub fade_in: u8,
    pub fade_out: u8,
    pub tracking_type: u8,
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub base_map_on: u32,
}

#[derive(Debug, Clone)]
pub struct ProcessedLight {
    pub hashcode: u32,
    pub position: Vec3,
    pub flags: u32,
    pub beam: Vec3,
    pub light_type: u16,
    pub beam_angle: u16,
    pub colour: [u8; 4],
    pub radius: f32,
    pub max_effect_fraction: f32,
}

/// Robots.exe 0x00554C20..0x00554D8A converts the serialized RGB bytes to
/// floating-point light colour with a 1/128 scale, not the usual 1/255 scale.
/// Values above 0x80 are therefore intentionally brighter than 1.0 and are
/// allowed to saturate later in the original D3D lighting pipeline.
pub fn robots_native_light_colour(colour: [u8; 4]) -> Vec3 {
    const ROBOTS_LIGHT_COLOUR_SCALE: f32 = 1.0 / 128.0;
    Vec3::new(
        colour[0] as f32 * ROBOTS_LIGHT_COLOUR_SCALE,
        colour[1] as f32 * ROBOTS_LIGHT_COLOUR_SCALE,
        colour[2] as f32 * ROBOTS_LIGHT_COLOUR_SCALE,
    )
}

pub fn robots_native_light_type_description(light_type: u16) -> String {
    let mut features = Vec::new();
    if light_type & 0x1 != 0 {
        features.push("range");
    }
    if light_type & 0x2 != 0 {
        features.push("position-normal");
    }
    if light_type & 0x4 != 0 {
        features.push("beam-cone");
    }
    if light_type & 0x8 != 0 {
        features.push("beam-normal");
    }

    let unknown_bits = light_type & !0x000f;
    if unknown_bits != 0 {
        features.push("unknown-bits");
    }
    if features.is_empty() {
        features.push("constant");
    }

    if unknown_bits != 0 {
        format!("{} + 0x{unknown_bits:04x}", features.join(" + "))
    } else {
        features.join(" + ")
    }
}

#[derive(Clone, Debug)]
pub struct ProcessedPathNode {
    pub position: Vec3,
    pub size: Vec2,
    pub value: [u16; 4],
    pub flags: u32,
    pub distance: f32,
    pub num_links: u16,
}

#[derive(Clone)]
pub struct ProcessedPath {
    pub hashcode: u32,
    pub position: Vec3,
    pub flags: u32,
    pub path_type: u16,
    pub nodes: Vec<ProcessedPathNode>,
    pub links: Vec<(usize, usize)>,
}

#[derive(Clone)]
pub struct ProcessedTriggerScript {
    pub file_offset: u64,
    pub aux: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProcessedCharacterCollisionShape {
    Sphere { radius: f32 },
    Capsule { half_segment: f32, radius: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProcessedCharacterCollisionProfile {
    pub animskin: Hashcode,
    pub shape: ProcessedCharacterCollisionShape,
    pub local_center: Vec3,
    pub local_orientation: [f32; 4],
    pub transform_selector: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProcessedCharacterAnimationBonePose {
    pub position: Vec3,
    pub rotation: Quat,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProcessedCharacterAnimDatum {
    pub hashcode: Hashcode,
    pub animskin: Hashcode,
    /// Native searchable AnimDatum shape. HitCheck source selectors such as
    /// HT_AnimDatum_AttackPoint require this geometry; point-only consumers may
    /// ignore it and use the sampled transform.
    pub shape: ProcessedCharacterCollisionShape,
    pub local_center: Vec3,
    pub local_orientation: [f32; 4],
    pub transform_selector: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProcessedCharacterRootMotionSample {
    /// Native Robots root-motion translation after `0x005039D5` channel extraction.
    pub position: Vec3,
    /// Native Robots root-motion quaternion. Coordinate conversion belongs at the
    /// engine boundary, not in the EDB decoder.
    pub rotation: Quat,
}

#[derive(Clone, Debug)]
pub struct ProcessedCharacterAnimationTrack {
    pub animation: Hashcode,
    pub animskin: Hashcode,
    /// Serialized EXGeoAnim +0x0C rate byte. Native divides by 60 per fixed tick;
    /// at the default 60-Hz update and fresh node scale 1.0 this is effective frames/s.
    pub clip_rate: u8,
    /// AnimSet contribution +0x06. Native `0x004F2A67 -> 0x0054F8A6`
    /// converts this to a direct layer-weight rate of `1 / transition_fixed_ticks`.
    /// Fresh initial animation has no transition and stores zero.
    pub transition_fixed_ticks: u16,
    pub frame_count: usize,
    /// One native root-motion transform per integer frame, extracted from decoded
    /// bone 0 before pose-side root channel removal. Storage is frame-major 1:1.
    pub root_motion_samples: Vec<ProcessedCharacterRootMotionSample>,
    /// Root-to-selector bone chain. Pose storage is frame-major with exactly
    /// `bone_chain.len()` local transforms per integer frame.
    pub bone_chain: Vec<usize>,
    pub poses: Vec<ProcessedCharacterAnimationBonePose>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ProcessedCharacterVisual {
    pub file: Hashcode,
    pub script: Hashcode,
    pub runtime_type: u32,
    pub config_index: u32,
    /// Exact concrete native XItemHandler selected by `0x0047EA70` from the
    /// MonsterDatabase runtime sheet and selector. Brain dispatch keys from this
    /// identity; the character EDB is a resource, not the class discriminator.
    pub handler_class: RobotsAiHandlerClass,
    /// Native XItemHandler_AI_Character +0x628 seed from the selected
    /// HT_SpreadSheet_MonsterDatabase row second dword.
    pub handler_flags_628: u32,
    /// Native Monster Handler+0x62E initial health byte from MonsterDatabase
    /// row +0x08. NPC sheet rows are only eight bytes and expose None.
    pub initial_health: Option<u8>,
    /// MonsterDatabase row +0x09 copied to Handler+0x638. Native AI global manager
    /// `0x004563C0` reads it through handler vslot +0x14C when selecting the
    /// process-wide current-attacker owner.
    pub attacker_priority_638: Option<u8>,
    /// MonsterDatabase row +0x0C copied to Handler+0x634. Common Monster action
    /// `0x004550A0` uses this explosion UID while Handler+0x628 bit0x40000 is clear.
    pub explosion_uid_634: Option<u32>,
    /// MonsterDatabase row +0x10 copied to Handler+0x630. Common Monster action
    /// `0x004550A0` uses this explosion UID while Handler+0x628 bit0x40000 is set.
    pub explosion_uid_630: Option<u32>,
    /// MonsterDatabase row +0x14 copied to Handler+0x63C. `AI_MagneticHit`
    /// consumes values <=100 as its vertical spring mass coefficient.
    pub magnetic_mass: Option<u8>,
    /// MonsterDatabase row +0x15 copied to Handler+0x63D. Native death/drop
    /// logic owns the same byte that `AI_MagneticHit` decrements while shedding
    /// 0x47000001 objects.
    pub pickup_drop_count: Option<u8>,
    /// Searchable HT_AnimDatum_MapCollisionCapsule from the selected character
    /// EDB's AnimSkin +0x60/+0x64 channel. Shipped multi-skin character EDBs
    /// use identical MapCollision geometry across their variations.
    pub collision: Option<ProcessedCharacterCollisionProfile>,
    /// Candidate-side HT_AnimDatum_HitArea (0x10000010) consumed by native
    /// `0x00425EF0`. This is intentionally separate from MapCollisionCapsule:
    /// the two datums can have different geometry even when they share a bone.
    pub hit_area: Option<ProcessedCharacterCollisionProfile>,
    /// Native-proven freshly-created AI animator state only: layer 0,
    /// HT_Animation_Idle_Attack (0x0300000E), flags 0x3 / looping, with local
    /// pose samples restricted to the ancestry of the MapCollision selector.
    pub initial_animation: Option<Arc<ProcessedCharacterAnimationTrack>>,
    /// Every unambiguous `HT_AnimMode_Default -> target mode` animation resolved
    /// through the native AnimMode/AnimSet chain. Concrete AI brains request a
    /// mode by hashcode; the body/runtime layer must not grow one field per NPC
    /// animation as more shipped handlers are recovered.
    pub animation_modes: BTreeMap<Hashcode, Arc<ProcessedCharacterAnimationTrack>>,
    /// AnimScript bound to each resolvable AnimMode when the AnimSet contribution
    /// targets an HT_AnimScript rather than a bare Animation. Runtime event timing
    /// stays data-driven and reuses the shared native Script scheduler.
    pub animation_mode_scripts: BTreeMap<Hashcode, Arc<UXGeoScript>>,
    /// Searchable AnimDatum metadata from the selected AnimSkin. Spatial queries
    /// keep the datum local transform separate from the animated selector chain.
    pub anim_datums: BTreeMap<Hashcode, ProcessedCharacterAnimDatum>,
    /// Minimal extra sampled bone chains for gameplay events that reference an
    /// AnimDatum outside the collision selector chain, keyed by (AnimMode, datum).
    pub animation_mode_datum_tracks:
        BTreeMap<(Hashcode, Hashcode), Arc<ProcessedCharacterAnimationTrack>>,
    /// Data-driven `HT_AnimMode_Default -> HT_AnimMode_Move` result, resolved
    /// through the native AnimMode -> control -> AnimSet -> AnimScript ->
    /// Animation chain. This is diagnostic/predecoded data only until the
    /// gameplay state-machine ingress that activates Move is replayed.
    pub move_animation: Option<Arc<ProcessedCharacterAnimationTrack>>,
    /// Directional turn modes requested by AI when Handler+0x628 bit0x4 is set.
    pub turn_on_spot_l_animation: Option<Arc<ProcessedCharacterAnimationTrack>>,
    pub turn_on_spot_r_animation: Option<Arc<ProcessedCharacterAnimationTrack>>,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct ProcessedTrigger {
    pub file_offset: u64,
    pub link_ref: i32,
    pub type_index: u16,

    pub ttype: u32,
    pub tsubtype: Option<u32>,

    pub debug: u16,
    pub game_flags: u32,
    pub trig_flags: u32,
    pub position: Vec3,
    pub rotation: Vec3,
    pub scale: Vec3,

    pub data: Vec<Option<u32>>,
    pub links: Vec<i32>,
    pub engine_options: EXGeoTriggerEngineOptions,
    pub trigger_script: Option<ProcessedTriggerScript>,
    pub character_visual: Option<ProcessedCharacterVisual>,

    /// Every trigger that links to this one
    pub incoming_links: Vec<i32>,
}

#[cfg(test)]
pub(crate) fn robots_sweeper_boss_trigger_event_target(
    map: &ProcessedMap,
    controller_trigger_index: usize,
    event: NativeSweeperBossTriggerEvent,
) -> Option<usize> {
    let (owner_index, link_ordinal) = match event {
        NativeSweeperBossTriggerEvent::ControllerLink6CommonMask1 => {
            let controller = map.triggers.get(controller_trigger_index)?;
            if controller.ttype != ROBOTS_SWEEPER_CONTROLLER_TYPE {
                return None;
            }
            (controller_trigger_index, 6)
        }
        NativeSweeperBossTriggerEvent::PlayerLink0CommonMask1 => {
            let player_index = map.triggers.iter().position(|trigger| trigger.ttype == 0)?;
            (player_index, 0)
        }
    };
    let target_index =
        usize::try_from(*map.triggers.get(owner_index)?.links.get(link_ordinal)?).ok()?;
    map.triggers.get(target_index)?;
    Some(target_index)
}

impl MapViewerPanel {
    pub fn new(
        file: Hashcode,
        gl: Arc<glow::Context>,
        maps: Vec<ProcessedMap>,
        ref_entities: Vec<IdentifiableResult<(EXGeoEntity, ProcessedEntityMesh)>>,
        render_store: Arc<RwLock<RenderStore>>,
        platform: Platform,
        hashcodes: Arc<IntMap<u32, String>>,
        game: &str,
        sound_preview: SharedSoundPreview,
    ) -> Self {
        let mut maps = maps;
        Self::populate_lighting_triangles(file, &mut maps, &ref_entities, &render_store);
        let initial_camera_position = maps.first().and_then(map_editor_start_position);
        MapViewerPanel {
            frame: {
                let ef = MapFrame::new(
                    file,
                    Self::load_map_meshes(file, &gl, &maps, &ref_entities, platform),
                    gl,
                    render_store,
                    hashcodes,
                    game,
                    sound_preview,
                );

                {
                    let mut e = ef.viewer.lock();
                    e.selected_camera = CameraType::Fly;
                    e.show_grid = false;
                    if let Some(position) = initial_camera_position {
                        e.camera_fly.position = position;
                        e.camera_fly.front = Vec3::Z;
                        e.camera_fly.right = Vec3::X;
                    }
                }

                ef
            },
            maps,
        }
    }

    fn populate_lighting_triangles(
        file: Hashcode,
        maps: &mut [ProcessedMap],
        ref_entities: &[IdentifiableResult<(EXGeoEntity, ProcessedEntityMesh)>],
        render_store: &Arc<RwLock<RenderStore>>,
    ) {
        let render_store = render_store.read();
        for map in maps {
            map.lighting_triangles.clear();
            map.zone_surface_mask_counts = vec![BTreeMap::new(); map.mapzone_entities.len()];
            map.zone_surface_triangles = vec![Vec::new(); map.mapzone_entities.len()];
            map.zone_raycast_triangles = vec![None; map.mapzone_entities.len()];
            map.zone_navmeshes = vec![Vec::new(); map.mapzone_entities.len()];
            map.placement_raycast_triangles = vec![None; map.placements.len()];
            for (zone_index, zone_entity) in map.mapzone_entities.iter().enumerate() {
                let Some(Ok((_, mesh))) = ref_entities
                    .iter()
                    .find(|entry| entry.hashcode == zone_entity.entity_refptr)
                    .map(|entry| entry.data.as_ref())
                else {
                    continue;
                };

                if let Some(zone_surface_counts) = map.zone_surface_mask_counts.get_mut(zone_index)
                {
                    *zone_surface_counts = mesh.robots_surface_mask_counts.clone();
                }
                if let Some(zone_surface_triangles) = map.zone_surface_triangles.get_mut(zone_index)
                {
                    *zone_surface_triangles = mesh.robots_surface_triangles.clone();
                }
                if let Some(zone_raycast_triangles) = map.zone_raycast_triangles.get_mut(zone_index)
                {
                    *zone_raycast_triangles = Some(mesh.robots_raycast_triangles.clone());
                }
                if let Some(zone_navmeshes) = map.zone_navmeshes.get_mut(zone_index) {
                    *zone_navmeshes = mesh.robots_navmeshes.clone();
                }

                for strip in mesh.strips.iter().filter(|strip| !strip.is_navmesh) {
                    let start = strip.start_index as usize;
                    let count = strip.index_count as usize;
                    let Some(indices) = mesh.indices.get(start..start.saturating_add(count)) else {
                        continue;
                    };
                    for triangle_index in 0..indices.len().saturating_sub(2) {
                        let mut tri_indices = [
                            indices[triangle_index] as usize,
                            indices[triangle_index + 1] as usize,
                            indices[triangle_index + 2] as usize,
                        ];
                        if triangle_index & 1 != 0 {
                            tri_indices.swap(0, 1);
                        }
                        if tri_indices[0] == tri_indices[1]
                            || tri_indices[1] == tri_indices[2]
                            || tri_indices[0] == tri_indices[2]
                        {
                            continue;
                        }
                        let Some(a) = mesh.vertex_data.get(tri_indices[0]) else {
                            continue;
                        };
                        let Some(b) = mesh.vertex_data.get(tri_indices[1]) else {
                            continue;
                        };
                        let Some(c) = mesh.vertex_data.get(tri_indices[2]) else {
                            continue;
                        };
                        map.lighting_triangles.push(NativeLightingTriangle {
                            positions: [Vec3::from(a.pos), Vec3::from(b.pos), Vec3::from(c.pos)],
                            colours: [
                                Vec4::from(a.color),
                                Vec4::from(b.color),
                                Vec4::from(c.color),
                            ],
                            zone_index,
                        });
                    }
                }
            }

            for (placement_index, placement) in map.placements.iter().enumerate() {
                if placement.engine_flags & 0x08 == 0 || placement.object_ref.base() != 0x0200_0000
                {
                    continue;
                }
                let Some(resolved) =
                    render_store.resolve_entity_hashcode(file, placement.object_ref)
                else {
                    continue;
                };
                let Some(Ok((_, mesh))) = ref_entities
                    .iter()
                    .find(|entry| entry.hashcode == resolved)
                    .map(|entry| entry.data.as_ref())
                else {
                    continue;
                };
                let transform = Mat4::from_scale_rotation_translation(
                    Vec3::from(placement.scale),
                    Quat::from_euler(
                        glam::EulerRot::ZXY,
                        placement.rotation[2],
                        placement.rotation[0],
                        placement.rotation[1],
                    ),
                    Vec3::from(placement.position),
                );
                map.placement_raycast_triangles[placement_index] = Some(
                    mesh.robots_raycast_triangles
                        .iter()
                        .map(|triangle| RobotsRaycastTriangle {
                            positions: triangle
                                .positions
                                .map(|position| transform.transform_point3(position)),
                            face_mask: triangle.face_mask,
                            trailing_raw: triangle.trailing_raw,
                        })
                        .collect(),
                );
            }
        }
    }

    fn load_map_meshes(
        file: Hashcode,
        gl: &glow::Context,
        maps: &[ProcessedMap],
        ref_entities: &[IdentifiableResult<(EXGeoEntity, ProcessedEntityMesh)>],
        platform: Platform,
    ) -> Vec<(u32, Arc<Mutex<EntityRenderer>>)> {
        let mut ref_renderers = vec![];

        // FIXME(cohae): Map picking is a bit dirty at the moment
        for map in maps.iter() {
            for (zone_index, v) in map.mapzone_entities.iter().enumerate() {
                if let Some(Ok((_, e))) = &ref_entities
                    .iter()
                    .find(|ir| ir.hashcode == v.entity_refptr)
                    .map(|v| v.data.as_ref())
                {
                    let r = Arc::new(Mutex::new(EntityRenderer::new(file, platform)));
                    {
                        let mut renderer = r.lock();
                        renderer.native_light_zone = Some(zone_index);
                        renderer.native_light_sample_position =
                            map.zones.get(zone_index).map(|zone| {
                                let a = Vec3::from(zone.bounds_box[0]);
                                let b = Vec3::from(zone.bounds_box[1]);
                                (a + b) * 0.5
                            });
                        unsafe {
                            renderer.load_mesh(gl, e);
                        }
                    }
                    ref_renderers.push((map.hashcode, r));
                } else {
                    error!(
                        "Couldn't find ref entity #{} for mapzone entity!",
                        v.entity_refptr
                    );
                }
            }
        }

        ref_renderers
    }

    pub fn show(&mut self, context: &egui::Context, ui: &mut egui::Ui) -> anyhow::Result<()> {
        self.frame.show(ui, context, &self.maps)
    }
}

pub fn resolve_robots_pattern_groups(
    maps: &mut [ProcessedMap],
    path_cache: &IntMap<Hashcode, String>,
    platform: Platform,
) -> anyhow::Result<usize> {
    let Some(path) = path_cache.get(&ROBOTS_TRIGGER_DATABASE_FILE) else {
        return Ok(0);
    };
    let file = std::fs::File::open(path)?;
    let mut edb = EdbFile::new(Box::new(std::io::BufReader::new(file)), platform)?;
    let groups = read_robots_pattern_groups(&mut edb)?;
    let catalog = groups
        .into_iter()
        .map(|group| (group.hashcode, group))
        .collect::<IntMap<_, _>>();
    let count = catalog.len();
    for map in maps {
        map.pattern_groups = catalog.clone();
    }
    Ok(count)
}

pub fn resolve_robots_ball_track_pairs(
    maps: &mut [ProcessedMap],
    path_cache: &IntMap<Hashcode, String>,
    platform: Platform,
) -> anyhow::Result<(usize, usize)> {
    const ROBOTS_BALL_TRACK_SERIALIZED_TYPE: u32 = 40;

    let Some(path) = path_cache.get(&ROBOTS_HUB_TRACK_FILE) else {
        return Ok((0, 0));
    };
    let triggers = maps
        .iter()
        .flat_map(|map| map.triggers.iter())
        .filter(|trigger| trigger.ttype == ROBOTS_BALL_TRACK_SERIALIZED_TYPE)
        .collect::<Vec<_>>();
    let mut sheet_indices = triggers
        .iter()
        .map(|trigger| trigger.data.get(2).copied().flatten().unwrap_or_default())
        .collect::<Vec<_>>();
    sheet_indices.sort_unstable();
    sheet_indices.dedup();
    if sheet_indices.is_empty() {
        return Ok((0, 0));
    }
    let mut schedule_indices = triggers
        .iter()
        .filter(|trigger| {
            trigger
                .data
                .get(1)
                .copied()
                .flatten()
                .map(|value| value as i32 > 0)
                .unwrap_or(false)
        })
        .filter_map(|trigger| trigger.data.get(3).copied().flatten())
        .collect::<Vec<_>>();
    schedule_indices.sort_unstable();
    schedule_indices.dedup();

    let file = std::fs::File::open(path)?;
    let mut edb = EdbFile::new(Box::new(std::io::BufReader::new(file)), platform)?;
    let mut catalog = IntMap::default();
    for sheet_index in sheet_indices {
        if let Some(pair) = read_robots_ball_track_pair(&mut edb, sheet_index)? {
            catalog.insert(sheet_index, pair);
        }
    }
    let mut schedules = IntMap::default();
    for sheet_index in schedule_indices {
        if let Some(schedule) = read_robots_ball_track_schedule(&mut edb, sheet_index)? {
            schedules.insert(sheet_index, schedule);
        }
    }
    let pair_count = catalog.len();
    let schedule_count = schedules.len();
    for map in maps {
        map.ball_track_pairs = catalog.clone();
        map.ball_track_schedules = schedules.clone();
    }
    Ok((pair_count, schedule_count))
}

pub fn resolve_robots_mission_runtime_databases(
    maps: &mut [ProcessedMap],
    path_cache: &IntMap<Hashcode, String>,
    platform: Platform,
) -> anyhow::Result<(usize, usize)> {
    let Some(mission_path) = path_cache.get(&ROBOTS_MISSIONS_FILE_UID) else {
        return Ok((0, 0));
    };
    let Some(inventory_path) = path_cache.get(&ROBOTS_INVENTORY_FILE_UID) else {
        return Ok((0, 0));
    };

    let mission_file = std::fs::File::open(mission_path)?;
    let mut mission_edb = EdbFile::new(Box::new(std::io::BufReader::new(mission_file)), platform)?;
    let mission_definitions = read_robots_mission_definitions(&mut mission_edb)?;

    let inventory_file = std::fs::File::open(inventory_path)?;
    let mut inventory_edb =
        EdbFile::new(Box::new(std::io::BufReader::new(inventory_file)), platform)?;
    let inventory_definitions = read_robots_inventory_definitions(&mut inventory_edb)?;

    let counts = (mission_definitions.len(), inventory_definitions.len());
    for map in maps {
        map.mission_definitions = mission_definitions.clone();
        map.inventory_definitions = inventory_definitions.clone();
    }
    Ok(counts)
}

pub const ROBOTS_PLAYER_RODNEY_FILE_UID: u32 = 0x0100_0002;
pub const ROBOTS_PLAYER_RODNEY_ANIMSKIN_UID: u32 = 0x0D00_0001;

pub fn resolve_robots_player_hit_area(
    maps: &mut [ProcessedMap],
    path_cache: &IntMap<Hashcode, String>,
    platform: Platform,
) -> anyhow::Result<bool> {
    let Some(player_path) = path_cache.get(&ROBOTS_PLAYER_RODNEY_FILE_UID) else {
        return Ok(false);
    };
    let player_file = std::fs::File::open(player_path)?;
    let mut player_edb = EdbFile::new(Box::new(std::io::BufReader::new(player_file)), platform)?;
    let hit_area =
        entities::preview_hit_area_profile(&mut player_edb, ROBOTS_PLAYER_RODNEY_ANIMSKIN_UID)?;
    let solid_collision = entities::preview_anim_datum_collision_profile(
        &mut player_edb,
        ROBOTS_PLAYER_RODNEY_ANIMSKIN_UID,
        entities::ROBOTS_ANIM_DATUM_SOLID_COLLISION,
    )?;
    for map in maps {
        map.player_hit_area = hit_area;
        map.player_solid_collision = solid_collision;
    }
    Ok(hit_area.is_some())
}

pub fn resolve_robots_missile_database(
    maps: &mut [ProcessedMap],
    path_cache: &IntMap<Hashcode, String>,
    platform: Platform,
) -> anyhow::Result<usize> {
    let Some(missile_path) = path_cache.get(&ROBOTS_MISSILE_DATABASE_FILE_UID) else {
        return Ok(0);
    };
    let missile_file = std::fs::File::open(missile_path)?;
    let mut missile_edb = EdbFile::new(Box::new(std::io::BufReader::new(missile_file)), platform)?;
    let database = read_robots_missile_database(&mut missile_edb)?;
    let count = database.rows.len();
    for map in maps {
        map.missile_database = Some(database.clone());
    }
    Ok(count)
}

pub fn resolve_robots_explosion_database(
    maps: &mut [ProcessedMap],
    path_cache: &IntMap<Hashcode, String>,
    platform: Platform,
) -> anyhow::Result<(usize, usize)> {
    let Some(explosion_path) = path_cache.get(&ROBOTS_EXPLOSION_DATABASE_FILE_UID) else {
        return Ok((0, 0));
    };
    let explosion_file = std::fs::File::open(explosion_path)?;
    let mut explosion_edb =
        EdbFile::new(Box::new(std::io::BufReader::new(explosion_file)), platform)?;
    let database = read_robots_explosion_database(&mut explosion_edb)?;
    let counts = (database.definitions.len(), database.fragments.len());
    for map in maps {
        map.explosion_database = Some(database.clone());
    }
    Ok(counts)
}

pub fn resolve_robots_shop_database(
    maps: &mut [ProcessedMap],
    path_cache: &IntMap<Hashcode, String>,
    platform: Platform,
) -> anyhow::Result<(usize, usize)> {
    let Some(shop_path) = path_cache.get(&ROBOTS_SHOP_FILE_UID) else {
        return Ok((0, 0));
    };
    let shop_file = std::fs::File::open(shop_path)?;
    let mut shop_edb = EdbFile::new(Box::new(std::io::BufReader::new(shop_file)), platform)?;
    let database = read_robots_shop_database(&mut shop_edb)?;
    let counts = (database.groups.len(), database.items.len());
    for map in maps {
        map.shop_database = Some(database.clone());
    }
    Ok(counts)
}

pub fn resolve_robots_text_groups(
    maps: &mut [ProcessedMap],
    path_cache: &IntMap<Hashcode, String>,
    platform: Platform,
) -> anyhow::Result<usize> {
    let Some(text_path) = path_cache.get(&ROBOTS_TEXT_FILE_UID) else {
        return Ok(0);
    };
    let text_file = std::fs::File::open(text_path)?;
    let mut text_edb = EdbFile::new(Box::new(std::io::BufReader::new(text_file)), platform)?;
    let catalog = read_robots_text_groups(&mut text_edb)?;
    let count = catalog.groups.len();
    for map in maps {
        map.text_groups = catalog.clone();
    }
    Ok(count)
}

fn robots_edb_has_pickup_generation_script_event(edb: &mut EdbFile) -> anyhow::Result<bool> {
    let saved_internal_references = edb.internal_references.clone();
    let saved_external_references = edb.external_references.clone();
    let scripts = UXGeoScript::read_all(edb);
    edb.internal_references = saved_internal_references;
    edb.external_references = saved_external_references;
    let scripts = scripts?;
    Ok(scripts.iter().any(|script| {
        script.commands.iter().any(|command| {
            matches!(
                command.data,
                UXGeoScriptCommandData::Event {
                    event_type: 0x1600_0020,
                    ..
                }
            )
        })
    }))
}

pub fn resolve_robots_sweeper_boss_patterns(
    source_edb: &mut EdbFile,
    maps: &mut [ProcessedMap],
    path_cache: &IntMap<Hashcode, String>,
    platform: Platform,
) -> anyhow::Result<usize> {
    let controller_maps = maps
        .iter()
        .enumerate()
        .filter(|(_, map)| {
            map.triggers
                .iter()
                .any(|trigger| trigger.ttype == ROBOTS_SWEEPER_CONTROLLER_TYPE)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if controller_maps.is_empty() {
        return Ok(0);
    }
    let source_scripts_have_pickup_event =
        robots_edb_has_pickup_generation_script_event(source_edb)?;

    let Some(path) = path_cache.get(&ROBOTS_SWEEPER_PATTERN_FILE) else {
        return Ok(0);
    };
    let file = std::fs::File::open(path)?;
    let mut edb = EdbFile::new(Box::new(std::io::BufReader::new(file)), platform)?;
    let mut patterns = RobotsSweeperBossPatterns::read(&mut edb)?;
    let eye_scripts = RobotsSweeperEyeScripts::read(&mut edb)?;
    let pattern_scripts_have_pickup_event =
        robots_edb_has_pickup_generation_script_event(&mut edb)?;
    let ratchet_entity_header = edb
        .header
        .entity_list
        .data()
        .iter()
        .find(|header| header.common.hashcode == ROBOTS_SWEEPER_RAT_ENTITY)
        .cloned();
    let ratchet_position_local = if let Some(header) = ratchet_entity_header {
        let endian = edb.endian;
        read_robots_v248_entity_anim_datums(&mut edb, endian, header.common.address as u64)?
            .and_then(|directory| {
                directory
                    .records
                    .into_iter()
                    .find(|record| record.hashcode == ROBOTS_SWEEPER_RAT_POSITION_DATUM)
                    .map(|record| record.local_center)
            })
    } else {
        None
    };
    let (ratchet_missile_hand_local, ratchet_scripts, ratchet_scripts_have_pickup_event) =
        if let Some(ratchet_path) = path_cache.get(&ROBOTS_SWEEPER_RAT_RESOURCE_FILE) {
            let ratchet_file = std::fs::File::open(ratchet_path)?;
            let mut ratchet_edb =
                EdbFile::new(Box::new(std::io::BufReader::new(ratchet_file)), platform)?;
            let ratchet_scripts = RobotsSweeperRatchetScripts::read(&mut ratchet_edb)?;
            let catalog = crate::animations::read_from_file(&mut ratchet_edb)?;
            let hand = crate::animations::sample_bound_animation_bone_position(
                &catalog,
                ROBOTS_SWEEPER_ATTACK_ANIMATION,
                ROBOTS_SWEEPER_MISSILE_LAUNCH_BONE,
                ROBOTS_SWEEPER_MISSILE_LIVE_BONE_FRAME,
            )
            .map(|position| position.to_array());
            let has_pickup_event = robots_edb_has_pickup_generation_script_event(&mut ratchet_edb)?;
            (hand, Some(ratchet_scripts), has_pickup_event)
        } else {
            (None, None, true)
        };
    let transporter_spawn_position_local =
        if let Some(transporter_path) = path_cache.get(&ROBOTS_SWEEPER_TRANSPORTER_RESOURCE_FILE) {
            let transporter_file = std::fs::File::open(transporter_path)?;
            let mut transporter_edb = EdbFile::new(
                Box::new(std::io::BufReader::new(transporter_file)),
                platform,
            )?;
            let transporter_entity_header = transporter_edb
                .header
                .entity_list
                .data()
                .iter()
                .find(|header| header.common.hashcode == ROBOTS_SWEEPER_TRANSPORTER_ENTITY)
                .cloned();
            if let Some(header) = transporter_entity_header {
                let endian = transporter_edb.endian;
                read_robots_v248_entity_anim_datums(
                    &mut transporter_edb,
                    endian,
                    header.common.address as u64,
                )?
                .and_then(|directory| {
                    directory
                        .records
                        .into_iter()
                        .find(|record| record.hashcode == ROBOTS_SWEEPER_TRANSPORTER_SPAWN_DATUM)
                        .map(|record| record.local_center)
                })
            } else {
                None
            }
        } else {
            None
        };
    if let Some(database_path) = path_cache.get(&ROBOTS_MONSTER_DATABASE_FILE) {
        let database_file = std::fs::File::open(database_path)?;
        let mut database_edb =
            EdbFile::new(Box::new(std::io::BufReader::new(database_file)), platform)?;
        let database = RobotsCharacterDatabase::read(&mut database_edb)?;
        for &selector in RobotsSweeperBossPatterns::monster_selectors() {
            if let Some(file) = database
                .file_for_runtime_selector(ROBOTS_SWEEPER_MONSTER_RUNTIME_TYPE, selector as usize)
            {
                patterns.set_monster_file(selector, file);
            }
            if let Some(drop_count) = database.pickup_drop_count_for_runtime_selector(
                ROBOTS_SWEEPER_MONSTER_RUNTIME_TYPE,
                selector as usize,
            ) {
                patterns.set_monster_pickup_drop_count(selector, drop_count);
            }
        }
    }
    let dynamic_visual_selectors = RobotsSweeperBossPatterns::monster_selectors()
        .iter()
        .map(|&selector| (ROBOTS_SWEEPER_MONSTER_RUNTIME_TYPE, selector as u32))
        .collect::<Vec<_>>();
    let dynamic_character_visuals = entities::resolve_robots_character_visual_catalog(
        source_edb,
        path_cache,
        platform,
        &dynamic_visual_selectors,
    )?;
    let shared_scripts_have_pickup_event = source_scripts_have_pickup_event
        || pattern_scripts_have_pickup_event
        || ratchet_scripts_have_pickup_event;
    let count = patterns.rows.len();
    for map_index in controller_maps {
        let serialized_pickup_ingress_absent = maps[map_index].triggers.iter().all(|trigger| {
            robots_pickup_visual(trigger.ttype, &trigger.data).is_none()
                && !matches!(trigger.ttype, 68 | 69)
        });
        maps[map_index].sweeper_post_xitem_pickup_static_zero =
            serialized_pickup_ingress_absent && !shared_scripts_have_pickup_event;
        maps[map_index].runtime_character_visuals.extend(
            dynamic_character_visuals
                .iter()
                .map(|(key, visual)| (*key, visual.clone())),
        );
        maps[map_index].sweeper_boss_patterns = Some(patterns.clone());
        maps[map_index].sweeper_eye_scripts = Some(eye_scripts.clone());
        maps[map_index].sweeper_ratchet_scripts = ratchet_scripts.clone();
        maps[map_index].sweeper_ratchet_position_local = ratchet_position_local;
        maps[map_index].sweeper_ratchet_missile_hand_local = ratchet_missile_hand_local;
        maps[map_index].sweeper_transporter_spawn_position_local = transporter_spawn_position_local;
    }
    Ok(count)
}

fn validated_fluid_initial_shared_rng_draws(
    edb: &mut EdbFile,
    trigger: &ProcessedTrigger,
) -> Option<u32> {
    if trigger.ttype != ROBOTS_FLUID_TYPE {
        return None;
    }
    let draws = robots_fluid_initial_shared_rng_draw_count(&trigger.data)?;
    let width = trigger.data.first().copied().flatten()?;
    let height = trigger.data.get(1).copied().flatten()?;
    let expected_positions = usize::try_from(width.checked_mul(height)?).ok()?;
    let visual = trigger.engine_options.visual_object?;
    if !visual.is_local() {
        return None;
    }

    let entity_record = edb
        .header
        .entity_list
        .iter()
        .find(|record| record.common.hashcode == visual)?;
    let entity_address = entity_record.common.address as u64;
    let endian = edb.endian;
    let version = edb.header.version;
    let platform = edb.platform;
    edb.seek(std::io::SeekFrom::Start(entity_address)).ok()?;
    let entity = edb
        .read_type_args::<EXGeoEntity>(endian, (version, platform))
        .ok()?;
    let EXGeoEntity::Mesh(mesh) = entity else {
        return None;
    };
    let unique_positions = mesh
        .vertices
        .iter()
        .map(|vertex| {
            (
                vertex.pos[0].to_bits(),
                vertex.pos[1].to_bits(),
                vertex.pos[2].to_bits(),
            )
        })
        .collect::<BTreeSet<_>>()
        .len();
    (unique_positions == expected_positions).then_some(draws)
}

pub fn read_from_file(edb: &mut EdbFile) -> Vec<ProcessedMap> {
    let header = edb.header.clone();

    let mut maps = vec![];
    for m in header.map_list.iter() {
        edb.seek(std::io::SeekFrom::Start(m.address as u64))
            .unwrap();

        let xmap = edb
            .read_type_args::<EXGeoMap>(edb.endian, (header.version,))
            .context("Failed to read map")
            .unwrap();

        let mut map = ProcessedMap {
            hashcode: m.hashcode,
            bsp_nodes: xmap.bsp_tree.0.clone(),
            mapzone_entities: vec![],
            placements: xmap.placements.data().clone(),
            placement_group_count: xmap.placement_groups.serialized_len(),
            cameras: xmap
                .cameras
                .iter()
                .map(|camera| ProcessedCamera {
                    hashcode: camera.hashcode,
                    position: camera.position.into(),
                    flags: camera.flags,
                    look: camera.look.into(),
                    focal_length: camera.focal_length,
                    aperture_width: camera.aperture_width,
                    aperture_height: camera.aperture_height,
                })
                .collect(),
            portals: xmap
                .portals
                .iter()
                .map(|portal| ProcessedPortal {
                    map_a: portal.map_a,
                    map_b: portal.map_b,
                    flags: portal.flags,
                    distance: portal.distance,
                    vertices: portal.vertices.map(Vec3::from),
                    face_common: portal.portal_face.common,
                    face_texture_ref: portal.portal_face.texture_ref,
                    face_flags: portal.portal_face.flags,
                    face_vertices: portal
                        .portal_face
                        .vertices
                        .iter()
                        .map(|vertex| Vec3::new(vertex.v[0], vertex.v[1], vertex.v[2]))
                        .collect(),
                })
                .collect(),
            isounds: xmap.isounds.data().clone(),
            lights: xmap
                .lights
                .iter()
                .map(|light| ProcessedLight {
                    hashcode: light.hashcode,
                    position: light.position.into(),
                    flags: light.flags,
                    beam: light.beam.into(),
                    light_type: light.ltype,
                    beam_angle: light.beam_angle,
                    colour: light.colour,
                    radius: light.radius,
                    max_effect_fraction: light.max_effect_fraction,
                })
                .collect(),
            sounds: xmap
                .sounds
                .iter()
                .map(|sound| ProcessedSound {
                    hashcode: sound.hashcode,
                    position: sound.position.into(),
                    flags: sound.flags,
                    sound_ref: sound.sound_ref,
                    color: sound.color,
                    volume: sound.volume,
                    fade_in: sound.fade_in,
                    fade_out: sound.fade_out,
                    tracking_type: sound.tracking_type,
                    inner_radius: sound.inner_radius,
                    outer_radius: sound.outer_radius,
                    base_map_on: sound.base_map_on,
                })
                .collect(),
            lighting_triangles: Vec::new(),
            zone_surface_mask_counts: Vec::new(),
            zone_surface_triangles: Vec::new(),
            zone_raycast_triangles: Vec::new(),
            zone_navmeshes: Vec::new(),
            placement_raycast_triangles: Vec::new(),
            paths: xmap
                .paths
                .iter()
                .map(|path| ProcessedPath {
                    hashcode: path.hashcode,
                    position: path.position.into(),
                    flags: path.flags,
                    path_type: path.ptype,
                    nodes: path
                        .nodes
                        .iter()
                        .map(|node| ProcessedPathNode {
                            position: node.position.into(),
                            size: node.size.into(),
                            value: node.value,
                            flags: node.flags,
                            distance: node.distance,
                            num_links: node.num_links,
                        })
                        .collect(),
                    links: path
                        .links
                        .iter()
                        .map(|link| (link.node_a as usize, link.node_b as usize))
                        .collect(),
                })
                .collect(),
            triggers: vec![],
            runtime_character_visuals: BTreeMap::new(),
            fluid_initial_shared_rng_draws: vec![],
            sweeper_post_xitem_pickup_static_zero: false,
            trigger_collisions: xmap.trigger_header.trigger_collisions.0.clone(),
            pattern_groups: IntMap::default(),
            ball_track_pairs: IntMap::default(),
            ball_track_schedules: IntMap::default(),
            mission_definitions: vec![],
            inventory_definitions: vec![],
            missile_database: None,
            explosion_database: None,
            player_hit_area: None,
            player_solid_collision: None,
            shop_database: None,
            text_groups: RobotsTextGroupCatalog::default(),
            sweeper_boss_patterns: None,
            sweeper_eye_scripts: None,
            sweeper_ratchet_scripts: None,
            sweeper_ratchet_position_local: None,
            sweeper_ratchet_missile_hand_local: None,
            sweeper_transporter_spawn_position_local: None,
            skies: xmap.skies.iter().map(|s| s.hashcode).collect(),
            zones: vec![],
        };

        for z in &xmap.zones {
            let entity_offset = header.refpointer_list[z.entity_refptr as usize].address;
            edb.seek(std::io::SeekFrom::Start(entity_offset as u64))
                .context("Mapzone refptr pointer to a non-entity object!")
                .unwrap();

            let ent = edb
                .read_type_args::<EXGeoEntity>(edb.endian, (header.version, edb.platform))
                .unwrap();

            if let EXGeoEntity::MapZone(mapzone) = ent {
                map.mapzone_entities.push(mapzone);
            } else {
                error!("Refptr entity does not have a mapzone entity!");
                // Result::<()>::Err(anyhow::anyhow!(
                //     "Refptr entity does not have a mapzone entity!"
                // ))
                // .unwrap();
            }
        }

        map.zones = xmap.zones;

        for t in xmap.trigger_header.triggers.iter() {
            let trig = &t.trigger;
            let (ttype, tsubtype) = {
                let t = &xmap.trigger_header.trigger_types[trig.type_index as usize];

                (t.trig_type, t.trig_subtype)
            };

            // Trigger-only pickups do not serialize visual_object/file. Resolve the
            // native pickup Script/entity and preserve local 0x82 objects in the
            // namespace of their explicitly named external EDB.
            if let Some(pickup) = robots_pickup_visual(ttype, &trig.data) {
                edb.add_external_reference(pickup.file, pickup.object);
            }

            let trigger_script = trig.engine_options.gamescript_index.and_then(|index| {
                xmap.trigger_header
                    .trigger_scripts
                    .get(index as usize)
                    .map(|(script, aux)| ProcessedTriggerScript {
                        file_offset: script.offset_absolute(),
                        aux: *aux,
                    })
            });

            let trigger = ProcessedTrigger {
                file_offset: t.trigger.offset_absolute(),
                link_ref: t.link_ref,
                type_index: trig.type_index,
                ttype,
                tsubtype: if tsubtype != 0 && tsubtype != 0x42000001 {
                    Some(tsubtype)
                } else {
                    None
                },
                debug: trig.debug,
                game_flags: trig.game_flags,
                trig_flags: trig.trig_flags,
                position: trig.position.into(),
                rotation: trig.rotation.into(),
                scale: trig.scale.into(),
                engine_options: trig.engine_options.clone(),
                trigger_script,
                character_visual: None,
                data: trig.data.to_vec(),
                links: trig.links.to_vec(),
                incoming_links: vec![],
            };

            let fluid_shared_rng_draws = validated_fluid_initial_shared_rng_draws(edb, &trigger);
            map.fluid_initial_shared_rng_draws
                .push(fluid_shared_rng_draws);
            map.triggers.push(trigger);
        }

        for i in 0..map.triggers.len() {
            for ei in 0..map.triggers.len() {
                if i == ei {
                    continue;
                }

                if map.triggers[ei].links.iter().any(|v| *v == i as i32) {
                    map.triggers[i].incoming_links.push(ei as i32);
                }
            }
        }

        maps.push(map);
    }

    maps
}

#[cfg(test)]
mod tests {
    use super::triggers::NativeSweeperRatchetScriptEventKind;
    use super::{
        map_editor_start_position, native_portal_clip, read_from_file,
        resolve_robots_sweeper_boss_patterns, robots_camera_controller_plan, robots_camera_flags,
        robots_camera_marker_scaled_data0, robots_camera_mode, robots_camera_scaled_data4,
        robots_camera_scaled_data5, robots_monster_data15_value, robots_monster_data4_value,
        robots_monster_flags, robots_monster_is_family, robots_monster_proximity_radius,
        robots_monster_runtime_selector, robots_monster_test_runtime_value,
        robots_monster_transporter_secondary_path_hash, robots_native_light_colour,
        robots_native_light_type_description, robots_npc_alternate_cutscenes,
        robots_npc_cutscene_is_null, robots_npc_flags, robots_npc_runtime_selector,
        robots_npc_runtime_uid, robots_npc_text_group, robots_portal_neighbor_zone,
        robots_trigger_path_data_slot, robots_trigger_path_hash, robots_trigger_path_is_proven,
        robots_trigger_platform_angular_velocity, robots_trigger_runtime_path_acceleration,
        robots_trigger_runtime_path_speed, robots_watchbot_enter_distance, robots_watchbot_flags,
        robots_watchbot_leave_distance, robots_watchbot_mode, NativePortalClip, ProcessedPortal,
    };
    use eurochef_edb::{
        anim::EXGeoBaseAnimSkin,
        binrw::BinReaderExt,
        edb::EdbFile,
        entity::{read_robots_v248_entity_anim_datums, EXGeoEntity, ROBOTS_ENTITY_FLAG_NO_FOG},
        script::EXGeoAnimScript,
        texture::EXGeoTexture,
        versions::Platform,
        HashcodeUtils,
    };
    use eurochef_shared::{
        robots_runtime::ai_character::RobotsAiHandlerClass,
        script::{UXGeoScript, UXGeoScriptCommandData},
    };
    use glam::{Mat4, Vec3};
    use std::{
        fs::File,
        io::{BufReader, Seek, SeekFrom},
        path::{Path, PathBuf},
    };

    fn format_mesh_diagnostics(mesh: &crate::entities::ProcessedEntityMesh) -> String {
        let strips = mesh
            .strips
            .iter()
            .enumerate()
            .map(|(index, strip)| {
                format!(
                    "#{index}:indices={} triangles={} texture={} transparency=0x{:04X} flags=0x{:04X} navmesh={}",
                    strip.index_count,
                    strip.tri_count,
                    strip.texture_index,
                    strip.transparency,
                    strip.flags,
                    strip.is_navmesh,
                )
            })
            .collect::<Vec<_>>()
            .join(";");
        format!(
            "vertices={} indices={} strips={} entity_flags=0x{:08X} strip_data=[{}]",
            mesh.vertex_data.len(),
            mesh.indices.len(),
            mesh.strips.len(),
            mesh.flags,
            strips,
        )
    }

    fn audio_manifest_edb_paths(manifest_path: &str, manifest: &str) -> Vec<PathBuf> {
        let mut lines = manifest.lines();
        let header = lines
            .next()
            .expect("real audio manifest does not contain a header")
            .split('\t')
            .collect::<Vec<_>>();
        let path_column = ["source_path", "physical_path", "path", "file_name"]
            .into_iter()
            .find_map(|name| header.iter().position(|column| *column == name))
            .expect("real audio manifest has no source_path/path/file_name column");
        let relative_root = std::env::var_os("EUROCHEF_REAL_AUDIO_EDB_ROOT")
            .map(PathBuf::from)
            .or_else(|| {
                Path::new(manifest_path)
                    .ancestors()
                    .find(|ancestor| {
                        ancestor
                            .file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| name.eq_ignore_ascii_case("_eurotools_out"))
                    })
                    .map(|eurotools_out| eurotools_out.join("extracted_main/robots/binary/_bin_pc"))
            });

        lines
            .filter_map(|line| {
                let value = line.split('\t').nth(path_column)?.trim();
                if value.is_empty() {
                    return None;
                }
                let path = PathBuf::from(value);
                Some(if path.is_absolute() {
                    path
                } else {
                    relative_root
                        .as_ref()
                        .expect("relative manifest paths require EUROCHEF_REAL_AUDIO_EDB_ROOT")
                        .join(path)
                })
            })
            .collect()
    }

    #[test]
    fn real_robots_v248_sweeper_projectile_attack_and_rodney_hit_shapes_match_native_query_data() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let root =
            Path::new(&game_root).join("_eurotools_out/extracted_main/robots/binary/_bin_pc");

        let bo5_path = root.join("bo5_fin.edb");
        let file = File::open(&bo5_path).expect("open bo5_fin.edb");
        let mut bo5 =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse bo5_fin.edb");
        let missile_header = bo5
            .header
            .entity_list
            .data()
            .iter()
            .find(|header| header.common.hashcode == 0x0200_0195)
            .cloned()
            .expect("bo5_fin RatchetMissile entity 0x02000195");
        let bo5_endian = bo5.endian;
        let missile_datums = read_robots_v248_entity_anim_datums(
            &mut bo5,
            bo5_endian,
            missile_header.common.address as u64,
        )
        .expect("read RatchetMissile Entity AnimDatum directory")
        .expect("RatchetMissile Entity AnimDatum directory missing");
        let attack = missile_datums
            .records
            .iter()
            .find(|datum| datum.hashcode == 0x1000_0009)
            .expect("RatchetMissile HT_AnimDatum_AttackPoint missing");
        assert_eq!(attack.shape_mode, 1);
        assert!((attack.shape_scalars[0] - 0.70).abs() <= 1.0e-6);
        assert!(attack
            .local_center
            .iter()
            .all(|value| value.abs() <= 1.0e-6));

        let rodney_path = root.join("p01_rod.edb");
        let file = File::open(&rodney_path).expect("open p01_rod.edb");
        let mut rodney =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse p01_rod.edb");
        let skin_header = rodney
            .header
            .animskin_list
            .data()
            .iter()
            .find(|header| header.common.hashcode == 0x0D00_0001)
            .cloned()
            .expect("Rodney AnimSkin 0x0D000001");
        rodney
            .seek(SeekFrom::Start(skin_header.common.address as u64))
            .expect("seek Rodney AnimSkin");
        let skin = rodney
            .read_type_args::<EXGeoBaseAnimSkin>(rodney.endian, (rodney.header.version,))
            .expect("parse Rodney AnimSkin");
        let hit_area = skin
            .robots_animdatum_section
            .as_ref()
            .expect("Rodney AnimDatum section")
            .find(0x1000_0010)
            .expect("Rodney searchable HT_AnimDatum_HitArea missing");
        assert_eq!(hit_area.header.shape_mode, 1);
        assert!((hit_area.shape_scalars[0] - 0.35).abs() <= 1.0e-6);
        assert!((hit_area.local_center[0] - 0.0).abs() <= 1.0e-6);
        assert!((hit_area.local_center[1] - 0.35).abs() <= 1.0e-6);
        assert!((hit_area.local_center[2] - 0.0).abs() <= 1.0e-6);
        assert_eq!(hit_area.transform_selector, 0);
        assert_eq!(hit_area.hierarchy_chain, [0]);

        let solid_collision = skin
            .robots_animdatum_section
            .as_ref()
            .expect("Rodney AnimDatum section")
            .find(0x1000_0001)
            .expect("Rodney HT_AnimDatum_SolidCollision missing");
        assert_eq!(solid_collision.header.shape_mode, 1);
        assert!((solid_collision.shape_scalars[0] - 0.325).abs() <= 1.0e-6);
        assert!((solid_collision.local_center[0] - 0.0).abs() <= 1.0e-6);
        assert!((solid_collision.local_center[1] - 0.35).abs() <= 1.0e-6);
        assert!(solid_collision.local_center[2].abs() <= 1.0e-6);
        assert_eq!(solid_collision.transform_selector, 0);
        assert_eq!(solid_collision.hierarchy_chain, [0]);
    }

    #[test]
    fn real_robots_v248_sweeper_boss_resolver_hydrates_native_missile_hand_when_game_root_is_configured(
    ) {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let root =
            Path::new(&game_root).join("_eurotools_out/extracted_main/robots/binary/_bin_pc");
        let m10_path = root.join("m10_boss.edb");
        let file = File::open(&m10_path).expect("open m10_boss.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse m10_boss.edb");
        let mut maps = read_from_file(&mut edb);
        assert!(maps.iter().any(|map| {
            map.triggers
                .iter()
                .any(|trigger| trigger.ttype == super::ROBOTS_SWEEPER_CONTROLLER_TYPE)
        }));

        let mut path_cache = nohash_hasher::IntMap::<u32, String>::default();
        for entry in std::fs::read_dir(&root).expect("scan Robots EDB folder") {
            let path = entry.expect("bad Robots EDB directory entry").path();
            if path.extension().and_then(|value| value.to_str()) != Some("edb") {
                continue;
            }
            let file = File::open(&path).expect("open indexed Robots EDB");
            let indexed_edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .expect("parse indexed Robots EDB header");
            path_cache.insert(
                indexed_edb.header.hashcode,
                path.to_string_lossy().into_owned(),
            );
        }

        let mut stage184_pickup_script_events = Vec::new();
        for (label, path) in [
            ("m10_boss", root.join("m10_boss.edb")),
            ("bo5_fin", root.join("bo5_fin.edb")),
            ("nb11_rat", root.join("nb11_rat.edb")),
        ] {
            let file = File::open(&path).unwrap_or_else(|error| panic!("open {label}: {error}"));
            let mut script_edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .unwrap_or_else(|error| panic!("parse {label}: {error}"));
            let scripts = UXGeoScript::read_all(&mut script_edb)
                .unwrap_or_else(|error| panic!("read {label} scripts: {error}"));
            for script in scripts {
                for command in script.commands {
                    let UXGeoScriptCommandData::Event { event_type, .. } = command.data else {
                        continue;
                    };
                    if event_type == 0x1600_0020 {
                        stage184_pickup_script_events.push((
                            label,
                            script.hashcode,
                            command.start,
                            command.length,
                        ));
                    }
                }
            }
        }
        assert!(
            stage184_pickup_script_events.is_empty(),
            "m10_boss/bo5_fin/nb11_rat unexpectedly contain HT_ScriptEvents pickup-generation event 0x16000020: {stage184_pickup_script_events:?}"
        );

        let rows =
            resolve_robots_sweeper_boss_patterns(&mut edb, &mut maps, &path_cache, Platform::Pc)
                .expect("hydrate Sweeper boss resources");
        assert_eq!(rows, super::ROBOTS_SWEEPER_PATTERN_ROW_COUNT);
        let map = maps
            .iter()
            .find(|map| {
                map.triggers
                    .iter()
                    .any(|trigger| trigger.ttype == super::ROBOTS_SWEEPER_CONTROLLER_TYPE)
            })
            .expect("m10_boss Sweeper controller map");
        for (selector, expected_class) in [
            (0, RobotsAiHandlerClass::DogBot),
            (7, RobotsAiHandlerClass::MalfBot),
            (9, RobotsAiHandlerClass::Eb10RollerBot),
            (13, RobotsAiHandlerClass::Ew10Minion),
            (14, RobotsAiHandlerClass::Eb14Minion),
            (20, RobotsAiHandlerClass::ShuntBotBoss),
        ] {
            let visual = map
                .runtime_character_visuals
                .get(&(super::ROBOTS_SWEEPER_MONSTER_RUNTIME_TYPE, selector))
                .unwrap_or_else(|| panic!("dynamic selector {selector} visual missing"));
            assert_eq!(visual.handler_class, expected_class);
            assert_eq!(
                visual.runtime_type,
                super::ROBOTS_SWEEPER_MONSTER_RUNTIME_TYPE
            );
            assert_eq!(visual.config_index, selector);
        }
        let roller_visual = map
            .runtime_character_visuals
            .get(&(super::ROBOTS_SWEEPER_MONSTER_RUNTIME_TYPE, 9))
            .expect("dynamic Sweeper selector9 RollerBot visual");
        assert_eq!(roller_visual.file, 0x0100_0046);
        assert!(roller_visual.collision.is_some());
        assert!(roller_visual.hit_area.is_some());
        assert!(!roller_visual.animation_modes.is_empty());
        let eye_scripts = map
            .sweeper_eye_scripts
            .as_ref()
            .expect("m10_boss Sweeper Eye Script catalog");
        assert_eq!(eye_scripts.len(), 6);
        for script in [
            0x0400_0258_u32,
            0x0400_0259,
            0x0400_025A,
            0x0400_025B,
            0x0400_025C,
            0x0400_025D,
        ] {
            let profile = eye_scripts.profile(script).expect("Eye Script profile");
            eprintln!(
                "eye-script {script:08X} rate={} len={} events={:?}",
                profile.frame_rate, profile.length, profile.events
            );
        }
        let stage174_character_triggers = map
            .triggers
            .iter()
            .enumerate()
            .filter(|(_, trigger)| matches!(trigger.ttype, 3 | 10 | 11 | 18 | 33 | 48 | 70 | 74))
            .map(|(index, trigger)| (index, trigger.ttype, trigger.debug))
            .collect::<Vec<_>>();
        assert_eq!(
            stage174_character_triggers,
            vec![(8, 10, 9)],
            "m10_boss serialized AI-character trigger census changed"
        );
        let stage175_serialized_ai = &map.triggers[8];
        let stage175_player = &map.triggers[0];
        assert_eq!(stage175_serialized_ai.trig_flags, 0x0000_0085);
        assert_eq!(stage175_serialized_ai.game_flags, 0x0000_0001);
        assert!(stage175_serialized_ai.links.iter().all(|link| *link == -1));
        assert_eq!(stage175_serialized_ai.data[0], Some(7));
        let serialized_ai_initial_distance_squared = stage175_serialized_ai
            .position
            .distance_squared(stage175_player.position);
        assert!(
            (serialized_ai_initial_distance_squared - 7_103.876).abs() < 0.01,
            "serialized m10 type10 initial distance changed: {serialized_ai_initial_distance_squared}"
        );
        assert!(
            serialized_ai_initial_distance_squared > 900.0,
            "serialized m10 type10 must begin outside the native 30-unit far-band threshold"
        );
        let (controller_index, controller) = map
            .triggers
            .iter()
            .enumerate()
            .find(|(_, trigger)| trigger.ttype == super::ROBOTS_SWEEPER_CONTROLLER_TYPE)
            .expect("m10_boss controller trigger");
        assert_eq!((controller_index, controller.debug), (6, 7));
        let stage180_controller_start_distance_squared = stage175_player
            .position
            .distance_squared(controller.position);
        assert!(
            (stage180_controller_start_distance_squared - 73.0112).abs() < 0.001,
            "m10_boss Player0/controller start distance changed: {stage180_controller_start_distance_squared}"
        );
        assert!(
            stage180_controller_start_distance_squared < 100.0,
            "shipped m10_boss controller must begin inside the native 10-unit Event0 service band"
        );
        assert_eq!(
            controller.links.iter().take(8).copied().collect::<Vec<_>>(),
            vec![3, 2, 1, 4, 5, 7, 10, -1]
        );
        let linked_types = controller
            .links
            .iter()
            .take(7)
            .map(|link| {
                usize::try_from(*link)
                    .ok()
                    .and_then(|index| map.triggers.get(index))
                    .map(|trigger| trigger.ttype)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            linked_types,
            vec![
                Some(86),
                Some(86),
                Some(86),
                Some(86),
                Some(86),
                Some(73),
                Some(19)
            ]
        );
        let stage181_m10_trigger_types = map
            .triggers
            .iter()
            .map(|trigger| trigger.ttype)
            .collect::<Vec<_>>();
        assert_eq!(
            stage181_m10_trigger_types,
            vec![
                0, 86, 86, 86, 86, 86, 87, 73, 10, 19, 19, 19, 19, 15, 50, 15, 13, 16, 16, 8, 8, 8
            ]
        );
        let stage181_pickup_types = [
            0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x29, 0x3E, 0x3F, 0x40, 0x41, 0x43, 0x47, 0x52,
            0x53,
        ];
        assert!(
            map.triggers
                .iter()
                .all(|trigger| !stage181_pickup_types.contains(&trigger.ttype)),
            "m10_boss unexpectedly gained a serialized Pickup trigger that would consume shared RNG during XTrigger_Pickup::CreateItem"
        );
        let stage185_fluid = &map.triggers[14];
        assert_eq!((stage185_fluid.ttype, stage185_fluid.debug), (50, 15));
        assert_eq!(stage185_fluid.game_flags, 0x0000_8000);
        assert_eq!(stage185_fluid.trig_flags, 0x0100_003F);
        assert_eq!(
            stage185_fluid.engine_options.visual_object,
            Some(0x8200_0000)
        );
        assert_eq!(stage185_fluid.engine_options.visual_object_file, None);
        let stage185_fluid_distance_squared = stage185_fluid
            .position
            .distance_squared(stage175_player.position);
        assert!((stage185_fluid_distance_squared - 16.637_848).abs() < 0.001);
        assert!(stage185_fluid_distance_squared < 100.0);
        assert_eq!(
            map.native_zone_index(controller.position),
            map.native_zone_index(stage185_fluid.position),
            "m10 Sweeper controller and Fluid must remain in the same native TriggerManager zone group"
        );
        let stage185_fluid_entity_record = edb
            .header
            .entity_list
            .iter()
            .next()
            .expect("m10_boss local entity0 for Fluid");
        let stage185_fluid_entity_address = stage185_fluid_entity_record.common.address as u64;
        let stage185_fluid_entity_hash = stage185_fluid_entity_record.common.hashcode;
        let stage185_fluid_entity_endian = edb.endian;
        let stage185_fluid_entity_version = edb.header.version;
        edb.seek(std::io::SeekFrom::Start(stage185_fluid_entity_address))
            .expect("seek m10_boss Fluid local entity0");
        let stage185_fluid_entity = edb
            .read_type_args::<EXGeoEntity>(
                stage185_fluid_entity_endian,
                (stage185_fluid_entity_version, Platform::Pc),
            )
            .expect("parse m10_boss Fluid local entity0");
        let EXGeoEntity::Mesh(stage185_fluid_mesh) = stage185_fluid_entity else {
            panic!("m10_boss Fluid local entity0 is not a mesh");
        };
        assert_eq!(stage185_fluid_entity_hash, 0x8200_0000);
        assert_eq!(stage185_fluid_mesh.vertices.len(), 741);
        let stage185_unique_grid_positions = stage185_fluid_mesh
            .vertices
            .iter()
            .map(|vertex| {
                (
                    vertex.pos[0].to_bits(),
                    vertex.pos[1].to_bits(),
                    vertex.pos[2].to_bits(),
                )
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(stage185_unique_grid_positions.len(), 247);
        assert_eq!(stage185_fluid.data[0], Some(13));
        assert_eq!(stage185_fluid.data[1], Some(19));
        assert_eq!(stage185_fluid.data[5], Some(5));
        assert_eq!(stage185_fluid.data[6], None);
        assert_eq!(stage185_fluid.data[7], None);
        assert_eq!(map.fluid_initial_shared_rng_draws.get(14), Some(&Some(4)));
        assert!(
            map.sweeper_post_xitem_pickup_static_zero,
            "m10 Sweeper static post-XItem Pickup ingress proof changed"
        );
        assert!(
            map.triggers
                .iter()
                .all(|trigger| !matches!(trigger.ttype, 68 | 69)),
            "m10_boss unexpectedly gained serialized BossExecutive/BossExecController triggers"
        );
        let stage179_transporter = &map.triggers[7];
        assert_eq!(stage179_transporter.ttype, 73);
        assert_eq!(stage179_transporter.game_flags, 0x0000_e001);
        assert_ne!(stage179_transporter.game_flags & 1, 0);
        assert_eq!(stage179_transporter.data[3], Some(100));
        assert_eq!(stage179_transporter.data[5], Some(6));
        assert_eq!(stage179_transporter.data[6], Some(1));
        let stage182_patterns = map
            .sweeper_boss_patterns
            .as_ref()
            .expect("hydrated Sweeper boss patterns");
        assert_eq!(stage182_patterns.monster_pickup_drop_count(7), Some(0));
        assert_eq!(stage182_patterns.monster_pickup_drop_count(9), Some(0));
        let stage182_row28 =
            stage182_patterns.rows[28].map(|cell| (cell.monster_id, cell.spawn_count));
        assert_eq!(
            stage182_row28,
            [(0, 0), (5, 1), (3, 1), (5, 1), (0, 0)],
            "fresh-process first RNG draw selects difficulty-5 row28; shipped pattern changed"
        );
        assert!(map.sweeper_ratchet_position_local.is_some());
        let transporter_spawn = map
            .sweeper_transporter_spawn_position_local
            .expect("resolver did not hydrate MonsterTransporter spawn datum");
        assert!((transporter_spawn[0] - 0.009_995_218).abs() <= 1.0e-6);
        assert!((transporter_spawn[1] + 1.028_803_945).abs() <= 1.0e-6);
        assert!((transporter_spawn[2] - 0.067_002_304).abs() <= 1.0e-6);
        let ratchet_scripts = map
            .sweeper_ratchet_scripts
            .as_ref()
            .expect("resolver did not hydrate native Ratchet Script profiles");
        assert_eq!(ratchet_scripts.len(), 13);
        let appear = ratchet_scripts
            .profile_for_anim_mode(super::ROBOTS_SWEEPER_APPEAR_ANIM_MODE)
            .expect("Appear Script profile");
        assert_eq!(appear.script_frame_rate, 30.0);
        assert_eq!(appear.animation_frame_rate, 30.0);
        assert_eq!(appear.length, 19);
        assert_eq!(
            appear
                .events
                .iter()
                .map(|event| event.start_frame)
                .collect::<Vec<_>>(),
            vec![3, 18]
        );
        let attack2 = ratchet_scripts
            .profile_for_anim_mode(super::triggers::ROBOTS_SWEEPER_ATTACK2_ANIM_MODE)
            .expect("Attack2 Script profile");
        assert_eq!(
            attack2
                .events
                .iter()
                .map(|event| event.start_frame)
                .collect::<Vec<_>>(),
            vec![10, 145]
        );
        let attack = ratchet_scripts
            .profile_for_anim_mode(super::ROBOTS_SWEEPER_ATTACK_ANIM_MODE)
            .expect("Attack Script profile");
        assert_eq!(
            attack
                .events
                .iter()
                .map(|event| event.start_frame)
                .collect::<Vec<_>>(),
            vec![17, 35]
        );
        let stage217_disappear = ratchet_scripts
            .profile(0x8400_0008)
            .expect("Disappear Script profile");
        assert_eq!(stage217_disappear.length, 19);
        assert_eq!(stage217_disappear.animation, 0x8300_0013);
        assert_eq!(stage217_disappear.animation_frame_count, 19);
        assert_eq!(stage217_disappear.animation_frame_rate, 30.0);
        assert_eq!(
            stage217_disappear
                .events
                .iter()
                .map(|event| (event.start_frame, event.length, event.kind))
                .collect::<Vec<_>>(),
            vec![(18, 1, NativeSweeperRatchetScriptEventKind::ScriptValue(1.0),)]
        );
        let stage217_idle_combat2 = ratchet_scripts
            .profile(0x8400_0010)
            .expect("IdleCombat2 Script profile");
        assert_eq!(stage217_idle_combat2.length, 76);
        assert_eq!(stage217_idle_combat2.animation, 0x8300_0010);
        assert_eq!(stage217_idle_combat2.animation_frame_count, 76);
        assert_eq!(stage217_idle_combat2.animation_frame_rate, 30.0);
        assert_eq!(
            stage217_idle_combat2
                .events
                .iter()
                .map(|event| (event.start_frame, event.length, event.kind))
                .collect::<Vec<_>>(),
            vec![(75, 1, NativeSweeperRatchetScriptEventKind::ScriptValue(1.0),)]
        );
        let stage250_idle_combat3 = ratchet_scripts
            .profile(0x8400_0011)
            .expect("IdleCombat3 Script profile");
        assert_eq!(stage250_idle_combat3.script_frame_rate, 30.0);
        assert_eq!(stage250_idle_combat3.length, 91);
        assert_eq!(stage250_idle_combat3.animation, 0x8300_0011);
        assert_eq!(stage250_idle_combat3.animation_frame_count, 91);
        assert_eq!(stage250_idle_combat3.animation_frame_rate, 30.0);
        assert_eq!(
            stage250_idle_combat3
                .events
                .iter()
                .map(|event| (event.start_frame, event.length, event.kind))
                .collect::<Vec<_>>(),
            vec![(90, 1, NativeSweeperRatchetScriptEventKind::ScriptValue(1.0),)]
        );
        let player = map.triggers.first().expect("m10_boss Player trigger #0");
        assert_eq!(player.ttype, 0);
        assert_eq!(player.debug, 1);
        assert_eq!(player.links.first().copied(), Some(11));
        let player_death_target = &map.triggers[11];
        assert_eq!(player_death_target.ttype, 19);
        assert_eq!(player_death_target.debug, 12);

        let (controller_index, controller) = map
            .triggers
            .iter()
            .enumerate()
            .find(|(_, trigger)| trigger.ttype == super::ROBOTS_SWEEPER_CONTROLLER_TYPE)
            .expect("m10_boss Sweeper controller trigger");
        assert_eq!(controller_index, 6);
        assert_eq!(controller.links.get(6).copied(), Some(10));
        let controller_phase_target = &map.triggers[10];
        assert_eq!(controller_phase_target.ttype, 19);
        assert_eq!(controller_phase_target.debug, 11);
        assert_eq!(controller_phase_target.data[2], Some(272));
        assert_eq!(controller_phase_target.data[3], Some(1));
        assert_eq!(
            super::robots_sweeper_boss_trigger_event_target(
                map,
                controller_index,
                super::NativeSweeperBossTriggerEvent::ControllerLink6CommonMask1,
            ),
            Some(10),
        );
        assert_eq!(
            super::robots_sweeper_boss_trigger_event_target(
                map,
                controller_index,
                super::NativeSweeperBossTriggerEvent::PlayerLink0CommonMask1,
            ),
            Some(11),
        );
        let hand = Vec3::from(
            map.sweeper_ratchet_missile_hand_local
                .expect("resolver did not hydrate native firing R_Hand"),
        );
        let expected = Vec3::new(-0.101_767_67, 0.788_267_73, 0.772_494_4);
        assert!(
            hand.distance(expected) < 1.0e-6,
            "resolved firing R_Hand mismatch: {hand:?} != {expected:?}"
        );
    }

    #[test]
    fn robots_platform_and_lift_use_the_runtime_proven_path_slots() {
        let mut data = vec![None; 16];
        data[2] = Some(0x0B00_0014);
        assert_eq!(robots_trigger_path_hash(8, &data), Some(0x0B00_0014));

        data[2] = None;
        data[1] = Some(0x0B00_0037);
        assert_eq!(robots_trigger_path_hash(37, &data), Some(0x0B00_0037));
        assert_eq!(robots_trigger_path_hash(80, &data), Some(0x0B00_0037));
    }

    #[test]
    fn robots_camera_and_marker_use_runtime_proven_path_slots() {
        let mut camera = vec![None; 16];
        camera[0] = Some(4);
        camera[1] = Some(0x0B00_002E);
        camera[2] = Some(0x0000_8008);
        camera[4] = Some(40);
        camera[5] = Some(45);
        assert_eq!(robots_trigger_path_hash(1, &camera), Some(0x0B00_002E));
        assert_eq!(robots_trigger_path_data_slot(1), Some(1));
        assert_eq!(robots_camera_mode(1, &camera), Some(4));
        assert_eq!(robots_camera_scaled_data4(1, &camera), Some(4.0));
        assert_eq!(robots_camera_scaled_data5(1, &camera), Some(4.5));
        assert_eq!(robots_camera_flags(1, &camera), Some(0x0000_8008));
        assert_eq!(robots_trigger_runtime_path_speed(1, &camera), None);

        let mut marker = vec![None; 16];
        marker[0] = Some((-30i32) as u32);
        marker[2] = Some(0x0000_8002);
        marker[4] = Some(0x0B00_0053);
        assert_eq!(robots_trigger_path_hash(20, &marker), Some(0x0B00_0053));
        assert_eq!(robots_trigger_path_data_slot(20), Some(4));
        assert_eq!(robots_camera_marker_scaled_data0(20, &marker), Some(-3.0));
        assert_eq!(robots_camera_flags(20, &marker), Some(0x0000_8002));
        assert_eq!(robots_trigger_runtime_path_speed(20, &marker), None);
    }

    #[test]
    fn portal_endpoints_form_bidirectional_zone_adjacency() {
        let portal = ProcessedPortal {
            map_a: 2,
            map_b: 7,
            flags: 0,
            distance: 1.0,
            vertices: [Vec3::ZERO; 4],
            face_common: 0,
            face_texture_ref: 0,
            face_flags: 0,
            face_vertices: vec![],
        };
        assert_eq!(robots_portal_neighbor_zone(&portal, 2, 8), Some(7));
        assert_eq!(robots_portal_neighbor_zone(&portal, 7, 8), Some(2));
        assert_eq!(robots_portal_neighbor_zone(&portal, 3, 8), None);
        assert_eq!(robots_portal_neighbor_zone(&portal, 2, 7), None);
    }

    #[test]
    fn native_portal_clip_respects_winding_and_view_frustum() {
        let projection = glam::camera::rh::proj::directx::perspective(
            60.0f32.to_radians(),
            16.0 / 9.0,
            0.02,
            2000.0,
        );
        let mut view_projection = projection * Mat4::IDENTITY;
        view_projection.x_axis = -view_projection.x_axis;
        let portal = ProcessedPortal {
            map_a: 0,
            map_b: 1,
            flags: 0,
            distance: 5.0,
            vertices: [
                Vec3::new(-1.0, -1.0, -5.0),
                Vec3::new(1.0, -1.0, -5.0),
                Vec3::new(1.0, 1.0, -5.0),
                Vec3::new(-1.0, 1.0, -5.0),
            ],
            face_common: 0,
            face_texture_ref: 0,
            face_flags: 0,
            face_vertices: vec![],
        };
        let front = native_portal_clip(&portal, 0, NativePortalClip::FULL, view_projection);
        let back = native_portal_clip(&portal, 1, NativePortalClip::FULL, view_projection);
        assert_ne!(front.is_some(), back.is_some());

        let outside = ProcessedPortal {
            vertices: portal
                .vertices
                .map(|vertex| vertex + Vec3::new(1000.0, 0.0, 0.0)),
            ..portal
        };
        assert!(native_portal_clip(&outside, 0, NativePortalClip::FULL, view_projection).is_none());
        assert!(native_portal_clip(&outside, 1, NativePortalClip::FULL, view_projection).is_none());
    }

    #[test]
    fn real_map_visual_zone_exclusion_mask_corpus_when_requested() {
        let Ok(root) = std::env::var("EUROCHEF_REAL_MAP_CORPUS_ROOT") else {
            return;
        };

        fn collect_edb_paths(root: &Path, output: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(root) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_edb_paths(&path, output);
                } else if path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("edb"))
                {
                    output.push(path);
                }
            }
        }

        let mut paths = Vec::new();
        collect_edb_paths(Path::new(&root), &mut paths);
        paths.sort();

        let mut map_count = 0usize;
        let mut zone_count = 0usize;
        let mut nonzero_zone_masks = Vec::new();
        for path in paths {
            let Ok(file) = File::open(&path) else {
                continue;
            };
            let Ok(mut edb) = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc) else {
                continue;
            };
            for map in read_from_file(&mut edb) {
                map_count += 1;
                zone_count += map.zones.len();
                for (zone_index, zone) in map.zones.iter().enumerate() {
                    if zone
                        .visual_zone_exclusion_mask
                        .iter()
                        .any(|word| *word != 0)
                    {
                        nonzero_zone_masks.push((
                            path.file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_else(|| path.display().to_string()),
                            zone_index,
                            zone.visual_zone_exclusion_mask,
                            map.zones.len(),
                        ));
                    }
                }
            }
        }

        eprintln!(
            "VIS_ZONE_EXCLUSION_CORPUS maps={map_count} zones={zone_count} nonzero_masks={}",
            nonzero_zone_masks.len()
        );
        for (file, zone, mask, zones) in &nonzero_zone_masks {
            eprintln!("  {file} zone={zone}/{zones} mask={mask:08X?}");
        }

        assert_eq!(map_count, 18);
        assert_eq!(zone_count, 351);
        let compact = nonzero_zone_masks
            .iter()
            .map(|(file, zone, mask, zones)| (file.as_str(), *zone, mask[0], *zones))
            .collect::<Vec<_>>();
        assert_eq!(
            compact,
            [
                ("m04_cour.edb", 7, 0x0000_3800, 15),
                ("m04_cour.edb", 8, 0x0000_3800, 15),
                ("m04_cour.edb", 9, 0x0000_3000, 15),
                ("m04_cour.edb", 11, 0x0000_0080, 15),
                ("m04_cour.edb", 12, 0x0000_0380, 15),
                ("m04_cour.edb", 13, 0x0000_0380, 15),
            ]
        );
        assert!(nonzero_zone_masks
            .iter()
            .all(|(_, _, mask, _)| mask[1..] == [0; 7]));
    }

    #[test]
    fn real_map_cross_submap_portal_corpus_when_requested() {
        let Ok(root) = std::env::var("EUROCHEF_REAL_MAP_CORPUS_ROOT") else {
            return;
        };

        fn collect_edb_paths(root: &Path, output: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(root) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_edb_paths(&path, output);
                } else if path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("edb"))
                {
                    output.push(path);
                }
            }
        }

        let mut paths = Vec::new();
        collect_edb_paths(Path::new(&root), &mut paths);
        paths.sort();

        let mut multi_map_files = Vec::new();
        let mut external_portal_rows = Vec::new();
        let mut external_info_rows = Vec::new();
        for path in paths {
            let Ok(file) = File::open(&path) else {
                continue;
            };
            let Ok(mut edb) = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc) else {
                continue;
            };
            let maps = read_from_file(&mut edb);
            let file_name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            if maps.len() > 1 {
                multi_map_files.push((file_name.clone(), maps.len()));
            }
            for (map_index, map) in maps.iter().enumerate() {
                for (portal_index, portal) in map.portals.iter().enumerate() {
                    if portal.map_a == u16::MAX || portal.map_b == u16::MAX {
                        external_portal_rows.push((
                            file_name.clone(),
                            map_index,
                            map.hashcode,
                            portal_index,
                            portal.map_a,
                            portal.map_b,
                        ));
                    }
                }
                for (zone_index, zone) in map.zones.iter().enumerate() {
                    if let Some(infos) = zone.unk18.as_ref() {
                        for (info_index, info) in infos.data().iter().enumerate() {
                            if info.map_to == u16::MAX {
                                external_info_rows.push((
                                    file_name.clone(),
                                    map_index,
                                    map.hashcode,
                                    zone_index,
                                    info_index,
                                    info.index,
                                    info.map_on,
                                ));
                            }
                        }
                    }
                }
            }
        }

        eprintln!("CROSS_SUBMAP multi_map_files={multi_map_files:?}");
        eprintln!(
            "CROSS_SUBMAP external_portals={}",
            external_portal_rows.len()
        );
        for row in &external_portal_rows {
            eprintln!("  portal {row:?}");
        }
        eprintln!("CROSS_SUBMAP external_infos={}", external_info_rows.len());
        for row in &external_info_rows {
            eprintln!("  info {row:?}");
        }

        assert!(multi_map_files.is_empty());
        assert!(external_portal_rows.is_empty());
        assert!(external_info_rows.is_empty());
    }

    #[test]
    fn real_map_zone_resource_fields_corpus_when_requested() {
        let Ok(root) = std::env::var("EUROCHEF_REAL_MAP_CORPUS_ROOT") else {
            return;
        };

        fn collect_edb_paths(root: &Path, output: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(root) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_edb_paths(&path, output);
                } else if path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("edb"))
                {
                    output.push(path);
                }
            }
        }

        let mut paths = Vec::new();
        collect_edb_paths(Path::new(&root), &mut paths);
        paths.sort();

        let mut zone_count = 0usize;
        let mut zero_resource_refs = 0usize;
        let mut nonzero_masks = 0usize;
        let mut resource_refs = std::collections::BTreeMap::<u32, usize>::new();
        let mut mask_rows = Vec::new();
        for path in paths {
            let Ok(file) = File::open(&path) else {
                continue;
            };
            let Ok(mut edb) = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc) else {
                continue;
            };
            let file_name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            for map in read_from_file(&mut edb) {
                for (zone_index, zone) in map.zones.iter().enumerate() {
                    zone_count += 1;
                    let resource_ref = zone.zone_resource_ref;
                    *resource_refs.entry(resource_ref).or_default() += 1;
                    if resource_ref == 0 {
                        zero_resource_refs += 1;
                    }
                    let mask = zone.stream_resource_mask;
                    if resource_ref != 0 {
                        assert_eq!(resource_ref & 0xFF00_0000, 0x0800_0000);
                        let resource_index = (resource_ref & 0x00FF_FFFF) as usize;
                        assert!(resource_index < 128);
                        assert_ne!(
                            mask[resource_index / 32] & (1u32 << (resource_index & 31)),
                            0
                        );
                    }
                    if mask.iter().any(|word| *word != 0) {
                        nonzero_masks += 1;
                        mask_rows.push((file_name.clone(), zone_index, resource_ref, mask));
                    }
                }
            }
        }

        eprintln!(
            "ZONE_RESOURCE_CORPUS zones={zone_count} zero_refs={zero_resource_refs} unique_refs={} nonzero_masks={nonzero_masks}",
            resource_refs.len()
        );
        eprintln!("ZONE_RESOURCE_REFS {resource_refs:08X?}");
        for row in mask_rows.iter().take(64) {
            eprintln!("  resource {row:08X?}");
        }

        assert_eq!(zone_count, 351);
        assert!(resource_refs.len() > 1);
        assert!(nonzero_masks > 0);
    }

    #[test]
    fn real_map_sky_identifier_bit_zero_corpus_when_requested() {
        let Ok(root) = std::env::var("EUROCHEF_REAL_MAP_CORPUS_ROOT") else {
            return;
        };

        fn collect_edb_paths(root: &Path, output: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(root) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_edb_paths(&path, output);
                } else if path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("edb"))
                {
                    output.push(path);
                }
            }
        }

        let mut paths = Vec::new();
        collect_edb_paths(Path::new(&root), &mut paths);
        paths.sort();
        let mut flagged = Vec::new();
        for path in paths {
            let Ok(file) = File::open(&path) else {
                continue;
            };
            let Ok(mut edb) = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc) else {
                continue;
            };
            let file_name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            for map in read_from_file(&mut edb) {
                for (zone_index, zone) in map.zones.iter().enumerate() {
                    if zone.identifier.flags & 1 != 0 {
                        flagged.push((
                            file_name.clone(),
                            zone_index,
                            zone.identifier.flags,
                            zone.identifier.sky_index,
                            zone.identifier.sky_anchor_y,
                        ));
                    }
                }
            }
        }

        assert!(
            flagged.is_empty(),
            "unexpected identifier bit0 zones: {flagged:?}"
        );
    }

    #[test]
    fn real_map_native_fog_corpus_when_requested() {
        let Ok(root) = std::env::var("EUROCHEF_REAL_MAP_CORPUS_ROOT") else {
            return;
        };

        fn collect_edb_paths(root: &Path, output: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(root) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_edb_paths(&path, output);
                } else if path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("edb"))
                {
                    output.push(path);
                }
            }
        }

        let mut paths = Vec::new();
        collect_edb_paths(Path::new(&root), &mut paths);
        paths.sort();
        let mut zone_count = 0usize;
        let mut enabled_count = 0usize;
        let mut methods = std::collections::BTreeSet::new();
        for path in paths {
            let Ok(file) = File::open(&path) else {
                continue;
            };
            let Ok(mut edb) = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc) else {
                continue;
            };
            for map in read_from_file(&mut edb) {
                for zone in &map.zones {
                    zone_count += 1;
                    let identifier = &zone.identifier;
                    methods.insert(identifier.fog_method);
                    if identifier.fog_method == 0 {
                        continue;
                    }
                    enabled_count += 1;
                    assert!(identifier.fog_near.is_finite());
                    assert!(identifier.fog_far.is_finite());
                    assert!(identifier.fog_min.is_finite());
                    assert!(identifier.fog_max.is_finite());
                    assert!(
                        (identifier.fog_far - identifier.fog_near).abs() > f32::EPSILON,
                        "enabled native fog has a zero near/far span"
                    );
                }
            }
        }

        eprintln!(
            "NATIVE_FOG_CORPUS zones={zone_count} enabled={enabled_count} methods={methods:?}"
        );
        assert_eq!(zone_count, 351);
        assert!(enabled_count > 0);
    }

    #[test]
    fn real_map_identifier_ambience_corpus_when_requested() {
        let Ok(root) = std::env::var("EUROCHEF_REAL_MAP_CORPUS_ROOT") else {
            return;
        };

        fn collect_edb_paths(root: &Path, output: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(root) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_edb_paths(&path, output);
                } else if path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("edb"))
                {
                    output.push(path);
                }
            }
        }

        let mut paths = Vec::new();
        collect_edb_paths(Path::new(&root), &mut paths);
        paths.sort();
        let mut zone_count = 0usize;
        let mut nonzero_count = 0usize;
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        let mut values = std::collections::BTreeMap::<u32, usize>::new();
        let mut examples = Vec::new();
        for path in paths {
            let Ok(file) = File::open(&path) else {
                continue;
            };
            let Ok(mut edb) = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc) else {
                continue;
            };
            let file_name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            for map in read_from_file(&mut edb) {
                for (zone_index, zone) in map.zones.iter().enumerate() {
                    let ambience = zone.identifier.ambience;
                    zone_count += 1;
                    assert!(ambience.is_finite());
                    min = min.min(ambience);
                    max = max.max(ambience);
                    *values.entry(ambience.to_bits()).or_default() += 1;
                    if ambience != 0.0 {
                        nonzero_count += 1;
                        if examples.len() < 32 {
                            examples.push((file_name.clone(), zone_index, ambience));
                        }
                    }
                }
            }
        }

        eprintln!(
            "IDENTIFIER_AMBIENCE_CORPUS zones={zone_count} nonzero={nonzero_count} unique={} min={min} max={max}",
            values.len()
        );
        eprintln!("IDENTIFIER_AMBIENCE_VALUES {values:08X?}");
        eprintln!("IDENTIFIER_AMBIENCE_EXAMPLES {examples:?}");
        assert_eq!(zone_count, 351);
    }

    #[test]
    fn robots_npc_uses_runtime_proven_getters_and_alternate_cutscene_slots() {
        let data = [
            Some(7),
            Some(0x0B00_0000),
            Some(0x0000_0340),
            Some(0x4508_0005),
            Some(0x0400_00E5),
            Some(0x0400_00E6),
            Some(0x0400_0000),
            Some(0x0400_00E8),
        ];
        assert_eq!(robots_npc_runtime_selector(48, &data), Some(7));
        assert_eq!(robots_npc_runtime_uid(48, &data), Some(0x0B00_0000));
        assert_eq!(robots_npc_flags(48, &data), Some(0x340));
        assert_eq!(robots_npc_text_group(48, &data), Some(0x4508_0005));
        assert_eq!(
            robots_npc_alternate_cutscenes(48, &data),
            Some([
                Some(0x0400_00E5),
                Some(0x0400_00E6),
                Some(0x0400_0000),
                Some(0x0400_00E8),
            ])
        );
        assert!(robots_npc_cutscene_is_null(0x0400_0000));
        assert!(!robots_npc_cutscene_is_null(0x0400_00E5));
        assert_eq!(robots_npc_runtime_selector(47, &data), None);
    }

    #[test]
    fn robots_watchbot_uses_runtime_proven_path_and_hysteresis_slots() {
        let mut watchbot = vec![None; 16];
        watchbot[0] = Some(3);
        watchbot[1] = Some(0x0B00_0035);
        watchbot[2] = Some(3);
        watchbot[3] = Some(20);
        watchbot[4] = Some(30);

        assert_eq!(robots_trigger_path_hash(60, &watchbot), Some(0x0B00_0035));
        assert_eq!(robots_trigger_path_data_slot(60), Some(1));
        assert_eq!(robots_watchbot_mode(60, &watchbot), Some(3));
        assert_eq!(robots_watchbot_flags(60, &watchbot), Some(3));
        assert_eq!(robots_watchbot_enter_distance(60, &watchbot), Some(2.0));
        assert_eq!(robots_watchbot_leave_distance(60, &watchbot), Some(3.0));
        assert_eq!(robots_trigger_runtime_path_speed(60, &watchbot), None);
    }

    #[test]
    fn robots_ratchet_and_transporter_use_runtime_proven_path_context_slots() {
        let mut ratchet = vec![None; 16];
        ratchet[0] = Some(0x0B00_001F);
        assert_eq!(robots_trigger_path_hash(72, &ratchet), Some(0x0B00_001F));
        assert_eq!(robots_trigger_path_data_slot(72), Some(0));
        assert!(robots_trigger_path_is_proven(72, &ratchet, 0x0B00_001F));
        assert_eq!(robots_trigger_runtime_path_speed(72, &ratchet), None);

        let mut transporter = vec![None; 16];
        transporter[1] = Some(0x0B00_0094);
        transporter[4] = Some(0x0B00_0093);
        assert_eq!(
            robots_trigger_path_hash(73, &transporter),
            Some(0x0B00_0094)
        );
        assert_eq!(robots_trigger_path_data_slot(73), Some(1));
        assert_eq!(
            robots_monster_transporter_secondary_path_hash(73, &transporter),
            Some(0x0B00_0093)
        );
        assert!(robots_trigger_path_is_proven(73, &transporter, 0x0B00_0094));
        assert!(robots_trigger_path_is_proven(73, &transporter, 0x0B00_0093));
        assert_eq!(robots_trigger_runtime_path_speed(73, &transporter), None);

        transporter[4] = Some(0x0B00_0000);
        assert_eq!(
            robots_monster_transporter_secondary_path_hash(73, &transporter),
            None
        );
    }

    #[test]
    fn robots_monster_family_uses_runtime_proven_getters() {
        let mut monster = vec![None; 16];
        monster[0] = Some(7);
        monster[1] = Some(25);
        monster[2] = Some(0x0B00_003C);
        monster[4] = Some(4);
        monster[7] = Some(0xC000);
        monster[15] = Some(9);

        assert!(robots_monster_is_family(10));
        assert!(robots_monster_is_family(3));
        assert!(robots_monster_is_family(70));
        assert!(!robots_monster_is_family(48));
        assert_eq!(robots_monster_runtime_selector(10, &monster), Some(7));
        assert_eq!(robots_monster_proximity_radius(10, &monster), Some(2.5));
        assert_eq!(robots_trigger_path_hash(10, &monster), Some(0x0B00_003C));
        assert_eq!(robots_trigger_path_data_slot(10), Some(2));
        assert_eq!(robots_monster_data4_value(10, &monster), Some(4));
        assert_eq!(robots_monster_flags(10, &monster), Some(0xC000));
        assert_eq!(robots_monster_data15_value(10, &monster), Some(9));

        assert_eq!(robots_monster_test_runtime_value(3, &monster), Some(25));
        assert_eq!(robots_monster_proximity_radius(3, &monster), None);
        assert_eq!(robots_trigger_path_hash(3, &monster), None);
        assert_eq!(robots_trigger_path_data_slot(3), None);
    }

    #[test]
    fn robots_monster_path_is_proven_but_boss_sewer_path_like_value_is_not() {
        let mut monster = vec![None; 16];
        monster[2] = Some(0x0B00_003C);
        assert_eq!(robots_trigger_path_hash(74, &monster), Some(0x0B00_003C));
        assert_eq!(robots_trigger_path_data_slot(74), Some(2));
        assert!(robots_trigger_path_is_proven(74, &monster, 0x0B00_003C));
        assert_eq!(robots_trigger_runtime_path_speed(74, &monster), None);

        let mut boss_sewer = vec![None; 16];
        boss_sewer[0] = Some(0x0B00_0059);
        assert_eq!(robots_trigger_path_hash(75, &boss_sewer), None);
        assert_eq!(robots_trigger_path_data_slot(75), None);
        assert!(!robots_trigger_path_is_proven(75, &boss_sewer, 0x0B00_0059));
    }

    #[test]
    fn robots_runtime_path_sentinels_are_not_treated_as_paths() {
        for sentinel in [0, u32::MAX, 0x0B00_0000] {
            let mut data = vec![None; 16];
            data[2] = Some(sentinel);
            assert_eq!(robots_trigger_path_hash(8, &data), None);
        }
    }

    #[test]
    fn robots_runtime_path_speed_uses_each_controller_serialized_field() {
        let mut lift = vec![None; 16];
        lift[3] = Some(4.0f32.to_bits());
        lift[4] = Some(30);
        assert_eq!(robots_trigger_runtime_path_speed(37, &lift), Some(3.0));
        assert_eq!(
            robots_trigger_runtime_path_acceleration(37, &lift),
            Some(0.4)
        );

        let mut vehicle = vec![None; 16];
        vehicle[2] = Some(70.0f32.to_bits());
        assert_eq!(robots_trigger_runtime_path_speed(80, &vehicle), Some(7.0));

        let mut platform = vec![None; 16];
        platform[5] = Some(25.0f32.to_bits());
        platform[6] = Some(2.0f32.to_bits());
        assert_eq!(robots_trigger_runtime_path_speed(8, &platform), Some(2.5));
        assert_eq!(
            robots_trigger_runtime_path_acceleration(8, &platform),
            Some(0.2)
        );

        assert_eq!(
            robots_trigger_runtime_path_speed(37, &[None; 16]),
            Some(1.0)
        );
    }

    #[test]
    fn robots_platform_rotation_uses_runtime_proven_serialized_slots() {
        let mut data = vec![None; 16];
        data[1] = Some((-5.0f32).to_bits());
        data[3] = Some(10.0f32.to_bits());
        data[4] = Some(2.5f32.to_bits());

        assert_eq!(
            robots_trigger_platform_angular_velocity(8, &data),
            Some(Vec3::new(10.0, 2.5, -5.0))
        );
        assert_eq!(robots_trigger_platform_angular_velocity(37, &data), None);
    }

    #[test]
    fn robots_native_light_types_are_decoded_as_feature_masks() {
        assert_eq!(robots_native_light_type_description(1), "range");
        assert_eq!(
            robots_native_light_type_description(3),
            "range + position-normal"
        );
        assert_eq!(robots_native_light_type_description(5), "range + beam-cone");
        assert_eq!(
            robots_native_light_type_description(7),
            "range + position-normal + beam-cone"
        );
        assert_eq!(
            robots_native_light_type_description(11),
            "range + position-normal + beam-normal"
        );
        assert_eq!(
            robots_native_light_type_description(0x21),
            "range + unknown-bits + 0x0020"
        );
    }

    #[test]
    fn robots_native_light_colour_uses_the_runtime_one_over_128_scale() {
        let colour = robots_native_light_colour([0x80, 0x40, 0x20, 0xff]);
        assert_eq!(colour, Vec3::new(1.0, 0.5, 0.25));

        let full_red = robots_native_light_colour([0xff, 0, 0, 0xff]);
        assert!((full_red.x - 255.0 / 128.0).abs() < f32::EPSILON);
        assert_eq!(full_red.y, 0.0);
        assert_eq!(full_red.z, 0.0);
    }

    #[test]
    fn real_audio_corpus_when_fixture_is_requested() {
        let Ok(path) = std::env::var("EUROCHEF_REAL_AUDIO_EDB") else {
            eprintln!("SKIP real_audio_corpus_when_fixture_is_requested: EUROCHEF_REAL_AUDIO_EDB is not set");
            return;
        };

        let open_edb = || {
            let file = File::open(&path).expect("real audio EDB fixture is missing");
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .expect("real audio EDB fixture is not a valid PC EDB")
        };

        let mut map_edb = open_edb();
        let maps = read_from_file(&mut map_edb);
        assert!(!maps.is_empty(), "real audio EDB has no maps");

        let map_sound_count = maps.iter().map(|map| map.sounds.len()).sum::<usize>();
        let spatial_sound_count = maps
            .iter()
            .flat_map(|map| &map.sounds)
            .filter(|sound| sound.outer_radius > 0.0)
            .count();
        let tracking_type_counts = maps.iter().flat_map(|map| &map.sounds).fold(
            std::collections::BTreeMap::<u8, usize>::new(),
            |mut counts, sound| {
                *counts.entry(sound.tracking_type).or_default() += 1;
                counts
            },
        );
        let zone_sound_references = maps
            .iter()
            .flat_map(|map| &map.zones)
            .map(|zone| zone.sound_array.len())
            .sum::<usize>();
        let invalid_zone_sound_indices = maps
            .iter()
            .map(|map| {
                map.zones
                    .iter()
                    .flat_map(|zone| zone.sound_array.iter())
                    .filter(|index| **index as usize >= map.sounds.len())
                    .count()
            })
            .sum::<usize>();

        let mut script_edb = open_edb();
        let scripts = UXGeoScript::read_all(&mut script_edb).expect("could not read real scripts");
        let script_sound_commands = scripts
            .iter()
            .flat_map(|script| &script.commands)
            .filter(|command| matches!(command.data, UXGeoScriptCommandData::Sound { .. }))
            .count();
        let referenced_sound_hashes = maps
            .iter()
            .flat_map(|map| &map.sounds)
            .map(|sound| sound.sound_ref)
            .chain(
                scripts
                    .iter()
                    .flat_map(|script| &script.commands)
                    .filter_map(|command| match &command.data {
                        UXGeoScriptCommandData::Sound { hashcode } => Some(*hashcode),
                        _ => None,
                    }),
            )
            .collect::<std::collections::BTreeSet<_>>();
        let referenced_sound_hash_list = referenced_sound_hashes
            .iter()
            .map(|hashcode| format!("0x{hashcode:08x}"))
            .collect::<Vec<_>>()
            .join(",");

        assert!(map_sound_count > 0, "real map has no EXGeoSound emitters");
        assert!(
            zone_sound_references > 0,
            "real MapZone data has no sound references"
        );
        assert_eq!(
            invalid_zone_sound_indices, 0,
            "MapZone.sound_array contains invalid indices"
        );
        assert!(
            script_sound_commands > 0,
            "real scripts contain no Sound commands"
        );

        let report = format!(
            "key\tvalue\nmaps\t{}\nmap_sounds\t{}\nspatial_sounds_by_radius\t{}\nzone_sound_references\t{}\ntracking_type_counts\t{:?}\ninvalid_zone_sound_indices\t{}\nscripts\t{}\nscript_sound_commands\t{}\nunique_referenced_sound_hashes\t{}\nreferenced_sound_hashes\t{}\n",
            maps.len(),
            map_sound_count,
            spatial_sound_count,
            zone_sound_references,
            tracking_type_counts,
            invalid_zone_sound_indices,
            scripts.len(),
            script_sound_commands,
            referenced_sound_hashes.len(),
            referenced_sound_hash_list,
        );
        eprintln!("{report}");
        if let Ok(output) = std::env::var("EUROCHEF_REAL_AUDIO_REPORT") {
            if let Some(parent) = std::path::Path::new(&output).parent() {
                std::fs::create_dir_all(parent).expect("could not create audio report folder");
            }
            std::fs::write(output, report).expect("could not write real audio report");
        }
    }

    #[test]
    fn real_audio_manifest_corpus_when_requested() {
        let Ok(manifest_path) = std::env::var("EUROCHEF_REAL_AUDIO_MANIFEST") else {
            eprintln!("SKIP real_audio_manifest_corpus_when_requested: EUROCHEF_REAL_AUDIO_MANIFEST is not set");
            return;
        };
        let manifest =
            std::fs::read_to_string(&manifest_path).expect("real audio manifest could not be read");
        let mut references = std::collections::BTreeSet::<u32>::new();
        let mut edb_count = 0usize;
        let mut map_count = 0usize;
        let mut map_sound_count = 0usize;
        let mut zone_sound_reference_count = 0usize;
        let mut script_count = 0usize;
        let mut script_sound_command_count = 0usize;
        let mut failures = Vec::<String>::new();

        for source_path in audio_manifest_edb_paths(&manifest_path, &manifest) {
            edb_count += 1;

            let open_edb = || -> Result<EdbFile, String> {
                let file = File::open(&source_path)
                    .map_err(|error| format!("{}: {error}", source_path.display()))?;
                EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                    .map_err(|error| format!("{}: {error}", source_path.display()))
            };

            match open_edb() {
                Ok(mut edb) => {
                    let maps = read_from_file(&mut edb);
                    map_count += maps.len();
                    for map in maps {
                        map_sound_count += map.sounds.len();
                        references.extend(map.sounds.iter().map(|sound| sound.sound_ref));
                        zone_sound_reference_count += map
                            .zones
                            .iter()
                            .map(|zone| zone.sound_array.len())
                            .sum::<usize>();
                    }
                }
                Err(error) => failures.push(error),
            }

            match open_edb() {
                Ok(mut edb) => match UXGeoScript::read_all(&mut edb) {
                    Ok(scripts) => {
                        script_count += scripts.len();
                        for command in scripts.iter().flat_map(|script| &script.commands) {
                            if let UXGeoScriptCommandData::Sound { hashcode } = command.data {
                                script_sound_command_count += 1;
                                references.insert(hashcode);
                            }
                        }
                    }
                    Err(error) => {
                        failures.push(format!("{} scripts: {error}", source_path.display()))
                    }
                },
                Err(error) => failures.push(error),
            }
        }

        assert_eq!(edb_count, 179, "unexpected Robots manifest EDB count");
        assert!(map_count > 0, "manifest corpus has no maps");
        assert!(map_sound_count > 0, "manifest corpus has no map sounds");
        assert!(
            script_sound_command_count > 0,
            "manifest corpus has no script sounds"
        );
        assert!(
            failures.is_empty(),
            "manifest audio parse failures: {failures:?}"
        );

        let reference_list = references
            .iter()
            .map(|hashcode| format!("0x{hashcode:08x}"))
            .collect::<Vec<_>>()
            .join(",");
        let report = format!(
            "key\tvalue\nedbs\t{}\nmaps\t{}\nmap_sounds\t{}\nzone_sound_references\t{}\nscripts\t{}\nscript_sound_commands\t{}\nunique_audio_references\t{}\naudio_references\t{}\nparse_failures\t{}\n",
            edb_count,
            map_count,
            map_sound_count,
            zone_sound_reference_count,
            script_count,
            script_sound_command_count,
            references.len(),
            reference_list,
            failures.len(),
        );
        eprintln!("{report}");
        if let Ok(output) = std::env::var("EUROCHEF_REAL_AUDIO_MANIFEST_REPORT") {
            if let Some(parent) = std::path::Path::new(&output).parent() {
                std::fs::create_dir_all(parent)
                    .expect("could not create manifest audio report folder");
            }
            std::fs::write(output, report).expect("could not write manifest audio report");
        }
    }

    #[test]
    fn real_map_runtime_sections_manifest_when_requested() {
        let Ok(source) = std::env::var("EUROCHEF_REAL_MAP_RUNTIME_MANIFEST") else {
            return;
        };
        let source = PathBuf::from(source);
        let source_paths = if source.is_dir() {
            let mut pending = vec![source.clone()];
            let mut paths = std::collections::BTreeSet::new();
            while let Some(directory) = pending.pop() {
                for entry in std::fs::read_dir(&directory)
                    .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
                {
                    let entry = entry.expect("map runtime corpus directory entry is invalid");
                    let path = entry.path();
                    if path.is_dir() {
                        pending.push(path);
                    } else if path
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("edb"))
                    {
                        paths.insert(path);
                    }
                }
            }
            paths
        } else {
            let manifest = std::fs::read_to_string(&source)
                .expect("real map runtime manifest could not be read");
            let manifest_base = source.parent().unwrap_or_else(|| Path::new("."));
            let mut manifest_lines = manifest.lines();
            let header = manifest_lines
                .next()
                .expect("real map runtime manifest has no header")
                .split('\t')
                .collect::<Vec<_>>();
            let path_column = header
                .iter()
                .position(|column| {
                    let normalized = column.trim().to_ascii_lowercase().replace(' ', "_");
                    matches!(
                        normalized.as_str(),
                        "source_edb"
                            | "source_path"
                            | "physical_path"
                            | "path"
                            | "file_name"
                            | "edb_path"
                    )
                })
                .expect("real map runtime manifest has no EDB path column");
            manifest_lines
                .filter_map(|line| {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        return None;
                    }
                    let source_text = line.split('\t').nth(path_column)?.trim();
                    if source_text.is_empty() {
                        return None;
                    }
                    let source_path = PathBuf::from(source_text);
                    Some(if source_path.is_absolute() {
                        source_path
                    } else {
                        manifest_base.join(source_path)
                    })
                })
                .collect::<std::collections::BTreeSet<_>>()
        };

        let mut edb_count = 0usize;
        let mut map_count = 0usize;
        let mut camera_count = 0usize;
        let mut portal_count = 0usize;
        let mut placement_group_count = 0usize;
        let mut isound_count = 0usize;
        let mut maps_with_cameras = 0usize;
        let mut maps_with_portals = 0usize;
        let mut trigger_camera_count = 0usize;
        let mut camera_controller_plan_count = 0usize;
        let mut camera_mode_counts = std::collections::BTreeMap::<u32, usize>::new();
        let mut camera_plans_with_marker = 0usize;

        let mut portal_endpoint_in_zone_range = 0usize;
        let mut portal_endpoint_outside_zone_range = 0usize;
        let mut portal_self_pairs = 0usize;
        let mut portal_endpoint_max = 0u16;
        let mut failures = Vec::<String>::new();

        for source_path in source_paths {
            edb_count += 1;
            let file = match File::open(&source_path) {
                Ok(file) => file,
                Err(error) => {
                    failures.push(format!("{}: {error}", source_path.display()));
                    continue;
                }
            };
            let mut edb = match EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc) {
                Ok(edb) => edb,
                Err(error) => {
                    failures.push(format!("{}: {error}", source_path.display()));
                    continue;
                }
            };
            let maps = read_from_file(&mut edb);
            map_count += maps.len();
            for map in maps {
                camera_count += map.cameras.len();
                portal_count += map.portals.len();
                placement_group_count += map.placement_group_count;
                isound_count += map.isounds.len();
                maps_with_cameras += usize::from(!map.cameras.is_empty());
                maps_with_portals += usize::from(!map.portals.is_empty());
                for (trigger_index, trigger) in map.triggers.iter().enumerate() {
                    if trigger.ttype != 1 {
                        continue;
                    }
                    trigger_camera_count += 1;
                    if let Some(plan) = robots_camera_controller_plan(&map, trigger_index) {
                        camera_controller_plan_count += 1;
                        *camera_mode_counts.entry(plan.mode).or_default() += 1;
                        camera_plans_with_marker += usize::from(plan.linked_marker_index.is_some());
                        if plan.mode == 4 {
                            let path = plan.path_hashcode.and_then(|hashcode| {
                                map.paths.iter().find(|path| path.hashcode == hashcode)
                            });
                            assert!(
                                path.is_some_and(|path| {
                                    path.path_type == 0
                                        && path.flags & 0x2000_0000 != 0
                                        && path.links.is_empty()
                                        && !path.nodes.is_empty()
                                }),
                                "CAMERA_MODE4 file={} trigger={} path={:?} type={:?} flags={:?} nodes={:?} links={:?} data6={:?} data7={:?} opts=0x{:X}",
                                source_path.file_name().and_then(|name| name.to_str()).unwrap_or("?"),
                                trigger_index,
                                plan.path_hashcode.map(|value| format!("0x{value:08X}")),
                                path.map(|path| path.path_type),
                                path.map(|path| path.flags),
                                path.map(|path| path.nodes.len()),
                                path.map(|path| path.links.len()),
                                plan.mode4_data6,
                                plan.mode4_data7,
                                plan.mode4_option_flags,
                            );
                        }
                    }
                }
                for portal in &map.portals {
                    portal_endpoint_max = portal_endpoint_max.max(portal.map_a).max(portal.map_b);
                    portal_self_pairs += usize::from(portal.map_a == portal.map_b);
                    for endpoint in [portal.map_a, portal.map_b] {
                        if (endpoint as usize) < map.zones.len() {
                            portal_endpoint_in_zone_range += 1;
                        } else {
                            portal_endpoint_outside_zone_range += 1;
                        }
                    }
                }
                assert!(map.cameras.iter().all(|camera| {
                    camera.position.is_finite()
                        && camera.look.is_finite()
                        && camera.focal_length.is_finite()
                        && camera.aperture_width.is_finite()
                        && camera.aperture_height.is_finite()
                }));
                assert!(map.portals.iter().all(|portal| {
                    portal.distance.is_finite()
                        && portal.vertices.iter().all(|vertex| vertex.is_finite())
                        && portal.face_vertices.iter().all(|vertex| vertex.is_finite())
                }));
            }
        }

        assert_eq!(edb_count, 179, "unexpected Robots manifest EDB count");
        assert_eq!(map_count, 18, "unexpected Robots map count");
        assert_eq!(
            camera_count, 0,
            "shipped PC maps unexpectedly use EXGeoCamera"
        );
        assert_eq!(portal_count, 402, "unexpected EXGeoPortal count");
        assert_eq!(maps_with_portals, 12, "unexpected maps-with-portals count");
        assert_eq!(
            placement_group_count, 0,
            "shipped PC maps unexpectedly use placement groups"
        );
        assert_eq!(
            isound_count, 0,
            "shipped PC maps unexpectedly use EXGeoMap.isounds"
        );
        assert_eq!(trigger_camera_count, 45, "unexpected XTrigger_Camera count");
        assert_eq!(
            camera_controller_plan_count, trigger_camera_count,
            "every shipped Camera must produce a native controller plan"
        );
        assert_eq!(
            camera_mode_counts,
            std::collections::BTreeMap::from([(0, 5), (3, 22), (4, 18)]),
            "unexpected shipped Camera mode census"
        );
        assert_eq!(
            camera_plans_with_marker, 27,
            "unexpected Camera/Marker plan count"
        );
        assert_eq!(portal_endpoint_in_zone_range, portal_count * 2);
        assert_eq!(portal_endpoint_outside_zone_range, 0);
        assert_eq!(portal_self_pairs, 0);
        eprintln!(
            "camera plans: triggers={} plans={} modes={:?} with_marker={}; portal endpoints: in_zone_range={} outside_zone_range={} self_pairs={} max={}",
            trigger_camera_count,
            camera_controller_plan_count,
            camera_mode_counts,
            camera_plans_with_marker,
            portal_endpoint_in_zone_range,
            portal_endpoint_outside_zone_range,
            portal_self_pairs,
            portal_endpoint_max,
        );
        assert!(
            failures.is_empty(),
            "map runtime parse failures: {failures:?}"
        );
        eprintln!(
            "map runtime corpus: edbs={} maps={} cameras={} maps_with_cameras={} portals={} maps_with_portals={} placement_groups={} isounds={} failures={}",
            edb_count,
            map_count,
            camera_count,
            maps_with_cameras,
            portal_count,
            maps_with_portals,
            placement_group_count,
            isound_count,
            failures.len(),
        );
    }

    #[test]
    fn real_m02_city_router_triggers_preserve_serialized_type_codes_when_requested() {
        let Ok(path) = std::env::var("EUROCHEF_REAL_M02_CITY_EDB") else {
            return;
        };
        let file = File::open(&path).expect("m02_city fixture is missing");
        let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
            .expect("m02_city fixture is not a valid PC EDB");
        let maps = read_from_file(&mut edb);
        let map = maps.first().expect("m02_city map is missing");
        for (trigger_index, expected_type) in [
            (148usize, 5u32),
            (202, 43),
            (347, 5),
            (581, 14),
            (655, 14),
            (669, 13),
        ] {
            let trigger = map
                .triggers
                .get(trigger_index)
                .unwrap_or_else(|| panic!("m02_city trigger {trigger_index} is missing"));
            assert_eq!(
                trigger.ttype, expected_type,
                "ProcessedMap must preserve serialized trigger type at index {trigger_index}"
            );
        }
    }

    #[test]
    fn real_m02_city_map_runtime_sections_when_requested() {
        let Ok(path) = std::env::var("EUROCHEF_REAL_M02_CITY_EDB") else {
            return;
        };
        let file = File::open(&path).expect("m02_city fixture is missing");
        let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
            .expect("m02_city fixture is not a valid PC EDB");
        let maps = read_from_file(&mut edb);
        let map = maps.first().expect("m02_city map is missing");
        eprintln!(
            "m02_city runtime sections: cameras={} portals={} placement_groups={} isounds={:?}",
            map.cameras.len(),
            map.portals.len(),
            map.placement_group_count,
            map.isounds,
        );
        assert!(map.cameras.iter().all(|camera| {
            camera.position.is_finite()
                && camera.look.is_finite()
                && camera.focal_length.is_finite()
                && camera.aperture_width.is_finite()
                && camera.aperture_height.is_finite()
        }));
        assert!(map.portals.iter().all(|portal| {
            portal.distance.is_finite()
                && portal.vertices.iter().all(|vertex| vertex.is_finite())
                && portal.face_vertices.iter().all(|vertex| vertex.is_finite())
        }));
    }

    #[test]
    fn real_m02_city_structural_audit_when_requested() {
        let Ok(path) = std::env::var("EUROCHEF_REAL_M02_CITY_EDB") else {
            return;
        };
        let open_edb = || {
            let file = File::open(&path).expect("m02_city fixture is missing");
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .expect("m02_city fixture is not a valid PC EDB")
        };

        let mut map_edb = open_edb();
        assert_eq!(map_edb.header.hashcode, 0x0100_001D);
        assert_eq!(map_edb.header.entity_list.len(), 171);
        let maps = read_from_file(&mut map_edb);
        assert_eq!(maps.len(), 1);
        let map = &maps[0];
        assert_eq!(map.triggers.len(), 726);

        let invalid_trigger_links = map
            .triggers
            .iter()
            .flat_map(|trigger| &trigger.links)
            .filter(|link| **link != -1 && (**link < 0 || **link as usize >= map.triggers.len()))
            .count();
        let invalid_path_links = map
            .paths
            .iter()
            .map(|path| {
                path.links
                    .iter()
                    .filter(|(from, to)| *from >= path.nodes.len() || *to >= path.nodes.len())
                    .count()
            })
            .sum::<usize>();
        assert_eq!(invalid_trigger_links, 0);
        assert_eq!(invalid_path_links, 0);

        let trigger_info: eurochef_shared::maps::TriggerInformation =
            serde_yaml::from_str(include_str!("../../../assets/triggers_robots.yml"))
                .expect("Robots trigger typemap did not parse");
        let missing_trigger_type_definitions = map
            .triggers
            .iter()
            .filter(|trigger| !trigger_info.triggers.contains_key(&trigger.ttype))
            .count();
        let mut non_null_trigger_values = 0usize;
        let mut named_trigger_values = 0usize;
        for trigger in &map.triggers {
            let definition = trigger_info
                .triggers
                .get(&trigger.ttype)
                .expect("m02_city trigger type is missing from the typemap");
            for (slot, value) in trigger.data.iter().enumerate() {
                if value.is_none() {
                    continue;
                }
                non_null_trigger_values += 1;
                if definition.values.contains_key(&(slot as u32)) {
                    named_trigger_values += 1;
                }
            }
        }
        assert_eq!(missing_trigger_type_definitions, 0);
        assert_eq!(non_null_trigger_values, 1821);
        let dynamic_los_scripts = map
            .triggers
            .iter()
            .enumerate()
            .filter(|(_, trigger)| {
                trigger.ttype == 4
                    && trigger.data.first().and_then(|value| *value).unwrap_or(0) & 0x100 != 0
            })
            .map(|(index, trigger)| {
                (
                    index,
                    trigger.debug,
                    trigger.data.first().and_then(|value| *value).unwrap_or(0),
                    trigger.game_flags,
                    trigger.engine_options.visual_object,
                    trigger.engine_options.visual_object_file,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(dynamic_los_scripts.len(), 63);
        assert!(dynamic_los_scripts
            .iter()
            .all(|(_, _, _, game_flags, _, _)| {
                matches!(*game_flags, 0 | 0x2000 | 0x4000 | 0x8000)
                    && game_flags & 0x0008_0000 == 0
                    && game_flags & 0x0800_0000 == 0
            }));
        assert!(map
            .triggers
            .iter()
            .filter(|trigger| {
                trigger.ttype == 4
                    && trigger.data.first().and_then(|value| *value).unwrap_or(0) & 0x100 != 0
            })
            .all(|trigger| map.native_zone_index(trigger.position).is_some()));
        let raw_only_trigger_values = non_null_trigger_values - named_trigger_values;

        let pickup_types = [
            0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x29, 0x3E, 0x3F, 0x40, 0x41, 0x43, 0x47, 0x52,
            0x53,
        ];
        let pickup_triggers = map
            .triggers
            .iter()
            .filter(|trigger| pickup_types.contains(&trigger.ttype))
            .collect::<Vec<_>>();
        let unresolved_pickups = pickup_triggers
            .iter()
            .filter(|trigger| super::robots_pickup_visual(trigger.ttype, &trigger.data).is_none())
            .count();
        assert_eq!(pickup_triggers.len(), 311);
        assert_eq!(unresolved_pickups, 0);

        let zero_entities = [0x8200_0026, 0x8200_0027, 0x8200_0058];
        let placement_zero_refs = zero_entities.map(|entity| {
            map.placements
                .iter()
                .filter(|placement| placement.object_ref == entity)
                .count()
        });
        assert_eq!(placement_zero_refs, [0, 0, 1]);
        let zero_entity_placements = map
            .placements
            .iter()
            .filter(|placement| zero_entities.contains(&placement.object_ref))
            .map(|placement| {
                format!(
                    "placement=0x{:08X} object=0x{:08X} pos={:?} rot={:?} scale={:?} flags=0x{:08X} engine_flags=0x{:04X} map_on=0x{:04X} light_set={} group={} unk=0x{:08X}",
                    placement.hashcode,
                    placement.object_ref,
                    placement.position,
                    placement.rotation,
                    placement.scale,
                    placement.flags,
                    placement.engine_flags,
                    placement.map_on,
                    placement.light_set,
                    placement.group,
                    placement.unk,
                )
            })
            .collect::<Vec<_>>();

        let mut entity_edb = open_edb();
        let entity_header = entity_edb.header.clone();
        let entity_endian = entity_edb.endian;
        let zero_geometry_counts = zero_entities.map(|entity| {
            let record = entity_header
                .entity_list
                .iter()
                .find(|record| record.common.hashcode == entity)
                .expect("zero-geometry entity header is missing");
            entity_edb
                .seek(std::io::SeekFrom::Start(record.common.address as u64))
                .expect("could not seek to zero-geometry entity");
            let parsed = entity_edb
                .read_type_args::<EXGeoEntity>(entity_endian, (entity_header.version, Platform::Pc))
                .expect("zero-geometry entity did not parse");
            let EXGeoEntity::Mesh(mesh) = parsed else {
                panic!("0x{entity:08X} is not a mesh entity");
            };
            (
                mesh.vertices.len(),
                mesh.indices.len(),
                mesh.tristrips.len(),
            )
        });
        assert_eq!(zero_geometry_counts, [(0, 0, 0); 3]);

        let mut script_edb = open_edb();
        let scripts =
            UXGeoScript::read_all(&mut script_edb).expect("m02_city scripts did not parse");
        assert_eq!(scripts.len(), 60);

        let expected_skies = [
            0x8400_0019,
            0x8400_0017,
            0x8400_0035,
            0x8400_0018,
            0x8400_0033,
            0x8400_0036,
        ];
        assert_eq!(map.skies.as_slice(), expected_skies.as_slice());
        let expected_zone_skies = [
            -1, 0, 0, 0, 1, 1, 1, 1, -1, 2, 1, 1, 3, -1, -1, 3, 4, 1, 1, 1, -1, -1, 5,
        ];
        let zone_skies = map
            .zones
            .iter()
            .map(|zone| zone.identifier.sky_index)
            .collect::<Vec<_>>();
        assert_eq!(zone_skies.as_slice(), expected_zone_skies.as_slice());
        assert!(map.zones.iter().all(|zone| {
            let min = zone.bounds_box[0];
            let max = zone.bounds_box[1];
            min.iter().chain(max.iter()).all(|value| value.is_finite())
                && min[0] <= max[0]
                && min[1] <= max[1]
                && min[2] <= max[2]
        }));
        let expected_zone0_bounds = [
            [82.593_84, -8.488_69, 98.871_7],
            [116.718_2, 6.008_4, 133.052_1],
        ];
        for (actual, expected) in map.zones[0]
            .bounds_box
            .iter()
            .flatten()
            .zip(expected_zone0_bounds.iter().flatten())
        {
            assert!(
                (*actual - *expected).abs() < 0.002,
                "City v248 zone-0 bounds shifted: actual={actual} expected={expected}"
            );
        }

        let editor_start = map_editor_start_position(map).expect("City editor start position");
        assert!(
            editor_start.distance(Vec3::new(200.076_77, 0.0, 88.114_5)) < 0.003,
            "unexpected City editor start position: {editor_start:?}"
        );
        let start_zone = map
            .native_zone_index(editor_start)
            .expect("City start MapZone BSP leaf");
        assert_eq!(start_zone, 6);
        assert_eq!(map.zones[start_zone].identifier.sky_index, 1);
        assert_eq!(map.skies[1], 0x8400_0017);

        let sky_script_contains = |script_hashcode, entity_hashcode| {
            scripts
                .iter()
                .find(|script| script.hashcode == script_hashcode)
                .expect("City sky Script is missing")
                .commands
                .iter()
                .any(|command| {
                    matches!(
                        &command.data,
                        UXGeoScriptCommandData::Entity { hashcode, .. }
                            if *hashcode == entity_hashcode
                    )
                })
        };
        assert!(sky_script_contains(0x8400_0017, 0x8200_002E));
        assert!(sky_script_contains(0x8400_0035, 0x8200_0098));

        let sky_zone_rows = map
            .zones
            .iter()
            .enumerate()
            .filter(|(_, zone)| matches!(zone.identifier.sky_index, 1 | 2))
            .map(|(index, zone)| {
                format!(
                    "zone={index} sky_index={} sky=0x{:08X} bounds={:?}",
                    zone.identifier.sky_index,
                    map.skies[zone.identifier.sky_index as usize],
                    zone.bounds_box,
                )
            })
            .collect::<Vec<_>>();
        assert!(
            sky_zone_rows.iter().any(|row| row.contains("sky_index=1")),
            "m02_city has no zone selecting 0x84000017 / 0x8200002E"
        );
        assert!(
            sky_zone_rows.iter().any(|row| row.contains("sky_index=2")),
            "m02_city has no zone selecting 0x84000035 / 0x82000098"
        );

        let script_zero_refs = zero_entities.map(|entity| {
            scripts
                .iter()
                .flat_map(|script| &script.commands)
                .filter(|command| {
                    matches!(
                        &command.data,
                        UXGeoScriptCommandData::Entity { hashcode, .. } if *hashcode == entity
                    )
                })
                .count()
        });
        assert_eq!(script_zero_refs, [2, 1, 0]);

        let report = format!(
            "uid=0x0100001D maps={} zones={} placements={} lights={} sounds={} paths={} triggers={} entities={} scripts={} pickups={} unresolved_pickups={} invalid_trigger_links={} invalid_path_links={} missing_trigger_type_definitions={} trigger_values_total={} trigger_values_named={} trigger_values_raw_only={} zero_entity_geometry_counts={:?} zero_entity_placement_refs={:?} zero_entity_script_refs={:?} zero_entity_placements={:?} sky_zone_rows={:?}",
            maps.len(),
            map.zones.len(),
            map.placements.len(),
            map.lights.len(),
            map.sounds.len(),
            map.paths.len(),
            map.triggers.len(),
            map_edb.header.entity_list.len(),
            scripts.len(),
            pickup_triggers.len(),
            unresolved_pickups,
            invalid_trigger_links,
            invalid_path_links,
            missing_trigger_type_definitions,
            non_null_trigger_values,
            named_trigger_values,
            raw_only_trigger_values,
            zero_geometry_counts,
            placement_zero_refs,
            script_zero_refs,
            zero_entity_placements,
            sky_zone_rows,
        );
        eprintln!("{report}");
        if let Ok(output) = std::env::var("EUROCHEF_REAL_M02_CITY_REPORT") {
            if let Some(parent) = std::path::Path::new(&output).parent() {
                std::fs::create_dir_all(parent)
                    .expect("could not create m02_city audit report folder");
            }
            std::fs::write(output, report).expect("could not write m02_city audit report");
        }
    }

    #[test]
    fn real_m02_city_geometry_usage_audit_when_requested() {
        let Ok(path) = std::env::var("EUROCHEF_REAL_M02_CITY_EDB") else {
            return;
        };
        let open_edb = || {
            let file = File::open(&path).expect("m02_city fixture is missing");
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .expect("m02_city fixture is not a valid PC EDB")
        };

        let mut map_edb = open_edb();
        let maps = read_from_file(&mut map_edb);
        let map = maps.first().expect("m02_city has no map");

        let mut entity_edb = open_edb();
        let (entities, _, ref_entities) = crate::entities::read_from_file(&mut entity_edb, None)
            .expect("m02_city entities did not parse");
        let mut script_edb = open_edb();
        let scripts =
            UXGeoScript::read_all(&mut script_edb).expect("m02_city scripts did not parse");
        let mut render_store = crate::render::RenderStore::new();
        for (entity_index, entry) in &entities {
            if let Ok((_, mesh)) = &entry.data {
                let mut renderer = crate::render::entity::EntityRenderer::new(
                    map_edb.header.hashcode,
                    Platform::Pc,
                );
                renderer.set_serialized_vertex_count_for_test(mesh.vertex_data.len());
                render_store.insert_entity(
                    map_edb.header.hashcode,
                    entry.hashcode,
                    *entity_index,
                    renderer,
                );
            }
        }
        for script in &scripts {
            render_store.insert_script(map_edb.header.hashcode, script.clone());
        }

        let selected = [
            0x0200_01B4,
            // User-reported City gears.
            0x8200_002D,
            0x8200_0046,
            0x8200_009E,
            0x8200_009F,
            0x8200_00A0,
            0x8200_00A1,
            0x8200_00A9,
            // User-reported missing or misplaced City entities.
            0x8200_0006,
            0x8200_0007,
            0x8200_000D,
            0x8200_0026,
            0x8200_0027,
            0x8200_002E,
            0x8200_002F,
            0x8200_0030,
            0x8200_0031,
            0x8200_0032,
            0x8200_0033,
            0x8200_0034,
            0x8200_0035,
            0x8200_0036,
            0x8200_0037,
            0x8200_0038,
            0x8200_0039,
            0x8200_003A,
            0x8200_003B,
            0x8200_003C,
            0x8200_003D,
            0x8200_003E,
            0x8200_0058,
            0x8200_0094,
            0x8200_0098,
            0x8200_0099,
            0x8200_009A,
            0x8200_009D,
            // Existing structural sentinels retained for regression coverage.
            0x8200_0047,
            0x8200_00A8,
            0x8200_00AA,
        ];
        let mut lines = Vec::<String>::new();

        lines.push("-- City static-script header census --".to_string());
        for script_hashcode in [
            0x8400_0017,
            0x8400_0018,
            0x8400_0019,
            0x8400_0033,
            0x8400_0035,
            0x8400_0036,
        ] {
            let header = map_edb
                .header
                .animscript_list
                .iter()
                .find(|header| header.hashcode == script_hashcode)
                .unwrap_or_else(|| panic!("0x{script_hashcode:08X} is missing from City headers"));
            let mut raw_edb = open_edb();
            raw_edb
                .seek(std::io::SeekFrom::Start(header.address as u64))
                .expect("could not seek to City Script header");
            let raw = raw_edb
                .read_type::<EXGeoAnimScript>(raw_edb.endian)
                .expect("could not parse City Script header");
            let processed = scripts
                .iter()
                .find(|script| script.hashcode == script_hashcode)
                .expect("processed City Script is missing");
            lines.push(format!(
                "script=0x{script_hashcode:08X}\taddress=0x{:08X}\tlength={}\tthreads={}\ttimejumps={}\tflags=0x{:04X}\tfps={}\tbounds={:?}\tunk30=0x{:08X}\tserialized_controllers={}\tused_controller_types=0x{:08X}\trecord_metadata={:?}",
                header.address,
                raw.length,
                raw._unk8,
                raw.timejump_count,
                raw.script_flags,
                raw.frame_rate,
                raw.bounds_box,
                raw.unk30,
                raw.thread_controller_count,
                raw.used_controller_types,
                processed.controller_record_metadata,
            ));
        }

        lines.push("-- mapzone render-pass census --".to_string());
        for (zone_index, zone) in map.mapzone_entities.iter().enumerate() {
            let Some(Ok((_, mesh))) = ref_entities
                .iter()
                .find(|entry| entry.hashcode == zone.entity_refptr)
                .map(|entry| entry.data.as_ref())
            else {
                lines.push(format!(
                    "zone#{zone_index}\tref={}\tunresolved",
                    zone.entity_refptr
                ));
                continue;
            };
            let normal_triangles = mesh
                .strips
                .iter()
                .filter(|strip| strip.flags & 0x10 == 0)
                .map(|strip| strip.index_count.saturating_sub(2) as usize)
                .sum::<usize>();
            let excluded_triangles = mesh
                .strips
                .iter()
                .filter(|strip| strip.flags & 0x10 != 0)
                .map(|strip| strip.index_count.saturating_sub(2) as usize)
                .sum::<usize>();
            lines.push(format!(
                "zone#{zone_index}\tref={}\tvertices={}\tstrips={}\tnormal_triangles={normal_triangles}\texcluded_0x10_triangles={excluded_triangles}",
                zone.entity_refptr,
                mesh.vertex_data.len(),
                mesh.strips.len(),
            ));
        }

        let gear_script_hashcode = 0x8400_0015;
        let gear_script = render_store
            .get_script(map_edb.header.hashcode, gear_script_hashcode)
            .expect("City gear Script 0x84000015 is missing");
        lines.push("-- animated entity controller census --".to_string());
        for script in &scripts {
            for (command_index, command) in script.commands.iter().enumerate() {
                let UXGeoScriptCommandData::Entity { hashcode, .. } = command.data else {
                    continue;
                };
                let Some(controller) = script
                    .controllers
                    .get(command.controller_header_index as usize)
                else {
                    continue;
                };
                if controller.channels.vector_0.is_empty()
                    && controller.channels.quat_0.is_empty()
                    && controller.channels.vector_1.is_empty()
                {
                    continue;
                }
                lines.push(format!(
                    "script=0x{:08X}\tcmd={command_index}\tentity=0x{hashcode:08X}\tcontroller={}\tstart={}\tlength={}\tposition_keys={}\trotation_keys={}\tscale_keys={}\tposition_first_last={:?}/{:?}\trotation_first_last={:?}/{:?}",
                    script.hashcode,
                    command.controller_header_index,
                    command.start,
                    command.length,
                    controller.channels.vector_0.len(),
                    controller.channels.quat_0.len(),
                    controller.channels.vector_1.len(),
                    controller.channels.vector_0.first(),
                    controller.channels.vector_0.last(),
                    controller.channels.quat_0.first(),
                    controller.channels.quat_0.last(),
                ));
            }
        }

        lines.push("-- city gear script 0x84000015 controllers --".to_string());
        for (controller_index, controller) in gear_script.controllers.iter().enumerate() {
            lines.push(format!(
                "controller#{controller_index}\tmask=0x{:08X}\tchannel_mask=0x{:08X}\tvector_0={:?}\tquat_0={:?}\tvector_1={:?}\tvector_2={:?}",
                controller.ctrl_mask,
                controller.ctrl_channel_mask,
                controller.channels.vector_0,
                controller.channels.quat_0,
                controller.channels.vector_1,
                controller.channels.vector_2,
            ));
        }
        lines.push("-- city gear script 0x84000015 queued transforms --".to_string());
        for frame in 0..gear_script.length {
            let time = gear_script.time_at_frame(frame as f32);
            let mut queue = Vec::new();
            crate::render::script::render_script(
                Vec3::ZERO,
                glam::Quat::IDENTITY,
                Vec3::ONE,
                map_edb.header.hashcode,
                gear_script_hashcode,
                time,
                &render_store,
                &mut |queued| queue.push(queued),
                vec![],
            );
            let gears = queue
                .iter()
                .filter(|queued| matches!(queued.entity.1, 0x8200_002B | 0x8200_002C))
                .map(|queued| {
                    format!(
                        "entity=0x{:08X},pos={:?},rot={:?},scale={:?}",
                        queued.entity.1, queued.position, queued.rotation, queued.scale
                    )
                })
                .collect::<Vec<_>>();
            lines.push(format!(
                "frame={frame}\ttime={time:.9}\t{}",
                gears.join("\t")
            ));
        }

        for script_hashcode in [0x8400_0028, 0x8400_0029, 0x8400_002A] {
            let script = render_store
                .get_script(map_edb.header.hashcode, script_hashcode)
                .unwrap_or_else(|| panic!("0x{script_hashcode:08X} is missing from City Scripts"));
            let root_visual_time = script
                .first_visual_frame()
                .map(|frame| script.time_at_frame(frame.max(0) as f32));
            let resolved_visual_time = crate::render::script::first_resolved_visual_time(
                map_edb.header.hashcode,
                script_hashcode,
                &render_store,
            )
            .unwrap_or_else(|| {
                panic!("0x{script_hashcode:08X} has no recursively resolved visual time")
            });
            let mut queue = Vec::new();
            crate::render::script::render_script(
                Vec3::ZERO,
                glam::Quat::IDENTITY,
                Vec3::ONE,
                map_edb.header.hashcode,
                script_hashcode,
                resolved_visual_time,
                &render_store,
                &mut |queued| queue.push(queued),
                vec![],
            );
            assert!(
                !queue.is_empty(),
                "0x{script_hashcode:08X} still queues no model at its recursively resolved visual time"
            );
            lines.push(format!(
                "script=0x{script_hashcode:08X}\troot_visual_time={root_visual_time:?}\tresolved_visual_time={resolved_visual_time}\tqueued_entities={}",
                queue.len()
            ));
        }

        let ref_6 = ref_entities
            .iter()
            .find(|entry| entry.hashcode == 6)
            .expect("ref_6 is missing from the decoded refpointer entities");
        let ref_6_mesh = ref_6
            .data
            .as_ref()
            .expect("ref_6 failed to parse")
            .1
            .clone();
        assert!(
            !ref_6_mesh.vertex_data.is_empty(),
            "ref_6 has no decoded geometry"
        );
        let ref_6_zones = map
            .mapzone_entities
            .iter()
            .enumerate()
            .filter_map(|(zone_index, zone)| (zone.entity_refptr == 6).then_some(zone_index))
            .collect::<Vec<_>>();
        lines.push(format!(
            "ref_6\tusage=mapzone_refpointer\tzones={ref_6_zones:?}\t{}",
            format_mesh_diagnostics(&ref_6_mesh)
        ));

        for hashcode in selected {
            let entity = entities
                .iter()
                .find(|(_, entry)| entry.hashcode == hashcode)
                .unwrap_or_else(|| panic!("0x{hashcode:08X} is missing from the entity list"));
            let (raw_entity, mesh) = entity
                .1
                .data
                .as_ref()
                .unwrap_or_else(|error| panic!("0x{hashcode:08X} failed to parse: {error}"));
            let placements = map
                .placements
                .iter()
                .enumerate()
                .filter(|(_, placement)| placement.object_ref == hashcode)
                .map(|(index, placement)| {
                    format!(
                        "#{index}@pos={:?},rot={:?},scale={:?},map_on=0x{:04X},flags=0x{:08X}",
                        placement.position,
                        placement.rotation,
                        placement.scale,
                        placement.map_on,
                        placement.flags,
                    )
                })
                .collect::<Vec<_>>();
            let script_refs = scripts
                .iter()
                .flat_map(|script| {
                    script.commands.iter().enumerate().filter_map(move |(command_index, command)| {
                        matches!(
                            command.data,
                            UXGeoScriptCommandData::Entity { hashcode: object, .. } if object == hashcode
                        )
                        .then_some(format!(
                            "0x{:08X}/cmd{command_index}/start{}/len{}/controller{}",
                            script.hashcode,
                            command.start,
                            command.length,
                            command.controller_header_index,
                        ))
                    })
                })
                .collect::<Vec<_>>();
            let trigger_refs = map
                .triggers
                .iter()
                .enumerate()
                .filter_map(|(index, trigger)| {
                    (trigger.engine_options.visual_object == Some(hashcode)).then_some(format!(
                        "#{index}/type{}@pos={:?},rot={:?},scale={:?}",
                        trigger.ttype, trigger.position, trigger.rotation, trigger.scale
                    ))
                })
                .collect::<Vec<_>>();
            let kind = match raw_entity {
                EXGeoEntity::Mesh(_) => "mesh",
                EXGeoEntity::Split(_) => "split",
                EXGeoEntity::MapZone(_) => "mapzone",
                EXGeoEntity::Instance(_) => "instance",
                EXGeoEntity::NavMesh(_) => "navmesh",
                EXGeoEntity::UnknownType(_) => "unknown",
            };
            lines.push(format!(
                "0x{hashcode:08X}\tkind={kind}\tplacements={placements:?}\tscripts={script_refs:?}\ttriggers={trigger_refs:?}\t{}",
                format_mesh_diagnostics(mesh)
            ));
        }

        lines.push("-- all placements --".to_string());
        for (placement_index, placement) in map.placements.iter().enumerate() {
            let position = Vec3::from(placement.position);
            let rotation = glam::Quat::from_euler(
                glam::EulerRot::ZXY,
                placement.rotation[2],
                placement.rotation[0],
                placement.rotation[1],
            );
            let scale = Vec3::from(placement.scale);
            if placement.object_ref.base() == 0x0200_0000 {
                let Some(entity) = entities
                    .iter()
                    .find(|(_, entry)| entry.hashcode == placement.object_ref)
                else {
                    lines.push(format!(
                        "placement#{placement_index}\tobject=0x{:08X}\tunresolved_entity",
                        placement.object_ref
                    ));
                    continue;
                };
                let mesh = &entity.1.data.as_ref().unwrap().1;
                let (bb_min, bb_max) = mesh.bounding_box();
                let local_center = if mesh.vertex_data.is_empty() {
                    Vec3::ZERO
                } else {
                    (bb_min + bb_max) * 0.5
                };
                let world_center = position + rotation.mul_vec3(scale * local_center);
                let containing_zones = map
                    .zones
                    .iter()
                    .enumerate()
                    .filter_map(|(zone_index, zone)| {
                        let a = Vec3::from(zone.bounds_box[0]);
                        let b = Vec3::from(zone.bounds_box[1]);
                        let min = a.min(b);
                        let max = a.max(b);
                        (world_center.cmpge(min).all() && world_center.cmple(max).all())
                            .then_some(zone_index)
                    })
                    .collect::<Vec<_>>();
                lines.push(format!(
                    "placement#{placement_index}\tobject=0x{:08X}\tlocal_center={local_center:?}\tposition={position:?}\tworld_center={world_center:?}\tzones={containing_zones:?}\tmap_on=0x{:04X}\t{}",
                    placement.object_ref,
                    placement.map_on,
                    format_mesh_diagnostics(mesh),
                ));
            } else {
                lines.push(format!(
                    "placement#{placement_index}\tobject=0x{:08X}\tposition={position:?}\trotation={:?}\tscale={scale:?}\tmap_on=0x{:04X}",
                    placement.object_ref,
                    placement.rotation,
                    placement.map_on,
                ));
            }
        }

        let true_zero_geometry = [0x8200_0026, 0x8200_0027, 0x8200_0058];
        for hashcode in selected {
            let mesh = &entities
                .iter()
                .find(|(_, entry)| entry.hashcode == hashcode)
                .unwrap()
                .1
                .data
                .as_ref()
                .unwrap()
                .1;
            assert_eq!(
                mesh.vertex_data.is_empty(),
                true_zero_geometry.contains(&hashcode),
                "unexpected zero/nonzero geometry classification for 0x{hashcode:08X}"
            );
        }

        let report = lines.join("\n") + "\n";
        eprintln!("{report}");
        if let Ok(output) = std::env::var("EUROCHEF_REAL_M02_GEOMETRY_REPORT") {
            if let Some(parent) = std::path::Path::new(&output).parent() {
                std::fs::create_dir_all(parent)
                    .expect("could not create m02_city geometry report folder");
            }
            std::fs::write(output, report).expect("could not write m02_city geometry report");
        }
    }

    #[test]
    fn real_map_geometry_and_motion_corpus_when_requested() {
        let Ok(root) = std::env::var("EUROCHEF_REAL_MAP_CORPUS_ROOT") else {
            return;
        };

        fn collect_edb_paths(root: &Path, output: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(root) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_edb_paths(&path, output);
                } else if path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("edb"))
                {
                    output.push(path);
                }
            }
        }

        fn script_has_nonzero_entity(
            current_file: u32,
            script_hashcode: u32,
            render_store: &crate::render::RenderStore,
            nonzero_entities: &std::collections::HashSet<u32>,
            ancestry: &mut Vec<u32>,
        ) -> bool {
            if ancestry.len() >= 64 || ancestry.contains(&script_hashcode) {
                return false;
            }
            let Some(script) = render_store.get_script(current_file, script_hashcode) else {
                return false;
            };
            ancestry.push(script_hashcode);
            let found = script.commands.iter().any(|command| match command.data {
                UXGeoScriptCommandData::Entity { hashcode, file } => {
                    let source_file = if file == u32::MAX || hashcode.is_local() {
                        current_file
                    } else {
                        file
                    };
                    source_file == current_file
                        && render_store
                            .resolve_entity_hashcode(source_file, hashcode)
                            .is_some_and(|resolved| nonzero_entities.contains(&resolved))
                }
                UXGeoScriptCommandData::SubScript { hashcode, file } => {
                    let source_file = if file == u32::MAX || hashcode.is_local() {
                        current_file
                    } else {
                        file
                    };
                    source_file == current_file
                        && script_has_nonzero_entity(
                            source_file,
                            hashcode,
                            render_store,
                            nonzero_entities,
                            ancestry,
                        )
                }
                _ => false,
            });
            ancestry.pop();
            found
        }

        fn finite_vec3(value: [f32; 3]) -> bool {
            value.into_iter().all(f32::is_finite)
        }

        fn finite_vec4(value: [f32; 4]) -> bool {
            value.into_iter().all(f32::is_finite)
        }

        let mut paths = Vec::new();
        collect_edb_paths(Path::new(&root), &mut paths);
        paths.sort();

        let mut edb_files = 0usize;
        let mut map_files = 0usize;
        let mut map_count = 0usize;
        let mut entity_placements = 0usize;
        let mut script_placements = 0usize;
        let mut internal_model_scripts = 0usize;
        let mut moving_triggers = 0usize;
        let mut controller_keyframes = 0usize;
        let mut parse_failures = Vec::<String>::new();
        let mut unresolved_entity_placements = Vec::<String>::new();
        let mut model_scripts_without_queue = Vec::<String>::new();
        let mut nonfinite_rows = Vec::<String>::new();
        let mut missing_motion_paths = Vec::<String>::new();
        let mut suspicious_double_transforms = Vec::<String>::new();
        let mut motion_rows = Vec::<String>::new();
        let mut translation_axis_counts = [0usize; 3];
        let mut rotation_controller_count = 0usize;

        for path in paths {
            edb_files += 1;
            let open_edb = || {
                let file = File::open(&path).ok()?;
                EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).ok()
            };
            let Some(mut map_edb) = open_edb() else {
                parse_failures.push(format!("{}:header", path.display()));
                continue;
            };
            let maps = read_from_file(&mut map_edb);
            if maps.is_empty() {
                continue;
            }
            map_files += 1;
            map_count += maps.len();
            let file_uid = map_edb.header.hashcode;
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("<unknown>");

            let Some(mut entity_edb) = open_edb() else {
                parse_failures.push(format!("{file_name}:entities-open"));
                continue;
            };
            let Ok((entities, _, _)) = crate::entities::read_from_file(&mut entity_edb, None)
            else {
                parse_failures.push(format!("{file_name}:entities-parse"));
                continue;
            };
            let Some(mut script_edb) = open_edb() else {
                parse_failures.push(format!("{file_name}:scripts-open"));
                continue;
            };
            let Ok(scripts) = UXGeoScript::read_all(&mut script_edb) else {
                parse_failures.push(format!("{file_name}:scripts-parse"));
                continue;
            };

            let mut render_store = crate::render::RenderStore::new();
            let mut nonzero_entities = std::collections::HashSet::<u32>::new();
            let mut mesh_by_hash =
                std::collections::HashMap::<u32, &crate::entities::ProcessedEntityMesh>::new();
            for (entity_index, entry) in &entities {
                let Ok((_, mesh)) = &entry.data else {
                    parse_failures.push(format!("{file_name}:entity:0x{:08X}", entry.hashcode));
                    continue;
                };
                let mut renderer =
                    crate::render::entity::EntityRenderer::new(file_uid, Platform::Pc);
                renderer.set_serialized_vertex_count_for_test(mesh.vertex_data.len());
                render_store.insert_entity(file_uid, entry.hashcode, *entity_index, renderer);
                mesh_by_hash.insert(entry.hashcode, mesh);
                if !mesh.vertex_data.is_empty() {
                    nonzero_entities.insert(entry.hashcode);
                }
            }
            for script in &scripts {
                render_store.insert_script(file_uid, script.clone());

                for controller in &script.controllers {
                    for (time, value) in controller
                        .channels
                        .vector_0
                        .iter()
                        .chain(controller.channels.vector_1.iter())
                        .chain(controller.channels.vector_2.iter())
                    {
                        controller_keyframes += 1;
                        if !time.is_finite() || !finite_vec3(*value) {
                            nonfinite_rows.push(format!(
                                "{file_name}:script=0x{:08X}:vector-key",
                                script.hashcode
                            ));
                        }
                    }
                    for (time, value) in &controller.channels.quat_0 {
                        controller_keyframes += 1;
                        rotation_controller_count += 1;
                        if !time.is_finite() || !finite_vec4(*value) {
                            nonfinite_rows.push(format!(
                                "{file_name}:script=0x{:08X}:quat-key",
                                script.hashcode
                            ));
                        }
                    }
                    if let (Some((_, first)), Some((_, last))) = (
                        controller.channels.vector_0.first(),
                        controller.channels.vector_0.last(),
                    ) {
                        for axis in 0..3 {
                            if (last[axis] - first[axis]).abs() > 0.0001 {
                                translation_axis_counts[axis] += 1;
                            }
                        }
                    }
                }
            }

            for script in &scripts {
                if !script_has_nonzero_entity(
                    file_uid,
                    script.hashcode,
                    &render_store,
                    &nonzero_entities,
                    &mut Vec::new(),
                ) {
                    continue;
                }
                internal_model_scripts += 1;
                let Some(time) = crate::render::script::first_resolved_visual_time(
                    file_uid,
                    script.hashcode,
                    &render_store,
                ) else {
                    model_scripts_without_queue.push(format!(
                        "{file_name}:script=0x{:08X}:no-visual-time",
                        script.hashcode
                    ));
                    continue;
                };
                let mut queue = Vec::new();
                crate::render::script::render_script(
                    Vec3::ZERO,
                    glam::Quat::IDENTITY,
                    Vec3::ONE,
                    file_uid,
                    script.hashcode,
                    time,
                    &render_store,
                    &mut |queued| queue.push(queued),
                    vec![],
                );
                let visible_count = queue
                    .iter()
                    .filter(|queued| {
                        render_store
                            .resolve_entity_hashcode(queued.entity.0, queued.entity.1)
                            .is_some_and(|resolved| nonzero_entities.contains(&resolved))
                    })
                    .count();
                if visible_count == 0 {
                    model_scripts_without_queue.push(format!(
                        "{file_name}:script=0x{:08X}:time={time}:queue={}",
                        script.hashcode,
                        queue.len()
                    ));
                }
            }

            for map in &maps {
                for (placement_index, placement) in map.placements.iter().enumerate() {
                    if !placement.position.into_iter().all(f32::is_finite)
                        || !placement.rotation.into_iter().all(f32::is_finite)
                        || !placement.scale.into_iter().all(f32::is_finite)
                    {
                        nonfinite_rows.push(format!(
                            "{file_name}:map=0x{:08X}:placement#{placement_index}",
                            map.hashcode
                        ));
                    }
                    match placement.object_ref.base() {
                        0x0200_0000 => {
                            entity_placements += 1;
                            let Some(resolved) = render_store
                                .resolve_entity_hashcode(file_uid, placement.object_ref)
                            else {
                                unresolved_entity_placements.push(format!(
                                    "{file_name}:map=0x{:08X}:placement#{placement_index}:0x{:08X}",
                                    map.hashcode, placement.object_ref
                                ));
                                continue;
                            };
                            let Some(mesh) = mesh_by_hash.get(&resolved) else {
                                continue;
                            };
                            if !mesh.vertex_data.is_empty() {
                                let (minimum, maximum) = mesh.bounding_box();
                                let center = (minimum + maximum) * 0.5;
                                let position = Vec3::from(placement.position);
                                if position.length() > 10.0 && center.length() > 50.0 {
                                    suspicious_double_transforms.push(format!(
                                        "{file_name}:map=0x{:08X}:placement#{placement_index}:object=0x{resolved:08X}:position={position:?}:mesh_center={center:?}",
                                        map.hashcode
                                    ));
                                }
                            }
                        }
                        0x0400_0000 => {
                            script_placements += 1;
                        }
                        _ => {}
                    }
                }

                for (trigger_index, trigger) in map.triggers.iter().enumerate() {
                    if !matches!(trigger.ttype, 8 | 37 | 80) {
                        continue;
                    }
                    moving_triggers += 1;
                    let speed = robots_trigger_runtime_path_speed(trigger.ttype, &trigger.data)
                        .unwrap_or_default();
                    let acceleration =
                        robots_trigger_runtime_path_acceleration(trigger.ttype, &trigger.data)
                            .unwrap_or_default();
                    let angular =
                        robots_trigger_platform_angular_velocity(trigger.ttype, &trigger.data)
                            .unwrap_or(Vec3::ZERO);
                    let sample = crate::map_runtime::runtime_path_preview_position(
                        map, trigger, 1.0, true, 1.0,
                    );
                    if !speed.is_finite()
                        || !acceleration.is_finite()
                        || !angular.is_finite()
                        || !sample.is_finite()
                    {
                        nonfinite_rows.push(format!(
                            "{file_name}:map=0x{:08X}:trigger#{trigger_index}:type{}",
                            map.hashcode, trigger.ttype
                        ));
                    }
                    let path_hash = robots_trigger_path_hash(trigger.ttype, &trigger.data);
                    if let Some(path_hash) = path_hash {
                        if !map.paths.iter().any(|path| path.hashcode == path_hash) {
                            missing_motion_paths.push(format!(
                                "{file_name}:map=0x{:08X}:trigger#{trigger_index}:type{}:path=0x{path_hash:08X}",
                                map.hashcode, trigger.ttype
                            ));
                        }
                    }
                    motion_rows.push(format!(
                        "{file_name}\tmap=0x{:08X}\ttrigger={trigger_index}\ttype={}\tpath={path_hash:?}\tspeed={speed}\tacceleration={acceleration}\tangular_xyz_deg_s={angular:?}\tposition_t1={sample:?}",
                        map.hashcode, trigger.ttype
                    ));
                }
            }
        }

        let mut report = format!(
            "edb_files={edb_files}\nmap_files={map_files}\nmaps={map_count}\nentity_placements={entity_placements}\nscript_placements={script_placements}\ninternal_model_scripts={internal_model_scripts}\nmoving_triggers={moving_triggers}\ncontroller_keyframes={controller_keyframes}\ntranslation_axis_x={}\ntranslation_axis_y={}\ntranslation_axis_z={}\nrotation_controller_keys={rotation_controller_count}\nparse_failures={}\nunresolved_entity_placements={}\nmodel_scripts_without_queue={}\nnonfinite_rows={}\nmissing_motion_paths={}\nsuspicious_double_transforms={}\n",
            translation_axis_counts[0],
            translation_axis_counts[1],
            translation_axis_counts[2],
            parse_failures.len(),
            unresolved_entity_placements.len(),
            model_scripts_without_queue.len(),
            nonfinite_rows.len(),
            missing_motion_paths.len(),
            suspicious_double_transforms.len(),
        );
        for (heading, rows) in [
            ("parse_failures", &parse_failures),
            (
                "unresolved_entity_placements",
                &unresolved_entity_placements,
            ),
            ("model_scripts_without_queue", &model_scripts_without_queue),
            ("nonfinite_rows", &nonfinite_rows),
            ("missing_motion_paths", &missing_motion_paths),
            (
                "suspicious_double_transforms",
                &suspicious_double_transforms,
            ),
            ("motion", &motion_rows),
        ] {
            report.push_str(&format!("\n[{heading}]\n"));
            for row in rows {
                report.push_str(row);
                report.push('\n');
            }
        }

        assert!(edb_files >= 179, "expected the shipped 179-EDB PC corpus");
        assert!(map_files >= 18, "expected the shipped map EDB corpus");
        assert!(parse_failures.is_empty(), "{parse_failures:#?}");
        assert!(
            unresolved_entity_placements.is_empty(),
            "{unresolved_entity_placements:#?}"
        );
        assert!(
            model_scripts_without_queue.is_empty(),
            "{model_scripts_without_queue:#?}"
        );
        assert!(nonfinite_rows.is_empty(), "{nonfinite_rows:#?}");
        assert!(missing_motion_paths.is_empty(), "{missing_motion_paths:#?}");

        if let Ok(output) = std::env::var("EUROCHEF_REAL_MAP_CORPUS_REPORT") {
            if let Some(parent) = Path::new(&output).parent() {
                std::fs::create_dir_all(parent)
                    .expect("could not create map geometry/motion corpus folder");
            }
            std::fs::write(output, report)
                .expect("could not write map geometry/motion corpus report");
        }
    }

    #[test]
    fn real_m02_city_reported_sky_entities_preserve_native_flag_classes_when_requested() {
        let Ok(path) = std::env::var("EUROCHEF_REAL_M02_CITY_EDB") else {
            return;
        };
        let open_edb = || {
            let file = File::open(&path).expect("m02_city fixture is missing");
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .expect("m02_city fixture is not a valid PC EDB")
        };

        let targets = [
            (0x8200_0006, 0x8400_0017, None, 0u32),
            (0x8200_0007, 0x8400_0017, Some(15usize), 0u32),
            (0x8200_002E, 0x8400_0017, Some(70usize), 768u32),
            (0x8200_002F, 0x8400_0018, Some(8usize), 768u32),
            (0x8200_0030, 0x8400_0019, Some(64usize), 768u32),
            (0x8200_0098, 0x8400_0035, Some(27usize), 768u32),
        ];
        let mut map_edb = open_edb();
        let maps = read_from_file(&mut map_edb);
        let map = maps.first().expect("m02_city map is missing");

        let mut script_edb = open_edb();
        let scripts =
            UXGeoScript::read_all(&mut script_edb).expect("m02_city scripts did not parse");

        for (target, expected_script, expected_children, expected_flags) in targets {
            assert!(
                map.placements
                    .iter()
                    .all(|placement| placement.object_ref != target),
                "0x{target:08X} is Script-owned and must not become a direct placement"
            );
            assert!(
                map.triggers
                    .iter()
                    .all(|trigger| trigger.engine_options.visual_object != Some(target)),
                "0x{target:08X} is not a direct trigger visual"
            );

            let mut matching_commands = Vec::new();
            for script in &scripts {
                for (command_index, command) in script.commands.iter().enumerate() {
                    let UXGeoScriptCommandData::Entity { hashcode, file } = &command.data else {
                        continue;
                    };
                    if *hashcode == target {
                        matching_commands.push((script.hashcode, command_index, command, *file));
                    }
                }
            }
            let (_, command_index, command, file) = matching_commands
                .iter()
                .find(|(script, _, _, _)| *script == expected_script)
                .copied()
                .unwrap_or_else(|| {
                    panic!(
                        "0x{target:08X} is missing from expected sky Script 0x{expected_script:08X}"
                    )
                });
            let is_base_sky_root =
                map.skies.first().copied() == Some(expected_script) && command_index == 0;
            assert_eq!(is_base_sky_root, target == 0x8200_0030);
            assert_eq!(file, u32::MAX);
            assert_eq!(command.opcode, 3);
            assert_eq!(command.parent_controller_index, u8::MAX);
            let controller = scripts
                .iter()
                .find(|script| script.hashcode == expected_script)
                .and_then(|script| {
                    script
                        .controllers
                        .get(command.controller_header_index as usize)
                })
                .expect("reported sky Entity controller is missing");
            if target == 0x8200_002E {
                assert_eq!(controller.channels.vector_0.len(), 1);
                let (frame, translation) = controller.channels.vector_0[0];
                assert_eq!(frame, 0.0);
                let expected = [-0.0016253801, -0.0042165825, 0.00225354];
                for (actual, expected) in translation.into_iter().zip(expected) {
                    assert!((actual - expected).abs() < 1.0e-8);
                }
            } else {
                assert_eq!(controller.controller_count, 0);
                assert_eq!(controller.channel_count, 0);
                assert!(controller.channels.vector_0.is_empty());
                assert!(controller.channels.quat_0.is_empty());
            }

            let mut entity_edb = open_edb();
            let header = entity_edb.header.clone();
            let endian = entity_edb.endian;
            let record = header
                .entity_list
                .iter()
                .find(|record| record.common.hashcode == target)
                .expect("reported entity header is missing");
            entity_edb
                .seek(std::io::SeekFrom::Start(record.common.address as u64))
                .expect("could not seek to reported entity");
            let entity = entity_edb
                .read_type_args::<EXGeoEntity>(endian, (header.version, Platform::Pc))
                .expect("reported entity did not parse");
            let base = entity.base().expect("reported entity has no base");
            assert_eq!(
                entity.type_code(),
                if expected_children.is_some() {
                    0x603
                } else {
                    0x601
                }
            );
            assert_eq!(
                base.flags, expected_flags,
                "0x{target:08X} changed its serialized Entity flags"
            );
            assert!(base
                .bounds_box
                .iter()
                .flatten()
                .all(|value| value.is_finite()));
            match (entity, expected_children) {
                (EXGeoEntity::Mesh(_), None) => {}
                (EXGeoEntity::Split(split), Some(expected_children)) => {
                    assert_eq!(split.entities.len(), expected_children);
                }
                _ => panic!("0x{target:08X} changed its expected Mesh/Split shape"),
            }
        }

        for script_hashcode in [0x8400_0019, 0x8400_0035] {
            let script = scripts
                .iter()
                .find(|script| script.hashcode == script_hashcode)
                .unwrap_or_else(|| panic!("City sky Script 0x{script_hashcode:08X} is missing"));
            let command = script
                .commands
                .iter()
                .find(|command| {
                    matches!(
                        command.data,
                        UXGeoScriptCommandData::Entity {
                            hashcode: 0x8200_003A,
                            ..
                        }
                    )
                })
                .unwrap_or_else(|| {
                    panic!("0x8200003A is missing from City sky Script 0x{script_hashcode:08X}")
                });
            let controller = script
                .controllers
                .get(command.controller_header_index as usize)
                .expect("0x8200003A controller is missing");
            assert_eq!(controller.channels.vector_0.len(), 1);
            let (frame, translation) = controller.channels.vector_0[0];
            assert_eq!(frame, 0.0);
            let expected = [26.549105, 0.0111720245, 155.493];
            for (actual, expected) in translation.into_iter().zip(expected) {
                assert!((actual - expected).abs() < 1.0e-5);
            }
        }

        let mut entity_edb = open_edb();
        let header = entity_edb.header.clone();
        let endian = entity_edb.endian;
        let record = header
            .entity_list
            .iter()
            .find(|record| record.common.hashcode == 0x8200_003A)
            .expect("City sky Entity 0x8200003A is missing");
        entity_edb
            .seek(std::io::SeekFrom::Start(record.common.address as u64))
            .expect("could not seek to City sky Entity 0x8200003A");
        let entity = entity_edb
            .read_type_args::<EXGeoEntity>(endian, (header.version, Platform::Pc))
            .expect("City sky Entity 0x8200003A did not parse");
        assert_eq!(
            entity
                .base()
                .expect("City sky Entity 0x8200003A has no base")
                .flags,
            0
        );
        assert!(matches!(entity, EXGeoEntity::Split(_)));

        for background in [0x8200_003C, 0x8200_003D, 0x8200_003E] {
            let mut entity_edb = open_edb();
            let header = entity_edb.header.clone();
            let endian = entity_edb.endian;
            let record = header
                .entity_list
                .iter()
                .find(|record| record.common.hashcode == background)
                .expect("City sky background entity header is missing");
            entity_edb
                .seek(std::io::SeekFrom::Start(record.common.address as u64))
                .expect("could not seek to City sky background entity");
            let entity = entity_edb
                .read_type_args::<EXGeoEntity>(endian, (header.version, Platform::Pc))
                .expect("City sky background entity did not parse");
            let base = entity
                .base()
                .expect("City sky background entity has no base");
            assert_ne!(
                base.flags & 0x10,
                0,
                "0x{background:08X} must retain the camera-relative background flag"
            );
        }
    }

    #[test]
    fn real_m02_city_base_sky_rejects_the_misattributed_h01_main_transform_when_requested() {
        let Ok(path) = std::env::var("EUROCHEF_REAL_M02_CITY_EDB") else {
            return;
        };

        let file = File::open(&path).expect("m02_city fixture is missing");
        let mut city_edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
            .expect("m02_city fixture is not a valid PC EDB");
        let city_script = UXGeoScript::read_hashcodes(&mut city_edb, &[0x8400_0019])
            .expect("m02_city base sky Script did not parse")
            .into_iter()
            .next()
            .expect("m02_city base sky Script 0x84000019 is missing");
        let city_root = city_script
            .commands
            .first()
            .expect("m02_city base sky Script has no root command");
        assert_eq!(city_root.controller_header_index, 0);
        assert_eq!(city_root.controller_index, 0);
        assert_eq!(city_root.parent_controller_index, u8::MAX);
        assert!(matches!(
            city_root.data,
            UXGeoScriptCommandData::Entity {
                hashcode: 0x8200_0030,
                ..
            }
        ));

        let city_controller = city_script
            .controllers
            .first()
            .expect("m02_city base sky root controller is missing");
        assert_eq!(city_controller.ctrl_mask, 0);
        assert!(city_controller.channels.vector_0.is_empty());
        assert!(city_controller.channels.vector_1.is_empty());

        let h01_path = std::path::Path::new(&path)
            .parent()
            .expect("m02_city fixture has no parent directory")
            .join("h01_main.edb");
        let file = File::open(&h01_path).expect("h01_main fixture is missing beside m02_city");
        let mut h01_edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
            .expect("h01_main fixture is not a valid PC EDB");
        let h01_script = UXGeoScript::read_hashcodes(&mut h01_edb, &[0x0400_0001])
            .expect("h01_main Script 0x04000001 did not parse")
            .into_iter()
            .next()
            .expect("h01_main Script 0x04000001 is missing");
        let h01_command = h01_script
            .commands
            .get(6)
            .expect("h01_main Script 0x04000001 command6 is missing");
        let h01_controller = h01_script
            .controllers
            .get(h01_command.controller_header_index as usize)
            .expect("h01_main command6 controller is missing");
        assert_eq!(h01_controller.ctrl_mask & 0x14, 0x14);

        let (_, position) = h01_controller
            .channels
            .vector_0
            .first()
            .expect("h01_main command6 position key is missing");
        let (_, scale) = h01_controller
            .channels
            .vector_1
            .first()
            .expect("h01_main command6 scale key is missing");
        let expected_position = [21.152079_f32, 12.63301, -5.5051265];
        let expected_scale = [1.6899993_f32; 3];
        for axis in 0..3 {
            assert!((position[axis] - expected_position[axis]).abs() < 1.0e-5);
            assert!((scale[axis] - expected_scale[axis]).abs() < 1.0e-6);
        }
    }

    #[test]
    fn real_main_map_sky_no_fog_flag_corpus_when_requested() {
        let Ok(root) = std::env::var("EUROCHEF_REAL_MAIN_MAP_SKY_ROOT") else {
            return;
        };

        fn collect_sky_entities(
            object: u32,
            scripts: &std::collections::HashMap<u32, &UXGeoScript>,
            entities: &mut std::collections::HashSet<u32>,
            ancestry: &mut Vec<u32>,
        ) {
            match object.base() {
                0x0200_0000 => {
                    entities.insert(object);
                }
                0x0400_0000 => {
                    if ancestry.len() >= 64 || ancestry.contains(&object) {
                        return;
                    }
                    let Some(script) = scripts.get(&object).copied() else {
                        return;
                    };
                    ancestry.push(object);
                    for command in &script.commands {
                        match command.data {
                            UXGeoScriptCommandData::Entity { hashcode, file }
                                if file == u32::MAX || hashcode.is_local() =>
                            {
                                entities.insert(hashcode);
                            }
                            UXGeoScriptCommandData::SubScript { hashcode, file }
                                if file == u32::MAX || hashcode.is_local() =>
                            {
                                collect_sky_entities(hashcode, scripts, entities, ancestry);
                            }
                            _ => {}
                        }
                    }
                    ancestry.pop();
                }
                _ => {}
            }
        }

        let mut paths = std::fs::read_dir(&root)
            .expect("main-map sky root is missing")
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("edb"))
                    && path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with('m'))
            })
            .collect::<Vec<_>>();
        paths.sort();

        let mut audited_maps = 0usize;
        let mut fog_eligible_only_maps = Vec::new();
        for path in paths {
            let open_edb = || {
                let file = File::open(&path).expect("main-map fixture disappeared");
                EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                    .expect("main-map fixture is not a valid PC EDB")
            };

            let mut map_edb = open_edb();
            let maps = read_from_file(&mut map_edb);
            let sky_objects = maps
                .iter()
                .flat_map(|map| map.skies.iter().copied())
                .collect::<std::collections::HashSet<_>>();
            if sky_objects.is_empty() {
                continue;
            }
            let mut sky_object_list = sky_objects.iter().copied().collect::<Vec<_>>();
            sky_object_list.sort_unstable();

            let mut script_edb = open_edb();
            let scripts =
                UXGeoScript::read_all(&mut script_edb).expect("main-map sky Scripts did not parse");
            let scripts = scripts
                .iter()
                .map(|script| (script.hashcode, script))
                .collect::<std::collections::HashMap<_, _>>();
            let mut sky_entities = std::collections::HashSet::new();
            for sky in sky_objects {
                collect_sky_entities(sky, &scripts, &mut sky_entities, &mut Vec::new());
            }

            let mut entity_edb = open_edb();
            let header = entity_edb.header.clone();
            let endian = entity_edb.endian;
            let mut no_fog_count = 0usize;
            let mut fog_eligible_count = 0usize;
            let mut classified_entities = Vec::new();
            for record in header
                .entity_list
                .iter()
                .filter(|record| sky_entities.contains(&record.common.hashcode))
            {
                entity_edb
                    .seek(std::io::SeekFrom::Start(record.common.address as u64))
                    .expect("could not seek to main-map sky Entity");
                let entity = entity_edb
                    .read_type_args::<EXGeoEntity>(endian, (header.version, Platform::Pc))
                    .expect("main-map sky Entity did not parse");
                let flags = entity.base().map(|base| base.flags).unwrap_or_default();
                classified_entities.push((record.common.hashcode, flags));
                if flags & ROBOTS_ENTITY_FLAG_NO_FOG != 0 {
                    no_fog_count += 1;
                } else {
                    fog_eligible_count += 1;
                }
            }

            let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
            if no_fog_count == 0 {
                fog_eligible_only_maps.push(file_name.clone());
            }
            eprintln!(
                "{}: skies={:?} no_fog={} fog_eligible={} entities={:?}",
                file_name, sky_object_list, no_fog_count, fog_eligible_count, classified_entities
            );
            audited_maps += 1;
        }

        assert!(
            audited_maps >= 5,
            "too few main maps with skies were audited"
        );
        assert_eq!(fog_eligible_only_maps, ["m08_chas.edb"]);
    }

    #[test]
    fn real_m03_hub1_no_sky_zones_keep_the_large_no_fog_background_entity_when_requested() {
        let Ok(path) = std::env::var("EUROCHEF_REAL_M03_HUB1_EDB") else {
            return;
        };
        let open_edb = || {
            let file = File::open(&path).expect("m03_hub1 fixture is missing");
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
                .expect("m03_hub1 fixture is not a valid PC EDB")
        };

        let mut map_edb = open_edb();
        let maps = read_from_file(&mut map_edb);
        let map = maps.first().expect("m03_hub1 map is missing");
        assert_eq!(map.skies.first().copied(), Some(0x8400_000D));
        let no_sky_zones = map
            .zones
            .iter()
            .enumerate()
            .filter_map(|(index, zone)| (zone.identifier.sky_index == -1).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(no_sky_zones, [29, 30]);

        let mut script_edb = open_edb();
        let scripts =
            UXGeoScript::read_all(&mut script_edb).expect("m03_hub1 scripts did not parse");
        let base_sky = scripts
            .iter()
            .find(|script| script.hashcode == 0x8400_000D)
            .expect("m03_hub1 base sky Script 0x8400000D is missing");
        assert!(base_sky.commands.iter().any(|command| {
            matches!(
                command.data,
                UXGeoScriptCommandData::Entity {
                    hashcode: 0x8200_0040,
                    ..
                }
            )
        }));

        let mut entity_edb = open_edb();
        let header = entity_edb.header.clone();
        let endian = entity_edb.endian;
        let record = header
            .entity_list
            .iter()
            .find(|record| record.common.hashcode == 0x8200_0040)
            .expect("m03_hub1 background Entity 0x82000040 is missing");
        entity_edb
            .seek(std::io::SeekFrom::Start(record.common.address as u64))
            .expect("could not seek to m03_hub1 background Entity");
        let entity = entity_edb
            .read_type_args::<EXGeoEntity>(endian, (header.version, Platform::Pc))
            .expect("m03_hub1 background Entity did not parse");
        let base = entity
            .base()
            .expect("m03_hub1 background Entity has no base");
        assert_ne!(base.flags & 0x10, 0);
        assert!(base.bounds_box[0][0] < 599.34375 && base.bounds_box[1][0] > 948.65625);
        assert!(base.bounds_box[0][2] < -231.75 && base.bounds_box[1][2] > -93.9375);
    }

    #[test]
    fn real_m03_hub1_ball_track_paths_are_already_in_the_map_path_catalog_when_requested() {
        let Ok(path) = std::env::var("EUROCHEF_REAL_M03_HUB1_EDB") else {
            return;
        };
        let file = File::open(&path).expect("m03_hub1 fixture is missing");
        let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
            .expect("m03_hub1 fixture is not a valid PC EDB");
        let maps = read_from_file(&mut edb);
        let map = maps.first().expect("m03_hub1 map is missing");

        for path_hash in [
            0x0B00_0008,
            0x0B00_000B,
            0x0B00_000C,
            0x0B00_000D,
            0x0B00_000E,
            0x0B00_0010,
            0x0B00_0011,
            0x0B00_0012,
            0x0B00_0013,
        ] {
            let path = map
                .paths
                .iter()
                .find(|path| path.hashcode == path_hash)
                .unwrap_or_else(|| {
                    panic!("BallTrack path {path_hash:#010X} is missing from m03_hub1")
                });
            assert!(
                !path.nodes.is_empty(),
                "BallTrack path {path_hash:#010X} has no nodes"
            );
        }
    }

    #[test]
    fn real_m04_cour_platform_paths_start_at_serialized_trigger_positions_when_requested() {
        let Ok(path) = std::env::var("EUROCHEF_REAL_M04_COUR_EDB") else {
            return;
        };
        let file = File::open(&path).expect("m04_cour fixture is missing");
        let mut edb = EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc)
            .expect("m04_cour fixture is not a valid PC EDB");
        let maps = read_from_file(&mut edb);
        let map = maps.first().expect("m04_cour map is missing");

        let mut path_platform_count = 0usize;
        for (index, trigger) in map
            .triggers
            .iter()
            .enumerate()
            .filter(|(_, trigger)| trigger.ttype == 8)
        {
            let Some(path_hash) = robots_trigger_path_hash(trigger.ttype, &trigger.data) else {
                continue;
            };
            let Some(path) = map.paths.iter().find(|path| path.hashcode == path_hash) else {
                continue;
            };
            if crate::map_runtime::runtime_path_route(path).len() < 2 {
                continue;
            }
            path_platform_count += 1;
            let sample =
                crate::map_runtime::runtime_path_preview_sample_at_distance(map, trigger, 0.0)
                    .unwrap_or_else(|| {
                        panic!("Courtyard Platform #{index} has no initial path sample")
                    });
            assert!(
                sample.position.distance(trigger.position) < 0.001,
                "Courtyard Platform #{index} shifted from {:?} to {:?}",
                trigger.position,
                sample.position,
            );
        }
        assert!(
            path_platform_count >= 7,
            "m04_cour path-driven Platform corpus unexpectedly shrank"
        );
    }

    #[test]
    fn real_robots_v248_m10_rollerbot_behavior_path_when_game_root_is_configured() {
        let Ok(game_root) = std::env::var("EUROCHEF_ROBOTS_GAME_ROOT") else {
            return;
        };
        let root =
            Path::new(&game_root).join("_eurotools_out/extracted_main/robots/binary/_bin_pc");
        let m10_path = root.join("m10_boss.edb");
        let texture_file = File::open(&m10_path).expect("open m10_boss.edb for texture scan");
        let mut texture_edb = EdbFile::new(Box::new(BufReader::new(texture_file)), Platform::Pc)
            .expect("parse m10_boss.edb for texture scan");
        let texture_headers = texture_edb.header.texture_list.data().clone();
        let mut scrolling_textures = Vec::new();
        for (texture_index, header) in texture_headers.into_iter().enumerate() {
            texture_edb
                .seek(SeekFrom::Start(header.common.address as u64))
                .expect("seek m10 texture");
            let texture = texture_edb
                .read_type_args::<EXGeoTexture>(texture_edb.endian, (248, Platform::Pc))
                .expect("parse m10 texture");
            if texture.scroll_u != 0 || texture.scroll_v != 0 {
                scrolling_textures.push((
                    texture_index,
                    header.common.hashcode,
                    texture.scroll_u,
                    texture.scroll_v,
                ));
            }
        }
        assert_eq!(
            scrolling_textures,
            vec![(8, 0x8600_0008, 25, 0), (9, 0x8600_0009, 0, 33)],
            "m10_boss scrolling texture corpus changed"
        );

        let file = File::open(&m10_path).expect("open m10_boss.edb");
        let mut edb =
            EdbFile::new(Box::new(BufReader::new(file)), Platform::Pc).expect("parse m10_boss.edb");
        let maps = read_from_file(&mut edb);

        fn collect_scrolling_texture_meshes(
            entity: &EXGeoEntity,
            meshes: &mut Vec<(u16, u16, Vec<u16>, usize)>,
        ) {
            match entity {
                EXGeoEntity::Mesh(mesh) => {
                    if mesh
                        .texture_list
                        .iter()
                        .any(|index| matches!(*index, 8 | 9))
                    {
                        meshes.push((
                            mesh.data.base.gdi_index,
                            mesh.data.base.gdi_count,
                            mesh.texture_list.clone(),
                            mesh.robots_face_info
                                .as_ref()
                                .map(|info| info.groups.iter().map(|group| group.faces.len()).sum())
                                .unwrap_or_default(),
                        ));
                    }
                }
                EXGeoEntity::Split(split) => {
                    for child in &split.entities {
                        collect_scrolling_texture_meshes(child, meshes);
                    }
                }
                _ => {}
            }
        }

        let entity_headers = edb.header.entity_list.data().clone();
        let mut scrolling_entity_users = Vec::new();
        for header in entity_headers {
            edb.seek(SeekFrom::Start(header.common.address as u64))
                .expect("seek m10 entity for scrolling-texture census");
            let entity = edb
                .read_type_args::<EXGeoEntity>(edb.endian, (edb.header.version, Platform::Pc))
                .expect("parse m10 entity for scrolling-texture census");
            let mut meshes = Vec::new();
            collect_scrolling_texture_meshes(&entity, &mut meshes);
            if !meshes.is_empty() {
                scrolling_entity_users.push((header.common.hashcode, meshes));
            }
        }
        assert_eq!(
            scrolling_entity_users,
            vec![(0x8200_0000, vec![(0, 0, vec![8, 49, 9], 0)])],
            "m10_boss scrolling texture users changed"
        );

        let map = maps
            .iter()
            .find(|map| {
                map.paths.iter().any(|path| path.hashcode == 0x0B00_0049)
                    && map
                        .triggers
                        .iter()
                        .any(|trigger| trigger.ttype == super::ROBOTS_SWEEPER_CONTROLLER_TYPE)
            })
            .expect("m10 RollerBot path/controller map");
        let (_, _, stage226_ref_entities) = crate::entities::read_from_file(&mut edb, None)
            .expect("decode m10 entities for RollerBot collision corridor");
        assert_eq!(
            maps.iter()
                .flat_map(|map| map.paths.iter())
                .filter(|path| path.hashcode == 0x0B00_0049)
                .count(),
            1,
            "RollerBot path 0x0B000049 ownership changed"
        );
        let path = map
            .paths
            .iter()
            .find(|path| path.hashcode == 0x0B00_0049)
            .expect("m10 RollerBot path 0x0B000049");
        assert_eq!(path.nodes.len(), 11);
        assert_eq!(path.links.len(), 20);
        assert_eq!(
            path.nodes
                .iter()
                .map(|node| node.num_links)
                .collect::<Vec<_>>(),
            vec![2, 2, 2, 2, 2, 8, 8, 4, 4, 3, 3]
        );
        assert!(path.nodes.iter().all(|node| {
            node.size.x == 4.0 && node.size.y == 0.0 && node.value == [0; 4] && node.flags == 0
        }));
        assert_eq!(
            path.links,
            vec![
                (0, 6),
                (1, 6),
                (2, 6),
                (3, 6),
                (4, 5),
                (3, 5),
                (2, 5),
                (1, 5),
                (5, 8),
                (6, 7),
                (5, 9),
                (6, 10),
                (8, 10),
                (7, 9),
                (9, 8),
                (10, 7),
                (0, 5),
                (5, 7),
                (4, 6),
                (6, 8),
            ]
        );
        let stage227_node2 = path.nodes[2].position;
        let stage227_node6 = path.nodes[6].position;
        let stage293_node4 = path.nodes[4].position;
        let stage331_node5 = path.nodes[5].position;
        let stage368_node9 = path.nodes[9].position;
        let stage391_node8 = path.nodes[8].position;
        let stage_new_node7 = path.nodes[7].position;
        let stage_new_node10 = path.nodes[10].position;
        let stage252_node1 = path.nodes[1].position;
        let stage252_node6_neighbors = path
            .links
            .iter()
            .filter_map(|&(a, b)| {
                if a == 6 {
                    Some(b)
                } else if b == 6 {
                    Some(a)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let stage267_node1_neighbors = path
            .links
            .iter()
            .filter_map(|&(a, b)| {
                if a == 1 {
                    Some(b)
                } else if b == 1 {
                    Some(a)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let stage307_node4_neighbors = path
            .links
            .iter()
            .filter_map(|&(a, b)| {
                if a == 4 {
                    Some(b)
                } else if b == 4 {
                    Some(a)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let stage_latest_node3_neighbors = path
            .links
            .iter()
            .filter_map(|&(a, b)| {
                if a == 3 {
                    Some(b)
                } else if b == 3 {
                    Some(a)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let stage367_node5_neighbors = path
            .links
            .iter()
            .filter_map(|&(a, b)| {
                if a == 5 {
                    Some(b)
                } else if b == 5 {
                    Some(a)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let stage390_node9_neighbors = path
            .links
            .iter()
            .filter_map(|&(a, b)| {
                if a == 9 {
                    Some(b)
                } else if b == 9 {
                    Some(a)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert!((stage252_node1.x - -14.779_201_5).abs() <= 1.0e-5);
        assert!((stage252_node1.y - 0.0).abs() <= 1.0e-6);
        assert!((stage252_node1.z - 20.417_852).abs() <= 1.0e-5);
        assert_eq!(stage252_node6_neighbors, vec![0, 1, 2, 3, 7, 10, 4, 8]);
        assert_eq!(stage267_node1_neighbors, vec![6, 5]);
        assert_eq!(stage307_node4_neighbors, vec![5, 6]);
        assert_eq!(stage_latest_node3_neighbors, vec![6, 5]);
        assert_eq!(stage367_node5_neighbors, vec![4, 3, 2, 1, 8, 9, 0, 7]);
        let stage402_node8_neighbors = path
            .links
            .iter()
            .filter_map(|&(a, b)| {
                if a == 8 {
                    Some(b)
                } else if b == 8 {
                    Some(a)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(stage390_node9_neighbors, vec![5, 7, 8]);
        assert_eq!(stage402_node8_neighbors, vec![5, 10, 9, 6]);
        assert!((stage227_node2.x - 0.0).abs() <= 1.0e-5);
        assert!((stage227_node2.y - 0.0).abs() <= 1.0e-6);
        assert!((stage227_node2.z - 20.477_207).abs() <= 1.0e-5);
        assert!((stage227_node6.x - 21.130_104).abs() <= 1.0e-5);
        assert!((stage227_node6.y - 0.0).abs() <= 1.0e-6);
        assert!((stage227_node6.z - 15.847_578).abs() <= 1.0e-5);
        assert!((stage293_node4.x - 27.837_132).abs() <= 1.0e-5);
        assert!((stage293_node4.y - 0.0).abs() <= 1.0e-6);
        assert!((stage293_node4.z - 20.714_624).abs() <= 1.0e-5);
        assert!((stage331_node5.x - -22.020_416).abs() <= 1.0e-5);
        assert!((stage331_node5.y - 0.0).abs() <= 1.0e-6);
        assert!((stage331_node5.z - 15.906_932).abs() <= 1.0e-5);
        assert!((stage368_node9.x - -8.547_008_5).abs() <= 1.0e-5);
        assert!((stage368_node9.y - 0.0).abs() <= 1.0e-6);
        assert!((stage368_node9.z - 16.025_64).abs() <= 1.0e-5);
        assert!((stage391_node8.x - 16.381_765).abs() <= 1.0e-5);
        assert!((stage391_node8.y - 0.0).abs() <= 1.0e-6);
        assert!((stage391_node8.z - 10.802_468).abs() <= 1.0e-5);
        assert!((stage_new_node7.x - -18.162_392).abs() <= 1.0e-5);
        assert!((stage_new_node7.y - 0.0).abs() <= 1.0e-6);
        assert!((stage_new_node7.z - 10.565_052).abs() <= 1.0e-5);
        assert_eq!(stage_new_node10.x.to_bits(), 0x411F_8B4E);
        assert_eq!(stage_new_node10.y.to_bits(), 0x0000_0000);
        assert_eq!(stage_new_node10.z.to_bits(), 0x4183_0DDB);
        let stage227_delta = stage227_node6 - stage227_node2;
        assert!((stage227_delta.length() - 21.631_338).abs() <= 1.0e-5);
        assert!((stage227_delta.x.atan2(stage227_delta.z) - 1.786_489_1).abs() <= 1.0e-6);

        // Eye2 row28 spawns one RollerBot at [eye_x, 2.4, 35]. Resolve the actual shipped
        // Eye2 through the controller's third Eye link rather than borrowing the synthetic
        // production-test anchor. Native 0x00420990 uses a 1-unit arrival radius and one first
        // update can move at most 10/60 before damping.
        let controller = map
            .triggers
            .iter()
            .find(|trigger| trigger.ttype == super::ROBOTS_SWEEPER_CONTROLLER_TYPE)
            .expect("m10 Sweeper controller");
        let eye2_index = usize::try_from(controller.links[2]).expect("m10 Eye2 trigger link");
        let eye2 = map
            .triggers
            .get(eye2_index)
            .expect("m10 Eye2 trigger by controller link");
        assert_eq!(eye2.ttype, super::ROBOTS_SWEEPER_EYE_TYPE);
        let spawn_plan =
            super::robots_sweeper_boss_spawn_transform(eye2.position.x, 1.0, eye2.rotation.y, 0);
        let spawn = Vec3::new(
            spawn_plan.position[0],
            spawn_plan.position[1],
            spawn_plan.position[2],
        );
        let (nearest_index, nearest_squared) = path
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (index, node.position.distance_squared(spawn)))
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .expect("RollerBot path has no nodes");
        assert_eq!(nearest_index, 2);
        let nearest_distance = nearest_squared.sqrt();
        let nearest_node = &path.nodes[nearest_index];
        let horizontal_dx = nearest_node.position.x - spawn.x;
        let horizontal_dz = nearest_node.position.z - spawn.z;
        let horizontal_distance =
            (horizontal_dx * horizontal_dx + horizontal_dz * horizontal_dz).sqrt();
        assert!((horizontal_distance - 14.523_612).abs() < 0.001);
        assert!((nearest_distance - 14.719_766).abs() < 0.001);
        assert_eq!(map.mapzone_entities.len(), 1);
        let zone_entity = &map.mapzone_entities[0];
        assert_eq!(zone_entity.entity_refptr, 0);
        let zone_mesh = stage226_ref_entities
            .iter()
            .find(|entry| entry.hashcode == zone_entity.entity_refptr)
            .and_then(|entry| entry.data.as_ref().ok())
            .map(|(_, mesh)| mesh)
            .expect("m10 arena refpointer #0 mesh");
        assert_eq!(zone_mesh.robots_raycast_triangles.len(), 18_275);
        assert!(map
            .placements
            .iter()
            .all(|placement| placement.engine_flags & 0x08 == 0));

        let corridor_min = Vec3::new(
            spawn.x.min(nearest_node.position.x) - 0.4,
            0.0,
            spawn.z.min(nearest_node.position.z) - 0.4,
        );
        let corridor_max = Vec3::new(
            spawn.x.max(nearest_node.position.x) + 0.4,
            spawn.y + 0.922_545_8 + 0.524_026_6 + 0.4,
            spawn.z.max(nearest_node.position.z) + 0.4,
        );
        let mut corridor_triangles = 0usize;
        let mut min_abs_normal_y = 1.0_f32;
        let mut candidate_y_min = f32::INFINITY;
        let mut candidate_y_max = f32::NEG_INFINITY;
        for triangle in &zone_mesh.robots_raycast_triangles {
            let [a, b, c] = triangle.positions;
            let tri_min = Vec3::new(
                a.x.min(b.x).min(c.x),
                a.y.min(b.y).min(c.y),
                a.z.min(b.z).min(c.z),
            );
            let tri_max = Vec3::new(
                a.x.max(b.x).max(c.x),
                a.y.max(b.y).max(c.y),
                a.z.max(b.z).max(c.z),
            );
            if tri_max.x < corridor_min.x
                || tri_min.x > corridor_max.x
                || tri_max.y < corridor_min.y
                || tri_min.y > corridor_max.y
                || tri_max.z < corridor_min.z
                || tri_min.z > corridor_max.z
            {
                continue;
            }
            corridor_triangles += 1;
            let normal = (b - a).cross(c - a).normalize_or_zero();
            min_abs_normal_y = min_abs_normal_y.min(normal.y.abs());
            candidate_y_min = candidate_y_min.min(tri_min.y);
            candidate_y_max = candidate_y_max.max(tri_max.y);
        }
        assert_eq!(corridor_triangles, 21);
        assert!(min_abs_normal_y > 0.999_999_9);
        assert!(candidate_y_min.abs() <= 1.0e-6);
        assert!(candidate_y_max.abs() <= 1.0e-6);

        // Stage253: after tick454 selects node1, prove the full retained Roller capsule
        // corridor from the exact arrival pose to node1 has no non-floor static contact.
        let stage253_start = Vec3::new(21.346_087, 0.001_480_8, 16.794_098);
        let stage253_corridor_min = Vec3::new(
            stage253_start.x.min(stage252_node1.x) - 0.4,
            0.0,
            stage253_start.z.min(stage252_node1.z) - 0.4,
        );
        let stage253_corridor_max = Vec3::new(
            stage253_start.x.max(stage252_node1.x) + 0.4,
            stage253_start.y + 0.922_545_8 + 0.524_026_6 + 0.4,
            stage253_start.z.max(stage252_node1.z) + 0.4,
        );
        let mut stage253_corridor_triangles = 0usize;
        let mut stage253_min_abs_normal_y = 1.0_f32;
        let mut stage253_candidate_y_min = f32::INFINITY;
        let mut stage253_candidate_y_max = f32::NEG_INFINITY;
        for triangle in &zone_mesh.robots_raycast_triangles {
            let [a, b, c] = triangle.positions;
            let tri_min = Vec3::new(
                a.x.min(b.x).min(c.x),
                a.y.min(b.y).min(c.y),
                a.z.min(b.z).min(c.z),
            );
            let tri_max = Vec3::new(
                a.x.max(b.x).max(c.x),
                a.y.max(b.y).max(c.y),
                a.z.max(b.z).max(c.z),
            );
            if tri_max.x < stage253_corridor_min.x
                || tri_min.x > stage253_corridor_max.x
                || tri_max.y < stage253_corridor_min.y
                || tri_min.y > stage253_corridor_max.y
                || tri_max.z < stage253_corridor_min.z
                || tri_min.z > stage253_corridor_max.z
            {
                continue;
            }
            stage253_corridor_triangles += 1;
            let normal = (b - a).cross(c - a).normalize_or_zero();
            stage253_min_abs_normal_y = stage253_min_abs_normal_y.min(normal.y.abs());
            stage253_candidate_y_min = stage253_candidate_y_min.min(tri_min.y);
            stage253_candidate_y_max = stage253_candidate_y_max.max(tri_max.y);
        }
        assert_eq!(stage253_corridor_triangles, 85);
        assert!(stage253_min_abs_normal_y > 0.999_999_9);
        assert!(stage253_candidate_y_min.abs() <= 1.0e-6);
        assert!(stage253_candidate_y_max.abs() <= 1.0e-6);
        assert!(nearest_distance > 1.0 + 10.0 / 60.0);

        // Stage236 broad node2->node6 swept-capsule census. This deliberately covers the
        // exact curved Roller trajectory plus the node6 endpoint rather than only the chord.
        let node6_corridor_min = Vec3::new(-0.4, 0.0, 11.7);
        let node6_corridor_max = Vec3::new(21.8, 1.9, 21.9);
        let mut node6_corridor_triangles = 0usize;
        let mut node6_min_abs_normal_y = 1.0_f32;
        let mut node6_candidate_y_min = f32::INFINITY;
        let mut node6_candidate_y_max = f32::NEG_INFINITY;
        for triangle in &zone_mesh.robots_raycast_triangles {
            let [a, b, c] = triangle.positions;
            let tri_min = Vec3::new(
                a.x.min(b.x).min(c.x),
                a.y.min(b.y).min(c.y),
                a.z.min(b.z).min(c.z),
            );
            let tri_max = Vec3::new(
                a.x.max(b.x).max(c.x),
                a.y.max(b.y).max(c.y),
                a.z.max(b.z).max(c.z),
            );
            if tri_max.x < node6_corridor_min.x
                || tri_min.x > node6_corridor_max.x
                || tri_max.y < node6_corridor_min.y
                || tri_min.y > node6_corridor_max.y
                || tri_max.z < node6_corridor_min.z
                || tri_min.z > node6_corridor_max.z
            {
                continue;
            }
            node6_corridor_triangles += 1;
            let normal = (b - a).cross(c - a).normalize_or_zero();
            node6_min_abs_normal_y = node6_min_abs_normal_y.min(normal.y.abs());
            node6_candidate_y_min = node6_candidate_y_min.min(tri_min.y);
            node6_candidate_y_max = node6_candidate_y_max.max(tri_max.y);
        }
        assert_eq!(node6_corridor_triangles, 95);
        assert!(node6_min_abs_normal_y > 0.999_999_9);
        assert!(node6_candidate_y_min.abs() <= 1.0e-6);
        assert!(node6_candidate_y_max.abs() <= 1.0e-6);

        // Stage245 MalfBot state0 floor proof. Eye1/Eye3 MalfBots have no target/movement
        // command in the retained corridor; their only unconstrained body input is gravity.
        // Census a deliberately generous 4-unit XZ column around each shipped spawn, larger
        // than the 3.5299912 HitArea owner enclosure used by projectile queries. If every
        // arena face reachable in that column is the horizontal Y=0 floor, contact cannot
        // inject lateral owner motion and gravity cannot make the owner diverge indefinitely.
        for eye_ordinal in [1_usize, 3_usize] {
            let eye_index = usize::try_from(controller.links[eye_ordinal])
                .expect("m10 MalfBot Eye trigger link");
            let eye = map
                .triggers
                .get(eye_index)
                .expect("m10 MalfBot Eye trigger by controller link");
            assert_eq!(eye.ttype, super::ROBOTS_SWEEPER_EYE_TYPE);
            let malf_spawn_plan =
                super::robots_sweeper_boss_spawn_transform(eye.position.x, 1.0, eye.rotation.y, 0);
            let malf_spawn = Vec3::new(
                malf_spawn_plan.position[0],
                malf_spawn_plan.position[1],
                malf_spawn_plan.position[2],
            );
            let column_min = Vec3::new(malf_spawn.x - 4.0, -0.5, malf_spawn.z - 4.0);
            let column_max = Vec3::new(malf_spawn.x + 4.0, malf_spawn.y + 4.0, malf_spawn.z + 4.0);
            let mut column_triangles = 0usize;
            let mut column_min_abs_normal_y = 1.0_f32;
            let mut column_candidate_y_min = f32::INFINITY;
            let mut column_candidate_y_max = f32::NEG_INFINITY;
            for triangle in &zone_mesh.robots_raycast_triangles {
                let [a, b, c] = triangle.positions;
                let tri_min = Vec3::new(
                    a.x.min(b.x).min(c.x),
                    a.y.min(b.y).min(c.y),
                    a.z.min(b.z).min(c.z),
                );
                let tri_max = Vec3::new(
                    a.x.max(b.x).max(c.x),
                    a.y.max(b.y).max(c.y),
                    a.z.max(b.z).max(c.z),
                );
                if tri_max.x < column_min.x
                    || tri_min.x > column_max.x
                    || tri_max.y < column_min.y
                    || tri_min.y > column_max.y
                    || tri_max.z < column_min.z
                    || tri_min.z > column_max.z
                {
                    continue;
                }
                column_triangles += 1;
                let normal = (b - a).cross(c - a).normalize_or_zero();
                column_min_abs_normal_y = column_min_abs_normal_y.min(normal.y.abs());
                column_candidate_y_min = column_candidate_y_min.min(tri_min.y);
                column_candidate_y_max = column_candidate_y_max.max(tri_max.y);
            }
            assert!(
                column_triangles > 0
                    && column_min_abs_normal_y > 0.999_999_9
                    && column_candidate_y_min.abs() <= 1.0e-6
                    && column_candidate_y_max.abs() <= 1.0e-6,
                "m10 Eye{eye_ordinal} MalfBot gravity column changed: triangles={column_triangles} min_abs_normal_y={column_min_abs_normal_y} y=[{column_candidate_y_min},{column_candidate_y_max}] spawn={malf_spawn:?}"
            );
        }
    }
}
