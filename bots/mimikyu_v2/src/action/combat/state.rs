use crate::{action::Action, memory::champion_memory::ChampionMemory};
use cambc::{Champion, ChampionType};

pub const COMBAT_PROXIMITY: u16 = 4;

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
    fn champion_combat_stall(&mut self, champion_memory: &mut ChampionMemory) {
        let me: &Champion = cambc::self_champion().unwrap().unwrap();

        match me.champion_name().unwrap() {
            ChampionType::Willow => (),
            ChampionType::HeartOfRuin => (),
            ChampionType::Varg => (),
            ChampionType::Ren => (),
            ChampionType::Vendetta => (),
            _ => (),
        }
    }
    fn champion_combat_kill(&mut self, champion_memory: &mut ChampionMemory) {
        let me: &Champion = cambc::self_champion().unwrap().unwrap();

        match me.champion_name().unwrap() {
            ChampionType::Willow => (),
            ChampionType::HeartOfRuin => (),
            ChampionType::Varg => (),
            ChampionType::Ren => (),
            ChampionType::Vendetta => (),
            _ => (),
        }
    }
    fn champion_combat_escape(&mut self, champion_memory: &mut ChampionMemory) {
        let me: &Champion = cambc::self_champion().unwrap().unwrap();

        match me.champion_name().unwrap() {
            ChampionType::Willow => (),
            ChampionType::HeartOfRuin => (),
            ChampionType::Varg => (),
            ChampionType::Ren => (),
            ChampionType::Vendetta => (),
            _ => (),
        }
    }
    fn defend(&mut self, champion_memory: &mut ChampionMemory) {
        let me: &Champion = cambc::self_champion().unwrap().unwrap();

        match me.champion_name().unwrap() {
            ChampionType::Willow => (),
            ChampionType::HeartOfRuin => (),
            ChampionType::Varg => (),
            ChampionType::Ren => (),
            ChampionType::Vendetta => (),
            _ => (),
        }
    }
}
