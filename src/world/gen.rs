//! Procedural raid map generation: terrain, roads, buildings, cover, loot
//! containers, spawn points and extraction zones.

use super::{Block, ContainerKind, World};
use crate::rng::{fbm, Rng};
use glam::{IVec3, Vec3};

/// Raid map dimensions in chunks (192 x 64 x 192 blocks).
pub const RAID_CHUNKS: IVec3 = IVec3::new(12, 4, 12);

#[derive(Clone, Debug)]
pub struct ExtractPoint {
    pub name: String,
    /// Feet-level centre of the extraction zone.
    pub pos: Vec3,
    pub radius: f32,
}

#[derive(Default, Clone, Debug)]
pub struct MapInfo {
    pub player_spawns: Vec<Vec3>,
    pub extracts: Vec<ExtractPoint>,
    /// Walkable feet positions used for AI patrols.
    pub patrol_points: Vec<Vec3>,
    pub scav_spawns: Vec<Vec3>,
    pub containers: Vec<(IVec3, ContainerKind)>,
}

const FREE: u8 = 0;
const ROAD: u8 = 1;
const SHOULDER: u8 = 2;
const LOT: u8 = 3;
const DECOR: u8 = 4;

#[derive(Clone, Copy)]
struct Lot {
    x0: i32,
    z0: i32,
    w: i32,
    d: i32,
    /// Ground level (y of the floor block).
    y0: i32,
}

impl Lot {
    fn x1(&self) -> i32 {
        self.x0 + self.w - 1
    }
    fn z1(&self) -> i32 {
        self.z0 + self.d - 1
    }
}

struct Gen {
    w: World,
    rng: Rng,
    seed: u32,
    sx: i32,
    sz: i32,
    height: Vec<i32>,
    mask: Vec<u8>,
    info: MapInfo,
    road_a_phase: f32,
    road_b_phase: f32,
}

pub fn generate_raid_map(seed: u64) -> (World, MapInfo) {
    let world = World::new(RAID_CHUNKS);
    let sx = world.size.x;
    let sz = world.size.z;
    let mut rng = Rng::new(seed);
    let road_a_phase = rng.range_f32(0.0, std::f32::consts::TAU);
    let road_b_phase = rng.range_f32(0.0, std::f32::consts::TAU);
    let mut g = Gen {
        w: world,
        rng,
        seed: (seed as u32) ^ 0x51ED_1234,
        sx,
        sz,
        height: vec![0; (sx * sz) as usize],
        mask: vec![FREE; (sx * sz) as usize],
        info: MapInfo::default(),
        road_a_phase,
        road_b_phase,
    };
    g.heights();
    let lots = g.place_lots();
    g.fill_terrain();
    g.road_markings();
    for (kind, lot) in &lots {
        match kind {
            LotKind::House => g.build_house(lot, false),
            LotKind::TwoStory => g.build_house(lot, true),
            LotKind::Warehouse => g.build_warehouse(lot),
            LotKind::Office => g.build_office(lot),
            LotKind::ShippingContainer => g.build_shipping_container(lot),
        }
    }
    g.checkpoints();
    g.road_cover();
    g.field_cover();
    g.trees();
    g.extracts();
    g.spawns();
    g.w.finish_generation();
    (g.w, g.info)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum LotKind {
    House,
    TwoStory,
    Warehouse,
    Office,
    ShippingContainer,
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn box_blur(src: &[f32], sx: i32, sz: i32, r: i32) -> Vec<f32> {
    let mut tmp = vec![0.0; src.len()];
    let mut out = vec![0.0; src.len()];
    for z in 0..sz {
        for x in 0..sx {
            let mut s = 0.0;
            let mut n = 0.0;
            for xx in (x - r).max(0)..=(x + r).min(sx - 1) {
                s += src[(z * sx + xx) as usize];
                n += 1.0;
            }
            tmp[(z * sx + x) as usize] = s / n;
        }
    }
    for z in 0..sz {
        for x in 0..sx {
            let mut s = 0.0;
            let mut n = 0.0;
            for zz in (z - r).max(0)..=(z + r).min(sz - 1) {
                s += tmp[(zz * sx + x) as usize];
                n += 1.0;
            }
            out[(z * sx + x) as usize] = s / n;
        }
    }
    out
}

impl Gen {
    #[inline]
    fn idx(&self, x: i32, z: i32) -> usize {
        (z * self.sx + x) as usize
    }

    fn in_map(&self, x: i32, z: i32) -> bool {
        x >= 0 && z >= 0 && x < self.sx && z < self.sz
    }

    fn ground(&self, x: i32, z: i32) -> i32 {
        if !self.in_map(x, z) {
            return 0;
        }
        self.height[self.idx(x, z)]
    }

    fn mask_at(&self, x: i32, z: i32) -> u8 {
        if !self.in_map(x, z) {
            return LOT;
        }
        self.mask[self.idx(x, z)]
    }

    fn set_mask(&mut self, x: i32, z: i32, m: u8) {
        if self.in_map(x, z) {
            let i = self.idx(x, z);
            self.mask[i] = m;
        }
    }

    fn road_a_x(&self, z: i32) -> f32 {
        self.sx as f32 * 0.5 + 10.0 * (z as f32 * 0.035 + self.road_a_phase).sin()
    }

    fn road_b_z(&self, x: i32) -> f32 {
        self.sz as f32 * 0.38 + 8.0 * (x as f32 * 0.03 + self.road_b_phase).sin()
    }

    fn road_dist(&self, x: i32, z: i32) -> f32 {
        let a = (x as f32 + 0.5 - self.road_a_x(z)).abs();
        let b = (z as f32 + 0.5 - self.road_b_z(x)).abs();
        a.min(b)
    }

    fn set(&mut self, x: i32, y: i32, z: i32, b: Block) {
        self.w.set_gen(IVec3::new(x, y, z), b);
    }

    fn get(&self, x: i32, y: i32, z: i32) -> Block {
        self.w.get_xyz(x, y, z)
    }

    fn fill(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, b: Block) {
        self.w.fill_gen(
            IVec3::new(x0.min(x1), y0.min(y1), z0.min(z1)),
            IVec3::new(x0.max(x1), y0.max(y1), z0.max(z1)),
            b,
        );
    }

    /// Perimeter walls of a rectangle between two heights (inclusive).
    fn walls(&mut self, x0: i32, z0: i32, x1: i32, z1: i32, y0: i32, y1: i32, b: Block) {
        self.fill(x0, y0, z0, x1, y1, z0, b);
        self.fill(x0, y0, z1, x1, y1, z1, b);
        self.fill(x0, y0, z0, x0, y1, z1, b);
        self.fill(x1, y0, z0, x1, y1, z1, b);
    }

    fn container(&mut self, x: i32, y: i32, z: i32, kind: ContainerKind) {
        let b = match kind {
            ContainerKind::WoodenCrate => Block::Crate,
            ContainerKind::WeaponBox => Block::WeaponBox,
            ContainerKind::MedCase => Block::MedCase,
            ContainerKind::FileCabinet => Block::Cabinet,
        };
        self.set(x, y, z, b);
        self.info.containers.push((IVec3::new(x, y, z), kind));
    }

    fn patrol(&mut self, x: i32, y: i32, z: i32) {
        self.info
            .patrol_points
            .push(Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5));
    }

    // ------------------------------------------------------------------
    // Terrain
    // ------------------------------------------------------------------

    fn heights(&mut self) {
        let n = (self.sx * self.sz) as usize;
        let mut raw = vec![0.0f32; n];
        let mut base = vec![0.0f32; n];
        for z in 0..self.sz {
            for x in 0..self.sx {
                let i = self.idx(x, z);
                let n1 = fbm(x as f32 / 60.0, z as f32 / 60.0, 4, self.seed);
                let b = 16.0 + (n1 - 0.5) * 14.0;
                let edge = x.min(z).min(self.sx - 1 - x).min(self.sz - 1 - z) as f32;
                let bump = if edge < 26.0 {
                    (1.0 - edge / 26.0).powi(2) * 18.0
                } else {
                    0.0
                };
                base[i] = b;
                raw[i] = b + bump;
            }
        }
        let blurred = box_blur(&base, self.sx, self.sz, 10);
        for z in 0..self.sz {
            for x in 0..self.sx {
                let i = self.idx(x, z);
                let d = self.road_dist(x, z);
                let h = if d <= 3.5 {
                    self.mask[i] = ROAD;
                    blurred[i]
                } else if d <= 5.5 {
                    self.mask[i] = SHOULDER;
                    blurred[i]
                } else if d < 12.0 {
                    let t = smoothstep((d - 5.5) / 6.5);
                    blurred[i] + (raw[i] - blurred[i]) * t
                } else {
                    raw[i]
                };
                self.height[i] = (h.round() as i32).clamp(3, 44);
            }
        }
    }

    fn place_lots(&mut self) -> Vec<(LotKind, Lot)> {
        let plan: [(LotKind, usize); 5] = [
            (LotKind::Warehouse, 2),
            (LotKind::Office, 1),
            (LotKind::TwoStory, 3),
            (LotKind::House, 6),
            (LotKind::ShippingContainer, 8),
        ];
        let mut lots = Vec::new();
        for (kind, count) in plan {
            for _ in 0..count {
                let (w, d, near) = match kind {
                    LotKind::Warehouse => (20, 14, 34.0),
                    LotKind::Office => (14, 12, 28.0),
                    LotKind::TwoStory => (11, 9, 26.0),
                    LotKind::House => (self.rng.range_i32(8, 10), self.rng.range_i32(7, 9), 30.0),
                    LotKind::ShippingContainer => {
                        if self.rng.chance(0.5) {
                            (4, 8, 60.0)
                        } else {
                            (8, 4, 60.0)
                        }
                    }
                };
                if let Some(lot) = self.try_lot(w, d, near) {
                    lots.push((kind, lot));
                }
            }
        }
        lots
    }

    fn try_lot(&mut self, w: i32, d: i32, max_road_dist: f32) -> Option<Lot> {
        for _ in 0..400 {
            let x0 = self.rng.range_i32(26, self.sx - 26 - w);
            let z0 = self.rng.range_i32(26, self.sz - 26 - d);
            let mut ok = true;
            'check: for z in z0 - 3..z0 + d + 3 {
                for x in x0 - 3..x0 + w + 3 {
                    if self.mask_at(x, z) != FREE {
                        ok = false;
                        break 'check;
                    }
                }
            }
            if !ok {
                continue;
            }
            if self.road_dist(x0 + w / 2, z0 + d / 2) > max_road_dist {
                continue;
            }
            let mut sum = 0i64;
            let mut count = 0i64;
            for z in z0..z0 + d {
                for x in x0..x0 + w {
                    sum += self.ground(x, z) as i64;
                    count += 1;
                }
            }
            let level = (sum as f32 / count as f32).round() as i32;
            let blend = 6;
            for z in (z0 - 1 - blend).max(0)..(z0 + d + 1 + blend).min(self.sz) {
                for x in (x0 - 1 - blend).max(0)..(x0 + w + 1 + blend).min(self.sx) {
                    let dx = (x0 - 1 - x).max(x - (x0 + w)).max(0);
                    let dz = (z0 - 1 - z).max(z - (z0 + d)).max(0);
                    let ring = dx.max(dz);
                    let i = self.idx(x, z);
                    if ring == 0 {
                        self.height[i] = level;
                        self.mask[i] = LOT;
                    } else if self.mask[i] == FREE {
                        let t = smoothstep(ring as f32 / blend as f32);
                        let h = level as f32 + (self.height[i] - level) as f32 * t;
                        self.height[i] = h.round() as i32;
                    }
                }
            }
            return Some(Lot { x0, z0, w, d, y0: level });
        }
        None
    }

    fn fill_terrain(&mut self) {
        for z in 0..self.sz {
            for x in 0..self.sx {
                let i = self.idx(x, z);
                let h = self.height[i];
                let m = self.mask[i];
                self.set(x, 0, z, Block::Bedrock);
                for y in 1..=h {
                    let b = if y == h {
                        match m {
                            ROAD => Block::Asphalt,
                            SHOULDER => Block::Gravel,
                            LOT => Block::Gravel,
                            _ => {
                                if h <= 11 {
                                    Block::Sand
                                } else {
                                    Block::Grass
                                }
                            }
                        }
                    } else if y >= h - 3 {
                        Block::Dirt
                    } else {
                        Block::Stone
                    };
                    self.set(x, y, z, b);
                }
            }
        }
    }

    fn road_markings(&mut self) {
        for z in 0..self.sz {
            if (z / 3) % 2 == 0 && (z as f32 - self.road_b_z(self.road_a_x(z) as i32)).abs() > 6.0 {
                let x = self.road_a_x(z).floor() as i32;
                let y = self.ground(x, z);
                if self.mask_at(x, z) == ROAD {
                    self.set(x, y, z, Block::AsphaltLine);
                }
            }
        }
        for x in 0..self.sx {
            if (x / 3) % 2 == 0 && (x as f32 - self.road_a_x(self.road_b_z(x) as i32)).abs() > 6.0 {
                let z = self.road_b_z(x).floor() as i32;
                let y = self.ground(x, z);
                if self.mask_at(x, z) == ROAD {
                    self.set(x, y, z, Block::AsphaltLine);
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Buildings
    // ------------------------------------------------------------------

    fn windows(&mut self, x0: i32, z0: i32, x1: i32, z1: i32, y0: i32, y1: i32, spacing: i32) {
        for x in (x0 + 2)..=(x1 - 2) {
            if (x - x0) % spacing == 0 {
                self.fill(x, y0, z0, x, y1, z0, Block::Glass);
                self.fill(x, y0, z1, x, y1, z1, Block::Glass);
            }
        }
        for z in (z0 + 2)..=(z1 - 2) {
            if (z - z0) % spacing == 0 {
                self.fill(x0, y0, z, x0, y1, z, Block::Glass);
                self.fill(x1, y0, z, x1, y1, z, Block::Glass);
            }
        }
    }

    fn gable_roof(&mut self, x0: i32, z0: i32, x1: i32, z1: i32, y: i32, roof: Block, gable: Block) {
        let along_x = (x1 - x0) >= (z1 - z0);
        let mut k = 0;
        loop {
            let yy = y + k;
            if along_x {
                let lo = z0 - 1 + k;
                let hi = z1 + 1 - k;
                if lo > hi {
                    break;
                }
                for x in x0 - 1..=x1 + 1 {
                    self.set(x, yy, lo, roof);
                    self.set(x, yy, hi, roof);
                }
                if k >= 1 {
                    for z in lo + 1..hi {
                        self.set(x0, yy, z, gable);
                        self.set(x1, yy, z, gable);
                    }
                }
            } else {
                let lo = x0 - 1 + k;
                let hi = x1 + 1 - k;
                if lo > hi {
                    break;
                }
                for z in z0 - 1..=z1 + 1 {
                    self.set(lo, yy, z, roof);
                    self.set(hi, yy, z, roof);
                }
                if k >= 1 {
                    for x in lo + 1..hi {
                        self.set(x, yy, z0, gable);
                        self.set(x, yy, z1, gable);
                    }
                }
            }
            k += 1;
        }
    }

    /// Four one-block steps along +x in rows z and z+1 starting at x, rising from floor fy,
    /// with a matching hole cut in the floor above (at fy + 5).
    fn stairs(&mut self, x: i32, z: i32, fy: i32) {
        for i in 0..4 {
            self.fill(x + i, fy + 1, z, x + i, fy + 1 + i, z + 1, Block::Planks);
        }
        self.fill(x, fy + 5, z, x + 3, fy + 5, z + 1, Block::Air);
    }

    fn door(&mut self, x: i32, z: i32, fy: i32, width_x: i32, width_z: i32) {
        self.fill(x, fy + 1, z, x + width_x - 1, fy + 2, z + width_z - 1, Block::Air);
    }

    fn build_house(&mut self, lot: &Lot, two_story: bool) {
        let (x0, z0, x1, z1, y0) = (lot.x0, lot.z0, lot.x1(), lot.z1(), lot.y0);
        let wall = *self
            .rng
            .pick(&[Block::Brick, Block::Planks, Block::Plaster, Block::Brick, Block::Planks]);
        let floor_b = *self.rng.pick(&[Block::Planks, Block::FloorTile]);
        let roof_b = *self.rng.pick(&[Block::RustMetal, Block::Brick, Block::Planks]);
        let stories = if two_story { 2 } else { 1 };

        self.fill(x0, y0 - 3, z0, x1, y0 - 1, z1, Block::Concrete);
        for s in 0..stories {
            let fy = y0 + s * 5;
            self.fill(x0, fy, z0, x1, fy, z1, floor_b);
            self.fill(x0 + 1, fy + 1, z0 + 1, x1 - 1, fy + 4, z1 - 1, Block::Air);
            self.walls(x0, z0, x1, z1, fy + 1, fy + 4, wall);
            if wall == Block::Planks {
                for (cx, cz) in [(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
                    self.fill(cx, fy + 1, cz, cx, fy + 4, cz, Block::Log);
                }
            }
            self.windows(x0, z0, x1, z1, fy + 2, fy + 3, 3);
        }
        let roof_y = y0 + stories * 5;
        self.fill(x0, roof_y, z0, x1, roof_y, z1, Block::Planks);
        self.gable_roof(x0, z0, x1, z1, roof_y, roof_b, wall);

        // Doors on front and back walls, clear of the stairs.
        let door_min = if two_story { x0 + 6 } else { x0 + 2 };
        let fx = self.rng.range_i32(door_min, (x1 - 2).max(door_min));
        let bx = self.rng.range_i32(x0 + 2, x1 - 2);
        self.door(fx, z0, y0, 1, 1);
        self.door(bx, z1, y0, 1, 1);
        // Clear outside the doors in case of terrain.
        self.fill(fx, y0 + 1, z0 - 1, fx, y0 + 2, z0 - 1, Block::Air);
        self.fill(bx, y0 + 1, z1 + 1, bx, y0 + 2, z1 + 1, Block::Air);

        if two_story {
            self.stairs(x0 + 1, z0 + 1, y0);
            if self.rng.chance(0.5) {
                self.container(x1 - 1, y0 + 6, z1 - 1, ContainerKind::WoodenCrate);
            } else {
                self.container(x1 - 1, y0 + 6, z1 - 1, ContainerKind::WeaponBox);
            }
            if self.rng.chance(0.4) {
                self.container(x0 + 1, y0 + 6, z1 - 1, ContainerKind::MedCase);
            }
            self.patrol((x0 + x1) / 2, y0 + 6, (z0 + z1) / 2 + 1);
        }

        // Ground floor loot and furniture.
        if self.rng.chance(0.75) {
            self.container(x1 - 1, y0 + 1, z1 - 1, ContainerKind::WoodenCrate);
        }
        if self.rng.chance(0.45) {
            self.container(x0 + 1, y0 + 1, z1 - 1, ContainerKind::FileCabinet);
        }
        if self.rng.chance(0.2) {
            self.container(x1 - 1, y0 + 1, z0 + 1, ContainerKind::MedCase);
        }
        // A table as indoor cover.
        let tx = (x0 + x1) / 2;
        let tz = (z0 + z1) / 2 + 1;
        if tz < z1 - 1 {
            self.set(tx, y0 + 1, tz, Block::Planks);
        }

        self.patrol((x0 + x1) / 2, y0 + 1, (z0 + z1) / 2 - 1);
        self.patrol(fx, y0 + 1, z0 - 2);
        self.patrol(bx, y0 + 1, z1 + 2);
    }

    fn build_warehouse(&mut self, lot: &Lot) {
        let (x0, z0, x1, z1, y0) = (lot.x0, lot.z0, lot.x1(), lot.z1(), lot.y0);
        self.fill(x0, y0 - 3, z0, x1, y0, z1, Block::Concrete);
        self.fill(x0 + 1, y0 + 1, z0 + 1, x1 - 1, y0 + 7, z1 - 1, Block::Air);
        self.walls(x0, z0, x1, z1, y0 + 1, y0 + 2, Block::Concrete);
        self.walls(x0, z0, x1, z1, y0 + 3, y0 + 7, Block::Metal);
        self.fill(x0, y0 + 8, z0, x1, y0 + 8, z1, Block::Metal);
        // High window strip.
        for x in x0 + 2..=x1 - 2 {
            if x % 2 == 0 {
                self.set(x, y0 + 6, z0, Block::Glass);
                self.set(x, y0 + 6, z1, Block::Glass);
            }
        }
        // Ceiling lamps.
        let mut x = x0 + 3;
        while x < x1 - 1 {
            self.set(x, y0 + 8, z0 + 4, Block::Lamp);
            self.set(x, y0 + 8, z1 - 4, Block::Lamp);
            x += 5;
        }
        // Big loading doors on both short sides and a small side door.
        let zc = (z0 + z1) / 2;
        self.fill(x0, y0 + 1, zc - 2, x0, y0 + 4, zc + 2, Block::Air);
        self.fill(x1, y0 + 1, zc - 2, x1, y0 + 4, zc + 2, Block::Air);
        let sx = (x0 + x1) / 2;
        self.door(sx, z0, y0, 2, 1);
        self.fill(sx, y0 + 1, z0 - 1, sx + 1, y0 + 2, z0 - 1, Block::Air);

        // Pallet stacks (cover) with crates.
        for i in 0..3 {
            let px = x0 + 3 + i * 5;
            for pz in [z0 + 2, z1 - 3] {
                let h = self.rng.range_i32(1, 3);
                self.fill(px, y0 + 1, pz, px + 1, y0 + h, pz + 1, Block::Planks);
                if self.rng.chance(0.35) {
                    self.container(px, y0 + h + 1, pz, ContainerKind::WoodenCrate);
                }
            }
        }
        self.container(x1 - 2, y0 + 1, zc + 3, ContainerKind::WeaponBox);
        if self.rng.chance(0.6) {
            self.container(x0 + 2, y0 + 1, zc - 3, ContainerKind::WoodenCrate);
        }
        // Barrels outside.
        let bz = z1 + 2;
        for i in 0..self.rng.range_i32(2, 4) {
            let bx = x0 + 2 + i;
            let g = self.ground(bx, bz);
            if self.mask_at(bx, bz) != ROAD {
                self.set(bx, g + 1, bz, Block::Barrel);
            }
        }
        self.patrol(sx, y0 + 1, zc);
        self.patrol(x0 - 2, y0 + 1, zc);
        self.patrol(x1 + 2, y0 + 1, zc);
        self.info.scav_spawns.push(Vec3::new(sx as f32 + 0.5, (y0 + 1) as f32, zc as f32 + 0.5));
    }

    fn build_office(&mut self, lot: &Lot) {
        let (x0, z0, x1, z1, y0) = (lot.x0, lot.z0, lot.x1(), lot.z1(), lot.y0);
        self.fill(x0, y0 - 3, z0, x1, y0 - 1, z1, Block::Concrete);
        for s in 0..2 {
            let fy = y0 + s * 5;
            self.fill(x0, fy, z0, x1, fy, z1, Block::FloorTile);
            self.fill(x0 + 1, fy + 1, z0 + 1, x1 - 1, fy + 4, z1 - 1, Block::Air);
            self.walls(x0, z0, x1, z1, fy + 1, fy + 4, Block::Concrete);
            self.windows(x0, z0, x1, z1, fy + 2, fy + 3, 2);
            // Plaster partition with a doorway.
            let px = x0 + 7;
            self.fill(px, fy + 1, z0 + 3, px, fy + 4, z1 - 1, Block::Plaster);
            self.fill(px, fy + 1, z0 + 5, px, fy + 2, z0 + 6, Block::Air);
            self.set(x0 + 3, fy + 4, z1 - 3, Block::Lamp);
            self.set(x1 - 3, fy + 4, z1 - 3, Block::Lamp);
        }
        let roof = y0 + 10;
        self.fill(x0, roof, z0, x1, roof, z1, Block::Concrete);
        self.walls(x0, z0, x1, z1, roof + 1, roof + 1, Block::Concrete);
        self.stairs(x0 + 1, z0 + 1, y0);
        self.door(x0 + 9, z0, y0, 2, 1);
        self.fill(x0 + 9, y0 + 1, z0 - 1, x0 + 10, y0 + 2, z0 - 1, Block::Air);
        self.door(x0 + 3, z1, y0, 2, 1);
        self.fill(x0 + 3, y0 + 1, z1 + 1, x0 + 4, y0 + 2, z1 + 1, Block::Air);

        self.container(x1 - 1, y0 + 1, z1 - 1, ContainerKind::FileCabinet);
        self.container(x1 - 1, y0 + 1, z1 - 2, ContainerKind::FileCabinet);
        self.container(x0 + 1, y0 + 1, z1 - 1, ContainerKind::MedCase);
        self.container(x1 - 1, y0 + 6, z1 - 1, ContainerKind::WeaponBox);
        self.container(x1 - 2, y0 + 6, z0 + 1, ContainerKind::FileCabinet);
        self.container(x0 + 1, y0 + 6, z1 - 1, ContainerKind::WoodenCrate);

        self.patrol(x0 + 10, y0 + 1, (z0 + z1) / 2);
        self.patrol(x0 + 10, y0 + 6, (z0 + z1) / 2);
        self.patrol(x0 + 9, y0 + 1, z0 - 2);
        self.info
            .scav_spawns
            .push(Vec3::new((x0 + 10) as f32 + 0.5, (y0 + 6) as f32, ((z0 + z1) / 2) as f32 + 0.5));
    }

    fn build_shipping_container(&mut self, lot: &Lot) {
        let (x0, z0, x1, z1, y0) = (lot.x0, lot.z0, lot.x1(), lot.z1(), lot.y0);
        let mat = if self.rng.chance(0.7) { Block::RustMetal } else { Block::Metal };
        self.fill(x0, y0, z0, x1, y0, z1, Block::Metal);
        self.fill(x0 + 1, y0 + 1, z0 + 1, x1 - 1, y0 + 3, z1 - 1, Block::Air);
        self.walls(x0, z0, x1, z1, y0 + 1, y0 + 3, mat);
        self.fill(x0, y0 + 4, z0, x1, y0 + 4, z1, mat);
        let along_z = lot.d > lot.w;
        let open_low = self.rng.chance(0.5);
        if along_z {
            let z = if open_low { z0 } else { z1 };
            self.fill(x0 + 1, y0 + 1, z, x1 - 1, y0 + 3, z, Block::Air);
            let far = if open_low { z1 - 1 } else { z0 + 1 };
            if self.rng.chance(0.55) {
                self.container(x0 + 1, y0 + 1, far, ContainerKind::WoodenCrate);
            }
            let outside = if open_low { z0 - 2 } else { z1 + 2 };
            self.patrol(x0 + 1, y0 + 1, outside);
        } else {
            let x = if open_low { x0 } else { x1 };
            self.fill(x, y0 + 1, z0 + 1, x, y0 + 3, z1 - 1, Block::Air);
            let far = if open_low { x1 - 1 } else { x0 + 1 };
            if self.rng.chance(0.55) {
                self.container(far, y0 + 1, z0 + 1, ContainerKind::WoodenCrate);
            }
            let outside = if open_low { x0 - 2 } else { x1 + 2 };
            self.patrol(outside, y0 + 1, z0 + 1);
        }
    }

    // ------------------------------------------------------------------
    // Cover & decoration
    // ------------------------------------------------------------------

    fn checkpoints(&mut self) {
        let candidates = [(0.28f32, true), (0.72, true), (0.25, false), (0.78, false)];
        for (frac, on_a) in candidates {
            if !self.rng.chance(0.75) {
                continue;
            }
            let (cx, cz, dir_x) = if on_a {
                let z = (self.sz as f32 * frac) as i32;
                (self.road_a_x(z).floor() as i32, z, true)
            } else {
                let x = (self.sx as f32 * frac) as i32;
                (x, self.road_b_z(x).floor() as i32, false)
            };
            let side = if self.rng.chance(0.5) { 1 } else { -1 };
            // Sandbag U beside the road.
            for t in -3..=3 {
                for k in 4..=6 {
                    let on_edge = t == -3 || t == 3 || k == 6;
                    if !on_edge {
                        continue;
                    }
                    let (x, z) = if dir_x { (cx + k * side, cz + t) } else { (cx + t, cz + k * side) };
                    let g = self.ground(x, z);
                    self.fill(x, g + 1, z, x, g + 2, z, Block::Sandbag);
                }
            }
            let (ix, iz) = if dir_x { (cx + 5 * side, cz) } else { (cx, cz + 5 * side) };
            let g = self.ground(ix, iz);
            let kind = if self.rng.chance(0.5) { ContainerKind::MedCase } else { ContainerKind::WoodenCrate };
            let (lx, lz) = if dir_x { (ix, iz + 2) } else { (ix + 2, iz) };
            let lg = self.ground(lx, lz);
            self.container(lx, lg + 1, lz, kind);
            self.patrol(ix, g + 1, iz);
            self.info
                .scav_spawns
                .push(Vec3::new(ix as f32 + 0.5, (g + 1) as f32, iz as f32 + 0.5));
            // Concrete barrier half across the road.
            for t in 0..3 {
                let (x, z) = if dir_x { (cx - side * t, cz + 6) } else { (cx + 6, cz - side * t) };
                let g = self.ground(x, z);
                self.set(x, g + 1, z, Block::Concrete);
            }
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let (x, z) = (ix + dx, iz + dz);
                    if self.mask_at(x, z) == FREE {
                        self.set_mask(x, z, DECOR);
                    }
                }
            }
        }
    }

    fn car_wreck(&mut self, cx: i32, cz: i32, along_z: bool) {
        let g = self.ground(cx, cz);
        let (hx, hz) = if along_z { (1, 2) } else { (2, 1) };
        self.fill(cx - hx, g + 1, cz - hz, cx + hx, g + 1, cz + hz, Block::RustMetal);
        let (cx0, cz0, cx1, cz1) = if along_z {
            (cx - 1, cz - 1, cx + 1, cz)
        } else {
            (cx - 1, cz - 1, cx, cz + 1)
        };
        self.fill(cx0, g + 2, cz0, cx1, g + 2, cz1, Block::Glass);
        // Clear the cells above in case of terrain steps.
        self.fill(cx - hx, g + 3, cz - hz, cx + hx, g + 3, cz + hz, Block::Air);
    }

    fn road_cover(&mut self) {
        for _ in 0..4 {
            let z = self.rng.range_i32(20, self.sz - 20);
            let x = (self.road_a_x(z) + self.rng.range_f32(-2.0, 2.0)).floor() as i32;
            if (z as f32 - self.road_b_z(x)).abs() > 10.0 {
                self.car_wreck(x, z, true);
            }
        }
        for _ in 0..4 {
            let x = self.rng.range_i32(20, self.sx - 20);
            let z = (self.road_b_z(x) + self.rng.range_f32(-2.0, 2.0)).floor() as i32;
            if (x as f32 - self.road_a_x(z)).abs() > 10.0 {
                self.car_wreck(x, z, false);
            }
        }
        // Jersey barriers along shoulders.
        for _ in 0..10 {
            let z = self.rng.range_i32(20, self.sz - 24);
            let side = if self.rng.chance(0.5) { 4.5 } else { -4.5 };
            for dz in 0..3 {
                let x = (self.road_a_x(z + dz) + side).floor() as i32;
                let g = self.ground(x, z + dz);
                self.fill(x, g + 1, z + dz, x, g + 2, z + dz, Block::Concrete);
            }
        }
    }

    fn field_cover(&mut self) {
        let mut placed = 0;
        let mut attempts = 0;
        while placed < 22 && attempts < 600 {
            attempts += 1;
            let x = self.rng.range_i32(22, self.sx - 22);
            let z = self.rng.range_i32(22, self.sz - 22);
            if self.mask_at(x, z) != FREE {
                continue;
            }
            let kind = self.rng.range_i32(0, 3);
            let along_x = self.rng.chance(0.5);
            let len = self.rng.range_i32(3, 6);
            let mut cells = Vec::new();
            for i in 0..len {
                let (cx, cz) = if along_x { (x + i, z) } else { (x, z + i) };
                if self.mask_at(cx, cz) != FREE {
                    cells.clear();
                    break;
                }
                cells.push((cx, cz));
            }
            if cells.is_empty() {
                continue;
            }
            for (cx, cz) in cells {
                let g = self.ground(cx, cz);
                match kind {
                    0 => self.fill(cx, g + 1, cz, cx, g + 2, cz, Block::Sandbag),
                    1 => self.set(cx, g + 1, cz, Block::Log),
                    2 => {
                        self.fill(cx, g + 1, cz, cx, g + 2, cz, Block::Planks);
                    }
                    _ => {
                        self.set(cx, g + 1, cz, Block::Leaves);
                        if self.rng.chance(0.5) {
                            self.set(cx, g + 2, cz, Block::Leaves);
                        }
                    }
                }
                self.set_mask(cx, cz, DECOR);
            }
            if kind == 0 && self.rng.chance(0.3) {
                let g = self.ground(x, z + 1);
                if self.mask_at(x, z + 1) == FREE {
                    self.container(x, g + 1, z + 1, ContainerKind::WoodenCrate);
                    self.set_mask(x, z + 1, DECOR);
                }
            }
            placed += 1;
        }
        // Bushes.
        for _ in 0..90 {
            let x = self.rng.range_i32(4, self.sx - 4);
            let z = self.rng.range_i32(4, self.sz - 4);
            if self.mask_at(x, z) == FREE {
                let g = self.ground(x, z);
                if self.get(x, g, z) == Block::Grass {
                    self.set(x, g + 1, z, Block::Leaves);
                    if self.rng.chance(0.3) {
                        self.set(x, g + 2, z, Block::Leaves);
                    }
                }
            }
        }
    }

    fn trees(&mut self) {
        let mut placed = 0;
        let mut attempts = 0;
        while placed < 70 && attempts < 2000 {
            attempts += 1;
            let x = self.rng.range_i32(3, self.sx - 3);
            let z = self.rng.range_i32(3, self.sz - 3);
            let mut ok = true;
            'c: for dz in -2..=2 {
                for dx in -2..=2 {
                    if self.mask_at(x + dx, z + dz) != FREE {
                        ok = false;
                        break 'c;
                    }
                }
            }
            if !ok {
                continue;
            }
            let g = self.ground(x, z);
            if self.get(x, g, z) != Block::Grass {
                continue;
            }
            let h = self.rng.range_i32(4, 6);
            for y in g + 1..=g + h {
                self.set(x, y, z, Block::Log);
            }
            let top = g + h;
            for dy in -2..=1 {
                for dz in -2..=2 {
                    for dx in -2..=2 {
                        let r2 = dx * dx + dz * dz + dy * dy;
                        let lim = if dy <= -1 { 6 } else { 4 };
                        if r2 <= lim && !(dx == 0 && dz == 0 && dy <= 0) {
                            let (lx, ly, lz) = (x + dx, top + dy, z + dz);
                            if self.get(lx, ly, lz).is_air() && self.rng.chance(0.9) {
                                self.set(lx, ly, lz, Block::Leaves);
                            }
                        }
                    }
                }
            }
            self.set(x, top + 1, z, Block::Leaves);
            for dz in -1..=1 {
                for dx in -1..=1 {
                    self.set_mask(x + dx, z + dz, DECOR);
                }
            }
            placed += 1;
        }
    }

    // ------------------------------------------------------------------
    // Extracts & spawns
    // ------------------------------------------------------------------

    fn extract_at(&mut self, name: &str, x: i32, z: i32, along_z: bool) {
        let g = self.ground(x, z);
        self.info.extracts.push(ExtractPoint {
            name: name.to_string(),
            pos: Vec3::new(x as f32 + 0.5, (g + 1) as f32, z as f32 + 0.5),
            radius: 3.5,
        });
        // Lamp posts either side of the road.
        for side in [-5, 5] {
            let (px, pz) = if along_z { (x + side, z) } else { (x, z + side) };
            let pg = self.ground(px, pz);
            self.fill(px, pg + 1, pz, px, pg + 3, pz, Block::Log);
            self.set(px, pg + 4, pz, Block::Lamp);
        }
    }

    fn extracts(&mut self) {
        let zn = 14;
        let zs = self.sz - 15;
        let xw = 14;
        let xe = self.sx - 15;
        let a_n = self.road_a_x(zn).floor() as i32;
        let a_s = self.road_a_x(zs).floor() as i32;
        let b_w = self.road_b_z(xw).floor() as i32;
        let b_e = self.road_b_z(xe).floor() as i32;
        self.extract_at("Road to Customs", a_n, zn, false);
        self.extract_at("Crossroads", a_s, zs, false);
        self.extract_at("Railway Bridge", xw, b_w, true);
        self.extract_at("Old Gas Station", xe, b_e, true);
    }

    fn spawns(&mut self) {
        let cx = self.sx as f32 * 0.5;
        let cz = self.sz as f32 * 0.5;
        for k in 0..8 {
            let angle = k as f32 / 8.0 * std::f32::consts::TAU + self.rng.range_f32(-0.2, 0.2);
            let r = 66.0;
            let tx = (cx + angle.cos() * r) as i32;
            let tz = (cz + angle.sin() * r) as i32;
            // Spiral search for a free, open spot.
            'search: for rad in 0i32..12 {
                for dz in -rad..=rad {
                    for dx in -rad..=rad {
                        if dx.abs() != rad && dz.abs() != rad {
                            continue;
                        }
                        let (x, z) = (tx + dx, tz + dz);
                        if self.mask_at(x, z) != FREE {
                            continue;
                        }
                        let g = self.ground(x, z);
                        let clear = (1..=3).all(|dy| self.get(x, g + dy, z).is_air());
                        if clear {
                            self.info
                                .player_spawns
                                .push(Vec3::new(x as f32 + 0.5, (g + 1) as f32, z as f32 + 0.5));
                            break 'search;
                        }
                    }
                }
            }
        }
        // Road patrol points.
        let mut z = 24;
        while z < self.sz - 24 {
            let x = self.road_a_x(z).floor() as i32 + 2;
            let g = self.ground(x, z);
            if self.get(x, g + 1, z).is_air() && self.get(x, g + 2, z).is_air() {
                self.patrol(x, g + 1, z);
            }
            z += 18;
        }
        let mut x = 24;
        while x < self.sx - 24 {
            let z = self.road_b_z(x).floor() as i32 + 2;
            let g = self.ground(x, z);
            if self.get(x, g + 1, z).is_air() && self.get(x, g + 2, z).is_air() {
                self.patrol(x, g + 1, z);
            }
            x += 18;
        }
        // Extra scav spawns from patrol points.
        let pts = self.info.patrol_points.clone();
        for p in pts.iter() {
            if self.rng.chance(0.25) {
                self.info.scav_spawns.push(*p);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_playable_map() {
        let (world, info) = generate_raid_map(1234);
        assert_eq!(world.size, IVec3::new(192, 64, 192));
        assert!(info.extracts.len() >= 2);
        assert!(!info.player_spawns.is_empty());
        assert!(info.patrol_points.len() > 10);
        assert!(info.containers.len() > 10);
        for (p, kind) in &info.containers {
            assert_eq!(world.get(*p).container(), Some(*kind));
        }
        // Spawns must not be inside solid blocks.
        for s in &info.player_spawns {
            let p = s.floor().as_ivec3();
            assert!(!world.get(p).is_solid(), "spawn inside block at {p}");
            assert!(world.get(p - IVec3::Y).is_solid(), "spawn floating at {p}");
        }
    }

    #[test]
    fn same_seed_same_map() {
        let (a, ia) = generate_raid_map(99);
        let (b, ib) = generate_raid_map(99);
        assert_eq!(ia.containers.len(), ib.containers.len());
        for x in (0..192).step_by(7) {
            for z in (0..192).step_by(5) {
                assert_eq!(a.top_solid_y(x, z), b.top_solid_y(x, z));
            }
        }
    }
}
