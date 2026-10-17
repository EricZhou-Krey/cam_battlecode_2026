use crate::draft::camp_graph::{CampGraph, CampSpawnDistanceMatrix};

const NEIGHBOUR_LINEAR_TOLERANCE: f32 = 0.2;
const NEIGHBOUR_CONSTANT_TOLERANCE: f32 = 2.0;

const ABSORB_DETOUR: i32 = 8;
const N_LANES: usize = 8;
const MAX_ATTEMPTS: usize = 24;
const DISTANCE_WEIGHT: i32 = 4;
const VALUE_WEIGHT: i32 = 2;
const REPEAT_PENALTY: i32 = 40;
pub const MAX_LANE_CAMPS: usize = 10;
pub const NOT_SPANNING_FLAG: u8 = 0x80;

#[derive(Debug, Clone, PartialEq)]
pub struct LaneGraph {
    pub value_lanes: [Vec<usize>; 8],
    pub safe_lanes: [Vec<usize>; 8],
}

impl LaneGraph {
    pub fn new(camp_graph: &CampGraph, distance_matrix: &CampSpawnDistanceMatrix) -> Self {
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

        let mut neighbour_graph: Vec<(usize, usize)> = Vec::new();
        let n: usize = distance_matrix.len();
        for from_index in 0..n {
            for to_index in from_index + 1..n {
                let max_distance: f32 = (1.0 + NEIGHBOUR_LINEAR_TOLERANCE)
                    * distance_matrix.get(from_index, to_index) as f32
                    + NEIGHBOUR_CONSTANT_TOLERANCE;
                if (0..n).any(|inbetween_index: usize| {
                    inbetween_index != from_index
                        && inbetween_index != to_index
                        && (distance_matrix.get(from_index, inbetween_index)
                            + distance_matrix.get(inbetween_index, to_index))
                            as f32
                            <= max_distance
                }) {
                    continue;
                }
                neighbour_graph.push((from_index, to_index));
            }
        }

        let mut safe_neighbour_graph: Vec<(usize, usize)> = Vec::new();
        let mut filtered_indicies: Vec<usize> = (0_usize..n - 2)
            .filter(|i| camp_graph.camp_group_safe[*i])
            .collect();
        filtered_indicies.extend([n - 2, n - 1]);
        for (filtered_index, &from_index) in filtered_indicies.iter().enumerate() {
            for &to_index in filtered_indicies[filtered_index..].iter() {
                let max_distance: f32 = (1.0 + NEIGHBOUR_LINEAR_TOLERANCE)
                    * distance_matrix.get(from_index, to_index) as f32
                    + NEIGHBOUR_CONSTANT_TOLERANCE;
                if filtered_indicies.iter().any(|&inbetween_index: &usize| {
                    inbetween_index != from_index
                        && inbetween_index != to_index
                        && (distance_matrix.get(from_index, inbetween_index)
                            + distance_matrix.get(inbetween_index, to_index))
                            as f32
                            <= max_distance
                }) {
                    continue;
                }
                safe_neighbour_graph.push((from_index, to_index));
            }
        }

        Self {
            value_lanes: Self::calculate_lanes(
                &neighbour_graph,
                distance_matrix,
                camp_graph,
                &estimated_value,
                &spawn_delta,
                &vec![true; n],
            ),
            safe_lanes: Self::calculate_lanes(
                &safe_neighbour_graph,
                distance_matrix,
                camp_graph,
                &estimated_value,
                &spawn_delta,
                &safe,
            ),
        }
    }

    fn calculate_lanes(
        edges: &[(usize, usize)],
        distance_matrix: &CampSpawnDistanceMatrix,
        camp_graph: &CampGraph,
        estimated_value: &[u16],
        spawn_delta: &[i16],
        allowed: &[bool],
    ) -> [Vec<usize>; N_LANES] {
        let camp_count: usize = camp_graph.camp_group_centers.len();
        let node_count: usize = camp_count + 2;
        let ally: usize = camp_count;
        let enemy: usize = camp_count + 1;
        let mut out: [Vec<usize>; N_LANES] = [const { Vec::new() }; N_LANES];

        let mut ordered: Vec<usize> = (0..node_count)
            .filter(|&node: &usize| allowed[node])
            .collect();
        if ordered.len() <= 2 {
            return out;
        }
        let progress = |node: usize| -> i32 {
            if node == ally {
                i32::MIN
            } else if node == enemy {
                i32::MAX
            } else {
                spawn_delta[node] as i32
            }
        };
        ordered.sort_unstable_by_key(|&node: &usize| (progress(node), node));
        let mut rank: Vec<usize> = vec![0; node_count];
        for (position, &node) in ordered.iter().enumerate() {
            rank[node] = position;
        }

        let mut successors: Vec<Vec<usize>> = vec![Vec::new(); node_count];
        let mut has_predecessor: Vec<bool> = vec![false; node_count];
        for &(first, second) in edges {
            if first == second || !allowed[first] || !allowed[second] {
                continue;
            }
            if (first == ally && second == enemy) || (first == enemy && second == ally) {
                continue;
            }
            let (from_node, to_node): (usize, usize) = if rank[first] < rank[second] {
                (first, second)
            } else {
                (second, first)
            };
            if successors[from_node].contains(&to_node) {
                continue;
            }
            successors[from_node].push(to_node);
            has_predecessor[to_node] = true;
        }

        for &node in ordered.iter().filter(|&&node| node < camp_count) {
            if successors[node].is_empty() {
                let next: usize = ordered
                    .iter()
                    .copied()
                    .filter(|&other: &usize| rank[other] > rank[node])
                    .min_by_key(|&other: &usize| (distance_matrix.get(node, other), other))
                    .unwrap();
                successors[node].push(next);
                has_predecessor[next] = true;
            }
            if !has_predecessor[node] {
                let previous: usize = ordered
                    .iter()
                    .copied()
                    .filter(|&other: &usize| rank[other] < rank[node])
                    .min_by_key(|&other: &usize| (distance_matrix.get(other, node), other))
                    .unwrap();
                successors[previous].push(node);
                has_predecessor[node] = true;
            }
        }

        let dist =
            |first: usize, second: usize| -> i32 { distance_matrix.get(first, second) as i32 };

        let shortest_lane = |uses: &[i32], value_weight: i32| -> Option<Vec<usize>> {
            let mut best: Vec<i32> = vec![i32::MAX; node_count];
            let mut via: Vec<usize> = vec![usize::MAX; node_count];
            best[ally] = 0;
            for &from_node in &ordered {
                if best[from_node] == i32::MAX {
                    continue;
                }
                for &to_node in &successors[from_node] {
                    let step: i32 = DISTANCE_WEIGHT * dist(from_node, to_node);
                    let cost: i32 = if to_node == enemy {
                        step
                    } else {
                        step + REPEAT_PENALTY * uses[to_node]
                            - value_weight * estimated_value[to_node] as i32 / (1 + uses[to_node])
                    };
                    let total: i32 = best[from_node] + cost;
                    if total < best[to_node] {
                        best[to_node] = total;
                        via[to_node] = from_node;
                    }
                }
            }
            if best[enemy] == i32::MAX {
                return None;
            }
            let mut lane: Vec<usize> = Vec::new();
            let mut node: usize = via[enemy];
            while node != ally {
                lane.push(node);
                node = via[node];
            }
            lane.reverse();
            Some(lane)
        };

        let mut uses: Vec<i32> = vec![0; node_count];
        let mut lanes: Vec<Vec<usize>> = Vec::new();

        if let Some(mut spine) = shortest_lane(&uses, 0) {
            let mut pending: Vec<usize> = ordered
                .iter()
                .copied()
                .filter(|&node: &usize| node < camp_count && !spine.contains(&node))
                .collect();
            loop {
                let mut best_insert: Option<(i32, usize, usize)> = None;
                for &camp in &pending {
                    for position in 0..=spine.len() {
                        let previous: usize = if position == 0 {
                            ally
                        } else {
                            spine[position - 1]
                        };
                        let next: usize = if position == spine.len() {
                            enemy
                        } else {
                            spine[position]
                        };
                        let detour: i32 =
                            dist(previous, camp) + dist(camp, next) - dist(previous, next);
                        let candidate: (i32, usize, usize) = (detour, camp, position);
                        if best_insert.is_none_or(|current| candidate < current) {
                            best_insert = Some(candidate);
                        }
                    }
                }
                match best_insert {
                    Some((detour, camp, position)) if detour <= ABSORB_DETOUR => {
                        spine.insert(position, camp);
                        pending.retain(|&other: &usize| other != camp);
                    }
                    _ => break,
                }
            }

            let outer: Option<Vec<usize>> = if pending.is_empty() {
                None
            } else {
                let mut keyed: Vec<(i32, i32, usize)> = pending
                    .iter()
                    .map(|&camp: &usize| {
                        let (depth, attach): (i32, usize) = spine
                            .iter()
                            .map(|&spine_camp: &usize| (dist(camp, spine_camp), spine_camp))
                            .min()
                            .unwrap();
                        let attach_progress: i32 = spawn_delta[attach] as i32;
                        let signed_depth: i32 = if attach_progress < 0 { depth } else { -depth };
                        (attach_progress, signed_depth, camp)
                    })
                    .collect();
                keyed.sort_unstable();
                Some(keyed.into_iter().map(|(_, _, camp)| camp).collect())
            };

            for &camp in &spine {
                uses[camp] += 1;
            }
            lanes.push(spine);
            if let Some(outer) = outer {
                for &camp in &outer {
                    uses[camp] += 1;
                }
                lanes.push(outer);
            }
        }

        for _ in 0..MAX_ATTEMPTS {
            if lanes.len() == N_LANES {
                break;
            }
            let Some(lane) = shortest_lane(&uses, VALUE_WEIGHT) else {
                break;
            };
            for &camp in &lane {
                uses[camp] += 1;
            }
            if !lane.is_empty() && !lanes.contains(&lane) {
                lanes.push(lane);
            }
        }

        if !lanes.is_empty() && lanes[0].len() > MAX_LANE_CAMPS {
            let overflow: Vec<usize> = lanes[0].split_off(MAX_LANE_CAMPS);
            if lanes.len() == 1 {
                lanes.push(Vec::new());
            }
            for camp in overflow {
                if !lanes[1].contains(&camp) {
                    lanes[1].push(camp);
                }
            }
        }

        for (slot, lane) in out.iter_mut().zip(lanes) {
            *slot = lane;
        }
        out
    }

    pub fn spanning_lane_index(
        &self,
        camp_graph: &CampGraph,
        max_camps_per_lane: usize,
    ) -> [u8; 2] {
        let camp_count: usize = camp_graph.camp_group_centers.len();
        let all_camps: Vec<bool> = vec![true; camp_count];
        [
            Self::spanning_count(&self.value_lanes, &all_camps, max_camps_per_lane),
            Self::spanning_count(
                &self.safe_lanes,
                &camp_graph.camp_group_safe,
                max_camps_per_lane,
            ),
        ]
    }

    fn spanning_count(lanes: &[Vec<usize>], required: &[bool], max_camps_per_lane: usize) -> u8 {
        let mut remaining: usize = required.iter().filter(|&&is_required| is_required).count();
        if remaining == 0 {
            return 0;
        }
        let mut covered: Vec<bool> = vec![false; required.len()];
        let mut non_empty_lanes: usize = 0;
        for (lane_index, lane) in lanes.iter().enumerate() {
            if lane.is_empty() {
                continue;
            }
            non_empty_lanes = lane_index + 1;
            for &camp in lane.iter().take(max_camps_per_lane) {
                if required[camp] && !covered[camp] {
                    covered[camp] = true;
                    remaining -= 1;
                }
            }
            if remaining == 0 {
                return (lane_index + 1) as u8;
            }
        }
        non_empty_lanes as u8 | NOT_SPANNING_FLAG
    }
}
