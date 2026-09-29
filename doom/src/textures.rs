//! Procedurally generated wall/floor/ceiling textures and the sky.
//!
//! Every surface is a 64x64 layer of one texture array. Alpha is repurposed as
//! an *emissive* mask: 1.0 means "lit by the world", lower values glow
//! regardless of light level (screens, lamps, nukage...).

use crate::font;
use crate::math::{fbm, hash2, value_noise};

pub const TEX: usize = 64;

// Walls.
pub const T_BRICK: u8 = 0;
pub const T_STONE: u8 = 1;
pub const T_TECH: u8 = 2;
pub const T_WOOD: u8 = 3;
pub const T_MARBLE: u8 = 4;
pub const T_FLESH: u8 = 5;
pub const T_DOOR: u8 = 6;
pub const T_DOORTRAK: u8 = 7;
pub const T_DOOR_RED: u8 = 8;
pub const T_DOOR_BLUE: u8 = 9;
pub const T_DOOR_YELLOW: u8 = 10;
pub const T_EXIT_OFF: u8 = 11;
pub const T_EXIT_ON: u8 = 12;
pub const T_COMPUTER: u8 = 13;
pub const T_SUPPORT: u8 = 14;
pub const T_HELLROCK: u8 = 15;
// Flats.
pub const F_TILE: u8 = 16;
pub const F_DIRT: u8 = 17;
pub const F_NUKAGE: u8 = 18;
pub const F_LAVA: u8 = 19;
pub const F_CEIL_LIGHT: u8 = 20;
pub const F_CEIL_METAL: u8 = 21;
pub const F_WOOD: u8 = 22;
pub const F_GRATE: u8 = 23;
pub const F_ROCK: u8 = 24;
pub const F_CARPET: u8 = 25;
pub const NUM_SURFACES: usize = 26;

/// Ceiling value meaning "open sky".
pub const SKY: u8 = 255;

pub const SKY_W: usize = 256;
pub const SKY_H: usize = 128;

/// Floating point RGBA image used while generating assets.
#[derive(Clone)]
pub struct Image {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[f32; 4]>,
}

impl Image {
    pub fn new(w: usize, h: usize) -> Self {
        Image { w, h, px: vec![[0.0; 4]; w * h] }
    }

    pub fn get(&self, x: usize, y: usize) -> [f32; 4] {
        self.px[y * self.w + x]
    }

    pub fn set(&mut self, x: usize, y: usize, c: [f32; 4]) {
        if x < self.w && y < self.h {
            self.px[y * self.w + x] = c;
        }
    }

    pub fn to_rgba8(&self) -> Vec<u8> {
        self.px
            .iter()
            .flat_map(|p| p.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8))
            .collect()
    }

    /// Box-filter mip chain. With `cutout` set, texels with alpha < 0.5 are
    /// treated as holes so sprites keep their silhouettes when shrunk.
    pub fn mip_chain(&self, cutout: bool) -> Vec<Image> {
        let mut out = vec![self.clone()];
        while out.last().unwrap().w > 1 {
            let src = out.last().unwrap();
            let (w, h) = (src.w / 2, src.h / 2);
            let mut dst = Image::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let taps = [
                        src.get(2 * x, 2 * y),
                        src.get(2 * x + 1, 2 * y),
                        src.get(2 * x, 2 * y + 1),
                        src.get(2 * x + 1, 2 * y + 1),
                    ];
                    let mut acc = [0.0f32; 4];
                    let mut n = 0.0;
                    for t in taps {
                        if !cutout || t[3] >= 0.5 {
                            for i in 0..4 {
                                acc[i] += t[i];
                            }
                            n += 1.0;
                        }
                    }
                    let keep = if cutout { n >= 2.0 } else { n > 0.0 };
                    if keep {
                        dst.set(x, y, acc.map(|v| v / n));
                    }
                }
            }
            out.push(dst);
        }
        out
    }
}

type Rgb = [f32; 3];

fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

fn scale(c: Rgb, s: f32) -> Rgb {
    [c[0] * s, c[1] * s, c[2] * s]
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn px(c: Rgb) -> [f32; 4] {
    [c[0], c[1], c[2], 1.0]
}

/// Pixel with an emissive strength in [0, 1].
fn glow(c: Rgb, e: f32) -> [f32; 4] {
    [c[0], c[1], c[2], 1.0 - e.clamp(0.0, 1.0)]
}

fn fill(f: impl Fn(usize, usize) -> [f32; 4]) -> Image {
    let mut img = Image::new(TEX, TEX);
    for y in 0..TEX {
        for x in 0..TEX {
            img.set(x, y, f(x, y));
        }
    }
    img
}

fn n(x: usize, y: usize, base: i32, oct: u32, seed: u32) -> f32 {
    fbm(x as f32, y as f32, TEX as f32, base, oct, seed)
}

/// Stamp text into a texture with the 8x8 font.
fn stamp_text(img: &mut Image, text: &str, x0: usize, y0: usize, color: [f32; 4]) {
    for (i, ch) in text.bytes().enumerate() {
        for gy in 0..8 {
            for gx in 0..8 {
                if font::pixel(ch, gx, gy) {
                    img.set(x0 + i * 8 + gx, y0 + gy, color);
                }
            }
        }
    }
}

fn brick(x: usize, y: usize) -> [f32; 4] {
    let row = y / 8;
    let off = if row % 2 == 1 { 8 } else { 0 };
    let bx = ((x + off) % TEX) / 16;
    let lx = (x + off) % 16;
    let ly = y % 8;
    let grain = n(x, y, 16, 3, 11);
    if lx == 0 || ly == 0 {
        return px(scale(rgb(58, 50, 44), 0.8 + 0.4 * grain));
    }
    let tone = 0.75 + 0.45 * hash2(bx as i32, row as i32, 3);
    let mut c = scale(rgb(142, 70, 46), tone * (0.8 + 0.4 * grain));
    if ly == 1 || lx == 1 {
        c = scale(c, 1.2);
    }
    if ly == 7 || lx == 15 {
        c = scale(c, 0.7);
    }
    if hash2(x as i32, y as i32, 5) > 0.93 {
        c = scale(c, 0.8);
    }
    px(c)
}

fn stone(x: usize, y: usize) -> [f32; 4] {
    let row = y / 16;
    let off = [0, 16, 8, 24][row % 4];
    let bx = ((x + off) % TEX) / 32;
    let lx = (x + off) % 32;
    let ly = y % 16;
    let grain = n(x, y, 8, 4, 21);
    if lx == 0 || ly == 0 {
        return px(scale(rgb(40, 40, 42), 0.8 + 0.3 * grain));
    }
    let tone = 0.8 + 0.3 * hash2(bx as i32, row as i32, 7);
    let mut c = scale(rgb(118, 116, 110), tone * (0.7 + 0.5 * grain));
    let crack = (n(x, y, 4, 3, 99) - 0.5).abs();
    if crack < 0.012 {
        c = scale(c, 0.55);
    }
    if ly == 1 || lx == 1 {
        c = scale(c, 1.15);
    }
    if ly == 15 || lx == 31 {
        c = scale(c, 0.72);
    }
    px(c)
}

fn metal_base(x: usize, y: usize, seed: u32) -> Rgb {
    let g = n(x, y, 8, 4, seed);
    let streak = value_noise(x as f32 * 0.5, y as f32 * 0.06, 32, seed + 1);
    scale(rgb(92, 96, 104), 0.75 + 0.25 * g + 0.15 * streak)
}

fn tech(x: usize, y: usize) -> [f32; 4] {
    let base = metal_base(x, y, 31);
    // Horizontal trims.
    if !(3..61).contains(&y) {
        return px(scale(base, if y == 0 || y == 61 { 1.3 } else { 0.55 }));
    }
    // Upper panel with inset.
    if (6..40).contains(&y) {
        let edge = x == 4 || x == 59 || y == 6 || y == 39;
        let inner = (5..59).contains(&x) && (7..39).contains(&y);
        if edge {
            return px(scale(base, 0.45));
        }
        if inner {
            let mut c = scale(base, 0.95);
            if y == 7 || x == 5 {
                c = scale(c, 1.35);
            }
            // Rivets.
            for (rx, ry) in [(9, 11), (54, 11), (9, 34), (54, 34)] {
                let d = (x as i32 - rx).pow(2) + (y as i32 - ry).pow(2);
                if d <= 2 {
                    c = scale(rgb(170, 170, 176), if d == 0 { 1.2 } else { 0.8 });
                }
            }
            return px(c);
        }
    }
    // Status lights strip.
    if (43..49).contains(&y) {
        let slot = x / 8;
        let lx = x % 8;
        if (2..6).contains(&lx) && (44..48).contains(&y) {
            let colors = [rgb(40, 255, 60), rgb(255, 40, 30), rgb(255, 200, 40), rgb(40, 255, 60)];
            let on = hash2(slot as i32, 0, 77) > 0.35;
            let c = colors[slot % 4];
            return if on { glow(c, 0.9) } else { px(scale(c, 0.25)) };
        }
        return px(scale(base, 0.35));
    }
    // Vents.
    if (51..59).contains(&y) && (8..56).contains(&x) {
        return px(scale(base, if y.is_multiple_of(2) { 0.3 } else { 0.8 }));
    }
    px(base)
}

fn wood(x: usize, y: usize) -> [f32; 4] {
    let plank = x / 16;
    let lx = x % 16;
    let warp = n(x, y, 4, 3, 41 + plank as u32) * 6.0;
    let grain = ((lx as f32 + warp) * 1.3).sin() * 0.5 + 0.5;
    let tone = 0.8 + 0.3 * hash2(plank as i32, 0, 13);
    let mut c = mix(rgb(96, 58, 30), rgb(138, 88, 48), grain);
    c = scale(c, tone * (0.85 + 0.2 * n(x, y, 16, 2, 42)));
    if lx == 0 {
        c = scale(c, 0.4);
    } else if lx == 1 {
        c = scale(c, 1.2);
    }
    // Horizontal braces.
    if (4..8).contains(&y) || (56..60).contains(&y) {
        c = scale(rgb(70, 44, 24), 0.9 + 0.2 * grain);
        if y == 4 || y == 56 {
            c = scale(c, 1.3);
        }
    }
    px(c)
}

fn marble(x: usize, y: usize) -> [f32; 4] {
    let border = x < 3 || y < 3 || x >= 61 || y >= 61;
    if border {
        let bevel = if x == 0 || y == 0 { 1.4 } else if x == 63 || y == 63 { 0.6 } else { 1.0 };
        return px(scale(rgb(150, 118, 60), bevel * (0.8 + 0.3 * n(x, y, 8, 2, 3))));
    }
    let warp = n(x, y, 4, 4, 51) * 9.0;
    let v = ((x as f32 * 0.09 + y as f32 * 0.14 + warp).sin()).abs();
    let mut c = mix(rgb(24, 66, 40), rgb(46, 104, 66), n(x, y, 8, 3, 52));
    if v < 0.12 {
        c = mix(rgb(190, 214, 190), c, v / 0.12);
    }
    px(c)
}

fn flesh(x: usize, y: usize) -> [f32; 4] {
    let bumps = n(x, y, 8, 4, 61);
    let mut c = mix(rgb(120, 36, 30), rgb(196, 92, 80), bumps);
    let vein = (n(x, y, 4, 3, 62) - 0.5).abs();
    if vein < 0.03 {
        c = mix(rgb(70, 10, 20), c, vein / 0.03);
    }
    let pore = hash2(x as i32, y as i32, 63);
    if pore > 0.97 {
        c = scale(c, 0.6);
    }
    px(c)
}

fn door_base(x: usize, y: usize) -> [f32; 4] {
    let base = metal_base(x, y, 71);
    // Frame.
    if !(3..61).contains(&x) || y < 3 {
        return px(scale(base, if x == 0 || y == 0 { 1.3 } else { 0.5 }));
    }
    // Hazard stripes along the bottom.
    if y >= 54 {
        let stripe = ((x + y) / 5).is_multiple_of(2);
        return px(if stripe { rgb(210, 170, 30) } else { rgb(30, 28, 24) });
    }
    let mut c = base;
    // Raised horizontal ribs.
    let ry = (y as i32 - 3) % 12;
    if ry == 0 {
        c = scale(c, 1.35);
    } else if ry == 1 {
        c = scale(c, 1.1);
    } else if ry == 11 {
        c = scale(c, 0.55);
    }
    // Central seam.
    if x == 31 || x == 32 {
        c = scale(c, if x == 31 { 0.4 } else { 1.2 });
    }
    px(c)
}

fn key_door(x: usize, y: usize, key: Rgb) -> [f32; 4] {
    let base = door_base(x, y);
    if (22..40).contains(&y) && (3..61).contains(&x) {
        if y == 22 || y == 39 {
            return px(rgb(30, 30, 30));
        }
        // Skull-ish key emblem in the middle of the band.
        let dx = x as f32 - 31.5;
        let dy = y as f32 - 30.5;
        let skull = dx * dx / 36.0 + dy * dy / 42.0 < 1.0;
        let eye = ((dx.abs() - 2.5).powi(2) + (dy + 1.0).powi(2)) < 2.2;
        if skull && !eye {
            return glow(mix(key, rgb(255, 255, 255), 0.35), 0.6);
        }
        return glow(scale(key, 0.85 + 0.2 * n(x, y, 8, 2, 5)), 0.35);
    }
    base
}

fn doortrak(x: usize, y: usize) -> [f32; 4] {
    let base = metal_base(x, y, 81);
    let lx = x % 16;
    if lx < 4 {
        let stripe = ((y + x * 2) / 6).is_multiple_of(2);
        return px(if stripe { rgb(200, 160, 30) } else { rgb(24, 22, 20) });
    }
    let mut c = scale(base, 0.7);
    if lx == 4 || lx == 15 {
        c = scale(c, 0.5);
    }
    if lx == 9 {
        c = scale(c, 1.4);
    }
    px(c)
}

fn exit_switch(on: bool) -> Image {
    let mut img = fill(|x, y| {
        let base = metal_base(x, y, 91);
        if !(2..62).contains(&x) || !(2..62).contains(&y) {
            return px(scale(base, 0.5));
        }
        // Sign plate behind the text.
        if (14..50).contains(&x) && (4..18).contains(&y) {
            return px(rgb(20, 16, 14));
        }
        // Switch housing.
        if (20..44).contains(&x) && (24..56).contains(&y) {
            let edge = x == 20 || x == 43 || y == 24 || y == 55;
            return px(if edge { scale(base, 1.3) } else { scale(rgb(40, 40, 44), 0.9) });
        }
        px(scale(base, 0.85))
    });
    let red = glow(rgb(255, 40, 30), 1.0);
    stamp_text(&mut img, "EXIT", 16, 7, red);
    // Lever: up when off, down when on.
    let (ly0, ly1) = if on { (40, 52) } else { (28, 40) };
    for y in ly0..ly1 {
        for x in 30..34 {
            img.set(x, y, px(rgb(200, 200, 205)));
        }
    }
    let lamp = if on { rgb(40, 255, 60) } else { rgb(255, 50, 30) };
    for y in 27..31 {
        for x in 37..41 {
            img.set(x, y, glow(lamp, 1.0));
        }
    }
    img
}

fn computer(x: usize, y: usize) -> [f32; 4] {
    let base = scale(metal_base(x, y, 101), 0.7);
    if !(2..62).contains(&x) || !(2..62).contains(&y) {
        return px(scale(base, 0.6));
    }
    if (6..58).contains(&x) && (6..36).contains(&y) {
        if x == 6 || x == 57 || y == 6 || y == 35 {
            return px(rgb(20, 20, 20));
        }
        let row = (y as i32 - 8).div_euclid(4);
        let on = y >= 8 && (y - 8) % 4 < 2 && hash2(x as i32 / 2, row, 7) > 0.45 && x < 50;
        return if on { glow(rgb(90, 255, 120), 0.9) } else { glow(rgb(6, 30, 12), 0.5) };
    }
    if (6..58).contains(&x) && (42..58).contains(&y) {
        let bx = (x - 6) % 6;
        let by = (y - 42) % 5;
        if bx < 4 && by < 3 {
            let h = hash2((x - 6) as i32 / 6, (y - 42) as i32 / 5, 9);
            let c = if h > 0.85 {
                return glow(rgb(255, 60, 40), 0.8);
            } else if h > 0.7 {
                return glow(rgb(255, 210, 60), 0.8);
            } else {
                rgb(150, 150, 150)
            };
            return px(c);
        }
        return px(scale(base, 0.5));
    }
    px(base)
}

fn support(x: usize, y: usize) -> [f32; 4] {
    let base = metal_base(x, y, 111);
    let lx = x % 32;
    let mut c = if (10..22).contains(&lx) { scale(base, 1.15) } else { scale(base, 0.7) };
    if lx == 10 || lx == 21 {
        c = scale(c, 0.5);
    }
    if (lx == 4 || lx == 27) && y % 8 == 4 {
        c = rgb(180, 180, 185);
    }
    px(scale(c, 0.9 + 0.1 * y.is_multiple_of(8) as i32 as f32))
}

fn hellrock(x: usize, y: usize) -> [f32; 4] {
    let g = n(x, y, 4, 5, 121);
    let mut c = mix(rgb(40, 14, 10), rgb(110, 50, 32), g);
    let crack = (n(x, y, 4, 3, 122) - 0.5).abs();
    if crack < 0.02 {
        let t = 1.0 - crack / 0.02;
        return glow(mix(c, rgb(255, 140, 30), t), t * 0.9);
    }
    if g > 0.62 {
        c = scale(c, 1.2);
    }
    px(c)
}

fn tile(x: usize, y: usize) -> [f32; 4] {
    let tx = x / 16;
    let ty = y / 16;
    let lx = x % 16;
    let ly = y % 16;
    if lx == 0 || ly == 0 {
        return px(rgb(46, 44, 40));
    }
    let tone = 0.85 + 0.25 * hash2(tx as i32, ty as i32, 131);
    let mut c = scale(rgb(128, 122, 108), tone * (0.85 + 0.2 * n(x, y, 16, 3, 132)));
    if lx == 1 || ly == 1 {
        c = scale(c, 1.12);
    }
    if lx == 15 || ly == 15 {
        c = scale(c, 0.8);
    }
    px(c)
}

fn dirt(x: usize, y: usize) -> [f32; 4] {
    let g = n(x, y, 4, 5, 141);
    let mut c = mix(rgb(64, 48, 32), rgb(118, 92, 62), g);
    let h = hash2(x as i32, y as i32, 142);
    if h > 0.94 {
        c = scale(rgb(140, 130, 116), 0.8 + h * 0.2);
    } else if h < 0.05 {
        c = scale(c, 0.6);
    }
    px(c)
}

fn nukage(x: usize, y: usize) -> [f32; 4] {
    let g = n(x, y, 4, 4, 151);
    let blob = n(x, y, 8, 2, 152);
    let mut c = mix(rgb(20, 110, 20), rgb(90, 230, 60), g);
    if blob > 0.62 {
        c = mix(c, rgb(170, 255, 120), (blob - 0.62) * 3.0);
    }
    glow(c, 0.55)
}

fn lava(x: usize, y: usize) -> [f32; 4] {
    let g = n(x, y, 4, 4, 161);
    let crust = n(x, y, 8, 3, 162);
    if crust > 0.6 {
        return px(mix(rgb(70, 30, 20), rgb(30, 14, 10), (crust - 0.6) * 2.5));
    }
    glow(mix(rgb(220, 60, 10), rgb(255, 210, 60), g), 0.85)
}

fn ceil_light(x: usize, y: usize) -> [f32; 4] {
    let base = metal_base(x, y, 171);
    let d = (x as i32 - 32).abs().max((y as i32 - 32).abs());
    if d < 14 {
        let t = 1.0 - d as f32 / 14.0;
        return glow(mix(rgb(230, 230, 210), rgb(255, 255, 245), t), 1.0);
    }
    if d < 16 {
        return px(scale(base, 0.4));
    }
    if d == 31 || d == 30 {
        return px(scale(base, 0.5));
    }
    px(base)
}

fn ceil_metal(x: usize, y: usize) -> [f32; 4] {
    let base = scale(metal_base(x, y, 181), 0.85);
    let lx = x % 32;
    let ly = y % 32;
    if lx == 0 || ly == 0 {
        return px(scale(base, 0.45));
    }
    if lx == 1 || ly == 1 {
        return px(scale(base, 1.25));
    }
    if (lx == 4 || lx == 28) && (ly == 4 || ly == 28) {
        return px(rgb(170, 170, 175));
    }
    px(base)
}

fn floor_wood(x: usize, y: usize) -> [f32; 4] {
    let plank = y / 8;
    let ly = y % 8;
    let off = (hash2(plank as i32, 0, 191) * 64.0) as usize;
    let lx = (x + off) % 64;
    let warp = n(x, y, 4, 3, 192) * 5.0;
    let grain = ((ly as f32 + warp) * 1.7).sin() * 0.5 + 0.5;
    let mut c = mix(rgb(86, 50, 26), rgb(128, 80, 42), grain);
    c = scale(c, 0.85 + 0.25 * hash2(plank as i32, 1, 193));
    if ly == 0 || lx == 0 {
        c = scale(c, 0.45);
    }
    px(c)
}

fn grate(x: usize, y: usize) -> [f32; 4] {
    let base = metal_base(x, y, 201);
    let lx = x % 8;
    let ly = y % 8;
    if (2..7).contains(&lx) && (2..7).contains(&ly) {
        return px(scale(base, 0.15));
    }
    if lx == 1 || ly == 1 {
        return px(scale(base, 1.2));
    }
    px(scale(base, 0.8))
}

fn rock(x: usize, y: usize) -> [f32; 4] {
    let g = n(x, y, 4, 5, 211);
    let mut c = mix(rgb(50, 22, 16), rgb(104, 54, 38), g);
    let cell = (n(x, y, 8, 2, 212) * 5.0).fract();
    if cell < 0.08 {
        c = scale(c, 0.55);
    }
    px(c)
}

fn carpet(x: usize, y: usize) -> [f32; 4] {
    let g = n(x, y, 16, 3, 221);
    let c = mix(rgb(96, 12, 14), rgb(140, 24, 22), g);
    if x.is_multiple_of(32) || y.is_multiple_of(32) {
        return px(rgb(160, 120, 40));
    }
    px(scale(c, 0.9 + 0.2 * hash2(x as i32, y as i32, 222)))
}

/// All surface layers in index order.
pub fn build_surfaces() -> Vec<Image> {
    let mut v = Vec::with_capacity(NUM_SURFACES);
    for i in 0..NUM_SURFACES as u8 {
        let img = match i {
            T_BRICK => fill(brick),
            T_STONE => fill(stone),
            T_TECH => fill(tech),
            T_WOOD => fill(wood),
            T_MARBLE => fill(marble),
            T_FLESH => fill(flesh),
            T_DOOR => fill(door_base),
            T_DOORTRAK => fill(doortrak),
            T_DOOR_RED => fill(|x, y| key_door(x, y, rgb(230, 30, 30))),
            T_DOOR_BLUE => fill(|x, y| key_door(x, y, rgb(40, 80, 255))),
            T_DOOR_YELLOW => fill(|x, y| key_door(x, y, rgb(250, 210, 30))),
            T_EXIT_OFF => exit_switch(false),
            T_EXIT_ON => exit_switch(true),
            T_COMPUTER => fill(computer),
            T_SUPPORT => fill(support),
            T_HELLROCK => fill(hellrock),
            F_TILE => fill(tile),
            F_DIRT => fill(dirt),
            F_NUKAGE => fill(nukage),
            F_LAVA => fill(lava),
            F_CEIL_LIGHT => fill(ceil_light),
            F_CEIL_METAL => fill(ceil_metal),
            F_WOOD => fill(floor_wood),
            F_GRATE => fill(grate),
            F_ROCK => fill(rock),
            F_CARPET => fill(carpet),
            _ => unreachable!(),
        };
        v.push(img);
    }
    v
}

/// Blood-red hell sky with a jagged mountain range; tiles horizontally.
pub fn build_sky() -> Image {
    let mut img = Image::new(SKY_W, SKY_H);
    for x in 0..SKY_W {
        let xf = x as f32;
        let ridge = 0.5 * value_noise(xf / 32.0, 0.5, (SKY_W / 32) as i32, 301)
            + 0.3 * value_noise(xf / 16.0, 0.5, (SKY_W / 16) as i32, 302)
            + 0.2 * value_noise(xf / 4.0, 0.5, (SKY_W / 4) as i32, 303);
        let mountain_top = SKY_H as f32 * (0.62 + 0.25 * (1.0 - ridge));
        for y in 0..SKY_H {
            let yf = y as f32;
            let t = yf / SKY_H as f32;
            let mut c = mix(rgb(24, 2, 4), rgb(210, 70, 24), t.powf(1.6));
            let cloud = fbm(xf, yf * 2.0, SKY_W as f32, 8, 4, 304);
            if cloud > 0.5 {
                c = mix(c, rgb(255, 150, 70), ((cloud - 0.5) * 2.2).min(1.0) * (0.3 + 0.5 * t));
            }
            if yf > mountain_top {
                let depth = (yf - mountain_top) / 12.0;
                let rock = fbm(xf, yf, SKY_W as f32, 16, 3, 305);
                c = mix(rgb(60, 24, 16), rgb(18, 8, 6), depth.min(1.0));
                c = scale(c, 0.8 + 0.4 * rock);
            }
            img.set(x, y, [c[0], c[1], c[2], 1.0]);
        }
    }
    img
}
