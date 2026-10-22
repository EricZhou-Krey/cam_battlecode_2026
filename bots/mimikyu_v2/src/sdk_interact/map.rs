use cambc::{Bitboard, Champion, DistanceField, Vec2};

pub fn step_with_fallback(me: &Champion, field: &DistanceField, goal: Vec2) -> bool {
    let pos = me.position().unwrap();

    for dir in field.next_moves_from(pos).unwrap() {
        if !me.can_move().unwrap() {
            return false;
        }
        match me.r#move(dir) {
            Ok(true) => return true,
            Ok(false) => continue,
            Err(_e) => continue,
        }
    }

    if me.can_move().unwrap() {
        if let Ok(true) = me.move_towards(goal) {
            return true;
        }
    }
    false
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
