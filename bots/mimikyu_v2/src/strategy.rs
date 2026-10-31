use crate::{
    action::Action,
    draft::draft_memory::{DraftMemory, DraftMemoryBlock},
    memory::champion_memory::ChampionMemory,
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
                crate::sdk_interact::debug::debug_draft_init(&draft_memory);

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
                let mut current_action: Action = Action::Idle;

                loop {
                    champion_memory.map.distance_fields.reset_discovered_state();
                    champion_memory.map.tick_camp_group_respawn_timers();
                    champion_memory.message_board.sync();
                    champion_memory.update_by_visible_champions();
                    champion_memory.update_by_status_messgges();
                    champion_memory
                        .map
                        .distance_fields
                        .lazy_discover(&champion_memory.map.points_of_interest);

                    current_action.turn(&mut champion_memory);

                    champion_memory.message_board.flush();

                    // crate::sdk_interact::debug::debug_champion_turn(&champion_memory);
                    crate::sdk_interact::debug::debug_current_action(
                        &current_action,
                        &champion_memory,
                    );
                    cambc::end_turn();
                }
            }
        }
    }
}
