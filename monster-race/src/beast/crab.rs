//! Old Brine, the island crab. A palm-covered island rides his shell; the
//! track circles the beach, climbs the summit, then runs out along one arm,
//! leaps between his claws and comes home along the other.

use super::{
    AnimOut, Animator, BeastCtl, Built, EnvDef, Fauna, FaunaKind, Ground, Layer, Mat, ShowView, aim_bone, child, flora,
    ik_knee, limb, pivot,
};
use crate::{
    meshkit::{Col, MeshBuilder, Skin},
    track::{Ctrl, Edge, Features, Track, TrackStyle},
    util::{Rng, damp, fbm2, lin, mix, noise2, shade, smoothstep},
};
use bevy::{math::Affine3A, prelude::*};
use std::f32::consts::{PI, TAU};

// Shell outline: a superellipse, wider than it is long.
const A: f32 = 150.0;
const B: f32 = 105.0;
const POW: f32 = 2.6;
const DOME: f32 = 38.0;
pub const SEA: f32 = -14.0;
pub const FLOOR: f32 = -62.0;

// Bones.
const SHELL: usize = 0;
const ARM: usize = 1; // + side * 4: upper, fore, claw, pincer
const LEG: usize = 9; // + (side * 4 + k) * 2: upper, lower
const EYE: usize = 25; // + side
const JAW: usize = 27; // + side
const BONES: usize = 29;

const LEG_UPPER: f32 = 95.0;
const LEG_LOWER: f32 = 112.0;
const GAIT_PERIOD: f32 = 5.2;
const DUTY: f32 = 0.62;
const STRIDE: f32 = 34.0;

// Arm joints on the +X side; the other arm mirrors them.
const SHOULDER: Vec3 = Vec3::new(98.0, 2.0, 92.0);
const ELBOW: Vec3 = Vec3::new(142.0, 12.0, 178.0);
const WRIST: Vec3 = Vec3::new(104.0, 22.0, 248.0);
const TIP: Vec3 = Vec3::new(22.0, 30.0, 268.0);
const PINCER: Vec3 = Vec3::new(78.0, 12.0, 260.0);
const ARM_RY: f32 = 12.2;

fn mirror(p: Vec3, side: usize) -> Vec3 {
    if side == 0 { p } else { Vec3::new(-p.x, p.y, p.z) }
}

fn rho(x: f32, z: f32) -> f32 {
    ((x.abs() / A).powf(POW) + (z.abs() / B).powf(POW)).powf(1.0 / POW)
}

/// Height of the bare carapace.
fn dome(x: f32, z: f32) -> f32 {
    let r = rho(x, z).min(1.0);
    DOME * (1.0 - r.powf(2.2)).powf(0.8)
}

fn gauss(x: f32, z: f32, cx: f32, cz: f32, radius: f32) -> f32 {
    let d2 = (x - cx).powi(2) + (z - cz).powi(2);
    (-d2 / (radius * radius)).exp()
}

/// How much island (sand, soil) covers the shell here, 0..1.
fn cover(x: f32, z: f32) -> f32 {
    smoothstep(0.98, 0.76, rho(x, z))
}

/// Height of the island before the road is cut into it.
fn natural(x: f32, z: f32) -> f32 {
    let peak = 25.0 * gauss(x, z, -5.0, -12.0, 34.0);
    let knolls = 8.0 * gauss(x, z, 66.0, 50.0, 24.0) + 7.0 * gauss(x, z, -108.0, 2.0, 26.0);
    let dunes = (fbm2(x * 0.03 + 9.0, z * 0.03 + 4.0, 3, 7) - 0.5) * 7.0;
    let lagoon = -6.5 * gauss(x, z, LAGOON.0, LAGOON.1, 19.0);
    dome(x, z) + (1.2 + peak + knolls + dunes + lagoon) * cover(x, z)
}

const LAGOON: (f32, f32) = (74.0, -10.0);

fn arm_ry(seg: usize, f: f32) -> f32 {
    match seg {
        0 | 1 => ARM_RY * (1.0 + 0.08 * (f * PI).sin()),
        _ => {
            // Claw: a swollen palm tapering into the fixed finger.
            let pts = [(0.0, 12.0), (0.35, 17.0), (0.6, 11.5), (0.9, 9.0), (1.0, 3.5)];
            let mut v = pts[pts.len() - 1].1;
            for w in pts.windows(2) {
                if f <= w[1].0 {
                    v = w[0].1 + (w[1].1 - w[0].1) * ((f - w[0].0) / (w[1].0 - w[0].0));
                    break;
                }
            }
            v
        }
    }
}

fn arm_rx(seg: usize, f: f32) -> f32 {
    match seg {
        0 => 17.0 * (1.0 + 0.1 * (f * PI).sin()),
        1 => 16.0 * (1.0 + 0.1 * (f * PI).sin()),
        _ => arm_ry(2, f) * 1.42,
    }
}

fn arm_joints(side: usize) -> [Vec3; 4] {
    [
        mirror(SHOULDER, side),
        mirror(ELBOW, side),
        mirror(WRIST, side),
        mirror(TIP, side),
    ]
}

/// A point riding on top of arm segment `seg` at fraction `f`.
fn arm_top(side: usize, seg: usize, f: f32) -> Vec3 {
    let j = arm_joints(side);
    j[seg].lerp(j[seg + 1], f) + Vec3::Y * (arm_ry(seg, f) + 1.0)
}

/// Skin weights for a rest-pose point on or near an arm.
fn arm_skin(side: usize, p: Vec3) -> Skin {
    let j = arm_joints(side);
    let bone = ARM + side * 4;
    // Arc length of the closest point on the joint polyline.
    let mut best = (f32::MAX, 0.0);
    let mut acc = 0.0;
    let mut joints_s = [0.0f32; 4];
    for s in 0..3 {
        let (a, b) = (j[s], j[s + 1]);
        let len = a.distance(b);
        let f = ((p - a).dot(b - a) / (len * len)).clamp(0.0, 1.0);
        let d = p.distance(a.lerp(b, f));
        if d < best.0 {
            best = (d, acc + f * len);
        }
        acc += len;
        joints_s[s + 1] = acc;
    }
    let s = best.1;
    const ZONE: f32 = 18.0;
    if s < ZONE {
        Skin::two(SHELL, bone, smoothstep(-ZONE, ZONE, s))
    } else if s < (joints_s[1] + joints_s[2]) * 0.5 {
        Skin::two(bone, bone + 1, smoothstep(joints_s[1] - ZONE, joints_s[1] + ZONE, s))
    } else {
        Skin::two(
            bone + 1,
            bone + 2,
            smoothstep(joints_s[2] - ZONE, joints_s[2] + ZONE, s),
        )
    }
}

fn rim_x(z: f32) -> f32 {
    A * (1.0 - (z.abs() / B).powf(POW)).max(0.0).powf(1.0 / POW)
}

#[derive(Clone, Copy)]
struct LegRest {
    hip: Vec3,
    knee: Vec3,
    foot: Vec3,
    normal: Vec3,
}

fn leg_rest(side: usize, k: usize) -> LegRest {
    let zs = [52.0, 16.0, -24.0, -62.0];
    let z = zs[k];
    let sx = if side == 0 { 1.0 } else { -1.0 };
    let rim = rim_x(z);
    let hip = Vec3::new(sx * rim * 0.78, -9.0, z);
    let foot = Vec3::new(sx * (rim + 96.0), FLOOR, z * 1.5);
    let bend = Vec3::new(sx * 0.3, 1.0, 0.0);
    let (knee, foot) = ik_knee(hip, foot, LEG_UPPER, LEG_LOWER, bend);
    LegRest {
        hip,
        knee,
        foot,
        normal: (foot - hip).cross(bend).normalize(),
    }
}

fn eye_pivot(side: usize) -> Vec3 {
    mirror(Vec3::new(34.0, 38.0, 106.0), side)
}

fn jaw_pivot(side: usize) -> Vec3 {
    mirror(Vec3::new(13.0, -6.0, 102.0), side)
}

fn pivots() -> Vec<Vec3> {
    let mut p = vec![Vec3::ZERO; BONES];
    for side in 0..2 {
        let b = ARM + side * 4;
        p[b] = mirror(SHOULDER, side);
        p[b + 1] = mirror(ELBOW, side);
        p[b + 2] = mirror(WRIST, side);
        p[b + 3] = mirror(PINCER, side);
        for k in 0..4 {
            let l = leg_rest(side, k);
            p[LEG + (side * 4 + k) * 2] = l.hip;
            p[LEG + (side * 4 + k) * 2 + 1] = l.knee;
        }
        p[EYE + side] = eye_pivot(side);
        p[JAW + side] = jaw_pivot(side);
    }
    p
}

/// Smooth 0→1→0 pulse over `[a, b]` with `ease` seconds of ramp at each end.
fn bump(t: f32, a: f32, b: f32, ease: f32) -> f32 {
    smoothstep(a, a + ease, t) * (1.0 - smoothstep(b - ease, b, t))
}

struct CrabAnim {
    piv: Vec<Vec3>,
    phase: f32,
    speed: f32,
    prev_cycle: f32,
    eye_dir: [Vec3; 2],
}

const CYCLE: f32 = 68.0;

impl Animator for CrabAnim {
    fn pose(&mut self, t: f32, dt: f32, ctl: &BeastCtl, out: &mut [Affine3A], info: &mut AnimOut) {
        let cyc = if ctl.calm { 0.0 } else { t.rem_euclid(CYCLE) };
        let claws = bump(cyc, 20.0, 34.0, 4.5);
        let dip = bump(cyc, 46.0, 60.0, 4.0);
        let dip_side = if (t / CYCLE).floor() as i32 % 2 == 0 { 1.0 } else { -1.0 };
        if !ctl.calm {
            if self.prev_cycle < 20.0 && cyc >= 20.0 {
                info.banner = Some("OLD BRINE RAISES HIS CLAWS!");
            }
            if self.prev_cycle < 46.0 && cyc >= 46.0 {
                info.banner = Some("THE TIDE COMES IN!");
            }
        }
        self.prev_cycle = cyc;

        // He slows to a shuffle while showing off.
        self.speed = damp(self.speed, 1.0 - 0.75 * claws.max(dip), 2.0, dt);
        self.phase += dt * self.speed / GAIT_PERIOD;
        let ph = self.phase * TAU;
        let walk_speed = STRIDE / (DUTY * GAIT_PERIOD) * self.speed;
        info.scroll = Vec3::new(-walk_speed, 0.0, 0.0);

        let sh = ctl.shudder;
        let bob =
            2.0 * self.speed * (2.0 * ph).sin() + 0.9 * (t * 0.8).sin() - 17.5 * dip + 0.55 * sh * (t * 11.0).sin();
        let roll = 0.04 * self.speed * (ph + 0.6).sin() + 0.03 * (t * 0.21).sin() - 0.075 * dip * dip_side
            + 0.02 * sh * (t * 8.0).sin();
        let pitch = 0.018 * self.speed * (2.0 * ph + 1.0).sin() + 0.018 * (t * 0.17 + 2.0).sin() - 0.03 * claws;
        let shell = Affine3A::from_rotation_translation(
            Quat::from_rotation_z(roll) * Quat::from_rotation_x(pitch),
            Vec3::new(0.0, bob, 0.0),
        );
        out[SHELL] = shell;
        info.rumble = sh;

        let p = &self.piv;
        for side in 0..2 {
            let sx = if side == 0 { 1.0 } else { -1.0 };
            let b = ARM + side * 4;
            let j = arm_joints(side);
            let lift = |dir: Vec3| dir.cross(Vec3::Y).normalize();
            let sway = (ph + side as f32 * PI).sin();
            let r_shoulder = Quat::from_axis_angle(
                lift(j[1] - j[0]),
                0.035 * (t * 0.7 + side as f32 * 2.0).sin() + 0.42 * claws,
            ) * Quat::from_rotation_y(sx * (0.04 * sway * self.speed + 0.11 * claws));
            let r_elbow = Quat::from_axis_angle(lift(j[2] - j[1]), 0.03 * (t * 0.9 + 1.0).sin() + 0.12 * claws);
            let r_wrist =
                Quat::from_axis_angle(lift(j[3] - j[2]), 0.025 * (t * 1.1 + side as f32).sin() + 0.08 * claws);
            let snap = 0.5 + 0.5 * (t * 5.5 + side as f32 * 1.3).sin();
            let open = 0.12 + 0.1 * (t * 0.6 + side as f32).sin().max(0.0) + 0.5 * claws * snap;
            out[b] = child(shell, Vec3::ZERO, p[b], r_shoulder);
            out[b + 1] = child(out[b], p[b], p[b + 1], r_elbow);
            out[b + 2] = child(out[b + 1], p[b + 1], p[b + 2], r_wrist);
            out[b + 3] = child(out[b + 2], p[b + 2], p[b + 3], Quat::from_rotation_z(sx * open));

            for k in 0..4 {
                let rest = leg_rest(side, k);
                let bone = LEG + (side * 4 + k) * 2;
                // Alternating tetrapods: neighbours and opposites are half a cycle apart.
                let offset = if (k + side) % 2 == 0 { 0.0 } else { 0.5 };
                let c = (self.phase + offset + k as f32 * 0.04).rem_euclid(1.0);
                let mut foot = rest.foot;
                if c < DUTY {
                    foot.x += STRIDE * (0.5 - c / DUTY);
                } else {
                    let s = (c - DUTY) / (1.0 - DUTY);
                    foot.x += STRIDE * (-0.5 + crate::util::smooth(s));
                    foot.y += 20.0 * (s * PI).sin();
                }
                let hip = shell.transform_point3(rest.hip);
                let bend = Vec3::new(sx * 0.3, 1.0, 0.0);
                let (knee, foot) = ik_knee(hip, foot, LEG_UPPER, LEG_LOWER, bend);
                let normal = (foot - hip).cross(bend).normalize();
                out[bone] = aim_bone(hip, knee, rest.knee - rest.hip, rest.normal, normal);
                out[bone + 1] = aim_bone(knee, foot, rest.foot - rest.knee, rest.normal, normal);
                if knee.y > SEA && foot.y < SEA {
                    info.splashes.push(knee.lerp(foot, (SEA - knee.y) / (foot.y - knee.y)));
                }
            }

            // Eyes follow the leader, or wander.
            let eye_world = shell.transform_point3(p[EYE + side]);
            let wander = Vec3::new((t * 0.31 + side as f32).sin() * 0.5, 0.15 + 0.2 * (t * 0.23).sin(), 1.0);
            let want = match ctl.look_at {
                Some(target) => shell.matrix3.transpose() * (target - eye_world),
                None => wander,
            }
            .normalize_or(Vec3::Z);
            // Keep the gaze within a cone around straight ahead.
            let want = (want + Vec3::Z * 0.55).normalize();
            self.eye_dir[side] = self.eye_dir[side].lerp(want, (dt * 4.0).min(1.0)).normalize_or(Vec3::Z);
            out[EYE + side] = child(
                shell,
                Vec3::ZERO,
                p[EYE + side],
                Quat::from_rotation_arc(Vec3::Z, self.eye_dir[side]),
            );
            out[JAW + side] = child(
                shell,
                Vec3::ZERO,
                p[JAW + side],
                Quat::from_rotation_y(sx * (0.2 + 0.2 * (t * 2.1).sin())),
            );
        }
    }
}

pub fn build() -> Built {
    let mut rng = Rng::new(0xC2AB);
    let piv = pivots();

    // ---------------------------------------------------------------- track
    let on_shell = |x: f32, z: f32| {
        let y = natural(x, z) + 0.8;
        let e = 2.0;
        let n = Vec3::new(
            dome(x - e, z) - dome(x + e, z),
            2.0 * e,
            dome(x, z - e) - dome(x, z + e),
        )
        .normalize();
        Ctrl::new(Vec3::new(x, y, z), (n * 0.22 + Vec3::Y * 0.78).normalize(), 0)
    };
    let beach = |x: f32, z: f32| on_shell(x, z).sh(5.0, 5.0).edges(Edge::Fall, Edge::Soft);
    let inland = |x: f32, z: f32| on_shell(x, z).half(7.5).sh(4.0, 4.0).edges(Edge::Soft, Edge::Soft);
    let arm = |side: usize, seg: usize, f: f32| {
        Ctrl::new(arm_top(side, seg, f), Vec3::Y, 1 + side as u8)
            .half(6.5)
            .sh(1.3, 1.3)
            .surf(1)
    };
    let ctrl = vec![
        beach(-20.0, -88.0), // 0: start line
        beach(35.0, -89.0),
        beach(88.0, -74.0),
        beach(121.0, -42.0),
        beach(133.0, -2.0),
        beach(126.0, 36.0),
        beach(109.0, 66.0).edges(Edge::Wall, Edge::Wall),
        arm(0, 0, 0.05), // 7
        arm(0, 0, 0.5),
        arm(0, 0, 1.0),
        arm(0, 1, 0.5),
        arm(0, 1, 1.0),
        arm(0, 2, 0.42),
        arm(0, 2, 0.93).gap(), // 13: the leap
        arm(1, 2, 0.93),
        arm(1, 2, 0.42),
        arm(1, 1, 1.0),
        arm(1, 1, 0.5),
        arm(1, 0, 1.0),
        arm(1, 0, 0.5),
        arm(1, 0, 0.05),
        inland(-109.0, 66.0).edges(Edge::Wall, Edge::Wall), // 21
        inland(-92.0, 38.0),
        inland(-60.0, 20.0),
        inland(-22.0, 24.0),
        inland(14.0, 12.0),
        inland(30.0, -16.0),
        inland(14.0, -42.0),
        inland(-22.0, -50.0),
        inland(-60.0, -40.0),
        inland(-96.0, -42.0),
        beach(-112.0, -62.0).edges(Edge::Soft, Edge::Fall),
        beach(-96.0, -82.0).edges(Edge::Soft, Edge::Fall),
        beach(-60.0, -88.0), // 33
    ];
    let features = Features {
        boosts: vec![
            (4.4, 0.0),
            (9.6, -2.0),
            (12.35, 0.0),
            (18.4, 2.0),
            (26.5, 0.0),
            (33.3, 3.0),
        ],
        item_rows: vec![2.2, 8.0, 17.0, 23.5, 28.6],
        ramps: vec![(3.3, 14.0, 2.6), (24.5, 12.0, 2.4)],
        critters: vec![1.2, 5.4, 22.6, 29.5],
        geysers: vec![],
    };
    let skin_of = |part: u8, p: Vec3| match part {
        0 => Skin::one(SHELL),
        1 => arm_skin(0, p),
        _ => arm_skin(1, p),
    };
    // On the shell the road hugs the island; on the arms it follows the spline.
    let settle = |a: u8, b: u8, p: Vec3| {
        if a == 0 && b == 0 {
            Vec3::new(p.x, natural(p.x, p.z) + 0.8, p.z)
        } else {
            p
        }
    };
    let track = Track::build(&ctrl, &features, &skin_of, &settle);

    // The road as seen by the terrain: shell samples only.
    let road: Vec<(Vec3, f32)> = track
        .samples
        .iter()
        .filter(|s| !s.gap && s.skin.j[0] as usize == SHELL && s.skin.w[0] > 0.5)
        .map(|s| (s.c, s.half + s.sh[0].max(s.sh[1])))
        .collect();
    let nearest = |x: f32, z: f32| -> (f32, Vec3, f32) {
        let mut best = (f32::MAX, Vec3::ZERO, 0.0);
        for &(c, lim) in &road {
            let d = (c.x - x).powi(2) + (c.z - z).powi(2);
            if d < best.0 {
                best = (d, c, lim);
            }
        }
        (best.0.sqrt(), best.1, best.2)
    };
    // Island height with the road cut in, and how strongly the cut applies.
    let terrain = |x: f32, z: f32| -> (f32, f32) {
        let (d, c, lim) = nearest(x, z);
        let w = smoothstep(lim + 11.0, lim + 0.5, d);
        let y = natural(x, z);
        (y + (c.y - 0.7 - y) * w, w)
    };

    // ---------------------------------------------------------------- shell
    let mut top = MeshBuilder::new();
    let mut hide = MeshBuilder::new();
    let (rings, sectors) = (60usize, 132usize);
    let shape = |r: f32, k: usize| -> (f32, f32) {
        let a = k as f32 / sectors as f32 * TAU;
        let (s, c) = a.sin_cos();
        let e = 2.0 / POW;
        (
            A * r * c.signum() * c.abs().powf(e),
            B * r * s.signum() * s.abs().powf(e),
        )
    };
    let sand = lin(0xecdca6);
    let wet_sand = lin(0xd8c48a);
    let grass = lin(0x79c24c);
    let grass2 = lin(0x4f9e3c);
    let rock = lin(0x8d877c);
    let shell_a = lin(0xd6452c);
    let shell_b = lin(0xef7a45);
    let shell_dark = lin(0x9c2f22);
    let ground_color = |x: f32, z: f32, y: f32, road_w: f32| -> Col {
        let r = rho(x, z);
        let cov = cover(x, z);
        let n = noise2(x * 0.08, z * 0.08, 0, 3);
        let bare = mix(mix(shell_a, shell_b, n), shell_dark, smoothstep(0.93, 1.0, r));
        if cov < 0.04 {
            return bare;
        }
        let lag = gauss(x, z, LAGOON.0, LAGOON.1, 24.0);
        let beachy = smoothstep(0.55, 0.72, r).max(lag * 1.4).min(1.0);
        let lush = mix(grass, grass2, noise2(x * 0.05 + 40.0, z * 0.05, 0, 9));
        let high = smoothstep(16.0, 25.0, y - dome(x, z));
        let mut c = mix(lush, mix(sand, wet_sand, n), beachy);
        c = mix(c, shade(rock, 0.85 + 0.3 * n), high);
        c = mix(c, shade(sand, 0.93), road_w * 0.8);
        mix(bare, c, smoothstep(0.04, 0.4, cov))
    };
    for i in 0..rings {
        let (r0, r1) = (i as f32 / rings as f32, (i + 1) as f32 / rings as f32);
        for k in 0..sectors {
            let corners = [shape(r0, k), shape(r1, k), shape(r1, k + 1), shape(r0, k + 1)];
            let (cx, cz) = (
                corners.iter().map(|c| c.0).sum::<f32>() / 4.0,
                corners.iter().map(|c| c.1).sum::<f32>() / 4.0,
            );
            let (cy, cw) = terrain(cx, cz);
            let color = shade(ground_color(cx, cz, cy, cw), 0.94 + 0.12 * rng.f());
            // Bare carapace goes to the glossy layer, island cover to the matte one.
            let layer = if cover(cx, cz) < 0.04 { &mut hide } else { &mut top };
            let v: Vec<u32> = corners
                .iter()
                .map(|&(x, z)| layer.vs(Vec3::new(x, terrain(x, z).0, z), color, Skin::one(SHELL)))
                .collect();
            layer.quad(v[0], v[3], v[2], v[1]);
            // Close the rim where the road cut lifts it off the underside.
            if i == rings - 1 {
                let (p0, p1) = (corners[1], corners[2]);
                let (t0, t1) = (terrain(p0.0, p0.1).0, terrain(p1.0, p1.1).0);
                let w = [
                    hide.vs(Vec3::new(p0.0, t0, p0.1), shell_dark, Skin::one(SHELL)),
                    hide.vs(Vec3::new(p1.0, t1, p1.1), shell_dark, Skin::one(SHELL)),
                    hide.vs(Vec3::new(p1.0, 0.0, p1.1), shell_dark, Skin::one(SHELL)),
                    hide.vs(Vec3::new(p0.0, 0.0, p0.1), shell_dark, Skin::one(SHELL)),
                ];
                hide.quad(w[0], w[1], w[2], w[3]);
            }
            // Underside: a shallow cream bowl.
            let belly = |x: f32, z: f32| -30.0 * (1.0 - rho(x, z).min(1.0).powf(2.4)).powf(0.7);
            let bc = mix(lin(0xf3dcc0), shell_dark, smoothstep(0.8, 1.0, r1));
            let v: Vec<u32> = corners
                .iter()
                .map(|&(x, z)| hide.vs(Vec3::new(x, belly(x, z), z), bc, Skin::one(SHELL)))
                .collect();
            hide.quad(v[0], v[1], v[2], v[3]);
        }
    }
    // Spines around the front of the rim.
    hide.cur = Skin::one(SHELL);
    for k in 0..sectors {
        let a = k as f32 / sectors as f32 * TAU;
        if a.sin() < 0.1 || k % 3 != 0 {
            continue;
        }
        let (x, z) = shape(0.985, k);
        let out = Vec3::new(x / A, 0.0, z / B).normalize();
        let base = Vec3::new(x, 1.5, z);
        hide.cone(
            base,
            base + out * rng.range(9.0, 15.0) + Vec3::Y * 3.0,
            3.6,
            5,
            shell_dark,
        );
    }

    // ---------------------------------------------------------------- face
    for side in 0..2 {
        let eye = eye_pivot(side);
        let base = mirror(Vec3::new(30.0, 4.0, 97.0), side);
        hide.cur = Skin::one(SHELL);
        hide.tube(
            &[(base, 4.2), (base.lerp(eye, 0.5) + Vec3::Z * 3.0, 3.4), (eye, 3.0)],
            7,
            shell_a,
            false,
        );
        hide.cur = Skin::one(EYE + side);
        hide.ico_with(eye, Vec3::splat(9.0), 2, |d| {
            let c = if d.z > 0.8 {
                lin(0x141018)
            } else if d.z > 0.62 {
                lin(0xf2a23a)
            } else {
                lin(0xfbf4e2)
            };
            (1.0, c)
        });
        // A little glint so the eyes read as wet.
        hide.ball(eye + Vec3::new(2.4, 3.0, 8.2), 1.3, 0, lin(0xffffff));
        hide.cur = Skin::one(JAW + side);
        let jp = jaw_pivot(side);
        let sx = if side == 0 { 1.0 } else { -1.0 };
        hide.boxy(
            jp + Vec3::new(-sx * 6.0, -5.0, 3.0),
            Vec3::new(6.5, 8.0, 2.2),
            lin(0xf3cfa8),
        );
        hide.cur = Skin::one(SHELL);
        let feeler = mirror(Vec3::new(15.0, 3.0, 102.0), side);
        hide.cone(feeler, feeler + Vec3::new(sx * 6.0, 9.0, 22.0), 1.4, 5, shell_b);
    }
    hide.cur = Skin::one(SHELL);
    hide.boxy(Vec3::new(0.0, -9.0, 100.0), Vec3::new(17.0, 9.0, 3.0), lin(0x3a1518));

    // ---------------------------------------------------------------- arms
    for side in 0..2 {
        let j = arm_joints(side);
        let b = ARM + side * 4;
        let under = lin(0xf3cfa8);
        for seg in 0..3 {
            hide.cur = Skin::one(b + seg);
            let steps = if seg == 2 { 9 } else { 5 };
            let profile: Vec<(f32, f32, f32)> = (0..=steps)
                .map(|i| {
                    let f = i as f32 / steps as f32;
                    (f, arm_rx(seg, f), arm_ry(seg, f))
                })
                .collect();
            limb(&mut hide, j[seg], j[seg + 1], Vec3::Y, &profile, 10, shell_a, under);
            // Ball joint at the base of each segment.
            hide.ico_with(
                j[seg],
                Vec3::new(
                    arm_rx(seg, 0.0) * 1.04,
                    arm_ry(seg, 0.0) * 1.04,
                    arm_rx(seg, 0.0) * 1.04,
                ),
                1,
                |d| (1.0, mix(under, shell_b, (d.y * 0.7 + 0.5).clamp(0.0, 1.0))),
            );
        }
        // The moving finger hangs under the palm and snaps up against it.
        hide.cur = Skin::one(b + 3);
        let pin = mirror(PINCER, side);
        let tip = mirror(Vec3::new(26.0, 10.0, 266.0), side);
        let mid = pin.lerp(tip, 0.5) - Vec3::Y * 5.0;
        hide.tube(&[(pin, 9.0), (mid, 7.5), (tip, 1.2)], 7, shell_b, true);
        // Knobbly teeth along both fingers.
        for k in 0..6 {
            let f = 0.2 + k as f32 * 0.13;
            let p = pin.lerp(tip, f) + Vec3::Y * (4.5 - 5.0 * (f * PI).sin());
            hide.cone(p, p + Vec3::Y * 4.5, 2.2, 4, lin(0xfbf4e2));
        }
    }

    // ---------------------------------------------------------------- legs
    for side in 0..2 {
        for k in 0..4 {
            let rest = leg_rest(side, k);
            let bone = LEG + (side * 4 + k) * 2;
            let up = rest.normal.cross(rest.knee - rest.hip).normalize();
            let up = if up.y < 0.0 { -up } else { up };
            hide.cur = Skin::one(bone);
            limb(
                &mut hide,
                rest.hip,
                rest.knee,
                up,
                &[(0.0, 9.0, 10.5), (0.35, 10.0, 12.0), (1.0, 7.5, 9.0)],
                8,
                shell_a,
                lin(0xf3cfa8),
            );
            hide.ball(rest.hip, 11.0, 1, shell_b);
            hide.cur = Skin::one(bone + 1);
            let up2 = rest.normal.cross(rest.foot - rest.knee).normalize();
            let up2 = if up2.dot(up) < 0.0 { -up2 } else { up2 };
            limb(
                &mut hide,
                rest.knee,
                rest.foot,
                up2,
                &[(0.0, 7.5, 9.0), (0.3, 7.0, 8.5), (0.8, 3.5, 4.5), (1.0, 0.8, 1.0)],
                8,
                shell_a,
                shell_dark,
            );
            hide.ball(rest.knee, 9.6, 1, shell_b);
        }
    }

    // ---------------------------------------------------------------- flora
    let mut plants = MeshBuilder::new();
    plants.cur = Skin::one(SHELL);
    let upright = |x: f32, y: f32, z: f32, rng: &mut Rng| {
        Affine3A::from_rotation_translation(Quat::from_rotation_y(rng.angle()), Vec3::new(x, y, z))
    };
    // Random point on the shell at normalized radius within [r0, r1].
    let scatter = |rng: &mut Rng, r0: f32, r1: f32| -> (f32, f32) {
        loop {
            let (x, z) = (rng.sym(A), rng.sym(B));
            let r = rho(x, z);
            if r >= r0 && r <= r1 {
                return (x, z);
            }
        }
    };
    let coral_colors = [0xff6f91u32, 0xff9671, 0xc86bfa, 0xffc75f, 0x4fd1c5];

    // The summit's landmark: a giant conch.
    {
        let (x, z) = (-5.0, -12.0);
        plants.xf = Affine3A::from_rotation_translation(
            Quat::from_rotation_y(0.6) * Quat::from_rotation_z(0.25),
            Vec3::new(x, terrain(x, z).0 - 1.0, z),
        );
        flora::conch(&mut plants, 13.0);
    }
    let mut placed = 0;
    while placed < 150 {
        let (x, z) = scatter(&mut rng, 0.0, 0.86);
        let (d, _, lim) = nearest(x, z);
        let (y, _) = terrain(x, z);
        if d < lim + 2.5 || y - dome(x, z) > 19.0 || gauss(x, z, LAGOON.0, LAGOON.1, 17.0) > 0.4 {
            continue;
        }
        plants.xf = upright(x, y - 0.3, z, &mut rng);
        flora::palm(&mut plants, 13.0 + 9.0 * rng.f(), &mut rng);
        placed += 1;
    }
    for _ in 0..420 {
        let (x, z) = scatter(&mut rng, 0.0, 0.84);
        let (d, _, lim) = nearest(x, z);
        if d < lim + 0.8 {
            continue;
        }
        let (y, _) = terrain(x, z);
        plants.xf = upright(x, y - 0.1, z, &mut rng);
        let beachy = rho(x, z) > 0.66;
        match rng.below(6) {
            0 | 1 if !beachy => flora::bush(&mut plants, 2.4 + 2.0 * rng.f(), lin(0x3f9a46), &mut rng),
            2 if !beachy => flora::fern(&mut plants, 3.0 + 1.5 * rng.f(), lin(0x5dbb4f), &mut rng),
            3 => flora::boulders(&mut plants, 1.5 + 2.5 * rng.f(), lin(0x9a9488), &mut rng),
            4 if !beachy => {
                let petal = [0xff5d8f, 0xffd23f, 0xf7f7f2, 0xff8c42][rng.below(4)];
                flora::flower(&mut plants, 1.6 + rng.f(), lin(petal), &mut rng)
            }
            _ => flora::tuft(&mut plants, 1.6 + 1.4 * rng.f(), lin(0x8fce55), &mut rng),
        }
    }
    // Sea life toward the rim.
    for _ in 0..150 {
        let (x, z) = scatter(&mut rng, 0.78, 0.985);
        let (d, _, lim) = nearest(x, z);
        if d < lim + 1.0 {
            continue;
        }
        let (y, _) = terrain(x, z);
        plants.xf = upright(x, y - 0.2, z, &mut rng);
        let c = lin(coral_colors[rng.below(coral_colors.len())]);
        match rng.below(6) {
            0 => flora::coral(&mut plants, 4.0 + 4.0 * rng.f(), c, &mut rng),
            1 => flora::sea_fan(&mut plants, 4.0 + 3.0 * rng.f(), c, &mut rng),
            2 => flora::anemone(&mut plants, 1.8 + 1.4 * rng.f(), c, &mut rng),
            3 => flora::starfish(&mut plants, 1.6 + 1.2 * rng.f(), c, &mut rng),
            _ => flora::barnacles(&mut plants, 1.8 + 2.4 * rng.f(), &mut rng),
        }
    }
    // Kelp trailing from the rim.
    for k in 0..90 {
        let (x, z) = shape(0.995, (k * sectors / 90 + rng.below(2)) % sectors);
        plants.xf = Affine3A::from_translation(Vec3::new(x, 0.5, z));
        flora::strand(&mut plants, 9.0 + 12.0 * rng.f(), lin(0x2f7d4f), &mut rng);
    }
    // Barnacles and coral crusting the arms, clear of the road on top.
    for side in 0..2 {
        let j = arm_joints(side);
        for _ in 0..46 {
            let seg = rng.below(3);
            let f = rng.range(0.08, 0.9);
            let axis = (j[seg + 1] - j[seg]).normalize();
            let sideways = axis.cross(Vec3::Y).normalize() * if rng.chance(0.5) { 1.0 } else { -1.0 };
            let tilt = rng.range(0.15, 0.75);
            let normal = (sideways * (1.0 - tilt * 0.5) + Vec3::Y * tilt).normalize();
            let surface = j[seg].lerp(j[seg + 1], f)
                + sideways * arm_rx(seg, f) * normal.dot(sideways)
                + Vec3::Y * arm_ry(seg, f) * normal.y;
            plants.cur = arm_skin(side, surface);
            plants.xf =
                Affine3A::from_rotation_translation(Quat::from_rotation_arc(Vec3::Y, normal), surface - normal * 0.6);
            let c = lin(coral_colors[rng.below(coral_colors.len())]);
            match rng.below(4) {
                0 => flora::coral(&mut plants, 4.0 + 3.0 * rng.f(), c, &mut rng),
                1 => flora::sea_fan(&mut plants, 4.0 + 2.0 * rng.f(), c, &mut rng),
                _ => flora::barnacles(&mut plants, 2.0 + 2.0 * rng.f(), &mut rng),
            }
        }
    }
    plants.xf = Affine3A::IDENTITY;
    plants.cur = Skin::one(SHELL);

    // The lagoon.
    let mut water = MeshBuilder::new();
    water.cur = Skin::one(SHELL);
    let lagoon_y = terrain(LAGOON.0, LAGOON.1).0 + 3.4;
    water.disc(
        Vec3::new(LAGOON.0, lagoon_y, LAGOON.1),
        Vec3::Y,
        21.0,
        20,
        lin(0x3fc1c9),
    );

    // ---------------------------------------------------------------- road
    let style = TrackStyle {
        road: vec![[lin(0xd2a868), lin(0xc99c5a)], [lin(0xb98a55), lin(0xa97845)]],
        curb: [lin(0xf8f4ea), lin(0xff7a59)],
        line: lin(0xfff8dc),
        shoulder: [lin(0xe6d49c), lin(0xdcc88c)],
        skirt: lin(0x94724c),
        wall: [lin(0xff8a65), lin(0xfff1d6)],
        post: lin(0x7a5230),
    };
    let (ribbon, glow) = track.build_mesh(&style);

    // ---------------------------------------------------------------- fauna
    let mut fauna = Vec::new();
    for k in 0..16 {
        let (x, z) = scatter(&mut rng, 0.0, 1.0);
        fauna.push(Fauna {
            kind: FaunaKind::Gull,
            skin: Skin::one(SHELL),
            rest: Vec3::new(x * 1.2, 60.0 + 40.0 * rng.f(), z * 1.2),
            seed: k as f32 * 1.7 + rng.f(),
        });
    }
    for k in 0..7 {
        let a = rng.angle();
        fauna.push(Fauna {
            kind: FaunaKind::Fish,
            skin: Skin::one(SHELL),
            rest: Vec3::new(a.cos() * 330.0, SEA, a.sin() * 300.0 + 60.0),
            seed: k as f32 * 2.3 + rng.f(),
        });
    }

    Built {
        rest: piv.iter().map(|&p| pivot(p)).collect(),
        track,
        layers: vec![
            Layer {
                mesh: top,
                mat: Mat::Matte,
                flat: true,
            },
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
            Layer {
                mesh: water,
                mat: Mat::Water,
                flat: true,
            },
        ],
        fauna,
        animator: Box::new(CrabAnim {
            piv,
            phase: 0.0,
            speed: 1.0,
            prev_cycle: 0.0,
            eye_dir: [Vec3::Z; 2],
        }),
        env: EnvDef {
            ground: Ground::Sea {
                level: SEA,
                floor: FLOOR,
            },
            sky_top: 0x1f74dc,
            sky_horizon: 0xbfe8ff,
            fog: 0xb4e0f5,
            fog_start: 900.0,
            fog_end: 5200.0,
            sun_dir: Vec3::new(0.46, -0.74, 0.5),
            sun_color: 0xfff4dc,
            sun_lux: 11000.0,
            ambient: 1500.0,
            cloud_height: (260.0, 520.0),
        },
        view: ShowView {
            center: Vec3::new(0.0, 10.0, 70.0),
            radius: 470.0,
            height: 150.0,
        },
        critter_color: 0xff5a3c,
    }
}
