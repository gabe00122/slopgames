//! Small deterministic RNG and value-noise helpers.
//!
//! We avoid external RNG crates so that world generation is stable across
//! dependency upgrades (same seed => same map).

#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    /// Seed from the system clock (used for raid seeds and AI jitter).
    pub fn from_time() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(12345);
        Self::new(nanos)
    }

    /// SplitMix64.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform float in [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform float in [a, b).
    pub fn range_f32(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f32()
    }

    /// Uniform integer in [a, b] (inclusive).
    pub fn range_i32(&mut self, a: i32, b: i32) -> i32 {
        if b <= a {
            return a;
        }
        let span = (b - a + 1) as u64;
        a + (self.next_u64() % span) as i32
    }

    pub fn range_usize(&mut self, a: usize, b_exclusive: usize) -> usize {
        if b_exclusive <= a {
            return a;
        }
        a + (self.next_u64() % (b_exclusive - a) as u64) as usize
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.range_usize(0, items.len())]
    }

    /// Pick an index according to integer weights.
    pub fn weighted(&mut self, weights: &[u32]) -> usize {
        let total: u32 = weights.iter().sum();
        if total == 0 {
            return 0;
        }
        let mut r = (self.next_u64() % total as u64) as u32;
        for (i, w) in weights.iter().enumerate() {
            if r < *w {
                return i;
            }
            r -= *w;
        }
        weights.len() - 1
    }

    /// Approximately normal distributed value (sum of uniforms), mean 0, std ~1.
    pub fn gaussian(&mut self) -> f32 {
        let mut s = 0.0;
        for _ in 0..4 {
            s += self.f32();
        }
        (s - 2.0) * 1.732
    }
}

/// Integer hash used for noise and procedural textures.
pub fn hash2(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = (x as u32)
        .wrapping_mul(374_761_393)
        .wrapping_add((y as u32).wrapping_mul(668_265_263))
        .wrapping_add(seed.wrapping_mul(2_246_822_519));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^ (h >> 16)
}

pub fn hash3(x: i32, y: i32, z: i32, seed: u32) -> u32 {
    hash2(x, hash2(y, z, seed) as i32, seed ^ 0xA5A5_A5A5)
}

/// Hash to float in [0, 1).
pub fn hash2f(x: i32, y: i32, seed: u32) -> f32 {
    (hash2(x, y, seed) & 0x00FF_FFFF) as f32 / 16_777_216.0
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// 2D value noise in [0, 1).
pub fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let x0 = x.floor();
    let y0 = y.floor();
    let tx = smooth(x - x0);
    let ty = smooth(y - y0);
    let (ix, iy) = (x0 as i32, y0 as i32);
    let a = hash2f(ix, iy, seed);
    let b = hash2f(ix + 1, iy, seed);
    let c = hash2f(ix, iy + 1, seed);
    let d = hash2f(ix + 1, iy + 1, seed);
    let ab = a + (b - a) * tx;
    let cd = c + (d - c) * tx;
    ab + (cd - ab) * ty
}

/// Fractal Brownian motion over value noise, result roughly in [0, 1).
pub fn fbm(x: f32, y: f32, octaves: u32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for i in 0..octaves {
        sum += value_noise(x * freq, y * freq, seed.wrapping_add(i * 101)) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn ranges_are_respected() {
        let mut r = Rng::new(7);
        for _ in 0..1000 {
            let v = r.range_i32(-3, 5);
            assert!((-3..=5).contains(&v));
            let f = r.f32();
            assert!((0.0..1.0).contains(&f));
        }
    }
}
