use crate::memory::{
    champion_info::{AllChampionInfo, ChampionInfo},
    champion_memory::ChampionMemory,
    CampID, StatID,
};
use cambc::{CampType, ChampionType};

#[repr(usize)]
#[derive(Debug, Clone, PartialEq, Eq, Copy, Hash)]
pub enum CampTypeAttackOrder {
    Monolith,
    Golem,
    Elk,
    Wolf,
    GemCluster,
    GoldCluster,
}

impl From<CampType> for CampTypeAttackOrder {
    fn from(value: CampType) -> Self {
        match value {
            CampType::Elk => Self::Elk,
            CampType::Wolf => Self::Wolf,
            CampType::Golem => Self::Golem,
            CampType::Monolith => Self::Monolith,
            CampType::GemCluster => Self::GemCluster,
            CampType::GoldCluster => Self::GoldCluster,
        }
    }
}

pub trait MemoryChampionIDToInfoExtension {
    fn ally_champion_infos<'a>(
        &'a self,
        champion_types: &'a [ChampionType],
    ) -> impl Iterator<Item = &'a ChampionInfo> + 'a;

    fn enemy_champion_infos<'a>(
        &'a self,
        champion_types: &'a [ChampionType],
    ) -> impl Iterator<Item = &'a ChampionInfo> + 'a;
}

impl MemoryChampionIDToInfoExtension for AllChampionInfo {
    fn ally_champion_infos<'a>(
        &'a self,
        champion_types: &'a [ChampionType],
    ) -> impl Iterator<Item = &'a ChampionInfo> + 'a {
        champion_types
            .iter()
            .filter_map(move |champion_type| self.ally.get(champion_type))
    }

    fn enemy_champion_infos<'a>(
        &'a self,
        champion_types: &'a [ChampionType],
    ) -> impl Iterator<Item = &'a ChampionInfo> + 'a {
        champion_types
            .iter()
            .filter_map(move |champion_type| self.enemy.get(champion_type))
    }
}

pub trait ChampionInfoCompositeExtension {
    fn current_stat(self, stat: StatID) -> f32;
}

impl<'a, I> ChampionInfoCompositeExtension for I
where
    I: Iterator<Item = &'a ChampionInfo>,
{
    fn current_stat(self, stat: StatID) -> f32 {
        self.fold(0.0, |acc, champion_info| {
            acc + champion_info.current_stats[stat as usize]
        })
    }
}

pub trait MemoryCombatEstimator {
    fn can_kill_camp(&self, camp_id: CampID, ally_champions: &[ChampionType]) -> bool;
    fn is_good_combat(
        &self,
        enemy_champion: &[ChampionType],
        ally_champion: &[ChampionType],
    ) -> bool;
}

// PRECACHED - by inspection
const AVERAGE_FOCUS_SCALE: f32 = 14.55 / 10.;
const AVERAGE_INDICATOR_FOR_DAMAGE_ABILITY: f32 = 10. / 26.;
const N_ABLITIES_PER_CHAMPION: f32 = 2.;

const ABILITY_DAMAGE_INTERVAL: f32 = 8.0;
const FOCUS_DAMAGE_SCALE: f32 =
    AVERAGE_FOCUS_SCALE * AVERAGE_INDICATOR_FOR_DAMAGE_ABILITY * N_ABLITIES_PER_CHAMPION
        / ABILITY_DAMAGE_INTERVAL;

impl MemoryCombatEstimator for ChampionMemory {
    fn can_kill_camp(&self, camp_id: CampID, ally_champions: &[ChampionType]) -> bool {
        let camp_health: f32 = self.map.camp_group_effective_hp[camp_id.0];
        let camp_damage: f32 = self.map.camp_group_damage_per_round[camp_id.0];
        let champion_health: f32 = self
            .champion_info
            .ally_champion_infos(ally_champions)
            .current_stat(StatID::Health);
        let champion_damage: f32 = self
            .champion_info
            .ally_champion_infos(ally_champions)
            .current_stat(StatID::Strength);

        let round_count: f32 = camp_health / champion_damage;

        champion_health > round_count * camp_damage
    }

    fn is_good_combat(
        &self,
        enemy_champion: &[ChampionType],
        ally_champion: &[ChampionType],
    ) -> bool {
        let [enemy_health, enemy_strength, enemy_focus]: [f32; 3] =
            [StatID::Health, StatID::Strength, StatID::Focus].map(|stat_id| {
                self.champion_info
                    .enemy_champion_infos(enemy_champion)
                    .current_stat(stat_id)
            });
        let enemy_damage: f32 = enemy_strength + enemy_focus * FOCUS_DAMAGE_SCALE;

        let [ally_health, ally_strength, ally_focus]: [f32; 3] =
            [StatID::Health, StatID::Strength, StatID::Focus].map(|stat_id| {
                self.champion_info
                    .ally_champion_infos(ally_champion)
                    .current_stat(stat_id)
            });
        let ally_damage: f32 = ally_strength + ally_focus * FOCUS_DAMAGE_SCALE;

        ally_health / enemy_damage > enemy_health / ally_damage
    }
}
