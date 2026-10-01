//! Low-poly mesh construction. Everything in the game is built from these
//! primitives with per-vertex colors; beast geometry also carries skin weights
//! so it bends with the skeleton.

use crate::util::Rng;
use bevy::{
    asset::RenderAssetUsages,
    math::Affine3A,
    mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
    prelude::*,
};
use std::{collections::HashMap, f32::consts::TAU};

pub type Col = [f32; 4];

/// Up to four bone influences for one vertex or anchor point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Skin {
    pub j: [u16; 4],
    pub w: [f32; 4],
}

impl Default for Skin {
    fn default() -> Self {
        Skin::one(0)
    }
}

impl Skin {
    pub const fn one(j: usize) -> Skin {
        Skin {
            j: [j as u16, 0, 0, 0],
            w: [1.0, 0.0, 0.0, 0.0],
        }
    }

    /// `1 - t` on bone `a`, `t` on bone `b`.
    pub fn two(a: usize, b: usize, t: f32) -> Skin {
        let t = t.clamp(0.0, 1.0);
        if a == b || t <= 0.0 {
            return Skin::one(a);
        }
        if t >= 1.0 {
            return Skin::one(b);
        }
        Skin {
            j: [a as u16, b as u16, 0, 0],
            w: [1.0 - t, t, 0.0, 0.0],
        }
    }

    /// Blends two skins, keeping the four strongest influences.
    pub fn mix(a: Skin, b: Skin, t: f32) -> Skin {
        let t = t.clamp(0.0, 1.0);
        let mut acc: Vec<(u16, f32)> = Vec::with_capacity(8);
        let mut push = |j: u16, w: f32| {
            if w <= 0.0 {
                return;
            }
            match acc.iter_mut().find(|e| e.0 == j) {
                Some(e) => e.1 += w,
                None => acc.push((j, w)),
            }
        };
        for i in 0..4 {
            push(a.j[i], a.w[i] * (1.0 - t));
            push(b.j[i], b.w[i] * t);
        }
        acc.sort_by(|x, y| y.1.total_cmp(&x.1));
        acc.truncate(4);
        let sum: f32 = acc.iter().map(|e| e.1).sum();
        let mut s = Skin { j: [0; 4], w: [0.0; 4] };
        for (i, (j, w)) in acc.iter().enumerate() {
            s.j[i] = *j;
            s.w[i] = w / sum;
        }
        s
    }

    /// The blended bone matrix for this skin (linear blend skinning).
    pub fn apply(&self, mats: &[Affine3A]) -> Affine3A {
        let mut m = Mat3::ZERO;
        let mut t = Vec3::ZERO;
        for i in 0..4 {
            let w = self.w[i];
            if w > 0.0 {
                let a = &mats[self.j[i] as usize];
                m += Mat3::from(a.matrix3) * w;
                t += Vec3::from(a.translation) * w;
            }
        }
        Affine3A::from_mat3_translation(m, t)
    }
}

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
    skin: Vec<Skin>,
    tris: Vec<[u32; 3]>,
    /// Applied to every vertex added through [`MeshBuilder::v`].
    pub xf: Affine3A,
    /// Skin given to every vertex added through [`MeshBuilder::v`].
    pub cur: Skin,
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
            skin: Vec::new(),
            tris: Vec::new(),
            xf: Affine3A::IDENTITY,
            cur: Skin::one(0),
        }
    }

    pub fn triangles(&self) -> usize {
        self.tris.len()
    }

    pub fn vertex_count(&self) -> usize {
        self.pos.len()
    }

    /// Reassigns the skin of every vertex from index `first` on, by position.
    pub fn reskin_from(&mut self, first: usize, f: impl Fn(Vec3) -> Skin) {
        for i in first..self.pos.len() {
            self.skin[i] = f(self.pos[i]);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }

    /// Adds a vertex in local space (transformed by `xf`, skinned to `cur`).
    pub fn v(&mut self, p: Vec3, c: Col) -> u32 {
        self.pos.push(self.xf.transform_point3(p));
        self.col.push(c);
        self.skin.push(self.cur);
        (self.pos.len() - 1) as u32
    }

    /// Adds a vertex with explicit skin, ignoring `xf`.
    pub fn vs(&mut self, p: Vec3, c: Col, skin: Skin) -> u32 {
        self.pos.push(p);
        self.col.push(c);
        self.skin.push(skin);
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

    /// Quad wound so it faces away from `inside`, whatever order the corners come in.
    pub fn quad_facing(&mut self, p: [Vec3; 4], c: Col, inside: Vec3) {
        let n = (p[1] - p[0]).cross(p[3] - p[0]);
        if n.dot(p[0] - inside) >= 0.0 {
            self.quad_p(p, c);
        } else {
            self.quad_p([p[3], p[2], p[1], p[0]], c);
        }
    }

    /// Triangle wound so its normal points along `dir`.
    pub fn tri_facing(&mut self, p: [Vec3; 3], c: Col, dir: Vec3) {
        if (p[1] - p[0]).cross(p[2] - p[0]).dot(dir) >= 0.0 {
            self.tri_p(p, c);
        } else {
            self.tri_p([p[0], p[2], p[1]], c);
        }
    }

    /// Double-sided quad (leaves, fins, flags).
    pub fn quad_2(&mut self, p: [Vec3; 4], c: Col) {
        self.quad_p(p, c);
        self.quad_p([p[3], p[2], p[1], p[0]], c);
    }

    pub fn tri_2(&mut self, p: [Vec3; 3], c: Col) {
        self.tri_p(p, c);
        self.tri_p([p[2], p[1], p[0]], c);
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
        // u × v = a, with v pointing "up" and u to the side.
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
    pub fn cyl(&mut self, a: Vec3, b: Vec3, ra: f32, rb: f32, sides: usize, c: Col, caps: bool) {
        self.tube(&[(a, ra), (b, rb)], sides, c, caps);
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
        let p = |x: f32, y: f32, z: f32| center + Vec3::new(x * half.x, y * half.y, z * half.z);
        self.quad_p([p(-1., -1., 1.), p(1., -1., 1.), p(1., 1., 1.), p(-1., 1., 1.)], c);
        self.quad_p([p(1., -1., -1.), p(-1., -1., -1.), p(-1., 1., -1.), p(1., 1., -1.)], c);
        self.quad_p([p(1., -1., 1.), p(1., -1., -1.), p(1., 1., -1.), p(1., 1., 1.)], c);
        self.quad_p([p(-1., -1., -1.), p(-1., -1., 1.), p(-1., 1., 1.), p(-1., 1., -1.)], c);
        self.quad_p([p(-1., 1., 1.), p(1., 1., 1.), p(1., 1., -1.), p(-1., 1., -1.)], c);
        self.quad_p([p(-1., -1., -1.), p(1., -1., -1.), p(1., -1., 1.), p(-1., -1., 1.)], c);
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

    /// Ellipsoid from a subdivided icosahedron. `color` sees the unit direction.
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

    /// Lumpy boulder.
    pub fn rock(&mut self, center: Vec3, radii: Vec3, c: Col, rng: &mut Rng) {
        let tilt = rng.sym(0.12);
        self.ico_with(center, radii, 1, |d| {
            (rng.range(0.78, 1.12), crate::util::shade(c, 0.85 + d.y * 0.2 + tilt))
        });
    }

    /// Appends `other` moved by `xf` and re-skinned to `skin`.
    pub fn append_at(&mut self, other: &MeshBuilder, xf: Affine3A, skin: Skin) {
        let base = self.pos.len() as u32;
        self.pos.extend(other.pos.iter().map(|p| xf.transform_point3(*p)));
        self.col.extend_from_slice(&other.col);
        self.skin.extend(std::iter::repeat_n(skin, other.pos.len()));
        self.tris
            .extend(other.tris.iter().map(|t| [t[0] + base, t[1] + base, t[2] + base]));
    }

    /// Bakes the mesh. `flat` gives every triangle its own normal (the faceted
    /// look); otherwise normals are averaged over shared vertices.
    pub fn build(&self, flat: bool, skinned: bool) -> Mesh {
        let mut pos: Vec<[f32; 3]> = Vec::new();
        let mut nor: Vec<[f32; 3]> = Vec::new();
        let mut col: Vec<Col> = Vec::new();
        let mut ji: Vec<[u16; 4]> = Vec::new();
        let mut jw: Vec<[f32; 4]> = Vec::new();
        let mut indices: Option<Vec<u32>> = None;

        if flat {
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
                    if skinned {
                        ji.push(self.skin[i].j);
                        jw.push(self.skin[i].w);
                    }
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
            if skinned {
                ji = self.skin.iter().map(|s| s.j).collect();
                jw = self.skin.iter().map(|s| s.w).collect();
            }
            indices = Some(idx);
        }

        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
        if skinned {
            mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_INDEX, VertexAttributeValues::Uint16x4(ji));
            mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, jw);
        }
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
