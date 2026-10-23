use crate::{
    draft::draft_memory::{DraftMemory, DraftMemoryBlock, DRAFT_MEMORY_BYTE_SIZE},
    memory::{
        champion_info::AllChampionInfo, lazy_map::LazyMap, message::message_board::MessageBoard,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct ChampionMemory {
    pub map: LazyMap,
    pub champion_info: AllChampionInfo,
    pub message_board: MessageBoard,
}

impl ChampionMemory {
    pub fn from_draft_bytes(bytes: [u8; DRAFT_MEMORY_BYTE_SIZE]) -> Self {
        let draft_memory: DraftMemory = DraftMemory::from_bytes(&bytes);

        Self {
            map: LazyMap::new(&draft_memory),
            champion_info: AllChampionInfo::new(),
            message_board: MessageBoard::new(),
        }
    }

    pub fn turn(&mut self) {
        //TODO
    }
}
