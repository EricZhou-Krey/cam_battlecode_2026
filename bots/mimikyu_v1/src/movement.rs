use cambc::{Champion, DistanceField, Vec2};

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
