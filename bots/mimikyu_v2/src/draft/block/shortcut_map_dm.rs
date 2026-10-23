use crate::draft::draft_memory::DraftMemoryBlock;
use cambc::{Bitboard, DistanceField, Team, Vec2};

pub const MAX_SHORTCUTS: usize = 64;

const SHORTCUTS_BYTE_SIZE: usize = MAX_SHORTCUTS * 3;
const COUNT_BYTE_SIZE: usize = 1;
pub const SHORTCUT_DRAFT_BYTE_SIZE: usize = SHORTCUTS_BYTE_SIZE + COUNT_BYTE_SIZE;

const SHORTCUT_THRESDHOLD: u16 = 10;
const DASH_RANGE: i32 = 2;
const TELEPORT_RANGE: i32 = 3;
const DASH_ACTION_COST: u16 = 1;
const TELEPORT_ACTION_COST: u16 = 1;
const OFFSET_BIAS: i32 = 8;
const MINIMUM_SHORTCUT_SPACING: i32 = 3;
const UNREACHED_DISTANCE: u16 = u16::MAX;

#[derive(Debug, Clone, Copy)]
struct ShiftAxes {
    right_sign: i32,
    up_sign: i32,
}

#[derive(Debug, Clone, Copy)]
struct ShortcutCandidate {
    gain: u16,
    action_cost: u16,
    from: Vec2,
    to: Vec2,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShortcutDraftBlock {
    pub shortcuts: [[u8; 3]; MAX_SHORTCUTS], // [x (u8), y (u8), dx (u4), dy (u4)]
    pub shortcut_count: u8,
}

fn probe_shift_axes() -> ShiftAxes {
    let origin: Vec2 = Vec2 { x: 8, y: 8 };
    let mut probe: Bitboard = Bitboard::new().unwrap();
    probe.set(origin, true).unwrap();
    let right: Vec2 = probe.shift_right(1).unwrap().positions().unwrap()[0];
    let up: Vec2 = probe.shift_up(1).unwrap().positions().unwrap()[0];
    ShiftAxes {
        right_sign: right.x - origin.x,
        up_sign: up.y - origin.y,
    }
}

fn shifted(board: &Bitboard, axes: ShiftAxes, along_x: bool, tiles: i32) -> Bitboard {
    if along_x {
        board.shift_right(tiles * axes.right_sign).unwrap()
    } else {
        board.shift_up(tiles * axes.up_sign).unwrap()
    }
}

fn crossing_starts(
    free: &Bitboard,
    gap: &Bitboard,
    gap_width: i32,
    along_x: bool,
    axes: ShiftAxes,
) -> Vec<Vec2> {
    let mut starts: Bitboard = shifted(free, axes, along_x, -(gap_width + 1))
        .intersect(free)
        .unwrap();
    for gap_offset in 1..=gap_width {
        starts
            .intersect_assign(&shifted(gap, axes, along_x, -gap_offset))
            .unwrap();
    }
    starts.positions().unwrap()
}

fn chebyshev_distance(first: Vec2, second: Vec2) -> i32 {
    (first.x - second.x).abs().max((first.y - second.y).abs())
}

fn apply_gain(candidate: &mut ShortcutCandidate, from_distance: u16, to_distance: u16) {
    if from_distance == UNREACHED_DISTANCE || to_distance == UNREACHED_DISTANCE {
        return;
    }
    let walking_saved: u16 = from_distance.abs_diff(to_distance);
    if walking_saved > candidate.action_cost {
        candidate.gain = candidate.gain.max(walking_saved - candidate.action_cost);
    }
}

fn score_candidates(
    fields: &[&DistanceField],
    candidates: &mut [ShortcutCandidate],
    map_width: i32,
) {
    let mut buffer: Vec<u16> = vec![0; fields[0].linear_buffer_size()];
    for field in fields {
        field.export_linear_buffer_into(&mut buffer).unwrap();
        for candidate in candidates.iter_mut() {
            let from_distance: u16 =
                buffer[(candidate.from.y * map_width + candidate.from.x) as usize];
            let to_distance: u16 = buffer[(candidate.to.y * map_width + candidate.to.x) as usize];
            apply_gain(candidate, from_distance, to_distance);
        }
    }
}

impl ShortcutDraftBlock {
    pub fn new(altar_distance_field: &mut DistanceField, blocked_bitboard: &Bitboard) -> Self {
        let altar_bitboard: &Bitboard = cambc::map::altars().unwrap();
        altar_distance_field
            .compute(
                blocked_bitboard,
                altar_bitboard,
                &mut Bitboard::new().unwrap(),
                u16::MAX,
            )
            .unwrap();

        let spawn_bitboard: &Bitboard = cambc::map::spawn_region(Team::Ally).unwrap();
        let mut spawn_distance_field: DistanceField = DistanceField::new().unwrap();
        spawn_distance_field
            .compute(
                blocked_bitboard,
                spawn_bitboard,
                &mut Bitboard::new().unwrap(),
                u16::MAX,
            )
            .unwrap();

        let map_width: i32 = altar_distance_field.width();
        let map_height: i32 = altar_distance_field.height();

        let axes: ShiftAxes = probe_shift_axes();

        let free_bitboard: Bitboard = blocked_bitboard.complement().unwrap();
        let hole_bitboard: &Bitboard = cambc::map::holes().unwrap();
        let wall_bitboard: &Bitboard = cambc::map::walls().unwrap();
        let wall_hole_bitboard: Bitboard = wall_bitboard.union(hole_bitboard).unwrap();

        let crossing_passes: [(&Bitboard, i32, u16); 3] = [
            (hole_bitboard, DASH_RANGE - 1, DASH_ACTION_COST),
            (wall_bitboard, 1, TELEPORT_ACTION_COST),
            (
                &wall_hole_bitboard,
                TELEPORT_RANGE - 1,
                TELEPORT_ACTION_COST,
            ),
        ];

        let mut candidates: Vec<ShortcutCandidate> = Vec::new();

        for (gap_bitboard, gap_width, action_cost) in crossing_passes {
            let landing_offset: i32 = gap_width + 1;
            for along_x in [true, false] {
                for from in crossing_starts(&free_bitboard, gap_bitboard, gap_width, along_x, axes)
                {
                    let to: Vec2 = if along_x {
                        Vec2 {
                            x: from.x + landing_offset,
                            y: from.y,
                        }
                    } else {
                        Vec2 {
                            x: from.x,
                            y: from.y + landing_offset,
                        }
                    };
                    if from.x >= map_width
                        || from.y >= map_height
                        || to.x >= map_width
                        || to.y >= map_height
                    {
                        continue;
                    }
                    candidates.push(ShortcutCandidate {
                        gain: 0,
                        action_cost,
                        from,
                        to,
                    });
                }
            }
        }

        let fields: [&DistanceField; 2] = [&spawn_distance_field, &*altar_distance_field];
        score_candidates(&fields, &mut candidates, map_width);

        candidates.retain(|candidate| candidate.gain >= SHORTCUT_THRESDHOLD);
        candidates.sort_unstable_by_key(|first| std::cmp::Reverse(first.gain));

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
