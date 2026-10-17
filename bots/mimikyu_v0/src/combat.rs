//! Fighting: target abstraction, ability choice, engagement and fight evaluation.
use cambc as bc;
use cambc::Vec2;

use crate::ctx::Ctx;
use crate::nav::Nav;
use crate::util::{cheb, dpt_of, has_ready_cc, retaliation, stun_immune};

#[derive(Clone, Copy)]
pub enum Tgt<'a> {
    Foe(&'a bc::Champion, Vec2),
    Camp(&'a bc::Camp),
}

impl<'a> Tgt<'a> {
    pub fn pos(&self) -> Vec2 {
        match *self {
            Tgt::Foe(_, p) => p,
            Tgt::Camp(c) => c.spawn_position(), // monsters never move
        }
    }
    pub fn is_foe(&self) -> bool {
        matches!(self, Tgt::Foe(..))
    }
    /// Gem and gold clusters never fight back.
    pub fn hits_back(&self) -> bool {
        match *self {
            Tgt::Foe(..) => true,
            Tgt::Camp(c) => !(c.is_gem_cluster() || c.is_gold_cluster()),
        }
    }
    fn attack(&self, me: &bc::Champion) -> bc::Result<()> {
        match *self {
            Tgt::Foe(c, _) => me.attack(c),
            Tgt::Camp(c) => me.attack(c),
        }
    }
    fn cast(&self, a: &bc::Ability) -> bc::Result<bool> {
        match *self {
            Tgt::Foe(c, _) => a.cast_on(c),
            Tgt::Camp(c) => a.cast_on(c),
        }
    }
}

/// Cast at most one ability: CC, then damage, then shield (real fights only),
/// then an attack reset. Called after the basic attack so a reset is worth a hit.
pub fn cast_best(me: &bc::Champion, t: &Tgt) {
    if !me.can_cast().unwrap_or(false) {
        return;
    }
    let tp = t.pos();
    for want_cc in [true, false] {
        for a in me.abilities().iter() {
            if !a.is_ready().unwrap_or(false) {
                continue;
            }
            if a.is_attack_reset() || a.is_dash() || a.is_shield() || a.is_heal() {
                continue;
            }
            let hurts = a.is_damage() || a.is_aoe_damage() || a.damage_amount().unwrap_or(0.0) > 0.0;
            if want_cc != a.is_cc() || (!want_cc && !hurts) {
                continue;
            }
            if !t.is_foe() && !hurts {
                continue; // never waste a pure stun on a camp
            }
            let in_range = a
                .valid_tiles()
                .ok()
                .and_then(|b| b.is_set(tp).ok())
                .unwrap_or(false);
            if !in_range {
                continue;
            }
            let ok = if t.is_foe() && a.can_target_enemies().unwrap_or(false) {
                t.cast(a)
            } else if !t.is_foe() && a.can_target_camps().unwrap_or(false) {
                t.cast(a)
            } else if a.can_target_ground().unwrap_or(false) {
                a.cast_at(tp)
            } else {
                Ok(false)
            };
            if ok.unwrap_or(false) {
                return;
            }
        }
    }
    if t.hits_back() {
        for a in me.abilities().iter() {
            if a.is_shield() && a.is_ready().unwrap_or(false) && a.cast_untargeted().unwrap_or(false) {
                return;
            }
        }
    }
    for a in me.abilities().iter() {
        if a.is_attack_reset() && a.is_ready().unwrap_or(false) && a.cast_untargeted().unwrap_or(false) {
            return;
        }
    }
}

/// Close to attack range (ranged units hold maximum range), then attack + cast.
/// Ranged champions cannot move and attack in one turn; melee can.
pub fn engage(nav: &Nav, c: &Ctx, t: &Tgt) {
    let range = c.me.attack_range().unwrap_or(1);
    let melee = range <= 1;
    let tp = t.pos();
    let mut pos = c.pos;
    if cheb(pos, tp) > range {
        nav.to_range(c.me, pos, &c.occ, tp, 1, range);
        if !melee {
            return;
        }
        pos = c.me.position().unwrap_or(pos);
        if cheb(pos, tp) > range {
            return;
        }
    }
    if c.me.can_attack().unwrap_or(false) {
        let _ = t.attack(c.me);
    }
    cast_best(c.me, t);
    if c.me.can_attack().unwrap_or(false) {
        let _ = t.attack(c.me); // second hit after an attack reset
    }
}

/// > 1 means we expect to win the fight against `target` with `near` foes helping it.
/// Counts allies nearby, a ready stun on the target (it loses a turn; Marcella is
/// immune) and Ren-style retaliation damage.
pub fn fight_ratio(c: &Ctx, target: &bc::Champion, near: &[&(bc::Champion, Vec2, i32)]) -> f32 {
    let (sd, sh) = c.support(c.pos);
    let team_dpt = (c.my_dpt + sd).max(1.0);
    let foe_dpt: f32 = near.iter().map(|f| dpt_of(&f.0)).sum::<f32>().max(1.0);
    let reflect = retaliation(target) * 1.3; // ~1.3 damage instances per turn
    let t_kill = target.health().unwrap_or(1.0) / team_dpt;
    let mut t_die = (c.hp + sh) / (foe_dpt + reflect);
    if has_ready_cc(c.me) && !stun_immune(target) {
        t_die += dpt_of(target) / foe_dpt;
    }
    t_die / t_kill.max(0.5)
}
