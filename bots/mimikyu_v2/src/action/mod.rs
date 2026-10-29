use crate::{
    action::{
        combat::state::{ChampionCombatState, CombatActions, DefendState},
        farm::{CampGroupCombatState, CollectionState, FarmActions, TributeState},
        movement::{MoveToSpawnState, MovementActions, MovementState, MOVE_PROXIMITY},
        shop::ActionBuyExtension,
    },
    memory::{champion_memory::ChampionMemory, message::messages::Message, LocationID},
};
use cambc::{DistanceField, Vec2};

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
        // Note: interesting distance field try_get() "leak" attempted by message reaction even if
        // current or other action is higher precedence, meaning better understand of the world for
        // when the situation it is currently in resolves
        let message_reaction: Option<Self> = Self::message_reaction(champion_memory);

        // Should attempt to prempt when:
        //  - Combat range/ Champion visible
        //  - Low hp/need to escape and heal
        //  - Reaction to messages

        // for signal in current

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
        let me_position: Vec2 = cambc::self_champion().unwrap().unwrap().position().unwrap();
        let mut max_precedence: i32 = i32::MIN;
        let mut reaction: Option<Self> = None;

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
                    reaction = Some(Self::MoveToLocation(MovementState {
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
                    }))
                }
                Message::FarmCollection {
                    camp_group_id,
                    ally_champion_claiming: _,
                } => {
                    if max_precedence > ActionGroupType::EndFarm as i32 {
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
                    reaction = Some(Self::MoveToLocation(MovementState {
                        next_action: Box::new(Some(Self::CollectingFromCamp(CollectionState {}))),
                        distance_field,
                        location_id,
                        accepted_distance: 0,
                        action_group_type: ActionGroupType::EndFarm,
                    }))
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
                    reaction = Some(Self::MoveToLocation(MovementState {
                        next_action: Box::new(Some(Self::CollectingFromCamp(CollectionState {}))),
                        distance_field,
                        location_id: *location_id,
                        accepted_distance: MOVE_PROXIMITY,
                        action_group_type: ActionGroupType::Support,
                    }));
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
                    reaction = Some(Self::MoveToLocation(MovementState {
                        next_action: Box::new(None),
                        distance_field,
                        location_id: *location_id,
                        accepted_distance: MOVE_PROXIMITY,
                        action_group_type: ActionGroupType::Support,
                    }));
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
                    reaction = Some(Self::MoveToLocation(MovementState {
                        next_action: Box::new(None),
                        distance_field,
                        location_id: *location_id,
                        accepted_distance: MOVE_PROXIMITY,
                        action_group_type: ActionGroupType::Support,
                    }))
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
                    reaction = Some(Self::MoveToLocation(MovementState {
                        next_action: Box::new(None),
                        distance_field,
                        location_id: *location_id,
                        accepted_distance: MOVE_PROXIMITY,
                        action_group_type: ActionGroupType::Support,
                    }));
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
                    reaction = Some(Self::MoveToLocation(MovementState {
                        next_action: Box::new(None),
                        distance_field,
                        location_id: *location_id,
                        accepted_distance: MOVE_PROXIMITY,
                        action_group_type: ActionGroupType::Support,
                    }))
                }
            }
        }

        reaction
    }
}
