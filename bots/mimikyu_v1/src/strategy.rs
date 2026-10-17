use crate::draft::memory::DraftMemory;
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
                    if cambc::round_number()? + 1 == (cambc::draft::team_size()? as u32) {
                        let _ = cambc::draft_memory().submit(&draft_memory.bytes);
                    }
                    let _ = cambc::draft::pick(pick);
                    cambc::end_turn();
                }
            }
            Self::Champion => {
                cambc::end_turn();
            }
        }
        Ok(())
    }
}
