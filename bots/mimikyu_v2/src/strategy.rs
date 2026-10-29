use crate::{
    action::{movement::MovementState, Action},
    draft::draft_memory::{DraftMemory, DraftMemoryBlock},
    memory::champion_memory::ChampionMemory,
    sdk_interact::debug::{debug_current_action, debug_draft_init},
};
use cambc::ChampionDraft;

#[derive(Debug, Clone, PartialEq)]
pub enum Strategy {
    Draft,
    Champion,
}

impl Strategy {
    pub fn determine_strategy() -> Self {
        if cambc::is_draft() {
            Self::Draft
        } else {
            Self::Champion
        }
    }

    pub fn enter(&self) {
        match self {
            Self::Draft => {
                let draft_memory: DraftMemory = DraftMemory::new();
                let _ = cambc::draft_memory().submit(&draft_memory.as_bytes());
                debug_draft_init(&draft_memory);

                loop {
                    let pick: ChampionDraft = draft_memory.pick();
                    let _ = cambc::draft::pick(pick);
                    cambc::end_turn();
                }
            }
            Self::Champion => {
                let mut champion_memory: ChampionMemory = ChampionMemory::from_draft_bytes(
                    cambc::draft_memory().bytes().try_into().unwrap(),
                );
                //let mut current_action: Action = Action::Idle;

                // --- DEBUG SETUP START ---
                let mut current_action = Action::MoveToLocation(MovementState {
                    next_action: Box::new(None),
                    distance_field: None,
                    location_id: Some(crate::memory::LocationID::CampGroup(
                        champion_memory
                            .map
                            .distance_fields
                            .camp_group_discovery_queue[0],
                    )),
                    accepted_distance: 3,
                    action_group_type: crate::action::ActionGroupType::Intermediate,
                });
                // --- DEBUG SETUP END ---

                loop {
                    champion_memory.map.distance_fields.reset_discovered_state();
                    champion_memory.map.tick_camp_group_respawn_timers();
                    champion_memory.message_board.sync();
                    champion_memory.update_by_status_messgges();

                    current_action.turn(&mut champion_memory);

                    champion_memory
                        .map
                        .distance_fields
                        .lazy_discover_from_queue();
                    champion_memory.message_board.flush();

                    //debug_champion_turn(&champion_memory);
                    debug_current_action(&current_action, &champion_memory);

                    cambc::end_turn();
                }
            }
        }
    }
}
