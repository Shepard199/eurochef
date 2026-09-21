use super::runtime_ai_explosion::native_common_monster_explosion_uid;
use super::*;
use crate::map_runtime::{
    runtime_character_animation_datum_world_shape,
    runtime_character_animation_datum_world_transform, runtime_live_character_collision_shape,
    runtime_test_live_character_hit_candidate, RuntimeCharacterDatumWorldTransform,
    RuntimeCharacterHitCandidateResult, RuntimeCharacterHitQueryGeometry,
    RuntimeCharacterWorldShape,
};
use crate::maps::{ProcessedCharacterVisual, ROBOTS_ANIM_MODE_DEFAULT};
use eurochef_shared::robots_runtime::{
    ai_character::RobotsAiHandlerClass,
    hit_candidate_policy::{RobotsHitQueryCandidateContext, ROBOTS_HIT_QUERY_RAW_GROUP1},
    hit_narrowphase::{RobotsHitSampleSweepPlan, RobotsHitSweepSample},
};

const RUNTIME_AI_DYNAMIC_ID_BIT: u32 = 0x8000_0000;

fn runtime_ai_character_live_policy(
    native_lifecycle_valid: bool,
    simulate_ai: bool,
    native_xitem_live: bool,
    preview_live: bool,
) -> bool {
    if !native_lifecycle_valid && simulate_ai {
        preview_live
    } else {
        native_xitem_live
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct RuntimeAiInstanceKey(u64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct RuntimeDynamicAiSnapshot {
    pub(super) key: u64,
    pub(super) live_id: u32,
    pub(super) selector: u32,
    pub(super) pending_destroy: bool,
    pub(super) owner_position: Vec3,
    pub(super) owner_yaw: f32,
    pub(super) bootstrap: Option<NativeSweeperBossGenericAiBootstrap>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RuntimeAiHitCandidateSnapshot {
    pub(super) key: u64,
    pub(super) handler_class: RobotsAiHandlerClass,
    pub(super) common_monster_explosion_uid: Option<u32>,
}

impl RuntimeAiInstanceKey {
    pub(super) fn serialized(map_hash: u32, trigger_index: usize) -> Option<Self> {
        let trigger_index = u32::try_from(trigger_index).ok()?;
        if trigger_index & RUNTIME_AI_DYNAMIC_ID_BIT != 0 {
            return None;
        }
        Some(Self(((map_hash as u64) << 32) | trigger_index as u64))
    }

    pub(super) fn dynamic(map_hash: u32, live_id: u32) -> Option<Self> {
        if live_id & RUNTIME_AI_DYNAMIC_ID_BIT != 0 {
            return None;
        }
        Some(Self(
            ((map_hash as u64) << 32) | (RUNTIME_AI_DYNAMIC_ID_BIT | live_id) as u64,
        ))
    }

    pub(super) const fn raw(self) -> u64 {
        self.0
    }

    pub(super) const fn raw_is_dynamic_for_map(raw: u64, map_hash: u32) -> bool {
        (raw >> 32) as u32 == map_hash && (raw as u32 & RUNTIME_AI_DYNAMIC_ID_BIT) != 0
    }

    pub(super) const fn dynamic_live_id_for_map(raw: u64, map_hash: u32) -> Option<u32> {
        if !Self::raw_is_dynamic_for_map(raw, map_hash) {
            return None;
        }
        Some(raw as u32 & !RUNTIME_AI_DYNAMIC_ID_BIT)
    }
}

impl MapFrame {
    pub(super) fn runtime_character_body_key(map_hash: u32, trigger_index: usize) -> u64 {
        RuntimeAiInstanceKey::serialized(map_hash, trigger_index)
            .expect("serialized trigger index must fit the runtime AI key namespace")
            .raw()
    }

    /// Synchronize editor-side gameplay-body ownership from the current map's
    /// runtime-created character triggers. This is deliberately separate from
    /// `native_camera_player_preview`: bodies come only from the real character
    /// EDB AnimSkin MapCollision channel and the trigger/XItem owner pose.
    pub(super) fn sync_runtime_character_bodies(&mut self, map: &ProcessedMap, runtime_time: f32) {
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
            let Some(mut fresh) = RuntimeCharacterBodyState::from_trigger(trigger) else {
                self.runtime_character_bodies.remove(&key);
                continue;
            };
            fresh.initial_animation_seconds = runtime_time.max(0.0);
            self.runtime_character_bodies
                .entry(key)
                .and_modify(|body| {
                    // Owner pose is runtime state after XItem creation. Recopying
                    // the serialized Trigger transform here would erase native
                    // AI/Physics movement every canvas sync. Map/runtime reset
                    // clears this table, so serialized pose remains bootstrap-only.
                    body.sync_initial_animation(trigger, runtime_time);
                    // Character EDB metadata is immutable for this ProcessedMap,
                    // but refresh it too so future hot-reload/re-resolution does
                    // not leave a stale shape behind.
                    body.collision = fresh.collision;
                    body.hit_area = fresh.hit_area;
                    body.registration_mask = fresh.registration_mask;
                    body.raw_hit_query_group = fresh.raw_hit_query_group;
                })
                .or_insert(fresh);
        }
    }

    pub(super) fn runtime_dynamic_ai_snapshots(
        &self,
        map: &ProcessedMap,
    ) -> Vec<RuntimeDynamicAiSnapshot> {
        self.native_sweeper_boss_runtime
            .as_ref()
            .map(|runtime| {
                runtime
                    .live_ai
                    .entries()
                    .filter_map(|entry| {
                        let serialized_origin = match entry.source {
                            NativeSweeperBossLiveAiSource::SerializedTrigger { .. } => true,
                            NativeSweeperBossLiveAiSource::TransporterCarry { trigger_index }
                            | NativeSweeperBossLiveAiSource::TransporterReleased {
                                trigger_index,
                            } => map
                                .triggers
                                .get(trigger_index)
                                .and_then(|trigger| trigger.character_visual.as_ref())
                                .is_some_and(|visual| {
                                    visual.config_index == u32::from(entry.config_index)
                                        && entry.file.is_none_or(|file| file == visual.file)
                                }),
                            NativeSweeperBossLiveAiSource::EyeSpawn => false,
                        };
                        if serialized_origin {
                            return None;
                        }
                        let key = RuntimeAiInstanceKey::dynamic(map.hashcode, entry.id)?.raw();
                        let selector = u32::from(entry.config_index);
                        let visual = map
                            .runtime_character_visuals
                            .get(&(ROBOTS_SWEEPER_MONSTER_RUNTIME_TYPE, selector))?;
                        if entry.file.is_some_and(|file| file != visual.file) {
                            return None;
                        }
                        Some(RuntimeDynamicAiSnapshot {
                            key,
                            live_id: entry.id,
                            selector,
                            pending_destroy: entry.pending_destroy,
                            owner_position: Vec3::new(
                                entry.position[0],
                                entry.position[1],
                                entry.position[2],
                            ),
                            owner_yaw: entry.yaw,
                            bootstrap: runtime.post_ratchet_ai.generic_bootstrap(entry.id),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    /// Enumerate currently live AI bodies independent of creator provenance.
    /// Serialized Trigger/XItem and factory-created Sweeper/Transporter instances
    /// collapse to the same runtime key + concrete Handler identity from here on.
    pub(super) fn runtime_ai_hit_candidates(
        &self,
        map: &ProcessedMap,
    ) -> Vec<RuntimeAiHitCandidateSnapshot> {
        let mut candidates = Vec::new();
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            let Some(visual) = trigger.character_visual.as_ref() else {
                continue;
            };
            let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
            if self.runtime_ai_character_xitem_live(map.hashcode, trigger_index)
                && self.runtime_character_bodies.contains_key(&key)
            {
                candidates.push(RuntimeAiHitCandidateSnapshot {
                    key,
                    handler_class: visual.handler_class,
                    common_monster_explosion_uid: native_common_monster_explosion_uid(visual),
                });
            }
        }

        for snapshot in self.runtime_dynamic_ai_snapshots(map) {
            if snapshot.pending_destroy
                || !self.runtime_character_bodies.contains_key(&snapshot.key)
            {
                continue;
            }
            let Some(visual) = map
                .runtime_character_visuals
                .get(&(ROBOTS_SWEEPER_MONSTER_RUNTIME_TYPE, snapshot.selector))
            else {
                continue;
            };
            candidates.push(RuntimeAiHitCandidateSnapshot {
                key: snapshot.key,
                handler_class: visual.handler_class,
                common_monster_explosion_uid: native_common_monster_explosion_uid(visual),
            });
        }
        candidates
    }

    /// Resolve immutable character metadata from the unified serialized/dynamic AI
    /// key namespace. Consumers such as AnimDatum queries should use this instead of
    /// re-deriving Trigger-vs-factory provenance themselves.
    pub(super) fn runtime_ai_visual_by_key<'a>(
        &self,
        map: &'a ProcessedMap,
        key: u64,
    ) -> Option<&'a ProcessedCharacterVisual> {
        if (key >> 32) as u32 != map.hashcode {
            return None;
        }
        let raw_id = key as u32;
        if raw_id & RUNTIME_AI_DYNAMIC_ID_BIT == 0 {
            return map.triggers.get(raw_id as usize)?.character_visual.as_ref();
        }
        let selector = self
            .runtime_dynamic_ai_snapshots(map)
            .into_iter()
            .find(|snapshot| snapshot.key == key)?
            .selector;
        map.runtime_character_visuals
            .get(&(ROBOTS_SWEEPER_MONSTER_RUNTIME_TYPE, selector))
    }

    pub(super) fn runtime_ai_animation_datum_world_transform_by_key(
        &self,
        map: &ProcessedMap,
        key: u64,
        datum_hashcode: u32,
    ) -> Option<RuntimeCharacterDatumWorldTransform> {
        let body = self.runtime_character_bodies.get(&key)?;
        let visual = self.runtime_ai_visual_by_key(map, key)?;
        let (anim_mode, pose_seconds) = self
            .native_ai_animation_sample_by_key(key)
            .unwrap_or((ROBOTS_ANIM_MODE_DEFAULT, body.initial_animation_seconds));
        runtime_character_animation_datum_world_transform(
            body.owner_position,
            body.owner_rotation,
            body.native_transform_scale_xyz(),
            visual,
            anim_mode,
            datum_hashcode,
            pose_seconds,
        )
    }

    pub(super) fn runtime_ai_animation_datum_world_shape_by_key(
        &self,
        map: &ProcessedMap,
        key: u64,
        datum_hashcode: u32,
    ) -> Option<RuntimeCharacterWorldShape> {
        let body = self.runtime_character_bodies.get(&key)?;
        let visual = self.runtime_ai_visual_by_key(map, key)?;
        let (anim_mode, pose_seconds) = self
            .native_ai_animation_sample_by_key(key)
            .unwrap_or((ROBOTS_ANIM_MODE_DEFAULT, body.initial_animation_seconds));
        runtime_character_animation_datum_world_shape(
            body.owner_position,
            body.owner_rotation,
            body.native_transform_scale_xyz(),
            visual,
            anim_mode,
            datum_hashcode,
            pose_seconds,
        )
    }

    /// Bind truly factory-created Sweeper/Transporter monsters to the same
    /// gameplay-body table as serialized AI without fabricating Trigger records.
    pub(super) fn sync_runtime_dynamic_character_bodies(&mut self, map: &ProcessedMap) {
        let dynamic_snapshots = self.runtime_dynamic_ai_snapshots(map);
        let mut live_dynamic_keys = Vec::with_capacity(dynamic_snapshots.len());
        for snapshot in dynamic_snapshots {
            if snapshot.pending_destroy {
                continue;
            }
            let Some(visual) = map
                .runtime_character_visuals
                .get(&(ROBOTS_SWEEPER_MONSTER_RUNTIME_TYPE, snapshot.selector))
            else {
                continue;
            };
            let Some(fresh) = RuntimeCharacterBodyState::from_visual(
                visual,
                ROBOTS_HIT_QUERY_RAW_GROUP1,
                snapshot.owner_position,
                Quat::from_rotation_y(snapshot.owner_yaw),
            ) else {
                continue;
            };
            live_dynamic_keys.push(snapshot.key);
            self.runtime_character_bodies
                .entry(snapshot.key)
                .and_modify(|body| {
                    // Once created, the body is the mutable owner pose. Registry
                    // spawn coordinates remain bootstrap evidence and must not erase
                    // locomotion on every canvas sync.
                    body.collision = fresh.collision;
                    body.hit_area = fresh.hit_area;
                    body.registration_mask = fresh.registration_mask;
                    body.raw_hit_query_group = fresh.raw_hit_query_group;
                })
                .or_insert(fresh);
        }

        self.runtime_character_bodies.retain(|key, _| {
            !RuntimeAiInstanceKey::raw_is_dynamic_for_map(*key, map.hashcode)
                || live_dynamic_keys.contains(key)
        });
    }

    pub(super) fn runtime_character_body(
        &self,
        map_hash: u32,
        trigger_index: usize,
    ) -> Option<&RuntimeCharacterBodyState> {
        self.runtime_character_bodies
            .get(&Self::runtime_character_body_key(map_hash, trigger_index))
    }

    pub(super) fn runtime_ai_character_xitem_live(
        &self,
        map_hash: u32,
        trigger_index: usize,
    ) -> bool {
        let key = Self::runtime_character_body_key(map_hash, trigger_index);
        runtime_ai_character_live_policy(
            self.native_script_trigger_lifecycle_valid,
            self.simulate_ai,
            self.native_ai_trigger_lifecycle
                .get(&key)
                .is_some_and(|state| state.xitem_exists),
            self.native_ai_preview_live.contains(&key),
        )
    }

    pub(super) fn runtime_ai_character_live_pose(
        &self,
        map_hash: u32,
        trigger_index: usize,
    ) -> Option<(Vec3, Quat)> {
        if !self.runtime_ai_character_xitem_live(map_hash, trigger_index) {
            return None;
        }
        let body = self
            .runtime_character_bodies
            .get(&Self::runtime_character_body_key(map_hash, trigger_index))?;
        Some((body.owner_position, body.owner_rotation))
    }

    /// Current live NPC MapCollision shape for physics/world-contact consumers.
    /// Common hit queries use the separate HT_AnimDatum_HitArea profile below;
    /// the concrete NPC TriggerManager lifecycle still owns XItem existence.
    pub(super) fn runtime_live_npc_collision_shape(
        &self,
        map_hash: u32,
        trigger_index: usize,
    ) -> Option<RuntimeCharacterWorldShape> {
        let key = Self::runtime_character_body_key(map_hash, trigger_index);
        let xitem_exists = self
            .native_ai_trigger_lifecycle
            .get(&key)
            .is_some_and(|state| state.xitem_exists);
        runtime_live_character_collision_shape(
            self.runtime_character_bodies.get(&key),
            xitem_exists,
        )
    }

    /// Test one currently-live AI body against an already-resolved native query
    /// shape. Creator provenance is intentionally absent here: candidate geometry,
    /// query serial ownership and Handler reaction identity are XItem/body concerns.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn test_runtime_ai_hit_candidate_by_key(
        &mut self,
        key: u64,
        context: RobotsHitQueryCandidateContext,
        query_shape: RuntimeCharacterWorldShape,
        is_source: bool,
        is_secondary_source: bool,
        coarse_bounds_miss: bool,
    ) -> RuntimeCharacterHitCandidateResult {
        runtime_test_live_character_hit_candidate(
            self.runtime_character_bodies.get_mut(&key),
            true,
            context,
            RuntimeCharacterHitQueryGeometry::PreparedShape(query_shape),
            is_source,
            is_secondary_source,
            coarse_bounds_miss,
        )
    }

    /// Native flag 0x200 path: the query owner supplies its sample list/source
    /// position while this body remains responsible only for candidate HitArea.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn test_runtime_live_npc_hit_sample_sweep_candidate(
        &mut self,
        map_hash: u32,
        trigger_index: usize,
        context: RobotsHitQueryCandidateContext,
        plan: RobotsHitSampleSweepPlan,
        source_xyzw: Option<[f32; 4]>,
        samples: &mut [RobotsHitSweepSample],
        is_source: bool,
        is_secondary_source: bool,
        coarse_bounds_miss: bool,
    ) -> RuntimeCharacterHitCandidateResult {
        let key = Self::runtime_character_body_key(map_hash, trigger_index);
        let xitem_exists = self
            .native_ai_trigger_lifecycle
            .get(&key)
            .is_some_and(|state| state.xitem_exists);
        runtime_test_live_character_hit_candidate(
            self.runtime_character_bodies.get_mut(&key),
            xitem_exists,
            context,
            RuntimeCharacterHitQueryGeometry::SampleSweep {
                plan,
                source_xyzw,
                samples,
            },
            is_source,
            is_secondary_source,
            coarse_bounds_miss,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_ai_preview_live_policy_is_independent_from_native_lifecycle() {
        assert!(runtime_ai_character_live_policy(false, true, false, true));
        assert!(!runtime_ai_character_live_policy(false, true, true, false));
        assert!(runtime_ai_character_live_policy(true, true, true, false));
        assert!(!runtime_ai_character_live_policy(true, true, false, true));
        assert!(runtime_ai_character_live_policy(false, false, true, false));
    }

    #[test]
    fn runtime_ai_instance_key_keeps_serialized_and_dynamic_namespaces_distinct() {
        let map_hash = 0x1234_5678;
        let serialized = RuntimeAiInstanceKey::serialized(map_hash, 7)
            .expect("serialized key")
            .raw();
        let dynamic = RuntimeAiInstanceKey::dynamic(map_hash, 7)
            .expect("dynamic key")
            .raw();

        assert_ne!(serialized, dynamic);
        assert!(!RuntimeAiInstanceKey::raw_is_dynamic_for_map(
            serialized, map_hash
        ));
        assert!(RuntimeAiInstanceKey::raw_is_dynamic_for_map(
            dynamic, map_hash
        ));
        assert_eq!(
            RuntimeAiInstanceKey::dynamic_live_id_for_map(dynamic, map_hash),
            Some(7)
        );
        assert_eq!(
            RuntimeAiInstanceKey::dynamic_live_id_for_map(dynamic, 0x8765_4321),
            None
        );
    }
}
