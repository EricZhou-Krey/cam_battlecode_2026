use bot::champion::ChampionPlayExtension;
use bot::draft_strategy;
use cambc::{self_champion, Result};

fn main() -> Result<()> {
    loop {
        if cambc::is_draft() {
            if let Err(err) = draft_strategy::play() {
                println!("bot error: {error}");
            }
            cambc::end_turn();
            continue;
        }

        match self_champion() {
            Ok(Some(champion)) => champion.play_turn(),
            Ok(None) => unreachable!(),
            Err(err) => println!("bot error: {err}"),
        }
        cambc::end_turn();
    }
}
