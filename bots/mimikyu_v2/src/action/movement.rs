use crate::{
    action::{
        combat::kit::{tile_distance, try_dash, try_step_away, visible_enemies, DASH_MINIMUM_GAIN},
        route::{capabilities, dash_forward, follow_route, Capabilities, Route},
        Action, ActionGroupType,
    },
    memory::{champion_memory::ChampionMemory, CampGroupID, LocationID},
    sdk_interact::{champion::passive_active, map::step_with_fallback},
};
use cambc::{Bitboard, Champion, DistanceField, ItemPassive, Team, Vec2};

pub const MOVE_PROXIMITY: u16 = 3;
pub const RECALL_TIME: u16 = 5;
pub const RECALL_TIME_HOME: u16 = 3;
pub const MAX_ROUTE_FAILURES: u8 = 2;
pub const ESCAPE_THREAT_RANGE: i32 = 6;
pub const CAMP_ESCAPE_MARGIN: i32 = 2;

fn threat_sources() -> Vec<(Vec2, i32)> {
    let mut threats: Vec<(Vec2, i32)> = Vec::new();

    for enemy in visible_enemies() {
        if let Ok(position) = enemy.position() {
            threats.push((position, ESCAPE_THREAT_RANGE));
        }
    }

    if let Ok(camp_groups) = cambc::map::camp_groups() {
        for camp_group in camp_groups.iter() {
            for camp in camp_group.camps() {
                if camp.is_dead().unwrap_or(true) || !camp.is_awake().unwrap_or(false) {
                    continue;
                }
                let Ok(stats) = camp.base_stats() else {
                    continue;
                };
                threats.push((
                    camp.spawn_position(),
                    stats.base_attack_range + CAMP_ESCAPE_MARGIN,
                ));
            }
        }
    }
    threats
}

fn clearance(tile: Vec2, threats: &[(Vec2, i32)]) -> i32 {
    threats
        .iter()
        .map(|(position, reach)| tile_distance(tile, *position) - reach)
        .min()
        .unwrap_or(i32::MAX)
}

fn disengage(me: &Champion, blocked: &Bitboard, threats: &[(Vec2, i32)]) {
    let Ok(position): Result<Vec2, _> = me.position() else {
        return;
    };
    let score = |tile: Vec2| -clearance(tile, threats);
    let _ = try_dash(me, blocked, score(position), DASH_MINIMUM_GAIN, score);

    let position_after_dash: Vec2 = me.position().unwrap_or(position);
    if let Some((nearest, _)) = threats
        .iter()
        .min_by_key(|(threat, reach)| tile_distance(position_after_dash, *threat) - reach)
    {
        let _ = try_step_away(me, *nearest);
    }
}
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

        let routed: bool = follow_route(me, state, champion_memory, current_capabilities);

        if !routed {
            if !step_with_fallback(&state.distance_field, target_position) {
                *self = Self::Idle;
                return;
            }
            dash_forward(me, state, champion_memory, current_capabilities);
        }

        dash_forward(me, state, champion_memory, current_capabilities);

        let new_position: Vec2 = me.position().unwrap();
        let distance_to_goal: u16 = state.distance_field.get(new_position).unwrap_or(u16::MAX);
        if distance_to_goal <= state.accepted_distance {
            let Some(valid_next_action): Option<Self> = state.next_action.take() else {
                *self = Self::Idle;
                return;
            };
            *self = valid_next_action;
        }
    }

    fn escape_to_spawn(&mut self, champion_memory: &mut ChampionMemory) {
        let me: &Champion = cambc::self_champion().unwrap().unwrap();
        let Ok(me_position): Result<Vec2, _> = me.position() else {
            *self = Self::Idle;
            return;
        };

        let threats: Vec<(Vec2, i32)> = threat_sources();
        let threatened: bool = clearance(me_position, &threats) <= 0;
        if me.is_recalling().unwrap_or(false) && !threatened {
            return;
        }

        let in_spawn_region: bool = match cambc::map::spawn_region(Team::Ally) {
            Ok(region) => match region.is_set(me_position) {
                Ok(is_set) => is_set,
                Err(error) => {
                    println!(
                        "escape_to_spawn: invalid position ({}, {}): {:?}",
                        me_position.x, me_position.y, error
                    );
                    *self = Self::Idle;
                    return;
                }
            },
            Err(_) => false,
        };
        if in_spawn_region {
            *self = Self::Idle;
            return;
        }
        if threatened {
            disengage(
                me,
                &champion_memory.map.distance_fields.blocked_bitboard,
                &threats,
            );
            return;
        }
        let Some(spawn_distance_field): Option<DistanceField> = champion_memory
            .map
            .distance_fields
            .try_get_spawn_distance_field()
            .cloned()
        else {
            if !threatened {
                let _ = me.recall();
            }
            return;
        };

        let recall_time: u16 = if passive_active(ItemPassive::Home, me) {
            RECALL_TIME_HOME
        } else {
            RECALL_TIME
        };

        if spawn_distance_field.get(me_position).unwrap_or(u16::MAX) <= recall_time {
            *self = Self::MoveToLocation(MovementState {
                next_action: Box::new(None),
                distance_field: spawn_distance_field,
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
