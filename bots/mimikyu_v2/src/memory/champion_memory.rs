use cambc::{Ability, Champion, ChampionType, Team};

use crate::{
    draft::draft_memory::{DraftMemory, DraftMemoryBlock, DRAFT_MEMORY_BYTE_SIZE},
    memory::{
        champion_info::{AllChampionInfo, ChampionInfoCalculationExtension},
        lazy_map::LazyMap,
        message::{message_board::MessageBoard, messages::Message},
        LocationID,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct ChampionMemory {
    pub map: LazyMap,
    pub champion_info: AllChampionInfo,
    pub message_board: MessageBoard,

    pub buy_index: usize,
}

impl ChampionMemory {
    pub fn from_draft_bytes(bytes: [u8; DRAFT_MEMORY_BYTE_SIZE]) -> Self {
        let draft_memory: DraftMemory = DraftMemory::from_bytes(&bytes);

        Self {
            map: LazyMap::new(&draft_memory),
            champion_info: AllChampionInfo::new(),
            message_board: MessageBoard::new(),

            buy_index: 0,
        }
    }

    pub fn update_by_visible_champions(&mut self) {
        self.champion_info.enemy_updated_this_turn.clear();
        let enemy_visible_champions: Vec<Champion> =
            cambc::vision::champions(Some(Team::Enemy)).unwrap();

        for enemy_champion in enemy_visible_champions {
            let champion_type: ChampionType = enemy_champion.champion_name().unwrap();
            self.champion_info
                .enemy_updated_this_turn
                .push(champion_type);
            self.champion_info
                .enemy
                .entry(champion_type)
                .and_modify(|info| {
                    info.current_hp = Some(enemy_champion.health().unwrap());
                    info.current_cooldowns = enemy_champion
                        .abilities()
                        .clone()
                        .map(|ability: Ability| ability.cooldown().unwrap());
                    info.current_stats = enemy_champion.current_stats();
                    info.gems_held = Some(enemy_champion.gem_count().unwrap());
                    info.last_seen_near = enemy_champion.try_near_location(&mut self.map);
                    info.last_seen = Some(enemy_champion.position().unwrap());
                });
        }
    }

    pub fn update_by_status_messgges(&mut self) {
        for received_message in &self.message_board.inbox {
            match received_message.message {
                Message::FarmClear { camp_group_id } => {
                    self.map.start_camp_group_respawn_timer(camp_group_id);
                }
                Message::FarmGoingToCamp {
                    camp_group_id,
                    ally_champion,
                } => {
                    self.champion_info
                        .ally
                        .entry(ally_champion)
                        .and_modify(|champion_info| {
                            champion_info.last_seen_near =
                                Some(LocationID::CampGroup(camp_group_id))
                        });
                }
                Message::StatusAllyHealth {
                    ally_champion_id,
                    current_health_in_hundreds,
                } => {
                    self.champion_info
                        .ally
                        .entry(ally_champion_id)
                        .and_modify(|champion_info| {
                            champion_info.current_hp =
                                Some(current_health_in_hundreds as f32 * 100.)
                        });
                }
                Message::StatusEnemyHealth {
                    enemy_champion_id,
                    current_health_in_hundreds,
                } => {
                    self.champion_info
                        .enemy
                        .entry(enemy_champion_id)
                        .and_modify(|champion_info| {
                            champion_info.current_hp =
                                Some(current_health_in_hundreds as f32 * 100.)
                        });
                }
                Message::StatusAllyLocation {
                    ally_champion_id,
                    location_id,
                } => {
                    self.champion_info
                        .ally
                        .entry(ally_champion_id)
                        .and_modify(|champion_info| {
                            champion_info.last_seen_near = Some(location_id)
                        });
                }
                Message::StatusEnemyLocation {
                    enemy_champion_id,
                    location_id,
                } => {
                    self.champion_info
                        .enemy
                        .entry(enemy_champion_id)
                        .and_modify(|champion_info| {
                            champion_info.last_seen_near = Some(location_id)
                        });
                }
                Message::StatusAllyGemsHeld {
                    ally_champion_id,
                    gems_held_tens,
                } => {
                    self.champion_info
                        .ally
                        .entry(ally_champion_id)
                        .and_modify(|champion_info| {
                            champion_info.gems_held = Some(gems_held_tens * 10)
                        });
                }
                Message::StatusEnemyGemsHeld {
                    enemy_champion_id,
                    gems_held_tens,
                } => {
                    self.champion_info
                        .enemy
                        .entry(enemy_champion_id)
                        .and_modify(|champion_info| {
                            champion_info.gems_held = Some(gems_held_tens * 10)
                        });
                }
                _ => (),
            }
        }
    }
}
