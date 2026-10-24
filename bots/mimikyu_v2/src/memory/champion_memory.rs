use crate::{
    draft::draft_memory::{DraftMemory, DraftMemoryBlock, DRAFT_MEMORY_BYTE_SIZE},
    memory::{
        champion_info::AllChampionInfo,
        lazy_map::LazyMap,
        message::{message_board::MessageBoard, messages::Message},
        AltarID,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct ChampionMemory {
    pub map: LazyMap,
    pub champion_info: AllChampionInfo,
    pub message_board: MessageBoard,

    pub last_bastion: Option<AltarID>,
}

impl ChampionMemory {
    pub fn from_draft_bytes(bytes: [u8; DRAFT_MEMORY_BYTE_SIZE]) -> Self {
        let draft_memory: DraftMemory = DraftMemory::from_bytes(&bytes);

        Self {
            map: LazyMap::new(&draft_memory),
            champion_info: AllChampionInfo::new(),
            message_board: MessageBoard::new(),

            last_bastion: None,
        }
    }

    pub fn update_by_status_messgges(&mut self) {
        for received_message in &self.message_board.inbox {
            match received_message.message {
                Message::FarmClear { camp_group_id } => {
                    self.map.camp_group_respawn_timer[camp_group_id.0] =
                        self.map.camp_group_reset_timer[camp_group_id.0];
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
                        .and_modify(|champion_info| champion_info.last_seen = Some(location_id));
                }
                Message::StatusEnemyLocation {
                    enemy_champion_id,
                    location_id,
                } => {
                    self.champion_info
                        .enemy
                        .entry(enemy_champion_id)
                        .and_modify(|champion_info| champion_info.last_seen = Some(location_id));
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
                Message::EndgameAltarProtect { altar_id } => {
                    self.last_bastion = Some(altar_id);
                }
                _ => (),
            }
        }
    }
}
