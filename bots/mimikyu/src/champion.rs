use cambc::Champion;

pub trait ChampionPlayExtension {
    fn play_turn();
}

impl ChampionPlayExtension for Champion {
    fn play_turn() {}
}
