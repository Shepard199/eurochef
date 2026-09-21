use std::io::{Seek, SeekFrom};

use anyhow::Context;
use eurochef_edb::{binrw::BinReaderExt, edb::EdbFile};
use serde::Serialize;

use crate::spreadsheets::UXGeoSpreadsheet;

use super::inventory::{
    is_robots_inventory_uid, RobotsInventoryDefinition, RobotsInventoryState, ROBOTS_SCRAP_UID,
};
use super::player_items::{
    is_robots_player_item_uid, RobotsPlayerItemState, ROBOTS_PLAYER_ITEM_GATE_AGGREGATE,
};

pub const ROBOTS_SHOP_FILE_UID: u32 = 0x0100_0022;
pub const ROBOTS_SHOP_SPREADSHEET_UID: u32 = 0x1400_0004;
pub const ROBOTS_SHOP_GROUP_ROW_SIZE: usize = 0x44;
pub const ROBOTS_SHOP_ITEM_ROW_SIZE: usize = 0x24;
pub const ROBOTS_SHOP_SLOT_COUNT: usize = 8;
pub const ROBOTS_SHOP_EXIT_ITEM_UID: u32 = 0x4800_0007;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsShopSlot {
    pub item_uid: u32,
    pub price: i32,
}

impl RobotsShopSlot {
    pub const fn is_present(self) -> bool {
        self.item_uid != u32::MAX
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsShopGroupDefinition {
    pub shop_uid: u32,
    pub slots: [RobotsShopSlot; ROBOTS_SHOP_SLOT_COUNT],
}

impl RobotsShopGroupDefinition {
    /// Native `0x004ACEA0` copies sheet0 of `HT_SpreadSheet_Shops` as fixed
    /// 0x44-byte rows and removes rows whose first dword is `0xFFFFFFFF`.
    /// `0x004AA160` then walks eight `{item_uid, price}` pairs beginning at +4.
    pub fn from_native_words(words: [u32; 17]) -> Option<Self> {
        if words[0] == u32::MAX {
            return None;
        }
        let mut slots = [RobotsShopSlot {
            item_uid: u32::MAX,
            price: 0,
        }; ROBOTS_SHOP_SLOT_COUNT];
        for (slot, pair) in slots.iter_mut().zip(words[1..].chunks_exact(2)) {
            *slot = RobotsShopSlot {
                item_uid: pair[0],
                price: pair[1] as i32,
            };
        }
        Some(Self {
            shop_uid: words[0],
            slots,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsShopItemDefinition {
    pub uid: u32,
    /// Native dwords at +0x04..+0x20. Only consumers already proven in
    /// `0x004AA820/0x004AAE20` receive named accessors below.
    pub words_04_to_20: [u32; 8],
}

impl RobotsShopItemDefinition {
    /// Native `0x004ACC10` copies sheet1 of `HT_SpreadSheet_Shops` as fixed
    /// 0x24-byte rows and removes rows whose first dword is `0xFFFFFFFF`.
    pub fn from_native_words(words: [u32; 9]) -> Option<Self> {
        (words[0] != u32::MAX).then_some(Self {
            uid: words[0],
            words_04_to_20: [
                words[1], words[2], words[3], words[4], words[5], words[6], words[7], words[8],
            ],
        })
    }

    /// `0x004AA820` routes native +0x08 through the shop HUD visual-resource
    /// path. Keep the role generic until the downstream resource class is
    /// needed by the UE adapter.
    pub const fn display_resource_uid(self) -> u32 {
        self.words_04_to_20[1]
    }

    /// `0x004AA820` passes native +0x14 to the primary shop-item text path.
    pub const fn name_text_uid(self) -> u32 {
        self.words_04_to_20[4]
    }

    /// `0x004AA820` passes native +0x18 to the description text path.
    pub const fn description_text_uid(self) -> u32 {
        self.words_04_to_20[5]
    }

    /// `0x004AAE20` sign-extends the low 16 bits at native +0x1C and passes
    /// that value to the selected Player namespace container for preflight and
    /// purchase mutation.
    pub const fn purchase_quantity(self) -> i16 {
        self.words_04_to_20[6] as u16 as i16
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsShopOffer {
    pub shop_uid: u32,
    pub slot: usize,
    pub item_uid: u32,
    pub price: i32,
    pub purchase_quantity: i16,
    pub display_resource_uid: u32,
    pub name_text_uid: u32,
    pub description_text_uid: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsShopSpecialAction {
    ExitShop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsShopOfferAvailability {
    pub offer: RobotsShopOffer,
    pub current_scrap: i32,
    pub affordable: bool,
    pub native_item_preflight_result: Option<u32>,
    pub special_preflight_required: bool,
    pub special_action: Option<RobotsShopSpecialAction>,
    pub purchasable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsShopPurchaseOutcome {
    pub availability: RobotsShopOfferAvailability,
    pub committed: bool,
    pub special_action: Option<RobotsShopSpecialAction>,
    pub native_item_result: Option<u32>,
    pub native_scrap_remove_result: Option<u32>,
}

pub const ROBOTS_SHOP_EVENT_OPEN: u32 = 0x100;
pub const ROBOTS_SHOP_EVENT_CLOSE: u32 = 0x200;
pub const ROBOTS_SHOP_HANDLER_RETURN: u32 = 0x8000_0000;
pub const ROBOTS_SHOP_PLAYER_STATE_ACTIVE: u8 = 0x3b;
pub const ROBOTS_SHOP_PLAYER_STATE_DEFAULT: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RobotsShopTriggerEffect {
    OpenHud { shop_uid: u32, has_linked_npc: bool },
    CloseHud,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RobotsShopTriggerPlan {
    pub effects: Vec<RobotsShopTriggerEffect>,
    pub native_return: u32,
}

/// Exact `XTrigger_Shop +0x6C = 0x00482D50` event planner. Event `0x100`
/// opens the HUD unless ordinary gameplay is currently in GameWnd state 1/2/3;
/// a Cutscene sender bypasses that guard. Event `0x200` closes the HUD and is
/// evaluated independently, so combined flags preserve native open-then-close order.
pub fn plan_shop_trigger_event(
    event_flags: u32,
    sender_is_cutscene: bool,
    top_game_state: Option<i32>,
    shop_uid: u32,
    has_linked_npc: bool,
) -> RobotsShopTriggerPlan {
    let mut effects = Vec::new();
    if event_flags & ROBOTS_SHOP_EVENT_OPEN != 0 {
        let blocked = !sender_is_cutscene && matches!(top_game_state, Some(1 | 2 | 3));
        if !blocked {
            effects.push(RobotsShopTriggerEffect::OpenHud {
                shop_uid,
                has_linked_npc,
            });
        }
    }
    if event_flags & ROBOTS_SHOP_EVENT_CLOSE != 0 {
        effects.push(RobotsShopTriggerEffect::CloseHud);
    }
    RobotsShopTriggerPlan {
        effects,
        native_return: ROBOTS_SHOP_HANDLER_RETURN,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsShopOpenCompletion {
    pub player_state: Option<u8>,
    pub dispatch_linked_player_event: Option<u32>,
}

/// Post-open part of `0x00482DC0`. Linked Player notification happens even if
/// `0x004A98F0` fails; Player state `0x3B` requires both a successful HUD open
/// and a linked NPC.
pub fn complete_shop_open(
    open_succeeded: bool,
    has_linked_npc: bool,
    has_linked_player: bool,
) -> RobotsShopOpenCompletion {
    RobotsShopOpenCompletion {
        player_state: (open_succeeded && has_linked_npc).then_some(ROBOTS_SHOP_PLAYER_STATE_ACTIVE),
        dispatch_linked_player_event: has_linked_player.then_some(ROBOTS_SHOP_EVENT_OPEN),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RobotsShopHudTeardownPlan {
    /// `0x00482E70` first resolves the linked NPC and dispatches event `0x100`
    /// to that NPC's first linked Cutscene, if one exists.
    pub dispatch_npc_first_cutscene: bool,
    /// It then independently iterates every Cutscene linked directly to the Shop.
    /// The host resolves these indices against the Shop-linked Cutscene list.
    pub shop_cutscene_dispatch_order: Vec<usize>,
    pub player_state: Option<u8>,
}

pub fn plan_shop_hud_teardown(
    has_linked_npc: bool,
    npc_has_linked_cutscene: bool,
    shop_linked_cutscene_count: usize,
) -> RobotsShopHudTeardownPlan {
    if !has_linked_npc {
        return RobotsShopHudTeardownPlan {
            dispatch_npc_first_cutscene: false,
            shop_cutscene_dispatch_order: Vec::new(),
            player_state: None,
        };
    }
    RobotsShopHudTeardownPlan {
        dispatch_npc_first_cutscene: npc_has_linked_cutscene,
        shop_cutscene_dispatch_order: (0..shop_linked_cutscene_count).collect(),
        player_state: Some(ROBOTS_SHOP_PLAYER_STATE_DEFAULT),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct RobotsShopLifecycleState {
    pub active_shop_uid: Option<u32>,
    pub active_has_linked_npc: bool,
}

impl RobotsShopLifecycleState {
    pub fn set_open_result(&mut self, shop_uid: u32, has_linked_npc: bool, succeeded: bool) {
        if succeeded && self.active_shop_uid.is_none() {
            self.active_shop_uid = Some(shop_uid);
            self.active_has_linked_npc = has_linked_npc;
        }
    }

    pub fn close_hud(&mut self) {
        self.active_shop_uid = None;
        self.active_has_linked_npc = false;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RobotsShopDatabase {
    pub groups: Vec<RobotsShopGroupDefinition>,
    pub items: Vec<RobotsShopItemDefinition>,
}

impl RobotsShopDatabase {
    /// Shipped H05 ordinary Player namespace-`0x48` items that enter the normal
    /// Shop `+0x2C/+0x30` preflight/commit path. Native `0x0040B900` returns 2
    /// when the backing definition is absent, so every ordinary H05 row that is
    /// actually purchasable must have a live Player definition. This projection
    /// intentionally does not claim to reconstruct the process-global
    /// `0x007B30F0` loader; it only exposes the membership needed by the H05 Shop
    /// host. `0x48000007` is the separate ExitShop action and `0x48000009` is the
    /// aggregate gate selector, so neither belongs to the ordinary purchase set.
    pub fn ordinary_player_item_definition_uids(&self) -> Vec<u32> {
        let mut uids = self
            .items
            .iter()
            .map(|item| item.uid)
            .filter(|uid| is_robots_player_item_uid(*uid))
            .filter(|uid| *uid != ROBOTS_SHOP_EXIT_ITEM_UID)
            .filter(|uid| *uid != ROBOTS_PLAYER_ITEM_GATE_AGGREGATE)
            .collect::<Vec<_>>();
        uids.sort_unstable();
        uids.dedup();
        uids
    }

    pub fn resolve_offer(&self, shop_uid: u32, slot: usize) -> Option<RobotsShopOffer> {
        let group = self
            .groups
            .iter()
            .find(|group| group.shop_uid == shop_uid)?;
        let slot_definition = *group.slots.get(slot)?;
        if !slot_definition.is_present() {
            return None;
        }
        let item = self
            .items
            .iter()
            .copied()
            .find(|item| item.uid == slot_definition.item_uid)?;
        Some(RobotsShopOffer {
            shop_uid,
            slot,
            item_uid: item.uid,
            price: slot_definition.price,
            purchase_quantity: item.purchase_quantity(),
            display_resource_uid: item.display_resource_uid(),
            name_text_uid: item.name_text_uid(),
            description_text_uid: item.description_text_uid(),
        })
    }

    /// Engine-neutral projection of the ordinary `0x004AA160 -> 0x004AC6A0`
    /// shop-card gate. The two aggregate upgrade selectors use dedicated native
    /// child-count logic and remain explicitly marked as special here.
    pub fn evaluate_offer(
        &self,
        shop_uid: u32,
        slot: usize,
        inventory_definitions: &[RobotsInventoryDefinition],
        inventory_state: &RobotsInventoryState,
        player_items: &RobotsPlayerItemState,
        player_item_definition_uids: &[u32],
    ) -> Option<RobotsShopOfferAvailability> {
        const INVENTORY_AGGREGATE_UID: u32 = 0x4700_0022;
        const PLAYER_AGGREGATE_UID: u32 = 0x4800_0009;

        let offer = self.resolve_offer(shop_uid, slot)?;
        let current_scrap = inventory_state.stored_current(ROBOTS_SCRAP_UID);
        let affordable = offer.price <= current_scrap;
        let special_action = (offer.item_uid == ROBOTS_SHOP_EXIT_ITEM_UID)
            .then_some(RobotsShopSpecialAction::ExitShop);
        let special_preflight_required = matches!(
            offer.item_uid,
            INVENTORY_AGGREGATE_UID | PLAYER_AGGREGATE_UID
        );
        let native_item_preflight_result = if special_action.is_some() || special_preflight_required
        {
            None
        } else if is_robots_inventory_uid(offer.item_uid) {
            let definition = inventory_definitions
                .iter()
                .copied()
                .find(|definition| definition.uid == offer.item_uid);
            Some(
                inventory_state
                    .preview_add(definition, i32::from(offer.purchase_quantity))
                    .native_result_code,
            )
        } else if is_robots_player_item_uid(offer.item_uid) {
            Some(player_items.preflight_add_result(
                offer.item_uid,
                i32::from(offer.purchase_quantity),
                player_item_definition_uids.contains(&offer.item_uid),
            ))
        } else {
            Some(2)
        };
        let purchasable = affordable
            && (special_action.is_some()
                || native_item_preflight_result.is_some_and(|result| matches!(result, 1 | 4)));
        Some(RobotsShopOfferAvailability {
            offer,
            current_scrap,
            affordable,
            native_item_preflight_result,
            special_preflight_required,
            special_action,
            purchasable,
        })
    }

    /// Ordinary `0x004AAE20` purchase transaction. Native reaches this path
    /// only for an enabled runtime card: item mutation happens first; result
    /// 1/4 is success; then Scrap `0x47000001` is removed and that removal
    /// result is ignored by the original caller. This reducer preserves that
    /// ordering while failing closed if the host omitted the Scrap definition.
    pub fn purchase_offer(
        &self,
        shop_uid: u32,
        slot: usize,
        inventory_definitions: &[RobotsInventoryDefinition],
        inventory_state: &mut RobotsInventoryState,
        player_items: &mut RobotsPlayerItemState,
        player_item_definition_uids: &[u32],
        scrap_removal_locked: bool,
    ) -> Option<RobotsShopPurchaseOutcome> {
        let availability = self.evaluate_offer(
            shop_uid,
            slot,
            inventory_definitions,
            inventory_state,
            player_items,
            player_item_definition_uids,
        )?;
        if !availability.purchasable {
            return Some(RobotsShopPurchaseOutcome {
                availability,
                committed: false,
                special_action: availability.special_action,
                native_item_result: availability.native_item_preflight_result,
                native_scrap_remove_result: None,
            });
        }

        let offer = availability.offer;
        if let Some(special_action) = availability.special_action {
            let scrap_definition = inventory_definitions
                .iter()
                .copied()
                .find(|definition| definition.uid == ROBOTS_SCRAP_UID);
            let scrap = inventory_state.remove_quantity(
                scrap_definition,
                offer.price,
                scrap_removal_locked,
            );
            return Some(RobotsShopPurchaseOutcome {
                availability,
                committed: true,
                special_action: Some(special_action),
                native_item_result: None,
                native_scrap_remove_result: Some(scrap.native_result_code),
            });
        }

        let scrap_definition = inventory_definitions
            .iter()
            .copied()
            .find(|definition| definition.uid == ROBOTS_SCRAP_UID)?;
        let native_item_result = if is_robots_inventory_uid(offer.item_uid) {
            let definition = inventory_definitions
                .iter()
                .copied()
                .find(|definition| definition.uid == offer.item_uid);
            inventory_state
                .add(definition, i32::from(offer.purchase_quantity))
                .native_result_code
        } else if is_robots_player_item_uid(offer.item_uid) {
            player_items.add_with_native_result(
                offer.item_uid,
                i32::from(offer.purchase_quantity),
                player_item_definition_uids.contains(&offer.item_uid),
            )
        } else {
            2
        };

        if !matches!(native_item_result, 1 | 4) {
            return Some(RobotsShopPurchaseOutcome {
                availability,
                committed: false,
                special_action: None,
                native_item_result: Some(native_item_result),
                native_scrap_remove_result: None,
            });
        }

        let scrap = inventory_state.remove_quantity(
            Some(scrap_definition),
            offer.price,
            scrap_removal_locked,
        );
        Some(RobotsShopPurchaseOutcome {
            availability,
            committed: true,
            special_action: None,
            native_item_result: Some(native_item_result),
            native_scrap_remove_result: Some(scrap.native_result_code),
        })
    }
}

pub fn read_robots_shop_database(edb: &mut EdbFile) -> anyhow::Result<RobotsShopDatabase> {
    anyhow::ensure!(
        edb.header.hashcode == ROBOTS_SHOP_FILE_UID,
        "expected Robots shop EDB 0x{ROBOTS_SHOP_FILE_UID:08X}, got 0x{:08X}",
        edb.header.hashcode
    );
    let spreadsheets = UXGeoSpreadsheet::read_all(edb)?;
    let sheets = spreadsheets
        .iter()
        .find_map(|(uid, spreadsheet)| {
            (*uid == ROBOTS_SHOP_SPREADSHEET_UID)
                .then_some(spreadsheet)
                .and_then(|spreadsheet| match spreadsheet {
                    UXGeoSpreadsheet::Data(sheets) => Some(sheets.as_slice()),
                    UXGeoSpreadsheet::Text(_) => None,
                })
        })
        .context("missing HT_SpreadSheet_Shops")?;
    anyhow::ensure!(
        sheets.len() >= 2,
        "HT_SpreadSheet_Shops requires sheets 0 and 1"
    );

    edb.seek(SeekFrom::Start(sheets[0].address as u64))?;
    let mut groups = Vec::with_capacity(sheets[0].row_count as usize);
    for _ in 0..sheets[0].row_count {
        let mut words = [0u32; 17];
        for word in &mut words {
            *word = edb.read_type(edb.endian)?;
        }
        if let Some(group) = RobotsShopGroupDefinition::from_native_words(words) {
            groups.push(group);
        }
    }

    edb.seek(SeekFrom::Start(sheets[1].address as u64))?;
    let mut items = Vec::with_capacity(sheets[1].row_count as usize);
    for _ in 0..sheets[1].row_count {
        let mut words = [0u32; 9];
        for word in &mut words {
            *word = edb.read_type(edb.endian)?;
        }
        if let Some(item) = RobotsShopItemDefinition::from_native_words(words) {
            items.push(item);
        }
    }

    Ok(RobotsShopDatabase { groups, items })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shop_trigger_open_guard_and_cutscene_bypass_match_native_00482d50() {
        let blocked = plan_shop_trigger_event(ROBOTS_SHOP_EVENT_OPEN, false, Some(2), 7, true);
        assert!(blocked.effects.is_empty());
        assert_eq!(blocked.native_return, ROBOTS_SHOP_HANDLER_RETURN);

        let cutscene = plan_shop_trigger_event(ROBOTS_SHOP_EVENT_OPEN, true, Some(2), 7, true);
        assert_eq!(
            cutscene.effects,
            vec![RobotsShopTriggerEffect::OpenHud {
                shop_uid: 7,
                has_linked_npc: true,
            }]
        );

        let combined = plan_shop_trigger_event(
            ROBOTS_SHOP_EVENT_OPEN | ROBOTS_SHOP_EVENT_CLOSE,
            false,
            None,
            7,
            false,
        );
        assert_eq!(
            combined.effects,
            vec![
                RobotsShopTriggerEffect::OpenHud {
                    shop_uid: 7,
                    has_linked_npc: false,
                },
                RobotsShopTriggerEffect::CloseHud,
            ]
        );
    }

    #[test]
    fn shop_open_completion_preserves_linked_player_dispatch_on_hud_failure() {
        let failed = complete_shop_open(false, true, true);
        assert_eq!(failed.player_state, None);
        assert_eq!(
            failed.dispatch_linked_player_event,
            Some(ROBOTS_SHOP_EVENT_OPEN)
        );

        let success = complete_shop_open(true, true, true);
        assert_eq!(success.player_state, Some(ROBOTS_SHOP_PLAYER_STATE_ACTIVE));
        assert_eq!(
            success.dispatch_linked_player_event,
            Some(ROBOTS_SHOP_EVENT_OPEN)
        );
    }

    #[test]
    fn shop_hud_teardown_separates_npc_cutscene_from_shop_cutscene_iteration() {
        let plan = plan_shop_hud_teardown(true, true, 3);
        assert!(plan.dispatch_npc_first_cutscene);
        assert_eq!(plan.shop_cutscene_dispatch_order, vec![0, 1, 2]);
        assert_eq!(plan.player_state, Some(ROBOTS_SHOP_PLAYER_STATE_DEFAULT));

        let no_npc = plan_shop_hud_teardown(false, true, 3);
        assert!(!no_npc.dispatch_npc_first_cutscene);
        assert!(no_npc.shop_cutscene_dispatch_order.is_empty());
        assert_eq!(no_npc.player_state, None);
    }

    #[test]
    fn shop_group_row_is_uid_plus_eight_item_price_pairs() {
        let group = RobotsShopGroupDefinition::from_native_words([
            0x4900_0001,
            0x4800_0021,
            125,
            0x4800_0022,
            250,
            u32::MAX,
            0,
            0x4800_0023,
            375,
            u32::MAX,
            0,
            u32::MAX,
            0,
            u32::MAX,
            0,
            u32::MAX,
            0,
        ])
        .unwrap();
        assert_eq!(group.shop_uid, 0x4900_0001);
        assert_eq!(group.slots[0].item_uid, 0x4800_0021);
        assert_eq!(group.slots[0].price, 125);
        assert_eq!(group.slots[1].price, 250);
        assert!(!group.slots[2].is_present());
        assert_eq!(group.slots[3].item_uid, 0x4800_0023);
        assert_eq!(ROBOTS_SHOP_GROUP_ROW_SIZE, 0x44);
    }

    #[test]
    fn shop_item_row_preserves_unknown_words_and_signed_purchase_quantity() {
        let item = RobotsShopItemDefinition::from_native_words([
            0x4800_0021,
            0x1111_1111,
            0x0600_002c,
            0x3333_3333,
            0x4444_4444,
            0x4500_01ee,
            0x4500_01f2,
            0xabcd_fffe,
            0x8888_8888,
        ])
        .unwrap();
        assert_eq!(item.display_resource_uid(), 0x0600_002c);
        assert_eq!(item.name_text_uid(), 0x4500_01ee);
        assert_eq!(item.description_text_uid(), 0x4500_01f2);
        assert_eq!(item.purchase_quantity(), -2);
        assert_eq!(item.words_04_to_20[0], 0x1111_1111);
        assert_eq!(item.words_04_to_20[7], 0x8888_8888);
        assert_eq!(ROBOTS_SHOP_ITEM_ROW_SIZE, 0x24);
    }

    #[test]
    fn shop_offer_joins_group_price_with_item_quantity_and_presentation() {
        let database = RobotsShopDatabase {
            groups: vec![RobotsShopGroupDefinition::from_native_words([
                1,
                0x4800_0021,
                250,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
            ])
            .unwrap()],
            items: vec![RobotsShopItemDefinition::from_native_words([
                0x4800_0021,
                u32::MAX,
                0x0600_002c,
                0x0400_0293,
                u32::MAX,
                0x4500_01ee,
                0x4500_01f2,
                1,
                1,
            ])
            .unwrap()],
        };
        let offer = database.resolve_offer(1, 0).unwrap();
        assert_eq!(offer.item_uid, 0x4800_0021);
        assert_eq!(offer.price, 250);
        assert_eq!(offer.purchase_quantity, 1);
        assert_eq!(offer.display_resource_uid, 0x0600_002c);
        assert_eq!(offer.name_text_uid, 0x4500_01ee);
        assert_eq!(offer.description_text_uid, 0x4500_01f2);
        assert!(database.resolve_offer(1, 1).is_none());
    }

    #[test]
    fn ordinary_scrapgun_purchase_checks_affordability_then_commits_item_before_scrap() {
        let database = RobotsShopDatabase {
            groups: vec![RobotsShopGroupDefinition::from_native_words([
                1,
                0x4800_0021,
                250,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
            ])
            .unwrap()],
            items: vec![RobotsShopItemDefinition::from_native_words([
                0x4800_0021,
                u32::MAX,
                0x0600_002c,
                0x0400_0293,
                u32::MAX,
                0x4500_01ee,
                0x4500_01f2,
                1,
                1,
            ])
            .unwrap()],
        };
        let scrap_definition =
            RobotsInventoryDefinition::from_native_words([ROBOTS_SCRAP_UID, 0, 0, 0, 0, 0, 1000])
                .unwrap();
        let mut inventory = RobotsInventoryState::default();
        assert_eq!(
            inventory
                .add(Some(scrap_definition), 500)
                .native_result_code,
            1
        );
        let mut player_items = RobotsPlayerItemState::default();

        let availability = database
            .evaluate_offer(
                1,
                0,
                &[scrap_definition],
                &inventory,
                &player_items,
                &[0x4800_0021],
            )
            .unwrap();
        assert!(availability.affordable);
        assert_eq!(availability.native_item_preflight_result, Some(4));
        assert!(availability.purchasable);

        let outcome = database
            .purchase_offer(
                1,
                0,
                &[scrap_definition],
                &mut inventory,
                &mut player_items,
                &[0x4800_0021],
                false,
            )
            .unwrap();
        assert!(outcome.committed);
        assert_eq!(outcome.native_item_result, Some(4));
        assert_eq!(outcome.native_scrap_remove_result, Some(1));
        assert_eq!(inventory.stored_current(ROBOTS_SCRAP_UID), 250);
        assert_eq!(player_items.stored_current(0x4800_0021), 1);

        let repeat = database
            .purchase_offer(
                1,
                0,
                &[scrap_definition],
                &mut inventory,
                &mut player_items,
                &[0x4800_0021],
                false,
            )
            .unwrap();
        assert!(!repeat.committed);
        assert_eq!(repeat.native_item_result, Some(3));
        assert_eq!(inventory.stored_current(ROBOTS_SCRAP_UID), 250);
    }

    #[test]
    fn unaffordable_offer_does_not_mutate_item_or_scrap() {
        let database = RobotsShopDatabase {
            groups: vec![RobotsShopGroupDefinition::from_native_words([
                1,
                0x4800_0021,
                250,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
            ])
            .unwrap()],
            items: vec![RobotsShopItemDefinition::from_native_words([
                0x4800_0021,
                u32::MAX,
                0x0600_002c,
                0x0400_0293,
                u32::MAX,
                0x4500_01ee,
                0x4500_01f2,
                1,
                1,
            ])
            .unwrap()],
        };
        let scrap_definition =
            RobotsInventoryDefinition::from_native_words([ROBOTS_SCRAP_UID, 0, 0, 0, 0, 0, 1000])
                .unwrap();
        let mut inventory = RobotsInventoryState::default();
        inventory.add(Some(scrap_definition), 100);
        let mut player_items = RobotsPlayerItemState::default();

        let outcome = database
            .purchase_offer(
                1,
                0,
                &[scrap_definition],
                &mut inventory,
                &mut player_items,
                &[0x4800_0021],
                false,
            )
            .unwrap();
        assert!(!outcome.committed);
        assert!(!outcome.availability.affordable);
        assert_eq!(inventory.stored_current(ROBOTS_SCRAP_UID), 100);
        assert_eq!(player_items.stored_current(0x4800_0021), 0);
    }

    #[test]
    fn exit_shop_offer_is_special_and_does_not_require_player_item_definition() {
        let database = RobotsShopDatabase {
            groups: vec![RobotsShopGroupDefinition::from_native_words([
                1,
                ROBOTS_SHOP_EXIT_ITEM_UID,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
                u32::MAX,
                0,
            ])
            .unwrap()],
            items: vec![RobotsShopItemDefinition::from_native_words([
                ROBOTS_SHOP_EXIT_ITEM_UID,
                u32::MAX,
                0x0600_0005,
                0x0400_006b,
                u32::MAX,
                0x4500_0052,
                0x4500_0019,
                1,
                0,
            ])
            .unwrap()],
        };
        let mut inventory = RobotsInventoryState::default();
        let mut player_items = RobotsPlayerItemState::default();

        let availability = database
            .evaluate_offer(1, 0, &[], &inventory, &player_items, &[])
            .unwrap();
        assert!(availability.affordable);
        assert!(availability.purchasable);
        assert_eq!(availability.native_item_preflight_result, None);
        assert_eq!(
            availability.special_action,
            Some(RobotsShopSpecialAction::ExitShop)
        );

        let outcome = database
            .purchase_offer(1, 0, &[], &mut inventory, &mut player_items, &[], false)
            .unwrap();
        assert!(outcome.committed);
        assert_eq!(
            outcome.special_action,
            Some(RobotsShopSpecialAction::ExitShop)
        );
        assert_eq!(outcome.native_item_result, None);
        assert_eq!(player_items.stored_current(ROBOTS_SHOP_EXIT_ITEM_UID), 0);
        assert_eq!(inventory.stored_current(ROBOTS_SCRAP_UID), 0);
    }

    #[test]
    fn ordinary_player_item_definition_uids_are_data_derived_and_exclude_shop_specials() {
        let make_item = |uid| {
            RobotsShopItemDefinition::from_native_words([uid, u32::MAX, 0, 0, u32::MAX, 0, 0, 1, 0])
                .unwrap()
        };
        let database = RobotsShopDatabase {
            groups: vec![],
            items: vec![
                make_item(ROBOTS_SHOP_EXIT_ITEM_UID),
                make_item(ROBOTS_PLAYER_ITEM_GATE_AGGREGATE),
                make_item(0x4800_000d),
                make_item(0x4800_001f),
                make_item(0x4800_0021),
                make_item(0x4800_0021),
                make_item(0x4700_0018),
            ],
        };
        assert_eq!(
            database.ordinary_player_item_definition_uids(),
            vec![0x4800_000d, 0x4800_001f, 0x4800_0021]
        );
    }

    #[test]
    fn native_invalid_shop_rows_are_filtered() {
        assert!(RobotsShopGroupDefinition::from_native_words([u32::MAX; 17]).is_none());
        assert!(RobotsShopItemDefinition::from_native_words([u32::MAX; 9]).is_none());
    }
}
