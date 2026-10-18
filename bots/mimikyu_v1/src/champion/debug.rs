use crate::{
    champion::memory::{CampID, ChampionMemory, ChampionState, Lane, LaneID},
    draft::camp_graph::Center,
};
use cambc::{debug, Result, Team, Vec2};

const LANE_COLOURS: [u32; 8] = [
    0xE6194B, 0x3CB44B, 0xFFE119, 0x4363D8, 0xF58231, 0x911EB4, 0x42D4F4, 0xF032E6,
];
const NO_LANE_COLOUR: u32 = 0x808080;
const ALLY_COLOUR: u32 = 0x40FF40;
const ENEMY_COLOUR: u32 = 0xFF4040;
const GOLD_COLOUR: u32 = 0xFFD700;
const GEM_COLOUR: u32 = 0x00E5FF;
const DANGER_COLOUR: u32 = 0xFF3030;
const COMPLETE_COLOUR: u32 = 0x40FF40;
const INCOMPLETE_COLOUR: u32 = 0xFF3030;
const CONTESTED_COLOUR: u32 = 0xFFFFFF;

const CIRCLE_SEGMENTS: u32 = 12;
const MIN_CIRCLE_RADIUS: f32 = 1.0;
const MAX_CIRCLE_RADIUS: f32 = 4.0;
const CIRCLE_VALUE_STEP: f32 = 40.0;
const SPAWN_RADIUS: f32 = 2.0;
const CONTESTED_RADIUS_START: f32 = 3.0;
const CONTESTED_RADIUS_STEP: f32 = 0.25;
const CONTESTED_RADIUS_MIN: f32 = 1.5;
const WARD_ARM: i32 = 2;
const WARD_RADIUS: f32 = 1.5;

const BAR_DIVISOR: u32 = 16;
const BAR_MAX_LENGTH: i32 = 16;
const BAR_MARGIN: i32 = 1;
const BAR_ROW_STRIDE: i32 = 4;
const BAR_START_OFFSET: i32 = 2;
const VALUE_PANEL_ROWS: i32 = 3;
const SAFE_PANEL_ROWS: i32 = 2;

/*
ChampionMemory debug overlay and console output. Read-only; draws are clamped to the map.

DRAW (draw_debug = everything; each part also has its own draw_* method)
  spawns              ally: green dot + ring | enemy: red dot + ring + cross
  paths               draw_value_paths / draw_safe_paths
                        ally -> camps (lane order) -> enemy, one colour per lane
                        safe lanes dimmed and drawn first, value lanes on top
                        camp: dot + ring (radius 1 + value / 40, max 4; value = gold + 2 * gem)
                        altar: dim line + dim dot
  values              draw_lane_values, bar panels (value left, safe from half map width)
                        per lane: colour stripe, yellow = gold, cyan = gem (total / 16, max 16),
                        red = danger (value lanes only, 1 tile per point)
                        header: green = all lanes complete, red = not
  contested           draw_contested_camps, white rings, largest = most contested
                        (-0.25 per rank, min 1.5); (0, 0) slots skipped
  ally champions      draw_initial_champions
                        at each ward: cross + ring in its lane colour (grey = no lane)
                        plus (champion type index + 1) dots above-right

PRINT (print_debug, console only)
  <set> lanes N complete B                        lane count, all complete
    lane i colour RRGGBB gold G gem E danger D champions [(Team, Type)] camps (x,y) -> ...
  contested [(x, y), ...]                         most contested first
  ally champions [(type, lane, ward_x, ward_y)]

PALETTE (lane index -> colour)
  0 red E6194B    1 green 3CB44B   2 yellow FFE119   3 blue 4363D8
  4 orange F58231 5 purple 911EB4  6 cyan 42D4F4     7 magenta F032E6

USAGE
  let memory = ChampionMemory::from(DraftExtractedChampionMemory::from_bytes(&bytes));
  memory.print_debug();
  if let Err(error) = memory.draw_debug() { println!("{:?}", error); }
*/

macro_rules! free_fuel {
    ($($body:tt)*) => {
        cambc::debug_block! { $($body)* }
    };
}

fn lane_colour(lane_index: usize) -> u32 {
    LANE_COLOURS[lane_index % LANE_COLOURS.len()]
}

fn dim(rgb: u32) -> u32 {
    (rgb >> 1) & 0x7F7F7F
}

fn spawn_points() -> Result<(Vec2, Vec2)> {
    let ally: [u8; 2] = cambc::map::spawn_region(Team::Ally)?.center();
    let enemy: [u8; 2] = cambc::map::spawn_region(Team::Enemy)?.center();

    Ok((
        Vec2 {
            x: ally[0] as i32,
            y: ally[1] as i32,
        },
        Vec2 {
            x: enemy[0] as i32,
            y: enemy[1] as i32,
        },
    ))
}

fn bar_length(total: u32) -> i32 {
    if total == 0 {
        0
    } else {
        ((total / BAR_DIVISOR) as i32).clamp(1, BAR_MAX_LENGTH)
    }
}

fn prefix_total(prefix: &[i32]) -> u32 {
    prefix.last().copied().unwrap_or(0).max(0) as u32
}

fn lanes_complete(lanes: &[Lane]) -> bool {
    lanes.iter().all(|lane: &Lane| lane.complete)
}

fn camp_values(lane: &Lane) -> Vec<u32> {
    let mut previous_gold: i32 = 0;
    let mut previous_gem: i32 = 0;

    lane.prefix_gold_sum
        .iter()
        .zip(lane.prefix_gem_sum.iter())
        .map(|(&gold, &gem): (&i32, &i32)| {
            let value: u32 =
                (gold - previous_gold).max(0) as u32 + 2 * (gem - previous_gem).max(0) as u32;

            previous_gold = gold;
            previous_gem = gem;

            value
        })
        .collect()
}

pub struct Canvas {
    width: i32,
    height: i32,
}

impl Canvas {
    pub fn new() -> Result<Self> {
        Ok(Self {
            width: cambc::map::width()?,
            height: cambc::map::height()?,
        })
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    fn clamp(&self, position: Vec2) -> Vec2 {
        Vec2 {
            x: position.x.clamp(0, self.width - 1),
            y: position.y.clamp(0, self.height - 1),
        }
    }

    pub fn dot(&self, position: Vec2, rgb: u32) -> Result<()> {
        debug::draw_dot(self.clamp(position), rgb)
    }

    pub fn line(&self, from: Vec2, to: Vec2, rgb: u32) -> Result<()> {
        debug::draw_line(self.clamp(from), self.clamp(to), rgb)
    }

    pub fn polyline(&self, points: &[Vec2], rgb: u32) -> Result<()> {
        for pair in points.windows(2) {
            self.line(pair[0], pair[1], rgb)?;
        }

        Ok(())
    }

    fn circle_point(center: Vec2, radius: f32, step: u32) -> Vec2 {
        let angle: f32 = step as f32 * std::f32::consts::TAU / CIRCLE_SEGMENTS as f32;

        Vec2 {
            x: center.x + (radius * angle.cos()).round() as i32,
            y: center.y + (radius * angle.sin()).round() as i32,
        }
    }

    pub fn circle(&self, center: Vec2, radius: f32, rgb: u32) -> Result<()> {
        let radius: f32 = radius.max(MIN_CIRCLE_RADIUS);
        let mut previous: Vec2 = Self::circle_point(center, radius, 0);

        for step in 1..=CIRCLE_SEGMENTS {
            let next: Vec2 = Self::circle_point(center, radius, step);

            if next.x != previous.x || next.y != previous.y {
                self.line(previous, next, rgb)?;
            }

            previous = next;
        }

        Ok(())
    }

    pub fn cross(&self, center: Vec2, arm: i32, rgb: u32) -> Result<()> {
        self.line(
            Vec2 {
                x: center.x - arm,
                y: center.y - arm,
            },
            Vec2 {
                x: center.x + arm,
                y: center.y + arm,
            },
            rgb,
        )?;

        self.line(
            Vec2 {
                x: center.x - arm,
                y: center.y + arm,
            },
            Vec2 {
                x: center.x + arm,
                y: center.y - arm,
            },
            rgb,
        )
    }

    pub fn bar(&self, origin: Vec2, length: i32, rgb: u32) -> Result<()> {
        if length <= 0 {
            return Ok(());
        }

        let end: Vec2 = Vec2 {
            x: origin.x + length,
            y: origin.y,
        };

        self.line(origin, end, rgb)?;
        self.dot(end, rgb)
    }
}

fn paint_spawns(canvas: &Canvas, ally: Vec2, enemy: Vec2) -> Result<()> {
    canvas.dot(ally, ALLY_COLOUR)?;
    canvas.circle(ally, SPAWN_RADIUS, ALLY_COLOUR)?;
    canvas.dot(enemy, ENEMY_COLOUR)?;
    canvas.circle(enemy, SPAWN_RADIUS, ENEMY_COLOUR)?;
    canvas.cross(enemy, 1, ENEMY_COLOUR)
}

fn paint_paths(
    canvas: &Canvas,
    ally: Vec2,
    enemy: Vec2,
    memory: &ChampionMemory,
    lanes: &[Lane],
    dimmed: bool,
) -> Result<()> {
    for (lane_index, lane) in lanes.iter().enumerate() {
        if lane.camps.is_empty() {
            continue;
        }

        let base: u32 = lane_colour(lane_index);
        let colour: u32 = if dimmed { dim(base) } else { base };
        let values: Vec<u32> = camp_values(lane);

        let mut path: Vec<Vec2> = Vec::with_capacity(lane.camps.len() + 2);
        path.push(ally);

        for (camp_index, camp_id) in lane.camps.iter().enumerate() {
            let Some(&camp) = memory.camp_centers.get(camp_id.0) else {
                continue;
            };

            path.push(camp);

            let value: u32 = values.get(camp_index).copied().unwrap_or(0);
            let radius: f32 =
                (MIN_CIRCLE_RADIUS + value as f32 / CIRCLE_VALUE_STEP).min(MAX_CIRCLE_RADIUS);

            canvas.dot(camp, colour)?;
            canvas.circle(camp, radius, colour)?;

            if let Some(altar_id) = lane.altars.get(camp_index) {
                if let Some(&altar) = memory.altar_positions.get(altar_id.0) {
                    let altar_colour: u32 = dim(colour);
                    canvas.line(camp, altar, altar_colour)?;
                    canvas.dot(altar, altar_colour)?;
                }
            }
        }

        path.push(enemy);
        canvas.polyline(&path, colour)?;
    }

    Ok(())
}

fn paint_panel(canvas: &Canvas, origin: Vec2, lanes: &[Lane], show_danger: bool) -> Result<()> {
    let header_colour: u32 = if lanes_complete(lanes) {
        COMPLETE_COLOUR
    } else {
        INCOMPLETE_COLOUR
    };

    canvas.line(
        Vec2 {
            x: origin.x,
            y: origin.y - 1,
        },
        Vec2 {
            x: origin.x + BAR_START_OFFSET + BAR_MAX_LENGTH,
            y: origin.y - 1,
        },
        header_colour,
    )?;

    let rows: i32 = if show_danger {
        VALUE_PANEL_ROWS
    } else {
        SAFE_PANEL_ROWS
    };

    for (lane_index, lane) in lanes.iter().enumerate() {
        let row_y: i32 = origin.y + lane_index as i32 * BAR_ROW_STRIDE;

        canvas.line(
            Vec2 {
                x: origin.x,
                y: row_y,
            },
            Vec2 {
                x: origin.x,
                y: row_y + rows - 1,
            },
            lane_colour(lane_index),
        )?;

        let bar_x: i32 = origin.x + BAR_START_OFFSET;

        canvas.bar(
            Vec2 { x: bar_x, y: row_y },
            bar_length(prefix_total(&lane.prefix_gold_sum)),
            GOLD_COLOUR,
        )?;

        canvas.bar(
            Vec2 {
                x: bar_x,
                y: row_y + 1,
            },
            bar_length(prefix_total(&lane.prefix_gem_sum)),
            GEM_COLOUR,
        )?;

        if show_danger {
            canvas.bar(
                Vec2 {
                    x: bar_x,
                    y: row_y + 2,
                },
                lane.danger.clamp(0, BAR_MAX_LENGTH),
                DANGER_COLOUR,
            )?;
        }
    }

    Ok(())
}

fn paint_panels(canvas: &Canvas, memory: &ChampionMemory) -> Result<()> {
    paint_panel(
        canvas,
        Vec2 {
            x: BAR_MARGIN,
            y: BAR_MARGIN + 1,
        },
        &memory.value_lanes,
        true,
    )?;

    paint_panel(
        canvas,
        Vec2 {
            x: canvas.width() / 2 + BAR_MARGIN,
            y: BAR_MARGIN + 1,
        },
        &memory.safe_lanes,
        false,
    )
}

fn paint_contested(canvas: &Canvas, memory: &ChampionMemory) -> Result<()> {
    for (rank, camp_id) in memory.contested_camps.iter().enumerate() {
        let Some(&camp) = memory.camp_centers.get(camp_id.0) else {
            continue;
        };

        if camp.x == 0 && camp.y == 0 {
            continue;
        }

        let radius: f32 = (CONTESTED_RADIUS_START - CONTESTED_RADIUS_STEP * rank as f32)
            .max(CONTESTED_RADIUS_MIN);

        canvas.circle(camp, radius, CONTESTED_COLOUR)?;
    }

    Ok(())
}

fn paint_champions(canvas: &Canvas, memory: &ChampionMemory) -> Result<()> {
    for state in &memory.ally_states {
        let colour: u32 = state
            .lane
            .map_or(NO_LANE_COLOUR, |lane: LaneID| lane_colour(lane.0));

        let ward: Vec2 = state.ward_position;

        canvas.cross(ward, WARD_ARM, colour)?;
        canvas.circle(ward, WARD_RADIUS, colour)?;

        for dot in 0..=(state.champion_type as i32) {
            canvas.dot(
                Vec2 {
                    x: ward.x + 2 + dot,
                    y: ward.y - 3,
                },
                colour,
            )?;
        }
    }

    Ok(())
}

fn paint_all(memory: &ChampionMemory) -> Result<()> {
    let canvas: Canvas = Canvas::new()?;
    let (ally, enemy): (Vec2, Vec2) = spawn_points()?;

    paint_spawns(&canvas, ally, enemy)?;
    paint_paths(&canvas, ally, enemy, memory, &memory.safe_lanes, true)?;
    paint_paths(&canvas, ally, enemy, memory, &memory.value_lanes, false)?;
    paint_panels(&canvas, memory)?;
    paint_contested(&canvas, memory)?;
    paint_champions(&canvas, memory)
}

fn print_set(label: &str, lanes: &[Lane], memory: &ChampionMemory) {
    println!(
        "{} lanes {} complete {}",
        label,
        lanes.len(),
        lanes_complete(lanes)
    );

    for (lane_index, lane) in lanes.iter().enumerate() {
        let path: String = lane
            .camps
            .iter()
            .filter_map(|camp_id: &CampID| memory.camp_centers.get(camp_id.0))
            .map(|camp: &Vec2| format!("({},{})", camp.x, camp.y))
            .collect::<Vec<String>>()
            .join(" -> ");

        println!(
            "  lane {} colour {:06X} gold {} gem {} danger {} champions {:?} camps {}",
            lane_index,
            lane_colour(lane_index),
            prefix_total(&lane.prefix_gold_sum),
            prefix_total(&lane.prefix_gem_sum),
            lane.danger,
            lane.champions_in_lane,
            path
        );
    }
}

pub trait MemoryDebug {
    fn draw_debug(&self) -> Result<()>;
    fn draw_value_paths(&self) -> Result<()>;
    fn draw_safe_paths(&self) -> Result<()>;
    fn draw_lane_values(&self) -> Result<()>;
    fn draw_contested_camps(&self) -> Result<()>;
    fn draw_initial_champions(&self) -> Result<()>;
    fn print_debug(&self);
}

impl MemoryDebug for ChampionMemory {
    fn draw_debug(&self) -> Result<()> {
        free_fuel! {
            paint_all(self)?;
        }

        Ok(())
    }

    fn draw_value_paths(&self) -> Result<()> {
        free_fuel! {
            let canvas: Canvas = Canvas::new()?;
            let (ally, enemy): (Vec2, Vec2) = spawn_points()?;

            paint_spawns(&canvas, ally, enemy)?;
            paint_paths(&canvas, ally, enemy, self, &self.value_lanes, false)?;
        }

        Ok(())
    }

    fn draw_safe_paths(&self) -> Result<()> {
        free_fuel! {
            let canvas: Canvas = Canvas::new()?;
            let (ally, enemy): (Vec2, Vec2) = spawn_points()?;

            paint_spawns(&canvas, ally, enemy)?;
            paint_paths(&canvas, ally, enemy, self, &self.safe_lanes, false)?;
        }

        Ok(())
    }

    fn draw_lane_values(&self) -> Result<()> {
        free_fuel! {
            let canvas: Canvas = Canvas::new()?;
            paint_panels(&canvas, self)?;
        }

        Ok(())
    }

    fn draw_contested_camps(&self) -> Result<()> {
        free_fuel! {
            let canvas: Canvas = Canvas::new()?;
            paint_contested(&canvas, self)?;
        }

        Ok(())
    }

    fn draw_initial_champions(&self) -> Result<()> {
        free_fuel! {
            let canvas: Canvas = Canvas::new()?;
            paint_champions(&canvas, self)?;
        }

        Ok(())
    }

    fn print_debug(&self) {
        free_fuel! {
            print_set("value", &self.value_lanes, self);
            print_set("safe", &self.safe_lanes, self);

            println!(
                "contested {:?}",
                self.contested_camps
                    .iter()
                    .filter_map(|camp_id: &CampID| {
                        self.camp_centers.get(camp_id.0)
                    })
                    .map(|camp: &Vec2| (camp.x, camp.y))
                    .collect::<Vec<(i32, i32)>>()
            );

            println!(
                "current champion {:?}",
                (
                    format!("{:?}", self.state.champion_type),
                    self.state.lane,
                    self.state.ward_position.x,
                    self.state.ward_position.y
                )
            );

            println!(
                "ally champions {:?}",
                self.ally_states
                    .iter()
                    .map(|state: &ChampionState| (
                        format!("{:?}", state.champion_type),
                        state.lane,
                        state.ward_position.x,
                        state.ward_position.y
                    ))
                    .collect::<Vec<(String, Option<LaneID>, i32, i32)>>()
            );

            println!(
                "enemy champions {:?}",
                self.enemy_states
                    .iter()
                    .map(|state: &ChampionState| (
                        format!("{:?}", state.champion_type),
                        state.lane,
                        state.ward_position.x,
                        state.ward_position.y
                    ))
                    .collect::<Vec<(String, Option<LaneID>, i32, i32)>>()
            );
        }
    }
}
