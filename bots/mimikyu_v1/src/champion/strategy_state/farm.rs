use crate::{
    camp::combat::CampCombatPredictionExtension,
    champion::{
        memory::{AltarID, CampID},
        strategy_state::{ChampionMemory, ChampionStrategyState, StrategyData, StrategyState},
    },
    movement::step_with_fallback,
};
use cambc::Champion;

pub const HP_DELTA_REQUIREMENT: f32 = 100.;

#[derive(Debug, Clone, PartialEq)]
pub enum FarmAction {
    HealingAtSpawn,
    MoveToCamp,
    InCombatWithCamp,
    CapturingCamp,
    DeliverToAltar,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FarmData {
    current_action: FarmAction,
    target_camp: Option<CampID>,
    target_altar: Option<AltarID>,
    target_internal_camp_priority_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FarmStrategy;
impl FarmStrategy {
    fn find_target_camp(memory: &mut ChampionMemory) -> CampID {
        let mut valid_camps: Vec<CampID> = if let Some(lane_id) = memory.state.lane {
            memory.value_lanes[lane_id.0].camps.clone()
        } else {
            (0..memory.camp_centers.len()).map(CampID).collect()
        };

        valid_camps.retain(|camp_id: &CampID| {
            memory.basic_combat_health_delta(*camp_id, &[&memory.state]) > HP_DELTA_REQUIREMENT
                && !memory
                    .cleared_camps
                    .iter()
                    .map(|c| c.1)
                    .any(|c| c == *camp_id)
        });

        if valid_camps.is_empty() {
            if let Some(lane_id) = memory.state.lane {
                memory.value_lanes[lane_id.0].camps[0]
            } else {
                (0..memory.camp_centers.len()).map(CampID).next().unwrap()
            }
        } else {
            valid_camps[0]
        }
    }
}

impl StrategyState for FarmStrategy {
    fn init_data(memory: &mut ChampionMemory) {
        memory.strategy_data = StrategyData::Farm(FarmData {
            current_action: FarmAction::MoveToCamp,
            target_camp: None,
            target_altar: None,
            target_internal_camp_priority_index: None,
        });
    }

    fn turn(memory: &mut ChampionMemory) {
        let StrategyData::Farm(FarmData {
            current_action,
            target_camp,
            ..
        }) = memory.strategy_data.clone()
        else {
            println!(
                "State - Data Desync Error in: State {:?} - Data {:?}",
                Self, memory.strategy_data
            );
            Self::init_data(memory);
            Self::turn(memory);
            return;
        };

        let me: &Champion = cambc::self_champion().unwrap().unwrap();

        match current_action {
            FarmAction::HealingAtSpawn => {
                todo!();
            }
            FarmAction::MoveToCamp => {
                let move_to_camp_id = match target_camp {
                    Some(camp_id) => camp_id,
                    None => {
                        let camp_id = Self::find_target_camp(memory);

                        if let StrategyData::Farm(FarmData { target_camp, .. }) =
                            &mut memory.strategy_data
                        {
                            *target_camp = Some(camp_id);
                        }

                        camp_id
                    }
                };

                let _ = step_with_fallback(
                    me,
                    memory.camp_distance_fields.get(move_to_camp_id),
                    cambc::map::camp_groups().unwrap()[move_to_camp_id.0].camps()
                        [memory.camp_target_order[move_to_camp_id.0][0].0]
                        .position()
                        .unwrap(),
                );

                if memory
                    .camp_distance_fields
                    .get(move_to_camp_id)
                    .get(me.position().unwrap())
                    .unwrap()
                    <= 2
                {
                    if let StrategyData::Farm(FarmData {
                        current_action,
                        target_internal_camp_priority_index,
                        ..
                    }) = &mut memory.strategy_data
                    {
                        *current_action = FarmAction::InCombatWithCamp;
                        *target_internal_camp_priority_index = Some(0);
                    }
                }
            }
            FarmAction::InCombatWithCamp => {
                todo!();
            }
            FarmAction::CapturingCamp => {
                todo!();
            }
            FarmAction::DeliverToAltar => {
                todo!();
            }
        }
    }

    fn get_state_switch(memory: &mut ChampionMemory) -> Option<ChampionStrategyState> {
        None
    }
}
