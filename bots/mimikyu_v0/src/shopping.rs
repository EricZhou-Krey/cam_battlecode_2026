//! Item builds. ItemType values are laid out as family*6 + stat*2 + (emblem?1:0)
//! with families Monster, Power, Durability, Treasure, Jungle, Home, Bruiser and
//! stats Strength, Focus, Health, so builds are just (family, stat) pairs.
use cambc as bc;

pub const MONSTER: u32 = 0;
pub const POWER: u32 = 1;
pub const DURABILITY: u32 = 2;
pub const TREASURE: u32 = 3;
pub const JUNGLE: u32 = 4;
const STR: u32 = 0;
const FOC: u32 = 1;
const HP: u32 = 2;

/// Emblem order per champion. Jungle (-30% camp damage) first for bruisers so
/// they can clear camps safely, Monster (+10% camp damage) next, Treasure (+20% gold)
/// to snowball, Durability, then Power (counts as 6 shards) forever after.
fn build_for(t: bc::ChampionType) -> &'static [(u32, u32)] {
    use bc::ChampionType as C;
    match t {
        C::Varg | C::Marcella | C::Ryth | C::Ren => &[
            (JUNGLE, HP),
            (MONSTER, STR),
            (TREASURE, HP),
            (DURABILITY, HP),
            (POWER, STR),
        ],
        C::Cassi | C::Stellar | C::Prism => &[
            (MONSTER, FOC),
            (TREASURE, FOC),
            (JUNGLE, HP),
            (DURABILITY, HP),
            (POWER, FOC),
        ],
        C::Tavi | C::Issac | C::Vendetta | C::VesperVale | C::Willow => &[
            (MONSTER, STR),
            (JUNGLE, HP),
            (TREASURE, STR),
            (DURABILITY, HP),
            (POWER, STR),
        ],
    }
}

pub fn item(family: u32, stat: u32, emblem: bool) -> Option<bc::ItemType> {
    bc::ItemType::try_from(family * 6 + stat * 2 + emblem as u32).ok()
}

/// Do we own a finished emblem of this family (any stat)?
pub fn has_emblem(counts: &bc::ItemCounts, family: u32) -> bool {
    (0..3).any(|s| {
        item(family, s, true)
            .map(|e| counts[e] > 0)
            .unwrap_or(false)
    })
}

/// Buy the next shard of the first unfinished emblem. The engine rejects purchases
/// away from the spawn (except Tavi's shards), so calling this every turn is safe.
pub fn shop(me: &bc::Champion, t: bc::ChampionType) -> bc::Result<()> {
    let order = build_for(t);
    for _ in 0..12 {
        if me.gold_count()? < 30 {
            break;
        }
        let counts = me.item_count()?;
        let (f, s) = order
            .iter()
            .copied()
            .find(|&(f, s)| item(f, s, true).map(|e| counts[e] == 0).unwrap_or(false))
            .unwrap_or((POWER, order[0].1));
        let Some(it) = item(f, s, false) else { break };
        if !me.can_buy(it, 1)? {
            break;
        }
        me.buy(it)?;
    }
    // One altar lock (10g) once the first emblem is done: it cancels and blocks enemy tributes.
    if let Some(first) = item(order[0].0, order[0].1, true) {
        let lock = bc::ConsumableType::AltarLock;
        if me.item_count()?[first] > 0
            && me.consumables()[lock].count().unwrap_or(1) == 0
            && me.can_buy_consumable(lock).unwrap_or(false)
        {
            let _ = me.buy_consumable(lock);
        }
    }
    Ok(())
}
