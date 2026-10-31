use crate::{action::Action, memory::champion_memory::ChampionMemory};
use cambc::{Champion, ChampionType, ConsumableType, ItemFamily, ItemStat, ItemType};

const JUNGLER_ITEM_FAMILY: &[ItemFamily; 14] = &[
    ItemFamily::Treasure,
    ItemFamily::Monster,
    ItemFamily::Prospector,
    ItemFamily::Capture,
    ItemFamily::Durability,
    ItemFamily::Collectors,
    ItemFamily::Jungle,
    ItemFamily::Power,
    ItemFamily::Bruiser,
    ItemFamily::Tribute,
    ItemFamily::Hunter,
    ItemFamily::Skirmish,
    ItemFamily::Home,
    ItemFamily::Greed,
];

const DPS_ITEM_FAMILY: &[ItemFamily; 14] = &[
    ItemFamily::Treasure,
    ItemFamily::Monster,
    ItemFamily::Skirmish,
    ItemFamily::Hunter,
    ItemFamily::Collectors,
    ItemFamily::Durability,
    ItemFamily::Power,
    ItemFamily::Bruiser,
    ItemFamily::Jungle,
    ItemFamily::Capture,
    ItemFamily::Tribute,
    ItemFamily::Prospector,
    ItemFamily::Home,
    ItemFamily::Greed,
];

const SURVIVAL_ITEM_FAMILY: &[ItemFamily; 14] = &[
    ItemFamily::Treasure,
    ItemFamily::Durability,
    ItemFamily::Home,
    ItemFamily::Bruiser,
    ItemFamily::Power,
    ItemFamily::Monster,
    ItemFamily::Prospector,
    ItemFamily::Capture,
    ItemFamily::Collectors,
    ItemFamily::Jungle,
    ItemFamily::Tribute,
    ItemFamily::Hunter,
    ItemFamily::Skirmish,
    ItemFamily::Greed,
];

const WILLOW_BUY: &[ItemStat] = &[ItemStat::Health, ItemStat::Focus];
const HEARTOFRUIN_BUY: &[ItemStat] = &[
    ItemStat::Focus,
    ItemStat::Focus,
    ItemStat::Focus,
    ItemStat::Health,
];
const VARG_BUY: &[ItemStat] = &[ItemStat::Strength, ItemStat::Strength, ItemStat::Health];
const VENDETTA_BUY: &[ItemStat] = &[ItemStat::Health, ItemStat::Strength];
const REN_BUY: &[ItemStat] = &[
    ItemStat::Health,
    ItemStat::Focus,
    ItemStat::Health,
    ItemStat::Strength,
];

pub trait ActionBuyExtension {
    fn buy_action(&self, champion_memory: &mut ChampionMemory);
}

impl ActionBuyExtension for Action {
    fn buy_action(&self, champion_memory: &mut ChampionMemory) {
        let me: &Champion = cambc::self_champion().unwrap().unwrap();
        let _ = me.buy_consumable(ConsumableType::TpCrystal);
        let _ = me.buy_consumable(cambc::ConsumableType::AltarLock);
        while me.gold_count().unwrap() >= 30 && me.can_buy(ItemType::HomeShardOfFocus, 1).unwrap() {
            let buy_item: ItemType = match me.champion_name().unwrap() {
                ChampionType::Willow => item_from_type_stat(
                    SURVIVAL_ITEM_FAMILY[(champion_memory.buy_index / 5) % 14],
                    WILLOW_BUY[(champion_memory.buy_index / 5) % 3],
                ),
                ChampionType::HeartOfRuin => item_from_type_stat(
                    DPS_ITEM_FAMILY[(champion_memory.buy_index / 5) % 14],
                    HEARTOFRUIN_BUY[(champion_memory.buy_index / 5) % 3],
                ),
                ChampionType::Varg => item_from_type_stat(
                    JUNGLER_ITEM_FAMILY[(champion_memory.buy_index / 5) % 14],
                    VARG_BUY[(champion_memory.buy_index / 5) % 3],
                ),
                ChampionType::Ren => item_from_type_stat(
                    DPS_ITEM_FAMILY[(champion_memory.buy_index / 5) % 14],
                    REN_BUY[(champion_memory.buy_index / 5) % 3],
                ),
                ChampionType::Vendetta => item_from_type_stat(
                    DPS_ITEM_FAMILY[(champion_memory.buy_index / 5) % 14],
                    VENDETTA_BUY[(champion_memory.buy_index / 5) % 3],
                ),
                _ => item_from_type_stat(
                    SURVIVAL_ITEM_FAMILY[(champion_memory.buy_index / 5) % 14],
                    REN_BUY[(champion_memory.buy_index / 5) % 3],
                ),
            };
            let _ = me.buy(buy_item);
        }
    }
}

fn item_from_type_stat(item_family: ItemFamily, item_stat: ItemStat) -> ItemType {
    ItemType::try_from(6 * item_family as u32 + 2 * item_stat as u32).unwrap()
}
