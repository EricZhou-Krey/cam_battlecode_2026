use crate::draft::camp_graph::{CampGraph, CampSpawnDistanceMatrix};

const CAMP_FALLBACK_LIMIT: usize = 20;

const NEIGHBOUR_LINEAR_TOLERANCE: f32 = 0.2;
const NEIGHBOUR_CONSTANT_TOLERANCE: f32 = 2.0;

const ABSORB_DETOUR: i32 = 8;
const N_LANES: usize = 8;
pub const COMPUTED_LANES: usize = 8;
const MAX_ATTEMPTS: usize = COMPUTED_LANES * 3;
const DISTANCE_WEIGHT: i32 = 4;
const VALUE_WEIGHT: i32 = 2;
const REPEAT_PENALTY: i32 = 40;
pub const MAX_LANE_CAMPS: usize = 10;
pub const NOT_SPANNING_FLAG: u8 = 0x0F;

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

fn limit_table() -> [i32; 256] {
    let mut table: [i32; 256] = [0; 256];
    (0..256).for_each(|distance| {
        table[distance] = ((1.0 + NEIGHBOUR_LINEAR_TOLERANCE) * distance as f32
            + NEIGHBOUR_CONSTANT_TOLERANCE)
            .floor() as i32;
    });
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

fn refresh_cost(node_cost: &mut [i32], uses: &[i32], estimated_value: &[u16], camp: usize) {
    node_cost[camp] = REPEAT_PENALTY * uses[camp]
        - VALUE_WEIGHT * estimated_value[camp] as i32 / (1 + uses[camp]);
}
struct Dp<'a> {
    ordered: &'a [usize],
    offsets: &'a [usize],
    targets: &'a [usize],
    steps: &'a [i32],
    node_cost: &'a [i32],
    best: &'a mut [i32],
    via: &'a mut [usize],
}

impl Dp<'_> {
    fn run(&mut self, ally: usize, enemy: usize) -> Option<Vec<usize>> {
        self.best.fill(i32::MAX);
        self.via.fill(usize::MAX);
        self.best[ally] = 0;

        for (position, &from_node) in self.ordered.iter().enumerate() {
            let base = self.best[from_node];
            if base == i32::MAX {
                continue;
            }

            for edge in self.offsets[position]..self.offsets[position + 1] {
                let to_node = self.targets[edge];
                let total = base + self.steps[edge] + self.node_cost[to_node];

                if total < self.best[to_node] {
                    self.best[to_node] = total;
                    self.via[to_node] = from_node;
                }
            }
        }

        if self.best[enemy] == i32::MAX {
            return None;
        }

        let mut lane = Vec::new();
        let mut node = enemy;
        while node != ally {
            if node < self.ordered.len() - 2 {
                lane.push(node);
            }
            node = self.via[node];
        }
        lane.reverse();
        Some(lane)
    }
}

impl LaneGraph {
    pub fn new(camp_graph: &CampGraph, distance_matrix: &CampSpawnDistanceMatrix) -> Self {
        if camp_graph.camp_group_centers.len() > CAMP_FALLBACK_LIMIT {
            return Self::lane_fallback(camp_graph, distance_matrix);
        }

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

        let mut offsets: Vec<usize> = Vec::with_capacity(ordered.len() + 1);
        let mut targets: Vec<usize> = Vec::new();
        let mut steps: Vec<i32> = Vec::new();
        offsets.push(0);
        for &from_node in &ordered {
            for &to_node in &successors[from_node] {
                targets.push(to_node);
                steps.push(DISTANCE_WEIGHT * dist(from_node, to_node));
            }
            offsets.push(targets.len());
        }

        let zero_cost: Vec<i32> = vec![0; node_count];
        let mut variant_cost: Vec<i32> = (0..node_count)
            .map(|node: usize| {
                if node < camp_count {
                    -(VALUE_WEIGHT * estimated_value[node] as i32)
                } else {
                    0
                }
            })
            .collect();
        let mut best_buffer: Vec<i32> = vec![i32::MAX; node_count];
        let mut via_buffer: Vec<usize> = vec![usize::MAX; node_count];
        let mut uses: Vec<i32> = vec![0; node_count];
        let mut lanes: Vec<Vec<usize>> = Vec::new();

        let spine_lane: Option<Vec<usize>> = Dp {
            ordered: &ordered,
            offsets: &offsets,
            targets: &targets,
            steps: &steps,
            node_cost: &zero_cost,
            best: &mut best_buffer,
            via: &mut via_buffer,
        }
        .run(ally, enemy);

        if let Some(mut spine) = spine_lane {
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
                refresh_cost(&mut variant_cost, &uses, estimated_value, camp);
            }
            lanes.push(spine);
            if let Some(outer) = outer {
                for &camp in &outer {
                    uses[camp] += 1;
                    refresh_cost(&mut variant_cost, &uses, estimated_value, camp);
                }
                lanes.push(outer);
            }
        }

        for _ in 0..MAX_ATTEMPTS {
            if lanes.len() >= COMPUTED_LANES {
                break;
            }
            let variant_lane: Option<Vec<usize>> = Dp {
                ordered: &ordered,
                offsets: &offsets,
                targets: &targets,
                steps: &steps,
                node_cost: &variant_cost,
                best: &mut best_buffer,
                via: &mut via_buffer,
            }
            .run(ally, enemy);

            let Some(lane) = variant_lane else {
                break;
            };
            for &camp in &lane {
                uses[camp] += 1;
                refresh_cost(&mut variant_cost, &uses, estimated_value, camp);
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

        lanes.truncate(COMPUTED_LANES);
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
        for (lane_index, lane) in lanes.iter().enumerate() {
            if lane.is_empty() {
                continue;
            }
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

        NOT_SPANNING_FLAG
    }

    fn lane_fallback(
        camp_graph: &CampGraph,
        distance_matrix: &CampSpawnDistanceMatrix,
    ) -> LaneGraph {
        let dense: Dense = Dense::new(distance_matrix);
        let camp_count: usize = camp_graph.camp_group_centers.len();
        let n: usize = dense.n;
        let ally: usize = camp_count;

        let generate_fallback_lanes = |allowed_nodes: &[bool]| -> [Vec<usize>; N_LANES] {
            let mut out: [Vec<usize>; N_LANES] = [const { Vec::new() }; N_LANES];
            let mut unvisited_camps: Vec<usize> = (0..camp_count)
                .filter(|&camp: &usize| allowed_nodes[camp])
                .collect();

            (0..N_LANES).for_each(|lane_slot| {
                if unvisited_camps.is_empty() {
                    return;
                }
                let mut current_lane: Vec<usize> = Vec::new();
                let mut current_node: usize = ally;

                while !unvisited_camps.is_empty() {
                    let next_index: usize = unvisited_camps
                        .iter()
                        .copied()
                        .min_by_key(|&camp: &usize| dense.get(current_node, camp))
                        .unwrap();

                    unvisited_camps.retain(|&c: &usize| c != next_index);
                    current_lane.push(next_index);
                    current_node = next_index;

                    if current_lane.len() >= MAX_LANE_CAMPS {
                        break;
                    }
                }
                out[lane_slot] = current_lane;
            });
            out
        };

        let mut safe: Vec<bool> = camp_graph.camp_group_safe.clone();
        safe.extend([true, true]);
        let all_allowed: Vec<bool> = vec![true; n];

        let value_lanes: [Vec<usize>; N_LANES] = generate_fallback_lanes(&all_allowed);
        let safe_lanes: [Vec<usize>; N_LANES] = generate_fallback_lanes(&safe);

        Self {
            value_lanes,
            safe_lanes,
        }
    }
}
