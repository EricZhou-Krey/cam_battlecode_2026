use cambc::{Ability, Champion, ChampionStats, ChampionType, Team};
use std::collections::HashMap;

use crate::memory::LocationID;

#[derive(Debug, Clone, PartialEq)]
pub struct ChampionInfo {
    pub current_hp: Option<f32>,
    pub current_cooldowns: [i32; 2],
    pub current_stats: [f32; 3],
    pub gems_held: Option<u32>,
    pub last_seen: Option<LocationID>,
    pub going_to: Option<LocationID>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AllChampionInfo {
    pub ally: HashMap<ChampionType, ChampionInfo>,
    pub enemy: HashMap<ChampionType, ChampionInfo>,

    pub updated: Vec<(Team, ChampionType)>,
}

impl Default for AllChampionInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl AllChampionInfo {
    pub fn new() -> Self {
        let init_champion_info = |team: Team| -> HashMap<ChampionType, ChampionInfo> {
            cambc::champions(Some(team))
                .unwrap()
                .iter()
                .map(|champion| {
                    let champion_type: ChampionType = champion.champion_name().unwrap();
                    let current_stats: [f32; 3] = [
                        champion.base_stats().unwrap().base_health,
                        champion.base_stats().unwrap().base_strength,
                        champion.base_stats().unwrap().base_focus,
                    ];
                    (
                        champion_type,
                        ChampionInfo {
                            current_hp: None,
                            current_cooldowns: [0; 2],
                            current_stats,
                            gems_held: None,
                            last_seen: None,
                            going_to: None,
                        },
                    )
                })
                .collect()
        };

        let ally: HashMap<ChampionType, ChampionInfo> = init_champion_info(Team::Ally);
        let enemy: HashMap<ChampionType, ChampionInfo> = init_champion_info(Team::Enemy);

        Self {
            ally,
            enemy,
            updated: Vec::new(),
        }
    }

    pub fn update_by_visible_champions(&mut self) {
        self.updated.clear();
        let ally_visible_champions: Vec<Champion> =
            cambc::vision::champions(Some(Team::Ally)).unwrap();
        let enemy_visible_champions: Vec<Champion> =
            cambc::vision::champions(Some(Team::Enemy)).unwrap();

        for ally_champion in ally_visible_champions {
            let ally_champion_type: ChampionType = ally_champion.champion_name().unwrap();
            self.ally.entry(ally_champion_type).and_modify(|ally_info| {
                ally_info.current_hp = Some(ally_champion.health().unwrap());
                ally_info.current_cooldowns = ally_champion
                    .abilities()
                    .clone()
                    .map(|ability: Ability| ability.cooldown().unwrap());
                // let base_states: ChampionStats = ally_champion.base_stats().unwrap();
                // ally_info.current_stats = ...
                ally_info.gems_held = Some(ally_champion.gem_count().unwrap());
                // ally_info.last_seen = ...
                // ally_info.going_to = ...
            });
        }
    }
}
