use crate::draft::{block::camp_dm::MAX_CAMP_GROUPS, draft_memory::DraftMemoryBlock};
use cambc::DistanceField;

pub const MAX_CONTROL_POINTS: usize = 32;

const CONTROL_POINTS_BYTE_SIZE: usize = MAX_CONTROL_POINTS * 2;
pub const CONTROL_POINT_DRAFT_SIZE: usize = CONTROL_POINTS_BYTE_SIZE;

#[derive(Debug, Clone, PartialEq)]
pub struct ControlPointDraftBlock {
    pub control_points: [[u8; 2]; MAX_CONTROL_POINTS],
}

impl ControlPointDraftBlock {
    pub fn new(
        camp_group_distance_fields: &[DistanceField; MAX_CAMP_GROUPS],
        altar_distance_field: &DistanceField,
    ) -> Self {
        // TODO
        Self {
            control_points: [[0; 2]; MAX_CONTROL_POINTS],
        }
    }
}

impl DraftMemoryBlock<CONTROL_POINT_DRAFT_SIZE> for ControlPointDraftBlock {
    fn as_bytes(&self) -> [u8; CONTROL_POINT_DRAFT_SIZE] {
        let mut bytes: [u8; CONTROL_POINT_DRAFT_SIZE] = [0; CONTROL_POINT_DRAFT_SIZE];

        bytes[0..CONTROL_POINTS_BYTE_SIZE].copy_from_slice(self.control_points.as_flattened());
        bytes
    }
    fn from_bytes(bytes: &[u8; CONTROL_POINT_DRAFT_SIZE]) -> Self {
        let control_points: [[u8; 2]; MAX_CONTROL_POINTS] = std::array::from_fn(|i| {
            let start = i * 2;
            [bytes[start], bytes[start + 1]]
        });

        Self { control_points }
    }
}
