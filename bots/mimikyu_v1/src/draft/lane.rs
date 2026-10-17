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

struct Dense {
    n: usize,
    d: Vec<u8>,
}

impl Dense {
    fn new(distance_matrix: &CampSpawnDistanceMatrix) -> Self {
        let camp_count: usize = distance_matrix.distances_to_camp.len();
        let n: usize = camp_count + 2;
        let mut d: Vec<u8> = Vec::with_capacity(n * n);
        for camp in 0..camp_count {
            d.extend_from_slice(&distance_matrix.distances_to_camp[camp]);
            d.push(distance_matrix.distances_to_spawns[camp][0]);
            d.push(distance_matrix.distances_to_spawns[camp][1]);
        }
        for spawn in 0..2 {
            for camp in 0..camp_count {
                d.push(distance_matrix.distances_to_spawns[camp][spawn]);
            }
            d.push(*distance_matrix.spawns_distance);
            d.push(*distance_matrix.spawns_distance);
        }
        Self { n, d }
    }

    #[inline(always)]
    fn get(&self, from: usize, to: usize) -> i32 {
        self.d[from * self.n + to] as i32
    }
}

fn sorted_by_distance(dense: &Dense, origin: usize, nodes: &[usize]) -> Vec<usize> {
    let mut counts: [usize; 257] = [0; 257];
    for &node in nodes {
        counts[dense.get(origin, node) as usize + 1] += 1;
    }
    for value in 1..257 {
        counts[value] += counts[value - 1];
    }
    let mut sorted: Vec<usize> = vec![0; nodes.len()];
    for &node in nodes {
        let slot: &mut usize = &mut counts[dense.get(origin, node) as usize];
        sorted[*slot] = node;
        *slot += 1;
    }
    sorted
}

fn limit_table() -> [i32; 256] {
    let mut table: [i32; 256] = [0; 256];
    for distance in 0..256 {
        table[distance] = ((1.0 + NEIGHBOUR_LINEAR_TOLERANCE) * distance as f32
            + NEIGHBOUR_CONSTANT_TOLERANCE)
            .floor() as i32;
    }
    table
}

fn all_near_lists(dense: &Dense) -> Vec<Vec<usize>> {
    let n: usize = dense.n;
    let mut lists: Vec<Vec<usize>> = Vec::with_capacity(n);
    for origin in 0..n {
        let row: &[u8] = &dense.d[origin * n..origin * n + n];
        let row_max: usize = row.iter().copied().max().unwrap_or(0) as usize;
        let mut counts: Vec<usize> = vec![0; row_max + 2];
        for &value in row {
            counts[value as usize + 1] += 1;
        }
        for value in 1..counts.len() {
            counts[value] += counts[value - 1];
        }
        let mut sorted: Vec<usize> = vec![0; n];
        for (node, &value) in row.iter().enumerate() {
            let slot: &mut usize = &mut counts[value as usize];
            sorted[*slot] = node;
            *slot += 1;
        }
        lists.push(sorted);
    }
    lists
}

fn neighbour_edges(
    dense: &Dense,
    nodes: &[usize],
    near: &[Vec<usize>],
    limits: &[i32; 256],
) -> Vec<(usize, usize)> {
    let n: usize = dense.n;
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for (position, &from) in nodes.iter().enumerate() {
        let row_from: &[u8] = &dense.d[from * n..from * n + n];
        let near_from: &[usize] = &near[from];
        for &to in &nodes[position + 1..] {
            let limit: i32 = limits[row_from[to] as usize];
            let mut blocked: bool = false;
            for &middle in near_from {
                let first_leg: i32 = row_from[middle] as i32;
                if first_leg > limit {
                    break;
                }
                if middle != from
                    && middle != to
                    && first_leg + dense.d[middle * n + to] as i32 <= limit
                {
                    blocked = true;
                    break;
                }
            }
            if !blocked {
                edges.push((from, to));
            }
        }
    }
    edges
}

fn gap_detour(
    dense: &Dense,
    spine: &[usize],
    ally: usize,
    enemy: usize,
    camp: usize,
    gap: usize,
) -> i32 {
    let previous: usize = if gap == 0 { ally } else { spine[gap - 1] };
    let next: usize = if gap == spine.len() {
        enemy
    } else {
        spine[gap]
    };
    dense.get(previous, camp) + dense.get(camp, next) - dense.get(previous, next)
}

fn best_gap(
    dense: &Dense,
    spine: &[usize],
    ally: usize,
    enemy: usize,
    camp: usize,
) -> (i32, usize) {
    (0..=spine.len())
        .map(|gap: usize| (gap_detour(dense, spine, ally, enemy, camp, gap), gap))
        .min()
        .unwrap()
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

        let dense: Dense = Dense::new(distance_matrix);

        let n: usize = dense.n;

        let limits: [i32; 256] = limit_table();
        let near: Vec<Vec<usize>> = all_near_lists(&dense);

        let all_nodes: Vec<usize> = (0..n).collect();
        let neighbour_graph: Vec<(usize, usize)> =
            neighbour_edges(&dense, &all_nodes, &near, &limits);

        let mut safe_nodes: Vec<usize> = (0_usize..n - 2)
            .filter(|&index: &usize| camp_graph.camp_group_safe[index])
            .collect();
        safe_nodes.extend([n - 2, n - 1]);
        let mut is_safe_node: Vec<bool> = vec![false; n];
        for &node in &safe_nodes {
            is_safe_node[node] = true;
        }
        let mut safe_near: Vec<Vec<usize>> = vec![Vec::new(); n];
        for &origin in &safe_nodes {
            safe_near[origin] = near[origin]
                .iter()
                .copied()
                .filter(|&node: &usize| is_safe_node[node])
                .collect();
        }
        let safe_neighbour_graph: Vec<(usize, usize)> =
            neighbour_edges(&dense, &safe_nodes, &safe_near, &limits);

        Self {
            value_lanes: Self::calculate_lanes(
                &neighbour_graph,
                &dense,
                &estimated_value,
                &spawn_delta,
                &vec![true; n],
            ),
            safe_lanes: Self::calculate_lanes(
                &safe_neighbour_graph,
                &dense,
                &estimated_value,
                &spawn_delta,
                &safe,
            ),
        }
    }

    fn calculate_lanes(
        edges: &[(usize, usize)],
        dense: &Dense,
        estimated_value: &[u16],
        spawn_delta: &[i16],
        allowed: &[bool],
    ) -> [Vec<usize>; N_LANES] {
        let node_count: usize = estimated_value.len();
        let camp_count: usize = node_count - 2;
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
                    .min_by_key(|&other: &usize| (dense.get(node, other), other))
                    .unwrap();
                successors[node].push(next);
                has_predecessor[next] = true;
            }
            if !has_predecessor[node] {
                let previous: usize = ordered
                    .iter()
                    .copied()
                    .filter(|&other: &usize| rank[other] < rank[node])
                    .min_by_key(|&other: &usize| (dense.get(other, node), other))
                    .unwrap();
                successors[previous].push(node);
                has_predecessor[node] = true;
            }
        }

        let dist = |first: usize, second: usize| -> i32 { dense.get(first, second) };

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
            let mut best_gaps: Vec<(i32, usize)> = pending
                .iter()
                .map(|&camp: &usize| best_gap(dense, &spine, ally, enemy, camp))
                .collect();

            loop {
                let mut choice: Option<(i32, usize, usize, usize)> = None;
                for (pending_index, &(detour, gap)) in best_gaps.iter().enumerate() {
                    let candidate: (i32, usize, usize, usize) =
                        (detour, pending[pending_index], gap, pending_index);
                    if choice.is_none_or(|current| {
                        (candidate.0, candidate.1, candidate.2) < (current.0, current.1, current.2)
                    }) {
                        choice = Some(candidate);
                    }
                }
                let Some((detour, camp, gap, pending_index)) = choice else {
                    break;
                };
                if detour > ABSORB_DETOUR {
                    break;
                }

                spine.insert(gap, camp);
                pending.swap_remove(pending_index);
                best_gaps.swap_remove(pending_index);

                let previous: usize = if gap == 0 { ally } else { spine[gap - 1] };
                let next: usize = if gap + 1 == spine.len() {
                    enemy
                } else {
                    spine[gap + 1]
                };
                for (index, &other) in pending.iter().enumerate() {
                    let (current_detour, current_gap): (i32, usize) = best_gaps[index];
                    if current_gap == gap {
                        best_gaps[index] = best_gap(dense, &spine, ally, enemy, other);
                        continue;
                    }
                    let shifted_gap: usize = if current_gap > gap {
                        current_gap + 1
                    } else {
                        current_gap
                    };
                    let mut best: (i32, usize) = (current_detour, shifted_gap);
                    let left: (i32, usize) = (
                        dist(previous, other) + dist(other, camp) - dist(previous, camp),
                        gap,
                    );
                    let right: (i32, usize) = (
                        dist(camp, other) + dist(other, next) - dist(camp, next),
                        gap + 1,
                    );
                    if left < best {
                        best = left;
                    }
                    if right < best {
                        best = right;
                    }
                    best_gaps[index] = best;
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
