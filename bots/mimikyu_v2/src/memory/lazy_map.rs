use crate::{
    draft::draft_memory::DraftMemory,
    memory::{AltarID, CampGroupID, ControlPointID, LocationID, ShortcutID},
    sdk_interact::map::{chebyshev_distance, Center},
};
use cambc::{Bitboard, DistanceField, Vec2};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, PartialEq)]
pub struct LazyDistanceFields {
    pub camp_group_discovery_queue: VecDeque<CampGroupID>,
    pub camp_group_distance_fields: Vec<Option<DistanceField>>,
    pub altar_distance_field: Option<DistanceField>,
    pub discovered_this_round: bool,
    pub blocked_bitboard: Bitboard,
}

impl LazyDistanceFields {
    pub fn new(draft_memory: &DraftMemory) -> Self {
        let camp_group_discovery_queue: VecDeque<CampGroupID> = draft_memory
            .camp_memory
            .camp_group_discovery_order
            .into_iter()
            .take(draft_memory.camp_memory.camp_group_count as usize)
            .map(|byte| CampGroupID(byte as usize))
            .collect();
        let camp_group_distance_fields: Vec<Option<DistanceField>> =
            vec![None; draft_memory.camp_memory.camp_group_count as usize];
        let blocked_bitboard: Bitboard = cambc::map::walls()
            .unwrap()
            .union(cambc::map::holes().unwrap())
            .unwrap()
            .union(cambc::map::altars().unwrap())
            .unwrap();

        Self {
            camp_group_discovery_queue,
            camp_group_distance_fields,
            altar_distance_field: None,
            discovered_this_round: false,
            blocked_bitboard,
        }
    }

    pub fn reset_discovered_state(&mut self) {
        self.discovered_this_round = false;
    }

    pub fn lazy_discover_from_queue(&mut self) {
        if self.discovered_this_round {
            return;
        } else if self.camp_group_discovery_queue.is_empty() {
            if self.altar_distance_field.is_none() {
                self.try_get_altar_distance_matrix();
                return;
            } else {
                return;
            }
        }

        let mut next_camp_group_id: Option<CampGroupID> =
            self.camp_group_discovery_queue.pop_front();
        while next_camp_group_id.is_some()
            && self.camp_group_distance_fields[next_camp_group_id.unwrap().0].is_some()
        {
            next_camp_group_id = self.camp_group_discovery_queue.pop_front();
        }

        if let Some(id) = next_camp_group_id {
            self.try_get(id);
        }
    }

    pub fn try_get(&mut self, camp_group_id: CampGroupID) -> Option<&DistanceField> {
        if self.discovered_this_round || self.camp_group_distance_fields[camp_group_id.0].is_some()
        {
            return self.camp_group_distance_fields[camp_group_id.0].as_ref();
        }

        let mut distance_field: DistanceField = DistanceField::new().unwrap();
        let _ = distance_field.compute(
            &self.blocked_bitboard,
            &cambc::map::camp_groups().unwrap()[camp_group_id.0]
                .spawn_tiles()
                .unwrap(),
            &mut Bitboard::new().unwrap(),
            u16::MAX,
        );

        self.camp_group_distance_fields[camp_group_id.0] = Some(distance_field);
        self.discovered_this_round = true;
        self.camp_group_distance_fields[camp_group_id.0].as_ref()
    }

    pub fn try_get_altar_distance_matrix(&mut self) -> Option<&DistanceField> {
        if self.discovered_this_round || self.altar_distance_field.is_some() {
            return self.altar_distance_field.as_ref();
        }

        let mut distance_field: DistanceField = DistanceField::new().unwrap();
        let _ = distance_field.compute(
            &self.blocked_bitboard,
            cambc::map::altars().unwrap(),
            &mut Bitboard::new().unwrap(),
            u16::MAX,
        );

        self.altar_distance_field = Some(distance_field);
        self.altar_distance_field.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LazyPointsOfInterest {
    // Indexed by CampGroupID
    pub camp_group_centers: Vec<Vec2>,
    // Indexed by AltarID.0
    pub altar_locations: Vec<Vec2>,
    // Indexed by ControlPointID.0
    pub control_points: Vec<Vec2>,
    // Indexed by ShortcutID.0
    pub shortcut_points: Vec<Vec2>,
    pub shortcuts: HashMap<Vec2, Vec2>, // bi-direction (from, to)

    pub closest_point_of_interest: HashMap<Vec2, LocationID>,
}

impl LazyPointsOfInterest {
    pub fn locate(&self, location_id: LocationID) -> Vec2 {
        match location_id {
            LocationID::CampGroup(id) => self.camp_group_centers[id.0],
            LocationID::Altar(id) => self.altar_locations[id.0],
            LocationID::ControlPoint(id) => self.control_points[id.0],
            LocationID::Shortcut(id) => self.shortcut_points[id.0],
        }
    }

    pub fn try_get_closest_poi(&mut self, location: Vec2) -> LocationID {
        *self
            .closest_point_of_interest
            .entry(location)
            .or_insert_with_key(|key| {
                let current_location = *key;
                let mut closest: Option<(i32, LocationID)> = None;

                let mut consider = |id: LocationID, point: Vec2| {
                    let distance = chebyshev_distance(current_location, point);

                    if closest
                        .as_ref()
                        .is_none_or(|(best_distance, _)| distance < *best_distance)
                    {
                        closest = Some((distance, id));
                    }
                };

                for (index, &point) in self.control_points.iter().enumerate() {
                    consider(LocationID::ControlPoint(ControlPointID(index)), point);
                }
                for (index, &point) in self.altar_locations.iter().enumerate() {
                    consider(LocationID::Altar(AltarID(index)), point);
                }
                for (index, &point) in self.shortcut_points.iter().enumerate() {
                    consider(LocationID::Shortcut(ShortcutID(index)), point);
                }
                for (index, &point) in self.camp_group_centers.iter().enumerate() {
                    consider(LocationID::CampGroup(CampGroupID(index)), point);
                }

                closest
                    .map(|(_, id)| id)
                    .unwrap_or(LocationID::CampGroup(CampGroupID(0)))
            })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LazyMap {
    pub distance_fields: LazyDistanceFields,

    // Indexed by CampGroupID.0
    pub camp_group_respawn_timer: Vec<i32>,
    pub camp_group_effective_hp: Vec<f32>,
    pub camp_group_damage_per_round: Vec<f32>,
    pub camp_group_gem_value: Vec<u32>,
    pub camp_group_gold_value: Vec<u32>,
    pub camp_group_count: usize,

    pub points_of_interest: LazyPointsOfInterest,
}

impl LazyMap {
    pub fn new(draft_memory: &DraftMemory) -> Self {
        let camp_group_count: usize = draft_memory.camp_memory.camp_group_count as usize;
        let camp_group_centers: Vec<Vec2> = cambc::map::camp_groups()
            .unwrap()
            .into_iter()
            .take(camp_group_count)
            .map(|camp_group| {
                let center: [u8; 2] = camp_group.spawn_tiles().unwrap().center();
                Vec2 {
                    x: center[0] as i32,
                    y: center[1] as i32,
                }
            })
            .collect();
        let distance_fields: LazyDistanceFields = LazyDistanceFields::new(draft_memory);
        let camp_group_respawn_timer: Vec<i32> = vec![0; camp_group_count];
        let camp_group_effective_hp: Vec<f32> = draft_memory
            .camp_memory
            .camp_group_effective_hp
            .into_iter()
            .take(camp_group_count)
            .map(|u16_bytes| u16_bytes as f32)
            .collect();
        let camp_group_damage_per_round: Vec<f32> = draft_memory
            .camp_memory
            .camp_group_damage_per_round
            .into_iter()
            .take(camp_group_count)
            .map(|u8_byte| u8_byte as f32)
            .collect();
        let camp_group_gem_value: Vec<u32> = draft_memory
            .camp_memory
            .camp_group_gem_value
            .into_iter()
            .take(camp_group_count)
            .map(|u16_bytes| u16_bytes as u32)
            .collect();
        let camp_group_gold_value: Vec<u32> = draft_memory
            .camp_memory
            .camp_group_gold_value
            .into_iter()
            .take(camp_group_count)
            .map(|u16_bytes| u16_bytes as u32)
            .collect();

        let altar_locations: Vec<Vec2> = cambc::map::altars().unwrap().positions().unwrap();

        let control_points: Vec<Vec2> = draft_memory
            .control_point_memory
            .control_points
            .into_iter()
            .filter_map(|control_point| {
                if control_point != [0, 0] {
                    Some(Vec2 {
                        x: control_point[0] as i32,
                        y: control_point[1] as i32,
                    })
                } else {
                    None
                }
            })
            .collect();

        let shortcuts: HashMap<Vec2, Vec2> = draft_memory.shortcut_memory.decoded_shortcuts();

        let points_of_interest: LazyPointsOfInterest = LazyPointsOfInterest {
            camp_group_centers,
            altar_locations,
            control_points,
            shortcut_points: shortcuts.keys().copied().collect(),
            shortcuts,
            closest_point_of_interest: HashMap::new(),
        };

        Self {
            distance_fields,

            camp_group_respawn_timer,
            camp_group_effective_hp,
            camp_group_damage_per_round,
            camp_group_gem_value,
            camp_group_gold_value,
            camp_group_count,

            points_of_interest,
        }
    }
}
