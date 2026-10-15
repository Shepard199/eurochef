//! Inactive sub-map state. A suspended sub-map does not tick. All owned
//! AI/Trigger/XItem maps are **moved**, never cloned, out of the active MapFrame.
//! Engine-wide RNG, frame counters, hit serial and Player persistent state are
//! deliberately NOT captured here.
//!
//! The explicit field list is one source of truth for take/restore. Future
//! native owners must be classified as sub-map-local or process-global before
//! being added. Gameplay does not use editor "Preview ALL NPCs".
use super::*;

macro_rules! suspended_submap_fields {
    ($($field:ident: $kind:ty,)*) => {
        #[derive(Default)]
        pub(super) struct NativeSuspendedSubmapRuntime {
            $($field: $kind,)*
            parked_at_wall_time: f64,
        }

        impl NativeSuspendedSubmapRuntime {
            pub(super) fn park(frame: &mut MapFrame, wall_time: f64) -> Self {
                Self {
                    $($field: std::mem::take(&mut frame.$field),)*
                    parked_at_wall_time: wall_time,
                }
            }

            pub(super) fn resume(mut self, frame: &mut MapFrame, wall_time: f64, map_uid: u32) {
                self.rebase_local_clocks(wall_time);
                $(frame.$field = self.$field;)*
                frame.native_zone_runtime_map = Some(map_uid);
                // A dormant map must not consume the elapsed time of another
                // sub-map as one enormous fixed-step update.
                frame.native_ai_fixed_last_time = Some(wall_time);
                frame.native_camera_fixed_last_time = Some(wall_time);
            }
        }
    };
}

suspended_submap_fields! {
    native_zone_runtime: Vec<NativeMapZoneRuntimeState>,
    native_sky_cache: Vec<Option<NativeSkyCacheEntry>>,
    runtime_event_states: FxHashMap<u64, RuntimeEventPreviewState>,
    runtime_character_bodies: FxHashMap<u64, RuntimeCharacterBodyState>,
    native_script_trigger_lifecycle: FxHashMap<u64, RuntimeScriptTriggerLifecycleState>,
    native_ai_trigger_lifecycle: FxHashMap<u64, RuntimeScriptTriggerLifecycleState>,
    native_ai_creator_runtime: FxHashMap<u64, runtime_ai::NativeSerializedAiCreatorRuntime>,
    native_ai_deferred_destroy: runtime_ai::NativeSerializedAiDeferredDestroyRuntime,
    native_npc_behavior: FxHashMap<u64, NativeNpcBehaviorRuntime>,
    native_eq04_mine_runtime: FxHashMap<u64, runtime_ai::NativeEq04MineRuntime>,
    native_minebot_runtime: FxHashMap<u64, runtime_ai::NativeMineBotRuntime>,
    native_dogbot_runtime: FxHashMap<u64, runtime_ai::NativeDogBotRuntime>,
    native_turret_runtime: FxHashMap<u64, runtime_ai::NativeTurretRuntime>,
    native_ep05_turret_runtime: FxHashMap<u64, runtime_ai::NativeEp05TurretRuntime>,
    native_ep06_turret_runtime: FxHashMap<u64, runtime_ai::NativeEp06TurretRuntime>,
    native_turretbot_runtime: FxHashMap<u64, runtime_ai::NativeTurretBotRuntime>,
    native_standard_monster_runtime: FxHashMap<u64, runtime_ai::NativeStandardMonsterRuntime>,
    native_base_monster_runtime: FxHashMap<u64, runtime_ai::NativeBaseMonsterRuntime>,
    native_em07_piranha_runtime: FxHashMap<u64, runtime_ai::NativeEm07PiranhaRuntime>,
    native_test_anim_bot_runtime: FxHashMap<u64, runtime_ai::NativeTestAnimBotRuntime>,
    native_dodgem_runtime: FxHashMap<u64, runtime_ai::NativeDodgemRuntime>,
    native_spintop_runtime: FxHashMap<u64, runtime_ai::NativeSpinTopRuntime>,
    native_rollerbot_runtime: FxHashMap<u64, runtime_ai::NativeRollerBotRuntime>,
    native_monster_navigation: FxHashMap<u64, RobotsMonsterNavigationRuntimeState>,
    native_monster_physics: FxHashMap<u64, RobotsCharacterPhysicsRuntimeState>,
    native_ai_fatal_runtime: FxHashMap<u64, runtime_ai::NativeCommonAiFatalRuntime>,
    native_ai_hit_reactions: FxHashMap<u64, RobotsAiHitReactionState>,
    native_ai_last_hit_source_yaw: FxHashMap<u64, f32>,
    native_ai_last_hit_source_position: FxHashMap<u64, Vec3>,
    native_current_attacker_watchbot_gates: FxHashMap<u64, RobotsCurrentAttackerWatchBotGateState>,
    native_ai_transient_sounds: Vec<runtime_ai::NativeAiTransientSoundRequest>,
    native_script_xitem_spawns: Vec<runtime_script_xitem::NativeScriptXItemSpawnRequest>,
    native_transient_script_xitems: Vec<runtime_script_xitem::NativeTransientScriptXItemRuntime>,
    native_ai_projectiles: Vec<runtime_ai_projectile::NativeAiProjectileRuntime>,
    native_ai_explosion_spawns: Vec<runtime_ai_explosion::NativeAiExplosionSpawnRequest>,
    native_ai_explosion_main_spawns: Vec<runtime_ai_explosion::NativeAiExplosionMainSpawnRequest>,
    native_ai_explosions: Vec<runtime_ai_explosion::NativeAiExplosionRuntime>,
    native_ai_explosion_fragments: Vec<runtime_ai_explosion::NativeAiExplosionFragmentRuntime>,
    native_dynamic_pickup_spawns: Vec<runtime_pickup::NativeDynamicPickupSpawnRequest>,
    native_dynamic_pickups: Vec<runtime_pickup::NativeDynamicPickupRuntime>,
    native_ai_projectile_terminal_events: Vec<runtime_ai_projectile::NativeAiProjectileTerminalEvent>,
    native_lightweight_trigger_lifecycle: FxHashMap<u64, RuntimeScriptTriggerLifecycleState>,
    native_fluid_trigger_lifecycle: FxHashMap<u64, RuntimeScriptTriggerLifecycleState>,
    native_pickup_trigger_lifecycle: FxHashMap<u64, RuntimeScriptTriggerLifecycleState>,
    native_pickup_xitems: FxHashMap<u64, NativePickupXItemRuntime>,
    native_camera_bit0_trigger_lifecycle: FxHashMap<u64, RuntimeCameraBit0TriggerState>,
    native_common_trigger_events: FxHashMap<u64, NativeCommonTriggerEventState>,
    native_trigger_graph: FxHashMap<u64, RuntimeTriggerGraphState>,
    native_monster_transporter_lifecycle: FxHashMap<u64, RuntimeScriptTriggerLifecycleState>,
    native_monster_transporters: FxHashMap<u64, NativeMonsterTransporterRuntime>,
    native_boss_ratchet_lifecycle: FxHashMap<u64, RuntimeScriptTriggerLifecycleState>,
    native_boss_ratchets: FxHashMap<u64, NativeBossRatchetHostRuntime>,
    native_sweeper_boss_controller_lifecycle: FxHashMap<u64, RuntimeScriptTriggerLifecycleState>,
    native_sweeper_boss_runtime_map: Option<u32>,
    native_sweeper_boss_bindings: Option<NativeSweeperBossMapBindings>,
    native_sweeper_boss_runtime: Option<NativeSweeperBossReplayRuntime>,
    native_cutscene_runtime: FxHashMap<u64, NativeCutsceneRuntimeState>,
    native_cutscene_host_runtime: RobotsCutsceneHostRuntimeState,
    native_pickup_global_scheduler: RobotsPickupGlobalSchedulerState,
    native_shop_lifecycle: RobotsShopLifecycleState,
    native_ai_fixed_accumulator: f64,
    // Fractional 60Hz TriggerManager phase belongs to this sub-map, not B.
    native_camera_fixed_accumulator: f64,
    runtime_motion_start_time: Option<f64>,
    script_animation_start_time: Option<f64>,
}

impl NativeSuspendedSubmapRuntime {
    /// Local simulation pauses while the owning sub-map is absent. Shift only
    /// the known wall-time anchors; accumulated route progress stays intact.
    /// Class-owned absolute timestamps need separate native verification.
    fn rebase_local_clocks(&mut self, wall_time: f64) {
        let delta = (wall_time - self.parked_at_wall_time).max(0.0);
        if !delta.is_finite() {
            return;
        }
        for state in self.runtime_event_states.values_mut() {
            state.last_wall_time = state.last_wall_time.map(|last| last + delta);
        }
        if let Some(start) = &mut self.runtime_motion_start_time {
            *start += delta;
        }
        if let Some(start) = &mut self.script_animation_start_time {
            *start += delta;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parked_submaps_keep_independent_trigger_and_body_ownership() {
        let mut cache = FxHashMap::<u32, NativeSuspendedSubmapRuntime>::default();
        let key_a = MapFrame::runtime_event_key(0x1100_0001, 4);
        let key_b = MapFrame::runtime_event_key(0x1100_0002, 4);
        let mut a = NativeSuspendedSubmapRuntime::default();
        let mut event_a = RuntimeEventPreviewState::default();
        event_a.active = true;
        event_a.elapsed_seconds = 12.5;
        event_a.last_wall_time = Some(20.0);
        a.runtime_event_states.insert(key_a, event_a);
        a.native_ai_trigger_lifecycle.insert(key_a, RuntimeScriptTriggerLifecycleState {
            xitem_exists: true, ..Default::default()
        });
        a.parked_at_wall_time = 20.0;
        a.runtime_motion_start_time = Some(5.0);
        a.native_ai_fixed_accumulator = 0.5 / 60.0;
        a.native_camera_fixed_accumulator = 0.5 / 60.0;
        let mut b = NativeSuspendedSubmapRuntime::default();
        let mut event_b = RuntimeEventPreviewState::default();
        event_b.active = true;
        event_b.elapsed_seconds = 3.0;
        event_b.last_wall_time = Some(70.0);
        b.runtime_event_states.insert(key_b, event_b);
        b.parked_at_wall_time = 70.0;
        assert!(cache.insert(0x1100_0001, a).is_none());
        assert!(cache.insert(0x1100_0002, b).is_none());

        let mut a = cache.remove(&0x1100_0001).unwrap();
        a.rebase_local_clocks(120.0);
        assert_eq!(a.runtime_event_states[&key_a].elapsed_seconds, 12.5);
        assert_eq!(a.runtime_event_states[&key_a].last_wall_time, Some(120.0));
        assert_eq!(a.runtime_motion_start_time, Some(105.0));
        assert_eq!(a.native_ai_fixed_accumulator, 0.5 / 60.0);
        assert_eq!(a.native_camera_fixed_accumulator, 0.5 / 60.0,
            "returning to A must retain its partial TriggerManager 60 Hz tick");
        assert_eq!(cache[&0x1100_0002].native_camera_fixed_accumulator, 0.0,
            "B must not borrow A's fixed-step phase");
        assert!(!a.runtime_event_states.contains_key(&key_b));
        assert!(a.native_ai_trigger_lifecycle[&key_a].xitem_exists,
            "previously spawned AI XItem must remain alive on returning to A");
        assert_eq!(cache[&0x1100_0002].runtime_event_states[&key_b].elapsed_seconds, 3.0);
        assert!(!cache[&0x1100_0002].native_ai_trigger_lifecycle.contains_key(&key_a),
            "A's AI XItem must not leak into B");
        assert!(!cache.contains_key(&0x1100_0001));
    }

    #[test]
    fn native_message_queue_keeps_level_global_deadline_across_submap_sleep() {
        use eurochef_shared::robots_runtime::message_presentation::{
            RobotsMessageFinishTiming, RobotsMessageQueueRecord,
        };
        // DAT_007B2C44 belongs to GameWnd, not any parked sub-map. Its
        // 60Hz finish deadline continues across a same-level handoff.
        let mut global = NativeMessagePresentationState::default();
        let message = RobotsMessageQueueRecord {
            message_uid: 0x1700_0001,
            finish_timing: RobotsMessageFinishTiming::AbsoluteFrame(600.0),
            parameter_08: 0,
            parameter_0c: -1,
            parameter_10: -1,
            parameter_14: -1,
            ui_flags: 0,
            sound_uid: u32::MAX,
        };
        global.active = Some(message);
        global.enqueue_requests = 1;
        let mut parked_a = NativeSuspendedSubmapRuntime::default();
        parked_a.parked_at_wall_time = 3.0;
        parked_a.rebase_local_clocks(100.0);
        assert!(!global.active_is_timed_out(599.0));
        assert!(global.active_is_timed_out(600.0));
        // Same-level reset temporarily moves the one global queue out and
        // back. A's paused local timeline cannot clone it or shift deadline.
        let carried = std::mem::take(&mut global);
        assert!(global.active.is_none());
        global = carried;
        assert_eq!(global.active, Some(message));
        assert_eq!(global.enqueue_requests, 1);
        assert!(global.active_is_timed_out(600.0));
    }

    #[test]
    fn suspended_submap_rejects_nonfinite_clock_adjustment() {
        let mut state = NativeSuspendedSubmapRuntime::default();
        state.parked_at_wall_time = 30.0;
        state.runtime_motion_start_time = Some(5.0);
        state.rebase_local_clocks(f64::INFINITY);
        assert_eq!(state.runtime_motion_start_time, Some(5.0));
    }
}
