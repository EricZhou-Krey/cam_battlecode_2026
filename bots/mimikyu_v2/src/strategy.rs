use crate::{
    draft::draft_memory::{DraftMemory, DraftMemoryBlock},
    memory::champion_memory::ChampionMemory,
    sdk_interact::debug::print_distance_fields,
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
                // debug_draft_init(&draft_memory);

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
                loop {
                    // debug_champion_turn(&champion_memory);
                    champion_memory.turn();
                    print_distance_fields(&champion_memory);
                    cambc::end_turn();
                }
            }
        }
    }
}
