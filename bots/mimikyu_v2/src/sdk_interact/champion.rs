use cambc::{Champion, Error, ItemCounts, ItemPassive};
use std::mem::discriminant;

pub fn passive_active(passive: ItemPassive, me: &Champion) -> bool {
    let Ok(counts): Result<ItemCounts, Error> = me.item_count() else {
        return false;
    };
    let active: bool = counts.iter().any(|(item, n)| {
        n > 0 && matches!(item.passive(), Ok(Some(p)) if discriminant(&p) == discriminant(&passive))
    });
    active
}
