use crate::{
    champion::memory::{ChampionMemory, LaneMemory, LaneSet},
    draft::camp_graph::Center,
};
use cambc::{debug, Result, Team, Vec2};

/*
Debug overlay and console output for ChampionMemory (the decoded draft bytes).
Everything is read-only. All draw calls are clamped to the map

DRAWING (draw_debug draws all of this; the other draw_* methods draw one part)
  Spawns            draw_debug
    ally  = green dot + ring (radius 2)
    enemy = red dot + ring + small cross
  Lane paths        draw_value_paths / draw_safe_paths
    polyline: ally spawn -> each camp in lane order -> enemy spawn
    colour by lane index (see palette); safe lanes are dimmed and drawn first,
    so value lanes sit on top
    camp:  dot + ring, ring radius = 1 + camp value / 40 (max 4),
           camp value = gold + 2 * gem (from the prefix-sum step)
    altar: dim line from camp to its altar, dim dot on the altar
  Lane values       draw_lane_values (bar panels, bottom-left)
    value panel at the left, safe panel starting at half the map width
    one block per lane: vertical stripe in the lane colour, then
      yellow bar = total gold, cyan bar = total gem (length = total / 16,
      max 16 tiles), red bar = lane danger (value lanes only, 1 tile per point)
    header line above each panel: green = lanes span all camps, red = incomplete
      (always green now that the spanning byte has no incomplete flag)
  Contested camps   draw_contested_camps
    white rings; biggest ring = most contested camp, shrinks 0.25 per rank
    (min 1.5); slots at (0, 0) are skipped
  Initial champions draw_initial_champions
    one marker per pick at its ward location: cross + ring in the colour of the
    champion's starting lane, plus (champion_index + 1) dots above-right of it

PRINTING (print_debug, console only, nothing is drawn)
  "value lanes N complete B" / "safe lanes N complete B"
      N = valid lanes decoded from the spanning count
  "lane i colour RRGGBB gold G gem E danger D champions [a, b, c] camps (x,y) -> ..."
      gold/gem = lane totals (saturate at 255), danger = 0..15,
      champions = raw 4-bit champion slots (0 may mean empty),
      colour matches the overlay, so this is the legend for the drawing
  "contested [(x, y), ...]"                      most contested first
  "initial champions [(index, lane, ward_x, ward_y), ...]"

LANE PALETTE (lane index -> colour)
  0 red E6194B      1 green 3CB44B    2 yellow FFE119    3 blue 4363D8
  4 orange F58231   5 purple 911EB4   6 cyan 42D4F4     7 magenta F032E6

USAGE
  let memory = ChampionMemory::from_snapshot(cambc::draft_memory().bytes());
  memory.print_debug();
  if let Err(error) = memory.draw_debug() { println!("{:?}", error); }
*/

const LANE_COLOURS: [u32; 8] = [
    0xE6194B, 0x3CB44B, 0xFFE119, 0x4363D8, 0xF58231, 0x911EB4, 0x42D4F4, 0xF032E6,
];
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

fn point(position: [u8; 2]) -> Vec2 {
    Vec2 {
        x: position[0] as i32,
        y: position[1] as i32,
    }
}

fn spawn_points() -> Result<(Vec2, Vec2)> {
    let ally: [u8; 2] = cambc::map::spawn_region(Team::Ally)?.center();
    let enemy: [u8; 2] = cambc::map::spawn_region(Team::Enemy)?.center();
    Ok((point(ally), point(enemy)))
}

fn bar_length(total: u32) -> i32 {
    if total == 0 {
        0
    } else {
        ((total / BAR_DIVISOR) as i32).clamp(1, BAR_MAX_LENGTH)
    }
}

fn camp_values(lane: &LaneMemory) -> Vec<u32> {
    let mut previous_gold: u8 = 0;
    let mut previous_gem: u8 = 0;
    lane.prefix_gold_sum
        .iter()
        .zip(lane.prefix_gem_sum.iter())
        .map(|(&gold, &gem): (&u8, &u8)| {
            let value: u32 = gold.saturating_sub(previous_gold) as u32
                + 2 * gem.saturating_sub(previous_gem) as u32;
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
    set: &LaneSet,
    dimmed: bool,
) -> Result<()> {
    for (lane_index, lane) in set.lanes.iter().enumerate() {
        if lane.is_empty() {
            continue;
        }
        let base: u32 = lane_colour(lane_index);
        let colour: u32 = if dimmed { dim(base) } else { base };
        let mut path: Vec<Vec2> = Vec::with_capacity(lane.camps.len() + 2);
        path.push(ally);
        path.extend(lane.camps.iter().copied());
        path.push(enemy);
        canvas.polyline(&path, colour)?;

        let values: Vec<u32> = camp_values(lane);
        for (camp_index, &camp) in lane.camps.iter().enumerate() {
            let value: u32 = values.get(camp_index).copied().unwrap_or(0);
            let radius: f32 =
                (MIN_CIRCLE_RADIUS + value as f32 / CIRCLE_VALUE_STEP).min(MAX_CIRCLE_RADIUS);
            canvas.dot(camp, colour)?;
            canvas.circle(camp, radius, colour)?;
            if let Some(&altar) = lane.altars.get(camp_index) {
                canvas.line(camp, altar, dim(colour))?;
                canvas.dot(altar, dim(colour))?;
            }
        }
    }
    Ok(())
}

fn paint_panel(canvas: &Canvas, origin: Vec2, set: &LaneSet, show_danger: bool) -> Result<()> {
    let header_colour: u32 = if set.complete {
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
    for (lane_index, lane) in set.lanes.iter().enumerate() {
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
            bar_length(lane.total_gold() as u32),
            GOLD_COLOUR,
        )?;
        canvas.bar(
            Vec2 {
                x: bar_x,
                y: row_y + 1,
            },
            bar_length(lane.total_gem() as u32),
            GEM_COLOUR,
        )?;
        if show_danger {
            canvas.bar(
                Vec2 {
                    x: bar_x,
                    y: row_y + 2,
                },
                (lane.danger as i32).min(BAR_MAX_LENGTH),
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
    for (rank, &camp) in memory.contested_camps.iter().enumerate() {
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
    for state in &memory.initial_states {
        let colour: u32 = lane_colour(state.starting_lane as usize);
        let ward: Vec2 = state.ward_location;
        canvas.cross(ward, WARD_ARM, colour)?;
        canvas.circle(ward, WARD_RADIUS, colour)?;
        for dot in 0..=(state.champion_index as i32) {
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
    paint_paths(&canvas, ally, enemy, &memory.safe_lanes, true)?;
    paint_paths(&canvas, ally, enemy, &memory.value_lanes, false)?;
    paint_panels(&canvas, memory)?;
    paint_contested(&canvas, memory)?;
    paint_champions(&canvas, memory)
}

fn print_set(label: &str, set: &LaneSet) {
    println!(
        "{} lanes {} complete {}",
        label,
        set.lanes.len(),
        set.complete
    );
    for (lane_index, lane) in set.lanes.iter().enumerate() {
        let path: String = lane
            .camps
            .iter()
            .map(|camp: &Vec2| format!("({},{})", camp.x, camp.y))
            .collect::<Vec<String>>()
            .join(" -> ");
        println!(
            "  lane {} colour {:06X} gold {} gem {} danger {} champions {:?} camps {}",
            lane_index,
            lane_colour(lane_index),
            lane.total_gold(),
            lane.total_gem(),
            lane.danger,
            lane.champion_slots,
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
            paint_paths(&canvas, ally, enemy, &self.value_lanes, false)?;
        }
        Ok(())
    }

    fn draw_safe_paths(&self) -> Result<()> {
        free_fuel! {
            let canvas: Canvas = Canvas::new()?;
            let (ally, enemy): (Vec2, Vec2) = spawn_points()?;
            paint_spawns(&canvas, ally, enemy)?;
            paint_paths(&canvas, ally, enemy, &self.safe_lanes, false)?;
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
            print_set("value", &self.value_lanes);
            print_set("safe", &self.safe_lanes);
            println!(
                "contested {:?}",
                self.contested_camps
                    .iter()
                    .map(|camp: &Vec2| (camp.x, camp.y))
                    .collect::<Vec<(i32, i32)>>()
            );
            println!(
                "initial champions {:?}",
                self.initial_states
                    .iter()
                    .map(|state| (state.champion_index, state.starting_lane, state.ward_location.x, state.ward_location.y))
                    .collect::<Vec<(u8, u8, i32, i32)>>()
            );
        }
    }
}
