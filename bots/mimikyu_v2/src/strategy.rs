use crate::sdk_interact::{self, ChampionUnwrapperExtension};
use cambc::{ChampionDraft, ChampionType, Result};

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
            Self::Champion(sdk_interact::me().champion_type())
        }
    }

    pub fn enter(&self) {
        match self {
            Self::Draft => {
                let mut draft_memory: DraftMemory = DraftMemory::new();
                loop {
                    let pick: ChampionDraft = draft_memory.pick();
                    draft_memory.update(&pick);
                    if sdk_interact::round_number() + 1 == sdk_interact::team_size() as u32 {
                        let _ = cambc::draft_memory().submit(&draft_memory.bytes);
                    }
                    let _ = cambc::draft::pick(pick);
                    cambc::end_turn();
                }
            }
            Self::Champion(champion_type) => {
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
            }
        }
    }
}
