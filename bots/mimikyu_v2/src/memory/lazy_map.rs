use crate::{draft::draft_memory::DraftMemory, memory::CampGroupID};
use cambc::DistanceField;
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq)]
pub struct LazyCampGroupDistanceFields {
    camp_group_distance_fields: Vec<Option<DistanceField>>,
}

impl LazyCampGroupDistanceFields {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            camp_group_distance_fields: vec![None; capacity],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LazyMap {
    discovery_queue: VecDeque<CampGroupID>,
    // Vectors indexed by CampGroupID.0
    pub camp_group_distance_fields: LazyCampGroupDistanceFields,
    pub camp_group_effective_hp: Vec<f32>,
    pub camp_group_damage_per_round: Vec<f32>,
    pub camp_group_gem_value: Vec<u32>,
    pub camp_group_gold_value: Vec<u32>,
    pub camp_group_count: usize,
    discovered_this_round: bool,
}

impl LazyMap {
    pub fn new(draft_memory: &DraftMemory) -> Self {
        Self {
            discovery_queue: draft_memory
                .camp_memory
                .camp_group_discovery_order
                .into_iter()
                .map(|byte| CampGroupID(byte as usize))
                .collect(),
            camp_group_distance_fields: LazyCampGroupDistanceFields::with_capacity(
                draft_memory.camp_memory.camp_group_count as usize,
            ),
            camp_group_effective_hp: draft_memory
                .camp_memory
                .camp_group_effective_hp
                .into_iter()
                .map(|u16_bytes| u16_bytes as f32)
                .collect(),
            camp_group_damage_per_round: draft_memory
                .camp_memory
                .camp_group_damage_per_round
                .into_iter()
                .map(|u8| u8_bytes as f32)
                .collect(),
            camp_group_gem_value: draft_memory.,
            camp_group_gold_value ,
            camp_group_count: draft_memory.camp_memory.camp_group_count as usize,
            discovered_this_round: false,
        }
    }

    pub fn update_discovery(&mut self) {
        if self.discovered_this_round {
            self.discovered_this_round = !self.discovered_this_round;
            return;
        }
    }
}
