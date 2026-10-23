use crate::draft::draft_memory::DraftMemoryBlock;
use cambc::{Bitboard, DistanceField, Vec2};

pub const MAX_CONTROL_POINTS: usize = 8;

const CONTROL_POINTS_BYTE_SIZE: usize = MAX_CONTROL_POINTS * 2;
pub const CONTROL_POINT_DRAFT_SIZE: usize = CONTROL_POINTS_BYTE_SIZE;

const MINIMUM_CONTROL_POINT_SPACING: i32 = 8;
const MINIMUM_NEAREST_DISTANCE: u16 = 2;
const NEAREST_WEIGHT: i32 = 4;
const CENTROID_WEIGHT: i32 = 1;
const SAMPLE_STRIDE: i32 = 2;
const UNREACHED_DISTANCE: u16 = u16::MAX;

fn chebyshev_distance(first: [u8; 2], second: [u8; 2]) -> i32 {
    (first[0] as i32 - second[0] as i32)
        .abs()
        .max((first[1] as i32 - second[1] as i32).abs())
}

#[derive(Debug, Clone, Copy)]
struct ControlPointCandidate {
    score: i32,
    tile: [u8; 2],
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
        let blocked_bitboard: Bitboard = cambc::map::walls()
            .unwrap()
            .union(cambc::map::holes().unwrap())
            .unwrap();

        let mut root_bitboard: Bitboard = Bitboard::new().unwrap();
        root_bitboard.union_assign(altar_bitboard).unwrap();
        let mut camp_group_tiles: Bitboard = Bitboard::new().unwrap();
        for camp_group in cambc::map::camp_groups().unwrap() {
            camp_group.spawn_tiles_into(&mut camp_group_tiles).unwrap();
            root_bitboard.union_assign(&camp_group_tiles).unwrap();
        }

        let root_positions: Vec<Vec2> = root_bitboard.positions().unwrap();
        let mut sum_x: i32 = 0;
        let mut sum_y: i32 = 0;
        let mut box_min_x: i32 = i32::MAX;
        let mut box_min_y: i32 = i32::MAX;
        let mut box_max_x: i32 = 0;
        let mut box_max_y: i32 = 0;
        for position in &root_positions {
            sum_x += position.x;
            sum_y += position.y;
            box_min_x = box_min_x.min(position.x);
            box_min_y = box_min_y.min(position.y);
            box_max_x = box_max_x.max(position.x);
            box_max_y = box_max_y.max(position.y);
        }
        let root_count: i32 = (root_positions.len() as i32).max(1);
        let centroid: [u8; 2] = [(sum_x / root_count) as u8, (sum_y / root_count) as u8];

        let mut nearest_distance_field: DistanceField = DistanceField::new().unwrap();
        nearest_distance_field
            .compute(
                &blocked_bitboard,
                &root_bitboard,
                &mut Bitboard::new().unwrap(),
                u16::MAX,
            )
            .unwrap();

        let map_width: i32 = cambc::map::width().unwrap();
        let map_height: i32 = cambc::map::height().unwrap();
        let mut buffer: Vec<u16> = vec![0; nearest_distance_field.linear_buffer_size()];
        nearest_distance_field
            .export_linear_buffer_into(&mut buffer)
            .unwrap();

        let cell_size: i32 = MINIMUM_CONTROL_POINT_SPACING;
        let cell_columns: i32 = map_width / cell_size + 1;
        let cell_rows: i32 = map_height / cell_size + 1;
        let mut cell_best: Vec<Option<ControlPointCandidate>> =
            vec![None; (cell_columns * cell_rows) as usize];

        let end_x: i32 = box_max_x.min(map_width - 1);
        let end_y: i32 = box_max_y.min(map_height - 1);
        let mut y: i32 = box_min_y.max(0);
        while y <= end_y {
            let mut x: i32 = box_min_x.max(0);
            while x <= end_x {
                let distance: u16 = buffer[(y * map_width + x) as usize];
                if distance != UNREACHED_DISTANCE && distance >= MINIMUM_NEAREST_DISTANCE {
                    let tile: [u8; 2] = [x as u8, y as u8];
                    let score: i32 = distance as i32 * NEAREST_WEIGHT
                        - chebyshev_distance(tile, centroid) * CENTROID_WEIGHT;
                    let cell_index: usize =
                        ((y / cell_size) * cell_columns + x / cell_size) as usize;
                    let is_better: bool = match cell_best[cell_index] {
                        Some(existing) => score > existing.score,
                        None => true,
                    };
                    if is_better {
                        cell_best[cell_index] = Some(ControlPointCandidate { score, tile });
                    }
                }
                x += SAMPLE_STRIDE;
            }
            y += SAMPLE_STRIDE;
        }

        let mut candidates: Vec<ControlPointCandidate> = cell_best.into_iter().flatten().collect();
        candidates.sort_unstable_by_key(|candidate: &ControlPointCandidate| {
            std::cmp::Reverse(candidate.score)
        });

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
                        chebyshev_distance(*chosen, candidate.tile) < MINIMUM_CONTROL_POINT_SPACING
                    });
            if !too_close {
                control_points[selected_count] = candidate.tile;
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
