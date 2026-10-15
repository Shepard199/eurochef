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
    AttachParticlesToSkeleton { args: [u32; 4] },
    TriggerSpecialEffectZero,
    DetachFirst { kind: RobotsCharacterAttachmentKind },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsCharacterAttachmentRuntimeEntry {
    pub kind: RobotsCharacterAttachmentKind,
    pub resource_uid: u32,
    pub binding_uid: u32,
    pub age_fixed_ticks: u32,
}

impl From<RobotsCharacterAttachment> for RobotsCharacterAttachmentRuntimeEntry {
    fn from(attachment: RobotsCharacterAttachment) -> Self {
        Self {
            kind: attachment.kind,
            resource_uid: attachment.resource_uid,
            binding_uid: attachment.binding_uid,
            age_fixed_ticks: 0,
        }
    }
}

impl PartialEq<RobotsCharacterAttachment> for RobotsCharacterAttachmentRuntimeEntry {
    fn eq(&self, attachment: &RobotsCharacterAttachment) -> bool {
        self.kind == attachment.kind
            && self.resource_uid == attachment.resource_uid
            && self.binding_uid == attachment.binding_uid
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RobotsCharacterAttachmentRuntimeState {
    pub attachments: Vec<RobotsCharacterAttachmentRuntimeEntry>,
    /// Native vslot +0xA8 (`0x00403B50`) requests. Keep the four serialized
    /// arguments intact: arg2 may be an explicit binding UID, while arg3 selects
    /// one of the native built-in binding sets when arg2 is -1.
    pub skeleton_particle_requests: Vec<[u32; 4]>,
    /// Native vslot +0xAC (`0x00403AF0`) only has a presentation side effect for
    /// effect id 0. Count the requests deterministically; GUI/UE owns rendering.
    pub special_effect_zero_requests: u32,
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
        event_type::ATTACH_PARTICLES_TO_SKELETON => {
            let args = [
                event.native_arg_word(0).unwrap_or(u32::MAX),
                event.native_arg_word(1).unwrap_or(u32::MAX),
                event.native_arg_word(2).unwrap_or(u32::MAX),
                event.native_arg_word(3).unwrap_or(0),
            ];
            // 0x00403B50 releases/validates arg0+arg1 and creates nothing unless
            // both resources are valid. Preserve arg2/arg3 for explicit/built-in bindings.
            (args[0] != u32::MAX && args[1] != u32::MAX)
                .then_some(RobotsCharacterAttachmentAction::AttachParticlesToSkeleton { args })
        }
        event_type::ATTACH_SPECIAL_EFFECT => {
            // Generic dispatcher already rejects -1; common +0xAC 0x00403AF0
            // has an actual side effect only for id 0.
            (event.native_arg_word(0) == Some(0))
                .then_some(RobotsCharacterAttachmentAction::TriggerSpecialEffectZero)
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
            state.attachments.push(attachment.into());
            true
        }
        RobotsCharacterAttachmentAction::AttachParticlesToSkeleton { args } => {
            state.skeleton_particle_requests.push(args);
            true
        }
        RobotsCharacterAttachmentAction::TriggerSpecialEffectZero => {
            state.special_effect_zero_requests =
                state.special_effect_zero_requests.saturating_add(1);
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

pub fn advance_character_attachments_fixed_tick(
    state: &mut RobotsCharacterAttachmentRuntimeState,
) {
    for attachment in &mut state.attachments {
        attachment.age_fixed_ticks = attachment.age_fixed_ticks.saturating_add(1);
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
    fn skeleton_and_special_effect_events_preserve_native_common_slot_semantics() {
        let mut state = RobotsCharacterAttachmentRuntimeState::default();
        let skeleton = [0x1100_0007, 0x1200_0001, u32::MAX, 2];
        assert_eq!(
            apply_ai_character_attachment_event(
                &mut state,
                event(event_type::ATTACH_PARTICLES_TO_SKELETON, &skeleton),
            ),
            Some(RobotsCharacterAttachmentAction::AttachParticlesToSkeleton { args: skeleton })
        );
        assert_eq!(state.skeleton_particle_requests, vec![skeleton]);

        assert_eq!(
            apply_ai_character_attachment_event(
                &mut state,
                event(event_type::ATTACH_SPECIAL_EFFECT, &[1]),
            ),
            None
        );
        assert_eq!(
            apply_ai_character_attachment_event(
                &mut state,
                event(event_type::ATTACH_SPECIAL_EFFECT, &[0]),
            ),
            Some(RobotsCharacterAttachmentAction::TriggerSpecialEffectZero)
        );
        assert_eq!(state.special_effect_zero_requests, 1);
    }

    #[test]
    fn fixed_step_age_starts_at_zero_and_advances_only_existing_attachments() {
        let mut state = RobotsCharacterAttachmentRuntimeState::default();
        apply_ai_character_attachment_event(
            &mut state,
            event(event_type::ATTACH_SWOOSH, &[0x1900_0018, u32::MAX]),
        )
        .expect("attach swoosh");
        assert_eq!(state.attachments[0].age_fixed_ticks, 0);

        advance_character_attachments_fixed_tick(&mut state);
        advance_character_attachments_fixed_tick(&mut state);
        assert_eq!(state.attachments[0].age_fixed_ticks, 2);

        apply_ai_character_attachment_event(
            &mut state,
            event(event_type::ATTACH_PARTICLE, &[0x1100_0007, u32::MAX]),
        )
        .expect("attach particle");
        assert_eq!(state.attachments[0].age_fixed_ticks, 2);
        assert_eq!(state.attachments[1].age_fixed_ticks, 0);
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
