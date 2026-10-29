use crate::{
    action::{Action, ActionGroupType},
    memory::{champion_memory::ChampionMemory, LocationID},
};
use cambc::DistanceField;

pub const MOVE_PROXIMITY: u16 = 3;

#[derive(Debug, Clone, PartialEq)]
pub struct MovementState {
    pub next_action: Box<Option<Action>>,
    pub distance_field: Option<DistanceField>,
    pub location_id: Option<LocationID>,
    pub accepted_distance: u16,
    pub action_group_type: ActionGroupType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MoveToSpawnState {}

pub trait MovementActions {
    fn move_to_location(&mut self, champion_memory: &mut ChampionMemory);
    fn escape_to_spawn(&mut self, champion_memory: &mut ChampionMemory);
}

impl MovementActions for Action {
    fn move_to_location(&mut self, champion_memory: &mut ChampionMemory) {}
    fn escape_to_spawn(&mut self, champion_memory: &mut ChampionMemory) {}
}
