use eurochef_edb::Hashcode;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickupVisual {
    pub file: Hashcode,
    pub entity: Hashcode,
    pub scale: f32,
    pub provenance: &'static str,
}

const O01_PICKUPS: Hashcode = 0x0100_0003;
const H05_SHOP: Hashcode = 0x0100_0022;

const fn visual(
    file: Hashcode,
    entity: Hashcode,
    scale: f32,
    provenance: &'static str,
) -> PickupVisual {
    PickupVisual {
        file,
        entity,
        scale,
        provenance,
    }
}

pub fn robots_pickup_visual(
    trigger_type: u32,
    trigger_data: &[Option<u32>],
) -> Option<PickupVisual> {
    if trigger_type == 0x1D {
        return Some(visual(
            O01_PICKUPS,
            0x0200_0088,
            1.0,
            "serialized XTrigger_Pickup blueprint fallback",
        ));
    }

    if !matches!(
        trigger_type,
        25..=28 | 30 | 41 | 62..=65 | 67 | 71 | 82 | 83
    ) {
        return None;
    }

    let raw_kind = trigger_data.first().copied().flatten()?;
    let kind = if raw_kind <= 0xFF {
        0x4700_0000 | raw_kind
    } else {
        raw_kind
    };
    Some(match kind {
        0x4700_0001 => visual(O01_PICKUPS, 0x0200_01C8, 1.0, "normal scrap world entity"),
        0x4700_0002 => visual(O01_PICKUPS, 0x0200_01CC, 1.0, "golden scrap world entity"),
        0x4700_0003 => visual(
            O01_PICKUPS,
            0x8200_000B,
            1.0,
            "HT_Script_Pickup_Health_EnergyCell local world assembly",
        ),
        0x4700_0004..=0x4700_000B => {
            visual(O01_PICKUPS, 0x0200_0088, 1.0, "blueprint world entity")
        }
        0x4700_000C => visual(
            O01_PICKUPS,
            0x0200_0088,
            1.0,
            "Goldprint PC script is absent; uses the shipped Blueprint-family world entity while gold effect/highlight remain separate runtime Scripts",
        ),
        0x4700_000D => visual(O01_PICKUPS, 0x0200_0199, 1.0, "TrickChip world entity"),
        0x4700_000E..=0x4700_0015 => {
            visual(O01_PICKUPS, 0x0200_0088, 1.0, "blueprint-piece world entity")
        }
        0x4700_0016 => visual(O01_PICKUPS, 0x0200_0087, 1.0, "toolkit inventory entity"),
        0x4700_0018 => visual(
            H05_SHOP,
            0x8200_0016,
            0.05,
            "shipped ScrapMaxMultiplier2 shop-model assembly",
        ),
        0x4700_0019 => visual(
            H05_SHOP,
            0x8200_0017,
            0.05,
            "shipped ScrapMaxMultiplier5 shop-model assembly",
        ),
        0x4700_001B => visual(
            H05_SHOP,
            0x8200_0018,
            0.05,
            "ScrapMultiplier25 PC family alias to shipped ScrapMultiplier50 model",
        ),
        0x4700_001C => visual(
            H05_SHOP,
            0x8200_0018,
            0.05,
            "shipped ScrapMultiplier50 shop-model assembly",
        ),
        0x4700_001D => visual(
            H05_SHOP,
            0x8200_000A,
            0.05,
            "BatteryMaxMultiplier50 PC family alias to shipped BatteryMaxMultiplier100 model",
        ),
        0x4700_001E => visual(
            H05_SHOP,
            0x8200_000A,
            0.05,
            "shipped BatteryMaxMultiplier100 shop-model entity",
        ),
        0x4700_001F..=0x4700_0022 | 0x4700_0024 | 0x4700_002E..=0x4700_0031 => {
            visual(O01_PICKUPS, 0x0200_00BF, 1.0, "bonus-feature inventory entity")
        }
        0x4700_0023 => visual(
            O01_PICKUPS,
            0x8200_000B,
            1.0,
            "native HealthReplenish branch shares EnergyCell world assembly",
        ),
        0x4700_0025 => visual(O01_PICKUPS, 0x0200_00A3, 1.0, "clock-piece inventory entity"),
        0x4700_0026 => visual(O01_PICKUPS, 0x0200_00A5, 1.0, "parcel inventory entity"),
        0x4700_0027 => visual(O01_PICKUPS, 0x0200_00A4, 1.0, "cargo inventory entity"),
        0x4700_0028 => visual(O01_PICKUPS, 0x0200_00A6, 1.0, "spare-part inventory entity"),
        0x4700_0029 => visual(O01_PICKUPS, 0x0200_00A2, 1.0, "Fender head inventory entity"),
        0x4700_002A => visual(O01_PICKUPS, 0x0200_00B0, 1.0, "fuel inventory entity"),
        0x4700_002B => visual(O01_PICKUPS, 0x0200_01C9, 1.0, "silver scrap world entity"),
        0x4700_002C => visual(O01_PICKUPS, 0x0200_0185, 1.0, "Oldbot keys inventory entity"),
        0x4700_002D => visual(O01_PICKUPS, 0x0200_00D8, 1.0, "ElectroBomb inventory entity"),
        0x4700_0032 => visual(O01_PICKUPS, 0x0200_006C, 1.0, "Scrambler inventory entity"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remaining_named_pickups_have_resolved_visuals() {
        for kind in [
            0x03, 0x0C, 0x18, 0x19, 0x1B, 0x1C, 0x1D, 0x1E, 0x23,
        ] {
            assert!(robots_pickup_visual(25, &[Some(kind)]).is_some(), "kind=0x{kind:02X}");
            assert!(
                robots_pickup_visual(41, &[Some(0x4700_0000 | kind)]).is_some(),
                "full kind=0x{:08X}",
                0x4700_0000 | kind
            );
        }
    }

    #[test]
    fn goldprint_uses_blueprint_family_not_shop_arrow() {
        let visual = robots_pickup_visual(41, &[Some(0x4700_000C)]).unwrap();
        assert_eq!(visual.file, O01_PICKUPS);
        assert_eq!(visual.entity, 0x0200_0088);
        assert_ne!(visual.entity, 0x0200_0089);
        assert!(visual.provenance.contains("PC script is absent"));
    }

    #[test]
    fn non_pickup_trigger_is_not_promoted() {
        assert_eq!(robots_pickup_visual(80, &[Some(0x4700_0003)]), None);
    }
}
