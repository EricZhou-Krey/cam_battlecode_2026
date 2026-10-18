use crate::champion::strategy_state::ChampionStrategyState;

/* For refernce:
pub enum ChampionType {
    Ryth = 0,
    VesperVale = 1,
    Vendetta = 2,
    Cassi = 3,
    Issac = 4,
    Stellar = 5,
    Prism = 6,
    Marcella = 7,
    Willow = 8,
    Ren = 9,
    Varg = 10,
    Tavi = 11
}
*/
pub const CHAMPION_CHOICE_STRATEGY: &[ChampionStrategyState; 12] = &[
    ChampionStrategyState::Scout, // Ryth
    ChampionStrategyState::Sneak, // VesperVale
    ChampionStrategyState::Sneak, // Vendetta
    ChampionStrategyState::Farm,  // Cassi
    ChampionStrategyState::Farm,  // Issac
    ChampionStrategyState::Hunt,  // Stellar
    ChampionStrategyState::Farm,  // Prism
    ChampionStrategyState::Scout, // Marcella
    ChampionStrategyState::Farm,  // Willow
    ChampionStrategyState::Scout, // Ren
    ChampionStrategyState::Farm,  // Varg
    ChampionStrategyState::Scout, // Tavi
];

pub const CHAMPION_STAT_CHOICE_RATIO: &[[i32; 3]; 12] = &[
    // [HP, STR, FOC]
    [1, 0, 0], // Ryth
    [0, 1, 0], // VesperVale
    [0, 1, 0], // Vendetta
    [0, 0, 1], // Cassi
    [1, 1, 0], // Issac
    [0, 0, 1], // Stellar
    [0, 0, 1], // Prism
    [1, 0, 0], // Marcella
    [0, 0, 1], // Willow
    [1, 0, 3], // Ren
    [0, 1, 0], // Varg
    [0, 1, 0], // Tavi
];
