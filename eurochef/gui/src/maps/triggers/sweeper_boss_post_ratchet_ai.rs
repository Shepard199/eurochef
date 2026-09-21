use std::collections::BTreeMap;

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeSweeperBossMalfBotPostRatchetRuntime {
    pub random_behavior_countdown: u32,
}

impl NativeSweeperBossMalfBotPostRatchetRuntime {
    pub(crate) fn from_setup_random_mod90(random_mod90: u8) -> Option<Self> {
        (random_mod90 < 90).then_some(Self {
            // 0x004591C0: 180 + draw%90 - 45. The first state0 update happens later,
            // after Ratchet, so keep the constructor value here and decrement in step().
            random_behavior_countdown: 135 + u32::from(random_mod90),
        })
    }

    pub(crate) fn step(&mut self) -> NativeSweeperBossMalfBotPostRatchetStep {
        let countdown_before = self.random_behavior_countdown;
        // The inactive random-choice node saturates at zero. Reaching zero by itself does not
        // consume process-global RNG; the selector cannot promote its priority-1 idle node.
        self.random_behavior_countdown = self.random_behavior_countdown.saturating_sub(1);
        NativeSweeperBossMalfBotPostRatchetStep {
            countdown_before,
            countdown_after: self.random_behavior_countdown,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeSweeperBossMalfBotPostRatchetStep {
    pub countdown_before: u32,
    pub countdown_after: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossRollerBotPostRatchetRuntime {
    pub setup_window: u8,
    pub updates_completed: u32,
    pub first_behavior_priority: Option<u8>,
    pub current_behavior_priority: Option<u8>,
    pub first_update_random_bit: Option<u8>,
    pub last_fallback_enter_random_bit: Option<u8>,
    pub path_runtime: Option<NativeSweeperRollerbotPathRuntime>,
}

impl NativeSweeperBossRollerBotPostRatchetRuntime {
    pub(crate) fn from_setup_random_mod3(setup_random_mod3: u8) -> Option<Self> {
        (setup_random_mod3 < 3).then_some(Self {
            setup_window: 2 + setup_random_mod3,
            updates_completed: 0,
            first_behavior_priority: None,
            current_behavior_priority: None,
            first_update_random_bit: None,
            last_fallback_enter_random_bit: None,
            path_runtime: None,
        })
    }

    fn owner_position(&self, live: &NativeSweeperBossLiveAiCharacter) -> [f32; 3] {
        self.path_runtime
            .map(|runtime| runtime.owner_position)
            .unwrap_or([live.position[0], live.position[1], live.position[2]])
    }

    fn fallback_selected(
        &self,
        live: &NativeSweeperBossLiveAiCharacter,
        player_position: [f32; 4],
    ) -> bool {
        let owner = self.owner_position(live);
        let dx = owner[0] - player_position[0];
        let dy = owner[1] - player_position[1];
        let dz = owner[2] - player_position[2];
        let distance_squared = dx * dx + dy * dy + dz * dz;
        // Native <= radius² comparison is unordered for NaN, so NaN selects fallback.
        !(distance_squared <= ROLLERBOT_PROXIMITY_RADIUS * ROLLERBOT_PROXIMITY_RADIUS)
    }

    pub(crate) fn step_owned_rng(
        &mut self,
        live: &NativeSweeperBossLiveAiCharacter,
        player_position: [f32; 4],
        path_graph: Option<&NativeSweeperRollerbotPathGraph>,
        rng: &mut crate::map_runtime::RuntimeRobotsGlobalRngState,
    ) -> Option<NativeSweeperBossRollerBotPostRatchetStep> {
        let mut next = *self;
        let mut next_rng = *rng;
        let fallback_selected = next.fallback_selected(live, player_position);
        let first_update = next.first_behavior_priority.is_none();
        let fallback_enter =
            fallback_selected && (first_update || next.current_behavior_priority == Some(30));
        let fallback_enter_random_bit = if fallback_enter {
            Some((next_rng.next_u32()? & 1) as u8)
        } else {
            None
        };

        let selected_priority = if fallback_selected { 15 } else { 30 };
        if first_update {
            next.first_behavior_priority = Some(selected_priority);
            next.first_update_random_bit = fallback_enter_random_bit;
        }
        next.current_behavior_priority = Some(selected_priority);
        next.last_fallback_enter_random_bit = fallback_enter_random_bit;

        let path = if fallback_selected {
            match (next.path_runtime.as_mut(), path_graph) {
                (Some(runtime), Some(graph)) => {
                    let preview = runtime.step(&graph.node_positions, &graph.links, None)?;
                    if preview.needs_neighbor_rng {
                        let draw = next_rng.next_u32()?;
                        Some(runtime.step(&graph.node_positions, &graph.links, Some(draw))?)
                    } else {
                        Some(preview)
                    }
                }
                (Some(_), None) => return None,
                (None, _) => None,
            }
        } else {
            None
        };

        next.updates_completed = next.updates_completed.wrapping_add(1);
        *self = next;
        *rng = next_rng;
        Some(NativeSweeperBossRollerBotPostRatchetStep {
            fallback_selected,
            selected_priority,
            fallback_enter_random_bit,
            path,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NativeSweeperBossRollerBotPostRatchetStep {
    pub fallback_selected: bool,
    pub selected_priority: u8,
    pub fallback_enter_random_bit: Option<u8>,
    pub path: Option<NativeSweeperRollerbotPathStep>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum NativeSweeperBossPostRatchetAiCharacterRuntime {
    MalfBot(NativeSweeperBossMalfBotPostRatchetRuntime),
    RollerBot(NativeSweeperBossRollerBotPostRatchetRuntime),
    /// EW10/EB14 need only their constructor-time PeriodicIdle seed here.
    /// Production behavior belongs to the common standard-monster host.
    StandardMonsterBootstrap {
        periodic_idle_offset: u8,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum NativeSweeperBossPostRatchetAiCharacterStep {
    MalfBot {
        live_id: u32,
        step: NativeSweeperBossMalfBotPostRatchetStep,
    },
    RollerBot {
        live_id: u32,
        step: NativeSweeperBossRollerBotPostRatchetStep,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NativeSweeperBossPostRatchetAiRuntime {
    pub entries: BTreeMap<u32, NativeSweeperBossPostRatchetAiCharacterRuntime>,
    /// Normal GUI/UE-oriented host ownership switch. Legacy stage replay leaves
    /// this false and preserves its historical deterministic oracle. Production
    /// MapFrame sets it true after map binding: factory RNG/bootstrap remain here,
    /// but behavior execution moves to the common class host exactly once.
    pub generic_handoff_enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeSweeperBossGenericAiBootstrap {
    MalfBot { random_mod90: u8 },
    RollerBot { setup_random_mod3: u8 },
    StandardMonster { periodic_idle_offset: u8 },
}

impl NativeSweeperBossPostRatchetAiRuntime {
    pub(crate) fn enable_generic_handoff(&mut self) {
        self.generic_handoff_enabled = true;
    }

    pub(crate) fn generic_bootstrap(
        &self,
        live_id: u32,
    ) -> Option<NativeSweeperBossGenericAiBootstrap> {
        match *self.entries.get(&live_id)? {
            NativeSweeperBossPostRatchetAiCharacterRuntime::MalfBot(malf) => {
                let random_mod90 = malf.random_behavior_countdown.checked_sub(135)?;
                (random_mod90 < 90).then_some(NativeSweeperBossGenericAiBootstrap::MalfBot {
                    random_mod90: random_mod90 as u8,
                })
            }
            NativeSweeperBossPostRatchetAiCharacterRuntime::RollerBot(roller) => {
                let setup_random_mod3 = roller.setup_window.checked_sub(2)?;
                (setup_random_mod3 < 3)
                    .then_some(NativeSweeperBossGenericAiBootstrap::RollerBot { setup_random_mod3 })
            }
            NativeSweeperBossPostRatchetAiCharacterRuntime::StandardMonsterBootstrap {
                periodic_idle_offset,
            } => (periodic_idle_offset < 90).then_some(
                NativeSweeperBossGenericAiBootstrap::StandardMonster {
                    periodic_idle_offset,
                },
            ),
        }
    }

    pub(crate) fn register_factory_steps(
        &mut self,
        live_ai: &NativeSweeperBossLiveAiRegistry,
        factory_steps: &[NativeSweeperBossSpawnFactoryRngStep],
    ) -> Option<u32> {
        let untracked = live_ai
            .entries
            .values()
            .filter(|live| {
                matches!(
                    live.source,
                    NativeSweeperBossLiveAiSource::EyeSpawn
                        | NativeSweeperBossLiveAiSource::TransporterCarry { .. }
                ) && !self.entries.contains_key(&live.id)
                    && matches!(
                        live.file,
                        Some(
                            SWEEPER_MALFBOT_FILE
                                | SWEEPER_ROLLERBOT_FILE
                                | SWEEPER_EW10_MINION_FILE
                                | SWEEPER_EB14_MINION_FILE
                                | SWEEPER_SHUNTBOT_BOSS_FILE
                        )
                    )
            })
            .collect::<Vec<_>>();
        if untracked.len() != factory_steps.len() {
            return None;
        }

        let mut next = self.clone();
        for (live, factory) in untracked.into_iter().zip(factory_steps.iter().copied()) {
            let origin = match factory {
                NativeSweeperBossSpawnFactoryRngStep::MalfBot { origin, .. }
                | NativeSweeperBossSpawnFactoryRngStep::RollerBot { origin, .. }
                | NativeSweeperBossSpawnFactoryRngStep::StandardMonster { origin, .. } => origin,
            };
            let source_matches_origin = match (origin, live.source) {
                (
                    NativeSweeperBossSpawnOrigin::Eye { .. },
                    NativeSweeperBossLiveAiSource::EyeSpawn,
                ) => true,
                (
                    NativeSweeperBossSpawnOrigin::Transporter {
                        target_trigger_index,
                        ..
                    },
                    NativeSweeperBossLiveAiSource::TransporterCarry { trigger_index },
                ) => target_trigger_index == trigger_index,
                _ => false,
            };
            if !source_matches_origin {
                return None;
            }
            let runtime = match factory {
                NativeSweeperBossSpawnFactoryRngStep::MalfBot {
                    config_index,
                    random_mod90,
                    ..
                } if live.config_index == config_index
                    && live.file == Some(SWEEPER_MALFBOT_FILE) =>
                {
                    NativeSweeperBossPostRatchetAiCharacterRuntime::MalfBot(
                        NativeSweeperBossMalfBotPostRatchetRuntime::from_setup_random_mod90(
                            random_mod90,
                        )?,
                    )
                }
                NativeSweeperBossSpawnFactoryRngStep::RollerBot {
                    config_index,
                    setup_random_mod3,
                    config_random_mod1,
                    ..
                } if live.config_index == config_index
                    && live.file == Some(SWEEPER_ROLLERBOT_FILE)
                    && config_random_mod1 == 0 =>
                {
                    NativeSweeperBossPostRatchetAiCharacterRuntime::RollerBot(
                        NativeSweeperBossRollerBotPostRatchetRuntime::from_setup_random_mod3(
                            setup_random_mod3,
                        )?,
                    )
                }
                NativeSweeperBossSpawnFactoryRngStep::StandardMonster {
                    config_index,
                    periodic_idle_offset,
                    ..
                } if live.config_index == config_index
                    && match live.file {
                        Some(SWEEPER_EW10_MINION_FILE | SWEEPER_EB14_MINION_FILE) => {
                            periodic_idle_offset < 90
                        }
                        Some(SWEEPER_SHUNTBOT_BOSS_FILE) => periodic_idle_offset < 60,
                        _ => false,
                    } =>
                {
                    NativeSweeperBossPostRatchetAiCharacterRuntime::StandardMonsterBootstrap {
                        periodic_idle_offset,
                    }
                }
                _ => return None,
            };
            next.entries.insert(live.id, runtime);
        }
        let registered = factory_steps.len().min(u32::MAX as usize) as u32;
        *self = next;
        Some(registered)
    }

    pub(crate) fn step_owned_rng(
        &mut self,
        live_ai: &NativeSweeperBossLiveAiRegistry,
        player_position: [f32; 4],
        path_graph: Option<&NativeSweeperRollerbotPathGraph>,
        rng: &mut crate::map_runtime::RuntimeRobotsGlobalRngState,
    ) -> Option<Vec<NativeSweeperBossPostRatchetAiCharacterStep>> {
        let mut next = self.clone();
        let mut next_rng = *rng;
        next.entries
            .retain(|id, _| live_ai.entries.contains_key(id));
        if next.generic_handoff_enabled {
            // Factory registration above remains native-order and owns the already
            // consumed setup draws. Common MapFrame AI now owns all later behavior,
            // movement and RNG for these XItems, so the superseded 15/30 reducer
            // must not execute or spend another process-global draw here.
            *self = next;
            return Some(Vec::new());
        }

        let mut steps = Vec::new();
        for (&live_id, runtime) in next.entries.iter_mut() {
            let live = live_ai.entries.get(&live_id)?;
            if live.pending_destroy {
                continue;
            }
            match runtime {
                NativeSweeperBossPostRatchetAiCharacterRuntime::MalfBot(malf) => {
                    steps.push(NativeSweeperBossPostRatchetAiCharacterStep::MalfBot {
                        live_id,
                        step: malf.step(),
                    });
                }
                NativeSweeperBossPostRatchetAiCharacterRuntime::RollerBot(roller) => {
                    let step =
                        roller.step_owned_rng(live, player_position, path_graph, &mut next_rng)?;
                    steps.push(NativeSweeperBossPostRatchetAiCharacterStep::RollerBot {
                        live_id,
                        step,
                    });
                }
                NativeSweeperBossPostRatchetAiCharacterRuntime::StandardMonsterBootstrap {
                    ..
                } => {
                    // No class-local behavior lives here. Production always hands
                    // EW10/EB14 to the common standard-monster host.
                }
            }
        }

        *self = next;
        *rng = next_rng;
        Some(steps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live_eye_ai(
        config_index: u8,
        file: u32,
        eye_x: f32,
        eye_w: f32,
    ) -> (NativeSweeperBossLiveAiRegistry, u32) {
        let mut patterns = RobotsSweeperBossPatterns::default();
        patterns.set_monster_file(config_index, file);
        let mut live_ai = NativeSweeperBossLiveAiRegistry::default();
        let id = live_ai.register_spawn(
            &patterns,
            config_index,
            sweeper_boss_spawn_transform(eye_x, eye_w, 0.0, 0),
        );
        (live_ai, id)
    }

    #[test]
    fn post_ratchet_malf_countdown_is_stateful_and_rng_quiet_after_factory_setup() {
        let (live_ai, live_id) = live_eye_ai(7, SWEEPER_MALFBOT_FILE, 0.0, 0.0);
        let mut runtime = NativeSweeperBossPostRatchetAiRuntime::default();
        assert_eq!(
            runtime.register_factory_steps(
                &live_ai,
                &[NativeSweeperBossSpawnFactoryRngStep::MalfBot {
                    origin: NativeSweeperBossSpawnOrigin::Eye {
                        eye_ordinal: 1,
                        spawn_ordinal: 0,
                    },
                    config_index: 7,
                    random_mod90: 87,
                }],
            ),
            Some(1)
        );
        assert_eq!(
            runtime.entries.get(&live_id),
            Some(&NativeSweeperBossPostRatchetAiCharacterRuntime::MalfBot(
                NativeSweeperBossMalfBotPostRatchetRuntime {
                    random_behavior_countdown: 222,
                }
            ))
        );

        let mut rng =
            crate::map_runtime::RuntimeRobotsGlobalRngState::from_observed_seed(0x1234_5678);
        let rng_before = rng;
        let step = runtime
            .step_owned_rng(&live_ai, [0.0; 4], None, &mut rng)
            .expect("Malf post-Ratchet update");
        assert_eq!(rng, rng_before);
        assert_eq!(
            step,
            vec![NativeSweeperBossPostRatchetAiCharacterStep::MalfBot {
                live_id,
                step: NativeSweeperBossMalfBotPostRatchetStep {
                    countdown_before: 222,
                    countdown_after: 221,
                },
            }]
        );
    }

    #[test]
    fn generic_handoff_keeps_factory_bootstrap_but_skips_legacy_ai_rng_and_step() {
        let (malf_live, malf_id) = live_eye_ai(7, SWEEPER_MALFBOT_FILE, 0.0, 0.0);
        let mut malf_runtime = NativeSweeperBossPostRatchetAiRuntime::default();
        malf_runtime.enable_generic_handoff();
        assert_eq!(
            malf_runtime.register_factory_steps(
                &malf_live,
                &[NativeSweeperBossSpawnFactoryRngStep::MalfBot {
                    origin: NativeSweeperBossSpawnOrigin::Eye {
                        eye_ordinal: 1,
                        spawn_ordinal: 0,
                    },
                    config_index: 7,
                    random_mod90: 87,
                }],
            ),
            Some(1)
        );
        assert_eq!(
            malf_runtime.generic_bootstrap(malf_id),
            Some(NativeSweeperBossGenericAiBootstrap::MalfBot { random_mod90: 87 })
        );
        let mut malf_rng =
            crate::map_runtime::RuntimeRobotsGlobalRngState::from_observed_seed(0x1234_5678);
        let malf_rng_before = malf_rng;
        assert_eq!(
            malf_runtime.step_owned_rng(&malf_live, [0.0; 4], None, &mut malf_rng),
            Some(Vec::new())
        );
        assert_eq!(malf_rng, malf_rng_before);
        assert_eq!(
            malf_runtime.generic_bootstrap(malf_id),
            Some(NativeSweeperBossGenericAiBootstrap::MalfBot { random_mod90: 87 })
        );

        let (roller_live, roller_id) = live_eye_ai(9, SWEEPER_ROLLERBOT_FILE, 0.0, 0.0);
        let mut roller_runtime = NativeSweeperBossPostRatchetAiRuntime::default();
        roller_runtime.enable_generic_handoff();
        assert_eq!(
            roller_runtime.register_factory_steps(
                &roller_live,
                &[NativeSweeperBossSpawnFactoryRngStep::RollerBot {
                    origin: NativeSweeperBossSpawnOrigin::Eye {
                        eye_ordinal: 2,
                        spawn_ordinal: 0,
                    },
                    config_index: 9,
                    setup_random_mod3: 2,
                    config_random_mod1: 0,
                }],
            ),
            Some(1)
        );
        assert_eq!(
            roller_runtime.generic_bootstrap(roller_id),
            Some(NativeSweeperBossGenericAiBootstrap::RollerBot {
                setup_random_mod3: 2,
            })
        );
        let mut roller_rng =
            crate::map_runtime::RuntimeRobotsGlobalRngState::from_observed_seed(0x8765_4321);
        let roller_rng_before = roller_rng;
        assert_eq!(
            roller_runtime.step_owned_rng(
                &roller_live,
                [100.0, 0.0, 100.0, 1.0],
                None,
                &mut roller_rng
            ),
            Some(Vec::new())
        );
        assert_eq!(roller_rng, roller_rng_before);
    }

    #[test]
    fn standard_monster_factory_periodic_idle_rng_is_pre_ratchet_and_generic_handoff_is_rng_quiet()
    {
        for (config_index, file, modulus) in [
            (13, SWEEPER_EW10_MINION_FILE, 90),
            (14, SWEEPER_EB14_MINION_FILE, 90),
            (20, SWEEPER_SHUNTBOT_BOSS_FILE, 60),
        ] {
            let origin = NativeSweeperBossSpawnOrigin::Eye {
                eye_ordinal: 3,
                spawn_ordinal: 0,
            };
            let mut patterns = RobotsSweeperBossPatterns::default();
            patterns.set_monster_file(config_index, file);
            let mut rng =
                crate::map_runtime::RuntimeRobotsGlobalRngState::from_observed_seed(0x2468_ace1);
            let mut expected_rng = rng;
            let expected_offset = (expected_rng
                .next_u32()
                .expect("PeriodicIdle constructor draw")
                % modulus) as u8;
            let factory =
                sweeper_boss_consume_spawn_factory_rng(origin, config_index, &patterns, &mut rng)
                    .expect("EW10/EB14 factory RNG");
            assert_eq!(rng, expected_rng);
            assert_eq!(
                factory,
                NativeSweeperBossSpawnFactoryRngStep::StandardMonster {
                    origin,
                    config_index,
                    periodic_idle_offset: expected_offset,
                }
            );
            assert_eq!(factory.random_draws_used(), 1);

            let (live_ai, live_id) = live_eye_ai(config_index, file, 0.0, 0.0);
            let mut runtime = NativeSweeperBossPostRatchetAiRuntime::default();
            runtime.enable_generic_handoff();
            assert_eq!(
                runtime.register_factory_steps(&live_ai, &[factory]),
                Some(1)
            );
            assert_eq!(
                runtime.generic_bootstrap(live_id),
                Some(NativeSweeperBossGenericAiBootstrap::StandardMonster {
                    periodic_idle_offset: expected_offset,
                })
            );

            let rng_before_handoff = rng;
            assert_eq!(
                runtime.step_owned_rng(&live_ai, [0.0; 4], None, &mut rng),
                Some(Vec::new())
            );
            assert_eq!(rng, rng_before_handoff);
        }
    }

    #[test]
    fn post_ratchet_roller_rng_is_owned_only_on_fallback_entry() {
        let (live_ai, live_id) = live_eye_ai(9, SWEEPER_ROLLERBOT_FILE, 0.0, 0.0);
        let mut runtime = NativeSweeperBossPostRatchetAiRuntime::default();
        assert_eq!(
            runtime.register_factory_steps(
                &live_ai,
                &[NativeSweeperBossSpawnFactoryRngStep::RollerBot {
                    origin: NativeSweeperBossSpawnOrigin::Eye {
                        eye_ordinal: 2,
                        spawn_ordinal: 0,
                    },
                    config_index: 9,
                    setup_random_mod3: 2,
                    config_random_mod1: 0,
                }],
            ),
            Some(1)
        );

        let far_player = [100.0, 0.0, 100.0, 1.0];
        let near_player = [0.0, 2.4, 35.0, 1.0];
        let mut expected_rng =
            crate::map_runtime::RuntimeRobotsGlobalRngState::from_observed_seed(0x8765_4321);
        let first_bit = (expected_rng.next_u32().expect("first fallback draw") & 1) as u8;
        let second_bit = (expected_rng.next_u32().expect("second fallback draw") & 1) as u8;
        let expected_after_second_entry = expected_rng;

        let mut rng =
            crate::map_runtime::RuntimeRobotsGlobalRngState::from_observed_seed(0x8765_4321);
        let first = runtime
            .step_owned_rng(&live_ai, far_player, None, &mut rng)
            .expect("fresh Roller fallback update");
        assert_eq!(
            first,
            vec![NativeSweeperBossPostRatchetAiCharacterStep::RollerBot {
                live_id,
                step: NativeSweeperBossRollerBotPostRatchetStep {
                    fallback_selected: true,
                    selected_priority: 15,
                    fallback_enter_random_bit: Some(first_bit),
                    path: None,
                },
            }]
        );
        let rng_after_first_entry = rng;

        let retained = runtime
            .step_owned_rng(&live_ai, far_player, None, &mut rng)
            .expect("retained fallback update");
        assert_eq!(rng, rng_after_first_entry);
        assert!(matches!(
            retained.as_slice(),
            [NativeSweeperBossPostRatchetAiCharacterStep::RollerBot {
                step: NativeSweeperBossRollerBotPostRatchetStep {
                    fallback_selected: true,
                    selected_priority: 15,
                    fallback_enter_random_bit: None,
                    ..
                },
                ..
            }]
        ));

        let proximity = runtime
            .step_owned_rng(&live_ai, near_player, None, &mut rng)
            .expect("proximity update");
        assert_eq!(rng, rng_after_first_entry);
        assert!(matches!(
            proximity.as_slice(),
            [NativeSweeperBossPostRatchetAiCharacterStep::RollerBot {
                step: NativeSweeperBossRollerBotPostRatchetStep {
                    fallback_selected: false,
                    selected_priority: 30,
                    fallback_enter_random_bit: None,
                    ..
                },
                ..
            }]
        ));

        let fallback_again = runtime
            .step_owned_rng(&live_ai, far_player, None, &mut rng)
            .expect("second fallback entry");
        assert_eq!(rng, expected_after_second_entry);
        assert!(matches!(
            fallback_again.as_slice(),
            [NativeSweeperBossPostRatchetAiCharacterStep::RollerBot {
                step: NativeSweeperBossRollerBotPostRatchetStep {
                    fallback_selected: true,
                    selected_priority: 15,
                    fallback_enter_random_bit: Some(bit),
                    ..
                },
                ..
            }] if *bit == second_bit
        ));
    }
}
