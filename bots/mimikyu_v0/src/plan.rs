//! The lane plan written during the draft and read by every champion bot.
//!
//! Draft memory layout (1024 bytes):
//!   [0]=MAGIC [1]=lane count K [2]=entry count [3]=team size
//!   [4..9]  initial state per draft slot   [9..14] lane per draft slot
//!   from HDR: 9-byte entries  x y lane hp/100 dmg gems/2 gold/3 respawn/2 radius
//! (x,y) is the spawn tile of the group's first camp, which also identifies the group.
use cambc as bc;

pub const MAGIC: u8 = 0xCB;
pub const HDR: usize = 16;
pub const ENT: usize = 9;
pub const MAX_ENT: usize = (1024 - HDR) / ENT;

/// What a champion is currently doing (also the initial role assigned in the draft).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum St {
    Farm = 0,
    Steal = 1,
    Hunt = 2,
    Survive = 3,
    Defend = 4,
    Scout = 5,
}
impl St {
    pub fn from_u8(b: u8) -> St {
        match b {
            1 => St::Steal,
            2 => St::Hunt,
            3 => St::Survive,
            4 => St::Defend,
            5 => St::Scout,
            _ => St::Farm,
        }
    }
    /// Starting state per champion: bruisers hunt, mobile damage dealers steal,
    /// Tavi scouts (remote shopping means he never has to leave the lane).
    pub fn role_for(key: &str) -> St {
        match key {
            "marcella" => St::Hunt,
            "tavi" => St::Scout,
            "ren" | "cassi" | "vendetta" => St::Steal,
            _ => St::Farm,
        }
    }
}

pub struct Entry {
    pub x: i32,
    pub y: i32,
    pub lane: u8,
    pub hp: i32,
    pub dmg: i32,
    pub gems: i32,
    pub gold: i32,
    pub resp: i32,
    pub rad: i32,
}
impl Entry {
    pub fn tile(&self) -> bc::Vec2 {
        bc::Vec2::new(self.x, self.y)
    }
}

pub struct Plan {
    pub roles: [u8; 5],
    pub lanes: [u8; 5],
    pub entries: Vec<Entry>,
}

pub fn parse() -> Option<Plan> {
    let b = bc::draft_memory().bytes();
    if b.len() < HDR || b[0] != MAGIC {
        return None;
    }
    let n = b[2] as usize;
    let mut roles = [0u8; 5];
    let mut lanes = [0u8; 5];
    roles.copy_from_slice(&b[4..9]);
    lanes.copy_from_slice(&b[9..14]);
    let mut entries = Vec::new();
    for i in 0..n {
        let o = HDR + i * ENT;
        if o + ENT > b.len() {
            break;
        }
        entries.push(Entry {
            x: b[o] as i32,
            y: b[o + 1] as i32,
            lane: b[o + 2],
            hp: b[o + 3] as i32 * 100,
            dmg: b[o + 4] as i32,
            gems: b[o + 5] as i32 * 2,
            gold: b[o + 6] as i32 * 3,
            resp: b[o + 7] as i32 * 2,
            rad: b[o + 8] as i32,
        });
    }
    Some(Plan {
        roles,
        lanes,
        entries,
    })
}
