use cambc::{Bitboard, Champion, DistanceField, Vec2};

pub fn chebyshev_distance(first: Vec2, second: Vec2) -> u16 {
    (first.x - second.x).abs().max((first.y - second.y).abs()) as u16
}

pub fn step_with_fallback(field: &DistanceField, goal: Vec2) -> bool {
    let Ok(Some(me)): Result<Option<&Champion>, _> = cambc::self_champion() else {
        return false;
    };
    let Ok(position): Result<Vec2, _> = me.position() else {
        return false;
    };
    if !me.can_move().unwrap_or(false) {
        return false;
    }

    let directions: Vec<Vec2> = field.next_moves_from(position).unwrap_or_default();
    for direction in directions {
        if !me.can_move().unwrap_or(false) {
            return false;
        }
        if let Ok(true) = me.r#move(direction) {
            return true;
        }
    }

    matches!(me.move_towards(goal), Ok(true))
}

pub trait Center {
    fn center(&self) -> [u8; 2];
}

impl Center for Vec<[u8; 2]> {
    fn center(&self) -> [u8; 2] {
        let (sx, sy) = self
            .iter()
            .fold((0u32, 0u32), |(x, y), v| (x + v[0] as u32, y + v[1] as u32));
        let n = self.len().max(1) as u32;
        [(sx / n) as u8, (sy / n) as u8]
    }
}

impl Center for Bitboard {
    fn center(&self) -> [u8; 2] {
        self.positions()
            .unwrap()
            .iter()
            .map(|position: &Vec2| [position.x as u8, position.y as u8])
            .collect::<Vec<[u8; 2]>>()
            .center()
    }
}
