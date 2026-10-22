use crate::draft::{block::camp_dm::MAX_CAMP_GROUPS, draft_memory::DraftMemoryBlock};
use cambc::{Bitboard, DistanceField, Vec2};

pub const MAX_SHORTCUTS: usize = 50;

const SHORTCUTS_BYTE_SIZE: usize = MAX_SHORTCUTS * 3;
const COUNT_BYTE_SIZE: usize = 1;
pub const SHORTCUT_DRAFT_BYTE_SIZE: usize = SHORTCUTS_BYTE_SIZE + COUNT_BYTE_SIZE;

const SHORTCUT_THRESDHOLD: u16 = 10;
const DEFAULT_DASH_RANGE: i32 = 2;
const OFFSET_BIAS: i32 = 8;
const MINIMUM_SHORTCUT_SPACING: i32 = 3;
const UNREACHED_DISTANCE: u16 = u16::MAX;

#[derive(Debug, Clone, PartialEq)]
pub struct ShortcutDraftBlock {
    pub shortcuts: [[u8; 3]; MAX_SHORTCUTS], // [x (u8), y (u8), dx (u4), dy (u4)]
    pub shortcut_count: u8,
}

#[derive(Debug, Clone, Copy)]
struct ShortcutCandidate {
    gain: u16,
    from: Vec2,
    to: Vec2,
}

fn chebyshev_distance(first: Vec2, second: Vec2) -> i32 {
    (first.x - second.x).abs().max((first.y - second.y).abs())
}

fn line_crosses_blocked(from: Vec2, to: Vec2, map_width: i32, blocked_tiles: &[bool]) -> bool {
    let delta_x: i32 = to.x - from.x;
    let delta_y: i32 = to.y - from.y;
    let step_count: i32 = delta_x.abs().max(delta_y.abs());
    for step in 1..step_count {
        let sample_x: i32 = from.x + ((delta_x * step) as f32 / step_count as f32).round() as i32;
        let sample_y: i32 = from.y + ((delta_y * step) as f32 / step_count as f32).round() as i32;
        if blocked_tiles[(sample_y * map_width + sample_x) as usize] {
            return true;
        }
    }
    false
}

impl ShortcutDraftBlock {
    pub fn new(
        camp_group_distance_fields: &[DistanceField; MAX_CAMP_GROUPS],
        camp_group_count: usize,
        altar_distance_field: &mut DistanceField,
        blocked_bitboard: &Bitboard,
    ) -> Self {
        let altar_bitboard: &Bitboard = cambc::map::altars().unwrap();
        altar_distance_field
            .compute(
                blocked_bitboard,
                altar_bitboard,
                &mut Bitboard::new().unwrap(),
                u16::MAX,
            )
            .unwrap();

        let map_width: i32 = altar_distance_field.width();
        let map_height: i32 = altar_distance_field.height();
        let tile_count: usize = (map_width * map_height) as usize;

        let mut blocked_tiles: Vec<bool> = vec![false; tile_count];
        for position in blocked_bitboard.positions().unwrap() {
            blocked_tiles[(position.y * map_width + position.x) as usize] = true;
        }

        let field_count: usize = camp_group_count.min(MAX_CAMP_GROUPS);
        let mut distance_buffers: Vec<Vec<u16>> = Vec::with_capacity(field_count + 1);
        for field in camp_group_distance_fields.iter().take(field_count) {
            let mut buffer: Vec<u16> = vec![0; field.linear_buffer_size()];
            field.export_linear_buffer_into(&mut buffer).unwrap();
            distance_buffers.push(buffer);
        }
        let mut altar_buffer: Vec<u16> = vec![0; altar_distance_field.linear_buffer_size()];
        altar_distance_field
            .export_linear_buffer_into(&mut altar_buffer)
            .unwrap();
        distance_buffers.push(altar_buffer);

        let mut frontier_tiles: Vec<Vec2> = Vec::new();
        for y in 0..map_height {
            for x in 0..map_width {
                if blocked_tiles[(y * map_width + x) as usize] {
                    continue;
                }
                let mut touches_blocked: bool = false;
                for neighbour_y in (y - 1).max(0)..=(y + 1).min(map_height - 1) {
                    for neighbour_x in (x - 1).max(0)..=(x + 1).min(map_width - 1) {
                        if blocked_tiles[(neighbour_y * map_width + neighbour_x) as usize] {
                            touches_blocked = true;
                        }
                    }
                }
                if touches_blocked {
                    frontier_tiles.push(Vec2 { x, y });
                }
            }
        }

        let range_squared: i32 = (DEFAULT_DASH_RANGE * DEFAULT_DASH_RANGE) as i32;
        let maximum_offset: i32 = (OFFSET_BIAS - 1).min(DEFAULT_DASH_RANGE as i32);
        let mut candidates: Vec<ShortcutCandidate> = Vec::new();

        for from in &frontier_tiles {
            for to in &frontier_tiles {
                let delta_x: i32 = to.x - from.x;
                let delta_y: i32 = to.y - from.y;
                if delta_x.abs().max(delta_y.abs()) < 2
                    || delta_x.abs() > maximum_offset
                    || delta_y.abs() > maximum_offset
                    || delta_x * delta_x + delta_y * delta_y > range_squared
                {
                    continue;
                }
                if !line_crosses_blocked(*from, *to, map_width, &blocked_tiles) {
                    continue;
                }

                let from_index: usize = (from.y * map_width + from.x) as usize;
                let to_index: usize = (to.y * map_width + to.x) as usize;
                let mut best_gain: u16 = 0;
                for buffer in &distance_buffers {
                    let from_distance: u16 = buffer[from_index];
                    let to_distance: u16 = buffer[to_index];
                    if from_distance == UNREACHED_DISTANCE || to_distance == UNREACHED_DISTANCE {
                        continue;
                    }
                    if from_distance > to_distance + 1 {
                        best_gain = best_gain.max(from_distance - to_distance - 1);
                    }
                }
                if best_gain >= SHORTCUT_THRESDHOLD {
                    candidates.push(ShortcutCandidate {
                        gain: best_gain,
                        from: *from,
                        to: *to,
                    });
                }
            }
        }

        candidates.sort_unstable_by(|first, second| second.gain.cmp(&first.gain));

        let mut selected: Vec<ShortcutCandidate> = Vec::with_capacity(MAX_SHORTCUTS);
        for candidate in candidates {
            if selected.len() == MAX_SHORTCUTS {
                break;
            }
            let too_close: bool = selected.iter().any(|chosen: &ShortcutCandidate| {
                chebyshev_distance(chosen.from, candidate.from) < MINIMUM_SHORTCUT_SPACING
                    && chebyshev_distance(chosen.to, candidate.to) < MINIMUM_SHORTCUT_SPACING
            });
            if !too_close {
                selected.push(candidate);
            }
        }

        let shortcut_count: u8 = selected.len() as u8;

        let mut shortcuts: [[u8; 3]; MAX_SHORTCUTS] = [[0; 3]; MAX_SHORTCUTS];
        for (slot, candidate) in selected.iter().enumerate() {
            let encoded_delta_x: u8 = (candidate.to.x - candidate.from.x + OFFSET_BIAS) as u8;
            let encoded_delta_y: u8 = (candidate.to.y - candidate.from.y + OFFSET_BIAS) as u8;
            shortcuts[slot] = [
                candidate.from.x as u8,
                candidate.from.y as u8,
                (encoded_delta_x << 4) | encoded_delta_y,
            ];
        }

        Self {
            shortcuts,
            shortcut_count,
        }
    }
}

impl DraftMemoryBlock<SHORTCUT_DRAFT_BYTE_SIZE> for ShortcutDraftBlock {
    fn as_bytes(&self) -> [u8; SHORTCUT_DRAFT_BYTE_SIZE] {
        let mut bytes: [u8; SHORTCUT_DRAFT_BYTE_SIZE] = [0; SHORTCUT_DRAFT_BYTE_SIZE];
        let mut byte_pointer: usize = 0;

        bytes[byte_pointer..byte_pointer + SHORTCUTS_BYTE_SIZE]
            .copy_from_slice(self.shortcuts.as_flattened());
        byte_pointer += SHORTCUTS_BYTE_SIZE;

        bytes[byte_pointer] = self.shortcut_count;
        bytes
    }
    fn from_bytes(bytes: &[u8; SHORTCUT_DRAFT_BYTE_SIZE]) -> Self {
        let shortcuts = std::array::from_fn(|i| {
            let start = i * 3;
            [bytes[start], bytes[start + 1], bytes[start + 2]]
        });

        Self {
            shortcuts,
            shortcut_count: bytes[SHORTCUTS_BYTE_SIZE],
        }
    }
}
