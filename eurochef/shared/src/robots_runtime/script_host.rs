use serde::Serialize;

use super::{
    events::{
        resolve_recovered_handler_script_command, RobotsHandlerScriptCommandFamily,
        RobotsHandlerScriptCommandPlan, RobotsHandlerScriptCommandSemantic, RobotsScriptEventView,
    },
    handler_classes::robots_handler_script_command_family_for_class,
    inventory::{
        RobotsInventoryDefinition, RobotsInventoryMutationOutcome, RobotsInventoryState,
        ROBOTS_INVENTORY_UID_BASE,
    },
    mission::{
        is_robots_mission_uid, RobotsMissionDefinition, RobotsMissionState, RobotsMissionStatus,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct RobotsScriptGameplayState {
    pub inventory: RobotsInventoryState,
    pub missions: RobotsMissionState,
}

impl RobotsScriptGameplayState {
    pub fn clear(&mut self) {
        self.inventory.clear();
        self.missions.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsScriptCommandExecutionStatus {
    Applied,
    RecoveredNoMutation,
    NativeRejected,
    InvalidPayload,
    UnsupportedInventoryBackend,
    ExternalActionRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RobotsScriptCommandMutation {
    Inventory {
        item: u32,
        outcome: RobotsInventoryMutationOutcome,
    },
    MissionStatus {
        mission_uid: u32,
        previous: RobotsMissionStatus,
        current: RobotsMissionStatus,
    },
    ClearCurrentMissionOwner,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RobotsScriptCommandExecution {
    pub plan: RobotsHandlerScriptCommandPlan,
    pub status: RobotsScriptCommandExecutionStatus,
    pub mutations: Vec<RobotsScriptCommandMutation>,
}

/// Execute only the engine-neutral gameplay mutations that are already proven
/// for a recovered Handler +0x5C family. The returned `plan` remains the full
/// semantic contract: UE or another host can consume Cutscene/visual/audio
/// actions without coupling this shared state reducer to an engine.
///
/// `None` means the selected +0x5C family has not been recovered yet. This is
/// intentionally fail-closed; an unknown specialized family must never be
/// silently treated as Generic.
pub fn execute_recovered_handler_script_command_for_class(
    class_name: &str,
    event: RobotsScriptEventView<'_>,
    inventory_definitions: &[RobotsInventoryDefinition],
    mission_definitions: &[RobotsMissionDefinition],
    state: &mut RobotsScriptGameplayState,
    scrap_removal_locked: bool,
) -> Option<RobotsScriptCommandExecution> {
    let family = robots_handler_script_command_family_for_class(class_name)?;
    execute_recovered_handler_script_command(
        family,
        event,
        inventory_definitions,
        mission_definitions,
        state,
        scrap_removal_locked,
    )
}

pub fn execute_recovered_handler_script_command(
    family: RobotsHandlerScriptCommandFamily,
    event: RobotsScriptEventView<'_>,
    inventory_definitions: &[RobotsInventoryDefinition],
    mission_definitions: &[RobotsMissionDefinition],
    state: &mut RobotsScriptGameplayState,
    scrap_removal_locked: bool,
) -> Option<RobotsScriptCommandExecution> {
    let plan = resolve_recovered_handler_script_command(family, event)?;
    let mut execution = RobotsScriptCommandExecution {
        plan,
        status: RobotsScriptCommandExecutionStatus::RecoveredNoMutation,
        mutations: Vec::new(),
    };

    match &execution.plan.semantic {
        RobotsHandlerScriptCommandSemantic::InventoryAdd {
            owner_selector,
            item,
            quantity,
            fallback_item,
            mode,
        } => execute_inventory_add(
            *owner_selector,
            *item,
            *quantity,
            *fallback_item,
            *mode,
            inventory_definitions,
            state,
            &mut execution,
        ),
        RobotsHandlerScriptCommandSemantic::InventoryRemove {
            owner_selector,
            item,
            quantity,
        } => execute_inventory_remove(
            *owner_selector,
            *item,
            *quantity,
            inventory_definitions,
            state,
            scrap_removal_locked,
            &mut execution,
        ),
        RobotsHandlerScriptCommandSemantic::MissionUpdate {
            mission,
            mode,
            flags,
        } => execute_mission_update(
            *mission,
            *mode,
            *flags,
            mission_definitions,
            state,
            &mut execution,
        ),
        RobotsHandlerScriptCommandSemantic::CutsceneCommand { .. }
        | RobotsHandlerScriptCommandSemantic::DoorCommand { .. }
        | RobotsHandlerScriptCommandSemantic::SpecializedCommand { .. }
        | RobotsHandlerScriptCommandSemantic::ShowMessage { .. }
        | RobotsHandlerScriptCommandSemantic::ShakeCamera { .. }
        | RobotsHandlerScriptCommandSemantic::GeneratePickup { .. }
        | RobotsHandlerScriptCommandSemantic::AttachSwoosh { .. }
        | RobotsHandlerScriptCommandSemantic::AttachParticle { .. }
        | RobotsHandlerScriptCommandSemantic::AttachParticlesToSkeleton { .. }
        | RobotsHandlerScriptCommandSemantic::AttachSpecialEffect { .. }
        | RobotsHandlerScriptCommandSemantic::CreateExplosion { .. }
        | RobotsHandlerScriptCommandSemantic::DettachSwoosh { .. }
        | RobotsHandlerScriptCommandSemantic::FileLoad { .. }
        | RobotsHandlerScriptCommandSemantic::FileDeload { .. }
        | RobotsHandlerScriptCommandSemantic::FileLoadSubfile { .. }
        | RobotsHandlerScriptCommandSemantic::FileDeloadSubfile { .. }
        | RobotsHandlerScriptCommandSemantic::MessageRelay
        | RobotsHandlerScriptCommandSemantic::MessageRelaySpecific { .. }
        | RobotsHandlerScriptCommandSemantic::ExitScript
        | RobotsHandlerScriptCommandSemantic::Wait
        | RobotsHandlerScriptCommandSemantic::WaitForMessage
        | RobotsHandlerScriptCommandSemantic::WaitForItemLandOn
        | RobotsHandlerScriptCommandSemantic::WaitForItemOff
        | RobotsHandlerScriptCommandSemantic::WaitForMessageSpecific
        | RobotsHandlerScriptCommandSemantic::StateMarker { .. }
        | RobotsHandlerScriptCommandSemantic::SetAlternateState
        | RobotsHandlerScriptCommandSemantic::ClearStateMarker
        | RobotsHandlerScriptCommandSemantic::WaitForHit { .. }
        | RobotsHandlerScriptCommandSemantic::ChangeScript { .. }
        | RobotsHandlerScriptCommandSemantic::InventoryCheck { .. }
        | RobotsHandlerScriptCommandSemantic::TextureControl { .. }
        | RobotsHandlerScriptCommandSemantic::SoundStop { .. }
        | RobotsHandlerScriptCommandSemantic::PlayToStateMarker { .. }
        | RobotsHandlerScriptCommandSemantic::WaitForHitMissiles { .. }
        | RobotsHandlerScriptCommandSemantic::StateMarkerEnd => {
            execution.status = RobotsScriptCommandExecutionStatus::ExternalActionRequired;
        }
        RobotsHandlerScriptCommandSemantic::DelegateFamily { .. }
        | RobotsHandlerScriptCommandSemantic::Unhandled => {}
    }

    Some(execution)
}

fn execute_inventory_add(
    owner_selector: Option<u32>,
    item: Option<u32>,
    quantity: Option<f32>,
    fallback_item: Option<u32>,
    mode: Option<u32>,
    inventory_definitions: &[RobotsInventoryDefinition],
    state: &mut RobotsScriptGameplayState,
    execution: &mut RobotsScriptCommandExecution,
) {
    // Generic 0x00402EF0 immediately returns when event+0x14 != 0.
    if owner_selector != Some(0) {
        return;
    }
    let (Some(mut item), Some(quantity)) = (item, quantity) else {
        execution.status = RobotsScriptCommandExecutionStatus::InvalidPayload;
        return;
    };

    let mut quantity = native_script_ftol(quantity);
    let mut mode = mode.unwrap_or_default();
    if item == u32::MAX {
        let Some(fallback_item) = fallback_item.filter(|item| *item != u32::MAX) else {
            execution.status = RobotsScriptCommandExecutionStatus::NativeRejected;
            return;
        };
        item = fallback_item;
        quantity = 1;
        // Native 0x004031FC..0x0040320D bypasses the serialized mode for the
        // fallback-item path.
        mode = 0;
    }

    let Some(definition) = inventory_definition(inventory_definitions, item) else {
        execution.status = if (item & 0xFFFF_0000) == ROBOTS_INVENTORY_UID_BASE {
            RobotsScriptCommandExecutionStatus::NativeRejected
        } else {
            RobotsScriptCommandExecutionStatus::UnsupportedInventoryBackend
        };
        return;
    };

    if quantity > 0 {
        match mode {
            // 0x00403240..0x00403259: mode1 means "raise current to requested
            // quantity", so only the missing delta is passed to +0x30.
            1 => {
                let current = state.inventory.stored_current(item);
                if current >= quantity {
                    return;
                }
                quantity -= current;
            }
            // mode2 sets the same preflight flag but fails the native mode==1
            // branch for positive quantities, producing no mutation.
            2 => return,
            _ => {}
        }
    } else if quantity < 0 {
        return;
    } else if !definition.is_bitset() {
        // +0x28 -> +0x4C (0x0040AA30 -> 0x0040A630) permits zero only for
        // definitions with word_14 bit0 set. For a bitset, zero is bit index 0.
        return;
    }

    let outcome = state.inventory.add(Some(definition), quantity);
    execution
        .mutations
        .push(RobotsScriptCommandMutation::Inventory { item, outcome });
    execution.status = if matches!(outcome.native_result_code, 1 | 4) {
        RobotsScriptCommandExecutionStatus::Applied
    } else {
        RobotsScriptCommandExecutionStatus::NativeRejected
    };
}

fn execute_inventory_remove(
    owner_selector: Option<u32>,
    item: Option<u32>,
    quantity: Option<f32>,
    inventory_definitions: &[RobotsInventoryDefinition],
    state: &mut RobotsScriptGameplayState,
    scrap_removal_locked: bool,
    execution: &mut RobotsScriptCommandExecution,
) {
    if owner_selector != Some(0) {
        return;
    }
    let (Some(item), Some(quantity)) = (item, quantity) else {
        execution.status = RobotsScriptCommandExecutionStatus::InvalidPayload;
        return;
    };
    if item == u32::MAX {
        execution.status = RobotsScriptCommandExecutionStatus::NativeRejected;
        return;
    }
    let Some(definition) = inventory_definition(inventory_definitions, item) else {
        execution.status = if (item & 0xFFFF_0000) == ROBOTS_INVENTORY_UID_BASE {
            RobotsScriptCommandExecutionStatus::NativeRejected
        } else {
            RobotsScriptCommandExecutionStatus::UnsupportedInventoryBackend
        };
        return;
    };

    let quantity = native_script_ftol(quantity);
    let outcome = if quantity < 1 {
        state.inventory.remove_all(item)
    } else {
        state
            .inventory
            .remove_quantity(Some(definition), quantity, scrap_removal_locked)
    };
    execution
        .mutations
        .push(RobotsScriptCommandMutation::Inventory { item, outcome });
    // The Generic wrapper accepts only 1/4 without logging, but +0x40 can
    // legitimately return 6 after physically removing the last pair. Preserve
    // that native result while still reporting that state mutation occurred.
    execution.status = if matches!(outcome.native_result_code, 1 | 4 | 6) {
        RobotsScriptCommandExecutionStatus::Applied
    } else {
        RobotsScriptCommandExecutionStatus::NativeRejected
    };
}

fn execute_mission_update(
    mission: Option<u32>,
    mode: Option<u32>,
    flags: Option<u32>,
    mission_definitions: &[RobotsMissionDefinition],
    state: &mut RobotsScriptGameplayState,
    execution: &mut RobotsScriptCommandExecution,
) {
    let (Some(mission_uid), Some(mode), Some(flags)) = (mission, mode, flags) else {
        execution.status = RobotsScriptCommandExecutionStatus::InvalidPayload;
        return;
    };

    if is_robots_mission_uid(mission_uid)
        && mission_definitions
            .iter()
            .any(|definition| definition.mission_uid == mission_uid)
    {
        if let Some(status) = RobotsMissionStatus::from_update_mode(mode) {
            let previous = state.missions.set_status(mission_uid, status);
            execution
                .mutations
                .push(RobotsScriptCommandMutation::MissionStatus {
                    mission_uid,
                    previous,
                    current: status,
                });
        }
    }

    // 0x004033FE..0x00403412: flags are processed independently of Mission UID
    // validity; positive odd flags clear DAT_007B31CC, the current Mission owner.
    if flags > 0 && flags & 1 != 0 {
        execution
            .mutations
            .push(RobotsScriptCommandMutation::ClearCurrentMissionOwner);
    }

    if !execution.mutations.is_empty() {
        execution.status = RobotsScriptCommandExecutionStatus::Applied;
    }
}

fn inventory_definition(
    definitions: &[RobotsInventoryDefinition],
    uid: u32,
) -> Option<RobotsInventoryDefinition> {
    definitions
        .iter()
        .copied()
        .find(|definition| definition.uid == uid)
}

pub fn native_script_ftol(value: f32) -> i32 {
    // Shipped Robots Event payloads use finite values in i32 range. MSVC's
    // __ftol path used here truncates toward zero for those values.
    value.trunc() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::robots_runtime::events::event_type;

    fn event(event_type: u32, args: &[u32]) -> RobotsScriptEventView<'static> {
        let mut words = Vec::with_capacity(args.len() + 1);
        words.push(0);
        words.extend_from_slice(args);
        let bytes = words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        RobotsScriptEventView {
            event_type,
            data: Box::leak(bytes),
            start: None,
            length: None,
        }
    }

    fn counter_definition(uid: u32, target: u32) -> RobotsInventoryDefinition {
        RobotsInventoryDefinition::from_native_words([uid, u32::MAX, 0, 0, u32::MAX, 0, target])
            .unwrap()
    }

    fn bitset_definition(uid: u32, bit_count: u32) -> RobotsInventoryDefinition {
        RobotsInventoryDefinition::from_native_words([uid, u32::MAX, 0, 0, u32::MAX, 1, bit_count])
            .unwrap()
    }

    #[test]
    fn cutscene_inventory_add_resolves_to_generic_but_selector_four_stays_noop() {
        let definition = counter_definition(0x4700_0015, 100);
        let mut state = RobotsScriptGameplayState::default();
        let execution = execute_recovered_handler_script_command(
            RobotsHandlerScriptCommandFamily::Cutscene,
            event(
                event_type::INVENTORY_ADD,
                &[4, definition.uid, 12.0f32.to_bits(), u32::MAX, 0],
            ),
            &[definition],
            &[],
            &mut state,
            false,
        )
        .unwrap();
        assert_eq!(
            execution.status,
            RobotsScriptCommandExecutionStatus::RecoveredNoMutation
        );
        assert_eq!(state.inventory.stored_current(definition.uid), 0);
    }

    #[test]
    fn generic_bitset_zero_quantity_sets_bit_zero() {
        let definition = bitset_definition(0x4700_0012, 16);
        let mut state = RobotsScriptGameplayState::default();
        let execution = execute_recovered_handler_script_command(
            RobotsHandlerScriptCommandFamily::Generic,
            event(
                event_type::INVENTORY_ADD,
                &[0, definition.uid, 0.0f32.to_bits(), u32::MAX, 0],
            ),
            &[definition],
            &[],
            &mut state,
            false,
        )
        .unwrap();
        assert_eq!(
            execution.status,
            RobotsScriptCommandExecutionStatus::Applied
        );
        assert_eq!(state.inventory.stored_current(definition.uid), 1);
    }

    #[test]
    fn generic_inventory_add_mode_one_adds_only_missing_delta() {
        let definition = counter_definition(0x4700_0025, 100);
        let mut state = RobotsScriptGameplayState::default();
        state.inventory.add(Some(definition), 3);
        let execution = execute_recovered_handler_script_command(
            RobotsHandlerScriptCommandFamily::Generic,
            event(
                event_type::INVENTORY_ADD,
                &[0, definition.uid, 5.0f32.to_bits(), u32::MAX, 1],
            ),
            &[definition],
            &[],
            &mut state,
            false,
        )
        .unwrap();
        assert_eq!(
            execution.status,
            RobotsScriptCommandExecutionStatus::Applied
        );
        assert_eq!(state.inventory.stored_current(definition.uid), 5);
    }

    #[test]
    fn generic_inventory_add_mode_two_positive_quantity_is_noop() {
        let definition = counter_definition(0x4700_0025, 100);
        let mut state = RobotsScriptGameplayState::default();
        let execution = execute_recovered_handler_script_command(
            RobotsHandlerScriptCommandFamily::Generic,
            event(
                event_type::INVENTORY_ADD,
                &[0, definition.uid, 5.0f32.to_bits(), u32::MAX, 2],
            ),
            &[definition],
            &[],
            &mut state,
            false,
        )
        .unwrap();
        assert_eq!(
            execution.status,
            RobotsScriptCommandExecutionStatus::RecoveredNoMutation
        );
        assert_eq!(state.inventory.stored_current(definition.uid), 0);
    }

    #[test]
    fn generic_inventory_add_fallback_item_forces_quantity_one() {
        let definition = counter_definition(0x4700_0027, 100);
        let mut state = RobotsScriptGameplayState::default();
        let execution = execute_recovered_handler_script_command(
            RobotsHandlerScriptCommandFamily::Generic,
            event(
                event_type::INVENTORY_ADD,
                &[0, u32::MAX, 99.0f32.to_bits(), definition.uid, 2],
            ),
            &[definition],
            &[],
            &mut state,
            false,
        )
        .unwrap();
        assert_eq!(
            execution.status,
            RobotsScriptCommandExecutionStatus::Applied
        );
        assert_eq!(state.inventory.stored_current(definition.uid), 1);
    }

    #[test]
    fn mission_update_changes_shared_status_and_can_clear_owner() {
        let mission = RobotsMissionDefinition {
            mission_uid: 0x5400_0002,
            objective_uid: 0x4700_0012,
            hud_item_uid: u32::MAX,
            mission_text_uid: u32::MAX,
        };
        let mut state = RobotsScriptGameplayState::default();
        let execution = execute_recovered_handler_script_command(
            RobotsHandlerScriptCommandFamily::Generic,
            event(event_type::MISSION_UPDATE, &[mission.mission_uid, 2, 1]),
            &[],
            &[mission],
            &mut state,
            false,
        )
        .unwrap();
        assert_eq!(
            execution.status,
            RobotsScriptCommandExecutionStatus::Applied
        );
        assert_eq!(
            state.missions.status(mission.mission_uid),
            RobotsMissionStatus::Completed
        );
        assert_eq!(execution.mutations.len(), 2);
        assert!(matches!(
            execution.mutations[1],
            RobotsScriptCommandMutation::ClearCurrentMissionOwner
        ));
    }

    #[test]
    fn recovered_specialized_family_can_reach_generic_gameplay_mutation() {
        let mission = RobotsMissionDefinition {
            mission_uid: 0x5400_0002,
            objective_uid: 0x4700_0012,
            hud_item_uid: u32::MAX,
            mission_text_uid: u32::MAX,
        };
        let mut state = RobotsScriptGameplayState::default();
        let execution = execute_recovered_handler_script_command(
            RobotsHandlerScriptCommandFamily::MagneticScript,
            event(event_type::MISSION_UPDATE, &[mission.mission_uid, 3, 0]),
            &[],
            &[mission],
            &mut state,
            false,
        )
        .expect("MagneticScript -> ScriptLifecycle -> Generic is recovered");
        assert_eq!(
            execution.status,
            RobotsScriptCommandExecutionStatus::Applied
        );
        assert_eq!(
            state.missions.status(mission.mission_uid),
            RobotsMissionStatus::Failed
        );
    }

    #[test]
    fn runtime_class_ingress_preserves_shop_delegation_and_pickup_swallow() {
        let mission = RobotsMissionDefinition {
            mission_uid: 0x5400_0004,
            objective_uid: 0x4700_0012,
            hud_item_uid: u32::MAX,
            mission_text_uid: u32::MAX,
        };

        let mut shop_state = RobotsScriptGameplayState::default();
        let shop = execute_recovered_handler_script_command_for_class(
            "XItemHandler_Shop",
            event(event_type::MISSION_UPDATE, &[mission.mission_uid, 2, 0]),
            &[],
            &[mission],
            &mut shop_state,
            false,
        )
        .expect("Shop -> ArcadeShop -> Interactive -> ScriptLifecycle -> Generic");
        assert_eq!(shop.status, RobotsScriptCommandExecutionStatus::Applied);
        assert_eq!(
            shop_state.missions.status(mission.mission_uid),
            RobotsMissionStatus::Completed
        );

        let mut pickup_state = RobotsScriptGameplayState::default();
        let pickup = execute_recovered_handler_script_command_for_class(
            "XItemHandler_Pickup",
            event(event_type::MISSION_UPDATE, &[mission.mission_uid, 2, 0]),
            &[],
            &[mission],
            &mut pickup_state,
            false,
        )
        .expect("Pickup family is recovered and intentionally swallows non-FFFFFFFF commands");
        assert_eq!(
            pickup.status,
            RobotsScriptCommandExecutionStatus::RecoveredNoMutation
        );
        assert_eq!(
            pickup_state.missions.status(mission.mission_uid),
            RobotsMissionStatus::Inactive
        );

        assert!(execute_recovered_handler_script_command_for_class(
            "XItemHandler_NotInRobotsExe",
            event(event_type::MISSION_UPDATE, &[mission.mission_uid, 2, 0]),
            &[],
            &[mission],
            &mut RobotsScriptGameplayState::default(),
            false,
        )
        .is_none());
    }
}
