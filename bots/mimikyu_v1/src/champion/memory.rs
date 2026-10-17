use cambc::{ChampionDraft, Vec2};

use crate::draft::{
    lane::{MAX_LANE_CAMPS, NOT_SPANNING_FLAG},
    memory::{
        DraftMemory, CHAMPION_STATES, CHAMPION_STATE_BYTES, CONTESTED_BYTES, CONTESTED_OFFSET,
        DRAFT_MEMORY_BYTES, LANES_OFFSET, LANE_CHAMPION_BYTE_OFFSET, LANE_SLOTS, SAFE_LANE_BYTES,
        SAFE_OFFSET, SPANNING_OFFSET, VALUE_LANE_BYTES,
    },
};

const POINT_BYTES: usize = 2;
const LANE_POINTS_BYTES: usize = MAX_LANE_CAMPS * POINT_BYTES;
const LANE_COUNT_BYTE: usize = 2 * LANE_POINTS_BYTES;
const SAFE_PREFIX_START: usize = LANE_COUNT_BYTE + 1;
const VALUE_PREFIX_START: usize = LANE_CHAMPION_BYTE_OFFSET + 2;
const CONTESTED_CAMPS: usize = CONTESTED_BYTES / POINT_BYTES;

fn point(x: u8, y: u8) -> Vec2 {
    Vec2 {
        x: x as i32,
        y: y as i32,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InitialChampionState {
    pub champion_index: u8,
    pub starting_lane: u8,
    pub ward_location: Vec2,
}

impl InitialChampionState {
    pub fn champion_draft(&self) -> Option<ChampionDraft> {
        DraftMemory::CHAMPION_PRIORITY
            .iter()
            .copied()
            .find(|champion: &ChampionDraft| ((*champion as u8) & 0xF) == self.champion_index)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LaneMemory {
    pub camps: Vec<Vec2>,
    pub altars: Vec<Vec2>,
    pub prefix_gold_sum: Vec<u8>,
    pub prefix_gem_sum: Vec<u8>,
    pub danger: u8,
    pub champion_slots: [u8; 3],
}

impl LaneMemory {
    pub fn camp_count(&self) -> usize {
        self.camps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.camps.is_empty()
    }

    pub fn total_gold(&self) -> u8 {
        self.prefix_gold_sum.last().copied().unwrap_or(0)
    }

    pub fn total_gem(&self) -> u8 {
        self.prefix_gem_sum.last().copied().unwrap_or(0)
    }

    pub fn middle_camp(&self) -> Option<Vec2> {
        self.camps.get(self.camps.len() / 2).copied()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LaneSet {
    pub lanes: Vec<LaneMemory>,
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChampionMemory {
    pub initial_states: Vec<InitialChampionState>,
    pub value_lanes: LaneSet,
    pub safe_lanes: LaneSet,
    pub contested_camps: [Vec2; CONTESTED_CAMPS],
}

fn read_points(slice: &[u8], count: usize) -> Vec<Vec2> {
    let (pairs, _remainder): (&[[u8; POINT_BYTES]], &[u8]) = slice.as_chunks::<POINT_BYTES>();
    pairs
        .iter()
        .take(count)
        .map(|pair: &[u8; POINT_BYTES]| point(pair[0], pair[1]))
        .collect()
}

fn parse_lane(lane: &[u8], is_value: bool) -> LaneMemory {
    let camp_count: usize = ((lane[LANE_COUNT_BYTE] >> 4) as usize).min(MAX_LANE_CAMPS);
    let altar_count: usize = ((lane[LANE_COUNT_BYTE] & 0xF) as usize).min(MAX_LANE_CAMPS);
    let (word, prefix_start): (u16, usize) = if is_value {
        (
            u16::from_le_bytes([
                lane[LANE_CHAMPION_BYTE_OFFSET],
                lane[LANE_CHAMPION_BYTE_OFFSET + 1],
            ]),
            VALUE_PREFIX_START,
        )
    } else {
        (0, SAFE_PREFIX_START)
    };
    LaneMemory {
        camps: read_points(&lane[..LANE_POINTS_BYTES], camp_count),
        altars: read_points(&lane[LANE_POINTS_BYTES..LANE_COUNT_BYTE], altar_count),
        prefix_gold_sum: lane[prefix_start..prefix_start + camp_count].to_vec(),
        prefix_gem_sum: lane
            [prefix_start + MAX_LANE_CAMPS..prefix_start + MAX_LANE_CAMPS + camp_count]
            .to_vec(),
        danger: (word >> 12) as u8,
        champion_slots: [
            (word & 0xF) as u8,
            ((word >> 4) & 0xF) as u8,
            ((word >> 8) & 0xF) as u8,
        ],
    }
}

fn parse_lane_set(
    bytes: &[u8; DRAFT_MEMORY_BYTES],
    offset: usize,
    lane_bytes: usize,
    is_value: bool,
    spanning_byte: u8,
) -> LaneSet {
    let lane_count: usize = ((spanning_byte & !NOT_SPANNING_FLAG) as usize).min(LANE_SLOTS);
    let lanes: Vec<LaneMemory> = (0..lane_count)
        .map(|lane_index: usize| {
            let start: usize = offset + lane_index * lane_bytes;
            parse_lane(&bytes[start..start + lane_bytes], is_value)
        })
        .collect();
    LaneSet {
        lanes,
        complete: spanning_byte & NOT_SPANNING_FLAG == 0,
    }
}

impl ChampionMemory {
    pub fn from_bytes(bytes: &[u8; DRAFT_MEMORY_BYTES]) -> Self {
        let initial_states: Vec<InitialChampionState> = (0..CHAMPION_STATES)
            .filter_map(|state_index: usize| {
                let start: usize = state_index * CHAMPION_STATE_BYTES;
                let state: &[u8] = &bytes[start..start + CHAMPION_STATE_BYTES];
                if state[0] == 0 && state[1] == 0 && state[2] == 0 {
                    return None;
                }
                Some(InitialChampionState {
                    champion_index: state[0] >> 4,
                    starting_lane: state[0] & 0xF,
                    ward_location: point(state[1], state[2]),
                })
            })
            .collect();

        let contested_camps: [Vec2; CONTESTED_CAMPS] =
            std::array::from_fn::<Vec2, CONTESTED_CAMPS, _>(|slot: usize| {
                let start: usize = CONTESTED_OFFSET + slot * POINT_BYTES;
                point(bytes[start], bytes[start + 1])
            });

        let spanning_byte: u8 = bytes[SPANNING_OFFSET];
        let value_count: u8 = spanning_byte >> 4;
        let safe_count: u8 = spanning_byte & 0x0F;

        Self {
            initial_states,
            value_lanes: parse_lane_set(bytes, LANES_OFFSET, VALUE_LANE_BYTES, true, value_count),
            safe_lanes: parse_lane_set(bytes, SAFE_OFFSET, SAFE_LANE_BYTES, false, safe_count),
            contested_camps,
        }
    }
}
