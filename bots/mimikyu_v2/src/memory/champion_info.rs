use cambc::{Champion, ChampionStats, ChampionType, Team, Vec2};
use std::collections::HashMap;

use crate::memory::{lazy_map::LazyMap, LocationID};

#[derive(Debug, Clone, PartialEq)]
pub struct ChampionInfo {
    pub current_hp: Option<f32>,
    pub current_cooldowns: [i32; 2],
    pub current_stats: [f32; 3],
    pub gems_held: Option<u32>,
    pub last_seen_near: Option<LocationID>,
    pub last_seen: Option<Vec2>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AllChampionInfo {
    pub ally: HashMap<ChampionType, ChampionInfo>,
    pub enemy: HashMap<ChampionType, ChampionInfo>,

    pub enemy_updated_this_turn: Vec<ChampionType>,
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
                            last_seen_near: None,
                            last_seen: None,
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
            enemy_updated_this_turn: Vec::new(),
        }
    }
}

pub trait ChampionInfoCalculationExtension {
    fn current_stats(&self) -> [f32; 3];
    fn try_near_location(&self, map: &mut LazyMap) -> Option<LocationID>;
}

impl ChampionInfoCalculationExtension for Champion {
    fn current_stats(&self) -> [f32; 3] {
        [
            self.max_health().unwrap(),
            self.strength().unwrap(),
            self.focus().unwrap(),
        ]
    }
    fn try_near_location(&self, map: &mut LazyMap) -> Option<LocationID> {
        let champion_location: Vec2 = self.position().unwrap();
        map.points_of_interest
            .try_get_closest_poi(champion_location, &mut map.distance_fields)
    }
}
