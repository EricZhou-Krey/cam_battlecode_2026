use crate::{
    action::{
        combat::{
            estimator::MemoryCombatEstimator,
            state::{ChampionCombatState, CombatActions, DefendState, COMBAT_PROXIMITY},
        },
        farm::{CampGroupCombatState, CollectionState, FarmActions, TributeState},
        movement::{MoveToSpawnState, MovementActions, MovementState, MOVE_PROXIMITY},
        shop::ActionBuyExtension,
    },
    memory::{
        champion_memory::ChampionMemory,
        message::{message_board::ReceivedMessage, messages::Message},
        CampGroupID, LocationID,
    },
    sdk_interact::map::chebyshev_distance,
};
use cambc::{Champion, ChampionType, DistanceField, Vec2};

pub mod combat;
pub mod farm;
pub mod movement;
pub mod route;
pub mod shop;

pub const COMBAT_HEAL_SIGNAL_STRENGTH: u16 = 15;
pub const DANGER_HP_THRESHOLD: f32 = 0.2;
pub const WARNING_HP_THRESHOLD: f32 = 0.5;

#[repr(usize)]
#[derive(Debug, Default, Clone, PartialEq, Copy, Hash)]
pub enum ActionGroupType {
    #[default]
    Intermediate, // [Idle]
    StartFarm,      // [MoveToLocation( -> InCampGroupCombat)]
    Support,        // [GoingToHeal, DefendLocation, MoveToLocation( -> ChampionCombat)]
    EndFarm, // [InCampGroupCombat, MoveToLocation(-> ), CollectingFromCamp, TributingAtAltar]
    ChampionCombat, // [InChampionCombatStall, InChampionComabtKill]
    Survival, // [EscapeToSpawn, InChampionCombatEscape]
}

#[derive(Debug, Default, Clone, PartialEq)]
pub enum Action {
    #[default]
    Idle,
    MoveToLocation(MovementState),
    CollectingFromCamp(CollectionState),
    InChampionComabtKill(ChampionCombatState),
    InChampionCombatStall(ChampionCombatState),
    InChampionCombatEscape(ChampionCombatState),
    InCampGroupCombat(CampGroupCombatState),
    TributingAtAltar(TributeState),
    DefendLocation(DefendState),
    EscapeToSpawn(MoveToSpawnState),
}

impl Action {
    const IDLE: i32 = ActionGroupType::Intermediate as i32;

    const COLLECTING_FROM_CAMP: i32 = ActionGroupType::EndFarm as i32;
    const IN_CAMP_GROUP_COMBAT: i32 = ActionGroupType::EndFarm as i32;
    const TRIBUTING_AT_ALTAR: i32 = ActionGroupType::EndFarm as i32;

    const DEFEND_LOCATION: i32 = ActionGroupType::Support as i32;

    const IN_CHAMPION_COMBAT_STALL: i32 = ActionGroupType::ChampionCombat as i32;
    const IN_CHAMPION_COMABT_KILL: i32 = ActionGroupType::ChampionCombat as i32;

    const ESCAPE_TO_SPAWN: i32 = ActionGroupType::Survival as i32;
    const IN_CHAMPION_COMBAT_ESCAPE: i32 = ActionGroupType::Survival as i32;

    pub fn precedence(&self) -> i32 {
        match self {
            Action::Idle => Self::IDLE,
            Action::DefendLocation(_state) => Self::DEFEND_LOCATION,

            Action::CollectingFromCamp(_state) => Self::COLLECTING_FROM_CAMP,
            Action::InCampGroupCombat(_state) => Self::IN_CAMP_GROUP_COMBAT,
            Action::TributingAtAltar(_state) => Self::TRIBUTING_AT_ALTAR,

            Action::InChampionCombatStall(_state) => Self::IN_CHAMPION_COMBAT_STALL,
            Action::InChampionComabtKill(_state) => Self::IN_CHAMPION_COMABT_KILL,

            Action::EscapeToSpawn(_state) => Self::ESCAPE_TO_SPAWN,
            Action::InChampionCombatEscape(_state) => Self::IN_CHAMPION_COMBAT_ESCAPE,

            Action::MoveToLocation(state) => state.action_group_type as i32,
        }
    }

    fn try_prempt_current_action(&mut self, champion_memory: &mut ChampionMemory) {
        let current_precedence: i32 = self.precedence();
        let mut reacted = false;

        if current_precedence < ActionGroupType::Survival as i32 {
            if let Some(reaction) = Self::survival_reaction(champion_memory) {
                reacted = true;
                *self = reaction;
            }
        }

        if !reacted && current_precedence < ActionGroupType::ChampionCombat as i32 {
            if let Some(reaction) = Self::combat_reaction(champion_memory) {
                reacted = true;
                *self = reaction;
            }
        }

        if !reacted && current_precedence < ActionGroupType::EndFarm as i32 {
            if let Some(reaction) = Self::reply_reaction(champion_memory) {
                *self = reaction;
            }
        }

        if !reacted && current_precedence < ActionGroupType::EndFarm as i32 {
            let message_reaction: Option<(ReceivedMessage, Self)> = {
                Self::message_reaction(champion_memory)
                    .map(|(received_message, reaction)| ((*received_message).clone(), reaction))
            };

            if let Some((received_message, reaction)) = message_reaction {
                champion_memory.message_board.respond_to(
                    &received_message,
                    cambc::self_champion()
                        .unwrap()
                        .unwrap()
                        .champion_name()
                        .unwrap(),
                );
                reacted = true;
                *self = reaction;
            }
        }

        if !reacted && current_precedence < ActionGroupType::StartFarm as i32 {
            if let Some(reaction) = Self::farm_reaction(champion_memory) {
                *self = reaction;
            }
        }
    }

    pub fn turn(&mut self, champion_memory: &mut ChampionMemory) {
        self.buy_action(champion_memory);
        self.try_prempt_current_action(champion_memory);

        match self {
            Action::Idle => (),
            Action::MoveToLocation(_) => self.move_to_location(champion_memory),
            Action::CollectingFromCamp(_) => self.collection(champion_memory),
            Action::InChampionComabtKill(_) => self.champion_combat_kill(champion_memory),
            Action::InChampionCombatStall(_) => self.champion_combat_stall(champion_memory),
            Action::InChampionCombatEscape(_) => self.champion_combat_escape(champion_memory),
            Action::InCampGroupCombat(_) => self.camp_group_combat(champion_memory),
            Action::TributingAtAltar(_) => self.tributing_at_altar(champion_memory),
            Action::DefendLocation(_) => self.defend(champion_memory),
            Action::EscapeToSpawn(_) => self.escape_to_spawn(champion_memory),
        }
    }

    fn farm_reaction(champion_memory: &mut ChampionMemory) -> Option<Self> {
        let me: &Champion = cambc::self_champion().unwrap().unwrap();
        let me_position: Vec2 = me.position().unwrap();
        let alive_camp_group_ids: Vec<CampGroupID> = (0..champion_memory
            .map
            .points_of_interest
            .camp_group_centers
            .len())
            .filter_map(|index| {
                let camp_group_id: CampGroupID = CampGroupID(index);
                let distance_field: Option<&DistanceField> =
                    champion_memory.map.distance_fields.try_get(
                        LocationID::CampGroup(camp_group_id),
                        &champion_memory.map.points_of_interest,
                    );

                let time_to_respawn: i32 =
                    *champion_memory.map.camp_group_respawn_timer.get(index)?;

                if distance_field.map_or(-1_i32, |field| field.get(me_position).unwrap() as i32)
                    > time_to_respawn
                {
                    Some(CampGroupID(index))
                } else {
                    None
                }
            })
            .collect();

        let target_camp_group: Option<CampGroupID> = alive_camp_group_ids
            .iter()
            .min_by_key(|camp_group_id| {
                chebyshev_distance(
                    me_position,
                    champion_memory.map.points_of_interest.camp_group_centers[camp_group_id.0],
                )
            })
            .copied();

        let camp_group = target_camp_group?;
        let location_id: LocationID = LocationID::CampGroup(camp_group);
        let distance_field = champion_memory
            .map
            .distance_fields
            .try_get(location_id, &champion_memory.map.points_of_interest)
            .cloned()?;

        Some(Self::MoveToLocation(MovementState {
            next_action: Box::new(Some(Self::InCampGroupCombat(CampGroupCombatState {
                camp_group_id: camp_group,
                current_target_camp: 0,
            }))),
            distance_field,
            location_id,
            accepted_distance: MOVE_PROXIMITY,
            action_group_type: ActionGroupType::StartFarm,
            route: None,
            route_failures: 0,
            allow_route: true,
        }))
    }

    fn survival_reaction(champion_memory: &mut ChampionMemory) -> Option<Self> {
        let me: &Champion = cambc::self_champion().unwrap().unwrap();
        let current_hp_percentage: f32 = me.health().unwrap() / me.max_health().unwrap();

        if current_hp_percentage <= DANGER_HP_THRESHOLD {
            Some(Self::EscapeToSpawn(MoveToSpawnState {}))
        } else if current_hp_percentage <= WARNING_HP_THRESHOLD {
            let location_id = champion_memory.map.points_of_interest.try_get_closest_poi(
                me.position().unwrap(),
                &mut champion_memory.map.distance_fields,
            )?;

            champion_memory.message_board.send(Message::CombatHeal {
                location_id,
                signal_strength: COMBAT_HEAL_SIGNAL_STRENGTH,
            });

            None
        } else {
            None
        }
    }

    fn reply_reaction(champion_memory: &mut ChampionMemory) -> Option<Self> {
        let mut reaction: Option<Self> = None;
        let replies: Vec<Message> = std::mem::take(&mut champion_memory.message_board.responses);

        for reply in replies {
            match reply {
                Message::FarmHelp {
                    camp_group_id,
                    ally_champions_helping,
                    ..
                } => {
                    let me_type: ChampionType = cambc::self_champion()
                        .unwrap()
                        .unwrap()
                        .champion_name()
                        .unwrap();
                    let mut ally_champions: Vec<ChampionType> =
                        ally_champions_helping.into_iter().flatten().collect();
                    ally_champions.push(me_type);

                    if !champion_memory.can_kill_camp(camp_group_id, &ally_champions) {
                        for ally_champion_id in ally_champions {
                            if ally_champion_id == me_type {
                                continue;
                            }
                            champion_memory
                                .message_board
                                .send(Message::FarmCancel { ally_champion_id });
                        }
                        reaction = Some(Self::Idle);
                    }
                }
                Message::FarmCollection {
                    camp_group_id,
                    ally_champion_claiming,
                } => {
                    if ally_champion_claiming.is_some() {
                        continue;
                    }
                    let location_id: LocationID = LocationID::CampGroup(camp_group_id);
                    let Some(distance_field): Option<DistanceField> = champion_memory
                        .map
                        .distance_fields
                        .try_get(location_id, &champion_memory.map.points_of_interest)
                        .cloned()
                    else {
                        continue;
                    };

                    reaction = Some(Self::MoveToLocation(MovementState {
                        next_action: Box::new(Some(Self::CollectingFromCamp(CollectionState {}))),
                        distance_field,
                        location_id,
                        accepted_distance: 0,
                        action_group_type: ActionGroupType::EndFarm,
                        route: None,
                        route_failures: 0,
                        allow_route: true,
                    }));
                }
                _ => (),
            }
        }

        reaction
    }

    fn combat_reaction(champion_memory: &mut ChampionMemory) -> Option<Self> {
        let me_position: Vec2 = cambc::self_champion().unwrap().unwrap().position().unwrap();
        let mut reaction: Option<Self> = None;

        for enemy_champion_id in &champion_memory.champion_info.enemy_updated_this_turn {
            let Some(enemy_position): Option<Vec2> =
                champion_memory.champion_info.enemy[enemy_champion_id].last_seen
            else {
                continue;
            };

            if chebyshev_distance(me_position, enemy_position) as u16 <= COMBAT_PROXIMITY {
                reaction = Some(Self::InChampionComabtKill(ChampionCombatState {}));
            }
        }

        reaction
    }

    fn message_reaction(champion_memory: &mut ChampionMemory) -> Option<(&ReceivedMessage, Self)> {
        let me_position: Vec2 = cambc::self_champion().unwrap().unwrap().position().unwrap();
        let mut max_precedence: i32 = i32::MIN;
        let mut reaction: Option<(&ReceivedMessage, Self)> = None;

        for received_message in champion_memory.message_board.inbox.iter() {
            match &received_message.message {
                Message::FarmGoingToCamp { .. }
                | Message::FarmClear { .. }
                | Message::StatusAllyHealth { .. }
                | Message::StatusEnemyHealth { .. }
                | Message::StatusAllyLocation { .. }
                | Message::StatusEnemyLocation { .. }
                | Message::StatusAllyGemsHeld { .. }
                | Message::StatusEnemyGemsHeld { .. } => (),

                Message::FarmHelp {
                    camp_group_id,
                    signal_strength,
                    ally_champions_helping: _,
                } => {
                    if max_precedence > ActionGroupType::StartFarm as i32 {
                        continue;
                    }
                    let location_id: LocationID = LocationID::CampGroup(*camp_group_id);
                    let Some(distance_field): Option<DistanceField> = champion_memory
                        .map
                        .distance_fields
                        .try_get(location_id, &champion_memory.map.points_of_interest)
                        .cloned()
                    else {
                        continue;
                    };
                    let distance: u16 = distance_field.get(me_position).unwrap();
                    if *signal_strength < distance {
                        continue;
                    }

                    max_precedence = ActionGroupType::StartFarm as i32;
                    reaction = Some((
                        received_message,
                        Self::MoveToLocation(MovementState {
                            next_action: Box::new(Some(Self::InCampGroupCombat(
                                CampGroupCombatState {
                                    camp_group_id: *camp_group_id,
                                    current_target_camp: 0,
                                },
                            ))),
                            distance_field,
                            location_id,
                            accepted_distance: MOVE_PROXIMITY,
                            action_group_type: ActionGroupType::StartFarm,
                            route: None,
                            route_failures: 0,
                            allow_route: true,
                        }),
                    ));
                }
                Message::FarmCancel { ally_champion_id } => {
                    if max_precedence > ActionGroupType::StartFarm as i32 {
                        continue;
                    }
                    let me_type: ChampionType = cambc::self_champion()
                        .unwrap()
                        .unwrap()
                        .champion_name()
                        .unwrap();

                    if me_type != *ally_champion_id {
                        continue;
                    }

                    max_precedence = ActionGroupType::StartFarm as i32;
                    reaction = Some((received_message, Self::Idle));
                }
                Message::FarmCollection {
                    camp_group_id,
                    ally_champion_claiming,
                } => {
                    if max_precedence > ActionGroupType::EndFarm as i32
                        || ally_champion_claiming.is_some()
                    {
                        continue;
                    }
                    let location_id: LocationID = LocationID::CampGroup(*camp_group_id);
                    let Some(distance_field): Option<DistanceField> = champion_memory
                        .map
                        .distance_fields
                        .try_get(location_id, &champion_memory.map.points_of_interest)
                        .cloned()
                    else {
                        continue;
                    };

                    max_precedence = ActionGroupType::EndFarm as i32;
                    reaction = Some((
                        received_message,
                        Self::MoveToLocation(MovementState {
                            next_action: Box::new(Some(Self::CollectingFromCamp(
                                CollectionState {},
                            ))),
                            distance_field,
                            location_id,
                            accepted_distance: 0,
                            action_group_type: ActionGroupType::EndFarm,
                            route: None,
                            route_failures: 0,
                            allow_route: true,
                        }),
                    ));
                }
                Message::CombatHeal {
                    location_id,
                    signal_strength,
                } => {
                    if max_precedence > ActionGroupType::Support as i32 {
                        continue;
                    }
                    let Some(distance_field): Option<DistanceField> = champion_memory
                        .map
                        .distance_fields
                        .try_get(*location_id, &champion_memory.map.points_of_interest)
                        .cloned()
                    else {
                        continue;
                    };
                    let distance: u16 = distance_field.get(me_position).unwrap();
                    if *signal_strength < distance {
                        continue;
                    }

                    max_precedence = ActionGroupType::Support as i32;
                    reaction = Some((
                        received_message,
                        Self::MoveToLocation(MovementState {
                            next_action: Box::new(Some(Self::CollectingFromCamp(
                                CollectionState {},
                            ))),
                            distance_field,
                            location_id: *location_id,
                            accepted_distance: MOVE_PROXIMITY,
                            action_group_type: ActionGroupType::Support,
                            route: None,
                            route_failures: 0,
                            allow_route: true,
                        }),
                    ));
                }
                Message::CombatLosingFightNeedHelp {
                    location_id,
                    signal_strength,
                    ally_champion: _,
                } => {
                    if max_precedence > ActionGroupType::Support as i32 {
                        continue;
                    }
                    let Some(distance_field): Option<DistanceField> = champion_memory
                        .map
                        .distance_fields
                        .try_get(*location_id, &champion_memory.map.points_of_interest)
                        .cloned()
                    else {
                        continue;
                    };
                    let distance: u16 = distance_field.get(me_position).unwrap();
                    if *signal_strength < distance {
                        continue;
                    }

                    max_precedence = ActionGroupType::Support as i32;
                    reaction = Some((
                        received_message,
                        Self::MoveToLocation(MovementState {
                            next_action: Box::new(None),
                            distance_field,
                            location_id: *location_id,
                            accepted_distance: MOVE_PROXIMITY,
                            action_group_type: ActionGroupType::Support,
                            route: None,
                            route_failures: 0,
                            allow_route: true,
                        }),
                    ));
                }
                Message::CombatWinningFightWantHelp {
                    location_id,
                    signal_strength,
                    ally_champion: _,
                } => {
                    if max_precedence > ActionGroupType::Support as i32 {
                        continue;
                    }
                    let Some(distance_field): Option<DistanceField> = champion_memory
                        .map
                        .distance_fields
                        .try_get(*location_id, &champion_memory.map.points_of_interest)
                        .cloned()
                    else {
                        continue;
                    };
                    let distance: u16 = distance_field.get(me_position).unwrap();
                    if *signal_strength < distance {
                        continue;
                    }

                    max_precedence = ActionGroupType::Support as i32;
                    reaction = Some((
                        received_message,
                        Self::MoveToLocation(MovementState {
                            next_action: Box::new(None),
                            distance_field,
                            location_id: *location_id,
                            accepted_distance: MOVE_PROXIMITY,
                            action_group_type: ActionGroupType::Support,
                            route: None,
                            route_failures: 0,
                            allow_route: true,
                        }),
                    ));
                }
                Message::CombatTargetEnemy {
                    location_id,
                    signal_strength,
                } => {
                    if max_precedence > ActionGroupType::Support as i32 {
                        continue;
                    }
                    let Some(distance_field): Option<DistanceField> = champion_memory
                        .map
                        .distance_fields
                        .try_get(*location_id, &champion_memory.map.points_of_interest)
                        .cloned()
                    else {
                        continue;
                    };
                    let distance: u16 = distance_field.get(me_position).unwrap();
                    if *signal_strength < distance {
                        continue;
                    }

                    max_precedence = ActionGroupType::Support as i32;
                    reaction = Some((
                        received_message,
                        Self::MoveToLocation(MovementState {
                            next_action: Box::new(None),
                            distance_field,
                            location_id: *location_id,
                            accepted_distance: MOVE_PROXIMITY,
                            action_group_type: ActionGroupType::Support,
                            route: None,
                            route_failures: 0,
                            allow_route: true,
                        }),
                    ));
                }
                Message::GroupTogether { location_id } => {
                    if max_precedence > ActionGroupType::StartFarm as i32 {
                        continue;
                    }
                    let Some(distance_field): Option<DistanceField> = champion_memory
                        .map
                        .distance_fields
                        .try_get(*location_id, &champion_memory.map.points_of_interest)
                        .cloned()
                    else {
                        continue;
                    };

                    max_precedence = ActionGroupType::Support as i32;
                    reaction = Some((
                        received_message,
                        Self::MoveToLocation(MovementState {
                            next_action: Box::new(None),
                            distance_field,
                            location_id: *location_id,
                            accepted_distance: MOVE_PROXIMITY,
                            action_group_type: ActionGroupType::Support,
                            route: None,
                            route_failures: 0,
                            allow_route: true,
                        }),
                    ));
                }
            }
        }

        reaction
    }
}
