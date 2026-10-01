//! Procedural textures. The museum ships with no image files: every surface
//! texture, normal map and the sky are generated here at startup.

use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    math::Vec3,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension},
};

// ---------------------------------------------------------------------------
// Noise
// ---------------------------------------------------------------------------

fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h =
        (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0x00ff_ffff) as f32 / 16_777_215.0
}

fn hash3(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    hash2(x ^ z.wrapping_mul(0x2c1b_3c6d_u32 as i32), y.wrapping_add(z), seed)
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Value noise that tiles with integer `period` (in lattice cells).
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

/// Tileable fractal noise in `[0, 1]`. `(u, v)` are in `[0, 1)` texture space.
pub fn fbm2(u: f32, v: f32, base: i32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut freq) = (0.0, 0.5, 0.0, base);
    for o in 0..octaves {
        sum += amp * noise2(u * freq as f32, v * freq as f32, freq, seed.wrapping_add(o * 17));
        norm += amp;
        amp *= 0.5;
        freq *= 2;
    }
    sum / norm
}

pub fn noise3(p: Vec3, seed: u32) -> f32 {
    let i = p.floor();
    let f = p - i;
    let (x, y, z) = (i.x as i32, i.y as i32, i.z as i32);
    let (u, v, w) = (smooth(f.x), smooth(f.y), smooth(f.z));
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let c = |dx, dy, dz| hash3(x + dx, y + dy, z + dz, seed);
    l(
        l(l(c(0, 0, 0), c(1, 0, 0), u), l(c(0, 1, 0), c(1, 1, 0), u), v),
        l(l(c(0, 0, 1), c(1, 0, 1), u), l(c(0, 1, 1), c(1, 1, 1), u), v),
        w,
    )
}

pub fn fbm3(p: Vec3, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut q) = (0.0, 0.5, 0.0, p);
    for o in 0..octaves {
        sum += amp * noise3(q, seed.wrapping_add(o * 31));
        norm += amp;
        amp *= 0.5;
        q *= 2.03;
    }
    sum / norm
}

// ---------------------------------------------------------------------------
// Image helpers
// ---------------------------------------------------------------------------

fn to_u8(c: Vec3) -> [u8; 3] {
    let f = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    [f(c.x), f(c.y), f(c.z)]
}

/// Builds a full mip chain with a box filter. For normal maps the averaged
/// vectors are re-normalized.
fn with_mips(w: u32, h: u32, base: Vec<u8>, normal_map: bool) -> (Vec<u8>, u32) {
    let mut data = base.clone();
    let mut level = base;
    let (mut lw, mut lh) = (w, h);
    let mut count = 1;
    while lw > 1 || lh > 1 {
        let (nw, nh) = ((lw / 2).max(1), (lh / 2).max(1));
        let mut next = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                let mut acc = [0u32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let sx = (x * 2 + dx).min(lw - 1);
                    let sy = (y * 2 + dy).min(lh - 1);
                    let i = ((sy * lw + sx) * 4) as usize;
                    for c in 0..4 {
                        acc[c] += level[i + c] as u32;
                    }
                }
                let o = ((y * nw + x) * 4) as usize;
                if normal_map {
                    let n = Vec3::new(
                        acc[0] as f32 / 4.0 / 127.5 - 1.0,
                        acc[1] as f32 / 4.0 / 127.5 - 1.0,
                        acc[2] as f32 / 4.0 / 127.5 - 1.0,
                    )
                    .normalize_or(Vec3::Z);
                    let b = to_u8(n * 0.5 + 0.5);
                    next[o..o + 3].copy_from_slice(&b);
                    next[o + 3] = 255;
                } else {
                    for c in 0..4 {
                        next[o + c] = ((acc[c] + 2) / 4) as u8;
                    }
                }
            }
        }
        data.extend_from_slice(&next);
        level = next;
        lw = nw;
        lh = nh;
        count += 1;
    }
    (data, count)
}

pub struct TexOpts {
    pub srgb: bool,
    pub repeat: bool,
    pub normal_map: bool,
}

pub const COLOR_TILE: TexOpts = TexOpts {
    srgb: true,
    repeat: true,
    normal_map: false,
};
pub const COLOR_CLAMP: TexOpts = TexOpts {
    srgb: true,
    repeat: false,
    normal_map: false,
};
pub const NORMAL_TILE: TexOpts = TexOpts {
    srgb: false,
    repeat: true,
    normal_map: true,
};

/// Generates an RGBA8 image by evaluating `f(u, v)` for every texel. `u`/`v`
/// are texel centers in `[0, 1)`. Returns linear RGB in `[0, 1]` plus alpha.
pub fn generate(w: u32, h: u32, opts: TexOpts, f: impl Fn(f32, f32) -> Vec4) -> Image {
    let mut base = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let c = f((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
            let i = ((y * w + x) * 4) as usize;
            let rgb = if opts.srgb {
                let s = Color::linear_rgb(c.x.max(0.0), c.y.max(0.0), c.z.max(0.0)).to_srgba();
                Vec3::new(s.red, s.green, s.blue)
            } else {
                c.truncate()
            };
            base[i..i + 3].copy_from_slice(&to_u8(rgb));
            base[i + 3] = (c.w.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        }
    }
    let (data, mips) = with_mips(w, h, base, opts.normal_map);
    let format = if opts.srgb {
        TextureFormat::Rgba8UnormSrgb
    } else {
        TextureFormat::Rgba8Unorm
    };
    let mut image = Image::new_uninit(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(data);
    image.texture_descriptor.mip_level_count = mips;
    let mode = if opts.repeat {
        ImageAddressMode::Repeat
    } else {
        ImageAddressMode::ClampToEdge
    };
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: mode,
        address_mode_v: mode,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 16,
        ..default()
    });
    image
}

/// Turns a tileable height function into a tangent-space normal map.
pub fn normal_from_height(w: u32, h: u32, strength: f32, height: impl Fn(f32, f32) -> f32) -> Image {
    let du = 1.0 / w as f32;
    let dv = 1.0 / h as f32;
    generate(w, h, NORMAL_TILE, |u, v| {
        let hx = height((u + du).fract(), v) - height((u - du + 1.0).fract(), v);
        let hy = height(u, (v + dv).fract()) - height(u, (v - dv + 1.0).fract());
        // Texture v runs down the image; tangent-space +Y points up the image.
        let n = Vec3::new(-hx * strength, hy * strength, 1.0).normalize();
        (n * 0.5 + 0.5).extend(1.0)
    })
}

fn mix(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn srgb(r: u8, g: u8, b: u8) -> Vec3 {
    let c = Color::srgb_u8(r, g, b).to_linear();
    Vec3::new(c.red, c.green, c.blue)
}

// ---------------------------------------------------------------------------
// Surfaces
// ---------------------------------------------------------------------------

/// Veins for a marble slab; `t` is a 0..1 texture coordinate pair.
fn marble_veins(u: f32, v: f32, seed: u32) -> f32 {
    // Ridged noise gives thin, branching veins rather than contour lines.
    let warp = fbm2(u, v, 2, 4, seed + 1) * 0.35;
    let n = fbm2((u + warp).fract(), (v + warp * 0.6).fract(), 3, 6, seed);
    let ridge = 1.0 - (n * 2.0 - 1.0).abs();
    let fine = fbm2(u, v, 8, 4, seed + 9);
    let fine_ridge = 1.0 - (fine * 2.0 - 1.0).abs();
    (ridge.powf(14.0) * 0.9 + fine_ridge.powf(24.0) * 0.4).clamp(0.0, 1.0)
}

/// 2x2 checkerboard of white and grey marble tiles with grout lines. One
/// texture repeat covers 2 m x 2 m.
pub fn marble_floor() -> (Image, Image) {
    let size = 1024;
    let grout = |u: f32, v: f32| {
        let gu = ((u * 2.0).fract() - 0.5).abs();
        let gv = ((v * 2.0).fract() - 0.5).abs();
        let d = 0.5 - gu.max(gv);
        (d / 0.004).clamp(0.0, 1.0)
    };
    let albedo = generate(size, size, COLOR_TILE, |u, v| {
        let dark = ((u * 2.0) as i32 + (v * 2.0) as i32) % 2 == 1;
        let veins = marble_veins(u, v, if dark { 7 } else { 3 });
        let cloud = fbm2(u, v, 4, 5, 21);
        let col = if dark {
            let base = mix(srgb(122, 116, 108), srgb(146, 140, 130), cloud);
            mix(base, srgb(220, 214, 204), veins * 0.8)
        } else {
            let base = mix(srgb(226, 222, 212), srgb(240, 237, 230), cloud);
            mix(base, srgb(120, 118, 120), veins * 0.7)
        };
        mix(srgb(60, 58, 55), col, grout(u, v)).extend(1.0)
    });
    let normal = normal_from_height(512, 512, 1.5, |u, v| grout(u, v));
    (albedo, normal)
}

/// The rotunda floor: a disc of inlaid marble with a compass star at its
/// center. UVs map the square [0,1]^2 onto the octagon's bounding square.
pub fn rotunda_floor() -> Image {
    let size = 2048;
    let white = srgb(232, 228, 218);
    let grey = srgb(84, 88, 92);
    let red = srgb(128, 44, 38);
    let gold = srgb(196, 150, 70);
    let green = srgb(38, 78, 64);
    generate(size, size, COLOR_CLAMP, |u, v| {
        let p = Vec2::new(u - 0.5, v - 0.5) * 36.0; // meters
        let r = p.length();
        let a = p.y.atan2(p.x);
        let veins = marble_veins(u * 3.0 % 1.0, v * 3.0 % 1.0, 11);
        let cloud = fbm2(u, v, 8, 4, 5);
        let mut col = mix(white, srgb(244, 241, 234), cloud);
        col = mix(col, srgb(150, 146, 140), veins * 0.5);

        // Outer field: large radial tiles
        let ring = |r0: f32, r1: f32| r > r0 && r < r1;
        if r > 5.5 {
            let seg = (a / std::f32::consts::TAU * 32.0 + 32.0) as i32;
            let band = ((r - 5.5) / 2.1) as i32;
            if (seg + band) % 2 == 0 {
                col = mix(col, srgb(214, 208, 196), 0.6);
            }
            // tile joints
            let fa = (a / std::f32::consts::TAU * 32.0).fract();
            let fr = ((r - 5.5) / 2.1).fract();
            let joint = (fa.min(1.0 - fa) * r * 0.196).min(fr.min(1.0 - fr) * 2.1);
            if joint < 0.012 {
                col = srgb(120, 116, 110);
            }
        }
        // Concentric inlay bands
        if ring(4.9, 5.5) {
            col = mix(grey, srgb(110, 114, 118), cloud);
        }
        if ring(5.05, 5.12) || ring(5.28, 5.35) {
            col = gold;
        }
        if ring(13.2, 13.9) {
            col = mix(red, srgb(150, 60, 50), veins);
        }
        if ring(13.45, 13.65) {
            col = gold;
        }
        // Compass star (16 points) inside the pool ring is hidden by the pool,
        // so the star lives in a band just outside it.
        if r > 5.5 && r < 12.8 {
            // 16 rays: 8 long ones aligned with the gallery axes, 8 short.
            let s = a / std::f32::consts::TAU * 16.0 + 16.5;
            let ray = s.floor() as i32 % 16;
            let off = s.fract() - 0.5; // -0.5..0.5 across one ray sector
            let reach = if ray % 2 == 0 { 12.6 } else { 9.8 };
            let width = (1.0 - (r - 5.5) / (reach - 5.5)).max(0.0) * 0.5;
            if off.abs() < width {
                col = if off > 0.0 {
                    mix(green, srgb(60, 100, 84), veins)
                } else {
                    mix(grey, srgb(40, 44, 48), cloud)
                };
                if (off.abs() - width).abs() * r < 0.04 {
                    col = gold;
                }
            }
        }
        col.extend(1.0)
    })
}

/// Soft plaster. Neutral so walls can be tinted with `base_color`.
pub fn plaster() -> (Image, Image) {
    let albedo = generate(512, 512, COLOR_TILE, |u, v| {
        let n = fbm2(u, v, 4, 6, 41);
        Vec3::splat(0.86 + (n - 0.5) * 0.12).extend(1.0)
    });
    let normal = normal_from_height(512, 512, 2.0, |u, v| fbm2(u, v, 16, 3, 43));
    (albedo, normal)
}

/// Wood grain, running along U. One repeat is ~1 m.
pub fn wood() -> Image {
    generate(512, 512, COLOR_TILE, |u, v| {
        let warp = fbm2(u, v, 2, 4, 51) * 2.5;
        let rings = ((v * 18.0 + warp * 4.0) * std::f32::consts::TAU).sin() * 0.5 + 0.5;
        let fine = fbm2(u, v, 32, 2, 53);
        let dark = srgb(92, 54, 30);
        let light = srgb(160, 106, 62);
        mix(light, dark, rings.powf(2.5) * 0.7 + fine * 0.3).extend(1.0)
    })
}

/// A single Pantheon-style coffer: stepped recess with a gilded rosette.
/// Returns (albedo, normal). The dome UVs repeat this per coffer.
pub fn coffer() -> (Image, Image) {
    let height = |u: f32, v: f32| {
        let d = (u - 0.5).abs().max((v - 0.5).abs()); // 0 center .. 0.5 edge
        let step = |e: f32, w: f32| ((e - d) / w).clamp(0.0, 1.0);
        let mut h = 1.0;
        h -= 0.35 * step(0.42, 0.015);
        h -= 0.3 * step(0.34, 0.015);
        h -= 0.2 * step(0.26, 0.015);
        let r = Vec2::new(u - 0.5, v - 0.5).length();
        h += 0.25 * (1.0 - (r / 0.07)).clamp(0.0, 1.0).powf(0.5);
        h
    };
    let albedo = generate(256, 256, COLOR_TILE, |u, v| {
        let d = (u - 0.5).abs().max((v - 0.5).abs());
        let r = Vec2::new(u - 0.5, v - 0.5).length();
        let n = fbm2(u, v, 8, 3, 61);
        let stone = srgb(214, 206, 190) * (0.92 + n * 0.1);
        let recess = srgb(170, 176, 190) * (0.92 + n * 0.1);
        let mut c = if d < 0.26 {
            recess
        } else if d < 0.34 {
            stone * 0.9
        } else {
            stone
        };
        if r < 0.07 {
            c = srgb(212, 170, 80);
        }
        c.extend(1.0)
    });
    (albedo, normal_from_height(256, 256, 6.0, height))
}

/// Rough cave rock with red-ochre hand stencils and animals, for the cave art
/// exhibit. Aspect 2:1.
pub fn cave_painting() -> Image {
    let (w, h) = (1024, 512);
    // Hand stencils: (center, scale, rotation)
    let hands = [
        (Vec2::new(0.12, 0.30), 0.075, 0.2),
        (Vec2::new(0.22, 0.22), 0.07, -0.3),
        (Vec2::new(0.86, 0.28), 0.08, 0.1),
        (Vec2::new(0.78, 0.70), 0.065, -0.5),
        (Vec2::new(0.10, 0.72), 0.07, 0.4),
    ];
    let hand = |p: Vec2| -> f32 {
        // signed-ish "inside" test for a stylized hand in local space
        let palm = (p - Vec2::new(0.0, 0.15)) / Vec2::new(0.42, 0.5);
        if palm.length() < 1.0 {
            return 1.0;
        }
        let fingers = [
            (-0.36, 0.55, 0.32, 0.9),
            (-0.14, 0.9, 0.45, 0.0),
            (0.06, 0.95, 0.48, 0.0),
            (0.25, 0.85, 0.42, 0.0),
            (0.4, 0.62, 0.3, -0.2),
        ];
        for (fx, fy, len, tilt) in fingers {
            let base = Vec2::new(fx, fy - len * 0.5);
            let dir = Vec2::new((tilt as f32).sin(), (tilt as f32).cos());
            let q = p - base;
            let t = q.dot(dir).clamp(0.0, len);
            if (q - dir * t).length() < 0.085 {
                return 1.0;
            }
        }
        0.0
    };
    // A bison-ish animal made of ellipses: (center, radii)
    let animal = |p: Vec2| -> f32 {
        let parts = [
            (Vec2::new(0.0, 0.0), Vec2::new(0.55, 0.28)),   // body
            (Vec2::new(0.32, 0.12), Vec2::new(0.28, 0.26)), // hump
            (Vec2::new(-0.62, 0.02), Vec2::new(0.2, 0.15)), // head
            (Vec2::new(-0.35, -0.35), Vec2::new(0.07, 0.22)),
            (Vec2::new(-0.15, -0.38), Vec2::new(0.07, 0.22)),
            (Vec2::new(0.25, -0.36), Vec2::new(0.07, 0.22)),
            (Vec2::new(0.42, -0.34), Vec2::new(0.07, 0.22)),
        ];
        let mut inside: f32 = 0.0;
        for (c, r) in parts {
            let d = ((p - c) / r).length();
            inside = inside.max((1.0 - d) * 8.0);
        }
        inside.clamp(0.0, 1.0)
    };
    generate(w, h, COLOR_CLAMP, |u, v| {
        let p = Vec2::new(u * 2.0, 1.0 - v);
        let n = fbm2(u * 2.0 % 1.0, v, 6, 6, 71);
        let n2 = fbm2(u, v, 24, 3, 73);
        let mut col = mix(srgb(150, 118, 88), srgb(196, 164, 124), n) * (0.85 + n2 * 0.25);
        let ochre = srgb(150, 48, 26);
        let charcoal = srgb(34, 26, 22);
        // Stencils: pigment sprayed around a hand leaves a halo.
        for (c, s, rot) in hands {
            let q = (Vec2::new(u, 1.0 - v) - c) * Vec2::new(2.0, 1.0);
            let q = Vec2::from_angle(-rot).rotate(q) / s;
            let d = q.length();
            let spray = (1.0 - (d / 1.6)).clamp(0.0, 1.0).powf(0.7) * (0.6 + 0.4 * n2);
            if hand(q) < 0.5 {
                col = mix(col, ochre, spray * 0.85);
            }
        }
        // Two animals, one in charcoal outline + ochre fill.
        let a1 = animal((p - Vec2::new(0.95, 0.55)) / 0.28);
        col = mix(col, ochre * 1.1, a1 * 0.8 * (0.7 + 0.3 * n));
        let a2 = animal((p - Vec2::new(1.45, 0.35)) / Vec2::new(-0.22, 0.22));
        col = mix(col, charcoal, a2 * 0.85);
        // Dotted line of ochre dots
        let dots = Vec2::new(1.2 + (p.x * 20.0).round() / 20.0 - 1.2, 0.83);
        if p.x > 0.7 && p.x < 1.6 && (p - dots).length() < 0.012 {
            col = ochre;
        }
        col.extend(1.0)
    })
}

/// Agar plate with a Penicillium colony and a clear halo of dead bacteria.
pub fn petri_dish() -> Image {
    generate(512, 512, COLOR_CLAMP, |u, v| {
        let p = Vec2::new(u - 0.5, v - 0.5) * 2.0;
        let r = p.length();
        let agar = srgb(214, 190, 120);
        let mut col = agar;
        let mold_c = Vec2::new(-0.25, -0.2);
        let dm = (p - mold_c).length() + (fbm2(u, v, 8, 3, 81) - 0.5) * 0.12;
        // bacterial colonies everywhere except near the mold
        let cell = (p * 11.0).floor();
        let jitter = Vec2::new(
            hash2(cell.x as i32, cell.y as i32, 3),
            hash2(cell.x as i32, cell.y as i32, 4),
        );
        let cp = (cell + 0.2 + jitter * 0.6) / 11.0;
        let dcol = (p - cp).length();
        let colony_r = 0.02 + hash2(cell.x as i32, cell.y as i32, 5) * 0.025;
        if dm > 0.55 && dcol < colony_r {
            col = srgb(236, 226, 196);
        }
        if dm < 0.3 {
            let ring = (dm / 0.3).powf(2.0);
            col = mix(srgb(40, 120, 90), srgb(236, 236, 226), ring);
        }
        if r > 0.97 {
            col = srgb(200, 200, 196);
        }
        col.extend(1.0)
    })
}

/// A stylized globe: lapis oceans and gilded landmasses generated from 3D
/// noise (so it wraps seamlessly on a UV sphere).
pub fn globe(gilded: bool) -> Image {
    generate(1024, 512, COLOR_TILE, |u, v| {
        let lon = u * std::f32::consts::TAU;
        let lat = (0.5 - v) * std::f32::consts::PI;
        let d = Vec3::new(lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin());
        let n = fbm3(d * 1.8 + 4.0, 6, 91);
        let land = n > 0.53;
        let ice = lat.abs() > 1.25;
        if gilded {
            if land || ice {
                mix(srgb(186, 138, 56), srgb(236, 196, 110), fbm3(d * 8.0, 3, 93)).extend(1.0)
            } else {
                mix(srgb(18, 38, 96), srgb(34, 64, 140), n * 1.4).extend(1.0)
            }
        } else if ice {
            srgb(236, 240, 245).extend(1.0)
        } else if land {
            let dry = fbm3(d * 4.0, 3, 95);
            mix(srgb(52, 104, 46), srgb(170, 146, 96), dry * 1.3 - 0.2).extend(1.0)
        } else {
            mix(srgb(10, 36, 90), srgb(24, 74, 140), n * 1.5).extend(1.0)
        }
    })
}

// ---------------------------------------------------------------------------
// Cubemaps
// ---------------------------------------------------------------------------

fn f16_bits(v: f32) -> u16 {
    let bits = v.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    let mant = bits & 0x7f_ffff;
    if v.is_nan() {
        return 0x7e00;
    }
    if exp <= 0 {
        return sign; // flush tiny values to zero
    }
    if exp >= 31 {
        return sign | 0x7c00;
    }
    sign | ((exp as u16) << 10) | ((mant >> 13) as u16)
}

/// Direction for a texel on a cubemap face, following the wgpu face order
/// (+X, -X, +Y, -Y, +Z, -Z).
fn cube_dir(face: usize, u: f32, v: f32) -> Vec3 {
    let (a, b) = (u * 2.0 - 1.0, v * 2.0 - 1.0);
    match face {
        0 => Vec3::new(1.0, -b, -a),
        1 => Vec3::new(-1.0, -b, a),
        2 => Vec3::new(a, 1.0, b),
        3 => Vec3::new(a, -1.0, -b),
        4 => Vec3::new(a, -b, 1.0),
        _ => Vec3::new(-a, -b, -1.0),
    }
    .normalize()
}

pub fn cubemap(size: u32, f: impl Fn(Vec3) -> Vec3) -> Image {
    let mut data = Vec::with_capacity((size * size * 6 * 8) as usize);
    for face in 0..6 {
        for y in 0..size {
            for x in 0..size {
                let d = cube_dir(face, (x as f32 + 0.5) / size as f32, (y as f32 + 0.5) / size as f32);
                let c = f(d);
                for ch in [c.x, c.y, c.z, 1.0] {
                    data.extend_from_slice(&f16_bits(ch).to_le_bytes());
                }
            }
        }
    }
    Image {
        texture_view_descriptor: Some(TextureViewDescriptor {
            dimension: Some(TextureViewDimension::Cube),
            ..default()
        }),
        ..Image::new(
            Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 6,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba16Float,
            RenderAssetUsages::RENDER_WORLD,
        )
    }
}

/// A clear afternoon sky with a few soft clouds, seen through the oculus.
pub fn sky(sun_dir: Vec3) -> Image {
    cubemap(256, |d| {
        let h = d.y.max(0.0);
        let zenith = Vec3::new(0.10, 0.28, 0.75);
        let horizon = Vec3::new(0.62, 0.76, 0.95);
        let mut c = mix(horizon, zenith, h.powf(0.45));
        if d.y < 0.0 {
            c = mix(horizon, Vec3::new(0.3, 0.3, 0.32), (-d.y * 4.0).min(1.0));
        }
        let cloud = fbm3(d * 3.5 + Vec3::new(3.0, 0.0, 1.0), 5, 17);
        let cover = ((cloud - 0.52) * 4.0).clamp(0.0, 1.0) * (h * 3.0).min(1.0);
        c = mix(c, Vec3::splat(1.05), cover * 0.85);
        let s = d.dot(sun_dir).max(0.0);
        c += Vec3::new(1.0, 0.9, 0.7) * (s.powf(400.0) * 40.0 + s.powf(12.0) * 0.4);
        c
    })
}

/// A stand-in for the museum interior used for reflections on metal and
/// glass: warm walls, dark floor, and bright ceiling light strips.
pub fn interior() -> Image {
    cubemap(128, |d| {
        let wall = Vec3::new(0.55, 0.5, 0.44);
        let floor = Vec3::new(0.28, 0.26, 0.24);
        let ceil = Vec3::new(0.7, 0.68, 0.64);
        let mut c = if d.y > 0.35 {
            ceil
        } else if d.y < -0.3 {
            floor
        } else {
            wall
        };
        // Light strips on the ceiling, and a few bright "windows".
        if d.y > 0.5 && (d.x * 6.0).fract().abs() < 0.12 {
            c = Vec3::splat(6.0);
        }
        if d.y > -0.05 && d.y < 0.25 && (d.x.atan2(d.z) * 3.0).sin() > 0.93 {
            c = Vec3::new(3.0, 2.9, 2.6);
        }
        c
    })
}
