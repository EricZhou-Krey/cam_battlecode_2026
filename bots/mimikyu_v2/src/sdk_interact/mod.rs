use cambc::{Champion, ChampionType};

pub mod camp;
pub mod map;

pub fn me() -> &'static Champion {
    cambc::self_champion().unwrap().unwrap()
}

pub trait ChampionUnwrapperExtension {
    fn champion_type(&self) -> ChampionType;
}
impl ChampionUnwrapperExtension for Champion {
    fn champion_type(&self) -> ChampionType {
        self.champion_name().unwrap()
    }
}

pub fn round_number() -> u32 {
    cambc::round_number().unwrap()
}

pub fn team_size() -> usize {
    cambc::draft::team_size().unwrap()
}
