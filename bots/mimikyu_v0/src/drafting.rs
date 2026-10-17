//! Draft phase: pick champions and write the lane plan into draft memory.
//!
//! Pick score = W_WIN * duel win rate + W_DPS * normalised stun/reflect-adjusted
//! damage per turn + W_COUNTER * average duel margin against the enemy picks we
//! can already see + a small utility bonus. The statistics come from data.rs,
//! generated from analysis.py (see tools/gen_data.py).
use cambc as bc;

use crate::data;
use crate::plan::{self, St};
use crate::util::{centroid, cheb, draft_key, draft_variant};

const W_WIN: f32 = 0.5;
const W_DPS: f32 = 0.3;
const W_COUNTER: f32 = 0.4;
const RANGED: [&str; 6] = ["cassi", "vesper", "prism", "stellar", "tavi", "willow"];

/// Things the duel simulation cannot see: Tavi shops from anywhere, Varg splashes
/// whole camp groups, Cassi & Issac are two bodies for one pick.
fn utility(key: &str) -> f32 {
    match key {
        "tavi" => 0.12,
        "varg" | "cassi" => 0.05,
        _ => 0.0,
    }
}

fn score(key: &str, mine: &[&str], theirs: &[&str], size: usize) -> Option<f32> {
    let i = data::index(key)?;
    let lo = data::ADJ_DPT.iter().cloned().fold(f32::MAX, f32::min);
    let hi = data::ADJ_DPT.iter().cloned().fold(f32::MIN, f32::max);
    let dps = (data::ADJ_DPT[i] - lo) / (hi - lo).max(1.0);
    let seen: Vec<usize> = theirs.iter().filter_map(|k| data::index(k)).collect();
    let counter = if seen.is_empty() {
        0.0
    } else {
        seen.iter().map(|&j| data::MARGIN[i][j]).sum::<f32>() / seen.len() as f32
    };
    let mut s = W_WIN * data::WIN_RATE[i] + W_DPS * dps + W_COUNTER * counter + utility(key);
    // never finish the draft without a ranged unit
    if mine.len() + 1 == size && !mine.iter().any(|k| RANGED.contains(k)) && !RANGED.contains(&key)
    {
        s -= 0.5;
    }
    Some(s)
}

struct G {
    x: i32,
    y: i32,
    t: f32,
    s: f32,
    hp: i32,
    dmg: i32,
    gems: u32,
    gold: u32,
    resp: i32,
    rad: i32,
    lane: u8,
}

fn camp_hp_dmg(c: &bc::Camp) -> (i32, i32) {
    match c.base_stats() {
        Ok(s) => (s.base_health as i32, s.base_strength as i32),
        Err(_) => {
            if c.is_wolf() {
                (1350, 20)
            } else if c.is_elk() {
                (2400, 60)
            } else if c.is_monolith() {
                (20000, 200)
            } else {
                (500, 0)
            }
        }
    }
}

/// Project camp groups onto the spawn->spawn axis, split them into one lane per
/// champion by sideways offset, order each lane from our spawn to theirs, and give
/// the richest lanes to the strongest (earliest-picked) champions.
fn build_memory(picks: &[&'static str], size: usize) -> bc::Result<Vec<u8>> {
    let a = centroid(bc::map::spawn_region(bc::Team::Ally)?)?;
    let e = centroid(bc::map::spawn_region(bc::Team::Enemy)?)?;
    let (ax, ay) = ((e.x - a.x) as f32, (e.y - a.y) as f32);
    let len2 = (ax * ax + ay * ay).max(1.0);
    let len = len2.sqrt();

    let mut gs: Vec<G> = Vec::new();
    for g in bc::map::camp_groups()? {
        let camps = g.camps();
        if camps.is_empty() {
            continue;
        }
        let p0 = camps[0].spawn_position();
        let (mut hp, mut dmg, mut rad) = (0, 0, 0);
        for c in camps {
            let (h, d) = camp_hp_dmg(c);
            hp += h;
            dmg += d;
            rad = rad.max(cheb(c.spawn_position(), p0));
        }
        let (dx, dy) = ((p0.x - a.x) as f32, (p0.y - a.y) as f32);
        gs.push(G {
            x: p0.x,
            y: p0.y,
            t: (dx * ax + dy * ay) / len2,
            s: (dx * ay - dy * ax) / len,
            hp,
            dmg,
            gems: g.gem_value(),
            gold: g.gold_value(),
            resp: g.max_respawn_timer(),
            rad,
            lane: 0,
        });
    }
    gs.sort_by(|p, q| (q.gems * 2 + q.gold / 2).cmp(&(p.gems * 2 + p.gold / 2)));
    gs.truncate(plan::MAX_ENT);

    let k = size.clamp(2, 5);
    gs.sort_by(|p, q| p.s.partial_cmp(&q.s).unwrap_or(std::cmp::Ordering::Equal));
    let n = gs.len().max(1);
    for (i, g) in gs.iter_mut().enumerate() {
        g.lane = (i * k / n).min(k - 1) as u8;
    }
    let mut lane_val = vec![0u32; k];
    for g in &gs {
        lane_val[g.lane as usize] += g.gems * 2 + g.gold / 2;
    }
    let mut by_val: Vec<usize> = (0..k).collect();
    by_val.sort_by(|&p, &q| lane_val[q].cmp(&lane_val[p]));
    gs.sort_by(|p, q| {
        p.lane
            .cmp(&q.lane)
            .then(p.t.partial_cmp(&q.t).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut m = vec![0u8; plan::HDR];
    m[0] = plan::MAGIC;
    m[1] = k as u8;
    m[2] = gs.len() as u8;
    m[3] = size as u8;
    for (i, key) in picks.iter().take(5).enumerate() {
        m[4 + i] = St::role_for(key) as u8;
        m[9 + i] = by_val[i % k] as u8;
    }
    for g in &gs {
        m.extend_from_slice(&[
            g.x as u8,
            g.y as u8,
            g.lane,
            (g.hp / 100).min(255) as u8,
            g.dmg.min(255) as u8,
            (g.gems / 2).min(255) as u8,
            (g.gold / 3).min(255) as u8,
            (g.resp / 2).min(255) as u8,
            g.rad.min(255) as u8,
        ]);
    }
    m.truncate(1024);
    Ok(m)
}

fn update_memory(mem: &mut [u8], picks: &[&'static str]) {
    if mem.len() < plan::HDR || mem[0] != plan::MAGIC {
        return;
    }

    let k = (mem[1] as usize).clamp(2, 5);

    for i in 0..5 {
        mem[4 + i] = picks.get(i).map(|key| St::role_for(key) as u8).unwrap_or(0);

        if i >= picks.len() {
            mem[9 + i] = (i % k) as u8;
        }
    }
}

/// One draft turn = one pick (a turn that makes no pick gets a random one).
pub fn turn(mem: &mut Vec<u8>) -> bc::Result<()> {
    let size = bc::draft::team_size()?;

    let mine: Vec<&'static str> = bc::draft::current_draft(bc::Team::Ally)?
        .into_iter()
        .map(draft_key)
        .collect();

    let theirs: Vec<&'static str> = bc::draft::current_draft(bc::Team::Enemy)
        .unwrap_or_default()
        .into_iter()
        .map(draft_key)
        .collect();

    if mem.len() < plan::HDR || mem[0] != plan::MAGIC {
        *mem = build_memory(&mine, size)?;
    }

    update_memory(mem, &mine);

    if mine.len() >= size - 1 {
        let _ = bc::draft_memory().submit(mem);
    }

    let mut ranked: Vec<(f32, &'static str)> = data::KEYS
        .iter()
        .copied()
        .filter(|k| !mine.contains(k))
        .filter_map(|k| score(k, &mine, &theirs, size).map(|s| (s, k)))
        .collect();

    ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    for (_, key) in ranked {
        if let Some(v) = draft_variant(key) {
            if bc::draft::pick(v).is_ok() {
                break;
            }
        }
    }

    Ok(())
}
