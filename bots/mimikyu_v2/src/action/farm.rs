use cambc::{
    Camp, CampGroupCaptureState, CampGroupCaptureType, CampGroupState, Champion, DistanceField,
    ItemPassive, Vec2,
};

use crate::{
    action::{
        combat::kit::{
            engage, health_fraction, is_stunned, report_self_health, report_self_status,
            should_wait_for_front_line, status_due, tile_distance, try_cast_heal, visible_allies,
            HELP_SIGNAL_STRENGTH,
        },
        movement::{MoveToSpawnState, MovementState, RECALL_TIME, RECALL_TIME_HOME},
        Action, ActionGroupType, WARNING_HP_THRESHOLD,
    },
    memory::{
        champion_memory::ChampionMemory, message::messages::Message, AltarID, CampGroupID, CampID,
        LocationID,
    },
    sdk_interact::{champion::passive_active, map::step_with_fallback},
};

pub const COLLECTION_TIME: u16 = 5;

const DISENGAGE_ROUNDS: f32 = 3.0;
const CAMP_THREAT_MARGIN: i32 = 1;
const CAMP_SUPPORT_RADIUS: i32 = 4;

#[derive(Debug, Clone, PartialEq)]
pub struct CampGroupCombatState {
    pub camp_group_id: CampGroupID,
    pub current_target_camp: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CollectionState {
    pub camp_group_id: CampGroupID,
}

impl CollectionState {
    pub fn new(camp_group_id: CampGroupID) -> Self {
        Self { camp_group_id }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TributeState {
    altar_id: AltarID,
}

pub trait FarmActions {
    fn camp_group_combat(&mut self, champion_memory: &mut ChampionMemory);
    fn collection(&mut self, champion_memory: &mut ChampionMemory);
    fn tributing_at_altar(&mut self, champion_memory: &mut ChampionMemory);
}

impl Action {
    fn begin_collection(
        &mut self,
        champion_memory: &mut ChampionMemory,
        camp_group_id: CampGroupID,
    ) {
        let location_id: LocationID = LocationID::CampGroup(camp_group_id);
        let distance_field: Option<DistanceField> = champion_memory
            .map
            .distance_fields
            .try_get(location_id, &champion_memory.map.points_of_interest)
            .cloned();
        let Some(distance_field) = distance_field else {
            *self = Self::Idle;
            return;
        };

        *self = Self::MoveToLocation(MovementState {
            next_action: Box::new(Some(Self::CollectingFromCamp(CollectionState::new(
                camp_group_id,
            )))),
            distance_field,
            location_id,
            accepted_distance: 0,
            action_group_type: ActionGroupType::EndFarm,
            route: None,
            route_failures: 0,
            allow_route: true,
        });
    }

    fn finish_camp_group(
        &mut self,
        champion_memory: &mut ChampionMemory,
        camp_group_id: CampGroupID,
        capture_state: Option<CampGroupCaptureState>,
    ) {
        let me: &Champion = cambc::self_champion().unwrap().unwrap();
        let me_type = me.champion_name().unwrap();

        let already_claimed: bool = champion_memory.message_board.inbox.iter().any(|received| {
            matches!(
                &received.message,
                Message::FarmCollection {
                    camp_group_id: claimed_id,
                    ally_champion_claiming: Some(_),
                } if *claimed_id == camp_group_id
            )
        });
        let ally_capturing: bool = capture_state
            .is_some_and(|capture| capture.state == CampGroupCaptureType::AllyCapturing);
        if already_claimed || ally_capturing {
            *self = Self::Idle;
            return;
        }

        champion_memory
            .map
            .start_camp_group_respawn_timer(camp_group_id);
        champion_memory
            .message_board
            .send(Message::FarmClear { camp_group_id });

        if health_fraction(me) > WARNING_HP_THRESHOLD {
            champion_memory.message_board.send(Message::FarmCollection {
                camp_group_id,
                ally_champion_claiming: Some(me_type),
            });
            self.begin_collection(champion_memory, camp_group_id);
        } else {
            champion_memory
                .message_board
                .send(Message::farm_collection(camp_group_id));
            report_self_status(champion_memory, me, true);
            *self = Self::EscapeToSpawn(MoveToSpawnState {});
        }
    }
}

impl FarmActions for Action {
    fn camp_group_combat(&mut self, champion_memory: &mut ChampionMemory) {
        let Self::InCampGroupCombat(state) = self else {
            println!("Error: State Action Mismatch");
            return;
        };
        let camp_group_id: CampGroupID = state.camp_group_id;
        let me: &Champion = cambc::self_champion().unwrap().unwrap();
        if is_stunned(me) {
            return;
        }

        let camp_groups = cambc::map::camp_groups().unwrap();
        let camp_group = &camp_groups[camp_group_id.0];
        let camps: &[Camp] = camp_group.camps();
        let capture_state: Option<CampGroupCaptureState> = camp_group.capture_state().ok();
        let all_camps_dead: bool = camps.iter().all(|camp| camp.is_dead().unwrap_or(false));

        match camp_group.state() {
            Ok(CampGroupState::Respawning(remaining)) => {
                champion_memory.map.camp_group_respawn_timer[camp_group_id.0] = remaining;
                *self = Self::Idle;
                return;
            }
            Ok(CampGroupState::RewardAvailable) => {
                self.finish_camp_group(champion_memory, camp_group_id, capture_state);
                return;
            }
            _ if all_camps_dead => {
                self.finish_camp_group(champion_memory, camp_group_id, capture_state);
                return;
            }
            _ => (),
        }

        let attack_order: Vec<CampID> =
            champion_memory.map.camp_group_attack_order[camp_group_id.0].clone();
        let target_slot: Option<usize> = attack_order.iter().position(|camp_id| {
            camps
                .get(camp_id.0)
                .is_some_and(|camp| !camp.is_dead().unwrap_or(false))
        });
        let Some(target_slot) = target_slot else {
            self.finish_camp_group(champion_memory, camp_group_id, capture_state);
            return;
        };
        let target_camp: &Camp = &camps[attack_order[target_slot].0];
        let target_position: Vec2 = target_camp.spawn_position();
        let own_position: Vec2 = me.position().unwrap();
        let attack_range: i32 = me.attack_range().unwrap_or(1);

        let allies: Vec<Champion> = visible_allies(me);
        let allies_at_camp: usize = allies
            .iter()
            .filter(|ally| {
                ally.position().is_ok_and(|position| {
                    tile_distance(position, target_position) <= CAMP_SUPPORT_RADIUS
                })
            })
            .count();

        let mut incoming_damage_per_round: f32 = 0.0;
        for camp in camps.iter() {
            if camp.is_dead().unwrap_or(true) || !camp.is_awake().unwrap_or(false) {
                continue;
            }
            let Ok(stats) = camp.base_stats() else {
                continue;
            };
            if tile_distance(own_position, camp.spawn_position())
                <= stats.base_attack_range + CAMP_THREAT_MARGIN
            {
                incoming_damage_per_round += stats.base_strength;
            }
        }

        let shared_damage_per_round: f32 =
            incoming_damage_per_round / (1.0 + allies_at_camp as f32);

        if shared_damage_per_round > 0.0
            && me.health().unwrap_or(0.0) <= shared_damage_per_round * DISENGAGE_ROUNDS
        {
            report_self_status(champion_memory, me, true);
            champion_memory
                .message_board
                .send(Message::farm_help(camp_group_id, HELP_SIGNAL_STRENGTH));
            *self = Self::EscapeToSpawn(MoveToSpawnState {});
            return;
        }

        if status_due(me) {
            report_self_health(champion_memory, me);
        }
        let _ = try_cast_heal(me, &allies);

        let hold_position: bool = should_wait_for_front_line(me, &allies, target_position);

        let outcome = engage(
            me,
            target_camp,
            target_position,
            incoming_damage_per_round > 0.0,
            hold_position,
            &champion_memory.map.distance_fields.blocked_bitboard,
        );

        let still_far: bool =
            tile_distance(me.position().unwrap_or(own_position), target_position) > attack_range;
        if !outcome.moved && !outcome.attacked && !outcome.cast && still_far && !hold_position {
            let camp_center: Vec2 = champion_memory
                .map
                .points_of_interest
                .locate(LocationID::CampGroup(camp_group_id));
            if let Some(field) = champion_memory
                .map
                .distance_fields
                .weak_get(LocationID::CampGroup(camp_group_id))
            {
                let _ = step_with_fallback(field, camp_center);
            }
        }

        if let Self::InCampGroupCombat(state) = self {
            state.current_target_camp = target_slot;
        }
    }

    fn collection(&mut self, champion_memory: &mut ChampionMemory) {
        let Self::CollectingFromCamp(state) = self else {
            println!("Error: State Action Mismatch");
            return;
        };
        let me: &Champion = cambc::self_champion().unwrap().unwrap();
        let camp_group = &cambc::map::camp_groups().unwrap()[state.camp_group_id.0];
        let capture_state: CampGroupCaptureState = camp_group.capture_state().unwrap();
        let group_finished: bool = matches!(camp_group.state(), Ok(CampGroupState::Respawning(_)));
        let capture_lost: bool = matches!(
            capture_state.state,
            CampGroupCaptureType::Contested | CampGroupCaptureType::EnemyCapturing
        );
        if group_finished || capture_lost {
            *self = Self::Idle;
            return;
        }

        let recall_time: u16 = if passive_active(ItemPassive::Home, me) {
            RECALL_TIME_HOME
        } else {
            RECALL_TIME
        };
        let current_hp_percentage: f32 = me.health().unwrap() / me.max_health().unwrap();
        if capture_state.state == CampGroupCaptureType::AllyCapturing
            && current_hp_percentage <= WARNING_HP_THRESHOLD
            && capture_state.timer as u16 == recall_time
        {
            report_self_status(champion_memory, me, true);
            let _ = me.recall();
        }
    }

    fn tributing_at_altar(&mut self, champion_memory: &mut ChampionMemory) {
        let Self::TributingAtAltar(state) = self else {
            println!("Error: State Action Mismatch");
            return;
        };
        let me: &Champion = cambc::self_champion().unwrap().unwrap();

        if me.is_submitting_gems().unwrap() {
            return;
        }

        if me.gem_count().unwrap() == 0 {
            *self = Self::Idle;
            return;
        }

        let _ = me.submit_gems(
            champion_memory
                .map
                .points_of_interest
                .locate(LocationID::Altar(state.altar_id)),
        );
    }
}
