use crate::draft::{
    block::{
        camp_dm::CAMP_DRAFT_BYTE_SIZE, control_point_dm::CONTROL_POINT_DRAFT_SIZE,
        shortcut_map_dm::SHORTCUT_DRAFT_BYTE_SIZE,
    },
    draft_memory::{DraftMemory, DraftMemoryBlock, DRAFT_MEMORY_BYTE_SIZE},
};
use crate::sdk_interact::map::Center;
use cambc::{CampGroup, Team, Vec2};

const COLOR_SPAWN: u32 = 0x00FFFF;
const COLOR_SHORT_JUMP: u32 = 0x3399FF;
const COLOR_LONG_JUMP: u32 = 0xFF33CC;
const COLOR_SHORTCUT_FROM: u32 = 0xFFFF00;
const COLOR_SHORTCUT_TO: u32 = 0xFFFFFF;
const COLOR_CONTROL_POINT: u32 = 0xFF8800;
const LINES_TO_NEAREST_CAMPS: usize = 5;

fn pack_rgb(red: u8, green: u8, blue: u8) -> u32 {
    ((red as u32) << 16) | ((green as u32) << 8) | blue as u32
}

fn rank_color(rank: usize, total: usize) -> u32 {
    let fraction: f32 = if total > 1 {
        rank as f32 / (total - 1) as f32
    } else {
        0.0
    };
    pack_rgb(
        (255.0 * fraction) as u8,
        (255.0 * (1.0 - fraction)) as u8,
        0,
    )
}

fn to_vec2(point: [u8; 2]) -> Vec2 {
    Vec2 {
        x: point[0] as i32,
        y: point[1] as i32,
    }
}

fn print_round_trip(memory: &DraftMemory) {
    let bytes: [u8; DRAFT_MEMORY_BYTE_SIZE] = memory.as_bytes();
    let decoded: DraftMemory = DraftMemory::from_bytes(&bytes);
    let used_bytes: usize =
        CAMP_DRAFT_BYTE_SIZE + SHORTCUT_DRAFT_BYTE_SIZE + CONTROL_POINT_DRAFT_SIZE;
    println!("== DRAFT MEMORY ==");
    println!(
        "The draft memory is packed into a fixed {DRAFT_MEMORY_BYTE_SIZE} byte buffer that the bot reads back on later turns."
    );
    println!("Blocks use {used_bytes} bytes in total (camps {CAMP_DRAFT_BYTE_SIZE}, shortcuts {SHORTCUT_DRAFT_BYTE_SIZE}, control points {CONTROL_POINT_DRAFT_SIZE}).");
    println!("Round trip check encodes the memory to bytes and decodes it again. Anything other than ok means the byte layout is broken.");
    println!(
        "Round trip result: {}",
        if decoded == *memory { "ok" } else { "MISMATCH" }
    );
}

fn draw_camps(memory: &DraftMemory) {
    let camp_block = &memory.camp_memory;
    let camp_groups: Vec<CampGroup> = cambc::map::camp_groups().unwrap();
    let spawn_center: [u8; 2] = cambc::map::spawn_region(Team::Ally).unwrap().center();
    let camp_group_count: usize = camp_block.camp_group_count as usize;

    cambc::debug::draw_dot(to_vec2(spawn_center), COLOR_SPAWN).unwrap();

    println!("== CAMPS ==");
    println!("Camp groups are ranked by straight line distance from the centre of our spawn region to the centre of each group. Rank 0 is the closest and is expected to be found first.");
    println!("On the map: the cyan dot is the spawn centre. Each camp group centre is a dot, green for the lowest rank through red for the highest rank. Lines run from the spawn centre to the {LINES_TO_NEAREST_CAMPS} closest groups.");
    println!("Columns: rank, camp group id, centre tile, effective hp and damage per round (wolf, elk, monolith and golem camps only), gem value, gold value.");
    println!("Spawn centre: {spawn_center:?}, camp groups: {camp_group_count}");

    for rank in 0..camp_group_count {
        let camp_id: usize = camp_block.camp_group_discovery_order[rank] as usize;
        let camp_center: [u8; 2] = camp_groups[camp_id].spawn_tiles().unwrap().center();
        let color: u32 = rank_color(rank, camp_group_count);

        cambc::debug::draw_dot(to_vec2(camp_center), color).unwrap();
        if rank < LINES_TO_NEAREST_CAMPS {
            cambc::debug::draw_line(to_vec2(spawn_center), to_vec2(camp_center), color).unwrap();
        }

        println!(
            "  rank {:>2} camp {:>2} at {:?} hp {} dmg {} gem {} gold {}",
            rank,
            camp_id,
            camp_center,
            camp_block.camp_group_effective_hp[camp_id],
            camp_block.camp_group_damage_per_round[camp_id],
            camp_block.camp_group_gem_value[camp_id],
            camp_block.camp_group_gold_value[camp_id],
        );
    }
}

fn draw_shortcuts(memory: &DraftMemory) {
    let shortcut_block = &memory.shortcut_memory;
    let shortcut_count: usize = shortcut_block.shortcut_count as usize;

    println!("== SHORTCUTS ==");
    println!("A shortcut is a jump over a hole or wall that saves at least the gain threshold in walking distance, measured from our spawn or from the altars. It works in both directions.");
    println!("Each entry stores a start tile and an offset to the landing tile. The offset is stored with a bias of 8 in each half byte, and the printout shows it already decoded.");
    println!("On the map: yellow dot is the start tile, white dot is the landing tile. A blue line is a jump of length 2 (a dash over a hole, or a teleport over a wall). A magenta line is a jump of length 3, which needs a teleport crystal.");
    println!("Columns: slot, start tile -> landing tile, decoded offset, jump length in tiles.");
    println!(
        "Shortcuts found: {shortcut_count} of a maximum {}",
        shortcut_block.shortcuts.len()
    );

    for (slot, entry) in shortcut_block
        .shortcuts
        .iter()
        .take(shortcut_count)
        .enumerate()
    {
        let from: Vec2 = Vec2 {
            x: entry[0] as i32,
            y: entry[1] as i32,
        };
        let delta_x: i32 = (entry[2] >> 4) as i32 - 8;
        let delta_y: i32 = (entry[2] & 0x0F) as i32 - 8;
        let to: Vec2 = Vec2 {
            x: from.x + delta_x,
            y: from.y + delta_y,
        };
        let jump_length: i32 = delta_x.abs().max(delta_y.abs());
        let line_color: u32 = if jump_length > 2 {
            COLOR_LONG_JUMP
        } else {
            COLOR_SHORT_JUMP
        };

        cambc::debug::draw_line(from, to, line_color).unwrap();
        cambc::debug::draw_dot(from, COLOR_SHORTCUT_FROM).unwrap();
        cambc::debug::draw_dot(to, COLOR_SHORTCUT_TO).unwrap();

        println!(
            "  {:>2}: ({}, {}) -> ({}, {}) offset ({}, {}) length {}",
            slot, from.x, from.y, to.x, to.y, delta_x, delta_y, jump_length
        );
    }
}

fn draw_control_points(memory: &DraftMemory) {
    let control_points = &memory.control_point_memory.control_points;
    let used_count: usize = control_points
        .iter()
        .filter(|point: &&[u8; 2]| **point != [0, 0])
        .count();

    println!("== CONTROL POINTS ==");
    println!("Control points are tiles the bot wants to hold. Empty slots are stored as [0, 0] and are skipped here.");
    println!("On the map: each control point is an orange dot. Columns: slot, tile.");
    if used_count == 0 {
        println!(
            "No control points are set. The block is still a TODO, so this is expected for now."
        );
    }

    for (slot, point) in control_points.iter().enumerate() {
        if *point == [0, 0] {
            continue;
        }
        cambc::debug::draw_dot(to_vec2(*point), COLOR_CONTROL_POINT).unwrap();
        println!("  {:>2}: {:?}", slot, point);
    }
}

pub fn debug_draft(memory: &DraftMemory) {
    cambc::debug_block! {
        print_round_trip(memory);
        draw_camps(memory);
        draw_shortcuts(memory);
        draw_control_points(memory);
    }
}
