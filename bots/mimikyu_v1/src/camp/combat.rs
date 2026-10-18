use crate::{
    camp::camp::{CampType, CampTypeExtension},
    champion::memory::{CampID, ChampionMemory},
};
use cambc::{Camp, ChampionState};

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
            .fold(0., |acc, &state: &&ChampionState| acc + state.champion_type);

        todo!()
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
