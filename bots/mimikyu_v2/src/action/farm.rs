use cambc::{CampGroupCaptureState, CampGroupCaptureType, Champion, ChampionType, ItemPassive};

use crate::{
    action::{
        movement::{RECALL_TIME, RECALL_TIME_HOME},
        Action, WARNING_HP_THRESHOLD,
    },
    memory::{champion_memory::ChampionMemory, AltarID, CampGroupID, LocationID},
    sdk_interact::champion::passive_active,
};

pub const COLLECTION_TIME: u16 = 5;

#[derive(Debug, Clone, PartialEq)]
pub struct CampGroupCombatState {
    pub camp_group_id: CampGroupID,
    pub current_target_camp: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CollectionState {
    camp_group_id: CampGroupID,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TributeState {
    altar_id: AltarID,
}

pub trait FarmActions {
    fn camp_group_combat(&mut self, champion_memory: &mut ChampionMemory);
    fn collection(&mut self, champion_memory: &mut ChampionMemory);
    fn tributing_at_altar(&mut self, champion_memory: &mut ChampionMemory);
}

impl FarmActions for Action {
    fn camp_group_combat(&mut self, champion_memory: &mut ChampionMemory) {
        let me: &Champion = cambc::self_champion().unwrap().unwrap();

        match me.champion_name().unwrap() {
            ChampionType::Willow => (),
            ChampionType::HeartOfRuin => (),
            ChampionType::Varg => (),
            ChampionType::Ren => (),
            ChampionType::Vendetta => (),
            _ => (),
        }
    }

    fn collection(&mut self, champion_memory: &mut ChampionMemory) {
        let Self::CollectingFromCamp(state) = self else {
            println!("Error: State Action Mismatch");
            return;
        };
        let me: &Champion = cambc::self_champion().unwrap().unwrap();
        let current_hp_percentage: f32 = me.health().unwrap() / me.max_health().unwrap();
        let recall_time: u16 = if passive_active(ItemPassive::Home, me) {
            RECALL_TIME_HOME
        } else {
            RECALL_TIME
        };
        let capture_state: CampGroupCaptureState = cambc::map::camp_groups().unwrap()
            [state.camp_group_id.0]
            .capture_state()
            .unwrap();

        if capture_state.state != CampGroupCaptureType::AllyCapturing {
            *self = Self::Idle;
            self.turn(champion_memory);
            return;
        }
        if current_hp_percentage <= WARNING_HP_THRESHOLD
            && capture_state.timer as u16 == recall_time
        {
            me.recall();
        }
    }

    fn tributing_at_altar(&mut self, champion_memory: &mut ChampionMemory) {
        let Self::TributingAtAltar(state) = self else {
            println!("Error: State Action Mismatch");
            return;
        };
        let me: &Champion = cambc::self_champion().unwrap().unwrap();

        if me.is_submitting_gems().unwrap() {
            return;
        }

        if me.gem_count().unwrap() == 0 {
            *self = Self::Idle;
            self.turn(champion_memory);
            return;
        }

        let _ = me.submit_gems(
            champion_memory
                .map
                .points_of_interest
                .locate(LocationID::Altar(state.altar_id)),
        );
    }
}
