use crate::draft::draft_memory::{DraftMemory, DraftMemoryBlock};
use cambc::{ChampionDraft, ChampionType};

#[derive(Debug, Clone, PartialEq)]
pub enum Strategy {
    Draft,
    Champion(ChampionType),
}

impl Strategy {
    pub fn determine_strategy() -> Self {
        if cambc::is_draft() {
            Self::Draft
        } else {
            Self::Champion(
                cambc::self_champion()
                    .unwrap()
                    .unwrap()
                    .champion_name()
                    .unwrap(),
            )
        }
    }

    pub fn enter(&self) {
        match self {
            Self::Draft => {
                let draft_memory: DraftMemory = DraftMemory::new();
                let _ = cambc::draft_memory().submit(&draft_memory.as_bytes());

                loop {
                    let pick: ChampionDraft = draft_memory.pick();
                    let _ = cambc::draft::pick(pick);
                    cambc::end_turn();
                }
            }
            Self::Champion(champion_type) => {
                println!("{:?}", cambc::draft_memory());
                /*
                let mut champion_memory: ChampionMemory =
                    ChampionMemory::from(DraftExtractedChampionMemory::from_bytes(
                        cambc::draft_memory().bytes().try_into().unwrap(),
                    ));

                let mut champion_state: ChampionStrategyState =
                    ChampionStrategyState::new(&champion_memory);
                champion_state.init(&mut champion_memory);

                loop {
                    champion_memory.update();
                    champion_state.resolve_state(&mut champion_memory);
                    champion_state.turn(&mut champion_memory);

                    cambc::end_turn();
                }
                */
                loop {
                    cambc::end_turn();
                }
            }
        }
    }
}
