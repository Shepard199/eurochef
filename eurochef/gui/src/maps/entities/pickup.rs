use eurochef_edb::Hashcode;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickupVisual {
    pub file: Hashcode,
    pub object: Hashcode,
    pub scale: f32,
    pub provenance: &'static str,
}

const O01_PICKUPS: Hashcode = 0x0100_0003;
const H01_MAIN: Hashcode = 0x0100_0006;

const fn visual(
    file: Hashcode,
    object: Hashcode,
    scale: f32,
    provenance: &'static str,
) -> PickupVisual {
    PickupVisual {
        file,
        object,
        scale,
        provenance,
    }
}

const fn entity(
    file: Hashcode,
    hashcode: Hashcode,
    scale: f32,
    provenance: &'static str,
) -> PickupVisual {
    visual(file, hashcode, scale, provenance)
}

const fn script(hashcode: Hashcode, provenance: &'static str) -> PickupVisual {
    visual(O01_PICKUPS, hashcode, 1.0, provenance)
}

fn blueprint_piece_visual(trigger_data: &[Option<u32>]) -> Option<PickupVisual> {
    // Robots.exe 0x00489F10 indexes the exact table at 0x005EA9FC by
    // XTrigger_Pickup+0x6C. Serialized raw type 0x1D stores that family
    // selector in data[0]; data[1] is the gameplay piece index.
    let family = trigger_data.first().copied().flatten()? as usize;
    Some(match family {
        0 => script(0x0400_0026, "Watchbot blueprint-piece pickup Script"),
        1 => script(
            0x0400_0027,
            "Watchbot RemoteControl blueprint-piece pickup Script",
        ),
        2 => script(0x0400_0028, "Watchbot Glide blueprint-piece pickup Script"),
        // Named in HashDB, but absent from the shipped PC o01_pick.edb catalog.
        3 => entity(
            O01_PICKUPS,
            0x0200_0088,
            1.0,
            "Tinkerfist blueprint-piece PC Script absent; common Blueprint world entity",
        ),
        4 => script(0x0400_002B, "Magnagun blueprint-piece pickup Script"),
        5 => script(0x0400_002D, "Electroshot blueprint-piece pickup Script"),
        6 => script(0x0400_002A, "Scrambler blueprint-piece pickup Script"),
        7 => script(0x0400_002C, "Magnagrip blueprint-piece pickup Script"),
        _ => return None,
    })
}

/// Resolves serialized Robots EXGeoTriggerType.trig_type values, not runtime
/// XTrigger registry IDs. The exact bridge is the 16-byte table at
/// Robots.exe 0x0061F380, consumed by XTriggerManager::CreateTriggers at
/// 0x0044B614..0x0044B632. Its +4 dword maps these fifteen serialized
/// Pickup layouts to runtime Pickup types 41..55.
pub fn robots_pickup_visual(
    trigger_type: u32,
    trigger_data: &[Option<u32>],
) -> Option<PickupVisual> {
    Some(match trigger_type {
        // 0x19 -> runtime 44 -> HT_Pickup_Scrap
        0x19 => entity(
            O01_PICKUPS,
            0x0200_01C8,
            1.0,
            "serialized 0x19 native Scrap branch; scrap nut world entity",
        ),
        // 0x1A -> runtime 47 -> HT_Pickup_HealthReplenish
        0x1A => script(
            0x0400_0022,
            "serialized 0x1A native HealthReplenish branch; complete Health/EnergyCell Script",
        ),
        // 0x1B -> runtime 45 -> HT_Pickup_GoldenScrap
        0x1B => entity(
            O01_PICKUPS,
            0x0200_01CC,
            1.0,
            "serialized 0x1B native GoldenScrap branch; gold nut world entity",
        ),
        // 0x1C -> runtime 48. Gameplay replaces the handler datum with
        // HT_Upgrade[data0], but the world presentation is the TrickChip carrier.
        0x1C => script(
            0x0400_002E,
            "serialized 0x1C native TrickChip/upgrade carrier; complete TrickChip Script",
        ),
        // 0x1D -> runtime 41 -> exact blueprint family selector table.
        0x1D => return blueprint_piece_visual(trigger_data),
        // 0x1E -> runtime 42 -> HT_Pickup_Goldprint.
        0x1E => entity(
            O01_PICKUPS,
            0x0200_0088,
            1.0,
            "serialized 0x1E native Goldprint; PC world Script absent, common Blueprint entity",
        ),
        // 0x29 -> runtime 43 -> HT_Pickup_ToolkitPiece.
        0x29 => script(0x0400_0063, "complete ToolkitPiece pickup Script"),
        // 0x3E..0x52 -> runtime 49..55 story pickups.
        0x3E => script(0x0400_012D, "complete ClockPiece pickup Script"),
        0x3F => script(0x0400_012E, "complete Parcel pickup Script"),
        0x40 => script(0x0400_012F, "complete Cargo pickup Script"),
        0x41 => script(0x0400_0130, "complete SparePart pickup Script"),
        0x43 => script(0x0400_012C, "complete FendersHead pickup Script"),
        // HT_Script_Pickup_Fuel is named in HashDB but absent from the shipped
        // PC Script corpus. Use the exact named Fuel asset from h01_main,
        // normalized from HUD authoring scale for map preview.
        0x47 => entity(
            H01_MAIN,
            0x0200_00B0,
            0.1,
            "serialized 0x47 native Fuel; PC world Script absent, named Fuel asset fallback",
        ),
        0x52 => script(0x0400_0232, "complete OldbotKeys pickup Script"),
        // 0x53 -> runtime 46 -> HT_Pickup_Scrap with fixed datum 25.
        0x53 => entity(
            O01_PICKUPS,
            0x0200_01C8,
            1.0,
            "serialized 0x53 native Scrap fixed-25 branch; scrap nut world entity",
        ),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_serialized_pickup_layout_resolves_without_fake_uid_data() {
        let layouts = [
            0x19, 0x1A, 0x1B, 0x1C, 0x1E, 0x29, 0x3E, 0x3F, 0x40, 0x41, 0x43, 0x47, 0x52, 0x53,
        ];
        for trigger_type in layouts {
            assert!(
                robots_pickup_visual(trigger_type, &[None; 16]).is_some(),
                "serialized trigger type 0x{trigger_type:02X}"
            );
        }
    }

    #[test]
    fn blueprint_layout_uses_native_family_selector_table() {
        let expected = [
            0x0400_0026,
            0x0400_0027,
            0x0400_0028,
            0x0200_0088,
            0x0400_002B,
            0x0400_002D,
            0x0400_002A,
            0x0400_002C,
        ];
        for (family, object) in expected.into_iter().enumerate() {
            let visual = robots_pickup_visual(0x1D, &[Some(family as u32), Some(15)])
                .expect("valid native blueprint family");
            assert_eq!(visual.object, object, "family={family}");
        }
        assert!(robots_pickup_visual(0x1D, &[Some(8), Some(0)]).is_none());
    }

    #[test]
    fn serialized_layout_semantics_ignore_quantity_as_a_pickup_uid() {
        assert_eq!(
            robots_pickup_visual(0x19, &[Some(50)]).unwrap().object,
            0x0200_01C8
        );
        assert_eq!(
            robots_pickup_visual(0x1A, &[Some(50)]).unwrap().object,
            0x0400_0022
        );
        assert_eq!(
            robots_pickup_visual(0x1B, &[Some(50)]).unwrap().object,
            0x0200_01CC
        );
    }

    #[test]
    fn goldprint_never_uses_the_shop_arrow() {
        let visual = robots_pickup_visual(0x1E, &[None; 16]).unwrap();
        assert_eq!(visual.file, O01_PICKUPS);
        assert_eq!(visual.object, 0x0200_0088);
        assert_ne!(visual.object, 0x0200_0089);
    }

    #[test]
    fn non_pickup_trigger_is_not_promoted() {
        assert_eq!(robots_pickup_visual(80, &[None; 16]), None);
    }
}
