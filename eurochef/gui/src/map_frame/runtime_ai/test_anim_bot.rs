use super::*;

const ROBOTS_TEST_ANIM_ATTACK_COUNT: usize = ROBOTS_TEST_ANIM_ATTACK_ANIM_MODES.len();

#[derive(Clone, Copy)]
struct NativeTestAnimHitQueryRuntime {
    query: RobotsHitQueryState,
    source_anim_mode: u32,
}

pub(crate) struct NativeTestAnimBotRuntime {
    idle: RobotsBaseMonsterIdleRuntimeState,
    patrol: RobotsAiPatrolRuntimeState,
    periodic_idle: RobotsPeriodicIdleRuntimeState,
    locomotion: RobotsAiLocomotionRuntimeState,
    attack_group: RobotsGenericAttackGroupRuntimeState,
    attacks: [RobotsGenericAttackRuntimeState; ROBOTS_TEST_ANIM_ATTACK_COUNT],
    common_hit: RobotsCommonAiHitRuntimeState,
    scrambled_hit: RobotsMalfBotScrambledHitRuntimeState,
    electro_hit: RobotsMalfBotElectroHitRuntimeState,
    magnetic_hit: RobotsMalfBotMagneticHitRuntimeState,
    magnetic_drop_charge_count: Option<u8>,
    active_node: Option<RobotsTestAnimBehaviorWinner>,
    animation: NativeAiAnimationRuntime,
    handler_script_value_45c: u8,
    character_attachments: RobotsCharacterAttachmentRuntimeState,
    hit_query: Option<NativeTestAnimHitQueryRuntime>,
}

impl Default for NativeTestAnimBotRuntime {
    fn default() -> Self {
        Self {
            idle: RobotsBaseMonsterIdleRuntimeState::default(),
            patrol: RobotsAiPatrolRuntimeState::configured(test_anim_patrol_config()),
            periodic_idle: RobotsPeriodicIdleRuntimeState::default(),
            locomotion: RobotsAiLocomotionRuntimeState::default(),
            attack_group: RobotsGenericAttackGroupRuntimeState::default(),
            attacks: [RobotsGenericAttackRuntimeState::default(); ROBOTS_TEST_ANIM_ATTACK_COUNT],
            common_hit: RobotsCommonAiHitRuntimeState::default(),
            scrambled_hit: RobotsMalfBotScrambledHitRuntimeState::default(),
            electro_hit: RobotsMalfBotElectroHitRuntimeState::default(),
            magnetic_hit: RobotsMalfBotMagneticHitRuntimeState::default(),
            magnetic_drop_charge_count: None,
            active_node: None,
            animation: NativeAiAnimationRuntime::default(),
            handler_script_value_45c: 0,
            character_attachments: RobotsCharacterAttachmentRuntimeState::default(),
            hit_query: None,
        }
    }
}

impl NativeTestAnimBotRuntime {
    fn advance_animation(
        &mut self,
        body: &mut RuntimeCharacterBodyState,
        requested_anim_mode: u32,
        owner_yaw_write: Option<f32>,
        policy: NativeAiRootMotionPolicy,
    ) -> Vec<NativeAiAnimationEvent> {
        let events = self.animation.advance(
            body,
            requested_anim_mode,
            owner_yaw_write,
            ROBOTS_FIXED_STEP_SECONDS,
            policy,
        );
        for event in &events {
            if event.event_type == event_type::SET_SCRIPT_VALUE {
                if let Some(value) = event.as_view().native_arg_float(0) {
                    self.handler_script_value_45c = native_script_ftol(value) as u8;
                }
            }
            let _ = apply_ai_character_attachment_event(
                &mut self.character_attachments,
                event.as_view(),
            );
        }
        events
    }
}

impl MapFrame {
    pub(super) fn advance_native_test_anim_bot_fixed(
        &mut self,
        map: &ProcessedMap,
        nav_regions: &[RobotsMonsterNavMeshView<'_>],
        player_position: Vec3,
        wall_time: f64,
    ) {
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            let Some(visual) = trigger.character_visual.as_ref() else {
                continue;
            };
            if visual.handler_class != RobotsAiHandlerClass::TestAnimBot {
                continue;
            }

            let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
            if !self.runtime_ai_character_xitem_live(map.hashcode, trigger_index) {
                self.native_test_anim_bot_runtime.remove(&key);
                self.native_monster_navigation.remove(&key);
                self.native_monster_physics.remove(&key);
                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                    body.clear_collision_animation_track();
                }
                continue;
            }
            if !self.runtime_character_bodies.contains_key(&key) {
                self.native_test_anim_bot_runtime.remove(&key);
                self.native_monster_navigation.remove(&key);
                self.native_monster_physics.remove(&key);
                continue;
            }

            let has_patrol = visual
                .animation_modes
                .contains_key(&ROBOTS_TEST_ANIM_PATROL_ANIM_MODE);
            let periodic_modes = ROBOTS_TEST_ANIM_PERIODIC_IDLE_ANIM_MODES
                .iter()
                .copied()
                .filter(|mode| visual.animation_modes.contains_key(mode))
                .collect::<Vec<_>>();
            let attack_modes = ROBOTS_TEST_ANIM_ATTACK_ANIM_MODES
                .iter()
                .copied()
                .filter(|mode| visual.animation_modes.contains_key(mode))
                .collect::<Vec<_>>();
            let has_scrambled = [0x0900_0076, 0x0900_0078, 0x0900_0077]
                .iter()
                .all(|mode| visual.animation_modes.contains_key(mode));
            let has_electro = visual.animation_modes.contains_key(&0x0900_007D);
            let has_magnetic = visual.animation_modes.contains_key(&0x0900_00E9);
            let has_common_hit = [0x0900_0029, 0x0900_002A]
                .iter()
                .all(|mode| visual.animation_modes.contains_key(mode));
            let effective_handler_flags = test_anim_effective_handler_flags(
                visual.handler_flags_628,
                visual.animation_modes.contains_key(&0x0900_0035),
                visual.animation_modes.contains_key(&0x0900_0036),
            );

            let allow_cross_group = trigger
                .data
                .get(7)
                .and_then(|value| *value)
                .is_some_and(|flags| flags & 0x8000 != 0);
            let Some(_) = self.apply_native_common_monster_nav_constraint(
                key,
                nav_regions,
                allow_cross_group,
                effective_handler_flags,
            ) else {
                continue;
            };

            let runtime_is_new = !self.native_test_anim_bot_runtime.contains_key(&key);
            if runtime_is_new {
                self.native_test_anim_bot_runtime
                    .insert(key, NativeTestAnimBotRuntime::default());
                if !periodic_modes.is_empty() {
                    let Some(draw) = self.next_native_ai_gameplay_rng_u32() else {
                        self.native_test_anim_bot_runtime.remove(&key);
                        continue;
                    };
                    initialize_periodic_idle(
                        &mut self
                            .native_test_anim_bot_runtime
                            .get_mut(&key)
                            .unwrap()
                            .periodic_idle,
                        ROBOTS_TEST_ANIM_PERIODIC_IDLE_BASE_DELAY_TICKS,
                        draw,
                    );
                }
            }

            let Some(body) = self.runtime_character_bodies.get(&key) else {
                continue;
            };
            let owner_position = body.owner_position;
            let owner_rotation = body.owner_rotation;
            let owner_scale = body.native_transform_scale_xyz();
            let owner_yaw_radians = horizontal_yaw(owner_rotation);

            // Common Monster +0x130 services the retained HitCheck before selector execution.
            let active_query = self
                .native_test_anim_bot_runtime
                .get(&key)
                .and_then(|runtime| {
                    runtime.hit_query.map(|entry| {
                        (
                            entry,
                            runtime.animation.sampled_pose_seconds(),
                            runtime.animation.current_anim_mode(),
                        )
                    })
                });
            if let Some((entry, pose_seconds, current_anim_mode)) = active_query {
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
                let _ = query.step(source_shape.is_some(), hit, 0, 1.0);
                if let Some(runtime) = self.native_test_anim_bot_runtime.get_mut(&key) {
                    runtime.hit_query = query.is_active().then_some(NativeTestAnimHitQueryRuntime {
                        query,
                        source_anim_mode: entry.source_anim_mode,
                    });
                }
            }

            if self.native_ai_normal_fatal_condition(key) {
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
            let top_game_state = self
                .native_cutscene_host_runtime
                .game_state_stack
                .last()
                .copied();
            let player_state = self.native_player_focus_runtime.player_state;
            let class_attack_allowed = robots_common_monster_attack_allowed(
                effective_handler_flags,
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
            let (got_hit_latch, query_flags_snapshot) = hit_snapshot
                .map(|hit| (hit.got_hit_latch, hit.query_flags_snapshot))
                .unwrap_or((false, 0));

            let attack_priorities = {
                let runtime = self.native_test_anim_bot_runtime.get(&key).unwrap();
                attack_modes
                    .iter()
                    .map(|mode| {
                        let native_index = ROBOTS_TEST_ANIM_ATTACK_ANIM_MODES
                            .iter()
                            .position(|candidate| candidate == mode)
                            .unwrap();
                        generic_attack_priority(
                            runtime.attacks[native_index],
                            test_anim_attack_config(*mode),
                            attack_input,
                        )
                    })
                    .collect::<Vec<_>>()
            };
            let attack_group_priority = if attack_modes.is_empty() {
                1
            } else {
                generic_attack_group_priority(
                    &self
                        .native_test_anim_bot_runtime
                        .get(&key)
                        .unwrap()
                        .attack_group,
                    &attack_priorities,
                )
            };
            let pursue_priority = if has_patrol {
                ai_pursue_priority(
                    test_anim_pursue_config(),
                    owner_position.to_array(),
                    Some(player_position.to_array()),
                )
            } else {
                1
            };
            let patrol_priority = if has_patrol {
                ai_patrol_priority(top_game_state)
            } else {
                1
            };
            let periodic_priority = if periodic_modes.is_empty() {
                1
            } else {
                periodic_idle_priority(
                    &self
                        .native_test_anim_bot_runtime
                        .get(&key)
                        .unwrap()
                        .periodic_idle,
                )
            };
            let common_hit_priority = if has_common_hit {
                hit_snapshot
                    .map(|hit| {
                        self.native_test_anim_bot_runtime
                            .get(&key)
                            .unwrap()
                            .common_hit
                            .priority(hit)
                    })
                    .unwrap_or(1)
            } else {
                1
            };
            let scrambled_priority = if has_scrambled {
                self.native_test_anim_bot_runtime
                    .get(&key)
                    .unwrap()
                    .scrambled_hit
                    .priority(got_hit_latch, query_flags_snapshot)
            } else {
                1
            };
            let electro_priority = if has_electro {
                self.native_test_anim_bot_runtime
                    .get(&key)
                    .unwrap()
                    .electro_hit
                    .priority(got_hit_latch, query_flags_snapshot)
            } else {
                1
            };
            let magnetic_priority = if has_magnetic {
                self.native_test_anim_bot_runtime
                    .get(&key)
                    .unwrap()
                    .magnetic_hit
                    .priority(got_hit_latch, query_flags_snapshot)
            } else {
                1
            };

            let winner = test_anim_behavior_winner(
                RobotsBaseMonsterIdleRuntimeState::priority(),
                patrol_priority,
                pursue_priority,
                periodic_priority,
                attack_group_priority,
                scrambled_priority,
                electro_priority,
                magnetic_priority,
                common_hit_priority,
            );
            let previous = self
                .native_test_anim_bot_runtime
                .get(&key)
                .and_then(|runtime| runtime.active_node);

            if previous != winner {
                if let Some(previous) = previous {
                    let runtime = self.native_test_anim_bot_runtime.get_mut(&key).unwrap();
                    match previous {
                        RobotsTestAnimBehaviorWinner::CommonIdle => runtime.idle.leave(),
                        RobotsTestAnimBehaviorWinner::PeriodicIdle => {
                            leave_periodic_idle(&mut runtime.periodic_idle)
                        }
                        RobotsTestAnimBehaviorWinner::AttackGroup => {
                            if let Some(selected_available_index) = runtime.attack_group.selected_index {
                                if let Some(mode) = attack_modes.get(selected_available_index) {
                                    if let Some(native_index) = ROBOTS_TEST_ANIM_ATTACK_ANIM_MODES
                                        .iter()
                                        .position(|candidate| candidate == mode)
                                    {
                                        leave_generic_attack(&mut runtime.attacks[native_index]);
                                    }
                                }
                            }
                            leave_generic_attack_group(&mut runtime.attack_group);
                            runtime.hit_query = None;
                        }
                        RobotsTestAnimBehaviorWinner::CommonHit => {
                            if let Some(hit) = self.native_ai_hit_reactions.get_mut(&key) {
                                runtime.common_hit.leave(hit);
                            }
                        }
                        RobotsTestAnimBehaviorWinner::ScrambledHit => runtime.scrambled_hit.leave(),
                        RobotsTestAnimBehaviorWinner::ElectroHit => runtime.electro_hit.leave(),
                        RobotsTestAnimBehaviorWinner::MagneticHit => runtime.magnetic_hit.leave(),
                        RobotsTestAnimBehaviorWinner::Patrol
                        | RobotsTestAnimBehaviorWinner::Pursue => {}
                    }
                }
                if matches!(
                    previous,
                    Some(
                        RobotsTestAnimBehaviorWinner::ElectroHit
                            | RobotsTestAnimBehaviorWinner::MagneticHit
                    )
                ) {
                    if let Some(hit) = self.native_ai_hit_reactions.get_mut(&key) {
                        hit.query_serial_snapshot = 0;
                        hit.last_query_serial = 0;
                    }
                }

                let mut entered = true;
                let mut record_attack = false;
                if let Some(next) = winner {
                    match next {
                        RobotsTestAnimBehaviorWinner::CommonIdle => {
                            self.native_test_anim_bot_runtime
                                .get_mut(&key)
                                .unwrap()
                                .idle
                                .enter();
                        }
                        RobotsTestAnimBehaviorWinner::PeriodicIdle => {
                            let select_draw = self.next_native_ai_gameplay_rng_u32();
                            let delay_draw =
                                select_draw.and_then(|_| self.next_native_ai_gameplay_rng_u32());
                            if let (Some(select_draw), Some(delay_draw)) = (select_draw, delay_draw) {
                                entered = enter_periodic_idle(
                                    &mut self
                                        .native_test_anim_bot_runtime
                                        .get_mut(&key)
                                        .unwrap()
                                        .periodic_idle,
                                    ROBOTS_TEST_ANIM_PERIODIC_IDLE_BASE_DELAY_TICKS,
                                    &periodic_modes,
                                    select_draw,
                                    delay_draw,
                                )
                                .is_some();
                            } else {
                                entered = false;
                            }
                        }
                        RobotsTestAnimBehaviorWinner::AttackGroup => {
                            let draw = self.next_native_ai_gameplay_rng_u32();
                            if let Some(draw) = draw {
                                let selected = enter_generic_attack_group(
                                    &mut self
                                        .native_test_anim_bot_runtime
                                        .get_mut(&key)
                                        .unwrap()
                                        .attack_group,
                                    &attack_priorities,
                                    Some(draw),
                                );
                                if let Some(selected_available_index) = selected {
                                    if let Some(mode) = attack_modes.get(selected_available_index) {
                                        let native_index = ROBOTS_TEST_ANIM_ATTACK_ANIM_MODES
                                            .iter()
                                            .position(|candidate| candidate == mode)
                                            .unwrap();
                                        enter_generic_attack(
                                            &mut self
                                                .native_test_anim_bot_runtime
                                                .get_mut(&key)
                                                .unwrap()
                                                .attacks[native_index],
                                            test_anim_attack_config(*mode),
                                        );
                                        record_attack = true;
                                    } else {
                                        entered = false;
                                    }
                                } else {
                                    entered = false;
                                }
                            } else {
                                entered = false;
                            }
                        }
                        RobotsTestAnimBehaviorWinner::CommonHit => {
                            if let (Some(source_yaw), Some(hit)) = (
                                self.native_ai_last_hit_source_yaw.get(&key).copied(),
                                self.native_ai_hit_reactions.get(&key).copied(),
                            ) {
                                self.native_test_anim_bot_runtime
                                    .get_mut(&key)
                                    .unwrap()
                                    .common_hit
                                    .enter_configured(
                                        test_anim_common_hit_config(),
                                        owner_yaw_radians,
                                        source_yaw,
                                        hit.last_query_serial,
                                    );
                            } else {
                                entered = false;
                            }
                        }
                        RobotsTestAnimBehaviorWinner::ScrambledHit => self
                            .native_test_anim_bot_runtime
                            .get_mut(&key)
                            .unwrap()
                            .scrambled_hit
                            .enter(),
                        RobotsTestAnimBehaviorWinner::ElectroHit => self
                            .native_test_anim_bot_runtime
                            .get_mut(&key)
                            .unwrap()
                            .electro_hit
                            .enter(),
                        RobotsTestAnimBehaviorWinner::MagneticHit => {
                            let runtime = self.native_test_anim_bot_runtime.get_mut(&key).unwrap();
                            runtime.magnetic_hit.enter(owner_position.y);
                            if runtime.magnetic_drop_charge_count.is_none() {
                                runtime.magnetic_drop_charge_count =
                                    Some(visual.pickup_drop_count.unwrap_or(0));
                            }
                        }
                        RobotsTestAnimBehaviorWinner::Patrol
                        | RobotsTestAnimBehaviorWinner::Pursue => {}
                    }
                }
                self.native_test_anim_bot_runtime
                    .get_mut(&key)
                    .unwrap()
                    .active_node = entered.then_some(winner).flatten();
                if record_attack {
                    self.native_monster_attack_cooldown.record_attack();
                }
            }

            let active_node = self
                .native_test_anim_bot_runtime
                .get(&key)
                .and_then(|runtime| runtime.active_node);
            let mut pending_hit_query: Option<(RobotsHitQueryInitPlan, u32)> = None;
            let mut projectile_events = Vec::<(
                RobotsCreateProjectileRequest,
                u32,
                f32,
                Vec3,
                Quat,
            )>::new();

            match active_node {
                Some(RobotsTestAnimBehaviorWinner::CommonIdle) => {
                    let runtime = self.native_test_anim_bot_runtime.get_mut(&key).unwrap();
                    if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                        let _ = runtime.advance_animation(
                            body,
                            ROBOTS_BASE_MONSTER_IDLE_ANIM_MODE,
                            None,
                            NativeAiRootMotionPolicy::NONE,
                        );
                    }
                }
                Some(RobotsTestAnimBehaviorWinner::Patrol) => {
                    let locomotion = {
                        let runtime = self.native_test_anim_bot_runtime.get_mut(&key).unwrap();
                        let patrol = step_ai_patrol(&mut runtime.patrol, test_anim_patrol_config());
                        let move_mode_active_on_entry =
                            runtime.animation.is_mode(ROBOTS_ANIM_MODE_MOVE);
                        step_ai_locomotion_after_steering_prepass(
                            &mut runtime.locomotion,
                            RobotsAiLocomotionInput {
                                steering_target_yaw_radians: patrol.steering_target_yaw_radians,
                                target_locomotion_scalar: patrol.target_locomotion_scalar,
                                turn_rate: patrol.turn_rate,
                                handler_flags_628: effective_handler_flags,
                                move_mode_active_on_entry,
                                current_owner_yaw_radians: owner_yaw_radians,
                                runtime_rate_scale: 1.0,
                            },
                        )
                    };
                    let owner_yaw_write = locomotion
                        .direct_owner_yaw_write
                        .then_some(locomotion.owner_yaw_radians);
                    let policy = locomotion_root_motion_policy(locomotion);
                    let runtime = self.native_test_anim_bot_runtime.get_mut(&key).unwrap();
                    if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                        let _ = runtime.advance_animation(
                            body,
                            locomotion.requested_anim_mode,
                            owner_yaw_write,
                            policy,
                        );
                    }
                }
                Some(RobotsTestAnimBehaviorWinner::Pursue) => {
                    let pursue = step_ai_pursue(
                        test_anim_pursue_config(),
                        owner_position.to_array(),
                        player_position.to_array(),
                    );
                    let locomotion = {
                        let runtime = self.native_test_anim_bot_runtime.get_mut(&key).unwrap();
                        let move_mode_active_on_entry =
                            runtime.animation.is_mode(ROBOTS_ANIM_MODE_MOVE);
                        step_ai_locomotion_after_steering_prepass(
                            &mut runtime.locomotion,
                            RobotsAiLocomotionInput {
                                steering_target_yaw_radians: pursue.target_yaw_radians,
                                target_locomotion_scalar: pursue.locomotion_scalar,
                                turn_rate: RobotsAiTurnRateInput::Default,
                                handler_flags_628: effective_handler_flags,
                                move_mode_active_on_entry,
                                current_owner_yaw_radians: owner_yaw_radians,
                                runtime_rate_scale: 1.0,
                            },
                        )
                    };
                    let owner_yaw_write = locomotion
                        .direct_owner_yaw_write
                        .then_some(locomotion.owner_yaw_radians);
                    let policy = locomotion_root_motion_policy(locomotion);
                    let runtime = self.native_test_anim_bot_runtime.get_mut(&key).unwrap();
                    if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                        let _ = runtime.advance_animation(
                            body,
                            locomotion.requested_anim_mode,
                            owner_yaw_write,
                            policy,
                        );
                    }
                }
                Some(RobotsTestAnimBehaviorWinner::PeriodicIdle) => {
                    let requested_mode = self
                        .native_test_anim_bot_runtime
                        .get(&key)
                        .and_then(|runtime| runtime.periodic_idle.selected_anim_mode);
                    if let Some(requested_mode) = requested_mode {
                        let events = {
                            let runtime =
                                self.native_test_anim_bot_runtime.get_mut(&key).unwrap();
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                runtime.advance_animation(
                                    body,
                                    requested_mode,
                                    None,
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
                                self.native_test_anim_bot_runtime.get_mut(&key).unwrap();
                            complete_periodic_idle_setup_idle(&mut runtime.periodic_idle);
                            runtime.animation.setup_idle();
                        }
                    }
                }
                Some(RobotsTestAnimBehaviorWinner::AttackGroup) => {
                    let selected = self
                        .native_test_anim_bot_runtime
                        .get(&key)
                        .and_then(|runtime| runtime.attack_group.selected_index);
                    if let Some(selected_available_index) = selected {
                        if let Some(mode) = attack_modes.get(selected_available_index).copied() {
                            let native_index = ROBOTS_TEST_ANIM_ATTACK_ANIM_MODES
                                .iter()
                                .position(|candidate| *candidate == mode)
                                .unwrap();
                            let config = test_anim_attack_config(mode);
                            let step = {
                                let runtime =
                                    self.native_test_anim_bot_runtime.get_mut(&key).unwrap();
                                step_generic_attack(&mut runtime.attacks[native_index], config)
                            };
                            if let Some(requested_mode) = step.requested_anim_mode {
                                let events = {
                                    let runtime =
                                        self.native_test_anim_bot_runtime.get_mut(&key).unwrap();
                                    if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                        runtime.advance_animation(
                                            body,
                                            requested_mode,
                                            None,
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
                                                .native_test_anim_bot_runtime
                                                .get_mut(&key)
                                                .unwrap();
                                            generic_attack_setup_idle(
                                                &mut runtime.attacks[native_index],
                                                config,
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
                                                    requested_mode,
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
                                                pending_hit_query = Some((plan, requested_mode));
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
                Some(RobotsTestAnimBehaviorWinner::CommonHit) => {
                    let reenter = self
                        .native_ai_hit_reactions
                        .get(&key)
                        .and_then(|hit| {
                            self.native_test_anim_bot_runtime.get(&key).map(|runtime| {
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
                                self.native_test_anim_bot_runtime.get_mut(&key).unwrap();
                            runtime.animation.setup_idle();
                            runtime.common_hit.enter_configured(
                                test_anim_common_hit_config(),
                                owner_yaw_radians,
                                source_yaw,
                                hit.last_query_serial,
                            );
                        }
                    }
                    let (requested_mode, owner_yaw_write) = {
                        let runtime = self.native_test_anim_bot_runtime.get(&key).unwrap();
                        (
                            runtime.common_hit.requested_anim_mode,
                            runtime.common_hit.step_owner_yaw_configured(
                                test_anim_common_hit_config(),
                                owner_yaw_radians,
                            ),
                        )
                    };
                    let events = {
                        let runtime = self.native_test_anim_bot_runtime.get_mut(&key).unwrap();
                        if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                            runtime.advance_animation(
                                body,
                                requested_mode,
                                owner_yaw_write,
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
                        let runtimes = &mut self.native_test_anim_bot_runtime;
                        let hits = &mut self.native_ai_hit_reactions;
                        if let (Some(runtime), Some(hit)) =
                            (runtimes.get_mut(&key), hits.get_mut(&key))
                        {
                            runtime.common_hit.setup_idle(hit);
                            runtime.animation.setup_idle();
                        }
                    }
                }
                Some(RobotsTestAnimBehaviorWinner::ScrambledHit) => {
                    let requested_mode = {
                        let runtimes = &mut self.native_test_anim_bot_runtime;
                        let hits = &mut self.native_ai_hit_reactions;
                        let Some(runtime) = runtimes.get_mut(&key) else {
                            continue;
                        };
                        let Some(hit) = hits.get_mut(&key) else {
                            continue;
                        };
                        runtime.scrambled_hit.step(&mut hit.got_hit_latch)
                    };
                    if requested_mode != 0 {
                        let events = {
                            let runtime =
                                self.native_test_anim_bot_runtime.get_mut(&key).unwrap();
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                runtime.advance_animation(
                                    body,
                                    requested_mode,
                                    None,
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
                            let runtimes = &mut self.native_test_anim_bot_runtime;
                            let hits = &mut self.native_ai_hit_reactions;
                            if let (Some(runtime), Some(hit)) =
                                (runtimes.get_mut(&key), hits.get_mut(&key))
                            {
                                runtime.scrambled_hit.setup_idle(&mut hit.got_hit_latch);
                                runtime.animation.setup_idle();
                            }
                        }
                    }
                }
                Some(RobotsTestAnimBehaviorWinner::ElectroHit) => {
                    let requested_mode = self
                        .native_test_anim_bot_runtime
                        .get(&key)
                        .unwrap()
                        .electro_hit
                        .requested_anim_mode();
                    let events = {
                        let runtime = self.native_test_anim_bot_runtime.get_mut(&key).unwrap();
                        if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                            runtime.advance_animation(
                                body,
                                requested_mode,
                                None,
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
                        let plan = {
                            let runtime =
                                self.native_test_anim_bot_runtime.get_mut(&key).unwrap();
                            let plan = runtime.electro_hit.setup_idle();
                            runtime.animation.setup_idle();
                            plan
                        };
                        if plan.request_natural_death {
                            self.request_native_ai_natural_death(
                                map.hashcode,
                                key,
                                owner_position,
                            );
                        }
                    }
                }
                Some(RobotsTestAnimBehaviorWinner::MagneticHit) => {
                    let attachment_relation_matches =
                        self.native_player_magnetic_target_key == Some(key);
                    let owner_y = self
                        .runtime_character_bodies
                        .get(&key)
                        .map(|body| body.owner_position.y)
                        .unwrap_or(owner_position.y);
                    let plan = {
                        let runtimes = &mut self.native_test_anim_bot_runtime;
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
                        let runtime =
                            self.native_test_anim_bot_runtime.get_mut(&key).unwrap();
                        let drop_charge_count = runtime
                            .magnetic_drop_charge_count
                            .unwrap_or_else(|| visual.pickup_drop_count.unwrap_or(0));
                        let effect = runtime.magnetic_hit.step_attached_effect(
                            RobotsMalfBotMagneticEffectInput {
                                owner_y,
                                magnetic_mass: visual.magnetic_mass,
                                drop_charge_count,
                            },
                        );
                        if effect.serviced {
                            runtime.magnetic_drop_charge_count =
                                Some(effect.next_drop_charge_count);
                            magnetic_vertical_velocity_delta =
                                Some(effect.physics_vertical_velocity_delta);
                        }
                        Some(effect)
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

                    if let Some(requested_mode) = plan.requested_anim_mode {
                        let events = {
                            let runtime =
                                self.native_test_anim_bot_runtime.get_mut(&key).unwrap();
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                runtime.advance_animation(
                                    body,
                                    requested_mode,
                                    None,
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
                            self.native_test_anim_bot_runtime
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
                                physics.integrate_position(
                                    &mut position,
                                    ROBOTS_FIXED_STEP_SECONDS,
                                );
                                body.owner_position = Vec3::from_array(position);
                            }
                        }
                    }
                    if plan.request_natural_death {
                        self.request_native_ai_natural_death(
                            map.hashcode,
                            key,
                            owner_position,
                        );
                    }
                }
                None => {}
            }

            if let Some(runtime) = self.native_test_anim_bot_runtime.get_mut(&key) {
                tick_periodic_idle(&mut runtime.periodic_idle);
                for attack in &mut runtime.attacks {
                    tick_generic_attack(attack);
                }
            }

            if let Some((plan, source_anim_mode)) = pending_hit_query {
                let serial = self.allocate_native_hit_query_serial();
                if let Some(runtime) = self.native_test_anim_bot_runtime.get_mut(&key) {
                    runtime.hit_query = Some(NativeTestAnimHitQueryRuntime {
                        query: plan.instantiate(true, false, serial),
                        source_anim_mode,
                    });
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_defaults_match_test_anim_builder_shape() {
        let runtime = NativeTestAnimBotRuntime::default();
        assert_eq!(runtime.patrol.countdown_ticks, 420);
        assert!(!runtime.periodic_idle.initialized);
        assert!(runtime.active_node.is_none());
        assert_eq!(runtime.attacks.len(), 5);
        assert_eq!(runtime.handler_script_value_45c, 0);
    }
}
