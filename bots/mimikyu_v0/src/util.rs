//! Small shared helpers: distances, damage estimates, champion keys.
use cambc as bc;
use cambc::Vec2;

/// Every ability slot has an 8-turn cooldown (see the rules).
pub const CD: f32 = 8.0;

pub fn cheb(a: Vec2, b: Vec2) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

/// Average position of a tile mask (used for spawn regions).
pub fn centroid(b: &bc::Bitboard) -> bc::Result<Vec2> {
    let ps = b.positions()?;
    if ps.is_empty() {
        return Ok(Vec2::new(bc::map::width()? / 2, bc::map::height()? / 2));
    }
    let n = ps.len() as i32;
    Ok(Vec2::new(
        ps.iter().map(|p| p.x).sum::<i32>() / n,
        ps.iter().map(|p| p.y).sum::<i32>() / n,
    ))
}

/// Sustained damage per turn: basic attack plus every damaging ability / cooldown.
/// Pure stuns (no damage) contribute nothing here; see `combat::fight_ratio` for stun value.
pub fn dpt_of(c: &bc::Champion) -> f32 {
    let mut d = c.strength().unwrap_or(0.0);
    for a in c.abilities().iter() {
        if a.is_damage() || a.is_aoe_damage() || a.is_cc() {
            d += a.damage_amount().unwrap_or(0.0) / CD;
        }
    }
    d.max(1.0)
}

pub fn stun_immune(c: &bc::Champion) -> bool {
    matches!(c.passive(), Ok(bc::PassiveType::StunImmunity))
}

/// Retaliation damage per hit taken (Ren): 0.2 x Focus.
pub fn retaliation(c: &bc::Champion) -> f32 {
    if matches!(c.passive(), Ok(bc::PassiveType::Retaliation)) {
        bc::Constants::REN_RETALIATION_FOCUS_RATIO * c.focus().unwrap_or(0.0)
    } else {
        0.0
    }
}

pub fn has_ready_cc(c: &bc::Champion) -> bool {
    c.abilities()
        .iter()
        .any(|a| a.is_cc() && a.is_ready().unwrap_or(false))
}

/// Lower-case key used by data.rs and the draft memory ("cassi" covers Cassi AND Issac).
pub fn champion_key(t: bc::ChampionType) -> &'static str {
    use bc::ChampionType as T;
    match t {
        T::Ryth => "ryth",
        T::VesperVale => "vesper",
        T::Vendetta => "vendetta",
        T::Cassi | T::Issac => "cassi",
        T::Stellar => "stellar",
        T::Prism => "prism",
        T::Marcella => "marcella",
        T::Willow => "willow",
        T::Ren => "ren",
        T::Varg => "varg",
        T::Tavi => "tavi",
    }
}

pub fn draft_key(d: bc::ChampionDraft) -> &'static str {
    use bc::ChampionDraft as D;
    match d {
        D::Ryth => "ryth",
        D::VesperVale => "vesper",
        D::Vendetta => "vendetta",
        D::CassiAndIssac => "cassi",
        D::Stellar => "stellar",
        D::Prism => "prism",
        D::Marcella => "marcella",
        D::Willow => "willow",
        D::Ren => "ren",
        D::Varg => "varg",
        D::Tavi => "tavi",
    }
}

pub fn draft_variant(key: &str) -> Option<bc::ChampionDraft> {
    use bc::ChampionDraft as D;
    Some(match key {
        "ryth" => D::Ryth,
        "vesper" => D::VesperVale,
        "vendetta" => D::Vendetta,
        "cassi" => D::CassiAndIssac,
        "stellar" => D::Stellar,
        "prism" => D::Prism,
        "marcella" => D::Marcella,
        "willow" => D::Willow,
        "ren" => D::Ren,
        "varg" => D::Varg,
        "tavi" => D::Tavi,
        _ => return None,
    })
}
