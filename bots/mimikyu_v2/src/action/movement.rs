use crate::{
    action::{
        route::{capabilities, dash_forward, follow_route, Capabilities, Route},
        Action, ActionGroupType,
    },
    memory::{champion_memory::ChampionMemory, CampGroupID, LocationID},
    sdk_interact::{champion::passive_active, map::step_with_fallback},
};
use cambc::{Champion, DistanceField, ItemPassive, Team, Vec2};

pub const MOVE_PROXIMITY: u16 = 3;
pub const RECALL_TIME: u16 = 5;
pub const RECALL_TIME_HOME: u16 = 3;

pub const MAX_ROUTE_FAILURES: u8 = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct MovementState {
    pub next_action: Box<Option<Action>>,
    pub distance_field: DistanceField,
    pub location_id: LocationID,
    pub accepted_distance: u16,
    pub action_group_type: ActionGroupType,
    pub route: Option<Route>,
    pub route_failures: u8,
    pub allow_route: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MoveToSpawnState {}

pub trait MovementActions {
    fn move_to_location(&mut self, champion_memory: &mut ChampionMemory);
    fn escape_to_spawn(&mut self, champion_memory: &mut ChampionMemory);
}

impl MovementActions for Action {
    fn move_to_location(&mut self, champion_memory: &mut ChampionMemory) {
        let Self::MoveToLocation(state) = self else {
            println!("Error: State Action Mismatch");
            return;
        };

        let me: &Champion = cambc::self_champion().unwrap().unwrap();
        if me.is_teleporting().unwrap_or(false) {
            return;
        }

        let current_capabilities: Capabilities = capabilities(me);
        let target_position: Vec2 = champion_memory
            .map
            .points_of_interest
            .locate(state.location_id);

        if step_with_fallback(&state.distance_field, target_position) {
            state.route = None;
            state.route_failures = 0;
            dash_forward(me, state, champion_memory, current_capabilities);
        } else {
            follow_route(me, state, champion_memory, current_capabilities);
        }

        let new_position: Vec2 = me.position().unwrap();
        let distance_to_goal: u16 = state.distance_field.get(new_position).unwrap_or(u16::MAX);
        if distance_to_goal <= state.accepted_distance {
            let Some(valid_next_action): Option<Self> = state.next_action.take() else {
                *self = Self::Idle;
                self.turn(champion_memory);
                return;
            };
            *self = valid_next_action;
        }
    }

    fn escape_to_spawn(&mut self, champion_memory: &mut ChampionMemory) {
        let me: &Champion = cambc::self_champion().unwrap().unwrap();
        let me_position: Vec2 = me.position().unwrap();
        if me.is_recalling().unwrap() {
            return;
        } else if cambc::map::spawn_region(Team::Ally)
            .unwrap()
            .is_set(me_position)
            .unwrap()
        {
            *self = Self::Idle;
            return;
        }

        let Some(spawn_distance_field): Option<&DistanceField> = champion_memory
            .map
            .distance_fields
            .try_get_spawn_distance_field()
        else {
            let _ = me.recall();
            return;
        };

        let recall_time: u16 = if passive_active(ItemPassive::Home, me) {
            RECALL_TIME_HOME
        } else {
            RECALL_TIME
        };

        if spawn_distance_field.get(me_position).unwrap() <= recall_time {
            *self = Self::MoveToLocation(MovementState {
                next_action: Box::new(None),
                distance_field: spawn_distance_field.clone(),
                location_id: LocationID::CampGroup(CampGroupID(0)),
                accepted_distance: 0,
                action_group_type: ActionGroupType::Survival,
                route: None,
                allow_route: false,
                route_failures: 0,
            });
        } else {
            let _ = me.recall();
        }
    }
}
