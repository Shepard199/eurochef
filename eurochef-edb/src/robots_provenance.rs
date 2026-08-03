use serde::{Serialize, Serializer};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HexU32(pub u32);

impl Serialize for HexU32 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("0x{:08X}", self.0))
    }
}

pub const ROBOTS_PC_EXE_SHA256: &str =
    "8fefaa09767d9d1e76ca8c023e4e60720808cc529fc3abe1ff6d863d93f668bc";

const SCRIPT_CREATE_KNOWN_BITS: u32 = 0x0000_0103;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct RobotsScriptCreateFlags {
    pub raw: u32,
    pub bit0_xitem_state_list: bool,
    pub bit1_deferred_trigger_flag: bool,
    pub bit8_registration_mask: bool,
    pub created_xitem_state_mask: u32,
    pub created_xitem_registration_mask: u32,
    pub deferred_trigger_flag_or_mask: u32,
    pub created_xitem_class_byte: u8,
    pub unknown_bits: u32,
}

pub fn decode_script_create_flags(raw: u32) -> RobotsScriptCreateFlags {
    let bit0 = raw & 0x0000_0001 != 0;
    let bit1 = raw & 0x0000_0002 != 0;
    let bit8 = raw & 0x0000_0100 != 0;

    RobotsScriptCreateFlags {
        raw,
        bit0_xitem_state_list: bit0,
        bit1_deferred_trigger_flag: bit1,
        bit8_registration_mask: bit8,
        created_xitem_state_mask: 0x0000_0004 | if bit0 { 0x0000_0800 } else { 0 },
        created_xitem_registration_mask: if bit8 { 0x0000_0002 } else { 0 },
        deferred_trigger_flag_or_mask: if bit1 { 0x0010_0000 } else { 0 },
        created_xitem_class_byte: 0x14,
        unknown_bits: raw & !SCRIPT_CREATE_KNOWN_BITS,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct RobotsScriptSpawnFunctionProvenance {
    pub address: HexU32,
    pub role: &'static str,
}

pub const ROBOTS_SCRIPT_SPAWN_CHAIN: &[RobotsScriptSpawnFunctionProvenance] = &[
    RobotsScriptSpawnFunctionProvenance {
        address: HexU32(0x0040_4360),
        role: "explicit-transform Script-XItem wrapper",
    },
    RobotsScriptSpawnFunctionProvenance {
        address: HexU32(0x0040_4480),
        role: "owner-transform Script-XItem wrapper",
    },
    RobotsScriptSpawnFunctionProvenance {
        address: HexU32(0x0040_43A0),
        role: "core Script-XItem spawn",
    },
    RobotsScriptSpawnFunctionProvenance {
        address: HexU32(0x0044_3D40),
        role: "generic XItem and handler creator",
    },
    RobotsScriptSpawnFunctionProvenance {
        address: HexU32(0x004E_8C68),
        role: "resource-object factory and compatibility lookup",
    },
    RobotsScriptSpawnFunctionProvenance {
        address: HexU32(0x004E_805D),
        role: "attach resolved resource object",
    },
    RobotsScriptSpawnFunctionProvenance {
        address: HexU32(0x004E_9A1C),
        role: "XItem manager registration",
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct RobotsScriptCreatorProvenance {
    pub script_hashcode: HexU32,
    pub script_name: &'static str,
    pub script_definition_file: Option<HexU32>,
    pub creator_class: &'static str,
    pub creator_method: HexU32,
    pub immediate_site: HexU32,
    pub spawn_callsite: HexU32,
    pub spawn_wrapper: HexU32,
    pub selector_or_gate: &'static str,
    pub proof_status: &'static str,
}

pub const ROBOTS_SCRIPT_CREATOR_PROVENANCE: &[RobotsScriptCreatorProvenance] = &[
    RobotsScriptCreatorProvenance {
        script_hashcode: HexU32(0x0400_0246),
        script_name: "HT_Script_FootDustCloud",
        script_definition_file: Some(HexU32(0x0100_005D)),
        creator_class: "XItemHandler_Player family",
        creator_method: HexU32(0x004B_0690),
        immediate_site: HexU32(0x004B_0851),
        spawn_callsite: HexU32(0x004B_0882),
        spawn_wrapper: HexU32(0x0040_4360),
        selector_or_gate: "foot/bone query succeeds and handler+0x6DE != 12",
        proof_status: "PROVEN_STATIC_AND_SERIALIZED",
    },
    RobotsScriptCreatorProvenance {
        script_hashcode: HexU32(0x0400_02F9),
        script_name: "o01_pick long pickup spot effect",
        script_definition_file: Some(HexU32(0x0100_0003)),
        creator_class: "XItemHandler_Pickup",
        creator_method: HexU32(0x0041_4100),
        immediate_site: HexU32(0x0041_4165),
        spawn_callsite: HexU32(0x0041_4171),
        spawn_wrapper: HexU32(0x0040_4480),
        selector_or_gate: "owner XItem+0x264 != 45",
        proof_status: "PROVEN_STATIC_SERIALIZED_AND_RUNTIME_IDENTITY",
    },
    RobotsScriptCreatorProvenance {
        script_hashcode: HexU32(0x0400_02FA),
        script_name: "o01_pick alternate long pickup spot effect",
        script_definition_file: Some(HexU32(0x0100_0003)),
        creator_class: "XItemHandler_Pickup",
        creator_method: HexU32(0x0041_4100),
        immediate_site: HexU32(0x0041_416C),
        spawn_callsite: HexU32(0x0041_4171),
        spawn_wrapper: HexU32(0x0040_4480),
        selector_or_gate: "owner XItem+0x264 == 45",
        proof_status: "PROVEN_STATIC_AND_SERIALIZED",
    },
    RobotsScriptCreatorProvenance {
        script_hashcode: HexU32(0x0400_02FB),
        script_name: "o01_pick short pickup particle effect",
        script_definition_file: Some(HexU32(0x0100_0003)),
        creator_class: "XItemHandler_Pickup",
        creator_method: HexU32(0x0041_3BB0),
        immediate_site: HexU32(0x0041_3C0C),
        spawn_callsite: HexU32(0x0041_3C18),
        spawn_wrapper: HexU32(0x0040_4480),
        selector_or_gate: "owner XItem+0x264 != 45 plus pickup cooldown gates",
        proof_status: "PROVEN_STATIC_SERIALIZED_AND_RUNTIME_IDENTITY",
    },
    RobotsScriptCreatorProvenance {
        script_hashcode: HexU32(0x0400_02FC),
        script_name: "o01_pick alternate short pickup particle effect",
        script_definition_file: Some(HexU32(0x0100_0003)),
        creator_class: "XItemHandler_Pickup",
        creator_method: HexU32(0x0041_3BB0),
        immediate_site: HexU32(0x0041_3C13),
        spawn_callsite: HexU32(0x0041_3C18),
        spawn_wrapper: HexU32(0x0040_4480),
        selector_or_gate: "owner XItem+0x264 == 45 plus pickup cooldown gates",
        proof_status: "PROVEN_STATIC_AND_SERIALIZED",
    },
    RobotsScriptCreatorProvenance {
        script_hashcode: HexU32(0x0400_02F9),
        script_name: "o01_pick long effect reused by electricity explosion",
        script_definition_file: Some(HexU32(0x0100_0003)),
        creator_class: "XItemHandler_ElectricityExplosion",
        creator_method: HexU32(0x004D_BF90),
        immediate_site: HexU32(0x004D_C09A),
        spawn_callsite: HexU32(0x004D_C09F),
        spawn_wrapper: HexU32(0x0040_4480),
        selector_or_gate: "electricity-explosion state permits helper and owner XItem+0x264 != 45",
        proof_status: "PROVEN_STATIC_AND_SERIALIZED",
    },
    RobotsScriptCreatorProvenance {
        script_hashcode: HexU32(0x0400_02FA),
        script_name: "o01_pick alternate long effect reused by electricity explosion",
        script_definition_file: Some(HexU32(0x0100_0003)),
        creator_class: "XItemHandler_ElectricityExplosion",
        creator_method: HexU32(0x004D_BF90),
        immediate_site: HexU32(0x004D_C0D1),
        spawn_callsite: HexU32(0x004D_C0D6),
        spawn_wrapper: HexU32(0x0040_4480),
        selector_or_gate: "electricity-explosion state permits helper and owner XItem+0x264 == 45",
        proof_status: "PROVEN_STATIC_AND_SERIALIZED",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_lzbox_0x103() {
        let decoded = decode_script_create_flags(0x0000_0103);
        assert!(decoded.bit0_xitem_state_list);
        assert!(decoded.bit1_deferred_trigger_flag);
        assert!(decoded.bit8_registration_mask);
        assert_eq!(decoded.created_xitem_state_mask, 0x0000_0804);
        assert_eq!(decoded.created_xitem_registration_mask, 0x0000_0002);
        assert_eq!(decoded.deferred_trigger_flag_or_mask, 0x0010_0000);
        assert_eq!(decoded.created_xitem_class_byte, 0x14);
        assert_eq!(decoded.unknown_bits, 0);
    }

    #[test]
    fn preserves_unresolved_script_create_bits() {
        assert_eq!(
            decode_script_create_flags(0x8000_0103).unknown_bits,
            0x8000_0000
        );
    }

    #[test]
    fn records_both_known_creators_for_f9() {
        assert_eq!(
            ROBOTS_SCRIPT_CREATOR_PROVENANCE
                .iter()
                .filter(|entry| entry.script_hashcode.0 == 0x0400_02F9)
                .count(),
            2
        );
    }
}
