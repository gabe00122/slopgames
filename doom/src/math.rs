//! Tiny 2D vector type and a deterministic RNG. The world is a flat grid, so
//! nearly all game math happens in the XY plane.

use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

impl V2 {
    pub const ZERO: V2 = V2 { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        V2 { x, y }
    }

    /// Unit vector for an angle. Angles grow clockwise because +y points "down"
    /// the map (south), matching row order in the level strings.
    pub fn from_angle(a: f32) -> Self {
        V2::new(a.cos(), a.sin())
    }

    pub fn dot(self, o: V2) -> f32 {
        self.x * o.x + self.y * o.y
    }

    pub fn len2(self) -> f32 {
        self.dot(self)
    }

    pub fn len(self) -> f32 {
        self.len2().sqrt()
    }

    pub fn norm(self) -> V2 {
        let l = self.len();
        if l > 1e-6 { self * (1.0 / l) } else { V2::ZERO }
    }

    /// Perpendicular pointing to the right of `self` (in a y-down world).
    pub fn right(self) -> V2 {
        V2::new(-self.y, self.x)
    }

    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }

    pub fn dist(self, o: V2) -> f32 {
        (self - o).len()
    }
}

impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        V2::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        V2::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, s: f32) -> V2 {
        V2::new(self.x * s, self.y * s)
    }
}

impl Neg for V2 {
    type Output = V2;
    fn neg(self) -> V2 {
        V2::new(-self.x, -self.y)
    }
}

impl AddAssign for V2 {
    fn add_assign(&mut self, o: V2) {
        self.x += o.x;
        self.y += o.y;
    }
}

impl SubAssign for V2 {
    fn sub_assign(&mut self, o: V2) {
        self.x -= o.x;
        self.y -= o.y;
    }
}

/// Wrap an angle into [-PI, PI].
pub fn wrap_angle(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let mut a = a % tau;
    if a > std::f32::consts::PI {
        a -= tau;
    } else if a < -std::f32::consts::PI {
        a += tau;
    }
    a
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// xorshift64* — small, fast and good enough for gameplay randomness.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }

    /// Uniform in [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }

    /// Uniform integer in [lo, hi] inclusive.
    pub fn int(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next_u32() % ((hi - lo + 1) as u32)) as i32
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }
}

/// Stateless hash noise used by the procedural texture generators.
pub fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0x00ff_ffff) as f32 / 0x0100_0000 as f32
}

/// Tileable value noise: the lattice wraps every `period` cells.
pub fn value_noise(x: f32, y: f32, period: i32, seed: u32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let fx = x - xi as f32;
    let fy = y - yi as f32;
    let w = |v: i32| v.rem_euclid(period);
    let a = hash2(w(xi), w(yi), seed);
    let b = hash2(w(xi + 1), w(yi), seed);
    let c = hash2(w(xi), w(yi + 1), seed);
    let d = hash2(w(xi + 1), w(yi + 1), seed);
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    lerp(lerp(a, b, sx), lerp(c, d, sx), sy)
}

/// Fractal noise over a tile of `size` pixels; `base` lattice cells span the tile.
pub fn fbm(x: f32, y: f32, size: f32, base: i32, octaves: u32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut norm = 0.0;
    let mut period = base;
    for o in 0..octaves {
        let s = period as f32 / size;
        sum += amp * value_noise(x * s, y * s, period, seed.wrapping_add(o * 7919));
        norm += amp;
        amp *= 0.5;
        period *= 2;
    }
    sum / norm
}
