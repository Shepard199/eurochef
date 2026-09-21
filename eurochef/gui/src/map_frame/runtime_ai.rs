use super::{runtime_ai_explosion::native_common_monster_explosion_uid, MapFrame};
use crate::{
    map_runtime::{
        runtime_character_animation_datum_world_shape,
        runtime_character_animation_datum_world_transform, runtime_character_floor_contact_projection,
        runtime_map_ai_environment_floor_contact, runtime_map_ai_environment_floor_y,
        runtime_world_shape_for_collision_profile, RuntimeAiEnvironmentFloorContact,
        RuntimeCharacterBodyState, RuntimeCharacterWorldShape, RuntimeNativeAiHitFatalState,
        RuntimeRobotsGlobalRngState,
    },
    maps::{
        robots_character_runtime_type, NativeSweeperBossGenericAiBootstrap,
        ProcessedCharacterVisual, ProcessedMap, ProcessedTrigger,
        ROBOTS_ANIM_DATUM_SOLID_COLLISION, ROBOTS_ANIM_MODE_DEFAULT,
        ROBOTS_SWEEPER_MONSTER_RUNTIME_TYPE,
    },
};
use eurochef_shared::robots_runtime::{
    ai_character::{
        ai_patrol_priority, robots_common_monster_attack_allowed,
        robots_player_reaction_gate_active, step_ai_patrol, RobotsAiHandlerClass,
        RobotsAiPatrolRuntimeState,
    },
    ai_event_throttle::ROBOTS_AI_EVENT_THROTTLE_STALK_ID,
    ai_fall::{
        ai_fall_priority, leave_ai_fall, RobotsAiFallConfig, RobotsAiFallGateInput,
        RobotsAiFallRuntimeState,
    },
    ai_pursue::{ai_pursue_priority, step_ai_pursue},
    base_monster::{
        uses_base_monster_idle_only, RobotsBaseMonsterIdleRuntimeState,
        ROBOTS_BASE_MONSTER_IDLE_ANIM_MODE,
    },
    bounce_navmesh::{
        RobotsBounceNavMeshPhase, RobotsBounceNavMeshRuntimeState,
        ROBOTS_BOUNCE_NAVMESH_IMPACT_ANIM_MODE,
    },
    character_effects::{
        apply_ai_character_attachment_event, RobotsCharacterAttachmentRuntimeState,
    },
    character_physics::RobotsCharacterPhysicsRuntimeState,
    current_attacker::{
        robots_current_attacker_candidate_eligible, robots_current_attacker_claim_active,
        robots_current_attacker_player_environment_blocked,
        RobotsCurrentAttackerCandidate, RobotsCurrentAttackerEligibilityInput,
        ROBOTS_CURRENT_ATTACKER_WATCHBOT_DATUM_UID,
    },
    circle_target::{
        circle_target_priority, enter_circle_target, step_circle_target,
        RobotsCircleTargetRuntimeState, ROBOTS_CIRCLE_TARGET_DIRECTION_RANDOM,
    },
    dodgem::{
        dodgem_attack_config, dodgem_primary_winner, dodgem_secondary_scrambled_config,
        dodgem_secondary_winner, RobotsDodgemBounceNavMeshRuntimeState, RobotsDodgemPrimaryWinner,
        RobotsDodgemSecondaryWinner, ROBOTS_DODGEM_BOUNCE_ANIM_MODE,
        ROBOTS_DODGEM_BOUNCE_PROBE_RADIUS, ROBOTS_DODGEM_IDLE_ANIM_MODE,
        ROBOTS_DODGEM_MAX_MOVE_SPEED, ROBOTS_DODGEM_MIN_MOVE_SPEED,
    },
    dogbot::{
        dogbot_attack_gate, dogbot_attack_setup_idle, step_dogbot_attack, RobotsDogBotAttackInput,
        RobotsDogBotAttackRuntimeState, ROBOTS_DOGBOT_ATTACK_ANIM_MODE,
    },
    ef01_mine::{
        ef01_mine_behavior_winner, ef01_mine_first_update_effects, ef01_mine_first_update_route,
        ef01_mine_peer_contact_requests_monster_action, RobotsEf01MineFirstUpdateRoute,
    },
    blades_attachment::{step_blades_attachment_rotation, RobotsBladesAttachmentSpec},
    eq03_spider::{step_eq03_spider_attachment_rotation, RobotsEq03SpiderAttachmentRuntimeState},
    em07_piranha::{
        em07_piranha_attack_config, em07_piranha_behavior_winner,
        em07_piranha_cached_target_from_draws, em07_piranha_creator_retarget_ticks,
        em07_piranha_creator_spawn_position_from_draws, em07_piranha_creator_target_radius,
        em07_piranha_distance_radius_from_raw, em07_piranha_flight_reset_ready,
        em07_piranha_height_transition,
        em07_piranha_random_radius_modulus,
        em07_piranha_attachment_rotation_per_fixed_tick, em07_piranha_height_transition_plan,
        em07_piranha_owner_pitch_yaw_from_physics,
        RobotsEm07PiranhaBehaviorWinner,
        RobotsEm07PiranhaCachedPoint, RobotsEm07PiranhaFlightCycleState,
        RobotsEm07PiranhaHeightTransition, RobotsEm07PiranhaHeightTransitionPlan,
        ROBOTS_EM07_PIRANHA_FILE_UID, ROBOTS_EM07_PIRANHA_FIRST_UPDATE_COMPONENT_SCALAR,
        ROBOTS_EM07_PIRANHA_HEIGHT_OFFSET, ROBOTS_EM07_PIRANHA_LINK_END_INDEX,
        ROBOTS_EM07_PIRANHA_LINK_FIRST_INDEX,
        ROBOTS_EM07_PIRANHA_PERMANENT_SOUND_PARAMETER, ROBOTS_EM07_PIRANHA_PERMANENT_SOUND_UID,
    },
    eq04_mine::{
        eq04_mine_behavior_winner, eq04_mine_patrol_config,
        eq04_mine_peer_contact_self_destruct_ready, eq04_mine_pursue_config,
        eq04_mine_self_destruct_ready, RobotsEq04MineBehaviorWinner,
        ROBOTS_EQ04_MINE_EXPLOSION_UID, ROBOTS_EQ04_MINE_MOVE_SPEED,
    },
    events::event_type,
    ew09_armoured::{
        ew09_attack_character_physics_velocity, ew09_effective_handler_flags,
        ew09_follow_network_path_config, step_ew09_attachment_rotation,
    },
    flee_navmesh::{
        enter_flee_navmesh, flee_navmesh_enter_needs_rng, flee_navmesh_priority,
        flee_navmesh_step_needs_rng, step_flee_navmesh, RobotsFleeNavMeshAction,
        RobotsFleeNavMeshRuntimeState, ROBOTS_FLEE_NAVMESH_ROUTE_SAMPLE_COUNT,
    },
    follow_flying_path::{
        follow_flying_path_priority, RobotsFollowFlyingPathConfig,
        RobotsFollowFlyingPathRuntimeState,
    },
    follow_network_path::{
        follow_network_path_priority, RobotsFollowNetworkPathNode,
        RobotsFollowNetworkPathRuntimeState,
    },
    generic_attack::{
        enter_generic_attack, enter_generic_attack_group, generic_attack_group_priority,
        generic_attack_priority, generic_attack_setup_idle, leave_generic_attack,
        leave_generic_attack_group, step_generic_attack, tick_generic_attack,
        RobotsGenericAttackGateInput, RobotsGenericAttackGroupRuntimeState,
        RobotsGenericAttackRuntimeState,
    },
    headtrack_attack::{
        enter_headtrack_attack, headtrack_attack_priority, headtrack_attack_setup_idle,
        leave_headtrack_attack, step_headtrack_attack, tick_headtrack_attack,
        RobotsHeadtrackAttackGateInput, RobotsHeadtrackAttackRuntimeState,
        RobotsHeadtrackAttackStepInput,
    },
    hit_candidate_policy::{RobotsHitQueryCandidateContext, ROBOTS_HIT_QUERY_RAW_GROUP1},
    hit_query::{RobotsHitQueryInitPlan, RobotsHitQueryState, RobotsHitQueryStepKind},
    hit_reaction::{
        robots_common_ai_hit_direction_selection, robots_monster_explosion_action_modulus,
        RobotsAiHitReactionState, RobotsCommonAiHitConfig, RobotsCommonAiHitRuntimeState,
        ROBOTS_AI_HIT_YAW_EPSILON,
    },
    hit_shapes::robots_hit_shapes_intersect,
    knightbot_animator::{
        step_knightbot_animator, RobotsKnightBotAnimatorConfig,
        RobotsKnightBotAnimatorRuntimeState,
    },
    locomotion::{
        ai_character_physics_velocity_from_scalar, shortest_yaw_delta,
        step_ai_locomotion_after_steering_prepass, step_ai_movement_error, RobotsAiLocomotionInput,
        RobotsAiLocomotionRuntimeState, RobotsAiLocomotionStep, RobotsAiMovementErrorRuntimeState,
        RobotsAiTurnRateInput, RobotsLocomotionState,
        ROBOTS_AI_DEFAULT_TURN_RATE_RADIANS_PER_SECOND, ROBOTS_AI_TURN_IN_PLACE_ENABLE_FLAG,
        ROBOTS_ANIM_MODE_MOVE, ROBOTS_ANIM_MODE_TURN_ON_SPOT, ROBOTS_ANIM_MODE_TURN_ON_SPOT_L,
        ROBOTS_ANIM_MODE_TURN_ON_SPOT_R, ROBOTS_FIXED_STEP_SECONDS,
    },
    magnabot::{
        resolve_magnabot_base_turn, RobotsMagnaBotTurnAction, RobotsMagnaBotTurnRuntimeState,
    },
    malfbot::{
        RobotsMalfBotElectroHitRuntimeState, RobotsMalfBotMagneticEffectInput,
        RobotsMalfBotMagneticHitRuntimeState, RobotsMalfBotScrambledHitRuntimeState,
    },
    minebot::{
        enter_minebot_attack, minebot_attack_gate, minebot_attack_setup_idle,
        minebot_move_attack_winner, step_minebot_attack, step_minebot_move,
        RobotsMineBotAttackInput, RobotsMineBotAttackPhase, RobotsMineBotAttackRuntimeState,
        RobotsMineBotMoveAttackWinner, RobotsMineBotMoveInput, RobotsMineBotMoveRuntimeState,
        ROBOTS_MINEBOT_ATTACK_TRACKING_MAX_YAW_PER_TICK,
    },
    minion_attachment::{
        step_minion_attachment_rotation, RobotsMinionAttachmentConfig,
    },
    monster_navigation::{
        RobotsMonsterNavMeshView, RobotsMonsterNavigationRuntimeState,
        ROBOTS_MONSTER_NAV_BOUNDARY_PROBE_RADIUS,
    },
    npc_behavior::{
        complete_periodic_idle_setup_idle, enter_patrol_navmesh2, enter_periodic_idle,
        initialize_periodic_idle, leave_periodic_idle, patrol_navmesh2_enter_needs_rebuild,
        patrol_navmesh2_priority, patrol_navmesh2_rebuild_will_consume_rng,
        patrol_navmesh2_step_needs_rebuild, periodic_idle_priority, step_patrol_navmesh2,
        tick_patrol_navmesh2, tick_periodic_idle, RobotsPatrolNavMesh2Action,
        RobotsPatrolNavMesh2RuntimeState, RobotsPeriodicIdleRuntimeState,
        ROBOTS_ANIM_MODE_IDLE_ATTACK, ROBOTS_NPC_FLAG2_ROUTE_SAMPLE_COUNT,
    },
    process_rng::robots_process_lcg_modulo_zero_gate,
    projectile::RobotsCreateProjectileRequest,
    pursue_navmesh::{
        RobotsPursueNavMeshInput, RobotsPursueNavMeshRuntimeState,
        ROBOTS_DIRECT_MONSTER_FACTORY_ALLOW_CROSS_GROUP, ROBOTS_DOGBOT_PURSUE_NAV_STOP_DISTANCE,
        ROBOTS_PURSUE_NAV_PRIORITY,
    },
    rollerbot::{
        rollerbot_attack_config, rollerbot_behavior_winner, rollerbot_patrol_config,
        rollerbot_pursue_priority, rollerbot_target_yaw, step_rollerbot_inertia,
        step_rollerbot_locomotion, RobotsRollerBotBehaviorWinner,
        RobotsRollerBotFollowPathRuntimeState, RobotsRollerBotPathGraphView,
        ROBOTS_ROLLERBOT_PATH_UID,
    },
    scrambled_hit::{RobotsScrambledHitConfig, RobotsScrambledHitRuntimeState},
    script_host::native_script_ftol,
    shunt_attack::{
        apply_shunt_attack_event, enter_shunt_attack, leave_shunt_attack,
        shunt_attack_priority_configured, step_shunt_attack_configured,
        RobotsShuntAttackPriorityInput, RobotsShuntAttackRuntimeState, RobotsShuntAttackStepInput,
        ROBOTS_SHUNT_ATTACK_TURN_ANIM_MODE, ROBOTS_SHUNT_ATTACK_TURN_RATE_RADIANS_PER_SECOND,
    },
    spike_attack::{
        apply_spike_attack_event, enter_spike_attack, leave_spike_attack, spike_attack_priority,
        step_spike_attack, RobotsSpikeAttackPriorityInput, RobotsSpikeAttackRuntimeState,
        RobotsSpikeAttackStepInput,
    },
    spintop::{
        spintop_behavior_winner, spintop_patrol_config, RobotsSpinTopBehaviorWinner,
        ROBOTS_SPINTOP_BOUNCE_PROBE_RADIUS, ROBOTS_SPINTOP_MAX_MOVE_SPEED,
        ROBOTS_SPINTOP_MIN_MOVE_SPEED,
    },
    stalk_navmesh::{
        complete_stalk_navmesh_animation, enter_stalk_navmesh, leave_stalk_navmesh,
        stalk_navmesh_priority, stalk_navmesh_requests_event_throttle, step_stalk_navmesh,
        tick_stalk_navmesh,
        RobotsStalkNavMeshPriorityInput, RobotsStalkNavMeshRuntimeState,
    },
    standard_monster::{
        eb07_minebot_behavior_winner, eq03_spider_behavior_winner, ew09_armoured_behavior_winner,
        flambe_behavior_winner, guardbot_behavior_winner, knightbot_behavior_winner,
        magnabot_behavior_winner, shieldbot_behavior_winner, standard_monster_behavior_winner,
        standard_monster_physics_locomotion_speed_range, sweeper_behavior_winner,
        thiefbot_behavior_winner,
        RobotsPermanentSoundConfig, RobotsStandardMonsterBehaviorConfig,
        RobotsStandardMonsterBehaviorWinner, ROBOTS_STANDARD_MONSTER_MAX_ATTACKS,
    },
    target_behavior::{
        proximity_anim_priority, target_facing_turn_priority, target_facing_turn_request,
        RobotsProximityAnimBehaviorConfig, RobotsTargetFacingTurnBehaviorConfig,
    },
    test_anim_bot::{
        test_anim_attack_config, test_anim_behavior_winner, test_anim_common_hit_config,
        test_anim_effective_handler_flags, test_anim_patrol_config, test_anim_pursue_config,
        RobotsTestAnimBehaviorWinner, ROBOTS_TEST_ANIM_ATTACK_ANIM_MODES,
        ROBOTS_TEST_ANIM_PATROL_ANIM_MODE, ROBOTS_TEST_ANIM_PERIODIC_IDLE_ANIM_MODES,
        ROBOTS_TEST_ANIM_PERIODIC_IDLE_BASE_DELAY_TICKS,
    },
    thiefbot::{
        step_thiefbot_attachment, RobotsThiefBotAttachmentInput,
        RobotsThiefBotAttachmentRuntimeState,
    },
    three_phase_attack::{
        enter_three_phase_attack, leave_three_phase_attack, step_three_phase_attack,
        three_phase_attack_geometry_gate, three_phase_attack_priority,
        three_phase_attack_setup_idle, tick_three_phase_attack, RobotsThreePhaseAttackInput,
        RobotsThreePhaseAttackPhase, RobotsThreePhaseAttackRuntimeState,
    },
    turn_then_attack::{
        enter_turn_then_attack, leave_turn_then_attack, step_turn_then_attack,
        tick_turn_then_attack, turn_then_attack_priority, turn_then_attack_setup_idle,
        RobotsTurnThenAttackGateInput, RobotsTurnThenAttackRuntimeState,
        RobotsTurnThenAttackStepInput,
    },
    turret::{
        enter_pitched_turret_attack, enter_pitched_turret_attack_configured, ep02_primary_winner,
        ep02_secondary_winner, ep04_attack_config, ep04_primary_winner, ep05_activation_winner,
        ep05_attack_config, ep05_combat_winner, ep05_distance_deactivate_priority,
        ep05_exit_priority, ep06_host_4c4_winner, ep06_host_4dc_winner,
        ep06_primary_pitched_attack_config, ep06_secondary_pitched_attack_config,
        leave_pitched_turret_attack, leave_pitched_turret_attack_configured,
        pitched_turret_attack_priority, pitched_turret_attack_priority_configured,
        pitched_turret_attack_setup_idle, step_ep05_turret_yaw_toward, step_pitched_turret_attack,
        step_pitched_turret_attack_configured, step_turret_yaw_toward, tick_pitched_turret_attack,
        track_target_priority, RobotsEp02PrimaryWinner, RobotsEp02SecondaryWinner,
        RobotsEp04PrimaryWinner, RobotsEp05ActivationWinner, RobotsEp05CombatWinner,
        RobotsEp06Host4c4Winner, RobotsEp06Host4dcWinner, RobotsEp06MagneticAnimationRuntimeState,
        RobotsPitchedTurretAttackRuntimeState, RobotsTurretAimRuntimeState,
        ROBOTS_EP02_ATTACK_ANIM_MODE, ROBOTS_EP04_ATTACK_ANIM_MODE, ROBOTS_EP05_ATTACK_ANIM_MODE,
        ROBOTS_EP05_DEACTIVATE_EXIT2_ANIM_MODE, ROBOTS_EP05_DEACTIVATE_EXIT_ANIM_MODE,
        ROBOTS_EP05_DEACTIVATE_IDLE2_ANIM_MODE, ROBOTS_EP05_DEACTIVATE_IDLE_ANIM_MODE,
        ROBOTS_EP05_IDLE_ANIM_MODE, ROBOTS_EP06_PRIMARY_ATTACK_ANIM_MODE,
        ROBOTS_EP06_PRIMARY_IDLE_ANIM_MODE, ROBOTS_EP06_SECONDARY_ATTACK_ANIM_MODE,
        ROBOTS_EP06_SECONDARY_IDLE_ANIM_MODE, ROBOTS_TURRET_ANGLE_EPSILON,
    },
    turret_bot::{
        turretbot_behavior_winner, turretbot_headtrack_attack_config,
        turretbot_nonfatal_hit_config, turretbot_scrambled_hit_config,
        RobotsTurretBotBehaviorWinner, ROBOTS_TURRETBOT_PERIODIC_IDLE_ANIM_MODES,
        ROBOTS_TURRETBOT_PERIODIC_IDLE_BASE_DELAY_TICKS,
    },
};
use fxhash::{FxHashMap, FxHashSet};
use glam::{Mat4, Quat, Vec3};

mod em07_piranha;
pub(super) use em07_piranha::NativeEm07PiranhaRuntime;
mod spintop;
pub(super) use spintop::NativeSpinTopRuntime;
mod test_anim_bot;
pub(super) use test_anim_bot::NativeTestAnimBotRuntime;

use super::{
    runtime_ai_motion::{
        NativeAiAnimationEvent, NativeAiAnimationRuntime, NativeAiRootMotionPolicy,
    },
    runtime_ai_projectile::resolve_ai_projectile_spawn_plan,
};

const ROBOTS_AI_MAX_FIXED_STEPS_PER_FRAME: usize = 200_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeAiHostScope {
    Serialized,
    DynamicSweeper,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeAiProductionHostKind {
    BaseMonster,
    StandardMonster,
    DedicatedAi,
    NpcBehavior,
}

/// Exhaustive production ownership census for every recovered concrete AI handler.
/// Adding another enum variant must update this match before the GUI test target can compile.
#[cfg(test)]
pub(super) const fn native_ai_production_host_kind(
    handler_class: RobotsAiHandlerClass,
) -> NativeAiProductionHostKind {
    use NativeAiProductionHostKind::*;
    use RobotsAiHandlerClass::*;
    match handler_class {
        MonsterBase => BaseMonster,
        Monster2Rockets
        | ConstructionBot
        | Eb07MineBot
        | Eb11MagnaBot
        | Eb12EvilBot
        | Eb13KnightBot
        | Eb14Minion
        | Eb15Launcher
        | Eb16KnuckleBot
        | Ef01Mine
        | Ef03EvilBot
        | Eq03Spider
        | Ew08Flambe
        | Ew08FlambeLarge
        | Ew09Armoured
        | Ew10Minion
        | Ew11FatBot
        | GuardBot
        | JailBotLarge
        | JailBotNormal
        | MalfBot
        | SawBot
        | SecurityBot
        | ShieldBot
        | ShuntBot
        | ShuntBotBoss
        | SpikeBot
        | Sweeper
        | ThiefBot => StandardMonster,
        DogBot
        | Eb10RollerBot
        | Em07PiranhaBot
        | Ep02Turret
        | Ep04Turret
        | Ep05Turret
        | Ep06Turret
        | Eq02MineBot
        | Eq04Mine
        | Ew07Dodgem
        | SpinTop
        | TestAnimBot
        | TurretBot => DedicatedAi,
        Npc | NpcFender => NpcBehavior,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct NativeSerializedAiCreatorRuntime {
    /// Native XTrigger_AI_Character +0x104. Common natural death sets this to 1;
    /// CreateItem 0x0047E4F0 refuses to recreate while it remains set.
    pub(super) death_latched: bool,
    /// Creator +0x84 -> +0x70 callback copies the dying XItem's live position
    /// back into trigger +0x24..+0x30 before deferred destruction.
    pub(super) position_override: Option<Vec3>,
}

impl NativeSerializedAiCreatorRuntime {
    pub(super) fn allows_create_item(self) -> bool {
        !self.death_latched
    }

    pub(super) fn record_natural_death(&mut self, position: Vec3) {
        self.death_latched = true;
        self.position_override = Some(position);
    }
}

/// Native AI handler +0x12C is a deferred-destroy request, not an immediate
/// Trigger/XItem unlink. Keep this queue class-independent so other recovered AI
/// families can reuse the same end-of-fixed-step boundary.
#[derive(Default)]
pub(super) struct NativeSerializedAiDeferredDestroyRuntime {
    pending: FxHashSet<u64>,
}

impl NativeSerializedAiDeferredDestroyRuntime {
    pub(super) fn mark(&mut self, key: u64) {
        self.pending.insert(key);
    }

    pub(super) fn is_pending(&self, key: u64) -> bool {
        self.pending.contains(&key)
    }

    pub(super) fn take_all(&mut self) -> Vec<u64> {
        self.pending.drain().collect()
    }

    pub(super) fn clear(&mut self) {
        self.pending.clear();
    }
}

pub(super) struct NativeCommonAiFatalRuntime {
    anim_mode: u32,
    target_yaw_radians: f32,
    turn_rate_radians_per_second: f32,
    fatal: RuntimeNativeAiHitFatalState,
    animation: NativeAiAnimationRuntime,
}

pub(super) struct NativeEq04MineRuntime {
    patrol: RobotsAiPatrolRuntimeState,
    fall: RobotsAiFallRuntimeState,
    locomotion: RobotsAiLocomotionRuntimeState,
    active_node: Option<RobotsEq04MineBehaviorWinner>,
    animation: NativeAiAnimationRuntime,
}

impl Default for NativeEq04MineRuntime {
    fn default() -> Self {
        Self {
            patrol: RobotsAiPatrolRuntimeState::configured(eq04_mine_patrol_config()),
            fall: RobotsAiFallRuntimeState::default(),
            locomotion: RobotsAiLocomotionRuntimeState::default(),
            active_node: None,
            animation: NativeAiAnimationRuntime::default(),
        }
    }
}

#[derive(Default)]
pub(super) struct NativeMineBotRuntime {
    move_state: RobotsMineBotMoveRuntimeState,
    attack_state: RobotsMineBotAttackRuntimeState,
    animation: NativeAiAnimationRuntime,
}

#[derive(Default)]
pub(super) struct NativeDogBotRuntime {
    attack_state: RobotsDogBotAttackRuntimeState,
    pursue_navmesh: RobotsPursueNavMeshRuntimeState,
    animation: NativeAiAnimationRuntime,
    hit_query: Option<RobotsHitQueryState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeTurretPrimaryWinner {
    Ep02PitchedAttackGroup,
    Ep04AttackGroup,
    Ep04CommonHit,
    ScrambledHit,
}

pub(super) struct NativeTurretRuntime {
    attack_group: RobotsGenericAttackGroupRuntimeState,
    pitched_attack: RobotsPitchedTurretAttackRuntimeState,
    generic_attack: RobotsGenericAttackRuntimeState,
    common_hit: RobotsCommonAiHitRuntimeState,
    primary_scrambled: RobotsScrambledHitRuntimeState,
    secondary_scrambled: RobotsScrambledHitRuntimeState,
    aim: RobotsTurretAimRuntimeState,
    primary_active: Option<NativeTurretPrimaryWinner>,
    secondary_active: Option<RobotsEp02SecondaryWinner>,
    primary_animation: NativeAiAnimationRuntime,
    secondary_animation: NativeAiAnimationRuntime,
}

impl Default for NativeTurretRuntime {
    fn default() -> Self {
        Self {
            attack_group: RobotsGenericAttackGroupRuntimeState::default(),
            pitched_attack: RobotsPitchedTurretAttackRuntimeState::default(),
            generic_attack: RobotsGenericAttackRuntimeState::default(),
            common_hit: RobotsCommonAiHitRuntimeState::default(),
            primary_scrambled: RobotsScrambledHitRuntimeState::new(
                RobotsScrambledHitConfig::ep02_primary(),
            ),
            secondary_scrambled: RobotsScrambledHitRuntimeState::new(
                RobotsScrambledHitConfig::ep02_secondary(),
            ),
            aim: RobotsTurretAimRuntimeState::default(),
            primary_active: None,
            secondary_active: None,
            primary_animation: NativeAiAnimationRuntime::default(),
            secondary_animation: NativeAiAnimationRuntime::default(),
        }
    }
}

#[derive(Default)]
pub(super) struct NativeEp05TurretRuntime {
    attack: RobotsGenericAttackRuntimeState,
    aim: RobotsTurretAimRuntimeState,
    host_4c4_active: Option<RobotsEp05ActivationWinner>,
    host_4dc_active: Option<RobotsEp05CombatWinner>,
    host_4c4_animation: NativeAiAnimationRuntime,
    host_4dc_animation: NativeAiAnimationRuntime,
    /// EP05 +0x168/+0x16C motor sound state. Presentation remains an effect seam,
    /// but retaining the native state prevents later audio integration from
    /// inventing a second movement detector.
    yaw_sound_active: bool,
}

pub(super) struct NativeEp06TurretRuntime {
    host_4c4_pitched: RobotsPitchedTurretAttackRuntimeState,
    host_4dc_pitched: RobotsPitchedTurretAttackRuntimeState,
    host_4c4_hit: RobotsCommonAiHitRuntimeState,
    host_4dc_hit: RobotsCommonAiHitRuntimeState,
    host_4c4_magnetic: RobotsEp06MagneticAnimationRuntimeState,
    host_4dc_magnetic: RobotsMalfBotMagneticHitRuntimeState,
    host_4c4_scrambled: RobotsScrambledHitRuntimeState,
    host_4dc_scrambled: RobotsScrambledHitRuntimeState,
    aim: RobotsTurretAimRuntimeState,
    host_4c4_active: Option<RobotsEp06Host4c4Winner>,
    host_4dc_active: Option<RobotsEp06Host4dcWinner>,
    host_4c4_animation: NativeAiAnimationRuntime,
    host_4dc_animation: NativeAiAnimationRuntime,
    magnetic_drop_charge_count: Option<u8>,
}

impl Default for NativeEp06TurretRuntime {
    fn default() -> Self {
        Self {
            host_4c4_pitched: RobotsPitchedTurretAttackRuntimeState::default(),
            host_4dc_pitched: RobotsPitchedTurretAttackRuntimeState::default(),
            host_4c4_hit: RobotsCommonAiHitRuntimeState::default(),
            host_4dc_hit: RobotsCommonAiHitRuntimeState::default(),
            host_4c4_magnetic: RobotsEp06MagneticAnimationRuntimeState::default(),
            host_4dc_magnetic: RobotsMalfBotMagneticHitRuntimeState::default(),
            host_4c4_scrambled: RobotsScrambledHitRuntimeState::new(
                RobotsScrambledHitConfig::ep06_secondary(),
            ),
            host_4dc_scrambled: RobotsScrambledHitRuntimeState::new(
                RobotsScrambledHitConfig::ep06_primary(),
            ),
            aim: RobotsTurretAimRuntimeState::default(),
            host_4c4_active: None,
            host_4dc_active: None,
            host_4c4_animation: NativeAiAnimationRuntime::default(),
            host_4dc_animation: NativeAiAnimationRuntime::default(),
            magnetic_drop_charge_count: None,
        }
    }
}

pub(super) struct NativeTurretBotRuntime {
    periodic_idle: RobotsPeriodicIdleRuntimeState,
    attack_group: RobotsGenericAttackGroupRuntimeState,
    headtrack_attack: RobotsHeadtrackAttackRuntimeState,
    common_hit: RobotsCommonAiHitRuntimeState,
    magnetic_hit: RobotsMalfBotMagneticHitRuntimeState,
    scrambled_hit: RobotsScrambledHitRuntimeState,
    magnetic_drop_charge_count: Option<u8>,
    active_node: Option<RobotsTurretBotBehaviorWinner>,
    animation: NativeAiAnimationRuntime,
}

impl Default for NativeTurretBotRuntime {
    fn default() -> Self {
        Self {
            periodic_idle: RobotsPeriodicIdleRuntimeState::default(),
            attack_group: RobotsGenericAttackGroupRuntimeState::default(),
            headtrack_attack: RobotsHeadtrackAttackRuntimeState::default(),
            common_hit: RobotsCommonAiHitRuntimeState::default(),
            magnetic_hit: RobotsMalfBotMagneticHitRuntimeState::default(),
            scrambled_hit: RobotsScrambledHitRuntimeState::new(turretbot_scrambled_hit_config()),
            magnetic_drop_charge_count: None,
            active_node: None,
            animation: NativeAiAnimationRuntime::default(),
        }
    }
}

#[derive(Default)]
pub(super) struct NativeBaseMonsterRuntime {
    idle: RobotsBaseMonsterIdleRuntimeState,
    animation: NativeAiAnimationRuntime,
}

pub(super) struct NativeDodgemRuntime {
    bounce: RobotsDodgemBounceNavMeshRuntimeState,
    locomotion: RobotsAiLocomotionRuntimeState,
    primary_scrambled: RobotsScrambledHitRuntimeState,
    magnetic_hit: RobotsMalfBotMagneticHitRuntimeState,
    attack: RobotsGenericAttackRuntimeState,
    secondary_scrambled: RobotsScrambledHitRuntimeState,
    primary_active: Option<RobotsDodgemPrimaryWinner>,
    secondary_active: Option<RobotsDodgemSecondaryWinner>,
    primary_animation: NativeAiAnimationRuntime,
    secondary_animation: NativeAiAnimationRuntime,
    magnetic_drop_charge_count: Option<u8>,
    hit_query: Option<RobotsHitQueryState>,
}

impl NativeDodgemRuntime {
    fn new(setup_draw: u32) -> Self {
        Self {
            bounce: RobotsDodgemBounceNavMeshRuntimeState::from_setup_draw(setup_draw),
            locomotion: RobotsAiLocomotionRuntimeState::default(),
            primary_scrambled: RobotsScrambledHitRuntimeState::new(
                RobotsScrambledHitConfig::malfbot(),
            ),
            magnetic_hit: RobotsMalfBotMagneticHitRuntimeState::default(),
            attack: RobotsGenericAttackRuntimeState::default(),
            secondary_scrambled: RobotsScrambledHitRuntimeState::new(
                dodgem_secondary_scrambled_config(),
            ),
            primary_active: None,
            secondary_active: None,
            primary_animation: NativeAiAnimationRuntime::default(),
            secondary_animation: NativeAiAnimationRuntime::default(),
            magnetic_drop_charge_count: None,
            hit_query: None,
        }
    }
}

pub(super) struct NativeRollerBotRuntime {
    patrol: RobotsAiPatrolRuntimeState,
    attack: RobotsGenericAttackRuntimeState,
    hit: RobotsCommonAiHitRuntimeState,
    follow_path: RobotsRollerBotFollowPathRuntimeState,
    locomotion: RobotsLocomotionState,
    active_node: Option<RobotsRollerBotBehaviorWinner>,
    animation: NativeAiAnimationRuntime,
    hit_query: Option<RobotsHitQueryState>,
}

impl NativeRollerBotRuntime {
    fn new(setup_draw: u32, owner_position: Vec3, owner_yaw_radians: f32) -> Self {
        Self {
            patrol: RobotsAiPatrolRuntimeState::configured(rollerbot_patrol_config()),
            attack: RobotsGenericAttackRuntimeState::default(),
            hit: RobotsCommonAiHitRuntimeState::default(),
            follow_path: RobotsRollerBotFollowPathRuntimeState::from_setup_draw(setup_draw),
            locomotion: RobotsLocomotionState {
                position_xyz: owner_position.to_array(),
                yaw_radians: owner_yaw_radians,
                linear_velocity_xyz: [0.0; 3],
            },
            active_node: None,
            animation: NativeAiAnimationRuntime::default(),
            hit_query: None,
        }
    }

    pub(super) fn clear_retained_motion(&mut self) {
        self.locomotion.linear_velocity_xyz = [0.0; 3];
    }
}

#[derive(Clone, Copy)]
struct NativeStandardMonsterHitQueryRuntime {
    query: RobotsHitQueryState,
    source_anim_mode: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NativeAiPermanentSoundRegistration {
    pub(super) owner_key: u64,
    pub(super) sound_uid: u32,
    pub(super) native_parameter: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NativeAiTransientSoundRequest {
    pub(super) owner_key: u64,
    pub(super) sound_uid: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct NativeAiScriptSpawnRequest {
    pub(super) owner_key: u64,
    pub(super) file_uid: u32,
    pub(super) script_uid: u32,
    pub(super) position_xyzw: [f32; 4],
}

fn apply_native_ew09_derived_attack_motion(
    runtime: &mut NativeStandardMonsterRuntime,
    physics: &mut RobotsCharacterPhysicsRuntimeState,
    body: &mut RuntimeCharacterBodyState,
    owner_yaw_radians: f32,
) {
    let velocity = Vec3::from_array(ew09_attack_character_physics_velocity(owner_yaw_radians));
    runtime
        .animation
        .overwrite_character_physics_velocity(velocity);
    physics.overwrite_velocity(velocity.to_array());
    let mut position = body.owner_position.to_array();
    physics.integrate_position(&mut position, ROBOTS_FIXED_STEP_SECONDS);
    body.owner_position = Vec3::from_array(position);
}

fn service_native_eq03_spider_post_common_update(runtime: &mut NativeStandardMonsterRuntime) {
    runtime.eq03_attachment.local_spin_xyzw =
        step_eq03_spider_attachment_rotation(runtime.eq03_attachment.local_spin_xyzw);
}

fn service_native_blades_post_common_update(runtime: &mut NativeStandardMonsterRuntime) {
    runtime.blades_attachment_local_spin_xyzw =
        step_blades_attachment_rotation(runtime.blades_attachment_local_spin_xyzw);
}

fn service_native_thiefbot_post_common_update(
    runtime: &mut NativeStandardMonsterRuntime,
    owner_position: Vec3,
    owner_yaw_radians: f32,
    gameplay_target: Option<Vec3>,
) {
    let (target_available, target_distance_squared, target_yaw_error_radians) = gameplay_target
        .map(|target| {
            let delta = target - owner_position;
            let target_yaw = delta.x.atan2(delta.z);
            (
                true,
                delta.length_squared(),
                shortest_yaw_delta(owner_yaw_radians, target_yaw),
            )
        })
        .unwrap_or((false, f32::INFINITY, 0.0));
    step_thiefbot_attachment(
        &mut runtime.thiefbot_attachment,
        RobotsThiefBotAttachmentInput {
            target_available,
            target_distance_squared,
            target_yaw_error_radians,
        },
    );
}

fn service_native_ew09_post_common_update(runtime: &mut NativeStandardMonsterRuntime) {
    runtime.ew09_attachment_local_spin_xyzw =
        step_ew09_attachment_rotation(runtime.ew09_attachment_local_spin_xyzw);
}

fn service_native_minion_attachment_post_common_update(
    runtime: &mut NativeStandardMonsterRuntime,
    config: RobotsMinionAttachmentConfig,
) {
    runtime.minion_attachment_local_spin_xyzw = step_minion_attachment_rotation(
        runtime.minion_attachment_local_spin_xyzw,
        config,
    );
}

fn service_native_knightbot_pre_common_update(
    runtime: &mut NativeStandardMonsterRuntime,
    animator_position: Vec3,
    animator_rotation_point: Vec3,
) {
    let Some(animator) = runtime.knightbot_animator.as_mut() else {
        return;
    };
    step_knightbot_animator(
        animator,
        animator_position.to_array(),
        animator_rotation_point.to_array(),
    );
}

fn apply_native_ef01_first_update_effects(
    runtime: &mut NativeStandardMonsterRuntime,
    physics: &mut RobotsCharacterPhysicsRuntimeState,
    route: RobotsEf01MineFirstUpdateRoute,
) {
    let effects = ef01_mine_first_update_effects(route);
    runtime.ef01_flying_path_latch_644 = effects.flying_path_latch_644;
    if let Some(mode) = effects.handler_606_mode {
        physics.apply_handler_606_mode(mode);
    }
    physics.service_ef01_flying_path_bit2(effects.physics_object_flag_bit2);
}

fn register_native_ai_permanent_sound(
    registry: &mut FxHashMap<u32, NativeAiPermanentSoundRegistration>,
    owner_key: u64,
    sound: RobotsPermanentSoundConfig,
) -> NativeAiPermanentSoundRegistration {
    *registry
        .entry(sound.sound_uid)
        .or_insert(NativeAiPermanentSoundRegistration {
            owner_key,
            sound_uid: sound.sound_uid,
            native_parameter: sound.native_parameter,
        })
}

fn native_character_solid_collision_shape(
    body: &RuntimeCharacterBodyState,
    visual: &ProcessedCharacterVisual,
    animation: Option<&NativeAiAnimationRuntime>,
) -> Option<RuntimeCharacterWorldShape> {
    let (anim_mode, pose_seconds) = animation
        .and_then(|runtime| {
            let anim_mode = runtime.current_anim_mode();
            (anim_mode != 0).then_some((anim_mode, runtime.sampled_pose_seconds()))
        })
        .unwrap_or((ROBOTS_ANIM_MODE_DEFAULT, body.initial_animation_seconds));
    runtime_character_animation_datum_world_shape(
        body.owner_position,
        body.owner_rotation,
        body.native_transform_scale_xyz(),
        visual,
        anim_mode,
        ROBOTS_ANIM_DATUM_SOLID_COLLISION,
        pose_seconds,
    )
}

fn native_player_solid_collision_shape(
    map: &ProcessedMap,
    player_position: Vec3,
    player_yaw_radians: f32,
) -> Option<RuntimeCharacterWorldShape> {
    let profile = map.player_solid_collision.as_ref()?;
    // Shipped Rodney proves selector0/root. Until a full Player skeletal pose host
    // exists, refuse a future non-root profile rather than silently treating it as root.
    if profile.transform_selector != 0 {
        return None;
    }
    Some(runtime_world_shape_for_collision_profile(
        player_position,
        Quat::from_rotation_y(player_yaw_radians),
        Vec3::ONE,
        profile,
        Mat4::IDENTITY,
    ))
}

fn native_special_ai_solid_contact_requests_death(
    handler_class: RobotsAiHandlerClass,
    owner_pending_destroy: bool,
    owner_shape: RuntimeCharacterWorldShape,
    player_shape: RuntimeCharacterWorldShape,
) -> bool {
    if !robots_hit_shapes_intersect(owner_shape, player_shape) {
        return false;
    }
    match handler_class {
        RobotsAiHandlerClass::Ef01Mine => {
            ef01_mine_peer_contact_requests_monster_action(Some(0), owner_pending_destroy)
        }
        RobotsAiHandlerClass::Eq04Mine => {
            eq04_mine_peer_contact_self_destruct_ready(Some(0), owner_pending_destroy)
        }
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct NativeMonsterNavConstraintHostStep {
    pre_constraint_owner_position: Vec3,
    correction_xz: [f32; 2],
}

pub(super) struct NativeStandardMonsterRuntime {
    handler_class: RobotsAiHandlerClass,
    magnabot_turn: RobotsMagnaBotTurnRuntimeState,
    patrol: RobotsAiPatrolRuntimeState,
    periodic_idle: RobotsPeriodicIdleRuntimeState,
    patrol_navmesh2: RobotsPatrolNavMesh2RuntimeState,
    pursue_navmesh: RobotsPursueNavMeshRuntimeState,
    stalk_navmesh: RobotsStalkNavMeshRuntimeState,
    flee_navmesh: RobotsFleeNavMeshRuntimeState,
    follow_flying_path: RobotsFollowFlyingPathRuntimeState,
    /// EF01 Handler+0x644. First-update latches this only when a real 0x0B path
    /// installs FollowFlyingPath; native +0x170 suppresses ordinary route refresh while set.
    ef01_flying_path_latch_644: bool,
    follow_network_path: RobotsFollowNetworkPathRuntimeState,
    /// EQ03 Handler+0x640 child attachment local rotation. Native creates the
    /// attachment in +0x08 (`0x00464F20`) and services it in +0x34
    /// (`0x00465220`) after common Monster update.
    eq03_attachment: RobotsEq03SpiderAttachmentRuntimeState,
    /// EF01/EF03 Handler+0x640 `HT_Entity_Blades` local rotation. Creation/bone
    /// binding stays in the renderer/UE adapter; shared +0x34 `0x00467500` owns
    /// this exact local-Y spin state for both handlers.
    blades_attachment_local_spin_xyzw: [f32; 4],
    /// Relative EW09 Handler+0x640 attachment spin accumulated by native
    /// `0x00463190 -> 0x00454CA0`. The attachment's creation/bone transform is
    /// owned by the renderer/UE adapter, so this starts at identity and stores only
    /// the exact post-multiplied local-Y spin delta applied after common AI update.
    ew09_attachment_local_spin_xyzw: [f32; 4],
    /// EB14/EW10 Handler+0x640 ctor-created attachment spin serviced by their
    /// class +0x34 overrides after common Monster update.
    minion_attachment_local_spin_xyzw: [f32; 4],
    thiefbot_attachment: RobotsThiefBotAttachmentRuntimeState,
    /// EB13 builder-owned AttachAnimator entry at Handler+0x4FC[0]. The GUI/UE
    /// adapter materializes the child object; shared state owns its exact pose.
    knightbot_animator: Option<RobotsKnightBotAnimatorRuntimeState>,
    locomotion: RobotsAiLocomotionRuntimeState,
    movement_error: RobotsAiMovementErrorRuntimeState,
    attack_group: RobotsGenericAttackGroupRuntimeState,
    attacks: [RobotsGenericAttackRuntimeState; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
    attack_group_three_phase: RobotsThreePhaseAttackRuntimeState,
    turn_then_attacks: [RobotsTurnThenAttackRuntimeState; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
    direct_turn_then_attack: RobotsTurnThenAttackRuntimeState,
    circle_target: RobotsCircleTargetRuntimeState,
    secondary_three_phase_attack: RobotsThreePhaseAttackRuntimeState,
    secondary_animation: NativeAiAnimationRuntime,
    shunt_attack: RobotsShuntAttackRuntimeState,
    spike_attack: RobotsSpikeAttackRuntimeState,
    /// Handler+0x45C. Common AI_Character event `SET_SCRIPT_VALUE` writes the
    /// event float through native `__ftol`; Spike state4 suppresses direct yaw while nonzero.
    handler_script_value_45c: u8,
    /// Common AI_Character vslot +0xA4/+0xB0 attachment lane. This keeps native
    /// Particle/Swoosh ownership engine-neutral; GUI/UE rendering is an adapter concern.
    character_attachments: RobotsCharacterAttachmentRuntimeState,
    common_hit: RobotsCommonAiHitRuntimeState,
    /// Common Handler+0x4B8/+0x4BC retained HitCheck records. Most Monster
    /// classes keep one; Monster_2Rockets vslot +0x08 raises Handler+0x618 capacity to two.
    hit_queries: Vec<NativeStandardMonsterHitQueryRuntime>,
    scrambled_hit: RobotsMalfBotScrambledHitRuntimeState,
    electro_hit: RobotsMalfBotElectroHitRuntimeState,
    magnetic_hit: RobotsMalfBotMagneticHitRuntimeState,
    magnetic_drop_charge_count: Option<u8>,
    active_node: Option<RobotsStandardMonsterBehaviorWinner>,
    animation: NativeAiAnimationRuntime,
}

impl NativeStandardMonsterRuntime {
    fn new(config: RobotsStandardMonsterBehaviorConfig) -> Self {
        let mut stalk_navmesh = RobotsStalkNavMeshRuntimeState::default();
        if let Some(stalk) = config.stalk {
            eurochef_shared::robots_runtime::stalk_navmesh::initialize_stalk_navmesh(
                &mut stalk_navmesh,
                stalk,
            );
        }
        Self {
            handler_class: config.handler_class,
            magnabot_turn: RobotsMagnaBotTurnRuntimeState::default(),
            patrol: RobotsAiPatrolRuntimeState::configured(config.patrol),
            periodic_idle: RobotsPeriodicIdleRuntimeState::default(),
            patrol_navmesh2: RobotsPatrolNavMesh2RuntimeState::default(),
            pursue_navmesh: RobotsPursueNavMeshRuntimeState::default(),
            stalk_navmesh,
            flee_navmesh: RobotsFleeNavMeshRuntimeState::default(),
            follow_flying_path: RobotsFollowFlyingPathRuntimeState::default(),
            ef01_flying_path_latch_644: false,
            follow_network_path: RobotsFollowNetworkPathRuntimeState::default(),
            eq03_attachment: RobotsEq03SpiderAttachmentRuntimeState::default(),
            blades_attachment_local_spin_xyzw: [0.0, 0.0, 0.0, 1.0],
            ew09_attachment_local_spin_xyzw: [0.0, 0.0, 0.0, 1.0],
            minion_attachment_local_spin_xyzw: [0.0, 0.0, 0.0, 1.0],
            thiefbot_attachment: RobotsThiefBotAttachmentRuntimeState::default(),
            knightbot_animator: (config.handler_class == RobotsAiHandlerClass::Eb13KnightBot)
                .then_some(RobotsKnightBotAnimatorRuntimeState::default()),
            locomotion: RobotsAiLocomotionRuntimeState::default(),
            movement_error: RobotsAiMovementErrorRuntimeState::default(),
            attack_group: RobotsGenericAttackGroupRuntimeState::default(),
            attacks: [RobotsGenericAttackRuntimeState::default();
                ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            attack_group_three_phase: RobotsThreePhaseAttackRuntimeState::default(),
            turn_then_attacks: [RobotsTurnThenAttackRuntimeState::default();
                ROBOTS_STANDARD_MONSTER_MAX_ATTACKS],
            direct_turn_then_attack: RobotsTurnThenAttackRuntimeState::default(),
            circle_target: RobotsCircleTargetRuntimeState::default(),
            secondary_three_phase_attack: RobotsThreePhaseAttackRuntimeState::default(),
            secondary_animation: NativeAiAnimationRuntime::default(),
            shunt_attack: RobotsShuntAttackRuntimeState::default(),
            spike_attack: RobotsSpikeAttackRuntimeState::default(),
            handler_script_value_45c: 0,
            character_attachments: RobotsCharacterAttachmentRuntimeState::default(),
            common_hit: RobotsCommonAiHitRuntimeState::default(),
            hit_queries: Vec::new(),
            scrambled_hit: RobotsMalfBotScrambledHitRuntimeState::default(),
            electro_hit: RobotsMalfBotElectroHitRuntimeState::default(),
            magnetic_hit: RobotsMalfBotMagneticHitRuntimeState::default(),
            magnetic_drop_charge_count: None,
            active_node: None,
            animation: NativeAiAnimationRuntime::default(),
        }
    }

    fn clear_hit_queries(&mut self) {
        self.hit_queries.clear();
    }

    /// Native common HIT_CHECK registration `0x00453160`: when retained count is
    /// already at Handler+0x618 capacity, erase exactly the oldest 0x98-byte record
    /// before appending the new query.
    fn register_hit_query(&mut self, entry: NativeStandardMonsterHitQueryRuntime, capacity: usize) {
        if capacity == 0 {
            return;
        }
        if self.hit_queries.len() >= capacity {
            self.hit_queries.remove(0);
        }
        self.hit_queries.push(entry);
    }

    /// Standard Monster animation boundary plus common AI_Character event state.
    /// Native handler event `0x16000007` writes Handler+0x45C regardless of which
    /// behavior node requested the AnimMode, so keep this above class-specific FSMs.
    fn advance_animation(
        &mut self,
        body: &mut RuntimeCharacterBodyState,
        requested_anim_mode: u32,
        owner_yaw_write: Option<f32>,
        fixed_step_seconds: f32,
        policy: NativeAiRootMotionPolicy,
    ) -> Vec<NativeAiAnimationEvent> {
        let policy =
            if standard_monster_physics_locomotion_speed_range(self.handler_class).is_some() {
                NativeAiRootMotionPolicy::NONE
            } else {
                policy
            };
        let events = self.animation.advance(
            body,
            requested_anim_mode,
            owner_yaw_write,
            fixed_step_seconds,
            policy,
        );
        for event in &events {
            self.apply_common_animation_event(event);
        }
        events
    }

    fn step_standard_locomotion(
        &mut self,
        mut input: RobotsAiLocomotionInput,
    ) -> Option<RobotsAiLocomotionStep> {
        if self.handler_class != RobotsAiHandlerClass::Eb11MagnaBot {
            return Some(step_ai_locomotion_after_steering_prepass(
                &mut self.locomotion,
                input,
            ));
        }

        let yaw_error_radians = shortest_yaw_delta(
            input.current_owner_yaw_radians,
            input.steering_target_yaw_radians,
        );
        let resolved_turn_rate = match input.turn_rate {
            RobotsAiTurnRateInput::Default => ROBOTS_AI_DEFAULT_TURN_RATE_RADIANS_PER_SECOND,
            RobotsAiTurnRateInput::Explicit(value) => value,
        };

        let result = if self.magnabot_turn.turn_gate(yaw_error_radians) {
            let action = self.magnabot_turn.turn_action(
                self.animation.current_anim_mode(),
                yaw_error_radians,
                resolved_turn_rate,
                ROBOTS_ANIM_MODE_TURN_ON_SPOT,
            );
            match action {
                RobotsMagnaBotTurnAction::None => None,
                RobotsMagnaBotTurnAction::RequestAnimMode(requested_anim_mode) => {
                    Some(RobotsAiLocomotionStep {
                        owner_yaw_radians: input.current_owner_yaw_radians,
                        locomotion_scalar: self.locomotion.locomotion_scalar,
                        requested_anim_mode,
                        turn_in_place_active: true,
                        direct_owner_yaw_write: false,
                    })
                }
                action @ RobotsMagnaBotTurnAction::BaseTurn { .. } => resolve_magnabot_base_turn(
                    action,
                    input.handler_flags_628,
                    input.current_owner_yaw_radians,
                    input.runtime_rate_scale,
                )
                .map(|turn| RobotsAiLocomotionStep {
                    owner_yaw_radians: turn.owner_yaw_radians,
                    locomotion_scalar: self.locomotion.locomotion_scalar,
                    requested_anim_mode: turn.requested_anim_mode,
                    turn_in_place_active: true,
                    direct_owner_yaw_write: turn.direct_owner_yaw_write,
                }),
            }
        } else {
            // `0x00452800` already asked Magna's +0x114 and only reaches its normal
            // Move tail when that override returned false. Mask the base +0x114
            // emulation here so we do not run the standard 30/5-degree latch again.
            self.locomotion.turn_in_place_latch = false;
            input.handler_flags_628 &= !ROBOTS_AI_TURN_IN_PLACE_ENABLE_FLAG;
            Some(step_ai_locomotion_after_steering_prepass(
                &mut self.locomotion,
                input,
            ))
        };
        // Magna +0x10C `0x00464D60` sets +0x649 after common Move returns even when
        // +0x118 consumed the call without requesting an animation.
        self.magnabot_turn.record_locomotion_service();
        result
    }

    fn step_magnabot_direct_turn(
        &mut self,
        yaw_error_radians: f32,
        turn_rate_radians_per_second: f32,
        anim_mode: u32,
        handler_flags_628: u32,
        current_owner_yaw_radians: f32,
    ) -> Option<RobotsAiLocomotionStep> {
        let action = self.magnabot_turn.turn_action(
            self.animation.current_anim_mode(),
            yaw_error_radians,
            turn_rate_radians_per_second,
            anim_mode,
        );
        match action {
            RobotsMagnaBotTurnAction::None => None,
            RobotsMagnaBotTurnAction::RequestAnimMode(requested_anim_mode) => {
                Some(RobotsAiLocomotionStep {
                    owner_yaw_radians: current_owner_yaw_radians,
                    locomotion_scalar: self.locomotion.locomotion_scalar,
                    requested_anim_mode,
                    turn_in_place_active: true,
                    direct_owner_yaw_write: false,
                })
            }
            action @ RobotsMagnaBotTurnAction::BaseTurn { .. } => resolve_magnabot_base_turn(
                action,
                handler_flags_628,
                current_owner_yaw_radians,
                1.0,
            )
            .map(|turn| RobotsAiLocomotionStep {
                owner_yaw_radians: turn.owner_yaw_radians,
                locomotion_scalar: self.locomotion.locomotion_scalar,
                requested_anim_mode: turn.requested_anim_mode,
                turn_in_place_active: true,
                direct_owner_yaw_write: turn.direct_owner_yaw_write,
            }),
        }
    }

    fn apply_common_animation_event(&mut self, event: &NativeAiAnimationEvent) {
        if self.handler_class == RobotsAiHandlerClass::Eb11MagnaBot {
            self.magnabot_turn
                .apply_event(event.event_type, event.as_view().native_arg_word(0));
        }
        if event.event_type == event_type::SET_SCRIPT_VALUE {
            if let Some(value) = event.as_view().native_arg_float(0) {
                self.handler_script_value_45c = native_script_ftol(value) as u8;
            }
        }
        let _ =
            apply_ai_character_attachment_event(&mut self.character_attachments, event.as_view());
    }
}

impl Default for NativeStandardMonsterRuntime {
    fn default() -> Self {
        Self::new(RobotsStandardMonsterBehaviorConfig::malfbot())
    }
}

fn is_eq04_mine(trigger: &ProcessedTrigger) -> bool {
    trigger
        .character_visual
        .as_ref()
        .is_some_and(|visual| visual.handler_class == RobotsAiHandlerClass::Eq04Mine)
}

fn translated_character_world_shape(
    shape: RuntimeCharacterWorldShape,
    translation: Vec3,
) -> RuntimeCharacterWorldShape {
    match shape {
        RuntimeCharacterWorldShape::Sphere { center_xyz, radius } => {
            RuntimeCharacterWorldShape::Sphere {
                center_xyz: (Vec3::from_array(center_xyz) + translation).to_array(),
                radius,
            }
        }
        RuntimeCharacterWorldShape::Capsule {
            start_xyz,
            delta_xyz,
            radius,
        } => RuntimeCharacterWorldShape::Capsule {
            start_xyz: (Vec3::from_array(start_xyz) + translation).to_array(),
            delta_xyz,
            radius,
        },
    }
}

/// Production EQ04 slice of the native scheduler: base Physics slot0 gravity and
/// owner-pose integration, then the later slot4 floor-plane contact projection.
/// Handler environment state is intentionally serviced by the caller afterwards.
fn advance_eq04_character_physics_step(
    physics: &mut RobotsCharacterPhysicsRuntimeState,
    owner_position: Vec3,
    world_shape: RuntimeCharacterWorldShape,
    floor_contact: Option<RuntimeAiEnvironmentFloorContact>,
) -> Vec3 {
    physics.advance_base_gravity(ROBOTS_FIXED_STEP_SECONDS);
    let mut position = owner_position.to_array();
    physics.integrate_position_with_gravity(&mut position, ROBOTS_FIXED_STEP_SECONDS);
    let mut position = Vec3::from_array(position);
    let integrated_shape = translated_character_world_shape(world_shape, position - owner_position);

    physics.latch_contact_response_after_base_update();
    physics.begin_world_contact_projection();
    if let Some(contact) = floor_contact {
        if let Some(correction) =
            runtime_character_floor_contact_projection(integrated_shape, contact)
        {
            position += correction;
            physics.mark_floor_contact_response();
        }
    }
    position
}

fn is_eq02_minebot(trigger: &ProcessedTrigger) -> bool {
    trigger
        .character_visual
        .as_ref()
        .is_some_and(|visual| visual.handler_class == RobotsAiHandlerClass::Eq02MineBot)
}

fn is_dogbot(trigger: &ProcessedTrigger) -> bool {
    trigger
        .character_visual
        .as_ref()
        .is_some_and(|visual| visual.handler_class == RobotsAiHandlerClass::DogBot)
}

fn is_rollerbot(trigger: &ProcessedTrigger) -> bool {
    trigger
        .character_visual
        .as_ref()
        .is_some_and(|visual| visual.handler_class == RobotsAiHandlerClass::Eb10RollerBot)
}

fn rollerbot_follow_path_host_priority(
    path_bound: bool,
    state: RobotsRollerBotFollowPathRuntimeState,
) -> u8 {
    if path_bound {
        state.priority()
    } else {
        1
    }
}

pub(super) fn horizontal_yaw(rotation: Quat) -> f32 {
    let mut forward = rotation * Vec3::Z;
    forward.y = 0.0;
    if forward.is_finite() && forward.length_squared() > f32::EPSILON {
        forward.x.atan2(forward.z)
    } else {
        0.0
    }
}

pub(super) fn locomotion_root_motion_policy(
    locomotion: RobotsAiLocomotionStep,
) -> NativeAiRootMotionPolicy {
    if locomotion.requested_anim_mode == ROBOTS_ANIM_MODE_MOVE {
        NativeAiRootMotionPolicy::TRANSLATION_ONLY
    } else if !locomotion.direct_owner_yaw_write
        && matches!(
            locomotion.requested_anim_mode,
            ROBOTS_ANIM_MODE_TURN_ON_SPOT_L | ROBOTS_ANIM_MODE_TURN_ON_SPOT_R
        )
    {
        NativeAiRootMotionPolicy::FULL
    } else {
        NativeAiRootMotionPolicy::NONE
    }
}

pub(super) fn advance_native_retained_horizontal_physics(
    physics: Option<&RobotsCharacterPhysicsRuntimeState>,
    body: &mut RuntimeCharacterBodyState,
) {
    let Some(physics) = physics else {
        return;
    };
    body.owner_position.x += physics.velocity_xyz[0] * ROBOTS_FIXED_STEP_SECONDS;
    body.owner_position.z += physics.velocity_xyz[2] * ROBOTS_FIXED_STEP_SECONDS;
}

pub(super) fn write_native_ai_physics_locomotion(
    physics: &mut RobotsCharacterPhysicsRuntimeState,
    owner_yaw_radians: f32,
    locomotion_scalar: f32,
    min_speed: f32,
    max_speed: f32,
) {
    physics.overwrite_velocity(ai_character_physics_velocity_from_scalar(
        owner_yaw_radians,
        min_speed,
        max_speed,
        locomotion_scalar,
        1.0,
    ));
}

fn commit_native_standard_physics_locomotion(
    physics: &mut FxHashMap<u64, RobotsCharacterPhysicsRuntimeState>,
    key: u64,
    handler_class: RobotsAiHandlerClass,
    locomotion: RobotsAiLocomotionStep,
) {
    let Some([min_speed, max_speed]) =
        standard_monster_physics_locomotion_speed_range(handler_class)
    else {
        return;
    };
    write_native_ai_physics_locomotion(
        physics
            .entry(key)
            .or_insert_with(RobotsCharacterPhysicsRuntimeState::monster_ctor_default),
        locomotion.owner_yaw_radians,
        locomotion.locomotion_scalar,
        min_speed,
        max_speed,
    );
}

pub(super) fn monster_nav_regions(map: &ProcessedMap) -> Vec<RobotsMonsterNavMeshView<'_>> {
    map.zone_navmeshes
        .iter()
        .flat_map(|zone| zone.iter())
        .map(|navmesh| RobotsMonsterNavMeshView {
            vertices: &navmesh.vertices,
            faces: &navmesh.faces,
            adjacency: &navmesh.adjacency,
            groups: &navmesh.groups,
        })
        .collect()
}

fn next_ai_gameplay_rng_u32(
    native_lifecycle_valid: bool,
    native_global: &mut RuntimeRobotsGlobalRngState,
    preview: &mut RuntimeRobotsGlobalRngState,
) -> Option<u32> {
    if native_lifecycle_valid && native_global.seed().is_some() {
        native_global.next_u32()
    } else {
        preview.next_u32()
    }
}

fn ai_process_lcg_modulo_zero_gate(
    native_lifecycle_valid: bool,
    native_seed: &mut Option<u32>,
    preview_seed: &mut Option<u32>,
    modulus: u32,
) -> Option<bool> {
    if native_lifecycle_valid && native_seed.is_some() {
        robots_process_lcg_modulo_zero_gate(native_seed, modulus)
    } else {
        robots_process_lcg_modulo_zero_gate(preview_seed, modulus)
    }
}

impl MapFrame {
    /// Player-owned equivalent of native Player +0x544 written by XWeapon_Magnet.
    /// This seam deliberately does not live in MalfBot runtime: any future weapon
    /// host binds the currently controlled magnetic XItem here, and AI only reads it.
    pub fn bind_native_player_magnetic_target(&mut self, target_key: u64) {
        self.native_player_magnetic_target_key = Some(target_key);
    }

    pub fn clear_native_player_magnetic_target(&mut self, target_key: u64) {
        if self.native_player_magnetic_target_key == Some(target_key) {
            self.native_player_magnetic_target_key = None;
        }
    }

    /// AI nodes consume the real process-global gameplay stream only while Maps is
    /// running the validated native TriggerManager lifecycle *and* that stream is
    /// explicitly anchored. Otherwise `Simulate AI` falls back to its own deterministic
    /// fresh-process preview stream, so RNG-backed PatrolNavMesh/Flee/Attack nodes can
    /// execute without mutating or pretending to know the native session seed.
    pub(super) fn next_native_ai_gameplay_rng_u32(&mut self) -> Option<u32> {
        next_ai_gameplay_rng_u32(
            self.native_script_trigger_lifecycle_valid,
            &mut self.native_global_gameplay_rng,
            &mut self.native_ai_preview_gameplay_rng,
        )
    }

    /// Resolve the currently sampled AnimMode/time from the shared live-AI key.
    /// Fatal playback owns the XItem animation while present; otherwise each class
    /// host exposes its same common animation channel here. Ambiguous dual-turret
    /// playback fails closed rather than inventing one component as the owner.
    pub(super) fn native_ai_animation_sample_by_key(&self, key: u64) -> Option<(u32, f32)> {
        let sample = |animation: &NativeAiAnimationRuntime| {
            let mode = animation.current_anim_mode();
            (mode != 0).then_some((mode, animation.sampled_pose_seconds()))
        };
        if let Some(animation) = self
            .native_ai_fatal_runtime
            .get(&key)
            .map(|runtime| &runtime.animation)
            .or_else(|| {
                self.native_standard_monster_runtime
                    .get(&key)
                    .map(|runtime| &runtime.animation)
            })
            .or_else(|| {
                self.native_eq04_mine_runtime
                    .get(&key)
                    .map(|runtime| &runtime.animation)
            })
            .or_else(|| {
                self.native_minebot_runtime
                    .get(&key)
                    .map(|runtime| &runtime.animation)
            })
            .or_else(|| {
                self.native_dogbot_runtime
                    .get(&key)
                    .map(|runtime| &runtime.animation)
            })
            .or_else(|| {
                self.native_rollerbot_runtime
                    .get(&key)
                    .map(|runtime| &runtime.animation)
            })
        {
            return sample(animation);
        }
        let turret = self.native_turret_runtime.get(&key)?;
        match (
            sample(&turret.primary_animation),
            sample(&turret.secondary_animation),
        ) {
            (Some(sample), None) | (None, Some(sample)) => Some(sample),
            _ => None,
        }
    }

    fn ensure_native_ai_hit_reaction_for_visual(
        &mut self,
        key: u64,
        visual: &ProcessedCharacterVisual,
    ) {
        let Some(initial_health) = visual.initial_health else {
            return;
        };
        self.native_ai_hit_reactions
            .entry(key)
            .or_insert(RobotsAiHitReactionState {
                query_flags_snapshot: 0,
                query_serial_snapshot: u16::MAX,
                hit_metadata_snapshot: 0,
                last_query_serial: u16::MAX,
                capability_flags: visual.handler_flags_628,
                health: initial_health,
                got_hit_latch: false,
                owner_category: visual.runtime_type,
            });
    }

    fn sync_native_ai_hit_reactions(&mut self, map: &ProcessedMap) {
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            let Some(visual) = trigger.character_visual.as_ref() else {
                continue;
            };
            let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
            if !self.runtime_ai_character_xitem_live(map.hashcode, trigger_index) {
                self.native_ai_hit_reactions.remove(&key);
                self.native_ai_last_hit_source_yaw.remove(&key);
                self.native_ai_last_hit_source_position.remove(&key);
                continue;
            }
            self.ensure_native_ai_hit_reaction_for_visual(key, visual);
        }

        let dynamic_snapshots = self.runtime_dynamic_ai_snapshots(map);
        let mut live_dynamic_keys = FxHashSet::default();
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
            live_dynamic_keys.insert(snapshot.key);
            self.ensure_native_ai_hit_reaction_for_visual(snapshot.key, visual);
        }
        self.native_ai_hit_reactions.retain(|key, _| {
            !super::runtime_bodies::RuntimeAiInstanceKey::raw_is_dynamic_for_map(*key, map.hashcode)
                || live_dynamic_keys.contains(key)
        });
        self.native_ai_last_hit_source_yaw.retain(|key, _| {
            !super::runtime_bodies::RuntimeAiInstanceKey::raw_is_dynamic_for_map(*key, map.hashcode)
                || live_dynamic_keys.contains(key)
        });
        self.native_ai_last_hit_source_position.retain(|key, _| {
            !super::runtime_bodies::RuntimeAiInstanceKey::raw_is_dynamic_for_map(*key, map.hashcode)
                || live_dynamic_keys.contains(key)
        });
    }

    fn sync_native_dynamic_ai_pose_back_to_registry(&mut self, map: &ProcessedMap) {
        let updates = self
            .runtime_dynamic_ai_snapshots(map)
            .into_iter()
            .filter(|snapshot| !snapshot.pending_destroy)
            .filter_map(|snapshot| {
                let body = self.runtime_character_bodies.get(&snapshot.key)?;
                let position = body.owner_position;
                Some((
                    snapshot.live_id,
                    [position.x, position.y, position.z, 1.0],
                    horizontal_yaw(body.owner_rotation),
                ))
            })
            .collect::<Vec<_>>();
        let Some(runtime) = self.native_sweeper_boss_runtime.as_mut() else {
            return;
        };
        for (live_id, position, yaw) in updates {
            let _ = runtime.live_ai.sync_live_pose(live_id, position, yaw);
        }
    }

    fn advance_native_current_attacker_fixed(
        &mut self,
        map: &ProcessedMap,
        nav_regions: &[RobotsMonsterNavMeshView<'_>],
        player_position: Vec3,
    ) {
        let player_state = self.native_player_focus_runtime.player_state;
        let Some(target_position) = self.native_ai_gameplay_target_position(player_position) else {
            self.native_current_attacker.current_owner_key = None;
            return;
        };

        let mut target_nav = RobotsMonsterNavigationRuntimeState::default();
        let _ = target_nav.initialize(nav_regions, target_position.to_array());
        let target_region_ordinal = target_nav.region_ordinal;
        let target_group_flags0 = target_nav.group_flags0;

        // 0x00455560 reads the same common environment-floor lane used by AI physics.
        // Unknown geometry is a host seam, not a native "no floor" result, so do not
        // mutate the process-global selection until that geometry is available.
        let Some(target_floor) = runtime_map_ai_environment_floor_contact(map, target_position) else {
            return;
        };
        let target_height_above_floor = target_floor.map(|contact| {
            target_position.y - contact.point.y
        });
        let watchbot_target = matches!(player_state, 0x2a | 0x35);
        let normal_environment_blocked = (!watchbot_target).then(|| {
            robots_current_attacker_player_environment_blocked(
                self.native_game_control_runtime.mode_50f,
                player_state,
                target_height_above_floor,
            )
        });

        #[derive(Clone, Copy)]
        struct CandidateHostSnapshot {
            key: u64,
            handler_flags_628: u32,
            allow_cross_group: bool,
            base_priority: u8,
            position: Vec3,
            owner_region_ordinal: Option<usize>,
            owner_group_flags0: Option<u8>,
        }

        let mut host_candidates = Vec::<CandidateHostSnapshot>::new();
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            let Some(visual) = trigger.character_visual.as_ref() else {
                continue;
            };
            let Some(base_priority) = visual.attacker_priority_638 else {
                continue;
            };
            if !self.runtime_ai_character_xitem_live(map.hashcode, trigger_index) {
                continue;
            }
            let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
            let Some(body) = self.runtime_character_bodies.get(&key) else {
                continue;
            };
            let nav = self.native_monster_navigation.get(&key);
            host_candidates.push(CandidateHostSnapshot {
                key,
                handler_flags_628: visual.handler_flags_628,
                allow_cross_group: trigger
                    .data
                    .get(7)
                    .and_then(|value| *value)
                    .is_some_and(|flags| flags & 0x8000 != 0),
                base_priority,
                position: body.owner_position,
                owner_region_ordinal: nav.and_then(|state| state.region_ordinal),
                owner_group_flags0: nav.and_then(|state| state.group_flags0),
            });
        }

        for snapshot in self
            .runtime_dynamic_ai_snapshots(map)
            .into_iter()
            .filter(|snapshot| !snapshot.pending_destroy)
        {
            let Some(visual) = map
                .runtime_character_visuals
                .get(&(ROBOTS_SWEEPER_MONSTER_RUNTIME_TYPE, snapshot.selector))
            else {
                continue;
            };
            let Some(base_priority) = visual.attacker_priority_638 else {
                continue;
            };
            let Some(body) = self.runtime_character_bodies.get(&snapshot.key) else {
                continue;
            };
            let nav = self.native_monster_navigation.get(&snapshot.key);
            host_candidates.push(CandidateHostSnapshot {
                key: snapshot.key,
                handler_flags_628: visual.handler_flags_628,
                allow_cross_group: ROBOTS_DIRECT_MONSTER_FACTORY_ALLOW_CROSS_GROUP,
                base_priority,
                position: body.owner_position,
                owner_region_ordinal: nav.and_then(|state| state.region_ordinal),
                owner_group_flags0: nav.and_then(|state| state.group_flags0),
            });
        }

        let mut live_gate_keys = FxHashSet::default();
        let mut candidates = Vec::<RobotsCurrentAttackerCandidate>::with_capacity(host_candidates.len());
        for host in host_candidates {
            live_gate_keys.insert(host.key);
            let environment_blocked = if watchbot_target {
                let datum_relative_y = self
                    .runtime_ai_animation_datum_world_transform_by_key(
                        map,
                        host.key,
                        ROBOTS_CURRENT_ATTACKER_WATCHBOT_DATUM_UID,
                    )
                    .map(|datum| datum.position.y - host.position.y);
                self.native_current_attacker_watchbot_gates
                    .entry(host.key)
                    .or_default()
                    .advance(target_height_above_floor, datum_relative_y)
            } else {
                normal_environment_blocked.unwrap_or(false)
            };
            let eligible = robots_current_attacker_candidate_eligible(
                RobotsCurrentAttackerEligibilityInput {
                    handler_flags_628: host.handler_flags_628,
                    owner_region_ordinal: host.owner_region_ordinal,
                    owner_group_flags0: host.owner_group_flags0,
                    allow_cross_group: host.allow_cross_group,
                    target_region_ordinal,
                    target_group_flags0,
                    environment_blocked,
                },
            );
            candidates.push(RobotsCurrentAttackerCandidate {
                owner_key: host.key,
                eligible,
                base_priority: host.base_priority,
                position_xyz: host.position.to_array(),
            });
        }
        self.native_current_attacker_watchbot_gates
            .retain(|key, _| live_gate_keys.contains(key));

        let mut state = self.native_current_attacker;
        let bonus_owner = self.native_current_attacker_bonus_owner;
        state.advance(
            &candidates,
            bonus_owner,
            target_position.to_array(),
            || {
                self.next_native_ai_gameplay_rng_u32()
                    .expect("AI preview RNG must remain anchored while simulation is active")
            },
        );
        self.native_current_attacker = state;
    }

    /// Runtime-created Sweeper/Transporter MalfBot and RollerBot update in the
    /// native XItem slice. Factory setup/RNG remains owned by the Sweeper runtime;
    /// from this seam onward behavior, movement, attacks, hit reaction and fatal
    /// handling are the same class hosts used by serialized AI.
    pub(super) fn advance_native_dynamic_sweeper_ai_fixed(
        &mut self,
        map: &ProcessedMap,
        player_position: Vec3,
        wall_time: f64,
    ) {
        if !self.simulate_ai {
            return;
        }
        self.sync_runtime_dynamic_character_bodies(map);
        self.sync_native_ai_hit_reactions(map);
        self.advance_native_dynamic_common_fatal_ai_fixed(map);
        let nav_regions = monster_nav_regions(map);
        self.advance_native_standard_monsters_fixed(
            map,
            &nav_regions,
            player_position,
            wall_time,
            NativeAiHostScope::DynamicSweeper,
        );
        self.advance_native_rollerbot_fixed(
            map,
            &nav_regions,
            player_position,
            wall_time,
            NativeAiHostScope::DynamicSweeper,
        );
        self.flush_native_ai_deferred_destroy_fixed(map.hashcode);
        self.sync_native_dynamic_ai_pose_back_to_registry(map);
    }

    pub(super) fn advance_native_ai_simulation(&mut self, map: &ProcessedMap, wall_time: f64) {
        self.native_ai_script_spawns.clear();
        if !self.simulate_ai {
            self.native_ai_fixed_last_time = None;
            self.native_ai_fixed_accumulator = 0.0;
            return;
        }

        if self.native_script_trigger_lifecycle_valid {
            self.native_ai_preview_live.clear();
            self.native_ai_preview_map = None;
        } else if self.native_ai_preview_map != Some(map.hashcode) {
            self.native_ai_preview_live.clear();
            for (trigger_index, trigger) in map.triggers.iter().enumerate() {
                if robots_character_runtime_type(trigger.ttype).is_none() {
                    continue;
                }
                let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
                if self.runtime_character_bodies.contains_key(&key) {
                    self.native_ai_preview_live.insert(key);
                }
            }
            self.native_ai_preview_map = Some(map.hashcode);
        }

        let Some((player_position, player_yaw_radians)) = self.native_gameplay_player_pose(map)
        else {
            self.native_ai_fixed_last_time = None;
            self.native_ai_fixed_accumulator = 0.0;
            return;
        };

        // runtime_event_snapshots() has already updated TriggerManager/XItem
        // presence before this host runs. Recreate Handler hit state only across
        // the same native XItem lifetime, never from immutable map metadata alone.
        self.sync_native_ai_hit_reactions(map);

        let delta_seconds = self
            .native_ai_fixed_last_time
            .replace(wall_time)
            .map(|previous| (wall_time - previous).max(0.0))
            .unwrap_or_default();
        let fixed_seconds = f64::from(ROBOTS_FIXED_STEP_SECONDS);
        self.native_ai_fixed_accumulator += delta_seconds;
        let steps = (self.native_ai_fixed_accumulator / fixed_seconds)
            .floor()
            .min(ROBOTS_AI_MAX_FIXED_STEPS_PER_FRAME as f64) as usize;
        self.native_ai_fixed_accumulator -= steps as f64 * fixed_seconds;
        let nav_regions = monster_nav_regions(map);

        for _ in 0..steps {
            if self.native_player_hit_runtime.advance_fixed(1.0) {
                self.native_runtime_player_last_hit_query_serial = 0;
            }
            self.native_monster_attack_cooldown
                .advance(ROBOTS_FIXED_STEP_SECONDS);
            self.advance_native_ai_projectiles_fixed(map);
            self.advance_native_ai_explosion_fragments_fixed(map, player_position);
            self.advance_native_ai_explosions_fixed(map);
            self.advance_native_current_attacker_fixed(map, &nav_regions, player_position);
            self.advance_native_common_fatal_ai_fixed(map);
            self.advance_native_dogbot_fixed(map, &nav_regions, player_position, wall_time);
            self.advance_native_turret_fixed(map, player_position, wall_time);
            self.advance_native_ep05_turret_fixed(map, player_position, wall_time);
            self.advance_native_ep06_turret_fixed(map, player_position, wall_time);
            self.advance_native_turretbot_fixed(map, player_position, wall_time);
            self.advance_native_base_monsters_fixed(map, &nav_regions);
            self.advance_native_em07_piranha_fixed(map, player_position, wall_time);
            self.advance_native_test_anim_bot_fixed(
                map,
                &nav_regions,
                player_position,
                wall_time,
            );
            self.advance_native_standard_monsters_fixed(
                map,
                &nav_regions,
                player_position,
                wall_time,
                NativeAiHostScope::Serialized,
            );
            self.advance_native_dodgem_fixed(map, &nav_regions, player_position, wall_time);
            self.advance_native_spintop_fixed(map, &nav_regions);
            self.advance_native_rollerbot_fixed(
                map,
                &nav_regions,
                player_position,
                wall_time,
                NativeAiHostScope::Serialized,
            );
            self.advance_native_eq04_mine_fixed(map, player_position);
            self.advance_native_minebot_fixed(map, &nav_regions, player_position, wall_time);
            // Native main item update (0x00444C20) advances DAT_007B29E0 only
            // after XItem/AI updates have consumed the current throttle time.
            self.native_ai_event_throttle.advance_fixed();
            self.service_native_player_ai_solid_contacts_fixed(
                map,
                player_position,
                player_yaw_radians,
            );
            // Native MonsterExplosion order is factory/init before the producer's
            // deferred destroy is physically unlinked at end of tick.
            self.consume_native_ai_explosion_spawns(map);
            self.flush_native_ai_explosion_fragment_destroy_fixed();
            self.flush_native_ai_deferred_destroy_fixed(map.hashcode);
            self.native_ai_engine_frame_counter =
                self.native_ai_engine_frame_counter.wrapping_add(1);
        }
    }

    fn flush_native_ai_deferred_destroy_fixed(&mut self, map_hash: u32) {
        for key in self.native_ai_deferred_destroy.take_all() {
            if let Some(live_id) =
                super::runtime_bodies::RuntimeAiInstanceKey::dynamic_live_id_for_map(key, map_hash)
            {
                if let Some(runtime) = self.native_sweeper_boss_runtime.as_mut() {
                    let _ = runtime
                        .live_ai
                        .mark_live_id_pending_destroy_after_native_request(live_id);
                }
            } else if let Some(lifecycle) = self.native_ai_trigger_lifecycle.get_mut(&key) {
                lifecycle.cleanup_zone_failed();
            }
            self.native_ai_preview_live.remove(&key);
            self.native_ai_hit_reactions.remove(&key);
            self.native_ai_last_hit_source_yaw.remove(&key);
            self.native_ai_fatal_runtime.remove(&key);
            self.native_eq04_mine_runtime.remove(&key);
            self.native_minebot_runtime.remove(&key);
            self.native_dogbot_runtime.remove(&key);
            self.native_test_anim_bot_runtime.remove(&key);
            self.native_standard_monster_runtime.remove(&key);
            self.native_rollerbot_runtime.remove(&key);
            self.native_monster_navigation.remove(&key);
            self.native_monster_physics.remove(&key);
            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                body.clear_collision_animation_track();
            }
        }
    }

    pub(super) fn request_native_ai_natural_death(
        &mut self,
        map_hash: u32,
        key: u64,
        position: Vec3,
    ) {
        if super::runtime_bodies::RuntimeAiInstanceKey::dynamic_live_id_for_map(key, map_hash)
            .is_none()
        {
            self.native_ai_creator_runtime
                .entry(key)
                .or_default()
                .record_natural_death(position);
        }
        self.native_ai_deferred_destroy.mark(key);
    }

    /// Native global XItem collision phase `0x00444C20 -> 0x004D5750 -> 0x004D5B80`.
    /// Only the recovered specialized AI Handler +0x64 callbacks are projected here;
    /// the post-callback physical response `0x004D6010` remains Player/Physics host state.
    /// Native confirms overlap first, then invokes the two Handler callbacks, and only
    /// afterwards reaches end-of-tick deferred destruction `0x00444E80`.
    fn service_native_player_ai_solid_contacts_fixed(
        &mut self,
        map: &ProcessedMap,
        player_position: Vec3,
        player_yaw_radians: f32,
    ) {
        let Some(player_shape) =
            native_player_solid_collision_shape(map, player_position, player_yaw_radians)
        else {
            return;
        };
        let mut contacts = Vec::<(u64, u32, Vec3)>::new();

        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            let Some(visual) = trigger.character_visual.as_ref() else {
                continue;
            };
            if !matches!(
                visual.handler_class,
                RobotsAiHandlerClass::Ef01Mine | RobotsAiHandlerClass::Eq04Mine
            ) {
                continue;
            }
            if !self.runtime_ai_character_xitem_live(map.hashcode, trigger_index) {
                continue;
            }

            let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
            let owner_pending_destroy = self.native_ai_deferred_destroy.is_pending(key);
            let Some(body) = self.runtime_character_bodies.get(&key) else {
                continue;
            };
            let animation = match visual.handler_class {
                RobotsAiHandlerClass::Ef01Mine => self
                    .native_standard_monster_runtime
                    .get(&key)
                    .map(|runtime| &runtime.animation),
                RobotsAiHandlerClass::Eq04Mine => self
                    .native_eq04_mine_runtime
                    .get(&key)
                    .map(|runtime| &runtime.animation),
                _ => None,
            };
            let Some(owner_shape) = native_character_solid_collision_shape(body, visual, animation)
            else {
                continue;
            };
            if native_special_ai_solid_contact_requests_death(
                visual.handler_class,
                owner_pending_destroy,
                owner_shape,
                player_shape,
            ) {
                let explosion_uid = match visual.handler_class {
                    RobotsAiHandlerClass::Ef01Mine => native_common_monster_explosion_uid(visual),
                    RobotsAiHandlerClass::Eq04Mine => Some(ROBOTS_EQ04_MINE_EXPLOSION_UID),
                    _ => None,
                };
                if let Some(explosion_uid) = explosion_uid {
                    contacts.push((key, explosion_uid, body.owner_position));
                }
            }
        }

        for (key, explosion_uid, position) in contacts {
            // Native overlap callbacks create the explosion before Handler +0x12C.
            // Preserve that order and keep the factory request distinct from XItem lifetime.
            self.queue_native_ai_explosion(key, explosion_uid, position);
            self.request_native_ai_natural_death(map.hashcode, key, position);
        }
    }

    fn native_ai_normal_fatal_condition(&self, key: u64) -> bool {
        self.native_ai_hit_reactions
            .get(&key)
            .is_some_and(|hit_state| {
                RuntimeNativeAiHitFatalState::normal_fatal_hit_condition(
                    hit_state.got_hit_latch,
                    hit_state.health,
                )
            })
    }

    fn advance_native_common_fatal_fixed(
        &mut self,
        map_hash: u32,
        visual: &ProcessedCharacterVisual,
        key: u64,
        owner_yaw_radians: f32,
    ) -> bool {
        let Some(fatal_config) = visual.handler_class.common_fatal_hit_config() else {
            self.native_ai_fatal_runtime.remove(&key);
            return false;
        };
        if !self.native_ai_normal_fatal_condition(key) {
            self.native_ai_fatal_runtime.remove(&key);
            return false;
        }

        if !self.native_ai_fatal_runtime.contains_key(&key) {
            let Some(source_yaw_radians) = self.native_ai_last_hit_source_yaw.get(&key).copied()
            else {
                // Native fatal direction comes from Handler+0x330. A missing source
                // pointer blocks normal AI progression but does not invent a yaw.
                return true;
            };
            let (anim_mode, target_yaw_radians) = robots_common_ai_hit_direction_selection(
                fatal_config,
                owner_yaw_radians,
                source_yaw_radians,
            );
            let Some(body) = self.runtime_character_bodies.get(&key) else {
                return true;
            };
            if !body.animation_modes.contains_key(&anim_mode)
                || !body.animation_mode_scripts.contains_key(&anim_mode)
            {
                // The native node is proven, but a missing shipped animation/script
                // resource is not something the host may silently synthesize.
                return true;
            }
            self.native_ai_fatal_runtime.insert(
                key,
                NativeCommonAiFatalRuntime {
                    anim_mode,
                    target_yaw_radians,
                    turn_rate_radians_per_second: fatal_config.turn_rate_radians_per_second,
                    fatal: RuntimeNativeAiHitFatalState::default(),
                    animation: NativeAiAnimationRuntime::default(),
                },
            );
            if let Some(minebot) = self.native_minebot_runtime.get_mut(&key) {
                minebot.move_state = RobotsMineBotMoveRuntimeState::default();
                minebot.attack_state = RobotsMineBotAttackRuntimeState::default();
                minebot.animation.setup_idle();
            }
            if let Some(malfbot) = self.native_standard_monster_runtime.get_mut(&key) {
                malfbot.active_node = None;
                malfbot.attack_group = RobotsGenericAttackGroupRuntimeState::default();
                malfbot.attacks = [RobotsGenericAttackRuntimeState::default();
                    ROBOTS_STANDARD_MONSTER_MAX_ATTACKS];
                malfbot.shunt_attack = RobotsShuntAttackRuntimeState::default();
                malfbot.spike_attack = RobotsSpikeAttackRuntimeState::default();
                malfbot.handler_script_value_45c = 0;
                malfbot.common_hit = RobotsCommonAiHitRuntimeState::default();
                malfbot.clear_hit_queries();
                malfbot.animation.setup_idle();
            }
            if let Some(rollerbot) = self.native_rollerbot_runtime.get_mut(&key) {
                rollerbot.active_node = None;
                rollerbot.attack = RobotsGenericAttackRuntimeState::default();
                rollerbot.hit = RobotsCommonAiHitRuntimeState::default();
                rollerbot.animation.setup_idle();
            }
            if let Some(physics) = self.native_monster_physics.get_mut(&key) {
                physics.service_attack_bit4(false);
            }
        }

        let (events, death_position) = {
            let Some(runtime) = self.native_ai_fatal_runtime.get_mut(&key) else {
                return true;
            };
            let yaw_delta = shortest_yaw_delta(owner_yaw_radians, runtime.target_yaw_radians);
            let max_yaw_per_tick = runtime.turn_rate_radians_per_second * ROBOTS_FIXED_STEP_SECONDS;
            let owner_yaw_write = (yaw_delta.abs() > ROBOTS_AI_HIT_YAW_EPSILON).then_some(
                owner_yaw_radians + yaw_delta.clamp(-max_yaw_per_tick, max_yaw_per_tick),
            );
            let Some(body) = self.runtime_character_bodies.get_mut(&key) else {
                return true;
            };
            let events = runtime.animation.advance(
                body,
                runtime.anim_mode,
                owner_yaw_write,
                ROBOTS_FIXED_STEP_SECONDS,
                NativeAiRootMotionPolicy::TRANSLATION_ONLY,
            );
            (events, body.owner_position)
        };

        for event in events {
            if event.event_type != event_type::MONSTER_EXPLOSION {
                continue;
            }
            let selector = event.as_view().native_args::<4>()[0].unwrap_or(0);
            let modulus = robots_monster_explosion_action_modulus(selector);
            let probability_passed = ai_process_lcg_modulo_zero_gate(
                self.native_script_trigger_lifecycle_valid,
                &mut self.native_process_lcg_seed,
                &mut self.native_ai_preview_process_lcg_seed,
                modulus,
            ) == Some(true);
            let action_succeeded = probability_passed
                && self.queue_native_common_monster_explosion(key, visual, death_position);
            if action_succeeded {
                if let Some(runtime) = self.native_ai_fatal_runtime.get_mut(&key) {
                    let _ = runtime.fatal.apply_monster_explosion_event(true);
                }
            }
        }

        // Preserve the established Event-before-node-update ordering used by the
        // existing HitFatal runtime: a successful MonsterExplosion crossing arms
        // the 0.1s fade before this fixed update's 0x00458060 step.
        let Some(plan) = self
            .native_ai_fatal_runtime
            .get_mut(&key)
            .map(|runtime| runtime.fatal.advance_fixed_tick_plan(false))
        else {
            return true;
        };
        if let Some(opacity) = plan.opacity_write {
            if let Some(lifecycle) = self.native_ai_trigger_lifecycle.get_mut(&key) {
                lifecycle.current_opacity = opacity;
            }
        }
        if plan.request_destroy {
            self.request_native_ai_natural_death(map_hash, key, death_position);
        }
        true
    }

    fn advance_native_common_fatal_ai_fixed(&mut self, map: &ProcessedMap) {
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            let Some(visual) = trigger.character_visual.as_ref() else {
                continue;
            };
            if !visual.handler_class.has_proven_common_fatal_node() {
                continue;
            }
            let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
            if !self.runtime_ai_character_xitem_live(map.hashcode, trigger_index) {
                self.native_ai_fatal_runtime.remove(&key);
                continue;
            }
            let Some(body) = self.runtime_character_bodies.get(&key) else {
                continue;
            };
            let owner_yaw_radians = horizontal_yaw(body.owner_rotation);
            let _ = self.advance_native_common_fatal_fixed(
                map.hashcode,
                visual,
                key,
                owner_yaw_radians,
            );
        }
    }

    fn advance_native_dynamic_common_fatal_ai_fixed(&mut self, map: &ProcessedMap) {
        for snapshot in self.runtime_dynamic_ai_snapshots(map) {
            if snapshot.pending_destroy {
                self.native_ai_fatal_runtime.remove(&snapshot.key);
                continue;
            }
            let Some(visual) = map
                .runtime_character_visuals
                .get(&(ROBOTS_SWEEPER_MONSTER_RUNTIME_TYPE, snapshot.selector))
            else {
                continue;
            };
            if !visual.handler_class.has_proven_common_fatal_node() {
                continue;
            }
            let Some(body) = self.runtime_character_bodies.get(&snapshot.key) else {
                continue;
            };
            let owner_yaw_radians = horizontal_yaw(body.owner_rotation);
            let _ = self.advance_native_common_fatal_fixed(
                map.hashcode,
                visual,
                snapshot.key,
                owner_yaw_radians,
            );
        }
    }

    fn apply_native_common_monster_nav_constraint(
        &mut self,
        key: u64,
        nav_regions: &[RobotsMonsterNavMeshView<'_>],
        allow_cross_group: bool,
        handler_flags_628: u32,
    ) -> Option<NativeMonsterNavConstraintHostStep> {
        let pre_constraint_owner_position = self.runtime_character_bodies.get(&key)?.owner_position;
        self.native_monster_physics
            .entry(key)
            .or_insert_with(RobotsCharacterPhysicsRuntimeState::monster_ctor_default);

        if nav_regions.is_empty() {
            if let Some(physics) = self.native_monster_physics.get_mut(&key) {
                physics.service_monster_navmesh_flag_14c(true, false, handler_flags_628);
            }
            return Some(NativeMonsterNavConstraintHostStep {
                pre_constraint_owner_position,
                ..Default::default()
            });
        }

        let nav_state = self.native_monster_navigation.entry(key).or_default();
        let _ = nav_state.maintain(nav_regions, pre_constraint_owner_position.to_array());
        let constraint = self.native_monster_navigation.get(&key).and_then(|state| {
            let region_ordinal = state.region_ordinal?;
            let current_face = state.face_index?;
            let nav = nav_regions.get(region_ordinal).copied()?;
            nav.constrain_owner_to_navmesh(
                pre_constraint_owner_position.to_array(),
                current_face,
                ROBOTS_MONSTER_NAV_BOUNDARY_PROBE_RADIUS,
                allow_cross_group,
            )
            .map(|result| (region_ordinal, nav, result))
        });

        let Some((region_ordinal, nav, result)) = constraint else {
            if let Some(physics) = self.native_monster_physics.get_mut(&key) {
                physics.service_monster_navmesh_flag_14c(true, false, handler_flags_628);
            }
            return Some(NativeMonsterNavConstraintHostStep {
                pre_constraint_owner_position,
                ..Default::default()
            });
        };
        if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
            body.owner_position = Vec3::from_array(result.owner_xyz);
        }
        if let Some(nav_state) = self.native_monster_navigation.get_mut(&key) {
            nav_state.region_ordinal = Some(region_ordinal);
            nav_state.face_index = Some(result.face_index);
            nav_state.group_flags0 = nav.group_flags0_for_face(result.face_index);
        }
        if let Some(physics) = self.native_monster_physics.get_mut(&key) {
            if result.floor_clamped {
                physics.velocity_xyz[1] = physics.velocity_xyz[1].max(0.0);
            }
            physics.service_monster_navmesh_flag_14c(true, true, handler_flags_628);
        }
        Some(NativeMonsterNavConstraintHostStep {
            pre_constraint_owner_position,
            correction_xz: [result.correction_xyz[0], result.correction_xyz[2]],
        })
    }

    fn advance_native_dogbot_fixed(
        &mut self,
        map: &ProcessedMap,
        nav_regions: &[RobotsMonsterNavMeshView<'_>],
        player_position: Vec3,
        wall_time: f64,
    ) {
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            if !is_dogbot(trigger) {
                continue;
            }

            let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
            if !self.runtime_ai_character_xitem_live(map.hashcode, trigger_index) {
                self.native_dogbot_runtime.remove(&key);
                self.native_monster_navigation.remove(&key);
                self.native_monster_physics.remove(&key);
                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                    body.clear_collision_animation_track();
                }
                continue;
            }
            let Some(visual) = trigger.character_visual.as_ref() else {
                continue;
            };
            let Some(body) = self.runtime_character_bodies.get(&key) else {
                self.native_dogbot_runtime.remove(&key);
                self.native_monster_navigation.remove(&key);
                self.native_monster_physics.remove(&key);
                continue;
            };
            let owner_rotation = body.owner_rotation;
            let allow_cross_group = trigger
                .data
                .get(7)
                .and_then(|value| *value)
                .is_some_and(|flags| flags & 0x8000 != 0);
            let Some(_) = self.apply_native_common_monster_nav_constraint(
                key,
                nav_regions,
                allow_cross_group,
                visual.handler_flags_628,
            ) else {
                continue;
            };
            let Some(body) = self.runtime_character_bodies.get(&key) else {
                continue;
            };
            let owner_position = body.owner_position;
            let owner_yaw_radians = horizontal_yaw(owner_rotation);
            let nav_context = self.native_monster_navigation.get(&key).and_then(|state| {
                let region = nav_regions.get(state.region_ordinal?).copied()?;
                Some((region, state.face_index?))
            });

            // Handler +0x34 services already-enqueued HitCheck records before the
            // behavior/AnimScript pass that may enqueue a fresh one this tick.
            let active_query = self.native_dogbot_runtime.get(&key).and_then(|runtime| {
                runtime.hit_query.map(|query| {
                    (
                        query,
                        runtime.animation.sampled_pose_seconds(),
                        runtime.animation.is_mode(ROBOTS_DOGBOT_ATTACK_ANIM_MODE),
                    )
                })
            });
            if let Some((mut query, pose_seconds, attack_pose_owned)) = active_query {
                let source_shape = attack_pose_owned
                    .then(|| {
                        runtime_character_animation_datum_world_shape(
                            owner_position,
                            owner_rotation,
                            body.native_transform_scale_xyz(),
                            visual,
                            ROBOTS_DOGBOT_ATTACK_ANIM_MODE,
                            query.selector,
                            pose_seconds,
                        )
                    })
                    .flatten();
                let hit = source_shape.is_some_and(|shape| {
                    self.native_hit_shape_hits_player(
                        map,
                        shape,
                        RobotsHitQueryCandidateContext {
                            flags: query.flags,
                            query_serial: query.serial,
                            source_raw_group: Some(ROBOTS_HIT_QUERY_RAW_GROUP1),
                            secondary_source_raw_group: None,
                        },
                    )
                });
                let _ = query.step(source_shape.is_some(), hit, 0, 1.0);
                if let Some(runtime) = self.native_dogbot_runtime.get_mut(&key) {
                    runtime.hit_query = query.is_active().then_some(query);
                }
            }

            if self.native_ai_normal_fatal_condition(key) {
                continue;
            }
            let player_state = self.native_player_focus_runtime.player_state;
            let Some(player_position) = self.native_ai_gameplay_target_position(player_position)
            else {
                continue;
            };
            let target_visible = self
                .runtime_map_script_line_of_sight_state(
                    map,
                    owner_position,
                    player_position,
                    wall_time,
                )
                .is_some_and(|state| state.0);
            let top_game_state = self
                .native_cutscene_host_runtime
                .game_state_stack
                .last()
                .copied();
            let class_attack_allowed = robots_common_monster_attack_allowed(
                visual.handler_flags_628,
                top_game_state,
                player_state,
                true,
                self.native_player_hit_runtime.reaction_window,
                self.native_monster_attack_cooldown,
            );
            let attack_eligible = dogbot_attack_gate(RobotsDogBotAttackInput {
                owner_position_xyz: owner_position.to_array(),
                owner_yaw_radians,
                target_position_xyz: player_position.to_array(),
                target_visible,
                class_attack_allowed,
            });

            let entered_attack;
            let mut pending_hit_query = None::<RobotsHitQueryInitPlan>;
            {
                let runtime = self.native_dogbot_runtime.entry(key).or_default();
                let move_mode_active_on_entry = runtime.animation.is_mode(ROBOTS_ANIM_MODE_MOVE);
                let pursue_input = nav_context.map(|(_, current_face)| RobotsPursueNavMeshInput {
                    owner_position_xyz: owner_position.to_array(),
                    owner_yaw_radians,
                    current_face,
                    target_position_xyz: player_position.to_array(),
                    handler_flags_628: visual.handler_flags_628,
                    move_mode_active_on_entry,
                    runtime_rate_scale: 1.0,
                    allow_cross_group,
                    stop_distance: ROBOTS_DOGBOT_PURSUE_NAV_STOP_DISTANCE,
                });
                let pursue_ready =
                    nav_context
                        .zip(pursue_input)
                        .is_some_and(|((nav, _), input)| {
                            runtime.pursue_navmesh.refresh_navigation(nav, input)
                        });

                // Native selector priority is Attack2 0x32 over PursueNavMesh 0x1F.
                // The pursuit service/pre-pass above still runs independently, as
                // 0x00457110 does before the active node executes.
                let attack_step = step_dogbot_attack(&mut runtime.attack_state, attack_eligible);
                entered_attack = attack_step.entered;
                if let Some(requested_anim_mode) = attack_step.requested_anim_mode {
                    // Attack2 outranks PursueNavMesh. Native selector Leave clears
                    // inherited BehaviorNode+0x09 before the attack executes.
                    if runtime.pursue_navmesh.active {
                        runtime.pursue_navmesh.leave();
                    }
                    let events = if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                        runtime.animation.advance(
                            body,
                            requested_anim_mode,
                            None,
                            ROBOTS_FIXED_STEP_SECONDS,
                            NativeAiRootMotionPolicy::TRANSLATION_ONLY,
                        )
                    } else {
                        Vec::new()
                    };
                    for event in events {
                        match event.event_type {
                            event_type::SETUP_IDLE => {
                                dogbot_attack_setup_idle(&mut runtime.attack_state);
                                runtime.animation.setup_idle();
                            }
                            event_type::HIT_CHECK => {
                                pending_hit_query =
                                    RobotsHitQueryInitPlan::from_event(event.as_view());
                            }
                            _ => {}
                        }
                    }
                } else if pursue_ready {
                    if !runtime.pursue_navmesh.active {
                        runtime.pursue_navmesh.enter(None);
                    }
                    if let (Some((nav, _)), Some(input)) = (nav_context, pursue_input) {
                        let pursue_step = runtime.pursue_navmesh.step_execute(nav, input);
                        if let Some(locomotion) = pursue_step.locomotion {
                            let requested_anim_mode = locomotion.requested_anim_mode;
                            let owner_yaw_write = locomotion
                                .direct_owner_yaw_write
                                .then_some(locomotion.owner_yaw_radians);
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                let _ = runtime.animation.advance(
                                    body,
                                    requested_anim_mode,
                                    owner_yaw_write,
                                    ROBOTS_FIXED_STEP_SECONDS,
                                    locomotion_root_motion_policy(locomotion),
                                );
                            }
                        }
                    }
                } else if runtime.pursue_navmesh.active {
                    runtime.pursue_navmesh.leave();
                }
            }
            if entered_attack {
                self.native_monster_attack_cooldown.record_attack();
            }
            if let Some(plan) = pending_hit_query {
                let serial = self.allocate_native_hit_query_serial();
                if let Some(runtime) = self.native_dogbot_runtime.get_mut(&key) {
                    runtime.hit_query = Some(plan.instantiate(true, false, serial));
                }
            }
        }
    }

    fn advance_native_turret_fixed(
        &mut self,
        map: &ProcessedMap,
        player_position: Vec3,
        wall_time: f64,
    ) {
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            let Some(visual) = trigger.character_visual.as_ref() else {
                continue;
            };
            if !matches!(
                visual.handler_class,
                RobotsAiHandlerClass::Ep02Turret | RobotsAiHandlerClass::Ep04Turret
            ) {
                continue;
            }
            let is_ep04 = visual.handler_class == RobotsAiHandlerClass::Ep04Turret;

            let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
            if !self.runtime_ai_character_xitem_live(map.hashcode, trigger_index) {
                self.native_turret_runtime.remove(&key);
                self.native_monster_physics.remove(&key);
                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                    body.clear_collision_animation_track();
                }
                continue;
            }
            let Some(body) = self.runtime_character_bodies.get(&key) else {
                self.native_turret_runtime.remove(&key);
                self.native_monster_physics.remove(&key);
                continue;
            };
            let owner_position = body.owner_position;
            let owner_rotation = body.owner_rotation;
            let owner_scale = body.native_transform_scale_xyz();
            let runtime_is_new = !self.native_turret_runtime.contains_key(&key);
            if runtime_is_new && !is_ep04 {
                // EP02 first-update `0x00460E10` calls both common Physics writers
                // `0x004535D0(1)` and `0x00453600(1)`. EP04 has a distinct
                // class init path; do not inherit this write without native proof.
                self.native_monster_physics
                    .entry(key)
                    .or_insert_with(RobotsCharacterPhysicsRuntimeState::monster_ctor_default)
                    .service_attack_bit4(true);
            }
            self.native_turret_runtime.entry(key).or_default();

            if self.native_ai_normal_fatal_condition(key) {
                continue;
            }
            let player_state = self.native_player_focus_runtime.player_state;
            let Some(player_position) = self.native_ai_gameplay_target_position(player_position)
            else {
                continue;
            };

            let target_visible = self
                .runtime_map_script_line_of_sight_state(
                    map,
                    owner_position,
                    player_position,
                    wall_time,
                )
                .is_some_and(|state| state.0);
            let top_game_state = self
                .native_cutscene_host_runtime
                .game_state_stack
                .last()
                .copied();
            let class_attack_allowed = robots_common_monster_attack_allowed(
                visual.handler_flags_628,
                top_game_state,
                player_state,
                true,
                self.native_player_hit_runtime.reaction_window,
                self.native_monster_attack_cooldown,
            );
            let hit_snapshot = self.native_ai_hit_reactions.get(&key).copied();
            let (mut got_hit_latch, query_flags_snapshot) = hit_snapshot
                .map(|hit| (hit.got_hit_latch, hit.query_flags_snapshot))
                .unwrap_or((false, 0));

            let (attack_priority, primary_winner, secondary_winner, previous_primary) = {
                let runtime = self.native_turret_runtime.get(&key).unwrap();
                let gate = RobotsGenericAttackGateInput {
                    owner_position_xyz: owner_position.to_array(),
                    owner_yaw_radians: runtime.aim.yaw_radians,
                    target_position_xyz: player_position.to_array(),
                    target_visible,
                    class_attack_allowed,
                };
                let attack_priority = if is_ep04 {
                    generic_attack_priority(runtime.generic_attack, ep04_attack_config(), gate)
                } else {
                    pitched_turret_attack_priority(runtime.pitched_attack, gate)
                };
                let group_priority =
                    generic_attack_group_priority(&runtime.attack_group, &[attack_priority]);
                let primary_scrambled_priority = runtime
                    .primary_scrambled
                    .priority(got_hit_latch, query_flags_snapshot);
                let secondary_scrambled_priority = runtime
                    .secondary_scrambled
                    .priority(got_hit_latch, query_flags_snapshot);
                let primary_winner = if is_ep04 {
                    let common_hit_priority = hit_snapshot
                        .map(|hit| runtime.common_hit.priority(hit))
                        .unwrap_or(1);
                    ep04_primary_winner(
                        group_priority,
                        common_hit_priority,
                        primary_scrambled_priority,
                    )
                    .map(|winner| match winner {
                        RobotsEp04PrimaryWinner::AttackGroup => {
                            NativeTurretPrimaryWinner::Ep04AttackGroup
                        }
                        RobotsEp04PrimaryWinner::CommonHit => {
                            NativeTurretPrimaryWinner::Ep04CommonHit
                        }
                        RobotsEp04PrimaryWinner::ScrambledHit => {
                            NativeTurretPrimaryWinner::ScrambledHit
                        }
                    })
                } else {
                    ep02_primary_winner(group_priority, primary_scrambled_priority).map(|winner| {
                        match winner {
                            RobotsEp02PrimaryWinner::PitchedAttackGroup => {
                                NativeTurretPrimaryWinner::Ep02PitchedAttackGroup
                            }
                            RobotsEp02PrimaryWinner::ScrambledHit => {
                                NativeTurretPrimaryWinner::ScrambledHit
                            }
                        }
                    })
                };
                (
                    attack_priority,
                    primary_winner,
                    ep02_secondary_winner(track_target_priority(), secondary_scrambled_priority),
                    runtime.primary_active,
                )
            };
            let attack_group_enter_draw = (matches!(
                primary_winner,
                Some(
                    NativeTurretPrimaryWinner::Ep02PitchedAttackGroup
                        | NativeTurretPrimaryWinner::Ep04AttackGroup
                )
            ) && previous_primary != primary_winner)
                .then(|| self.next_native_ai_gameplay_rng_u32())
                .flatten();

            let mut runtime = self.native_turret_runtime.remove(&key).unwrap_or_default();
            let mut projectile_events =
                Vec::<(RobotsCreateProjectileRequest, f32, Vec3, Quat)>::new();
            let mut entered_attack = false;

            let mut committed_primary = primary_winner;
            if runtime.primary_active != primary_winner {
                match runtime.primary_active {
                    Some(NativeTurretPrimaryWinner::Ep02PitchedAttackGroup) => {
                        leave_pitched_turret_attack(&mut runtime.pitched_attack, &mut runtime.aim);
                        leave_generic_attack_group(&mut runtime.attack_group);
                    }
                    Some(NativeTurretPrimaryWinner::Ep04AttackGroup) => {
                        leave_generic_attack(&mut runtime.generic_attack);
                        leave_generic_attack_group(&mut runtime.attack_group);
                    }
                    Some(NativeTurretPrimaryWinner::Ep04CommonHit) => {
                        runtime.common_hit.active = false;
                    }
                    Some(NativeTurretPrimaryWinner::ScrambledHit) => {
                        runtime.primary_scrambled.leave();
                    }
                    None => {}
                }
                match primary_winner {
                    Some(NativeTurretPrimaryWinner::Ep02PitchedAttackGroup) => {
                        if enter_generic_attack_group(
                            &mut runtime.attack_group,
                            &[attack_priority],
                            attack_group_enter_draw,
                        ) == Some(0)
                        {
                            enter_pitched_turret_attack(&mut runtime.pitched_attack);
                            entered_attack = true;
                        } else {
                            committed_primary = None;
                        }
                    }
                    Some(NativeTurretPrimaryWinner::Ep04AttackGroup) => {
                        if enter_generic_attack_group(
                            &mut runtime.attack_group,
                            &[attack_priority],
                            attack_group_enter_draw,
                        ) == Some(0)
                        {
                            enter_generic_attack(&mut runtime.generic_attack, ep04_attack_config());
                            entered_attack = true;
                        } else {
                            committed_primary = None;
                        }
                    }
                    Some(NativeTurretPrimaryWinner::Ep04CommonHit) => {
                        if let (Some(source_yaw), Some(hit)) = (
                            self.native_ai_last_hit_source_yaw.get(&key).copied(),
                            hit_snapshot,
                        ) {
                            runtime.common_hit.enter_configured(
                                RobotsCommonAiHitConfig::ep04_turret(),
                                horizontal_yaw(owner_rotation),
                                source_yaw,
                                hit.last_query_serial,
                            );
                        } else {
                            committed_primary = None;
                        }
                    }
                    Some(NativeTurretPrimaryWinner::ScrambledHit) => {
                        runtime.primary_scrambled.enter();
                    }
                    None => {}
                }
                runtime.primary_active = committed_primary;
            }

            match runtime.primary_active {
                Some(NativeTurretPrimaryWinner::Ep02PitchedAttackGroup) => {
                    if runtime.attack_group.selected_index == Some(0) {
                        let attack_step = step_pitched_turret_attack(
                            &mut runtime.pitched_attack,
                            &mut runtime.aim,
                        );
                        if attack_step.requested_anim_mode == ROBOTS_EP02_ATTACK_ANIM_MODE {
                            let events =
                                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                    runtime.primary_animation.advance(
                                        body,
                                        ROBOTS_EP02_ATTACK_ANIM_MODE,
                                        None,
                                        ROBOTS_FIXED_STEP_SECONDS,
                                        NativeAiRootMotionPolicy::NONE,
                                    )
                                } else {
                                    Vec::new()
                                };
                            for event in events {
                                match event.event_type {
                                    event_type::SETUP_IDLE => {
                                        pitched_turret_attack_setup_idle(
                                            &mut runtime.pitched_attack,
                                        );
                                        runtime.primary_animation.setup_idle();
                                    }
                                    event_type::CREATE_PROJECTILE => {
                                        if let Some(request) =
                                            RobotsCreateProjectileRequest::from_event(
                                                event.as_view(),
                                            )
                                        {
                                            projectile_events.push((
                                                request,
                                                event.pose_seconds,
                                                event.owner_position,
                                                event.owner_rotation,
                                            ));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        } else if let Some(body) = self.runtime_character_bodies.get(&key) {
                            let _ = runtime.primary_animation.advance_auxiliary_script_channel(
                                body,
                                attack_step.requested_anim_mode,
                                ROBOTS_FIXED_STEP_SECONDS,
                            );
                        }
                    }
                }
                Some(NativeTurretPrimaryWinner::Ep04AttackGroup) => {
                    if runtime.attack_group.selected_index == Some(0) {
                        let attack_step =
                            step_generic_attack(&mut runtime.generic_attack, ep04_attack_config());
                        if let Some(mode) = attack_step.requested_anim_mode {
                            let events =
                                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                    runtime.primary_animation.advance(
                                        body,
                                        mode,
                                        None,
                                        ROBOTS_FIXED_STEP_SECONDS,
                                        NativeAiRootMotionPolicy::NONE,
                                    )
                                } else {
                                    Vec::new()
                                };
                            for event in events {
                                match event.event_type {
                                    event_type::SETUP_IDLE => {
                                        generic_attack_setup_idle(
                                            &mut runtime.generic_attack,
                                            ep04_attack_config(),
                                        );
                                        runtime.primary_animation.setup_idle();
                                    }
                                    event_type::CREATE_PROJECTILE => {
                                        if let Some(request) =
                                            RobotsCreateProjectileRequest::from_event(
                                                event.as_view(),
                                            )
                                        {
                                            projectile_events.push((
                                                request,
                                                event.pose_seconds,
                                                event.owner_position,
                                                event.owner_rotation,
                                            ));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                Some(NativeTurretPrimaryWinner::Ep04CommonHit) => {
                    let hit_config = RobotsCommonAiHitConfig::ep04_turret();
                    let reenter = hit_snapshot.is_some_and(|hit| {
                        hit.health != 0
                            && hit.query_serial_snapshot != u16::MAX
                            && hit.last_query_serial != runtime.common_hit.captured_query_serial
                    });
                    if reenter {
                        if let (Some(source_yaw), Some(hit)) = (
                            self.native_ai_last_hit_source_yaw.get(&key).copied(),
                            hit_snapshot,
                        ) {
                            runtime.primary_animation.setup_idle();
                            runtime.common_hit.enter_configured(
                                hit_config,
                                horizontal_yaw(owner_rotation),
                                source_yaw,
                                hit.last_query_serial,
                            );
                        }
                    }
                    let owner_yaw_write = runtime
                        .common_hit
                        .step_owner_yaw_configured(hit_config, horizontal_yaw(owner_rotation));
                    let events = if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                        runtime.primary_animation.advance(
                            body,
                            runtime.common_hit.requested_anim_mode,
                            owner_yaw_write,
                            ROBOTS_FIXED_STEP_SECONDS,
                            NativeAiRootMotionPolicy::NONE,
                        )
                    } else {
                        Vec::new()
                    };
                    if events
                        .iter()
                        .any(|event| event.event_type == event_type::SETUP_IDLE)
                    {
                        if let Some(hit) = self.native_ai_hit_reactions.get_mut(&key) {
                            runtime.common_hit.setup_idle(hit);
                            got_hit_latch = hit.got_hit_latch;
                        }
                        runtime.primary_animation.setup_idle();
                    }
                }
                Some(NativeTurretPrimaryWinner::ScrambledHit) => {
                    let mode = runtime.primary_scrambled.step(&mut got_hit_latch);
                    if let Some(body) = self.runtime_character_bodies.get(&key) {
                        let _ = runtime.primary_animation.advance_auxiliary_script_channel(
                            body,
                            mode,
                            ROBOTS_FIXED_STEP_SECONDS,
                        );
                    }
                }
                None => {}
            }
            // BehaviorHost `0x004570C0` ticks every child after active Execute.
            if is_ep04 {
                tick_generic_attack(&mut runtime.generic_attack);
            } else {
                tick_pitched_turret_attack(&mut runtime.pitched_attack);
            }

            if runtime.secondary_active != secondary_winner {
                if runtime.secondary_active == Some(RobotsEp02SecondaryWinner::TrackTarget) {
                    runtime.aim.tracking_latch = false;
                } else if runtime.secondary_active == Some(RobotsEp02SecondaryWinner::ScrambledHit)
                {
                    runtime.secondary_scrambled.leave();
                }
                if secondary_winner == Some(RobotsEp02SecondaryWinner::TrackTarget) {
                    runtime.aim.tracking_latch = true;
                } else if secondary_winner == Some(RobotsEp02SecondaryWinner::ScrambledHit) {
                    runtime.secondary_scrambled.enter();
                }
                runtime.secondary_active = secondary_winner;
            }

            match runtime.secondary_active {
                Some(RobotsEp02SecondaryWinner::TrackTarget) => {
                    let target = player_position - owner_position;
                    let target_yaw = target.x.atan2(target.z);
                    let yaw_step = step_turret_yaw_toward(
                        &mut runtime.aim,
                        horizontal_yaw(owner_rotation),
                        target_yaw,
                    );
                    if let Some(body) = self.runtime_character_bodies.get(&key) {
                        let _ = runtime
                            .secondary_animation
                            .advance_auxiliary_script_channel(
                                body,
                                yaw_step.requested_anim_mode,
                                ROBOTS_FIXED_STEP_SECONDS,
                            );
                    }
                }
                Some(RobotsEp02SecondaryWinner::ScrambledHit) => {
                    let mode = runtime.secondary_scrambled.step(&mut got_hit_latch);
                    if let Some(body) = self.runtime_character_bodies.get(&key) {
                        let _ = runtime
                            .secondary_animation
                            .advance_auxiliary_script_channel(
                                body,
                                mode,
                                ROBOTS_FIXED_STEP_SECONDS,
                            );
                    }
                }
                None => {}
            }

            if let Some(hit_state) = self.native_ai_hit_reactions.get_mut(&key) {
                hit_state.got_hit_latch = got_hit_latch;
            }
            self.native_turret_runtime.insert(key, runtime);

            if entered_attack {
                self.native_monster_attack_cooldown.record_attack();
            }
            let attack_anim_mode = if is_ep04 {
                ROBOTS_EP04_ATTACK_ANIM_MODE
            } else {
                ROBOTS_EP02_ATTACK_ANIM_MODE
            };
            for (request, pose_seconds, event_owner_position, event_owner_rotation) in
                projectile_events
            {
                let query_serial = self.allocate_native_hit_query_serial();
                if let Some(plan) = resolve_ai_projectile_spawn_plan(
                    map,
                    key,
                    event_owner_position,
                    event_owner_rotation,
                    owner_scale,
                    visual,
                    attack_anim_mode,
                    request,
                    query_serial,
                    pose_seconds,
                    player_position,
                ) {
                    self.spawn_native_ai_projectile(plan);
                }
            }
        }
    }

    fn advance_native_ep05_turret_fixed(
        &mut self,
        map: &ProcessedMap,
        player_position: Vec3,
        wall_time: f64,
    ) {
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            let Some(visual) = trigger.character_visual.as_ref() else {
                continue;
            };
            if visual.handler_class != RobotsAiHandlerClass::Ep05Turret {
                continue;
            }

            let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
            if !self.runtime_ai_character_xitem_live(map.hashcode, trigger_index) {
                self.native_ep05_turret_runtime.remove(&key);
                self.native_monster_physics.remove(&key);
                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                    body.clear_collision_animation_track();
                }
                continue;
            }
            let Some(body) = self.runtime_character_bodies.get(&key) else {
                self.native_ep05_turret_runtime.remove(&key);
                self.native_monster_physics.remove(&key);
                continue;
            };
            let owner_position = body.owner_position;
            let owner_rotation = body.owner_rotation;
            let owner_scale = body.native_transform_scale_xyz();
            let owner_yaw_radians = horizontal_yaw(owner_rotation);

            self.native_ep05_turret_runtime.entry(key).or_default();
            if self.native_ai_normal_fatal_condition(key) {
                continue;
            }

            let gameplay_target = self.native_ai_gameplay_target_position(player_position);
            let target_distance_squared =
                gameplay_target.map(|target| (target - owner_position).length_squared());
            let target_visible = gameplay_target.is_some_and(|target| {
                self.runtime_map_script_line_of_sight_state(map, owner_position, target, wall_time)
                    .is_some_and(|state| state.0)
            });
            let player_state = self.native_player_focus_runtime.player_state;
            let top_game_state = self
                .native_cutscene_host_runtime
                .game_state_stack
                .last()
                .copied();
            let class_attack_allowed = robots_common_monster_attack_allowed(
                visual.handler_flags_628,
                top_game_state,
                player_state,
                true,
                self.native_player_hit_runtime.reaction_window,
                self.native_monster_attack_cooldown,
            );

            let mut runtime = self
                .native_ep05_turret_runtime
                .remove(&key)
                .unwrap_or_default();
            let mut projectile_events =
                Vec::<(RobotsCreateProjectileRequest, f32, Vec3, Quat)>::new();
            let mut entered_attack = false;

            // Native 0x004517A0 services BehaviorHost +0x4C0 before +0x4D8.
            // EP05 shares Handler+0x64C between both, so the second host must see
            // latch writes made by the first one in this same fixed update.
            let distance_priority = ep05_distance_deactivate_priority(target_distance_squared);
            let exit_priority = ep05_exit_priority(runtime.aim.tracking_latch);
            let host_4c4_winner = ep05_activation_winner(distance_priority, exit_priority);
            runtime.host_4c4_active = host_4c4_winner;

            let mut host_4c4_mode = None;
            match host_4c4_winner {
                Some(RobotsEp05ActivationWinner::TrackTarget) => {
                    if let Some(target) = gameplay_target {
                        let delta = target - owner_position;
                        let target_yaw = delta.x.atan2(delta.z);
                        let yaw_step = step_ep05_turret_yaw_toward(
                            &mut runtime.aim,
                            owner_yaw_radians,
                            target_yaw,
                        );
                        runtime.yaw_sound_active = yaw_step.movement_sound_active;
                    }
                }
                Some(RobotsEp05ActivationWinner::DeactivateIdle) => {
                    if runtime.aim.yaw_radians <= ROBOTS_TURRET_ANGLE_EPSILON {
                        // EP05 +0x16C 0x00461930: latch on and stop its motor sound.
                        runtime.aim.tracking_latch = true;
                        runtime.yaw_sound_active = false;
                        host_4c4_mode = Some(ROBOTS_EP05_DEACTIVATE_IDLE_ANIM_MODE);
                    } else {
                        let yaw_step = step_ep05_turret_yaw_toward(
                            &mut runtime.aim,
                            owner_yaw_radians,
                            owner_yaw_radians,
                        );
                        runtime.yaw_sound_active = yaw_step.movement_sound_active;
                    }
                }
                Some(RobotsEp05ActivationWinner::DeactivateExit) => {
                    host_4c4_mode = Some(ROBOTS_EP05_DEACTIVATE_EXIT_ANIM_MODE);
                }
                None => {}
            }
            if let Some(mode) = host_4c4_mode {
                let events = if let Some(body) = self.runtime_character_bodies.get(&key) {
                    runtime.host_4c4_animation.advance_auxiliary_script_channel(
                        body,
                        mode,
                        ROBOTS_FIXED_STEP_SECONDS,
                    )
                } else {
                    Vec::new()
                };
                if host_4c4_winner == Some(RobotsEp05ActivationWinner::DeactivateExit)
                    && events
                        .iter()
                        .any(|event| event.event_type == event_type::SETUP_IDLE)
                {
                    // Exit event 0x0046E560 -> handler +0x170 0x00459F00.
                    runtime.aim.tracking_latch = false;
                    runtime.host_4c4_animation.setup_idle();
                }
            }

            let distance_priority = ep05_distance_deactivate_priority(target_distance_squared);
            let exit_priority = ep05_exit_priority(runtime.aim.tracking_latch);
            let attack_priority = gameplay_target
                .map(|target| {
                    generic_attack_priority(
                        runtime.attack,
                        ep05_attack_config(),
                        RobotsGenericAttackGateInput {
                            owner_position_xyz: owner_position.to_array(),
                            owner_yaw_radians: runtime.aim.yaw_radians,
                            target_position_xyz: target.to_array(),
                            target_visible,
                            class_attack_allowed,
                        },
                    )
                })
                .unwrap_or(1);
            let host_4dc_winner =
                ep05_combat_winner(distance_priority, exit_priority, attack_priority);
            let attack_completed_reentry = runtime.host_4dc_active
                == Some(RobotsEp05CombatWinner::Attack)
                && host_4dc_winner == Some(RobotsEp05CombatWinner::Attack)
                && runtime.attack.completion_latch;

            if runtime.host_4dc_active != host_4dc_winner || attack_completed_reentry {
                if runtime.host_4dc_active == Some(RobotsEp05CombatWinner::Attack) {
                    leave_generic_attack(&mut runtime.attack);
                }
                if host_4dc_winner == Some(RobotsEp05CombatWinner::Attack) {
                    enter_generic_attack(&mut runtime.attack, ep05_attack_config());
                    entered_attack = true;
                }
                runtime.host_4dc_active = host_4dc_winner;
            }

            match runtime.host_4dc_active {
                Some(RobotsEp05CombatWinner::IdleAnimator)
                | Some(RobotsEp05CombatWinner::IdleFallback) => {
                    if let Some(body) = self.runtime_character_bodies.get(&key) {
                        let _ = runtime.host_4dc_animation.advance_auxiliary_script_channel(
                            body,
                            ROBOTS_EP05_IDLE_ANIM_MODE,
                            ROBOTS_FIXED_STEP_SECONDS,
                        );
                    }
                }
                Some(RobotsEp05CombatWinner::DeactivateIdle) => {
                    if runtime.aim.yaw_radians <= ROBOTS_TURRET_ANGLE_EPSILON {
                        runtime.aim.tracking_latch = true;
                        runtime.yaw_sound_active = false;
                        if let Some(body) = self.runtime_character_bodies.get(&key) {
                            let _ = runtime.host_4dc_animation.advance_auxiliary_script_channel(
                                body,
                                ROBOTS_EP05_DEACTIVATE_IDLE2_ANIM_MODE,
                                ROBOTS_FIXED_STEP_SECONDS,
                            );
                        }
                    }
                }
                Some(RobotsEp05CombatWinner::DeactivateExit) => {
                    let events = if let Some(body) = self.runtime_character_bodies.get(&key) {
                        runtime.host_4dc_animation.advance_auxiliary_script_channel(
                            body,
                            ROBOTS_EP05_DEACTIVATE_EXIT2_ANIM_MODE,
                            ROBOTS_FIXED_STEP_SECONDS,
                        )
                    } else {
                        Vec::new()
                    };
                    if events
                        .iter()
                        .any(|event| event.event_type == event_type::SETUP_IDLE)
                    {
                        runtime.aim.tracking_latch = false;
                        runtime.host_4dc_animation.setup_idle();
                    }
                }
                Some(RobotsEp05CombatWinner::Attack) => {
                    let attack_step =
                        step_generic_attack(&mut runtime.attack, ep05_attack_config());
                    if let Some(mode) = attack_step.requested_anim_mode {
                        let events = if let Some(body) = self.runtime_character_bodies.get(&key) {
                            runtime.host_4dc_animation.advance_auxiliary_script_channel(
                                body,
                                mode,
                                ROBOTS_FIXED_STEP_SECONDS,
                            )
                        } else {
                            Vec::new()
                        };
                        for event in events {
                            match event.event_type {
                                event_type::SETUP_IDLE => {
                                    generic_attack_setup_idle(
                                        &mut runtime.attack,
                                        ep05_attack_config(),
                                    );
                                    runtime.host_4dc_animation.setup_idle();
                                }
                                event_type::CREATE_PROJECTILE => {
                                    if let Some(request) =
                                        RobotsCreateProjectileRequest::from_event(event.as_view())
                                    {
                                        projectile_events.push((
                                            request,
                                            event.pose_seconds,
                                            event.owner_position,
                                            event.owner_rotation,
                                        ));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                None => {}
            }

            // BehaviorHost 0x004570C0 ticks every child after active Execute.
            tick_generic_attack(&mut runtime.attack);
            self.native_ep05_turret_runtime.insert(key, runtime);

            if entered_attack {
                self.native_monster_attack_cooldown.record_attack();
            }
            if let Some(projectile_target) = gameplay_target {
                for (request, pose_seconds, event_owner_position, event_owner_rotation) in
                    projectile_events
                {
                    let query_serial = self.allocate_native_hit_query_serial();
                    if let Some(plan) = resolve_ai_projectile_spawn_plan(
                        map,
                        key,
                        event_owner_position,
                        event_owner_rotation,
                        owner_scale,
                        visual,
                        ROBOTS_EP05_ATTACK_ANIM_MODE,
                        request,
                        query_serial,
                        pose_seconds,
                        projectile_target,
                    ) {
                        self.spawn_native_ai_projectile(plan);
                    }
                }
            }
        }
    }

    fn advance_native_ep06_turret_fixed(
        &mut self,
        map: &ProcessedMap,
        player_position: Vec3,
        wall_time: f64,
    ) {
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            let Some(visual) = trigger.character_visual.as_ref() else {
                continue;
            };
            if visual.handler_class != RobotsAiHandlerClass::Ep06Turret {
                continue;
            }

            let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
            if !self.runtime_ai_character_xitem_live(map.hashcode, trigger_index) {
                self.native_ep06_turret_runtime.remove(&key);
                self.native_monster_physics.remove(&key);
                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                    body.clear_collision_animation_track();
                }
                continue;
            }
            let Some(body) = self.runtime_character_bodies.get(&key) else {
                self.native_ep06_turret_runtime.remove(&key);
                self.native_monster_physics.remove(&key);
                continue;
            };
            let owner_position = body.owner_position;
            let owner_rotation = body.owner_rotation;
            let owner_scale = body.native_transform_scale_xyz();
            let owner_yaw_radians = horizontal_yaw(owner_rotation);

            self.native_ep06_turret_runtime.entry(key).or_default();
            if self.native_ai_normal_fatal_condition(key) {
                continue;
            }
            let Some(gameplay_target) = self.native_ai_gameplay_target_position(player_position)
            else {
                continue;
            };
            let target_visible = self
                .runtime_map_script_line_of_sight_state(
                    map,
                    owner_position,
                    gameplay_target,
                    wall_time,
                )
                .is_some_and(|state| state.0);
            let player_state = self.native_player_focus_runtime.player_state;
            let top_game_state = self
                .native_cutscene_host_runtime
                .game_state_stack
                .last()
                .copied();

            let mut runtime = self
                .native_ep06_turret_runtime
                .remove(&key)
                .unwrap_or_default();
            let mut projectile_events =
                Vec::<(u32, RobotsCreateProjectileRequest, f32, Vec3, Quat)>::new();
            let hit_snapshot = self.native_ai_hit_reactions.get(&key).copied();
            let (mut got_hit_latch, query_flags_snapshot) = hit_snapshot
                .map(|hit| (hit.got_hit_latch, hit.query_flags_snapshot))
                .unwrap_or((false, 0));
            let mut request_natural_death = false;

            // Native 0x004517A0 services Handler+0x4C0 / vector +0x4C4 first.
            let secondary_config = ep06_secondary_pitched_attack_config();
            let class_attack_allowed = robots_common_monster_attack_allowed(
                visual.handler_flags_628,
                top_game_state,
                player_state,
                true,
                self.native_player_hit_runtime.reaction_window,
                self.native_monster_attack_cooldown,
            );
            let secondary_gate = RobotsGenericAttackGateInput {
                owner_position_xyz: owner_position.to_array(),
                owner_yaw_radians: runtime.aim.yaw_radians,
                target_position_xyz: gameplay_target.to_array(),
                target_visible,
                class_attack_allowed,
            };
            let secondary_attack_priority = pitched_turret_attack_priority_configured(
                runtime.host_4c4_pitched,
                secondary_config,
                secondary_gate,
            );
            let secondary_hit_priority = hit_snapshot
                .map(|hit| runtime.host_4c4_hit.priority(hit))
                .unwrap_or(1);
            let secondary_magnetic_priority = runtime
                .host_4c4_magnetic
                .priority(got_hit_latch, query_flags_snapshot);
            let secondary_scrambled_priority = runtime
                .host_4c4_scrambled
                .priority(got_hit_latch, query_flags_snapshot);
            let secondary_winner = ep06_host_4c4_winner(
                secondary_attack_priority,
                secondary_hit_priority,
                secondary_magnetic_priority,
                secondary_scrambled_priority,
            );

            if runtime.host_4c4_active != secondary_winner {
                match runtime.host_4c4_active {
                    Some(RobotsEp06Host4c4Winner::PitchedAttack) => {
                        leave_pitched_turret_attack_configured(
                            &mut runtime.host_4c4_pitched,
                            &mut runtime.aim,
                            secondary_config,
                        );
                    }
                    Some(RobotsEp06Host4c4Winner::CommonHit) => {
                        runtime.host_4c4_hit.active = false;
                    }
                    Some(RobotsEp06Host4c4Winner::MagneticHit) => {
                        runtime.host_4c4_magnetic.leave();
                    }
                    Some(RobotsEp06Host4c4Winner::ScrambledHit) => {
                        runtime.host_4c4_scrambled.leave();
                    }
                    None => {}
                }

                let mut committed = secondary_winner;
                match secondary_winner {
                    Some(RobotsEp06Host4c4Winner::PitchedAttack) => {
                        enter_pitched_turret_attack_configured(
                            &mut runtime.host_4c4_pitched,
                            secondary_config,
                        );
                        // Claim immediately so Handler+0x4DC sees the same native
                        // global attack suppression in this fixed update.
                        self.native_monster_attack_cooldown.record_attack();
                    }
                    Some(RobotsEp06Host4c4Winner::CommonHit) => {
                        if let (Some(source_yaw), Some(hit)) = (
                            self.native_ai_last_hit_source_yaw.get(&key).copied(),
                            hit_snapshot,
                        ) {
                            runtime.host_4c4_hit.enter_configured(
                                RobotsCommonAiHitConfig::ep06_secondary_turret(),
                                owner_yaw_radians,
                                source_yaw,
                                hit.last_query_serial,
                            );
                        } else {
                            committed = None;
                        }
                    }
                    Some(RobotsEp06Host4c4Winner::MagneticHit) => {
                        runtime.host_4c4_magnetic.enter();
                    }
                    Some(RobotsEp06Host4c4Winner::ScrambledHit) => {
                        runtime.host_4c4_scrambled.enter();
                    }
                    None => {}
                }
                runtime.host_4c4_active = committed;
            }

            match runtime.host_4c4_active {
                Some(RobotsEp06Host4c4Winner::PitchedAttack) => {
                    let step = step_pitched_turret_attack_configured(
                        &mut runtime.host_4c4_pitched,
                        &mut runtime.aim,
                        secondary_config,
                    );
                    if step.requested_anim_mode == ROBOTS_EP06_SECONDARY_ATTACK_ANIM_MODE {
                        let events = if let Some(body) = self.runtime_character_bodies.get_mut(&key)
                        {
                            runtime.host_4c4_animation.advance(
                                body,
                                step.requested_anim_mode,
                                None,
                                ROBOTS_FIXED_STEP_SECONDS,
                                NativeAiRootMotionPolicy::NONE,
                            )
                        } else {
                            Vec::new()
                        };
                        for event in events {
                            match event.event_type {
                                event_type::SETUP_IDLE => {
                                    pitched_turret_attack_setup_idle(&mut runtime.host_4c4_pitched);
                                    runtime.host_4c4_animation.setup_idle();
                                }
                                event_type::CREATE_PROJECTILE => {
                                    if let Some(request) =
                                        RobotsCreateProjectileRequest::from_event(event.as_view())
                                    {
                                        projectile_events.push((
                                            ROBOTS_EP06_SECONDARY_ATTACK_ANIM_MODE,
                                            request,
                                            event.pose_seconds,
                                            event.owner_position,
                                            event.owner_rotation,
                                        ));
                                    }
                                }
                                _ => {}
                            }
                        }
                    } else if let Some(body) = self.runtime_character_bodies.get(&key) {
                        let _ = runtime.host_4c4_animation.advance_auxiliary_script_channel(
                            body,
                            step.requested_anim_mode,
                            ROBOTS_FIXED_STEP_SECONDS,
                        );
                    }
                }
                Some(RobotsEp06Host4c4Winner::CommonHit) => {
                    let hit_config = RobotsCommonAiHitConfig::ep06_secondary_turret();
                    let reenter = hit_snapshot.is_some_and(|hit| {
                        hit.health != 0
                            && hit.query_serial_snapshot != u16::MAX
                            && hit.last_query_serial != runtime.host_4c4_hit.captured_query_serial
                    });
                    if reenter {
                        if let (Some(source_yaw), Some(hit)) = (
                            self.native_ai_last_hit_source_yaw.get(&key).copied(),
                            hit_snapshot,
                        ) {
                            runtime.host_4c4_animation.setup_idle();
                            runtime.host_4c4_hit.enter_configured(
                                hit_config,
                                owner_yaw_radians,
                                source_yaw,
                                hit.last_query_serial,
                            );
                        }
                    }
                    let events = if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                        runtime.host_4c4_animation.advance(
                            body,
                            runtime.host_4c4_hit.requested_anim_mode,
                            None,
                            ROBOTS_FIXED_STEP_SECONDS,
                            NativeAiRootMotionPolicy::NONE,
                        )
                    } else {
                        Vec::new()
                    };
                    if events
                        .iter()
                        .any(|event| event.event_type == event_type::SETUP_IDLE)
                    {
                        if let Some(hit) = self.native_ai_hit_reactions.get_mut(&key) {
                            runtime.host_4c4_hit.setup_idle(hit);
                            got_hit_latch = hit.got_hit_latch;
                        }
                        runtime.host_4c4_animation.setup_idle();
                    }
                }
                Some(RobotsEp06Host4c4Winner::MagneticHit) => {
                    // Derived vtable 0x005E6E28 replaces Execute with
                    // 0x00458450: animation request only, no magnetic physics.
                    let mode = runtime.host_4c4_magnetic.requested_anim_mode();
                    if let Some(body) = self.runtime_character_bodies.get(&key) {
                        let _ = runtime.host_4c4_animation.advance_auxiliary_script_channel(
                            body,
                            mode,
                            ROBOTS_FIXED_STEP_SECONDS,
                        );
                    }
                }
                Some(RobotsEp06Host4c4Winner::ScrambledHit) => {
                    let mode = runtime.host_4c4_scrambled.step(&mut got_hit_latch);
                    if let Some(body) = self.runtime_character_bodies.get(&key) {
                        let _ = runtime.host_4c4_animation.advance_auxiliary_script_channel(
                            body,
                            mode,
                            ROBOTS_FIXED_STEP_SECONDS,
                        );
                    }
                }
                None => {}
            }
            tick_pitched_turret_attack(&mut runtime.host_4c4_pitched);

            // Handler+0x4D8 / vector +0x4DC runs second and observes any attack
            // claim or got-hit mutation produced above.
            let refreshed_hit_snapshot = self.native_ai_hit_reactions.get(&key).copied();
            let refreshed_query_flags = refreshed_hit_snapshot
                .map(|hit| hit.query_flags_snapshot)
                .unwrap_or(query_flags_snapshot);
            if let Some(hit) = refreshed_hit_snapshot {
                got_hit_latch = hit.got_hit_latch;
            }
            let primary_config = ep06_primary_pitched_attack_config();
            let class_attack_allowed = robots_common_monster_attack_allowed(
                visual.handler_flags_628,
                top_game_state,
                player_state,
                true,
                self.native_player_hit_runtime.reaction_window,
                self.native_monster_attack_cooldown,
            );
            let primary_gate = RobotsGenericAttackGateInput {
                owner_position_xyz: owner_position.to_array(),
                owner_yaw_radians: runtime.aim.yaw_radians,
                target_position_xyz: gameplay_target.to_array(),
                target_visible,
                class_attack_allowed,
            };
            let primary_attack_priority = pitched_turret_attack_priority_configured(
                runtime.host_4dc_pitched,
                primary_config,
                primary_gate,
            );
            let primary_hit_priority = refreshed_hit_snapshot
                .map(|hit| runtime.host_4dc_hit.priority(hit))
                .unwrap_or(1);
            let primary_magnetic_priority = runtime
                .host_4dc_magnetic
                .priority(got_hit_latch, refreshed_query_flags);
            let primary_scrambled_priority = runtime
                .host_4dc_scrambled
                .priority(got_hit_latch, refreshed_query_flags);
            let primary_winner = ep06_host_4dc_winner(
                track_target_priority(),
                primary_attack_priority,
                primary_hit_priority,
                primary_magnetic_priority,
                primary_scrambled_priority,
            );

            if runtime.host_4dc_active != primary_winner {
                match runtime.host_4dc_active {
                    Some(RobotsEp06Host4dcWinner::TrackTarget) => {
                        runtime.aim.tracking_latch = false;
                    }
                    Some(RobotsEp06Host4dcWinner::PitchedAttack) => {
                        leave_pitched_turret_attack_configured(
                            &mut runtime.host_4dc_pitched,
                            &mut runtime.aim,
                            primary_config,
                        );
                    }
                    Some(RobotsEp06Host4dcWinner::CommonHit) => {
                        runtime.host_4dc_hit.active = false;
                    }
                    Some(RobotsEp06Host4dcWinner::MagneticHit) => {
                        runtime.host_4dc_magnetic.leave();
                    }
                    Some(RobotsEp06Host4dcWinner::ScrambledHit) => {
                        runtime.host_4dc_scrambled.leave();
                    }
                    None => {}
                }

                let mut committed = primary_winner;
                match primary_winner {
                    Some(RobotsEp06Host4dcWinner::TrackTarget) => {
                        runtime.aim.tracking_latch = true;
                    }
                    Some(RobotsEp06Host4dcWinner::PitchedAttack) => {
                        enter_pitched_turret_attack_configured(
                            &mut runtime.host_4dc_pitched,
                            primary_config,
                        );
                        self.native_monster_attack_cooldown.record_attack();
                    }
                    Some(RobotsEp06Host4dcWinner::CommonHit) => {
                        if let (Some(source_yaw), Some(hit)) = (
                            self.native_ai_last_hit_source_yaw.get(&key).copied(),
                            refreshed_hit_snapshot,
                        ) {
                            runtime.host_4dc_hit.enter_configured(
                                RobotsCommonAiHitConfig::ep04_turret(),
                                owner_yaw_radians,
                                source_yaw,
                                hit.last_query_serial,
                            );
                        } else {
                            committed = None;
                        }
                    }
                    Some(RobotsEp06Host4dcWinner::MagneticHit) => {
                        runtime.host_4dc_magnetic.enter(owner_position.y);
                        if runtime.magnetic_drop_charge_count.is_none() {
                            runtime.magnetic_drop_charge_count =
                                Some(visual.pickup_drop_count.unwrap_or(0));
                        }
                    }
                    Some(RobotsEp06Host4dcWinner::ScrambledHit) => {
                        runtime.host_4dc_scrambled.enter();
                    }
                    None => {}
                }
                runtime.host_4dc_active = committed;
            }

            match runtime.host_4dc_active {
                Some(RobotsEp06Host4dcWinner::TrackTarget) => {
                    let target = gameplay_target - owner_position;
                    let target_yaw = target.x.atan2(target.z);
                    let yaw_step =
                        step_turret_yaw_toward(&mut runtime.aim, owner_yaw_radians, target_yaw);
                    if let Some(body) = self.runtime_character_bodies.get(&key) {
                        let _ = runtime.host_4dc_animation.advance_auxiliary_script_channel(
                            body,
                            yaw_step.requested_anim_mode,
                            ROBOTS_FIXED_STEP_SECONDS,
                        );
                    }
                }
                Some(RobotsEp06Host4dcWinner::PitchedAttack) => {
                    let step = step_pitched_turret_attack_configured(
                        &mut runtime.host_4dc_pitched,
                        &mut runtime.aim,
                        primary_config,
                    );
                    if step.requested_anim_mode == ROBOTS_EP06_PRIMARY_ATTACK_ANIM_MODE {
                        let events = if let Some(body) = self.runtime_character_bodies.get_mut(&key)
                        {
                            runtime.host_4dc_animation.advance(
                                body,
                                step.requested_anim_mode,
                                None,
                                ROBOTS_FIXED_STEP_SECONDS,
                                NativeAiRootMotionPolicy::NONE,
                            )
                        } else {
                            Vec::new()
                        };
                        for event in events {
                            match event.event_type {
                                event_type::SETUP_IDLE => {
                                    pitched_turret_attack_setup_idle(&mut runtime.host_4dc_pitched);
                                    runtime.host_4dc_animation.setup_idle();
                                }
                                event_type::CREATE_PROJECTILE => {
                                    if let Some(request) =
                                        RobotsCreateProjectileRequest::from_event(event.as_view())
                                    {
                                        projectile_events.push((
                                            ROBOTS_EP06_PRIMARY_ATTACK_ANIM_MODE,
                                            request,
                                            event.pose_seconds,
                                            event.owner_position,
                                            event.owner_rotation,
                                        ));
                                    }
                                }
                                _ => {}
                            }
                        }
                    } else if let Some(body) = self.runtime_character_bodies.get(&key) {
                        let _ = runtime.host_4dc_animation.advance_auxiliary_script_channel(
                            body,
                            step.requested_anim_mode,
                            ROBOTS_FIXED_STEP_SECONDS,
                        );
                    }
                }
                Some(RobotsEp06Host4dcWinner::CommonHit) => {
                    let hit_config = RobotsCommonAiHitConfig::ep04_turret();
                    let reenter = refreshed_hit_snapshot.is_some_and(|hit| {
                        hit.health != 0
                            && hit.query_serial_snapshot != u16::MAX
                            && hit.last_query_serial != runtime.host_4dc_hit.captured_query_serial
                    });
                    if reenter {
                        if let (Some(source_yaw), Some(hit)) = (
                            self.native_ai_last_hit_source_yaw.get(&key).copied(),
                            refreshed_hit_snapshot,
                        ) {
                            runtime.host_4dc_animation.setup_idle();
                            runtime.host_4dc_hit.enter_configured(
                                hit_config,
                                owner_yaw_radians,
                                source_yaw,
                                hit.last_query_serial,
                            );
                        }
                    }
                    let events = if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                        runtime.host_4dc_animation.advance(
                            body,
                            runtime.host_4dc_hit.requested_anim_mode,
                            None,
                            ROBOTS_FIXED_STEP_SECONDS,
                            NativeAiRootMotionPolicy::NONE,
                        )
                    } else {
                        Vec::new()
                    };
                    if events
                        .iter()
                        .any(|event| event.event_type == event_type::SETUP_IDLE)
                    {
                        if let Some(hit) = self.native_ai_hit_reactions.get_mut(&key) {
                            runtime.host_4dc_hit.setup_idle(hit);
                            got_hit_latch = hit.got_hit_latch;
                        }
                        runtime.host_4dc_animation.setup_idle();
                    }
                }
                Some(RobotsEp06Host4dcWinner::MagneticHit) => {
                    let attachment_relation_matches =
                        self.native_player_magnetic_target_key == Some(key);
                    let owner_y = self
                        .runtime_character_bodies
                        .get(&key)
                        .map(|body| body.owner_position.y)
                        .unwrap_or(owner_position.y);
                    let plan = {
                        let hit_latch = self
                            .native_ai_hit_reactions
                            .get_mut(&key)
                            .map(|hit| &mut hit.got_hit_latch);
                        if let Some(hit_latch) = hit_latch {
                            runtime.host_4dc_magnetic.step_with_anim_mode(
                                hit_latch,
                                owner_y,
                                attachment_relation_matches,
                                ROBOTS_FIXED_STEP_SECONDS,
                                ROBOTS_EP06_PRIMARY_IDLE_ANIM_MODE,
                            )
                        } else {
                            Default::default()
                        }
                    };

                    if let Some(value) = plan.set_handler_606 {
                        self.native_monster_physics
                            .entry(key)
                            .or_insert_with(
                                RobotsCharacterPhysicsRuntimeState::monster_ctor_default,
                            )
                            .apply_handler_606_mode(value);
                    }

                    let mut magnetic_vertical_velocity_delta = None;
                    if plan.service_magnetic_effect {
                        let drop_charge_count = runtime
                            .magnetic_drop_charge_count
                            .unwrap_or_else(|| visual.pickup_drop_count.unwrap_or(0));
                        let effect_plan = runtime.host_4dc_magnetic.step_attached_effect(
                            RobotsMalfBotMagneticEffectInput {
                                owner_y,
                                magnetic_mass: visual.magnetic_mass,
                                drop_charge_count,
                            },
                        );
                        if effect_plan.serviced {
                            runtime.magnetic_drop_charge_count =
                                Some(effect_plan.next_drop_charge_count);
                            magnetic_vertical_velocity_delta =
                                Some(effect_plan.physics_vertical_velocity_delta);
                        }
                    }
                    if let Some(delta_y) = magnetic_vertical_velocity_delta {
                        self.native_monster_physics
                            .entry(key)
                            .or_insert_with(
                                RobotsCharacterPhysicsRuntimeState::monster_ctor_default,
                            )
                            .add_vertical_velocity_delta(delta_y);
                    }
                    if let Some(mode) = plan.requested_anim_mode {
                        if let Some(body) = self.runtime_character_bodies.get(&key) {
                            let _ = runtime.host_4dc_animation.advance_auxiliary_script_channel(
                                body,
                                mode,
                                ROBOTS_FIXED_STEP_SECONDS,
                            );
                        }
                    }
                    request_natural_death |= plan.request_natural_death;
                }
                Some(RobotsEp06Host4dcWinner::ScrambledHit) => {
                    let mode = runtime.host_4dc_scrambled.step(&mut got_hit_latch);
                    if let Some(body) = self.runtime_character_bodies.get(&key) {
                        let _ = runtime.host_4dc_animation.advance_auxiliary_script_channel(
                            body,
                            mode,
                            ROBOTS_FIXED_STEP_SECONDS,
                        );
                    }
                }
                None => {}
            }
            tick_pitched_turret_attack(&mut runtime.host_4dc_pitched);

            if let Some(hit_state) = self.native_ai_hit_reactions.get_mut(&key) {
                hit_state.got_hit_latch = got_hit_latch;
            }
            self.native_ep06_turret_runtime.insert(key, runtime);

            if request_natural_death {
                self.request_native_ai_natural_death(map.hashcode, key, owner_position);
            }

            for (
                attack_anim_mode,
                request,
                pose_seconds,
                event_owner_position,
                event_owner_rotation,
            ) in projectile_events
            {
                let query_serial = self.allocate_native_hit_query_serial();
                if let Some(plan) = resolve_ai_projectile_spawn_plan(
                    map,
                    key,
                    event_owner_position,
                    event_owner_rotation,
                    owner_scale,
                    visual,
                    attack_anim_mode,
                    request,
                    query_serial,
                    pose_seconds,
                    gameplay_target,
                ) {
                    self.spawn_native_ai_projectile(plan);
                }
            }
        }
    }

    fn advance_native_turretbot_fixed(
        &mut self,
        map: &ProcessedMap,
        player_position: Vec3,
        wall_time: f64,
    ) {
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            let Some(visual) = trigger.character_visual.as_ref() else {
                continue;
            };
            if visual.handler_class != RobotsAiHandlerClass::TurretBot {
                continue;
            }

            let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
            if !self.runtime_ai_character_xitem_live(map.hashcode, trigger_index) {
                self.native_turretbot_runtime.remove(&key);
                self.native_monster_physics.remove(&key);
                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                    body.clear_collision_animation_track();
                }
                continue;
            }
            let Some(body) = self.runtime_character_bodies.get(&key) else {
                self.native_turretbot_runtime.remove(&key);
                self.native_monster_physics.remove(&key);
                continue;
            };
            let owner_position = body.owner_position;
            let owner_rotation = body.owner_rotation;
            let owner_scale = body.native_transform_scale_xyz();
            let owner_yaw_radians = horizontal_yaw(owner_rotation);

            let runtime_is_new = !self.native_turretbot_runtime.contains_key(&key);
            if runtime_is_new {
                // TurretBot vslot +0x100 = 0x004609D0 calls the paired native
                // Physics writers 0x004535D0(1)/0x00453600(1).
                self.native_monster_physics
                    .entry(key)
                    .or_insert_with(RobotsCharacterPhysicsRuntimeState::monster_ctor_default)
                    .service_attack_bit4(true);
            }
            self.native_turretbot_runtime.entry(key).or_default();

            if self.native_ai_normal_fatal_condition(key) {
                continue;
            }

            let periodic_needs_init = self
                .native_turretbot_runtime
                .get(&key)
                .is_some_and(|runtime| !runtime.periodic_idle.initialized);
            if periodic_needs_init {
                if let Some(draw) = self.next_native_ai_gameplay_rng_u32() {
                    if let Some(runtime) = self.native_turretbot_runtime.get_mut(&key) {
                        initialize_periodic_idle(
                            &mut runtime.periodic_idle,
                            ROBOTS_TURRETBOT_PERIODIC_IDLE_BASE_DELAY_TICKS,
                            draw,
                        );
                    }
                }
            }

            let gameplay_target = self.native_ai_gameplay_target_position(player_position);
            let target_visible = gameplay_target.is_some_and(|target| {
                self.runtime_map_script_line_of_sight_state(map, owner_position, target, wall_time)
                    .is_some_and(|state| state.0)
            });
            let player_state = self.native_player_focus_runtime.player_state;
            let top_game_state = self
                .native_cutscene_host_runtime
                .game_state_stack
                .last()
                .copied();
            let class_attack_allowed = robots_common_monster_attack_allowed(
                visual.handler_flags_628,
                top_game_state,
                player_state,
                true,
                self.native_player_hit_runtime.reaction_window,
                self.native_monster_attack_cooldown,
            );
            let hit_snapshot = self.native_ai_hit_reactions.get(&key).copied();
            let (got_hit_latch, query_flags_snapshot) = hit_snapshot
                .map(|hit| (hit.got_hit_latch, hit.query_flags_snapshot))
                .unwrap_or((false, 0));

            let mut runtime = self
                .native_turretbot_runtime
                .remove(&key)
                .unwrap_or_default();
            let headtrack_config = turretbot_headtrack_attack_config();
            let headtrack_priority = headtrack_attack_priority(
                runtime.headtrack_attack,
                headtrack_config,
                gameplay_target.map(|target| RobotsHeadtrackAttackGateInput {
                    owner_position_xyz: owner_position.to_array(),
                    target_position_xyz: target.to_array(),
                    target_visible,
                    class_attack_allowed,
                }),
            );
            let attack_group_priority =
                generic_attack_group_priority(&runtime.attack_group, &[headtrack_priority]);
            let periodic_priority = periodic_idle_priority(&runtime.periodic_idle);
            let common_hit_priority = hit_snapshot
                .map(|hit| runtime.common_hit.priority(hit))
                .unwrap_or(1);
            let magnetic_priority = runtime
                .magnetic_hit
                .priority(got_hit_latch, query_flags_snapshot);
            let scrambled_priority = runtime
                .scrambled_hit
                .priority(got_hit_latch, query_flags_snapshot);
            let winner = turretbot_behavior_winner(
                periodic_priority,
                common_hit_priority,
                attack_group_priority,
                magnetic_priority,
                scrambled_priority,
            );

            let mut entered_attack = false;
            if runtime.active_node != winner {
                match runtime.active_node {
                    Some(RobotsTurretBotBehaviorWinner::PeriodicIdle) => {
                        leave_periodic_idle(&mut runtime.periodic_idle);
                    }
                    Some(RobotsTurretBotBehaviorWinner::CommonHit) => {
                        runtime.common_hit.active = false;
                    }
                    Some(RobotsTurretBotBehaviorWinner::AttackGroup) => {
                        leave_headtrack_attack(&mut runtime.headtrack_attack);
                        leave_generic_attack_group(&mut runtime.attack_group);
                    }
                    Some(RobotsTurretBotBehaviorWinner::MagneticHit) => {
                        runtime.magnetic_hit.leave();
                    }
                    Some(RobotsTurretBotBehaviorWinner::ScrambledHit) => {
                        runtime.scrambled_hit.leave();
                    }
                    None => {}
                }
                if matches!(
                    runtime.active_node,
                    Some(
                        RobotsTurretBotBehaviorWinner::CommonHit
                            | RobotsTurretBotBehaviorWinner::MagneticHit
                    )
                ) {
                    if let Some(hit) = self.native_ai_hit_reactions.get_mut(&key) {
                        hit.query_serial_snapshot = 0;
                        hit.last_query_serial = 0;
                    }
                }

                let mut committed = winner;
                match winner {
                    Some(RobotsTurretBotBehaviorWinner::PeriodicIdle) => {
                        let select_draw = self.next_native_ai_gameplay_rng_u32();
                        let delay_draw =
                            select_draw.and_then(|_| self.next_native_ai_gameplay_rng_u32());
                        if let (Some(select_draw), Some(delay_draw)) = (select_draw, delay_draw) {
                            if enter_periodic_idle(
                                &mut runtime.periodic_idle,
                                ROBOTS_TURRETBOT_PERIODIC_IDLE_BASE_DELAY_TICKS,
                                &ROBOTS_TURRETBOT_PERIODIC_IDLE_ANIM_MODES,
                                select_draw,
                                delay_draw,
                            )
                            .is_none()
                            {
                                committed = None;
                            }
                        } else {
                            committed = None;
                        }
                    }
                    Some(RobotsTurretBotBehaviorWinner::CommonHit) => {
                        if let (Some(source_yaw), Some(hit)) = (
                            self.native_ai_last_hit_source_yaw.get(&key).copied(),
                            self.native_ai_hit_reactions.get(&key).copied(),
                        ) {
                            runtime.common_hit.enter_configured(
                                turretbot_nonfatal_hit_config(),
                                owner_yaw_radians,
                                source_yaw,
                                hit.last_query_serial,
                            );
                        } else {
                            committed = None;
                        }
                    }
                    Some(RobotsTurretBotBehaviorWinner::AttackGroup) => {
                        let draw = self.next_native_ai_gameplay_rng_u32();
                        if enter_generic_attack_group(
                            &mut runtime.attack_group,
                            &[headtrack_priority],
                            draw,
                        ) == Some(0)
                        {
                            enter_headtrack_attack(&mut runtime.headtrack_attack);
                            entered_attack = true;
                        } else {
                            leave_generic_attack_group(&mut runtime.attack_group);
                            committed = None;
                        }
                    }
                    Some(RobotsTurretBotBehaviorWinner::MagneticHit) => {
                        runtime.magnetic_hit.enter(owner_position.y);
                        if runtime.magnetic_drop_charge_count.is_none() {
                            runtime.magnetic_drop_charge_count =
                                Some(visual.pickup_drop_count.unwrap_or(0));
                        }
                    }
                    Some(RobotsTurretBotBehaviorWinner::ScrambledHit) => {
                        runtime.scrambled_hit.enter();
                    }
                    None => {}
                }
                runtime.active_node = committed;
            }

            let mut projectile_events =
                Vec::<(u32, RobotsCreateProjectileRequest, f32, Vec3, Quat)>::new();
            let mut request_natural_death = false;

            match runtime.active_node {
                Some(RobotsTurretBotBehaviorWinner::PeriodicIdle) => {
                    if let Some(mode) = runtime.periodic_idle.selected_anim_mode {
                        let events = if let Some(body) = self.runtime_character_bodies.get_mut(&key)
                        {
                            runtime.animation.advance(
                                body,
                                mode,
                                None,
                                ROBOTS_FIXED_STEP_SECONDS,
                                NativeAiRootMotionPolicy::NONE,
                            )
                        } else {
                            Vec::new()
                        };
                        if events
                            .iter()
                            .any(|event| event.event_type == event_type::SETUP_IDLE)
                        {
                            complete_periodic_idle_setup_idle(&mut runtime.periodic_idle);
                            runtime.animation.setup_idle();
                        }
                    }
                }
                Some(RobotsTurretBotBehaviorWinner::CommonHit) => {
                    let hit_config = turretbot_nonfatal_hit_config();
                    let reenter = self.native_ai_hit_reactions.get(&key).is_some_and(|hit| {
                        hit.health != 0
                            && hit.query_serial_snapshot != u16::MAX
                            && hit.last_query_serial != runtime.common_hit.captured_query_serial
                    });
                    if reenter {
                        if let (Some(source_yaw), Some(hit)) = (
                            self.native_ai_last_hit_source_yaw.get(&key).copied(),
                            self.native_ai_hit_reactions.get(&key).copied(),
                        ) {
                            runtime.animation.setup_idle();
                            runtime.common_hit.enter_configured(
                                hit_config,
                                owner_yaw_radians,
                                source_yaw,
                                hit.last_query_serial,
                            );
                        }
                    }
                    let owner_yaw_write = runtime
                        .common_hit
                        .step_owner_yaw_configured(hit_config, owner_yaw_radians);
                    let events = if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                        runtime.animation.advance(
                            body,
                            runtime.common_hit.requested_anim_mode,
                            owner_yaw_write,
                            ROBOTS_FIXED_STEP_SECONDS,
                            NativeAiRootMotionPolicy::NONE,
                        )
                    } else {
                        Vec::new()
                    };
                    if events
                        .iter()
                        .any(|event| event.event_type == event_type::SETUP_IDLE)
                    {
                        if let Some(hit) = self.native_ai_hit_reactions.get_mut(&key) {
                            runtime.common_hit.setup_idle(hit);
                        }
                        runtime.animation.setup_idle();
                    }
                }
                Some(RobotsTurretBotBehaviorWinner::AttackGroup) => {
                    let step = step_headtrack_attack(
                        &mut runtime.headtrack_attack,
                        headtrack_config,
                        RobotsHeadtrackAttackStepInput {
                            owner_position_xyz: owner_position.to_array(),
                            target_position_xyz: gameplay_target.map(|target| target.to_array()),
                        },
                    );
                    if let Some(mode) = step.requested_anim_mode {
                        let events = if let Some(body) = self.runtime_character_bodies.get_mut(&key)
                        {
                            runtime.animation.advance(
                                body,
                                mode,
                                None,
                                ROBOTS_FIXED_STEP_SECONDS,
                                NativeAiRootMotionPolicy::NONE,
                            )
                        } else {
                            Vec::new()
                        };
                        for event in events {
                            match event.event_type {
                                event_type::SETUP_IDLE => {
                                    headtrack_attack_setup_idle(&mut runtime.headtrack_attack);
                                    runtime.animation.setup_idle();
                                }
                                event_type::CREATE_PROJECTILE => {
                                    if let Some(request) =
                                        RobotsCreateProjectileRequest::from_event(event.as_view())
                                    {
                                        projectile_events.push((
                                            mode,
                                            request,
                                            event.pose_seconds,
                                            event.owner_position,
                                            event.owner_rotation,
                                        ));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Some(RobotsTurretBotBehaviorWinner::MagneticHit) => {
                    let attachment_relation_matches =
                        self.native_player_magnetic_target_key == Some(key);
                    let owner_y = self
                        .runtime_character_bodies
                        .get(&key)
                        .map(|body| body.owner_position.y)
                        .unwrap_or(owner_position.y);
                    let plan = if let Some(hit) = self.native_ai_hit_reactions.get_mut(&key) {
                        runtime.magnetic_hit.step(
                            &mut hit.got_hit_latch,
                            owner_y,
                            attachment_relation_matches,
                            ROBOTS_FIXED_STEP_SECONDS,
                        )
                    } else {
                        Default::default()
                    };

                    if let Some(value) = plan.set_handler_606 {
                        self.native_monster_physics
                            .entry(key)
                            .or_insert_with(
                                RobotsCharacterPhysicsRuntimeState::monster_ctor_default,
                            )
                            .apply_handler_606_mode(value);
                    }

                    let mut magnetic_vertical_velocity_delta = None;
                    let effect_plan = if plan.service_magnetic_effect {
                        let drop_charge_count = runtime
                            .magnetic_drop_charge_count
                            .unwrap_or_else(|| visual.pickup_drop_count.unwrap_or(0));
                        let effect_plan = runtime.magnetic_hit.step_attached_effect(
                            RobotsMalfBotMagneticEffectInput {
                                owner_y,
                                magnetic_mass: visual.magnetic_mass,
                                drop_charge_count,
                            },
                        );
                        if effect_plan.serviced {
                            runtime.magnetic_drop_charge_count =
                                Some(effect_plan.next_drop_charge_count);
                            magnetic_vertical_velocity_delta =
                                Some(effect_plan.physics_vertical_velocity_delta);
                        }
                        Some(effect_plan)
                    } else {
                        None
                    };
                    if let Some(delta_y) = magnetic_vertical_velocity_delta {
                        self.native_monster_physics
                            .entry(key)
                            .or_insert_with(
                                RobotsCharacterPhysicsRuntimeState::monster_ctor_default,
                            )
                            .add_vertical_velocity_delta(delta_y);
                    }
                    if let Some(mode) = plan.requested_anim_mode {
                        let events = if let Some(body) = self.runtime_character_bodies.get_mut(&key)
                        {
                            runtime.animation.advance(
                                body,
                                mode,
                                None,
                                ROBOTS_FIXED_STEP_SECONDS,
                                NativeAiRootMotionPolicy::FULL,
                            )
                        } else {
                            Vec::new()
                        };
                        if events
                            .iter()
                            .any(|event| event.event_type == event_type::SETUP_IDLE)
                        {
                            runtime.animation.setup_idle();
                        }
                    }
                    if effect_plan.is_some_and(|effect| effect.serviced) {
                        let physics = self.native_monster_physics.get(&key).copied();
                        if let (Some(physics), Some(body)) =
                            (physics, self.runtime_character_bodies.get_mut(&key))
                        {
                            if physics.object_flag_bit0 {
                                let mut position = body.owner_position.to_array();
                                physics
                                    .integrate_position(&mut position, ROBOTS_FIXED_STEP_SECONDS);
                                body.owner_position = Vec3::from_array(position);
                            }
                        }
                    }
                    request_natural_death |= plan.request_natural_death;
                }
                Some(RobotsTurretBotBehaviorWinner::ScrambledHit) => {
                    let mode = if let Some(hit) = self.native_ai_hit_reactions.get_mut(&key) {
                        runtime.scrambled_hit.step(&mut hit.got_hit_latch)
                    } else {
                        0
                    };
                    if mode != 0 {
                        let events = if let Some(body) = self.runtime_character_bodies.get_mut(&key)
                        {
                            runtime.animation.advance(
                                body,
                                mode,
                                None,
                                ROBOTS_FIXED_STEP_SECONDS,
                                NativeAiRootMotionPolicy::FULL,
                            )
                        } else {
                            Vec::new()
                        };
                        if events
                            .iter()
                            .any(|event| event.event_type == event_type::SETUP_IDLE)
                        {
                            if let Some(hit) = self.native_ai_hit_reactions.get_mut(&key) {
                                runtime.scrambled_hit.setup_idle(&mut hit.got_hit_latch);
                            }
                            runtime.animation.setup_idle();
                        }
                    }
                }
                None => {}
            }

            tick_periodic_idle(&mut runtime.periodic_idle);
            tick_headtrack_attack(&mut runtime.headtrack_attack);
            self.native_turretbot_runtime.insert(key, runtime);

            if entered_attack {
                self.native_monster_attack_cooldown.record_attack();
            }
            if request_natural_death {
                let death_position = self
                    .runtime_character_bodies
                    .get(&key)
                    .map(|body| body.owner_position)
                    .unwrap_or(owner_position);
                self.request_native_ai_natural_death(map.hashcode, key, death_position);
            }
            if let Some(projectile_target) = gameplay_target {
                for (attack_anim_mode, request, pose_seconds, event_position, event_rotation) in
                    projectile_events
                {
                    let query_serial = self.allocate_native_hit_query_serial();
                    if let Some(plan) = resolve_ai_projectile_spawn_plan(
                        map,
                        key,
                        event_position,
                        event_rotation,
                        owner_scale,
                        visual,
                        attack_anim_mode,
                        request,
                        query_serial,
                        pose_seconds,
                        projectile_target,
                    ) {
                        self.spawn_native_ai_projectile(plan);
                    }
                }
            }
        }
    }

    pub(super) fn advance_native_standard_monsters_fixed(
        &mut self,
        map: &ProcessedMap,
        nav_regions: &[RobotsMonsterNavMeshView<'_>],
        player_position: Vec3,
        wall_time: f64,
        scope: NativeAiHostScope,
    ) {
        let mut instances = Vec::<(
            u64,
            &ProcessedCharacterVisual,
            RobotsStandardMonsterBehaviorConfig,
            bool,
            Option<u32>,
            Option<u32>,
        )>::new();
        if scope == NativeAiHostScope::Serialized {
            for (trigger_index, trigger) in map.triggers.iter().enumerate() {
                let Some(visual) = trigger.character_visual.as_ref() else {
                    continue;
                };
                let Some(config) =
                    RobotsStandardMonsterBehaviorConfig::for_handler_class(visual.handler_class)
                else {
                    continue;
                };
                let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
                if !self.runtime_ai_character_xitem_live(map.hashcode, trigger_index) {
                    self.native_standard_monster_runtime.remove(&key);
                    self.native_monster_navigation.remove(&key);
                    self.native_monster_physics.remove(&key);
                    if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                        body.clear_collision_animation_track();
                    }
                    continue;
                }
                let allow_cross_group = trigger
                    .data
                    .get(7)
                    .and_then(|value| *value)
                    .is_some_and(|flags| flags & 0x8000 != 0);
                let creator_path_uid = trigger.data.get(2).and_then(|value| *value);
                instances.push((
                    key,
                    visual,
                    config,
                    allow_cross_group,
                    None,
                    creator_path_uid,
                ));
            }
        } else {
            let dynamic_snapshots = self.runtime_dynamic_ai_snapshots(map);
            let mut live_dynamic_standard_keys = FxHashSet::default();
            for snapshot in dynamic_snapshots {
                if snapshot.pending_destroy {
                    continue;
                }
                let Some(bootstrap) = snapshot.bootstrap else {
                    continue;
                };
                let Some(visual) = map
                    .runtime_character_visuals
                    .get(&(ROBOTS_SWEEPER_MONSTER_RUNTIME_TYPE, snapshot.selector))
                else {
                    continue;
                };
                let Some(config) =
                    RobotsStandardMonsterBehaviorConfig::for_handler_class(visual.handler_class)
                else {
                    continue;
                };
                let periodic_idle_offset = match (bootstrap, config.handler_class) {
                    (
                        NativeSweeperBossGenericAiBootstrap::MalfBot { random_mod90 },
                        RobotsAiHandlerClass::MalfBot,
                    ) => random_mod90,
                    (
                        NativeSweeperBossGenericAiBootstrap::StandardMonster {
                            periodic_idle_offset,
                        },
                        RobotsAiHandlerClass::Ew10Minion
                        | RobotsAiHandlerClass::Eb14Minion
                        | RobotsAiHandlerClass::ShuntBotBoss,
                    ) => periodic_idle_offset,
                    _ => continue,
                };
                live_dynamic_standard_keys.insert(snapshot.key);
                // Sweeper direct Monster factory 0x0047E8B0 -> 0x00443D40
                // allocates an XItem whose +0x154 creator pointer remains null.
                // Common 0x00451780 therefore leaves Handler+0x605 at ctor zero:
                // dynamic factory AI is natively same-group, not a fail-closed guess.
                instances.push((
                    snapshot.key,
                    visual,
                    config,
                    ROBOTS_DIRECT_MONSTER_FACTORY_ALLOW_CROSS_GROUP,
                    Some(u32::from(periodic_idle_offset)),
                    None,
                ));
            }
            self.native_standard_monster_runtime.retain(|key, _| {
                !super::runtime_bodies::RuntimeAiInstanceKey::raw_is_dynamic_for_map(
                    *key,
                    map.hashcode,
                ) || live_dynamic_standard_keys.contains(key)
            });
            self.native_monster_navigation.retain(|key, _| {
                !super::runtime_bodies::RuntimeAiInstanceKey::raw_is_dynamic_for_map(
                    *key,
                    map.hashcode,
                ) || live_dynamic_standard_keys.contains(key)
            });
        }

        for (key, visual, config, allow_cross_group, dynamic_setup_draw, creator_path_uid) in
            instances
        {
            let effective_handler_flags =
                if config.handler_class == RobotsAiHandlerClass::Ew09Armoured {
                    ew09_effective_handler_flags(visual.handler_flags_628)
                } else {
                    visual.handler_flags_628
                };
            let ew09_path = (config.handler_class == RobotsAiHandlerClass::Ew09Armoured)
                .then_some(creator_path_uid)
                .flatten()
                .filter(|uid| *uid != 0x0B00_0000 && (*uid & 0x0B00_0000) == 0x0B00_0000)
                .and_then(|uid| map.paths.iter().find(|path| path.hashcode == uid));
            let ew09_path_nodes = ew09_path.map(|path| {
                path.nodes
                    .iter()
                    .map(|node| RobotsFollowNetworkPathNode {
                        position_xyz: (path.position + node.position).to_array(),
                        size_x: node.size.x,
                        value: node.value,
                    })
                    .collect::<Vec<_>>()
            });
            let ef01_first_update_route = (config.handler_class == RobotsAiHandlerClass::Ef01Mine)
                .then(|| ef01_mine_first_update_route(creator_path_uid));
            let ef01_flying_path = match ef01_first_update_route {
                Some(RobotsEf01MineFirstUpdateRoute::FollowFlyingPath { path_uid }) => {
                    map.paths.iter().find(|path| path.hashcode == path_uid)
                }
                _ => None,
            };
            let ef01_flying_path_points = ef01_flying_path.map(|path| {
                path.nodes
                    .iter()
                    .map(|node| (path.position + node.position).to_array())
                    .collect::<Vec<_>>()
            });
            let standard_runtime_is_new = !self.native_standard_monster_runtime.contains_key(&key);
            let ef01_flying_path_active = self
                .native_standard_monster_runtime
                .get(&key)
                .map(|runtime| runtime.ef01_flying_path_latch_644)
                .unwrap_or(matches!(
                    ef01_first_update_route,
                    Some(RobotsEf01MineFirstUpdateRoute::FollowFlyingPath { .. })
                ));

            let Some(_) = self.runtime_character_bodies.get(&key) else {
                self.native_standard_monster_runtime.remove(&key);
                self.native_monster_navigation.remove(&key);
                self.native_monster_physics.remove(&key);
                continue;
            };
            // EB13 +0x34 = 0x00465E30 services its builder-owned first
            // AttachAnimator entry BEFORE common Monster update. Sample the current
            // animation pose now, before nav/selector/animation advancement mutates
            // this fixed tick. Missing datums fail closed like the native queries.
            if config.handler_class == RobotsAiHandlerClass::Eb13KnightBot {
                let animator = RobotsKnightBotAnimatorConfig::eb13();
                let sample = self
                    .runtime_ai_animation_datum_world_transform_by_key(
                        map,
                        key,
                        animator.position_datum_uid,
                    )
                    .zip(self.runtime_ai_animation_datum_world_transform_by_key(
                        map,
                        key,
                        animator.rotation_datum_uid,
                    ));
                if let Some((position, rotation_point)) = sample {
                    let runtime = self
                        .native_standard_monster_runtime
                        .entry(key)
                        .or_insert_with(|| NativeStandardMonsterRuntime::new(config));
                    service_native_knightbot_pre_common_update(
                        runtime,
                        position.position,
                        rotation_point.position,
                    );
                }
            }
            if config.physics_locomotion_speed_range().is_some() {
                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                    advance_native_retained_horizontal_physics(
                        self.native_monster_physics.get(&key),
                        body,
                    );
                }
            }
            let Some(body) = self.runtime_character_bodies.get(&key) else {
                continue;
            };
            let owner_rotation = body.owner_rotation;
            let pre_common_owner_position = body.owner_position;
            let nav_constraint_step = if ef01_flying_path_active && !standard_runtime_is_new {
                self.native_monster_navigation.remove(&key);
                if let Some(physics) = self.native_monster_physics.get_mut(&key) {
                    physics.service_ef01_flying_path_bit2(true);
                }
                NativeMonsterNavConstraintHostStep {
                    pre_constraint_owner_position: pre_common_owner_position,
                    correction_xz: [0.0; 2],
                }
            } else {
                let Some(step) = self.apply_native_common_monster_nav_constraint(
                    key,
                    nav_regions,
                    allow_cross_group,
                    effective_handler_flags,
                ) else {
                    continue;
                };
                if config.handler_class == RobotsAiHandlerClass::Ef01Mine {
                    self.native_monster_physics
                        .entry(key)
                        .or_insert_with(RobotsCharacterPhysicsRuntimeState::monster_ctor_default)
                        .service_ef01_flying_path_bit2(false);
                }
                step
            };
            let pre_constraint_owner_position = nav_constraint_step.pre_constraint_owner_position;

            let nav_constraint_correction_xz = nav_constraint_step.correction_xz;

            let Some(body) = self.runtime_character_bodies.get(&key) else {
                continue;
            };
            let owner_position = body.owner_position;
            let owner_yaw_radians = horizontal_yaw(owner_rotation);
            let nav_context = self.native_monster_navigation.get(&key).and_then(|state| {
                let region_ordinal = state.region_ordinal?;
                let current_face = state.face_index?;
                let group_flags0 = state.group_flags0?;
                Some((
                    nav_regions.get(region_ordinal).copied()?,
                    current_face,
                    group_flags0,
                ))
            });

            if self.native_ai_normal_fatal_condition(key) {
                continue;
            }
            let player_state = self.native_player_focus_runtime.player_state;
            let Some(player_position) = self.native_ai_gameplay_target_position(player_position)
            else {
                continue;
            };

            let target_visible = self
                .runtime_map_script_line_of_sight_state(
                    map,
                    owner_position,
                    player_position,
                    wall_time,
                )
                .is_some_and(|state| state.0);
            let top_game_state = self
                .native_cutscene_host_runtime
                .game_state_stack
                .last()
                .copied();
            let global_state_blocked = matches!(top_game_state, Some(1 | 2 | 3 | 0x0f));

            let periodic_enabled = !config.periodic_idle_anim_modes.is_empty();
            let periodic_needs_init = periodic_enabled
                && self
                    .native_standard_monster_runtime
                    .get(&key)
                    .is_none_or(|runtime| !runtime.periodic_idle.initialized);
            let periodic_initial_draw = if periodic_needs_init {
                dynamic_setup_draw.or_else(|| self.next_native_ai_gameplay_rng_u32())
            } else {
                None
            };
            if standard_runtime_is_new {
                if let Some(sound) = config.first_update_permanent_sound() {
                    register_native_ai_permanent_sound(
                        &mut self.native_ai_permanent_sounds,
                        key,
                        sound,
                    );
                }
                if config.first_update_uniform_scale != 1.0 {
                    if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                        body.apply_native_uniform_transform_scale(
                            config.first_update_uniform_scale,
                        );
                    }
                }
            }
            let owner_scale = self
                .runtime_character_bodies
                .get(&key)
                .map(RuntimeCharacterBodyState::native_transform_scale_xyz)
                .unwrap_or(Vec3::ONE);
            {
                let runtime = self
                    .native_standard_monster_runtime
                    .entry(key)
                    .or_insert_with(|| NativeStandardMonsterRuntime::new(config));
                if config.handler_class == RobotsAiHandlerClass::Eb11MagnaBot {
                    runtime.magnabot_turn.begin_update();
                }
                if standard_runtime_is_new && ef01_flying_path_active {
                    if let (Some(path), Some(points)) =
                        (ef01_flying_path, ef01_flying_path_points.as_deref())
                    {
                        let _ = runtime
                            .follow_flying_path
                            .bind_path(points, path.path_type == 1);
                    } else {
                        runtime.follow_flying_path.clear_path();
                    }
                }
                if let Some(draw) = periodic_initial_draw {
                    initialize_periodic_idle(
                        &mut runtime.periodic_idle,
                        config.periodic_idle_base_delay_ticks,
                        draw,
                    );
                }
            }
            if standard_runtime_is_new {
                if let Some(route) = ef01_first_update_route {
                    let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                    let physics = self
                        .native_monster_physics
                        .entry(key)
                        .or_insert_with(RobotsCharacterPhysicsRuntimeState::monster_ctor_default);
                    apply_native_ef01_first_update_effects(runtime, physics, route);
                }
            }

            // Handler +0x130 (`0x00455DD0`) services already-registered HitCheck
            // records before selector execution. Native keeps an ordered retained
            // array at Handler+0x4B8/+0x4BC; common Monster capacity is one and
            // Monster_2Rockets vslot +0x08 raises Handler+0x618 to two.
            let mut standard_hit_query_completed = false;
            let active_queries = self
                .native_standard_monster_runtime
                .get(&key)
                .map(|runtime| {
                    (
                        runtime.hit_queries.clone(),
                        runtime.animation.sampled_pose_seconds(),
                        runtime.animation.current_anim_mode(),
                    )
                });
            if let Some((entries, pose_seconds, current_anim_mode)) = active_queries {
                let mut retained = Vec::with_capacity(entries.len());
                for entry in entries {
                    let mut query = entry.query;
                    let source_shape = (current_anim_mode == entry.source_anim_mode)
                        .then(|| {
                            runtime_character_animation_datum_world_shape(
                                owner_position,
                                owner_rotation,
                                owner_scale,
                                visual,
                                entry.source_anim_mode,
                                query.selector,
                                pose_seconds,
                            )
                        })
                        .flatten();
                    let hit = source_shape.is_some_and(|shape| {
                        self.native_hit_shape_hits_player(
                            map,
                            shape,
                            RobotsHitQueryCandidateContext {
                                flags: query.flags,
                                query_serial: query.serial,
                                source_raw_group: Some(ROBOTS_HIT_QUERY_RAW_GROUP1),
                                secondary_source_raw_group: None,
                            },
                        )
                    });
                    let result = query.step(source_shape.is_some(), hit, 0, 1.0);
                    let completed = result.kind == RobotsHitQueryStepKind::ScannedHit;
                    standard_hit_query_completed |= completed;
                    // `0x00455DD0` services every retained 0x98-byte record but does
                    // not shrink Handler+0x4B8/+0x4BC. Even an inactive/completed
                    // record keeps its array slot until a later registration evicts
                    // the oldest entry or the owning behavior explicitly clears it.
                    retained.push(NativeStandardMonsterHitQueryRuntime {
                        query,
                        source_anim_mode: entry.source_anim_mode,
                    });
                }
                if let Some(runtime) = self.native_standard_monster_runtime.get_mut(&key) {
                    runtime.hit_queries = retained;
                }
            }

            // Common base-AI `0x00451DF0` runs before selector execution and sees
            // the previous tick's Character Physics velocity.
            let movement_error_ratio = {
                let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                let physics_velocity = runtime.animation.character_physics_velocity();
                step_ai_movement_error(
                    &mut runtime.movement_error,
                    pre_constraint_owner_position.to_array(),
                    physics_velocity.to_array(),
                    1.0,
                )
            };

            let pursue_input = nav_context.map(|(_, current_face, _)| RobotsPursueNavMeshInput {
                owner_position_xyz: owner_position.to_array(),
                owner_yaw_radians,
                current_face,
                target_position_xyz: player_position.to_array(),
                handler_flags_628: effective_handler_flags,
                move_mode_active_on_entry: self
                    .native_standard_monster_runtime
                    .get(&key)
                    .is_some_and(|runtime| runtime.animation.is_mode(ROBOTS_ANIM_MODE_MOVE)),
                runtime_rate_scale: 1.0,
                allow_cross_group,
                stop_distance: config.pursue_stop_distance,
            });
            let ef01_pursue_route_active = matches!(
                ef01_first_update_route,
                Some(RobotsEf01MineFirstUpdateRoute::PursueNavMesh)
            );
            let pursue_ready = (config.pursue_enabled || ef01_pursue_route_active)
                && nav_context
                    .zip(pursue_input)
                    .is_some_and(|((nav, _, _), input)| {
                        self.native_standard_monster_runtime
                            .get_mut(&key)
                            .unwrap()
                            .pursue_navmesh
                            .refresh_navigation(nav, input)
                    });

            let class_attack_allowed = if config.uses_common_monster_attack_gate {
                robots_common_monster_attack_allowed(
                    effective_handler_flags,
                    top_game_state,
                    player_state,
                    true,
                    self.native_player_hit_runtime.reaction_window,
                    self.native_monster_attack_cooldown,
                )
            } else {
                // MalfBot handler vslot +0x158 = 0x0041D5C0 returns 1.
                true
            };
            let player_reaction_active = robots_player_reaction_gate_active(
                top_game_state,
                player_state,
                true,
                self.native_player_hit_runtime.reaction_window,
            );
            let attack_input = RobotsGenericAttackGateInput {
                owner_position_xyz: owner_position.to_array(),
                owner_yaw_radians,
                target_position_xyz: player_position.to_array(),
                target_visible,
                class_attack_allowed,
            };
            let hit_snapshot = self.native_ai_hit_reactions.get(&key).copied();
            let (got_hit_latch, query_flags_snapshot) = hit_snapshot
                .map(|hit| (hit.got_hit_latch, hit.query_flags_snapshot))
                .unwrap_or((false, 0));
            let (attack_priorities, winner, previous_node) = {
                let runtime = self.native_standard_monster_runtime.get(&key).unwrap();
                let mut attack_priorities: [u8; ROBOTS_STANDARD_MONSTER_MAX_ATTACKS] =
                    std::array::from_fn(|index| {
                        config.attacks[index].map_or(0, |attack| {
                            generic_attack_priority(runtime.attacks[index], attack, attack_input)
                        })
                    });
                for (index, turn_config) in config.turn_then_attacks.iter().copied().enumerate() {
                    if let Some(turn_config) = turn_config {
                        attack_priorities[index] = turn_then_attack_priority(
                            runtime.turn_then_attacks[index],
                            turn_config,
                            RobotsTurnThenAttackGateInput {
                                owner_position_xyz: owner_position.to_array(),
                                target_position_xyz: player_position.to_array(),
                                target_visible,
                                class_attack_allowed,
                            },
                        );
                    }
                }
                if let (Some(three_phase_config), Some(index)) = (
                    config.attack_group_three_phase(),
                    config.attack_group_three_phase_index(),
                ) {
                    if let Some(priority) = attack_priorities.get_mut(index) {
                        *priority = three_phase_attack_priority(
                            runtime.attack_group_three_phase,
                            three_phase_config,
                            RobotsThreePhaseAttackInput {
                                owner_position_xyz: owner_position.to_array(),
                                player_position_xyz: player_position.to_array(),
                                target_visible,
                            },
                            class_attack_allowed,
                        );
                    }
                }
                let attack_group_priority = if config.attacks.iter().any(Option::is_some)
                    || config.turn_then_attacks.iter().any(Option::is_some)
                    || config.attack_group_three_phase_index().is_some()
                {
                    generic_attack_group_priority(&runtime.attack_group, &attack_priorities)
                } else {
                    1
                };
                let direct_turn_attack_priority =
                    config.direct_turn_then_attack().map_or(1, |turn_config| {
                        turn_then_attack_priority(
                            runtime.direct_turn_then_attack,
                            turn_config,
                            RobotsTurnThenAttackGateInput {
                                owner_position_xyz: owner_position.to_array(),
                                target_position_xyz: player_position.to_array(),
                                target_visible,
                                class_attack_allowed,
                            },
                        )
                    });
                let shunt_attack_priority = config.shunt_attack.map_or(1, |shunt_config| {
                    shunt_attack_priority_configured(
                        shunt_config,
                        runtime.shunt_attack,
                        RobotsShuntAttackPriorityInput {
                            common_attack_gate_ready: target_visible && class_attack_allowed,
                            target_distance_squared: owner_position
                                .distance_squared(player_position),
                        },
                    )
                });
                let spike_attack_priority = config.spike_attack.map_or(1, |spike_config| {
                    spike_attack_priority(
                        spike_config,
                        runtime.spike_attack,
                        RobotsSpikeAttackPriorityInput {
                            target_present: true,
                            target_visible,
                            target_distance_squared: owner_position
                                .distance_squared(player_position),
                        },
                    )
                });
                let common_hit_priority = config.common_hit.map_or(1, |_| {
                    hit_snapshot
                        .map(|hit| runtime.common_hit.priority(hit))
                        .unwrap_or(1)
                });
                let periodic_priority = if periodic_enabled {
                    periodic_idle_priority(&runtime.periodic_idle)
                } else {
                    1
                };
                let nav_priority =
                    patrol_navmesh2_priority(nav_context.is_some(), global_state_blocked);
                let patrol_priority = if config.has_patrol_node() {
                    ai_patrol_priority(top_game_state)
                } else {
                    1
                };
                let follow_network_path_node_priority = if config.handler_class
                    == RobotsAiHandlerClass::Ew09Armoured
                    && ew09_path_nodes
                        .as_ref()
                        .is_some_and(|nodes| !nodes.is_empty())
                {
                    follow_network_path_priority(ew09_follow_network_path_config())
                } else {
                    1
                };
                let follow_flying_path_node_priority = if ef01_flying_path_active {
                    follow_flying_path_priority(RobotsFollowFlyingPathConfig::ef01())
                } else {
                    1
                };
                let ef01_pursue_node_priority = if ef01_pursue_route_active && pursue_ready {
                    ROBOTS_PURSUE_NAV_PRIORITY
                } else {
                    1
                };
                let stalk_priority = config.stalk.map_or(1, |stalk_config| {
                    stalk_navmesh_priority(
                        runtime.stalk_navmesh,
                        stalk_config,
                        RobotsStalkNavMeshPriorityInput {
                            target_available: true,
                            navigation_context_ready: nav_context.is_some(),
                            target_distance_squared: owner_position
                                .distance_squared(player_position),
                            // Native 0x00455DA0 returns true when either the
                            // handler-local 0x100000 claim bit is set or the
                            // process-global current-attacker owner is this handler.
                            attack_claim_suppresses_stalk:
                                robots_current_attacker_claim_active(
                                    effective_handler_flags,
                                    self.native_current_attacker.current_owner_key == Some(key),
                                ) && class_attack_allowed,
                        },
                    )
                });
                let flee_priority = config.flee_navmesh().map_or(1, |flee_config| {
                    flee_navmesh_priority(
                        flee_config,
                        runtime.active_node
                            == Some(RobotsStandardMonsterBehaviorWinner::FleeNavMesh),
                        nav_context.is_some(),
                        owner_position.to_array(),
                        Some(player_position.to_array()),
                    )
                });
                let scrambled_priority = if config.has_status_hit_nodes {
                    runtime
                        .scrambled_hit
                        .priority(got_hit_latch, query_flags_snapshot)
                } else {
                    1
                };
                let electro_priority = if config.has_status_hit_nodes {
                    runtime
                        .electro_hit
                        .priority(got_hit_latch, query_flags_snapshot)
                } else {
                    1
                };
                let magnetic_priority = if config.has_magnetic_hit_node() {
                    runtime
                        .magnetic_hit
                        .priority(got_hit_latch, query_flags_snapshot)
                } else {
                    1
                };
                let target_delta = player_position - owner_position;
                let target_yaw_radians = target_delta.x.atan2(target_delta.z);
                let target_yaw_error_radians =
                    shortest_yaw_delta(owner_yaw_radians, target_yaw_radians);
                let target_distance_squared = owner_position.distance_squared(player_position);
                let circle_target_node_priority = config.circle_target().map_or(1, |circle| {
                    circle_target_priority(
                        circle,
                        runtime.active_node
                            == Some(RobotsStandardMonsterBehaviorWinner::CircleTarget),
                        target_visible,
                        owner_position.to_array(),
                        Some(player_position.to_array()),
                    )
                });
                let proximity_idle_attack_priority =
                    if config.handler_class == RobotsAiHandlerClass::Eb11MagnaBot {
                        proximity_anim_priority(
                            RobotsProximityAnimBehaviorConfig::magnabot_idle_attack(),
                            runtime.active_node
                                == Some(RobotsStandardMonsterBehaviorWinner::ProximityIdleAttack),
                            true,
                            target_distance_squared,
                            target_yaw_error_radians,
                        )
                    } else {
                        1
                    };
                let target_facing_turn_node_priority =
                    if config.handler_class == RobotsAiHandlerClass::Eb11MagnaBot {
                        target_facing_turn_priority(
                            RobotsTargetFacingTurnBehaviorConfig::magnabot(),
                            runtime.active_node
                                == Some(RobotsStandardMonsterBehaviorWinner::TargetFacingTurn),
                            true,
                            target_distance_squared,
                            target_yaw_error_radians,
                        )
                    } else {
                        1
                    };
                let winner = if config.handler_class == RobotsAiHandlerClass::Ef01Mine {
                    ef01_mine_behavior_winner(
                        patrol_priority,
                        periodic_priority,
                        electro_priority,
                        magnetic_priority,
                        nav_priority,
                        follow_flying_path_node_priority,
                        ef01_pursue_node_priority,
                    )
                } else if config.handler_class == RobotsAiHandlerClass::Ew09Armoured {
                    ew09_armoured_behavior_winner(
                        patrol_priority,
                        attack_group_priority,
                        follow_network_path_node_priority,
                    )
                } else if config.handler_class == RobotsAiHandlerClass::Eb11MagnaBot {
                    magnabot_behavior_winner(
                        patrol_priority,
                        proximity_idle_attack_priority,
                        target_facing_turn_node_priority,
                        pursue_ready,
                        stalk_priority,
                        nav_priority,
                        attack_group_priority,
                        common_hit_priority,
                        scrambled_priority,
                        electro_priority,
                    )
                } else if matches!(
                    config.handler_class,
                    RobotsAiHandlerClass::Ew08Flambe | RobotsAiHandlerClass::Ew08FlambeLarge
                ) {
                    flambe_behavior_winner(
                        patrol_priority,
                        pursue_ready,
                        nav_priority,
                        direct_turn_attack_priority,
                        circle_target_node_priority,
                        common_hit_priority,
                        scrambled_priority,
                        electro_priority,
                        magnetic_priority,
                    )
                } else if config.handler_class == RobotsAiHandlerClass::GuardBot {
                    guardbot_behavior_winner(
                        patrol_priority,
                        pursue_ready,
                        stalk_priority,
                        nav_priority,
                        common_hit_priority,
                        scrambled_priority,
                        electro_priority,
                        magnetic_priority,
                        attack_group_priority,
                        circle_target_node_priority,
                    )
                } else if config.handler_class == RobotsAiHandlerClass::ThiefBot {
                    thiefbot_behavior_winner(
                        patrol_priority,
                        pursue_ready,
                        nav_priority,
                        periodic_priority,
                        common_hit_priority,
                        scrambled_priority,
                        electro_priority,
                        magnetic_priority,
                        attack_group_priority,
                        stalk_priority,
                    )
                } else if config.handler_class == RobotsAiHandlerClass::ShieldBot {
                    shieldbot_behavior_winner(
                        patrol_priority,
                        pursue_ready,
                        stalk_priority,
                        nav_priority,
                        periodic_priority,
                        common_hit_priority,
                        scrambled_priority,
                        electro_priority,
                        magnetic_priority,
                        attack_group_priority,
                    )
                } else if config.handler_class == RobotsAiHandlerClass::Eb07MineBot {
                    eb07_minebot_behavior_winner(
                        patrol_priority,
                        nav_priority,
                        periodic_priority,
                        common_hit_priority,
                        scrambled_priority,
                        electro_priority,
                        magnetic_priority,
                        flee_priority,
                        attack_group_priority,
                    )
                } else if config.handler_class == RobotsAiHandlerClass::Eb13KnightBot {
                    knightbot_behavior_winner(
                        patrol_priority,
                        pursue_ready,
                        stalk_priority,
                        nav_priority,
                        periodic_priority,
                        attack_group_priority,
                        common_hit_priority,
                        scrambled_priority,
                        electro_priority,
                        magnetic_priority,
                    )
                } else if config.handler_class == RobotsAiHandlerClass::Eq03Spider {
                    eq03_spider_behavior_winner(
                        patrol_priority,
                        scrambled_priority,
                        attack_group_priority,
                        electro_priority,
                        magnetic_priority,
                    )
                } else if config.handler_class == RobotsAiHandlerClass::Sweeper {
                    sweeper_behavior_winner(
                        periodic_priority,
                        pursue_ready,
                        stalk_priority,
                        nav_priority,
                        common_hit_priority,
                        scrambled_priority,
                        electro_priority,
                        magnetic_priority,
                        attack_group_priority,
                    )
                } else {
                    standard_monster_behavior_winner(
                        attack_group_priority,
                        shunt_attack_priority,
                        spike_attack_priority,
                        pursue_ready,
                        stalk_priority,
                        periodic_priority,
                        nav_priority,
                        patrol_priority,
                        common_hit_priority,
                        scrambled_priority,
                        electro_priority,
                        magnetic_priority,
                    )
                };
                (attack_priorities, winner, runtime.active_node)
            };

            if previous_node != winner {
                if let Some(previous_node) = previous_node {
                    let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                    match previous_node {
                        RobotsStandardMonsterBehaviorWinner::PeriodicIdle => {
                            leave_periodic_idle(&mut runtime.periodic_idle);
                        }
                        RobotsStandardMonsterBehaviorWinner::PursueNavMesh => {
                            runtime.pursue_navmesh.leave();
                        }
                        RobotsStandardMonsterBehaviorWinner::StalkNavMesh => {
                            leave_stalk_navmesh(&mut runtime.stalk_navmesh);
                        }
                        RobotsStandardMonsterBehaviorWinner::CommonHit => {
                            runtime.common_hit.active = false;
                        }
                        RobotsStandardMonsterBehaviorWinner::ScrambledHit => {
                            runtime.scrambled_hit.leave();
                        }
                        RobotsStandardMonsterBehaviorWinner::ElectroHit => {
                            runtime.electro_hit.leave();
                        }
                        RobotsStandardMonsterBehaviorWinner::MagneticHit => {
                            runtime.magnetic_hit.leave();
                        }
                        RobotsStandardMonsterBehaviorWinner::DirectTurnThenAttack => {
                            leave_turn_then_attack(&mut runtime.direct_turn_then_attack);
                        }
                        RobotsStandardMonsterBehaviorWinner::AttackGroup => {
                            if let Some(index) = runtime.attack_group.selected_index {
                                if config.turn_then_attacks[index].is_some() {
                                    leave_turn_then_attack(&mut runtime.turn_then_attacks[index]);
                                } else if config.attack_group_three_phase_index() == Some(index) {
                                    leave_three_phase_attack(&mut runtime.attack_group_three_phase);
                                } else if let Some(attack) = runtime.attacks.get_mut(index) {
                                    leave_generic_attack(attack);
                                }
                            }
                            leave_generic_attack_group(&mut runtime.attack_group);
                            runtime.clear_hit_queries();
                        }
                        RobotsStandardMonsterBehaviorWinner::ShuntAttack => {
                            leave_shunt_attack(&mut runtime.shunt_attack);
                            runtime.clear_hit_queries();
                        }
                        RobotsStandardMonsterBehaviorWinner::SpikeAttack => {
                            leave_spike_attack(&mut runtime.spike_attack);
                            runtime.clear_hit_queries();
                        }
                        RobotsStandardMonsterBehaviorWinner::FollowNetworkPath => {
                            runtime.follow_network_path.leave();
                        }
                        RobotsStandardMonsterBehaviorWinner::FollowFlyingPath
                        | RobotsStandardMonsterBehaviorWinner::Patrol
                        | RobotsStandardMonsterBehaviorWinner::ProximityIdleAttack
                        | RobotsStandardMonsterBehaviorWinner::TargetFacingTurn
                        | RobotsStandardMonsterBehaviorWinner::PatrolNavMesh2
                        | RobotsStandardMonsterBehaviorWinner::FleeNavMesh
                        | RobotsStandardMonsterBehaviorWinner::CircleTarget => {}
                    }
                }
                if previous_node == Some(RobotsStandardMonsterBehaviorWinner::SpikeAttack) {
                    if let Some(physics) = self.native_monster_physics.get_mut(&key) {
                        physics.service_attack_bit4(false);
                    }
                }
                if matches!(
                    previous_node,
                    Some(
                        RobotsStandardMonsterBehaviorWinner::CommonHit
                            | RobotsStandardMonsterBehaviorWinner::ElectroHit
                            | RobotsStandardMonsterBehaviorWinner::MagneticHit
                    )
                ) {
                    if let Some(hit) = self.native_ai_hit_reactions.get_mut(&key) {
                        hit.query_serial_snapshot = 0;
                        hit.last_query_serial = 0;
                    }
                }

                if let Some(winner) = winner {
                    let mut entered = true;
                    let mut record_attack = false;
                    match winner {
                        RobotsStandardMonsterBehaviorWinner::FollowFlyingPath
                        | RobotsStandardMonsterBehaviorWinner::Patrol
                        | RobotsStandardMonsterBehaviorWinner::ProximityIdleAttack
                        | RobotsStandardMonsterBehaviorWinner::TargetFacingTurn => {}
                        RobotsStandardMonsterBehaviorWinner::CircleTarget => {
                            if let Some(circle) = config.circle_target() {
                                let draw = if circle.direction_mode
                                    == ROBOTS_CIRCLE_TARGET_DIRECTION_RANDOM
                                {
                                    self.next_native_ai_gameplay_rng_u32()
                                } else {
                                    None
                                };
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                entered =
                                    enter_circle_target(&mut runtime.circle_target, circle, draw);
                            } else {
                                entered = false;
                            }
                        }
                        RobotsStandardMonsterBehaviorWinner::FollowNetworkPath => {
                            if let Some(nodes) = ew09_path_nodes.as_ref() {
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                entered = runtime.follow_network_path.enter(nodes.len());
                            } else {
                                entered = false;
                            }
                        }
                        RobotsStandardMonsterBehaviorWinner::PeriodicIdle => {
                            let select_draw = self.next_native_ai_gameplay_rng_u32();
                            let delay_draw =
                                select_draw.and_then(|_| self.next_native_ai_gameplay_rng_u32());
                            if let (Some(select_draw), Some(delay_draw)) = (select_draw, delay_draw)
                            {
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                entered = enter_periodic_idle(
                                    &mut runtime.periodic_idle,
                                    config.periodic_idle_base_delay_ticks,
                                    &config.periodic_idle_anim_modes,
                                    select_draw,
                                    delay_draw,
                                )
                                .is_some();
                            } else {
                                entered = false;
                            }
                        }
                        RobotsStandardMonsterBehaviorWinner::PatrolNavMesh2 => {
                            let Some((nav, current_face, group_flags0)) = nav_context else {
                                self.native_standard_monster_runtime
                                    .get_mut(&key)
                                    .unwrap()
                                    .active_node = None;
                                continue;
                            };
                            let needs_rng = {
                                let runtime =
                                    self.native_standard_monster_runtime.get(&key).unwrap();
                                patrol_navmesh2_enter_needs_rebuild(
                                    &runtime.patrol_navmesh2,
                                    current_face,
                                ) && patrol_navmesh2_rebuild_will_consume_rng(
                                    &runtime.patrol_navmesh2,
                                )
                            };
                            let draws = if needs_rng {
                                let mut values = [0u32; ROBOTS_NPC_FLAG2_ROUTE_SAMPLE_COUNT];
                                let mut complete = true;
                                for value in &mut values {
                                    let Some(draw) = self.next_native_ai_gameplay_rng_u32() else {
                                        complete = false;
                                        break;
                                    };
                                    *value = draw;
                                }
                                complete.then_some(values)
                            } else {
                                None
                            };
                            if needs_rng && draws.is_none() {
                                entered = false;
                            } else {
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                enter_patrol_navmesh2(
                                    &mut runtime.patrol_navmesh2,
                                    nav,
                                    current_face,
                                    group_flags0,
                                    draws,
                                );
                            }
                        }
                        RobotsStandardMonsterBehaviorWinner::FleeNavMesh => {
                            let Some((nav, current_face, group_flags0)) = nav_context else {
                                self.native_standard_monster_runtime
                                    .get_mut(&key)
                                    .unwrap()
                                    .active_node = None;
                                continue;
                            };
                            let needs_rng =
                                self.native_standard_monster_runtime.get(&key).is_some_and(
                                    |runtime| flee_navmesh_enter_needs_rng(&runtime.flee_navmesh),
                                );
                            let draws = if needs_rng {
                                let mut values = [0u32; ROBOTS_FLEE_NAVMESH_ROUTE_SAMPLE_COUNT];
                                let mut complete = true;
                                for value in &mut values {
                                    let Some(draw) = self.next_native_ai_gameplay_rng_u32() else {
                                        complete = false;
                                        break;
                                    };
                                    *value = draw;
                                }
                                complete.then_some(values)
                            } else {
                                None
                            };
                            if needs_rng && draws.is_none() {
                                entered = false;
                            } else {
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                entered = enter_flee_navmesh(
                                    &mut runtime.flee_navmesh,
                                    nav,
                                    current_face,
                                    group_flags0,
                                    draws,
                                );
                            }
                        }
                        RobotsStandardMonsterBehaviorWinner::PursueNavMesh => {
                            self.native_standard_monster_runtime
                                .get_mut(&key)
                                .unwrap()
                                .pursue_navmesh
                                .enter(config.pursue_prelude_anim_mode);
                        }
                        RobotsStandardMonsterBehaviorWinner::StalkNavMesh => {
                            if config.stalk.is_some() {
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                enter_stalk_navmesh(&mut runtime.stalk_navmesh);
                            } else {
                                entered = false;
                            }
                        }
                        RobotsStandardMonsterBehaviorWinner::CommonHit => {
                            if let (Some(hit_config), Some(source_yaw), Some(hit)) = (
                                config.common_hit,
                                self.native_ai_last_hit_source_yaw.get(&key).copied(),
                                self.native_ai_hit_reactions.get(&key).copied(),
                            ) {
                                self.native_standard_monster_runtime
                                    .get_mut(&key)
                                    .unwrap()
                                    .common_hit
                                    .enter_configured(
                                        hit_config,
                                        owner_yaw_radians,
                                        source_yaw,
                                        hit.last_query_serial,
                                    );
                            } else {
                                entered = false;
                            }
                        }
                        RobotsStandardMonsterBehaviorWinner::ScrambledHit => {
                            self.native_standard_monster_runtime
                                .get_mut(&key)
                                .unwrap()
                                .scrambled_hit
                                .enter();
                        }
                        RobotsStandardMonsterBehaviorWinner::ElectroHit => {
                            self.native_standard_monster_runtime
                                .get_mut(&key)
                                .unwrap()
                                .electro_hit
                                .enter();
                        }
                        RobotsStandardMonsterBehaviorWinner::MagneticHit => {
                            let runtime =
                                self.native_standard_monster_runtime.get_mut(&key).unwrap();
                            runtime.magnetic_hit.enter(owner_position.y);
                            if runtime.magnetic_drop_charge_count.is_none() {
                                runtime.magnetic_drop_charge_count =
                                    Some(visual.pickup_drop_count.unwrap_or(0));
                            }
                        }
                        RobotsStandardMonsterBehaviorWinner::DirectTurnThenAttack => {
                            if config.direct_turn_then_attack().is_some() {
                                enter_turn_then_attack(
                                    &mut self
                                        .native_standard_monster_runtime
                                        .get_mut(&key)
                                        .unwrap()
                                        .direct_turn_then_attack,
                                );
                                record_attack = true;
                            } else {
                                entered = false;
                            }
                        }
                        RobotsStandardMonsterBehaviorWinner::AttackGroup => {
                            let draw = self.next_native_ai_gameplay_rng_u32();
                            if let Some(draw) = draw {
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                if let Some(index) = enter_generic_attack_group(
                                    &mut runtime.attack_group,
                                    &attack_priorities,
                                    Some(draw),
                                ) {
                                    if config.turn_then_attacks[index].is_some() {
                                        enter_turn_then_attack(
                                            &mut runtime.turn_then_attacks[index],
                                        );
                                        record_attack = true;
                                    } else if config.attack_group_three_phase_index() == Some(index)
                                    {
                                        if config.attack_group_three_phase().is_some() {
                                            enter_three_phase_attack(
                                                &mut runtime.attack_group_three_phase,
                                            );
                                        } else {
                                            leave_generic_attack_group(&mut runtime.attack_group);
                                            entered = false;
                                        }
                                    } else if let Some(attack) = config.attacks[index] {
                                        enter_generic_attack(&mut runtime.attacks[index], attack);
                                        record_attack = true;
                                    } else {
                                        leave_generic_attack_group(&mut runtime.attack_group);
                                        entered = false;
                                    }
                                } else {
                                    entered = false;
                                }
                            } else {
                                entered = false;
                            }
                        }
                        RobotsStandardMonsterBehaviorWinner::ShuntAttack => {
                            if config.shunt_attack.is_some() {
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                enter_shunt_attack(&mut runtime.shunt_attack);
                                runtime.clear_hit_queries();
                                record_attack = true;
                            } else {
                                entered = false;
                            }
                        }
                        RobotsStandardMonsterBehaviorWinner::SpikeAttack => {
                            if config.spike_attack.is_some() {
                                {
                                    let runtime =
                                        self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                    enter_spike_attack(&mut runtime.spike_attack);
                                    runtime.clear_hit_queries();
                                }
                                self.native_monster_physics
                                    .entry(key)
                                    .or_insert_with(
                                        RobotsCharacterPhysicsRuntimeState::monster_ctor_default,
                                    )
                                    .service_attack_bit4(true);
                            } else {
                                entered = false;
                            }
                        }
                    }
                    self.native_standard_monster_runtime
                        .get_mut(&key)
                        .unwrap()
                        .active_node = entered.then_some(winner);
                    if record_attack {
                        // Generic Attack enter `0x0044F100` writes DAT_007B2A3C even
                        // though MalfBot's own +0x158 does not read that cooldown.
                        self.native_monster_attack_cooldown.record_attack();
                    }
                } else {
                    self.native_standard_monster_runtime
                        .get_mut(&key)
                        .unwrap()
                        .active_node = None;
                }
            }

            let active_node = self
                .native_standard_monster_runtime
                .get(&key)
                .and_then(|runtime| runtime.active_node);
            let mut projectile_events =
                Vec::<(RobotsCreateProjectileRequest, u32, f32, Vec3, Quat)>::new();
            let mut pending_standard_hit_queries = Vec::<(RobotsHitQueryInitPlan, u32)>::new();

            match active_node {
                Some(RobotsStandardMonsterBehaviorWinner::FollowFlyingPath) => {
                    let path_step =
                        self.native_standard_monster_runtime
                            .get_mut(&key)
                            .and_then(|runtime| {
                                runtime.follow_flying_path.advance_fixed(
                                    RobotsFollowFlyingPathConfig::ef01(),
                                    owner_yaw_radians,
                                    1.0,
                                )
                            });
                    if let Some(path_step) = path_step {
                        let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                        if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                            if let Some(position_xyz) = path_step.owner_position_xyz {
                                body.owner_position = Vec3::from_array(position_xyz);
                            }
                            body.owner_rotation =
                                Quat::from_rotation_y(path_step.owner_yaw_radians);
                            let _ = runtime.advance_animation(
                                body,
                                ROBOTS_ANIM_MODE_MOVE,
                                Some(path_step.owner_yaw_radians),
                                ROBOTS_FIXED_STEP_SECONDS,
                                NativeAiRootMotionPolicy::NONE,
                            );
                        }
                    }
                }
                Some(RobotsStandardMonsterBehaviorWinner::Patrol) => {
                    let locomotion = {
                        let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                        let patrol = step_ai_patrol(&mut runtime.patrol, config.patrol);
                        let move_mode_active_on_entry =
                            runtime.animation.is_mode(ROBOTS_ANIM_MODE_MOVE);
                        runtime.step_standard_locomotion(RobotsAiLocomotionInput {
                            steering_target_yaw_radians: patrol.steering_target_yaw_radians,
                            target_locomotion_scalar: patrol.target_locomotion_scalar,
                            turn_rate: patrol.turn_rate,
                            handler_flags_628: effective_handler_flags,
                            move_mode_active_on_entry,
                            current_owner_yaw_radians: owner_yaw_radians,
                            runtime_rate_scale: 1.0,
                        })
                    };
                    if let Some(locomotion) = locomotion {
                        commit_native_standard_physics_locomotion(
                            &mut self.native_monster_physics,
                            key,
                            config.handler_class,
                            locomotion,
                        );
                        let owner_yaw_write = locomotion
                            .direct_owner_yaw_write
                            .then_some(locomotion.owner_yaw_radians);
                        let policy = locomotion_root_motion_policy(locomotion);
                        let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                        if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                            let _ = runtime.advance_animation(
                                body,
                                locomotion.requested_anim_mode,
                                owner_yaw_write,
                                ROBOTS_FIXED_STEP_SECONDS,
                                policy,
                            );
                        }
                    }
                }
                Some(RobotsStandardMonsterBehaviorWinner::CircleTarget) => {
                    let circle_step = config.circle_target().and_then(|circle| {
                        self.native_standard_monster_runtime
                            .get(&key)
                            .and_then(|runtime| {
                                step_circle_target(
                                    runtime.circle_target,
                                    circle,
                                    owner_position.to_array(),
                                    Some(player_position.to_array()),
                                )
                            })
                    });
                    if let Some(circle_step) = circle_step {
                        let locomotion = {
                            let runtime =
                                self.native_standard_monster_runtime.get_mut(&key).unwrap();
                            let move_mode_active_on_entry =
                                runtime.animation.is_mode(ROBOTS_ANIM_MODE_MOVE);
                            runtime.step_standard_locomotion(RobotsAiLocomotionInput {
                                steering_target_yaw_radians: circle_step
                                    .steering_target_yaw_radians,
                                target_locomotion_scalar: circle_step.target_locomotion_scalar,
                                turn_rate: circle_step.turn_rate,
                                handler_flags_628: effective_handler_flags,
                                move_mode_active_on_entry,
                                current_owner_yaw_radians: owner_yaw_radians,
                                runtime_rate_scale: 1.0,
                            })
                        };
                        if let Some(locomotion) = locomotion {
                            commit_native_standard_physics_locomotion(
                                &mut self.native_monster_physics,
                                key,
                                config.handler_class,
                                locomotion,
                            );
                            let owner_yaw_write = locomotion
                                .direct_owner_yaw_write
                                .then_some(locomotion.owner_yaw_radians);
                            let policy = locomotion_root_motion_policy(locomotion);
                            let runtime =
                                self.native_standard_monster_runtime.get_mut(&key).unwrap();
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                let _ = runtime.advance_animation(
                                    body,
                                    locomotion.requested_anim_mode,
                                    owner_yaw_write,
                                    ROBOTS_FIXED_STEP_SECONDS,
                                    policy,
                                );
                            }
                        }
                    }
                }
                Some(RobotsStandardMonsterBehaviorWinner::FollowNetworkPath) => {
                    let path_step = match (ew09_path, ew09_path_nodes.as_ref()) {
                        (Some(path), Some(nodes)) => {
                            let runtime =
                                self.native_standard_monster_runtime.get_mut(&key).unwrap();
                            runtime.follow_network_path.step(
                                ew09_follow_network_path_config(),
                                path.path_type,
                                nodes,
                                owner_position.to_array(),
                            )
                        }
                        _ => None,
                    };
                    if let Some(path_step) = path_step {
                        if let Some(requested_anim_mode) = path_step.requested_anim_mode {
                            let events = {
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                    runtime.advance_animation(
                                        body,
                                        requested_anim_mode,
                                        None,
                                        ROBOTS_FIXED_STEP_SECONDS,
                                        NativeAiRootMotionPolicy::TRANSLATION_ONLY,
                                    )
                                } else {
                                    Vec::new()
                                }
                            };
                            for event in events {
                                match event.event_type {
                                    event_type::SETUP_IDLE => {
                                        let runtime = self
                                            .native_standard_monster_runtime
                                            .get_mut(&key)
                                            .unwrap();
                                        runtime.follow_network_path.setup_idle();
                                        runtime.animation.setup_idle();
                                    }
                                    event_type::CREATE_PROJECTILE => {
                                        if let Some(request) =
                                            RobotsCreateProjectileRequest::from_event(
                                                event.as_view(),
                                            )
                                        {
                                            projectile_events.push((
                                                request,
                                                requested_anim_mode,
                                                event.pose_seconds,
                                                event.owner_position,
                                                event.owner_rotation,
                                            ));
                                        }
                                    }
                                    event_type::HIT_CHECK => {
                                        if let Some(plan) =
                                            RobotsHitQueryInitPlan::from_event(event.as_view())
                                        {
                                            pending_standard_hit_queries
                                                .push((plan, requested_anim_mode));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        } else if path_step.movement_enabled {
                            let locomotion = {
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                let move_mode_active_on_entry =
                                    runtime.animation.is_mode(ROBOTS_ANIM_MODE_MOVE);
                                runtime.step_standard_locomotion(RobotsAiLocomotionInput {
                                    steering_target_yaw_radians: path_step.target_yaw_radians,
                                    target_locomotion_scalar: path_step.target_locomotion_scalar,
                                    turn_rate: RobotsAiTurnRateInput::Default,
                                    handler_flags_628: effective_handler_flags,
                                    move_mode_active_on_entry,
                                    current_owner_yaw_radians: owner_yaw_radians,
                                    runtime_rate_scale: 1.0,
                                })
                            };
                            if let Some(locomotion) = locomotion {
                                commit_native_standard_physics_locomotion(
                                    &mut self.native_monster_physics,
                                    key,
                                    config.handler_class,
                                    locomotion,
                                );
                                let owner_yaw_write = locomotion
                                    .direct_owner_yaw_write
                                    .then_some(locomotion.owner_yaw_radians);
                                let policy = locomotion_root_motion_policy(locomotion);
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                    let _ = runtime.advance_animation(
                                        body,
                                        locomotion.requested_anim_mode,
                                        owner_yaw_write,
                                        ROBOTS_FIXED_STEP_SECONDS,
                                        policy,
                                    );
                                }
                            }
                        }
                    }
                }
                Some(RobotsStandardMonsterBehaviorWinner::ProximityIdleAttack) => {
                    let proximity = RobotsProximityAnimBehaviorConfig::magnabot_idle_attack();
                    let events = {
                        let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                        if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                            if body.animation_modes.contains_key(&proximity.anim_mode)
                                || body
                                    .animation_mode_scripts
                                    .contains_key(&proximity.anim_mode)
                            {
                                runtime.advance_animation(
                                    body,
                                    proximity.anim_mode,
                                    None,
                                    ROBOTS_FIXED_STEP_SECONDS,
                                    NativeAiRootMotionPolicy::NONE,
                                )
                            } else {
                                // Native 0x004051E0 only stages a pending AnimMode.
                                // EB11 ships without mode04, so do not synthesize it
                                // as the current pose in the editor runtime.
                                Vec::new()
                            }
                        } else {
                            Vec::new()
                        }
                    };
                    if events
                        .iter()
                        .any(|event| event.event_type == event_type::SETUP_IDLE)
                    {
                        self.native_standard_monster_runtime
                            .get_mut(&key)
                            .unwrap()
                            .animation
                            .setup_idle();
                    }
                }
                Some(RobotsStandardMonsterBehaviorWinner::TargetFacingTurn) => {
                    let target_delta = player_position - owner_position;
                    let target_yaw_radians = target_delta.x.atan2(target_delta.z);
                    let target_yaw_error_radians =
                        shortest_yaw_delta(owner_yaw_radians, target_yaw_radians);
                    let turn_request = target_facing_turn_request(
                        RobotsTargetFacingTurnBehaviorConfig::magnabot(),
                        target_yaw_error_radians,
                    );
                    let locomotion =
                        self.native_standard_monster_runtime
                            .get_mut(&key)
                            .and_then(|runtime| {
                                runtime.step_magnabot_direct_turn(
                                    turn_request.yaw_error_radians,
                                    turn_request.turn_rate_radians_per_second,
                                    turn_request.anim_mode,
                                    visual.handler_flags_628,
                                    owner_yaw_radians,
                                )
                            });
                    if let Some(locomotion) = locomotion {
                        commit_native_standard_physics_locomotion(
                            &mut self.native_monster_physics,
                            key,
                            config.handler_class,
                            locomotion,
                        );
                        let owner_yaw_write = locomotion
                            .direct_owner_yaw_write
                            .then_some(locomotion.owner_yaw_radians);
                        let policy = locomotion_root_motion_policy(locomotion);
                        let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                        if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                            let _ = runtime.advance_animation(
                                body,
                                locomotion.requested_anim_mode,
                                owner_yaw_write,
                                ROBOTS_FIXED_STEP_SECONDS,
                                policy,
                            );
                        }
                    }
                }
                Some(RobotsStandardMonsterBehaviorWinner::PeriodicIdle) => {
                    let requested = self
                        .native_standard_monster_runtime
                        .get(&key)
                        .and_then(|runtime| runtime.periodic_idle.selected_anim_mode);
                    if let Some(requested) = requested {
                        let events = {
                            let runtime =
                                self.native_standard_monster_runtime.get_mut(&key).unwrap();
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                runtime.advance_animation(
                                    body,
                                    requested,
                                    None,
                                    ROBOTS_FIXED_STEP_SECONDS,
                                    NativeAiRootMotionPolicy::NONE,
                                )
                            } else {
                                Vec::new()
                            }
                        };
                        if events
                            .iter()
                            .any(|event| event.event_type == event_type::SETUP_IDLE)
                        {
                            let runtime =
                                self.native_standard_monster_runtime.get_mut(&key).unwrap();
                            complete_periodic_idle_setup_idle(&mut runtime.periodic_idle);
                            runtime.animation.setup_idle();
                        }
                    }
                }
                Some(RobotsStandardMonsterBehaviorWinner::PatrolNavMesh2) => {
                    if let Some((nav, current_face, group_flags0)) = nav_context {
                        let step_needs_rng = {
                            let runtime = self.native_standard_monster_runtime.get(&key).unwrap();
                            patrol_navmesh2_step_needs_rebuild(
                                &runtime.patrol_navmesh2,
                                movement_error_ratio,
                            ) && patrol_navmesh2_rebuild_will_consume_rng(&runtime.patrol_navmesh2)
                        };
                        let step_draws = if step_needs_rng {
                            let mut values = [0u32; ROBOTS_NPC_FLAG2_ROUTE_SAMPLE_COUNT];
                            let mut complete = true;
                            for value in &mut values {
                                let Some(draw) = self.next_native_ai_gameplay_rng_u32() else {
                                    complete = false;
                                    break;
                                };
                                *value = draw;
                            }
                            complete.then_some(values)
                        } else {
                            None
                        };
                        if !step_needs_rng || step_draws.is_some() {
                            let action = {
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                step_patrol_navmesh2(
                                    &mut runtime.patrol_navmesh2,
                                    nav,
                                    owner_position.to_array(),
                                    current_face,
                                    group_flags0,
                                    movement_error_ratio,
                                    step_draws,
                                )
                            };
                            match action {
                                RobotsPatrolNavMesh2Action::RequestIdleAttack => {
                                    let events = {
                                        let runtime = self
                                            .native_standard_monster_runtime
                                            .get_mut(&key)
                                            .unwrap();
                                        if let Some(body) =
                                            self.runtime_character_bodies.get_mut(&key)
                                        {
                                            runtime.advance_animation(
                                                body,
                                                ROBOTS_ANIM_MODE_IDLE_ATTACK,
                                                None,
                                                ROBOTS_FIXED_STEP_SECONDS,
                                                NativeAiRootMotionPolicy::NONE,
                                            )
                                        } else {
                                            Vec::new()
                                        }
                                    };
                                    if events
                                        .iter()
                                        .any(|event| event.event_type == event_type::SETUP_IDLE)
                                    {
                                        self.native_standard_monster_runtime
                                            .get_mut(&key)
                                            .unwrap()
                                            .animation
                                            .setup_idle();
                                    }
                                }
                                RobotsPatrolNavMesh2Action::SteerToWaypoint {
                                    target_yaw_radians,
                                    target_locomotion_scalar,
                                    turn_rate_radians_per_second,
                                } => {
                                    let locomotion = {
                                        let runtime = self
                                            .native_standard_monster_runtime
                                            .get_mut(&key)
                                            .unwrap();
                                        let move_mode_active_on_entry =
                                            runtime.animation.is_mode(ROBOTS_ANIM_MODE_MOVE);
                                        runtime.step_standard_locomotion(RobotsAiLocomotionInput {
                                            steering_target_yaw_radians: target_yaw_radians,
                                            target_locomotion_scalar,
                                            turn_rate: RobotsAiTurnRateInput::Explicit(
                                                turn_rate_radians_per_second,
                                            ),
                                            handler_flags_628: visual.handler_flags_628,
                                            move_mode_active_on_entry,
                                            current_owner_yaw_radians: owner_yaw_radians,
                                            runtime_rate_scale: 1.0,
                                        })
                                    };
                                    if let Some(locomotion) = locomotion {
                                        commit_native_standard_physics_locomotion(
                                            &mut self.native_monster_physics,
                                            key,
                                            config.handler_class,
                                            locomotion,
                                        );
                                        let owner_yaw_write = locomotion
                                            .direct_owner_yaw_write
                                            .then_some(locomotion.owner_yaw_radians);
                                        let policy = locomotion_root_motion_policy(locomotion);
                                        let runtime = self
                                            .native_standard_monster_runtime
                                            .get_mut(&key)
                                            .unwrap();
                                        if let Some(body) =
                                            self.runtime_character_bodies.get_mut(&key)
                                        {
                                            let _ = runtime.advance_animation(
                                                body,
                                                locomotion.requested_anim_mode,
                                                owner_yaw_write,
                                                ROBOTS_FIXED_STEP_SECONDS,
                                                policy,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Some(RobotsStandardMonsterBehaviorWinner::FleeNavMesh) => {
                    if let Some((nav, current_face, group_flags0)) = nav_context {
                        let needs_rng =
                            self.native_standard_monster_runtime
                                .get(&key)
                                .is_some_and(|runtime| {
                                    flee_navmesh_step_needs_rng(
                                        &runtime.flee_navmesh,
                                        owner_position.to_array(),
                                    )
                                });
                        let draws = if needs_rng {
                            let mut values = [0u32; ROBOTS_FLEE_NAVMESH_ROUTE_SAMPLE_COUNT];
                            let mut complete = true;
                            for value in &mut values {
                                let Some(draw) = self.next_native_ai_gameplay_rng_u32() else {
                                    complete = false;
                                    break;
                                };
                                *value = draw;
                            }
                            complete.then_some(values)
                        } else {
                            None
                        };
                        if !needs_rng || draws.is_some() {
                            let action = {
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                step_flee_navmesh(
                                    &mut runtime.flee_navmesh,
                                    nav,
                                    owner_position.to_array(),
                                    current_face,
                                    group_flags0,
                                    draws,
                                )
                            };
                            match action {
                                Some(RobotsFleeNavMeshAction::RequestIdleAttack) => {
                                    let events = {
                                        let runtime = self
                                            .native_standard_monster_runtime
                                            .get_mut(&key)
                                            .unwrap();
                                        if let Some(body) =
                                            self.runtime_character_bodies.get_mut(&key)
                                        {
                                            runtime.advance_animation(
                                                body,
                                                ROBOTS_ANIM_MODE_IDLE_ATTACK,
                                                None,
                                                ROBOTS_FIXED_STEP_SECONDS,
                                                NativeAiRootMotionPolicy::NONE,
                                            )
                                        } else {
                                            Vec::new()
                                        }
                                    };
                                    if events
                                        .iter()
                                        .any(|event| event.event_type == event_type::SETUP_IDLE)
                                    {
                                        self.native_standard_monster_runtime
                                            .get_mut(&key)
                                            .unwrap()
                                            .animation
                                            .setup_idle();
                                    }
                                }
                                Some(RobotsFleeNavMeshAction::SteerToWaypoint {
                                    target_yaw_radians,
                                    target_locomotion_scalar,
                                    turn_rate_radians_per_second,
                                }) => {
                                    let locomotion = {
                                        let runtime = self
                                            .native_standard_monster_runtime
                                            .get_mut(&key)
                                            .unwrap();
                                        let move_mode_active_on_entry =
                                            runtime.animation.is_mode(ROBOTS_ANIM_MODE_MOVE);
                                        runtime.step_standard_locomotion(RobotsAiLocomotionInput {
                                            steering_target_yaw_radians: target_yaw_radians,
                                            target_locomotion_scalar,
                                            turn_rate: RobotsAiTurnRateInput::Explicit(
                                                turn_rate_radians_per_second,
                                            ),
                                            handler_flags_628: visual.handler_flags_628,
                                            move_mode_active_on_entry,
                                            current_owner_yaw_radians: owner_yaw_radians,
                                            runtime_rate_scale: 1.0,
                                        })
                                    };
                                    if let Some(locomotion) = locomotion {
                                        commit_native_standard_physics_locomotion(
                                            &mut self.native_monster_physics,
                                            key,
                                            config.handler_class,
                                            locomotion,
                                        );
                                        let owner_yaw_write = locomotion
                                            .direct_owner_yaw_write
                                            .then_some(locomotion.owner_yaw_radians);
                                        let policy = locomotion_root_motion_policy(locomotion);
                                        let runtime = self
                                            .native_standard_monster_runtime
                                            .get_mut(&key)
                                            .unwrap();
                                        if let Some(body) =
                                            self.runtime_character_bodies.get_mut(&key)
                                        {
                                            let _ = runtime.advance_animation(
                                                body,
                                                locomotion.requested_anim_mode,
                                                owner_yaw_write,
                                                ROBOTS_FIXED_STEP_SECONDS,
                                                policy,
                                            );
                                        }
                                    }
                                }
                                None => {}
                            }
                        }
                    }
                }
                Some(RobotsStandardMonsterBehaviorWinner::PursueNavMesh) => {
                    if let (Some((nav, _, _)), Some(input)) = (nav_context, pursue_input) {
                        let step = self
                            .native_standard_monster_runtime
                            .get_mut(&key)
                            .unwrap()
                            .pursue_navmesh
                            .step_steering(nav, input);
                        if let Some(turn) = step.direct_turn {
                            let owner_yaw_write = turn
                                .direct_owner_yaw_write
                                .then_some(turn.owner_yaw_radians);
                            let policy = if !turn.direct_owner_yaw_write
                                && matches!(
                                    turn.requested_anim_mode,
                                    ROBOTS_ANIM_MODE_TURN_ON_SPOT_L
                                        | ROBOTS_ANIM_MODE_TURN_ON_SPOT_R
                                ) {
                                NativeAiRootMotionPolicy::FULL
                            } else {
                                NativeAiRootMotionPolicy::NONE
                            };
                            let runtime =
                                self.native_standard_monster_runtime.get_mut(&key).unwrap();
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                let _ = runtime.advance_animation(
                                    body,
                                    turn.requested_anim_mode,
                                    owner_yaw_write,
                                    ROBOTS_FIXED_STEP_SECONDS,
                                    policy,
                                );
                            }
                        } else if let Some(requested_anim_mode) = step.requested_anim_mode {
                            let events = {
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                    runtime.advance_animation(
                                        body,
                                        requested_anim_mode,
                                        None,
                                        ROBOTS_FIXED_STEP_SECONDS,
                                        NativeAiRootMotionPolicy::FULL,
                                    )
                                } else {
                                    Vec::new()
                                }
                            };
                            if events
                                .iter()
                                .any(|event| event.event_type == event_type::SETUP_IDLE)
                            {
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                runtime.pursue_navmesh.setup_idle();
                                runtime.animation.setup_idle();
                            }
                        } else if let Some(target_yaw_radians) = step.target_yaw_radians {
                            let locomotion = {
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                let move_mode_active_on_entry =
                                    runtime.animation.is_mode(ROBOTS_ANIM_MODE_MOVE);
                                runtime.step_standard_locomotion(RobotsAiLocomotionInput {
                                    steering_target_yaw_radians: target_yaw_radians,
                                    target_locomotion_scalar: step.target_locomotion_scalar,
                                    turn_rate: RobotsAiTurnRateInput::Default,
                                    handler_flags_628: visual.handler_flags_628,
                                    move_mode_active_on_entry,
                                    current_owner_yaw_radians: owner_yaw_radians,
                                    runtime_rate_scale: 1.0,
                                })
                            };
                            if let Some(locomotion) = locomotion {
                                commit_native_standard_physics_locomotion(
                                    &mut self.native_monster_physics,
                                    key,
                                    config.handler_class,
                                    locomotion,
                                );
                                let owner_yaw_write = locomotion
                                    .direct_owner_yaw_write
                                    .then_some(locomotion.owner_yaw_radians);
                                let policy = locomotion_root_motion_policy(locomotion);
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                    let _ = runtime.advance_animation(
                                        body,
                                        locomotion.requested_anim_mode,
                                        owner_yaw_write,
                                        ROBOTS_FIXED_STEP_SECONDS,
                                        policy,
                                    );
                                }
                            }
                        }
                    }
                }
                Some(RobotsStandardMonsterBehaviorWinner::StalkNavMesh) => {
                    let Some(stalk_config) = config.stalk else {
                        continue;
                    };
                    let target_delta = player_position - owner_position;
                    let target_yaw_radians = target_delta.x.atan2(target_delta.z);
                    let relative_target_yaw_radians =
                        shortest_yaw_delta(owner_yaw_radians, target_yaw_radians);
                    let throttle_requested = self
                        .native_standard_monster_runtime
                        .get(&key)
                        .is_some_and(|runtime| {
                            stalk_navmesh_requests_event_throttle(
                                runtime.stalk_navmesh,
                                stalk_config,
                            )
                        });
                    let event_throttle_granted = throttle_requested
                        && self
                            .native_ai_event_throttle
                            .request(ROBOTS_AI_EVENT_THROTTLE_STALK_ID);
                    let stalk_step = {
                        let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                        step_stalk_navmesh(
                            &mut runtime.stalk_navmesh,
                            stalk_config,
                            target_yaw_radians,
                            relative_target_yaw_radians,
                            event_throttle_granted,
                        )
                    };

                    let events = if let Some(steering_target_yaw_radians) =
                        stalk_step.steering_target_yaw_radians
                    {
                        let locomotion = {
                            let runtime =
                                self.native_standard_monster_runtime.get_mut(&key).unwrap();
                            let move_mode_active_on_entry =
                                runtime.animation.is_mode(ROBOTS_ANIM_MODE_MOVE);
                            runtime.step_standard_locomotion(RobotsAiLocomotionInput {
                                steering_target_yaw_radians,
                                target_locomotion_scalar: stalk_step.target_locomotion_scalar,
                                turn_rate: stalk_step.turn_rate,
                                handler_flags_628: visual.handler_flags_628,
                                move_mode_active_on_entry,
                                current_owner_yaw_radians: owner_yaw_radians,
                                runtime_rate_scale: 1.0,
                            })
                        };
                        if let Some(locomotion) = locomotion {
                            commit_native_standard_physics_locomotion(
                                &mut self.native_monster_physics,
                                key,
                                config.handler_class,
                                locomotion,
                            );
                            let owner_yaw_write = locomotion
                                .direct_owner_yaw_write
                                .then_some(locomotion.owner_yaw_radians);
                            let policy = locomotion_root_motion_policy(locomotion);
                            let runtime =
                                self.native_standard_monster_runtime.get_mut(&key).unwrap();
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                runtime.advance_animation(
                                    body,
                                    locomotion.requested_anim_mode,
                                    owner_yaw_write,
                                    ROBOTS_FIXED_STEP_SECONDS,
                                    policy,
                                )
                            } else {
                                Vec::new()
                            }
                        } else {
                            Vec::new()
                        }
                    } else {
                        let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                        if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                            runtime.advance_animation(
                                body,
                                stalk_step.requested_anim_mode,
                                None,
                                ROBOTS_FIXED_STEP_SECONDS,
                                NativeAiRootMotionPolicy::NONE,
                            )
                        } else {
                            Vec::new()
                        }
                    };

                    if events
                        .iter()
                        .any(|event| event.event_type == event_type::SETUP_IDLE)
                    {
                        let completion_draw = stalk_step
                            .service_handler_130
                            .then(|| self.next_native_ai_gameplay_rng_u32())
                            .flatten();
                        let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                        if stalk_step.service_handler_130 {
                            if let Some(draw) = completion_draw {
                                complete_stalk_navmesh_animation(
                                    &mut runtime.stalk_navmesh,
                                    stalk_config,
                                    draw,
                                );
                                runtime.animation.setup_idle();
                            }
                        } else {
                            runtime.animation.setup_idle();
                        }
                    }
                }
                Some(RobotsStandardMonsterBehaviorWinner::CommonHit) => {
                    let Some(hit_config) = config.common_hit else {
                        continue;
                    };
                    let reenter = self
                        .native_ai_hit_reactions
                        .get(&key)
                        .and_then(|hit| {
                            self.native_standard_monster_runtime
                                .get(&key)
                                .map(|runtime| {
                                    hit.health != 0
                                        && hit.query_serial_snapshot != u16::MAX
                                        && hit.last_query_serial
                                            != runtime.common_hit.captured_query_serial
                                })
                        })
                        .unwrap_or(false);
                    if reenter {
                        if let (Some(source_yaw), Some(hit)) = (
                            self.native_ai_last_hit_source_yaw.get(&key).copied(),
                            self.native_ai_hit_reactions.get(&key).copied(),
                        ) {
                            let runtime =
                                self.native_standard_monster_runtime.get_mut(&key).unwrap();
                            runtime.animation.setup_idle();
                            runtime.common_hit.enter_configured(
                                hit_config,
                                owner_yaw_radians,
                                source_yaw,
                                hit.last_query_serial,
                            );
                        }
                    }
                    let (requested_anim_mode, owner_yaw_write) = {
                        let runtime = self.native_standard_monster_runtime.get(&key).unwrap();
                        (
                            runtime.common_hit.requested_anim_mode,
                            runtime
                                .common_hit
                                .step_owner_yaw_configured(hit_config, owner_yaw_radians),
                        )
                    };
                    let events = {
                        let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                        if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                            runtime.advance_animation(
                                body,
                                requested_anim_mode,
                                owner_yaw_write,
                                ROBOTS_FIXED_STEP_SECONDS,
                                NativeAiRootMotionPolicy::NONE,
                            )
                        } else {
                            Vec::new()
                        }
                    };
                    if events
                        .iter()
                        .any(|event| event.event_type == event_type::SETUP_IDLE)
                    {
                        let runtimes = &mut self.native_standard_monster_runtime;
                        let hits = &mut self.native_ai_hit_reactions;
                        if let (Some(runtime), Some(hit)) =
                            (runtimes.get_mut(&key), hits.get_mut(&key))
                        {
                            runtime.common_hit.setup_idle(hit);
                            runtime.animation.setup_idle();
                        }
                    }
                }
                Some(RobotsStandardMonsterBehaviorWinner::ScrambledHit) => {
                    let requested_anim_mode = {
                        let runtimes = &mut self.native_standard_monster_runtime;
                        let hits = &mut self.native_ai_hit_reactions;
                        let runtime = runtimes.get_mut(&key).unwrap();
                        let Some(hit) = hits.get_mut(&key) else {
                            continue;
                        };
                        runtime.scrambled_hit.step(&mut hit.got_hit_latch)
                    };
                    if requested_anim_mode != 0 {
                        let events = {
                            let runtime =
                                self.native_standard_monster_runtime.get_mut(&key).unwrap();
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                runtime.advance_animation(
                                    body,
                                    requested_anim_mode,
                                    None,
                                    ROBOTS_FIXED_STEP_SECONDS,
                                    NativeAiRootMotionPolicy::FULL,
                                )
                            } else {
                                Vec::new()
                            }
                        };
                        if events
                            .iter()
                            .any(|event| event.event_type == event_type::SETUP_IDLE)
                        {
                            let runtimes = &mut self.native_standard_monster_runtime;
                            let hits = &mut self.native_ai_hit_reactions;
                            let runtime = runtimes.get_mut(&key).unwrap();
                            if let Some(hit) = hits.get_mut(&key) {
                                runtime.scrambled_hit.setup_idle(&mut hit.got_hit_latch);
                            }
                            runtime.animation.setup_idle();
                        }
                    }
                }
                Some(RobotsStandardMonsterBehaviorWinner::ElectroHit) => {
                    let requested_anim_mode = self
                        .native_standard_monster_runtime
                        .get(&key)
                        .unwrap()
                        .electro_hit
                        .requested_anim_mode();
                    let events = {
                        let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                        if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                            runtime.advance_animation(
                                body,
                                requested_anim_mode,
                                None,
                                ROBOTS_FIXED_STEP_SECONDS,
                                NativeAiRootMotionPolicy::FULL,
                            )
                        } else {
                            Vec::new()
                        }
                    };
                    if events
                        .iter()
                        .any(|event| event.event_type == event_type::SETUP_IDLE)
                    {
                        let plan = {
                            let runtime =
                                self.native_standard_monster_runtime.get_mut(&key).unwrap();
                            let plan = runtime.electro_hit.setup_idle();
                            runtime.animation.setup_idle();
                            plan
                        };
                        if plan.request_natural_death {
                            let death_position = self
                                .runtime_character_bodies
                                .get(&key)
                                .map(|body| body.owner_position)
                                .unwrap_or(owner_position);
                            self.request_native_ai_natural_death(map.hashcode, key, death_position);
                        }
                    }
                }
                Some(RobotsStandardMonsterBehaviorWinner::MagneticHit) => {
                    let attachment_relation_matches =
                        self.native_player_magnetic_target_key == Some(key);
                    let owner_y = self
                        .runtime_character_bodies
                        .get(&key)
                        .map(|body| body.owner_position.y)
                        .unwrap_or(owner_position.y);
                    let plan = {
                        let runtimes = &mut self.native_standard_monster_runtime;
                        let hits = &mut self.native_ai_hit_reactions;
                        let runtime = runtimes.get_mut(&key).unwrap();
                        let Some(hit) = hits.get_mut(&key) else {
                            continue;
                        };
                        runtime.magnetic_hit.step(
                            &mut hit.got_hit_latch,
                            owner_y,
                            attachment_relation_matches,
                            ROBOTS_FIXED_STEP_SECONDS,
                        )
                    };

                    if let Some(value) = plan.set_handler_606 {
                        self.native_monster_physics
                            .entry(key)
                            .or_insert_with(
                                RobotsCharacterPhysicsRuntimeState::monster_ctor_default,
                            )
                            .apply_handler_606_mode(value);
                    }

                    let mut magnetic_vertical_velocity_delta = None;
                    let effect_plan = if plan.service_magnetic_effect {
                        let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                        let drop_charge_count = runtime
                            .magnetic_drop_charge_count
                            .unwrap_or_else(|| visual.pickup_drop_count.unwrap_or(0));
                        let effect_plan = runtime.magnetic_hit.step_attached_effect(
                            RobotsMalfBotMagneticEffectInput {
                                owner_y,
                                magnetic_mass: visual.magnetic_mass,
                                drop_charge_count,
                            },
                        );
                        if effect_plan.serviced {
                            runtime.magnetic_drop_charge_count =
                                Some(effect_plan.next_drop_charge_count);
                            magnetic_vertical_velocity_delta =
                                Some(effect_plan.physics_vertical_velocity_delta);
                        }
                        Some(effect_plan)
                    } else {
                        None
                    };
                    if let Some(delta_y) = magnetic_vertical_velocity_delta {
                        self.native_monster_physics
                            .entry(key)
                            .or_insert_with(
                                RobotsCharacterPhysicsRuntimeState::monster_ctor_default,
                            )
                            .add_vertical_velocity_delta(delta_y);
                    }

                    if let Some(requested_anim_mode) = plan.requested_anim_mode {
                        let events = {
                            let runtime =
                                self.native_standard_monster_runtime.get_mut(&key).unwrap();
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                runtime.advance_animation(
                                    body,
                                    requested_anim_mode,
                                    None,
                                    ROBOTS_FIXED_STEP_SECONDS,
                                    NativeAiRootMotionPolicy::FULL,
                                )
                            } else {
                                Vec::new()
                            }
                        };
                        if events
                            .iter()
                            .any(|event| event.event_type == event_type::SETUP_IDLE)
                        {
                            self.native_standard_monster_runtime
                                .get_mut(&key)
                                .unwrap()
                                .animation
                                .setup_idle();
                        }
                    }
                    if effect_plan.is_some_and(|effect| effect.serviced) {
                        let physics = self.native_monster_physics.get(&key).copied();
                        if let (Some(physics), Some(body)) =
                            (physics, self.runtime_character_bodies.get_mut(&key))
                        {
                            if physics.object_flag_bit0 {
                                let mut position = body.owner_position.to_array();
                                physics
                                    .integrate_position(&mut position, ROBOTS_FIXED_STEP_SECONDS);
                                body.owner_position = Vec3::from_array(position);
                            }
                        }
                    }
                    if plan.request_natural_death {
                        let death_position = self
                            .runtime_character_bodies
                            .get(&key)
                            .map(|body| body.owner_position)
                            .unwrap_or(owner_position);
                        self.request_native_ai_natural_death(map.hashcode, key, death_position);
                    }
                }
                Some(RobotsStandardMonsterBehaviorWinner::DirectTurnThenAttack) => {
                    let Some(turn_config) = config.direct_turn_then_attack() else {
                        continue;
                    };
                    let turn_step = {
                        let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                        step_turn_then_attack(
                            &mut runtime.direct_turn_then_attack,
                            turn_config,
                            RobotsTurnThenAttackStepInput {
                                owner_position_xyz: owner_position.to_array(),
                                owner_yaw_radians,
                                target_position_xyz: player_position.to_array(),
                                handler_flags_628: visual.handler_flags_628,
                                runtime_rate_scale: 1.0,
                            },
                        )
                    };
                    if let Some(requested_anim_mode) = turn_step.requested_anim_mode {
                        let owner_yaw_write = turn_step
                            .direct_turn
                            .filter(|turn| turn.direct_owner_yaw_write)
                            .map(|turn| turn.owner_yaw_radians);
                        let policy = if turn_step.direct_turn.is_some_and(|turn| {
                            !turn.direct_owner_yaw_write
                                && matches!(
                                    turn.requested_anim_mode,
                                    ROBOTS_ANIM_MODE_TURN_ON_SPOT_L
                                        | ROBOTS_ANIM_MODE_TURN_ON_SPOT_R
                                )
                        }) {
                            NativeAiRootMotionPolicy::FULL
                        } else if turn_step.direct_turn.is_some() {
                            NativeAiRootMotionPolicy::NONE
                        } else {
                            NativeAiRootMotionPolicy::TRANSLATION_ONLY
                        };
                        let events = {
                            let runtime =
                                self.native_standard_monster_runtime.get_mut(&key).unwrap();
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                runtime.advance_animation(
                                    body,
                                    requested_anim_mode,
                                    owner_yaw_write,
                                    ROBOTS_FIXED_STEP_SECONDS,
                                    policy,
                                )
                            } else {
                                Vec::new()
                            }
                        };
                        for event in events {
                            match event.event_type {
                                event_type::SETUP_IDLE => {
                                    let runtime =
                                        self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                    turn_then_attack_setup_idle(
                                        &mut runtime.direct_turn_then_attack,
                                    );
                                    runtime.animation.setup_idle();
                                }
                                event_type::CREATE_PROJECTILE => {
                                    if let Some(request) =
                                        RobotsCreateProjectileRequest::from_event(event.as_view())
                                    {
                                        projectile_events.push((
                                            request,
                                            requested_anim_mode,
                                            event.pose_seconds,
                                            event.owner_position,
                                            event.owner_rotation,
                                        ));
                                    }
                                }
                                event_type::HIT_CHECK => {
                                    if let Some(plan) =
                                        RobotsHitQueryInitPlan::from_event(event.as_view())
                                    {
                                        pending_standard_hit_queries
                                            .push((plan, requested_anim_mode));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Some(RobotsStandardMonsterBehaviorWinner::AttackGroup) => {
                    let selected_index = self
                        .native_standard_monster_runtime
                        .get(&key)
                        .and_then(|runtime| runtime.attack_group.selected_index);
                    if let Some(index) = selected_index {
                        if let Some(turn_config) = config.turn_then_attacks[index] {
                            let turn_step = {
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                step_turn_then_attack(
                                    &mut runtime.turn_then_attacks[index],
                                    turn_config,
                                    RobotsTurnThenAttackStepInput {
                                        owner_position_xyz: owner_position.to_array(),
                                        owner_yaw_radians,
                                        target_position_xyz: player_position.to_array(),
                                        handler_flags_628: visual.handler_flags_628,
                                        runtime_rate_scale: 1.0,
                                    },
                                )
                            };
                            if let Some(requested_anim_mode) = turn_step.requested_anim_mode {
                                let owner_yaw_write = turn_step
                                    .direct_turn
                                    .filter(|turn| turn.direct_owner_yaw_write)
                                    .map(|turn| turn.owner_yaw_radians);
                                let policy = if turn_step.direct_turn.is_some_and(|turn| {
                                    !turn.direct_owner_yaw_write
                                        && matches!(
                                            turn.requested_anim_mode,
                                            ROBOTS_ANIM_MODE_TURN_ON_SPOT_L
                                                | ROBOTS_ANIM_MODE_TURN_ON_SPOT_R
                                        )
                                }) {
                                    NativeAiRootMotionPolicy::FULL
                                } else if turn_step.direct_turn.is_some() {
                                    NativeAiRootMotionPolicy::NONE
                                } else {
                                    NativeAiRootMotionPolicy::TRANSLATION_ONLY
                                };
                                let events = {
                                    let runtime =
                                        self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                    if let Some(body) = self.runtime_character_bodies.get_mut(&key)
                                    {
                                        runtime.advance_animation(
                                            body,
                                            requested_anim_mode,
                                            owner_yaw_write,
                                            ROBOTS_FIXED_STEP_SECONDS,
                                            policy,
                                        )
                                    } else {
                                        Vec::new()
                                    }
                                };
                                for event in events {
                                    match event.event_type {
                                        event_type::SETUP_IDLE => {
                                            let runtime = self
                                                .native_standard_monster_runtime
                                                .get_mut(&key)
                                                .unwrap();
                                            turn_then_attack_setup_idle(
                                                &mut runtime.turn_then_attacks[index],
                                            );
                                            runtime.animation.setup_idle();
                                        }
                                        event_type::CREATE_PROJECTILE => {
                                            if let Some(request) =
                                                RobotsCreateProjectileRequest::from_event(
                                                    event.as_view(),
                                                )
                                            {
                                                projectile_events.push((
                                                    request,
                                                    requested_anim_mode,
                                                    event.pose_seconds,
                                                    event.owner_position,
                                                    event.owner_rotation,
                                                ));
                                            }
                                        }
                                        event_type::HIT_CHECK => {
                                            if let Some(plan) =
                                                RobotsHitQueryInitPlan::from_event(event.as_view())
                                            {
                                                pending_standard_hit_queries
                                                    .push((plan, requested_anim_mode));
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        } else if config.attack_group_three_phase_index() == Some(index) {
                            let Some(three_phase_config) = config.attack_group_three_phase() else {
                                continue;
                            };
                            let attack_step = {
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                step_three_phase_attack(
                                    &mut runtime.attack_group_three_phase,
                                    three_phase_config,
                                    RobotsThreePhaseAttackInput {
                                        owner_position_xyz: owner_position.to_array(),
                                        player_position_xyz: player_position.to_array(),
                                        target_visible,
                                    },
                                )
                            };
                            if let Some(requested_anim_mode) = attack_step.requested_anim_mode {
                                let owner_yaw_write =
                                    attack_step.tracking_target_yaw_radians.map(|target| {
                                        owner_yaw_radians
                                            + shortest_yaw_delta(owner_yaw_radians, target).clamp(
                                                -ROBOTS_MINEBOT_ATTACK_TRACKING_MAX_YAW_PER_TICK,
                                                ROBOTS_MINEBOT_ATTACK_TRACKING_MAX_YAW_PER_TICK,
                                            )
                                    });
                                let events = {
                                    let runtime =
                                        self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                    if let Some(body) = self.runtime_character_bodies.get_mut(&key)
                                    {
                                        runtime.advance_animation(
                                            body,
                                            requested_anim_mode,
                                            owner_yaw_write,
                                            ROBOTS_FIXED_STEP_SECONDS,
                                            NativeAiRootMotionPolicy::TRANSLATION_ONLY,
                                        )
                                    } else {
                                        Vec::new()
                                    }
                                };
                                for event in events {
                                    match event.event_type {
                                        event_type::SETUP_IDLE => {
                                            let runtime = self
                                                .native_standard_monster_runtime
                                                .get_mut(&key)
                                                .unwrap();
                                            three_phase_attack_setup_idle(
                                                &mut runtime.attack_group_three_phase,
                                            );
                                            runtime.animation.setup_idle();
                                        }
                                        event_type::CREATE_PROJECTILE => {
                                            if let Some(request) =
                                                RobotsCreateProjectileRequest::from_event(
                                                    event.as_view(),
                                                )
                                            {
                                                projectile_events.push((
                                                    request,
                                                    requested_anim_mode,
                                                    event.pose_seconds,
                                                    event.owner_position,
                                                    event.owner_rotation,
                                                ));
                                            }
                                        }
                                        event_type::HIT_CHECK => {
                                            if let Some(plan) =
                                                RobotsHitQueryInitPlan::from_event(event.as_view())
                                            {
                                                pending_standard_hit_queries
                                                    .push((plan, requested_anim_mode));
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        } else {
                            let Some(attack) = config.attacks[index] else {
                                continue;
                            };
                            let attack_step = {
                                let runtime =
                                    self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                step_generic_attack(&mut runtime.attacks[index], attack)
                            };
                            if let Some(requested_anim_mode) = attack_step.requested_anim_mode {
                                let events = {
                                    let runtime =
                                        self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                    if let Some(body) = self.runtime_character_bodies.get_mut(&key)
                                    {
                                        runtime.advance_animation(
                                            body,
                                            requested_anim_mode,
                                            None,
                                            ROBOTS_FIXED_STEP_SECONDS,
                                            NativeAiRootMotionPolicy::TRANSLATION_ONLY,
                                        )
                                    } else {
                                        Vec::new()
                                    }
                                };
                                for event in events {
                                    match event.event_type {
                                        event_type::SETUP_IDLE => {
                                            let runtime = self
                                                .native_standard_monster_runtime
                                                .get_mut(&key)
                                                .unwrap();
                                            generic_attack_setup_idle(
                                                &mut runtime.attacks[index],
                                                attack,
                                            );
                                            runtime.animation.setup_idle();
                                        }
                                        event_type::CREATE_PROJECTILE => {
                                            if let Some(request) =
                                                RobotsCreateProjectileRequest::from_event(
                                                    event.as_view(),
                                                )
                                            {
                                                projectile_events.push((
                                                    request,
                                                    requested_anim_mode,
                                                    event.pose_seconds,
                                                    event.owner_position,
                                                    event.owner_rotation,
                                                ));
                                            }
                                        }
                                        event_type::HIT_CHECK => {
                                            if let Some(plan) =
                                                RobotsHitQueryInitPlan::from_event(event.as_view())
                                            {
                                                pending_standard_hit_queries
                                                    .push((plan, requested_anim_mode));
                                            }
                                        }
                                        _ => {}
                                    }
                                }

                                // EW09's derived AI_Attack execute `0x0044FAF0`
                                // writes owner +0x110(0.4) after the animation service.
                                // Its shipped Attack25 track has exactly zero root
                                // translation, so this Character Physics lane is the
                                // attack's sole translational motion source.
                                if config.handler_class == RobotsAiHandlerClass::Ew09Armoured
                                    && index == 0
                                {
                                    let runtimes = &mut self.native_standard_monster_runtime;
                                    let physics = &mut self.native_monster_physics;
                                    let bodies = &mut self.runtime_character_bodies;
                                    if let (Some(runtime), Some(body)) =
                                        (runtimes.get_mut(&key), bodies.get_mut(&key))
                                    {
                                        apply_native_ew09_derived_attack_motion(
                                            runtime,
                                            physics.entry(key).or_insert_with(
                                                RobotsCharacterPhysicsRuntimeState::monster_ctor_default,
                                            ),
                                            body,
                                            owner_yaw_radians,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                Some(RobotsStandardMonsterBehaviorWinner::ShuntAttack) => {
                    let Some(shunt_config) = config.shunt_attack else {
                        continue;
                    };
                    let target_delta = player_position - owner_position;
                    let target_yaw_radians = target_delta.x.atan2(target_delta.z);
                    let relative_target_yaw_radians =
                        shortest_yaw_delta(owner_yaw_radians, target_yaw_radians);
                    let shunt_step = {
                        let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                        step_shunt_attack_configured(
                            shunt_config,
                            &mut runtime.shunt_attack,
                            RobotsShuntAttackStepInput {
                                relative_target_yaw_radians,
                                hit_query_completed: standard_hit_query_completed,
                                target_present: true,
                                target_visible,
                                // Exact common-Monster NavMesh correction accumulated
                                // by `0x00452220`; never substitute CharacterPhysics
                                // velocity here. Shunt state3 branches on this vector.
                                collision_correction_xz: nav_constraint_correction_xz,
                                owner_yaw_radians,
                            },
                        )
                    };

                    if shunt_step.service_turn_toward_target {
                        let max_turn_step = ROBOTS_SHUNT_ATTACK_TURN_RATE_RADIANS_PER_SECOND
                            * ROBOTS_FIXED_STEP_SECONDS;
                        let owner_yaw_write = Some(
                            owner_yaw_radians
                                + shortest_yaw_delta(owner_yaw_radians, target_yaw_radians)
                                    .clamp(-max_turn_step, max_turn_step),
                        );
                        let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                        if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                            let _ = runtime.advance_animation(
                                body,
                                ROBOTS_SHUNT_ATTACK_TURN_ANIM_MODE,
                                owner_yaw_write,
                                ROBOTS_FIXED_STEP_SECONDS,
                                NativeAiRootMotionPolicy::NONE,
                            );
                        }
                    } else if let Some(requested_anim_mode) = shunt_step.requested_anim_mode {
                        let events = {
                            let runtime =
                                self.native_standard_monster_runtime.get_mut(&key).unwrap();
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                runtime.advance_animation(
                                    body,
                                    requested_anim_mode,
                                    None,
                                    ROBOTS_FIXED_STEP_SECONDS,
                                    NativeAiRootMotionPolicy::TRANSLATION_ONLY,
                                )
                            } else {
                                Vec::new()
                            }
                        };
                        for event in events {
                            match event.event_type {
                                event_type::SETUP_IDLE => {
                                    let runtime =
                                        self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                    let _ = apply_shunt_attack_event(
                                        &mut runtime.shunt_attack,
                                        event_type::SETUP_IDLE,
                                    );
                                    runtime.animation.setup_idle();
                                }
                                event_type::HIT_CHECK => {
                                    if let Some(plan) =
                                        RobotsHitQueryInitPlan::from_event(event.as_view())
                                    {
                                        pending_standard_hit_queries
                                            .push((plan, requested_anim_mode));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Some(RobotsStandardMonsterBehaviorWinner::SpikeAttack) => {
                    let Some(spike_config) = config.spike_attack else {
                        continue;
                    };
                    let target_delta = player_position - owner_position;
                    let target_yaw_radians = target_delta.x.atan2(target_delta.z);
                    let relative_target_yaw_radians =
                        shortest_yaw_delta(owner_yaw_radians, target_yaw_radians);
                    let nav_direct_reachable = nav_context.is_some_and(|(nav, current_face, _)| {
                        nav.trace_direct_segment(
                            owner_position.to_array(),
                            player_position.to_array(),
                            current_face,
                            allow_cross_group,
                        )
                        .is_some_and(|trace| trace.reached_target)
                    });
                    let owner_turn_blocked = self
                        .native_standard_monster_runtime
                        .get(&key)
                        .is_some_and(|runtime| runtime.handler_script_value_45c != 0);
                    let spike_step = {
                        let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                        step_spike_attack(
                            spike_config,
                            &mut runtime.spike_attack,
                            RobotsSpikeAttackStepInput {
                                target_present: true,
                                target_visible,
                                target_distance_squared: owner_position
                                    .distance_squared(player_position),
                                nav_direct_reachable,
                                player_reaction_active,
                                relative_target_yaw_radians,
                                owner_turn_blocked,
                            },
                        )
                    };

                    if let Some(requested_anim_mode) = spike_step.requested_anim_mode {
                        let owner_yaw_write = (spike_step.owner_yaw_delta_radians != 0.0)
                            .then_some(owner_yaw_radians + spike_step.owner_yaw_delta_radians);
                        let events = {
                            let runtime =
                                self.native_standard_monster_runtime.get_mut(&key).unwrap();
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                runtime.advance_animation(
                                    body,
                                    requested_anim_mode,
                                    owner_yaw_write,
                                    ROBOTS_FIXED_STEP_SECONDS,
                                    NativeAiRootMotionPolicy::TRANSLATION_ONLY,
                                )
                            } else {
                                Vec::new()
                            }
                        };
                        for event in events {
                            match event.event_type {
                                event_type::SETUP_IDLE => {
                                    let runtime =
                                        self.native_standard_monster_runtime.get_mut(&key).unwrap();
                                    let _ = apply_spike_attack_event(
                                        &mut runtime.spike_attack,
                                        event_type::SETUP_IDLE,
                                    );
                                    runtime.animation.setup_idle();
                                }
                                event_type::HIT_CHECK => {
                                    if let Some(plan) =
                                        RobotsHitQueryInitPlan::from_event(event.as_view())
                                    {
                                        pending_standard_hit_queries
                                            .push((plan, requested_anim_mode));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                None => {}
            }

            // EQ03 vslot +0x34 = `0x00465220` runs after common AI update and
            // rotates Handler+0x640 around local Y by exactly pi/60 per fixed tick.
            // Resource materialization remains a renderer/UE adapter concern.
            if config.handler_class == RobotsAiHandlerClass::Eq03Spider {
                let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                service_native_eq03_spider_post_common_update(runtime);
            }

            // EF01/EF03 share vslot +0x34 = 0x00467500: after common Monster
            // update rotate the ctor-created HT_Entity_Blades attachment around
            // local Y by exactly 8*pi/60. Both constructors prove Handler+0x640.
            if RobotsBladesAttachmentSpec::for_handler_class(config.handler_class).is_some() {
                let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                service_native_blades_post_common_update(runtime);
            }

            // EB14 +0x34 = 0x004647B0 and EW10 +0x34 = 0x00460340 rotate
            // the ctor-created Handler+0x640 attachment around local Z after the
            // common Monster update. Resource materialization remains an adapter seam.
            if let Some(minion_attachment) =
                RobotsMinionAttachmentConfig::for_handler_class(config.handler_class)
            {
                let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                service_native_minion_attachment_post_common_update(runtime, minion_attachment);
            }

            // ThiefBot vslot +0x34 = 0x0045F7A0 calls common Monster update first,
            // then 0x0045F7C0 services HT_Entity_Blades on HT_AnimBone_Object01.
            if config.handler_class == RobotsAiHandlerClass::ThiefBot {
                let gameplay_target = self.native_ai_gameplay_target_position(player_position);
                let owner_pose = self
                    .runtime_character_bodies
                    .get(&key)
                    .map(|body| (body.owner_position, horizontal_yaw(body.owner_rotation)));
                if let Some((owner_position, owner_yaw_radians)) = owner_pose {
                    let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                    service_native_thiefbot_post_common_update(
                        runtime,
                        owner_position,
                        owner_yaw_radians,
                        gameplay_target,
                    );
                }
            }

            // EW09 vslot +0x34 = `0x00463190` runs after common AI update and
            // rotates the attached Idle_Animator/Object01 XItem every fixed tick.
            // Keep the quaternion in the runtime layer; rendering/UE attachment
            // materialization is a separate adapter concern.
            if config.handler_class == RobotsAiHandlerClass::Ew09Armoured {
                let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                service_native_ew09_post_common_update(runtime);
            }

            for (plan, source_anim_mode) in pending_standard_hit_queries {
                let serial = self.allocate_native_hit_query_serial();
                if let Some(runtime) = self.native_standard_monster_runtime.get_mut(&key) {
                    runtime.register_hit_query(
                        NativeStandardMonsterHitQueryRuntime {
                            query: plan.instantiate(true, false, serial),
                            source_anim_mode,
                        },
                        config.hit_query_capacity(),
                    );
                }
            }

            {
                let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                if periodic_enabled {
                    tick_periodic_idle(&mut runtime.periodic_idle);
                }
                tick_patrol_navmesh2(&mut runtime.patrol_navmesh2, 1.0);
                tick_stalk_navmesh(&mut runtime.stalk_navmesh);
                for attack in &mut runtime.attacks {
                    tick_generic_attack(attack);
                }
                if config.attack_group_three_phase().is_some() {
                    tick_three_phase_attack(&mut runtime.attack_group_three_phase);
                }
                for (index, turn_config) in config.turn_then_attacks.iter().enumerate() {
                    if turn_config.is_some() {
                        tick_turn_then_attack(&mut runtime.turn_then_attacks[index]);
                    }
                }
                if config.direct_turn_then_attack().is_some() {
                    tick_turn_then_attack(&mut runtime.direct_turn_then_attack);
                }
                if config.handler_class == RobotsAiHandlerClass::Eb11MagnaBot {
                    runtime.magnabot_turn.finish_update();
                }
            }

            // Native common AI state0 services Handler+0x4D8 with the same
            // BehaviorHost immediately after the primary +0x4C0 host. EB13 installs
            // one reusable three-phase node here; keep it independent from the
            // primary selector and from the primary body animation channel.
            if let Some(secondary_config) = config.secondary_three_phase_attack {
                let secondary_position = self
                    .runtime_character_bodies
                    .get(&key)
                    .map(|body| body.owner_position)
                    .unwrap_or(owner_position);
                let secondary_completed = self
                    .native_standard_monster_runtime
                    .get(&key)
                    .is_some_and(|runtime| {
                        runtime.secondary_three_phase_attack.phase
                            == RobotsThreePhaseAttackPhase::Complete
                    });
                if secondary_completed {
                    let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                    leave_three_phase_attack(&mut runtime.secondary_three_phase_attack);
                    runtime.secondary_animation.setup_idle();
                    tick_three_phase_attack(&mut runtime.secondary_three_phase_attack);
                } else {
                    let secondary_inactive = self
                        .native_standard_monster_runtime
                        .get(&key)
                        .is_some_and(|runtime| {
                            runtime.secondary_three_phase_attack.phase
                                == RobotsThreePhaseAttackPhase::Inactive
                        });
                    if secondary_inactive
                        && three_phase_attack_geometry_gate(
                            secondary_config,
                            secondary_position.to_array(),
                            player_position.to_array(),
                            target_visible,
                        )
                    {
                        enter_three_phase_attack(
                            &mut self
                                .native_standard_monster_runtime
                                .get_mut(&key)
                                .unwrap()
                                .secondary_three_phase_attack,
                        );
                    }

                    let secondary_step = {
                        let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                        step_three_phase_attack(
                            &mut runtime.secondary_three_phase_attack,
                            secondary_config,
                            RobotsThreePhaseAttackInput {
                                owner_position_xyz: secondary_position.to_array(),
                                player_position_xyz: player_position.to_array(),
                                target_visible,
                            },
                        )
                    };
                    if let Some(requested_anim_mode) = secondary_step.requested_anim_mode {
                        let events = {
                            let runtime =
                                self.native_standard_monster_runtime.get_mut(&key).unwrap();
                            if let Some(body) = self.runtime_character_bodies.get(&key) {
                                runtime
                                    .secondary_animation
                                    .advance_auxiliary_script_channel(
                                        body,
                                        requested_anim_mode,
                                        ROBOTS_FIXED_STEP_SECONDS,
                                    )
                            } else {
                                Vec::new()
                            }
                        };
                        if events
                            .iter()
                            .any(|event| event.event_type == event_type::SETUP_IDLE)
                        {
                            let runtime =
                                self.native_standard_monster_runtime.get_mut(&key).unwrap();
                            three_phase_attack_setup_idle(
                                &mut runtime.secondary_three_phase_attack,
                            );
                            runtime.secondary_animation.setup_idle();
                        }
                    }
                    // EB13 owner +0x130 eventually calls +0x134; its vtable maps
                    // +0x134 to 0x0041CDD0, a native no-op. The AnimScript remains
                    // the owner of projectile/hit/event side effects for this class.
                    let runtime = self.native_standard_monster_runtime.get_mut(&key).unwrap();
                    tick_three_phase_attack(&mut runtime.secondary_three_phase_attack);
                }
            }

            for (request, anim_mode, pose_seconds, event_position, event_rotation) in
                projectile_events
            {
                let query_serial = self.allocate_native_hit_query_serial();
                if let Some(plan) = resolve_ai_projectile_spawn_plan(
                    map,
                    key,
                    event_position,
                    event_rotation,
                    owner_scale,
                    visual,
                    anim_mode,
                    request,
                    query_serial,
                    pose_seconds,
                    player_position,
                ) {
                    self.spawn_native_ai_projectile(plan);
                }
            }
        }
    }

    fn advance_native_base_monsters_fixed(
        &mut self,
        map: &ProcessedMap,
        nav_regions: &[RobotsMonsterNavMeshView<'_>],
    ) {
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            let Some(visual) = trigger.character_visual.as_ref() else {
                continue;
            };
            if !uses_base_monster_idle_only(visual.handler_class) {
                continue;
            }

            let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
            if !self.runtime_ai_character_xitem_live(map.hashcode, trigger_index) {
                self.native_base_monster_runtime.remove(&key);
                self.native_monster_navigation.remove(&key);
                self.native_monster_physics.remove(&key);
                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                    body.clear_collision_animation_track();
                }
                continue;
            }
            if !self.runtime_character_bodies.contains_key(&key) {
                self.native_base_monster_runtime.remove(&key);
                self.native_monster_navigation.remove(&key);
                self.native_monster_physics.remove(&key);
                continue;
            }

            let allow_cross_group = trigger
                .data
                .get(7)
                .and_then(|value| *value)
                .is_some_and(|flags| flags & 0x8000 != 0);
            let Some(_) = self.apply_native_common_monster_nav_constraint(
                key,
                nav_regions,
                allow_cross_group,
                visual.handler_flags_628,
            ) else {
                continue;
            };

            let runtime = self.native_base_monster_runtime.entry(key).or_default();
            if !runtime.idle.active {
                runtime.idle.enter();
            }
            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                let _ = runtime.animation.advance(
                    body,
                    ROBOTS_BASE_MONSTER_IDLE_ANIM_MODE,
                    None,
                    ROBOTS_FIXED_STEP_SECONDS,
                    NativeAiRootMotionPolicy::NONE,
                );
            }
        }
    }

    fn advance_native_dodgem_fixed(
        &mut self,
        map: &ProcessedMap,
        nav_regions: &[RobotsMonsterNavMeshView<'_>],
        player_position: Vec3,
        wall_time: f64,
    ) {
        let attack_config = dodgem_attack_config();

        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            let Some(visual) = trigger.character_visual.as_ref() else {
                continue;
            };
            if visual.handler_class != RobotsAiHandlerClass::Ew07Dodgem {
                continue;
            }

            let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
            if !self.runtime_ai_character_xitem_live(map.hashcode, trigger_index) {
                self.native_dodgem_runtime.remove(&key);
                self.native_monster_navigation.remove(&key);
                self.native_monster_physics.remove(&key);
                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                    body.clear_collision_animation_track();
                }
                continue;
            }
            if !self.runtime_character_bodies.contains_key(&key) {
                self.native_dodgem_runtime.remove(&key);
                self.native_monster_navigation.remove(&key);
                self.native_monster_physics.remove(&key);
                continue;
            }

            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                advance_native_retained_horizontal_physics(
                    self.native_monster_physics.get(&key),
                    body,
                );
            }

            let allow_cross_group = trigger
                .data
                .get(7)
                .and_then(|value| *value)
                .is_some_and(|flags| flags & 0x8000 != 0);
            let Some(_) = self.apply_native_common_monster_nav_constraint(
                key,
                nav_regions,
                allow_cross_group,
                visual.handler_flags_628,
            ) else {
                continue;
            };

            if !self.native_dodgem_runtime.contains_key(&key) {
                let Some(setup_draw) = self.next_native_ai_gameplay_rng_u32() else {
                    continue;
                };
                self.native_dodgem_runtime
                    .insert(key, NativeDodgemRuntime::new(setup_draw));
            }

            if self.native_ai_normal_fatal_condition(key) {
                continue;
            }

            let Some(body) = self.runtime_character_bodies.get(&key) else {
                continue;
            };
            let owner_position = body.owner_position;
            let owner_rotation = body.owner_rotation;
            let owner_scale = body.native_transform_scale_xyz();
            let owner_yaw_radians = horizontal_yaw(owner_rotation);

            // Handler +0x34 services an already-enqueued Attack HitCheck before this
            // tick's two BehaviorHosts can enqueue another one.
            let active_query = self.native_dodgem_runtime.get(&key).and_then(|runtime| {
                runtime.hit_query.map(|query| {
                    (
                        query,
                        runtime.secondary_animation.sampled_pose_seconds(),
                        runtime
                            .secondary_animation
                            .is_mode(attack_config.primary_anim_mode),
                    )
                })
            });
            if let Some((mut query, pose_seconds, attack_pose_owned)) = active_query {
                let source_shape = attack_pose_owned
                    .then(|| {
                        runtime_character_animation_datum_world_shape(
                            owner_position,
                            owner_rotation,
                            owner_scale,
                            visual,
                            attack_config.primary_anim_mode,
                            query.selector,
                            pose_seconds,
                        )
                    })
                    .flatten();
                let hit = source_shape.is_some_and(|shape| {
                    self.native_hit_shape_hits_player(
                        map,
                        shape,
                        RobotsHitQueryCandidateContext {
                            flags: query.flags,
                            query_serial: query.serial,
                            source_raw_group: Some(ROBOTS_HIT_QUERY_RAW_GROUP1),
                            secondary_source_raw_group: None,
                        },
                    )
                });
                let _ = query.step(source_shape.is_some(), hit, 0, 1.0);
                if let Some(runtime) = self.native_dodgem_runtime.get_mut(&key) {
                    runtime.hit_query = query.is_active().then_some(query);
                }
            }

            let hit_snapshot = self.native_ai_hit_reactions.get(&key).copied();
            let navigation_ready = self
                .native_monster_navigation
                .get(&key)
                .is_some_and(|state| state.region_ordinal.is_some() && state.face_index.is_some());
            let (primary_winner, primary_previous) = {
                let runtime = self.native_dodgem_runtime.get(&key).unwrap();
                let scrambled_priority = hit_snapshot
                    .map(|hit| {
                        runtime
                            .primary_scrambled
                            .priority(hit.got_hit_latch, hit.query_flags_snapshot)
                    })
                    .unwrap_or(1);
                let magnetic_priority = hit_snapshot
                    .map(|hit| {
                        runtime
                            .magnetic_hit
                            .priority(hit.got_hit_latch, hit.query_flags_snapshot)
                    })
                    .unwrap_or(1);
                (
                    dodgem_primary_winner(
                        RobotsDodgemBounceNavMeshRuntimeState::priority(navigation_ready),
                        scrambled_priority,
                        magnetic_priority,
                    ),
                    runtime.primary_active,
                )
            };

            if primary_previous != primary_winner {
                if let Some(previous) = primary_previous {
                    let runtime = self.native_dodgem_runtime.get_mut(&key).unwrap();
                    match previous {
                        RobotsDodgemPrimaryWinner::BounceNavMesh => runtime.bounce.leave(),
                        RobotsDodgemPrimaryWinner::ScrambledHit => {
                            runtime.primary_scrambled.leave()
                        }
                        RobotsDodgemPrimaryWinner::MagneticHit => runtime.magnetic_hit.leave(),
                    }
                }
                if let Some(next) = primary_winner {
                    match next {
                        RobotsDodgemPrimaryWinner::BounceNavMesh => {
                            self.native_dodgem_runtime
                                .get_mut(&key)
                                .unwrap()
                                .bounce
                                .enter();
                        }
                        RobotsDodgemPrimaryWinner::ScrambledHit => {
                            self.native_dodgem_runtime
                                .get_mut(&key)
                                .unwrap()
                                .primary_scrambled
                                .enter();
                        }
                        RobotsDodgemPrimaryWinner::MagneticHit => {
                            let owner_y = self
                                .runtime_character_bodies
                                .get(&key)
                                .map(|body| body.owner_position.y)
                                .unwrap_or(owner_position.y);
                            self.native_dodgem_runtime
                                .get_mut(&key)
                                .unwrap()
                                .magnetic_hit
                                .enter(owner_y);
                        }
                    }
                }
                self.native_dodgem_runtime
                    .get_mut(&key)
                    .unwrap()
                    .primary_active = primary_winner;
            }

            match primary_winner {
                Some(RobotsDodgemPrimaryWinner::BounceNavMesh) => {
                    // Native Execute 0x0046D110 decrements +0x2C before probing
                    // NavMesh. A bounce created later in this same update must
                    // therefore retain the full 15-tick cooldown.
                    self.native_dodgem_runtime
                        .get_mut(&key)
                        .unwrap()
                        .bounce
                        .tick();

                    let radial_result =
                        self.native_monster_navigation
                            .get(&key)
                            .and_then(|nav_state| {
                                let region_ordinal = nav_state.region_ordinal?;
                                let current_face = nav_state.face_index?;
                                let nav = nav_regions.get(region_ordinal).copied()?;
                                let owner = self.runtime_character_bodies.get(&key)?.owner_position;
                                nav.constrain_owner_radially(
                                    owner.to_array(),
                                    current_face,
                                    ROBOTS_DODGEM_BOUNCE_PROBE_RADIUS,
                                    allow_cross_group,
                                )
                                .map(|result| (region_ordinal, nav, result))
                            });
                    if let Some((region_ordinal, nav, result)) = radial_result {
                        if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                            body.owner_position = Vec3::from_array(result.owner_xyz);
                        }
                        if let Some(nav_state) = self.native_monster_navigation.get_mut(&key) {
                            nav_state.region_ordinal = Some(region_ordinal);
                            nav_state.face_index = Some(result.face_index);
                            nav_state.group_flags0 = nav.group_flags0_for_face(result.face_index);
                        }
                        let correction_x = result.correction_xyz[0];
                        let correction_z = result.correction_xyz[2];
                        if correction_x * correction_x + correction_z * correction_z > f32::EPSILON
                        {
                            let correction_yaw = correction_x.atan2(correction_z);
                            let current_yaw = self
                                .runtime_character_bodies
                                .get(&key)
                                .map(|body| horizontal_yaw(body.owner_rotation))
                                .unwrap_or(owner_yaw_radians);
                            let _ = self
                                .native_dodgem_runtime
                                .get_mut(&key)
                                .unwrap()
                                .bounce
                                .apply_boundary_bounce(current_yaw, correction_yaw);
                        }
                    }

                    // 0x0046D260 runs after the radial correction and derives the
                    // steering heading from corrected owner position minus source XItem.
                    let source_heading = self
                        .native_ai_last_hit_source_position
                        .get(&key)
                        .copied()
                        .and_then(|source_position| {
                            let current_position =
                                self.runtime_character_bodies.get(&key)?.owner_position;
                            let delta = current_position - source_position;
                            let length_squared = delta.x * delta.x + delta.z * delta.z;
                            (length_squared > f32::EPSILON).then_some(delta.x.atan2(delta.z))
                        });
                    if let Some(hit) = self.native_ai_hit_reactions.get(&key).copied() {
                        let request_impact = self
                            .native_dodgem_runtime
                            .get_mut(&key)
                            .unwrap()
                            .bounce
                            .observe_query_serial(
                                hit.query_serial_snapshot,
                                hit.health,
                                source_heading,
                            );
                        if request_impact {
                            self.native_dodgem_runtime
                                .get_mut(&key)
                                .unwrap()
                                .bounce
                                .request_impact_animation();
                        }
                    }

                    let health = self
                        .native_ai_hit_reactions
                        .get(&key)
                        .map(|hit| hit.health)
                        .unwrap_or(3);
                    let (steering_target_yaw_radians, impact_phase) = {
                        let runtime = self.native_dodgem_runtime.get(&key).unwrap();
                        (
                            runtime.bounce.steering_target_yaw_radians,
                            runtime.bounce.phase
                                == eurochef_shared::robots_runtime::dodgem::RobotsDodgemBouncePhase::ImpactAnimation,
                        )
                    };
                    let current_yaw = self
                        .runtime_character_bodies
                        .get(&key)
                        .map(|body| horizontal_yaw(body.owner_rotation))
                        .unwrap_or(owner_yaw_radians);
                    let locomotion = {
                        let runtime = self.native_dodgem_runtime.get_mut(&key).unwrap();
                        let move_mode_active_on_entry =
                            runtime.primary_animation.is_mode(ROBOTS_ANIM_MODE_MOVE);
                        step_ai_locomotion_after_steering_prepass(
                            &mut runtime.locomotion,
                            RobotsAiLocomotionInput {
                                steering_target_yaw_radians,
                                target_locomotion_scalar:
                                    RobotsDodgemBounceNavMeshRuntimeState::target_locomotion_scalar(
                                        health,
                                    ),
                                turn_rate: RobotsDodgemBounceNavMeshRuntimeState::turn_rate(),
                                handler_flags_628: visual.handler_flags_628,
                                move_mode_active_on_entry,
                                current_owner_yaw_radians: current_yaw,
                                runtime_rate_scale: 1.0,
                            },
                        )
                    };
                    let owner_yaw_write = locomotion
                        .direct_owner_yaw_write
                        .then_some(locomotion.owner_yaw_radians);
                    write_native_ai_physics_locomotion(
                        self.native_monster_physics.entry(key).or_insert_with(
                            RobotsCharacterPhysicsRuntimeState::monster_ctor_default,
                        ),
                        locomotion.owner_yaw_radians,
                        locomotion.locomotion_scalar,
                        ROBOTS_DODGEM_MIN_MOVE_SPEED,
                        ROBOTS_DODGEM_MAX_MOVE_SPEED,
                    );
                    let requested_mode = if impact_phase {
                        ROBOTS_DODGEM_BOUNCE_ANIM_MODE
                    } else {
                        locomotion.requested_anim_mode
                    };
                    let events = {
                        let runtime = self.native_dodgem_runtime.get_mut(&key).unwrap();
                        if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                            runtime.primary_animation.advance(
                                body,
                                requested_mode,
                                owner_yaw_write,
                                ROBOTS_FIXED_STEP_SECONDS,
                                NativeAiRootMotionPolicy::NONE,
                            )
                        } else {
                            Vec::new()
                        }
                    };
                    if impact_phase
                        && events
                            .iter()
                            .any(|event| event.event_type == event_type::SETUP_IDLE)
                    {
                        let runtimes = &mut self.native_dodgem_runtime;
                        let hits = &mut self.native_ai_hit_reactions;
                        if let (Some(runtime), Some(hit)) =
                            (runtimes.get_mut(&key), hits.get_mut(&key))
                        {
                            runtime.bounce.setup_idle(&mut hit.got_hit_latch);
                            runtime.primary_animation.setup_idle();
                        }
                    }
                }
                Some(RobotsDodgemPrimaryWinner::ScrambledHit) => {
                    let requested_mode = {
                        let runtimes = &mut self.native_dodgem_runtime;
                        let hits = &mut self.native_ai_hit_reactions;
                        let Some(runtime) = runtimes.get_mut(&key) else {
                            continue;
                        };
                        let Some(hit) = hits.get_mut(&key) else {
                            continue;
                        };
                        runtime.primary_scrambled.step(&mut hit.got_hit_latch)
                    };
                    if requested_mode != 0 {
                        let events = {
                            let runtime = self.native_dodgem_runtime.get_mut(&key).unwrap();
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                runtime.primary_animation.advance(
                                    body,
                                    requested_mode,
                                    None,
                                    ROBOTS_FIXED_STEP_SECONDS,
                                    NativeAiRootMotionPolicy::NONE,
                                )
                            } else {
                                Vec::new()
                            }
                        };
                        if events
                            .iter()
                            .any(|event| event.event_type == event_type::SETUP_IDLE)
                        {
                            let runtimes = &mut self.native_dodgem_runtime;
                            let hits = &mut self.native_ai_hit_reactions;
                            if let (Some(runtime), Some(hit)) =
                                (runtimes.get_mut(&key), hits.get_mut(&key))
                            {
                                runtime.primary_scrambled.setup_idle(&mut hit.got_hit_latch);
                                runtime.primary_animation.setup_idle();
                            }
                        }
                    }
                }
                Some(RobotsDodgemPrimaryWinner::MagneticHit) => {
                    let attachment_relation_matches =
                        self.native_player_magnetic_target_key == Some(key);
                    let owner_y = self
                        .runtime_character_bodies
                        .get(&key)
                        .map(|body| body.owner_position.y)
                        .unwrap_or(owner_position.y);
                    let plan = {
                        let runtimes = &mut self.native_dodgem_runtime;
                        let hits = &mut self.native_ai_hit_reactions;
                        let Some(runtime) = runtimes.get_mut(&key) else {
                            continue;
                        };
                        let Some(hit) = hits.get_mut(&key) else {
                            continue;
                        };
                        runtime.magnetic_hit.step(
                            &mut hit.got_hit_latch,
                            owner_y,
                            attachment_relation_matches,
                            ROBOTS_FIXED_STEP_SECONDS,
                        )
                    };
                    if let Some(value) = plan.set_handler_606 {
                        self.native_monster_physics
                            .entry(key)
                            .or_insert_with(
                                RobotsCharacterPhysicsRuntimeState::monster_ctor_default,
                            )
                            .apply_handler_606_mode(value);
                    }

                    let mut magnetic_vertical_velocity_delta = None;
                    let effect_plan = if plan.service_magnetic_effect {
                        let runtime = self.native_dodgem_runtime.get_mut(&key).unwrap();
                        let drop_charge_count = runtime
                            .magnetic_drop_charge_count
                            .unwrap_or_else(|| visual.pickup_drop_count.unwrap_or(0));
                        let effect_plan = runtime.magnetic_hit.step_attached_effect(
                            RobotsMalfBotMagneticEffectInput {
                                owner_y,
                                magnetic_mass: visual.magnetic_mass,
                                drop_charge_count,
                            },
                        );
                        if effect_plan.serviced {
                            runtime.magnetic_drop_charge_count =
                                Some(effect_plan.next_drop_charge_count);
                            magnetic_vertical_velocity_delta =
                                Some(effect_plan.physics_vertical_velocity_delta);
                        }
                        Some(effect_plan)
                    } else {
                        None
                    };
                    if let Some(delta_y) = magnetic_vertical_velocity_delta {
                        self.native_monster_physics
                            .entry(key)
                            .or_insert_with(
                                RobotsCharacterPhysicsRuntimeState::monster_ctor_default,
                            )
                            .add_vertical_velocity_delta(delta_y);
                    }

                    if let Some(requested_anim_mode) = plan.requested_anim_mode {
                        let events = {
                            let runtime = self.native_dodgem_runtime.get_mut(&key).unwrap();
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                runtime.primary_animation.advance(
                                    body,
                                    requested_anim_mode,
                                    None,
                                    ROBOTS_FIXED_STEP_SECONDS,
                                    NativeAiRootMotionPolicy::NONE,
                                )
                            } else {
                                Vec::new()
                            }
                        };
                        if events
                            .iter()
                            .any(|event| event.event_type == event_type::SETUP_IDLE)
                        {
                            self.native_dodgem_runtime
                                .get_mut(&key)
                                .unwrap()
                                .primary_animation
                                .setup_idle();
                        }
                    }
                    if effect_plan.is_some_and(|effect| effect.serviced) {
                        let physics = self.native_monster_physics.get(&key).copied();
                        if let (Some(physics), Some(body)) =
                            (physics, self.runtime_character_bodies.get_mut(&key))
                        {
                            if physics.object_flag_bit0 {
                                let mut position = body.owner_position.to_array();
                                physics
                                    .integrate_position(&mut position, ROBOTS_FIXED_STEP_SECONDS);
                                body.owner_position = Vec3::from_array(position);
                            }
                        }
                    }
                    if plan.request_natural_death {
                        let death_position = self
                            .runtime_character_bodies
                            .get(&key)
                            .map(|body| body.owner_position)
                            .unwrap_or(owner_position);
                        self.request_native_ai_natural_death(map.hashcode, key, death_position);
                    }
                }
                None => {}
            }

            // Native 0x004517A0 services Handler+0x4D8 after +0x4C0. Re-read hit,
            // position and global attack state because the first host may have
            // cleared the shared got-hit latch or changed CharacterPhysics.
            let current_position = self
                .runtime_character_bodies
                .get(&key)
                .map(|body| body.owner_position)
                .unwrap_or(owner_position);
            let current_yaw = self
                .runtime_character_bodies
                .get(&key)
                .map(|body| horizontal_yaw(body.owner_rotation))
                .unwrap_or(owner_yaw_radians);
            let gameplay_target = self.native_ai_gameplay_target_position(player_position);
            let target_visible = gameplay_target.is_some_and(|target| {
                self.runtime_map_script_line_of_sight_state(
                    map,
                    current_position,
                    target,
                    wall_time,
                )
                .is_some_and(|state| state.0)
            });
            let player_state = self.native_player_focus_runtime.player_state;
            let top_game_state = self
                .native_cutscene_host_runtime
                .game_state_stack
                .last()
                .copied();
            let class_attack_allowed = robots_common_monster_attack_allowed(
                visual.handler_flags_628,
                top_game_state,
                player_state,
                true,
                self.native_player_hit_runtime.reaction_window,
                self.native_monster_attack_cooldown,
            );
            let attack_priority = gameplay_target
                .map(|target| {
                    generic_attack_priority(
                        self.native_dodgem_runtime.get(&key).unwrap().attack,
                        attack_config,
                        RobotsGenericAttackGateInput {
                            owner_position_xyz: current_position.to_array(),
                            owner_yaw_radians: current_yaw,
                            target_position_xyz: target.to_array(),
                            target_visible,
                            class_attack_allowed,
                        },
                    )
                })
                .unwrap_or(1);
            let hit_snapshot = self.native_ai_hit_reactions.get(&key).copied();
            let secondary_scrambled_priority = hit_snapshot
                .map(|hit| {
                    self.native_dodgem_runtime
                        .get(&key)
                        .unwrap()
                        .secondary_scrambled
                        .priority(hit.got_hit_latch, hit.query_flags_snapshot)
                })
                .unwrap_or(1);
            let (secondary_winner, secondary_previous, attack_completed_reentry) = {
                let runtime = self.native_dodgem_runtime.get(&key).unwrap();
                let winner =
                    dodgem_secondary_winner(2, attack_priority, secondary_scrambled_priority);
                (
                    winner,
                    runtime.secondary_active,
                    runtime.secondary_active == Some(RobotsDodgemSecondaryWinner::Attack)
                        && winner == Some(RobotsDodgemSecondaryWinner::Attack)
                        && runtime.attack.completion_latch,
                )
            };
            let mut entered_attack = false;
            if secondary_previous != secondary_winner || attack_completed_reentry {
                if let Some(previous) = secondary_previous {
                    let runtime = self.native_dodgem_runtime.get_mut(&key).unwrap();
                    match previous {
                        RobotsDodgemSecondaryWinner::Attack => {
                            leave_generic_attack(&mut runtime.attack)
                        }
                        RobotsDodgemSecondaryWinner::ScrambledHit => {
                            runtime.secondary_scrambled.leave()
                        }
                        RobotsDodgemSecondaryWinner::Idle => {}
                    }
                }
                if let Some(next) = secondary_winner {
                    let runtime = self.native_dodgem_runtime.get_mut(&key).unwrap();
                    match next {
                        RobotsDodgemSecondaryWinner::Attack => {
                            enter_generic_attack(&mut runtime.attack, attack_config);
                            entered_attack = true;
                        }
                        RobotsDodgemSecondaryWinner::ScrambledHit => {
                            runtime.secondary_scrambled.enter()
                        }
                        RobotsDodgemSecondaryWinner::Idle => {}
                    }
                }
                self.native_dodgem_runtime
                    .get_mut(&key)
                    .unwrap()
                    .secondary_active = secondary_winner;
            }

            let mut pending_hit_query = None::<RobotsHitQueryInitPlan>;
            match secondary_winner {
                Some(RobotsDodgemSecondaryWinner::Idle) => {
                    if let Some(body) = self.runtime_character_bodies.get(&key) {
                        let _ = self
                            .native_dodgem_runtime
                            .get_mut(&key)
                            .unwrap()
                            .secondary_animation
                            .advance_auxiliary_script_channel(
                                body,
                                ROBOTS_DODGEM_IDLE_ANIM_MODE,
                                ROBOTS_FIXED_STEP_SECONDS,
                            );
                    }
                }
                Some(RobotsDodgemSecondaryWinner::Attack) => {
                    let attack_step = {
                        let runtime = self.native_dodgem_runtime.get_mut(&key).unwrap();
                        step_generic_attack(&mut runtime.attack, attack_config)
                    };
                    if let Some(mode) = attack_step.requested_anim_mode {
                        let events = if let Some(body) = self.runtime_character_bodies.get(&key) {
                            self.native_dodgem_runtime
                                .get_mut(&key)
                                .unwrap()
                                .secondary_animation
                                .advance_auxiliary_script_channel(
                                    body,
                                    mode,
                                    ROBOTS_FIXED_STEP_SECONDS,
                                )
                        } else {
                            Vec::new()
                        };
                        for event in events {
                            match event.event_type {
                                event_type::SETUP_IDLE => {
                                    let runtime = self.native_dodgem_runtime.get_mut(&key).unwrap();
                                    generic_attack_setup_idle(&mut runtime.attack, attack_config);
                                    runtime.secondary_animation.setup_idle();
                                }
                                event_type::HIT_CHECK => {
                                    pending_hit_query =
                                        RobotsHitQueryInitPlan::from_event(event.as_view());
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Some(RobotsDodgemSecondaryWinner::ScrambledHit) => {
                    let requested_mode = {
                        let runtimes = &mut self.native_dodgem_runtime;
                        let hits = &mut self.native_ai_hit_reactions;
                        let Some(runtime) = runtimes.get_mut(&key) else {
                            continue;
                        };
                        let Some(hit) = hits.get_mut(&key) else {
                            continue;
                        };
                        runtime.secondary_scrambled.step(&mut hit.got_hit_latch)
                    };
                    if requested_mode != 0 {
                        let events = if let Some(body) = self.runtime_character_bodies.get(&key) {
                            self.native_dodgem_runtime
                                .get_mut(&key)
                                .unwrap()
                                .secondary_animation
                                .advance_auxiliary_script_channel(
                                    body,
                                    requested_mode,
                                    ROBOTS_FIXED_STEP_SECONDS,
                                )
                        } else {
                            Vec::new()
                        };
                        if events
                            .iter()
                            .any(|event| event.event_type == event_type::SETUP_IDLE)
                        {
                            let runtimes = &mut self.native_dodgem_runtime;
                            let hits = &mut self.native_ai_hit_reactions;
                            if let (Some(runtime), Some(hit)) =
                                (runtimes.get_mut(&key), hits.get_mut(&key))
                            {
                                runtime
                                    .secondary_scrambled
                                    .setup_idle(&mut hit.got_hit_latch);
                                runtime.secondary_animation.setup_idle();
                            }
                        }
                    }
                }
                None => {}
            }

            tick_generic_attack(&mut self.native_dodgem_runtime.get_mut(&key).unwrap().attack);
            if entered_attack {
                self.native_monster_attack_cooldown.record_attack();
            }
            if let Some(plan) = pending_hit_query {
                let serial = self.allocate_native_hit_query_serial();
                if let Some(runtime) = self.native_dodgem_runtime.get_mut(&key) {
                    runtime.hit_query = Some(plan.instantiate(true, false, serial));
                }
            }
        }
    }

    pub(super) fn advance_native_rollerbot_fixed(
        &mut self,
        map: &ProcessedMap,
        nav_regions: &[RobotsMonsterNavMeshView<'_>],
        player_position: Vec3,
        wall_time: f64,
        scope: NativeAiHostScope,
    ) {
        let path = map
            .paths
            .iter()
            .find(|path| path.hashcode == ROBOTS_ROLLERBOT_PATH_UID);
        let node_positions = path
            .map(|path| {
                path.nodes
                    .iter()
                    .map(|node| node.position.to_array())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let path_graph = path.map(|path| RobotsRollerBotPathGraphView {
            node_positions: &node_positions,
            links: &path.links,
        });
        let attack_config = rollerbot_attack_config();

        let mut instances = Vec::<(u64, &ProcessedCharacterVisual, Option<u8>, bool)>::new();
        if scope == NativeAiHostScope::Serialized {
            for (trigger_index, trigger) in map.triggers.iter().enumerate() {
                if !is_rollerbot(trigger) {
                    continue;
                }
                let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
                if !self.runtime_ai_character_xitem_live(map.hashcode, trigger_index) {
                    self.native_rollerbot_runtime.remove(&key);
                    self.native_monster_navigation.remove(&key);
                    self.native_monster_physics.remove(&key);
                    if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                        body.clear_collision_animation_track();
                    }
                    continue;
                }
                let Some(visual) = trigger.character_visual.as_ref() else {
                    continue;
                };
                let allow_cross_group = trigger
                    .data
                    .get(7)
                    .and_then(|value| *value)
                    .is_some_and(|flags| flags & 0x8000 != 0);
                instances.push((key, visual, None, allow_cross_group));
            }
        } else {
            let dynamic_snapshots = self.runtime_dynamic_ai_snapshots(map);
            let mut live_dynamic_roller_keys = FxHashSet::default();
            for snapshot in dynamic_snapshots {
                if snapshot.pending_destroy {
                    continue;
                }
                let Some(NativeSweeperBossGenericAiBootstrap::RollerBot { setup_random_mod3 }) =
                    snapshot.bootstrap
                else {
                    continue;
                };
                let Some(visual) = map
                    .runtime_character_visuals
                    .get(&(ROBOTS_SWEEPER_MONSTER_RUNTIME_TYPE, snapshot.selector))
                else {
                    continue;
                };
                if visual.handler_class != RobotsAiHandlerClass::Eb10RollerBot {
                    continue;
                }
                live_dynamic_roller_keys.insert(snapshot.key);
                // Same direct factory path as the other Sweeper-spawned Monsters:
                // XItem+0x154 creator is null, so Handler+0x605 remains zero.
                instances.push((
                    snapshot.key,
                    visual,
                    Some(setup_random_mod3),
                    ROBOTS_DIRECT_MONSTER_FACTORY_ALLOW_CROSS_GROUP,
                ));
            }
            self.native_rollerbot_runtime.retain(|key, _| {
                !super::runtime_bodies::RuntimeAiInstanceKey::raw_is_dynamic_for_map(
                    *key,
                    map.hashcode,
                ) || live_dynamic_roller_keys.contains(key)
            });
            self.native_monster_navigation.retain(|key, _| {
                !super::runtime_bodies::RuntimeAiInstanceKey::raw_is_dynamic_for_map(
                    *key,
                    map.hashcode,
                ) || live_dynamic_roller_keys.contains(key)
            });
        }

        for (key, visual, dynamic_setup_random_mod3, allow_cross_group) in instances {
            let Some(body) = self.runtime_character_bodies.get(&key) else {
                self.native_rollerbot_runtime.remove(&key);
                self.native_monster_navigation.remove(&key);
                self.native_monster_physics.remove(&key);
                continue;
            };
            let owner_rotation = body.owner_rotation;
            let Some(_) = self.apply_native_common_monster_nav_constraint(
                key,
                nav_regions,
                allow_cross_group,
                visual.handler_flags_628,
            ) else {
                continue;
            };
            let Some(body) = self.runtime_character_bodies.get(&key) else {
                continue;
            };
            let owner_position = body.owner_position;
            let owner_scale = body.native_transform_scale_xyz();
            let owner_yaw_radians = horizontal_yaw(owner_rotation);

            if !self.native_rollerbot_runtime.contains_key(&key) {
                // Serialized Roller +0x108 consumes setup `%3` plus mandatory `%1`.
                // Dynamic Sweeper/Transporter Rollers arrive with those exact draws
                // already consumed by factory ordering, so bootstrap reuses mod3 and
                // never advances the process-global stream a second time.
                let setup_draw = if let Some(setup_random_mod3) = dynamic_setup_random_mod3 {
                    u32::from(setup_random_mod3)
                } else {
                    let Some(setup_draw) = self.next_native_ai_gameplay_rng_u32() else {
                        continue;
                    };
                    let Some(_mandatory_config_draw) = self.next_native_ai_gameplay_rng_u32()
                    else {
                        continue;
                    };
                    setup_draw
                };
                self.native_rollerbot_runtime.insert(
                    key,
                    NativeRollerBotRuntime::new(setup_draw, owner_position, owner_yaw_radians),
                );
            }

            // Character Physics owner position/yaw is authoritative. Retained Roller
            // velocity stays class state, but external pose changes are observed here.
            if let Some(runtime) = self.native_rollerbot_runtime.get_mut(&key) {
                runtime.locomotion.position_xyz = owner_position.to_array();
                runtime.locomotion.yaw_radians = owner_yaw_radians;
            }

            // Handler +0x34 services an already-enqueued melee HitCheck before this
            // tick's selector/AnimScript can enqueue another one.
            let active_query = self.native_rollerbot_runtime.get(&key).and_then(|runtime| {
                runtime.hit_query.map(|query| {
                    (
                        query,
                        runtime.animation.sampled_pose_seconds(),
                        runtime.animation.is_mode(attack_config.primary_anim_mode),
                    )
                })
            });
            if let Some((mut query, pose_seconds, attack_pose_owned)) = active_query {
                let source_shape = attack_pose_owned
                    .then(|| {
                        runtime_character_animation_datum_world_shape(
                            owner_position,
                            owner_rotation,
                            owner_scale,
                            visual,
                            attack_config.primary_anim_mode,
                            query.selector,
                            pose_seconds,
                        )
                    })
                    .flatten();
                let hit = source_shape.is_some_and(|shape| {
                    self.native_hit_shape_hits_player(
                        map,
                        shape,
                        RobotsHitQueryCandidateContext {
                            flags: query.flags,
                            query_serial: query.serial,
                            source_raw_group: Some(ROBOTS_HIT_QUERY_RAW_GROUP1),
                            secondary_source_raw_group: None,
                        },
                    )
                });
                let _ = query.step(source_shape.is_some(), hit, 0, 1.0);
                if let Some(runtime) = self.native_rollerbot_runtime.get_mut(&key) {
                    runtime.hit_query = query.is_active().then_some(query);
                }
            }

            if self.native_ai_normal_fatal_condition(key) {
                // AI_HitFatal priority200 is serviced by the common host before this
                // class pass. Keep the class selector from issuing a competing node.
                continue;
            }

            let player_state = self.native_player_focus_runtime.player_state;
            let Some(player_position) = self.native_ai_gameplay_target_position(player_position)
            else {
                continue;
            };
            let target_visible = self
                .runtime_map_script_line_of_sight_state(
                    map,
                    owner_position,
                    player_position,
                    wall_time,
                )
                .is_some_and(|state| state.0);
            let top_game_state = self
                .native_cutscene_host_runtime
                .game_state_stack
                .last()
                .copied();
            let class_attack_allowed = robots_common_monster_attack_allowed(
                visual.handler_flags_628,
                top_game_state,
                player_state,
                true,
                self.native_player_hit_runtime.reaction_window,
                self.native_monster_attack_cooldown,
            );
            let attack_input = RobotsGenericAttackGateInput {
                owner_position_xyz: owner_position.to_array(),
                owner_yaw_radians,
                target_position_xyz: player_position.to_array(),
                target_visible,
                class_attack_allowed,
            };
            let hit_snapshot = self.native_ai_hit_reactions.get(&key).copied();

            let (winner, previous_node) = {
                let runtime = self.native_rollerbot_runtime.get(&key).unwrap();
                let attack_priority =
                    generic_attack_priority(runtime.attack, attack_config, attack_input);
                let hit_priority = hit_snapshot
                    .map(|hit| runtime.hit.priority(hit))
                    .unwrap_or(1);
                let pursue_priority = rollerbot_pursue_priority(
                    owner_position.to_array(),
                    Some(player_position.to_array()),
                );
                let patrol_priority = ai_patrol_priority(top_game_state);
                (
                    rollerbot_behavior_winner(
                        patrol_priority,
                        pursue_priority,
                        attack_priority,
                        hit_priority,
                        1,
                        rollerbot_follow_path_host_priority(
                            path_graph.is_some(),
                            runtime.follow_path,
                        ),
                    ),
                    runtime.active_node,
                )
            };

            if previous_node != winner {
                if let Some(previous) = previous_node {
                    match previous {
                        RobotsRollerBotBehaviorWinner::Attack => {
                            leave_generic_attack(
                                &mut self.native_rollerbot_runtime.get_mut(&key).unwrap().attack,
                            );
                        }
                        RobotsRollerBotBehaviorWinner::Hit => {
                            let runtimes = &mut self.native_rollerbot_runtime;
                            let hits = &mut self.native_ai_hit_reactions;
                            if let (Some(runtime), Some(hit)) =
                                (runtimes.get_mut(&key), hits.get_mut(&key))
                            {
                                runtime.hit.leave(hit);
                            }
                        }
                        RobotsRollerBotBehaviorWinner::FollowNetworkPath => {
                            self.native_rollerbot_runtime
                                .get_mut(&key)
                                .unwrap()
                                .follow_path
                                .leave();
                        }
                        RobotsRollerBotBehaviorWinner::Patrol
                        | RobotsRollerBotBehaviorWinner::Pursue
                        | RobotsRollerBotBehaviorWinner::HitFatal => {}
                    }
                }

                let mut entered = true;
                let mut record_attack = false;
                if let Some(next) = winner {
                    match next {
                        RobotsRollerBotBehaviorWinner::Attack => {
                            enter_generic_attack(
                                &mut self.native_rollerbot_runtime.get_mut(&key).unwrap().attack,
                                attack_config,
                            );
                            record_attack = true;
                        }
                        RobotsRollerBotBehaviorWinner::Hit => {
                            if let Some(source_yaw) =
                                self.native_ai_last_hit_source_yaw.get(&key).copied()
                            {
                                let serial = self
                                    .native_ai_hit_reactions
                                    .get(&key)
                                    .map(|hit| hit.last_query_serial)
                                    .unwrap_or(u16::MAX);
                                self.native_rollerbot_runtime
                                    .get_mut(&key)
                                    .unwrap()
                                    .hit
                                    .enter(owner_yaw_radians, source_yaw, serial);
                            } else {
                                entered = false;
                            }
                        }
                        RobotsRollerBotBehaviorWinner::FollowNetworkPath => {
                            if let (Some(path_graph), Some(draw)) =
                                (path_graph, self.next_native_ai_gameplay_rng_u32())
                            {
                                entered = self
                                    .native_rollerbot_runtime
                                    .get_mut(&key)
                                    .unwrap()
                                    .follow_path
                                    .enter(path_graph, owner_position.to_array(), draw);
                            } else {
                                entered = false;
                            }
                        }
                        RobotsRollerBotBehaviorWinner::Patrol
                        | RobotsRollerBotBehaviorWinner::Pursue
                        | RobotsRollerBotBehaviorWinner::HitFatal => {}
                    }
                }
                self.native_rollerbot_runtime
                    .get_mut(&key)
                    .unwrap()
                    .active_node = entered.then_some(winner).flatten();
                if record_attack {
                    self.native_monster_attack_cooldown.record_attack();
                }
            }

            let active_node = self
                .native_rollerbot_runtime
                .get(&key)
                .and_then(|runtime| runtime.active_node);
            let mut pending_hit_query = None::<RobotsHitQueryInitPlan>;
            let mut issued_acceleration = false;

            match active_node {
                Some(RobotsRollerBotBehaviorWinner::FollowNetworkPath) => {
                    if let Some(path_graph) = path_graph {
                        let needs_rng =
                            self.native_rollerbot_runtime
                                .get(&key)
                                .is_some_and(|runtime| {
                                    runtime
                                        .follow_path
                                        .needs_neighbor_rng(path_graph, owner_position.to_array())
                                });
                        let neighbor_draw = if needs_rng {
                            self.next_native_ai_gameplay_rng_u32()
                        } else {
                            None
                        };
                        if !needs_rng || neighbor_draw.is_some() {
                            let step = self
                                .native_rollerbot_runtime
                                .get_mut(&key)
                                .unwrap()
                                .follow_path
                                .step(path_graph, owner_position.to_array(), neighbor_draw);
                            if let Some(step) = step {
                                step_rollerbot_locomotion(
                                    &mut self
                                        .native_rollerbot_runtime
                                        .get_mut(&key)
                                        .unwrap()
                                        .locomotion,
                                    step.target_yaw_radians,
                                );
                                issued_acceleration = true;
                            }
                        }
                    }
                }
                Some(RobotsRollerBotBehaviorWinner::Pursue) => {
                    let target_yaw =
                        rollerbot_target_yaw(owner_position.to_array(), player_position.to_array());
                    step_rollerbot_locomotion(
                        &mut self
                            .native_rollerbot_runtime
                            .get_mut(&key)
                            .unwrap()
                            .locomotion,
                        target_yaw,
                    );
                    issued_acceleration = true;
                }
                Some(RobotsRollerBotBehaviorWinner::Patrol) => {
                    let patrol = {
                        let runtime = self.native_rollerbot_runtime.get_mut(&key).unwrap();
                        step_ai_patrol(&mut runtime.patrol, rollerbot_patrol_config())
                    };
                    step_rollerbot_locomotion(
                        &mut self
                            .native_rollerbot_runtime
                            .get_mut(&key)
                            .unwrap()
                            .locomotion,
                        patrol.steering_target_yaw_radians,
                    );
                    issued_acceleration = true;
                }
                Some(RobotsRollerBotBehaviorWinner::Attack) => {
                    let attack_step = {
                        let runtime = self.native_rollerbot_runtime.get_mut(&key).unwrap();
                        step_generic_attack(&mut runtime.attack, attack_config)
                    };
                    if let Some(requested_anim_mode) = attack_step.requested_anim_mode {
                        let events = {
                            let runtime = self.native_rollerbot_runtime.get_mut(&key).unwrap();
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                runtime.animation.advance(
                                    body,
                                    requested_anim_mode,
                                    None,
                                    ROBOTS_FIXED_STEP_SECONDS,
                                    NativeAiRootMotionPolicy::NONE,
                                )
                            } else {
                                Vec::new()
                            }
                        };
                        for event in events {
                            match event.event_type {
                                event_type::SETUP_IDLE => {
                                    let runtime =
                                        self.native_rollerbot_runtime.get_mut(&key).unwrap();
                                    generic_attack_setup_idle(&mut runtime.attack, attack_config);
                                    runtime.animation.setup_idle();
                                }
                                event_type::HIT_CHECK => {
                                    pending_hit_query =
                                        RobotsHitQueryInitPlan::from_event(event.as_view());
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Some(RobotsRollerBotBehaviorWinner::Hit) => {
                    let reenter = self
                        .native_ai_hit_reactions
                        .get(&key)
                        .and_then(|hit| {
                            self.native_rollerbot_runtime.get(&key).map(|runtime| {
                                hit.health != 0
                                    && hit.query_serial_snapshot != u16::MAX
                                    && hit.last_query_serial != runtime.hit.captured_query_serial
                            })
                        })
                        .unwrap_or(false);
                    if reenter {
                        if let (Some(source_yaw), Some(hit)) = (
                            self.native_ai_last_hit_source_yaw.get(&key).copied(),
                            self.native_ai_hit_reactions.get(&key).copied(),
                        ) {
                            let runtime = self.native_rollerbot_runtime.get_mut(&key).unwrap();
                            runtime.animation.setup_idle();
                            runtime
                                .hit
                                .enter(owner_yaw_radians, source_yaw, hit.last_query_serial);
                        }
                    }
                    let (requested_anim_mode, owner_yaw_write) = {
                        let runtime = self.native_rollerbot_runtime.get(&key).unwrap();
                        (
                            runtime.hit.requested_anim_mode,
                            runtime.hit.step_owner_yaw(runtime.locomotion.yaw_radians),
                        )
                    };
                    if let Some(yaw) = owner_yaw_write {
                        self.native_rollerbot_runtime
                            .get_mut(&key)
                            .unwrap()
                            .locomotion
                            .yaw_radians = yaw;
                    }
                    let events = {
                        let runtime = self.native_rollerbot_runtime.get_mut(&key).unwrap();
                        if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                            runtime.animation.advance(
                                body,
                                requested_anim_mode,
                                owner_yaw_write,
                                ROBOTS_FIXED_STEP_SECONDS,
                                NativeAiRootMotionPolicy::NONE,
                            )
                        } else {
                            Vec::new()
                        }
                    };
                    if events
                        .iter()
                        .any(|event| event.event_type == event_type::SETUP_IDLE)
                    {
                        let runtimes = &mut self.native_rollerbot_runtime;
                        let hits = &mut self.native_ai_hit_reactions;
                        if let (Some(runtime), Some(hit)) =
                            (runtimes.get_mut(&key), hits.get_mut(&key))
                        {
                            runtime.hit.setup_idle(hit);
                            runtime.animation.setup_idle();
                        }
                    }
                }
                Some(RobotsRollerBotBehaviorWinner::HitFatal) | None => {}
            }

            if !issued_acceleration {
                step_rollerbot_inertia(
                    &mut self
                        .native_rollerbot_runtime
                        .get_mut(&key)
                        .unwrap()
                        .locomotion,
                );
            }

            if let (Some(runtime), Some(body)) = (
                self.native_rollerbot_runtime.get(&key),
                self.runtime_character_bodies.get_mut(&key),
            ) {
                body.owner_position = Vec3::from_array(runtime.locomotion.position_xyz);
                body.owner_rotation = Quat::from_rotation_y(runtime.locomotion.yaw_radians);
            }

            self.native_rollerbot_runtime
                .get_mut(&key)
                .map(|runtime| tick_generic_attack(&mut runtime.attack));

            if let Some(plan) = pending_hit_query {
                let serial = self.allocate_native_hit_query_serial();
                if let Some(runtime) = self.native_rollerbot_runtime.get_mut(&key) {
                    runtime.hit_query = Some(plan.instantiate(true, false, serial));
                }
            }
        }
    }

    fn advance_native_eq04_mine_fixed(&mut self, map: &ProcessedMap, player_position: Vec3) {
        let top_game_state = self
            .native_cutscene_host_runtime
            .game_state_stack
            .last()
            .copied();
        let patrol_config = eq04_mine_patrol_config();
        let pursue_config = eq04_mine_pursue_config();
        let fall_config = RobotsAiFallConfig::eq04_mine();

        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            if !is_eq04_mine(trigger) {
                continue;
            }

            let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
            if !self.runtime_ai_character_xitem_live(map.hashcode, trigger_index) {
                self.native_eq04_mine_runtime.remove(&key);
                self.native_monster_physics.remove(&key);
                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                    body.clear_collision_animation_track();
                }
                continue;
            }
            let Some(visual) = trigger.character_visual.as_ref() else {
                continue;
            };
            let Some(body) = self.runtime_character_bodies.get(&key) else {
                self.native_eq04_mine_runtime.remove(&key);
                self.native_monster_physics.remove(&key);
                continue;
            };

            // Native scheduler `0x00444C20` executes Physics slot0 before the
            // later world-contact slot4 pass and only then reaches Handler AI.
            // Preserve that phase order: current Handler+0x2C0 state affects the
            // next fixed tick rather than being pulled forward into this one.
            let pre_physics_position = body.owner_position;
            let pre_physics_shape = body.current_world_shape();
            let Some(floor_contact) =
                runtime_map_ai_environment_floor_contact(map, pre_physics_position)
            else {
                // Missing decoded static geometry is a host failure. Do not let a
                // preview body fall through the world merely because the adapter
                // cannot reproduce native contact candidates.
                continue;
            };

            let post_physics_position = {
                let physics = self
                    .native_monster_physics
                    .entry(key)
                    .or_insert_with(RobotsCharacterPhysicsRuntimeState::monster_ctor_default);
                advance_eq04_character_physics_step(
                    physics,
                    pre_physics_position,
                    pre_physics_shape,
                    floor_contact,
                )
            };
            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                body.owner_position = post_physics_position;
            }

            let Some(body) = self.runtime_character_bodies.get(&key) else {
                continue;
            };
            let owner_position = body.owner_position;
            let owner_yaw_radians = horizontal_yaw(body.owner_rotation);

            // Native common AI update `0x00405320` initializes Handler+0x160
            // around the post-Physics owner pose and asks the Map environment
            // service to fill the surface lanes. The resulting +0x2C0 bit0 is
            // consumed by Monster state0 `0x00452180` for the following tick.
            let Some(reference_y) = runtime_map_ai_environment_floor_y(map, owner_position) else {
                continue;
            };
            if let Some(physics) = self.native_monster_physics.get_mut(&key) {
                physics.service_monster_owner_category_0b_bit0(true, reference_y.is_some());
            }
            let Some(player_position) = self.native_ai_gameplay_target_position(player_position)
            else {
                continue;
            };

            let runtime = self.native_eq04_mine_runtime.entry(key).or_default();
            let patrol_priority = ai_patrol_priority(top_game_state);
            let fall_priority = ai_fall_priority(
                runtime.fall,
                fall_config,
                RobotsAiFallGateInput {
                    owner_y: owner_position.y,
                    reference_y,
                },
            );
            let pursue_priority = ai_pursue_priority(
                pursue_config,
                owner_position.to_array(),
                Some(player_position.to_array()),
            );
            let winner = eq04_mine_behavior_winner(patrol_priority, fall_priority, pursue_priority);

            if runtime.active_node != winner {
                if runtime.active_node == Some(RobotsEq04MineBehaviorWinner::Fall) {
                    leave_ai_fall(&mut runtime.fall);
                }
                runtime.active_node = winner;
            }

            match winner {
                Some(RobotsEq04MineBehaviorWinner::Patrol) => {
                    let patrol = step_ai_patrol(&mut runtime.patrol, patrol_config);
                    let move_mode_active_on_entry =
                        runtime.animation.is_mode(ROBOTS_ANIM_MODE_MOVE);
                    let locomotion = step_ai_locomotion_after_steering_prepass(
                        &mut runtime.locomotion,
                        RobotsAiLocomotionInput {
                            steering_target_yaw_radians: patrol.steering_target_yaw_radians,
                            target_locomotion_scalar: patrol.target_locomotion_scalar,
                            turn_rate: patrol.turn_rate,
                            handler_flags_628: visual.handler_flags_628,
                            move_mode_active_on_entry,
                            current_owner_yaw_radians: owner_yaw_radians,
                            runtime_rate_scale: 1.0,
                        },
                    );
                    let owner_yaw_write = locomotion
                        .direct_owner_yaw_write
                        .then_some(locomotion.owner_yaw_radians);
                    write_native_ai_physics_locomotion(
                        self.native_monster_physics.entry(key).or_insert_with(
                            RobotsCharacterPhysicsRuntimeState::monster_ctor_default,
                        ),
                        locomotion.owner_yaw_radians,
                        locomotion.locomotion_scalar,
                        ROBOTS_EQ04_MINE_MOVE_SPEED,
                        ROBOTS_EQ04_MINE_MOVE_SPEED,
                    );
                    if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                        let _ = runtime.animation.advance(
                            body,
                            locomotion.requested_anim_mode,
                            owner_yaw_write,
                            ROBOTS_FIXED_STEP_SECONDS,
                            NativeAiRootMotionPolicy::NONE,
                        );
                    }
                }
                Some(RobotsEq04MineBehaviorWinner::Fall) => {
                    // EQ04 passes param5=0/param6=null to AI_Fall::Setup, so
                    // execute `0x0046B100` has no platform-relative transform
                    // branch for this class. It only requests AnimMode 03.
                    if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                        let _ = runtime.animation.advance(
                            body,
                            fall_config.anim_mode,
                            None,
                            ROBOTS_FIXED_STEP_SECONDS,
                            NativeAiRootMotionPolicy::NONE,
                        );
                    }
                }
                Some(RobotsEq04MineBehaviorWinner::Pursue) => {
                    let pursue = step_ai_pursue(
                        pursue_config,
                        owner_position.to_array(),
                        player_position.to_array(),
                    );
                    let move_mode_active_on_entry =
                        runtime.animation.is_mode(ROBOTS_ANIM_MODE_MOVE);
                    let locomotion = step_ai_locomotion_after_steering_prepass(
                        &mut runtime.locomotion,
                        RobotsAiLocomotionInput {
                            steering_target_yaw_radians: pursue.target_yaw_radians,
                            target_locomotion_scalar: pursue.locomotion_scalar,
                            turn_rate: RobotsAiTurnRateInput::Default,
                            handler_flags_628: visual.handler_flags_628,
                            move_mode_active_on_entry,
                            current_owner_yaw_radians: owner_yaw_radians,
                            runtime_rate_scale: 1.0,
                        },
                    );
                    let owner_yaw_write = locomotion
                        .direct_owner_yaw_write
                        .then_some(locomotion.owner_yaw_radians);
                    write_native_ai_physics_locomotion(
                        self.native_monster_physics.entry(key).or_insert_with(
                            RobotsCharacterPhysicsRuntimeState::monster_ctor_default,
                        ),
                        locomotion.owner_yaw_radians,
                        locomotion.locomotion_scalar,
                        ROBOTS_EQ04_MINE_MOVE_SPEED,
                        ROBOTS_EQ04_MINE_MOVE_SPEED,
                    );
                    if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                        let _ = runtime.animation.advance(
                            body,
                            locomotion.requested_anim_mode,
                            owner_yaw_write,
                            ROBOTS_FIXED_STEP_SECONDS,
                            NativeAiRootMotionPolicy::NONE,
                        );
                    }
                }
                None => {
                    if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                        runtime.animation.reset(body);
                    }
                }
            }

            // EQ04 vslot +0x34 (`0x00463590`) runs this after common AI update.
            // Native creates explosion 0x52000006 and then calls the same Monster
            // +0x12C cleanup `0x00453640` used by natural death, so preserve the
            // creator latch as well as the end-of-tick deferred destroy request.
            let current_position = self
                .runtime_character_bodies
                .get(&key)
                .map(|body| body.owner_position)
                .unwrap_or(owner_position);
            if eq04_mine_self_destruct_ready(
                current_position.to_array(),
                Some(player_position.to_array()),
            ) {
                self.queue_native_ai_explosion(
                    key,
                    ROBOTS_EQ04_MINE_EXPLOSION_UID,
                    current_position,
                );
                self.request_native_ai_natural_death(map.hashcode, key, current_position);
            }
        }
    }

    fn advance_native_minebot_fixed(
        &mut self,
        map: &ProcessedMap,
        nav_regions: &[RobotsMonsterNavMeshView<'_>],
        player_position: Vec3,
        wall_time: f64,
    ) {
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            if !is_eq02_minebot(trigger) {
                continue;
            }

            let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
            if !self.runtime_ai_character_xitem_live(map.hashcode, trigger_index) {
                self.native_minebot_runtime.remove(&key);
                self.native_monster_navigation.remove(&key);
                self.native_monster_physics.remove(&key);
                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                    body.clear_collision_animation_track();
                }
                continue;
            }

            let Some(visual) = trigger.character_visual.as_ref() else {
                continue;
            };
            let Some(body) = self.runtime_character_bodies.get(&key) else {
                self.native_minebot_runtime.remove(&key);
                self.native_monster_navigation.remove(&key);
                self.native_monster_physics.remove(&key);
                continue;
            };
            let owner_rotation = body.owner_rotation;
            let allow_cross_group = trigger
                .data
                .get(7)
                .and_then(|value| *value)
                .is_some_and(|flags| flags & 0x8000 != 0);
            let Some(_) = self.apply_native_common_monster_nav_constraint(
                key,
                nav_regions,
                allow_cross_group,
                visual.handler_flags_628,
            ) else {
                continue;
            };
            let Some(body) = self.runtime_character_bodies.get(&key) else {
                continue;
            };
            let owner_position = body.owner_position;
            let owner_yaw_radians = horizontal_yaw(owner_rotation);
            if self.native_ai_normal_fatal_condition(key) {
                // Common AI_HitFatal was serviced earlier in this priority-0x32
                // slice. Fatal selection preempts MineBot Move/Attack.
                continue;
            }
            let Some(player_position) = self.native_ai_gameplay_target_position(player_position)
            else {
                continue;
            };
            let target_visible = self
                .runtime_map_script_line_of_sight_state(
                    map,
                    owner_position,
                    player_position,
                    wall_time,
                )
                .is_some_and(|state| state.0);
            let mut projectile_events =
                Vec::<(RobotsCreateProjectileRequest, u32, f32, Vec3, Quat)>::new();

            let runtime = self.native_minebot_runtime.entry(key).or_default();
            if runtime.attack_state.phase == RobotsMineBotAttackPhase::Complete {
                runtime.attack_state = RobotsMineBotAttackRuntimeState::default();
            }
            let attack_in_progress = !matches!(
                runtime.attack_state.phase,
                RobotsMineBotAttackPhase::Inactive | RobotsMineBotAttackPhase::Complete
            );
            let move_mode_active_on_entry = runtime.animation.is_mode(ROBOTS_ANIM_MODE_MOVE);
            let move_step = step_minebot_move(
                &mut runtime.move_state,
                RobotsMineBotMoveInput {
                    owner_position_xyz: owner_position.to_array(),
                    owner_yaw_radians,
                    player_position_xyz: player_position.to_array(),
                    handler_flags_628: visual.handler_flags_628,
                    move_mode_active_on_entry,
                    runtime_rate_scale: 1.0,
                },
            );
            let attack_eligible = minebot_attack_gate(
                owner_position.to_array(),
                player_position.to_array(),
                target_visible,
            );
            let winner = if attack_in_progress {
                RobotsMineBotMoveAttackWinner::Attack
            } else {
                minebot_move_attack_winner(move_step.move_active, attack_eligible)
            };

            match winner {
                RobotsMineBotMoveAttackWinner::Attack => {
                    if runtime.attack_state.phase == RobotsMineBotAttackPhase::Inactive {
                        enter_minebot_attack(&mut runtime.attack_state);
                    }
                    let attack_step = step_minebot_attack(
                        &mut runtime.attack_state,
                        RobotsMineBotAttackInput {
                            owner_position_xyz: owner_position.to_array(),
                            player_position_xyz: player_position.to_array(),
                            target_visible,
                        },
                    );
                    let Some(requested_anim_mode) = attack_step.requested_anim_mode else {
                        continue;
                    };
                    let owner_yaw_write = attack_step.tracking_target_yaw_radians.map(|target| {
                        owner_yaw_radians
                            + shortest_yaw_delta(owner_yaw_radians, target).clamp(
                                -ROBOTS_MINEBOT_ATTACK_TRACKING_MAX_YAW_PER_TICK,
                                ROBOTS_MINEBOT_ATTACK_TRACKING_MAX_YAW_PER_TICK,
                            )
                    });
                    let events = if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                        runtime.animation.advance(
                            body,
                            requested_anim_mode,
                            owner_yaw_write,
                            ROBOTS_FIXED_STEP_SECONDS,
                            NativeAiRootMotionPolicy::TRANSLATION_ONLY,
                        )
                    } else {
                        Vec::new()
                    };
                    for event in events {
                        match event.event_type {
                            event_type::SETUP_IDLE => {
                                minebot_attack_setup_idle(&mut runtime.attack_state);
                                runtime.animation.setup_idle();
                            }
                            event_type::CREATE_PROJECTILE => {
                                if let Some(request) =
                                    RobotsCreateProjectileRequest::from_event(event.as_view())
                                {
                                    projectile_events.push((
                                        request,
                                        requested_anim_mode,
                                        event.pose_seconds,
                                        event.owner_position,
                                        event.owner_rotation,
                                    ));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                RobotsMineBotMoveAttackWinner::Move => {
                    let Some(locomotion) = move_step.locomotion else {
                        continue;
                    };
                    let requested_anim_mode = locomotion.requested_anim_mode;
                    let root_motion_policy = locomotion_root_motion_policy(locomotion);
                    let owner_yaw_write = locomotion
                        .direct_owner_yaw_write
                        .then_some(locomotion.owner_yaw_radians);
                    if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                        let _ = runtime.animation.advance(
                            body,
                            requested_anim_mode,
                            owner_yaw_write,
                            ROBOTS_FIXED_STEP_SECONDS,
                            root_motion_policy,
                        );
                    }
                }
                RobotsMineBotMoveAttackWinner::Neither => {
                    if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                        runtime.animation.reset(body);
                    }
                }
            }

            for (request, anim_mode, pose_seconds, owner_position, owner_rotation) in
                projectile_events
            {
                // Native 0x0044F1CC consumes DAT_00616DA8 before entering the
                // common producer, so failed resource/datum resolution still
                // advances the process-global query serial.
                let query_serial = self.allocate_native_hit_query_serial();
                if let Some(plan) = resolve_ai_projectile_spawn_plan(
                    map,
                    key,
                    owner_position,
                    owner_rotation,
                    Vec3::ONE,
                    visual,
                    anim_mode,
                    request,
                    query_serial,
                    pose_seconds,
                    player_position,
                ) {
                    self.spawn_native_ai_projectile(plan);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_runtime::RuntimeScriptTriggerLifecycleState;
    use eurochef_shared::robots_runtime::{
        hit_reaction::RobotsAcceptedHitReactionInput, hit_shapes::RobotsHitShape,
    };

    #[test]
    fn current_shipped_ai_handler_classes_have_exactly_one_production_host_family() {
        use RobotsAiHandlerClass as C;

        const CLASSES: [C; 45] = [
            C::MonsterBase,
            C::Monster2Rockets,
            C::ConstructionBot,
            C::DogBot,
            C::Eb07MineBot,
            C::Eb10RollerBot,
            C::Eb11MagnaBot,
            C::Eb12EvilBot,
            C::Eb13KnightBot,
            C::Eb14Minion,
            C::Eb15Launcher,
            C::Eb16KnuckleBot,
            C::Ef01Mine,
            C::Ef03EvilBot,
            C::Em07PiranhaBot,
            C::Ep02Turret,
            C::Ep04Turret,
            C::Ep05Turret,
            C::Ep06Turret,
            C::Eq02MineBot,
            C::Eq03Spider,
            C::Eq04Mine,
            C::Ew07Dodgem,
            C::Ew08Flambe,
            C::Ew08FlambeLarge,
            C::Ew09Armoured,
            C::Ew10Minion,
            C::Ew11FatBot,
            C::GuardBot,
            C::JailBotLarge,
            C::JailBotNormal,
            C::MalfBot,
            C::SawBot,
            C::SecurityBot,
            C::ShieldBot,
            C::ShuntBot,
            C::ShuntBotBoss,
            C::SpikeBot,
            C::SpinTop,
            C::Sweeper,
            C::TestAnimBot,
            C::ThiefBot,
            C::TurretBot,
            C::Npc,
            C::NpcFender,
        ];

        let specialized = |class: C| {
            matches!(
                class,
                C::DogBot
                    | C::Eb10RollerBot
                    | C::Ep02Turret
                    | C::Ep04Turret
                    | C::Ep05Turret
                    | C::Ep06Turret
                    | C::Eq02MineBot
                    | C::Eq04Mine
                    | C::Ew07Dodgem
                    | C::Em07PiranhaBot
                    | C::SpinTop
                    | C::TestAnimBot
                    | C::TurretBot
            )
        };
        let external = |class: C| matches!(class, C::Npc | C::NpcFender);

        let mut standard_count = 0;
        let mut base_count = 0;
        let mut specialized_count = 0;
        let mut external_count = 0;

        for class in CLASSES {
            let standard = RobotsStandardMonsterBehaviorConfig::for_handler_class(class).is_some();
            let base = uses_base_monster_idle_only(class);
            let special = specialized(class);
            let outside_runtime_ai = external(class);

            let owner_count = u8::from(standard)
                + u8::from(base)
                + u8::from(special)
                + u8::from(outside_runtime_ai);
            let expected_kind = if standard {
                NativeAiProductionHostKind::StandardMonster
            } else if base {
                NativeAiProductionHostKind::BaseMonster
            } else if special {
                NativeAiProductionHostKind::DedicatedAi
            } else {
                NativeAiProductionHostKind::NpcBehavior
            };
            assert_eq!(native_ai_production_host_kind(class), expected_kind);
            assert_eq!(
                owner_count, 1,
                "{class:?} must belong to exactly one production host family"
            );

            standard_count += usize::from(standard);
            base_count += usize::from(base);
            specialized_count += usize::from(special);
            external_count += usize::from(outside_runtime_ai);
        }

        assert_eq!(standard_count, 29);
        assert_eq!(base_count, 1);
        assert_eq!(specialized_count, 13);
        assert_eq!(external_count, 2);
    }

    #[test]
    fn maps_ai_preview_rng_is_deterministic_without_mutating_unknown_native_session_rng() {
        let mut native_global = RuntimeRobotsGlobalRngState::default();
        let mut preview = RuntimeRobotsGlobalRngState::fresh_process_startup();
        let preview_seed_before = preview.seed();

        assert!(next_ai_gameplay_rng_u32(false, &mut native_global, &mut preview).is_some());
        assert_eq!(native_global.seed(), None);
        assert_eq!(native_global.draws_from_anchor, 0);
        assert_ne!(preview.seed(), preview_seed_before);
        assert_eq!(preview.draws_from_anchor, 1);

        let preview_seed_after_first = preview.seed();
        assert!(next_ai_gameplay_rng_u32(true, &mut native_global, &mut preview).is_some());
        assert_eq!(native_global.seed(), None);
        assert_eq!(native_global.draws_from_anchor, 0);
        assert_ne!(preview.seed(), preview_seed_after_first);
        assert_eq!(preview.draws_from_anchor, 2);

        native_global = RuntimeRobotsGlobalRngState::from_observed_seed(0x1234_5678);
        let preview_seed_before_anchored_draw = preview.seed();
        assert!(next_ai_gameplay_rng_u32(true, &mut native_global, &mut preview).is_some());
        assert_eq!(native_global.draws_from_anchor, 1);
        assert_eq!(preview.seed(), preview_seed_before_anchored_draw);
        assert_eq!(preview.draws_from_anchor, 2);
    }

    #[test]
    fn maps_ai_process_lcg_fallback_keeps_unknown_native_seed_untouched() {
        let mut native_seed = None;
        let mut preview_seed =
            Some(eurochef_shared::robots_runtime::process_rng::ROBOTS_PROCESS_LCG_STARTUP_SEED);
        let preview_before = preview_seed;

        assert!(
            ai_process_lcg_modulo_zero_gate(true, &mut native_seed, &mut preview_seed, 3,)
                .is_some()
        );
        assert_eq!(native_seed, None);
        assert_ne!(preview_seed, preview_before);

        native_seed = Some(0x1234_5678);
        let preview_before_anchored = preview_seed;
        assert!(
            ai_process_lcg_modulo_zero_gate(true, &mut native_seed, &mut preview_seed, 3,)
                .is_some()
        );
        assert_ne!(native_seed, Some(0x1234_5678));
        assert_eq!(preview_seed, preview_before_anchored);
    }

    #[test]
    fn special_ai_solid_contact_requires_real_overlap_and_live_owner() {
        let player = RobotsHitShape::Sphere {
            center_xyz: [0.0, 0.35, 0.0],
            radius: 0.325,
        };
        let overlapping_mine = RobotsHitShape::Capsule {
            start_xyz: [0.0, 0.0, 0.0],
            delta_xyz: [0.0, 1.0, 0.0],
            radius: 0.836_756_05,
        };
        let distant_mine = RobotsHitShape::Capsule {
            start_xyz: [10.0, 0.0, 0.0],
            delta_xyz: [0.0, 1.0, 0.0],
            radius: 0.836_756_05,
        };

        assert!(native_special_ai_solid_contact_requests_death(
            RobotsAiHandlerClass::Ef01Mine,
            false,
            overlapping_mine,
            player,
        ));
        assert!(!native_special_ai_solid_contact_requests_death(
            RobotsAiHandlerClass::Ef01Mine,
            true,
            overlapping_mine,
            player,
        ));
        assert!(!native_special_ai_solid_contact_requests_death(
            RobotsAiHandlerClass::Ef01Mine,
            false,
            distant_mine,
            player,
        ));
        assert!(native_special_ai_solid_contact_requests_death(
            RobotsAiHandlerClass::Eq04Mine,
            false,
            overlapping_mine,
            player,
        ));
        assert!(!native_special_ai_solid_contact_requests_death(
            RobotsAiHandlerClass::DogBot,
            false,
            overlapping_mine,
            player,
        ));
    }

    #[test]
    fn eq03_spider_production_helper_commits_native_attachment_spin() {
        let config = RobotsStandardMonsterBehaviorConfig::eq03_spider();
        let mut runtime = NativeStandardMonsterRuntime::new(config);
        let expected = step_eq03_spider_attachment_rotation([0.0, 0.0, 0.0, 1.0]);

        service_native_eq03_spider_post_common_update(&mut runtime);

        assert_eq!(runtime.handler_class, RobotsAiHandlerClass::Eq03Spider);
        assert_eq!(runtime.eq03_attachment.local_spin_xyzw, expected);
    }

    #[test]
    fn ef01_first_update_helper_commits_native_606_bit2_and_644_state() {
        let config = RobotsStandardMonsterBehaviorConfig::ef01_mine();
        let mut runtime = NativeStandardMonsterRuntime::new(config);
        let mut physics = RobotsCharacterPhysicsRuntimeState::monster_ctor_default();
        apply_native_ef01_first_update_effects(
            &mut runtime,
            &mut physics,
            RobotsEf01MineFirstUpdateRoute::FollowFlyingPath { path_uid: 0x0b00_0049 },
        );
        assert!(runtime.ef01_flying_path_latch_644);
        assert!(!physics.handler_606);
        assert!(physics.object_flag_bit0);
        assert!(physics.object_flag_bit2);

        let mut runtime = NativeStandardMonsterRuntime::new(config);
        let mut physics = RobotsCharacterPhysicsRuntimeState::monster_ctor_default();
        apply_native_ef01_first_update_effects(
            &mut runtime,
            &mut physics,
            RobotsEf01MineFirstUpdateRoute::PursueNavMesh,
        );
        assert!(!runtime.ef01_flying_path_latch_644);
        assert!(physics.handler_606);
        assert!(!physics.object_flag_bit0);
        assert!(!physics.object_flag_bit2);
    }

    #[test]
    fn ef01_and_ef03_production_helper_commit_shared_native_blades_spin() {
        for config in [
            RobotsStandardMonsterBehaviorConfig::ef01_mine(),
            RobotsStandardMonsterBehaviorConfig::ef03_evilbot(),
        ] {
            assert!(RobotsBladesAttachmentSpec::for_handler_class(config.handler_class).is_some());
            let mut runtime = NativeStandardMonsterRuntime::new(config);
            for _ in 0..60 {
                service_native_blades_post_common_update(&mut runtime);
            }
            let q = runtime.blades_attachment_local_spin_xyzw;
            assert!(q[0].abs() < 2.0e-6);
            assert!(q[1].abs() < 2.0e-5);
            assert!(q[2].abs() < 2.0e-6);
            assert!((q[3] - 1.0).abs() < 2.0e-5);
        }
    }

    #[test]
    fn thiefbot_production_helper_commits_native_fast_blades_spin() {
        let config = RobotsStandardMonsterBehaviorConfig::thiefbot();
        let mut runtime = NativeStandardMonsterRuntime::new(config);

        service_native_thiefbot_post_common_update(
            &mut runtime,
            Vec3::ZERO,
            0.0,
            Some(Vec3::new(0.0, 0.0, 1.0)),
        );

        assert_eq!(runtime.handler_class, RobotsAiHandlerClass::ThiefBot);
        assert_eq!(
            runtime.thiefbot_attachment.target_spin_radians_per_second,
            eurochef_shared::robots_runtime::thiefbot::ROBOTS_THIEFBOT_FAST_SPIN_RADIANS_PER_SECOND
        );
        assert!(runtime.thiefbot_attachment.current_spin_radians_per_second
            > eurochef_shared::robots_runtime::thiefbot::ROBOTS_THIEFBOT_SLOW_SPIN_RADIANS_PER_SECOND);
        assert!(runtime.thiefbot_attachment.local_spin_xyzw[2] < 0.0);
    }

    #[test]
    fn ew09_production_helpers_commit_derived_attack_motion_and_attachment_spin() {
        let config = RobotsStandardMonsterBehaviorConfig::ew09_armoured();
        let visual = ProcessedCharacterVisual {
            file: 0x0100_0045,
            script: 0,
            runtime_type: 5,
            config_index: 8,
            handler_class: RobotsAiHandlerClass::Ew09Armoured,
            handler_flags_628: 1,
            initial_health: Some(1),
            attacker_priority_638: None,
            explosion_uid_634: None,
            explosion_uid_630: None,
            magnetic_mass: None,
            pickup_drop_count: None,
            collision: Some(crate::maps::ProcessedCharacterCollisionProfile {
                animskin: 0,
                shape: crate::maps::ProcessedCharacterCollisionShape::Sphere { radius: 1.0 },
                local_center: Vec3::ZERO,
                local_orientation: [0.0, 0.0, 0.0, 1.0],
                transform_selector: 0,
            }),
            hit_area: None,
            initial_animation: None,
            animation_modes: std::collections::BTreeMap::new(),
            animation_mode_scripts: std::collections::BTreeMap::new(),
            anim_datums: std::collections::BTreeMap::new(),
            animation_mode_datum_tracks: std::collections::BTreeMap::new(),
            move_animation: None,
            turn_on_spot_l_animation: None,
            turn_on_spot_r_animation: None,
        };
        let mut body = RuntimeCharacterBodyState::from_visual(
            &visual,
            ROBOTS_HIT_QUERY_RAW_GROUP1,
            Vec3::ZERO,
            Quat::IDENTITY,
        )
        .expect("EW09 test body");
        let mut runtime = NativeStandardMonsterRuntime::new(config);
        let mut physics = RobotsCharacterPhysicsRuntimeState::monster_ctor_default();

        apply_native_ew09_derived_attack_motion(&mut runtime, &mut physics, &mut body, 0.0);
        let expected_velocity = Vec3::new(0.0, 0.0, 4.4);
        assert_eq!(
            runtime.animation.character_physics_velocity(),
            expected_velocity
        );
        assert_eq!(physics.velocity_xyz, expected_velocity.to_array());
        assert!((body.owner_position.z - 4.4 * ROBOTS_FIXED_STEP_SECONDS).abs() < 1.0e-6);
        assert_eq!(body.owner_position.x, 0.0);
        assert_eq!(body.owner_position.y, 0.0);

        service_native_ew09_post_common_update(&mut runtime);
        assert_eq!(
            runtime.ew09_attachment_local_spin_xyzw,
            step_ew09_attachment_rotation([0.0, 0.0, 0.0, 1.0])
        );
    }

    #[test]
    fn eb13_pre_common_animator_service_uses_shared_native_pose_reducer() {
        let mut knight = NativeStandardMonsterRuntime::new(
            RobotsStandardMonsterBehaviorConfig::eb13_knightbot(),
        );
        assert!(knight.knightbot_animator.is_some());
        service_native_knightbot_pre_common_update(
            &mut knight,
            Vec3::new(2.0, 3.0, 4.0),
            Vec3::new(2.0, 3.0, 5.0),
        );
        let animator = knight.knightbot_animator.expect("EB13 AttachAnimator state");
        assert_eq!(animator.position_xyz, [2.0, 3.0, 4.0]);
        assert_eq!(animator.rotation_xyzw, [0.0, 0.0, 0.0, 1.0]);

        let ordinary = NativeStandardMonsterRuntime::new(
            RobotsStandardMonsterBehaviorConfig::eb14_minion(),
        );
        assert!(ordinary.knightbot_animator.is_none());
    }

    #[test]
    fn eb14_and_ew10_post_common_attachment_spin_reaches_production_runtime() {
        for config in [
            RobotsStandardMonsterBehaviorConfig::eb14_minion(),
            RobotsStandardMonsterBehaviorConfig::ew10_minion(),
        ] {
            let attachment = RobotsMinionAttachmentConfig::for_handler_class(config.handler_class)
                .expect("native minion attachment config");
            let mut runtime = NativeStandardMonsterRuntime::new(config);
            service_native_minion_attachment_post_common_update(&mut runtime, attachment);
            assert_eq!(
                runtime.minion_attachment_local_spin_xyzw,
                step_minion_attachment_rotation([0.0, 0.0, 0.0, 1.0], attachment)
            );
        }
    }

    #[test]
    fn permanent_sound_registration_preserves_first_native_owner_by_uid() {
        let sound = RobotsPermanentSoundConfig {
            sound_uid: 0x1AF0_0155,
            native_parameter: 100,
        };
        let mut registry = FxHashMap::default();
        let first = register_native_ai_permanent_sound(&mut registry, 0x1111, sound);
        let second = register_native_ai_permanent_sound(&mut registry, 0x2222, sound);
        assert_eq!(first.owner_key, 0x1111);
        assert_eq!(second.owner_key, 0x1111);
        assert_eq!(registry.len(), 1);
        assert_eq!(registry[&sound.sound_uid].native_parameter, 100);
    }

    #[test]
    fn serialized_ai_deferred_destroy_keeps_xitem_live_until_flush() {
        let key = 0x1234_u64;
        let mut lifecycle = RuntimeScriptTriggerLifecycleState {
            xitem_exists: true,
            xitem_state: 0,
            delay_counter: 0,
            current_opacity: 1.0,
            target_opacity: 1.0,
        };
        let mut deferred = NativeSerializedAiDeferredDestroyRuntime::default();

        deferred.mark(key);
        deferred.mark(key);
        assert!(deferred.pending.contains(&key));
        assert!(lifecycle.xitem_exists);

        let pending = deferred.take_all();
        assert_eq!(pending, vec![key]);
        assert!(!deferred.pending.contains(&key));
        lifecycle.cleanup_zone_failed();
        assert!(!lifecycle.xitem_exists);
    }

    #[test]
    fn minebot_three_hits_arm_fatal_and_creator_latch_blocks_respawn() {
        let mut hit = RobotsAiHitReactionState {
            query_flags_snapshot: 0,
            query_serial_snapshot: u16::MAX,
            hit_metadata_snapshot: 0,
            last_query_serial: u16::MAX,
            capability_flags: 0,
            health: 3,
            got_hit_latch: false,
            owner_category: 5,
        };
        for serial in 1..=3 {
            let result = hit.apply_hit(
                RobotsAcceptedHitReactionInput {
                    query_flags: 0,
                    query_serial: serial,
                    hit_metadata: 0,
                    source_is_candidate_owner: false,
                    secondary_source_is_candidate_owner: false,
                },
                false,
            );
            assert!(result.reaction_accepted);
            assert_eq!(result.health_damage, 1);
        }
        assert_eq!(hit.health, 0);
        assert!(RuntimeNativeAiHitFatalState::normal_fatal_hit_condition(
            hit.got_hit_latch,
            hit.health,
        ));

        let fatal_config = RobotsAiHandlerClass::DogBot
            .common_fatal_hit_config()
            .unwrap();
        let (anim_mode, _) =
            robots_common_ai_hit_direction_selection(fatal_config, 0.0, std::f32::consts::PI);
        assert_eq!(anim_mode, 0x0900_0033);

        let mut fatal = RuntimeNativeAiHitFatalState::default();
        assert!(!fatal.advance_fixed_tick_plan(false).request_destroy);
        assert!(fatal.apply_monster_explosion_event(true));
        let mut destroy = false;
        for _ in 0..6 {
            destroy = fatal.advance_fixed_tick_plan(false).request_destroy;
        }
        assert!(destroy);

        let death_position = Vec3::new(4.0, 5.0, 6.0);
        let mut creator = NativeSerializedAiCreatorRuntime::default();
        assert!(creator.allows_create_item());
        creator.record_natural_death(death_position);
        assert!(!creator.allows_create_item());
        assert_eq!(creator.position_override, Some(death_position));
    }

    #[test]
    fn dogbot_fatal_probability_event_fails_closed_then_late_event_arms_fade() {
        let mut process_seed = None;
        let mut fatal = RuntimeNativeAiHitFatalState::default();

        // Real EQ01 DogBot HitDeath scripts emit an early MonsterExplosion with
        // modulus 2. Unknown DAT_007BE1E8 provenance must not fabricate that draw.
        assert_eq!(
            robots_process_lcg_modulo_zero_gate(&mut process_seed, 2),
            None
        );
        assert!(!fatal.monster_explosion_completed);

        // Both F/B scripts later emit a guaranteed modulus-0 event. Native consumes
        // no RNG here, so death remains deterministic even without an RNG anchor.
        assert_eq!(
            robots_process_lcg_modulo_zero_gate(&mut process_seed, 0),
            Some(true)
        );
        assert!(fatal.apply_monster_explosion_event(true));
        let mut destroy = false;
        for _ in 0..6 {
            destroy = fatal.advance_fixed_tick_plan(false).request_destroy;
        }
        assert!(destroy);
        assert_eq!(process_seed, None);
    }

    #[test]
    fn rollerbot_missing_xpath_disables_only_follow_path_selector_node() {
        let follow = RobotsRollerBotFollowPathRuntimeState::from_setup_draw(0);
        assert_eq!(rollerbot_follow_path_host_priority(true, follow), 0x1f);
        assert_eq!(rollerbot_follow_path_host_priority(false, follow), 1);
        assert_eq!(
            rollerbot_behavior_winner(
                10,
                30,
                1,
                1,
                1,
                rollerbot_follow_path_host_priority(false, follow)
            ),
            Some(RobotsRollerBotBehaviorWinner::Pursue)
        );
        assert_eq!(
            rollerbot_behavior_winner(
                10,
                30,
                1,
                1,
                1,
                rollerbot_follow_path_host_priority(true, follow)
            ),
            Some(RobotsRollerBotBehaviorWinner::FollowNetworkPath)
        );
    }

    #[test]
    fn rollerbot_zero_health_side_effect_clears_only_retained_motion_lane() {
        let mut runtime = NativeRollerBotRuntime::new(0, Vec3::new(1.0, 2.0, 3.0), 0.75);
        runtime.locomotion.linear_velocity_xyz = [4.0, 5.0, 6.0];
        let position = runtime.locomotion.position_xyz;
        let yaw = runtime.locomotion.yaw_radians;
        runtime.clear_retained_motion();
        assert_eq!(runtime.locomotion.linear_velocity_xyz, [0.0; 3]);
        assert_eq!(runtime.locomotion.position_xyz, position);
        assert_eq!(runtime.locomotion.yaw_radians, yaw);
    }

    #[test]
    fn standard_monster_common_animation_event_host_retains_native_script_and_attachment_state() {
        fn animation_event(event_type: u32, args: &[u32]) -> NativeAiAnimationEvent {
            let mut data = vec![0u8; 4 + args.len() * 4];
            for (index, arg) in args.iter().copied().enumerate() {
                data[4 + index * 4..8 + index * 4].copy_from_slice(&arg.to_le_bytes());
            }
            NativeAiAnimationEvent {
                event_type,
                data,
                start: None,
                length: None,
                pose_seconds: 0.0,
                owner_position: Vec3::ZERO,
                owner_rotation: Quat::IDENTITY,
            }
        }

        let mut runtime = NativeStandardMonsterRuntime::new(
            RobotsStandardMonsterBehaviorConfig::jailbot_normal(),
        );
        runtime.apply_common_animation_event(&animation_event(
            event_type::SET_SCRIPT_VALUE,
            &[7.75f32.to_bits()],
        ));
        assert_eq!(runtime.handler_script_value_45c, 7);

        runtime.apply_common_animation_event(&animation_event(
            event_type::ATTACH_SWOOSH,
            &[0x1900_0018, u32::MAX],
        ));
        runtime.apply_common_animation_event(&animation_event(
            event_type::ATTACH_PARTICLE,
            &[0x1100_0007, 0x1000_0001],
        ));
        assert_eq!(runtime.character_attachments.attachments.len(), 2);
        assert_eq!(
            runtime.character_attachments.attachments[0].resource_uid,
            0x1900_0018
        );
        assert_eq!(
            runtime.character_attachments.attachments[1].resource_uid,
            0x1100_0007
        );

        runtime.apply_common_animation_event(&animation_event(
            event_type::DETTACH_SWOOSH,
            &[0x1900_0017],
        ));
        assert_eq!(runtime.character_attachments.attachments.len(), 1);
        assert_eq!(
            runtime.character_attachments.attachments[0].resource_uid,
            0x1100_0007
        );
    }

    #[test]
    fn turretbot_production_host_stays_specialized_and_tracks_head_without_body_turn() {
        assert!(RobotsStandardMonsterBehaviorConfig::for_handler_class(
            RobotsAiHandlerClass::TurretBot
        )
        .is_none());
        assert_eq!(
            RobotsAiHandlerClass::TurretBot.common_fatal_hit_config(),
            Some(RobotsCommonAiHitConfig::turretbot_fatal())
        );

        let mut runtime = NativeTurretBotRuntime::default();
        let config = turretbot_headtrack_attack_config();
        let priority = headtrack_attack_priority(
            runtime.headtrack_attack,
            config,
            Some(RobotsHeadtrackAttackGateInput {
                owner_position_xyz: [0.0; 3],
                target_position_xyz: [10.0, 0.0, 0.0],
                target_visible: true,
                class_attack_allowed: true,
            }),
        );
        assert_eq!(priority, 0x32);
        assert_eq!(
            generic_attack_group_priority(&runtime.attack_group, &[priority]),
            0x32
        );
        assert_eq!(
            enter_generic_attack_group(&mut runtime.attack_group, &[priority], Some(0)),
            Some(0)
        );
        enter_headtrack_attack(&mut runtime.headtrack_attack);
        let step = step_headtrack_attack(
            &mut runtime.headtrack_attack,
            config,
            RobotsHeadtrackAttackStepInput {
                owner_position_xyz: [0.0; 3],
                target_position_xyz: Some([10.0, 0.0, 0.0]),
            },
        );
        assert!((step.headtrack_yaw_radians - 1.5_f32.to_radians()).abs() < 1.0e-6);
        assert_eq!(
            runtime.scrambled_hit.config,
            turretbot_scrambled_hit_config()
        );
    }

    #[test]
    fn guardbot_production_host_uses_turn_attack_child_zero_and_circle_locomotion() {
        let config = RobotsStandardMonsterBehaviorConfig::guardbot();
        let mut runtime = NativeStandardMonsterRuntime::new(config);
        let turn_config = config.turn_then_attacks[0].expect("GuardBot AI_TurnAttack");
        let turn_priority = turn_then_attack_priority(
            runtime.turn_then_attacks[0],
            turn_config,
            RobotsTurnThenAttackGateInput {
                owner_position_xyz: [0.0, 0.0, 0.0],
                target_position_xyz: [0.0, 0.0, 10.0],
                target_visible: true,
                class_attack_allowed: true,
            },
        );
        let child_priorities = [turn_priority, 1, 1];
        assert!(config.turn_then_attacks[0].is_some());
        assert_eq!(
            generic_attack_group_priority(&runtime.attack_group, &child_priorities),
            0x32
        );
        assert_eq!(
            enter_generic_attack_group(&mut runtime.attack_group, &child_priorities, Some(0)),
            Some(0)
        );
        enter_turn_then_attack(&mut runtime.turn_then_attacks[0]);
        assert!(runtime.turn_then_attacks[0].active);

        let circle = config.circle_target().expect("GuardBot AI_CircleTarget");
        assert!(enter_circle_target(
            &mut runtime.circle_target,
            circle,
            Some(0x1235)
        ));
        let circle_step = step_circle_target(
            runtime.circle_target,
            circle,
            [0.0, 0.0, 0.0],
            Some([20.0, 0.0, 0.0]),
        )
        .expect("CircleTarget step");
        let locomotion = runtime
            .step_standard_locomotion(RobotsAiLocomotionInput {
                steering_target_yaw_radians: circle_step.steering_target_yaw_radians,
                target_locomotion_scalar: circle_step.target_locomotion_scalar,
                turn_rate: circle_step.turn_rate,
                handler_flags_628: 0,
                move_mode_active_on_entry: false,
                current_owner_yaw_radians: 0.0,
                runtime_rate_scale: 1.0,
            })
            .expect("GuardBot CircleTarget must use common Monster locomotion");
        assert_eq!(locomotion.locomotion_scalar, ROBOTS_FIXED_STEP_SECONDS);
    }

    #[test]
    fn monster_2rockets_dual_turn_attack_states_preserve_native_child_order() {
        let config = RobotsStandardMonsterBehaviorConfig::monster_2rockets();
        let mut runtime = NativeStandardMonsterRuntime::new(config);
        let input = RobotsTurnThenAttackGateInput {
            owner_position_xyz: [0.0, 0.0, 0.0],
            target_position_xyz: [0.0, 0.0, 1.95],
            target_visible: true,
            class_attack_allowed: true,
        };
        let primary = config.turn_then_attacks[0].expect("2Rockets primary");
        let secondary = config.turn_then_attacks[1].expect("2Rockets secondary");
        let priorities = [
            turn_then_attack_priority(runtime.turn_then_attacks[0], primary, input),
            turn_then_attack_priority(runtime.turn_then_attacks[1], secondary, input),
            0,
        ];
        assert_eq!(priorities, [0x32, 0x32, 0]);

        assert_eq!(
            enter_generic_attack_group(&mut runtime.attack_group, &priorities, Some(0)),
            Some(0)
        );
        enter_turn_then_attack(&mut runtime.turn_then_attacks[0]);
        assert!(runtime.turn_then_attacks[0].active);
        assert!(!runtime.turn_then_attacks[1].active);

        leave_turn_then_attack(&mut runtime.turn_then_attacks[0]);
        leave_generic_attack_group(&mut runtime.attack_group);
        assert_eq!(
            enter_generic_attack_group(&mut runtime.attack_group, &priorities, Some(1)),
            Some(1)
        );
        enter_turn_then_attack(&mut runtime.turn_then_attacks[1]);
        assert!(!runtime.turn_then_attacks[0].active);
        assert!(runtime.turn_then_attacks[1].active);
    }

    #[test]
    fn monster_2rockets_hitcheck_queue_keeps_two_and_drops_oldest_before_append() {
        fn entry(serial: u16, source_anim_mode: u32) -> NativeStandardMonsterHitQueryRuntime {
            NativeStandardMonsterHitQueryRuntime {
                query: RobotsHitQueryInitPlan::direct(0x1000_000C, 4.0, 0)
                    .instantiate(true, false, serial),
                source_anim_mode,
            }
        }

        let config = RobotsStandardMonsterBehaviorConfig::monster_2rockets();
        let mut runtime = NativeStandardMonsterRuntime::new(config);
        runtime.register_hit_query(entry(1, 0x0900_0025), config.hit_query_capacity());
        runtime.register_hit_query(entry(2, 0x0900_0027), config.hit_query_capacity());
        assert_eq!(runtime.hit_queries.len(), 2);
        assert_eq!(runtime.hit_queries[0].query.serial, 1);
        assert_eq!(runtime.hit_queries[1].query.serial, 2);

        runtime.register_hit_query(entry(3, 0x0900_0025), config.hit_query_capacity());
        assert_eq!(runtime.hit_queries.len(), 2);
        assert_eq!(runtime.hit_queries[0].query.serial, 2);
        assert_eq!(runtime.hit_queries[1].query.serial, 3);

        let ordinary = RobotsStandardMonsterBehaviorConfig::guardbot();
        let mut ordinary_runtime = NativeStandardMonsterRuntime::new(ordinary);
        ordinary_runtime.register_hit_query(entry(7, 0x0900_0025), ordinary.hit_query_capacity());
        ordinary_runtime.register_hit_query(entry(8, 0x0900_0025), ordinary.hit_query_capacity());
        assert_eq!(ordinary_runtime.hit_queries.len(), 1);
        assert_eq!(ordinary_runtime.hit_queries[0].query.serial, 8);
    }

    #[test]
    fn magnabot_production_locomotion_seam_preserves_only_serviced_transition_state() {
        use eurochef_shared::robots_runtime::magnabot::RobotsMagnaBotTurnPhase;

        fn animation_event(event_type: u32, arg0: Option<u32>) -> NativeAiAnimationEvent {
            let mut data = vec![0u8; 8];
            if let Some(arg0) = arg0 {
                data[4..8].copy_from_slice(&arg0.to_le_bytes());
            }
            NativeAiAnimationEvent {
                event_type,
                data,
                start: None,
                length: None,
                pose_seconds: 0.0,
                owner_position: Vec3::ZERO,
                owner_rotation: Quat::IDENTITY,
            }
        }

        let config = RobotsStandardMonsterBehaviorConfig::magnabot();
        let mut runtime = NativeStandardMonsterRuntime::new(config);
        runtime.apply_common_animation_event(&animation_event(
            event_type::CHANGE_ANIM_MODE,
            Some(0x0900_0025),
        ));
        assert_eq!(runtime.magnabot_turn.queued_anim_mode, 0x0900_0025);

        // The dedicated 0x0046A200 node calls Magna +0x118 directly. With no +0x10C
        // service in the same handler update, native 0x00464D20 clears +0x640 again.
        runtime.magnabot_turn.begin_update();
        let request =
            target_facing_turn_request(RobotsTargetFacingTurnBehaviorConfig::magnabot(), 0.5);
        assert!(runtime
            .step_magnabot_direct_turn(
                request.yaw_error_radians,
                request.turn_rate_radians_per_second,
                request.anim_mode,
                0,
                0.0,
            )
            .is_none());
        assert_eq!(
            runtime.magnabot_turn.phase,
            RobotsMagnaBotTurnPhase::AwaitingSetupIdle
        );
        runtime.magnabot_turn.finish_update();
        assert_eq!(runtime.magnabot_turn.phase, RobotsMagnaBotTurnPhase::Idle);

        // The same +0x118 transition reached through Magna +0x10C is retained by
        // +0x649, allowing the next update to request the queued animation and then
        // advance to state3 on SetupIdle.
        runtime.magnabot_turn.begin_update();
        assert!(runtime
            .step_standard_locomotion(RobotsAiLocomotionInput {
                steering_target_yaw_radians: std::f32::consts::FRAC_PI_2,
                target_locomotion_scalar: 0.4,
                turn_rate: RobotsAiTurnRateInput::Explicit(std::f32::consts::PI),
                handler_flags_628: 0,
                move_mode_active_on_entry: false,
                current_owner_yaw_radians: 0.0,
                runtime_rate_scale: 1.0,
            })
            .is_none());
        runtime.magnabot_turn.finish_update();
        assert_eq!(
            runtime.magnabot_turn.phase,
            RobotsMagnaBotTurnPhase::AwaitingSetupIdle
        );

        runtime.magnabot_turn.begin_update();
        let queued = runtime
            .step_standard_locomotion(RobotsAiLocomotionInput {
                steering_target_yaw_radians: std::f32::consts::FRAC_PI_2,
                target_locomotion_scalar: 0.4,
                turn_rate: RobotsAiTurnRateInput::Explicit(std::f32::consts::PI),
                handler_flags_628: 0,
                move_mode_active_on_entry: false,
                current_owner_yaw_radians: 0.0,
                runtime_rate_scale: 1.0,
            })
            .expect("state1 must request queued AnimMode through Magna +0x118");
        assert_eq!(queued.requested_anim_mode, 0x0900_0025);
        runtime
            .magnabot_turn
            .apply_event(event_type::SETUP_IDLE, None);
        runtime.magnabot_turn.finish_update();
        assert_eq!(
            runtime.magnabot_turn.phase,
            RobotsMagnaBotTurnPhase::Turning
        );

        runtime.magnabot_turn.begin_update();
        let turning = runtime
            .step_standard_locomotion(RobotsAiLocomotionInput {
                steering_target_yaw_radians: std::f32::consts::FRAC_PI_2,
                target_locomotion_scalar: 0.4,
                turn_rate: RobotsAiTurnRateInput::Explicit(std::f32::consts::PI),
                handler_flags_628: 0,
                move_mode_active_on_entry: false,
                current_owner_yaw_radians: 0.0,
                runtime_rate_scale: 1.0,
            })
            .expect("state3 must delegate to base +0x118 turn path");
        assert_eq!(turning.requested_anim_mode, ROBOTS_ANIM_MODE_TURN_ON_SPOT);
        assert!(turning.direct_owner_yaw_write);
        assert!(turning.owner_yaw_radians > 0.0);
    }

    #[test]
    fn knightbot_primary_attack_group_and_secondary_host_remain_independent() {
        let config = RobotsStandardMonsterBehaviorConfig::eb13_knightbot();
        let mut runtime = NativeStandardMonsterRuntime::new(config);
        let turn_config = config.turn_then_attacks[1].expect("EB13 turn-then-attack child");
        let secondary_config = config
            .secondary_three_phase_attack
            .expect("EB13 secondary three-phase host");

        let turn_priority = turn_then_attack_priority(
            runtime.turn_then_attacks[1],
            turn_config,
            RobotsTurnThenAttackGateInput {
                owner_position_xyz: [0.0, 0.0, 0.0],
                target_position_xyz: [0.0, 0.0, 10.0],
                target_visible: true,
                class_attack_allowed: true,
            },
        );
        assert_eq!(turn_priority, 0x32);
        let child_priorities = [1, turn_priority, 1];
        assert_eq!(
            generic_attack_group_priority(&runtime.attack_group, &child_priorities),
            0x32
        );
        assert_eq!(
            enter_generic_attack_group(&mut runtime.attack_group, &child_priorities, Some(0)),
            Some(1)
        );
        enter_turn_then_attack(&mut runtime.turn_then_attacks[1]);
        assert!(runtime.turn_then_attacks[1].active);

        assert!(three_phase_attack_geometry_gate(
            secondary_config,
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 7.0],
            true,
        ));
        enter_three_phase_attack(&mut runtime.secondary_three_phase_attack);
        let start = step_three_phase_attack(
            &mut runtime.secondary_three_phase_attack,
            secondary_config,
            RobotsThreePhaseAttackInput {
                owner_position_xyz: [0.0, 0.0, 0.0],
                player_position_xyz: [0.0, 0.0, 7.0],
                target_visible: true,
            },
        );
        assert_eq!(start.requested_anim_mode, Some(0x0900_003E));
        three_phase_attack_setup_idle(&mut runtime.secondary_three_phase_attack);
        assert_eq!(
            runtime.secondary_three_phase_attack.phase,
            RobotsThreePhaseAttackPhase::Active
        );
        assert!(runtime.turn_then_attacks[1].active);

        let active = step_three_phase_attack(
            &mut runtime.secondary_three_phase_attack,
            secondary_config,
            RobotsThreePhaseAttackInput {
                owner_position_xyz: [0.0, 0.0, 0.0],
                player_position_xyz: [0.0, 0.0, 7.0],
                target_visible: true,
            },
        );
        assert_eq!(active.requested_anim_mode, Some(0x0900_003F));
        let end = step_three_phase_attack(
            &mut runtime.secondary_three_phase_attack,
            secondary_config,
            RobotsThreePhaseAttackInput {
                owner_position_xyz: [0.0, 0.0, 0.0],
                player_position_xyz: [0.0, 0.0, 9.0],
                target_visible: true,
            },
        );
        assert_eq!(end.requested_anim_mode, Some(0x0900_0040));
        three_phase_attack_setup_idle(&mut runtime.secondary_three_phase_attack);
        assert_eq!(
            runtime.secondary_three_phase_attack.phase,
            RobotsThreePhaseAttackPhase::Complete
        );
        assert!(runtime.turn_then_attacks[1].active);
    }

    #[test]
    fn eb15_launcher_attack_group_preserves_native_child_order_and_special_turn_path() {
        let config = RobotsStandardMonsterBehaviorConfig::eb15_launcher();
        let mut runtime = NativeStandardMonsterRuntime::new(config);
        let generic = config.attacks[0].expect("EB15 generic attack27");
        let special = config.turn_then_attacks[1].expect("EB15 turn-then-attack child");
        let generic_input = RobotsGenericAttackGateInput {
            owner_position_xyz: [0.0, 0.0, 0.0],
            owner_yaw_radians: 0.0,
            target_position_xyz: [0.0, 0.0, 2.0],
            target_visible: true,
            class_attack_allowed: true,
        };
        let generic_priority = generic_attack_priority(runtime.attacks[0], generic, generic_input);
        let special_priority = turn_then_attack_priority(
            runtime.turn_then_attacks[1],
            special,
            RobotsTurnThenAttackGateInput {
                owner_position_xyz: [0.0, 0.0, 0.0],
                target_position_xyz: [0.0, 0.0, 2.0],
                target_visible: true,
                class_attack_allowed: true,
            },
        );
        assert_eq!([generic_priority, special_priority, 0], [0x32, 0x32, 0]);

        let priorities = [generic_priority, special_priority, 0];
        assert_eq!(
            enter_generic_attack_group(&mut runtime.attack_group, &priorities, Some(0)),
            Some(0),
            "draw0 must select the first inserted generic Attack27 child"
        );
        leave_generic_attack_group(&mut runtime.attack_group);
        assert_eq!(
            enter_generic_attack_group(&mut runtime.attack_group, &priorities, Some(1)),
            Some(1),
            "draw1 must select the second inserted TurnThenAttack25 child"
        );
        enter_turn_then_attack(&mut runtime.turn_then_attacks[1]);
        let turn = step_turn_then_attack(
            &mut runtime.turn_then_attacks[1],
            special,
            RobotsTurnThenAttackStepInput {
                owner_position_xyz: [0.0, 0.0, 0.0],
                owner_yaw_radians: 0.0,
                target_position_xyz: [0.0, 0.0, 2.0],
                handler_flags_628: 0x1,
                runtime_rate_scale: 1.0,
            },
        );
        assert_eq!(turn.requested_anim_mode, Some(0));
        assert!(turn
            .direct_turn
            .is_some_and(|step| step.direct_owner_yaw_write));
        let attack = step_turn_then_attack(
            &mut runtime.turn_then_attacks[1],
            special,
            RobotsTurnThenAttackStepInput {
                owner_position_xyz: [0.0, 0.0, 0.0],
                owner_yaw_radians: 0.0,
                target_position_xyz: [0.0, 0.0, 2.0],
                handler_flags_628: 0x1,
                runtime_rate_scale: 1.0,
            },
        );
        assert_eq!(attack.requested_anim_mode, Some(0x0900_0025));
        assert!(attack.request_attack_action);
        assert!(
            runtime.secondary_three_phase_attack.phase == RobotsThreePhaseAttackPhase::Inactive
        );
    }

    #[test]
    fn eq04_production_physics_step_keeps_native_gravity_contact_phase_order() {
        let mut physics = RobotsCharacterPhysicsRuntimeState::monster_ctor_default();
        let contact = RuntimeAiEnvironmentFloorContact {
            point: Vec3::ZERO,
            normal: Vec3::Y,
        };
        let shape_at = |y: f32| RuntimeCharacterWorldShape::Sphere {
            center_xyz: [0.0, y, 0.0],
            radius: 0.5,
        };

        let grounded = advance_eq04_character_physics_step(
            &mut physics,
            Vec3::new(0.0, 0.501, 0.0),
            shape_at(0.501),
            Some(contact),
        );
        assert!((grounded.y - 0.5).abs() < 1.0e-6);
        assert_eq!(physics.contact_response_bits & 1, 1);
        assert!((physics.gravity_velocity_y + 9.8 / 60.0).abs() < 1.0e-6);

        let grounded_again = advance_eq04_character_physics_step(
            &mut physics,
            grounded,
            shape_at(grounded.y),
            Some(contact),
        );
        assert!((grounded_again.y - 0.5).abs() < 1.0e-6);
        assert_eq!(physics.gravity_velocity_y.to_bits(), (-3.0f32).to_bits());
        assert_eq!(physics.contact_response_bits & 1, 1);

        let first_airborne = advance_eq04_character_physics_step(
            &mut physics,
            grounded_again,
            shape_at(grounded_again.y),
            None,
        );
        assert!((first_airborne.y - 0.45).abs() < 1.0e-6);
        assert_eq!(physics.contact_response_bits & 1, 0);
        assert_eq!(physics.previous_contact_response_bits & 1, 1);

        let second_airborne = advance_eq04_character_physics_step(
            &mut physics,
            first_airborne,
            shape_at(first_airborne.y),
            None,
        );
        assert!((physics.gravity_velocity_y + 9.8 / 60.0).abs() < 1.0e-6);
        assert!(second_airborne.y < first_airborne.y);
    }

    #[test]
    fn standard_physics_locomotion_commit_uses_class_native_range_only() {
        let step = RobotsAiLocomotionStep {
            owner_yaw_radians: 0.0,
            locomotion_scalar: 0.5,
            requested_anim_mode: ROBOTS_ANIM_MODE_MOVE,
            turn_in_place_active: false,
            direct_owner_yaw_write: true,
        };
        let mut physics = FxHashMap::default();

        commit_native_standard_physics_locomotion(
            &mut physics,
            7,
            RobotsAiHandlerClass::Ew10Minion,
            step,
        );
        assert_eq!(physics.get(&7).unwrap().velocity_xyz, [0.0, 0.0, 3.0]);

        commit_native_standard_physics_locomotion(
            &mut physics,
            8,
            RobotsAiHandlerClass::MalfBot,
            step,
        );
        assert!(!physics.contains_key(&8));
    }

    #[test]
    fn bounce_family_physics_locomotion_uses_native_four_to_seven_speed_range() {
        let mut physics = RobotsCharacterPhysicsRuntimeState::monster_ctor_default();

        write_native_ai_physics_locomotion(&mut physics, 0.0, 0.0, 4.0, 7.0);
        assert_eq!(physics.velocity_xyz, [0.0, 0.0, 4.0]);

        write_native_ai_physics_locomotion(&mut physics, 0.0, 1.0, 4.0, 7.0);
        assert_eq!(physics.velocity_xyz, [0.0, 0.0, 7.0]);
    }

    #[test]
    fn horizontal_yaw_uses_native_x_sin_z_cos_basis() {
        assert!(horizontal_yaw(Quat::IDENTITY).abs() < 1.0e-6);
        let right = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        assert!((horizontal_yaw(right) - std::f32::consts::FRAC_PI_2).abs() < 1.0e-6);
    }
}
