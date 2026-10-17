//! Everything a champion observes at the start of its turn.
use cambc as bc;
use cambc::Vec2;

use crate::util::cheb;

pub struct Ctx<'a> {
    pub me: &'a bc::Champion,
    pub pos: Vec2,
    pub round: i32,
    pub hp: f32,
    pub hpf: f32,
    pub gems: u32,
    pub gold: u32,
    /// Sustained damage per turn against champions.
    pub my_dpt: f32,
    /// Multiplier on our damage to camps (design bonus x Monster emblem).
    pub camp_dmg_mult: f32,
    /// Multiplier on damage camps deal to us (Jungle emblem).
    pub camp_taken_mult: f32,
    /// Visible enemy champions with position and distance, nearest first.
    pub foes: Vec<(bc::Champion, Vec2, i32)>,
    /// Visible allies (never ourselves).
    pub allies: Vec<(bc::Champion, Vec2)>,
    pub camps: Vec<bc::Camp>,
    pub vis: bc::Bitboard,
    pub occ: bc::Bitboard,
}

impl<'a> Ctx<'a> {
    pub fn visible(&self, p: Vec2) -> bool {
        self.vis.is_set(p).unwrap_or(false)
    }
    /// Damage per turn and half-HP of allies within 6 tiles of `around`.
    pub fn support(&self, around: Vec2) -> (f32, f32) {
        let (mut d, mut h) = (0.0, 0.0);
        for (a, p) in &self.allies {
            if cheb(*p, around) <= 6 {
                d += crate::util::dpt_of(a);
                h += a.health().unwrap_or(0.0) * 0.5;
            }
        }
        (d, h)
    }
}
