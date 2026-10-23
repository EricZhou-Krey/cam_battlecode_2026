use crate::{draft::draft_memory::DraftMemoryBlock, sdk_interact::map::Center};
use cambc::Bitboard;

pub const MAX_CONTROL_POINTS: usize = 8;

const CONTROL_POINTS_BYTE_SIZE: usize = MAX_CONTROL_POINTS * 2;
pub const CONTROL_POINT_DRAFT_SIZE: usize = CONTROL_POINTS_BYTE_SIZE;

const MINIMUM_CONTROL_POINT_SPACING: i32 = 6;

fn distance_squared(first: [u8; 2], second: [u8; 2]) -> i32 {
    let delta_x: i32 = first[0] as i32 - second[0] as i32;
    let delta_y: i32 = first[1] as i32 - second[1] as i32;
    delta_x * delta_x + delta_y * delta_y
}

fn chebyshev_distance(first: [u8; 2], second: [u8; 2]) -> i32 {
    (first[0] as i32 - second[0] as i32)
        .abs()
        .max((first[1] as i32 - second[1] as i32).abs())
}

#[derive(Debug, Clone, PartialEq)]
pub struct ControlPointDraftBlock {
    pub control_points: [[u8; 2]; MAX_CONTROL_POINTS],
}

impl Default for ControlPointDraftBlock {
    fn default() -> Self {
        Self::new()
    }
}

impl ControlPointDraftBlock {
    pub fn new() -> Self {
        let altar_bitboard: &Bitboard = cambc::map::altars().unwrap();
        let map_center: [u8; 2] = [
            (altar_bitboard.width() / 2) as u8,
            (altar_bitboard.height() / 2) as u8,
        ];

        let mut candidates: Vec<[u8; 2]> = Vec::new();
        for camp_group in cambc::map::camp_groups().unwrap() {
            candidates.push(camp_group.spawn_tiles().unwrap().center());
        }
        for position in altar_bitboard.positions().unwrap() {
            candidates.push([position.x as u8, position.y as u8]);
        }

        candidates.sort_by_key(|candidate: &[u8; 2]| distance_squared(*candidate, map_center));

        let mut control_points: [[u8; 2]; MAX_CONTROL_POINTS] = [[0; 2]; MAX_CONTROL_POINTS];
        let mut selected_count: usize = 0;
        for candidate in candidates {
            if selected_count == MAX_CONTROL_POINTS {
                break;
            }
            let too_close: bool =
                control_points[..selected_count]
                    .iter()
                    .any(|chosen: &[u8; 2]| {
                        chebyshev_distance(*chosen, candidate) < MINIMUM_CONTROL_POINT_SPACING
                    });
            if !too_close {
                control_points[selected_count] = candidate;
                selected_count += 1;
            }
        }

        Self { control_points }
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
