//! Galemother, the sky whale, swimming through the clouds with a coral
//! garden on her back. The track runs out along each pectoral fin, over the
//! blowhole on her head and around her flukes. She beats her fins, banks into
//! the wind, and blows.

use super::{
    AnimOut, Animator, BeastCtl, Built, EnvDef, Fauna, FaunaKind, Ground, Layer, Mat, ShowView, child, flora, pivot,
};
use crate::{
    meshkit::{Col, MeshBuilder, Skin},
    track::{Ctrl, Edge, Features, Track, TrackStyle},
    util::{Rng, fbm2, lin, mix, noise2, shade, smooth, smoothstep},
};
use bevy::{math::Affine3A, prelude::*};
use std::f32::consts::TAU;

const POW: f32 = 2.4;
pub const CLOUDS: f32 = -430.0;

// Bones.
const BODY: usize = 0;
const FRONT: usize = 1;
const HEAD: usize = 2;
const REAR: usize = 3;
const TAIL0: usize = 4;
const TAIL1: usize = 5;
const FLUKES: usize = 6;
const FIN: usize = 7; // + side * 3 + segment; side 0 is -X
const BONES: usize = 13;

/// Joints along the spine from tail to head, and the bones on either side.
const JOINTS: [f32; 6] = [-240.0, -190.0, -120.0, -40.0, 40.0, 120.0];
const SEGMENTS: [usize; 7] = [FLUKES, TAIL1, TAIL0, REAR, BODY, FRONT, HEAD];

/// Fin segments start this far out along the fin from its root.
const FIN_JOINTS: [f32; 3] = [0.0, 55.0, 105.0];
const FIN_Y: f32 = 18.0;
const FIN_ROOT: Vec3 = Vec3::new(78.0, FIN_Y, 88.0);
/// How far the fins sweep back, in radians.
const SWEEP: f32 = 0.4;
const FIN_SPAN: f32 = 160.0;

/// A fin's frame: root, span direction (out and back) and chord direction
/// (toward the leading edge). Side 0 is -X.
fn fin_frame(side: usize) -> (Vec3, Vec3, Vec3) {
    let sx = if side == 0 { -1.0 } else { 1.0 };
    let root = Vec3::new(sx * FIN_ROOT.x, FIN_ROOT.y, FIN_ROOT.z);
    let span = Vec3::new(sx * SWEEP.cos(), 0.0, -SWEEP.sin());
    let chord = Vec3::new(sx * SWEEP.sin(), 0.0, SWEEP.cos());
    (root, span, chord)
}

/// A point on the fin at `(along, across)`, at height `y`.
fn fin_point(side: usize, along: f32, across: f32, y: f32) -> Vec3 {
    let (root, span, chord) = fin_frame(side);
    let p = root + span * along + chord * across;
    Vec3::new(p.x, y, p.z)
}

/// How far out along the fin a point is.
fn fin_along(side: usize, p: Vec3) -> f32 {
    let (root, span, _) = fin_frame(side);
    (p - root).dot(span)
}

/// Cross-sections: `(z, half width, half height, center height)`.
const PROFILE: [(f32, f32, f32, f32); 14] = [
    (-266.0, 1.0, 1.0, 2.0),
    (-262.0, 8.0, 6.0, 2.0),
    (-240.0, 24.0, 14.0, 2.0),
    (-200.0, 34.0, 24.0, 2.0),
    (-140.0, 52.0, 34.0, 0.0),
    (-70.0, 78.0, 44.0, -2.0),
    (0.0, 90.0, 48.0, -4.0),
    (70.0, 92.0, 48.0, -4.0),
    (130.0, 82.0, 44.0, -4.0),
    (185.0, 64.0, 38.0, -6.0),
    (222.0, 46.0, 30.0, -8.0),
    (240.0, 30.0, 21.0, -10.0),
    (249.0, 14.0, 11.0, -10.0),
    (252.0, 1.0, 1.0, -10.0),
];

fn profile(z: f32) -> (f32, f32, f32) {
    let z = z.clamp(PROFILE[0].0, PROFILE[PROFILE.len() - 1].0);
    for w in PROFILE.windows(2) {
        if z <= w[1].0 {
            let t = smooth((z - w[0].0) / (w[1].0 - w[0].0));
            return (
                w[0].1 + (w[1].1 - w[0].1) * t,
                w[0].2 + (w[1].2 - w[0].2) * t,
                w[0].3 + (w[1].3 - w[0].3) * t,
            );
        }
    }
    let l = PROFILE[PROFILE.len() - 1];
    (l.1, l.2, l.3)
}

fn top(x: f32, z: f32) -> f32 {
    let (rx, ry, cy) = profile(z);
    let q = (x.abs() / rx).min(0.999);
    cy + ry * (1.0 - q.powf(POW)).powf(1.0 / POW)
}

fn top_normal(x: f32, z: f32) -> Vec3 {
    let e = 1.0;
    Vec3::new(top(x - e, z) - top(x + e, z), 2.0 * e, top(x, z - e) - top(x, z + e)).normalize()
}

fn spine_skin(z: f32) -> Skin {
    let mut i = 0;
    for (k, j) in JOINTS.iter().enumerate() {
        if (z - j).abs() < (z - JOINTS[i]).abs() {
            i = k;
        }
    }
    let before = if i == 0 { 40.0 } else { JOINTS[i] - JOINTS[i - 1] };
    let after = if i + 1 == JOINTS.len() {
        40.0
    } else {
        JOINTS[i + 1] - JOINTS[i]
    };
    let zone = (before.min(after) * 0.5).min(22.0);
    Skin::two(
        SEGMENTS[i],
        SEGMENTS[i + 1],
        smoothstep(JOINTS[i] - zone, JOINTS[i] + zone, z),
    )
}

/// The fin's outline: leading and trailing edge offsets across the chord,
/// `along` the span from the root.
fn fin_chord(along: f32) -> (f32, f32) {
    let le = [
        (-20.0, 31.0),
        (0.0, 31.0),
        (60.0, 33.0),
        (110.0, 30.0),
        (135.0, 24.0),
        (152.0, 14.0),
        (FIN_SPAN, 0.5),
    ];
    let te = [
        (-20.0, -34.0),
        (0.0, -34.0),
        (60.0, -31.0),
        (110.0, -27.0),
        (135.0, -24.0),
        (152.0, -14.0),
        (FIN_SPAN, -0.5),
    ];
    let at = |pts: &[(f32, f32)]| {
        let s = along.clamp(pts[0].0, pts[pts.len() - 1].0);
        for w in pts.windows(2) {
            if s <= w[1].0 {
                let t = (s - w[0].0) / (w[1].0 - w[0].0);
                return w[0].1 + (w[1].1 - w[0].1) * t;
            }
        }
        pts[pts.len() - 1].1
    };
    (at(&le), at(&te))
}

fn fin_thickness(along: f32) -> f32 {
    13.0 - 7.0 * smoothstep(0.0, FIN_SPAN, along)
}

/// Skin for a point on or near a fin, from how far out along it it is.
fn fin_skin(side: usize, p: Vec3) -> Skin {
    let s = fin_along(side, p);
    let bone = FIN + side * 3;
    const ZONE: f32 = 16.0;
    if s < (FIN_JOINTS[0] + FIN_JOINTS[1]) * 0.5 {
        Skin::two(FRONT, bone, smoothstep(FIN_JOINTS[0] - ZONE, FIN_JOINTS[0] + ZONE, s))
    } else if s < (FIN_JOINTS[1] + FIN_JOINTS[2]) * 0.5 {
        Skin::two(
            bone,
            bone + 1,
            smoothstep(FIN_JOINTS[1] - ZONE, FIN_JOINTS[1] + ZONE, s),
        )
    } else {
        Skin::two(
            bone + 1,
            bone + 2,
            smoothstep(FIN_JOINTS[2] - ZONE, FIN_JOINTS[2] + ZONE, s),
        )
    }
}

fn fin_top(along: f32) -> f32 {
    FIN_Y + fin_thickness(along) * 0.5
}

/// The flukes' outline, one side: `(x, z)` from the tail stock round to the tip and back.
const FLUKE: [(f32, f32); 8] = [
    (0.0, -236.0),
    (40.0, -248.0),
    (92.0, -266.0),
    (116.0, -290.0),
    (104.0, -306.0),
    (60.0, -309.0),
    (20.0, -313.0),
    (0.0, -318.0),
];
const FLUKE_Y: f32 = 2.0;
const FLUKE_T: f32 = 8.0;

fn pivots() -> Vec<Vec3> {
    let mut p = vec![Vec3::ZERO; BONES];
    let at = |z: f32| Vec3::new(0.0, profile(z).2, z);
    p[BODY] = at(0.0);
    p[FRONT] = at(40.0);
    p[HEAD] = at(120.0);
    p[REAR] = at(-40.0);
    p[TAIL0] = at(-120.0);
    p[TAIL1] = at(-190.0);
    p[FLUKES] = at(-240.0);
    for side in 0..2 {
        for seg in 0..3 {
            p[FIN + side * 3 + seg] = fin_point(side, FIN_JOINTS[seg], 0.0, FIN_Y);
        }
    }
    p
}

fn bump(t: f32, a: f32, b: f32, ease: f32) -> f32 {
    smoothstep(a, a + ease, t) * (1.0 - smoothstep(b - ease, b, t))
}

struct WhaleAnim {
    piv: Vec<Vec3>,
    prev_cycle: f32,
    swim: f32,
    bank_side: f32,
}

const CYCLE: f32 = 70.0;

impl Animator for WhaleAnim {
    fn pose(&mut self, t: f32, dt: f32, ctl: &BeastCtl, out: &mut [Affine3A], info: &mut AnimOut) {
        let cyc = if ctl.calm { 0.0 } else { t.rem_euclid(CYCLE) };
        let beat = bump(cyc, 16.0, 30.0, 4.0);
        let bank = bump(cyc, 42.0, 56.0, 4.5);
        if !ctl.calm {
            if self.prev_cycle < 16.0 && cyc >= 16.0 {
                info.banner = Some("GALEMOTHER BEATS HER GREAT FINS!");
            }
            if self.prev_cycle < 42.0 && cyc >= 42.0 {
                info.banner = Some("GALEMOTHER BANKS INTO THE WIND!");
                self.bank_side = if (t / CYCLE).floor() as i32 % 2 == 0 { 1.0 } else { -1.0 };
            }
        }
        self.prev_cycle = cyc;
        // She blows every so often; the geyser in the road erupts with her.
        let blow = (t + 3.0).rem_euclid(11.0);
        info.spout = !ctl.calm && blow < 2.2;

        self.swim += dt * (1.0 + 0.6 * beat);
        let s = self.swim;
        let sh = ctl.shudder;
        info.scroll = Vec3::new(0.0, 0.0, -24.0 * (1.0 + 0.3 * beat));
        info.rumble = sh;

        let p = &self.piv;
        let roll = 0.03 * (s * 0.5).sin() + 0.28 * bank * self.bank_side + 0.02 * sh * (t * 8.0).sin();
        let body = Affine3A::from_rotation_translation(
            Quat::from_rotation_y(0.05 * bank * self.bank_side)
                * Quat::from_rotation_z(roll)
                * Quat::from_rotation_x(0.025 * (s * 0.7).sin()),
            p[BODY] + Vec3::Y * (4.0 * (s * 0.7 + 0.5).sin() + 0.55 * sh * (t * 11.0).sin()),
        );
        out[BODY] = body;
        // A slow wave runs from head to flukes as she swims.
        let wave = |k: f32| 0.035 * (s * 0.7 - k * 0.9).sin() * (1.0 + 0.8 * beat);
        out[FRONT] = child(body, p[BODY], p[FRONT], Quat::from_rotation_x(-wave(-0.5) * 0.5));
        out[HEAD] = child(out[FRONT], p[FRONT], p[HEAD], Quat::from_rotation_x(-wave(-1.0) * 0.6));
        out[REAR] = child(body, p[BODY], p[REAR], Quat::from_rotation_x(wave(0.5)));
        out[TAIL0] = child(out[REAR], p[REAR], p[TAIL0], Quat::from_rotation_x(wave(1.0) * 1.3));
        out[TAIL1] = child(out[TAIL0], p[TAIL0], p[TAIL1], Quat::from_rotation_x(wave(1.6) * 1.6));
        out[FLUKES] = child(out[TAIL1], p[TAIL1], p[FLUKES], Quat::from_rotation_x(wave(2.2) * 2.0));

        // Fins: a lazy paddle, deep strokes when she beats them, and the low
        // fin dips further when she banks.
        for side in 0..2 {
            let sx = if side == 0 { -1.0 } else { 1.0 };
            let stroke = (s * 1.1 + side as f32 * 0.4).sin();
            let lift = (0.06 + 0.2 * beat) * stroke + 0.12 * bank * self.bank_side * sx;
            let chord = fin_frame(side).2;
            let mut parent = (out[FRONT], p[FRONT]);
            for seg in 0..3 {
                let bone = FIN + side * 3 + seg;
                let bend = lift * [1.0, 0.45, 0.35][seg];
                // Raising the fin tip: turn about the chord line toward +Y on this side.
                let g = child(parent.0, parent.1, p[bone], Quat::from_axis_angle(chord, sx * bend));
                out[bone] = g;
                parent = (g, p[bone]);
            }
        }
    }
}

pub fn build() -> Built {
    let mut rng = Rng::new(0x5CA1E);
    let piv = pivots();

    // ---------------------------------------------------------------- track
    let on_back = |x: f32, z: f32| {
        let n = top_normal(x, z);
        Ctrl::new(
            Vec3::new(x, top(x, z) + 0.9, z),
            (n * 0.4 + Vec3::Y * 0.6).normalize(),
            0,
        )
    };
    let back = |x: f32, z: f32| on_back(x, z).sh(3.5, 3.5).edges(Edge::Soft, Edge::Soft);
    // Fin stretches are placed in the fin's own coordinates: out along it, and across it.
    let fin = |side: usize, along: f32, across: f32| {
        Ctrl::new(
            fin_point(side, along, across, fin_top(along) + 0.9),
            Vec3::Y,
            1 + side as u8,
        )
        .half(6.2)
        .sh(1.4, 1.4)
        .edges(Edge::Wall, Edge::Wall)
        .surf(1)
    };
    let fluke = |x: f32, z: f32| {
        Ctrl::new(Vec3::new(x, FLUKE_Y + FLUKE_T * 0.5 + 0.9, z), Vec3::Y, 3)
            .half(6.8)
            .sh(1.4, 1.4)
            .edges(Edge::Wall, Edge::Wall)
            .surf(1)
    };
    let tail = |x: f32, z: f32| on_back(x, z).half(6.4).sh(1.4, 1.4).edges(Edge::Wall, Edge::Wall);
    let ctrl = vec![
        back(-30.0, -80.0), // 0: start line
        back(-38.0, -20.0),
        back(-45.0, 38.0),
        fin(0, 18.0, -14.0), // 3: out along the -X fin
        fin(0, 68.0, -14.0),
        fin(0, 114.0, -13.0),
        fin(0, 138.0, -9.0),
        fin(0, 149.0, 0.0),
        fin(0, 138.0, 9.0),
        fin(0, 114.0, 13.0),
        fin(0, 68.0, 14.0),
        fin(0, 18.0, 14.0),
        back(-58.0, 132.0), // 12: over the head
        back(-30.0, 166.0),
        back(0.0, 196.0).edges(Edge::Wall, Edge::Wall),
        back(30.0, 166.0),
        back(58.0, 132.0),
        fin(1, 18.0, 14.0), // 17: out along the +X fin
        fin(1, 68.0, 14.0),
        fin(1, 114.0, 13.0),
        fin(1, 138.0, 9.0),
        fin(1, 149.0, 0.0),
        fin(1, 138.0, -9.0),
        fin(1, 114.0, -13.0),
        fin(1, 68.0, -14.0),
        fin(1, 18.0, -14.0),
        back(45.0, 38.0), // 26: down her right side
        back(38.0, -20.0),
        back(30.0, -80.0),
        back(34.0, -150.0),
        tail(18.0, -205.0),
        fluke(44.0, -262.0), // 31: around the flukes
        fluke(62.0, -290.0),
        fluke(0.0, -306.0),
        fluke(-62.0, -290.0),
        fluke(-44.0, -262.0),
        tail(-18.0, -205.0),
        back(-34.0, -150.0), // 37
    ];
    let features = Features {
        boosts: vec![
            (1.5, 0.0),
            (5.5, 0.0),
            (14.5, 0.0),
            (21.5, 0.0),
            (28.5, 2.0),
            (33.0, 0.0),
        ],
        item_rows: vec![2.2, 10.6, 15.6, 24.5, 29.6, 36.6],
        ramps: vec![(30.45, 13.0, 2.4), (36.45, 13.0, 2.4)],
        critters: vec![1.5, 27.5],
        geysers: vec![14.0],
    };
    let skin_of = |part: u8, p: Vec3| match part {
        0 => spine_skin(p.z),
        1 => fin_skin(0, p),
        2 => fin_skin(1, p),
        _ => Skin::one(FLUKES),
    };
    let settle = |a: u8, b: u8, p: Vec3| match (a, b) {
        (0, 0) => Vec3::new(p.x, top(p.x, p.z) + 0.9, p.z),
        (1, 1) => Vec3::new(p.x, fin_top(fin_along(0, p)) + 0.9, p.z),
        (2, 2) => Vec3::new(p.x, fin_top(fin_along(1, p)) + 0.9, p.z),
        _ => p,
    };
    let track = Track::build(&ctrl, &features, &skin_of, &settle);
    let road: Vec<(Vec3, f32)> = track
        .samples
        .iter()
        .map(|s| (s.c, s.half + s.sh[0].max(s.sh[1])))
        .collect();
    let clearance = |p: Vec3| -> f32 {
        let mut best = f32::MAX;
        for &(c, lim) in &road {
            best = best.min(c.distance(p) - lim);
        }
        best
    };

    // ---------------------------------------------------------------- body
    let mut hide = MeshBuilder::new();
    let skin_top = lin(0x3a5b8c);
    let skin_dark = lin(0x2b4470);
    let belly = lin(0xe6e2dc);
    let garden_a = lin(0x8fd16a);
    let garden_b = lin(0x5fb56a);
    let sides = 30;
    let mut zs: Vec<f32> = Vec::new();
    let mut z = PROFILE[0].0;
    while z < PROFILE[PROFILE.len() - 1].0 {
        zs.push(z);
        z += if !(-250.0..=230.0).contains(&z) { 3.0 } else { 7.0 };
    }
    zs.push(PROFILE[PROFILE.len() - 1].0);
    let rings: Vec<Vec<(Vec3, Col, Skin)>> = zs
        .iter()
        .map(|&z| {
            let (rx, ry, cy) = profile(z);
            (0..sides)
                .map(|k| {
                    let a = k as f32 / sides as f32 * TAU;
                    let (s, c) = a.sin_cos();
                    let e = 2.0 / POW;
                    let mut p = Vec3::new(
                        rx * c.signum() * c.abs().powf(e),
                        cy + ry * s.signum() * s.abs().powf(e),
                        z,
                    );
                    let n = noise2(z * 0.04 + 3.0, a * 2.5, 0, 17);
                    // Pale spots on the dark hide, and throat grooves underneath.
                    let spots = smoothstep(0.62, 0.7, noise2(p.x * 0.09, z * 0.09, 0, 29));
                    let mut col = mix(mix(skin_top, skin_dark, n), lin(0x8fb0d8), spots * 0.6);
                    if s < -0.3 {
                        let groove = ((p.x * 0.35).sin() * 0.5 + 0.5).powf(3.0);
                        col = mix(
                            mix(skin_top, belly, smoothstep(-0.3, -0.6, s)),
                            shade(belly, 0.8),
                            groove * 0.5,
                        );
                    }
                    if s > 0.35 && z > -200.0 && z < 215.0 {
                        let lush = mix(garden_a, garden_b, noise2(p.x * 0.05, z * 0.05, 0, 31));
                        let cover = smoothstep(0.35, 0.6, s + (n - 0.5) * 0.3);
                        col = mix(col, lush, cover);
                        let free = smoothstep(0.0, 9.0, clearance(p));
                        p.y += (fbm2(p.x * 0.06, z * 0.06, 3, 41) - 0.35) * 4.0 * free * cover;
                    }
                    (
                        p,
                        shade(col, 0.93 + 0.14 * noise2(z * 0.3, a * 7.0, 0, 43)),
                        spine_skin(z),
                    )
                })
                .collect()
        })
        .collect();
    for w in rings.windows(2) {
        for k in 0..sides {
            let k1 = (k + 1) % sides;
            let v: Vec<u32> = [w[0][k], w[0][k1], w[1][k1], w[1][k]]
                .iter()
                .map(|&(p, c, s)| hide.vs(p, c, s))
                .collect();
            hide.quad(v[0], v[1], v[2], v[3]);
        }
    }
    // Eyes and a mouth line on the head.
    hide.cur = Skin::one(HEAD);
    for sx in [-1.0f32, 1.0] {
        let z = 196.0;
        let (rx, ry, cy) = profile(z);
        let eye = Vec3::new(sx * rx * 0.9, cy - ry * 0.3, z);
        hide.ico_with(eye, Vec3::splat(8.5), 2, |d| {
            let c = if d.x * sx > 0.7 {
                lin(0x0f1422)
            } else if d.x * sx > 0.45 {
                lin(0x6fd3ff)
            } else {
                lin(0xf4f1ea)
            };
            (1.0, c)
        });
        hide.ico(
            eye + Vec3::new(-sx * 1.0, 4.4, 0.0),
            Vec3::new(9.0, 4.2, 10.4),
            1,
            skin_dark,
        );
        // A long smile along the jaw.
        for i in 0..18 {
            let f0 = i as f32 / 18.0;
            let f1 = (i + 1) as f32 / 18.0;
            let at = |f: f32| {
                let z = 150.0 + f * 98.0;
                let (rx, ry, cy) = profile(z);
                Vec3::new(sx * rx * 0.99, cy - ry * 0.45, z)
            };
            hide.cyl(at(f0), at(f1), 1.4, 1.4, 4, lin(0x1d2a45), false);
        }
    }
    // The blowhole: a raised rim of pale hide around the geyser.
    let blow = track.samples[track.geysers[0] as usize].c;
    for k in 0..10 {
        let a = k as f32 / 10.0 * TAU;
        let p = blow + Vec3::new(a.cos() * 3.2, -0.4, a.sin() * 2.2);
        hide.ico(p, Vec3::new(1.4, 0.8, 1.4), 0, lin(0xcfd9e6));
    }

    // ---------------------------------------------------------------- fins
    for side in 0..2 {
        let (_, span, _) = fin_frame(side);
        let stations: Vec<f32> = (0..=28).map(|i| -18.0 + i as f32 * (FIN_SPAN + 18.0) / 28.0).collect();
        let fin_rings: Vec<Vec<(Vec3, Col)>> = stations
            .iter()
            .map(|&along| {
                let (le, te) = fin_chord(along);
                let center = fin_point(side, along, (le + te) * 0.5, FIN_Y);
                let (a, b) = (((le - te) * 0.5).max(0.5), fin_thickness(along) * 0.5);
                // Rings wind counter-clockwise about the direction the loft travels: out along the span.
                MeshBuilder::ring(center, span, Vec3::Y, a, b, 16, skin_top)
                    .into_iter()
                    .map(|(p, _)| {
                        let up = (p.y - FIN_Y) / b;
                        let spots = smoothstep(0.6, 0.7, noise2(p.x * 0.1, p.z * 0.1, 0, 51));
                        let c = if up > 0.2 {
                            mix(
                                mix(skin_top, skin_dark, noise2(p.x * 0.04, p.z * 0.04, 0, 53)),
                                lin(0x8fb0d8),
                                spots * 0.6,
                            )
                        } else {
                            mix(skin_top, belly, smoothstep(0.2, -0.5, up))
                        };
                        (p, c)
                    })
                    .collect()
            })
            .collect();
        let first = hide.vertex_count();
        hide.loft(&fin_rings, true, true);
        hide.reskin_from(first, |p| fin_skin(side, p));
        // Knobbly tubercles along the leading edge, as on a humpback's fin.
        for k in 0..9 {
            let along = 10.0 + k as f32 * 16.0;
            let le = fin_chord(along).0;
            let p = fin_point(side, along, le - 1.5, FIN_Y + 1.0);
            hide.cur = fin_skin(side, p);
            hide.ico(p, Vec3::new(3.2, 2.6, 3.2), 0, lin(0xb9cbe0));
        }
    }

    // ---------------------------------------------------------------- flukes
    {
        hide.cur = Skin::one(FLUKES);
        let mut outline: Vec<Vec3> = FLUKE.iter().map(|&(x, z)| Vec3::new(x, 0.0, z)).collect();
        let mirrored: Vec<Vec3> = FLUKE
            .iter()
            .rev()
            .skip(1)
            .take(FLUKE.len() - 2)
            .map(|&(x, z)| Vec3::new(-x, 0.0, z))
            .collect();
        outline.extend(mirrored);
        let center = Vec3::new(0.0, 0.0, -278.0);
        let n = outline.len();
        let y_top = FLUKE_Y + FLUKE_T * 0.5;
        let y_bot = FLUKE_Y - FLUKE_T * 0.5;
        for i in 0..n {
            let (a, b) = (outline[i], outline[(i + 1) % n]);
            let tone = |p: Vec3| mix(skin_top, skin_dark, noise2(p.x * 0.05, p.z * 0.05, 0, 61));
            hide.tri_facing(
                [
                    center + Vec3::Y * y_top,
                    Vec3::new(b.x, y_top, b.z),
                    Vec3::new(a.x, y_top, a.z),
                ],
                tone(a),
                Vec3::Y,
            );
            hide.tri_facing(
                [
                    center + Vec3::Y * y_bot,
                    Vec3::new(a.x, y_bot, a.z),
                    Vec3::new(b.x, y_bot, b.z),
                ],
                belly,
                Vec3::NEG_Y,
            );
            hide.quad_facing(
                [
                    Vec3::new(a.x, y_bot, a.z),
                    Vec3::new(b.x, y_bot, b.z),
                    Vec3::new(b.x, y_top, b.z),
                    Vec3::new(a.x, y_top, a.z),
                ],
                skin_dark,
                center + Vec3::Y * FLUKE_Y,
            );
        }
    }

    // ---------------------------------------------------------------- garden
    let mut plants = MeshBuilder::new();
    let corals = [0xff7aa8u32, 0xffb36b, 0xb38cff, 0x6fe0d8, 0xffe07a];
    let blossoms = [0xf7a8d0u32, 0xfff1f7, 0xc9b6ff];
    let spot = |rng: &mut Rng, z0: f32, z1: f32, spread: f32| -> (Vec3, Vec3) {
        let z = rng.range(z0, z1);
        let rx = profile(z).0;
        let x = rng.sym(rx * spread);
        (Vec3::new(x, top(x, z), z), top_normal(x, z))
    };
    let mut blooms: Vec<Vec3> = Vec::new();
    for _ in 0..1700 {
        let (p, n) = spot(&mut rng, -205.0, 215.0, 0.85);
        if clearance(p) < 1.2 || n.y < 0.5 {
            continue;
        }
        plants.cur = spine_skin(p.z);
        plants.xf = Affine3A::from_rotation_translation(Quat::from_rotation_y(rng.angle()), p - Vec3::Y * 0.3);
        let big = clearance(p) > 4.0;
        match rng.below(10) {
            0 | 1 if big => flora::broadleaf(&mut plants, 9.0 + 7.0 * rng.f(), lin(blossoms[rng.below(3)]), &mut rng),
            2 | 3 => flora::coral(
                &mut plants,
                3.0 + 4.0 * rng.f(),
                lin(corals[rng.below(corals.len())]),
                &mut rng,
            ),
            4 => flora::sea_fan(
                &mut plants,
                3.0 + 3.0 * rng.f(),
                lin(corals[rng.below(corals.len())]),
                &mut rng,
            ),
            5 => flora::crystals(
                &mut plants,
                2.5 + 4.0 * rng.f(),
                lin([0x9fe8ff, 0xd6b8ff, 0xfff4a8][rng.below(3)]),
                &mut rng,
            ),
            6 => flora::anemone(
                &mut plants,
                1.6 + 1.6 * rng.f(),
                lin(corals[rng.below(corals.len())]),
                &mut rng,
            ),
            7 => {
                flora::flower(&mut plants, 1.8 + 1.2 * rng.f(), lin(blossoms[rng.below(3)]), &mut rng);
                blooms.push(p);
            }
            8 => flora::fern(&mut plants, 2.4 + 1.6 * rng.f(), lin(0x7fd07a), &mut rng),
            _ => flora::tuft(&mut plants, 1.6 + 1.4 * rng.f(), lin(0xa8e07a), &mut rng),
        }
    }
    // Barnacles on the flanks and the fins; streamers of weed trailing below.
    for _ in 0..160 {
        let z = rng.range(-220.0, 220.0);
        let (rx, ry, cy) = profile(z);
        let sx = if rng.chance(0.5) { 1.0 } else { -1.0 };
        let a: f32 = rng.range(-0.5, 0.35);
        let p = Vec3::new(sx * rx * a.cos(), cy + ry * a.sin(), z);
        let normal = Vec3::new(sx * a.cos(), a.sin(), 0.0).normalize();
        plants.cur = spine_skin(z);
        plants.xf = Affine3A::from_rotation_translation(Quat::from_rotation_arc(Vec3::Y, normal), p);
        flora::barnacles(&mut plants, 1.6 + 2.2 * rng.f(), &mut rng);
    }
    for _ in 0..90 {
        let z = rng.range(-180.0, 200.0);
        let (rx, _, cy) = profile(z);
        let x = rng.sym(rx * 0.7);
        plants.cur = spine_skin(z);
        plants.xf = Affine3A::from_translation(Vec3::new(x, cy - profile(z).1 * 0.9, z));
        flora::strand(
            &mut plants,
            14.0 + 26.0 * rng.f(),
            lin(if rng.chance(0.5) { 0x6fb58a } else { 0x9ad0c0 }),
            &mut rng,
        );
    }
    for side in 0..2 {
        for _ in 0..26 {
            let along = rng.range(15.0, 150.0);
            let (le, te) = fin_chord(along);
            // Along the leading and trailing edges, clear of the lanes.
            let across = if rng.chance(0.5) { le - 2.5 } else { te + 2.5 };
            let p = fin_point(side, along, across, fin_top(along) - 0.4);
            if clearance(p) < 0.5 {
                continue;
            }
            plants.cur = fin_skin(side, p);
            plants.xf = Affine3A::from_translation(p);
            if rng.chance(0.6) {
                flora::barnacles(&mut plants, 1.5 + 1.5 * rng.f(), &mut rng);
            } else {
                flora::coral(
                    &mut plants,
                    2.5 + 2.5 * rng.f(),
                    lin(corals[rng.below(corals.len())]),
                    &mut rng,
                );
            }
        }
    }
    plants.xf = Affine3A::IDENTITY;

    // ---------------------------------------------------------------- road
    let style = TrackStyle {
        road: vec![[lin(0xb8916c), lin(0xad8662)], [lin(0x8e97c2), lin(0x838cb7)]],
        curb: [lin(0xfff7fb), lin(0x5ec8e8)],
        line: lin(0xfffaf0),
        shoulder: [lin(0x93d46e), lin(0x86c763)],
        skirt: lin(0x2f4a74),
        wall: [lin(0xf7a8d0), lin(0xfff1f7)],
        post: lin(0x6fe0d8),
    };
    let (ribbon, glow) = track.build_mesh(&style);

    // ---------------------------------------------------------------- fauna
    let mut fauna = Vec::new();
    for (k, p) in blooms.iter().step_by((blooms.len() / 20).max(1)).enumerate() {
        fauna.push(Fauna {
            kind: FaunaKind::Butterfly,
            skin: spine_skin(p.z),
            rest: *p + Vec3::Y * 2.0,
            seed: k as f32 * 2.1,
        });
    }
    for k in 0..16 {
        let z = rng.range(-280.0, 240.0);
        fauna.push(Fauna {
            kind: FaunaKind::Gull,
            skin: spine_skin(z),
            rest: Vec3::new(rng.sym(160.0), top(0.0, z) + rng.range(25.0, 90.0), z),
            seed: k as f32 * 1.7 + rng.f(),
        });
    }

    Built {
        rest: piv.iter().map(|&p| pivot(p)).collect(),
        track,
        layers: vec![
            Layer {
                mesh: hide,
                mat: Mat::Gloss,
                flat: true,
            },
            Layer {
                mesh: plants,
                mat: Mat::Matte,
                flat: true,
            },
            Layer {
                mesh: ribbon,
                mat: Mat::Matte,
                flat: true,
            },
            Layer {
                mesh: glow,
                mat: Mat::Glow,
                flat: true,
            },
        ],
        fauna,
        animator: Box::new(WhaleAnim {
            piv,
            prev_cycle: 0.0,
            swim: 0.0,
            bank_side: 1.0,
        }),
        env: EnvDef {
            ground: Ground::Sky { floor: CLOUDS },
            sky_top: 0x2a5fcf,
            sky_horizon: 0xffc690,
            fog: 0xf6c9a2,
            fog_start: 1400.0,
            fog_end: 7000.0,
            sun_dir: Vec3::new(0.55, -0.5, -0.67),
            sun_color: 0xffe1b8,
            sun_lux: 13000.0,
            ambient: 1050.0,
            cloud_height: (-300.0, 220.0),
        },
        view: ShowView {
            center: Vec3::new(0.0, 0.0, -20.0),
            radius: 620.0,
            height: 160.0,
        },
        critter_color: 0x7fd6ff,
    }
}
