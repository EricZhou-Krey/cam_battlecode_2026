use crate::{
    action::{
        combat::{
            estimator::MemoryCombatEstimator,
            kit::{
                current_location_id, engage, health_fraction, is_ranged, is_stunned,
                report_enemy_health, report_self_health, report_self_status, request_help,
                should_wait_for_front_line, status_due, tile_distance, try_attack,
                try_attack_reset, try_cast_heal, try_cast_shield, try_cast_targeted, try_dash,
                try_move_towards, try_step_along_field, try_step_away, visible_allies,
                visible_enemies, DASH_MINIMUM_GAIN, HELP_SIGNAL_STRENGTH,
            },
        },
        movement::MoveToSpawnState,
        Action, COMBAT_HEAL_SIGNAL_STRENGTH, DANGER_HP_THRESHOLD, WARNING_HP_THRESHOLD,
    },
    memory::{champion_memory::ChampionMemory, message::messages::Message, LocationID},
    sdk_interact::map::step_with_fallback,
};
use cambc::{Bitboard, Champion, ChampionType, DistanceField, Vec2};

pub const COMBAT_PROXIMITY: u16 = 4;

const COMBAT_ENEMY_RADIUS: i32 = 7;
const COMBAT_ALLY_RADIUS: i32 = 6;
const THREAT_DISTANCE: i32 = 4;
const STALL_RETREAT_DISTANCE: i32 = 3;
const RECALL_SAFE_DISTANCE: i32 = 7;
const ALLY_HOLD_DISTANCE: i32 = 3;
const PUSH_HP_THRESHOLD: f32 = 0.4;
const RE_ENGAGE_HP_THRESHOLD: f32 = 0.6;
const OUTNUMBERED_ESCAPE_HP_THRESHOLD: f32 = 0.7;
const HEALTH_ADVANTAGE_FLOOR: f32 = 0.6;
const MODE_MINIMUM_TURNS: u16 = 2;
const DISENGAGE_TURNS: u16 = 3;
const COMBAT_STATUS_INTERVAL: u16 = 4;
const DEFEND_ENGAGE_RADIUS: i32 = 7;
const DEFEND_LEASH_RADIUS: i32 = 6;
const DEFEND_HOLD_RADIUS: i32 = 2;
const DEFEND_PATIENCE: u16 = 20;
const NO_ENEMY_DISTANCE: i32 = i32::MAX;

#[derive(Debug, Clone, PartialEq)]
pub struct ChampionCombatState {
    pub turns_in_combat: u16,
    pub turns_in_mode: u16,
    pub turns_since_enemy_seen: u16,
    pub focus_enemy: Option<ChampionType>,
    pub last_enemy_position: Option<Vec2>,
}

impl ChampionCombatState {
    pub fn new() -> Self {
        Self {
            turns_in_combat: 0,
            turns_in_mode: 0,
            turns_since_enemy_seen: 0,
            focus_enemy: None,
            last_enemy_position: None,
        }
    }
}

impl Default for ChampionCombatState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DefendState {
    pub location_id: LocationID,
    pub turns_without_enemy: u16,
}

impl DefendState {
    pub fn new(location_id: LocationID) -> Self {
        Self {
            location_id,
            turns_without_enemy: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CombatMode {
    Kill,
    Stall,
    Escape,
}

impl CombatMode {
    fn into_action(self, state: ChampionCombatState) -> Action {
        match self {
            Self::Kill => Action::InChampionComabtKill(state),
            Self::Stall => Action::InChampionCombatStall(state),
            Self::Escape => Action::InChampionCombatEscape(state),
        }
    }
}

struct CombatAssessment {
    enemies: Vec<Champion>,
    enemy_distances: Vec<i32>,
    allies: Vec<Champion>,
    enemy_types: Vec<ChampionType>,
    ally_types: Vec<ChampionType>,
    own_health_fraction: f32,
    own_position: Vec2,
    nearest_enemy_distance: i32,
    enemy_health_total: f32,
    ally_health_total: f32,
}

impl CombatAssessment {
    fn nearest_enemy(&self) -> Option<&Champion> {
        self.enemy_distances
            .iter()
            .enumerate()
            .min_by_key(|(_, distance)| **distance)
            .map(|(index, _)| &self.enemies[index])
    }
}

fn assess(me: &Champion) -> CombatAssessment {
    let own_position: Vec2 = me.position().unwrap();

    let mut enemies: Vec<Champion> = Vec::new();
    let mut enemy_distances: Vec<i32> = Vec::new();
    for enemy in visible_enemies() {
        let Ok(position): Result<Vec2, _> = enemy.position() else {
            continue;
        };
        let distance: i32 = tile_distance(own_position, position);
        if distance <= COMBAT_ENEMY_RADIUS {
            enemies.push(enemy);
            enemy_distances.push(distance);
        }
    }

    let allies: Vec<Champion> = visible_allies(me)
        .into_iter()
        .filter(|ally| {
            ally.position()
                .is_ok_and(|position| tile_distance(own_position, position) <= COMBAT_ALLY_RADIUS)
        })
        .collect();

    let enemy_types: Vec<ChampionType> = enemies
        .iter()
        .filter_map(|enemy| enemy.champion_name().ok())
        .collect();
    let mut ally_types: Vec<ChampionType> = allies
        .iter()
        .filter_map(|ally| ally.champion_name().ok())
        .collect();
    if let Ok(own_type) = me.champion_name() {
        ally_types.push(own_type);
    }

    let nearest_enemy_distance: i32 = enemy_distances
        .iter()
        .copied()
        .min()
        .unwrap_or(NO_ENEMY_DISTANCE);
    let enemy_health_total: f32 = enemies
        .iter()
        .map(|enemy| enemy.health().unwrap_or(0.0))
        .sum();
    let ally_health_total: f32 = allies
        .iter()
        .map(|ally| ally.health().unwrap_or(0.0))
        .sum::<f32>()
        + me.health().unwrap_or(0.0);

    CombatAssessment {
        enemies,
        enemy_distances,
        allies,
        enemy_types,
        ally_types,
        own_health_fraction: health_fraction(me),
        own_position,
        nearest_enemy_distance,
        enemy_health_total,
        ally_health_total,
    }
}

fn choose_mode(
    assessment: &CombatAssessment,
    estimator_good: bool,
    current_mode: CombatMode,
    turns_in_mode: u16,
) -> CombatMode {
    let health: f32 = assessment.own_health_fraction;
    let outnumbered: bool = assessment.enemy_types.len() > assessment.ally_types.len();

    let desired_mode: CombatMode = if health <= DANGER_HP_THRESHOLD
        || (!estimator_good
            && (health <= WARNING_HP_THRESHOLD
                || (outnumbered && health <= OUTNUMBERED_ESCAPE_HP_THRESHOLD)))
    {
        CombatMode::Escape
    } else if !estimator_good || health < PUSH_HP_THRESHOLD {
        CombatMode::Stall
    } else {
        CombatMode::Kill
    };

    if desired_mode == current_mode {
        return current_mode;
    }
    if desired_mode == CombatMode::Escape && health <= WARNING_HP_THRESHOLD {
        return desired_mode;
    }
    if current_mode == CombatMode::Escape && health < RE_ENGAGE_HP_THRESHOLD {
        return current_mode;
    }
    if turns_in_mode < MODE_MINIMUM_TURNS {
        return current_mode;
    }
    desired_mode
}

fn target_score(
    enemy: &Champion,
    own_position: Vec2,
    focus_enemy: Option<ChampionType>,
    attack_range: i32,
) -> f32 {
    let health: f32 = enemy.health().unwrap_or(f32::MAX / 4.0);
    let distance: i32 = enemy
        .position()
        .map_or(99, |position| tile_distance(own_position, position));
    let mut score: f32 = health + 60.0 * distance as f32;
    if distance <= attack_range {
        score -= 400.0;
    }
    if enemy.champion_name().ok() == focus_enemy {
        score -= 250.0;
    }
    score
}

fn pick_kill_target(
    assessment: &CombatAssessment,
    focus_enemy: Option<ChampionType>,
    attack_range: i32,
) -> Option<&Champion> {
    assessment
        .enemies
        .iter()
        .map(|enemy| {
            (
                target_score(enemy, assessment.own_position, focus_enemy, attack_range),
                enemy,
            )
        })
        .min_by(|first, second| first.0.total_cmp(&second.0))
        .map(|(_, enemy)| enemy)
}

fn retreat_score(field: &DistanceField, position: Vec2, threat_position: Vec2) -> i32 {
    field
        .get(position)
        .map_or(i32::MAX / 4, |distance| distance as i32)
        - 2 * tile_distance(position, threat_position)
}

fn retreat_step(
    me: &Champion,
    champion_memory: &mut ChampionMemory,
    threat_position: Vec2,
    allow_dash: bool,
) -> bool {
    let blocked: Bitboard = champion_memory.map.distance_fields.blocked_bitboard.clone();
    let Some(spawn_field): Option<&DistanceField> = champion_memory
        .map
        .distance_fields
        .try_get_spawn_distance_field()
    else {
        return try_step_away(me, threat_position);
    };
    let own_position: Vec2 = me.position().unwrap();

    let mut retreated: bool = false;
    if allow_dash {
        let current_score: i32 = retreat_score(spawn_field, own_position, threat_position);
        retreated |= try_dash(
            me,
            &blocked,
            current_score,
            DASH_MINIMUM_GAIN,
            |destination| retreat_score(spawn_field, destination, threat_position),
        );
    }
    if try_step_along_field(me, spawn_field) || try_step_away(me, threat_position) {
        retreated = true;
    }
    retreated
}

fn kill_turn(
    me: &Champion,
    champion_memory: &mut ChampionMemory,
    assessment: &CombatAssessment,
    state: &mut ChampionCombatState,
) {
    let _ = try_cast_heal(me, &assessment.allies);

    let attack_range: i32 = me.attack_range().unwrap_or(1);
    let Some(target): Option<&Champion> =
        pick_kill_target(assessment, state.focus_enemy, attack_range)
    else {
        if let Some(position) = state.last_enemy_position {
            let _ = try_move_towards(me, position);
        }
        return;
    };
    let Ok(target_position): Result<Vec2, _> = target.position() else {
        return;
    };
    let target_type: Option<ChampionType> = target.champion_name().ok();

    if state.focus_enemy != target_type {
        state.focus_enemy = target_type;
        if let Some(location_id) = current_location_id(champion_memory, target_position) {
            champion_memory
                .message_board
                .send(Message::CombatTargetEnemy {
                    location_id,
                    signal_strength: HELP_SIGNAL_STRENGTH,
                });
        }
    }
    if state.turns_in_combat % COMBAT_STATUS_INTERVAL == 1
        && assessment.ally_types.len() <= assessment.enemy_types.len()
    {
        request_help(champion_memory, assessment.own_position, false);
    }

    if is_ranged(me) && assessment.nearest_enemy_distance <= 1 {
        if let Some(nearest_enemy) = assessment.nearest_enemy() {
            if let Ok(nearest_position) = nearest_enemy.position() {
                let ally_engaged: bool = assessment.allies.iter().any(|ally| {
                    ally.position()
                        .is_ok_and(|position| tile_distance(position, nearest_position) <= 1)
                });
                if !ally_engaged && try_step_away(me, nearest_position) {
                    return;
                }
            }
        }
    }

    let hold_position: bool = should_wait_for_front_line(me, &assessment.allies, target_position);
    let under_threat: bool = assessment.nearest_enemy_distance <= THREAT_DISTANCE;
    let _ = engage(
        me,
        target,
        target_position,
        under_threat,
        hold_position,
        &champion_memory.map.distance_fields.blocked_bitboard,
    );
}

fn stall_turn(
    me: &Champion,
    champion_memory: &mut ChampionMemory,
    assessment: &CombatAssessment,
    state: &mut ChampionCombatState,
) {
    let _ = try_cast_heal(me, &assessment.allies);

    let Some(enemy): Option<&Champion> = assessment.nearest_enemy() else {
        return;
    };
    let Ok(enemy_position): Result<Vec2, _> = enemy.position() else {
        return;
    };
    let attack_range: i32 = me.attack_range().unwrap_or(1);
    let own_health: f32 = assessment.own_health_fraction;

    if state.turns_in_combat % COMBAT_STATUS_INTERVAL == 1 {
        request_help(champion_memory, assessment.own_position, true);
    }

    if assessment.nearest_enemy_distance <= THREAT_DISTANCE {
        let _ = try_cast_shield(me);
    }
    let _ = try_cast_targeted(me, enemy, enemy_position);

    let ranged_needs_space: bool = is_ranged(me) && assessment.nearest_enemy_distance <= 2;
    if !ranged_needs_space
        && assessment.nearest_enemy_distance <= attack_range
        && try_attack(me, enemy)
        && try_attack_reset(me)
    {
        let _ = try_attack(me, enemy);
    }

    let ally_support_near: bool = assessment.allies.iter().any(|ally| {
        ally.position().is_ok_and(|position| {
            tile_distance(position, assessment.own_position) <= ALLY_HOLD_DISTANCE
        })
    });
    let should_retreat: bool = assessment.nearest_enemy_distance <= STALL_RETREAT_DISTANCE
        && (!ally_support_near || own_health <= WARNING_HP_THRESHOLD);
    if should_retreat || ranged_needs_space {
        let _ = retreat_step(
            me,
            champion_memory,
            enemy_position,
            own_health <= WARNING_HP_THRESHOLD,
        );
    }
}

fn escape_turn(
    me: &Champion,
    champion_memory: &mut ChampionMemory,
    assessment: &CombatAssessment,
    state: &mut ChampionCombatState,
) -> Option<Action> {
    let safe_to_recall: bool = assessment.nearest_enemy_distance > RECALL_SAFE_DISTANCE;
    let Some(enemy): Option<&Champion> = assessment.nearest_enemy() else {
        report_self_status(champion_memory, me, true);
        return Some(Action::EscapeToSpawn(MoveToSpawnState {}));
    };
    if safe_to_recall {
        report_self_status(champion_memory, me, true);
        return Some(Action::EscapeToSpawn(MoveToSpawnState {}));
    }
    let Ok(enemy_position): Result<Vec2, _> = enemy.position() else {
        return Some(Action::EscapeToSpawn(MoveToSpawnState {}));
    };

    if state.turns_in_mode == 1 {
        report_self_status(champion_memory, me, true);
    }
    if state.turns_in_combat % COMBAT_STATUS_INTERVAL == 1 {
        if let Some(location_id) = current_location_id(champion_memory, assessment.own_position) {
            champion_memory.message_board.send(Message::CombatHeal {
                location_id,
                signal_strength: COMBAT_HEAL_SIGNAL_STRENGTH,
            });
        }
    }

    let fled: bool = retreat_step(me, champion_memory, enemy_position, true);
    if assessment.nearest_enemy_distance <= THREAT_DISTANCE {
        let _ = try_cast_shield(me);
    }
    if !fled {
        let _ = try_cast_targeted(me, enemy, enemy_position);
        if assessment.nearest_enemy_distance <= me.attack_range().unwrap_or(1)
            && try_attack(me, enemy)
            && try_attack_reset(me)
        {
            let _ = try_attack(me, enemy);
        }
    }
    None
}

fn step_towards_location(
    me: &Champion,
    champion_memory: &ChampionMemory,
    location_id: LocationID,
    position: Vec2,
) {
    match champion_memory.map.distance_fields.weak_get(location_id) {
        Some(field) => {
            let _ = step_with_fallback(field, position);
        }
        None => {
            let _ = try_move_towards(me, position);
        }
    }
}

impl Action {
    fn combat_turn(
        &mut self,
        champion_memory: &mut ChampionMemory,
        mode: CombatMode,
        advance: bool,
    ) {
        let mut state: ChampionCombatState = match self {
            Self::InChampionComabtKill(state)
            | Self::InChampionCombatStall(state)
            | Self::InChampionCombatEscape(state) => state.clone(),
            _ => {
                println!("Error: State Action Mismatch");
                return;
            }
        };
        let me: &Champion = cambc::self_champion().unwrap().unwrap();
        if is_stunned(me) {
            return;
        }
        let assessment: CombatAssessment = assess(me);

        if advance {
            state.turns_in_combat = state.turns_in_combat.saturating_add(1);
            state.turns_in_mode = state.turns_in_mode.saturating_add(1);
            match assessment.nearest_enemy() {
                Some(enemy) => {
                    state.turns_since_enemy_seen = 0;
                    if let Ok(position) = enemy.position() {
                        state.last_enemy_position = Some(position);
                    }
                }
                None => {
                    state.turns_since_enemy_seen = state.turns_since_enemy_seen.saturating_add(1);
                }
            }

            if state.turns_in_combat == 1 {
                report_self_status(champion_memory, me, false);
                if let Some(enemy) = assessment.nearest_enemy() {
                    report_enemy_health(champion_memory, enemy);
                }
            } else if state.turns_in_combat.is_multiple_of(COMBAT_STATUS_INTERVAL) {
                report_self_health(champion_memory, me);
                if let Some(enemy) = assessment.nearest_enemy() {
                    report_enemy_health(champion_memory, enemy);
                }
            }
        }

        if assessment.enemies.is_empty() && state.turns_since_enemy_seen >= DISENGAGE_TURNS {
            if assessment.own_health_fraction <= WARNING_HP_THRESHOLD {
                report_self_status(champion_memory, me, true);
                *self = Self::EscapeToSpawn(MoveToSpawnState {});
            } else {
                *self = Self::Idle;
            }
            return;
        }

        let next_mode: CombatMode = if assessment.enemies.is_empty() {
            mode
        } else {
            let estimator_good: bool = champion_memory
                .is_good_combat(&assessment.enemy_types, &assessment.ally_types)
                && assessment.ally_health_total
                    >= assessment.enemy_health_total * HEALTH_ADVANTAGE_FLOOR;
            choose_mode(&assessment, estimator_good, mode, state.turns_in_mode)
        };
        if next_mode != mode {
            state.turns_in_mode = 0;
            *self = next_mode.into_action(state);
            return;
        }

        let handoff: Option<Action> = match mode {
            CombatMode::Kill => {
                kill_turn(me, champion_memory, &assessment, &mut state);
                None
            }
            CombatMode::Stall => {
                stall_turn(me, champion_memory, &assessment, &mut state);
                None
            }
            CombatMode::Escape => escape_turn(me, champion_memory, &assessment, &mut state),
        };
        *self = handoff.unwrap_or_else(|| mode.into_action(state));
    }
}

pub trait CombatActions {
    fn champion_combat_stall(&mut self, champion_memory: &mut ChampionMemory);
    fn champion_combat_kill(&mut self, champion_memory: &mut ChampionMemory);
    fn champion_combat_escape(&mut self, champion_memory: &mut ChampionMemory);
    fn defend(&mut self, champion_memory: &mut ChampionMemory);
}

impl CombatActions for Action {
    fn champion_combat_stall(&mut self, champion_memory: &mut ChampionMemory) {
        self.combat_turn(champion_memory, CombatMode::Stall, true);
    }

    fn champion_combat_kill(&mut self, champion_memory: &mut ChampionMemory) {
        self.combat_turn(champion_memory, CombatMode::Kill, true);
    }

    fn champion_combat_escape(&mut self, champion_memory: &mut ChampionMemory) {
        self.combat_turn(champion_memory, CombatMode::Escape, true);
    }

    fn defend(&mut self, champion_memory: &mut ChampionMemory) {
        let Self::DefendLocation(state) = self else {
            println!("Error: State Action Mismatch");
            return;
        };
        let mut defend_state: DefendState = state.clone();
        let me: &Champion = cambc::self_champion().unwrap().unwrap();
        if is_stunned(me) {
            return;
        }

        let defend_position: Vec2 = champion_memory
            .map
            .points_of_interest
            .locate(defend_state.location_id);
        let own_position: Vec2 = me.position().unwrap();
        let allies: Vec<Champion> = visible_allies(me);
        let enemies: Vec<Champion> = visible_enemies()
            .into_iter()
            .filter(|enemy| {
                enemy.position().is_ok_and(|position| {
                    tile_distance(position, defend_position) <= DEFEND_ENGAGE_RADIUS
                })
            })
            .collect();

        let _ = try_cast_heal(me, &allies);
        if status_due(me) {
            report_self_health(champion_memory, me);
        }

        let nearest_enemy: Option<(&Champion, Vec2, i32)> = enemies
            .iter()
            .filter_map(|enemy| {
                let position: Vec2 = enemy.position().ok()?;
                Some((enemy, position, tile_distance(own_position, position)))
            })
            .min_by_key(|(_, _, distance)| *distance);

        match nearest_enemy {
            Some((enemy, enemy_position, enemy_distance)) => {
                defend_state.turns_without_enemy = 0;
                if tile_distance(own_position, defend_position) > DEFEND_LEASH_RADIUS {
                    step_towards_location(
                        me,
                        champion_memory,
                        defend_state.location_id,
                        defend_position,
                    );
                } else {
                    let under_threat: bool = enemy_distance <= THREAT_DISTANCE;
                    let hold_position: bool =
                        should_wait_for_front_line(me, &allies, enemy_position);
                    let _ = engage(
                        me,
                        enemy,
                        enemy_position,
                        under_threat,
                        hold_position,
                        &champion_memory.map.distance_fields.blocked_bitboard,
                    );
                }
            }
            None => {
                defend_state.turns_without_enemy =
                    defend_state.turns_without_enemy.saturating_add(1);
                if defend_state.turns_without_enemy >= DEFEND_PATIENCE {
                    *self = Self::Idle;
                    return;
                }
                if tile_distance(own_position, defend_position) > DEFEND_HOLD_RADIUS {
                    step_towards_location(
                        me,
                        champion_memory,
                        defend_state.location_id,
                        defend_position,
                    );
                } else if me.ward_cooldown().is_ok_and(|cooldown| cooldown <= 0) {
                    let _ = me.place_ward(defend_position);
                }
            }
        }

        *self = Self::DefendLocation(defend_state);
    }
}
