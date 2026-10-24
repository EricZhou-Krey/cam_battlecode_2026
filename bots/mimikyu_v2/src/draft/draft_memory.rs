use crate::draft::block::{
    camp_dm::{CampDraftBlock, CAMP_DRAFT_BYTE_SIZE},
    control_point_dm::{ControlPointDraftBlock, CONTROL_POINT_DRAFT_SIZE},
    shortcut_map_dm::{ShortcutDraftBlock, SHORTCUT_DRAFT_BYTE_SIZE},
};
use cambc::{Bitboard, ChampionDraft, DistanceField};

pub const DRAFT_MEMORY_BYTE_SIZE: usize = 1024;

pub const DRAFT_PICKS: &[ChampionDraft; 5] = &[
    ChampionDraft::Willow,
    ChampionDraft::HeartOfRuin,
    ChampionDraft::Prism,
    ChampionDraft::VesperVale,
    ChampionDraft::CassiAndIssac,
];

#[derive(Debug, Clone, PartialEq)]
pub struct DraftMemory {
    pub camp_memory: CampDraftBlock,
    pub shortcut_memory: ShortcutDraftBlock,
    pub control_point_memory: ControlPointDraftBlock,
}

impl Default for DraftMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl DraftMemory {
    pub fn new() -> Self {
        let blocked_bitboard: Bitboard = cambc::map::walls()
            .unwrap()
            .union(cambc::map::holes().unwrap())
            .unwrap()
            .union(cambc::map::altars().unwrap())
            .unwrap();

        let mut altar_distance_field: DistanceField = DistanceField::new().unwrap();

        let camp_memory: CampDraftBlock = CampDraftBlock::new();

        let shortcut_memory: ShortcutDraftBlock =
            ShortcutDraftBlock::new(&mut altar_distance_field, &blocked_bitboard);

        let control_point_memory: ControlPointDraftBlock = ControlPointDraftBlock::new();

        Self {
            camp_memory,
            shortcut_memory,
            control_point_memory,
        }
    }

    pub fn pick(&self) -> ChampionDraft {
        DRAFT_PICKS[cambc::round_number().unwrap() as usize]
    }
}

impl DraftMemoryBlock<DRAFT_MEMORY_BYTE_SIZE> for DraftMemory {
    fn as_bytes(&self) -> [u8; DRAFT_MEMORY_BYTE_SIZE] {
        let mut bytes: [u8; DRAFT_MEMORY_BYTE_SIZE] = [0; DRAFT_MEMORY_BYTE_SIZE];
        let mut byte_pointer: usize = 0;

        bytes[byte_pointer..byte_pointer + CAMP_DRAFT_BYTE_SIZE]
            .copy_from_slice(&self.camp_memory.as_bytes());
        byte_pointer += CAMP_DRAFT_BYTE_SIZE;

        bytes[byte_pointer..byte_pointer + SHORTCUT_DRAFT_BYTE_SIZE]
            .copy_from_slice(&self.shortcut_memory.as_bytes());
        byte_pointer += SHORTCUT_DRAFT_BYTE_SIZE;

        bytes[byte_pointer..byte_pointer + CONTROL_POINT_DRAFT_SIZE]
            .copy_from_slice(&self.control_point_memory.as_bytes());

        bytes
    }

    fn from_bytes(bytes: &[u8; DRAFT_MEMORY_BYTE_SIZE]) -> Self {
        let mut byte_pointer: usize = 0;

        let camp_memory: CampDraftBlock = CampDraftBlock::from_bytes(
            bytes[byte_pointer..byte_pointer + CAMP_DRAFT_BYTE_SIZE]
                .try_into()
                .unwrap(),
        );
        byte_pointer += CAMP_DRAFT_BYTE_SIZE;

        let shortcut_memory: ShortcutDraftBlock = ShortcutDraftBlock::from_bytes(
            bytes[byte_pointer..byte_pointer + SHORTCUT_DRAFT_BYTE_SIZE]
                .try_into()
                .unwrap(),
        );
        byte_pointer += SHORTCUT_DRAFT_BYTE_SIZE;

        let control_point_memory: ControlPointDraftBlock = ControlPointDraftBlock::from_bytes(
            bytes[byte_pointer..byte_pointer + CONTROL_POINT_DRAFT_SIZE]
                .try_into()
                .unwrap(),
        );

        Self {
            camp_memory,
            shortcut_memory,
            control_point_memory,
        }
    }
}

pub trait DraftMemoryBlock<const N: usize> {
    fn as_bytes(&self) -> [u8; N];
    fn from_bytes(bytes: &[u8; N]) -> Self;
}
