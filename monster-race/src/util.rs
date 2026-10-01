//! Small shared helpers: a seeded RNG, value noise, easing and color conversion.

use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

/// Deterministic xorshift RNG. Courses are generated from fixed seeds so every
/// run (and every player) sees the same beast.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x2545_f491_4f6c_dd1d)
    }

    pub fn u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 24) as u32
    }

    /// Uniform in `[0, 1)`.
    pub fn f(&mut self) -> f32 {
        (self.u32() & 0x00ff_ffff) as f32 / 16_777_216.0
    }

    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }

    /// Uniform in `[-a, a)`.
    pub fn sym(&mut self, a: f32) -> f32 {
        self.range(-a, a)
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.u32() as usize) % n.max(1)
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.f() < p
    }

    pub fn angle(&mut self) -> f32 {
        self.f() * TAU
    }
}

fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h =
        (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0x00ff_ffff) as f32 / 16_777_215.0
}

/// Value noise in `[0, 1]`. A positive `period` makes it tile every `period` cells.
pub fn noise2(x: f32, y: f32, period: i32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (xf, yf) = (smooth(x - xi as f32), smooth(y - yi as f32));
    let w = |v: i32| if period > 0 { v.rem_euclid(period) } else { v };
    let a = hash2(w(xi), w(yi), seed);
    let b = hash2(w(xi + 1), w(yi), seed);
    let c = hash2(w(xi), w(yi + 1), seed);
    let d = hash2(w(xi + 1), w(yi + 1), seed);
    let ab = a + (b - a) * xf;
    let cd = c + (d - c) * xf;
    ab + (cd - ab) * yf
}

/// Fractal value noise in `[0, 1]`.
pub fn fbm2(x: f32, y: f32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut freq) = (0.0, 0.5, 0.0, 1.0);
    for o in 0..octaves {
        sum += amp * noise2(x * freq, y * freq, 0, seed.wrapping_add(o * 17));
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

pub fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    smooth(((x - a) / (b - a)).clamp(0.0, 1.0))
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Frame-rate independent approach of `current` toward `target`.
pub fn damp(current: f32, target: f32, rate: f32, dt: f32) -> f32 {
    target + (current - target) * (-rate * dt).exp()
}

pub fn damp_v(current: Vec3, target: Vec3, rate: f32, dt: f32) -> Vec3 {
    target + (current - target) * (-rate * dt).exp()
}

pub fn wrap_angle(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

/// sRGB hex to a `Color`.
pub fn hex(h: u32) -> Color {
    Color::srgb_u8((h >> 16) as u8, (h >> 8) as u8, h as u8)
}

/// sRGB hex to linear RGBA, the space vertex colors are stored in.
pub fn lin(h: u32) -> [f32; 4] {
    hex(h).to_linear().to_f32_array()
}

pub fn mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        lerp(a[0], b[0], t),
        lerp(a[1], b[1], t),
        lerp(a[2], b[2], t),
        lerp(a[3], b[3], t),
    ]
}

/// Scales a linear color's brightness.
pub fn shade(c: [f32; 4], k: f32) -> [f32; 4] {
    [c[0] * k, c[1] * k, c[2] * k, c[3]]
}

pub fn ordinal(place: usize) -> &'static str {
    match place {
        1 => "ST",
        2 => "ND",
        3 => "RD",
        _ => "TH",
    }
}

pub fn format_time(t: f32) -> String {
    let m = (t / 60.0).floor() as u32;
    let s = t - m as f32 * 60.0;
    format!("{m}:{s:05.2}")
}
