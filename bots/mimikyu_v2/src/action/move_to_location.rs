use crate::{
    action::ActionGroupType,
    memory::{champion_memory::ChampionMemory, LocationID},
};
use cambc::{DistanceField, Vec2};

const MINIMUM_EXTRAS_FUEL: u32 = 10_000;
const UNREACHED_DISTANCE: u16 = u16::MAX;
const DASH_MINIMUM_GAIN: u16 = 1;
const SURVIVAL_DASH_MINIMUM_GAIN: u16 = 1;
const DASH_ACTION_COST: u16 = 0;
const TELEPORT_ACTION_COST: u16 = 4;
const TELEPORT_MINIMUM_GAIN: u16 = 8;
const TELEPORT_RANGE: i32 = 3;
const SHORTCUT_LOOKAHEAD: i32 = 8;
const SHORTCUT_MINIMUM_SAVING: u16 = 4;
const SHORTCUT_APPROACH_SLACK: u16 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveToLocationResult {
    InProgress,
    Finished,
}

#[derive(Clone, Copy)]
struct ShortcutChoice {
    net_saving: u16,
    start: Vec2,
    landing_distance: u16,
    action_cost: u16,
    uses_teleport: bool,
    approach: i32,
}

pub fn move_to_location_turn(
    distance_field: &mut Option<DistanceField>,
    location_id: Option<LocationID>,
    accepted_distance: u16,
    action_group_type: ActionGroupType,
    champion_memory: &mut ChampionMemory,
) -> MoveToLocationResult {
    let Some(target_id): Option<LocationID> = location_id else {
        return MoveToLocationResult::Finished;
    };
    let Ok(Some(me)) = cambc::self_champion() else {
        return MoveToLocationResult::InProgress;
    };

    let Ok(position) = me.position() else {
        return MoveToLocationResult::InProgress;
    };
    if me.is_teleporting().unwrap_or(false) {
        return MoveToLocationResult::InProgress;
    }

    let spare_fuel: bool =
        cambc::fuel_remaining().is_ok_and(|fuel| fuel >= MINIMUM_EXTRAS_FUEL as _);
    let target: Vec2 = champion_memory.map.points_of_interest.locate(target_id);

    let teleport_crystal: Option<&cambc::Consumable> = me.consumables().iter().find(|consumable| {
        matches!(consumable.kind(), cambc::ConsumableType::TpCrystal)
            && consumable.count().unwrap_or(0) > 0
    });
    let dash_cooldown: Option<i32> = me
        .abilities()
        .iter()
        .filter(|ability| ability.is_dash())
        .filter_map(|ability| ability.cooldown().ok())
        .min();
    let shortcut_means_possible: bool = spare_fuel
        && (teleport_crystal.is_some() || dash_cooldown.is_some())
        && !champion_memory.map.points_of_interest.shortcuts.is_empty();

    let lazy_available: bool = match target_id {
        LocationID::CampGroup(camp_group_id) => {
            let lazy_fields = &champion_memory.map.distance_fields;
            match lazy_fields.camp_group_distance_fields.get(camp_group_id.0) {
                Some(Some(_)) => true,
                Some(None) => !lazy_fields.discovered_this_round,
                None => false,
            }
        }
        _ => false,
    };
    let needs_own_field: bool = distance_field.is_none() && !lazy_available;

    let blocked_bitboard: Option<cambc::Bitboard> = if needs_own_field || shortcut_means_possible {
        Some(champion_memory.map.distance_fields.blocked_bitboard.clone())
    } else {
        None
    };

    if needs_own_field {
        if let Some(blocked) = &blocked_bitboard {
            let roots: cambc::Bitboard = match target_id {
                LocationID::CampGroup(camp_group_id) => cambc::map::camp_groups().unwrap()
                    [camp_group_id.0]
                    .spawn_tiles()
                    .unwrap(),
                _ => {
                    let mut single_tile: cambc::Bitboard = cambc::Bitboard::new().unwrap();
                    single_tile.set(target, true).unwrap();
                    single_tile
                }
            };
            let open_bitboard: cambc::Bitboard =
                blocked.intersect(&roots.complement().unwrap()).unwrap();
            let mut own_field: DistanceField = DistanceField::new().unwrap();
            if own_field
                .compute(
                    &open_bitboard,
                    &roots,
                    &mut cambc::Bitboard::new().unwrap(),
                    u16::MAX,
                )
                .is_ok()
            {
                *distance_field = Some(own_field);
            }
        }
    }

    let field: &DistanceField = match (&*distance_field, target_id) {
        (Some(owned_field), _) => owned_field,
        (None, LocationID::CampGroup(camp_group_id)) => {
            match champion_memory.map.distance_fields.try_get(camp_group_id) {
                Some(cached_field) => cached_field,
                None => return MoveToLocationResult::InProgress,
            }
        }
        (None, _) => return MoveToLocationResult::InProgress,
    };

    let Ok(current_distance) = field.get(position) else {
        return MoveToLocationResult::InProgress;
    };
    if current_distance != UNREACHED_DISTANCE && current_distance <= accepted_distance {
        return MoveToLocationResult::Finished;
    }

    let minimum_dash_gain: u16 = if action_group_type == ActionGroupType::Survival {
        SURVIVAL_DASH_MINIMUM_GAIN
    } else {
        DASH_MINIMUM_GAIN
    };

    if spare_fuel && me.can_cast().unwrap_or(false) {
        for ability in me.abilities() {
            if !ability.is_dash() || !ability.is_ready().unwrap_or(false) {
                continue;
            }
            let Ok(landing_tiles) = ability.valid_tiles() else {
                continue;
            };
            let Ok(landing_positions) = landing_tiles.positions() else {
                continue;
            };

            let mut best_landing: Option<(u16, Vec2)> = None;
            for landing in landing_positions {
                let Ok(landing_distance) = field.get(landing) else {
                    continue;
                };
                if landing_distance == UNREACHED_DISTANCE {
                    continue;
                }
                if best_landing.is_none_or(|(best_distance, _)| landing_distance < best_distance) {
                    best_landing = Some((landing_distance, landing));
                }
            }

            if let Some((landing_distance, landing)) = best_landing {
                if current_distance.saturating_sub(landing_distance) >= minimum_dash_gain
                    && ability.cast_at(landing).unwrap_or(false)
                {
                    return MoveToLocationResult::InProgress;
                }
            }
        }
    }

    if shortcut_means_possible && current_distance != UNREACHED_DISTANCE {
        let holes: &cambc::Bitboard = cambc::map::holes().unwrap();
        let mut best_choice: Option<ShortcutChoice> = None;

        for (start, landing) in &champion_memory.map.points_of_interest.shortcuts {
            let approach: i32 = (start.x - position.x)
                .abs()
                .max((start.y - position.y).abs());
            if approach > SHORTCUT_LOOKAHEAD {
                continue;
            }

            let delta_x: i32 = landing.x - start.x;
            let delta_y: i32 = landing.y - start.y;
            let jump_length: i32 = delta_x.abs().max(delta_y.abs());
            let middle: Vec2 = Vec2 {
                x: start.x + delta_x / 2,
                y: start.y + delta_y / 2,
            };
            let dash_possible: bool = jump_length == 2
                && holes.is_set(middle).unwrap_or(false)
                && dash_cooldown.is_some_and(|cooldown| cooldown <= approach);
            let teleport_possible: bool =
                teleport_crystal.is_some() && jump_length <= TELEPORT_RANGE;
            if !dash_possible && !teleport_possible {
                continue;
            }

            let Ok(landing_distance) = field.get(*landing) else {
                continue;
            };
            if landing_distance == UNREACHED_DISTANCE {
                continue;
            }

            let action_cost: u16 = if dash_possible {
                DASH_ACTION_COST
            } else {
                TELEPORT_ACTION_COST
            };
            let total_cost: u16 = (approach as u16)
                .saturating_add(action_cost)
                .saturating_add(landing_distance);
            let net_saving: u16 = current_distance.saturating_sub(total_cost);
            if net_saving < SHORTCUT_MINIMUM_SAVING {
                continue;
            }

            if best_choice.is_none_or(|best| net_saving > best.net_saving) {
                best_choice = Some(ShortcutChoice {
                    net_saving,
                    start: *start,
                    landing_distance,
                    action_cost,
                    uses_teleport: !dash_possible,
                    approach,
                });
            }
        }

        if let Some(choice) = best_choice {
            if choice.start == position {
                if !choice.uses_teleport {
                    return MoveToLocationResult::InProgress;
                }
                if let Some(crystal) = teleport_crystal {
                    let mut crystal_landings: Vec<(u16, Vec2)> = Vec::new();
                    for offset_y in -TELEPORT_RANGE..=TELEPORT_RANGE {
                        for offset_x in -TELEPORT_RANGE..=TELEPORT_RANGE {
                            if offset_x == 0 && offset_y == 0 {
                                continue;
                            }
                            let tile: Vec2 = Vec2 {
                                x: position.x + offset_x,
                                y: position.y + offset_y,
                            };
                            if let Ok(tile_distance) = field.get(tile) {
                                if tile_distance != UNREACHED_DISTANCE {
                                    crystal_landings.push((tile_distance, tile));
                                }
                            }
                        }
                    }
                    crystal_landings.sort_unstable_by_key(|(tile_distance, _)| *tile_distance);

                    for (tile_distance, tile) in crystal_landings.into_iter().take(3) {
                        if current_distance.saturating_sub(tile_distance) < TELEPORT_MINIMUM_GAIN {
                            break;
                        }
                        if crystal.use_at(tile).unwrap_or(false) {
                            return MoveToLocationResult::InProgress;
                        }
                    }
                }
            } else if let Some(blocked) = &blocked_bitboard {
                let mut start_roots: cambc::Bitboard = cambc::Bitboard::new().unwrap();
                start_roots.set(choice.start, true).unwrap();
                let mut approach_field: DistanceField = DistanceField::new().unwrap();
                let maximum_walk: u16 =
                    (choice.approach as u16).saturating_add(SHORTCUT_APPROACH_SLACK);

                if approach_field
                    .compute(
                        blocked,
                        &start_roots,
                        &mut cambc::Bitboard::new().unwrap(),
                        maximum_walk,
                    )
                    .is_ok()
                {
                    if let (Ok(walk), Ok(Some(next_tile))) = (
                        approach_field.get(position),
                        approach_field.next_move_from(position),
                    ) {
                        let total_cost: u16 = walk
                            .saturating_add(choice.action_cost)
                            .saturating_add(choice.landing_distance);
                        if walk != UNREACHED_DISTANCE
                            && total_cost.saturating_add(SHORTCUT_MINIMUM_SAVING)
                                <= current_distance
                            && me.can_move().unwrap_or(false)
                            && me.move_absolute(next_tile).unwrap_or(false)
                        {
                            return MoveToLocationResult::InProgress;
                        }
                    }
                }
            }
        }
    }

    if !me.can_move().unwrap_or(false) {
        return MoveToLocationResult::InProgress;
    }

    let mut next_tiles: Vec<Vec2> = Vec::new();
    if let Ok(Some(next_tile)) = field.next_move_from(position) {
        next_tiles.push(next_tile);
    }

    if let Ok(other_tiles) = field.next_moves_from(position) {
        for tile in other_tiles {
            if !next_tiles.contains(&tile) {
                next_tiles.push(tile);
            }
        }
    }
    for next_tile in next_tiles {
        if me.move_absolute(position + next_tile).unwrap_or(false) {
            return MoveToLocationResult::InProgress;
        }
    }

    if me.move_towards(target).unwrap_or(false) {
        return MoveToLocationResult::InProgress;
    }

    MoveToLocationResult::Finished
}

// NEED GAP Pathfinding
