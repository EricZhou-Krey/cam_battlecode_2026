use cambc::{ChampionDraft, Result, Team};

pub fn play() -> Result<()> {
    let chosen: Vec<ChampionDraft> = cambc::draft::current_draft(Team::Ally)?;
    let mut available = Vec::new();
    for i in 0..11 {
        let pick = ChampionDraft::try_from(i)?;
        if !chosen.contains(&pick) {
            available.push(pick);
        }
    }
    cambc::draft_memory().submit(b"hello world")?;
    cambc::draft::pick(available[crate::random_number(available.len() as u32) as usize])

