//! Camp selection, objective timers and capture.
//!
//! League-of-Legends ideas used here:
//!  * objective timers - `CampGroup::state()` tells us exactly when a visible group
//!    returns, so we walk to arrive as it respawns instead of racing the enemy;
//!  * value over time - groups are scored by (3*gems + gold) / (travel + kill + capture);
//!  * don't take fights you can't win - a camp is skipped if we would die before
//!    clearing it (elks and monoliths need allies, so they naturally wait for a group);
//!  * counter-jungling - the Steal role prefers camps on the enemy half and the
//!    contest message redirects free champions to camps an enemy is working.
use cambc as bc;

use crate::combat::{self, Tgt};
use crate::comms::Comms;
use crate::ctx::Ctx;
use crate::nav::Nav;
use crate::plan::{Plan, St};
use crate::util::cheb;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Known {
    Unknown,
    Alive,
    Reward,
    /// Round at which the group returns.
    Respawn(i32),
}

struct Cap {
    idx: usize,
    start: i32,
    gems: u32,
    gold: u32,
}

pub struct Farm {
    groups: Vec<Option<bc::CampGroup>>,
    known: Vec<Known>,
    done_at: Vec<i32>,
    cap: Option<Cap>,
}

impl Farm {
    pub fn new(plan: &Plan) -> bc::Result<Farm> {
        let all = bc::map::camp_groups()?;
        let groups: Vec<Option<bc::CampGroup>> = plan
            .entries
            .iter()
            .map(|e| {
                all.iter()
                    .find(|g| g.camps().first().map(|c| c.spawn_position() == e.tile()).unwrap_or(false))
                    .cloned()
            })
            .collect();
        let n = groups.len();
        Ok(Farm {
            groups,
            known: vec![Known::Unknown; n],
            done_at: vec![-100_000; n],
            cap: None,
        })
    }

    pub fn holding(&self) -> bool {
        self.cap.is_some()
    }
    pub fn abort_capture(&mut self) {
        self.cap = None;
    }

    /// Query the live state of up to three visible groups (each query costs host calls).
    pub fn refresh(&mut self, c: &Ctx, plan: &Plan) {
        let mut idx: Vec<usize> = (0..plan.entries.len()).collect();
        idx.sort_by_key(|&i| cheb(c.pos, plan.entries[i].tile()));
        let mut seen = 0;
        for i in idx {
            if seen >= 3 || cheb(c.pos, plan.entries[i].tile()) > 25 {
                break;
            }
            if !c.visible(plan.entries[i].tile()) {
                continue;
            }
            if let Some(g) = &self.groups[i] {
                if let Ok(s) = g.state() {
                    self.known[i] = match s {
                        bc::CampGroupState::Alive => Known::Alive,
                        bc::CampGroupState::RewardAvailable => Known::Reward,
                        bc::CampGroupState::Respawning(n) => Known::Respawn(c.round + n),
                    };
                    seen += 1;
                }
            }
        }
    }

    /// Rounds until the group is next available (0 = now / unknown).
    fn eta(&self, i: usize, round: i32, resp: i32) -> i32 {
        match self.known[i] {
            Known::Respawn(t) => (t - round).max(0),
            Known::Alive | Known::Reward => 0,
            Known::Unknown => (self.done_at[i] + resp - round).max(0),
        }
    }

    /// Best entry for this champion, or None if nothing is safe to attempt alone.
    pub fn pick(
        &self,
        c: &Ctx,
        plan: &Plan,
        nav: &Nav,
        comms: &Comms,
        lane: u8,
        role: St,
    ) -> Option<usize> {
        let (sd, sh) = c.support(c.pos);
        let team_dpt = (c.my_dpt * c.camp_dmg_mult + sd).max(1.0);
        let team_hp = c.hp + sh;
        let margin = if role == St::Scout { 2.5 } else { 1.7 };
        let total = cheb(nav.home, nav.enemy_home).max(1) as f32;
        let cap_t = bc::constants().capture_time as f32;
        let mut best: Option<(f32, usize)> = None;
        for (i, e) in plan.entries.iter().enumerate() {
            let tile = e.tile();
            let dist = cheb(c.pos, tile) as f32;
            let eta = self.eta(i, c.round, e.resp) as f32;
            if eta > dist + 25.0 {
                continue; // not worth waiting for
            }
            let reward = self.known[i] == Known::Reward;
            let ttk = if reward { 0.0 } else { e.hp as f32 / team_dpt };
            if !reward && e.dmg > 0 {
                let tdie = team_hp / (e.dmg as f32 * c.camp_taken_mult).max(1.0);
                if tdie < ttk * margin {
                    continue; // we would lose too much HP: wait for company
                }
            }
            let value = e.gems as f32 * 3.0 + e.gold as f32;
            let mut s = value / (dist.max(eta) + ttk + cap_t);
            s *= if e.lane == lane {
                1.0
            } else if !comms.lane_taken(e.lane) {
                0.85 // empty lane: free space
            } else {
                0.55
            };
            if role == St::Steal {
                s *= 2.0 - (cheb(tile, nav.enemy_home) as f32 / total).min(1.0);
            }
            if best.map(|b| s > b.0).unwrap_or(true) {
                best = Some((s, i));
            }
        }
        best.map(|b| b.1)
    }

    /// Entry nearest to us (any safety): where to wait for allies or respawns.
    pub fn nearest(&self, c: &Ctx, plan: &Plan) -> Option<usize> {
        (0..plan.entries.len()).min_by_key(|&i| cheb(c.pos, plan.entries[i].tile()))
    }

    /// Walk to, fight, or capture the group `i`.
    pub fn work(&mut self, c: &Ctx, plan: &Plan, nav: &Nav, i: usize) {
        let e = &plan.entries[i];
        let tile = e.tile();
        let alive: Vec<&bc::Camp> = c
            .camps
            .iter()
            .filter(|k| cheb(k.spawn_position(), tile) <= e.rad + 1)
            .collect();
        if !alive.is_empty() {
            self.known[i] = Known::Alive;
            let free = alive
                .iter()
                .copied()
                .filter(|k| k.is_gem_cluster() || k.is_gold_cluster())
                .min_by_key(|k| cheb(k.spawn_position(), c.pos));
            let near = alive
                .iter()
                .copied()
                .min_by_key(|k| cheb(k.spawn_position(), c.pos));
            if let Some(k) = free.or(near) {
                combat::engage(nav, c, &Tgt::Camp(k));
            }
        } else if self.known[i] == Known::Reward || c.visible(tile) {
            // cleared: stand on a spawn tile for the capture timer
            if cheb(c.pos, tile) == 0 {
                if self.cap.is_none() {
                    self.cap = Some(Cap { idx: i, start: c.round, gems: c.gems, gold: c.gold });
                }
            } else {
                let _ = c.me.move_towards(tile);
            }
        } else if self.eta(i, c.round, e.resp) > 0 && cheb(c.pos, tile) <= 4 {
            // respawning soon: wait beside it (standing ON a spawn tile would delay the respawn)
            nav.to_range(c.me, c.pos, &c.occ, tile, 1, 3);
        } else {
            let _ = c.me.move_towards(tile);
        }
    }

    /// Advance the capture timer; true while we should keep standing still.
    pub fn capture_step(&mut self, c: &Ctx, plan: &Plan) -> bool {
        let (i, start, g0, gold0) = match &self.cap {
            Some(k) => (k.idx, k.start, k.gems, k.gold),
            None => return false,
        };
        let e = &plan.entries[i];
        let paid = c.gems > g0 || c.gold > gold0;
        let timeout = c.round - start > bc::constants().capture_time + 3;
        if paid || timeout || cheb(c.pos, e.tile()) > 0 {
            self.done_at[i] = c.round;
            self.known[i] = Known::Respawn(c.round + e.resp);
            self.cap = None;
            return false;
        }
        true
    }
}
