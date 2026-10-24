use crate::action::{Action, ActionGroupType};
use crate::memory::message::messages::{Message, MessageType};
use crate::sdk_interact::map::{chebyshev_distance, Center};
use crate::{
    draft::{
        block::{
            camp_dm::CAMP_DRAFT_BYTE_SIZE, control_point_dm::CONTROL_POINT_DRAFT_SIZE,
            shortcut_map_dm::SHORTCUT_DRAFT_BYTE_SIZE,
        },
        draft_memory::{DraftMemory, DraftMemoryBlock, DRAFT_MEMORY_BYTE_SIZE},
    },
    memory::{
        champion_info::ChampionInfo, champion_memory::ChampionMemory, AltarID, CampGroupID,
        LocationID,
    },
};
use cambc::{AbilityInfo, CampGroup, ChampionType, DistanceField, Team, Vec2};
use std::collections::HashMap;

const GREY: u32 = 0x808080;
const CYAN: u32 = 0x00FFFF;
const BLUE: u32 = 0x3399FF;
const PINK: u32 = 0xFF33CC;
const YELLOW: u32 = 0xFFFF00;
const WHITE: u32 = 0xFFFFFF;
const ORANGE: u32 = 0xFF8800;
const LIGHT_GREEN: u32 = 0x66FF66;
const PURPLE: u32 = 0xAA55FF;
const MINT_GREEN: u32 = 0x00FF99;
const RED: u32 = 0xFF3333;
const LIGHT_RED: u32 = 0xFF6666;

const COLOR_SPAWN: u32 = CYAN;
const COLOR_SHORT_JUMP: u32 = BLUE;
const COLOR_LONG_JUMP: u32 = PINK;
const COLOR_SHORTCUT_FROM: u32 = YELLOW;
const COLOR_SHORTCUT_TO: u32 = WHITE;
const COLOR_CONTROL_POINT: u32 = ORANGE;
const COLOR_CAMP_GROUP: u32 = LIGHT_GREEN;
const COLOR_ALTAR: u32 = PURPLE;
const COLOR_ALLY_CHAMPION: u32 = MINT_GREEN;
const COLOR_ENEMY_CHAMPION: u32 = RED;
const COLOR_ACTION_TARGET: u32 = WHITE;
const COLOR_ACTION_SELF: u32 = MINT_GREEN;
const COLOR_DASH_LANDING: u32 = YELLOW;
const PATH_PREVIEW_STEPS: u32 = 12;
const SHORTCUT_LOOKAHEAD: i32 = 8;
const SHORTCUT_PRINT_LIMIT: usize = 6;

const MIN_CIRCLE_RADIUS: f32 = 1.0;
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

pub fn draw_circle(center: Vec2, radius: f32, rgb: u32) {
    cambc::debug_block! {
        let radius: f32 = radius.max(MIN_CIRCLE_RADIUS);
        let min: Vec2 = Vec2 {
            x: center.x - radius.round() as i32,
            y: center.y - radius.round() as i32,
        };
        let max: Vec2 = Vec2 {
            x: center.x + radius.round() as i32,
            y: center.y + radius.round() as i32,
        };

        let top_left = Vec2 { x: min.x, y: min.y };
        let top_right = Vec2 { x: max.x, y: min.y };
        let bottom_right = Vec2 { x: max.x, y: max.y };
        let bottom_left = Vec2 { x: min.x, y: max.y };

        let _ = cambc::debug::draw_line(top_left, top_right, rgb);
        let _ = cambc::debug::draw_line(top_right, bottom_right, rgb);
        let _ = cambc::debug::draw_line(bottom_right, bottom_left, rgb);
        let _ = cambc::debug::draw_line(bottom_left, top_left, rgb);
    }
}

pub fn draw_cross(center: Vec2, arm: i32, rgb: u32) {
    cambc::debug_block! {
        let _ = cambc::debug::draw_line(
            Vec2 {
                x: center.x - arm,
                y: center.y - arm,
            },
            Vec2 {
                x: center.x + arm,
                y: center.y + arm,
            },
            rgb,
        );

        let _ = cambc::debug::draw_line(
            Vec2 {
                x: center.x - arm,
                y: center.y + arm,
            },
            Vec2 {
                x: center.x + arm,
                y: center.y - arm,
            },
            rgb,
        );
    }
}

pub fn print_round_trip(memory: &DraftMemory) {
    cambc::debug_block! {
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
}

pub fn draw_camps(memory: &DraftMemory) {
    cambc::debug_block! {
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
}

pub fn draw_shortcuts(memory: &DraftMemory) {
    cambc::debug_block! {
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
}

pub fn draw_control_points(memory: &DraftMemory) {
    cambc::debug_block! {
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
}

pub fn print_champion_overview(memory: &ChampionMemory) {
    cambc::debug_block! {
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
}

pub fn print_distance_fields(memory: &ChampionMemory) {
    cambc::debug_block! {
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
}

pub fn print_camp_group_stats(memory: &ChampionMemory) {
    cambc::debug_block! {
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
}

pub fn draw_points_of_interest(memory: &ChampionMemory) {
    cambc::debug_block! {
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
}

pub fn print_team_champion_info(
    memory: &ChampionMemory,
    label: &str,
    team_info: &HashMap<ChampionType, ChampionInfo>,
    color: u32,
) {
    cambc::debug_block! {
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
}

pub fn print_champion_info(memory: &ChampionMemory) {
    cambc::debug_block! {
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
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{:02X}", byte))
        .collect::<Vec<String>>()
        .join(" ")
}

fn safe_locate(memory: &ChampionMemory, location_id: LocationID) -> Option<Vec2> {
    let points_of_interest = &memory.map.points_of_interest;
    match location_id {
        LocationID::CampGroup(id) => points_of_interest.camp_group_centers.get(id.0).copied(),
        LocationID::Altar(id) => points_of_interest.altar_locations.get(id.0).copied(),
        LocationID::ControlPoint(id) => points_of_interest.control_points.get(id.0).copied(),
        LocationID::Shortcut(id) => points_of_interest.shortcut_points.get(id.0).copied(),
    }
}

fn message_location(memory: &ChampionMemory, message: &Message) -> Option<Vec2> {
    let points_of_interest = &memory.map.points_of_interest;
    match message {
        Message::FarmClear { camp_group_id }
        | Message::FarmHelp { camp_group_id, .. }
        | Message::FarmCollection { camp_group_id, .. }
        | Message::FarmGoingToCamp { camp_group_id, .. } => points_of_interest
            .camp_group_centers
            .get(camp_group_id.0)
            .copied(),
        Message::CombatHeal { location_id, .. }
        | Message::CombatLosingFightNeedHelp { location_id, .. }
        | Message::CombatWinningFightWantHelp { location_id, .. }
        | Message::StatusAllyLocation { location_id, .. }
        | Message::StatusEnemyLocation { location_id, .. }
        | Message::GroupTogether { location_id } => safe_locate(memory, *location_id),
        Message::EndgameAltarProtect { altar_id } => {
            points_of_interest.altar_locations.get(altar_id.0).copied()
        }
        _ => None,
    }
}

fn message_responders(message: &Message) -> Vec<ChampionType> {
    match message {
        Message::FarmHelp {
            ally_champions_helping,
            ..
        } => ally_champions_helping.iter().flatten().copied().collect(),
        Message::FarmCollection {
            ally_champion_claiming,
            ..
        } => ally_champion_claiming.iter().copied().collect(),
        Message::CombatLosingFightNeedHelp { ally_champion, .. }
        | Message::CombatWinningFightWantHelp { ally_champion, .. } => {
            ally_champion.iter().copied().collect()
        }
        _ => Vec::new(),
    }
}

fn describe_responders(message: &Message) -> String {
    if message.request_key().is_none() {
        return String::new();
    }
    let responders: Vec<ChampionType> = message_responders(message);
    if responders.is_empty() {
        String::from(" | no responders yet")
    } else {
        format!(" | responders {:?}", responders)
    }
}

fn draw_message(memory: &ChampionMemory, message: &Message) {
    cambc::debug_block! {
        let location: Option<Vec2> = message_location(memory, message);

        if let Some(center) = location {
            match message {
                Message::FarmClear { .. } => draw_cross(center, 2, GREY),
                Message::FarmHelp { signal_strength, .. } => {
                    draw_circle(center, *signal_strength as f32, LIGHT_GREEN)
                }
                Message::CombatHeal { signal_strength, .. } => {
                    draw_circle(center, *signal_strength as f32, LIGHT_GREEN)
                }
                Message::CombatLosingFightNeedHelp { signal_strength, .. } => {
                    draw_circle(center, *signal_strength as f32, RED)
                }
                Message::CombatWinningFightWantHelp { signal_strength, .. } => {
                    draw_circle(center, *signal_strength as f32, LIGHT_RED)
                }
                Message::StatusAllyLocation { .. } => draw_circle(center, 2.0, LIGHT_GREEN),
                Message::StatusEnemyLocation { .. } => draw_circle(center, 2.0, LIGHT_RED),
                Message::GroupTogether { .. } => draw_circle(center, 15.0, PURPLE),
                Message::EndgameAltarProtect { .. } => draw_circle(center, 15.0, PURPLE),
                _ => (),
            }

            let line_color: u32 = match message {
                Message::FarmHelp { .. } => LIGHT_GREEN,
                Message::FarmCollection { .. } => ORANGE,
                Message::CombatLosingFightNeedHelp { .. } => RED,
                Message::CombatWinningFightWantHelp { .. } => LIGHT_RED,
                _ => WHITE,
            };
            for champion in message_responders(message) {
                let responder_location: Option<Vec2> = memory
                    .champion_info
                    .ally
                    .get(&champion)
                    .and_then(|info| info.last_seen)
                    .and_then(|location_id| safe_locate(memory, location_id));
                if let Some(point) = responder_location {
                    let _ = cambc::debug::draw_line(point, center, line_color);
                    let _ = cambc::debug::draw_dot(point, COLOR_ALLY_CHAMPION);
                }
            }
        }
    }
}

pub fn print_board_layout(memory: &ChampionMemory) {
    cambc::debug_block! {
        let mut shared: [u8; 8] = [0; 8];
        match cambc::shared_memory().read(0, &mut shared) {
            Err(_) => println!("  board bytes: unreadable"),
            Ok(_) => {
                println!("  raw bytes: {}", hex_bytes(&shared));

                let mut free_run_start: Option<usize> = None;
                let mut position: usize = 0;
                while position < shared.len() {
                    let byte: u8 = shared[position];
                    if byte >> 2 == 0b11_1111 {
                        if free_run_start.is_none() {
                            free_run_start = Some(position);
                        }
                        position += 1;
                        continue;
                    }

                    if let Some(start) = free_run_start.take() {
                        println!("  [{}..{}] free ({} bytes)", start, position, position - start);
                    }

                    let message_type: MessageType = MessageType::from_nibble(byte >> 4);
                    let length: usize = message_type.byte_length().min(shared.len() - position);
                    let ours: bool = memory
                        .message_board
                        .owned_messages
                        .iter()
                        .any(|owned| owned.span.offset == position);
                    println!(
                        "  [{}..{}] {:?} ({} bytes) {}{}",
                        position,
                        position + length,
                        message_type,
                        length,
                        hex_bytes(&shared[position..position + length]),
                        if ours { " (ours)" } else { "" },
                    );
                    position += length;
                }
                if let Some(start) = free_run_start.take() {
                    println!("  [{}..{}] free ({} bytes)", start, position, position - start);
                }
            }
        }
    }
}

pub fn print_message_board(memory: &ChampionMemory) {
    cambc::debug_block! {
        let message_board = &memory.message_board;

        println!("== MESSAGE BOARD ==");
        println!("The 8 shared bytes are a heap of messages, and a byte with its top six bits set is free. Each champion frees its own messages at the start of its next sync, after reading any replies written into them, and collects every other champion's messages into its inbox. Messages that do not fit wait in the pending queue until space frees up.");
        println!("Farm help, farm collection and the two combat help messages can be answered in place. respond_to queues the champion, and the next sync or flush writes it into the first empty responder slot. The sender then sees the update as a reply.");
        println!("On the map: a grey cross is a farm clear, circles show the signal strength of a message (light green for farm help, heals and ally locations, red for losing a fight, light red for winning a fight and enemy locations, purple for group and altar protection). Each responding champion with a known location gets a mint dot and a line to the message.");
        println!("Sections: board layout (raw bytes and where each message sits), owned (messages we placed, as sent), pending (waiting for space), pending responses (replies queued for the next sync or flush), inbox (new or changed messages from other champions), replies (our own messages that a responder updated).");
        println!(
            "Owned: {}, pending: {}, pending responses: {}, inbox: {}, replies: {}, seen: {}, next fit start: {}",
            message_board.owned_messages.len(),
            message_board.pending_messages.len(),
            message_board.pending_responses.len(),
            message_board.inbox.len(),
            message_board.responses.len(),
            message_board.seen_messages.len(),
            message_board.next_fit_start,
        );

        println!("Board layout:");
        print_board_layout(memory);

        println!("Owned messages:");
        for owned in &message_board.owned_messages {
            let message: Message = Message::from_bytes(&owned.bytes[..owned.span.length]);
            println!(
                "  [{}..{}] {:?}{}",
                owned.span.offset,
                owned.span.offset + owned.span.length,
                message,
                describe_responders(&message),
            );
        }

        println!("Pending messages:");
        for message in &message_board.pending_messages {
            println!("  {:?}{}", message, describe_responders(message));
        }

        println!("Pending responses:");
        for response in &message_board.pending_responses {
            println!(
                "  [{}..{}] {:?} responder {:?}",
                response.span.offset,
                response.span.offset + response.span.length,
                response.request_key.0,
                response.responder,
            );
        }

        println!("Inbox:");
        for received in &message_board.inbox {
            println!(
                "  [{}..{}] {:?}{}",
                received.span.offset,
                received.span.offset + received.span.length,
                received.message,
                describe_responders(&received.message),
            );
        }

        println!("Replies to our messages:");
        for reply in &message_board.responses {
            println!("  {:?}{}", reply, describe_responders(reply));
        }
    }
}

pub fn draw_sent_messages(memory: &ChampionMemory) {
    cambc::debug_block! {
        for received in &memory.message_board.inbox {
            draw_message(memory, &received.message);
        }
        for owned in &memory.message_board.owned_messages {
            draw_message(
                memory,
                &Message::from_bytes(&owned.bytes[..owned.span.length]),
            );
        }
        for reply in &memory.message_board.responses {
            draw_message(memory, reply);
        }
    }
}

fn action_group_color(action_group_type: ActionGroupType) -> u32 {
    match action_group_type {
        ActionGroupType::Intermediate => GREY,
        ActionGroupType::Farm => LIGHT_GREEN,
        ActionGroupType::Support => BLUE,
        ActionGroupType::ChampionCombat => RED,
        ActionGroupType::Survival => ORANGE,
    }
}

fn describe_distance(distance: Option<u16>) -> String {
    match distance {
        Some(u16::MAX) => String::from("unreached"),
        Some(value) => value.to_string(),
        None => String::from("unknown"),
    }
}

fn debug_action_header(name: &str, action: &Action) {
    println!(
        "== CURRENT ACTION: {name} (precedence {}) ==",
        action.precedence()
    );
}

fn resolve_move_field<'a>(
    distance_field: &'a Option<DistanceField>,
    target_id: LocationID,
    memory: &'a ChampionMemory,
) -> (Option<&'a DistanceField>, &'static str) {
    if let Some(field) = distance_field {
        return (Some(field), "owned by the action");
    }
    if let LocationID::CampGroup(camp_group_id) = target_id {
        if let Some(Some(field)) = memory
            .map
            .distance_fields
            .camp_group_distance_fields
            .get(camp_group_id.0)
        {
            return (Some(field), "shared lazy cache");
        }
    }
    (None, "not computed yet")
}

fn debug_move_to_location(
    action: &Action,
    distance_field: &Option<DistanceField>,
    location_id: Option<LocationID>,
    accepted_distance: u16,
    action_group_type: ActionGroupType,
    memory: &ChampionMemory,
) {
    cambc::debug_block! {
        let group_color: u32 = action_group_color(action_group_type);

        debug_action_header("MOVE TO LOCATION", action);
        println!("The champion walks to a location by always stepping to the neighbouring tile with the lowest distance field value. The field is rooted at the target, so each value is the number of steps still to walk, and the action ends when the value at the champion's tile is at most the accepted distance.");
        println!("On the map: the champion is a mint dot, the target is a white dot with a ring of the accepted distance, and the line from the champion follows the field downhill for {PATH_PREVIEW_STEPS} steps, coloured by action group (grey intermediate, light green farm, blue support, red champion combat, orange survival). A yellow dot and line mark the best legal dash landing, and blue or pink lines mark shortcuts within {SHORTCUT_LOOKAHEAD} tiles.");
        println!("Group: {:?}, accepted distance: {}", action_group_type, accepted_distance);

        match location_id {
            None => println!("No target location is set, so the action finishes as soon as it runs."),
            Some(target_id) => {
                let target: Option<Vec2> = safe_locate(memory, target_id);
                let champion = cambc::self_champion().ok().flatten();
                let position: Option<Vec2> = champion.and_then(|me| me.position().ok());
                let (field, field_source): (Option<&DistanceField>, &str) =
                    resolve_move_field(distance_field, target_id, memory);

                println!(
                    "Target: {:?} at {:?}",
                    target_id,
                    target.map(|point| (point.x, point.y))
                );
                println!("Distance field: {field_source}");

                if let Some(target_point) = target {
                    let _ = cambc::debug::draw_dot(target_point, COLOR_ACTION_TARGET);
                    draw_circle(target_point, accepted_distance as f32, group_color);
                }
                if let Some(point) = position {
                    let _ = cambc::debug::draw_dot(point, COLOR_ACTION_SELF);
                }

                match (field, position) {
                    (Some(field), Some(point)) => {
                        let current_distance: Option<u16> = field.get(point).ok();
                        println!(
                            "Position: ({}, {}), distance to target: {}",
                            point.x,
                            point.y,
                            describe_distance(current_distance)
                        );
                        match current_distance {
                            Some(u16::MAX) => println!("Status: the target cannot be walked to from here, so only a dash or teleport can help."),
                            Some(distance) if distance <= accepted_distance => println!("Status: arrived, the action finishes on its next turn."),
                            Some(_) => println!("Status: walking."),
                            None => println!("Status: the position is outside the field."),
                        }

                        let mut path_tile: Vec2 = point;
                        let mut path_steps: u32 = 0;

                        while path_steps < PATH_PREVIEW_STEPS {
                            match field.next_move_from(path_tile) {
                                Ok(Some(next_tile)) => {
                                    let from: Vec2 = position.unwrap() + path_tile;
                                    let to: Vec2 = position.unwrap() + next_tile;

                                    let _ = cambc::debug::draw_line(from, to, group_color);

                                    path_tile = next_tile;
                                    path_steps += 1;
                                }
                                _ => break,
                            }
                        }

                        println!(
                            "Path preview: {} steps, ends at ({}, {}) with distance {}",
                            path_steps,
                            path_tile.x + position.unwrap().x,
                            path_tile.y + position.unwrap().y,
                            describe_distance(field.get(path_tile).ok())
                        );

                        if let Some(me) = champion {
                            let crystal_count: u32 = me
                                .consumables()
                                .iter()
                                .filter(|consumable| {
                                    matches!(consumable.kind(), cambc::ConsumableType::TpCrystal)
                                })
                                .map(|consumable| consumable.count().unwrap_or(0))
                                .sum();
                            println!("Teleport crystals held: {crystal_count}");

                            for ability in me.abilities() {
                                if !ability.is_dash() {
                                    continue;
                                }
                                let dash_range: Option<f32> = match ability.info() {
                                    Ok(Some(AbilityInfo::Dash(dash))) => Some(dash.range),
                                    _ => None,
                                };
                                println!(
                                    "Dash ability slot {}: ready {:?}, cooldown {:?}, range {:?}",
                                    ability.slot(),
                                    ability.is_ready(),
                                    ability.cooldown(),
                                    dash_range
                                );

                                let mut best_landing: Option<(u16, Vec2)> = None;
                                if let Ok(landing_tiles) = ability.valid_tiles() {
                                    if let Ok(landing_positions) = landing_tiles.positions() {
                                        for landing in landing_positions {
                                            let Ok(landing_distance) = field.get(landing) else {
                                                continue;
                                            };
                                            if landing_distance == u16::MAX {
                                                continue;
                                            }
                                            if best_landing.is_none_or(|(best_distance, _)| {
                                                landing_distance < best_distance
                                            }) {
                                                best_landing = Some((landing_distance, landing));
                                            }
                                        }
                                    }
                                }
                                match best_landing {
                                    Some((landing_distance, landing)) => {
                                        let gain: u16 = current_distance
                                            .unwrap_or(u16::MAX)
                                            .saturating_sub(landing_distance);
                                        println!(
                                            "  best landing ({}, {}) at distance {}, gain {}",
                                            landing.x, landing.y, landing_distance, gain
                                        );
                                        let _ = cambc::debug::draw_line(point, landing, COLOR_DASH_LANDING);
                                        let _ = cambc::debug::draw_dot(landing, COLOR_DASH_LANDING);
                                    }
                                    None => println!("  no legal landing tile with a known distance"),
                                }
                            }
                        }

                        let mut nearby_shortcuts: Vec<(u16, i32, Vec2, Vec2, u16)> = Vec::new();
                        for (start, landing) in &memory.map.points_of_interest.shortcuts {
                            let approach: i32 =
                                (start.x - point.x).abs().max((start.y - point.y).abs());
                            if approach > SHORTCUT_LOOKAHEAD {
                                continue;
                            }
                            let Ok(landing_distance) = field.get(*landing) else {
                                continue;
                            };
                            if landing_distance == u16::MAX {
                                continue;
                            }
                            let saving: u16 = current_distance.unwrap_or(u16::MAX).saturating_sub(
                                (approach as u16)
                                    .saturating_add(1)
                                    .saturating_add(landing_distance),
                            );
                            nearby_shortcuts.push((saving, approach, *start, *landing, landing_distance));
                        }
                        nearby_shortcuts.sort_unstable_by_key(|entry| std::cmp::Reverse(entry.0));

                        println!(
                            "Shortcuts within {} tiles: {} (estimated saving counts the approach as a straight walk and a jump cost of 1)",
                            SHORTCUT_LOOKAHEAD,
                            nearby_shortcuts.len()
                        );
                        for (rank, (saving, approach, start, landing, landing_distance)) in
                            nearby_shortcuts.iter().enumerate()
                        {
                            let jump_length: i32 = chebyshev_distance(*start, *landing);
                            let line_color: u32 = if jump_length > 2 {
                                COLOR_LONG_JUMP
                            } else {
                                COLOR_SHORT_JUMP
                            };
                            let _ = cambc::debug::draw_line(*start, *landing, line_color);
                            let _ = cambc::debug::draw_dot(*start, COLOR_SHORTCUT_FROM);
                            let _ = cambc::debug::draw_dot(*landing, COLOR_SHORTCUT_TO);
                            if rank < SHORTCUT_PRINT_LIMIT {
                                println!(
                                    "  ({}, {}) -> ({}, {}) length {}, approach {}, landing distance {}, saving {}",
                                    start.x, start.y, landing.x, landing.y,
                                    jump_length, approach, landing_distance, saving
                                );
                            }
                        }
                    }
                    (None, _) => println!("No distance field is available yet, so the champion waits this turn."),
                    (_, None) => println!("The champion's position is not available."),
                }
            }
        }
    }
}

fn debug_idle(action: &Action) {
    cambc::debug_block! {
        debug_action_header("IDLE", action);
        println!("The champion has no task. Intermediate actions have the lowest precedence, so any message or decision can preempt them.");
        // TODO: print how many turns the champion has been idle and what it is waiting for
    }
}

fn debug_collecting_from_camp(action: &Action, _memory: &ChampionMemory) {
    cambc::debug_block! {
        debug_action_header("COLLECTING FROM CAMP", action);
        println!("Farm group action: the champion collects what a cleared camp group dropped.");
        println!("Not implemented yet.");
        // TODO: print the camp group, gems and gold collected so far, and the respawn timer
        // TODO: draw a ring around the camp group centre and a line from the champion
    }
}

fn debug_in_champion_combat_kill(action: &Action, _memory: &ChampionMemory) {
    cambc::debug_block! {
        debug_action_header("IN CHAMPION COMBAT (KILL)", action);
        println!("Champion combat action: the champion is committing to killing an enemy champion.");
        println!("Not implemented yet.");
        // TODO: print the enemy target, its last known health and the damage race
        // TODO: draw a line from the champion to the target and the attack range
    }
}

fn debug_in_champion_combat_stall(action: &Action, _memory: &ChampionMemory) {
    cambc::debug_block! {
        debug_action_header("IN CHAMPION COMBAT (STALL)", action);
        println!("Champion combat action: the champion is holding an enemy champion in place without committing to a kill.");
        println!("Not implemented yet.");
        // TODO: print what is being stalled and until when
    }
}

fn debug_in_champion_combat_escape(action: &Action, _memory: &ChampionMemory) {
    cambc::debug_block! {
        debug_action_header("IN CHAMPION COMBAT (ESCAPE)", action);
        println!("Survival action: the champion is disengaging from an enemy champion.");
        println!("Not implemented yet.");
        // TODO: print the enemy being escaped and the chosen retreat point
    }
}

fn debug_in_camp_group_combat(
    action: &Action,
    camp_group_id: CampGroupID,
    memory: &ChampionMemory,
) {
    cambc::debug_block! {
        debug_action_header("IN CAMP GROUP COMBAT", action);
        println!("Farm group action: the champion is fighting the camps of one camp group.");
        println!("Camp group: {:?} at {:?}", camp_group_id, memory.map.points_of_interest.camp_group_centers.get(camp_group_id.0).map(|point| (point.x, point.y)));
        println!("Not implemented yet.");
        // TODO: print the camp group hp, damage per round and the expected fight outcome
        // TODO: draw a ring around the camp group centre
    }
}

fn debug_tributing_at_altar(action: &Action, altar_id: AltarID, memory: &ChampionMemory) {
    cambc::debug_block! {
        debug_action_header("TRIBUTING AT ALTAR", action);
        println!("Farm group action: the champion is submitting gems at an altar.");
        println!("Altar: {:?} at {:?}", altar_id, memory.map.points_of_interest.altar_locations.get(altar_id.0).map(|point| (point.x, point.y)));
        println!("Not implemented yet.");
        // TODO: print gems held and the remaining channel time
        // TODO: draw a ring around the altar
    }
}

fn debug_defend_location(action: &Action, location_id: LocationID, memory: &ChampionMemory) {
    cambc::debug_block! {
        debug_action_header("DEFEND LOCATION", action);
        println!("Support group action: the champion is guarding a location.");
        println!("Location: {:?} at {:?}", location_id, safe_locate(memory, location_id).map(|point| (point.x, point.y)));
        println!("Not implemented yet.");
        // TODO: print the threats near the location and draw a ring around it
    }
}

fn debug_escape_to_spawn(action: &Action, _memory: &ChampionMemory) {
    cambc::debug_block! {
        debug_action_header("ESCAPE TO SPAWN", action);
        println!("Survival action: the champion is retreating to the spawn region.");
        println!("Not implemented yet.");
        // TODO: print the recall state and the distance to the spawn region
    }
}

pub fn print_action_queue(memory: &ChampionMemory) {
    cambc::debug_block! {
        println!("== ACTION QUEUE ==");
        println!("Queued actions are processed in order. Each action's precedence indicates its action group.");
        println!("Queue length: {}", memory.action_state.action_queue.len());
        println!("Last bastion: {:?}", memory.action_state.last_bastion);

        if memory.action_state.action_queue.is_empty() {
            println!("  (empty)");
        }

        for (index, action) in memory.action_state.action_queue.iter().enumerate() {
            println!(
                "  [{:>2}] precedence {:>2} | {:?}",
                index,
                action.precedence(),
                action
            );
        }
    }
}

pub fn post_fuel(label: &str) {
    cambc::debug_block! {
        println!("{label}: {:?}", cambc::fuel_remaining());
    }
}

pub fn debug_draft_init(memory: &DraftMemory) {
    print_round_trip(memory);
    draw_camps(memory);
    draw_shortcuts(memory);
    draw_control_points(memory);
}

pub fn debug_champion_turn(memory: &ChampionMemory) {
    print_camp_group_stats(memory);
    print_champion_overview(memory);
    print_distance_fields(memory);
    print_champion_info(memory);
    print_message_board(memory);
    print_action_queue(memory);

    draw_sent_messages(memory);
    draw_points_of_interest(memory);
}

pub fn debug_current_action(action: &Action, memory: &ChampionMemory) {
    match action {
        Action::Idle => debug_idle(action),
        Action::MoveToLocation {
            distance_field,
            location_id,
            accepted_distance,
            action_group_type,
        } => debug_move_to_location(
            action,
            distance_field,
            *location_id,
            *accepted_distance,
            *action_group_type,
            memory,
        ),
        Action::CollectingFromCamp { .. } => debug_collecting_from_camp(action, memory),
        Action::InChampionComabtKill { .. } => debug_in_champion_combat_kill(action, memory),
        Action::InChampionCombatStall { .. } => debug_in_champion_combat_stall(action, memory),
        Action::InChampionCombatEscape { .. } => debug_in_champion_combat_escape(action, memory),
        Action::InCampGroupCombat { camp_group_id } => {
            debug_in_camp_group_combat(action, *camp_group_id, memory)
        }
        Action::TributingAtAltar { altar_id } => {
            debug_tributing_at_altar(action, *altar_id, memory)
        }
        Action::DefendLocation { location_id } => {
            debug_defend_location(action, *location_id, memory)
        }
        Action::EscapeToSpawn => debug_escape_to_spawn(action, memory),
    }
}
