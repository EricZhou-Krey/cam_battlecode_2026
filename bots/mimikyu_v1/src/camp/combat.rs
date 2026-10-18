use crate::{
    camp::camp::{CampType, CampTypeExtension},
    champion::memory::{CampID, ChampionMemory, ChampionState},
};
use cambc::{Camp, ChampionType};

pub const DANGER_HP_SCALE: f32 = 1350.;
pub const DANGER_DMG_SCALE: f32 = 20.;

pub trait CampCombatPredictionExtension {
    fn damage_per_round(&self, camp_id: CampID) -> f32;
    fn effective_health(&self, camp_id: CampID) -> f32;
    fn basic_combat_health_delta(&self, camp_id: CampID, champions: &[&ChampionState]) -> f32;
}

impl CampCombatPredictionExtension for ChampionMemory {
    fn damage_per_round(&self, camp_id: CampID) -> f32 {
        cambc::map::camp_groups().unwrap()[camp_id.0]
            .camps()
            .iter()
            .fold(0., |acc, camp| {
                acc + camp.base_stats().unwrap().base_strength
            })
    }

    fn effective_health(&self, camp_id: CampID) -> f32 {
        cambc::map::camp_groups().unwrap()[camp_id.0]
            .camps()
            .iter()
            .enumerate()
            .fold(0., |acc, (camp_index, camp)| {
                acc + if matches!(
                    self.camp_types[camp_id.0].types[camp_index],
                    CampType::GemCluster | CampType::GoldCluster
                ) {
                    0.
                } else {
                    camp.base_stats().unwrap().base_health
                }
            })
    }

    fn basic_combat_health_delta(&self, camp_id: CampID, champions: &[&ChampionState]) -> f32 {
        let champion_effective_health: f32 = champions
            .iter()
            .fold(0., |acc, state: &&ChampionState| acc + state.current_health);

        let champion_damage_per_round: f32 =
            champions
                .iter()
                .fold(0., |mut acc, state: &&ChampionState| {
                    let champion_type: ChampionType = state.champion_type;
                    acc += self.base_champion_stats[champion_type as usize].base_strength
                        + self.base_champion_stats[champion_type as usize].strength_growth
                            * state.stat_bonus.map_or(0., |s| s[0]);
                    acc
                });

        let camp_damage_per_round: f32 = self.camp_danager_levels[camp_id.0] * DANGER_DMG_SCALE;
        let mut champion_current_health: f32 = champion_effective_health;
        let mut camp_current_health: f32 = self.camp_danager_levels[camp_id.0] * DANGER_HP_SCALE;

        let rounds = (champion_current_health / camp_damage_per_round)
            .ceil()
            .max((camp_current_health / champion_damage_per_round).ceil());

        champion_current_health -= rounds * camp_damage_per_round;
        camp_current_health -= rounds * champion_damage_per_round;

        champion_current_health - camp_current_health
    }
}

pub trait TargetPriorityExtension {
    fn target_priority(&self) -> i32;
}

impl TargetPriorityExtension for Camp {
    fn target_priority(&self) -> i32 {
        // Lower means high priority
        match self.camp_type() {
            CampType::Monolith => 0,
            CampType::Elk => 1,
            CampType::Wolf => 2,
            CampType::GoldCluster | CampType::GemCluster => 3,
        }
    }
}
