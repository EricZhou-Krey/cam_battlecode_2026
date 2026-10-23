use crate::{
    draft::draft_memory::{DraftMemory, DraftMemoryBlock, DRAFT_MEMORY_BYTE_SIZE},
    memory::{
        champion_states::ChampionStates, map_cache::MapCache, message::message_board::MessageBoard,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct ChampionMemory {
    pub message_queue: MessageBoard,
    pub map_cache: MapCache,
    pub champion_states: ChampionStates,
}

impl ChampionMemory {
    pub fn from_draft_bytes(bytes: [u8; DRAFT_MEMORY_BYTE_SIZE]) -> Self {
        let draft_memory: DraftMemory = DraftMemory::from_bytes(&bytes);

        todo!()
    }

    pub fn turn(&mut self) {
        todo!()
    }
}
