//! route.rs - shortcut / dash / teleport aware routing on top of walking DistanceFields.
//!
//! Gaps (holes, thin walls) are handled only where they matter, never globally:
//!   * planned:       notable gaps from `LazyPointsOfInterest::shortcuts` become portal edges in
//!                    a tiny graph (Start, shortcut points, Goal). One plan is O(S^2) for S
//!                    shortcut points, using only `DistanceField::get` lookups.
//!   * opportunistic: after each walk step, an off-cooldown dash may skip ahead, scored with a
//!                    16-tile local stencil against the current leg's field.
//!
//! The planner never performs SDK actions. It returns orders; the caller executes them.
//! Walking distance == turns (1 tile per turn). All times below are in turns.

use crate::action::movement::{MovementState, MAX_ROUTE_FAILURES};
use crate::memory::champion_memory::ChampionMemory;
use crate::memory::lazy_map::{LazyDistanceFields, LazyPointsOfInterest}; // adjust to your module path
use crate::memory::{LocationID, ShortcutID};
use crate::sdk_interact::map::step_with_fallback;
use cambc::{Bitboard, Champion, ConsumableType, DistanceField, Vec2};
use std::collections::HashMap;

pub const DASH_RANGE: i32 = 2;
pub const TP_RANGE: i32 = 3;
pub const DASH_COOLDOWN: u16 = 8;
pub const TP_CHANNEL_TURNS: u16 = 3;

const UNKNOWN: u16 = u16::MAX; // matrix entry not computed yet
const UNREACHABLE: u16 = u16::MAX - 1; // matrix entry computed, no walking path
const NO_DASH: u16 = u16::MAX; // `ready_at` value meaning "this champion has no dash"

// ---------------------------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------------------------

/// What the champion can do right now. Build with [`capabilities`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// None = no dash ability. Some(0) = ready now. Some(n) = ready in n turns.
    pub dash_ready_in: Option<u16>,
    pub has_tp: bool,
}

/// Anything that can hand out the walking field that leads to a location.
pub trait FieldSource {
    fn field(&self, id: LocationID) -> Option<&DistanceField>;
}

impl FieldSource for LazyDistanceFields {
    fn field(&self, id: LocationID) -> Option<&DistanceField> {
        self.weak_get(id)
    }
}

/// Walking turns from `at` to the field's roots; None when unreached (assumes unreached == MAX).
pub fn walk_dist(field: &DistanceField, at: Vec2) -> Option<u16> {
    match field.get(at) {
        Ok(d) if d != u16::MAX => Some(d),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Static geometry (dash legality, gap classification)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossKind {
    Dash,
    Teleport,
}

/// How a gap between two tiles can be crossed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gap {
    pub dash: bool,
    pub tp: bool,
}

fn cheb(a: Vec2, b: Vec2) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

#[derive(Debug, Clone, PartialEq)]
pub struct Geometry {
    walls: Bitboard,
    /// Tiles you cannot land on: walls | holes | altars (your `blocked_bitboard`).
    blocked: Bitboard,
}

impl Geometry {
    pub fn new(walls: Bitboard, blocked: Bitboard) -> Self {
        Self { walls, blocked }
    }

    pub fn from_blocked(blocked: &Bitboard) -> Self {
        Self {
            walls: cambc::map::walls().unwrap().clone(),
            blocked: blocked.clone(),
        }
    }

    fn is_wall(&self, p: Vec2) -> bool {
        self.walls.is_set(p).unwrap_or(true)
    }

    fn is_blocked(&self, p: Vec2) -> bool {
        self.blocked.is_set(p).unwrap_or(true)
    }

    /// Legal dash: <= 2 king steps, landing free, no wall on the middle tile (holes are fine).
    /// Rules only list walls as undashable; altars/units in the middle are treated as passable.
    pub fn dash_path_ok(&self, from: Vec2, to: Vec2) -> bool {
        let range = cheb(from, to);
        if range == 0 || range > DASH_RANGE || self.is_blocked(to) {
            return false;
        }
        if range == 1 {
            return true;
        }
        (-1..=1).any(|dx| {
            (-1..=1).any(|dy| {
                let mid = Vec2 {
                    x: from.x + dx,
                    y: from.y + dy,
                };
                (dx, dy) != (0, 0) && cheb(mid, to) == 1 && !self.is_wall(mid)
            })
        })
    }

    pub fn gap(&self, from: Vec2, to: Vec2) -> Gap {
        let range = cheb(from, to);
        Gap {
            dash: self.dash_path_ok(from, to),
            tp: range > 0 && range <= TP_RANGE && !self.is_blocked(to),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Shortcut graph (built once per map)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct ShortcutGraph {
    /// Index == ShortcutID.0 (same order as `LazyPointsOfInterest::shortcut_points`).
    pub points: Vec<Vec2>,
    /// The other end of each shortcut (None if the map lists a one-way / unknown partner).
    pub partner: Vec<Option<usize>>,
    /// How `points[i] -> points[partner[i]]` can be crossed.
    pub gap: Vec<Gap>,
    /// Lazily filled walking turns [from * n + to], read from the field that leads to `to`.
    walk: Vec<u16>,
}

impl ShortcutGraph {
    pub fn new(poi: &LazyPointsOfInterest, geo: &Geometry) -> Self {
        let points: Vec<Vec2> = poi.shortcut_points.clone();
        let index_of: HashMap<Vec2, usize> = points
            .iter()
            .copied()
            .enumerate()
            .map(|(i, p)| (p, i))
            .collect();
        let partner: Vec<Option<usize>> = points
            .iter()
            .map(|p| poi.shortcuts.get(p).and_then(|q| index_of.get(q).copied()))
            .collect();
        let gap: Vec<Gap> = points
            .iter()
            .zip(&partner)
            .map(|(&p, q)| match q {
                Some(j) => geo.gap(p, points[*j]),
                None => Gap {
                    dash: false,
                    tp: false,
                },
            })
            .collect();
        let n = points.len();
        Self {
            points,
            partner,
            gap,
            walk: vec![UNKNOWN; n * n],
        }
    }

    /// Walking turns from shortcut point `a` to shortcut point `b`. None = unknown or unreachable.
    /// Cached: costs one `get` the first time a pair is needed (and its field exists).
    fn walk_between(&mut self, a: usize, b: usize, fields: &impl FieldSource) -> Option<u16> {
        let n = self.points.len();
        if self.walk[a * n + b] == UNKNOWN {
            let field = fields.field(LocationID::Shortcut(ShortcutID(b)))?;
            self.walk[a * n + b] = walk_dist(field, self.points[a]).unwrap_or(UNREACHABLE);
        }
        let d = self.walk[a * n + b];
        (d != UNREACHABLE).then_some(d)
    }
}

// ---------------------------------------------------------------------------------------------
// Route
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Leg {
    /// Walk down the field that leads to `via` until within `accepted` of it.
    Walk { via: LocationID, accepted: u16 },
    /// Stand on `from`, then dash / teleport to `to`.
    Cross {
        from: Vec2,
        to: Vec2,
        kind: CrossKind,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub legs: Vec<Leg>,
    pub cursor: usize,
    /// Planner estimate in turns, for comparing against recall etc.
    pub est_turns: u16,
}

/// What to do this turn, as decided by [`Route::advance`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    /// Take a walk step along the field leading to this location.
    Walk(LocationID),
    /// Standing on the dash entry tile: cast the dash to `to`.
    Dash {
        to: Vec2,
    },
    /// Standing on the teleport entry tile: start the crystal channel to `to`.
    Teleport {
        to: Vec2,
    },
    /// Not where the route expects (pushed, dash skip, blocked): replan.
    OffRoute,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DashSkipPolicy {
    /// Minimum walking turns a dash must save. A straight dash along the path saves 2.
    pub min_gain: u16,
    /// Hold the dash if a planned dash crossing is within this many walking turns.
    pub reserve_turns: u16,
}

impl Default for DashSkipPolicy {
    fn default() -> Self {
        Self {
            min_gain: 2,
            reserve_turns: DASH_COOLDOWN,
        }
    }
}

pub fn dash_skip(
    position: Vec2,
    field: &DistanceField,
    geometry: &Geometry,
    policy: &DashSkipPolicy,
) -> Option<Vec2> {
    let here: u16 = walk_dist(field, position)?;
    let mut candidates: Vec<(u16, Vec2)> = Vec::with_capacity(16);
    for dx in -DASH_RANGE..=DASH_RANGE {
        for dy in -DASH_RANGE..=DASH_RANGE {
            if dx.abs().max(dy.abs()) != DASH_RANGE {
                continue;
            }
            let landing: Vec2 = Vec2 {
                x: position.x + dx,
                y: position.y + dy,
            };
            if let Some(distance) = walk_dist(field, landing) {
                if distance + policy.min_gain <= here {
                    candidates.push((distance, landing));
                }
            }
        }
    }
    candidates.sort_by_key(|&(distance, _)| distance);
    candidates
        .into_iter()
        .find(|&(_, landing)| geometry.dash_path_ok(position, landing))
        .map(|(_, landing)| landing)
}

impl Route {
    pub fn walk_only(goal: LocationID, accepted: u16, est_turns: u16) -> Self {
        Self {
            legs: vec![Leg::Walk {
                via: goal,
                accepted,
            }],
            cursor: 0,
            est_turns,
        }
    }

    /// Skips completed legs and says what to do at `pos`. Cheap: at most one `get` per call.
    pub fn advance(&mut self, pos: Vec2, fields: &impl FieldSource) -> Phase {
        loop {
            let Some(leg) = self.legs.get(self.cursor).cloned() else {
                return Phase::Finished;
            };
            match leg {
                Leg::Walk { via, accepted } => {
                    let Some(field) = fields.field(via) else {
                        return Phase::OffRoute;
                    };
                    match walk_dist(field, pos) {
                        Some(d) if d <= accepted => self.cursor += 1,
                        Some(_) => return Phase::Walk(via),
                        None => return Phase::OffRoute,
                    }
                }
                Leg::Cross { from, to, kind } => {
                    if pos == to {
                        self.cursor += 1;
                    } else if pos == from {
                        return match kind {
                            CrossKind::Dash => Phase::Dash { to },
                            CrossKind::Teleport => Phase::Teleport { to },
                        };
                    } else {
                        return Phase::OffRoute;
                    }
                }
            }
        }
    }

    /// Call AFTER this turn's walk step, with the champion's NEW position.
    /// Returns a dash destination: either the planned crossing, or an opportunistic skip.
    pub fn dash_after_walk(
        &mut self,
        pos: Vec2,
        dash_ready: bool,
        fields: &impl FieldSource,
        geo: &Geometry,
        policy: &DashSkipPolicy,
    ) -> Option<Vec2> {
        if !dash_ready {
            return None;
        }
        let via = match self.advance(pos, fields) {
            Phase::Dash { to } => return Some(to), // planned crossing wins
            Phase::Walk(via) => via,
            _ => return None,
        };
        if self.dash_reserved(pos, fields, policy) {
            return None;
        }
        let field = fields.field(via)?;
        dash_skip(pos, field, geo, policy)
    }

    /// True when spending the dash now would leave it on cooldown for a planned dash crossing.
    fn dash_reserved(&self, pos: Vec2, fields: &impl FieldSource, policy: &DashSkipPolicy) -> bool {
        if let (
            Some(Leg::Walk { via, .. }),
            Some(Leg::Cross {
                kind: CrossKind::Dash,
                ..
            }),
        ) = (self.legs.get(self.cursor), self.legs.get(self.cursor + 1))
        {
            if let Some(d) = fields.field(*via).and_then(|f| walk_dist(f, pos)) {
                return d <= policy.reserve_turns;
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------------------------
// Planner
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Via {
    Start,
    Walk(u16),
    Cross(CrossKind),
}

#[derive(Clone, Copy)]
struct Label {
    t: u16,
    /// Absolute turn at which the dash is ready again (NO_DASH = no dash ability).
    ready_at: u16,
    parent: usize,
    via: Via,
    done: bool,
}

fn relax(labels: &mut [Option<Label>], st: usize, t: u16, ready_at: u16, parent: usize, via: Via) {
    let better = match labels[st] {
        None => true,
        Some(l) => !l.done && t < l.t,
    };
    if better {
        labels[st] = Some(Label {
            t,
            ready_at,
            parent,
            via,
            done: false,
        });
    }
}

fn sid(node: usize, tp_left: bool) -> usize {
    node * 2 + tp_left as usize
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoutePlanner {
    pub graph: ShortcutGraph,
    pub geometry: Geometry,
}

impl RoutePlanner {
    pub fn new(poi: &LazyPointsOfInterest, blocked: &Bitboard) -> Self {
        let geometry = Geometry::from_blocked(blocked);
        let graph = ShortcutGraph::new(poi, &geometry);
        Self { graph, geometry }
    }

    /// Plan from `start` to within `accepted` walking tiles of `goal`.
    ///
    /// Search space: (shortcut point, teleport still available) plus the start node. Edges:
    ///   walk       start/exit -> entry k          cost = cached walking turns
    ///   dash       entry -> partner               cost 0 (same turn as the walk step that got
    ///                                             there) + wait for cooldown; cooldown restarts
    ///   teleport   entry -> partner               cost TP_CHANNEL_TURNS, consumes the crystal
    ///   finish     point -> goal                  cost = goal field distance - accepted
    /// Missing fields (not discovered yet) simply remove those edges, so the result is always
    /// valid but may improve once more fields exist. Returns None if the goal is unreachable.
    pub fn plan(
        &mut self,
        start: Vec2,
        goal: LocationID,
        accepted: u16,
        caps: Capabilities,
        fields: &impl FieldSource,
    ) -> Option<Route> {
        let goal_field = fields.field(goal)?;
        let s = self.graph.points.len();
        let start_node = s;
        let n_states = (s + 1) * 2;
        let dash_ok = caps.dash_ready_in.is_some();

        let mut labels: Vec<Option<Label>> = vec![None; n_states];
        let start_state = sid(start_node, caps.has_tp);
        labels[start_state] = Some(Label {
            t: 0,
            ready_at: caps.dash_ready_in.unwrap_or(NO_DASH),
            parent: start_state,
            via: Via::Start,
            done: false,
        });

        let mut best: Option<(u16, usize)> = None;

        loop {
            let Some(cur) = (0..n_states)
                .filter(|&i| matches!(labels[i], Some(l) if !l.done))
                .min_by_key(|&i| labels[i].map(|l| l.t).unwrap_or(u16::MAX))
            else {
                break;
            };
            let lab = labels[cur].unwrap();
            if let Some((total, _)) = best {
                if lab.t >= total {
                    break;
                }
            }
            labels[cur].as_mut().unwrap().done = true;

            let node = cur / 2;
            let tp_left = cur % 2 == 1;
            let here = if node == start_node {
                start
            } else {
                self.graph.points[node]
            };

            // finish: walk the rest of the way to the goal
            if let Some(d) = walk_dist(goal_field, here) {
                let total = lab.t.saturating_add(d.saturating_sub(accepted));
                if best.map_or(true, |(b, _)| total < b) {
                    best = Some((total, cur));
                }
            }

            // walk to another entry point that we could actually cross from
            for k in 0..s {
                let g = self.graph.gap[k];
                if k == node
                    || self.graph.partner[k].is_none()
                    || !((g.dash && dash_ok) || (g.tp && tp_left))
                {
                    continue;
                }
                let w = if node == start_node {
                    fields
                        .field(LocationID::Shortcut(ShortcutID(k)))
                        .and_then(|f| walk_dist(f, start))
                } else {
                    self.graph.walk_between(node, k, fields)
                };
                if let Some(w) = w {
                    relax(
                        &mut labels,
                        sid(k, tp_left),
                        lab.t.saturating_add(w),
                        lab.ready_at,
                        cur,
                        Via::Walk(w),
                    );
                }
            }

            // cross the gap at this shortcut point
            if node != start_node {
                if let Some(j) = self.graph.partner[node] {
                    let g = self.graph.gap[node];
                    if g.dash && lab.ready_at != NO_DASH {
                        let wait = lab.ready_at.saturating_sub(lab.t);
                        // Dash rides on the walk step that reached the tile; standing on it at
                        // t == 0 means the dash is the whole turn.
                        let own_turn = if lab.t == 0 { 1 } else { 0 };
                        let t2 = lab.t.saturating_add(wait).saturating_add(own_turn);
                        relax(
                            &mut labels,
                            sid(j, tp_left),
                            t2,
                            t2.saturating_add(DASH_COOLDOWN),
                            cur,
                            Via::Cross(CrossKind::Dash),
                        );
                    }
                    if g.tp && tp_left {
                        relax(
                            &mut labels,
                            sid(j, false),
                            lab.t.saturating_add(TP_CHANNEL_TURNS),
                            lab.ready_at,
                            cur,
                            Via::Cross(CrossKind::Teleport),
                        );
                    }
                }
            }
        }

        let (total, mut st) = best?;

        // rebuild legs (walk back through parents)
        let mut steps: Vec<(usize, usize, Via)> = Vec::new();
        while let Some(l) = labels[st] {
            if matches!(l.via, Via::Start) {
                break;
            }
            steps.push((st / 2, l.parent / 2, l.via));
            st = l.parent;
        }
        steps.reverse();

        let mut legs: Vec<Leg> = Vec::with_capacity(steps.len() + 1);
        for (node, parent, via) in steps {
            match via {
                Via::Walk(w) if w > 0 => legs.push(Leg::Walk {
                    via: LocationID::Shortcut(ShortcutID(node)),
                    accepted: 0,
                }),
                Via::Cross(kind) => legs.push(Leg::Cross {
                    from: self.graph.points[parent],
                    to: self.graph.points[node],
                    kind,
                }),
                _ => {}
            }
        }
        legs.push(Leg::Walk {
            via: goal,
            accepted,
        });

        Some(Route {
            legs,
            cursor: 0,
            est_turns: total,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// SDK glue (names taken from BotAPIRust.md; adjust if your wrapper differs)
// ---------------------------------------------------------------------------------------------

pub fn capabilities(me: &Champion) -> Capabilities {
    let dash_ready_in = me.abilities().iter().find(|a| a.is_dash()).map(|a| {
        if a.is_ready().unwrap_or(false) {
            0
        } else {
            a.cooldown().unwrap_or(DASH_COOLDOWN as i32).max(1) as u16
        }
    });
    let has_tp = me
        .consumables()
        .iter()
        .any(|c| c.kind() == ConsumableType::TpCrystal && c.count().unwrap_or(0) > 0);
    Capabilities {
        dash_ready_in,
        has_tp,
    }
}

/// Dash to `to`. false = dash missing, on cooldown, or blocked (reveal) -> caller should replan.
pub fn cast_dash(me: &Champion, to: Vec2) -> bool {
    me.abilities()
        .iter()
        .find(|a| a.is_dash())
        .map(|a| a.cast_at(to).unwrap_or(false))
        .unwrap_or(false)
}

/// Start the teleport channel to `to`. While `me.is_teleporting()` do nothing else.
pub fn use_tp(me: &Champion, to: Vec2) -> bool {
    me.consumables()
        .iter()
        .find(|c| c.kind() == ConsumableType::TpCrystal)
        .map(|c| c.use_at(to).unwrap_or(false))
        .unwrap_or(false)
}

pub fn dash_forward(
    me: &Champion,
    state: &MovementState,
    champion_memory: &ChampionMemory,
    current_capabilities: Capabilities,
) {
    if current_capabilities.dash_ready_in != Some(0) {
        return;
    }
    let new_position: Vec2 = me.position().unwrap();
    let Some(landing): Option<Vec2> = dash_skip(
        new_position,
        &state.distance_field,
        &champion_memory.map.router.geometry,
        &DashSkipPolicy::default(),
    ) else {
        return;
    };
    let _ = cast_dash(me, landing);
}

pub fn follow_route(
    me: &Champion,
    state: &mut MovementState,
    champion_memory: &mut ChampionMemory,
    current_capabilities: Capabilities,
) {
    if !state.allow_route {
        return;
    }

    let position: Vec2 = me.position().unwrap();

    if state.route.is_none() {
        if state.route_failures >= MAX_ROUTE_FAILURES {
            return;
        }
        state.route = champion_memory.map.router.plan(
            position,
            state.location_id,
            state.accepted_distance,
            current_capabilities,
            &champion_memory.map.distance_fields,
        );
        if state.route.is_none() {
            state.route_failures += 1;
            return;
        }
    }

    let Some(route): Option<&mut Route> = state.route.as_mut() else {
        return;
    };

    let phase: Phase = route.advance(position, &champion_memory.map.distance_fields);
    let route_failed: bool = match phase {
        Phase::Walk(via) => walk_leg(me, route, via, champion_memory, current_capabilities),
        Phase::Dash { to } => !cast_dash(me, to),
        Phase::Teleport { to } => !use_tp(me, to),
        Phase::OffRoute => true,
        Phase::Finished => false,
    };

    if route_failed {
        state.route = None;
        state.route_failures += 1;
    }
}

pub fn walk_leg(
    me: &Champion,
    route: &mut Route,
    via: LocationID,
    champion_memory: &ChampionMemory,
    current_capabilities: Capabilities,
) -> bool {
    let Some(field): Option<&DistanceField> = champion_memory.map.distance_fields.weak_get(via)
    else {
        return true;
    };
    let target_position: Vec2 = champion_memory.map.points_of_interest.locate(via);
    if !step_with_fallback(field, target_position) {
        return true;
    }

    let new_position: Vec2 = me.position().unwrap();
    let dash_ready: bool = current_capabilities.dash_ready_in == Some(0);
    let Some(landing): Option<Vec2> = route.dash_after_walk(
        new_position,
        dash_ready,
        &champion_memory.map.distance_fields,
        &champion_memory.map.router.geometry,
        &DashSkipPolicy::default(),
    ) else {
        return false;
    };
    !cast_dash(me, landing)
}
