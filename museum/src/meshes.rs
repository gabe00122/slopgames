//! Custom mesh builders for shapes Bevy's primitives don't cover: tiled
//! floors and walls, surfaces of revolution, domes, gears, prisms, polyhedra
//! and tubes swept along curves.

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, Mesh},
    prelude::*,
    render::render_resource::PrimitiveTopology,
};
use std::f32::consts::{PI, TAU};

#[derive(Default)]
pub struct MeshData {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    idx: Vec<u32>,
}

impl MeshData {
    pub fn vert(&mut self, p: Vec3, n: Vec3, uv: Vec2) -> u32 {
        self.pos.push(p.into());
        self.nrm.push(n.into());
        self.uv.push(uv.into());
        (self.pos.len() - 1) as u32
    }

    /// Adds a triangle, choosing the winding so that its face agrees with the
    /// vertex normals.
    pub fn tri(&mut self, a: u32, b: u32, c: u32) {
        let p = |i: u32| Vec3::from(self.pos[i as usize]);
        let n = |i: u32| Vec3::from(self.nrm[i as usize]);
        let face = (p(b) - p(a)).cross(p(c) - p(a));
        if face.length_squared() < 1e-14 {
            return;
        }
        if face.dot(n(a) + n(b) + n(c)) >= 0.0 {
            self.idx.extend([a, b, c]);
        } else {
            self.idx.extend([a, c, b]);
        }
    }

    pub fn quad(&mut self, a: u32, b: u32, c: u32, d: u32) {
        self.tri(a, b, c);
        self.tri(a, c, d);
    }

    /// A flat quad with its own vertices, normal `n` and UVs.
    pub fn flat_quad(&mut self, p: [Vec3; 4], n: Vec3, uv: [Vec2; 4]) {
        let i: Vec<u32> = (0..4).map(|k| self.vert(p[k], n, uv[k])).collect();
        self.quad(i[0], i[1], i[2], i[3]);
    }

    /// A flat convex polygon (fan triangulated) with normal `n`.
    pub fn flat_poly(&mut self, pts: &[Vec3], n: Vec3) {
        let (t, b) = n.any_orthonormal_pair();
        let idx: Vec<u32> = pts
            .iter()
            .map(|p| self.vert(*p, n, Vec2::new(p.dot(t), p.dot(b))))
            .collect();
        for k in 1..idx.len() - 1 {
            self.tri(idx[0], idx[k], idx[k + 1]);
        }
    }

    pub fn build(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
            .with_inserted_indices(Indices::U32(self.idx))
    }

    /// Builds the mesh with tangents so it can take a normal map.
    pub fn build_with_tangents(self) -> Mesh {
        let mut mesh = self.build();
        mesh.generate_tangents().expect("mesh has positions, normals and UVs");
        mesh
    }
}

/// Horizontal rectangle centered at the origin, facing +Y (or -Y for
/// ceilings), with UVs repeating every `tile` meters.
pub fn plane(w: f32, d: f32, tile: f32, up: bool) -> Mesh {
    let mut m = MeshData::default();
    let (hw, hd) = (w / 2.0, d / 2.0);
    let n = if up { Vec3::Y } else { Vec3::NEG_Y };
    let p = [
        Vec3::new(-hw, 0.0, -hd),
        Vec3::new(-hw, 0.0, hd),
        Vec3::new(hw, 0.0, hd),
        Vec3::new(hw, 0.0, -hd),
    ];
    let uv = p.map(|p| Vec2::new(p.x + hw, p.z + hd) / tile);
    m.flat_quad(p, n, uv);
    m.build_with_tangents()
}

/// Axis-aligned box centered at the origin whose UVs repeat every `tile`
/// meters on every face.
pub fn tiled_box(size: Vec3, tile: f32) -> Mesh {
    let mut m = MeshData::default();
    let h = size / 2.0;
    for axis in 0..3 {
        for sign in [-1.0f32, 1.0] {
            let n = Vec3::AXES[axis] * sign;
            let u_axis = Vec3::AXES[(axis + 1) % 3];
            let v_axis = Vec3::AXES[(axis + 2) % 3];
            let (su, sv) = (size.dot(u_axis), size.dot(v_axis));
            let c = n * h;
            let corner = |a: f32, b: f32| c + u_axis * (a * su / 2.0) + v_axis * (b * sv / 2.0);
            let uv = |a: f32, b: f32| Vec2::new((a + 1.0) * su / 2.0, (b + 1.0) * sv / 2.0) / tile;
            m.flat_quad(
                [
                    corner(-1.0, -1.0),
                    corner(1.0, -1.0),
                    corner(1.0, 1.0),
                    corner(-1.0, 1.0),
                ],
                n,
                [uv(-1.0, -1.0), uv(1.0, -1.0), uv(1.0, 1.0), uv(-1.0, 1.0)],
            );
        }
    }
    m.build_with_tangents()
}

/// Surface of revolution around +Y. `profile` holds (radius, height) points
/// ordered from the bottom up; repeat a point to make a sharp crease.
pub fn lathe(profile: &[Vec2], segs: usize) -> Mesh {
    lathe_ex(profile, segs, 1.0)
}

pub fn lathe_ex(profile: &[Vec2], segs: usize, u_repeat: f32) -> Mesh {
    let mut m = MeshData::default();
    let n = profile.len();
    let mut normals2 = Vec::with_capacity(n);
    for i in 0..n {
        let mut t = Vec2::ZERO;
        if i > 0 {
            t += (profile[i] - profile[i - 1]).normalize_or_zero();
        }
        if i + 1 < n {
            t += (profile[i + 1] - profile[i]).normalize_or_zero();
        }
        let t = t.normalize_or(Vec2::Y);
        normals2.push(Vec2::new(t.y, -t.x));
    }
    let mut length = vec![0.0];
    for i in 1..n {
        length.push(length[i - 1] + profile[i].distance(profile[i - 1]));
    }
    let total = length[n - 1].max(1e-6);
    let mut rows = Vec::with_capacity(n);
    for i in 0..n {
        let mut row = Vec::with_capacity(segs + 1);
        for j in 0..=segs {
            let a = j as f32 / segs as f32 * TAU;
            let (s, c) = a.sin_cos();
            let p = Vec3::new(profile[i].x * s, profile[i].y, profile[i].x * c);
            let nn = Vec3::new(normals2[i].x * s, normals2[i].y, normals2[i].x * c);
            let uv = Vec2::new(j as f32 / segs as f32 * u_repeat, 1.0 - length[i] / total);
            row.push(m.vert(p, nn.normalize_or(Vec3::Y), uv));
        }
        rows.push(row);
    }
    for i in 0..n - 1 {
        if profile[i].distance(profile[i + 1]) < 1e-6 {
            continue;
        }
        for j in 0..segs {
            m.quad(rows[i][j], rows[i][j + 1], rows[i + 1][j + 1], rows[i + 1][j]);
        }
    }
    m.build_with_tangents()
}

/// Spherical dome seen from inside, from elevation `el0` to `el1`
/// (radians). UVs repeat `u_rep` times around and `v_rep` times up.
pub fn dome(radius: f32, el0: f32, el1: f32, rings: usize, segs: usize, u_rep: f32, v_rep: f32) -> Mesh {
    let mut m = MeshData::default();
    let mut rows = Vec::new();
    for i in 0..=rings {
        let el = el0 + (el1 - el0) * i as f32 / rings as f32;
        let mut row = Vec::new();
        for j in 0..=segs {
            let az = j as f32 / segs as f32 * TAU;
            let d = Vec3::new(el.cos() * az.sin(), el.sin(), el.cos() * az.cos());
            let uv = Vec2::new(j as f32 / segs as f32 * u_rep, (1.0 - i as f32 / rings as f32) * v_rep);
            row.push(m.vert(d * radius, -d, uv));
        }
        rows.push(row);
    }
    for i in 0..rings {
        for j in 0..segs {
            m.quad(rows[i][j], rows[i][j + 1], rows[i + 1][j + 1], rows[i + 1][j]);
        }
    }
    m.build_with_tangents()
}

/// Flat, downward-facing ring that fills the gap between a regular polygon
/// with `sides` sides (apothem `apothem`, first side facing -Z) and an inner
/// circle. Used where the round dome meets the octagonal walls.
pub fn polygon_ring_ceiling(sides: usize, apothem: f32, inner: f32, segs: usize) -> Mesh {
    let mut m = MeshData::default();
    let sector = TAU / sides as f32;
    for j in 0..segs {
        let a0 = j as f32 / segs as f32 * TAU;
        let a1 = (j + 1) as f32 / segs as f32 * TAU;
        let outer = |a: f32| {
            // Angle measured from -Z toward +X, matching the gallery layout.
            let rel = (a + sector / 2.0).rem_euclid(sector) - sector / 2.0;
            apothem / rel.cos()
        };
        let pt = |a: f32, r: f32| Vec3::new(a.sin() * r, 0.0, -a.cos() * r);
        let q = [pt(a0, inner), pt(a1, inner), pt(a1, outer(a1)), pt(a0, outer(a0))];
        let uv = q.map(|p| Vec2::new(p.x, p.z) / 4.0);
        m.flat_quad(q, Vec3::NEG_Y, uv);
    }
    m.build()
}

/// A flat, star-shaped outline extruded along Z (front face at +Z/2). `r(a)`
/// gives the outline radius at angle `a`; `hole` is the axle radius.
pub fn radial_extrude(r: impl Fn(f32) -> f32, n: usize, thickness: f32, hole: f32) -> Mesh {
    let mut m = MeshData::default();
    let hz = thickness / 2.0;
    let pts: Vec<Vec2> = (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * TAU;
            Vec2::from_angle(a) * r(a)
        })
        .collect();
    // Caps
    for z in [hz, -hz] {
        let nz = Vec3::Z * z.signum();
        let center = m.vert(Vec3::new(0.0, 0.0, z), nz, Vec2::splat(0.5));
        for i in 0..n {
            let (p0, p1) = (pts[i], pts[(i + 1) % n]);
            let uv = |p: Vec2| p / (2.0 * p.length().max(1e-3)) + 0.5;
            let o0 = m.vert(p0.extend(z), nz, uv(p0));
            let o1 = m.vert(p1.extend(z), nz, uv(p1));
            if hole > 0.0 {
                let h0 = p0.normalize() * hole;
                let h1 = p1.normalize() * hole;
                let i0 = m.vert(h0.extend(z), nz, uv(h0));
                let i1 = m.vert(h1.extend(z), nz, uv(h1));
                m.quad(i0, o0, o1, i1);
            } else {
                m.tri(center, o0, o1);
            }
        }
    }
    // Outer wall
    for i in 0..n {
        let (p0, p1) = (pts[i], pts[(i + 1) % n]);
        let e = p1 - p0;
        let nn = Vec3::new(e.y, -e.x, 0.0).normalize_or(p0.extend(0.0).normalize());
        let u0 = i as f32 / n as f32;
        let u1 = (i + 1) as f32 / n as f32;
        m.flat_quad(
            [p0.extend(-hz), p1.extend(-hz), p1.extend(hz), p0.extend(hz)],
            nn,
            [
                Vec2::new(u0, 0.0),
                Vec2::new(u1, 0.0),
                Vec2::new(u1, 1.0),
                Vec2::new(u0, 1.0),
            ],
        );
    }
    // Axle hole wall
    if hole > 0.0 {
        for i in 0..n {
            let (p0, p1) = (pts[i].normalize() * hole, pts[(i + 1) % n].normalize() * hole);
            let nn = -((p0 + p1) / 2.0).normalize().extend(0.0);
            m.flat_quad(
                [p0.extend(-hz), p1.extend(-hz), p1.extend(hz), p0.extend(hz)],
                nn,
                [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
            );
        }
    }
    m.build()
}

/// A spur gear lying in the XY plane (axle along Z).
pub fn gear(radius: f32, teeth: usize, thickness: f32, hole: f32) -> Mesh {
    let depth = (radius * 0.12).min(0.06).max(radius * 0.06);
    let tooth = TAU / teeth as f32;
    radial_extrude(
        |a| {
            let t = (a / tooth).fract();
            // trapezoid tooth: rise, top, fall, root
            let h = if t < 0.15 {
                t / 0.15
            } else if t < 0.45 {
                1.0
            } else if t < 0.6 {
                1.0 - (t - 0.45) / 0.15
            } else {
                0.0
            };
            radius - depth + depth * h
        },
        teeth * 8,
        thickness,
        hole,
    )
}

/// Straight prism: a convex polygon in the XY plane extruded along Z.
pub fn prism(points: &[Vec2], depth: f32) -> Mesh {
    let mut m = MeshData::default();
    let hz = depth / 2.0;
    let front: Vec<Vec3> = points.iter().map(|p| p.extend(hz)).collect();
    let back: Vec<Vec3> = points.iter().rev().map(|p| p.extend(-hz)).collect();
    m.flat_poly(&front, Vec3::Z);
    m.flat_poly(&back, Vec3::NEG_Z);
    let c: Vec2 = points.iter().copied().sum::<Vec2>() / points.len() as f32;
    for i in 0..points.len() {
        let (p0, p1) = (points[i], points[(i + 1) % points.len()]);
        let e = p1 - p0;
        let mut n = Vec2::new(e.y, -e.x).normalize();
        if n.dot((p0 + p1) / 2.0 - c) < 0.0 {
            n = -n;
        }
        m.flat_quad(
            [p0.extend(-hz), p1.extend(-hz), p1.extend(hz), p0.extend(hz)],
            n.extend(0.0),
            [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        );
    }
    m.build()
}

pub fn regular_polygon(sides: usize, radius: f32, rot: f32) -> Vec<Vec2> {
    (0..sides)
        .map(|i| Vec2::from_angle(rot + i as f32 / sides as f32 * TAU) * radius)
        .collect()
}

/// Flat-shaded convex polyhedron; faces may list vertices in any winding.
pub fn polyhedron(verts: &[Vec3], faces: &[Vec<usize>]) -> Mesh {
    let mut m = MeshData::default();
    for f in faces {
        let pts: Vec<Vec3> = f.iter().map(|&i| verts[i]).collect();
        let c = pts.iter().copied().sum::<Vec3>() / pts.len() as f32;
        let mut n = (pts[1] - pts[0]).cross(pts[2] - pts[0]).normalize();
        if n.dot(c) < 0.0 {
            n = -n;
        }
        // Order the points around the face center so the fan is valid.
        let (t, b) = n.any_orthonormal_pair();
        let mut sorted = pts.clone();
        sorted.sort_by(|p, q| {
            let ap = (*p - c).dot(b).atan2((*p - c).dot(t));
            let aq = (*q - c).dot(b).atan2((*q - c).dot(t));
            ap.partial_cmp(&aq).unwrap()
        });
        m.flat_poly(&sorted, n);
    }
    m.build()
}

/// Faces of the convex hull for the platonic solids, found by grouping
/// vertices that share a supporting plane. Works for any vertex set whose
/// faces are all at the same distance from the origin.
pub fn platonic(verts: &[Vec3]) -> Mesh {
    let r = verts.iter().map(|v| v.length()).fold(0.0, f32::max);
    let mut faces: Vec<Vec<usize>> = Vec::new();
    let mut normals: Vec<Vec3> = Vec::new();
    let n = verts.len();
    for a in 0..n {
        for b in a + 1..n {
            for c in b + 1..n {
                let nn = (verts[b] - verts[a]).cross(verts[c] - verts[a]);
                if nn.length_squared() < 1e-8 {
                    continue;
                }
                let mut nn = nn.normalize();
                if nn.dot(verts[a]) < 0.0 {
                    nn = -nn;
                }
                let d = nn.dot(verts[a]);
                if verts.iter().any(|v| nn.dot(*v) > d + 1e-4 * r) {
                    continue;
                }
                if normals.iter().any(|m| m.dot(nn) > 0.9999) {
                    continue;
                }
                let face: Vec<usize> = (0..n).filter(|&i| (nn.dot(verts[i]) - d).abs() < 1e-4 * r).collect();
                normals.push(nn);
                faces.push(face);
            }
        }
    }
    polyhedron(verts, &faces)
}

pub fn tetrahedron(r: f32) -> Mesh {
    let s = r / 3f32.sqrt();
    platonic(&[
        Vec3::new(1.0, 1.0, 1.0) * s,
        Vec3::new(1.0, -1.0, -1.0) * s,
        Vec3::new(-1.0, 1.0, -1.0) * s,
        Vec3::new(-1.0, -1.0, 1.0) * s,
    ])
}

pub fn octahedron(r: f32) -> Mesh {
    platonic(&[
        Vec3::X * r,
        Vec3::NEG_X * r,
        Vec3::Y * r,
        Vec3::NEG_Y * r,
        Vec3::Z * r,
        Vec3::NEG_Z * r,
    ])
}

pub fn icosahedron(r: f32) -> Mesh {
    let p = (1.0 + 5f32.sqrt()) / 2.0;
    let mut v = Vec::new();
    for (a, b) in [(1.0, p), (-1.0, p), (1.0, -p), (-1.0, -p)] {
        v.push(Vec3::new(0.0, a, b));
        v.push(Vec3::new(a, b, 0.0));
        v.push(Vec3::new(b, 0.0, a));
    }
    let s = r / v[0].length();
    platonic(&v.iter().map(|x| *x * s).collect::<Vec<_>>())
}

pub fn dodecahedron(r: f32) -> Mesh {
    let p = (1.0 + 5f32.sqrt()) / 2.0;
    let ip = 1.0 / p;
    let mut v = Vec::new();
    for x in [-1.0, 1.0] {
        for y in [-1.0, 1.0] {
            for z in [-1.0, 1.0] {
                v.push(Vec3::new(x, y, z));
            }
        }
    }
    for a in [-1.0, 1.0] {
        for b in [-1.0, 1.0] {
            v.push(Vec3::new(0.0, a * ip, b * p));
            v.push(Vec3::new(a * ip, b * p, 0.0));
            v.push(Vec3::new(a * p, 0.0, b * ip));
        }
    }
    let s = r / 3f32.sqrt();
    platonic(&v.iter().map(|x| *x * s).collect::<Vec<_>>())
}

/// Square pyramid with its base centered at the origin.
pub fn pyramid(base: f32, height: f32) -> Mesh {
    let h = base / 2.0;
    let verts = [
        Vec3::new(-h, 0.0, -h),
        Vec3::new(h, 0.0, -h),
        Vec3::new(h, 0.0, h),
        Vec3::new(-h, 0.0, h),
        Vec3::new(0.0, height, 0.0),
    ];
    // Shift so the centroid is inside for the outward-normal test.
    let c = Vec3::new(0.0, height / 4.0, 0.0);
    let shifted: Vec<Vec3> = verts.iter().map(|v| *v - c).collect();
    let faces = vec![
        vec![0, 1, 2, 3],
        vec![0, 1, 4],
        vec![1, 2, 4],
        vec![2, 3, 4],
        vec![3, 0, 4],
    ];
    let mut mesh = polyhedron(&shifted, &faces);
    mesh.translate_by(c);
    mesh
}

/// A tube of circular cross-section swept along `path`, with per-point
/// radius. Frames are parallel-transported to avoid twisting.
pub fn tube(path: &[Vec3], radius: impl Fn(usize) -> f32, segs: usize, caps: bool) -> Mesh {
    let mut m = MeshData::default();
    let n = path.len();
    let tangent = |i: usize| {
        let a = path[i.saturating_sub(1)];
        let b = path[(i + 1).min(n - 1)];
        (b - a).normalize_or(Vec3::Y)
    };
    let mut normal = tangent(0).any_orthonormal_vector();
    let mut rings = Vec::with_capacity(n);
    let mut prev_t = tangent(0);
    for i in 0..n {
        let t = tangent(i);
        // parallel transport
        let axis = prev_t.cross(t);
        if axis.length_squared() > 1e-10 {
            let ang = prev_t.dot(t).clamp(-1.0, 1.0).acos();
            normal = Quat::from_axis_angle(axis.normalize(), ang) * normal;
        }
        prev_t = t;
        let binormal = t.cross(normal);
        let r = radius(i);
        let mut ring = Vec::with_capacity(segs + 1);
        for j in 0..=segs {
            let a = j as f32 / segs as f32 * TAU;
            let d = normal * a.cos() + binormal * a.sin();
            ring.push(m.vert(
                path[i] + d * r,
                d,
                Vec2::new(j as f32 / segs as f32, i as f32 / (n - 1) as f32),
            ));
        }
        rings.push(ring);
    }
    for i in 0..n - 1 {
        for j in 0..segs {
            m.quad(rings[i][j], rings[i][j + 1], rings[i + 1][j + 1], rings[i + 1][j]);
        }
    }
    if caps {
        for (i, dir) in [(0, -tangent(0)), (n - 1, tangent(n - 1))] {
            let r = radius(i);
            let pts: Vec<Vec3> = (0..segs)
                .map(|j| {
                    let p = Vec3::from(m.pos[rings[i][j] as usize]);
                    path[i] + (p - path[i]).normalize_or_zero() * r
                })
                .collect();
            m.flat_poly(&pts, dir);
        }
    }
    m.build()
}

/// Points along a helix around +Y.
pub fn helix(radius: f32, height: f32, turns: f32, phase: f32, samples: usize) -> Vec<Vec3> {
    (0..samples)
        .map(|i| {
            let t = i as f32 / (samples - 1) as f32;
            let a = phase + t * turns * TAU;
            Vec3::new(a.cos() * radius, t * height, a.sin() * radius)
        })
        .collect()
}

/// Catenary-ish sag between two points, for ropes and cables.
pub fn sag(a: Vec3, b: Vec3, depth: f32, samples: usize) -> Vec<Vec3> {
    (0..samples)
        .map(|i| {
            let t = i as f32 / (samples - 1) as f32;
            a.lerp(b, t) - Vec3::Y * depth * (PI * t).sin()
        })
        .collect()
}
