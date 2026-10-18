use crate::{
    champion::{
        memory::{draft_extracted_memory::DraftExtractedChampionMemory, ChampionMemory},
        strategy_state::{farm::FarmStrategy, ChampionStrategyState, StrategyState},
    },
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
                let mut champion_memory: ChampionMemory =
                    ChampionMemory::from(DraftExtractedChampionMemory::from_bytes(
                        cambc::draft_memory().bytes().try_into().unwrap(),
                    ));

                //let mut champion_state: ChampionStrategyState =
                //    ChampionStrategyState::new(&champion_memory);
                let mut champion_state: ChampionStrategyState = ChampionStrategyState::Farm;
                FarmStrategy::init_data(&mut champion_memory);

                loop {
                    //let _ = champion_memory.draw_debug();
                    //let _ = champion_memory.draw_safe_paths();
                    //let _ = champion_memory.draw_value_paths();
                    //let _ = champion_memory.draw_contested_camps();
                    //let _ = champion_memory.draw_initial_champions();

                    champion_memory.update();
                    champion_state.resolve_state(&mut champion_memory);
                    champion_state.turn(&mut champion_memory);

                    cambc::end_turn();
                }
            }
        }
    }
}
