use crate::draft::{
    bit_reprsentation::{BitInitalChampionState, BitLaneSafe, BitLaneValue},
    camp_graph::{CampGraph, CampSpawnDistanceMatrix, Center},
    lane::LaneGraph,
};
use cambc::{ChampionDraft, Team};
use std::ops::Range;

/**

DraftMemory
bytes[0..15]      = 5 x InitialChampionState   (3 bytes each)  - 15 bytes
bytes[15..519]    = 8 x Lane Value             (63 bytes each) - 504 bytes
bytes[519..1007]  = 8 x Lane Safe              (61 bytes each) - 488 bytes
bytes[1007..1023] = 8 x Contested Camps        (2 bytes each)  - 16 bytes
bytes[1023..1028] = Padding                                    - 5 bytes

Total = 1023 bytes

**/

#[derive(Debug, Clone, PartialEq)]
pub struct DraftMemory {
    champion_byte_offset: usize,
    value_lane_4bit_offset: [u8; 8],
    value_lane_mid_camp_locations: [[u8; 2]; 8],
    pub bytes: [u8; 1028],
}

impl Default for DraftMemory {
    fn default() -> Self {
        Self {
            champion_byte_offset: 0,
            value_lane_4bit_offset: [0; 8],
            value_lane_mid_camp_locations: [[0; 2]; 8],
            bytes: [0; 1028],
        }
    }
}

impl DraftMemory {
    pub fn new() -> Self {
        let mut bytes: [u8; 1028] = [0; 1028];
        let ally_spawn_center: [u8; 2] = cambc::map::spawn_region(Team::Ally).unwrap().center();
        let enemy_spawn_center: [u8; 2] = cambc::map::spawn_region(Team::Enemy).unwrap().center();
        let camp_graph: CampGraph = CampGraph::new(ally_spawn_center, enemy_spawn_center).unwrap();
        let distance_matrix: CampSpawnDistanceMatrix = CampSpawnDistanceMatrix {
            distances_to_camp: &camp_graph.camp_group_distances,
            distances_to_spawns: &camp_graph.camp_group_spawn_distances,
            spawns_distance: &camp_graph.spawns_distance,
        };
        let lane_graph: LaneGraph = LaneGraph::new(&camp_graph, &distance_matrix).unwrap();

        let value_lanes: Vec<BitLaneValue> = lane_graph
            .value_lanes
            .iter()
            .take(8)
            .map(|lane_indicies: &Vec<usize>| {
                let camp_count: u8 =
                    lane_indicies[1..lane_indicies.len() - 1].len().clamp(0, 10) as u8;
                let camp_indicies = lane_indicies[1..lane_indicies.len() - 1]
                    .iter()
                    .take(camp_count as usize);
                let camp_group_gold = &camp_graph.camp_group_gold;
                let camp_group_gem = &camp_graph.camp_group_gem;
                let mut prefix_gold_sum: u8 = 0;
                let mut prefix_gem_sum: u8 = 0;
                BitLaneValue {
                    camps: camp_indicies
                        .clone()
                        .map(|camp_index: &usize| camp_graph.camp_group_centers[*camp_index])
                        .collect::<Vec<[u8; 2]>>()
                        .try_into()
                        .unwrap(),
                    altars: camp_indicies
                        .clone()
                        .map(|camp_index: &usize| camp_graph.closest_altars[*camp_index])
                        .collect::<Vec<[u8; 2]>>()
                        .try_into()
                        .unwrap(),
                    camp_altar_count: (camp_count << 4) + (camp_count),
                    weight_champion_index: (camp_indicies
                        .clone()
                        .fold(0, |acc, camp_index: &usize| {
                            acc + camp_graph.camp_group_danger_level[*camp_index]
                        })
                        .clamp(0, 15) as u16)
                        << 12,
                    prefix_gold_sum: camp_indicies
                        .clone()
                        .map(move |camp_index: &usize| {
                            prefix_gold_sum += camp_group_gold[*camp_index];
                            prefix_gold_sum
                        })
                        .collect::<Vec<u8>>()
                        .try_into()
                        .unwrap(),
                    prefix_gem_sum: camp_indicies
                        .clone()
                        .map(move |camp_index: &usize| {
                            prefix_gem_sum += camp_group_gem[*camp_index];
                            prefix_gem_sum
                        })
                        .collect::<Vec<u8>>()
                        .try_into()
                        .unwrap(),
                }
            })
            .collect();

        let value_lane_mid_camp_locations: [[u8; 2]; 8] = value_lanes
            .iter()
            .map(|bit_lane_value: &BitLaneValue| {
                let middle_camp_index: usize = (bit_lane_value.camp_altar_count >> 4) as usize / 2;
                bit_lane_value.camps[middle_camp_index]
            })
            .collect::<Vec<[u8; 2]>>()
            .try_into()
            .unwrap();

        let safe_lanes: Vec<BitLaneSafe> = lane_graph
            .safe_lanes
            .iter()
            .take(8)
            .map(|lane_indicies: &Vec<usize>| {
                let camp_count: u8 =
                    lane_indicies[1..lane_indicies.len() - 1].len().clamp(0, 10) as u8;
                let camp_indicies = lane_indicies[1..lane_indicies.len() - 1]
                    .iter()
                    .take(camp_count as usize);
                let camp_group_gold = &camp_graph.camp_group_gold;
                let camp_group_gem = &camp_graph.camp_group_gem;
                let mut prefix_gold_sum: u8 = 0;
                let mut prefix_gem_sum: u8 = 0;
                BitLaneSafe {
                    camps: camp_indicies
                        .clone()
                        .map(|camp_index: &usize| camp_graph.camp_group_centers[*camp_index])
                        .collect::<Vec<[u8; 2]>>()
                        .try_into()
                        .unwrap(),
                    altars: camp_indicies
                        .clone()
                        .map(|camp_index: &usize| camp_graph.closest_altars[*camp_index])
                        .collect::<Vec<[u8; 2]>>()
                        .try_into()
                        .unwrap(),
                    camp_altar_count: (camp_count << 4) + (camp_count),
                    prefix_gold_sum: camp_indicies
                        .clone()
                        .map(move |camp_index: &usize| {
                            prefix_gold_sum += camp_group_gold[*camp_index];
                            prefix_gold_sum
                        })
                        .collect::<Vec<u8>>()
                        .try_into()
                        .unwrap(),
                    prefix_gem_sum: camp_indicies
                        .clone()
                        .map(move |camp_index: &usize| {
                            prefix_gem_sum += camp_group_gem[*camp_index];
                            prefix_gem_sum
                        })
                        .collect::<Vec<u8>>()
                        .try_into()
                        .unwrap(),
                }
            })
            .collect();

        let offset: usize = 15;
        for value_lane in &value_lanes {
            bytes[offset..offset + 63].copy_from_slice(&value_lane.as_bytes());
        }
        for safe_lane in &safe_lanes {
            bytes[offset..offset + 61].copy_from_slice(&safe_lane.as_bytes());
        }
        bytes[offset..offset + 16]
            .copy_from_slice(&Self::generate_contested_camps(&lane_graph, &camp_graph));

        DraftMemory {
            champion_byte_offset: 0,
            value_lane_4bit_offset: [0; 8],
            value_lane_mid_camp_locations,
            bytes,
        }
    }

    fn generate_contested_camps(lane_graph: &LaneGraph, camp_graph: &CampGraph) -> [u8; 16] {
        let mut bytes: [u8; 16] = [0; 16];
        let mut indexed_camp_activity: Vec<(usize, u8)> = (0..camp_graph.camp_group_centers.len())
            .map(|i| (i, 0))
            .collect();
        for lane in lane_graph
            .safe_lanes
            .iter()
            .chain(lane_graph.value_lanes.iter())
        {
            for camp_index in &lane[1..lane.len() - 1] {
                indexed_camp_activity[*camp_index].1 += 1;
            }
        }

        indexed_camp_activity.sort_unstable_by_key(|a| std::cmp::Reverse(a.1));
        let mut offset: usize = 0;
        for (camp_index, _) in indexed_camp_activity {
            bytes[offset..offset + 2].copy_from_slice(&camp_graph.camp_group_centers[camp_index]);
            offset += 2;
            if offset >= 16 {
                break;
            }
        }

        bytes
    }

    pub const CHAMPION_PRIORITY: &'static [ChampionDraft; 11] = &[
        ChampionDraft::Varg,
        ChampionDraft::CassiAndIssac,
        ChampionDraft::Ren,
        ChampionDraft::Marcella,
        ChampionDraft::Ryth,
        ChampionDraft::Vendetta,
        ChampionDraft::Tavi,
        ChampionDraft::Stellar,
        ChampionDraft::Willow,
        ChampionDraft::VesperVale,
        ChampionDraft::Prism,
    ];

    pub fn pick(&mut self) -> ChampionDraft {
        Self::CHAMPION_PRIORITY[cambc::round_number().unwrap() as usize]
    }

    fn find_lane_champion_byte_index_range(lane_index: usize) -> Range<usize> {
        let value_lane_offset: usize = lane_index * 63;
        (56 + value_lane_offset)..(58 + value_lane_offset)
    }

    pub fn update(&mut self, pick: &ChampionDraft) {
        let lane_index: usize = cambc::round_number().unwrap() as usize;
        let lane_byte_range: Range<usize> = Self::find_lane_champion_byte_index_range(lane_index);
        let new_bytes: [u8; 2] = match self.value_lane_4bit_offset[lane_index] {
            0 => [self.bytes[lane_byte_range.clone()][0] + *pick as u8, 0],
            1 => [self.bytes[lane_byte_range.clone()][0], (*pick as u8) << 4],
            2 => [self.bytes[lane_byte_range.clone()][0], (*pick as u8)],
            _ => unreachable!(),
        };

        self.bytes[lane_byte_range].copy_from_slice(&new_bytes);
        let init_champ_state: BitInitalChampionState = BitInitalChampionState {
            champion_index_starting_lane: ((*pick as u8) << 4) + lane_index as u8,
            ward_location: self.value_lane_mid_camp_locations[lane_index],
        };

        self.bytes[self.champion_byte_offset * 3..(self.champion_byte_offset + 1) * 3]
            .copy_from_slice(&init_champ_state.as_bytes());
    }
}
