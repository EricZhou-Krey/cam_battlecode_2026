use crate::{
    action::Action,
    memory::{champion_memory::ChampionMemory, CampGroupID},
};

#[derive(Debug, Clone, PartialEq)]
pub struct CampGroupCombatState {
    pub camp_group_id: CampGroupID,
    pub current_target_camp: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CollectionState {}

#[derive(Debug, Clone, PartialEq)]
pub struct TributeState {}

pub trait FarmActions {
    fn camp_group_combat(&mut self, champion_memory: &mut ChampionMemory);
    fn collection(&mut self, champion_memory: &mut ChampionMemory);
    fn tributing_at_altar(&mut self, champion_memory: &mut ChampionMemory);
}

impl FarmActions for Action {
    fn camp_group_combat(&mut self, champion_memory: &mut ChampionMemory) {}
    fn collection(&mut self, champion_memory: &mut ChampionMemory) {}
    fn tributing_at_altar(&mut self, champion_memory: &mut ChampionMemory) {}
}
