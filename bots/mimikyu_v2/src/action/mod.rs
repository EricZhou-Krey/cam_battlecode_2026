use crate::memory::{
    champion_memory::ChampionMemory, message::messages::Message, CampGroupID, LocationID,
};

pub mod combat;

#[derive(Debug, Default, Clone, PartialEq, Copy, Hash)]
enum ActionPrecedence {
    #[default]
    Idle,
    MoveToLocation,
    DefendLocation,
    InCampGroupCombat,
    InChampionCombat,
    EscapeToSpawn,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub enum Action {
    #[default]
    Idle,
    MoveToLocation {
        next_action: Box<Action>,
        location_id: LocationID,
        precedence: i32,
    },
    InChampionComabtKill {
        // TODO
    },
    InChampionCombatStall {
        // TODO
    },
    InChampionCombatEscape {
        // TODO
    },
    InCampGroupCombat {
        camp_group_id: CampGroupID,
    },
    DefendLocation {
        location_id: LocationID,
    },
    EscapeToSpawn,
}

impl Action {
    fn try_prempt_current_action(&mut self, champion_memory: &mut ChampionMemory) {
        let highest_precedence_message_action: Option<Self> =
            champion_memory.message_board.inbox.iter().fold(
                None,
                |highest_precedence_message_action, received_message| match &received_message
                    .message
                {
                    Message::StatusAllyHealth { .. }
                    | Message::StatusEnemyHealth { .. }
                    | Message::StatusAllyLocation { .. }
                    | Message::StatusEnemyLocation { .. }
                    | Message::StatusAllyGemsHeld { .. }
                    | Message::StatusEnemyGemsHeld { .. } => highest_precedence_message_action,

                    message => None,
                },
            );
    }
    pub fn turn(&mut self, champion_memory: &mut ChampionMemory) {
        self.try_prempt_current_action(champion_memory);
        match self {
            _ => {
                //TODO
            }
        }
    }
}
