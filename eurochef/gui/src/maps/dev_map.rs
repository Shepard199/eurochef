use eurochef_edb::Hashcode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RobotsDevMapInfo {
    pub level_id: u32,
    pub file: Hashcode,
    pub label: &'static str,
    pub source_edb: &'static str,
    pub evidence_role: &'static str,
}

const ROBOTS_DEV_MAPS: [RobotsDevMapInfo; 5] = [
    RobotsDevMapInfo {
        level_id: 55,
        file: 0x0100_0037,
        label: "Mechanics",
        source_edb: "m00_demo.edb",
        evidence_role:
            "camera, watchbot, interaction, pickup, platform, fan, fluid and magnetic mechanics",
    },
    RobotsDevMapInfo {
        level_id: 15,
        file: 0x0100_000F,
        label: "Enemies",
        source_edb: "m99_enem.edb",
        evidence_role: "monster subclasses, test monster, fish and transporter",
    },
    RobotsDevMapInfo {
        level_id: 160,
        file: 0x0100_00A0,
        label: "NPCs",
        source_edb: "m98_npcs.edb",
        evidence_role: "21 concentrated XTrigger_NPC samples",
    },
    RobotsDevMapInfo {
        level_id: 116,
        file: 0x0100_0074,
        label: "Ball",
        source_edb: "m00_ball.edb",
        evidence_role: "ball, fan, hazard and script mechanics",
    },
    RobotsDevMapInfo {
        level_id: 1,
        file: 0x0100_0001,
        label: "Empty Aunt Fanny House",
        source_edb: "m00_mapt.edb",
        evidence_role: "empty-map/player baseline",
    },
];

pub(crate) fn robots_dev_map_info(file: Hashcode) -> Option<&'static RobotsDevMapInfo> {
    ROBOTS_DEV_MAPS.iter().find(|entry| entry.file == file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_level_ids_map_to_the_proven_edb_uids() {
        assert_eq!(robots_dev_map_info(0x0100_0037).unwrap().level_id, 55);
        assert_eq!(robots_dev_map_info(0x0100_000F).unwrap().level_id, 15);
        assert_eq!(robots_dev_map_info(0x0100_00A0).unwrap().level_id, 160);
        assert_eq!(robots_dev_map_info(0x0100_0074).unwrap().level_id, 116);
        assert_eq!(robots_dev_map_info(0x0100_0001).unwrap().level_id, 1);
    }

    #[test]
    fn front_end_map_is_not_misclassified_as_a_dev_map() {
        assert!(robots_dev_map_info(0x0100_000D).is_none());
    }
}
