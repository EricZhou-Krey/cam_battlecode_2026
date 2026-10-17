pub mod farming;

#[derive(Debug, Clone, PartialEq)]
pub enum ChampionStrategyState {
    Farming,
    Scout,
}
