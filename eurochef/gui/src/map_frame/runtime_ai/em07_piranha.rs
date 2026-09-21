use super::*;

#[derive(Clone, Copy)]
struct NativePiranhaHitQueryRuntime {
    query: RobotsHitQueryState,
    source_anim_mode: u32,
}

pub(crate) struct NativeEm07PiranhaRuntime {
    idle: RobotsBaseMonsterIdleRuntimeState,
    attack: RobotsGenericAttackRuntimeState,
    active_node: Option<RobotsEm07PiranhaBehaviorWinner>,
    animation: NativeAiAnimationRuntime,
    hit_query: Option<NativePiranhaHitQueryRuntime>,
    cycle: RobotsEm07PiranhaFlightCycleState,
    cached_points: Vec<RobotsEm07PiranhaCachedPoint>,
    ballistic_apex_parameter: f32,
    above_creator_latch: bool,
    attachment_spin_radians: f32,
    last_height_transition: Option<RobotsEm07PiranhaHeightTransition>,
    last_height_transition_plan: Option<RobotsEm07PiranhaHeightTransitionPlan>,
    transition_position_xyzw: [f32; 4],
}

impl NativeEm07PiranhaRuntime {
    fn new(
        retarget_period_ticks: i32,
        cached_points: Vec<RobotsEm07PiranhaCachedPoint>,
        ballistic_apex_parameter: f32,
    ) -> Self {
        Self {
            idle: RobotsBaseMonsterIdleRuntimeState::default(),
            attack: RobotsGenericAttackRuntimeState::default(),
            active_node: None,
            animation: NativeAiAnimationRuntime::default(),
            hit_query: None,
            cycle: RobotsEm07PiranhaFlightCycleState::new(retarget_period_ticks),
            cached_points,
            ballistic_apex_parameter,
            above_creator_latch: false,
            attachment_spin_radians: 0.0,
            last_height_transition: None,
            last_height_transition_plan: None,
            transition_position_xyzw: [0.0; 4],
        }
    }

    fn advance_animation(
        &mut self,
        body: &mut RuntimeCharacterBodyState,
        requested_anim_mode: u32,
        policy: NativeAiRootMotionPolicy,
    ) -> Vec<NativeAiAnimationEvent> {
        self.animation.advance(
            body,
            requested_anim_mode,
            None,
            ROBOTS_FIXED_STEP_SECONDS,
            policy,
        )
    }
}

fn em07_piranha_cached_points(map: &ProcessedMap, trigger: &ProcessedTrigger) -> Vec<RobotsEm07PiranhaCachedPoint> {
    (ROBOTS_EM07_PIRANHA_LINK_FIRST_INDEX..ROBOTS_EM07_PIRANHA_LINK_END_INDEX)
        .filter_map(|ordinal| trigger.links.get(ordinal).copied())
        .filter_map(|raw| eurochef_shared::robots_runtime::trigger_links::robots_trigger_link_index(raw, map.triggers.len()))
        .filter_map(|index| map.triggers.get(index))
        .filter(|linked| linked.ttype == 2)
        .filter_map(|linked| {
            let raw_radius = linked.data.first().copied().flatten()?;
            Some(RobotsEm07PiranhaCachedPoint {
                position_xyzw: [
                    linked.position.x,
                    linked.position.y,
                    linked.position.z,
                    0.0,
                ],
                native_radius: em07_piranha_distance_radius_from_raw(raw_radius),
            })
        })
        .collect()
}

impl MapFrame {
    fn em07_piranha_rng_offset_draws(&mut self, radius: f32) -> Option<(u32, u32)> {
        let angle_draw = self.next_native_ai_gameplay_rng_u32()?;
        let modulus = em07_piranha_random_radius_modulus(radius);
        let radius_draw = if modulus == 0 {
            0
        } else {
            self.next_native_ai_gameplay_rng_u32()?
        };
        Some((angle_draw, radius_draw))
    }

    fn em07_piranha_begin_flight(
        &mut self,
        key: u64,
        trigger: &ProcessedTrigger,
    ) -> bool {
        let proximity_radius = trigger
            .data
            .get(1)
            .and_then(|value| *value)
            .map(em07_piranha_distance_radius_from_raw)
            .unwrap_or_default();
        let Some((spawn_angle_draw, spawn_radius_draw)) =
            self.em07_piranha_rng_offset_draws(proximity_radius)
        else {
            return false;
        };

        let cached_len = self
            .native_em07_piranha_runtime
            .get(&key)
            .map(|runtime| runtime.cached_points.len())
            .unwrap_or_default();
        if cached_len == 0 {
            return false;
        }
        let Some(point_draw) = self.next_native_ai_gameplay_rng_u32() else {
            return false;
        };
        let target_radius = self
            .native_em07_piranha_runtime
            .get(&key)
            .and_then(|runtime| {
                runtime
                    .cached_points
                    .get(point_draw as usize % runtime.cached_points.len())
                    .map(|point| point.native_radius)
            })
            .unwrap_or_default();
        let Some((target_angle_draw, target_radius_draw)) =
            self.em07_piranha_rng_offset_draws(target_radius)
        else {
            return false;
        };

        let spawn = em07_piranha_creator_spawn_position_from_draws(
            [trigger.position.x, trigger.position.y, trigger.position.z, 0.0],
            proximity_radius,
            spawn_angle_draw,
            spawn_radius_draw,
        );
        let target = {
            let runtime = self.native_em07_piranha_runtime.get(&key).unwrap();
            em07_piranha_cached_target_from_draws(
                &runtime.cached_points,
                point_draw,
                target_angle_draw,
                target_radius_draw,
            )
        };
        let Some(target) = target else {
            return false;
        };

        let Some(body) = self.runtime_character_bodies.get_mut(&key) else {
            return false;
        };
        body.owner_position = Vec3::new(spawn[0], spawn[1], spawn[2]);

        let apex = self
            .native_em07_piranha_runtime
            .get(&key)
            .map(|runtime| runtime.ballistic_apex_parameter + 1.0)
            .unwrap_or(1.0);
        let physics = self
            .native_monster_physics
            .entry(key)
            .or_insert_with(RobotsCharacterPhysicsRuntimeState::monster_ctor_default);
        physics.gravity_acceleration = ROBOTS_EM07_PIRANHA_FIRST_UPDATE_COMPONENT_SCALAR;
        physics.apply_handler_606_mode(true);
        if physics
            .configure_ballistic_velocity_to_target(
                [spawn[0], spawn[1], spawn[2]],
                [target[0], target[1], target[2]],
                apex,
            )
            .is_none()
        {
            return false;
        }
        if let Some(runtime) = self.native_em07_piranha_runtime.get_mut(&key) {
            runtime.transition_position_xyzw = [target[0], trigger.position.y, target[2], 0.0];
            runtime.cycle.mark_flight_started();
        }
        true
    }

    pub(super) fn advance_native_em07_piranha_fixed(
        &mut self,
        map: &ProcessedMap,
        player_position: Vec3,
        wall_time: f64,
    ) {
        let attack_config = em07_piranha_attack_config();

        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            let Some(visual) = trigger.character_visual.as_ref() else {
                continue;
            };
            if visual.handler_class != RobotsAiHandlerClass::Em07PiranhaBot {
                continue;
            }

            let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
            if !self.runtime_ai_character_xitem_live(map.hashcode, trigger_index) {
                self.native_em07_piranha_runtime.remove(&key);
                self.native_monster_physics.remove(&key);
                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                    body.clear_collision_animation_track();
                }
                continue;
            }
            if !self.runtime_character_bodies.contains_key(&key) {
                self.native_em07_piranha_runtime.remove(&key);
                self.native_monster_physics.remove(&key);
                continue;
            }

            let runtime_is_new = !self.native_em07_piranha_runtime.contains_key(&key);
            if runtime_is_new {
                let ballistic_apex_parameter = trigger
                    .data
                    .get(2)
                    .and_then(|value| *value)
                    .map(em07_piranha_creator_target_radius)
                    .unwrap_or_default();
                let retarget_period_ticks = trigger
                    .data
                    .get(3)
                    .and_then(|value| *value)
                    .map(em07_piranha_creator_retarget_ticks)
                    .unwrap_or_default();
                let cached_points = em07_piranha_cached_points(map, trigger);
                self.native_em07_piranha_runtime.insert(
                    key,
                    NativeEm07PiranhaRuntime::new(
                        retarget_period_ticks,
                        cached_points,
                        ballistic_apex_parameter,
                    ),
                );
                let mut physics = RobotsCharacterPhysicsRuntimeState::monster_ctor_default();
                physics.gravity_acceleration = ROBOTS_EM07_PIRANHA_FIRST_UPDATE_COMPONENT_SCALAR;
                physics.apply_handler_606_mode(false);
                self.native_monster_physics.insert(key, physics);
                register_native_ai_permanent_sound(
                    &mut self.native_ai_permanent_sounds,
                    key,
                    RobotsPermanentSoundConfig {
                        sound_uid: ROBOTS_EM07_PIRANHA_PERMANENT_SOUND_UID,
                        native_parameter: ROBOTS_EM07_PIRANHA_PERMANENT_SOUND_PARAMETER,
                    },
                );
            }

            if self.native_ai_normal_fatal_condition(key) {
                continue;
            }

            // Native physics slot0 runs before the class +0x34 update.
            let flight_active = self
                .native_em07_piranha_runtime
                .get(&key)
                .is_some_and(|runtime| runtime.cycle.flight_active);
            if flight_active {
                if let (Some(physics), Some(body)) = (
                    self.native_monster_physics.get_mut(&key),
                    self.runtime_character_bodies.get_mut(&key),
                ) {
                    physics.advance_base_gravity(ROBOTS_FIXED_STEP_SECONDS);
                    let mut position = body.owner_position.to_array();
                    physics.integrate_position_with_gravity(&mut position, ROBOTS_FIXED_STEP_SECONDS);
                    body.owner_position = Vec3::from_array(position);
                    physics.latch_contact_response_after_base_update();
                }
            }

            let Some(body_snapshot) = self.runtime_character_bodies.get(&key) else {
                continue;
            };
            let owner_position = body_snapshot.owner_position;
            let owner_rotation = body_snapshot.owner_rotation;
            let owner_scale = body_snapshot.native_transform_scale_xyz();
            // Common retained HitCheck service precedes selector execution.
            let active_query = self.native_em07_piranha_runtime.get(&key).and_then(|runtime| {
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
                if let Some(runtime) = self.native_em07_piranha_runtime.get_mut(&key) {
                    runtime.hit_query = query.is_active().then_some(NativePiranhaHitQueryRuntime {
                        query,
                        source_anim_mode: entry.source_anim_mode,
                    });
                }
            }

            // Class update 0x004676F0: countdown reload starts a flight.
            // Radius draws are consumed only when the native modulus is nonzero.
            let should_launch = self
                .native_em07_piranha_runtime
                .get_mut(&key)
                .is_some_and(|runtime| runtime.cycle.tick_waiting());
            if should_launch {
                let _ = self.em07_piranha_begin_flight(key, trigger);
            }

            // Native class update writes owner yaw/pitch from current CharacterPhysics velocity.
            if let Some(physics) = self.native_monster_physics.get(&key).copied() {
                let [pitch, yaw] = em07_piranha_owner_pitch_yaw_from_physics(
                    physics.velocity_xyz,
                    physics.gravity_velocity_y,
                );
                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                    body.owner_rotation = Quat::from_euler(glam::EulerRot::ZXY, 0.0, pitch, yaw);
                }
            }

            // Flight completes on the native creator-height crossing: snap back to
            // creator X/Z at creatorY-1, disable gravity ownership and clear +0x654.
            let should_reset_flight = self
                .native_em07_piranha_runtime
                .get(&key)
                .is_some_and(|runtime| runtime.cycle.flight_active)
                && self
                    .runtime_character_bodies
                    .get(&key)
                    .is_some_and(|body| {
                        em07_piranha_flight_reset_ready(
                            body.owner_position.y,
                            trigger.position.y,
                        )
                    });
            if should_reset_flight {
                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                    body.owner_position.y = trigger.position.y - ROBOTS_EM07_PIRANHA_HEIGHT_OFFSET;
                }
                if let Some(physics) = self.native_monster_physics.get_mut(&key) {
                    physics.apply_handler_606_mode(false);
                    physics.overwrite_velocity([0.0; 3]);
                    physics.gravity_velocity_y = 0.0;
                }
                if let Some(runtime) = self.native_em07_piranha_runtime.get_mut(&key) {
                    runtime.cycle.mark_flight_completed();
                }
            }

            if let Some(runtime) = self.native_em07_piranha_runtime.get_mut(&key) {
                runtime.attachment_spin_radians += em07_piranha_attachment_rotation_per_fixed_tick();
            }

            let Some(player_position) = self.native_ai_gameplay_target_position(player_position) else {
                continue;
            };
            let owner_position = self
                .runtime_character_bodies
                .get(&key)
                .map(|body| body.owner_position)
                .unwrap_or(trigger.position);
            let owner_yaw_radians = self
                .runtime_character_bodies
                .get(&key)
                .map(|body| horizontal_yaw(body.owner_rotation))
                .unwrap_or_default();
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
                self.native_player_focus_runtime.player_state,
                true,
                self.native_player_hit_runtime.reaction_window,
                self.native_monster_attack_cooldown,
            );
            let attack_priority = {
                let runtime = self.native_em07_piranha_runtime.get(&key).unwrap();
                generic_attack_priority(
                    runtime.attack,
                    attack_config,
                    RobotsGenericAttackGateInput {
                        owner_position_xyz: owner_position.to_array(),
                        owner_yaw_radians,
                        target_position_xyz: player_position.to_array(),
                        target_visible,
                        class_attack_allowed,
                    },
                )
            };
            let winner = em07_piranha_behavior_winner(
                RobotsBaseMonsterIdleRuntimeState::priority(),
                attack_priority,
            );
            let previous = self
                .native_em07_piranha_runtime
                .get(&key)
                .and_then(|runtime| runtime.active_node);

            if previous != winner {
                if let Some(previous) = previous {
                    let runtime = self.native_em07_piranha_runtime.get_mut(&key).unwrap();
                    match previous {
                        RobotsEm07PiranhaBehaviorWinner::CommonIdle => runtime.idle.leave(),
                        RobotsEm07PiranhaBehaviorWinner::Attack => {
                            leave_generic_attack(&mut runtime.attack);
                            runtime.hit_query = None;
                        }
                    }
                }
                if let Some(next) = winner {
                    let runtime = self.native_em07_piranha_runtime.get_mut(&key).unwrap();
                    match next {
                        RobotsEm07PiranhaBehaviorWinner::CommonIdle => runtime.idle.enter(),
                        RobotsEm07PiranhaBehaviorWinner::Attack => {
                            enter_generic_attack(&mut runtime.attack, attack_config);
                            self.native_monster_attack_cooldown.record_attack();
                        }
                    }
                }
                self.native_em07_piranha_runtime.get_mut(&key).unwrap().active_node = winner;
            }

            match winner {
                Some(RobotsEm07PiranhaBehaviorWinner::CommonIdle) => {
                    let events = {
                        let runtime = self.native_em07_piranha_runtime.get_mut(&key).unwrap();
                        if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                            runtime.advance_animation(body, ROBOTS_BASE_MONSTER_IDLE_ANIM_MODE, NativeAiRootMotionPolicy::NONE)
                        } else {
                            Vec::new()
                        }
                    };
                    if events.iter().any(|event| event.event_type == event_type::SETUP_IDLE) {
                        self.native_em07_piranha_runtime
                            .get_mut(&key)
                            .unwrap()
                            .animation
                            .setup_idle();
                    }
                }
                Some(RobotsEm07PiranhaBehaviorWinner::Attack) => {
                    let step = {
                        let runtime = self.native_em07_piranha_runtime.get_mut(&key).unwrap();
                        step_generic_attack(&mut runtime.attack, attack_config)
                    };
                    if let Some(requested_anim_mode) = step.requested_anim_mode {
                        let events = {
                            let runtime = self.native_em07_piranha_runtime.get_mut(&key).unwrap();
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                runtime.advance_animation(body, requested_anim_mode, NativeAiRootMotionPolicy::TRANSLATION_ONLY)
                            } else {
                                Vec::new()
                            }
                        };
                        for event in events {
                            match event.event_type {
                                event_type::SETUP_IDLE => {
                                    let runtime =
                                        self.native_em07_piranha_runtime.get_mut(&key).unwrap();
                                    generic_attack_setup_idle(&mut runtime.attack, attack_config);
                                    runtime.animation.setup_idle();
                                }
                                event_type::HIT_CHECK => {
                                    if let Some(plan) = RobotsHitQueryInitPlan::from_event(event.as_view()) {
                                        let serial = self.allocate_native_hit_query_serial();
                                        self.native_em07_piranha_runtime.get_mut(&key).unwrap().hit_query =
                                            Some(NativePiranhaHitQueryRuntime {
                                                query: plan.instantiate(true, false, serial),
                                                source_anim_mode: requested_anim_mode,
                                            });
                                    }
                                }
                                event_type::CREATE_PROJECTILE => {
                                    if let Some(request) = RobotsCreateProjectileRequest::from_event(event.as_view()) {
                                        let serial = self.allocate_native_hit_query_serial();
                                        if let Some(plan) = resolve_ai_projectile_spawn_plan(
                                            map,
                                            key,
                                            event.owner_position,
                                            event.owner_rotation,
                                            owner_scale,
                                            visual,
                                            requested_anim_mode,
                                            request,
                                            serial,
                                            event.pose_seconds,
                                            player_position,
                                        ) {
                                            self.spawn_native_ai_projectile(plan);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    tick_generic_attack(
                        &mut self.native_em07_piranha_runtime.get_mut(&key).unwrap().attack,
                    );
                }
                None => {}
            }

            // 0x00467A8F..0x00467B37 queries datum 0x1000000F and falls back to owner Y.
            let datum_height = self.runtime_character_bodies.get(&key).and_then(|body| {
                let runtime = self.native_em07_piranha_runtime.get(&key)?;
                let anim_mode = runtime.animation.current_anim_mode();
                let pose_seconds = runtime.animation.sampled_pose_seconds();
                runtime_character_animation_datum_world_transform(
                    body.owner_position,
                    body.owner_rotation,
                    body.native_transform_scale_xyz(),
                    visual,
                    anim_mode,
                    0x1000_000F,
                    pose_seconds,
                )
                .map(|datum| datum.position.y)
            });
            let relative_height = datum_height
                .or_else(|| self.runtime_character_bodies.get(&key).map(|body| body.owner_position.y))
                .map(|height| height - trigger.position.y)
                .unwrap_or_default();
            let splash_selector = trigger
                .data
                .get(4)
                .and_then(|value| *value)
                .unwrap_or_default();
            let transition = self
                .native_em07_piranha_runtime
                .get(&key)
                .and_then(|runtime| em07_piranha_height_transition(runtime.above_creator_latch, relative_height));
            if let Some(transition) = transition {
                let plan = em07_piranha_height_transition_plan(transition, splash_selector);
                self.native_ai_permanent_sounds.remove(&plan.stop_loop_sound_uid);
                self.native_ai_transient_sounds.push(NativeAiTransientSoundRequest {
                    owner_key: key,
                    sound_uid: plan.one_shot_sound_uid,
                });
                if let Some(script_uid) = plan.script_uid {
                    let position_xyzw = self
                        .native_em07_piranha_runtime
                        .get(&key)
                        .map(|runtime| runtime.transition_position_xyzw)
                        .unwrap_or([trigger.position.x, trigger.position.y, trigger.position.z, 0.0]);
                    self.native_ai_script_spawns.push(NativeAiScriptSpawnRequest {
                        owner_key: key,
                        file_uid: ROBOTS_EM07_PIRANHA_FILE_UID,
                        script_uid,
                        position_xyzw,
                    });
                }
                register_native_ai_permanent_sound(
                    &mut self.native_ai_permanent_sounds,
                    key,
                    RobotsPermanentSoundConfig {
                        sound_uid: plan.start_loop_sound_uid,
                        native_parameter: ROBOTS_EM07_PIRANHA_PERMANENT_SOUND_PARAMETER,
                    },
                );
                if let Some(runtime) = self.native_em07_piranha_runtime.get_mut(&key) {
                    runtime.above_creator_latch =
                        transition == RobotsEm07PiranhaHeightTransition::EnteredAbove;
                    runtime.last_height_transition = Some(transition);
                    runtime.last_height_transition_plan = Some(plan);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_points_use_only_native_link_slots_4_through_7_and_distance_triggers() {
        assert_eq!(ROBOTS_EM07_PIRANHA_LINK_FIRST_INDEX, 4);
        assert_eq!(ROBOTS_EM07_PIRANHA_LINK_END_INDEX, 8);
        assert_eq!(em07_piranha_distance_radius_from_raw(30).to_bits(), 3.0f32.to_bits());
    }

    #[test]
    fn runtime_keeps_native_permanent_sound_and_ballistic_constants() {
        assert_eq!(ROBOTS_EM07_PIRANHA_PERMANENT_SOUND_UID, 0x1AF0_0388);
        assert_eq!(ROBOTS_EM07_PIRANHA_PERMANENT_SOUND_PARAMETER, 100);
        assert_eq!(
            ROBOTS_EM07_PIRANHA_FIRST_UPDATE_COMPONENT_SCALAR.to_bits(),
            12.74f32.to_bits()
        );
    }
}
