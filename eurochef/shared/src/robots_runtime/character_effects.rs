use serde::Serialize;

use super::events::{event_type, RobotsScriptEventView};

pub const ROBOTS_CHARACTER_EFFECT_PARTICLE_HASH_CLASS: u8 = 0x11;
pub const ROBOTS_CHARACTER_EFFECT_SWOOSH_HASH_CLASS: u8 = 0x19;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsCharacterAttachmentKind {
    Particle,
    Swoosh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsCharacterAttachment {
    pub kind: RobotsCharacterAttachmentKind,
    pub resource_uid: u32,
    pub binding_uid: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsCharacterAttachmentAction {
    Attach(RobotsCharacterAttachment),
    DetachFirst { kind: RobotsCharacterAttachmentKind },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RobotsCharacterAttachmentRuntimeState {
    pub attachments: Vec<RobotsCharacterAttachment>,
}

/// Native shared attachment factory `0x0042A180` derives the effect object type
/// from the resource UID's high hash-class byte, not from the Event opcode name.
/// Only Particle (0x11) and Swoosh (0x19) create attachment objects in this path.
pub const fn character_attachment_kind_from_resource_uid(
    resource_uid: u32,
) -> Option<RobotsCharacterAttachmentKind> {
    match ((resource_uid >> 24) & 0x7f) as u8 {
        ROBOTS_CHARACTER_EFFECT_PARTICLE_HASH_CLASS => {
            Some(RobotsCharacterAttachmentKind::Particle)
        }
        ROBOTS_CHARACTER_EFFECT_SWOOSH_HASH_CLASS => Some(RobotsCharacterAttachmentKind::Swoosh),
        _ => None,
    }
}

/// Common AI_Character event seam for handler events routed through vslot +0xA4
/// (`0x004AD930`) and vslot +0xB0 (`0x00403AC0`). AttachSwoosh and
/// AttachParticle both pass their two native args to the same factory. DettachSwoosh
/// passes only the resource UID and native `0x0042A2B0` removes the first attachment
/// of the derived type; it does not search for an exact resource UID match.
pub fn classify_ai_character_attachment_event(
    event: RobotsScriptEventView<'_>,
) -> Option<RobotsCharacterAttachmentAction> {
    match event.event_type {
        event_type::ATTACH_SWOOSH | event_type::ATTACH_PARTICLE => {
            let resource_uid = event.native_arg_word(0)?;
            if resource_uid == u32::MAX {
                return None;
            }
            let kind = character_attachment_kind_from_resource_uid(resource_uid)?;
            Some(RobotsCharacterAttachmentAction::Attach(
                RobotsCharacterAttachment {
                    kind,
                    resource_uid,
                    binding_uid: event.native_arg_word(1).unwrap_or(u32::MAX),
                },
            ))
        }
        event_type::DETTACH_SWOOSH => {
            let resource_uid = event.native_arg_word(0)?;
            if resource_uid == u32::MAX {
                return None;
            }
            let kind = character_attachment_kind_from_resource_uid(resource_uid)?;
            Some(RobotsCharacterAttachmentAction::DetachFirst { kind })
        }
        _ => None,
    }
}

pub fn apply_character_attachment_action(
    state: &mut RobotsCharacterAttachmentRuntimeState,
    action: RobotsCharacterAttachmentAction,
) -> bool {
    match action {
        RobotsCharacterAttachmentAction::Attach(attachment) => {
            state.attachments.push(attachment);
            true
        }
        RobotsCharacterAttachmentAction::DetachFirst { kind } => {
            let Some(index) = state
                .attachments
                .iter()
                .position(|attachment| attachment.kind == kind)
            else {
                return false;
            };
            state.attachments.remove(index);
            true
        }
    }
}

pub fn apply_ai_character_attachment_event(
    state: &mut RobotsCharacterAttachmentRuntimeState,
    event: RobotsScriptEventView<'_>,
) -> Option<RobotsCharacterAttachmentAction> {
    let action = classify_ai_character_attachment_event(event)?;
    apply_character_attachment_action(state, action);
    Some(action)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(event_type: u32, args: &[u32]) -> RobotsScriptEventView<'static> {
        let mut data = vec![0u8; 4 + args.len() * 4];
        for (index, arg) in args.iter().copied().enumerate() {
            data[4 + index * 4..8 + index * 4].copy_from_slice(&arg.to_le_bytes());
        }
        RobotsScriptEventView {
            event_type,
            data: Box::leak(data.into_boxed_slice()),
            start: None,
            length: None,
        }
    }

    #[test]
    fn native_hash_class_selects_particle_or_swoosh_independent_of_opcode_name() {
        assert_eq!(
            character_attachment_kind_from_resource_uid(0x1100_0007),
            Some(RobotsCharacterAttachmentKind::Particle)
        );
        assert_eq!(
            character_attachment_kind_from_resource_uid(0x1900_0018),
            Some(RobotsCharacterAttachmentKind::Swoosh)
        );
        assert_eq!(
            character_attachment_kind_from_resource_uid(0x1000_000C),
            None
        );

        let action = classify_ai_character_attachment_event(event(
            event_type::ATTACH_PARTICLE,
            &[0x1900_0018, u32::MAX],
        ));
        assert_eq!(
            action,
            Some(RobotsCharacterAttachmentAction::Attach(
                RobotsCharacterAttachment {
                    kind: RobotsCharacterAttachmentKind::Swoosh,
                    resource_uid: 0x1900_0018,
                    binding_uid: u32::MAX,
                }
            ))
        );
    }

    #[test]
    fn detach_matches_native_first_attachment_of_type_not_exact_uid() {
        let mut state = RobotsCharacterAttachmentRuntimeState::default();
        for (event_type, resource_uid) in [
            (event_type::ATTACH_SWOOSH, 0x1900_0017),
            (event_type::ATTACH_PARTICLE, 0x1100_0007),
            (event_type::ATTACH_SWOOSH, 0x1900_0018),
        ] {
            apply_ai_character_attachment_event(
                &mut state,
                event(event_type, &[resource_uid, u32::MAX]),
            )
            .expect("attachment event");
        }
        assert_eq!(state.attachments.len(), 3);

        let action = apply_ai_character_attachment_event(
            &mut state,
            event(event_type::DETTACH_SWOOSH, &[0x1900_0018]),
        )
        .expect("detach event");
        assert_eq!(
            action,
            RobotsCharacterAttachmentAction::DetachFirst {
                kind: RobotsCharacterAttachmentKind::Swoosh,
            }
        );
        assert_eq!(
            state.attachments,
            vec![
                RobotsCharacterAttachment {
                    kind: RobotsCharacterAttachmentKind::Particle,
                    resource_uid: 0x1100_0007,
                    binding_uid: u32::MAX,
                },
                RobotsCharacterAttachment {
                    kind: RobotsCharacterAttachmentKind::Swoosh,
                    resource_uid: 0x1900_0018,
                    binding_uid: u32::MAX,
                },
            ]
        );
    }

    #[test]
    fn sentinel_or_unsupported_resources_do_not_create_native_attachment_objects() {
        let mut state = RobotsCharacterAttachmentRuntimeState::default();
        assert_eq!(
            apply_ai_character_attachment_event(
                &mut state,
                event(event_type::ATTACH_SWOOSH, &[u32::MAX, u32::MAX]),
            ),
            None
        );
        assert_eq!(
            apply_ai_character_attachment_event(
                &mut state,
                event(event_type::ATTACH_SWOOSH, &[0x0400_0001, u32::MAX]),
            ),
            None
        );
        assert!(state.attachments.is_empty());
    }
}
