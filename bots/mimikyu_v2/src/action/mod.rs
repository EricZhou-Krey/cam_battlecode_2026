use crate::{
    action::{
        combat::state::{ChampionCombatState, CombatActions, DefendState},
        farm::{CampGroupCombatState, CollectionState, FarmActions, TributeState},
        movement::{MoveToSpawnState, MovementActions, MovementState, MOVE_PROXIMITY},
        shop::ActionBuyExtension,
    },
    memory::{champion_memory::ChampionMemory, message::messages::Message, LocationID},
    sdk_interact::map::chebyshev_distance,
};

pub mod combat;
pub mod farm;
pub mod movement;
pub mod shop;

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
        let message_reaction: Option<Self> = Self::message_reaction(champion_memory);

        todo!()
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

    fn message_reaction(champion_memory: &mut ChampionMemory) -> Option<Self> {
        champion_memory.message_board.inbox.iter().fold(
            None,
            |highest_precedence_message_action, received_message| match &received_message.message {
                Message::FarmGoingToCamp { .. }
                | Message::FarmClear { .. }
                | Message::StatusAllyHealth { .. }
                | Message::StatusEnemyHealth { .. }
                | Message::StatusAllyLocation { .. }
                | Message::StatusEnemyLocation { .. }
                | Message::StatusAllyGemsHeld { .. }
                | Message::StatusEnemyGemsHeld { .. } => highest_precedence_message_action,

                Message::FarmHelp {
                    camp_group_id,
                    signal_strength,
                    ally_champions_helping: _,
                } => {
                    if highest_precedence_message_action
                        .as_ref()
                        .map_or(0, |action| action.precedence())
                        < ActionGroupType::StartFarm as i32
                        && chebyshev_distance(
                            champion_memory.map.points_of_interest.camp_group_centers
                                [camp_group_id.0],
                            cambc::self_champion().unwrap().unwrap().position().unwrap(),
                        ) <= *signal_strength as i32
                    {
                        Some(Self::MoveToLocation(MovementState {
                            next_action: Box::new(Some(Self::InCampGroupCombat(
                                CampGroupCombatState {
                                    camp_group_id: *camp_group_id,
                                    current_target_camp: 0,
                                },
                            ))),
                            distance_field: champion_memory
                                .map
                                .distance_fields
                                .try_get(*camp_group_id)
                                .cloned(),
                            location_id: Some(LocationID::CampGroup(*camp_group_id)),
                            accepted_distance: MOVE_PROXIMITY,
                            action_group_type: ActionGroupType::StartFarm,
                        }))
                    } else {
                        highest_precedence_message_action
                    }
                }
                Message::FarmCollection {
                    camp_group_id,
                    ally_champion_claiming: _,
                } => {
                    if highest_precedence_message_action
                        .as_ref()
                        .map_or(0, |action| action.precedence())
                        < ActionGroupType::EndFarm as i32
                    {
                        Some(Self::MoveToLocation(MovementState {
                            next_action: Box::new(Some(Self::CollectingFromCamp(
                                CollectionState {},
                            ))),
                            distance_field: champion_memory
                                .map
                                .distance_fields
                                .try_get(*camp_group_id)
                                .cloned(),
                            location_id: Some(LocationID::CampGroup(*camp_group_id)),
                            accepted_distance: 0,
                            action_group_type: ActionGroupType::EndFarm,
                        }))
                    } else {
                        highest_precedence_message_action
                    }
                }

                Message::CombatHeal {
                    location_id,
                    signal_strength,
                } => {
                    if highest_precedence_message_action
                        .as_ref()
                        .map_or(0, |action| action.precedence())
                        < ActionGroupType::Support as i32
                        && chebyshev_distance(
                            champion_memory.map.points_of_interest.locate(*location_id),
                            cambc::self_champion().unwrap().unwrap().position().unwrap(),
                        ) <= *signal_strength as i32
                    {
                        Some(Self::MoveToLocation(MovementState {
                            next_action: Box::new(Some(Self::CollectingFromCamp(
                                CollectionState {},
                            ))),
                            distance_field: if let LocationID::CampGroup(camp_group_id) =
                                location_id
                            {
                                champion_memory
                                    .map
                                    .distance_fields
                                    .try_get(*camp_group_id)
                                    .cloned()
                            } else {
                                None
                            },
                            location_id: Some(*location_id),
                            accepted_distance: MOVE_PROXIMITY,
                            action_group_type: ActionGroupType::Support,
                        }))
                    } else {
                        highest_precedence_message_action
                    }
                }
                Message::CombatLosingFightNeedHelp {
                    location_id,
                    signal_strength,
                    ally_champion: _,
                } => {
                    if highest_precedence_message_action
                        .as_ref()
                        .map_or(0, |action| action.precedence())
                        < ActionGroupType::Support as i32
                        && chebyshev_distance(
                            champion_memory.map.points_of_interest.locate(*location_id),
                            cambc::self_champion().unwrap().unwrap().position().unwrap(),
                        ) <= *signal_strength as i32
                    {
                        Some(Self::MoveToLocation(MovementState {
                            next_action: Box::new(None),
                            distance_field: if let LocationID::CampGroup(camp_group_id) =
                                location_id
                            {
                                champion_memory
                                    .map
                                    .distance_fields
                                    .try_get(*camp_group_id)
                                    .cloned()
                            } else {
                                None
                            },
                            location_id: Some(*location_id),
                            accepted_distance: MOVE_PROXIMITY,
                            action_group_type: ActionGroupType::Support,
                        }))
                    } else {
                        highest_precedence_message_action
                    }
                }
                Message::CombatWinningFightWantHelp {
                    location_id,
                    signal_strength,
                    ally_champion: _,
                } => {
                    if highest_precedence_message_action
                        .as_ref()
                        .map_or(0, |action| action.precedence())
                        < ActionGroupType::Support as i32
                        && chebyshev_distance(
                            champion_memory.map.points_of_interest.locate(*location_id),
                            cambc::self_champion().unwrap().unwrap().position().unwrap(),
                        ) <= *signal_strength as i32
                    {
                        Some(Self::MoveToLocation(MovementState {
                            next_action: Box::new(None),
                            distance_field: if let LocationID::CampGroup(camp_group_id) =
                                location_id
                            {
                                champion_memory
                                    .map
                                    .distance_fields
                                    .try_get(*camp_group_id)
                                    .cloned()
                            } else {
                                None
                            },
                            location_id: Some(*location_id),
                            accepted_distance: MOVE_PROXIMITY,
                            action_group_type: ActionGroupType::Support,
                        }))
                    } else {
                        highest_precedence_message_action
                    }
                }
                Message::CombatTargetEnemy {
                    location_id,
                    signal_strength,
                } => {
                    if highest_precedence_message_action
                        .as_ref()
                        .map_or(0, |action| action.precedence())
                        < ActionGroupType::Support as i32
                        && chebyshev_distance(
                            champion_memory.map.points_of_interest.locate(*location_id),
                            cambc::self_champion().unwrap().unwrap().position().unwrap(),
                        ) <= *signal_strength as i32
                    {
                        Some(Self::MoveToLocation(MovementState {
                            next_action: Box::new(None),
                            distance_field: if let LocationID::CampGroup(camp_group_id) =
                                location_id
                            {
                                champion_memory
                                    .map
                                    .distance_fields
                                    .try_get(*camp_group_id)
                                    .cloned()
                            } else {
                                None
                            },
                            location_id: Some(*location_id),
                            accepted_distance: MOVE_PROXIMITY,
                            action_group_type: ActionGroupType::Support,
                        }))
                    } else {
                        highest_precedence_message_action
                    }
                }
                Message::GroupTogether { location_id } => {
                    if highest_precedence_message_action
                        .as_ref()
                        .map_or(0, |action| action.precedence())
                        < ActionGroupType::Support as i32
                    {
                        Some(Self::MoveToLocation(MovementState {
                            next_action: Box::new(None),
                            distance_field: if let LocationID::CampGroup(camp_group_id) =
                                location_id
                            {
                                champion_memory
                                    .map
                                    .distance_fields
                                    .try_get(*camp_group_id)
                                    .cloned()
                            } else {
                                None
                            },
                            location_id: Some(*location_id),
                            accepted_distance: MOVE_PROXIMITY,
                            action_group_type: ActionGroupType::Support,
                        }))
                    } else {
                        highest_precedence_message_action
                    }
                }
            },
        )
    }
}
