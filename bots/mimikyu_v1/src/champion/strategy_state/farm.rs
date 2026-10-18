use crate::{
    camp::combat::CampCombatPredictionExtension,
    champion::{
        memory::CampID,
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
        memory.strategy_data = StrategyData::Farm {
            current_action: FarmAction::MoveToCamp,
            target_camp: None,
            target_altar: None,
        };
    }

    fn turn(memory: &mut ChampionMemory) {
        let StrategyData::Farm {
            current_action,
            target_camp,
            target_altar: _,
        } = &memory.strategy_data
        else {
            println!("State - Data Desync Error in: State {:?} - Data {:?}, fallback init data and calling turn again may be unintended behaviour", Self, memory.strategy_data);
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
                let move_to_camp_id: CampID = match target_camp {
                    Some(camp_id) => *camp_id,
                    None => {
                        let camp_id: CampID = Self::find_target_camp(memory);
                        match &mut memory.strategy_data {
                            StrategyData::Farm { target_camp, .. } => {
                                *target_camp = Some(camp_id);
                            }
                            _ => return,
                        }
                        camp_id
                    }
                };

                let _: bool = step_with_fallback(
                    me,
                    memory.camp_distance_fields.get(move_to_camp_id),
                    memory.camp_centers[move_to_camp_id.0],
                );
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
