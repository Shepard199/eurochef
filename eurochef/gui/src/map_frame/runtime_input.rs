use eurochef_shared::robots_runtime::message_presentation::{
    ROBOTS_MESSAGE_INPUT_CONFIRM, ROBOTS_MESSAGE_INPUT_SELECT_FIRST,
    ROBOTS_MESSAGE_INPUT_SELECT_SECOND,
};

/// Native PC keyboard defaults from the 48-slot DirectInput action table at 0x005E1758.
///
/// These are DIK codes, not USB HID usages:
/// - action 0x04 -> DIK_Q 0x10
/// - action 0x25 -> DIK_APOSTROPHE 0x28
/// - action 0x2C -> DIK_NUMPAD0 0x52
/// - action 0x2D -> DIK_NUMPAD3 0x51
pub(super) const ROBOTS_PC_DIK_INTERACT: u8 = 0x10;
pub(super) const ROBOTS_PC_DIK_MESSAGE_CONFIRM: u8 = 0x28;
pub(super) const ROBOTS_PC_DIK_MESSAGE_SELECT_FIRST: u8 = 0x52;
pub(super) const ROBOTS_PC_DIK_MESSAGE_SELECT_SECOND: u8 = 0x51;

pub(super) const fn robots_message_action_from_pc_dik(dik: u8) -> Option<u32> {
    match dik {
        ROBOTS_PC_DIK_MESSAGE_CONFIRM => Some(ROBOTS_MESSAGE_INPUT_CONFIRM),
        ROBOTS_PC_DIK_MESSAGE_SELECT_FIRST => Some(ROBOTS_MESSAGE_INPUT_SELECT_FIRST),
        ROBOTS_PC_DIK_MESSAGE_SELECT_SECOND => Some(ROBOTS_MESSAGE_INPUT_SELECT_SECOND),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct RobotsGameplayHostInputFrame {
    pub(super) interact: bool,
    pub(super) watchbot_axis_xz: [f32; 2],
    pub(super) watchbot_action_0c: bool,
    pub(super) watchbot_action_0d: bool,
    pub(super) watchbot_action_0e: bool,
    pub(super) watchbot_action_0f: bool,
}

pub(super) fn robots_gameplay_input_from_egui(events: &[egui::Event]) -> RobotsGameplayHostInputFrame {
    let mut frame = RobotsGameplayHostInputFrame::default();
    for event in events {
        let egui::Event::Key {
            key,
            physical_key,
            pressed: true,
            repeat: false,
            ..
        } = event
        else {
            continue;
        };
        let key = physical_key.as_ref().copied().unwrap_or(*key);
        let native_dik = match key {
            egui::Key::Q => Some(ROBOTS_PC_DIK_INTERACT),
            _ => None,
        };
        if native_dik == Some(ROBOTS_PC_DIK_INTERACT) {
            frame.interact = true;
        }
    }

    frame
}

pub(super) fn robots_watchbot_input_from_egui(input: &egui::InputState) -> RobotsGameplayHostInputFrame {
    let held = |key: egui::Key| input.key_down(key);
    let x = held(egui::Key::D) as i8 - held(egui::Key::A) as i8;
    let z = held(egui::Key::U) as i8 - held(egui::Key::Z) as i8;
    let action_0f = input
        .events
        .iter()
        .any(|event| matches!(event, egui::Event::Key { key: egui::Key::Q, pressed: true, repeat: false, .. }));
    let action_0d = input.events.iter().any(|event| matches!(event, egui::Event::Key { key: egui::Key::Tab, pressed: true, repeat: false, .. }));
    let action_0e = input.events.iter().any(|event| matches!(event, egui::Event::Key { key: egui::Key::E, pressed: true, repeat: false, .. }));
    let action_0c = false;
    RobotsGameplayHostInputFrame {
        interact: false,
        watchbot_axis_xz: [x as f32, z as f32],
        watchbot_action_0c: action_0c,
        watchbot_action_0d: action_0d,
        watchbot_action_0e: action_0e,
        watchbot_action_0f: action_0f,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct RobotsMessageHostInputFrame {
    select_first: bool,
    select_second: bool,
    confirm: bool,
}

impl RobotsMessageHostInputFrame {
    /// Preserve the query order used by native 0x004A6AE0.
    pub(super) const fn ordered_actions(self) -> [Option<u32>; 3] {
        [
            if self.select_first {
                Some(ROBOTS_MESSAGE_INPUT_SELECT_FIRST)
            } else {
                None
            },
            if self.select_second {
                Some(ROBOTS_MESSAGE_INPUT_SELECT_SECOND)
            } else {
                None
            },
            if self.confirm {
                Some(ROBOTS_MESSAGE_INPUT_CONFIRM)
            } else {
                None
            },
        ]
    }
}

/// eframe/egui host adapter for the recovered native DirectInput defaults.
///
/// Physical keyboard details stay here; shared message presentation consumes
/// only logical actions so the same runtime contract can be reused by UE5.8.
/// egui 0.35 collapses main-row and numpad digits into Key::Num0..Num9, so the
/// preview host cannot distinguish DIK_NUMPAD0/3 from top-row 0/3. UE5.8 should
/// bind the exact physical numpad keys at its input adapter instead.
pub(super) fn robots_message_input_from_egui(events: &[egui::Event]) -> RobotsMessageHostInputFrame {
    let mut frame = RobotsMessageHostInputFrame::default();

    for event in events {
        let egui::Event::Key {
            key,
            physical_key,
            pressed: true,
            repeat: false,
            ..
        } = event
        else {
            continue;
        };
        let key = physical_key.as_ref().copied().unwrap_or(*key);
        let native_dik = match key {
            egui::Key::Quote => Some(ROBOTS_PC_DIK_MESSAGE_CONFIRM),
            egui::Key::Num0 => Some(ROBOTS_PC_DIK_MESSAGE_SELECT_FIRST),
            egui::Key::Num3 => Some(ROBOTS_PC_DIK_MESSAGE_SELECT_SECOND),
            _ => None,
        };
        let Some(action) = native_dik.and_then(robots_message_action_from_pc_dik) else {
            continue;
        };
        match action {
            ROBOTS_MESSAGE_INPUT_SELECT_FIRST => frame.select_first = true,
            ROBOTS_MESSAGE_INPUT_SELECT_SECOND => frame.select_second = true,
            ROBOTS_MESSAGE_INPUT_CONFIRM => frame.confirm = true,
            _ => {}
        }
    }

    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_pc_interact_binding_matches_recovered_directinput_default() {
        assert_eq!(ROBOTS_PC_DIK_INTERACT, 0x10);
        let event = egui::Event::Key {
            key: egui::Key::Q,
            physical_key: Some(egui::Key::Q),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        };
        assert!(robots_gameplay_input_from_egui(&[event]).interact);
    }

    #[test]
    fn legacy_pc_message_bindings_match_recovered_directinput_defaults() {
        assert_eq!(
            robots_message_action_from_pc_dik(0x28),
            Some(ROBOTS_MESSAGE_INPUT_CONFIRM)
        );
        assert_eq!(
            robots_message_action_from_pc_dik(0x52),
            Some(ROBOTS_MESSAGE_INPUT_SELECT_FIRST)
        );
        assert_eq!(
            robots_message_action_from_pc_dik(0x51),
            Some(ROBOTS_MESSAGE_INPUT_SELECT_SECOND)
        );
        assert_eq!(robots_message_action_from_pc_dik(0x1c), None);
    }

    #[test]
    fn egui_adapter_projects_quote_num0_num3_onto_recovered_actions() {
        let event = |key| egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        };
        let events = [
            event(egui::Key::Num3),
            event(egui::Key::Quote),
            event(egui::Key::Num0),
        ];
        assert_eq!(
            robots_message_input_from_egui(&events).ordered_actions(),
            [
                Some(ROBOTS_MESSAGE_INPUT_SELECT_FIRST),
                Some(ROBOTS_MESSAGE_INPUT_SELECT_SECOND),
                Some(ROBOTS_MESSAGE_INPUT_CONFIRM),
            ]
        );
    }

    #[test]
    fn host_frame_preserves_native_selection_then_confirm_order() {
        let frame = RobotsMessageHostInputFrame {
            select_first: true,
            select_second: true,
            confirm: true,
        };
        assert_eq!(
            frame.ordered_actions(),
            [
                Some(ROBOTS_MESSAGE_INPUT_SELECT_FIRST),
                Some(ROBOTS_MESSAGE_INPUT_SELECT_SECOND),
                Some(ROBOTS_MESSAGE_INPUT_CONFIRM),
            ]
        );
    }
}
