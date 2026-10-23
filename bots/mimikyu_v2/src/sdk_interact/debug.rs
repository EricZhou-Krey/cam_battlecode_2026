use crate::sdk_interact::map::{chebyshev_distance, Center};
use crate::{
    draft::{
        block::{
            camp_dm::CAMP_DRAFT_BYTE_SIZE, control_point_dm::CONTROL_POINT_DRAFT_SIZE,
            shortcut_map_dm::SHORTCUT_DRAFT_BYTE_SIZE,
        },
        draft_memory::{DraftMemory, DraftMemoryBlock, DRAFT_MEMORY_BYTE_SIZE},
    },
    memory::{champion_info::ChampionInfo, champion_memory::ChampionMemory, LocationID},
};
use cambc::{CampGroup, ChampionType, Team, Vec2};
use std::collections::HashMap;

const COLOR_SPAWN: u32 = 0x00FFFF;
const COLOR_SHORT_JUMP: u32 = 0x3399FF;
const COLOR_LONG_JUMP: u32 = 0xFF33CC;
const COLOR_SHORTCUT_FROM: u32 = 0xFFFF00;
const COLOR_SHORTCUT_TO: u32 = 0xFFFFFF;
const COLOR_CONTROL_POINT: u32 = 0xFF8800;
const COLOR_CAMP_GROUP: u32 = 0x66FF66;
const COLOR_ALTAR: u32 = 0xAA55FF;
const COLOR_ALLY_CHAMPION: u32 = 0x00FF99;
const COLOR_ENEMY_CHAMPION: u32 = 0xFF3333;
const LINES_TO_NEAREST_CAMPS: usize = 5;
const QUEUE_PREVIEW_LENGTH: usize = 5;

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

fn describe_location(memory: &ChampionMemory, location: Option<LocationID>) -> String {
    match location {
        Some(location_id) => {
            let point: Vec2 = memory.map.points_of_interest.locate(location_id);
            format!("{:?} at ({}, {})", location_id, point.x, point.y)
        }
        None => String::from("none"),
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
    println!("Control points are points used as meeting points or callouts. Empty slots are stored as [0, 0] and are skipped here.");
    println!("On the map: each control point is an orange dot. Columns: slot, tile.");
    if used_count == 0 {
        println!(
            "No control points are set. The block is still a TODO, so this is expected for now."
        );
    }
    let mut sorted_control_points: Vec<&[u8; 2]> = control_points.iter().collect();
    sorted_control_points.sort_unstable();

    let mut prev_point: Option<Vec2> = None;
    for (slot, point) in sorted_control_points.into_iter().enumerate() {
        if *point == [0, 0] {
            continue;
        }
        if let Some(prev) = prev_point {
            cambc::debug::draw_line(prev, to_vec2(*point), COLOR_CONTROL_POINT).unwrap();
        }
        cambc::debug::draw_dot(to_vec2(*point), COLOR_CONTROL_POINT).unwrap();
        prev_point = Some(to_vec2(*point));
        println!("  {:>2}: {:?}", slot, point);
    }
}

fn print_champion_overview(memory: &ChampionMemory) {
    let map = &memory.map;
    let points_of_interest = &map.points_of_interest;

    println!("== CHAMPION MEMORY ==");
    println!("Champion memory is rebuilt from the draft bytes when a champion starts. It holds a lazy map (camp group values, points of interest and distance fields that are only computed on demand), per champion info and the message board.");
    println!(
        "Camp groups: {}, altar tiles: {}, control points: {}, shortcuts: {}, cached closest point of interest lookups: {}",
        map.camp_group_count,
        points_of_interest.altar_locations.len(),
        points_of_interest.control_points.len(),
        points_of_interest.shortcuts.len(),
        points_of_interest.closest_point_of_interest.len(),
    );
}

fn print_distance_fields(memory: &ChampionMemory) {
    let distance_fields = &memory.map.distance_fields;
    let cached_camp_ids: Vec<usize> = distance_fields
        .camp_group_distance_fields
        .iter()
        .enumerate()
        .filter(|(_, field)| field.is_some())
        .map(|(camp_group_id, _)| camp_group_id)
        .collect();
    let queue_preview: Vec<usize> = distance_fields
        .camp_group_discovery_queue
        .iter()
        .take(QUEUE_PREVIEW_LENGTH)
        .map(|camp_group_id| camp_group_id.0)
        .collect();

    println!("== CHAMPION MAP: DISTANCE FIELDS ==");
    println!("Distance fields are computed lazily and cached. At most one new field is computed per round, and lazy_discover_from_queue works through the discovery queue nearest camp group first. A field that is not cached yet means no champion has asked for it and the queue has not reached it.");
    println!(
        "Camp group fields cached: {}/{} {:?}",
        cached_camp_ids.len(),
        distance_fields.camp_group_distance_fields.len(),
        cached_camp_ids,
    );
    println!(
        "Altar field: {}",
        if distance_fields.altar_distance_field.is_some() {
            "cached"
        } else {
            "not computed yet"
        }
    );
    println!(
        "Discovery queue: {} left, next {:?}",
        distance_fields.camp_group_discovery_queue.len(),
        queue_preview
    );
    println!(
        "Discovered this round: {}, blocked tiles in mask: {}",
        distance_fields.discovered_this_round,
        distance_fields.blocked_bitboard.count().unwrap(),
    );
}

fn print_camp_group_stats(memory: &ChampionMemory) {
    let map = &memory.map;

    println!("== CHAMPION MAP: CAMP GROUPS ==");
    println!("These are the per camp group values the champion reads at runtime, indexed by camp group id. Id 0 here is the same group as camp group 0 in the draft output.");
    println!("Columns: id, centre tile, respawn timer, effective hp, damage per round, gem value, gold value.");

    for camp_group_id in 0..map.camp_group_count {
        let center: Vec2 = map.points_of_interest.camp_group_centers[camp_group_id];
        println!(
            "  {:>2}: ({}, {}) respawn {} hp {} dmg {} gem {} gold {}",
            camp_group_id,
            center.x,
            center.y,
            map.camp_group_respawn_timer[camp_group_id],
            map.camp_group_effective_hp[camp_group_id],
            map.camp_group_damage_per_round[camp_group_id],
            map.camp_group_gem_value[camp_group_id],
            map.camp_group_gold_value[camp_group_id],
        );
    }
}

fn draw_points_of_interest(memory: &ChampionMemory) {
    let points_of_interest = &memory.map.points_of_interest;

    println!("== CHAMPION MAP: POINTS OF INTEREST ==");
    println!("Points of interest are how champions reason about locations: camp group centres, altars, control points and shortcut start tiles. try_get_closest_poi maps any tile to the nearest one by chebyshev distance and caches the answer.");
    println!("On the map: green dots are camp group centres, purple dots are altar tiles, orange dots are control points. Shortcuts are lines, blue for length 2 and magenta for length 3, with a yellow dot on the start tile and a white dot on the landing tile.");

    for center in &points_of_interest.camp_group_centers {
        cambc::debug::draw_dot(*center, COLOR_CAMP_GROUP).unwrap();
    }
    for altar in &points_of_interest.altar_locations {
        cambc::debug::draw_dot(*altar, COLOR_ALTAR).unwrap();
    }
    for control_point in &points_of_interest.control_points {
        cambc::debug::draw_dot(*control_point, COLOR_CONTROL_POINT).unwrap();
    }
    for (from, to) in &points_of_interest.shortcuts {
        let jump_length: i32 = chebyshev_distance(*from, *to);
        let line_color: u32 = if jump_length > 2 {
            COLOR_LONG_JUMP
        } else {
            COLOR_SHORT_JUMP
        };
        cambc::debug::draw_line(*from, *to, line_color).unwrap();
        cambc::debug::draw_dot(*from, COLOR_SHORTCUT_FROM).unwrap();
        cambc::debug::draw_dot(*to, COLOR_SHORTCUT_TO).unwrap();
    }

    println!(
        "Drawn: {} camp groups, {} altar tiles, {} control points, {} shortcuts",
        points_of_interest.camp_group_centers.len(),
        points_of_interest.altar_locations.len(),
        points_of_interest.control_points.len(),
        points_of_interest.shortcuts.len(),
    );
}

fn print_team_champion_info(
    memory: &ChampionMemory,
    label: &str,
    team_info: &HashMap<ChampionType, ChampionInfo>,
    color: u32,
) {
    let mut entries: Vec<(String, &ChampionInfo)> = team_info
        .iter()
        .map(|(champion_type, info)| (format!("{:?}", champion_type), info))
        .collect();
    entries.sort_by(|first, second| first.0.cmp(&second.0));

    println!("{label} champions: {}", entries.len());
    for (name, info) in entries {
        println!(
            "  {:<14} hp {:?} cooldowns {:?} stats (hp {}, strength {}, focus {}) gems {:?}",
            name,
            info.current_hp,
            info.current_cooldowns,
            info.current_stats[0],
            info.current_stats[1],
            info.current_stats[2],
            info.gems_held,
        );
        println!(
            "      last seen: {}, going to: {}",
            describe_location(memory, info.last_seen),
            describe_location(memory, info.going_to),
        );

        let last_seen_point: Option<Vec2> = info
            .last_seen
            .map(|location_id| memory.map.points_of_interest.locate(location_id));
        let going_to_point: Option<Vec2> = info
            .going_to
            .map(|location_id| memory.map.points_of_interest.locate(location_id));
        match (last_seen_point, going_to_point) {
            (Some(from), Some(to)) => {
                cambc::debug::draw_dot(from, color).unwrap();
                cambc::debug::draw_line(from, to, color).unwrap();
            }
            (Some(from), None) => cambc::debug::draw_dot(from, color).unwrap(),
            (None, Some(to)) => cambc::debug::draw_dot(to, color).unwrap(),
            (None, None) => (),
        }
    }
}

fn print_champion_info(memory: &ChampionMemory) {
    println!("== CHAMPION INFO ==");
    println!("Per champion data tracked by the bot, for both teams. Fields: current hp (none until seen), the two ability cooldowns, base stats (health, strength, focus), gems held (none until seen), the last point of interest it was seen at and the point of interest it is heading to.");
    println!("On the map: ally champions are green-teal and enemy champions are red. Each champion has a dot on its last seen location, with a line to where it is going when both are set.");

    print_team_champion_info(
        memory,
        "Ally",
        &memory.champion_info.ally,
        COLOR_ALLY_CHAMPION,
    );
    print_team_champion_info(
        memory,
        "Enemy",
        &memory.champion_info.enemy,
        COLOR_ENEMY_CHAMPION,
    );
}

fn print_message_board(memory: &ChampionMemory) {
    let message_board = &memory.message_board;

    println!("== MESSAGE BOARD ==");
    println!("The 8 shared bytes are a heap of messages. A free byte is all ones. Each champion frees its own messages the next time it syncs, collects everyone else's into its inbox, and queues messages that do not fit until space frees up.");
    println!(
        "Owned: {:?}, pending: {}, inbox: {}, next fit start: {}",
        message_board.owned_messages,
        message_board.pending_messages.len(),
        message_board.inbox.len(),
        message_board.next_fit_start,
    );
    for message in &message_board.inbox {
        println!("  inbox: {:?}", message);
    }
    for message in &message_board.pending_messages {
        println!("  pending: {:?}", message);
    }
}

pub fn debug_draft_init(memory: &DraftMemory) {
    cambc::debug_block! {
        print_round_trip(memory);
        draw_camps(memory);
        draw_shortcuts(memory);
        draw_control_points(memory);
    }
}

pub fn post_fuel(label: &str) {
    cambc::debug_block! {
        println!("{label}: {:?}", cambc::fuel_remaining());
    }
}

pub fn debug_champion_init(memory: &ChampionMemory) {
    cambc::debug_block! {
        print_champion_overview(memory);
        print_distance_fields(memory);
        print_camp_group_stats(memory);
        print_champion_info(memory);
        print_message_board(memory);
        draw_points_of_interest(memory);
    }
}
