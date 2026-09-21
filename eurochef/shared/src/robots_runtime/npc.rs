use serde::Serialize;

use super::mission::{objective_condition_met, RobotsMissionStatus, RobotsObjectiveProgress};

pub const ROBOTS_NPC_NULL_CUTSCENE_UID: u32 = 0x0400_0000;
pub const ROBOTS_NPC_NULL_OBJECTIVE_UID: u32 = 0x5400_0000;
pub const ROBOTS_NPC_NULL_TEXT_GROUP_UID: u32 = 0x4508_0000;
pub const ROBOTS_NPC_FOCUS_BLOCK_FLAG: u32 = 0x10;
pub const ROBOTS_NPC_CUTSCENE_COPY_POSITION_FLAG: u32 = 0x100;
pub const ROBOTS_NPC_CUTSCENE_COPY_ROTATION_FLAG: u32 = 0x200;
pub const ROBOTS_NPC_TUTORIAL_ACTIVATE_EVENT: u32 = 0x100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsNpcObjectiveStatus {
    Inactive = 0,
    Active = 1,
    Failed = 2,
    Completed = 3,
}

impl RobotsNpcObjectiveStatus {
    pub fn selector(self) -> u8 {
        self as u8
    }
}

impl From<RobotsMissionStatus> for RobotsNpcObjectiveStatus {
    fn from(status: RobotsMissionStatus) -> Self {
        match status {
            RobotsMissionStatus::Inactive => Self::Inactive,
            RobotsMissionStatus::Active => Self::Active,
            RobotsMissionStatus::Failed => Self::Failed,
            RobotsMissionStatus::Completed => Self::Completed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RobotsNpcSerializedContract {
    pub data0: Option<u32>,
    pub data1: Option<u32>,
    pub flags: u32,
    pub text_group: Option<u32>,
    pub alternate_cutscenes: [Option<u32>; 4],
    pub normal_cutscene_requires_clear_e10c: bool,
    pub completed_mission_uses_selector3_once: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsNpcRuntimeState {
    pub focus_active: bool,
    pub simple_text_active: bool,
    pub latch_e10c: bool,
    pub latch_e10d: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsNpcFocusAction {
    None,
    StopSimpleText,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsNpcTutorialInteractionState {
    pub state_e4: u32,
    pub latch_e8: bool,
    pub block_e9: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RobotsNpcTutorialInteractionPlan {
    NotConsumed,
    Consumed,
    Activate { event_mask: u32 },
    AdvanceState { from: u32, to: u32 },
}

impl RobotsNpcTutorialInteractionPlan {
    pub fn consumed(self) -> bool {
        !matches!(self, Self::NotConsumed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RobotsNpcInteractionInput {
    pub trigger_present: bool,
    pub objective_uid: Option<u32>,
    pub objective_status: Option<RobotsNpcObjectiveStatus>,
    pub objective_progress: Option<RobotsObjectiveProgress>,
    pub tutorial_consumes_interaction: bool,
    pub latch_e10c: bool,
    pub latch_e10d: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RobotsNpcInteractionAction {
    NoTrigger,
    RequiresObjectiveStatus {
        objective_uid: u32,
    },
    TutorialConsumed,
    SimpleNpc,
    NormalCutscene,
    MissionCutscene {
        selector: u8,
        cutscene_uid: Option<u32>,
        set_latch_e10d: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RobotsNpcPresentationPlan {
    None,
    StartSimpleText {
        text_group_uid: u32,
    },
    ActivateCutscene {
        alternate_uid: Option<u32>,
        copy_position: bool,
        copy_rotation: bool,
    },
}

pub fn normalize_npc_objective_uid(uid: Option<u32>) -> Option<u32> {
    uid.filter(|uid| *uid != u32::MAX && *uid != ROBOTS_NPC_NULL_OBJECTIVE_UID)
}

pub fn normalize_npc_cutscene_uid(uid: Option<u32>) -> Option<u32> {
    uid.filter(|uid| *uid != ROBOTS_NPC_NULL_CUTSCENE_UID)
}

pub fn normalize_npc_text_group_uid(uid: Option<u32>) -> Option<u32> {
    uid.filter(|uid| !matches!(*uid, 0 | u32::MAX | ROBOTS_NPC_NULL_TEXT_GROUP_UID))
}

pub fn decode_npc_serialized_contract(data: &[Option<u32>; 16]) -> RobotsNpcSerializedContract {
    decode_npc_serialized_contract_slice(data)
}

pub fn decode_npc_serialized_contract_slice(data: &[Option<u32>]) -> RobotsNpcSerializedContract {
    let get = |index: usize| data.get(index).copied().flatten();
    let flags = get(2).unwrap_or_default();
    RobotsNpcSerializedContract {
        data0: get(0),
        data1: get(1),
        flags,
        text_group: get(3),
        alternate_cutscenes: [
            normalize_npc_cutscene_uid(get(4)),
            normalize_npc_cutscene_uid(get(5)),
            normalize_npc_cutscene_uid(get(6)),
            normalize_npc_cutscene_uid(get(7)),
        ],
        normal_cutscene_requires_clear_e10c: flags & 0x1 != 0,
        completed_mission_uses_selector3_once: flags & 0x4 != 0,
    }
}

pub fn npc_focus_update_enabled(contract: &RobotsNpcSerializedContract) -> bool {
    contract.flags & ROBOTS_NPC_FOCUS_BLOCK_FLAG == 0
}

pub fn update_npc_focus(
    state: &mut RobotsNpcRuntimeState,
    contract: &RobotsNpcSerializedContract,
    is_player_focus_target: bool,
) -> RobotsNpcFocusAction {
    if !npc_focus_update_enabled(contract) {
        return RobotsNpcFocusAction::None;
    }
    if is_player_focus_target {
        state.focus_active = true;
        return RobotsNpcFocusAction::None;
    }
    if !state.focus_active {
        return RobotsNpcFocusAction::None;
    }
    state.focus_active = false;
    if state.simple_text_active {
        state.simple_text_active = false;
        RobotsNpcFocusAction::StopSimpleText
    } else {
        RobotsNpcFocusAction::None
    }
}

pub fn start_npc_simple_text(
    state: &mut RobotsNpcRuntimeState,
    contract: &RobotsNpcSerializedContract,
    presentation_busy: bool,
    text_group_resolved: bool,
) -> Option<u32> {
    if presentation_busy || !text_group_resolved {
        return None;
    }
    let text_group_uid = normalize_npc_text_group_uid(contract.text_group)?;
    state.simple_text_active = true;
    Some(text_group_uid)
}

/// Convert the interaction decision into the exact host-side presentation
/// request without making NPC logic own TextGroup/Cutscene execution.
///
/// Native `XItemHandler_Npc::DoInteraction` routes the simple branch through
/// `0x0046BBA0`; normal and mission cutscene branches end in
/// `XTrigger_NPC::ActivateCutscene` (`0x0047F1C0`). That helper writes an
/// optional alternate Cutscene UID, optionally copies the live NPC position and
/// orientation via flags 0x100/0x200, and dispatches event 0x101 to the linked
/// `XTrigger_Cutscene`. Resource resolution and playback stay host concerns.
pub fn plan_npc_presentation(
    contract: &RobotsNpcSerializedContract,
    action: RobotsNpcInteractionAction,
) -> RobotsNpcPresentationPlan {
    match action {
        RobotsNpcInteractionAction::SimpleNpc => normalize_npc_text_group_uid(contract.text_group)
            .map(|text_group_uid| RobotsNpcPresentationPlan::StartSimpleText { text_group_uid })
            .unwrap_or(RobotsNpcPresentationPlan::None),
        RobotsNpcInteractionAction::NormalCutscene => RobotsNpcPresentationPlan::ActivateCutscene {
            alternate_uid: None,
            copy_position: contract.flags & ROBOTS_NPC_CUTSCENE_COPY_POSITION_FLAG != 0,
            copy_rotation: contract.flags & ROBOTS_NPC_CUTSCENE_COPY_ROTATION_FLAG != 0,
        },
        RobotsNpcInteractionAction::MissionCutscene { cutscene_uid, .. } => {
            RobotsNpcPresentationPlan::ActivateCutscene {
                alternate_uid: cutscene_uid,
                copy_position: contract.flags & ROBOTS_NPC_CUTSCENE_COPY_POSITION_FLAG != 0,
                copy_rotation: contract.flags & ROBOTS_NPC_CUTSCENE_COPY_ROTATION_FLAG != 0,
            }
        }
        RobotsNpcInteractionAction::NoTrigger
        | RobotsNpcInteractionAction::RequiresObjectiveStatus { .. }
        | RobotsNpcInteractionAction::TutorialConsumed => RobotsNpcPresentationPlan::None,
    }
}

pub fn plan_npc_tutorial_interaction(
    state: Option<&mut RobotsNpcTutorialInteractionState>,
) -> RobotsNpcTutorialInteractionPlan {
    let Some(state) = state else {
        return RobotsNpcTutorialInteractionPlan::NotConsumed;
    };
    if state.block_e9 {
        return RobotsNpcTutorialInteractionPlan::NotConsumed;
    }
    if !state.latch_e8 {
        return RobotsNpcTutorialInteractionPlan::Activate {
            event_mask: ROBOTS_NPC_TUTORIAL_ACTIVATE_EVENT,
        };
    }
    if state.state_e4 == 1 {
        state.state_e4 = 2;
        return RobotsNpcTutorialInteractionPlan::AdvanceState { from: 1, to: 2 };
    }
    RobotsNpcTutorialInteractionPlan::Consumed
}

pub fn plan_npc_interaction(
    contract: &RobotsNpcSerializedContract,
    input: RobotsNpcInteractionInput,
) -> RobotsNpcInteractionAction {
    if !input.trigger_present {
        return RobotsNpcInteractionAction::NoTrigger;
    }

    let objective_uid = normalize_npc_objective_uid(input.objective_uid);
    if let Some(objective_uid) = objective_uid {
        let Some(status) = input.objective_status else {
            return RobotsNpcInteractionAction::RequiresObjectiveStatus { objective_uid };
        };

        if status == RobotsNpcObjectiveStatus::Completed {
            if contract.completed_mission_uses_selector3_once && !input.latch_e10d {
                return mission_cutscene(contract, 3, true);
            }
            return fallback_npc_interaction(contract, input.latch_e10c);
        }

        let selector = if status == RobotsNpcObjectiveStatus::Active
            && objective_condition_met(input.objective_progress)
        {
            3
        } else {
            status.selector()
        };
        return mission_cutscene(contract, selector, selector == 3);
    }

    if input.tutorial_consumes_interaction {
        return RobotsNpcInteractionAction::TutorialConsumed;
    }

    fallback_npc_interaction(contract, input.latch_e10c)
}

fn fallback_npc_interaction(
    contract: &RobotsNpcSerializedContract,
    latch_e10c: bool,
) -> RobotsNpcInteractionAction {
    if contract.normal_cutscene_requires_clear_e10c && !latch_e10c {
        RobotsNpcInteractionAction::NormalCutscene
    } else {
        RobotsNpcInteractionAction::SimpleNpc
    }
}

fn mission_cutscene(
    contract: &RobotsNpcSerializedContract,
    selector: u8,
    set_latch_e10d: bool,
) -> RobotsNpcInteractionAction {
    let cutscene_uid = contract
        .alternate_cutscenes
        .get(selector as usize)
        .copied()
        .flatten();
    RobotsNpcInteractionAction::MissionCutscene {
        selector,
        cutscene_uid,
        set_latch_e10d,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(flags: u32) -> [Option<u32>; 16] {
        let mut data = [None; 16];
        data[2] = Some(flags);
        data[3] = Some(0x4500_0123);
        data[4] = Some(0x0400_0100);
        data[5] = Some(0x0400_0101);
        data[6] = Some(ROBOTS_NPC_NULL_CUTSCENE_UID);
        data[7] = Some(0x0400_0103);
        data
    }

    fn input() -> RobotsNpcInteractionInput {
        RobotsNpcInteractionInput {
            trigger_present: true,
            objective_uid: None,
            objective_status: None,
            objective_progress: None,
            tutorial_consumes_interaction: false,
            latch_e10c: false,
            latch_e10d: false,
        }
    }

    #[test]
    fn serialized_contract_preserves_native_slots_and_cutscene_sentinel() {
        let contract = decode_npc_serialized_contract(&data(0x5));
        assert_eq!(contract.flags, 0x5);
        assert_eq!(contract.text_group, Some(0x4500_0123));
        assert_eq!(contract.alternate_cutscenes[0], Some(0x0400_0100));
        assert_eq!(contract.alternate_cutscenes[2], None);
        assert!(contract.normal_cutscene_requires_clear_e10c);
        assert!(contract.completed_mission_uses_selector3_once);
    }

    #[test]
    fn completed_objective_uses_selector3_once_then_falls_back() {
        let contract = decode_npc_serialized_contract(&data(0x5));
        let mut state = input();
        state.objective_uid = Some(0x5400_0100);
        state.objective_status = Some(RobotsNpcObjectiveStatus::Completed);
        assert_eq!(
            plan_npc_interaction(&contract, state),
            RobotsNpcInteractionAction::MissionCutscene {
                selector: 3,
                cutscene_uid: Some(0x0400_0103),
                set_latch_e10d: true,
            }
        );

        state.latch_e10d = true;
        assert_eq!(
            plan_npc_interaction(&contract, state),
            RobotsNpcInteractionAction::NormalCutscene
        );
    }

    #[test]
    fn active_objective_can_promote_to_selector3() {
        let contract = decode_npc_serialized_contract(&data(0));
        let mut state = input();
        state.objective_uid = Some(0x5400_0100);
        state.objective_status = Some(RobotsNpcObjectiveStatus::Active);
        assert_eq!(
            plan_npc_interaction(&contract, state),
            RobotsNpcInteractionAction::MissionCutscene {
                selector: 1,
                cutscene_uid: Some(0x0400_0101),
                set_latch_e10d: false,
            }
        );

        state.objective_progress = Some(RobotsObjectiveProgress {
            current: 3,
            target: 3,
        });
        assert_eq!(
            plan_npc_interaction(&contract, state),
            RobotsNpcInteractionAction::MissionCutscene {
                selector: 3,
                cutscene_uid: Some(0x0400_0103),
                set_latch_e10d: true,
            }
        );
    }

    #[test]
    fn missing_objective_delegates_to_tutorial_or_normal_simple_branch() {
        let contract = decode_npc_serialized_contract(&data(0x1));
        let mut state = input();
        state.objective_uid = Some(ROBOTS_NPC_NULL_OBJECTIVE_UID);
        state.tutorial_consumes_interaction = true;
        assert_eq!(
            plan_npc_interaction(&contract, state),
            RobotsNpcInteractionAction::TutorialConsumed
        );

        state.tutorial_consumes_interaction = false;
        assert_eq!(
            plan_npc_interaction(&contract, state),
            RobotsNpcInteractionAction::NormalCutscene
        );
        state.latch_e10c = true;
        assert_eq!(
            plan_npc_interaction(&contract, state),
            RobotsNpcInteractionAction::SimpleNpc
        );
    }

    #[test]
    fn unresolved_objective_status_fails_closed() {
        let contract = decode_npc_serialized_contract(&data(0));
        let mut state = input();
        state.objective_uid = Some(0x5400_0100);
        assert_eq!(
            plan_npc_interaction(&contract, state),
            RobotsNpcInteractionAction::RequiresObjectiveStatus {
                objective_uid: 0x5400_0100,
            }
        );
    }

    #[test]
    fn flag_0x10_blocks_focus_update_and_focus_loss_stops_simple_text() {
        let blocked = decode_npc_serialized_contract(&data(ROBOTS_NPC_FOCUS_BLOCK_FLAG));
        let mut state = RobotsNpcRuntimeState::default();
        assert!(!npc_focus_update_enabled(&blocked));
        assert_eq!(
            update_npc_focus(&mut state, &blocked, true),
            RobotsNpcFocusAction::None
        );
        assert!(!state.focus_active);

        let open = decode_npc_serialized_contract(&data(0));
        assert_eq!(
            update_npc_focus(&mut state, &open, true),
            RobotsNpcFocusAction::None
        );
        state.simple_text_active = true;
        assert_eq!(
            update_npc_focus(&mut state, &open, false),
            RobotsNpcFocusAction::StopSimpleText
        );
        assert!(!state.focus_active);
        assert!(!state.simple_text_active);
    }

    #[test]
    fn simple_text_requires_valid_resolved_text_group_and_free_channel() {
        let contract = decode_npc_serialized_contract(&data(0));
        let mut state = RobotsNpcRuntimeState::default();
        assert_eq!(
            start_npc_simple_text(&mut state, &contract, false, true),
            Some(0x4500_0123)
        );
        assert!(state.simple_text_active);
        state.simple_text_active = false;
        assert_eq!(
            start_npc_simple_text(&mut state, &contract, true, true),
            None
        );
        assert!(!state.simple_text_active);
    }

    #[test]
    fn presentation_plan_keeps_text_and_cutscene_host_boundaries_explicit() {
        let contract = decode_npc_serialized_contract(&data(
            ROBOTS_NPC_CUTSCENE_COPY_POSITION_FLAG | ROBOTS_NPC_CUTSCENE_COPY_ROTATION_FLAG,
        ));
        assert_eq!(
            plan_npc_presentation(&contract, RobotsNpcInteractionAction::SimpleNpc),
            RobotsNpcPresentationPlan::StartSimpleText {
                text_group_uid: 0x4500_0123,
            }
        );
        assert_eq!(
            plan_npc_presentation(&contract, RobotsNpcInteractionAction::NormalCutscene),
            RobotsNpcPresentationPlan::ActivateCutscene {
                alternate_uid: None,
                copy_position: true,
                copy_rotation: true,
            }
        );
        assert_eq!(
            plan_npc_presentation(
                &contract,
                RobotsNpcInteractionAction::MissionCutscene {
                    selector: 1,
                    cutscene_uid: Some(0x0400_0101),
                    set_latch_e10d: false,
                },
            ),
            RobotsNpcPresentationPlan::ActivateCutscene {
                alternate_uid: Some(0x0400_0101),
                copy_position: true,
                copy_rotation: true,
            }
        );
        assert_eq!(
            plan_npc_presentation(&contract, RobotsNpcInteractionAction::TutorialConsumed),
            RobotsNpcPresentationPlan::None
        );
    }

    #[test]
    fn tutorial_interaction_matches_native_consume_activate_and_state1_to2_paths() {
        assert_eq!(
            plan_npc_tutorial_interaction(None),
            RobotsNpcTutorialInteractionPlan::NotConsumed
        );

        let mut tutorial = RobotsNpcTutorialInteractionState::default();
        let first = plan_npc_tutorial_interaction(Some(&mut tutorial));
        assert_eq!(
            first,
            RobotsNpcTutorialInteractionPlan::Activate {
                event_mask: ROBOTS_NPC_TUTORIAL_ACTIVATE_EVENT,
            }
        );
        assert!(first.consumed());
        assert!(!tutorial.latch_e8);

        tutorial.latch_e8 = true;
        tutorial.state_e4 = 1;
        assert_eq!(
            plan_npc_tutorial_interaction(Some(&mut tutorial)),
            RobotsNpcTutorialInteractionPlan::AdvanceState { from: 1, to: 2 }
        );
        assert_eq!(tutorial.state_e4, 2);

        tutorial.block_e9 = true;
        assert_eq!(
            plan_npc_tutorial_interaction(Some(&mut tutorial)),
            RobotsNpcTutorialInteractionPlan::NotConsumed
        );
    }
}
