use crate::memory::{champion_memory::ChampionMemory, message::messages::Message, LocationID};
use cambc::{
    Ability, AttackTarget, Bitboard, BuffKind, CastTarget, Champion, ChampionType, DistanceField,
    Team, Vec2,
};

pub const STATUS_REPORT_INTERVAL: u32 = 6;
pub const HELP_SIGNAL_STRENGTH: u16 = 12;
pub const HEAL_TRIGGER_FRACTION: f32 = 0.75;
pub const DASH_RANGE: i32 = 2;
pub const DASH_MINIMUM_GAIN: i32 = 2;
pub const DASH_DAMAGE_MINIMUM_GAIN: i32 = 1;

const DASH_CANDIDATE_ATTEMPTS: usize = 3;
const STEP_AWAY_ATTEMPTS: usize = 4;
const NEIGHBOUR_OFFSETS: [(i32, i32); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EngageOutcome {
    pub cast: bool,
    pub attacked: bool,
    pub moved: bool,
}

pub fn tile_distance(first: Vec2, second: Vec2) -> i32 {
    (first.x - second.x).abs().max((first.y - second.y).abs())
}

pub fn health_fraction(champion: &Champion) -> f32 {
    let health: f32 = champion.health().unwrap_or(0.0);
    let maximum_health: f32 = champion.max_health().unwrap_or(1.0).max(1.0);
    health / maximum_health
}

pub fn is_ranged(champion: &Champion) -> bool {
    champion.attack_range().unwrap_or(1) > 1
}

fn has_buff(champion: &Champion, buff_kind: BuffKind) -> bool {
    champion
        .buffs()
        .ok()
        .and_then(|buffs| buffs.get(buff_kind))
        .is_some()
}

pub fn is_stunned(champion: &Champion) -> bool {
    has_buff(champion, BuffKind::Stun)
}

pub fn has_shield(champion: &Champion) -> bool {
    has_buff(champion, BuffKind::Shield)
}

pub fn visible_allies(me: &Champion) -> Vec<Champion> {
    let me_type: Option<ChampionType> = me.champion_name().ok();
    cambc::vision::champions(Some(Team::Ally))
        .unwrap_or_default()
        .into_iter()
        .filter(|champion| {
            champion.champion_name().ok() != me_type && champion.is_alive().unwrap_or(false)
        })
        .collect()
}

pub fn visible_enemies() -> Vec<Champion> {
    cambc::vision::champions(Some(Team::Enemy))
        .unwrap_or_default()
        .into_iter()
        .filter(|champion| champion.is_alive().unwrap_or(false))
        .collect()
}

fn ability_ready(ability: &Ability) -> bool {
    ability.is_ready().unwrap_or(false)
}

fn ability_reaches(ability: &Ability, position: Vec2) -> bool {
    match ability.valid_tiles() {
        Ok(tiles) => tiles.is_set(position).unwrap_or(false),
        Err(_) => false,
    }
}

fn can_cast_now(me: &Champion) -> bool {
    me.can_cast().unwrap_or(false)
}

pub fn try_cast_shield(me: &Champion) -> bool {
    if !can_cast_now(me) || has_shield(me) {
        return false;
    }
    for ability in me.abilities().iter() {
        if ability.is_shield() && ability_ready(ability) {
            return ability.cast_untargeted().unwrap_or(false);
        }
    }
    false
}

pub fn try_cast_heal(me: &Champion, allies: &[Champion]) -> bool {
    if !can_cast_now(me) {
        return false;
    }
    let Some(heal_ability): Option<&Ability> = me
        .abilities()
        .iter()
        .find(|ability| ability.is_heal() && ability_ready(ability))
    else {
        return false;
    };

    let mut best_target: Option<&Champion> = None;
    let mut best_fraction: f32 = HEAL_TRIGGER_FRACTION;
    for ally in allies {
        let fraction: f32 = health_fraction(ally);
        if fraction >= best_fraction {
            continue;
        }
        let Ok(position): Result<Vec2, _> = ally.position() else {
            continue;
        };
        if !ability_reaches(heal_ability, position) {
            continue;
        }
        best_fraction = fraction;
        best_target = Some(ally);
    }

    match best_target {
        Some(target) => heal_ability.cast_on(target).unwrap_or(false),
        None => false,
    }
}

pub fn try_cast_targeted<T: CastTarget + ?Sized>(
    me: &Champion,
    target: &T,
    target_position: Vec2,
) -> bool {
    if !can_cast_now(me) {
        return false;
    }
    for ability in me.abilities().iter() {
        let offensive: bool = ability.is_cc() || ability.is_damage() || ability.is_aoe_damage();
        let excluded: bool = ability.is_dash()
            || ability.is_shield()
            || ability.is_heal()
            || ability.is_attack_reset();
        if !offensive || excluded || !ability_ready(ability) {
            continue;
        }
        if !ability_reaches(ability, target_position) {
            continue;
        }

        let targets_units: bool = ability.can_target_enemies().unwrap_or(false)
            || ability.can_target_camps().unwrap_or(false);
        let cast_result = if targets_units {
            ability.cast_on(target)
        } else if ability.can_target_ground().unwrap_or(false) {
            ability.cast_at(target_position)
        } else {
            continue;
        };
        if cast_result.unwrap_or(false) {
            return true;
        }
    }
    false
}

pub fn try_attack<T: AttackTarget + ?Sized>(me: &Champion, target: &T) -> bool {
    me.can_attack().unwrap_or(false) && me.attack(target).is_ok()
}

pub fn try_attack_reset(me: &Champion) -> bool {
    if !can_cast_now(me) {
        return false;
    }
    for ability in me.abilities().iter() {
        if ability.is_attack_reset() && ability_ready(ability) {
            return ability.cast_untargeted().unwrap_or(false);
        }
    }
    false
}

pub fn try_dash(
    me: &Champion,
    blocked: &Bitboard,
    current_score: i32,
    minimum_gain: i32,
    score: impl Fn(Vec2) -> i32,
) -> bool {
    if !can_cast_now(me) {
        return false;
    }
    let Some(dash_ability): Option<&Ability> = me
        .abilities()
        .iter()
        .find(|ability| ability.is_dash() && ability_ready(ability))
    else {
        return false;
    };
    let Ok(reach_tiles): Result<Bitboard, _> = dash_ability.valid_tiles() else {
        return false;
    };
    let Ok(origin): Result<Vec2, _> = me.position() else {
        return false;
    };

    let mut candidates: Vec<(i32, Vec2)> = Vec::new();
    for delta_y in -DASH_RANGE..=DASH_RANGE {
        for delta_x in -DASH_RANGE..=DASH_RANGE {
            if delta_x == 0 && delta_y == 0 {
                continue;
            }
            let destination: Vec2 = Vec2 {
                x: origin.x + delta_x,
                y: origin.y + delta_y,
            };
            let candidate_score: i32 = score(destination);
            if candidate_score > current_score - minimum_gain {
                continue;
            }
            if !reach_tiles.is_set(destination).unwrap_or(false) {
                continue;
            }
            if blocked.is_set(destination).unwrap_or(true) {
                continue;
            }
            if cambc::vision::has_champion(destination).unwrap_or(true)
                || cambc::vision::has_camp(destination).unwrap_or(true)
            {
                continue;
            }
            candidates.push((candidate_score, destination));
        }
    }

    candidates.sort_by_key(|(candidate_score, _)| *candidate_score);
    for (_, destination) in candidates.into_iter().take(DASH_CANDIDATE_ATTEMPTS) {
        if dash_ability.cast_at(destination).unwrap_or(false) {
            return true;
        }
    }
    false
}

pub fn try_move_towards(me: &Champion, position: Vec2) -> bool {
    me.can_move().unwrap_or(false) && me.move_towards(position).unwrap_or(false)
}

pub fn try_step_along_field(me: &Champion, field: &DistanceField) -> bool {
    if !me.can_move().unwrap_or(false) {
        return false;
    }
    let Ok(position): Result<Vec2, _> = me.position() else {
        return false;
    };
    let Ok(Some(delta)): Result<Option<Vec2>, _> = field.next_move_from(position) else {
        return false;
    };
    me.r#move(delta).unwrap_or(false)
}

pub fn try_step_away(me: &Champion, threat_position: Vec2) -> bool {
    if !me.can_move().unwrap_or(false) {
        return false;
    }
    let Ok(origin): Result<Vec2, _> = me.position() else {
        return false;
    };
    let current_distance: i32 = tile_distance(origin, threat_position);

    let mut candidates: Vec<(i32, Vec2)> = NEIGHBOUR_OFFSETS
        .iter()
        .map(|(delta_x, delta_y)| Vec2 {
            x: origin.x + delta_x,
            y: origin.y + delta_y,
        })
        .map(|neighbour| (-tile_distance(neighbour, threat_position), neighbour))
        .filter(|(negative_distance, _)| -negative_distance > current_distance)
        .collect();
    candidates.sort_by_key(|(negative_distance, _)| *negative_distance);

    for (_, neighbour) in candidates.into_iter().take(STEP_AWAY_ATTEMPTS) {
        if me.move_absolute(neighbour).unwrap_or(false) {
            return true;
        }
    }
    false
}

pub fn should_wait_for_front_line(
    me: &Champion,
    allies: &[Champion],
    target_position: Vec2,
) -> bool {
    if !is_ranged(me) || allies.is_empty() {
        return false;
    }
    let Ok(own_position): Result<Vec2, _> = me.position() else {
        return false;
    };
    let own_distance: i32 = tile_distance(own_position, target_position);
    if own_distance <= me.attack_range().unwrap_or(1) {
        return false;
    }
    !allies.iter().any(|ally| {
        ally.position()
            .is_ok_and(|position| tile_distance(position, target_position) < own_distance)
    })
}

pub fn engage<T: AttackTarget + CastTarget>(
    me: &Champion,
    target: &T,
    target_position: Vec2,
    under_threat: bool,
    hold_position: bool,
    blocked: &Bitboard,
) -> EngageOutcome {
    let mut outcome: EngageOutcome = EngageOutcome::default();
    if is_stunned(me) {
        return outcome;
    }
    let Ok(origin): Result<Vec2, _> = me.position() else {
        return outcome;
    };
    let attack_range: i32 = me.attack_range().unwrap_or(1);
    let mut distance: i32 = tile_distance(origin, target_position);

    if try_cast_targeted(me, target, target_position) {
        outcome.cast = true;
    }
    if under_threat && distance <= attack_range + 1 && try_cast_shield(me) {
        outcome.cast = true;
    }

    if distance > attack_range && !hold_position {
        let dash_deals_damage: bool = me
            .abilities()
            .iter()
            .any(|ability| ability.is_dash() && ability.damage_amount().unwrap_or(0.0) > 0.0);
        let dash_closes_gap: bool = distance >= attack_range + DASH_RANGE;
        let dash_lands_hit: bool = dash_deals_damage && distance <= attack_range + DASH_RANGE;
        if dash_closes_gap || dash_lands_hit {
            let minimum_gain: i32 = if dash_lands_hit {
                DASH_DAMAGE_MINIMUM_GAIN
            } else {
                DASH_MINIMUM_GAIN
            };
            if try_dash(me, blocked, distance, minimum_gain, |destination| {
                tile_distance(destination, target_position)
            }) {
                outcome.moved = true;
            }
        }

        let position_after_dash: Vec2 = me.position().unwrap_or(origin);
        if tile_distance(position_after_dash, target_position) > attack_range
            && try_move_towards(me, target_position)
        {
            outcome.moved = true;
        }
        distance = tile_distance(me.position().unwrap_or(origin), target_position);
    }

    if distance <= attack_range && try_attack(me, target) {
        outcome.attacked = true;
        if try_attack_reset(me) {
            try_attack(me, target);
        }
    }

    outcome
}

pub fn status_due(me: &Champion) -> bool {
    let round: u32 = cambc::round_number().unwrap_or(0);
    let offset: u32 = me
        .champion_name()
        .map(|champion_type| champion_type as u32)
        .unwrap_or(0)
        % STATUS_REPORT_INTERVAL;
    round % STATUS_REPORT_INTERVAL == offset
}

pub fn current_location_id(
    champion_memory: &mut ChampionMemory,
    position: Vec2,
) -> Option<LocationID> {
    champion_memory
        .map
        .points_of_interest
        .try_get_closest_poi(position, &mut champion_memory.map.distance_fields)
}

fn health_in_hundreds(health: f32) -> u8 {
    (health / 100.0).round().clamp(0.0, 255.0) as u8
}

pub fn report_self_health(champion_memory: &mut ChampionMemory, me: &Champion) {
    let Ok(me_type): Result<ChampionType, _> = me.champion_name() else {
        return;
    };
    champion_memory
        .message_board
        .send(Message::StatusAllyHealth {
            ally_champion_id: me_type,
            current_health_in_hundreds: health_in_hundreds(me.health().unwrap_or(0.0)),
        });
}

pub fn report_self_status(champion_memory: &mut ChampionMemory, me: &Champion, include_gems: bool) {
    let Ok(me_type): Result<ChampionType, _> = me.champion_name() else {
        return;
    };
    report_self_health(champion_memory, me);

    if let Ok(position) = me.position() {
        if let Some(location_id) = current_location_id(champion_memory, position) {
            champion_memory
                .message_board
                .send(Message::StatusAllyLocation {
                    ally_champion_id: me_type,
                    location_id,
                });
        }
    }
    if include_gems {
        champion_memory
            .message_board
            .send(Message::StatusAllyGemsHeld {
                ally_champion_id: me_type,
                gems_held_tens: me.gem_count().unwrap_or(0) / 10,
            });
    }
}

pub fn report_enemy_health(champion_memory: &mut ChampionMemory, enemy: &Champion) {
    let Ok(enemy_type): Result<ChampionType, _> = enemy.champion_name() else {
        return;
    };
    champion_memory
        .message_board
        .send(Message::StatusEnemyHealth {
            enemy_champion_id: enemy_type,
            current_health_in_hundreds: health_in_hundreds(enemy.health().unwrap_or(0.0)),
        });
}

pub fn request_help(champion_memory: &mut ChampionMemory, position: Vec2, losing: bool) {
    let Some(location_id): Option<LocationID> = current_location_id(champion_memory, position)
    else {
        return;
    };
    let message: Message = if losing {
        Message::combat_losing_fight_need_help(location_id, HELP_SIGNAL_STRENGTH)
    } else {
        Message::combat_winning_fight_want_help(location_id, HELP_SIGNAL_STRENGTH)
    };
    champion_memory.message_board.send(message);
}
