use crate::draft::draft_memory::DraftMemoryBlock;
use cambc::{Bitboard, Camp, CampGroup, CampStats, CampType, DistanceField, Team, Vec2};

const DISCOVERY_ORDER_BYTE_SIZE: usize = 50;
const EFFECTIVE_HP_BYTE_SIZE: usize = 100;
const DAMAGE_PER_ROUND_BYTE_SIZE: usize = 50;
const GEM_VALUE_BYTE_SIZE: usize = 100;
const GOLD_VALUE_BYTE_SIZE: usize = 100;
const COUNT_BYTE_SIZE: usize = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct CampDraftBlock {
    pub camp_group_discovery_order: [u8; 50],
    pub camp_group_effective_hp: [u16; 50],
    pub camp_group_damage_per_round: [u8; 50],
    pub camp_group_gem_value: [u16; 50],
    pub camp_group_gold_value: [u16; 50],
    pub camp_group_count: u8,
}

impl CampDraftBlock {
    pub fn new() -> Self {
        let camp_groups: Vec<CampGroup> = cambc::map::camp_groups().unwrap();
        let camp_group_count: usize = camp_groups.len();
        let spawn_bitboard: &Bitboard = cambc::map::spawn_region(Team::Ally).unwrap();

        let mut camp_group_distance_indexed: Vec<(u16, usize)> =
            Vec::with_capacity(camp_group_count);
        let mut camp_group_effective_hp: Vec<u16> = Vec::with_capacity(camp_group_count);
        let mut camp_group_damage_per_round: Vec<u8> = Vec::with_capacity(camp_group_count);
        let mut camp_group_gem_value: Vec<u16> = Vec::with_capacity(camp_group_count);
        let mut camp_group_gold_value: Vec<u16> = Vec::with_capacity(camp_group_count);

        let blocked_bitboard: Bitboard = cambc::map::walls()
            .unwrap()
            .union(cambc::map::holes().unwrap())
            .unwrap()
            .union(cambc::map::altars().unwrap())
            .unwrap();

        for camp_id in 0..camp_group_count.clamp(0, 50) {
            let camp_group_tiles: Bitboard = camp_groups[camp_id].spawn_tiles().unwrap();
            let mut distance_field: DistanceField = DistanceField::new().unwrap();
            distance_field
                .compute_to(
                    &blocked_bitboard,
                    spawn_bitboard,
                    &camp_group_tiles,
                    &mut Bitboard::new().unwrap(),
                    u16::MAX,
                )
                .unwrap();

            let distance_to_camp_group: u16 = spawn_bitboard.positions().unwrap().iter().fold(
                u16::MAX,
                |acc, position: &Vec2| {
                    let distance: u16 = distance_field.get(*position).unwrap();
                    if acc < distance {
                        acc
                    } else {
                        distance
                    }
                },
            );

            camp_group_distance_indexed.push((distance_to_camp_group, camp_id));

            let camps: &[Camp] = camp_groups[camp_id].camps().unwrap();

            let mut effective_hp: u16 = 0;
            let mut damage_per_round: u8 = 0;
            let gem_value: u16 = camp_groups[camp_id].gem_value() as u16;
            let gold_value: u16 = camp_groups[camp_id].gold_value() as u16;

            for camp in camps {
                let camp_stats: CampStats = camp.base_stats().unwrap();
                match camp.kind() {
                    CampType::Wolf | CampType::Elk | CampType::Monolith | CampType::Golem => {
                        effective_hp += camp_stats.base_health;
                        damage_per_round += camp_stats.base_strength;
                    }
                    _ => (),
                }
            }

            camp_group_effective_hp.push(effective_hp);
            camp_group_damage_per_round.push(damage_per_round);
            camp_group_gem_value.push(gem_value);
            camp_group_gold_value.push(gold_value);
        }

        Self {
            camp_group_discovery_order,
            camp_group_effective_hp: camp_group_effective_hp.try_into().unwrap(),
            camp_group_damage_per_round: camp_group_damage_per_round.try_into().unwrap(),
            camp_group_gem_value: camp_group_gem_value.try_into().unwrap(),
            camp_group_gold_value: camp_group_gold_value.try_into().unwrap(),
            camp_group_count: camp_group_count as u8,
        }
    }
}

fn to_bytes_u8(bytes_u16: &[u16]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            bytes_u16.as_ptr() as *const u8,
            bytes_u16.len() * std::mem::size_of::<u16>(),
        )
    }
}

fn to_bytes_u16(bytes_u8: &[u8]) -> &[u16] {
    unsafe {
        std::slice::from_raw_parts(
            bytes_u8.as_ptr() as *const u16,
            bytes_u8.len() / std::mem::size_of::<u16>(),
        )
    }
}

impl DraftMemoryBlock for CampDraftBlock {
    const BYTE_SIZE: usize = DISCOVERY_ORDER_BYTE_SIZE
        + EFFECTIVE_HP_BYTE_SIZE
        + DAMAGE_PER_ROUND_BYTE_SIZE
        + GEM_VALUE_BYTE_SIZE
        + GOLD_VALUE_BYTE_SIZE
        + COUNT_BYTE_SIZE;
    fn as_bytes(&self) -> &[u8; BYTE_SIZE] {
        let mut bytes: &[u8; BYTE_SIZE] = [0; BYTE_SIZE];
        let mut byte_pointer: usize = 0;
        bytes[byte_pointer..byte_pointer + DISCOVERY_ORDER_BYTE_SIZE]
            .copy_from_slice(&self.camp_group_discovery_order);
        byte_pointer += DISCOVERY_ORDER_BYTE_SIZE;

        bytes[byte_pointer..byte_pointer + EFFECTIVE_HP_BYTE_SIZE]
            .copy_from_slice(to_bytes_u8(&self.camp_group_effective_hp));
        byte_pointer += EFFECTIVE_HP_BYTE_SIZE;

        bytes[byte_pointer..byte_pointer + DAMAGE_PER_ROUND_BYTE_SIZE]
            .copy_from_slice(&self.camp_group_damage_per_round);
        byte_pointer += DAMAGE_PER_ROUND_BYTE_SIZE;

        bytes[byte_pointer..byte_pointer + GEM_VALUE_BYTE_SIZE]
            .copy_from_slice(to_bytes_u8(&self.camp_group_gem_value));
        byte_pointer += GEM_VALUE_BYTE_SIZE;

        bytes[byte_pointer..byte_pointer + GOLD_VALUE_BYTE_SIZE]
            .copy_from_slice(to_bytes_u8(&self.camp_group_gold_value));
        byte_pointer += GEM_VALUE_BYTE_SIZE;

        bytes[byte_pointer..byte_pointer + COUNT_BYTE_SIZE]
            .copy_from_slice(&[self.camp_group_count]);

        bytes
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        let mut byte_pointer: usize = 0;
        let camp_group_discovery_order: [u8; 50] = bytes
            [byte_pointer..byte_pointer + DISCOVERY_ORDER_BYTE_SIZE]
            .try_into()
            .unwrap();
        byte_pointer += DISCOVERY_ORDER_BYTE_SIZE;

        let camp_group_effective_hp: [u16; 50] =
            to_bytes_u16(&bytes[byte_pointer..byte_pointer + EFFECTIVE_HP_BYTE_SIZE])
                .try_into()
                .unwrap();
        byte_pointer += EFFECTIVE_HP_BYTE_SIZE;

        let camp_group_damage_per_round: [u8; 50] = bytes
            [byte_pointer..byte_pointer + DAMAGE_PER_ROUND_BYTE_SIZE]
            .try_into()
            .unwrap();
        byte_pointer += DAMAGE_PER_ROUND_BYTE_SIZE;

        let camp_group_gem_value: [u16; 50] =
            to_bytes_u16(&bytes[byte_pointer..byte_pointer + GEM_VALUE_BYTE_SIZE])
                .try_into()
                .unwrap();
        byte_pointer += GEM_VALUE_BYTE_SIZE;

        let camp_group_gold_value: [u16; 50] =
            to_bytes_u16(&bytes[byte_pointer..byte_pointer + GOLD_VALUE_BYTE_SIZE])
                .try_into()
                .unwrap();
        byte_pointer += GOLD_VALUE_BYTE_SIZE;

        let camp_group_count: u8 = bytes[byte_pointer];

        Self {
            camp_group_discovery_order,
            camp_group_effective_hp,
            camp_group_damage_per_round,
            camp_group_gem_value,
            camp_group_gold_value,
            camp_group_count,
        }
    }
}
