use crate::{action::Action, memory::champion_memory::ChampionMemory};

#[derive(Debug, Clone, PartialEq)]
pub struct ChampionCombatState {}

#[derive(Debug, Clone, PartialEq)]
pub struct DefendState {}

pub trait CombatActions {
    fn champion_combat_stall(&mut self, champion_memory: &mut ChampionMemory);
    fn champion_combat_kill(&mut self, champion_memory: &mut ChampionMemory);
    fn champion_combat_escape(&mut self, champion_memory: &mut ChampionMemory);
    fn defend(&mut self, champion_memory: &mut ChampionMemory);
}

impl CombatActions for Action {
    fn champion_combat_stall(&mut self, champion_memory: &mut ChampionMemory) {}
    fn champion_combat_kill(&mut self, champion_memory: &mut ChampionMemory) {}
    fn champion_combat_escape(&mut self, champion_memory: &mut ChampionMemory) {}
    fn defend(&mut self, champion_memory: &mut ChampionMemory) {}
}
