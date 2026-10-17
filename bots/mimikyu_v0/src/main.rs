//! CAMBC bot (cambc2026a) - lanes, champion states and team messages.
//!
//! Module map
//!   drafting.rs  draft picks (matchup + DPS scoring, counter-picks) and the lane plan
//!   plan.rs      draft-memory format shared by all champion bots
//!   comms.rs     8-byte shared-memory messages (help, contest, farmers, lane claims)
//!   bot.rs       per-turn priority state machine
//!   farm.rs      camp choice, objective timers, capture
//!   combat.rs    targets, ability choice, engagement, fight evaluation
//!   nav.rs       terrain-aware movement helpers (host does the pathfinding)
//!   shopping.rs  emblem build orders, altar lock purchase
//!   data.rs      generated from tools/analysis.py via tools/gen_data.py
//!   ctx.rs/util.rs  shared observations and helpers
//!
//! Fuel: Rust gets ~100k per turn. All pathfinding, vision and masks run in the host.

use bot::{bot::Bot, drafting};
use cambc as bc;

fn main() {
    let mut bot = Bot::new();
    let mut draft_mem: Vec<u8> = Vec::new();
    loop {
        println!("{:?}", bc::round_number());
        if bc::is_draft() {
            let _ = drafting::turn(&mut draft_mem);
        } else {
            let _ = bot.turn();
        }
        bc::end_turn();
    }
}
