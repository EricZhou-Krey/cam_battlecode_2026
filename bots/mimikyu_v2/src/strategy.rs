use crate::{
    draft::draft_memory::{DraftMemory, DraftMemoryBlock},
    memory::champion_memory::ChampionMemory,
    sdk_interact::debug::{debug_champion_init, debug_draft_init},
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
                debug_champion_init(&champion_memory);
                loop {
                    champion_memory.turn();
                    cambc::end_turn();
                }
            }
        }
    }
}
