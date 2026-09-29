//! Procedurally generated texture atlas (16x16 tiles of 16x16 pixels).

use crate::rng::{hash2, hash2f, value_noise};
use crate::world::Tile;
use glam::Vec2;

pub const TILE_PX: u32 = 16;
pub const TILES_PER_ROW: u32 = 16;
pub const ATLAS_PX: u32 = TILE_PX * TILES_PER_ROW;
pub const MIP_LEVELS: u32 = 5;

/// UV rectangle (min, max) of a tile, slightly inset to limit bleeding.
pub fn tile_uv(tile: Tile) -> (Vec2, Vec2) {
    let i = tile as u32;
    let tx = (i % TILES_PER_ROW) as f32;
    let ty = (i / TILES_PER_ROW) as f32;
    let s = 1.0 / TILES_PER_ROW as f32;
    let inset = 0.25 / ATLAS_PX as f32;
    (
        Vec2::new(tx * s + inset, ty * s + inset),
        Vec2::new((tx + 1.0) * s - inset, (ty + 1.0) * s - inset),
    )
}

type Rgba = [u8; 4];

fn c(r: i32, g: i32, b: i32) -> Rgba {
    [r.clamp(0, 255) as u8, g.clamp(0, 255) as u8, b.clamp(0, 255) as u8, 255]
}

fn shade(base: (i32, i32, i32), d: i32) -> Rgba {
    c(base.0 + d, base.1 + d, base.2 + d)
}

fn n(x: i32, y: i32, seed: u32) -> f32 {
    hash2f(x, y, seed)
}

/// Signed noise in [-amp, amp].
fn jn(x: i32, y: i32, seed: u32, amp: i32) -> i32 {
    ((n(x, y, seed) * 2.0 - 1.0) * amp as f32) as i32
}

const ALL_TILES: [Tile; Tile::COUNT] = [
    Tile::White,
    Tile::GrassTop,
    Tile::GrassSide,
    Tile::Dirt,
    Tile::Stone,
    Tile::Sand,
    Tile::Gravel,
    Tile::Asphalt,
    Tile::Concrete,
    Tile::Brick,
    Tile::Planks,
    Tile::LogSide,
    Tile::LogTop,
    Tile::Leaves,
    Tile::Metal,
    Tile::Glass,
    Tile::Sandbag,
    Tile::Bedrock,
    Tile::FloorTile,
    Tile::RustMetal,
    Tile::CrateSide,
    Tile::CrateTop,
    Tile::WeaponBoxSide,
    Tile::WeaponBoxTop,
    Tile::MedCaseSide,
    Tile::MedCaseTop,
    Tile::CabinetSide,
    Tile::WorkbenchTop,
    Tile::WorkbenchSide,
    Tile::AmmoPressTop,
    Tile::AmmoPressSide,
    Tile::MedstationTop,
    Tile::MedstationSide,
    Tile::Lamp,
    Tile::Plaster,
    Tile::StashSide,
    Tile::StashTop,
    Tile::BarrelSide,
    Tile::BarrelTop,
    Tile::AsphaltLine,
    Tile::GeneratorSide,
    Tile::GeneratorTop,
];

fn dirt(x: i32, y: i32) -> Rgba {
    let d = jn(x, y, 11, 14) + if n(x, y, 12) > 0.85 { -22 } else { 0 };
    shade((121, 85, 58), d)
}

fn planks(x: i32, y: i32, base: (i32, i32, i32), seed: u32) -> Rgba {
    let row = y / 4;
    let off = (hash2(row, 0, seed) % 16) as i32;
    let plank_tone = jn(row, 1, seed, 14);
    if y % 4 == 3 {
        return shade(base, -45);
    }
    if (x + off) % 16 == 0 {
        return shade(base, -35);
    }
    let grain = ((value_noise((x + off) as f32 * 0.35, y as f32 * 2.0, seed) - 0.5) * 30.0) as i32;
    shade(base, plank_tone + grain)
}

fn corrugated(x: i32, y: i32, base: (i32, i32, i32), seed: u32) -> Rgba {
    let ridge = match x % 4 {
        0 => 18,
        1 => 8,
        2 => -6,
        _ => -16,
    };
    shade(base, ridge + jn(x, y, seed, 5))
}

fn framed(x: i32, y: i32, inner: Rgba, frame: Rgba, w: i32) -> Rgba {
    if x < w || y < w || x >= 16 - w || y >= 16 - w {
        frame
    } else {
        inner
    }
}

fn cross(x: i32, y: i32) -> bool {
    ((6..=9).contains(&x) && (3..=12).contains(&y)) || ((3..=12).contains(&x) && (6..=9).contains(&y))
}

fn paint(tile: Tile, x: i32, y: i32) -> Rgba {
    match tile {
        Tile::White => [255, 255, 255, 255],
        Tile::GrassTop => {
            let d = jn(x, y, 1, 18) + ((value_noise(x as f32 * 0.4, y as f32 * 0.4, 2) - 0.5) * 20.0) as i32;
            shade((88, 138, 58), d)
        }
        Tile::GrassSide => {
            let edge = 3 + (hash2(x, 0, 3) % 3) as i32;
            if y < edge {
                shade((88, 138, 58), jn(x, y, 4, 16))
            } else {
                dirt(x, y)
            }
        }
        Tile::Dirt => dirt(x, y),
        Tile::Stone => {
            let blot = ((value_noise(x as f32 * 0.3, y as f32 * 0.3, 5) - 0.5) * 36.0) as i32;
            shade((126, 126, 128), blot + jn(x, y, 6, 10))
        }
        Tile::Sand => shade((214, 197, 144), jn(x, y, 7, 10)),
        Tile::Gravel => {
            let cell = hash2(x / 3, y / 3, 8);
            let tone = (cell % 50) as i32 - 25;
            if cell % 3 == 0 {
                shade((128, 112, 96), tone)
            } else {
                shade((118, 118, 118), tone + jn(x, y, 9, 8))
            }
        }
        Tile::Asphalt => {
            let speck = if n(x, y, 10) > 0.93 { 30 } else { 0 };
            shade((56, 56, 60), jn(x, y, 11, 6) + speck)
        }
        Tile::AsphaltLine => {
            if (6..=9).contains(&x) {
                shade((226, 200, 70), jn(x, y, 12, 10))
            } else {
                let speck = if n(x, y, 10) > 0.93 { 30 } else { 0 };
                shade((56, 56, 60), jn(x, y, 11, 6) + speck)
            }
        }
        Tile::Concrete => {
            let seam = if y == 15 || x == 15 { -18 } else { 0 };
            let speck = if n(x, y, 13) > 0.9 { -16 } else { 0 };
            shade((158, 158, 154), jn(x, y, 14, 7) + seam + speck)
        }
        Tile::Brick => {
            let row = y / 4;
            let off = if row % 2 == 0 { 0 } else { 4 };
            if y % 4 == 3 || (x + off) % 8 == 7 {
                shade((186, 180, 168), jn(x, y, 15, 8))
            } else {
                let brick = jn((x + off) / 8, row, 16, 16);
                shade((152, 64, 48), brick + jn(x, y, 17, 8))
            }
        }
        Tile::Planks => planks(x, y, (162, 122, 74), 18),
        Tile::LogSide => {
            let stripe = ((value_noise(x as f32 * 0.9, y as f32 * 0.12, 19) - 0.5) * 40.0) as i32;
            shade((98, 72, 45), stripe + jn(x, y, 20, 6))
        }
        Tile::LogTop => {
            let dx = x as f32 - 7.5;
            let dy = y as f32 - 7.5;
            let r = (dx * dx + dy * dy).sqrt();
            if r > 7.0 {
                shade((98, 72, 45), jn(x, y, 21, 8))
            } else if (r as i32) % 2 == 0 {
                shade((176, 138, 88), jn(x, y, 22, 6))
            } else {
                shade((146, 108, 66), jn(x, y, 23, 6))
            }
        }
        Tile::Leaves => {
            if n(x, y, 24) < 0.16 {
                [0, 0, 0, 0]
            } else {
                shade((58, 106, 40), jn(x, y, 25, 22))
            }
        }
        Tile::Metal => corrugated(x, y, (150, 156, 162), 26),
        Tile::RustMetal => {
            let rust = value_noise(x as f32 * 0.35, y as f32 * 0.35, 27);
            let base = if rust > 0.45 { (136, 76, 44) } else { (112, 120, 110) };
            corrugated(x, y, base, 28)
        }
        Tile::Glass => {
            if x == 0 || y == 0 || x == 15 || y == 15 {
                c(196, 204, 212)
            } else if (x - y == 3 || x - y == 4) && x > 3 && x < 12 {
                [206, 228, 244, 255]
            } else {
                [0, 0, 0, 0]
            }
        }
        Tile::Sandbag => {
            let row = y / 5;
            let local = y % 5;
            let off = if row % 2 == 0 { 0 } else { 4 };
            let seam = local == 4 || (x + off) % 8 == 0;
            let round = match local {
                0 => 10,
                3 => -8,
                _ => 0,
            };
            if seam {
                shade((120, 104, 70), jn(x, y, 29, 6))
            } else {
                shade((174, 154, 108), round + jn(x, y, 30, 10))
            }
        }
        Tile::Bedrock => {
            let d = if n(x, y, 31) > 0.7 { -25 } else { 0 };
            shade((52, 52, 58), jn(x, y, 32, 12) + d)
        }
        Tile::FloorTile => {
            if x % 8 == 7 || y % 8 == 7 {
                c(120, 120, 116)
            } else if ((x / 8) + (y / 8)) % 2 == 0 {
                shade((204, 204, 196), jn(x, y, 33, 5))
            } else {
                shade((172, 170, 160), jn(x, y, 34, 5))
            }
        }
        Tile::CrateSide | Tile::CrateTop => {
            let wood = planks(x, y, (178, 138, 84), 35);
            let frame = shade((110, 80, 46), jn(x, y, 36, 6));
            let brace = tile == Tile::CrateSide && ((x - y).abs() <= 1);
            if brace {
                frame
            } else {
                framed(x, y, wood, frame, 2)
            }
        }
        Tile::WeaponBoxSide => {
            let inner = if y == 7 || y == 8 {
                c(210, 180, 60)
            } else if (x == 3 || x == 12) && (5..=10).contains(&y) {
                c(40, 40, 40)
            } else {
                shade((78, 90, 54), jn(x, y, 37, 8))
            };
            framed(x, y, inner, c(52, 60, 36), 1)
        }
        Tile::WeaponBoxTop => {
            let inner = if (5..=10).contains(&x) && (y == 4 || y == 11) {
                c(40, 40, 40)
            } else {
                shade((78, 90, 54), jn(x, y, 38, 8))
            };
            framed(x, y, inner, c(52, 60, 36), 1)
        }
        Tile::MedCaseSide | Tile::MedCaseTop => {
            let inner = if cross(x, y) {
                c(200, 30, 36)
            } else {
                shade((222, 222, 214), jn(x, y, 39, 5))
            };
            framed(x, y, inner, c(150, 150, 146), 1)
        }
        Tile::CabinetSide => {
            if y % 5 == 4 {
                c(70, 76, 84)
            } else if y % 5 == 2 && (6..=9).contains(&x) {
                c(200, 200, 200)
            } else {
                shade((112, 122, 134), jn(x, y, 40, 6))
            }
        }
        Tile::WorkbenchTop => {
            if (y == 5 && (2..=10).contains(&x)) || (x == 12 && (3..=12).contains(&y)) {
                c(90, 94, 100)
            } else {
                planks(x, y, (150, 110, 68), 41)
            }
        }
        Tile::WorkbenchSide => {
            if y < 3 {
                planks(x, y, (150, 110, 68), 41)
            } else if (x < 2 || x > 13) || y > 13 {
                c(60, 62, 66)
            } else if y == 8 {
                c(80, 60, 40)
            } else {
                [30, 30, 32, 255]
            }
        }
        Tile::AmmoPressTop | Tile::AmmoPressSide => {
            let brass = (x % 4 == 1 || x % 4 == 2) && (y % 4 == 1 || y % 4 == 2) && y > 4;
            if brass && tile == Tile::AmmoPressTop {
                c(206, 164, 64)
            } else if tile == Tile::AmmoPressSide && (6..=9).contains(&x) && y > 2 {
                c(140, 30, 30)
            } else {
                framed(x, y, shade((72, 76, 82), jn(x, y, 42, 6)), c(40, 42, 46), 1)
            }
        }
        Tile::MedstationTop | Tile::MedstationSide => {
            let inner = if cross(x, y) {
                c(40, 170, 70)
            } else {
                shade((228, 230, 226), jn(x, y, 43, 4))
            };
            framed(x, y, inner, c(90, 110, 100), 1)
        }
        Tile::Lamp => {
            if x == 0 || y == 0 || x == 15 || y == 15 {
                c(90, 90, 90)
            } else {
                c(255, 240 - jn(x, y, 44, 6).abs(), 196)
            }
        }
        Tile::Plaster => shade((204, 198, 182), jn(x, y, 45, 6)),
        Tile::StashSide | Tile::StashTop => {
            let inner = if y == 3 || y == 12 {
                c(220, 190, 40)
            } else {
                shade((60, 74, 60), jn(x, y, 46, 6))
            };
            framed(x, y, inner, c(34, 40, 34), 1)
        }
        Tile::BarrelSide => {
            if y == 2 || y == 13 || y == 7 {
                c(30, 50, 100)
            } else {
                shade((44, 76, 146), jn(x, y, 47, 8))
            }
        }
        Tile::BarrelTop => {
            let dx = x as f32 - 7.5;
            let dy = y as f32 - 7.5;
            if (dx * dx + dy * dy).sqrt() > 6.5 {
                c(30, 50, 100)
            } else if (x - 11).abs() <= 1 && (y - 4).abs() <= 1 {
                c(20, 20, 20)
            } else {
                shade((44, 76, 146), jn(x, y, 48, 6))
            }
        }
        Tile::GeneratorSide => {
            if (3..=12).contains(&x) && (4..=11).contains(&y) && y % 2 == 0 {
                c(30, 30, 30)
            } else {
                framed(x, y, shade((220, 160, 40), jn(x, y, 49, 8)), c(60, 60, 60), 1)
            }
        }
        Tile::GeneratorTop => framed(x, y, shade((200, 146, 36), jn(x, y, 50, 8)), c(60, 60, 60), 1),
    }
}

pub struct Atlas {
    /// RGBA8 pixel data for each mip level.
    pub levels: Vec<Vec<u8>>,
    /// Average colour of each tile (used for particles and UI).
    pub avg_colors: Vec<[u8; 4]>,
}

pub fn generate() -> Atlas {
    let size = ATLAS_PX as usize;
    let mut base = vec![0u8; size * size * 4];
    let mut avg_colors = vec![[255u8; 4]; (TILES_PER_ROW * TILES_PER_ROW) as usize];
    for tile in ALL_TILES {
        let i = tile as u32;
        let ox = (i % TILES_PER_ROW) * TILE_PX;
        let oy = (i / TILES_PER_ROW) * TILE_PX;
        let mut sum = [0u32; 3];
        let mut count = 0u32;
        for y in 0..TILE_PX as i32 {
            for x in 0..TILE_PX as i32 {
                let px = paint(tile, x, y);
                let idx = (((oy as i32 + y) as usize) * size + (ox as i32 + x) as usize) * 4;
                base[idx..idx + 4].copy_from_slice(&px);
                if px[3] > 0 {
                    sum[0] += px[0] as u32;
                    sum[1] += px[1] as u32;
                    sum[2] += px[2] as u32;
                    count += 1;
                }
            }
        }
        if count > 0 {
            avg_colors[i as usize] = [
                (sum[0] / count) as u8,
                (sum[1] / count) as u8,
                (sum[2] / count) as u8,
                255,
            ];
        }
    }

    let mut levels = vec![base];
    let mut dim = size;
    for _ in 1..MIP_LEVELS {
        let prev = levels.last().unwrap();
        let nd = dim / 2;
        let mut next = vec![0u8; nd * nd * 4];
        for y in 0..nd {
            for x in 0..nd {
                let mut acc = [0u32; 4];
                let mut wsum = 0u32;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let si = ((y * 2 + dy) * dim + (x * 2 + dx)) * 4;
                    let a = prev[si + 3] as u32;
                    // Alpha-weighted colour so transparent texels don't darken edges.
                    acc[0] += prev[si] as u32 * a;
                    acc[1] += prev[si + 1] as u32 * a;
                    acc[2] += prev[si + 2] as u32 * a;
                    acc[3] += a;
                    wsum += a;
                }
                let di = (y * nd + x) * 4;
                if wsum > 0 {
                    next[di] = (acc[0] / wsum) as u8;
                    next[di + 1] = (acc[1] / wsum) as u8;
                    next[di + 2] = (acc[2] / wsum) as u8;
                }
                next[di + 3] = (acc[3] / 4) as u8;
            }
        }
        levels.push(next);
        dim = nd;
    }
    Atlas { levels, avg_colors }
}

pub fn avg_color(atlas_colors: &[[u8; 4]], tile: Tile) -> [u8; 4] {
    atlas_colors[tile as usize]
}
