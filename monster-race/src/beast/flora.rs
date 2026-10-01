//! Plants, rocks and sea life that grow on the beasts. Each builder adds one
//! specimen at the origin of the builder's current transform, standing on +Y.

use crate::{
    meshkit::{Col, MeshBuilder},
    util::{Rng, lin, mix, shade},
};
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

/// Leaning palm with drooping fronds and a few coconuts.
pub fn palm(b: &mut MeshBuilder, height: f32, rng: &mut Rng) {
    let lean = Vec3::new(rng.sym(1.0), 0.0, rng.sym(1.0)).normalize_or(Vec3::X) * rng.range(0.1, 0.32) * height;
    let trunk = lin(0x9a7448);
    let segs = 5;
    let mut path = Vec::new();
    for i in 0..=segs {
        let f = i as f32 / segs as f32;
        let p = Vec3::new(lean.x * f * f, height * f, lean.z * f * f);
        path.push((p, height * (0.055 - 0.025 * f)));
    }
    b.tube(&path, 5, trunk, false);
    let top = path[segs].0;
    let fronds = 7;
    let a0 = rng.angle();
    for k in 0..fronds {
        let a = a0 + k as f32 / fronds as f32 * TAU + rng.sym(0.25);
        let dir = Vec3::new(a.cos(), 0.0, a.sin());
        let side = Vec3::new(-a.sin(), 0.0, a.cos());
        let len = height * rng.range(0.42, 0.56);
        let w = len * 0.2;
        let green = mix(lin(0x3fa34d), lin(0x8fd14f), rng.f());
        // Three segments arcing up, out, then down.
        let pts = [
            top,
            top + dir * len * 0.4 + Vec3::Y * len * 0.22,
            top + dir * len * 0.78 + Vec3::Y * len * 0.08,
            top + dir * len - Vec3::Y * len * 0.3,
        ];
        let widths = [w * 0.35, w, w * 0.8, 0.0];
        for s in 0..3 {
            let c = shade(green, 1.0 - s as f32 * 0.12);
            b.quad_2(
                [
                    pts[s] - side * widths[s],
                    pts[s] + side * widths[s],
                    pts[s + 1] + side * widths[s + 1],
                    pts[s + 1] - side * widths[s + 1],
                ],
                c,
            );
        }
    }
    for _ in 0..rng.below(4) {
        let a = rng.angle();
        b.ball(
            top + Vec3::new(a.cos(), -0.6, a.sin()) * height * 0.06,
            height * 0.04,
            0,
            lin(0x6b4a2a),
        );
    }
}

/// Round-crowned tree.
pub fn broadleaf(b: &mut MeshBuilder, height: f32, leaf: Col, rng: &mut Rng) {
    let trunk = lin(0x6e4b2f);
    let h = height * 0.5;
    b.cyl(Vec3::ZERO, Vec3::Y * h, height * 0.075, height * 0.05, 5, trunk, false);
    let blobs = 3 + rng.below(3);
    for k in 0..blobs {
        let a = rng.angle();
        let r = height * rng.range(0.2, 0.3);
        let off = if k == 0 {
            Vec3::ZERO
        } else {
            Vec3::new(a.cos(), rng.sym(0.5), a.sin()) * height * 0.2
        };
        let c = shade(leaf, rng.range(0.8, 1.15));
        b.ico_with(Vec3::Y * (h + height * 0.22) + off, Vec3::new(r, r * 0.85, r), 1, |d| {
            (1.0, shade(c, 0.82 + d.y * 0.22))
        });
    }
}

/// Tiered conifer.
pub fn conifer(b: &mut MeshBuilder, height: f32, rng: &mut Rng) {
    let trunk = lin(0x5d4030);
    b.cyl(
        Vec3::ZERO,
        Vec3::Y * height * 0.3,
        height * 0.05,
        height * 0.04,
        5,
        trunk,
        false,
    );
    let tiers = 3;
    let green = mix(lin(0x1f6b4a), lin(0x3d8f55), rng.f());
    for k in 0..tiers {
        let f = k as f32 / tiers as f32;
        let y0 = height * (0.2 + 0.26 * k as f32);
        let r = height * (0.24 - 0.06 * k as f32);
        b.cone(
            Vec3::Y * y0,
            Vec3::Y * (y0 + height * 0.36),
            r,
            7,
            shade(green, 0.85 + f * 0.35),
        );
    }
}

/// Toadstool with a spotted cap.
pub fn mushroom(b: &mut MeshBuilder, height: f32, cap: Col, rng: &mut Rng) {
    let stem = lin(0xf1e6cf);
    let r = height * rng.range(0.45, 0.62);
    let sr = height * 0.12;
    b.lathe(
        &[
            (sr * 1.3, 0.0, shade(stem, 0.8)),
            (sr, height * 0.3, stem),
            (sr * 0.85, height * 0.72, stem),
            (r, height * 0.7, shade(cap, 0.7)),
            (r * 0.95, height * 0.78, cap),
            (r * 0.62, height * 0.95, cap),
            (r * 0.2, height * 1.03, shade(cap, 1.1)),
        ],
        8,
    );
    for _ in 0..5 {
        let a = rng.angle();
        let f = rng.range(0.25, 0.8);
        let y = height * (0.79 + (1.0 - f) * 0.22);
        b.ball(
            Vec3::new(a.cos() * r * f, y, a.sin() * r * f),
            height * 0.06,
            0,
            lin(0xfff6e0),
        );
    }
}

/// Rosette of fronds.
pub fn fern(b: &mut MeshBuilder, size: f32, green: Col, rng: &mut Rng) {
    let n = 6;
    let a0 = rng.angle();
    for k in 0..n {
        let a = a0 + k as f32 / n as f32 * TAU;
        let dir = Vec3::new(a.cos(), 0.0, a.sin());
        let side = Vec3::new(-a.sin(), 0.0, a.cos());
        let c = shade(green, rng.range(0.8, 1.15));
        let mid = dir * size * 0.55 + Vec3::Y * size * 0.5;
        let tip = dir * size + Vec3::Y * size * 0.2;
        b.tri_2([Vec3::ZERO, mid + side * size * 0.2, mid - side * size * 0.2], c);
        b.tri_2([mid + side * size * 0.2, tip, mid - side * size * 0.2], shade(c, 0.85));
    }
}

/// Low leafy shrub.
pub fn bush(b: &mut MeshBuilder, size: f32, green: Col, rng: &mut Rng) {
    for _ in 0..3 {
        let a = rng.angle();
        let r = size * rng.range(0.45, 0.7);
        let c = shade(green, rng.range(0.8, 1.15));
        b.ico_with(
            Vec3::new(a.cos() * size * 0.35, r * 0.6, a.sin() * size * 0.35),
            Vec3::new(r, r * 0.75, r),
            0,
            |d| (1.0, shade(c, 0.85 + d.y * 0.2)),
        );
    }
}

/// A few blades.
pub fn tuft(b: &mut MeshBuilder, size: f32, green: Col, rng: &mut Rng) {
    for _ in 0..4 {
        let a = rng.angle();
        let dir = Vec3::new(a.cos(), 0.0, a.sin());
        let side = Vec3::new(-a.sin(), 0.0, a.cos()) * size * 0.16;
        let base = dir * size * 0.15;
        let tip = dir * size * rng.range(0.4, 0.7) + Vec3::Y * size * rng.range(0.7, 1.1);
        b.tri_2([base - side, base + side, tip], shade(green, rng.range(0.8, 1.2)));
    }
}

/// Flower on a stalk.
pub fn flower(b: &mut MeshBuilder, size: f32, petal: Col, rng: &mut Rng) {
    let stalk = lin(0x4f9e3c);
    let top = Vec3::new(rng.sym(0.2), 1.0, rng.sym(0.2)) * size;
    b.cyl(Vec3::ZERO, top, size * 0.05, size * 0.04, 4, stalk, false);
    let n = 6;
    for k in 0..n {
        let a = k as f32 / n as f32 * TAU;
        let dir = Vec3::new(a.cos(), 0.25, a.sin());
        let side = Vec3::new(-a.sin(), 0.0, a.cos()) * size * 0.16;
        b.tri_2(
            [top, top + dir * size * 0.42 + side, top + dir * size * 0.42 - side],
            petal,
        );
    }
    b.ball(top + Vec3::Y * size * 0.05, size * 0.11, 0, lin(0xffd23f));
}

/// Cluster of volcano-shaped barnacles.
pub fn barnacles(b: &mut MeshBuilder, size: f32, rng: &mut Rng) {
    let shell = lin(0xd9d2c2);
    for k in 0..(3 + rng.below(4)) {
        let a = rng.angle();
        let off = if k == 0 {
            Vec3::ZERO
        } else {
            Vec3::new(a.cos(), 0.0, a.sin()) * size * rng.range(0.6, 1.3)
        };
        let s = size * if k == 0 { 1.0 } else { rng.range(0.4, 0.75) };
        let c = shade(shell, rng.range(0.8, 1.05));
        let saved = b.xf;
        b.xf = saved * bevy::math::Affine3A::from_translation(off);
        b.lathe(
            &[
                (s * 0.75, -0.2 * s, shade(c, 0.75)),
                (s * 0.6, s * 0.35, c),
                (s * 0.34, s * 0.8, shade(c, 1.1)),
                (s * 0.22, s * 0.66, lin(0x3b342f)),
                (0.0, s * 0.5, lin(0x2a2522)),
            ],
            7,
        );
        b.xf = saved;
    }
}

/// Branching coral.
pub fn coral(b: &mut MeshBuilder, size: f32, c: Col, rng: &mut Rng) {
    fn branch(b: &mut MeshBuilder, rng: &mut Rng, from: Vec3, dir: Vec3, len: f32, r: f32, depth: u32, c: Col) {
        let to = from + dir * len;
        b.cyl(from, to, r, r * 0.7, 5, shade(c, 0.8 + depth as f32 * 0.12), depth == 2);
        if depth >= 2 {
            b.ball(to, r * 0.95, 0, shade(c, 1.2));
            return;
        }
        for _ in 0..2 {
            let bend = Vec3::new(rng.sym(0.8), rng.range(0.4, 1.0), rng.sym(0.8)).normalize();
            let d = (dir + bend * 0.9).normalize();
            branch(b, rng, to, d, len * 0.72, r * 0.7, depth + 1, c);
        }
    }
    for _ in 0..2 {
        let d = Vec3::new(rng.sym(0.35), 1.0, rng.sym(0.35)).normalize();
        branch(b, rng, Vec3::ZERO, d, size * 0.45, size * 0.1, 0, c);
    }
}

/// Fan coral: a flat lobed sheet on a stalk.
pub fn sea_fan(b: &mut MeshBuilder, size: f32, c: Col, rng: &mut Rng) {
    let a = rng.angle();
    let side = Vec3::new(a.cos(), 0.0, a.sin());
    let n = 7;
    let base = Vec3::Y * size * 0.15;
    b.cyl(Vec3::ZERO, base, size * 0.06, size * 0.05, 4, shade(c, 0.6), false);
    let mut prev = base + side * size * 0.5;
    for k in 1..=n {
        let t = k as f32 / n as f32 * PI;
        let r = size * (0.75 + 0.25 * ((k * 3) as f32).sin());
        let p = base + side * (t.cos() * r * 0.55) + Vec3::Y * (t.sin() * r);
        b.tri_2([base, prev, p], shade(c, 0.85 + 0.3 * (k % 2) as f32));
        prev = p;
    }
}

/// Five-armed starfish lying flat.
pub fn starfish(b: &mut MeshBuilder, size: f32, c: Col, rng: &mut Rng) {
    let a0 = rng.angle();
    let hub = b.v(Vec3::Y * size * 0.25, shade(c, 1.15));
    let mut ring = Vec::new();
    for k in 0..10 {
        let a = a0 + k as f32 / 10.0 * TAU;
        let r = if k % 2 == 0 { size } else { size * 0.38 };
        ring.push(b.v(
            Vec3::new(a.cos() * r, 0.04 * size, a.sin() * r),
            shade(c, if k % 2 == 0 { 0.8 } else { 1.0 }),
        ));
    }
    for k in 0..10 {
        b.tri(hub, ring[(k + 1) % 10], ring[k]);
    }
}

/// Anemone: a stubby column crowned with tentacles.
pub fn anemone(b: &mut MeshBuilder, size: f32, c: Col, rng: &mut Rng) {
    b.cyl(
        Vec3::ZERO,
        Vec3::Y * size * 0.5,
        size * 0.4,
        size * 0.34,
        7,
        shade(c, 0.7),
        true,
    );
    let n = 9;
    for k in 0..n {
        let a = k as f32 / n as f32 * TAU + rng.sym(0.2);
        let dir = Vec3::new(a.cos(), 0.0, a.sin());
        let base = dir * size * 0.26 + Vec3::Y * size * 0.5;
        let tip = dir * size * rng.range(0.5, 0.8) + Vec3::Y * size * rng.range(0.9, 1.3);
        b.cyl(base, tip, size * 0.07, size * 0.03, 4, c, false);
        b.ball(tip, size * 0.06, 0, shade(c, 1.3));
    }
}

/// A great spiral seashell lying on its side.
pub fn conch(b: &mut MeshBuilder, size: f32) {
    let shell = lin(0xf6d9c6);
    let pink = lin(0xf29cae);
    let turns = 3.2;
    let steps = 26;
    let mut path = Vec::new();
    for i in 0..=steps {
        let f = i as f32 / steps as f32;
        let a = f * turns * TAU;
        let r = size * 0.55 * (1.0 - f).powf(1.3);
        let y = size * (0.35 + 1.25 * f);
        path.push((Vec3::new(a.cos() * r, y, a.sin() * r), size * 0.36 * (1.0 - f * 0.92)));
    }
    b.tube(&path, 7, shell, true);
    // The flared lip.
    b.ico_with(
        Vec3::new(size * 0.62, size * 0.32, 0.0),
        Vec3::new(size * 0.5, size * 0.34, size * 0.62),
        1,
        |d| (1.0, mix(shell, pink, (d.y * 0.5 + 0.5).clamp(0.0, 1.0))),
    );
    for k in 0..6 {
        let a = k as f32 / 6.0 * TAU;
        b.cone(
            Vec3::new(a.cos() * size * 0.42, size * 0.75, a.sin() * size * 0.42),
            Vec3::new(a.cos() * size * 0.85, size * 0.95, a.sin() * size * 0.85),
            size * 0.12,
            5,
            shade(shell, 0.92),
        );
    }
}

/// Hanging strand (kelp or vine) drooping from the anchor point.
pub fn strand(b: &mut MeshBuilder, length: f32, c: Col, rng: &mut Rng) {
    let a = rng.angle();
    let side = Vec3::new(a.cos(), 0.0, a.sin()) * length * 0.07;
    let segs = 4;
    let sway = Vec3::new(rng.sym(0.12), 0.0, rng.sym(0.12)) * length;
    let mut prev = Vec3::ZERO;
    for s in 1..=segs {
        let f = s as f32 / segs as f32;
        let p = -Vec3::Y * length * f + sway * (f * PI).sin();
        let w0 = 1.0 - (s - 1) as f32 / segs as f32 * 0.8;
        let w1 = 1.0 - f * 0.8;
        b.quad_2(
            [prev - side * w0, prev + side * w0, p + side * w1, p - side * w1],
            shade(c, 1.1 - f * 0.4),
        );
        prev = p;
    }
}

/// A dorsal plate: a tall spade of bone, broad along local Z and thin along X.
pub fn plate(b: &mut MeshBuilder, height: f32, width: f32, c: Col) {
    let t = width * 0.14;
    let outline = [
        (0.0, 0.5),
        (0.18, 0.62),
        (0.45, 0.6),
        (0.72, 0.42),
        (0.9, 0.2),
        (1.0, 0.0),
    ];
    let rings: Vec<Vec<(Vec3, Col)>> = outline
        .iter()
        .map(|&(h, w)| {
            let (y, w, tx) = (h * height, w * width, t * (1.0 - h * 0.8));
            let col = shade(c, 0.75 + h * 0.45);
            vec![
                (Vec3::new(0.0, y, w), col),
                (Vec3::new(tx, y, 0.0), shade(col, 1.1)),
                (Vec3::new(0.0, y, -w), col),
                (Vec3::new(-tx, y, 0.0), shade(col, 1.1)),
            ]
        })
        .collect();
    b.loft(&rings, true, true);
}

/// Lichen-spotted boulder pile.
pub fn boulders(b: &mut MeshBuilder, size: f32, c: Col, rng: &mut Rng) {
    for k in 0..(1 + rng.below(3)) {
        let a = rng.angle();
        let off = if k == 0 {
            Vec3::ZERO
        } else {
            Vec3::new(a.cos(), 0.0, a.sin()) * size * 0.9
        };
        let s = size * if k == 0 { 1.0 } else { rng.range(0.35, 0.6) };
        b.rock(
            off + Vec3::Y * s * 0.3,
            Vec3::new(
                s * rng.range(0.8, 1.1),
                s * rng.range(0.55, 0.8),
                s * rng.range(0.8, 1.1),
            ),
            c,
            rng,
        );
    }
}

/// A cluster of glassy crystal shards.
pub fn crystals(b: &mut MeshBuilder, size: f32, c: Col, rng: &mut Rng) {
    for k in 0..(3 + rng.below(4)) {
        let tilt = Vec3::new(rng.sym(0.5), 1.0, rng.sym(0.5)).normalize();
        let h = size * if k == 0 { 1.0 } else { rng.range(0.45, 0.8) };
        let r = h * 0.2;
        let base = Vec3::new(rng.sym(size * 0.25), 0.0, rng.sym(size * 0.25));
        let (u, v) = crate::meshkit::frame(tilt);
        let ring: Vec<Vec3> = (0..5)
            .map(|i| base + (u * (i as f32 * TAU / 5.0).cos() + v * (i as f32 * TAU / 5.0).sin()) * r)
            .collect();
        let tip = base + tilt * h;
        for i in 0..5 {
            let (a, bb) = (ring[i], ring[(i + 1) % 5]);
            let (sa, sb) = (a + tilt * h * 0.75, bb + tilt * h * 0.75);
            let tone = shade(c, 0.75 + 0.1 * i as f32);
            b.quad_p([a, bb, sb, sa], tone);
            b.tri_p([sa, sb, tip], shade(tone, 1.25));
        }
    }
}
