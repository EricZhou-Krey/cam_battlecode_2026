use crate::draft::draft_memory::DraftMemoryBlock;
use cambc::{Bitboard, Camp, CampGroup, CampStats, CampType, DistanceField, Team, Vec2};

pub const MAX_CAMP_GROUPS: usize = 64;

const DISCOVERY_ORDER_BYTE_SIZE: usize = MAX_CAMP_GROUPS;
const EFFECTIVE_HP_BYTE_SIZE: usize = MAX_CAMP_GROUPS * 2;
const DAMAGE_PER_ROUND_BYTE_SIZE: usize = MAX_CAMP_GROUPS;
const GEM_VALUE_BYTE_SIZE: usize = MAX_CAMP_GROUPS * 2;
const GOLD_VALUE_BYTE_SIZE: usize = MAX_CAMP_GROUPS * 2;
const COUNT_BYTE_SIZE: usize = 1;
pub const CAMP_DRAFT_BYTE_SIZE: usize = DISCOVERY_ORDER_BYTE_SIZE
    + EFFECTIVE_HP_BYTE_SIZE
    + DAMAGE_PER_ROUND_BYTE_SIZE
    + GEM_VALUE_BYTE_SIZE
    + GOLD_VALUE_BYTE_SIZE
    + COUNT_BYTE_SIZE;

#[derive(Debug, Clone, PartialEq)]
pub struct CampDraftBlock {
    pub camp_group_discovery_order: [u8; MAX_CAMP_GROUPS],
    pub camp_group_effective_hp: [u16; MAX_CAMP_GROUPS],
    pub camp_group_damage_per_round: [u8; MAX_CAMP_GROUPS],
    pub camp_group_gem_value: [u16; MAX_CAMP_GROUPS],
    pub camp_group_gold_value: [u16; MAX_CAMP_GROUPS],
    pub camp_group_count: u8,
}

impl CampDraftBlock {
    pub fn new(
        camp_group_distance_fields: &mut [DistanceField; MAX_CAMP_GROUPS],
        blocked_bitboard: &Bitboard,
    ) -> Self {
        let camp_groups: Vec<CampGroup> = cambc::map::camp_groups().unwrap();
        let camp_group_count: usize = camp_groups.len();
        let spawn_bitboard: &Bitboard = cambc::map::spawn_region(Team::Ally).unwrap();

        let n_camp_group_count: usize = camp_group_count.min(MAX_CAMP_GROUPS);

        let mut camp_group_distance_indexed: Vec<(u16, usize)> =
            Vec::with_capacity(n_camp_group_count);
        let mut camp_group_effective_hp: [u16; MAX_CAMP_GROUPS] = [0; MAX_CAMP_GROUPS];
        let mut camp_group_damage_per_round: [u8; MAX_CAMP_GROUPS] = [0; MAX_CAMP_GROUPS];
        let mut camp_group_gem_value: [u16; MAX_CAMP_GROUPS] = [0; MAX_CAMP_GROUPS];
        let mut camp_group_gold_value: [u16; MAX_CAMP_GROUPS] = [0; MAX_CAMP_GROUPS];

        for camp_id in 0..n_camp_group_count {
            let camp_group_tiles: Bitboard = camp_groups[camp_id].spawn_tiles().unwrap();
            camp_group_distance_fields[camp_id]
                .compute_to(
                    blocked_bitboard,
                    spawn_bitboard,
                    &camp_group_tiles,
                    &mut Bitboard::new().unwrap(),
                    u16::MAX,
                )
                .unwrap();

            let distance_to_camp_group: u16 = spawn_bitboard.positions().unwrap().iter().fold(
                u16::MAX,
                |acc, position: &Vec2| {
                    let distance: u16 = camp_group_distance_fields[camp_id].get(*position).unwrap();
                    if acc < distance {
                        acc
                    } else {
                        distance
                    }
                },
            );

            camp_group_distance_indexed.push((distance_to_camp_group, camp_id));

            let camps: &[Camp] = camp_groups[camp_id].camps();

            let mut effective_hp: u16 = 0;
            let mut damage_per_round: u8 = 0;
            let gem_value: u16 = camp_groups[camp_id].gem_value() as u16;
            let gold_value: u16 = camp_groups[camp_id].gold_value() as u16;

            for camp in camps {
                let camp_stats: CampStats = camp.base_stats().unwrap();
                match camp.kind() {
                    CampType::Wolf | CampType::Elk | CampType::Monolith | CampType::Golem => {
                        effective_hp += camp_stats.base_health as u16;
                        damage_per_round += camp_stats.base_strength as u8;
                    }
                    _ => (),
                }
            }

            camp_group_effective_hp[camp_id] = effective_hp;
            camp_group_damage_per_round[camp_id] = damage_per_round;
            camp_group_gem_value[camp_id] = gem_value;
            camp_group_gold_value[camp_id] = gold_value;
        }

        camp_group_distance_indexed.sort_unstable();
        let mut camp_group_discovery_order: [u8; MAX_CAMP_GROUPS] = [0; MAX_CAMP_GROUPS];
        for (slot, (_, index)) in camp_group_distance_indexed.into_iter().enumerate() {
            camp_group_discovery_order[slot] = index as u8;
        }

        Self {
            camp_group_discovery_order,
            camp_group_effective_hp,
            camp_group_damage_per_round,
            camp_group_gem_value,
            camp_group_gold_value,
            camp_group_count: n_camp_group_count as u8,
        }
    }
}

fn to_bytes_u8(bytes_u16: &[u16]) -> Vec<u8> {
    bytes_u16.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn to_bytes_u16(bytes_u8: &[u8]) -> Vec<u16> {
    bytes_u8
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect()
}

impl DraftMemoryBlock<CAMP_DRAFT_BYTE_SIZE> for CampDraftBlock {
    fn as_bytes(&self) -> [u8; CAMP_DRAFT_BYTE_SIZE] {
        let mut bytes: [u8; CAMP_DRAFT_BYTE_SIZE] = [0; CAMP_DRAFT_BYTE_SIZE];
        let mut byte_pointer: usize = 0;
        bytes[byte_pointer..byte_pointer + DISCOVERY_ORDER_BYTE_SIZE]
            .copy_from_slice(&self.camp_group_discovery_order);
        byte_pointer += DISCOVERY_ORDER_BYTE_SIZE;

        bytes[byte_pointer..byte_pointer + EFFECTIVE_HP_BYTE_SIZE]
            .copy_from_slice(&to_bytes_u8(&self.camp_group_effective_hp));
        byte_pointer += EFFECTIVE_HP_BYTE_SIZE;

        bytes[byte_pointer..byte_pointer + DAMAGE_PER_ROUND_BYTE_SIZE]
            .copy_from_slice(&self.camp_group_damage_per_round);
        byte_pointer += DAMAGE_PER_ROUND_BYTE_SIZE;

        bytes[byte_pointer..byte_pointer + GEM_VALUE_BYTE_SIZE]
            .copy_from_slice(&to_bytes_u8(&self.camp_group_gem_value));
        byte_pointer += GEM_VALUE_BYTE_SIZE;

        bytes[byte_pointer..byte_pointer + GOLD_VALUE_BYTE_SIZE]
            .copy_from_slice(&to_bytes_u8(&self.camp_group_gold_value));
        byte_pointer += GOLD_VALUE_BYTE_SIZE;

        bytes[byte_pointer..byte_pointer + COUNT_BYTE_SIZE]
            .copy_from_slice(&[self.camp_group_count]);

        bytes
    }

    fn from_bytes(bytes: &[u8; CAMP_DRAFT_BYTE_SIZE]) -> Self {
        let mut byte_pointer: usize = 0;
        let camp_group_discovery_order: [u8; DISCOVERY_ORDER_BYTE_SIZE] = bytes
            [byte_pointer..byte_pointer + DISCOVERY_ORDER_BYTE_SIZE]
            .try_into()
            .unwrap();
        byte_pointer += DISCOVERY_ORDER_BYTE_SIZE;

        let camp_group_effective_hp: [u16; MAX_CAMP_GROUPS] =
            to_bytes_u16(&bytes[byte_pointer..byte_pointer + EFFECTIVE_HP_BYTE_SIZE])
                .try_into()
                .unwrap();
        byte_pointer += EFFECTIVE_HP_BYTE_SIZE;

        let camp_group_damage_per_round: [u8; DAMAGE_PER_ROUND_BYTE_SIZE] = bytes
            [byte_pointer..byte_pointer + DAMAGE_PER_ROUND_BYTE_SIZE]
            .try_into()
            .unwrap();
        byte_pointer += DAMAGE_PER_ROUND_BYTE_SIZE;

        let camp_group_gem_value: [u16; MAX_CAMP_GROUPS] =
            to_bytes_u16(&bytes[byte_pointer..byte_pointer + GEM_VALUE_BYTE_SIZE])
                .try_into()
                .unwrap();
        byte_pointer += GEM_VALUE_BYTE_SIZE;

        let camp_group_gold_value: [u16; MAX_CAMP_GROUPS] =
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
