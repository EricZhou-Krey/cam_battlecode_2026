use std::collections::HashSet;

use crate::{
    camp::{
        camp::{CampGroupTyped, CampTypeExtension},
        combat::TargetPriorityExtension,
    },
    champion::{
        memory::draft_extracted_memory::{DraftExtractedChampionMemory, LaneMemory},
        strategy_state::StrategyData,
    },
};
use cambc::{
    Bitboard, Camp, CampGroup, ChampionDraft, ChampionType, DistanceField, ItemPassive, Team, Vec2,
};

pub mod draft_extracted_memory;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CampID(pub usize);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CampInternalID(pub usize);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AltarID(pub usize);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LaneID(pub usize);

#[derive(Debug, Clone, PartialEq)]
pub struct ChampionMemory {
    pub camp_types: Vec<CampGroupTyped>,
    pub camp_target_order: Vec<Vec<CampInternalID>>,
    pub camp_distance_fields: Vec<DistanceField>,

    pub camp_centers: Vec<Vec2>,
    pub altar_positions: Vec<Vec2>,

    pub state: ChampionState,
    pub strategy_data: StrategyData,

    pub cleared_camps: Vec<(i32, CampID)>,

    pub ally_states: Vec<ChampionState>,
    pub enemy_states: Vec<ChampionState>,

    pub value_lanes: Vec<Lane>,
    pub safe_lanes: Vec<Lane>,

    pub contested_camps: Vec<CampID>,
}

impl ChampionMemory {
    pub fn update(&mut self) {
        self.cleared_camps.retain_mut(|(count, _position)| {
            *count = count.saturating_sub(1);
            *count > 0
        });
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChampionState {
    pub champion_type: ChampionType,
    pub lane: Option<LaneID>,
    pub gold_held: Option<i32>,
    pub gems_held: Option<i32>,
    pub active_passives: Option<HashSet<ItemPassive>>,
    pub cooldown_status: Option<[i32; 2]>,
    pub stat_bonus: Option<[i32; 3]>,
    pub ward_position: Vec2,
}

impl Default for ChampionState {
    fn default() -> Self {
        Self {
            champion_type: ChampionType::Ren,
            lane: None,
            gold_held: None,
            gems_held: None,
            active_passives: None,
            cooldown_status: None,
            stat_bonus: None,
            ward_position: Vec2 { x: 0, y: 0 },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lane {
    pub camps: Vec<CampID>,
    pub altars: Vec<AltarID>,
    pub complete: bool,
    pub prefix_gold_sum: Vec<i32>,
    pub prefix_gem_sum: Vec<i32>,
    pub danger: i32,
    pub champions_in_lane: Vec<(Team, ChampionType)>,
}

impl Lane {
    fn from_extracted(
        lane: LaneMemory,
        complete: bool,
        camp_centers: &mut Vec<Vec2>,
        altar_positions: &mut Vec<Vec2>,
    ) -> Self {
        let camps = lane
            .camps
            .into_iter()
            .map(|pos| {
                if let Some(idx) = camp_centers.iter().position(|&p| p == pos) {
                    CampID(idx)
                } else {
                    let idx = camp_centers.len();
                    camp_centers.push(pos);
                    CampID(idx)
                }
            })
            .collect();

        let altars = lane
            .altars
            .into_iter()
            .map(|pos| {
                if let Some(idx) = altar_positions.iter().position(|&p| p == pos) {
                    AltarID(idx)
                } else {
                    let idx = altar_positions.len();
                    altar_positions.push(pos);
                    AltarID(idx)
                }
            })
            .collect();

        Self {
            camps,
            altars,
            complete,
            prefix_gold_sum: lane
                .prefix_gold_sum
                .into_iter()
                .map(|value: u8| value as i32)
                .collect(),
            prefix_gem_sum: lane
                .prefix_gem_sum
                .into_iter()
                .map(|value: u8| value as i32)
                .collect(),
            danger: lane.danger as i32,
            champions_in_lane: Vec::new(),
        }
    }
}

fn champion_types(draft: ChampionDraft) -> &'static [ChampionType] {
    match draft {
        ChampionDraft::Ryth => &[ChampionType::Ryth],
        ChampionDraft::VesperVale => &[ChampionType::VesperVale],
        ChampionDraft::Vendetta => &[ChampionType::Vendetta],
        ChampionDraft::CassiAndIssac => &[ChampionType::Cassi, ChampionType::Issac],
        ChampionDraft::Stellar => &[ChampionType::Stellar],
        ChampionDraft::Prism => &[ChampionType::Prism],
        ChampionDraft::Marcella => &[ChampionType::Marcella],
        ChampionDraft::Willow => &[ChampionType::Willow],
        ChampionDraft::Ren => &[ChampionType::Ren],
        ChampionDraft::Varg => &[ChampionType::Varg],
        ChampionDraft::Tavi => &[ChampionType::Tavi],
    }
}

impl From<DraftExtractedChampionMemory> for ChampionMemory {
    fn from(extracted: DraftExtractedChampionMemory) -> Self {
        let DraftExtractedChampionMemory {
            champion_states,
            value_lanes,
            safe_lanes,
            contested_camps,
        } = extracted;

        let value_complete: bool = value_lanes.complete;
        let safe_complete: bool = safe_lanes.complete;

        let mut camp_centers: Vec<Vec2> = Vec::new();
        let mut altar_positions: Vec<Vec2> = Vec::new();

        let mut value_lanes: Vec<Lane> = value_lanes
            .lanes
            .into_iter()
            .map(|lane: LaneMemory| {
                Lane::from_extracted(
                    lane,
                    value_complete,
                    &mut camp_centers,
                    &mut altar_positions,
                )
            })
            .collect();

        let safe_lanes: Vec<Lane> = safe_lanes
            .lanes
            .into_iter()
            .map(|lane: LaneMemory| {
                Lane::from_extracted(lane, safe_complete, &mut camp_centers, &mut altar_positions)
            })
            .collect();

        let contested_camps: Vec<CampID> = contested_camps
            .into_iter()
            .map(|pos| {
                if let Some(idx) = camp_centers.iter().position(|&p| p == pos) {
                    CampID(idx)
                } else {
                    let idx = camp_centers.len();
                    camp_centers.push(pos);
                    CampID(idx)
                }
            })
            .collect();

        let mut champion_state: Option<ChampionState> = None;
        let mut ally_states: Vec<ChampionState> = Vec::with_capacity(champion_states.len() + 1);
        for state in &champion_states {
            let Some(draft) = state.champion_draft() else {
                continue;
            };
            let starting_lane: usize = state.starting_lane as usize;
            let lane_index: Option<usize> = if starting_lane < value_lanes.len() {
                Some(starting_lane)
            } else {
                None
            };
            for &champion_type in champion_types(draft) {
                if champion_type
                    == cambc::self_champion()
                        .unwrap()
                        .unwrap()
                        .champion_name()
                        .unwrap()
                {
                    champion_state = Some(ChampionState {
                        champion_type,
                        lane: lane_index.map(LaneID),
                        ward_position: state.ward_location,
                        ..Default::default()
                    })
                } else {
                    ally_states.push(ChampionState {
                        champion_type,
                        lane: lane_index.map(LaneID),
                        ward_position: state.ward_location,
                        ..Default::default()
                    });
                }
                if let Some(index) = lane_index {
                    value_lanes[index]
                        .champions_in_lane
                        .push((Team::Ally, champion_type));
                }
            }
        }

        let blocked_bitboard: Bitboard = cambc::map::walls()
            .unwrap()
            .union(cambc::map::holes().unwrap())
            .unwrap();

        let camp_distance_fields: Vec<DistanceField> = camp_centers
            .iter()
            .map(|center: &Vec2| {
                let mut center_board: Bitboard = Bitboard::new().unwrap();
                let _ = center_board.set(*center, true);
                let mut distance: DistanceField = DistanceField::new().unwrap();
                let _ = distance.compute(
                    &blocked_bitboard,
                    &center_board,
                    &mut Bitboard::new().unwrap(),
                    u16::MAX,
                );
                distance
            })
            .collect();

        let camp_groups: Vec<CampGroup> = cambc::map::camp_groups().unwrap();
        let n_camp_groups: usize = camp_groups.len();

        let camp_types: Vec<CampGroupTyped> = camp_groups
            .iter()
            .map(|camp_group: &CampGroup| CampGroupTyped {
                types: camp_group.camps().iter().map(Camp::camp_type).collect(),
            })
            .collect();

        let camp_target_order: Vec<Vec<CampInternalID>> = (0..n_camp_groups)
            .map(|camp_group_index: usize| {
                let camps: &[Camp] = camp_groups[camp_group_index].camps();
                let mut camp_ids: Vec<CampInternalID> =
                    (0..camps.len()).map(CampInternalID).collect();
                camp_ids.sort_unstable_by_key(|camp_id: &CampInternalID| {
                    camps[camp_id.0].target_priority()
                });
                camp_ids
            })
            .collect();

        Self {
            camp_distance_fields,
            camp_target_order,
            camp_types,
            camp_centers,
            altar_positions,
            state: champion_state.unwrap(),
            strategy_data: StrategyData::default(),
            cleared_camps: Vec::new(),
            ally_states,
            enemy_states: Vec::new(),
            value_lanes,
            safe_lanes,
            contested_camps,
        }
    }
}
