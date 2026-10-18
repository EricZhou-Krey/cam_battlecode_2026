use crate::champion::{
    memory::CampID,
    strategy_state::{ChampionMemory, ChampionStrategyState, StrategyData, StrategyState},
};
use cambc::{CampGroup, Vec2};

#[derive(Debug, Clone, PartialEq)]
pub struct FarmStrategy;
impl FarmStrategy {
    fn find_closest_camp(memory: &ChampionMemory) -> CampID {
        let champion_position: Vec2 = cambc::self_champion().unwrap().unwrap().position().unwrap();
        // Inside lane, is not cleared, ... etc
        CampID(
            memory
                .camp_distance_fields
                .iter()
                .enumerate()
                .fold(
                    (0_u16, 0_usize),
                    |(closest_distance, closest_index), (current_index, distance_field)| {
                        let Ok(current_distance) = distance_field.get(champion_position) else {
                            return (closest_distance, closest_index);
                        };
                        if current_distance < closest_distance {
                            (current_distance, current_index)
                        } else {
                            (closest_distance, closest_index)
                        }
                    },
                )
                .1,
        )
    }

    fn find_target_camp(memory: &ChampionMemory) -> Option<CampID> {
        let champion_position: Vec2 = cambc::self_champion().unwrap().unwrap().position().unwrap();
        let target_camps: Vec<CampGroup> = cambc::map::camp_groups()
            .unwrap()
            .into_iter()
            .filter(|camp_group: &CampGroup| {
                todo!();
            })
            .collect();
    }
}

impl StrategyState for FarmStrategy {
    fn init_data(memory: &mut ChampionMemory) {
        let potential_camp: Option<CampID> = Self::find_target_camp(memory);
        memory.strategy_data = StrategyData::Farm {
            target_camp: potential_camp,
        };
    }

    fn turn(memory: &mut ChampionMemory) {
        // TODO
    }

    fn get_state_switch(memory: &mut ChampionMemory) -> Option<ChampionStrategyState> {
        None
    }
}
