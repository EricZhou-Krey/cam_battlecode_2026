#[derive(Debug, Clone, PartialEq)]
pub struct ChampionInfo {
    pub current_hp: Option<f32>,
    pub current_cooldown: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AllChampionInfo {
    // Indexed by ChampionType as usize
    pub ally: Vec<ChampionInfo>,
    pub enemy: Vec<ChampionInfo>,
}

impl AllChampionInfo {
    pub fn new() -> Self {
        Self {
            ally: Vec::new(),
            enemy: Vec::new(),
        }
    }
}
