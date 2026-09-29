//! Grid map: parsing level strings, doors, collision and CPU-side ray casts
//! (hitscan, line of sight, "use" traces). The GPU performs the same DDA for
//! rendering in `shaders/raycast.wgsl`.

use crate::math::V2;
use crate::textures::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Red = 0,
    Blue = 1,
    Yellow = 2,
}

impl Key {
    pub fn name(self) -> &'static str {
        match self {
            Key::Red => "red",
            Key::Blue => "blue",
            Key::Yellow => "yellow",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    /// 0 = open, otherwise surface index + 1.
    pub wall: u8,
    pub floor: u8,
    /// Ceiling surface or `SKY`.
    pub ceil: u8,
    pub light: u8,
    pub flicker: bool,
    /// Damage per second while standing here.
    pub hazard: i32,
    pub door: Option<usize>,
    pub exit: bool,
}

const SOLID_CELL: Cell = Cell {
    wall: T_STONE + 1,
    floor: F_TILE,
    ceil: F_CEIL_METAL,
    light: 128,
    flicker: false,
    hazard: 0,
    door: None,
    exit: false,
};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DoorState {
    Closed,
    Opening,
    Open(f32),
    Closing,
}

#[derive(Clone, Debug)]
pub struct Door {
    pub x: i32,
    pub y: i32,
    pub key: Option<Key>,
    pub secret: bool,
    /// 0 = closed, 1 = fully retracted.
    pub open: f32,
    pub state: DoorState,
    /// True if the panel is the plane x = cx + 0.5 (you walk through it east-west).
    pub plane_x: bool,
}

impl Door {
    pub fn blocks(&self) -> bool {
        self.open < 0.8
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThingKind {
    Player,
    Zombie,
    Imp,
    Demon,
    Caco,
    Stimpack,
    Medikit,
    Potion,
    Soulsphere,
    GreenArmor,
    BlueArmor,
    Clip,
    Shells,
    ShellBox,
    RocketAmmo,
    RocketBox,
    Shotgun,
    Chaingun,
    Launcher,
    KeyRed,
    KeyBlue,
    KeyYellow,
    Barrel,
    Lamp,
    TorchRed,
    TorchBlue,
    SkullPole,
}

#[derive(Clone, Copy, Debug)]
pub struct Thing {
    pub kind: ThingKind,
    pub pos: V2,
}

pub struct LevelDef {
    pub name: &'static str,
    pub rows: &'static [&'static str],
    /// Facing at the start, radians (0 = east, PI/2 = south).
    pub start_angle: f32,
}

#[derive(Clone)]
pub struct Map {
    pub w: i32,
    pub h: i32,
    pub cells: Vec<Cell>,
    pub doors: Vec<Door>,
}

#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    pub dist: f32,
    pub x: i32,
    pub y: i32,
}

/// Wall characters and their surfaces. Space is solid rock outside the map.
fn wall_tex(c: char) -> Option<u8> {
    match c {
        '#' => Some(T_BRICK),
        'S' | ' ' => Some(T_STONE),
        'T' => Some(T_TECH),
        'W' => Some(T_WOOD),
        'M' => Some(T_MARBLE),
        'F' => Some(T_FLESH),
        'C' => Some(T_COMPUTER),
        'I' => Some(T_SUPPORT),
        'H' => Some(T_HELLROCK),
        'X' => Some(T_EXIT_OFF),
        _ => None,
    }
}

/// Floor characters: (floor, ceiling, light, flicker, hazard dps).
fn floor_def(c: char) -> Option<(u8, u8, u8, bool, i32)> {
    Some(match c {
        '.' => (F_TILE, F_CEIL_METAL, 170, false, 0),
        ':' => (F_TILE, F_CEIL_LIGHT, 235, false, 0),
        '\'' => (F_TILE, F_CEIL_LIGHT, 190, true, 0),
        ',' => (F_DIRT, SKY, 215, false, 0),
        '~' => (F_NUKAGE, F_CEIL_METAL, 160, false, 10),
        ';' => (F_NUKAGE, SKY, 200, false, 10),
        '=' => (F_LAVA, SKY, 240, false, 20),
        '_' => (F_WOOD, F_CEIL_METAL, 105, false, 0),
        '+' => (F_GRATE, F_CEIL_METAL, 140, false, 0),
        '-' => (F_TILE, F_CEIL_METAL, 95, false, 0),
        'r' => (F_ROCK, SKY, 185, false, 0),
        'h' => (F_ROCK, T_HELLROCK, 130, false, 0),
        'c' => (F_CARPET, F_CEIL_LIGHT, 200, false, 0),
        _ => return None,
    })
}

fn thing_kind(c: char) -> Option<ThingKind> {
    use ThingKind::*;
    Some(match c {
        'P' => Player,
        'z' => Zombie,
        'i' => Imp,
        'd' => Demon,
        'o' => Caco,
        '1' => Stimpack,
        '2' => Medikit,
        '3' => Potion,
        '4' => Soulsphere,
        '5' => GreenArmor,
        '6' => BlueArmor,
        'a' => Clip,
        's' => Shells,
        'b' => ShellBox,
        'q' => RocketAmmo,
        'Q' => RocketBox,
        'G' => Shotgun,
        'N' => Chaingun,
        'L' => Launcher,
        '!' => KeyRed,
        '&' => KeyBlue,
        '$' => KeyYellow,
        '%' => Barrel,
        'l' => Lamp,
        't' => TorchRed,
        'u' => TorchBlue,
        'p' => SkullPole,
        _ => return None,
    })
}

fn door_def(c: char) -> Option<(u8, Option<Key>, bool)> {
    match c {
        'D' => Some((T_DOOR, None, false)),
        'R' => Some((T_DOOR_RED, Some(Key::Red), false)),
        'B' => Some((T_DOOR_BLUE, Some(Key::Blue), false)),
        'Y' => Some((T_DOOR_YELLOW, Some(Key::Yellow), false)),
        '?' => Some((T_STONE, None, true)),
        _ => None,
    }
}

impl Map {
    /// Parse a level. Returns the map and the things placed on it.
    pub fn parse(def: &LevelDef) -> (Map, Vec<Thing>) {
        let h = def.rows.len() as i32;
        let w = def.rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32;
        let grid: Vec<Vec<char>> = def
            .rows
            .iter()
            .map(|r| {
                let mut v: Vec<char> = r.chars().collect();
                v.resize(w as usize, ' ');
                v
            })
            .collect();
        let at = |x: i32, y: i32| -> char {
            if x < 0 || y < 0 || x >= w || y >= h { ' ' } else { grid[y as usize][x as usize] }
        };

        let mut cells = vec![SOLID_CELL; (w * h) as usize];
        let mut doors = Vec::new();
        let mut things = Vec::new();

        for y in 0..h {
            for x in 0..w {
                let c = at(x, y);
                let cell = &mut cells[(y * w + x) as usize];
                if let Some(t) = wall_tex(c) {
                    cell.wall = t + 1;
                    cell.exit = c == 'X';
                    continue;
                }
                let floor_char = if let Some(kind) = thing_kind(c) {
                    things.push(Thing { kind, pos: V2::new(x as f32 + 0.5, y as f32 + 0.5) });
                    // Things stand on whatever floor surrounds them most.
                    let mut best = ('.', 0, false);
                    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)] {
                        let n = at(x + dx, y + dy);
                        let Some(f) = floor_def(n) else { continue };
                        let count = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)]
                            .iter()
                            .filter(|&&(ex, ey)| at(x + ex, y + ey) == n)
                            .count();
                        let safe = f.4 == 0;
                        if count > best.1 || (count == best.1 && safe && !best.2) {
                            best = (n, count, safe);
                        }
                    }
                    best.0
                } else if door_def(c).is_some() {
                    // Doors take the floor of their neighbours too.
                    [(-1, 0), (1, 0), (0, -1), (0, 1)]
                        .iter()
                        .map(|&(dx, dy)| at(x + dx, y + dy))
                        .find(|&n| floor_def(n).is_some())
                        .unwrap_or('.')
                } else {
                    c
                };
                let (floor, ceil, light, flicker, hazard) =
                    floor_def(floor_char).unwrap_or_else(|| panic!("unknown map char {c:?} at {x},{y} in {}", def.name));
                *cell = Cell { wall: 0, floor, ceil, light, flicker, hazard, door: None, exit: false };
                if let Some((tex, key, secret)) = door_def(c) {
                    let solid = |ch: char| wall_tex(ch).is_some() || door_def(ch).is_some();
                    let plane_x = !(solid(at(x - 1, y)) && solid(at(x + 1, y)));
                    // Secret doors borrow the texture of the wall they hide in.
                    let tex = if secret {
                        let n = if plane_x { at(x, y - 1) } else { at(x - 1, y) };
                        wall_tex(n).unwrap_or(T_STONE)
                    } else {
                        tex
                    };
                    cell.wall = tex + 1;
                    cell.door = Some(doors.len());
                    // Ceiling over a doorway should not be sky.
                    if cell.ceil == SKY {
                        cell.ceil = F_CEIL_METAL;
                    }
                    doors.push(Door { x, y, key, secret, open: 0.0, state: DoorState::Closed, plane_x });
                }
            }
        }
        (Map { w, h, cells, doors }, things)
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h
    }

    pub fn cell(&self, x: i32, y: i32) -> &Cell {
        if self.in_bounds(x, y) { &self.cells[(y * self.w + x) as usize] } else { &SOLID_CELL }
    }

    pub fn cell_mut(&mut self, x: i32, y: i32) -> Option<&mut Cell> {
        if self.in_bounds(x, y) { Some(&mut self.cells[(y * self.w + x) as usize]) } else { None }
    }

    pub fn cell_at(&self, p: V2) -> &Cell {
        self.cell(p.x.floor() as i32, p.y.floor() as i32)
    }

    /// Blocks movement: walls and mostly-closed doors.
    pub fn solid(&self, x: i32, y: i32) -> bool {
        let c = self.cell(x, y);
        match c.door {
            Some(d) => self.doors[d].blocks(),
            None => c.wall != 0,
        }
    }

    /// Packed cell data for the GPU: wall | floor(7 bits) + flicker | ceiling | light.
    pub fn gpu_cells(&self) -> Vec<u32> {
        self.cells
            .iter()
            .map(|c| {
                c.wall as u32
                    | ((c.floor as u32 & 0x7f) << 8)
                    | ((c.flicker as u32) << 15)
                    | ((c.ceil as u32) << 16)
                    | ((c.light as u32) << 24)
            })
            .collect()
    }

    /// Per-cell door state for the GPU: -1 = no door, [0,1] = door whose panel
    /// is the plane y = cy + 0.5, [2,3] = door whose panel is x = cx + 0.5.
    pub fn gpu_doors(&self) -> Vec<f32> {
        let mut v = vec![-1.0f32; self.cells.len()];
        for d in &self.doors {
            v[(d.y * self.w + d.x) as usize] = d.open + if d.plane_x { 2.0 } else { 0.0 };
        }
        v
    }

    fn door_hit(d: &Door, o: V2, dir: V2, t0: f32, t1: f32) -> Option<f32> {
        let (t, along) = if d.plane_x {
            if dir.x.abs() < 1e-9 {
                return None;
            }
            let t = (d.x as f32 + 0.5 - o.x) / dir.x;
            (t, o.y + dir.y * t - d.y as f32)
        } else {
            if dir.y.abs() < 1e-9 {
                return None;
            }
            let t = (d.y as f32 + 0.5 - o.y) / dir.y;
            (t, o.x + dir.x * t - d.x as f32)
        };
        (t >= t0 && t <= t1 && along >= d.open && along <= 1.0).then_some(t)
    }

    /// Grid DDA. Distances are in units of `dir` (use a unit vector for
    /// euclidean distance). Returns the first wall/door panel within `max`.
    pub fn cast(&self, o: V2, dir: V2, max: f32) -> Option<RayHit> {
        let mut mx = o.x.floor() as i32;
        let mut my = o.y.floor() as i32;
        let ddx = if dir.x.abs() < 1e-9 { f32::INFINITY } else { (1.0 / dir.x).abs() };
        let ddy = if dir.y.abs() < 1e-9 { f32::INFINITY } else { (1.0 / dir.y).abs() };
        let (sx, mut side_x) =
            if dir.x < 0.0 { (-1, (o.x - mx as f32) * ddx) } else { (1, (mx as f32 + 1.0 - o.x) * ddx) };
        let (sy, mut side_y) =
            if dir.y < 0.0 { (-1, (o.y - my as f32) * ddy) } else { (1, (my as f32 + 1.0 - o.y) * ddy) };
        let mut t_enter = 0.0f32;
        for _ in 0..512 {
            if let Some(di) = self.cell(mx, my).door
                && let Some(t) = Self::door_hit(&self.doors[di], o, dir, t_enter, side_x.min(side_y)) {
                    return (t <= max).then_some(RayHit { dist: t, x: mx, y: my });
                }
            if side_x < side_y {
                t_enter = side_x;
                side_x += ddx;
                mx += sx;
            } else {
                t_enter = side_y;
                side_y += ddy;
                my += sy;
            }
            if t_enter > max {
                return None;
            }
            let c = self.cell(mx, my);
            if !self.in_bounds(mx, my) || (c.wall != 0 && c.door.is_none()) {
                return Some(RayHit { dist: t_enter, x: mx, y: my });
            }
        }
        None
    }

    pub fn line_of_sight(&self, a: V2, b: V2) -> bool {
        self.cast(a, b - a, 1.0).is_none()
    }

    /// First non-open cell (wall, door or switch) within `max` along `dir`,
    /// regardless of door state — what the "use" key interacts with.
    pub fn use_trace(&self, o: V2, dir: V2, max: f32) -> Option<(i32, i32)> {
        let steps = (max / 0.05) as i32;
        let mut last = (o.x.floor() as i32, o.y.floor() as i32);
        for i in 1..=steps {
            let p = o + dir * (i as f32 * 0.05);
            let c = (p.x.floor() as i32, p.y.floor() as i32);
            if c != last {
                let cell = self.cell(c.0, c.1);
                if cell.wall != 0 {
                    if cell.door.is_some() && !self.doors[cell.door.unwrap()].blocks() {
                        last = c;
                        continue;
                    }
                    return Some(c);
                }
                last = c;
            }
        }
        None
    }

    /// Resolve a circle against solid cells, pushing it out. Returns the new position.
    pub fn collide(&self, mut p: V2, r: f32) -> V2 {
        for _ in 0..3 {
            let (x0, x1) = ((p.x - r).floor() as i32, (p.x + r).floor() as i32);
            let (y0, y1) = ((p.y - r).floor() as i32, (p.y + r).floor() as i32);
            let mut moved = false;
            for cy in y0..=y1 {
                for cx in x0..=x1 {
                    if !self.solid(cx, cy) {
                        continue;
                    }
                    let q = V2::new(p.x.clamp(cx as f32, cx as f32 + 1.0), p.y.clamp(cy as f32, cy as f32 + 1.0));
                    let d = p - q;
                    let l2 = d.len2();
                    if l2 < r * r {
                        if l2 > 1e-10 {
                            let l = l2.sqrt();
                            p += d * ((r - l) / l);
                        } else {
                            // Center inside the block: push out along the shallowest axis.
                            let c = V2::new(cx as f32 + 0.5, cy as f32 + 0.5);
                            let e = p - c;
                            if e.x.abs() > e.y.abs() {
                                p.x = c.x + e.x.signum() * (0.5 + r);
                            } else {
                                p.y = c.y + e.y.signum() * (0.5 + r);
                            }
                        }
                        moved = true;
                    }
                }
            }
            if !moved {
                break;
            }
        }
        p
    }

    /// True if a circle at `p` would overlap a solid cell.
    pub fn blocked(&self, p: V2, r: f32) -> bool {
        let (x0, x1) = ((p.x - r).floor() as i32, (p.x + r).floor() as i32);
        let (y0, y1) = ((p.y - r).floor() as i32, (p.y + r).floor() as i32);
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                if self.solid(cx, cy) {
                    let q = V2::new(p.x.clamp(cx as f32, cx as f32 + 1.0), p.y.clamp(cy as f32, cy as f32 + 1.0));
                    if (p - q).len2() < r * r {
                        return true;
                    }
                }
            }
        }
        false
    }
}
