use super::*;

pub(crate) struct NativeSpinTopRuntime {
    patrol: RobotsAiPatrolRuntimeState,
    bounce: RobotsBounceNavMeshRuntimeState,
    locomotion: RobotsAiLocomotionRuntimeState,
    scrambled_hit: RobotsMalfBotScrambledHitRuntimeState,
    electro_hit: RobotsMalfBotElectroHitRuntimeState,
    magnetic_hit: RobotsMalfBotMagneticHitRuntimeState,
    magnetic_drop_charge_count: Option<u8>,
    active_node: Option<RobotsSpinTopBehaviorWinner>,
    animation: NativeAiAnimationRuntime,
}

impl NativeSpinTopRuntime {
    fn new(setup_draw: u32) -> Self {
        Self {
            patrol: RobotsAiPatrolRuntimeState::configured(spintop_patrol_config()),
            bounce: RobotsBounceNavMeshRuntimeState::from_setup_draw(setup_draw),
            locomotion: RobotsAiLocomotionRuntimeState::default(),
            scrambled_hit: RobotsMalfBotScrambledHitRuntimeState::default(),
            electro_hit: RobotsMalfBotElectroHitRuntimeState::default(),
            magnetic_hit: RobotsMalfBotMagneticHitRuntimeState::default(),
            magnetic_drop_charge_count: None,
            active_node: None,
            animation: NativeAiAnimationRuntime::default(),
        }
    }
}

impl MapFrame {
    pub(super) fn advance_native_spintop_fixed(
        &mut self,
        map: &ProcessedMap,
        nav_regions: &[RobotsMonsterNavMeshView<'_>],
    ) {
        for (trigger_index, trigger) in map.triggers.iter().enumerate() {
            let Some(visual) = trigger.character_visual.as_ref() else {
                continue;
            };
            if visual.handler_class != RobotsAiHandlerClass::SpinTop {
                continue;
            }

            let key = Self::runtime_character_body_key(map.hashcode, trigger_index);
            if !self.runtime_ai_character_xitem_live(map.hashcode, trigger_index) {
                self.native_spintop_runtime.remove(&key);
                self.native_monster_navigation.remove(&key);
                self.native_monster_physics.remove(&key);
                if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                    body.clear_collision_animation_track();
                }
                continue;
            }
            if !self.runtime_character_bodies.contains_key(&key) {
                self.native_spintop_runtime.remove(&key);
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

            if !self.native_spintop_runtime.contains_key(&key) {
                let Some(setup_draw) = self.next_native_ai_gameplay_rng_u32() else {
                    continue;
                };
                self.native_spintop_runtime
                    .insert(key, NativeSpinTopRuntime::new(setup_draw));
            }

            if self.native_ai_normal_fatal_condition(key) {
                continue;
            }

            let Some(body) = self.runtime_character_bodies.get(&key) else {
                continue;
            };
            let owner_position = body.owner_position;
            let owner_yaw_radians = horizontal_yaw(body.owner_rotation);

            let navigation_ready = self
                .native_monster_navigation
                .get(&key)
                .is_some_and(|state| state.region_ordinal.is_some() && state.face_index.is_some());
            let top_game_state = self
                .native_cutscene_host_runtime
                .game_state_stack
                .last()
                .copied();
            let hit_snapshot = self.native_ai_hit_reactions.get(&key).copied();

            let (winner, previous) = {
                let runtime = self.native_spintop_runtime.get(&key).unwrap();
                let (got_hit, flags) = hit_snapshot
                    .map(|hit| (hit.got_hit_latch, hit.query_flags_snapshot))
                    .unwrap_or((false, 0));
                (
                    spintop_behavior_winner(
                        ai_patrol_priority(top_game_state),
                        RobotsBounceNavMeshRuntimeState::priority(navigation_ready),
                        runtime.scrambled_hit.priority(got_hit, flags),
                        runtime.electro_hit.priority(got_hit, flags),
                        runtime.magnetic_hit.priority(got_hit, flags),
                    ),
                    runtime.active_node,
                )
            };

            if previous != winner {
                if let Some(previous) = previous {
                    let runtime = self.native_spintop_runtime.get_mut(&key).unwrap();
                    match previous {
                        RobotsSpinTopBehaviorWinner::BounceNavMesh => runtime.bounce.leave(),
                        RobotsSpinTopBehaviorWinner::ScrambledHit => runtime.scrambled_hit.leave(),
                        RobotsSpinTopBehaviorWinner::ElectroHit => runtime.electro_hit.leave(),
                        RobotsSpinTopBehaviorWinner::MagneticHit => runtime.magnetic_hit.leave(),
                        RobotsSpinTopBehaviorWinner::CommonIdle
                        | RobotsSpinTopBehaviorWinner::Patrol => {}
                    }
                }
                if matches!(
                    previous,
                    Some(
                        RobotsSpinTopBehaviorWinner::ElectroHit
                            | RobotsSpinTopBehaviorWinner::MagneticHit
                    )
                ) {
                    if let Some(hit) = self.native_ai_hit_reactions.get_mut(&key) {
                        hit.query_serial_snapshot = 0;
                        hit.last_query_serial = 0;
                    }
                }

                if let Some(next) = winner {
                    match next {
                        RobotsSpinTopBehaviorWinner::BounceNavMesh => self
                            .native_spintop_runtime
                            .get_mut(&key)
                            .unwrap()
                            .bounce
                            .enter(),
                        RobotsSpinTopBehaviorWinner::ScrambledHit => self
                            .native_spintop_runtime
                            .get_mut(&key)
                            .unwrap()
                            .scrambled_hit
                            .enter(),
                        RobotsSpinTopBehaviorWinner::ElectroHit => self
                            .native_spintop_runtime
                            .get_mut(&key)
                            .unwrap()
                            .electro_hit
                            .enter(),
                        RobotsSpinTopBehaviorWinner::MagneticHit => {
                            let runtime = self.native_spintop_runtime.get_mut(&key).unwrap();
                            runtime.magnetic_hit.enter(owner_position.y);
                            if runtime.magnetic_drop_charge_count.is_none() {
                                runtime.magnetic_drop_charge_count =
                                    Some(visual.pickup_drop_count.unwrap_or(0));
                            }
                        }
                        RobotsSpinTopBehaviorWinner::CommonIdle
                        | RobotsSpinTopBehaviorWinner::Patrol => {}
                    }
                }
                self.native_spintop_runtime
                    .get_mut(&key)
                    .unwrap()
                    .active_node = winner;
            }

            match winner {
                Some(RobotsSpinTopBehaviorWinner::CommonIdle) => {
                    let runtime = self.native_spintop_runtime.get_mut(&key).unwrap();
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
                Some(RobotsSpinTopBehaviorWinner::Patrol) => {
                    let locomotion = {
                        let runtime = self.native_spintop_runtime.get_mut(&key).unwrap();
                        let patrol = step_ai_patrol(&mut runtime.patrol, spintop_patrol_config());
                        let move_mode_active_on_entry =
                            runtime.animation.is_mode(ROBOTS_ANIM_MODE_MOVE);
                        step_ai_locomotion_after_steering_prepass(
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
                        ROBOTS_SPINTOP_MIN_MOVE_SPEED,
                        ROBOTS_SPINTOP_MAX_MOVE_SPEED,
                    );
                    let runtime = self.native_spintop_runtime.get_mut(&key).unwrap();
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
                Some(RobotsSpinTopBehaviorWinner::BounceNavMesh) => {
                    self.native_spintop_runtime
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
                                    ROBOTS_SPINTOP_BOUNCE_PROBE_RADIUS,
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
                                .native_spintop_runtime
                                .get_mut(&key)
                                .unwrap()
                                .bounce
                                .apply_boundary_bounce(current_yaw, correction_yaw);
                        }
                    }

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
                            .native_spintop_runtime
                            .get_mut(&key)
                            .unwrap()
                            .bounce
                            .observe_query_serial(
                                hit.query_serial_snapshot,
                                hit.health,
                                source_heading,
                            );
                        if request_impact {
                            self.native_spintop_runtime
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
                    let (target_yaw, impact_phase) = {
                        let runtime = self.native_spintop_runtime.get(&key).unwrap();
                        (
                            runtime.bounce.steering_target_yaw_radians,
                            runtime.bounce.phase == RobotsBounceNavMeshPhase::ImpactAnimation,
                        )
                    };
                    let current_yaw = self
                        .runtime_character_bodies
                        .get(&key)
                        .map(|body| horizontal_yaw(body.owner_rotation))
                        .unwrap_or(owner_yaw_radians);
                    let locomotion = {
                        let runtime = self.native_spintop_runtime.get_mut(&key).unwrap();
                        let move_mode_active_on_entry =
                            runtime.animation.is_mode(ROBOTS_ANIM_MODE_MOVE);
                        step_ai_locomotion_after_steering_prepass(
                            &mut runtime.locomotion,
                            RobotsAiLocomotionInput {
                                steering_target_yaw_radians: target_yaw,
                                target_locomotion_scalar:
                                    RobotsBounceNavMeshRuntimeState::target_locomotion_scalar(
                                        health,
                                    ),
                                turn_rate: RobotsBounceNavMeshRuntimeState::turn_rate(),
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
                        ROBOTS_SPINTOP_MIN_MOVE_SPEED,
                        ROBOTS_SPINTOP_MAX_MOVE_SPEED,
                    );
                    let requested_mode = if impact_phase {
                        ROBOTS_BOUNCE_NAVMESH_IMPACT_ANIM_MODE
                    } else {
                        locomotion.requested_anim_mode
                    };
                    let events = {
                        let runtime = self.native_spintop_runtime.get_mut(&key).unwrap();
                        if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                            runtime.animation.advance(
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
                        let runtimes = &mut self.native_spintop_runtime;
                        let hits = &mut self.native_ai_hit_reactions;
                        if let (Some(runtime), Some(hit)) =
                            (runtimes.get_mut(&key), hits.get_mut(&key))
                        {
                            runtime.bounce.setup_idle(&mut hit.got_hit_latch);
                            runtime.animation.setup_idle();
                        }
                    }
                }
                Some(RobotsSpinTopBehaviorWinner::ScrambledHit) => {
                    let requested_mode = {
                        let runtimes = &mut self.native_spintop_runtime;
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
                            let runtime = self.native_spintop_runtime.get_mut(&key).unwrap();
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                runtime.animation.advance(
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
                            let runtimes = &mut self.native_spintop_runtime;
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
                Some(RobotsSpinTopBehaviorWinner::ElectroHit) => {
                    let requested_mode = self
                        .native_spintop_runtime
                        .get(&key)
                        .unwrap()
                        .electro_hit
                        .requested_anim_mode();
                    let events = {
                        let runtime = self.native_spintop_runtime.get_mut(&key).unwrap();
                        if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                            runtime.animation.advance(
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
                        let plan = {
                            let runtime = self.native_spintop_runtime.get_mut(&key).unwrap();
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
                Some(RobotsSpinTopBehaviorWinner::MagneticHit) => {
                    let attachment_relation_matches =
                        self.native_player_magnetic_target_key == Some(key);
                    let owner_y = self
                        .runtime_character_bodies
                        .get(&key)
                        .map(|body| body.owner_position.y)
                        .unwrap_or(owner_position.y);
                    let plan = {
                        let runtimes = &mut self.native_spintop_runtime;
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
                        let runtime = self.native_spintop_runtime.get_mut(&key).unwrap();
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
                            let runtime = self.native_spintop_runtime.get_mut(&key).unwrap();
                            if let Some(body) = self.runtime_character_bodies.get_mut(&key) {
                                runtime.animation.advance(
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
                            self.native_spintop_runtime
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
                None => {}
            }

            // SpinTop +0x34 (0x0045FC20) runs common update then only clears
            // Handler+0x640 when an externally attached child reports flag 0x10.
            // The GUI runtime does not synthesize that absent child XItem.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spintop_runtime_setup_keeps_native_bounce_draw_and_patrol_config() {
        let runtime = NativeSpinTopRuntime::new(358);
        assert_eq!(
            runtime.bounce.steering_target_yaw_radians.to_bits(),
            (358.0f32
                * eurochef_shared::robots_runtime::bounce_navmesh::ROBOTS_BOUNCE_NAVMESH_ONE_DEGREE_RADIANS)
                .to_bits()
        );
        assert_eq!(runtime.patrol.countdown_ticks, 300);
        assert!(runtime.active_node.is_none());
    }
}
