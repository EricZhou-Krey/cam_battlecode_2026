//! Terrain-aware movement helpers. Pathfinding itself is done by the host
//! (`Champion::move_towards`), which keeps fuel use low; this module only picks
//! *where* to walk.
use cambc as bc;
use cambc::Vec2;

use crate::util::{centroid, cheb};

pub struct Nav {
    /// Terrain walkable by us (no walls, holes, altars or the enemy spawn).
    walk: bc::Bitboard,
    pub spawn: &'static bc::Bitboard,
    pub home: Vec2,
    pub enemy_home: Vec2,
    /// (tile we can stand on, altar it reaches) for every altar.
    pub stands: Vec<(Vec2, Vec2)>,
}

impl Nav {
    pub fn new() -> bc::Result<Nav> {
        let spawn = bc::map::spawn_region(bc::Team::Ally)?;
        let enemy_sp = bc::map::spawn_region(bc::Team::Enemy)?;
        let altars = bc::map::altars()?;
        let blocked = bc::map::walls()?.union(bc::map::holes()?)?.union(altars)?;
        let walk = blocked.complement()?.intersect(&enemy_sp.complement()?)?;
        let r = bc::constants().submit_gems_range.max(1);
        let mut stands = Vec::new();
        for a in altars.positions()? {
            for dx in -r..=r {
                for dy in -r..=r {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let p = Vec2::new(a.x + dx, a.y + dy);
                    if bc::map::is_in_bounds(p)? && walk.is_set(p)? {
                        stands.push((p, a));
                    }
                }
            }
        }
        Ok(Nav {
            walk,
            spawn,
            home: centroid(spawn)?,
            enemy_home: centroid(enemy_sp)?,
            stands,
        })
    }

    pub fn walkable(&self, p: Vec2) -> bool {
        self.walk.is_set(p).unwrap_or(false)
    }
    pub fn on_spawn(&self, p: Vec2) -> bool {
        self.spawn.is_set(p).unwrap_or(false)
    }
    /// Nearest altar stand to `pos`.
    pub fn stand_for(&self, pos: Vec2) -> Option<(Vec2, Vec2)> {
        self.stands.iter().copied().min_by_key(|s| cheb(pos, s.0))
    }

    /// Walk to the nearest free tile from which `tp` is `min..=stop` tiles away,
    /// preferring maximum range. `move_towards` refuses occupied targets, so we
    /// never path onto the target itself. Returns true if we moved.
    pub fn to_range(
        &self,
        me: &bc::Champion,
        pos: Vec2,
        occ: &bc::Bitboard,
        tp: Vec2,
        min: i32,
        stop: i32,
    ) -> bool {
        let mut best: Option<((i32, i32), Vec2)> = None;
        for dx in -stop..=stop {
            for dy in -stop..=stop {
                let p = Vec2::new(tp.x + dx, tp.y + dy);
                if cheb(p, tp) < min {
                    continue;
                }
                let key = (cheb(pos, p), -cheb(p, tp));
                if let Some((k, _)) = best {
                    if key >= k {
                        continue;
                    }
                }
                if !self.walkable(p) || occ.is_set(p).unwrap_or(true) {
                    continue;
                }
                best = Some((key, p));
            }
        }
        match best {
            Some((_, p)) if cheb(pos, p) > 0 => me.move_towards(p).unwrap_or(false),
            _ => false,
        }
    }

    /// One step that maximises distance from every foe (ties: toward home).
    pub fn flee(&self, me: &bc::Champion, pos: Vec2, occ: &bc::Bitboard, foes: &[Vec2]) -> bool {
        let mut cands: Vec<(i32, Vec2)> = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let p = Vec2::new(pos.x + dx, pos.y + dy);
                if !self.walkable(p) || occ.is_set(p).unwrap_or(true) {
                    continue;
                }
                let away = foes.iter().map(|f| cheb(*f, p)).min().unwrap_or(9);
                cands.push((away * 4 - cheb(p, self.home), p));
            }
        }
        cands.sort_by(|a, b| b.0.cmp(&a.0));
        cands
            .into_iter()
            .any(|(_, p)| me.move_absolute(p).unwrap_or(false))
    }

    /// Use a ready dash ability to jump two tiles away from the foes.
    pub fn dash_away(
        &self,
        me: &bc::Champion,
        pos: Vec2,
        occ: &bc::Bitboard,
        foes: &[Vec2],
    ) -> bool {
        let mut best: Option<(i32, Vec2)> = None;
        for dx in -2_i32..=2 {
            for dy in -2_i32..=2 {
                if dx.abs().max(dy.abs()) != 2 {
                    continue;
                }
                let p = Vec2::new(pos.x + dx, pos.y + dy);
                if !self.walkable(p) || occ.is_set(p).unwrap_or(true) {
                    continue;
                }
                let away = foes.iter().map(|f| cheb(*f, p)).min().unwrap_or(9);
                let s = away * 4 - cheb(p, self.home);
                if best.map(|b| s > b.0).unwrap_or(true) {
                    best = Some((s, p));
                }
            }
        }
        if let Some((_, p)) = best {
            for a in me.abilities().iter() {
                if a.is_dash() && a.is_ready().unwrap_or(false) && a.cast_at(p).unwrap_or(false) {
                    return true;
                }
            }
        }
        false
    }
}
