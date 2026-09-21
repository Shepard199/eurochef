use serde::Serialize;

use super::events::RobotsHandlerScriptCommandFamily;

const HANDLER_SCRIPT_COMMAND_CENSUS: &str =
    include_str!("data/xitem_handler_script_command_slots.tsv");
const EXPECTED_CLASSIFICATION: &str = "script_command_family_target";
const EXPECTED_PROOF_STATUS: &str = "xitemhandler_slot23_script_command_dispatch";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsHandlerScriptCommandClassContract {
    pub class_name: &'static str,
    pub parent_class_name: Option<&'static str>,
    pub native_vtable: u32,
    pub vtable_proof: &'static str,
    pub native_command_target: u32,
    pub family: RobotsHandlerScriptCommandFamily,
}

pub fn robots_handler_script_command_class_contracts(
) -> impl Iterator<Item = RobotsHandlerScriptCommandClassContract> {
    HANDLER_SCRIPT_COMMAND_CENSUS
        .lines()
        .skip(1)
        .filter(|line| !line.is_empty())
        .map(parse_contract_row)
}

pub fn robots_handler_script_command_contract_for_class(
    class_name: &str,
) -> Option<RobotsHandlerScriptCommandClassContract> {
    robots_handler_script_command_class_contracts()
        .find(|contract| contract.class_name == class_name)
}

pub fn robots_handler_script_command_family_for_class(
    class_name: &str,
) -> Option<RobotsHandlerScriptCommandFamily> {
    robots_handler_script_command_contract_for_class(class_name).map(|contract| contract.family)
}

fn parse_contract_row(line: &'static str) -> RobotsHandlerScriptCommandClassContract {
    let mut fields = line.split('\t');
    let class_name = fields
        .next()
        .expect("Handler census row missing class name");
    let parent_class_name = fields
        .next()
        .expect("Handler census row missing parent class name");
    let native_vtable = parse_hex_u32(
        fields
            .next()
            .expect("Handler census row missing native vtable"),
    );
    let vtable_proof = fields
        .next()
        .expect("Handler census row missing vtable proof");
    let native_command_target = parse_hex_u32(
        fields
            .next()
            .expect("Handler census row missing +0x5C target"),
    );
    let classification = fields
        .next()
        .expect("Handler census row missing classification");
    let proof_status = fields
        .next()
        .expect("Handler census row missing proof status");
    assert!(
        fields.next().is_none(),
        "Handler census row has unexpected extra fields: {class_name}"
    );
    assert_eq!(
        classification, EXPECTED_CLASSIFICATION,
        "unexpected Handler census classification for {class_name}"
    );
    assert_eq!(
        proof_status, EXPECTED_PROOF_STATUS,
        "unexpected Handler census proof status for {class_name}"
    );
    assert!(
        matches!(vtable_proof, "exact" | "inferred_constructor_window"),
        "unexpected Handler vtable proof for {class_name}: {vtable_proof}"
    );
    let family = RobotsHandlerScriptCommandFamily::from_native_target(native_command_target)
        .unwrap_or_else(|| {
            panic!(
                "unrecovered Handler +0x5C target 0x{native_command_target:08X} for {class_name}"
            )
        });

    RobotsHandlerScriptCommandClassContract {
        class_name,
        parent_class_name: (!parent_class_name.is_empty()).then_some(parent_class_name),
        native_vtable,
        vtable_proof,
        native_command_target,
        family,
    }
}

fn parse_hex_u32(value: &str) -> u32 {
    let value = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .expect("Handler census native address lacks 0x prefix");
    u32::from_str_radix(value, 16).expect("Handler census native address is not hexadecimal")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn census_embeds_all_124_handler_classes_with_unique_names() {
        let contracts = robots_handler_script_command_class_contracts().collect::<Vec<_>>();
        assert_eq!(contracts.len(), 124);

        let mut names = std::collections::BTreeSet::new();
        for contract in &contracts {
            assert!(
                names.insert(contract.class_name),
                "duplicate {}",
                contract.class_name
            );
            assert_eq!(
                contract.family.native_target(),
                contract.native_command_target
            );
        }
    }

    #[test]
    fn census_family_histogram_matches_native_slot23_counts() {
        let mut counts = HashMap::<RobotsHandlerScriptCommandFamily, usize>::new();
        for contract in robots_handler_script_command_class_contracts() {
            *counts.entry(contract.family).or_default() += 1;
        }

        assert_eq!(
            counts.get(&RobotsHandlerScriptCommandFamily::Generic),
            Some(&101)
        );
        assert_eq!(
            counts.get(&RobotsHandlerScriptCommandFamily::Cutscene),
            Some(&1)
        );
        assert_eq!(
            counts.get(&RobotsHandlerScriptCommandFamily::Door),
            Some(&1)
        );
        assert_eq!(
            counts.get(&RobotsHandlerScriptCommandFamily::MagneticScript),
            Some(&1)
        );
        assert_eq!(
            counts.get(&RobotsHandlerScriptCommandFamily::ArcadeShop),
            Some(&2)
        );
        assert_eq!(
            counts.get(&RobotsHandlerScriptCommandFamily::Hazard),
            Some(&1)
        );
        assert_eq!(
            counts.get(&RobotsHandlerScriptCommandFamily::Interactive),
            Some(&7)
        );
        assert_eq!(
            counts.get(&RobotsHandlerScriptCommandFamily::Light),
            Some(&1)
        );
        assert_eq!(
            counts.get(&RobotsHandlerScriptCommandFamily::Pickup),
            Some(&1)
        );
        assert_eq!(
            counts.get(&RobotsHandlerScriptCommandFamily::ScriptLifecycle),
            Some(&6)
        );
        assert_eq!(
            counts.get(&RobotsHandlerScriptCommandFamily::BossExec),
            Some(&1)
        );
        assert_eq!(
            counts.get(&RobotsHandlerScriptCommandFamily::BossSewerCanon),
            Some(&1)
        );
        assert_eq!(counts.len(), 12);
    }

    #[test]
    fn representative_owner_classes_select_exact_native_families() {
        let cases = [
            (
                "XItemHandler_Cutscene",
                RobotsHandlerScriptCommandFamily::Cutscene,
            ),
            ("XItemHandler_Door", RobotsHandlerScriptCommandFamily::Door),
            (
                "XItemHandler_Shop",
                RobotsHandlerScriptCommandFamily::ArcadeShop,
            ),
            (
                "XItemHandler_ArcadeMachine",
                RobotsHandlerScriptCommandFamily::ArcadeShop,
            ),
            (
                "XItemHandler_FixSwitch",
                RobotsHandlerScriptCommandFamily::Interactive,
            ),
            (
                "XItemHandler_Sweeper_Boss",
                RobotsHandlerScriptCommandFamily::ScriptLifecycle,
            ),
            (
                "XItemHandler_Player",
                RobotsHandlerScriptCommandFamily::Generic,
            ),
            (
                "XItemHandler_Monster",
                RobotsHandlerScriptCommandFamily::Generic,
            ),
            (
                "XItemHandler_BossSewerCanon",
                RobotsHandlerScriptCommandFamily::BossSewerCanon,
            ),
        ];
        for (class_name, expected) in cases {
            assert_eq!(
                robots_handler_script_command_family_for_class(class_name),
                Some(expected),
                "{class_name}"
            );
        }
        assert_eq!(
            robots_handler_script_command_family_for_class("XItemHandler_NotInRobotsExe"),
            None
        );
    }

    #[test]
    fn exact_vtable_provenance_is_preserved_for_known_classes() {
        let base = robots_handler_script_command_contract_for_class("XItemHandler")
            .expect("base Handler contract");
        assert_eq!(base.native_vtable, 0x005D_D2B8);
        assert_eq!(base.vtable_proof, "exact");
        assert_eq!(base.parent_class_name, None);

        let boss = robots_handler_script_command_contract_for_class("XItemHandler_BossExec")
            .expect("BossExec Handler contract");
        assert_eq!(boss.native_vtable, 0x005E_FC20);
        assert_eq!(boss.parent_class_name, Some("XItemHandler_Script"));
        assert_eq!(boss.native_command_target, 0x004C_9440);
    }
}
