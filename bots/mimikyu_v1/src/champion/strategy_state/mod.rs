use crate::champion::{
    champion_preference::CHAMPION_CHOICE_STRATEGY,
    memory::{CampID, ChampionMemory},
    strategy_state::{
        farm::FarmStrategy, hunt::HuntStrategy, scout::ScoutStrategy, sneak::SneakStrategy,
    },
};

pub mod farm;
pub mod hunt;
pub mod scout;
pub mod sneak;

#[derive(Debug, Default, Clone, PartialEq)]
pub enum ChampionStrategyState {
    #[default]
    Idle,
    Farm,
    Scout,
    Hunt,
    Sneak,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub enum StrategyData {
    #[default]
    Idle,
    Farm {
        target_camp: Option<CampID>,
    },
    Scout,
    Hunt,
    Sneak,
}

pub trait StrategyState {
    fn init_data(memory: &mut ChampionMemory);
    fn turn(memory: &mut ChampionMemory);
    fn get_state_switch(memory: &mut ChampionMemory) -> Option<ChampionStrategyState>;
}

impl ChampionStrategyState {
    pub fn new(memory: &ChampionMemory) -> ChampionStrategyState {
        CHAMPION_CHOICE_STRATEGY[memory.state.champion_type as usize].clone()
    }

    pub fn turn(&mut self, memory: &mut ChampionMemory) {
        match self {
            Self::Idle => (),
            Self::Farm => FarmStrategy::turn(memory),
            Self::Scout => ScoutStrategy::turn(memory),
            Self::Hunt => HuntStrategy::turn(memory),
            Self::Sneak => SneakStrategy::turn(memory),
        }
    }

    pub fn resolve_state(&mut self, memory: &mut ChampionMemory) {
        let switch_state: Option<ChampionStrategyState> = match self {
            Self::Idle => None,
            Self::Farm => FarmStrategy::get_state_switch(memory),
            Self::Scout => ScoutStrategy::get_state_switch(memory),
            Self::Hunt => HuntStrategy::get_state_switch(memory),
            Self::Sneak => SneakStrategy::get_state_switch(memory),
        };

        if let Some(state) = switch_state {
            *self = state;
        }
    }
}
