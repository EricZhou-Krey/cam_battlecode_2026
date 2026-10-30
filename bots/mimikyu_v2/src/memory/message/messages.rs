use crate::memory::{CampGroupID, LocationID};
use cambc::ChampionType;

const MESSAGE_TYPE_BITS: u32 = 4;
const ID_BITS: u32 = 8;
const SIGNAL_STRENGTH_BITS: u32 = 4;
const CHAMPION_BITS: u32 = 4;
const HEALTH_BITS: u32 = 8;
const GEMS_BITS: u32 = 8;
const CONFIRMATION_BITS: u32 = 1;

const SIGNAL_STRENGTH_MAX: u16 = 15;
const GEMS_HELD_TENS_MAX: u32 = 255;
const NO_CHAMPION_BITS: u32 = 0xF;

struct BitWriter {
    bits: u32,
    used_bits: u32,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            bits: 0,
            used_bits: 0,
        }
    }

    fn push(&mut self, value: u32, bit_count: u32) {
        let mask: u32 = (1u32 << bit_count) - 1;
        self.bits |= (value & mask) << (32 - self.used_bits - bit_count);
        self.used_bits += bit_count;
    }

    fn finish(self) -> Vec<u8> {
        let byte_count: usize = self.used_bits.div_ceil(8) as usize;
        self.bits.to_be_bytes()[..byte_count].to_vec()
    }
}

struct BitReader {
    bits: u32,
}

impl BitReader {
    fn new(bytes: &[u8]) -> Self {
        let mut bits: u32 = 0;
        for (index, byte) in bytes.iter().take(4).enumerate() {
            bits |= (*byte as u32) << (24 - 8 * index as u32);
        }
        Self { bits }
    }

    fn take(&mut self, bit_count: u32) -> u32 {
        let value: u32 = self.bits >> (32 - bit_count);
        self.bits <<= bit_count;
        value
    }
}

fn champion_to_bits(champion: ChampionType) -> u32 {
    champion as u32
}

fn optional_champion_to_bits(champion: Option<ChampionType>) -> u32 {
    champion.map_or(NO_CHAMPION_BITS, champion_to_bits)
}

fn bits_to_champion(bits: u32) -> ChampionType {
    ChampionType::try_from(bits as i32).unwrap_or(ChampionType::Ryth)
}

fn bits_to_optional_champion(bits: u32) -> Option<ChampionType> {
    ChampionType::try_from(bits as i32).ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    FarmClear,
    FarmHelp,
    FarmConfirm,
    FarmCollection,
    FarmGoingToCamp,
    CombatHeal,
    CombatLosingFightNeedHelp,
    CombatWinningFightWantHelp,
    CombatTargetEnemy,
    StatusAllyHealth,
    StatusEnemyHealth,
    StatusAllyLocation,
    StatusEnemyLocation,
    StatusAllyGemsHeld,
    StatusEnemyGemsHeld,
    GroupTogether,
}

impl MessageType {
    const FARM_CLEAR: u8 = Self::FarmClear as u8;
    const FARM_HELP: u8 = Self::FarmHelp as u8;
    const FARM_CONFIRM: u8 = Self::FarmConfirm as u8;
    const FARM_COLLECTION: u8 = Self::FarmCollection as u8;
    const FARM_GOING_TO_CAMP: u8 = Self::FarmGoingToCamp as u8;
    const COMBAT_HEAL: u8 = Self::CombatHeal as u8;
    const COMBAT_LOSING_FIGHT_NEED_HELP: u8 = Self::CombatLosingFightNeedHelp as u8;
    const COMBAT_WINNING_FIGHT_WANT_HELP: u8 = Self::CombatWinningFightWantHelp as u8;
    const COMBAT_TARGET_ENEMY: u8 = Self::CombatTargetEnemy as u8;
    const STATUS_ALLY_HEALTH: u8 = Self::StatusAllyHealth as u8;
    const STATUS_ENEMY_HEALTH: u8 = Self::StatusEnemyHealth as u8;
    const STATUS_ALLY_LOCATION: u8 = Self::StatusAllyLocation as u8;
    const STATUS_ENEMY_LOCATION: u8 = Self::StatusEnemyLocation as u8;
    const STATUS_ALLY_GEMS_HELD: u8 = Self::StatusAllyGemsHeld as u8;
    const STATUS_ENEMY_GEMS_HELD: u8 = Self::StatusEnemyGemsHeld as u8;
    const GROUP_TOGETHER: u8 = Self::GroupTogether as u8;

    pub fn from_nibble(nibble: u8) -> Self {
        match nibble {
            Self::FARM_CLEAR => Self::FarmClear,
            Self::FARM_HELP => Self::FarmHelp,
            Self::FARM_CONFIRM => Self::FarmConfirm,
            Self::FARM_COLLECTION => Self::FarmCollection,
            Self::FARM_GOING_TO_CAMP => Self::FarmGoingToCamp,
            Self::COMBAT_HEAL => Self::CombatHeal,
            Self::COMBAT_LOSING_FIGHT_NEED_HELP => Self::CombatLosingFightNeedHelp,
            Self::COMBAT_WINNING_FIGHT_WANT_HELP => Self::CombatWinningFightWantHelp,
            Self::COMBAT_TARGET_ENEMY => Self::CombatTargetEnemy,
            Self::STATUS_ALLY_HEALTH => Self::StatusAllyHealth,
            Self::STATUS_ENEMY_HEALTH => Self::StatusEnemyHealth,
            Self::STATUS_ALLY_LOCATION => Self::StatusAllyLocation,
            Self::STATUS_ENEMY_LOCATION => Self::StatusEnemyLocation,
            Self::STATUS_ALLY_GEMS_HELD => Self::StatusAllyGemsHeld,
            Self::STATUS_ENEMY_GEMS_HELD => Self::StatusEnemyGemsHeld,
            Self::GROUP_TOGETHER => Self::GroupTogether,
            _ => unreachable!(),
        }
    }

    pub fn bit_length(&self) -> u32 {
        MESSAGE_TYPE_BITS
            + match self {
                Self::FarmClear | Self::GroupTogether => ID_BITS,
                Self::FarmHelp => ID_BITS + SIGNAL_STRENGTH_BITS + 3 * CHAMPION_BITS,
                Self::FarmConfirm => ID_BITS + CONFIRMATION_BITS,
                Self::FarmCollection | Self::FarmGoingToCamp => ID_BITS + CHAMPION_BITS,
                Self::CombatHeal => ID_BITS + SIGNAL_STRENGTH_BITS,
                Self::CombatLosingFightNeedHelp | Self::CombatWinningFightWantHelp => {
                    ID_BITS + SIGNAL_STRENGTH_BITS + CHAMPION_BITS
                }
                Self::CombatTargetEnemy => CHAMPION_BITS,
                Self::StatusAllyHealth | Self::StatusEnemyHealth => CHAMPION_BITS + HEALTH_BITS,
                Self::StatusAllyLocation | Self::StatusEnemyLocation => CHAMPION_BITS + ID_BITS,
                Self::StatusAllyGemsHeld | Self::StatusEnemyGemsHeld => CHAMPION_BITS + GEMS_BITS,
            }
    }

    pub fn byte_length(&self) -> usize {
        self.bit_length().div_ceil(8) as usize
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    FarmClear {
        camp_group_id: CampGroupID,
    },
    FarmHelp {
        camp_group_id: CampGroupID,
        signal_strength: u16,
        ally_champions_helping: [Option<ChampionType>; 3],
    },
    FarmConfirm {
        camp_group_id: CampGroupID,
        confirmed: bool,
    },
    FarmCollection {
        camp_group_id: CampGroupID,
        ally_champion_claiming: Option<ChampionType>,
    },
    FarmGoingToCamp {
        camp_group_id: CampGroupID,
        ally_champion: ChampionType,
    },

    CombatHeal {
        location_id: LocationID,
        signal_strength: u16,
    },
    CombatLosingFightNeedHelp {
        location_id: LocationID,
        signal_strength: u16,
        ally_champion: Option<ChampionType>,
    },
    CombatWinningFightWantHelp {
        location_id: LocationID,
        signal_strength: u16,
        ally_champion: Option<ChampionType>,
    },
    CombatTargetEnemy {
        location_id: LocationID,
        signal_strength: u16,
    },

    StatusAllyHealth {
        ally_champion_id: ChampionType,
        current_health_in_hundreds: u8,
    },
    StatusEnemyHealth {
        enemy_champion_id: ChampionType,
        current_health_in_hundreds: u8,
    },
    StatusAllyLocation {
        ally_champion_id: ChampionType,
        location_id: LocationID,
    },
    StatusEnemyLocation {
        enemy_champion_id: ChampionType,
        location_id: LocationID,
    },
    StatusAllyGemsHeld {
        ally_champion_id: ChampionType,
        gems_held_tens: u32,
    },
    StatusEnemyGemsHeld {
        enemy_champion_id: ChampionType,
        gems_held_tens: u32,
    },

    GroupTogether {
        location_id: LocationID,
    },
}

impl Message {
    pub fn farm_help(camp_group_id: CampGroupID, signal_strength: u16) -> Self {
        Self::FarmHelp {
            camp_group_id,
            signal_strength,
            ally_champions_helping: [None; 3],
        }
    }

    pub fn farm_collection(camp_group_id: CampGroupID) -> Self {
        Self::FarmCollection {
            camp_group_id,
            ally_champion_claiming: None,
        }
    }

    pub fn combat_losing_fight_need_help(location_id: LocationID, signal_strength: u16) -> Self {
        Self::CombatLosingFightNeedHelp {
            location_id,
            signal_strength,
            ally_champion: None,
        }
    }

    pub fn combat_winning_fight_want_help(location_id: LocationID, signal_strength: u16) -> Self {
        Self::CombatWinningFightWantHelp {
            location_id,
            signal_strength,
            ally_champion: None,
        }
    }

    pub fn message_type(&self) -> MessageType {
        match self {
            Self::FarmClear { .. } => MessageType::FarmClear,
            Self::FarmHelp { .. } => MessageType::FarmHelp,
            Self::FarmConfirm { .. } => MessageType::FarmConfirm,
            Self::FarmCollection { .. } => MessageType::FarmCollection,
            Self::FarmGoingToCamp { .. } => MessageType::FarmGoingToCamp,
            Self::CombatHeal { .. } => MessageType::CombatHeal,
            Self::CombatLosingFightNeedHelp { .. } => MessageType::CombatLosingFightNeedHelp,
            Self::CombatWinningFightWantHelp { .. } => MessageType::CombatWinningFightWantHelp,
            Self::CombatTargetEnemy { .. } => MessageType::CombatTargetEnemy,
            Self::StatusAllyHealth { .. } => MessageType::StatusAllyHealth,
            Self::StatusEnemyHealth { .. } => MessageType::StatusEnemyHealth,
            Self::StatusAllyLocation { .. } => MessageType::StatusAllyLocation,
            Self::StatusEnemyLocation { .. } => MessageType::StatusEnemyLocation,
            Self::StatusAllyGemsHeld { .. } => MessageType::StatusAllyGemsHeld,
            Self::StatusEnemyGemsHeld { .. } => MessageType::StatusEnemyGemsHeld,
            Self::GroupTogether { .. } => MessageType::GroupTogether,
        }
    }

    pub fn request_key(&self) -> Option<(MessageType, u8)> {
        match self {
            Self::FarmHelp { camp_group_id, .. } | Self::FarmCollection { camp_group_id, .. } => {
                Some((self.message_type(), camp_group_id.0 as u8))
            }
            Self::CombatLosingFightNeedHelp { location_id, .. }
            | Self::CombatWinningFightWantHelp { location_id, .. } => {
                Some((self.message_type(), location_id.as_byte()))
            }
            _ => None,
        }
    }

    pub fn respond(&mut self, responder: ChampionType) -> bool {
        match self {
            Self::FarmHelp {
                ally_champions_helping,
                ..
            } => {
                if ally_champions_helping.contains(&Some(responder)) {
                    return false;
                }
                match ally_champions_helping
                    .iter_mut()
                    .find(|slot| slot.is_none())
                {
                    Some(slot) => {
                        *slot = Some(responder);
                        true
                    }
                    None => false,
                }
            }
            Self::FarmCollection {
                ally_champion_claiming,
                ..
            } => {
                if ally_champion_claiming.is_none() {
                    *ally_champion_claiming = Some(responder);
                    true
                } else {
                    false
                }
            }
            Self::CombatLosingFightNeedHelp { ally_champion, .. }
            | Self::CombatWinningFightWantHelp { ally_champion, .. }
                if ally_champion.is_none() =>
            {
                *ally_champion = Some(responder);
                true
            }
            Self::CombatLosingFightNeedHelp { .. } | Self::CombatWinningFightWantHelp { .. } => {
                false
            }
            _ => false,
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut writer: BitWriter = BitWriter::new();
        writer.push(self.message_type() as u32, MESSAGE_TYPE_BITS);

        match self {
            Self::FarmClear { camp_group_id } => {
                writer.push(camp_group_id.0 as u32, ID_BITS);
            }
            Self::FarmHelp {
                camp_group_id,
                signal_strength,
                ally_champions_helping,
            } => {
                writer.push(camp_group_id.0 as u32, ID_BITS);
                writer.push(
                    (*signal_strength).min(SIGNAL_STRENGTH_MAX) as u32,
                    SIGNAL_STRENGTH_BITS,
                );
                for champion in ally_champions_helping {
                    writer.push(optional_champion_to_bits(*champion), CHAMPION_BITS);
                }
            }
            Self::FarmConfirm {
                camp_group_id,
                confirmed,
            } => {
                writer.push(camp_group_id.0 as u32, ID_BITS);
                writer.push(if *confirmed { 1 } else { 0 }, CONFIRMATION_BITS);
            }
            Self::FarmCollection {
                camp_group_id,
                ally_champion_claiming,
            } => {
                writer.push(camp_group_id.0 as u32, ID_BITS);
                writer.push(
                    optional_champion_to_bits(*ally_champion_claiming),
                    CHAMPION_BITS,
                );
            }
            Self::FarmGoingToCamp {
                camp_group_id,
                ally_champion,
            } => {
                writer.push(camp_group_id.0 as u32, ID_BITS);
                writer.push(champion_to_bits(*ally_champion), CHAMPION_BITS);
            }
            Self::CombatHeal {
                location_id,
                signal_strength,
            } => {
                writer.push(location_id.as_byte() as u32, ID_BITS);
                writer.push(
                    (*signal_strength).min(SIGNAL_STRENGTH_MAX) as u32,
                    SIGNAL_STRENGTH_BITS,
                );
            }
            Self::CombatLosingFightNeedHelp {
                location_id,
                signal_strength,
                ally_champion,
            }
            | Self::CombatWinningFightWantHelp {
                location_id,
                signal_strength,
                ally_champion,
            } => {
                writer.push(location_id.as_byte() as u32, ID_BITS);
                writer.push(
                    (*signal_strength).min(SIGNAL_STRENGTH_MAX) as u32,
                    SIGNAL_STRENGTH_BITS,
                );
                writer.push(optional_champion_to_bits(*ally_champion), CHAMPION_BITS);
            }
            Self::CombatTargetEnemy {
                location_id,
                signal_strength,
            } => {
                writer.push(location_id.as_byte() as u32, ID_BITS);
                writer.push(
                    (*signal_strength).min(SIGNAL_STRENGTH_MAX) as u32,
                    SIGNAL_STRENGTH_BITS,
                );
            }
            Self::StatusAllyHealth {
                ally_champion_id,
                current_health_in_hundreds,
            } => {
                writer.push(champion_to_bits(*ally_champion_id), CHAMPION_BITS);
                writer.push(*current_health_in_hundreds as u32, HEALTH_BITS);
            }
            Self::StatusEnemyHealth {
                enemy_champion_id,
                current_health_in_hundreds,
            } => {
                writer.push(champion_to_bits(*enemy_champion_id), CHAMPION_BITS);
                writer.push(*current_health_in_hundreds as u32, HEALTH_BITS);
            }
            Self::StatusAllyLocation {
                ally_champion_id,
                location_id,
            } => {
                writer.push(champion_to_bits(*ally_champion_id), CHAMPION_BITS);
                writer.push(location_id.as_byte() as u32, ID_BITS);
            }
            Self::StatusEnemyLocation {
                enemy_champion_id,
                location_id,
            } => {
                writer.push(champion_to_bits(*enemy_champion_id), CHAMPION_BITS);
                writer.push(location_id.as_byte() as u32, ID_BITS);
            }
            Self::StatusAllyGemsHeld {
                ally_champion_id,
                gems_held_tens,
            } => {
                writer.push(champion_to_bits(*ally_champion_id), CHAMPION_BITS);
                writer.push((*gems_held_tens).min(GEMS_HELD_TENS_MAX), GEMS_BITS);
            }
            Self::StatusEnemyGemsHeld {
                enemy_champion_id,
                gems_held_tens,
            } => {
                writer.push(champion_to_bits(*enemy_champion_id), CHAMPION_BITS);
                writer.push((*gems_held_tens).min(GEMS_HELD_TENS_MAX), GEMS_BITS);
            }
            Self::GroupTogether { location_id } => {
                writer.push(location_id.as_byte() as u32, ID_BITS);
            }
        }

        writer.finish()
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut reader: BitReader = BitReader::new(bytes);
        let message_type: MessageType =
            MessageType::from_nibble(reader.take(MESSAGE_TYPE_BITS) as u8);

        match message_type {
            MessageType::FarmClear => Self::FarmClear {
                camp_group_id: CampGroupID(reader.take(ID_BITS) as usize),
            },
            MessageType::FarmHelp => Self::FarmHelp {
                camp_group_id: CampGroupID(reader.take(ID_BITS) as usize),
                signal_strength: reader.take(SIGNAL_STRENGTH_BITS) as u16,
                ally_champions_helping: std::array::from_fn(|_| {
                    bits_to_optional_champion(reader.take(CHAMPION_BITS))
                }),
            },
            MessageType::FarmConfirm => Self::FarmConfirm {
                camp_group_id: CampGroupID(reader.take(ID_BITS) as usize),
                confirmed: reader.take(CONFIRMATION_BITS) == 1,
            },
            MessageType::FarmCollection => Self::FarmCollection {
                camp_group_id: CampGroupID(reader.take(ID_BITS) as usize),
                ally_champion_claiming: bits_to_optional_champion(reader.take(CHAMPION_BITS)),
            },
            MessageType::FarmGoingToCamp => Self::FarmGoingToCamp {
                camp_group_id: CampGroupID(reader.take(ID_BITS) as usize),
                ally_champion: bits_to_champion(reader.take(CHAMPION_BITS)),
            },
            MessageType::CombatHeal => Self::CombatHeal {
                location_id: LocationID::from_byte(reader.take(ID_BITS) as u8),
                signal_strength: reader.take(SIGNAL_STRENGTH_BITS) as u16,
            },
            MessageType::CombatLosingFightNeedHelp => Self::CombatLosingFightNeedHelp {
                location_id: LocationID::from_byte(reader.take(ID_BITS) as u8),
                signal_strength: reader.take(SIGNAL_STRENGTH_BITS) as u16,
                ally_champion: bits_to_optional_champion(reader.take(CHAMPION_BITS)),
            },
            MessageType::CombatWinningFightWantHelp => Self::CombatWinningFightWantHelp {
                location_id: LocationID::from_byte(reader.take(ID_BITS) as u8),
                signal_strength: reader.take(SIGNAL_STRENGTH_BITS) as u16,
                ally_champion: bits_to_optional_champion(reader.take(CHAMPION_BITS)),
            },
            MessageType::CombatTargetEnemy => Self::CombatTargetEnemy {
                location_id: LocationID::from_byte(reader.take(ID_BITS) as u8),
                signal_strength: reader.take(SIGNAL_STRENGTH_BITS) as u16,
            },
            MessageType::StatusAllyHealth => Self::StatusAllyHealth {
                ally_champion_id: bits_to_champion(reader.take(CHAMPION_BITS)),
                current_health_in_hundreds: reader.take(HEALTH_BITS) as u8,
            },
            MessageType::StatusEnemyHealth => Self::StatusEnemyHealth {
                enemy_champion_id: bits_to_champion(reader.take(CHAMPION_BITS)),
                current_health_in_hundreds: reader.take(HEALTH_BITS) as u8,
            },
            MessageType::StatusAllyLocation => Self::StatusAllyLocation {
                ally_champion_id: bits_to_champion(reader.take(CHAMPION_BITS)),
                location_id: LocationID::from_byte(reader.take(ID_BITS) as u8),
            },
            MessageType::StatusEnemyLocation => Self::StatusEnemyLocation {
                enemy_champion_id: bits_to_champion(reader.take(CHAMPION_BITS)),
                location_id: LocationID::from_byte(reader.take(ID_BITS) as u8),
            },
            MessageType::StatusAllyGemsHeld => Self::StatusAllyGemsHeld {
                ally_champion_id: bits_to_champion(reader.take(CHAMPION_BITS)),
                gems_held_tens: reader.take(GEMS_BITS),
            },
            MessageType::StatusEnemyGemsHeld => Self::StatusEnemyGemsHeld {
                enemy_champion_id: bits_to_champion(reader.take(CHAMPION_BITS)),
                gems_held_tens: reader.take(GEMS_BITS),
            },
            MessageType::GroupTogether => Self::GroupTogether {
                location_id: LocationID::from_byte(reader.take(ID_BITS) as u8),
            },
        }
    }
}
