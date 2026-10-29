use crate::{
    action::combat::estimator::CampTypeAttackOrder,
    draft::draft_memory::DraftMemory,
    memory::{AltarID, CampGroupID, CampID, ControlPointID, LocationID, ShortcutID},
    sdk_interact::map::Center,
};
use cambc::{Bitboard, Camp, DistanceField, Vec2};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, PartialEq)]
pub struct LazyDistanceFields {
    pub discovery_queue: VecDeque<LocationID>,

    pub camp_group_distance_fields: Vec<Option<DistanceField>>,
    pub altar_distance_fields: Vec<Option<DistanceField>>,
    pub shortcut_point_distance_fields: Vec<Option<DistanceField>>,
    pub control_point_distance_fields: Vec<Option<DistanceField>>,

    pub discovered_this_round: bool,
    pub blocked_bitboard: Bitboard,
}

impl LazyDistanceFields {
    pub fn new(draft_memory: &DraftMemory, points_of_interst: &LazyPointsOfInterest) -> Self {
        let camp_group_discovery_items = draft_memory
            .camp_memory
            .camp_group_discovery_order
            .into_iter()
            .take(draft_memory.camp_memory.camp_group_count as usize)
            .map(|byte| CampGroupID(byte as usize));

        let camp_group_distance_fields: Vec<Option<DistanceField>> =
            vec![None; draft_memory.camp_memory.camp_group_count as usize];

        let n_altars: usize = points_of_interst.altar_locations.len();
        let altar_discovery_items = (0..n_altars).map(AltarID);
        let altar_distance_fields: Vec<Option<DistanceField>> = vec![None; n_altars];

        let n_shortcuts: usize = points_of_interst.shortcut_points.len();
        let shortcut_point_discovery_items = (0..n_shortcuts).map(ShortcutID);
        let shortcut_point_distance_fields: Vec<Option<DistanceField>> = vec![None; n_shortcuts];

        let n_control: usize = points_of_interst.control_points.len();
        let control_point_discovery_items = (0..n_control).map(ControlPointID);
        let control_point_distance_fields: Vec<Option<DistanceField>> = vec![None; n_control];

        let discovery_queue: VecDeque<LocationID> = camp_group_discovery_items
            .map(LocationID::CampGroup)
            .chain(altar_discovery_items.map(LocationID::Altar))
            .chain(shortcut_point_discovery_items.map(LocationID::Shortcut))
            .chain(control_point_discovery_items.map(LocationID::ControlPoint))
            .collect();

        let blocked_bitboard: Bitboard = cambc::map::walls()
            .unwrap()
            .union(cambc::map::holes().unwrap())
            .unwrap()
            .union(cambc::map::altars().unwrap())
            .unwrap();

        Self {
            discovery_queue,

            camp_group_distance_fields,
            altar_distance_fields,
            shortcut_point_distance_fields,
            control_point_distance_fields,

            discovered_this_round: false,
            blocked_bitboard,
        }
    }

    pub fn reset_discovered_state(&mut self) {
        self.discovered_this_round = false;
    }

    pub fn lazy_discover(&mut self, points_of_interst: &LazyPointsOfInterest) {
        while !self.discovered_this_round {
            let Some(location_id): Option<LocationID> = self.discovery_queue.pop_front() else {
                return;
            };
            self.try_get(location_id, points_of_interst);
        }
    }

    pub fn weak_get(&self, location_id: LocationID) -> Option<&DistanceField> {
        match location_id {
            LocationID::CampGroup(id) => self.camp_group_distance_fields[id.0].as_ref(),
            LocationID::Altar(id) => self.altar_distance_fields[id.0].as_ref(),
            LocationID::Shortcut(id) => self.shortcut_point_distance_fields[id.0].as_ref(),
            LocationID::ControlPoint(id) => self.control_point_distance_fields[id.0].as_ref(),
        }
    }

    fn set_distance_field(&mut self, location_id: LocationID, distance_field: DistanceField) {
        match location_id {
            LocationID::CampGroup(id) => {
                self.camp_group_distance_fields[id.0] = Some(distance_field)
            }
            LocationID::Altar(id) => self.altar_distance_fields[id.0] = Some(distance_field),
            LocationID::Shortcut(id) => {
                self.shortcut_point_distance_fields[id.0] = Some(distance_field)
            }
            LocationID::ControlPoint(id) => {
                self.control_point_distance_fields[id.0] = Some(distance_field)
            }
        }
    }

    pub fn try_get(
        &mut self,
        location_id: LocationID,
        points_of_interst: &LazyPointsOfInterest,
    ) -> Option<&DistanceField> {
        let None: Option<&DistanceField> = self.weak_get(location_id) else {
            return self.weak_get(location_id);
        };
        if self.discovered_this_round {
            return self.weak_get(location_id);
        }

        let mut distance_field: DistanceField = DistanceField::new().unwrap();
        let destination_bitboard: Bitboard = match location_id {
            LocationID::CampGroup(id) => points_of_interst.camp_group_spawn_tiles[id.0].clone(),
            LocationID::Altar(id) => {
                let mut bitboard: Bitboard = Bitboard::new().unwrap();
                bitboard.set(points_of_interst.altar_locations[id.0], true);
                bitboard
            }
            LocationID::Shortcut(id) => {
                let mut bitboard: Bitboard = Bitboard::new().unwrap();
                bitboard.set(points_of_interst.shortcut_points[id.0], true);
                bitboard
            }
            LocationID::ControlPoint(id) => {
                let mut bitboard: Bitboard = Bitboard::new().unwrap();
                bitboard.set(points_of_interst.control_points[id.0], true);
                bitboard
            }
        };

        let _ = distance_field.compute(
            &self.blocked_bitboard,
            &destination_bitboard,
            &mut Bitboard::new().unwrap(),
            u16::MAX,
        );

        self.set_distance_field(location_id, distance_field);
        self.discovered_this_round = true;
        self.weak_get(location_id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LazyPointsOfInterest {
    // Indexed by CampGroupID
    pub camp_group_spawn_tiles: Vec<Bitboard>,
    pub camp_group_centers: Vec<Vec2>,
    // Indexed by AltarID.0
    pub altar_locations: Vec<Vec2>,
    // Indexed by ControlPointID.0
    pub control_points: Vec<Vec2>,
    // Indexed by ShortcutID.0
    pub shortcut_points: Vec<Vec2>,
    pub shortcuts: HashMap<Vec2, Vec2>, // bi-direction (from, to)

    pub closest_point_of_interest: HashMap<Vec2, Option<LocationID>>,

    pub points_of_interest_bitboard: Bitboard,
}

impl LazyPointsOfInterest {
    pub fn new(
        camp_group_spawn_tiles: Vec<Bitboard>,
        camp_group_centers: Vec<Vec2>,
        altar_locations: Vec<Vec2>,
        control_points: Vec<Vec2>,
        shortcuts: HashMap<Vec2, Vec2>,
    ) -> Self {
        let shortcut_points: Vec<Vec2> = shortcuts.keys().copied().collect();
        let mut points_of_interest_bitboard: Bitboard = Bitboard::new().unwrap();

        for &point in &control_points {
            points_of_interest_bitboard.set(point, true);
        }
        for &point in &altar_locations {
            points_of_interest_bitboard.set(point, true);
        }
        for &point in shortcuts.keys() {
            points_of_interest_bitboard.set(point, true);
        }
        for &point in &camp_group_centers {
            points_of_interest_bitboard.set(point, true);
        }

        let closest_point_of_interest: HashMap<Vec2, Option<LocationID>> = HashMap::new();

        LazyPointsOfInterest {
            camp_group_spawn_tiles,
            camp_group_centers,
            altar_locations,
            control_points,
            shortcut_points,
            shortcuts,
            closest_point_of_interest,
            points_of_interest_bitboard,
        }
    }

    pub fn locate(&self, location_id: LocationID) -> Vec2 {
        match location_id {
            LocationID::CampGroup(id) => self.camp_group_centers[id.0],
            LocationID::Altar(id) => self.altar_locations[id.0],
            LocationID::ControlPoint(id) => self.control_points[id.0],
            LocationID::Shortcut(id) => self.shortcut_points[id.0],
        }
    }

    pub fn try_get_closest_poi(
        &mut self,
        location: Vec2,
        distance_fields: &mut LazyDistanceFields,
    ) -> Option<LocationID> {
        if let Some(&location_id) = self.closest_point_of_interest.get(&location) {
            return location_id;
        }

        let result: Option<LocationID> = {
            let mut closest: Option<(u16, LocationID)> = None;

            let mut consider = |id: LocationID| {
                let distance: u16 = distance_fields
                    .try_get(id, self)
                    .map(|field| field.get(location).unwrap())
                    .unwrap_or(0);

                if closest.is_none_or(|(best_distance, _)| distance < best_distance) {
                    closest = Some((distance, id));
                }
            };

            for location_id in (0..self.camp_group_centers.len())
                .map(CampGroupID)
                .map(LocationID::CampGroup)
                .chain(
                    (0..self.altar_locations.len())
                        .map(AltarID)
                        .map(LocationID::Altar),
                )
                .chain(
                    (0..self.shortcut_points.len())
                        .map(ShortcutID)
                        .map(LocationID::Shortcut),
                )
                .chain(
                    (0..self.control_points.len())
                        .map(ControlPointID)
                        .map(LocationID::ControlPoint),
                )
            {
                consider(location_id)
            }

            closest.map(|(_, id)| id)
        };

        self.closest_point_of_interest.insert(location, result);
        result
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LazyMap {
    pub distance_fields: LazyDistanceFields,

    // Indexed by CampGroupID.0
    pub camp_group_attack_order: Vec<Vec<CampID>>,
    pub camp_group_reset_timer: Vec<i32>,
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
        let camp_group_spawn_tiles: Vec<Bitboard> = cambc::map::camp_groups()
            .unwrap()
            .into_iter()
            .take(camp_group_count)
            .map(|camp_group| camp_group.spawn_tiles().unwrap())
            .collect();
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
        let camp_group_attack_order: Vec<Vec<CampID>> = cambc::map::camp_groups()
            .unwrap()
            .iter()
            .map(|camp_group| {
                let camps: &[Camp] = camp_group.camps();
                let n_camps: usize = camps.len();
                let mut camp_ids: Vec<CampID> = (0..n_camps).map(CampID).collect();
                camp_ids.sort_unstable_by_key(|camp_id| {
                    CampTypeAttackOrder::from(camps[camp_id.0].kind()) as usize
                });
                camp_ids
            })
            .collect();
        let camp_group_reset_timer: Vec<i32> = cambc::map::camp_groups()
            .unwrap()
            .into_iter()
            .take(camp_group_count)
            .map(|camp_group| camp_group.max_respawn_timer())
            .collect();
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

        let points_of_interest: LazyPointsOfInterest = LazyPointsOfInterest::new(
            camp_group_spawn_tiles,
            camp_group_centers,
            altar_locations,
            control_points,
            shortcuts,
        );

        let distance_fields: LazyDistanceFields =
            LazyDistanceFields::new(draft_memory, &points_of_interest);

        Self {
            distance_fields,

            camp_group_attack_order,
            camp_group_reset_timer,
            camp_group_respawn_timer,
            camp_group_effective_hp,
            camp_group_damage_per_round,
            camp_group_gem_value,
            camp_group_gold_value,
            camp_group_count,

            points_of_interest,
        }
    }

    pub fn start_camp_group_respawn_timer(&mut self, camp_group_id: CampGroupID) {
        self.camp_group_respawn_timer[camp_group_id.0] =
            self.camp_group_reset_timer[camp_group_id.0];
    }

    pub fn tick_camp_group_respawn_timers(&mut self) {
        for respawn_timer in &mut self.camp_group_respawn_timer {
            *respawn_timer = (*respawn_timer - 1).max(0);
        }
    }
}
