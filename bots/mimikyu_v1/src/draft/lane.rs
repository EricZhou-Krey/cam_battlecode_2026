use crate::draft::camp_graph::{CampGraph, CampSpawnDistanceMatrix};
use cambc::Result;

pub const NEIGHBOUR_LINEAR_TOLERANCE: f32 = 0.2;
pub const NEIGHBOUR_CONSTANT_TOLERANCE: f32 = 2.0;

const N_LANES: usize = 8;
const REWARD: i64 = 1_000_000;
const SHARE_PRICE: i64 = 10;
const MAX_BALANCE_ITERS: usize = 200;

type Key = (i64, usize);

#[derive(Debug, Clone, PartialEq)]
pub struct LaneGraph {
    pub value_lanes: [Vec<usize>; 8],
    pub safe_lanes: [Vec<usize>; 8],
}

impl LaneGraph {
    pub fn new(camp_graph: &CampGraph, distance_matrix: &CampSpawnDistanceMatrix) -> Result<Self> {
        let mut estimated_value: Vec<u16> = camp_graph
            .camp_group_gem
            .iter()
            .zip(camp_graph.camp_group_gold.iter())
            .map(|(gem, gold)| *gem as u16 * 2 + *gold as u16)
            .collect();
        estimated_value.extend([0, 0]);
        let mut spawn_delta: Vec<i16> = camp_graph
            .camp_group_spawn_distances
            .iter()
            .map(|spawns| spawns[0] as i16 - spawns[1] as i16)
            .collect();
        spawn_delta.extend([
            -(*distance_matrix.spawns_distance as i16),
            (*distance_matrix.spawns_distance as i16),
        ]);
        let mut safe: Vec<bool> = camp_graph.camp_group_safe.clone();
        safe.extend([true, true]);

        let n: usize = distance_matrix.len();

        Ok(Self {
            value_lanes: Self::calculate_lanes(
                distance_matrix,
                &estimated_value,
                &spawn_delta,
                &vec![true; n],
                true,
            ),
            safe_lanes: Self::calculate_lanes(
                distance_matrix,
                &estimated_value,
                &spawn_delta,
                &safe,
                false,
            ),
        })
    }

    fn calculate_lanes(
        distance_matrix: &CampSpawnDistanceMatrix,
        estimated_value: &[u16],
        spawn_delta: &[i16],
        allowed_nodes: &[bool],
        balance: bool,
    ) -> [Vec<usize>; N_LANES] {
        let node_count: usize = estimated_value.len();
        let camp_count: usize = node_count - 2;
        let ally_spawn_index: usize = camp_count;
        let enemy_spawn_index: usize = camp_count + 1;
        let nodes: Vec<usize> = (0..node_count)
            .filter(|&node_index: &usize| allowed_nodes[node_index])
            .collect();
        if nodes.len() == 2 {
            return [const { Vec::new() }; N_LANES];
        }

        let walking_order = |node_index: usize| -> Key {
            let progress: i64 = if node_index == ally_spawn_index {
                i64::MIN / 2
            } else if node_index == enemy_spawn_index {
                i64::MAX / 2
            } else {
                spawn_delta[node_index] as i64
            };
            (progress, node_index)
        };

        let mut linked: Vec<Vec<bool>> = vec![vec![false; node_count]; node_count];
        for (position, &from_index) in nodes.iter().enumerate() {
            for &to_index in &nodes[position + 1..] {
                if from_index == ally_spawn_index && to_index == enemy_spawn_index {
                    continue;
                }
                let max_distance: f32 = (1.0 + NEIGHBOUR_LINEAR_TOLERANCE)
                    * distance_matrix.get(from_index, to_index) as f32
                    + NEIGHBOUR_CONSTANT_TOLERANCE;
                let is_blocked: bool = nodes.iter().any(|&inbetween_index: &usize| {
                    inbetween_index != from_index
                        && inbetween_index != to_index
                        && (distance_matrix.get(from_index, inbetween_index)
                            + distance_matrix.get(inbetween_index, to_index))
                            as f32
                            <= max_distance
                });
                if !is_blocked {
                    linked[from_index][to_index] = true;
                    linked[to_index][from_index] = true;
                }
            }
        }

        let mut in_tree: Vec<bool> = vec![false; node_count];
        in_tree[nodes[0]] = true;
        for _ in 1..nodes.len() {
            let mut cheapest: Option<(u32, usize, usize)> = None;
            for &tree_index in nodes.iter().filter(|&&node_index| in_tree[node_index]) {
                for &outside_index in nodes.iter().filter(|&&node_index| !in_tree[node_index]) {
                    if (tree_index == ally_spawn_index && outside_index == enemy_spawn_index)
                        || (tree_index == enemy_spawn_index && outside_index == ally_spawn_index)
                    {
                        continue;
                    }
                    let walking_distance: u32 =
                        distance_matrix.get(tree_index, outside_index) as u32;
                    if cheapest.map_or(true, |best| walking_distance < best.0) {
                        cheapest = Some((walking_distance, tree_index, outside_index));
                    }
                }
            }
            let Some((_, tree_index, outside_index)) = cheapest else {
                break;
            };
            in_tree[outside_index] = true;
            linked[tree_index][outside_index] = true;
            linked[outside_index][tree_index] = true;
        }

        let mut outgoing: Vec<Vec<usize>> = vec![Vec::new(); node_count];
        let mut has_incoming: Vec<bool> = vec![false; node_count];
        for &from_index in &nodes {
            for &to_index in &nodes {
                if linked[from_index][to_index]
                    && walking_order(from_index) < walking_order(to_index)
                {
                    outgoing[from_index].push(to_index);
                    has_incoming[to_index] = true;
                }
            }
        }

        for &camp_index in nodes.iter().filter(|&&node_index| node_index < camp_count) {
            if outgoing[camp_index].is_empty() {
                let next_index: usize = nodes
                    .iter()
                    .copied()
                    .filter(|&node_index| walking_order(node_index) > walking_order(camp_index))
                    .min_by_key(|&node_index| distance_matrix.get(camp_index, node_index))
                    .unwrap();
                outgoing[camp_index].push(next_index);
                has_incoming[next_index] = true;
            }
            if !has_incoming[camp_index] {
                let previous_index: usize = nodes
                    .iter()
                    .copied()
                    .filter(|&node_index| walking_order(node_index) < walking_order(camp_index))
                    .min_by_key(|&node_index| distance_matrix.get(node_index, camp_index))
                    .unwrap();
                outgoing[previous_index].push(camp_index);
                has_incoming[camp_index] = true;
            }
        }

        let node_in = |node_index: usize| -> usize {
            if node_index < camp_count {
                2 * node_index
            } else {
                2 * camp_count + (node_index - camp_count)
            }
        };
        let node_out = |node_index: usize| -> usize {
            if node_index < camp_count {
                2 * node_index + 1
            } else {
                2 * camp_count + (node_index - camp_count)
            }
        };
        let source: usize = 2 * camp_count;
        let sink: usize = 2 * camp_count + 1;

        let mut network: Network = Network::new(2 * camp_count + 2);
        for &camp_index in nodes.iter().filter(|&&node_index| node_index < camp_count) {
            for lane_number in 1..=N_LANES as i64 {
                let cost: i64 = if lane_number == 1 {
                    -REWARD
                } else {
                    SHARE_PRICE * (2 * lane_number - 3)
                };
                network.add(node_in(camp_index), node_out(camp_index), 1, cost);
            }
        }
        for &from_index in &nodes {
            for &to_index in &outgoing[from_index] {
                network.add(
                    node_out(from_index),
                    node_in(to_index),
                    N_LANES as i32,
                    distance_matrix.get(from_index, to_index) as i64,
                );
            }
        }
        for _ in 0..N_LANES {
            if !network.push_unit(source, sink) {
                break;
            }
        }

        let mut flow: Vec<i32> = (0..network.arcs.len())
            .map(|arc_id: usize| {
                if arc_id % 2 == 0 {
                    network.arcs[arc_id ^ 1].cap
                } else {
                    0
                }
            })
            .collect();
        let mut lanes: Vec<Vec<usize>> = Vec::new();
        for _ in 0..N_LANES {
            let mut lane: Vec<usize> = Vec::new();
            let mut current_node: usize = source;
            while current_node != sink {
                let Some(&arc_id) = network.adj[current_node]
                    .iter()
                    .find(|&&arc_id| arc_id % 2 == 0 && flow[arc_id] > 0)
                else {
                    break;
                };
                flow[arc_id] -= 1;
                if current_node < 2 * camp_count && current_node % 2 == 0 {
                    lane.push(current_node / 2);
                }
                current_node = network.arcs[arc_id].to;
            }
            if current_node == sink && !lane.is_empty() {
                lanes.push(lane);
            }
        }

        if balance {
            improve(
                &mut lanes,
                distance_matrix,
                estimated_value,
                allowed_nodes,
                &walking_order,
            );
        }

        let mut lane_array: [Vec<usize>; N_LANES] = [const { Vec::new() }; N_LANES];
        for (slot, lane) in lane_array.iter_mut().zip(lanes) {
            *slot = lane;
        }
        lane_array
    }
}

struct Arc {
    to: usize,
    cap: i32,
    cost: i64,
}

struct Network {
    arcs: Vec<Arc>,
    adj: Vec<Vec<usize>>,
}

impl Network {
    fn new(node_count: usize) -> Self {
        Self {
            arcs: Vec::new(),
            adj: vec![Vec::new(); node_count],
        }
    }

    fn add(&mut self, from: usize, to: usize, cap: i32, cost: i64) {
        self.adj[from].push(self.arcs.len());
        self.arcs.push(Arc { to, cap, cost });
        self.adj[to].push(self.arcs.len());
        self.arcs.push(Arc {
            to: from,
            cap: 0,
            cost: -cost,
        });
    }

    fn push_unit(&mut self, s: usize, t: usize) -> bool {
        let node_count: usize = self.adj.len();
        let mut dist: Vec<i64> = vec![i64::MAX; node_count];
        let mut via: Vec<usize> = vec![usize::MAX; node_count];
        dist[s] = 0;
        for _ in 0..node_count {
            let mut changed: bool = false;
            for u in 0..node_count {
                if dist[u] == i64::MAX {
                    continue;
                }
                for &arc_id in &self.adj[u] {
                    let arc: &Arc = &self.arcs[arc_id];
                    if arc.cap > 0 && dist[u] + arc.cost < dist[arc.to] {
                        dist[arc.to] = dist[u] + arc.cost;
                        via[arc.to] = arc_id;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        if dist[t] == i64::MAX {
            return false;
        }
        let mut node: usize = t;
        while node != s {
            let arc_id: usize = via[node];
            self.arcs[arc_id].cap -= 1;
            self.arcs[arc_id ^ 1].cap += 1;
            node = self.arcs[arc_id ^ 1].to;
        }
        true
    }
}

fn lane_cost(
    lanes: &[Vec<usize>],
    distance_matrix: &CampSpawnDistanceMatrix,
    estimated_value: &[u16],
    allowed_nodes: &[bool],
    balance_weight: f64,
) -> f64 {
    let camp_count: usize = estimated_value.len() - 2;
    let ally_spawn_index: usize = camp_count;
    let enemy_spawn_index: usize = camp_count + 1;

    let mut usage: Vec<u32> = vec![0; camp_count];
    for lane in lanes {
        for &camp_index in lane {
            usage[camp_index] += 1;
        }
    }

    let mut total_cost: f64 = 0.0;
    for lane in lanes {
        let mut previous_index: usize = ally_spawn_index;
        for &camp_index in lane {
            total_cost += distance_matrix.get(previous_index, camp_index) as f64;
            previous_index = camp_index;
        }
        total_cost += distance_matrix.get(previous_index, enemy_spawn_index) as f64;
    }
    for camp_index in (0..camp_count).filter(|&camp_index| allowed_nodes[camp_index]) {
        total_cost += if usage[camp_index] == 0 {
            REWARD as f64
        } else {
            SHARE_PRICE as f64 * ((usage[camp_index] - 1) as f64).powi(2)
        };
    }
    let lane_values: Vec<f64> = lanes
        .iter()
        .map(|lane: &Vec<usize>| {
            lane.iter()
                .map(|&camp_index: &usize| {
                    estimated_value[camp_index] as f64 / usage[camp_index] as f64
                })
                .sum::<f64>()
        })
        .collect();
    let mean_value: f64 = lane_values.iter().sum::<f64>() / lane_values.len().max(1) as f64;
    total_cost
        + balance_weight
            * lane_values
                .iter()
                .map(|value: &f64| (value - mean_value).powi(2))
                .sum::<f64>()
}

fn candidates(lanes: &[Vec<usize>], key: &dyn Fn(usize) -> Key) -> Vec<Vec<Vec<usize>>> {
    let mut moves: Vec<Vec<Vec<usize>>> = Vec::new();
    for from_lane in 0..lanes.len() {
        for (camp_position, &camp_index) in lanes[from_lane].iter().enumerate() {
            for to_lane in 0..lanes.len() {
                if to_lane == from_lane {
                    continue;
                }
                match lanes[to_lane].iter().position(|&other| other == camp_index) {
                    // Tail swap at a shared camp keeps usage the same and only moves value.
                    Some(other_position) if from_lane < to_lane => {
                        let mut candidate: Vec<Vec<usize>> = lanes.to_vec();
                        candidate[from_lane] = [
                            &lanes[from_lane][..=camp_position],
                            &lanes[to_lane][other_position + 1..],
                        ]
                        .concat();
                        candidate[to_lane] = [
                            &lanes[to_lane][..=other_position],
                            &lanes[from_lane][camp_position + 1..],
                        ]
                        .concat();
                        moves.push(candidate);
                    }
                    Some(_) => {}
                    None if lanes[from_lane].len() > 1 => {
                        let mut candidate: Vec<Vec<usize>> = lanes.to_vec();
                        candidate[from_lane].remove(camp_position);
                        let insert_position: usize = candidate[to_lane]
                            .partition_point(|&other| key(other) < key(camp_index));
                        candidate[to_lane].insert(insert_position, camp_index);
                        moves.push(candidate);
                    }
                    None => {}
                }
            }
            if lanes[from_lane].len() > 1 {
                let mut candidate: Vec<Vec<usize>> = lanes.to_vec();
                candidate[from_lane].remove(camp_position);
                moves.push(candidate);
            }
        }
    }
    moves
}

fn improve(
    lanes: &mut Vec<Vec<usize>>,
    distance_matrix: &CampSpawnDistanceMatrix,
    estimated_value: &[u16],
    allowed_nodes: &[bool],
    key: &dyn Fn(usize) -> Key,
) {
    if lanes.len() < 2 {
        return;
    }
    let camp_count: usize = estimated_value.len() - 2;
    let total_value: f64 = (0..camp_count)
        .filter(|&camp_index| allowed_nodes[camp_index])
        .map(|camp_index| estimated_value[camp_index] as f64)
        .sum();
    let mean_value: f64 = total_value / lanes.len() as f64;
    let balance_weight: f64 = if mean_value > 0.0 {
        SHARE_PRICE as f64 / (0.01 * mean_value * mean_value)
    } else {
        0.0
    };

    let mut best_cost: f64 = lane_cost(
        lanes,
        distance_matrix,
        estimated_value,
        allowed_nodes,
        balance_weight,
    );
    for _ in 0..MAX_BALANCE_ITERS {
        let best_candidate: Option<(f64, Vec<Vec<usize>>)> = candidates(lanes, key)
            .into_iter()
            .map(|candidate: Vec<Vec<usize>>| {
                let cost: f64 = lane_cost(
                    &candidate,
                    distance_matrix,
                    estimated_value,
                    allowed_nodes,
                    balance_weight,
                );
                (cost, candidate)
            })
            .min_by(|left, right| left.0.partial_cmp(&right.0).unwrap());
        match best_candidate {
            Some((cost, candidate)) if cost < best_cost - 1e-9 => {
                best_cost = cost;
                *lanes = candidate;
            }
            _ => break,
        }
    }
}
