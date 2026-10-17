//! Team messages in the 8 bytes of shared memory.
//!   [0] bit0 help, bit1 contest, bits2-4 number of farmers
//!   [1] help round  [2] help x  [3] help y
//!   [4] contest round [5] contest x [6] contest y
//!   [7] lane-claim bitmask (bit set = an ally is working that lane)
use cambc as bc;
use cambc::Vec2;

fn get(i: usize) -> u8 {
    bc::shared_memory().get(i).unwrap_or(0)
}
fn set(i: usize, v: u8) {
    let _ = bc::shared_memory().set(i, v);
}
fn fresh(stamp: u8, round: i32, ttl: u8) -> bool {
    (round as u8).wrapping_sub(stamp) <= ttl
}

/// Per-bot bookkeeping so a champion only writes its own flags on transitions.
#[derive(Default)]
pub struct Comms {
    farming: bool,
    claimed: bool,
}

impl Comms {
    /// Publish "I am farming" and "I am working lane `lane`" (changes only).
    pub fn set_flags(&mut self, farming: bool, claimed: bool, lane: u8) {
        if farming != self.farming {
            let b = get(0);
            let c = (b >> 2) & 7;
            let c2 = if farming { (c + 1).min(7) } else { c.saturating_sub(1) };
            set(0, (b & !(7 << 2)) | (c2 << 2));
            self.farming = farming;
        }
        if claimed != self.claimed {
            let m = get(7);
            let bit = 1u8 << lane.min(7);
            set(7, if claimed { m | bit } else { m & !bit });
            self.claimed = claimed;
        }
    }
    pub fn farmers(&self) -> u8 {
        (get(0) >> 2) & 7
    }
    /// Is any ally working this lane? (false = "lane is empty, take the space")
    pub fn lane_taken(&self, lane: u8) -> bool {
        get(7) & (1u8 << lane.min(7)) != 0
    }
    pub fn call_help(&self, p: Vec2, round: i32) {
        set(1, round as u8);
        set(2, p.x as u8);
        set(3, p.y as u8);
        set(0, get(0) | 1);
    }
    pub fn read_help(&self, round: i32) -> Option<Vec2> {
        if get(0) & 1 != 0 && fresh(get(1), round, 5) {
            Some(Vec2::new(get(2) as i32, get(3) as i32))
        } else {
            None
        }
    }
    pub fn call_contest(&self, p: Vec2, round: i32) {
        set(4, round as u8);
        set(5, p.x as u8);
        set(6, p.y as u8);
        set(0, get(0) | 2);
    }
    pub fn read_contest(&self, round: i32) -> Option<Vec2> {
        if get(0) & 2 != 0 && fresh(get(4), round, 12) {
            Some(Vec2::new(get(5) as i32, get(6) as i32))
        } else {
            None
        }
    }
}
