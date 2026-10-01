//! Thundermoss, the great grazer: a long-necked, club-tailed herbivore the
//! size of a mountain, with a forest on his back. The track runs up one side
//! of his spine, around the crown of his head, down the other side and out
//! around the club of his tail. His neck and tail carry the road with them.

use super::{
    AnimOut, Animator, BeastCtl, Built, EnvDef, Fauna, FaunaKind, Ground, Layer, Mat, ShowView, aim_bone, child, flora,
    ik_knee, limb, pivot,
};
use crate::{
    meshkit::{Col, MeshBuilder, Skin},
    track::{Ctrl, Edge, Features, Track, TrackStyle},
    util::{Rng, damp, fbm2, lin, mix, noise2, shade, smooth, smoothstep},
};
use bevy::{math::Affine3A, prelude::*};
use std::f32::consts::{PI, TAU};

pub const GROUND: f32 = -140.0;
/// Squareness of the body's cross-section: higher is flatter on top.
const POW: f32 = 3.0;

// Bones. The spine runs CLUB .. TAIL0, HIP, BODY, CHEST, NECK0 .. NECK4, HEAD.
const BODY: usize = 0;
const CHEST: usize = 1;
const NECK: usize = 2; // ..=6
const HEAD: usize = 7;
const JAW: usize = 8;
const HIP: usize = 9;
const TAIL: usize = 10; // ..=14
const CLUB: usize = 15;
const LEG: usize = 16; // + leg * 2: upper, lower. Legs: front +X, front -X, hind +X, hind -X.
const BONES: usize = 24;

/// Joints along the spine from tail to head, and the bones on either side.
const JOINTS: [f32; 14] = [
    -262.0, -232.0, -200.0, -165.0, -130.0, -95.0, -55.0, 55.0, 95.0, 125.0, 155.0, 185.0, 215.0, 243.0,
];
const SEGMENTS: [usize; 15] = [
    CLUB,
    TAIL + 4,
    TAIL + 3,
    TAIL + 2,
    TAIL + 1,
    TAIL,
    HIP,
    BODY,
    CHEST,
    NECK,
    NECK + 1,
    NECK + 2,
    NECK + 3,
    NECK + 4,
    HEAD,
];

const LEG_LEN: f32 = 62.0;
const GAIT_PERIOD: f32 = 6.4;
const DUTY: f32 = 0.72;
const STRIDE: f32 = 62.0;

/// Cross-sections along the body: `(z, half width, half height, center height)`.
const PROFILE: [(f32, f32, f32, f32); 21] = [
    (-342.0, 1.0, 1.0, 4.0),
    (-338.0, 12.0, 8.0, 4.0),
    (-326.0, 34.0, 13.0, 4.0),
    (-296.0, 38.0, 14.0, 4.0),
    (-266.0, 24.0, 12.0, 4.0),
    (-230.0, 20.0, 12.0, 3.0),
    (-160.0, 24.0, 15.0, 1.0),
    (-100.0, 40.0, 26.0, -4.0),
    (-70.0, 58.0, 36.0, -6.0),
    (-40.0, 62.0, 42.0, -6.0),
    (0.0, 64.0, 40.0, -8.0),
    (40.0, 62.0, 44.0, -6.0),
    (75.0, 54.0, 38.0, -6.0),
    (105.0, 36.0, 28.0, -2.0),
    (150.0, 25.0, 19.0, 2.0),
    (220.0, 21.0, 16.0, 4.0),
    (248.0, 38.0, 17.0, 5.0),
    (290.0, 40.0, 17.0, 5.0),
    (318.0, 30.0, 14.0, 3.0),
    (342.0, 14.0, 9.0, 1.0),
    (351.0, 1.0, 1.0, 0.0),
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

/// Height of the hide at `(x, z)` on the upper side.
fn top(x: f32, z: f32) -> f32 {
    let (rx, ry, cy) = profile(z);
    let q = (x.abs() / rx).min(0.999);
    cy + ry * (1.0 - q.powf(POW)).powf(1.0 / POW)
}

fn top_normal(x: f32, z: f32) -> Vec3 {
    let e = 1.0;
    Vec3::new(top(x - e, z) - top(x + e, z), 2.0 * e, top(x, z - e) - top(x, z + e)).normalize()
}

/// Skin weights for a rest-pose point, from its position along the spine.
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
    let zone = (before.min(after) * 0.5).min(16.0);
    Skin::two(
        SEGMENTS[i],
        SEGMENTS[i + 1],
        smoothstep(JOINTS[i] - zone, JOINTS[i] + zone, z),
    )
}

fn spine_pivot(z: f32) -> Vec3 {
    Vec3::new(0.0, profile(z).2, z)
}

#[derive(Clone, Copy)]
struct LegRest {
    hip: Vec3,
    knee: Vec3,
    foot: Vec3,
    normal: Vec3,
    parent: usize,
}

fn leg_rest(leg: usize) -> LegRest {
    let sx = if leg.is_multiple_of(2) { 1.0 } else { -1.0 };
    let front = leg < 2;
    let z = if front { 60.0 } else { -62.0 };
    let hip = Vec3::new(sx * 40.0, -24.0, z);
    let foot = Vec3::new(sx * 46.0, GROUND, z);
    let bend = Vec3::new(0.0, 0.25, if front { 1.0 } else { -1.0 });
    let (knee, foot) = ik_knee(hip, foot, LEG_LEN, LEG_LEN, bend);
    LegRest {
        hip,
        knee,
        foot,
        normal: (foot - hip).cross(bend).normalize(),
        parent: if front { CHEST } else { HIP },
    }
}

const JAW_PIVOT: Vec3 = Vec3::new(0.0, -9.0, 258.0);

fn pivots() -> Vec<Vec3> {
    let mut p = vec![Vec3::ZERO; BONES];
    p[BODY] = spine_pivot(0.0);
    p[CHEST] = spine_pivot(55.0);
    for k in 0..5 {
        p[NECK + k] = spine_pivot(JOINTS[8 + k]);
        p[TAIL + k] = spine_pivot(JOINTS[5 - k]);
    }
    p[HEAD] = spine_pivot(243.0);
    p[JAW] = JAW_PIVOT;
    p[HIP] = spine_pivot(-55.0);
    p[CLUB] = spine_pivot(-262.0);
    for leg in 0..4 {
        let l = leg_rest(leg);
        p[LEG + leg * 2] = l.hip;
        p[LEG + leg * 2 + 1] = l.knee;
    }
    p
}

fn bump(t: f32, a: f32, b: f32, ease: f32) -> f32 {
    smoothstep(a, a + ease, t) * (1.0 - smoothstep(b - ease, b, t))
}

struct GrazerAnim {
    piv: Vec<Vec3>,
    phase: f32,
    speed: f32,
    prev_cycle: f32,
}

const CYCLE: f32 = 78.0;

impl Animator for GrazerAnim {
    fn pose(&mut self, t: f32, dt: f32, ctl: &BeastCtl, out: &mut [Affine3A], info: &mut AnimOut) {
        let cyc = if ctl.calm { 0.0 } else { t.rem_euclid(CYCLE) };
        let graze = bump(cyc, 18.0, 38.0, 6.0);
        let lift = bump(cyc, 48.0, 65.0, 5.0);
        if !ctl.calm {
            if self.prev_cycle < 18.0 && cyc >= 18.0 {
                info.banner = Some("THUNDERMOSS STOOPS TO GRAZE!");
            }
            if self.prev_cycle < 48.0 && cyc >= 48.0 {
                info.banner = Some("THUNDERMOSS REARS HIS HEAD AND LASHES HIS TAIL!");
            }
        }
        self.prev_cycle = cyc;

        // He stands still to eat.
        self.speed = damp(self.speed, 1.0 - 0.85 * graze, 1.5, dt);
        self.phase += dt * self.speed / GAIT_PERIOD;
        let ph = self.phase * TAU;
        info.scroll = Vec3::new(0.0, 0.0, -STRIDE / (DUTY * GAIT_PERIOD) * self.speed);

        let sh = ctl.shudder;
        let p = &self.piv;
        let gait = self.speed;
        let bob = 2.4 * gait * (2.0 * ph).sin() + 1.0 * (t * 0.6).sin() + 0.55 * sh * (t * 11.0).sin();
        let body_rot = Quat::from_rotation_z(0.022 * gait * ph.sin() + 0.02 * sh * (t * 8.0).sin())
            * Quat::from_rotation_x(0.012 * gait * (2.0 * ph + 0.8).sin() + 0.05 * graze - 0.03 * lift)
            * Quat::from_rotation_y(0.02 * gait * (ph + 1.0).sin());
        let body = Affine3A::from_rotation_translation(body_rot, p[BODY] + Vec3::Y * bob);
        out[BODY] = body;
        info.rumble = sh;

        // Shoulders and hips roll against each other as he walks.
        out[CHEST] = child(
            body,
            p[BODY],
            p[CHEST],
            Quat::from_rotation_z(0.035 * gait * (ph + 0.5).sin()),
        );
        out[HIP] = child(
            body,
            p[BODY],
            p[HIP],
            Quat::from_rotation_z(-0.035 * gait * (ph + 0.5).sin()),
        );

        // Neck: held high, swept low to graze, reared up to bellow.
        let mut parent = (out[CHEST], p[CHEST]);
        let mut total_pitch = 0.0;
        for k in 0..5 {
            let kf = k as f32;
            let pitch = 0.1 - 0.185 * graze + 0.07 * lift + 0.012 * gait * (2.0 * ph - kf * 0.5).sin();
            let yaw = 0.03 * (t * 0.45 + kf * 0.7).sin() + 0.06 * graze * (t * 0.7 + kf * 0.5).sin();
            total_pitch += pitch;
            let g = child(
                parent.0,
                parent.1,
                p[NECK + k],
                Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-pitch),
            );
            out[NECK + k] = g;
            parent = (g, p[NECK + k]);
        }
        // The head stays close to level so the road around the crown is drivable.
        let head = child(
            parent.0,
            parent.1,
            p[HEAD],
            Quat::from_rotation_x(total_pitch * 0.85) * Quat::from_rotation_z(0.04 * (t * 0.5).sin()),
        );
        out[HEAD] = head;
        let chew = 0.04 + 0.2 * graze * (0.5 + 0.5 * (t * 4.5).sin()) + 0.3 * lift * (0.5 + 0.5 * (t * 1.3).sin());
        out[JAW] = child(head, p[HEAD], p[JAW], Quat::from_rotation_x(chew));

        // Tail: droops, and a wave runs down it.
        let mut parent = (out[HIP], p[HIP]);
        let swing = 0.05 + 0.085 * lift;
        let rate = 0.8 + 1.1 * lift;
        for k in 0..6 {
            let kf = k as f32;
            let bone = if k < 5 { TAIL + k } else { CLUB };
            let yaw = swing * (t * rate - kf * 0.75).sin() + 0.02 * gait * (ph - kf * 0.4).sin();
            let pitch = -0.035 + 0.012 * (t * 0.5 + kf).sin() + 0.02 * lift;
            let g = child(
                parent.0,
                parent.1,
                p[bone],
                Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch),
            );
            out[bone] = g;
            parent = (g, p[bone]);
        }

        // Legs: a slow four-beat walk.
        for leg in 0..4 {
            let rest = leg_rest(leg);
            let bone = LEG + leg * 2;
            let offsets = [0.25, 0.75, 0.5, 0.0];
            let c = (self.phase + offsets[leg]).rem_euclid(1.0);
            let mut foot = rest.foot;
            if c < DUTY {
                foot.z += STRIDE * (0.5 - c / DUTY);
                // The ground shakes as each foot comes down.
                info.rumble = info.rumble.max(0.22 * (1.0 - c / 0.05).max(0.0) * gait);
            } else {
                let s = (c - DUTY) / (1.0 - DUTY);
                foot.z += STRIDE * (-0.5 + smooth(s));
                foot.y += 24.0 * (s * PI).sin();
            }
            let hip = out[rest.parent].transform_point3(rest.hip - p[rest.parent]);
            let bend = Vec3::new(0.0, 0.25, if leg < 2 { 1.0 } else { -1.0 });
            let (knee, foot) = ik_knee(hip, foot, LEG_LEN, LEG_LEN, bend);
            let normal = (foot - hip).cross(bend).normalize();
            out[bone] = aim_bone(hip, knee, rest.knee - rest.hip, rest.normal, normal);
            out[bone + 1] = aim_bone(knee, foot, rest.foot - rest.knee, rest.normal, normal);
        }
    }
}

pub fn build() -> Built {
    let mut rng = Rng::new(0x7A05);
    let piv = pivots();

    // ---------------------------------------------------------------- track
    let on_back = |x: f32, z: f32| {
        let n = top_normal(x, z);
        Ctrl::new(
            Vec3::new(x, top(x, z) + 0.9, z),
            (n * 0.45 + Vec3::Y * 0.55).normalize(),
            0,
        )
    };
    let body = |x: f32, z: f32| on_back(x, z).sh(4.0, 4.0).edges(Edge::Soft, Edge::Soft);
    let spine = |x: f32, z: f32| {
        // Narrow lanes lean with the curve of the neck and tail.
        let mut c = on_back(x, z).half(5.2).sh(1.0, 1.0).surf(1);
        c.up = (top_normal(x, z) * 0.7 + Vec3::Y * 0.3).normalize();
        c
    };
    let bend = |x: f32, z: f32| {
        let mut c = on_back(x, z).half(6.2).sh(1.2, 1.2).surf(1);
        c.up = Vec3::Y;
        c
    };
    let ctrl = vec![
        body(30.0, -22.0), // 0: start line
        body(34.0, 8.0),
        body(26.0, 38.0),
        body(31.0, 70.0),
        spine(17.0, 106.0), // 4: up the neck
        spine(11.5, 150.0),
        spine(10.0, 190.0),
        spine(10.5, 224.0),
        bend(15.5, 252.0), // 8: around the crown
        bend(19.0, 282.0),
        bend(13.5, 306.0),
        bend(0.0, 316.0),
        bend(-13.5, 306.0),
        bend(-19.0, 282.0),
        bend(-15.5, 252.0),
        spine(-10.5, 224.0), // 15: down the neck
        spine(-10.0, 190.0),
        spine(-11.5, 150.0),
        spine(-17.0, 106.0),
        body(-30.0, 68.0), // 19: the far side of the back
        body(-39.0, 32.0),
        body(-27.0, -5.0),
        body(-35.0, -40.0),
        body(-30.0, -72.0),
        spine(-18.0, -104.0), // 24: out along the tail
        spine(-11.0, -160.0),
        spine(-10.0, -228.0),
        bend(-12.5, -266.0), // 27: around the club
        bend(-20.0, -294.0),
        bend(-14.5, -316.0),
        bend(0.0, -325.0),
        bend(14.5, -316.0),
        bend(20.0, -294.0),
        bend(12.5, -266.0),
        spine(10.0, -228.0), // 34: back up the tail
        spine(11.0, -160.0),
        spine(18.0, -104.0),
        body(30.0, -66.0), // 37
    ];
    let features = Features {
        boosts: vec![
            (1.6, 1.0),
            (6.3, 0.0),
            (14.4, 0.0),
            (20.4, -2.0),
            (25.4, 0.0),
            (33.5, 0.0),
            (35.6, 0.0),
        ],
        item_rows: vec![2.4, 5.6, 16.5, 21.5, 24.6, 34.6],
        ramps: vec![(3.35, 13.0, 2.6), (22.6, 13.0, 2.6)],
        critters: vec![1.4, 19.6, 21.9],
        geysers: vec![],
    };
    let skin_of = |_: u8, p: Vec3| spine_skin(p.z);
    let settle = |_: u8, _: u8, p: Vec3| Vec3::new(p.x, top(p.x, p.z) + 0.9, p.z);
    let track = Track::build(&ctrl, &features, &skin_of, &settle);
    let road: Vec<(Vec3, f32)> = track
        .samples
        .iter()
        .map(|s| (s.c, s.half + s.sh[0].max(s.sh[1])))
        .collect();
    // Distance from a point on the hide to the edge of the road (negative inside).
    let clearance = |p: Vec3| -> f32 {
        let mut best = f32::MAX;
        for &(c, lim) in &road {
            best = best.min(c.distance(p) - lim);
        }
        best
    };

    // ---------------------------------------------------------------- hide
    let mut hide = MeshBuilder::new();
    let moss_a = lin(0x74b83e);
    let moss_b = lin(0x4c9440);
    let skin_a = lin(0x5d8a8c);
    let skin_b = lin(0x3c6670);
    let belly = lin(0xd8d3b4);
    let bone = lin(0xefe6d2);
    let sides = 28;
    let mut zs: Vec<f32> = Vec::new();
    let mut z = PROFILE[0].0;
    while z < PROFILE[PROFILE.len() - 1].0 {
        zs.push(z);
        // Finer slices where the shape changes fastest: the club and the head.
        z += if !(-270.0..=240.0).contains(&z) { 4.0 } else { 7.0 };
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
                    let n = noise2(z * 0.035 + 7.0, a * 2.2, 0, 5);
                    let upness = s;
                    // Moss and soil lie on top; the hide shows on the flanks; the belly is pale.
                    let mut col = if upness > 0.25 {
                        let lush = mix(moss_a, moss_b, noise2(p.x * 0.06, z * 0.06, 0, 9));
                        mix(
                            mix(skin_a, skin_b, n),
                            lush,
                            smoothstep(0.25, 0.5, upness + (n - 0.5) * 0.3),
                        )
                    } else if upness > -0.45 {
                        let stripe = (z * 0.09 + n * 3.0).sin() * 0.5 + 0.5;
                        mix(skin_a, skin_b, stripe * 0.7 + n * 0.3)
                    } else {
                        mix(mix(skin_a, skin_b, n), belly, smoothstep(-0.45, -0.75, upness))
                    };
                    if upness > 0.3 {
                        // Lumpy ground, smoothed flat where the road runs.
                        let free = smoothstep(0.0, 9.0, clearance(p));
                        p.y += (fbm2(p.x * 0.07, z * 0.07, 3, 3) - 0.35) * 5.0 * free * upness;
                        col = mix(shade(col, 0.8), col, free);
                    }
                    (
                        p,
                        shade(col, 0.92 + 0.16 * noise2(z * 0.3, a * 9.0, 0, 13)),
                        spine_skin(z),
                    )
                })
                .collect()
        })
        .collect();
    for w in rings.windows(2) {
        for k in 0..sides {
            let k1 = (k + 1) % sides;
            let corners = [w[0][k], w[0][k1], w[1][k1], w[1][k]];
            let v: Vec<u32> = corners.iter().map(|&(p, c, s)| hide.vs(p, c, s)).collect();
            hide.quad(v[0], v[1], v[2], v[3]);
        }
    }

    // ---------------------------------------------------------------- head
    hide.cur = Skin::one(HEAD);
    for sx in [-1.0f32, 1.0] {
        // Eyes, low on the sides of the head.
        let eye = Vec3::new(sx * 37.5, 8.0, 296.0);
        hide.ico_with(eye, Vec3::splat(5.2), 2, |d| {
            let c = if d.x * sx > 0.72 {
                lin(0x15121a)
            } else if d.x * sx > 0.45 {
                lin(0xe0a82e)
            } else {
                lin(0xf7f1e1)
            };
            (1.0, c)
        });
        hide.ico(
            eye + Vec3::new(-sx * 1.0, 3.2, 0.0),
            Vec3::new(5.0, 3.4, 6.2),
            1,
            skin_b,
        );
        // Nostrils.
        hide.ball(Vec3::new(sx * 8.0, 9.5, 338.0), 2.0, 1, lin(0x2a3a40));
        // Great horns sweeping out and back from the brow.
        let base = Vec3::new(sx * 37.0, 12.0, 254.0);
        let path: Vec<(Vec3, f32)> = (0..=6)
            .map(|i| {
                let f = i as f32 / 6.0;
                let p = base + Vec3::new(sx * (22.0 * f + 10.0 * f * f), 46.0 * f - 12.0 * f * f, -30.0 * f * f);
                (p, 6.5 * (1.0 - f * 0.92))
            })
            .collect();
        hide.tube(&path, 7, bone, true);
    }
    // Teeth along the upper lip.
    for k in 0..22 {
        let a = (k as f32 / 21.0 - 0.5) * PI;
        let (rx, _, _) = profile(300.0);
        let p = Vec3::new(a.sin() * rx * 0.86, -8.5, 292.0 + a.cos() * 50.0);
        hide.cone(p, p - Vec3::Y * 4.0, 2.0, 4, bone);
    }
    // Lower jaw.
    hide.cur = Skin::one(JAW);
    let jaw_rings: Vec<Vec<(Vec3, Col)>> = [
        (258.0, 30.0, 6.0),
        (285.0, 33.0, 7.0),
        (315.0, 25.0, 6.0),
        (338.0, 11.0, 4.0),
        (344.0, 1.0, 1.0),
    ]
    .iter()
    .map(|&(z, rx, ry)| {
        MeshBuilder::ring(
            Vec3::new(0.0, -13.0, z),
            Vec3::Z,
            Vec3::Y,
            rx,
            ry,
            12,
            mix(skin_a, belly, 0.6),
        )
    })
    .collect();
    hide.loft(&jaw_rings, true, true);

    // ---------------------------------------------------------------- plates
    // A ridge of bone plates runs down the spine between the two lanes.
    let mut z = -250.0f32;
    let mut k = 0;
    while z < 236.0 {
        let on_body = (-92.0..=96.0).contains(&z);
        let h = if on_body {
            17.0 + 7.0 * (z * 0.05).cos()
        } else {
            8.5 + 2.5 * (z * 0.11).sin()
        };
        let w = if on_body { 12.0 } else { 6.5 };
        hide.cur = spine_skin(z);
        hide.xf = Affine3A::from_translation(Vec3::new(0.0, top(0.0, z) - 1.0, z));
        let tone = if k % 2 == 0 { bone } else { lin(0xd9c9a8) };
        flora::plate(&mut hide, h, w, tone);
        z += if on_body { 15.0 } else { 9.5 };
        k += 1;
    }
    // Spikes around the club.
    for k in 0..9 {
        let a = (k as f32 / 8.0 - 0.5) * PI * 1.25;
        let zc = -300.0;
        let (rx, _, cy) = profile(zc - a.cos() * 12.0);
        let base = Vec3::new(a.sin() * rx * 0.9, cy, zc - a.cos() * 32.0);
        let out = Vec3::new(a.sin(), 0.12, -a.cos()).normalize();
        hide.cur = Skin::one(CLUB);
        hide.xf = Affine3A::IDENTITY;
        hide.cone(base, base + out * 26.0, 6.0, 6, bone);
    }
    // A crest of plates inside each hairpin.
    for (zc, skin) in [(283.0, Skin::one(HEAD)), (-296.0, Skin::one(CLUB))] {
        for k in 0..3 {
            let z = zc + (k as f32 - 1.0) * 11.0;
            hide.cur = skin;
            hide.xf = Affine3A::from_translation(Vec3::new(0.0, top(0.0, z) - 1.0, z));
            flora::plate(&mut hide, 11.0 - (k as f32 - 1.0).abs() * 3.0, 7.0, bone);
        }
    }
    hide.xf = Affine3A::IDENTITY;

    // ---------------------------------------------------------------- legs
    for leg in 0..4 {
        let rest = leg_rest(leg);
        let b = LEG + leg * 2;
        let up = Vec3::Z;
        hide.cur = Skin::one(b);
        limb(
            &mut hide,
            rest.hip,
            rest.knee,
            up,
            &[
                (-0.25, 17.0, 20.0),
                (0.0, 19.0, 22.0),
                (0.5, 16.0, 18.0),
                (1.0, 14.0, 15.0),
            ],
            10,
            skin_a,
            skin_b,
        );
        hide.cur = Skin::one(b + 1);
        limb(
            &mut hide,
            rest.knee,
            rest.foot,
            up,
            &[
                (0.0, 14.0, 15.0),
                (0.6, 12.5, 13.5),
                (0.9, 15.0, 17.0),
                (1.0, 16.0, 19.0),
            ],
            10,
            skin_a,
            skin_b,
        );
        hide.ball(rest.knee, 15.0, 1, skin_a);
        // Toenails.
        for t in 0..3 {
            let a = (t as f32 - 1.0) * 0.6;
            let toe = rest.foot + Vec3::new(a.sin() * 15.0, 3.0, a.cos() * 17.0);
            hide.ico(toe, Vec3::new(4.5, 4.0, 4.5), 0, bone);
        }
    }

    // ---------------------------------------------------------------- flora
    let mut plants = MeshBuilder::new();
    let greens = [0x3f9a46u32, 0x2f8a4f, 0x62b04a, 0x8bc34a];
    let caps = [0xe6483cu32, 0xf08a2e, 0xb05ad8, 0xf2d24a];
    let petals = [0xff5d8f, 0xffd23f, 0xf7f7f2, 0xc86bfa, 0x6ec6ff];
    // Random spot on the upper hide in a z range, as `(point, outward normal)`.
    let spot = |rng: &mut Rng, z0: f32, z1: f32, spread: f32| -> (Vec3, Vec3) {
        let z = rng.range(z0, z1);
        let rx = profile(z).0;
        let x = rng.sym(rx * spread);
        (Vec3::new(x, top(x, z), z), top_normal(x, z))
    };
    let place = |plants: &mut MeshBuilder, rng: &mut Rng, p: Vec3, sink: f32| {
        plants.cur = spine_skin(p.z);
        plants.xf = Affine3A::from_rotation_translation(Quat::from_rotation_y(rng.angle()), p - Vec3::Y * sink);
    };
    // The forest on his back.
    let mut planted = 0;
    let mut tries = 0;
    while planted < 230 && tries < 6000 {
        tries += 1;
        let (p, n) = spot(&mut rng, -96.0, 100.0, 0.9);
        if clearance(p) < 3.0 || p.x.abs() < 5.0 || n.y < 0.55 {
            continue;
        }
        place(&mut plants, &mut rng, p, 0.6);
        let leaf = lin(greens[rng.below(greens.len())]);
        match rng.below(5) {
            0 | 1 => flora::conifer(&mut plants, 11.0 + 9.0 * rng.f(), &mut rng),
            2 | 3 => flora::broadleaf(&mut plants, 10.0 + 8.0 * rng.f(), leaf, &mut rng),
            _ => flora::mushroom(
                &mut plants,
                5.0 + 5.0 * rng.f(),
                lin(caps[rng.below(caps.len())]),
                &mut rng,
            ),
        }
        planted += 1;
    }
    // Undergrowth everywhere there is room, neck and tail included.
    let mut flowers: Vec<Vec3> = Vec::new();
    for _ in 0..1500 {
        let (p, n) = spot(&mut rng, -330.0, 335.0, 0.93);
        if clearance(p) < 0.8 || n.y < 0.35 {
            continue;
        }
        place(&mut plants, &mut rng, p, 0.2);
        let green = lin(greens[rng.below(greens.len())]);
        match rng.below(9) {
            0 | 1 => flora::fern(&mut plants, 2.6 + 1.8 * rng.f(), green, &mut rng),
            2 => flora::bush(&mut plants, 2.2 + 2.0 * rng.f(), green, &mut rng),
            3 => flora::mushroom(
                &mut plants,
                1.6 + 2.2 * rng.f(),
                lin(caps[rng.below(caps.len())]),
                &mut rng,
            ),
            4 => flora::boulders(&mut plants, 1.4 + 2.2 * rng.f(), lin(0x8f9a8c), &mut rng),
            5 | 6 => {
                flora::flower(
                    &mut plants,
                    1.8 + 1.4 * rng.f(),
                    lin(petals[rng.below(petals.len())]),
                    &mut rng,
                );
                flowers.push(p);
            }
            _ => flora::tuft(&mut plants, 1.8 + 1.6 * rng.f(), lin(0x9ad255), &mut rng),
        }
    }
    // Vines and hanging moss along his flanks, and a beard of it under the jaw.
    for _ in 0..170 {
        let z = rng.range(-250.0, 240.0);
        let (rx, _, cy) = profile(z);
        let sx = if rng.chance(0.5) { 1.0 } else { -1.0 };
        plants.cur = spine_skin(z);
        plants.xf = Affine3A::from_translation(Vec3::new(sx * rx * 0.995, cy + rng.range(-2.0, 4.0), z));
        flora::strand(
            &mut plants,
            10.0 + 18.0 * rng.f(),
            lin(if rng.chance(0.5) { 0x4c9440 } else { 0x7fae5a }),
            &mut rng,
        );
    }
    plants.cur = Skin::one(JAW);
    for _ in 0..24 {
        plants.xf = Affine3A::from_translation(Vec3::new(rng.sym(20.0), -20.0, rng.range(270.0, 325.0)));
        flora::strand(&mut plants, 8.0 + 12.0 * rng.f(), lin(0x7fae5a), &mut rng);
    }
    // Trees on the crown and the club, inside the hairpins.
    for (zc, skin) in [(268.0, Skin::one(HEAD)), (-282.0, Skin::one(CLUB))] {
        for _ in 0..5 {
            let p = Vec3::new(rng.sym(4.0), 0.0, zc + rng.sym(12.0));
            plants.cur = skin;
            plants.xf = Affine3A::from_translation(Vec3::new(p.x, top(p.x, p.z) - 0.5, p.z));
            flora::mushroom(
                &mut plants,
                5.0 + 4.0 * rng.f(),
                lin(caps[rng.below(caps.len())]),
                &mut rng,
            );
        }
    }
    plants.xf = Affine3A::IDENTITY;

    // ---------------------------------------------------------------- road
    let style = TrackStyle {
        road: vec![[lin(0xa98058), lin(0x9d744e)], [lin(0xcdb892), lin(0xc0aa84)]],
        curb: [lin(0xf4ecd8), lin(0xd9653b)],
        line: lin(0xf6ecd0),
        shoulder: [lin(0x77b44a), lin(0x68a542)],
        skirt: lin(0x5d4a38),
        wall: [lin(0xefe6d2), lin(0xd9c9a8)],
        post: lin(0x6e4b2f),
    };
    let (ribbon, glow) = track.build_mesh(&style);

    // ---------------------------------------------------------------- fauna
    let mut fauna = Vec::new();
    for (k, p) in flowers.iter().step_by((flowers.len() / 26).max(1)).enumerate() {
        fauna.push(Fauna {
            kind: FaunaKind::Butterfly,
            skin: spine_skin(p.z),
            rest: *p + Vec3::Y * 2.0,
            seed: k as f32 * 1.9,
        });
    }
    for k in 0..14 {
        let z = rng.range(-260.0, 300.0);
        fauna.push(Fauna {
            kind: FaunaKind::Gull,
            skin: spine_skin(z),
            rest: Vec3::new(rng.sym(50.0), top(0.0, z) + rng.range(30.0, 70.0), z),
            seed: k as f32 * 2.3 + rng.f(),
        });
    }

    Built {
        rest: piv.iter().map(|&p| pivot(p)).collect(),
        track,
        layers: vec![
            Layer {
                mesh: hide,
                mat: Mat::Matte,
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
        animator: Box::new(GrazerAnim {
            piv,
            phase: 0.0,
            speed: 1.0,
            prev_cycle: 0.0,
        }),
        env: EnvDef {
            ground: Ground::Plain { level: GROUND },
            sky_top: 0x2f7fe0,
            sky_horizon: 0xd6eeff,
            fog: 0xcfe6f4,
            fog_start: 1100.0,
            fog_end: 6200.0,
            sun_dir: Vec3::new(-0.5, -0.68, 0.36),
            sun_color: 0xfff1d6,
            sun_lux: 11000.0,
            ambient: 1500.0,
            cloud_height: (-60.0, 260.0),
        },
        view: ShowView {
            center: Vec3::new(0.0, -20.0, 10.0),
            radius: 640.0,
            height: 130.0,
        },
        critter_color: 0x6a4fd0,
    }
}
