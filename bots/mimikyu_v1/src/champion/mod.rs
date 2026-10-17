pub mod debug;
pub mod farming;
pub mod memory;

#[derive(Debug, Clone, PartialEq)]
pub enum ChampionStrategyState {
    Farming,
    Scout,
}
