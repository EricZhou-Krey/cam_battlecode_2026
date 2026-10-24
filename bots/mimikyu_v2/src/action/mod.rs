use std::collections::VecDeque;

use crate::{
    action::move_to_location::{move_to_location_turn, MoveToLocationResult},
    memory::{
        champion_memory::ChampionMemory, message::messages::Message, AltarID, CampGroupID,
        LocationID,
    },
};
use cambc::{Champion, ChampionType, ConsumableType, DistanceField, ItemType, Team, Vec2};

pub mod action_state;
pub mod combat;
pub mod move_to_location;

const BUY_ORDER_NON_FOCUS: &[ItemType; 7] = &[
    ItemType::TreasureShardOfHealth,
    ItemType::DurabilityShardOfStrength,
    ItemType::BruiserShardOfHealth,
    ItemType::PowerShardOfStrength,
    ItemType::HomeShardOfHealth,
    ItemType::MonsterShardOfStrength,
    ItemType::JungleShardOfHealth,
];
const BUY_ORDER_FOCUS: &[ItemType; 7] = &[
    ItemType::TreasureShardOfHealth,
    ItemType::DurabilityShardOfFocus,
    ItemType::BruiserShardOfHealth,
    ItemType::PowerShardOfStrength,
    ItemType::HomeShardOfFocus,
    ItemType::MonsterShardOfFocus,
    ItemType::JungleShardOfHealth,
];

const COLLECTION_TIME: i32 = 5;

#[derive(Debug, Default, Clone, PartialEq, Copy, Hash)]
pub enum ActionGroupType {
    #[default]
    Intermediate, // [Idle, MoveToLocation(Intermediate)]
    Farm,    // [CollectingFromCamp, InCampGroupCombat, TributingAtAltar, MoveToLocation(Farm)]
    Support, // [GoingToHeal, DefendLocation, MoveToLocation(Support)]
    ChampionCombat, // [InChampionCombatStall, InChampionComabtKill]
    Survival, // [EscapeToSpawn, InChampionCombatEscape]
}

#[derive(Debug, Default, Clone, PartialEq)]
pub enum Action {
    #[default]
    Idle,
    MoveToLocation {
        distance_field: Option<DistanceField>,
        location_id: Option<LocationID>,
        accepted_distance: u16,
        action_group_type: ActionGroupType,
    },
    CollectingFromCamp {
        timer: i32,
    },
    InChampionComabtKill {
        // TODO
    },
    InChampionCombatStall {
        // TODO
    },
    InChampionCombatEscape {
        // TODO
    },
    InCampGroupCombat {
        camp_group_id: CampGroupID,
        current_attack_tile: Vec2,
        attack_tile_queue: VecDeque<Vec2>,
    },
    TributingAtAltar {
        altar_id: AltarID,
    },
    DefendLocation {
        location_id: LocationID,
    },
    EscapeToSpawn,
}

impl Action {
    const IDLE: i32 = ActionGroupType::Intermediate as i32;

    const COLLECTING_FROM_CAMP: i32 = ActionGroupType::Farm as i32;
    const IN_CAMP_GROUP_COMBAT: i32 = ActionGroupType::Farm as i32;
    const TRIBUTING_AT_ALTAR: i32 = ActionGroupType::Farm as i32;

    const DEFEND_LOCATION: i32 = ActionGroupType::Support as i32;

    const IN_CHAMPION_COMBAT_STALL: i32 = ActionGroupType::ChampionCombat as i32;
    const IN_CHAMPION_COMABT_KILL: i32 = ActionGroupType::ChampionCombat as i32;

    const ESCAPE_TO_SPAWN: i32 = ActionGroupType::Survival as i32;
    const IN_CHAMPION_COMBAT_ESCAPE: i32 = ActionGroupType::Survival as i32;

    pub fn precedence(&self) -> i32 {
        match self {
            Action::Idle => Self::IDLE,
            Action::DefendLocation { .. } => Self::DEFEND_LOCATION,

            Action::CollectingFromCamp { .. } => Self::COLLECTING_FROM_CAMP,
            Action::InCampGroupCombat { .. } => Self::IN_CAMP_GROUP_COMBAT,
            Action::TributingAtAltar { .. } => Self::TRIBUTING_AT_ALTAR,

            Action::InChampionCombatStall { .. } => Self::IN_CHAMPION_COMBAT_STALL,
            Action::InChampionComabtKill { .. } => Self::IN_CHAMPION_COMABT_KILL,

            Action::EscapeToSpawn => Self::ESCAPE_TO_SPAWN,
            Action::InChampionCombatEscape { .. } => Self::IN_CHAMPION_COMBAT_ESCAPE,

            Action::MoveToLocation {
                action_group_type, ..
            } => *action_group_type as i32,
        }
    }

    fn try_prempt_current_action(&mut self, champion_memory: &mut ChampionMemory) {
        let highest_precedence_message_action: Option<Self> =
            champion_memory.message_board.inbox.iter().fold(
                None,
                |highest_precedence_message_action, received_message| match &received_message
                    .message
                {
                    Message::FarmGoingToCamp { .. }
                    | Message::EndgameAltarProtect { .. }
                    | Message::FarmClear { .. }
                    | Message::StatusAllyHealth { .. }
                    | Message::StatusEnemyHealth { .. }
                    | Message::StatusAllyLocation { .. }
                    | Message::StatusEnemyLocation { .. }
                    | Message::StatusAllyGemsHeld { .. }
                    | Message::StatusEnemyGemsHeld { .. } => highest_precedence_message_action,

                    Message::FarmHelp {
                        camp_group_id,
                        signal_strength,
                        ally_champions_helping,
                    } => {
                        // TODO
                        // Move to farm, enter farm combat
                        todo!();
                    }
                    Message::FarmCollection {
                        camp_group_id,
                        ally_champion_claiming,
                    } => {
                        // TODO
                        // Move to center, enter collection action
                        todo!();
                    }

                    Message::CombatHeal {
                        location_id,
                        signal_strength,
                    } => {
                        // TODO
                        // Move to location
                        todo!();
                    }
                    Message::CombatLosingFightNeedHelp {
                        location_id,
                        signal_strength,
                        ally_champion,
                    } => {
                        // TODO
                        // Move to location
                        todo!();
                    }
                    Message::CombatWinningFightWantHelp {
                        location_id,
                        signal_strength,
                        ally_champion,
                    } => {
                        // TODO
                        // Move to location
                        todo!();
                    }
                    Message::CombatTargetEnemy { enemy_champion_id } => {
                        // TODO
                        // If in combat action, assign enemy as target
                        todo!();
                    }
                    Message::GroupTogether { location_id } => {
                        // TODO
                        // Move to location
                        todo!();
                    }
                },
            );

        // TODO: Look around for enemies if the case enter combat no matter what,
        // also applie hp threshold to return to base and a higher threshold for asking for heals
        // from willow
    }

    fn auto_buy(&mut self, champion_memory: &mut ChampionMemory) {
        let me: &Champion = cambc::self_champion().unwrap().unwrap();
        let _ = me.buy_consumable(ConsumableType::TpCrystal);
        while me.gold_count().unwrap() >= 30 {
            let buy_item: ItemType = match me.champion_name().unwrap() {
                ChampionType::Willow | ChampionType::HeartOfRuin | ChampionType::Prism => {
                    BUY_ORDER_FOCUS[(champion_memory.action_state.buy_index / 5) % 7]
                }
                _ => BUY_ORDER_NON_FOCUS[(champion_memory.action_state.buy_index / 5) % 7],
            };
            champion_memory.action_state.buy_index += 1;
            let _ = me.buy(buy_item);
        }
    }

    pub fn turn(&mut self, champion_memory: &mut ChampionMemory) {
        self.auto_buy(champion_memory);
        self.try_prempt_current_action(champion_memory);

        match self {
            Action::Idle => {
                if let Some(action) = champion_memory.action_state.action_queue.pop_front() {
                    *self = action;
                }
                self.turn(champion_memory);
            }
            Action::MoveToLocation {
                distance_field,
                location_id,
                accepted_distance,
                action_group_type,
            } => {
                if move_to_location_turn(
                    distance_field,
                    *location_id,
                    *accepted_distance,
                    *action_group_type,
                    champion_memory,
                ) == MoveToLocationResult::Finished
                {
                    *self = Self::Idle;
                }
            }
            Action::CollectingFromCamp { timer } => {
                *timer -= 1;
                if *timer < 0 {
                    *self = Self::Idle;
                }
            }
            Action::InChampionComabtKill { .. } => {
                todo!()
            }
            Action::InChampionCombatStall { .. } => {
                todo!()
            }
            Action::InChampionCombatEscape { .. } => {
                todo!()
            }
            Action::InCampGroupCombat {
                camp_group_id,
                current_attack_tile,
                attack_tile_queue,
            } => {
                todo!()
            }
            Action::TributingAtAltar { altar_id } => {
                let me: &Champion = cambc::self_champion().unwrap().unwrap();
                if me.gem_count().unwrap() == 0 {
                    *self = Self::Idle;
                    return;
                }
                if !me.is_submitting_gems().unwrap() {
                    cambc::self_champion().unwrap().unwrap().submit_gems(
                        champion_memory.map.points_of_interest.altar_locations[altar_id.0],
                    );
                }
            }
            Action::DefendLocation { location_id: _ } => (),
            Action::EscapeToSpawn => {
                let me: &Champion = cambc::self_champion().unwrap().unwrap();
                if cambc::map::spawn_region(Team::Ally)
                    .unwrap()
                    .is_set(me.position().unwrap())
                    .unwrap()
                {
                    *self = Self::Idle;
                }
                if !me.is_recalling().unwrap() {
                    me.recall();
                }
            }
        }
    }
}
