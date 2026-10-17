use crate::{
    champion::{debug::MemoryDebug, memory::ChampionMemory},
    draft::memory::DraftMemory,
};
use cambc::{ChampionDraft, Result};

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

    pub fn enter(&self) -> Result<()> {
        match self {
            Self::Draft => {
                let mut draft_memory: DraftMemory = DraftMemory::new();
                loop {
                    let pick: ChampionDraft = draft_memory.pick();
                    draft_memory.update(&pick);
                    if cambc::round_number()? + 1
                        == (cambc::draft::team_size()?.try_into().unwrap())
                    {
                        let _ = cambc::draft_memory().submit(&draft_memory.bytes);
                    }
                    let _ = cambc::draft::pick(pick);
                    cambc::end_turn();
                }
            }
            Self::Champion => {
                let champion_memory: ChampionMemory =
                    ChampionMemory::from_bytes(cambc::draft_memory().bytes().try_into().unwrap());
                loop {
                    let _ = champion_memory.draw_debug();
                    cambc::end_turn();
                }
            }
        }
    }
}
