//! The race track: a closed ribbon laid over the beast and skinned to its
//! skeleton, so it bends, tilts and stretches as the beast moves.
//!
//! Karts don't drive in world space. They live in track coordinates `(u, d)`:
//! `u` is a fractional sample index along the centerline and `d` is the
//! lateral offset in meters. Every frame the centerline is re-posed from the
//! skeleton, so whatever the beast does to the ground, the karts ride with it.

use crate::{
    meshkit::{Col, MeshBuilder, Skin},
    util::{lin, shade, smooth},
};
use bevy::{math::Affine3A, prelude::*};

/// Target distance between centerline samples in the rest pose.
const SPACING: f32 = 2.5;
const CURB: f32 = 1.1;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Edge {
    /// A barrier: karts bounce off.
    Wall,
    /// Open edge: karts fall off the beast.
    Fall,
    /// Thick undergrowth: no barrier is drawn, but karts can't push through.
    Soft,
}

/// A control point of the track spline, in the beast's rest pose.
#[derive(Clone, Copy)]
pub struct Ctrl {
    pub p: Vec3,
    pub up: Vec3,
    pub half: f32,
    /// Off-road shoulder width on the left and right.
    pub sh: [f32; 2],
    pub edge: [Edge; 2],
    /// Which body part this point sits on (decides its skin weights).
    pub part: u8,
    /// The stretch from this point to the next has no ground: karts leap it.
    pub gap: bool,
    /// Surface style index into [`TrackStyle::road`].
    pub surf: u8,
}

impl Ctrl {
    pub fn new(p: Vec3, up: Vec3, part: u8) -> Self {
        Ctrl {
            p,
            up: up.normalize(),
            half: 8.0,
            sh: [4.0, 4.0],
            edge: [Edge::Wall, Edge::Wall],
            part,
            gap: false,
            surf: 0,
        }
    }
    pub fn half(mut self, h: f32) -> Self {
        self.half = h;
        self
    }
    pub fn sh(mut self, l: f32, r: f32) -> Self {
        self.sh = [l, r];
        self
    }
    pub fn edges(mut self, l: Edge, r: Edge) -> Self {
        self.edge = [l, r];
        self
    }
    pub fn gap(mut self) -> Self {
        self.gap = true;
        self
    }
    pub fn surf(mut self, s: u8) -> Self {
        self.surf = s;
        self
    }
}

/// Things placed along the track. Positions are in control-point units:
/// `3.5` is halfway between control points 3 and 4.
#[derive(Default, Clone)]
pub struct Features {
    /// `(at, d)`: a boost pad centered at lateral offset `d`.
    pub boosts: Vec<(f32, f32)>,
    /// Rows of item boxes across the road.
    pub item_rows: Vec<f32>,
    /// `(at, length_m, height_m)`: a jump ramp.
    pub ramps: Vec<(f32, f32, f32)>,
    /// Places where small fauna wander across the road.
    pub critters: Vec<f32>,
    /// Vents in the road that throw karts skyward when the beast spouts.
    pub geysers: Vec<f32>,
}

#[derive(Clone)]
pub struct TrackStyle {
    /// Two alternating band colors per surface style.
    pub road: Vec<[Col; 2]>,
    pub curb: [Col; 2],
    pub line: Col,
    pub shoulder: [Col; 2],
    pub skirt: Col,
    pub wall: [Col; 2],
    pub post: Col,
}

#[derive(Clone, Copy)]
pub struct Sample {
    pub c: Vec3,
    pub r: Vec3,
    pub n: Vec3,
    pub half: f32,
    pub sh: [f32; 2],
    pub edge: [Edge; 2],
    pub skin: Skin,
    pub gap: bool,
    pub surf: u8,
    pub ramp: bool,
}

/// A centerline sample posed in world space for the current frame.
#[derive(Clone, Copy, Default)]
pub struct Frame {
    pub p: Vec3,
    /// Lateral axis (toward +d). Not normalized, so `p + r * d` matches the skinned mesh.
    pub r: Vec3,
    pub n: Vec3,
    pub t: Vec3,
    /// Distance to the next sample.
    pub len: f32,
    /// Signed curvature; positive curves toward +d.
    pub k: f32,
}

/// The track at a fractional position `u`.
#[derive(Clone, Copy)]
pub struct Loc {
    pub p: Vec3,
    pub r: Vec3,
    pub n: Vec3,
    pub t: Vec3,
    pub len: f32,
    pub k: f32,
    pub half: f32,
    pub sh: [f32; 2],
    pub edge: [Edge; 2],
    pub gap: bool,
}

impl Loc {
    /// Farthest a kart can be from the centerline on the side of `d`.
    pub fn limit(&self, d: f32) -> f32 {
        self.half + self.sh[(d > 0.0) as usize]
    }
    pub fn edge_at(&self, d: f32) -> Edge {
        self.edge[(d > 0.0) as usize]
    }
    pub fn at(&self, d: f32) -> Vec3 {
        self.p + self.r * d
    }
    /// World rotation for something heading `yaw` radians right of the track direction.
    pub fn rotation(&self, yaw: f32) -> Quat {
        let right = self.r.normalize_or(Vec3::X);
        let fwd = self.t * yaw.cos() + right * yaw.sin();
        Transform::IDENTITY.looking_to(fwd, self.n).rotation
    }
}

pub struct BoostPad {
    pub u0: f32,
    pub u1: f32,
    pub d: f32,
    pub half: f32,
}

pub struct Track {
    pub samples: Vec<Sample>,
    pub frames: Vec<Frame>,
    /// Each frame's position and lateral axis as posed one update earlier.
    prev: Vec<(Vec3, Vec3)>,
    /// Lap length in the rest pose, meters.
    pub length: f32,
    pub boosts: Vec<BoostPad>,
    /// `(u, lateral offsets)` for each row of item boxes.
    pub item_rows: Vec<(f32, Vec<f32>)>,
    /// `(u_start, u_end)` of each ramp; leaving one launches the kart.
    pub ramps: Vec<(f32, f32)>,
    /// `(u_start, u_end)` of each gap.
    pub gaps: Vec<(f32, f32)>,
    pub critters: Vec<f32>,
    pub geysers: Vec<f32>,
}

fn catmull(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (p2 - p0) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3)
}

impl Track {
    /// Samples the closed spline through `ctrl`. `skin_of(part, position)`
    /// gives the skin weights of a rest-pose point on a body part, and
    /// `settle(part, next part, position)` moves a sample onto the beast's
    /// surface, so the road follows the ground between control points.
    pub fn build(
        ctrl: &[Ctrl],
        features: &Features,
        skin_of: &dyn Fn(u8, Vec3) -> Skin,
        settle: &dyn Fn(u8, u8, Vec3) -> Vec3,
    ) -> Track {
        let m = ctrl.len();
        const FINE: usize = 48;
        // Fine polyline with cumulative length.
        let mut fine: Vec<(Vec3, f32)> = Vec::with_capacity(m * FINE + 1);
        let mut total = 0.0;
        for k in 0..m {
            let (p0, p1, p2, p3) = (
                ctrl[(k + m - 1) % m].p,
                ctrl[k].p,
                ctrl[(k + 1) % m].p,
                ctrl[(k + 2) % m].p,
            );
            for j in 0..FINE {
                let p = catmull(p0, p1, p2, p3, j as f32 / FINE as f32);
                if let Some(&(last, _)) = fine.last() {
                    total += p.distance(last);
                }
                fine.push((p, total));
            }
        }
        let length = total + fine[0].0.distance(fine[fine.len() - 1].0);
        fine.push((fine[0].0, length));

        let n = (length / SPACING).round() as usize;
        let step = length / n as f32;
        let mut samples = Vec::with_capacity(n);
        let mut ctrl_u = vec![0.0; m];
        let mut seg = 0usize;
        for i in 0..n {
            let s = i as f32 * step;
            while seg + 2 < fine.len() && fine[seg + 1].1 <= s {
                seg += 1;
            }
            let (a, b) = (fine[seg], fine[seg + 1]);
            let f = ((s - a.1) / (b.1 - a.1).max(1e-6)).clamp(0.0, 1.0);
            let q = (seg as f32 + f) / FINE as f32;
            let k = (q.floor() as usize).min(m - 1);
            let kf = q - k as f32;
            let (c0, c1) = (&ctrl[k], &ctrl[(k + 1) % m]);
            let c = settle(c0.part, c1.part, a.0.lerp(b.0, f));
            let w = smooth(kf);
            // Tangent from the fine polyline, a little ahead and behind.
            let ahead = fine[(seg + 2).min(fine.len() - 1)].0;
            let behind = fine[seg.saturating_sub(1)].0;
            let t = (ahead - behind).normalize_or(Vec3::Z);
            let up = c0.up.lerp(c1.up, w).normalize_or(Vec3::Y);
            let r = t.cross(up).normalize_or(Vec3::X);
            let nrm = r.cross(t).normalize();
            let skin = if c0.part == c1.part {
                skin_of(c0.part, c)
            } else {
                Skin::mix(skin_of(c0.part, c), skin_of(c1.part, c), w)
            };
            samples.push(Sample {
                c,
                r,
                n: nrm,
                half: c0.half + (c1.half - c0.half) * w,
                sh: [
                    c0.sh[0] + (c1.sh[0] - c0.sh[0]) * w,
                    c0.sh[1] + (c1.sh[1] - c0.sh[1]) * w,
                ],
                edge: c0.edge,
                skin,
                gap: c0.gap,
                surf: c0.surf,
                ramp: false,
            });
        }
        // Where each control point landed among the samples.
        for (k, cu) in ctrl_u.iter_mut().enumerate() {
            *cu = fine[k * FINE].1 / step;
        }
        let to_u = |at: f32| -> f32 {
            let k = (at.floor() as usize) % m;
            let f = at - at.floor();
            let a = ctrl_u[k];
            let mut b = ctrl_u[(k + 1) % m];
            if b < a {
                b += n as f32;
            }
            (a + (b - a) * f).rem_euclid(n as f32)
        };

        let mut ramps = Vec::new();
        for &(at, len, height) in &features.ramps {
            let u0 = to_u(at);
            let count = (len / step).round().max(2.0) as usize;
            for j in 0..=count {
                let i = (u0 as usize + j) % n;
                let f = j as f32 / count as f32;
                let s = &mut samples[i];
                s.c += s.n * (height * f * f * (1.5 - 0.5 * f));
                s.ramp = true;
            }
            ramps.push((u0.floor(), (u0.floor() + count as f32).rem_euclid(n as f32)));
        }

        let boosts = features
            .boosts
            .iter()
            .map(|&(at, d)| {
                let u0 = to_u(at).floor();
                BoostPad {
                    u0,
                    u1: (u0 + 5.0).rem_euclid(n as f32),
                    d,
                    half: 2.6,
                }
            })
            .collect();

        let item_rows = features
            .item_rows
            .iter()
            .map(|&at| {
                let u = to_u(at).round().rem_euclid(n as f32);
                let half = samples[u as usize % n].half;
                let count = if half > 7.0 { 5 } else { 4 };
                let span = half - 1.8;
                let ds = (0..count)
                    .map(|j| -span + 2.0 * span * j as f32 / (count - 1) as f32)
                    .collect();
                (u, ds)
            })
            .collect();

        let mut gaps = Vec::new();
        let mut i = 0;
        while i < n {
            if samples[i].gap && !samples[(i + n - 1) % n].gap {
                let mut j = i;
                while samples[j % n].gap && j < i + n {
                    j += 1;
                }
                gaps.push((i as f32, (j % n) as f32));
                i = j;
            } else {
                i += 1;
            }
        }

        let critters = features.critters.iter().map(|&at| to_u(at)).collect();
        let geysers = features.geysers.iter().map(|&at| to_u(at)).collect();
        Track {
            frames: vec![Frame::default(); n],
            prev: Vec::new(),
            samples,
            length,
            boosts,
            item_rows,
            ramps,
            gaps,
            critters,
            geysers,
        }
    }

    pub fn n(&self) -> f32 {
        self.samples.len() as f32
    }

    pub fn wrap(&self, u: f32) -> f32 {
        u.rem_euclid(self.n())
    }

    /// Shortest signed distance from `a` to `b` in samples.
    pub fn delta(&self, a: f32, b: f32) -> f32 {
        let n = self.n();
        (b - a + n * 0.5).rem_euclid(n) - n * 0.5
    }

    /// Whether `u` lies in the wrapped range `[u0, u1)`.
    pub fn within(&self, u: f32, u0: f32, u1: f32) -> bool {
        if u0 <= u1 { u >= u0 && u < u1 } else { u >= u0 || u < u1 }
    }

    /// Re-poses the centerline from the skeleton's skinning matrices.
    pub fn update(&mut self, mats: &[Affine3A]) {
        let n = self.samples.len();
        let posed = !self.prev.is_empty();
        self.prev.clear();
        self.prev.extend(self.frames.iter().map(|f| (f.p, f.r)));
        for (s, f) in self.samples.iter().zip(self.frames.iter_mut()) {
            let a = s.skin.apply(mats);
            f.p = a.transform_point3(s.c);
            f.r = a.transform_vector3(s.r);
        }
        for i in 0..n {
            let next = self.frames[(i + 1) % n].p;
            let prev = self.frames[(i + n - 1) % n].p;
            let f = &mut self.frames[i];
            f.t = (next - prev).normalize_or(Vec3::Z);
            f.len = next.distance(f.p).max(0.05);
            // Keep the frame orthogonal: the surface normal follows the skinned lateral axis.
            let right = (f.r - f.t * f.r.dot(f.t)).normalize_or(Vec3::X);
            f.n = right.cross(f.t).normalize();
        }
        let mut raw = vec![0.0f32; n];
        for (i, k) in raw.iter_mut().enumerate() {
            let (a, b, c) = (
                &self.frames[(i + n - 1) % n],
                &self.frames[i],
                &self.frames[(i + 1) % n],
            );
            *k = (c.t - a.t).dot(b.r.normalize_or(Vec3::X)) / (a.len + b.len);
        }
        for i in 0..n {
            let mut sum = 0.0;
            for o in 0..5 {
                sum += raw[(i + n + o - 2) % n];
            }
            self.frames[i].k = sum / 5.0;
        }
        if !posed {
            // First pose: nothing has moved yet.
            self.prev.clear();
            self.prev.extend(self.frames.iter().map(|f| (f.p, f.r)));
        }
    }

    /// Where the road surface at `(u, d)` was on the previous update. The
    /// difference from its position now is how far the beast carried it.
    pub fn carried_from(&self, u: f32, d: f32) -> Vec3 {
        let n = self.samples.len();
        let u = u.rem_euclid(n as f32);
        let i = (u.floor() as usize).min(n - 1);
        let f = u - i as f32;
        let (a, b) = (self.prev[i], self.prev[(i + 1) % n]);
        a.0.lerp(b.0, f) + a.1.lerp(b.1, f) * d
    }

    pub fn at(&self, u: f32) -> Loc {
        let n = self.samples.len();
        let u = u.rem_euclid(n as f32);
        let i = (u.floor() as usize).min(n - 1);
        let j = (i + 1) % n;
        let f = u - i as f32;
        let (a, b) = (&self.frames[i], &self.frames[j]);
        let (sa, sb) = (&self.samples[i], &self.samples[j]);
        Loc {
            p: a.p.lerp(b.p, f),
            r: a.r.lerp(b.r, f),
            n: a.n.lerp(b.n, f).normalize_or(Vec3::Y),
            t: a.t.lerp(b.t, f).normalize_or(Vec3::Z),
            len: a.len,
            k: a.k + (b.k - a.k) * f,
            half: sa.half + (sb.half - sa.half) * f,
            sh: [
                sa.sh[0] + (sb.sh[0] - sa.sh[0]) * f,
                sa.sh[1] + (sb.sh[1] - sa.sh[1]) * f,
            ],
            edge: sa.edge,
            gap: sa.gap,
        }
    }

    /// The gap containing `u`, if any, as `(start, end, progress 0..1)`.
    pub fn gap_at(&self, u: f32) -> Option<(f32, f32, f32)> {
        let n = self.n();
        self.gaps.iter().find(|g| self.within(u, g.0, g.1)).map(|&(a, b)| {
            let span = (b - a).rem_euclid(n);
            (a, b, ((u - a).rem_euclid(n) / span).clamp(0.0, 1.0))
        })
    }

    /// Builds the road ribbon (matte) and its glowing overlays (boost pads).
    pub fn build_mesh(&self, style: &TrackStyle) -> (MeshBuilder, MeshBuilder) {
        let mut b = MeshBuilder::new();
        let mut glow = MeshBuilder::new();
        let n = self.samples.len();
        let cols = |s: &Sample| -> [f32; 10] {
            let (l, r) = (-s.half, s.half);
            [
                l - s.sh[0],
                l,
                l + CURB,
                l * 0.5,
                -0.35,
                0.35,
                r * 0.5,
                r - CURB,
                r,
                r + s.sh[1],
            ]
        };
        let point = |s: &Sample, d: f32, h: f32| s.c + s.r * d + s.n * h;

        for i in 0..n {
            let (s0, s1) = (&self.samples[i], &self.samples[(i + 1) % n]);
            if s0.gap {
                continue;
            }
            let (d0, d1) = (cols(s0), cols(s1));
            let band = (i / 2) % 2;
            let road = style.road[(s0.surf as usize).min(style.road.len() - 1)];
            for j in 0..9 {
                let mut c = match j {
                    0 | 8 => style.shoulder[(i / 3) % 2],
                    1 | 7 => style.curb[band],
                    4 => {
                        if (i / 3) % 2 == 0 {
                            style.line
                        } else {
                            road[band]
                        }
                    }
                    _ => road[band],
                };
                if s0.ramp && (2..=6).contains(&j) {
                    c = if i % 2 == 0 { lin(0xffd23f) } else { lin(0xf2643d) };
                }
                // Shoulders dip slightly at the outside so they read as verges.
                let h = |j: usize| if j == 0 || j == 9 { -0.25 } else { 0.0 };
                let a = b.vs(point(s0, d0[j], h(j)), c, s0.skin);
                let bb = b.vs(point(s0, d0[j + 1], h(j + 1)), c, s0.skin);
                let cc = b.vs(point(s1, d1[j + 1], h(j + 1)), c, s1.skin);
                let dd = b.vs(point(s1, d1[j], h(j)), c, s1.skin);
                b.quad(a, bb, cc, dd);
            }
            // Skirts hide the gap between the flat ribbon and the curved hide below.
            for side in 0..2 {
                let sign = if side == 0 { -1.0 } else { 1.0 };
                let (e0, e1) = (d0[side * 9], d1[side * 9]);
                let top0 = point(s0, e0, -0.25);
                let top1 = point(s1, e1, -0.25);
                let bot0 = point(s0, e0 + sign * 2.5, -7.0);
                let bot1 = point(s1, e1 + sign * 2.5, -7.0);
                let c = style.skirt;
                let v = [
                    b.vs(top0, c, s0.skin),
                    b.vs(bot0, shade(c, 0.7), s0.skin),
                    b.vs(bot1, shade(c, 0.7), s1.skin),
                    b.vs(top1, c, s1.skin),
                ];
                if side == 0 {
                    b.quad(v[3], v[2], v[1], v[0]);
                } else {
                    b.quad(v[0], v[1], v[2], v[3]);
                }
                // Barrier along walled edges.
                if s0.edge[side] == Edge::Wall {
                    let c = style.wall[(i / 2) % 2];
                    let (inner0, inner1) = (e0 - sign * 0.9, e1 - sign * 0.9);
                    let hgt = 1.3;
                    let q = [
                        (point(s0, inner0, 0.0), point(s1, inner1, 0.0)),
                        (point(s0, inner0, hgt), point(s1, inner1, hgt)),
                        (point(s0, e0, hgt), point(s1, e1, hgt)),
                        (point(s0, e0, -0.25), point(s1, e1, -0.25)),
                    ];
                    for w in 0..3 {
                        let cw = if w == 1 { shade(c, 1.12) } else { shade(c, 0.9) };
                        let v = [
                            b.vs(q[w].0, cw, s0.skin),
                            b.vs(q[w + 1].0, cw, s0.skin),
                            b.vs(q[w + 1].1, cw, s1.skin),
                            b.vs(q[w].1, cw, s1.skin),
                        ];
                        if side == 0 {
                            b.quad(v[3], v[2], v[1], v[0]);
                        } else {
                            b.quad(v[0], v[1], v[2], v[3]);
                        }
                    }
                    if i % 6 == 0 {
                        let mid = (inner0 + e0) * 0.5;
                        let base = point(s0, mid, 0.0);
                        let mut post = MeshBuilder::new();
                        post.cyl(Vec3::ZERO, Vec3::Y * 2.6, 0.55, 0.4, 5, style.post, true);
                        post.ball(Vec3::Y * 2.8, 0.65, 0, shade(style.post, 1.25));
                        let rot = Quat::from_rotation_arc(Vec3::Y, s0.n);
                        b.append_at(&post, Affine3A::from_rotation_translation(rot, base), s0.skin);
                    }
                }
            }
        }

        // Start line: a checkered strip across the road at u = 0.
        let (s0, s1) = (&self.samples[0], &self.samples[1]);
        let squares = 12;
        for row in 0..2 {
            for k in 0..squares {
                let c = if (row + k) % 2 == 0 {
                    lin(0xf7f7f2)
                } else {
                    lin(0x1b1b1f)
                };
                let fa = row as f32 * 0.5;
                let fb = fa + 0.5;
                let da = |s: &Sample, k: usize| -s.half + 2.0 * s.half * k as f32 / squares as f32;
                let pa = |f: f32, k: usize| {
                    let p0 = point(s0, da(s0, k), 0.06);
                    let p1 = point(s1, da(s1, k), 0.06);
                    p0.lerp(p1, f)
                };
                let v = [
                    b.vs(pa(fa, k), c, s0.skin),
                    b.vs(pa(fa, k + 1), c, s0.skin),
                    b.vs(pa(fb, k + 1), c, s0.skin),
                    b.vs(pa(fb, k), c, s0.skin),
                ];
                b.quad(v[0], v[1], v[2], v[3]);
            }
        }
        // Start gate.
        {
            let s = &self.samples[0];
            let rot = Quat::from_mat3(&Mat3::from_cols(s.r, s.n, s.r.cross(s.n)));
            let mut gate = MeshBuilder::new();
            let w = s.half + 2.0;
            let wood = lin(0x7a5230);
            for side in [-1.0, 1.0] {
                gate.cyl(
                    Vec3::new(side * w, -1.0, 0.0),
                    Vec3::new(side * w, 11.0, 0.0),
                    0.8,
                    0.6,
                    6,
                    wood,
                    true,
                );
                gate.ball(Vec3::new(side * w, 11.6, 0.0), 1.1, 1, lin(0xffd23f));
            }
            gate.boxy(Vec3::new(0.0, 9.6, 0.0), Vec3::new(w, 1.3, 0.25), lin(0x23232b));
            for k in 0..16 {
                let x = -w + (k as f32 + 0.5) * w / 8.0;
                for row in 0..2 {
                    if (k + row) % 2 == 0 {
                        let y = 9.6 - 0.65 + row as f32 * 1.3;
                        gate.boxy(Vec3::new(x, y, 0.0), Vec3::new(w / 16.0, 0.65, 0.3), lin(0xf7f7f2));
                    }
                }
            }
            b.append_at(&gate, Affine3A::from_rotation_translation(rot, s.c), s.skin);
        }

        // Boost pads: glowing chevrons.
        for pad in &self.boosts {
            let count = (pad.u1 - pad.u0).rem_euclid(n as f32) as usize;
            for j in 0..count {
                let i = (pad.u0 as usize + j) % n;
                let (s0, s1) = (&self.samples[i], &self.samples[(i + 1) % n]);
                let c = if j % 2 == 0 { lin(0x39e6ff) } else { lin(0xfff06a) };
                // A "V" pointing down-track: two slanted quads meeting at the center.
                for side in [-1.0f32, 1.0] {
                    let inner = pad.d;
                    let outer = pad.d + side * pad.half;
                    let along = |d: f32, f: f32| point(s0, d, 0.07).lerp(point(s1, d, 0.07), f);
                    let v = [
                        glow.vs(along(outer, 0.0), c, s0.skin),
                        glow.vs(along(inner, 0.5), c, s0.skin),
                        glow.vs(along(inner, 1.0), c, s1.skin),
                        glow.vs(along(outer, 0.5), c, s0.skin),
                    ];
                    if side < 0.0 {
                        glow.quad(v[0], v[1], v[2], v[3]);
                    } else {
                        glow.quad(v[3], v[2], v[1], v[0]);
                    }
                }
            }
        }
        (b, glow)
    }
}
