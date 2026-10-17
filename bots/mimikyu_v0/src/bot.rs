//! One champion's turn. Priorities, highest first:
//!   Survive > Altar-lock denial > Tribute > Defend (help call) > Hunt >
//!   Capture hold > Steal / Farm / Scout.
//! Every champion runs this same code; its draft role (plan.rs) only biases which
//! camps it prefers, and the team messages in comms.rs keep the lanes covered.
use cambc as bc;
use cambc::Vec2;

use crate::combat::{self, Tgt};
use crate::comms::Comms;
use crate::ctx::Ctx;
use crate::farm::Farm;
use crate::nav::Nav;
use crate::plan::{self, Plan, St};
use crate::shopping;
use crate::util::*;

pub struct Bot {
    inited: bool,
    ty: bc::ChampionType,
    key: &'static str,
    plan: Option<Plan>,
    nav: Option<Nav>,
    farm: Option<Farm>,
    comms: Comms,
    lane: u8,
    home_role: St,
    state: St,
}

impl Bot {
    pub fn new() -> Bot {
        Bot {
            inited: false,
            ty: bc::ChampionType::Ryth,
            key: "",
            plan: None,
            nav: None,
            farm: None,
            comms: Comms::default(),
            lane: 0,
            home_role: St::Farm,
            state: St::Farm,
        }
    }

    fn init(&mut self, me: &bc::Champion) -> bc::Result<()> {
        self.ty = me.champion_name()?;
        self.key = champion_key(self.ty);
        self.plan = plan::parse();
        // our slot in the draft decides our lane and starting role
        let picks = bc::draft::current_draft(bc::Team::Ally)?;
        let slot = picks
            .iter()
            .position(|d| draft_key(*d) == self.key)
            .unwrap_or(0)
            .min(4);
        if let Some(p) = &self.plan {
            self.lane = p.lanes[slot];
            self.home_role = St::from_u8(p.roles[slot]);
            self.state = self.home_role;
            self.farm = Some(Farm::new(p)?);
        }
        self.nav = Some(Nav::new()?);
        Ok(())
    }

    pub fn turn(&mut self) -> bc::Result<()> {
        let me = match bc::self_champion()? {
            Some(m) => m,
            None => return Ok(()),
        };
        if !self.inited {
            self.init(me)?;
            self.inited = true;
        }
        if !me.is_alive()? {
            // dead champions still get a turn: spend gold, drop all claims
            self.comms.set_flags(false, false, self.lane);
            if let Some(f) = self.farm.as_mut() {
                f.abort_capture();
            }
            return shopping::shop(me, self.ty);
        }
        let _ = shopping::shop(me, self.ty);
        if me.is_recalling()? || me.is_submitting_gems()? {
            return Ok(());
        }
        let nav = match self.nav.as_ref() {
            Some(n) => n,
            None => return Ok(()),
        };

        let pos = me.position()?;
        let hp = me.health()?;
        let hpf = hp / me.max_health()?.max(1.0);
        if nav.on_spawn(pos) && hpf < 0.99 {
            return Ok(()); // heal up on the spawn tiles
        }
        let round = bc::round_number()? as i32;

        // ---- observe --------------------------------------------------------
        let mut foes: Vec<(bc::Champion, Vec2, i32)> =
            bc::vision::champions(Some(bc::Team::Enemy))?
                .into_iter()
                .filter_map(|f| {
                    let p = f.position().ok()?;
                    Some((f, p, cheb(p, pos)))
                })
                .collect();
        foes.sort_by_key(|f| f.2);
        let allies: Vec<(bc::Champion, Vec2)> = bc::vision::champions(Some(bc::Team::Ally))?
            .into_iter()
            .filter_map(|a| {
                let p = a.position().ok()?;
                if cheb(p, pos) == 0 {
                    None
                } else {
                    Some((a, p))
                }
            })
            .collect();
        let occ = bc::vision::champion_tiles(None)?.union(&bc::vision::camp_tiles()?)?;
        let counts = me.item_count()?;
        let c = Ctx {
            me,
            pos,
            round,
            hp,
            hpf,
            gems: me.gem_count()?,
            gold: me.gold_count()?,
            my_dpt: dpt_of(me),
            camp_dmg_mult: me
                .base_stats()
                .ok()
                .map(|s| s.camp_damage_multiplier)
                .filter(|m| *m > 0.0)
                .unwrap_or(1.0)
                * if shopping::has_emblem(&counts, shopping::MONSTER) {
                    1.1
                } else {
                    1.0
                },
            camp_taken_mult: if shopping::has_emblem(&counts, shopping::JUNGLE) {
                0.7
            } else {
                1.0
            },
            foes,
            allies,
            camps: bc::vision::camps()?,
            vis: bc::vision::visible_tiles()?,
            occ,
        };
        let plan = self.plan.as_ref();
        if let (Some(f), Some(p)) = (self.farm.as_mut(), plan) {
            f.refresh(&c, p);
        }

        // ---- 1. SURVIVE -----------------------------------------------------------
        let near: Vec<&(bc::Champion, Vec2, i32)> = c.foes.iter().filter(|f| f.2 <= 7).collect();
        if let Some(first) = near.first() {
            let ratio = combat::fight_ratio(&c, &first.0, &near);
            let losing = ratio < 0.9;
            let gem_risk = c.gems >= 15 && ratio < 1.3;
            if losing || hpf < 0.3 || gem_risk {
                self.state = St::Survive;
                if let Some(f) = self.farm.as_mut() {
                    f.abort_capture();
                }
                self.comms.call_help(pos, round);
                self.comms.set_flags(false, false, self.lane);
                let tgt = Tgt::Foe(&first.0, first.1);
                if cheb(pos, first.1) <= 4 {
                    combat::cast_best(me, &tgt); // stun / damage / shield on the way out
                }
                let fpos: Vec<Vec2> = c.foes.iter().map(|f| f.1).collect();
                if !nav.dash_away(me, pos, &c.occ, &fpos) && !nav.flee(me, pos, &c.occ, &fpos) {
                    combat::engage(nav, &c, &tgt); // cornered: fight back
                }
                return Ok(());
            }
        } else if self.state == St::Survive {
            self.state = self.home_role;
        }

        // calm: recall when hurt, or when a shopping trip is worth it (Tavi shops anywhere)
        if c.foes.iter().all(|f| f.2 > 9) {
            let trip = self.key != "tavi" && c.gold >= 150 && c.gems < 15;
            if hpf < 0.4 || trip {
                if me.recall().is_ok() {
                    self.comms.set_flags(false, false, self.lane);
                    if let Some(f) = self.farm.as_mut() {
                        f.abort_capture();
                    }
                    return Ok(());
                }
            }
        }

        // ---- 2. DENY enemy tributes with an altar lock ---------------------------------
        let sr = bc::constants().submit_gems_range;
        for f in &c.foes {
            if f.0.is_submitting_gems().unwrap_or(false) && f.0.gem_count().unwrap_or(0) >= 10 {
                if let Some(&(_, altar)) = nav
                    .stands
                    .iter()
                    .find(|s| cheb(pos, s.1) <= 2 && cheb(f.1, s.1) <= sr)
                {
                    let lock = &me.consumables()[bc::ConsumableType::AltarLock];
                    if lock.use_at(altar).unwrap_or(false) {
                        return Ok(());
                    }
                }
            }
        }

        // ---- 3. TRIBUTE ----------------------------------------------------------------
        let banked = bc::global_submitted_gems(bc::Team::Ally).unwrap_or(0);
        let thr = if banked + c.gems >= bc::constants().winning_gems || round > 1850 {
            1
        } else if !c.foes.is_empty() {
            10
        } else {
            20
        };
        if c.gems >= thr {
            if let Some((stand, altar)) = nav.stand_for(pos) {
                if cheb(pos, altar) <= sr {
                    if me.submit_gems(altar).is_ok() {
                        self.comms.set_flags(false, true, self.lane);
                        return Ok(());
                    }
                } else {
                    let _ = me.move_towards(stand);
                    self.comms.set_flags(false, true, self.lane);
                    return Ok(());
                }
            }
        }

        // ---- 4. DEFEND: answer a help call ----------------------------------------------
        if hpf > 0.6 && c.foes.is_empty() {
            if let Some(h) = self.comms.read_help(round) {
                let d = cheb(pos, h);
                if d > 0 && d <= 30 {
                    self.state = St::Defend;
                    self.comms.set_flags(false, true, self.lane);
                    let _ = me.move_towards(h);
                    return Ok(());
                }
            }
        }

        // ---- 5. HUNT: favourable fights; gem carriers are worth extra ------------------------
        if !c.foes.is_empty() {
            let mut best: Option<(f32, usize)> = None;
            for (i, f) in c.foes.iter().enumerate() {
                if f.2 > 9 {
                    continue;
                }
                let others: Vec<&(bc::Champion, Vec2, i32)> =
                    c.foes.iter().filter(|g| cheb(g.1, f.1) <= 5).collect();
                let ratio = combat::fight_ratio(&c, &f.0, &others);
                let bounty = f.0.gem_count().unwrap_or(0) as f32;
                if !(ratio > 1.15 || (ratio > 0.95 && bounty >= 25.0)) {
                    continue;
                }
                let s = ratio * (1.0 + bounty / 20.0) / (1.0 + f.2 as f32 / 4.0);
                if best.map(|b| s > b.0).unwrap_or(true) {
                    best = Some((s, i));
                }
            }
            if let Some((_, i)) = best {
                self.state = St::Hunt;
                if let Some(f) = self.farm.as_mut() {
                    f.abort_capture();
                }
                self.comms.set_flags(false, true, self.lane);
                let f = &c.foes[i];
                combat::engage(nav, &c, &Tgt::Foe(&f.0, f.1));
                return Ok(());
            }
        }

        // ---- 6. contest message + capture hold ------------------------------------------------
        for f in &c.foes {
            if let Some(k) = c.camps.iter().find(|k| cheb(k.spawn_position(), f.1) <= 4) {
                self.comms.call_contest(k.spawn_position(), round);
                break;
            }
        }
        let (plan, farm) = match (self.plan.as_ref(), self.farm.as_mut()) {
            (Some(p), Some(f)) => (p, f),
            _ => {
                // no plan in draft memory: just guard the middle of our side
                let _ = me.move_towards(nav.home);
                return Ok(());
            }
        };
        if farm.capture_step(&c, plan) {
            self.comms.set_flags(true, true, self.lane);
            return Ok(()); // stand still while the capture timer runs
        }

        // ---- 7. STEAL / FARM / SCOUT ------------------------------------------------------------
        let mut target: Option<usize> = None;
        let mut st = self.home_role;
        if let Some(cp) = self.comms.read_contest(round) {
            if hpf > 0.6 && cheb(pos, cp) <= 30 {
                target = (0..plan.entries.len())
                    .filter(|&i| cheb(plan.entries[i].tile(), cp) <= plan.entries[i].rad + 3)
                    .min_by_key(|&i| cheb(plan.entries[i].tile(), pos));
                if target.is_some() {
                    st = St::Steal; // counter-jungle the camp an enemy is working
                }
            }
        }
        if target.is_none() {
            target = farm.pick(&c, plan, nav, &self.comms, self.lane, self.home_role);
            if matches!(st, St::Hunt | St::Steal) && self.comms.farmers() == 0 {
                st = St::Farm; // somebody has to farm
            }
        }
        self.state = if st == St::Hunt { St::Farm } else { st };
        self.comms
            .set_flags(matches!(self.state, St::Farm | St::Scout), true, self.lane);

        match target.or_else(|| farm.nearest(&c, plan)) {
            Some(i) => {
                let tile = plan.entries[i].tile();
                if target.is_some() {
                    farm.work(&c, plan, nav, i);
                } else {
                    // nothing safe alone: wait next to the nearest camps for company
                    nav.to_range(me, pos, &c.occ, tile, 2, 4);
                }
                ward(&c, tile);
            }
            None => {
                let _ = me.move_towards(nav.home);
            }
        }
        Ok(())
    }
}

/// Drop the ward as far toward our objective as placement allows: vision ahead of
/// the fight (and of the camp respawn) is the cheapest information in the game.
fn ward(c: &Ctx, toward: Vec2) {
    if c.me.ward_cooldown().unwrap_or(1) != 0 {
        return;
    }
    if let Ok(tiles) = c.me.ward_placement_tiles() {
        if let Ok(ps) = tiles.positions() {
            if let Some(p) = ps.iter().min_by_key(|p| cheb(**p, toward)) {
                let _ = c.me.place_ward(*p);
            }
        }
    }
}
