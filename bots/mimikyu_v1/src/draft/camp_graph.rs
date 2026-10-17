use cambc::{Bitboard, Camp, CampGroup, DistanceField, Result, Vec2};

#[derive(Debug, Clone, PartialEq)]
pub struct CampGraph {
    pub camp_group_centers: Vec<[u8; 2]>,
    pub closest_altars: Vec<[u8; 2]>,
    pub camp_group_safe: Vec<bool>,
    pub camp_group_danger_level: Vec<u8>,
    pub camp_group_gold: Vec<u8>,
    pub camp_group_gem: Vec<u8>,
    pub camp_group_distances: Vec<Vec<u8>>,
    pub camp_group_spawn_distances: Vec<[u8; 2]>, // [to_ally, to_enemy]
    pub spawns_distance: u8,
}

impl CampGraph {
    pub fn new(ally_spawn_center: [u8; 2], enemy_spawn_center: [u8; 2]) -> Result<CampGraph> {
        let camp_groups: Vec<CampGroup> = cambc::map::camp_groups()?;
        let n_camp_groups: usize = camp_groups.len();
        let camp_group_nodes: Vec<Vec<[u8; 2]>> = camp_groups
            .iter()
            .map(|camp_group: &CampGroup| {
                camp_group
                    .camps()
                    .iter()
                    .map(|camp: &Camp| {
                        let camp_position: Vec2 = camp.spawn_position();
                        [camp_position.x as u8, camp_position.y as u8]
                    })
                    .collect()
            })
            .collect();

        let camp_group_centers: Vec<[u8; 2]> = camp_group_nodes
            .iter()
            .map(|camp_group_nodes: &Vec<[u8; 2]>| camp_group_nodes.center())
            .collect();

        let mut camp_group_distances: Vec<Vec<u8>> = Vec::with_capacity(n_camp_groups);
        let mut camp_group_spawn_distances: Vec<[u8; 2]> = Vec::with_capacity(n_camp_groups);
        let mut closest_altars: Vec<[u8; 2]> = Vec::with_capacity(n_camp_groups);
        let altars: Vec<[u8; 2]> = cambc::map::altars()?
            .positions()?
            .iter()
            .map(|v| [v.x as u8, v.y as u8])
            .collect();

        let blocked_bitboard: Bitboard = cambc::map::walls()?.union(cambc::map::holes()?)?;

        for camp_group in &camp_groups {
            let mut distance_field: DistanceField = DistanceField::new()?;
            distance_field.compute(
                &blocked_bitboard,
                &camp_group.spawn_tiles()?,
                &mut Bitboard::new()?,
                u16::MAX,
            )?;

            let mut closest_altar_distance: u8 = u8::MAX;
            let mut closest_altar: [u8; 2] = [0, 0];
            for altar in altars.iter() {
                let distance = distance_field.get(Vec2 {
                    x: altar[0] as i32,
                    y: altar[1] as i32,
                })? as u8;
                if distance < closest_altar_distance {
                    closest_altar = *altar;
                    closest_altar_distance = distance;
                }
            }
            closest_altars.push(closest_altar);

            camp_group_spawn_distances.push([
                distance_field
                    .get(Vec2 {
                        x: ally_spawn_center[0] as i32,
                        y: ally_spawn_center[1] as i32,
                    })
                    .unwrap() as u8,
                distance_field
                    .get(Vec2 {
                        x: enemy_spawn_center[0] as i32,
                        y: enemy_spawn_center[1] as i32,
                    })
                    .unwrap() as u8,
            ]);

            camp_group_distances.push(
                camp_group_centers
                    .iter()
                    .map(move |&[x, y]: &[u8; 2]| {
                        distance_field
                            .get(Vec2 {
                                x: x as i32,
                                y: y as i32,
                            })
                            .map(|distance| distance as u8)
                    })
                    .collect::<Result<Vec<u8>>>()?,
            );
        }
        let mut ally_spawn_field: DistanceField = DistanceField::new()?;
        let mut ally_spawn_bitboard: Bitboard = Bitboard::new()?;
        let _ = ally_spawn_bitboard.set(
            Vec2 {
                x: ally_spawn_center[0] as i32,
                y: ally_spawn_center[1] as i32,
            },
            true,
        );
        ally_spawn_field.compute(
            &blocked_bitboard,
            &ally_spawn_bitboard,
            &mut Bitboard::new()?,
            u16::MAX,
        )?;
        let spawns_distance: u8 = ally_spawn_field
            .get(Vec2 {
                x: enemy_spawn_center[0] as i32,
                y: enemy_spawn_center[1] as i32,
            })
            .unwrap() as u8;

        let camp_group_safe: Vec<bool> = camp_groups
            .iter()
            .map(|camp_group: &CampGroup| {
                camp_group
                    .camps()
                    .iter()
                    .any(|camp: &Camp| camp.is_wolf() | camp.is_elk() | camp.is_monolith())
            })
            .collect();

        let camp_group_danger_level: Vec<u8> = camp_groups
            .iter()
            .map(|camp_group: &CampGroup| {
                camp_group
                    .camps()
                    .iter()
                    .fold(0, |acc, camp: &Camp| {
                        acc + match (camp.is_wolf(), camp.is_elk(), camp.is_elk()) {
                            (true, _, _) => 1,
                            (_, true, _) => 2,
                            (_, _, true) => 4,
                            _ => 0,
                        }
                    })
                    .clamp(0, 15)
            })
            .collect();

        let camp_group_gold: Vec<u8> = camp_groups
            .iter()
            .map(|camp_group: &CampGroup| camp_group.gold_value() as u8)
            .collect();

        let camp_group_gem: Vec<u8> = camp_groups
            .iter()
            .map(|camp_group: &CampGroup| camp_group.gem_value() as u8)
            .collect();

        Ok(Self {
            camp_group_centers,
            closest_altars,
            camp_group_safe,
            camp_group_danger_level,
            camp_group_distances,
            camp_group_gold,
            camp_group_gem,
            camp_group_spawn_distances,
            spawns_distance,
        })
    }
}

pub struct CampSpawnDistanceMatrix<'a> {
    pub distances_to_camp: &'a [Vec<u8>],
    pub distances_to_spawns: &'a [[u8; 2]],
    pub spawns_distance: &'a u8,
}

impl<'a> CampSpawnDistanceMatrix<'a> {
    pub fn get(&self, from: usize, to: usize) -> u8 {
        let n: usize = self.distances_to_camp.len();
        if from < n && to < n {
            self.distances_to_camp[from][to]
        } else if from > n && to < n {
            let spawn_from: usize = from - n;
            self.distances_to_spawns[to][spawn_from]
        } else if from < n && to > n {
            let spawn_to: usize = to - n;
            self.distances_to_spawns[from][spawn_to]
        } else {
            *self.spawns_distance
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.distances_to_camp.len() + self.distances_to_spawns.len()
    }
}

pub trait Center {
    fn center(&self) -> [u8; 2];
}

impl Center for Vec<[u8; 2]> {
    fn center(&self) -> [u8; 2] {
        let sum: [u8; 2] = self
            .iter()
            .fold([0_u8, 0_u8], |acc, v| [acc[0] + v[0], acc[1] + v[1]]);
        [sum[0] / self.len() as u8, sum[1] / self.len() as u8]
    }
}
impl Center for Bitboard {
    fn center(&self) -> [u8; 2] {
        self.positions()
            .unwrap()
            .iter()
            .map(|position: &Vec2| [position.x as u8, position.y as u8])
            .collect::<Vec<[u8; 2]>>()
            .center()
    }
}
