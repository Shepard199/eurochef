use super::super::{ProcessedMap, ProcessedTrigger};

pub const TYPE: u32 = 79;
pub const BLANK_SOUND: u32 = 0x1AF0_0001;

pub const SLOT_ACTIVATE: usize = 0;
pub const SLOT_DEACTIVATE: usize = 1;
pub const SLOT_ACTIVE_LOOP: usize = 2;
pub const SLOT_INACTIVE_LOOP: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectAudioProfile {
    pub linked_trigger_index: Option<usize>,
    pub activate: Option<u32>,
    pub deactivate: Option<u32>,
    pub active_loop: Option<u32>,
    pub inactive_loop: Option<u32>,
}

impl ObjectAudioProfile {
    pub fn sound(self, slot: usize) -> Option<u32> {
        match slot {
            SLOT_ACTIVATE => self.activate,
            SLOT_DEACTIVATE => self.deactivate,
            SLOT_ACTIVE_LOOP => self.active_loop,
            SLOT_INACTIVE_LOOP => self.inactive_loop,
            _ => None,
        }
    }

    pub fn playable_hashes(self) -> impl Iterator<Item = u32> {
        [
            self.activate,
            self.deactivate,
            self.active_loop,
            self.inactive_loop,
        ]
        .into_iter()
        .flatten()
    }
}

fn playable_sound(value: Option<u32>) -> Option<u32> {
    value.filter(|hash| !matches!(*hash, 0 | u32::MAX | BLANK_SOUND))
}

/// Native classes that override vtable slot +0xEC and search their first eight
/// links for an XTrigger_ObjectAudio descriptor through 0x0044CDB0.
pub fn is_consumer(trigger_type: u32) -> bool {
    matches!(trigger_type, 7 | 8 | 32 | 34 | 37 | 55 | 80)
}

/// Native per-class gates around the shared activate/deactivate sound calls.
/// Offsets are relative to XTrigger data at runtime base +0x6C.
fn enabled_for(trigger_type: u32, data: &[Option<u32>]) -> bool {
    let datum = |index: usize| data.get(index).copied().flatten().unwrap_or_default();
    match trigger_type {
        7 => datum(4) & 0x0000_0001 != 0,  // Fan vslot+0xF4 -> trigger+0x7C
        34 => datum(5) & 0x0000_0001 != 0, // FanHorizontal -> trigger+0x80
        8 => datum(7) & 0x0000_0100 != 0,  // Platform event handler +0x88
        32 => datum(3) & 0x0000_0200 != 0, // Hazard event handler +0x78
        37 => datum(2) & 0x0000_0800 != 0, // Lift event handler +0x74
        55 | 80 => true,                   // Clock / Vehicle call audio unconditionally
        _ => false,
    }
}

pub fn is_enabled(trigger: &ProcessedTrigger) -> bool {
    enabled_for(trigger.ttype, &trigger.data)
}

/// Class-local fallback values used when no ObjectAudio is linked or when its
/// selected field resolves to -1. These are the exact switch tables in the
/// shipped PC executable.
fn fallback_slots(trigger_type: u32) -> [Option<u32>; 4] {
    match trigger_type {
        // XTrigger_Fan / XTrigger_FanHorizontal, 0x004889B0.
        7 | 34 => [
            Some(0x1AF0_0321), // OBJECTS_GENERIC_ACTIVATE
            Some(0x1AF0_031E), // OBJECTS_FAN_SHUTOFF
            Some(0x1AF0_031D), // OBJECTS_FAN
            None,
        ],
        // XTrigger_Platform, 0x00486750.
        8 => [
            Some(0x1AF0_0357), // OBJECTS_SERVO_END
            Some(0x1AF0_0357), // OBJECTS_SERVO_END
            Some(0x1AF0_0358), // OBJECTS_SERVO_LOOP
            None,
        ],
        // XTrigger_Lift, 0x00486EC0.
        37 => [
            Some(0x1AF0_032C), // OBJECTS_LIFT_ACTIVATE
            Some(0x1AF0_032D), // OBJECTS_LIFT_DEACTIVATE
            Some(0x1AF0_032E), // OBJECTS_LIFT_LOOP
            None,
        ],
        // XTrigger_Clock, 0x0048AD50. Only the active loop has a native default.
        55 => [None, None, Some(0x1AF0_050D), None],
        // XTrigger_Vehicle, 0x00487470.
        80 => [
            Some(0x1AF0_0376), // OBJECTS_VILLAGE_TAXI_ACTIVATE
            Some(0x1AF0_0378), // OBJECTS_VILLAGE_TAXI_DEACTIVATE
            Some(0x1AF0_0377), // OBJECTS_VILLAGE_TAXI_ACTIVE
            None,
        ],
        // XTrigger_Hazard deliberately has no class-local fallback.
        32 => [None; 4],
        _ => [None; 4],
    }
}

fn linked_object_audio<'a>(
    map: &'a ProcessedMap,
    source: &ProcessedTrigger,
) -> Option<(usize, &'a ProcessedTrigger)> {
    source
        .links
        .iter()
        .take(8)
        .filter_map(|link| usize::try_from(*link).ok())
        .filter_map(|index| map.triggers.get(index).map(|trigger| (index, trigger)))
        .find(|(_, trigger)| trigger.ttype == TYPE)
}

fn merge_profile(
    linked_trigger_index: Option<usize>,
    data: Option<&[Option<u32>]>,
    fallback: [Option<u32>; 4],
) -> ObjectAudioProfile {
    let slot = |index: usize| {
        data.and_then(|values| values.get(index).copied().flatten())
            .and_then(|value| playable_sound(Some(value)))
            .or(fallback[index])
    };
    ObjectAudioProfile {
        linked_trigger_index,
        activate: slot(SLOT_ACTIVATE),
        deactivate: slot(SLOT_DEACTIVATE),
        active_loop: slot(SLOT_ACTIVE_LOOP),
        inactive_loop: slot(SLOT_INACTIVE_LOOP),
    }
}

/// Resolves the sound profile seen by a native object trigger. A linked
/// XTrigger_ObjectAudio overrides each of the four class defaults separately;
/// AA_BLANK and invalid references fall back exactly as the native forwarders do.
pub fn profile_for_source(map: &ProcessedMap, source_index: usize) -> Option<ObjectAudioProfile> {
    let source = map.triggers.get(source_index)?;
    if !is_consumer(source.ttype) {
        return None;
    }
    let fallback = fallback_slots(source.ttype);
    let linked = linked_object_audio(map, source);
    Some(merge_profile(
        linked.map(|(index, _)| index),
        linked.map(|(_, trigger)| trigger.data.as_slice()),
        fallback,
    ))
}

pub fn direct_profile(trigger: &ProcessedTrigger) -> Option<ObjectAudioProfile> {
    (trigger.ttype == TYPE).then(|| merge_profile(None, Some(&trigger.data), [None; 4]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_audio_slots_have_exact_native_roles() {
        let profile = merge_profile(
            Some(12),
            Some(&[
                Some(0x1AF0_1111),
                Some(0x1AF0_2222),
                Some(0x1AF0_3333),
                Some(0x1AF0_4444),
            ]),
            [None; 4],
        );
        assert_eq!(profile.sound(SLOT_ACTIVATE), Some(0x1AF0_1111));
        assert_eq!(profile.sound(SLOT_DEACTIVATE), Some(0x1AF0_2222));
        assert_eq!(profile.sound(SLOT_ACTIVE_LOOP), Some(0x1AF0_3333));
        assert_eq!(profile.sound(SLOT_INACTIVE_LOOP), Some(0x1AF0_4444));
    }

    #[test]
    fn blank_linked_fields_fall_back_per_slot() {
        let fallback = fallback_slots(37);
        let profile = merge_profile(
            Some(4),
            Some(&[Some(BLANK_SOUND), Some(0x1AF0_9001), None, Some(u32::MAX)]),
            fallback,
        );
        assert_eq!(profile.activate, Some(0x1AF0_032C));
        assert_eq!(profile.deactivate, Some(0x1AF0_9001));
        assert_eq!(profile.active_loop, Some(0x1AF0_032E));
        assert_eq!(profile.inactive_loop, None);
    }

    #[test]
    fn all_native_object_audio_consumers_are_registered() {
        for trigger_type in [7, 8, 32, 34, 37, 55, 80] {
            assert!(is_consumer(trigger_type), "type={trigger_type}");
        }
        assert!(!is_consumer(TYPE));
        assert!(!is_consumer(79 + 100));
    }

    #[test]
    fn vehicle_fallback_uses_activate_deactivate_active_order() {
        let profile = merge_profile(None, None, fallback_slots(80));
        assert_eq!(profile.activate, Some(0x1AF0_0376));
        assert_eq!(profile.deactivate, Some(0x1AF0_0378));
        assert_eq!(profile.active_loop, Some(0x1AF0_0377));
        assert_eq!(profile.inactive_loop, None);
    }

    #[test]
    fn real_object_audio_sound_catalog_when_requested() {
        let Ok(root) = std::env::var("ROBOTS_OBJECT_AUDIO_SOUND_ROOT") else {
            return;
        };
        let catalog =
            crate::sound_native::NativeSoundCatalog::load_pc_robots(std::path::Path::new(&root))
                .expect("load native Robots PC sound catalog");
        let hashes = [
            0x1AF0_0039,
            0x1AF0_003A,
            0x1AF0_02D7,
            0x1AF0_02D8,
            0x1AF0_02D9,
            0x1AF0_02F2,
            0x1AF0_02F3,
            0x1AF0_02F5,
            0x1AF0_0315,
            0x1AF0_0329,
            0x1AF0_032C,
            0x1AF0_032D,
            0x1AF0_032E,
            0x1AF0_0331,
            0x1AF0_0332,
            0x1AF0_0333,
            0x1AF0_0334,
            0x1AF0_0335,
            0x1AF0_0336,
            0x1AF0_033A,
            0x1AF0_0340,
            0x1AF0_034E,
            0x1AF0_0357,
            0x1AF0_0358,
            0x1AF0_0362,
            0x1AF0_036F,
            0x1AF0_0375,
        ];
        for hashcode in hashes {
            assert!(
                catalog.wave(hashcode, 0).is_some(),
                "ObjectAudio sound 0x{hashcode:08X}"
            );
        }
    }

    #[test]
    fn native_audio_enable_bits_match_each_consumer_layout() {
        let mut data = [None; 16];
        data[4] = Some(1);
        assert!(enabled_for(7, &data));
        assert!(!enabled_for(34, &data));

        data = [None; 16];
        data[5] = Some(1);
        assert!(enabled_for(34, &data));

        data = [None; 16];
        data[7] = Some(0x100);
        assert!(enabled_for(8, &data));

        data = [None; 16];
        data[3] = Some(0x200);
        assert!(enabled_for(32, &data));

        data = [None; 16];
        data[2] = Some(0x800);
        assert!(enabled_for(37, &data));
        assert!(enabled_for(55, &[None; 16]));
        assert!(enabled_for(80, &[None; 16]));
    }
}
