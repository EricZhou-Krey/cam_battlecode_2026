use crate::champion::strategy_state::{ChampionMemory, ChampionStrategyState, StrategyState};

#[derive(Debug, Clone, PartialEq)]
pub struct CombatStrategy;
impl StrategyState for CombatStrategy {
    fn init_data(memory: &mut ChampionMemory) {
        // TODO
    }

    fn turn(memory: &mut ChampionMemory) {
        // TODO
    }

    fn get_state_switch(memory: &mut ChampionMemory) -> Option<ChampionStrategyState> {
        None
    }
}
