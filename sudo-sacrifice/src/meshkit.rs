//! Low-poly mesh construction. Every model in the game is built from these
//! primitives with per-vertex colors and baked flat-shaded.

use crate::util::Rng;
use bevy::{
    asset::RenderAssetUsages,
    math::Affine3A,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use std::{collections::HashMap, f32::consts::TAU};

pub type Col = [f32; 4];

/// Two unit vectors `(u, v)` perpendicular to `axis` with `u × v = axis`.
pub fn frame(axis: Vec3) -> (Vec3, Vec3) {
    let a = axis.normalize_or(Vec3::Y);
    let hint = if a.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let u = hint.cross(a).normalize();
    (u, a.cross(u))
}

#[derive(Clone)]
pub struct MeshBuilder {
    pos: Vec<Vec3>,
    col: Vec<Col>,
    tris: Vec<[u32; 3]>,
    /// Applied to every vertex added through [`MeshBuilder::v`].
    pub xf: Affine3A,
}

impl Default for MeshBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MeshBuilder {
    pub fn new() -> Self {
        MeshBuilder {
            pos: Vec::new(),
            col: Vec::new(),
            tris: Vec::new(),
            xf: Affine3A::IDENTITY,
        }
    }

    /// Runs `f` with `xf` temporarily set to `xf`.
    pub fn with(&mut self, xf: Affine3A, f: impl FnOnce(&mut MeshBuilder)) {
        let old = self.xf;
        self.xf = old * xf;
        f(self);
        self.xf = old;
    }

    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }

    /// Adds a vertex in local space (transformed by `xf`).
    pub fn v(&mut self, p: Vec3, c: Col) -> u32 {
        self.pos.push(self.xf.transform_point3(p));
        self.col.push(c);
        (self.pos.len() - 1) as u32
    }

    pub fn tri(&mut self, a: u32, b: u32, c: u32) {
        self.tris.push([a, b, c]);
    }

    /// Counter-clockwise quad.
    pub fn quad(&mut self, a: u32, b: u32, c: u32, d: u32) {
        self.tris.push([a, b, c]);
        self.tris.push([a, c, d]);
    }

    pub fn tri_p(&mut self, p: [Vec3; 3], c: Col) {
        let (a, b, d) = (self.v(p[0], c), self.v(p[1], c), self.v(p[2], c));
        self.tri(a, b, d);
    }

    pub fn quad_p(&mut self, p: [Vec3; 4], c: Col) {
        let i = [self.v(p[0], c), self.v(p[1], c), self.v(p[2], c), self.v(p[3], c)];
        self.quad(i[0], i[1], i[2], i[3]);
    }

    /// Double-sided quad.
    pub fn quad_2(&mut self, p: [Vec3; 4], c: Col) {
        self.quad_p(p, c);
        self.quad_p([p[3], p[2], p[1], p[0]], c);
    }

    /// Connects consecutive closed rings. Rings must wind counter-clockwise
    /// around the direction of travel for the surface to face outward.
    pub fn loft(&mut self, rings: &[Vec<(Vec3, Col)>], cap_start: bool, cap_end: bool) {
        if rings.len() < 2 {
            return;
        }
        let n = rings[0].len();
        let base = self.pos.len() as u32;
        for ring in rings {
            for (p, c) in ring {
                self.v(*p, *c);
            }
        }
        for i in 0..rings.len() - 1 {
            for j in 0..n {
                let j1 = (j + 1) % n;
                let a = base + (i * n + j) as u32;
                let b = base + (i * n + j1) as u32;
                let c = base + ((i + 1) * n + j1) as u32;
                let d = base + ((i + 1) * n + j) as u32;
                self.quad(a, b, c, d);
            }
        }
        if cap_start {
            let ring = &rings[0];
            let center = ring.iter().map(|r| r.0).sum::<Vec3>() / n as f32;
            let ci = self.v(center, ring[0].1);
            for j in 0..n {
                self.tri(ci, base + ((j + 1) % n) as u32, base + j as u32);
            }
        }
        if cap_end {
            let ring = &rings[rings.len() - 1];
            let off = base + ((rings.len() - 1) * n) as u32;
            let center = ring.iter().map(|r| r.0).sum::<Vec3>() / n as f32;
            let ci = self.v(center, ring[0].1);
            for j in 0..n {
                self.tri(ci, off + j as u32, off + ((j + 1) % n) as u32);
            }
        }
    }

    /// A ring of `sides` points around `center`, perpendicular to `axis`.
    pub fn ring(center: Vec3, axis: Vec3, up_hint: Vec3, rx: f32, ry: f32, sides: usize, c: Col) -> Vec<(Vec3, Col)> {
        let a = axis.normalize_or(Vec3::Y);
        let mut v = (up_hint - a * up_hint.dot(a)).normalize_or_zero();
        if v == Vec3::ZERO {
            v = frame(a).1;
        }
        let u = v.cross(a);
        (0..sides)
            .map(|k| {
                let t = k as f32 / sides as f32 * TAU;
                (center + u * (t.cos() * rx) + v * (t.sin() * ry), c)
            })
            .collect()
    }

    /// Round tube through `path` points with per-point radius.
    pub fn tube(&mut self, path: &[(Vec3, f32)], sides: usize, c: Col, caps: bool) {
        if path.len() < 2 {
            return;
        }
        let mut rings = Vec::with_capacity(path.len());
        let mut up = Vec3::Y;
        for i in 0..path.len() {
            let prev = path[i.saturating_sub(1)].0;
            let next = path[(i + 1).min(path.len() - 1)].0;
            let axis = (next - prev).normalize_or(Vec3::Y);
            if up.cross(axis).length_squared() < 0.01 {
                up = frame(axis).1;
            }
            up = (up - axis * up.dot(axis)).normalize();
            rings.push(Self::ring(path[i].0, axis, up, path[i].1, path[i].1, sides, c));
        }
        self.loft(&rings, caps, caps);
    }

    /// Tapered cylinder between two points.
    pub fn cyl(&mut self, a: Vec3, b: Vec3, ra: f32, rb: f32, sides: usize, c: Col) {
        self.tube(&[(a, ra), (b, rb)], sides, c, true);
    }

    pub fn cone(&mut self, base: Vec3, tip: Vec3, r: f32, sides: usize, c: Col) {
        self.tube(&[(base, r), (tip, 0.0)], sides, c, true);
    }

    /// Surface of revolution around local Y from `(radius, y, color)` points,
    /// listed bottom to top.
    pub fn lathe(&mut self, profile: &[(f32, f32, Col)], sides: usize) {
        let rings: Vec<Vec<(Vec3, Col)>> = profile
            .iter()
            .map(|&(r, y, c)| {
                (0..sides)
                    .map(|k| {
                        let t = k as f32 / sides as f32 * TAU;
                        (Vec3::new(t.sin() * r, y, t.cos() * r), c)
                    })
                    .collect()
            })
            .collect();
        self.loft(&rings, true, true);
    }

    pub fn boxy(&mut self, center: Vec3, half: Vec3, c: Col) {
        self.boxy6(center, half, [c; 6]);
    }

    /// Box with a color per face: +z, -z, +x, -x, +y, -y.
    pub fn boxy6(&mut self, center: Vec3, half: Vec3, c: [Col; 6]) {
        let p = |x: f32, y: f32, z: f32| center + Vec3::new(x * half.x, y * half.y, z * half.z);
        self.quad_p([p(-1., -1., 1.), p(1., -1., 1.), p(1., 1., 1.), p(-1., 1., 1.)], c[0]);
        self.quad_p(
            [p(1., -1., -1.), p(-1., -1., -1.), p(-1., 1., -1.), p(1., 1., -1.)],
            c[1],
        );
        self.quad_p([p(1., -1., 1.), p(1., -1., -1.), p(1., 1., -1.), p(1., 1., 1.)], c[2]);
        self.quad_p(
            [p(-1., -1., -1.), p(-1., -1., 1.), p(-1., 1., 1.), p(-1., 1., -1.)],
            c[3],
        );
        self.quad_p([p(-1., 1., 1.), p(1., 1., 1.), p(1., 1., -1.), p(-1., 1., -1.)], c[4]);
        self.quad_p(
            [p(-1., -1., -1.), p(1., -1., -1.), p(1., -1., 1.), p(-1., -1., 1.)],
            c[5],
        );
    }

    /// A box between two points with a square cross-section.
    pub fn beam(&mut self, a: Vec3, b: Vec3, half: f32, c: Col) {
        let d = b - a;
        let len = d.length();
        if len < 1e-5 {
            return;
        }
        let rot = Quat::from_rotation_arc(Vec3::Y, d / len);
        self.with(Affine3A::from_rotation_translation(rot, (a + b) * 0.5), |m| {
            m.boxy(Vec3::ZERO, Vec3::new(half, len * 0.5, half), c);
        });
    }

    /// Flat disc facing `normal`.
    pub fn disc(&mut self, center: Vec3, normal: Vec3, r: f32, sides: usize, c: Col) {
        let (u, v) = frame(normal);
        let ci = self.v(center, c);
        let ring: Vec<u32> = (0..sides)
            .map(|k| {
                let t = k as f32 / sides as f32 * TAU;
                self.v(center + (u * t.cos() + v * t.sin()) * r, c)
            })
            .collect();
        for k in 0..sides {
            self.tri(ci, ring[k], ring[(k + 1) % sides]);
        }
    }

    /// Flat annulus facing `normal`.
    pub fn annulus(&mut self, center: Vec3, normal: Vec3, r0: f32, r1: f32, sides: usize, c: Col) {
        let (u, v) = frame(normal);
        for k in 0..sides {
            let (t0, t1) = (k as f32 / sides as f32 * TAU, (k + 1) as f32 / sides as f32 * TAU);
            let d0 = u * t0.cos() + v * t0.sin();
            let d1 = u * t1.cos() + v * t1.sin();
            self.quad_p(
                [center + d0 * r0, center + d0 * r1, center + d1 * r1, center + d1 * r0],
                c,
            );
        }
    }

    /// Ellipsoid from a subdivided icosahedron. `f` sees the unit direction
    /// and returns a radius scale and a color.
    pub fn ico_with(&mut self, center: Vec3, radii: Vec3, sub: u32, mut f: impl FnMut(Vec3) -> (f32, Col)) {
        let (verts, faces) = icosphere(sub);
        let base = self.pos.len() as u32;
        for d in &verts {
            let (k, c) = f(*d);
            self.v(center + *d * radii * k, c);
        }
        for t in &faces {
            self.tri(base + t[0], base + t[1], base + t[2]);
        }
    }

    pub fn ico(&mut self, center: Vec3, radii: Vec3, sub: u32, c: Col) {
        self.ico_with(center, radii, sub, |_| (1.0, c));
    }

    pub fn ball(&mut self, center: Vec3, r: f32, sub: u32, c: Col) {
        self.ico(center, Vec3::splat(r), sub, c);
    }

    /// Octahedron (a crystal), stretched along Y.
    pub fn gem(&mut self, center: Vec3, r: f32, h: f32, c_top: Col, c_bot: Col) {
        let top = center + Vec3::Y * h;
        let bot = center - Vec3::Y * h;
        let ring: Vec<Vec3> = (0..4)
            .map(|k| {
                let t = k as f32 / 4.0 * TAU;
                center + Vec3::new(t.cos() * r, 0.0, t.sin() * r)
            })
            .collect();
        for k in 0..4 {
            let (a, b) = (ring[k], ring[(k + 1) % 4]);
            self.tri_p([top, b, a], c_top);
            self.tri_p([bot, a, b], c_bot);
        }
    }

    /// Lumpy boulder.
    pub fn rock(&mut self, center: Vec3, radii: Vec3, c: Col, rng: &mut Rng) {
        let tilt = rng.sym(0.12);
        self.ico_with(center, radii, 1, |d| {
            (rng.range(0.75, 1.15), crate::util::shade(c, 0.82 + d.y * 0.22 + tilt))
        });
    }

    pub fn append(&mut self, other: &MeshBuilder) {
        let base = self.pos.len() as u32;
        self.pos.extend(other.pos.iter().map(|p| self.xf.transform_point3(*p)));
        self.col.extend_from_slice(&other.col);
        self.tris
            .extend(other.tris.iter().map(|t| [t[0] + base, t[1] + base, t[2] + base]));
    }

    /// Bakes the mesh. `flat` gives every triangle its own normal (the faceted
    /// look); otherwise normals are averaged over shared vertices.
    pub fn build(&self, flat: bool) -> Mesh {
        let mut pos: Vec<[f32; 3]> = Vec::new();
        let mut nor: Vec<[f32; 3]> = Vec::new();
        let mut col: Vec<Col> = Vec::new();
        let mut indices: Option<Vec<u32>> = None;

        if flat {
            pos.reserve(self.tris.len() * 3);
            for t in &self.tris {
                let (a, b, c) = (
                    self.pos[t[0] as usize],
                    self.pos[t[1] as usize],
                    self.pos[t[2] as usize],
                );
                let n = (b - a).cross(c - a);
                if n.length_squared() < 1e-12 {
                    continue;
                }
                let n = n.normalize().to_array();
                for &i in t {
                    let i = i as usize;
                    pos.push(self.pos[i].to_array());
                    nor.push(n);
                    col.push(self.col[i]);
                }
            }
        } else {
            let mut normals = vec![Vec3::ZERO; self.pos.len()];
            let mut idx = Vec::with_capacity(self.tris.len() * 3);
            for t in &self.tris {
                let (a, b, c) = (
                    self.pos[t[0] as usize],
                    self.pos[t[1] as usize],
                    self.pos[t[2] as usize],
                );
                let n = (b - a).cross(c - a);
                if n.length_squared() < 1e-12 {
                    continue;
                }
                for &i in t {
                    normals[i as usize] += n;
                    idx.push(i);
                }
            }
            pos = self.pos.iter().map(|p| p.to_array()).collect();
            nor = normals.iter().map(|n| n.normalize_or(Vec3::Y).to_array()).collect();
            col = self.col.clone();
            indices = Some(idx);
        }

        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
        if let Some(idx) = indices {
            mesh.insert_indices(Indices::U32(idx));
        }
        mesh
    }
}

/// Unit icosphere vertices and faces.
fn icosphere(sub: u32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let t = (1.0 + 5f32.sqrt()) / 2.0;
    let mut verts: Vec<Vec3> = [
        (-1.0, t, 0.0),
        (1.0, t, 0.0),
        (-1.0, -t, 0.0),
        (1.0, -t, 0.0),
        (0.0, -1.0, t),
        (0.0, 1.0, t),
        (0.0, -1.0, -t),
        (0.0, 1.0, -t),
        (t, 0.0, -1.0),
        (t, 0.0, 1.0),
        (-t, 0.0, -1.0),
        (-t, 0.0, 1.0),
    ]
    .iter()
    .map(|&(x, y, z)| Vec3::new(x, y, z).normalize())
    .collect();
    let mut faces: Vec<[u32; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    for _ in 0..sub {
        let mut cache: HashMap<(u32, u32), u32> = HashMap::new();
        let mut mid = |a: u32, b: u32, verts: &mut Vec<Vec3>| {
            let key = (a.min(b), a.max(b));
            *cache.entry(key).or_insert_with(|| {
                verts.push(((verts[a as usize] + verts[b as usize]) * 0.5).normalize());
                (verts.len() - 1) as u32
            })
        };
        let mut next = Vec::with_capacity(faces.len() * 4);
        for f in &faces {
            let (a, b, c) = (
                mid(f[0], f[1], &mut verts),
                mid(f[1], f[2], &mut verts),
                mid(f[2], f[0], &mut verts),
            );
            next.push([f[0], a, c]);
            next.push([f[1], b, a]);
            next.push([f[2], c, b]);
            next.push([a, b, c]);
        }
        faces = next;
    }
    (verts, faces)
}
