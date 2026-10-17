use crate::draft::{
    bit_reprsentation::{BitInitalChampionState, BitLaneSafe, BitLaneValue},
    camp_graph::{CampGraph, CampSpawnDistanceMatrix, Center},
    lane::{LaneGraph, MAX_LANE_CAMPS},
};
use cambc::{ChampionDraft, Team};
use std::ops::Range;

/**

DraftMemory
bytes[0..15]      = 5 x InitialChampionState       (3 bytes each)  - 15 bytes
bytes[15..519]    = 8 x Lane Value                 (63 bytes each) - 504 bytes
bytes[519..1007]  = 8 x Lane Safe                  (61 bytes each) - 488 bytes
bytes[1007..1023] = 8 x Contested Camps            (2 bytes each)  - 16 bytes
bytes[1023]       = 2 x Lanes Counts [value, safe] (4 bits each)   - 1 byte

Total = 1024 bytes

**/
pub const DRAFT_MEMORY_BYTES: usize = 1024;
pub const LANE_SLOTS: usize = 8;
pub const CHAMPION_STATES: usize = 5;
pub const CHAMPION_STATE_BYTES: usize = 3;
pub const LANES_OFFSET: usize = 15;
pub const VALUE_LANE_BYTES: usize = 63;
pub const SAFE_LANE_BYTES: usize = 61;
pub const SAFE_OFFSET: usize = LANES_OFFSET + LANE_SLOTS * VALUE_LANE_BYTES;
pub const CONTESTED_OFFSET: usize = SAFE_OFFSET + LANE_SLOTS * SAFE_LANE_BYTES;
pub const CONTESTED_BYTES: usize = 16;
pub const LANE_CHAMPION_BYTE_OFFSET: usize = 41;
pub const SPANNING_OFFSET: usize = CONTESTED_OFFSET + CONTESTED_BYTES;
const _: () = assert!(SPANNING_OFFSET < DRAFT_MEMORY_BYTES);

fn fixed<T: Copy + Default, const N: usize>(items: impl IntoIterator<Item = T>) -> [T; N] {
    let mut out: [T; N] = [T::default(); N];
    for (slot, item) in out.iter_mut().zip(items) {
        *slot = item;
    }
    out
}

struct LaneFields {
    camps: [[u8; 2]; MAX_LANE_CAMPS],
    altars: [[u8; 2]; MAX_LANE_CAMPS],
    camp_altar_count: u8,
    danger: u16,
    prefix_gold_sum: [u8; MAX_LANE_CAMPS],
    prefix_gem_sum: [u8; MAX_LANE_CAMPS],
}

fn lane_fields(lane: &[usize], camp_graph: &CampGraph) -> LaneFields {
    let lane: &[usize] = &lane[..lane.len().min(MAX_LANE_CAMPS)];
    let camp_count: u8 = lane.len() as u8;
    let mut gold: u8 = 0;
    let mut gem: u8 = 0;
    let mut prefix_gold: Vec<u8> = Vec::with_capacity(lane.len());
    let mut prefix_gem: Vec<u8> = Vec::with_capacity(lane.len());
    for &camp_index in lane {
        gold = gold.saturating_add(camp_graph.camp_group_gold[camp_index]);
        gem = gem.saturating_add(camp_graph.camp_group_gem[camp_index]);
        prefix_gold.push(gold);
        prefix_gem.push(gem);
    }
    let danger: u16 = lane
        .iter()
        .map(|&camp_index: &usize| camp_graph.camp_group_danger_level[camp_index] as u16)
        .sum::<u16>()
        .min(15);
    LaneFields {
        camps: fixed::<[u8; 2], MAX_LANE_CAMPS>(
            lane.iter()
                .map(|&camp_index: &usize| camp_graph.camp_group_centers[camp_index]),
        ),
        altars: fixed::<[u8; 2], MAX_LANE_CAMPS>(
            lane.iter()
                .map(|&camp_index: &usize| camp_graph.closest_altars[camp_index]),
        ),
        camp_altar_count: (camp_count << 4) + camp_count,
        danger,
        prefix_gold_sum: fixed::<u8, MAX_LANE_CAMPS>(prefix_gold),
        prefix_gem_sum: fixed::<u8, MAX_LANE_CAMPS>(prefix_gem),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DraftMemory {
    champion_byte_offset: usize,
    value_lane_4bit_offset: [u8; 8],
    value_lane_mid_camp_locations: [[u8; 2]; 8],
    pub bytes: [u8; DRAFT_MEMORY_BYTES],
}

impl Default for DraftMemory {
    fn default() -> Self {
        Self {
            champion_byte_offset: 0,
            value_lane_4bit_offset: [0; 8],
            value_lane_mid_camp_locations: [[0; 2]; 8],
            bytes: [0; DRAFT_MEMORY_BYTES],
        }
    }
}

impl DraftMemory {
    pub fn new() -> Self {
        let mut bytes: [u8; DRAFT_MEMORY_BYTES] = [0; DRAFT_MEMORY_BYTES];
        let ally_spawn_center: [u8; 2] = cambc::map::spawn_region(Team::Ally).unwrap().center();
        let enemy_spawn_center: [u8; 2] = cambc::map::spawn_region(Team::Enemy).unwrap().center();

        let camp_graph: CampGraph = CampGraph::new(ally_spawn_center, enemy_spawn_center).unwrap();
        let distance_matrix: CampSpawnDistanceMatrix = CampSpawnDistanceMatrix {
            distances_to_camp: &camp_graph.camp_group_distances,
            distances_to_spawns: &camp_graph.camp_group_spawn_distances,
            spawns_distance: &camp_graph.spawns_distance,
        };

        let lane_graph: LaneGraph = LaneGraph::new(&camp_graph, &distance_matrix);

        let value_lanes: Vec<BitLaneValue> = lane_graph
            .value_lanes
            .iter()
            .map(|lane: &Vec<usize>| {
                let fields: LaneFields = lane_fields(lane, &camp_graph);
                BitLaneValue {
                    camps: fields.camps,
                    altars: fields.altars,
                    camp_altar_count: fields.camp_altar_count,
                    weight_champion_index: fields.danger << 12,
                    prefix_gold_sum: fields.prefix_gold_sum,
                    prefix_gem_sum: fields.prefix_gem_sum,
                }
            })
            .collect();

        let value_lane_mid_camp_locations: [[u8; 2]; 8] =
            std::array::from_fn::<[u8; 2], 8, _>(|lane_index: usize| {
                let lane: &Vec<usize> = &lane_graph.value_lanes[lane_index];
                let camp_count: usize = lane.len().min(MAX_LANE_CAMPS);
                if camp_count == 0 {
                    camp_graph.spawn_centers[0]
                } else {
                    camp_graph.camp_group_centers[lane[camp_count / 2]]
                }
            });

        let safe_lanes: Vec<BitLaneSafe> = lane_graph
            .safe_lanes
            .iter()
            .map(|lane: &Vec<usize>| {
                let fields: LaneFields = lane_fields(lane, &camp_graph);
                BitLaneSafe {
                    camps: fields.camps,
                    altars: fields.altars,
                    camp_altar_count: fields.camp_altar_count,
                    prefix_gold_sum: fields.prefix_gold_sum,
                    prefix_gem_sum: fields.prefix_gem_sum,
                }
            })
            .collect();

        for (lane_index, value_lane) in value_lanes.iter().enumerate() {
            let start: usize = LANES_OFFSET + lane_index * VALUE_LANE_BYTES;
            bytes[start..start + VALUE_LANE_BYTES].copy_from_slice(&value_lane.as_bytes());
        }
        for (lane_index, safe_lane) in safe_lanes.iter().enumerate() {
            let start: usize = SAFE_OFFSET + lane_index * SAFE_LANE_BYTES;
            bytes[start..start + SAFE_LANE_BYTES].copy_from_slice(&safe_lane.as_bytes());
        }
        bytes[CONTESTED_OFFSET..CONTESTED_OFFSET + CONTESTED_BYTES]
            .copy_from_slice(&Self::generate_contested_camps(&lane_graph, &camp_graph));

        let spanning: [u8; 2] = lane_graph.spanning_lane_index(&camp_graph, MAX_LANE_CAMPS);
        bytes[SPANNING_OFFSET] =
            ((spanning[0] & 0x7F).min(0x0F) << 4) | (spanning[1] & 0x7F).min(0x0F);

        DraftMemory {
            champion_byte_offset: 0,
            value_lane_4bit_offset: [0; 8],
            value_lane_mid_camp_locations,
            bytes,
        }
    }

    fn generate_contested_camps(
        lane_graph: &LaneGraph,
        camp_graph: &CampGraph,
    ) -> [u8; CONTESTED_BYTES] {
        let mut bytes: [u8; CONTESTED_BYTES] = [0; CONTESTED_BYTES];
        let mut indexed_camp_activity: Vec<(usize, u8)> = (0..camp_graph.camp_group_centers.len())
            .map(|camp_index: usize| (camp_index, 0))
            .collect();
        for lane in lane_graph
            .safe_lanes
            .iter()
            .chain(lane_graph.value_lanes.iter())
        {
            for &camp_index in lane.iter().take(MAX_LANE_CAMPS) {
                indexed_camp_activity[camp_index].1 += 1;
            }
        }

        indexed_camp_activity.sort_by_key(|&(camp_index, activity): &(usize, u8)| {
            (std::cmp::Reverse(activity), camp_index)
        });
        for (slot, (camp_index, _)) in indexed_camp_activity
            .into_iter()
            .take(CONTESTED_BYTES / 2)
            .enumerate()
        {
            bytes[slot * 2..slot * 2 + 2]
                .copy_from_slice(&camp_graph.camp_group_centers[camp_index]);
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
        let index: usize =
            (cambc::round_number().unwrap() as usize).min(Self::CHAMPION_PRIORITY.len() - 1);
        Self::CHAMPION_PRIORITY[index]
    }

    fn find_lane_champion_byte_index_range(lane_index: usize) -> Range<usize> {
        let start: usize = LANES_OFFSET + lane_index * VALUE_LANE_BYTES + LANE_CHAMPION_BYTE_OFFSET;
        start..start + 2
    }

    pub fn update(&mut self, pick: &ChampionDraft) {
        let lane_index: usize = cambc::round_number().unwrap() as usize;
        if lane_index >= LANE_SLOTS {
            return;
        }
        let champion_nibble: u8 = (*pick as u8) & 0xF;

        let slot: usize = self.value_lane_4bit_offset[lane_index] as usize;
        if slot < 3 {
            let range: Range<usize> = Self::find_lane_champion_byte_index_range(lane_index);
            let mut word: u16 =
                u16::from_le_bytes([self.bytes[range.start], self.bytes[range.start + 1]]);
            let shift: usize = 4 * slot;
            word = (word & !(0xF_u16 << shift)) | ((champion_nibble as u16) << shift);
            self.bytes[range].copy_from_slice(&word.to_le_bytes());
            self.value_lane_4bit_offset[lane_index] += 1;
        }

        if self.champion_byte_offset < CHAMPION_STATES {
            let init_champ_state: BitInitalChampionState = BitInitalChampionState {
                champion_index_starting_lane: (champion_nibble << 4) | (lane_index as u8 & 0xF),
                ward_location: self.value_lane_mid_camp_locations[lane_index],
            };
            let start: usize = self.champion_byte_offset * CHAMPION_STATE_BYTES;
            self.bytes[start..start + CHAMPION_STATE_BYTES]
                .copy_from_slice(&init_champ_state.as_bytes());
            self.champion_byte_offset += 1;
        }
    }
}
