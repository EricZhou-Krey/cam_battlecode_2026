use crate::action::route::{CrossKind, Leg, Route};
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
    memory::{champion_info::ChampionInfo, champion_memory::ChampionMemory, LocationID},
};
use cambc::{CampGroup, ChampionType, DistanceField, Team, Vec2};
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

fn describe_location(location: Option<Vec2>) -> String {
    match location {
        Some(point) => format!("({}, {})", point.x, point.y),
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

fn cached_field_ids(fields: &[Option<DistanceField>]) -> Vec<usize> {
    fields
        .iter()
        .enumerate()
        .filter(|(_, field)| field.is_some())
        .map(|(id, _)| id)
        .collect()
}

pub fn print_distance_fields(memory: &ChampionMemory) {
    cambc::debug_block! {
        let distance_fields = &memory.map.distance_fields;

        let queue_preview: Vec<&LocationID> = distance_fields
            .discovery_queue
            .iter()
            .take(QUEUE_PREVIEW_LENGTH)
            .collect();

        println!("== CHAMPION MAP: DISTANCE FIELDS ==");
        println!("Distance fields are computed lazily and cached, one per kind of location: camp groups, altars, shortcut points and control points. At most one new field is computed per round, either on demand by try_get or by lazy_discover, which works through the shared discovery queue in order (camp groups first, then altars, shortcuts, control points). A field that is not cached yet means nothing has asked for it and the queue has not reached it.");

        let kinds: [(&str, &Vec<Option<DistanceField>>); 4] = [
            ("Camp group", &distance_fields.camp_group_distance_fields),
            ("Altar", &distance_fields.altar_distance_fields),
            ("Shortcut point", &distance_fields.shortcut_point_distance_fields),
            ("Control point", &distance_fields.control_point_distance_fields),
        ];
        for (name, fields) in kinds {
            let cached = cached_field_ids(fields);
            println!(
                "{} fields cached: {}/{} {:?}",
                name,
                cached.len(),
                fields.len(),
                cached,
            );
        }

        println!(
            "Discovery queue: {} left, next {:?}",
            distance_fields.discovery_queue.len(),
            queue_preview
        );
        println!(
            "Discovered this round: {}, blocked tiles in mask (walls, holes, altars): {}",
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
                "  {:>2}: ({}, {}) respawn {} hp {} dmg {} gem {} gold {} attack_order {:?}",
                camp_group_id,
                center.x,
                center.y,
                map.camp_group_respawn_timer[camp_group_id],
                map.camp_group_effective_hp[camp_group_id],
                map.camp_group_damage_per_round[camp_group_id],
                map.camp_group_gem_value[camp_group_id],
                map.camp_group_gold_value[camp_group_id],
                map.camp_group_attack_order[camp_group_id],
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
            let jump_length: i32 = chebyshev_distance(*from, *to).into();
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
    _memory: &ChampionMemory,
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
                "      last seen: {}",
                describe_location(info.last_seen),
            );

            if let Some(from) = info.last_seen {
                cambc::debug::draw_dot(from, color).unwrap();
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

    draw_sent_messages(memory);
    draw_points_of_interest(memory);
}

fn message_location_id(message: &Message) -> Option<LocationID> {
    match message {
        Message::FarmClear { camp_group_id }
        | Message::FarmHelp { camp_group_id, .. }
        | Message::FarmCollection { camp_group_id, .. }
        | Message::FarmGoingToCamp { camp_group_id, .. } => {
            Some(LocationID::CampGroup(*camp_group_id))
        }
        Message::CombatHeal { location_id, .. }
        | Message::CombatLosingFightNeedHelp { location_id, .. }
        | Message::CombatWinningFightWantHelp { location_id, .. }
        | Message::CombatTargetEnemy { location_id, .. }
        | Message::StatusAllyLocation { location_id, .. }
        | Message::StatusEnemyLocation { location_id, .. }
        | Message::GroupTogether { location_id } => Some(*location_id),
        _ => None,
    }
}

fn message_signal_strength(message: &Message) -> Option<u16> {
    match message {
        Message::FarmHelp {
            signal_strength, ..
        }
        | Message::CombatHeal {
            signal_strength, ..
        }
        | Message::CombatLosingFightNeedHelp {
            signal_strength, ..
        }
        | Message::CombatWinningFightWantHelp {
            signal_strength, ..
        }
        | Message::CombatTargetEnemy {
            signal_strength, ..
        } => Some(*signal_strength),
        _ => None,
    }
}

fn message_color(message: &Message) -> u32 {
    match message {
        Message::FarmClear { .. } => GREY,
        Message::FarmHelp { .. } => LIGHT_GREEN,
        Message::FarmCollection { .. } => ORANGE,
        Message::CombatHeal { .. } => YELLOW,
        Message::CombatLosingFightNeedHelp { .. } => RED,
        Message::CombatWinningFightWantHelp { .. } => LIGHT_RED,
        Message::CombatTargetEnemy { .. } => PINK,
        Message::GroupTogether { .. } => PURPLE,
        Message::StatusAllyLocation { .. } => LIGHT_GREEN,
        Message::StatusEnemyLocation { .. } => LIGHT_RED,
        _ => WHITE,
    }
}

fn action_group_color(action_group_type: ActionGroupType) -> u32 {
    match action_group_type {
        ActionGroupType::StartFarm => LIGHT_GREEN,
        ActionGroupType::EndFarm => ORANGE,
        ActionGroupType::Support => RED,
        ActionGroupType::ChampionCombat => LIGHT_RED,
        ActionGroupType::Survival => YELLOW,
        ActionGroupType::Intermediate => WHITE,
    }
}

fn draw_ping_area(
    memory: &ChampionMemory,
    location_id: LocationID,
    center: Vec2,
    signal_strength: u16,
    color: u32,
) {
    cambc::debug_block! {
        match memory.map.distance_fields.weak_get(location_id) {
            Some(field) => {
                let radius: i32 = signal_strength as i32;
                for delta_y in -radius..=radius {
                    for delta_x in -radius..=radius {
                        let tile: Vec2 = Vec2 {
                            x: center.x + delta_x,
                            y: center.y + delta_y,
                        };
                        if field
                            .get(tile)
                            .is_ok_and(|distance| distance == signal_strength)
                        {
                            let _ = cambc::debug::draw_dot(tile, color);
                        }
                    }
                }
            }
            None => draw_circle(center, signal_strength as f32, color),
        }
        draw_cross(center, 1, color);
    }
}

fn inside_ping_area(
    memory: &ChampionMemory,
    location_id: LocationID,
    center: Vec2,
    position: Vec2,
    signal_strength: u16,
) -> bool {
    match memory.map.distance_fields.weak_get(location_id) {
        Some(field) => field
            .get(position)
            .is_ok_and(|distance| distance <= signal_strength),
        None => {
            let distance: i32 = chebyshev_distance(center, position).into();
            distance <= signal_strength as i32
        }
    }
}

fn draw_message(memory: &ChampionMemory, message: &Message) {
    cambc::debug_block! {
        if let Some(location_id) = message_location_id(message) {
            if let Some(center) = safe_locate(memory, location_id) {
                let color: u32 = message_color(message);
                let signal_strength: Option<u16> = message_signal_strength(message);

                match message {
                    Message::FarmClear { .. } => draw_cross(center, 2, color),
                    Message::GroupTogether { .. } => draw_circle(center, 15.0, color),
                    Message::StatusAllyLocation { .. } | Message::StatusEnemyLocation { .. } => {
                        draw_circle(center, 2.0, color)
                    }
                    _ => (),
                }
                if let Some(strength) = signal_strength {
                    draw_ping_area(memory, location_id, center, strength, color);
                }

                let responders: Vec<ChampionType> = message_responders(message);
                for (champion, info) in memory.champion_info.ally.iter() {
                    let Some(position) = info.last_seen else {
                        continue;
                    };
                    let reacted: bool = responders.contains(champion);
                    let in_range: bool = signal_strength.is_some_and(|strength| {
                        inside_ping_area(memory, location_id, center, position, strength)
                    });

                    if reacted {
                        let _ = cambc::debug::draw_line(position, center, color);
                        let _ = cambc::debug::draw_dot(position, COLOR_ALLY_CHAMPION);
                    } else if in_range {
                        let _ = cambc::debug::draw_dot(position, GREY);
                    }
                }
            }
        }
    }
}

fn draw_reaction(memory: &ChampionMemory, location_id: LocationID, action_group: ActionGroupType) {
    cambc::debug_block! {
        if let (Ok(Some(me)), Some(center)) =
            (cambc::self_champion(), safe_locate(memory, location_id))
        {
            if let Ok(position) = me.position() {
                let triggering_color: Option<u32> = memory
                    .message_board
                    .inbox
                    .iter()
                    .map(|received| &received.message)
                    .chain(memory.message_board.responses.iter())
                    .find(|message| {
                        message_location_id(message) == Some(location_id)
                            && (message_signal_strength(message).is_some()
                                || matches!(message, Message::FarmCollection { .. }))
                    })
                    .map(message_color);
                let color: u32 = triggering_color.unwrap_or(action_group_color(action_group));

                let _ = cambc::debug::draw_line(position, center, color);
                let _ = cambc::debug::draw_dot(position, COLOR_ALLY_CHAMPION);
            }
        }
    }
}
fn describe_leg(leg: &Leg) -> String {
    match leg {
        Leg::Walk { via, accepted } => {
            format!("walk to {:?} until within {} tiles", via, accepted)
        }
        Leg::Cross { from, to, kind } => {
            let kind_name: &str = match kind {
                CrossKind::Dash => "dash",
                CrossKind::Teleport => "teleport",
            };
            format!(
                "{} from ({}, {}) to ({}, {})",
                kind_name, from.x, from.y, to.x, to.y
            )
        }
    }
}

fn print_route(route: &Route) {
    println!(
        "  route: {} legs, on leg {}, estimated {} turns",
        route.legs.len(),
        route.cursor,
        route.est_turns
    );
    for (leg_index, leg) in route.legs.iter().enumerate() {
        let marker: &str = if leg_index == route.cursor { ">" } else { " " };
        println!("   {} {:>2}: {}", marker, leg_index, describe_leg(leg));
        if let Leg::Cross { from, to, kind } = leg {
            let line_color: u32 = match kind {
                CrossKind::Dash => COLOR_SHORT_JUMP,
                CrossKind::Teleport => COLOR_LONG_JUMP,
            };
            let _ = cambc::debug::draw_line(*from, *to, line_color);
        }
    }
}

fn describe_progress(memory: &ChampionMemory, location_id: LocationID) -> String {
    let Some(target) = safe_locate(memory, location_id) else {
        return format!("target {:?} has no known location", location_id);
    };
    let Ok(Some(me)) = cambc::self_champion() else {
        return format!(
            "target {:?} at {}",
            location_id,
            describe_location(Some(target))
        );
    };
    let Ok(position) = me.position() else {
        return format!(
            "target {:?} at {}",
            location_id,
            describe_location(Some(target))
        );
    };
    let straight_distance: i32 = chebyshev_distance(target, position).into();
    let walking_distance: Option<u16> = memory
        .map
        .distance_fields
        .weak_get(location_id)
        .and_then(|field| field.get(position).ok())
        .filter(|distance| *distance != u16::MAX);
    format!(
        "target {:?} at {}, we are at {}, straight line {} tiles, walking {}",
        location_id,
        describe_location(Some(target)),
        describe_location(Some(position)),
        straight_distance,
        match walking_distance {
            Some(distance) => format!("{} tiles", distance),
            None => String::from("unknown (field not cached or unreachable)"),
        },
    )
}

fn print_camp_group_details(memory: &ChampionMemory, camp_group_id: usize) {
    let map = &memory.map;
    let (Some(center), Some(attack_order)) = (
        map.points_of_interest.camp_group_centers.get(camp_group_id),
        map.camp_group_attack_order.get(camp_group_id),
    ) else {
        println!("  camp group {} is not in the map", camp_group_id);
        return;
    };
    println!(
        "  camp group {} at ({}, {}) respawn {} hp {} dmg {} gem {} gold {} attack_order {:?}",
        camp_group_id,
        center.x,
        center.y,
        map.camp_group_respawn_timer[camp_group_id],
        map.camp_group_effective_hp[camp_group_id],
        map.camp_group_damage_per_round[camp_group_id],
        map.camp_group_gem_value[camp_group_id],
        map.camp_group_gold_value[camp_group_id],
        attack_order,
    );
}

pub fn print_current_action(action: &Action, memory: &ChampionMemory) {
    cambc::debug_block! {
        println!("== CURRENT ACTION ==");
        println!("The action is what this champion is doing this turn. Each action belongs to an action group, which is the category the strategy uses to decide what to do next. Move actions follow a distance field to a location, either directly or along a planned route of walk, dash and teleport legs.");
        println!("On the map: a mint dot marks this champion with a line to its target, coloured by the message that triggered it or by its action group. Dash legs of a route are blue lines and teleport legs are magenta lines.");
        println!("Sections: the action and its state, progress towards the target (straight line and walking distance), then the route or camp group values where they apply.");

        match action {
            Action::MoveToLocation(state) => {
                println!("Action: MoveToLocation in group {:?}", state.action_group_type);
                println!("  {}", describe_progress(memory, state.location_id));
                println!(
                    "  accepted distance {} route allowed {} route failures {}",
                    state.accepted_distance, state.allow_route, state.route_failures,
                );
                match &state.route {
                    Some(route) => print_route(route),
                    None => println!("  route: none, walking the distance field directly"),
                }
            }
            Action::DefendLocation(state) => {
                println!("Action: DefendLocation");
                println!("  {}", describe_progress(memory, state.location_id));
                println!("  state {:?}", state);
            }
            Action::InCampGroupCombat(state) => {
                println!("Action: InCampGroupCombat");
                print_camp_group_details(memory, state.camp_group_id.0);
                println!("  state {:?}", state);
            }
            Action::CollectingFromCamp(state) => {
                println!("Action: CollectingFromCamp");
                print_camp_group_details(memory, state.camp_group_id.0);
                println!("  state {:?}", state);
            }
            other => println!("Action: {:?}", other),
        }
    }
}

pub fn debug_current_action(action: &Action, memory: &ChampionMemory) {
    print_current_action(action, memory);
    match action {
        Action::MoveToLocation(state) => {
            draw_reaction(memory, state.location_id, state.action_group_type)
        }
        Action::DefendLocation(state) => {
            draw_reaction(memory, state.location_id, ActionGroupType::Support)
        }
        Action::InCampGroupCombat(state) => draw_reaction(
            memory,
            LocationID::CampGroup(state.camp_group_id),
            ActionGroupType::EndFarm,
        ),
        Action::CollectingFromCamp(state) => draw_reaction(
            memory,
            LocationID::CampGroup(state.camp_group_id),
            ActionGroupType::EndFarm,
        ),
        _ => (),
    }
}
