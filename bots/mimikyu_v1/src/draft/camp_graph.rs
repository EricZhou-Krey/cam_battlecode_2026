use cambc::{Bitboard, CampGroup, DistanceField, Result, Vec2};

const BULK_EXPORT: bool = true;

#[derive(Debug, Clone, PartialEq)]
pub struct CampGraph {
    pub camp_group_centers: Vec<[u8; 2]>,
    pub spawn_centers: [[u8; 2]; 2], // [ally, enemy]
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
        let spawn_centers: [[u8; 2]; 2] = [ally_spawn_center, enemy_spawn_center];
        let camp_groups: Vec<CampGroup> = cambc::map::camp_groups()?;
        let n_camp_groups: usize = camp_groups.len();

        let mut camp_group_nodes: Vec<Vec<[u8; 2]>> = Vec::with_capacity(n_camp_groups);
        let mut camp_group_safe: Vec<bool> = Vec::with_capacity(n_camp_groups);
        let mut camp_group_danger_level: Vec<u8> = Vec::with_capacity(n_camp_groups);
        let mut camp_group_gold: Vec<u8> = Vec::with_capacity(n_camp_groups);
        let mut camp_group_gem: Vec<u8> = Vec::with_capacity(n_camp_groups);
        for camp_group in &camp_groups {
            let mut nodes: Vec<[u8; 2]> = Vec::new();
            let mut safe: bool = true;
            let mut danger: u32 = 0;
            for camp in camp_group.camps().iter() {
                let position: Vec2 = camp.spawn_position();
                nodes.push([position.x as u8, position.y as u8]);
                let wolf: bool = camp.is_wolf();
                let elk: bool = camp.is_elk();
                let monolith: bool = camp.is_monolith();
                if wolf | elk | monolith {
                    safe = false;
                }
                danger += if wolf {
                    1
                } else if elk {
                    2
                } else if monolith {
                    4
                } else {
                    0
                };
            }
            camp_group_nodes.push(nodes);
            camp_group_safe.push(safe);
            camp_group_danger_level.push(danger.min(15) as u8);
            camp_group_gold.push(camp_group.gold_value() as u8);
            camp_group_gem.push(camp_group.gem_value() as u8);
        }

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
        let map_width: usize = cambc::map::width()? as usize;
        let map_height: usize = cambc::map::height()? as usize;
        let tile_index = |position: [u8; 2]| -> usize {
            position[1] as usize * map_width + position[0] as usize
        };

        let center_indices: Vec<usize> = camp_group_centers
            .iter()
            .map(|&center: &[u8; 2]| tile_index(center))
            .collect();
        let altar_indices: Vec<usize> = altars
            .iter()
            .map(|&altar: &[u8; 2]| tile_index(altar))
            .collect();
        let ally_index: usize = tile_index(ally_spawn_center);
        let enemy_index: usize = tile_index(enemy_spawn_center);
        let mut field_buffer: Vec<u16> = vec![0; map_width * map_height];
        let saturate = |value: u16| -> u8 { value.min(u8::MAX as u16) as u8 };

        for (group_index, camp_group) in camp_groups.iter().enumerate() {
            let mut distance_field: DistanceField = DistanceField::new()?;
            distance_field.compute(
                &blocked_bitboard,
                &camp_group.spawn_tiles()?,
                &mut Bitboard::new()?,
                u16::MAX,
            )?;
            if BULK_EXPORT {
                distance_field.export_linear_buffer_into(&mut field_buffer)?;
            }
            let lookup = |position: [u8; 2], tile: usize| -> Result<u8> {
                if BULK_EXPORT {
                    Ok(saturate(field_buffer[tile]))
                } else {
                    distance_field
                        .get(Vec2 {
                            x: position[0] as i32,
                            y: position[1] as i32,
                        })
                        .map(saturate)
                }
            };

            let mut closest_altar_distance: u8 = u8::MAX;
            let mut closest_altar: [u8; 2] = [0, 0];
            for (altar, &altar_index) in altars.iter().zip(altar_indices.iter()) {
                let distance: u8 = lookup(*altar, altar_index)?;
                if distance < closest_altar_distance {
                    closest_altar = *altar;
                    closest_altar_distance = distance;
                }
            }
            closest_altars.push(closest_altar);

            camp_group_spawn_distances.push([
                lookup(ally_spawn_center, ally_index)?,
                lookup(enemy_spawn_center, enemy_index)?,
            ]);

            let mut row: Vec<u8> = Vec::with_capacity(camp_group_centers.len());
            for (&center, &center_index) in camp_group_centers.iter().zip(center_indices.iter()) {
                row.push(lookup(center, center_index)?);
            }
            camp_group_distances.push(row);
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

        Ok(Self {
            camp_group_centers,
            spawn_centers,
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
        } else if from >= n && to < n {
            let spawn_from: usize = from - n;
            self.distances_to_spawns[to][spawn_from]
        } else if from < n && to >= n {
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
        self.distances_to_camp.len() + 2
    }
}

pub trait Center {
    fn center(&self) -> [u8; 2];
}

impl Center for Vec<[u8; 2]> {
    fn center(&self) -> [u8; 2] {
        let (sx, sy) = self
            .iter()
            .fold((0u32, 0u32), |(x, y), v| (x + v[0] as u32, y + v[1] as u32));
        let n = self.len().max(1) as u32;
        [(sx / n) as u8, (sy / n) as u8]
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
