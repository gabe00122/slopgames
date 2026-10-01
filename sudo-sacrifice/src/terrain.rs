//! The ground: a heightfield that spells dig into, raise and shatter.
//!
//! Heights live on a regular grid of vertices. The mesh is cut into chunks,
//! and a chunk is rebuilt whenever an edit touches it. Queries (height,
//! slope, ray casts) follow the same triangles the mesh draws, so things
//! stand exactly on the ground they see.

use crate::{
    game::MatchEntity,
    meshkit::Col,
    util::{fbm2, lin, mix, noise2, shade, smoothstep},
};
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

/// Grid cells per side.
pub const CELLS: usize = 192;
/// Cell size in meters.
pub const CELL: f32 = 1.25;
/// Half the width of the map.
pub const HALF: f32 = CELLS as f32 * CELL * 0.5;
const N: usize = CELLS + 1;
const CHUNK: usize = 16;
const CHUNKS: usize = CELLS / CHUNK;
/// The surface of the data lakes.
pub const WATER: f32 = -0.6;
pub const MIN_H: f32 = -10.0;
pub const MAX_H: f32 = 44.0;

/// What a patch of ground is paved as.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Paving {
    None,
    /// The plaza around an altar, in the owner's colors.
    Altar(usize),
    /// The ring around a compute well.
    Well,
}

#[derive(Resource)]
pub struct Terrain {
    pub h: Vec<f32>,
    /// Charring from blasts, 0..1 per vertex.
    pub scar: Vec<f32>,
    /// 1 where the ground may not be changed (altars, wells).
    pub lock: Vec<f32>,
    pub paving: Vec<Paving>,
    /// Static per-vertex variation for the ground colors.
    tint: Vec<f32>,
    dirty: Vec<bool>,
    /// Bumped on every edit, so dependents can tell the ground moved.
    pub version: u64,
    meshes: Vec<Handle<Mesh>>,
}

fn idx(i: usize, j: usize) -> usize {
    j * N + i
}

/// World XZ of vertex `(i, j)`.
pub fn vert_pos(i: usize, j: usize) -> Vec2 {
    Vec2::new(-HALF + i as f32 * CELL, -HALF + j as f32 * CELL)
}

impl Terrain {
    /// Builds a terrain from a height function; `lock` and `paving` are
    /// filled in by the caller afterwards.
    pub fn generate(height: impl Fn(Vec2) -> f32, seed: u32) -> Terrain {
        let mut h = vec![0.0; N * N];
        let mut tint = vec![0.0; N * N];
        for j in 0..N {
            for i in 0..N {
                let p = vert_pos(i, j);
                h[idx(i, j)] = height(p).clamp(MIN_H, MAX_H);
                tint[idx(i, j)] =
                    fbm2(p.x / 9.0, p.y / 9.0, 3, seed + 5) * 0.7 + noise2(p.x * 0.7, p.y * 0.7, seed + 9) * 0.3;
            }
        }
        Terrain {
            h,
            scar: vec![0.0; N * N],
            lock: vec![0.0; N * N],
            paving: vec![Paving::None; N * N],
            tint,
            dirty: vec![true; CHUNKS * CHUNKS],
            version: 1,
            meshes: Vec::new(),
        }
    }

    fn at(&self, i: usize, j: usize) -> f32 {
        self.h[idx(i.min(N - 1), j.min(N - 1))]
    }

    /// Ground height, exact to the drawn triangles.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        let gx = ((x + HALF) / CELL).clamp(0.0, CELLS as f32 - 0.001);
        let gz = ((z + HALF) / CELL).clamp(0.0, CELLS as f32 - 0.001);
        let (i, j) = (gx as usize, gz as usize);
        let (fx, fz) = (gx - i as f32, gz - j as f32);
        let a = self.at(i, j);
        let b = self.at(i + 1, j);
        let c = self.at(i + 1, j + 1);
        let d = self.at(i, j + 1);
        if fx > fz {
            a + (b - a) * fx + (c - b) * fz
        } else {
            a + (c - d) * fx + (d - a) * fz
        }
    }

    pub fn height_v(&self, p: Vec2) -> f32 {
        self.height(p.x, p.y)
    }

    /// A point on the ground.
    pub fn ground(&self, p: Vec2) -> Vec3 {
        Vec3::new(p.x, self.height(p.x, p.y), p.y)
    }

    /// Smoothed surface normal.
    pub fn normal(&self, x: f32, z: f32) -> Vec3 {
        let e = 0.7;
        let dx = self.height(x + e, z) - self.height(x - e, z);
        let dz = self.height(x, z + e) - self.height(x, z - e);
        Vec3::new(-dx, 2.0 * e, -dz).normalize()
    }

    /// Whether `p` is inside the walkable part of the map.
    pub fn inside(p: Vec2, margin: f32) -> bool {
        p.x.abs() < HALF - margin && p.y.abs() < HALF - margin
    }

    pub fn clamp_inside(p: Vec2, margin: f32) -> Vec2 {
        p.clamp(Vec2::splat(-HALF + margin), Vec2::splat(HALF - margin))
    }

    /// The first point where a ray meets the ground.
    pub fn raycast(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<Vec3> {
        let dir = dir.normalize_or(Vec3::NEG_Y);
        let below = |p: Vec3| p.y < self.height(p.x, p.z);
        if below(origin) {
            return Some(origin);
        }
        let step = 0.5;
        let mut t = 0.0;
        while t < max {
            let next = t + step;
            let p = origin + dir * next;
            if !Terrain::inside(Vec2::new(p.x, p.z), 0.0) {
                return None;
            }
            if below(p) {
                let (mut lo, mut hi) = (t, next);
                for _ in 0..10 {
                    let mid = (lo + hi) * 0.5;
                    if below(origin + dir * mid) {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                return Some(origin + dir * hi);
            }
            t = next;
        }
        None
    }

    /// Whether the straight line between two points clears the ground.
    pub fn line_of_sight(&self, a: Vec3, b: Vec3) -> bool {
        let d = b - a;
        let steps = (d.length() / 1.5).ceil().max(1.0) as usize;
        (1..steps).all(|k| {
            let p = a + d * (k as f32 / steps as f32);
            p.y > self.height(p.x, p.z) - 0.2
        })
    }

    /// Changes heights within `radius` of `center` by `f(point)`, scaled by
    /// how unlocked each vertex is. Returns true if anything moved.
    pub fn edit(&mut self, center: Vec2, radius: f32, f: impl Fn(Vec2) -> f32) -> bool {
        let (i0, i1, j0, j1) = self.span(center, radius);
        let mut moved = false;
        for j in j0..=j1 {
            for i in i0..=i1 {
                let k = idx(i, j);
                let free = 1.0 - self.lock[k];
                if free <= 0.0 {
                    continue;
                }
                let delta = f(vert_pos(i, j)) * free;
                if delta.abs() > 1e-5 {
                    self.h[k] = (self.h[k] + delta).clamp(MIN_H, MAX_H);
                    moved = true;
                }
            }
        }
        if moved {
            self.touch(i0, i1, j0, j1);
        }
        moved
    }

    /// Chars the ground within `radius`.
    pub fn char(&mut self, center: Vec2, radius: f32, amount: f32) {
        let (i0, i1, j0, j1) = self.span(center, radius);
        for j in j0..=j1 {
            for i in i0..=i1 {
                let d = vert_pos(i, j).distance(center);
                let k = idx(i, j);
                let a = amount * smoothstep(radius, radius * 0.4, d);
                if a > 0.0 && self.paving[k] == Paving::None {
                    self.scar[k] = (self.scar[k] + a).min(1.0);
                }
            }
        }
        self.touch(i0, i1, j0, j1);
    }

    fn span(&self, center: Vec2, radius: f32) -> (usize, usize, usize, usize) {
        let lo = ((center - Vec2::splat(radius) + Vec2::splat(HALF)) / CELL).floor();
        let hi = ((center + Vec2::splat(radius) + Vec2::splat(HALF)) / CELL).ceil();
        let c = |v: f32| (v.max(0.0) as usize).min(N - 1);
        (c(lo.x), c(hi.x), c(lo.y), c(hi.y))
    }

    fn touch(&mut self, i0: usize, i1: usize, j0: usize, j1: usize) {
        let c0 = |v: usize| v.saturating_sub(1) / CHUNK;
        let c1 = |v: usize| (v / CHUNK).min(CHUNKS - 1);
        for cj in c0(j0)..=c1(j1) {
            for ci in c0(i0)..=c1(i1) {
                self.dirty[cj * CHUNKS + ci] = true;
            }
        }
        self.version += 1;
    }

    /// Ground color for a point, shared by the mesh and the minimap.
    fn color(&self, k: usize, h: f32, up: f32, parity: bool, cell_k: usize) -> Col {
        let tint = self.tint[k];
        let scar = self.scar[k];
        let mut c = match self.paving[cell_k] {
            Paving::Altar(team) => {
                let (a, b) = if team == 0 {
                    (0x8f97b3, 0x6f7896)
                } else {
                    (0xb3919c, 0x967079)
                };
                lin(if parity { a } else { b })
            }
            Paving::Well => lin(if parity { 0x93a9b0 } else { 0x7b9098 }),
            Paving::None => {
                let grass = mix(lin(0x557f39), lin(0x7c9e45), tint);
                let dry = mix(lin(0x8e8b52), lin(0xa39360), tint);
                let sand = lin(0xcbb27c);
                let bed = lin(0x2a5a5e);
                let mut c = if h < WATER - 0.4 {
                    mix(bed, sand, smoothstep(WATER - 3.0, WATER - 0.4, h))
                } else if h < WATER + 0.7 {
                    sand
                } else {
                    mix(grass, dry, smoothstep(9.0, 16.0, h + tint * 3.0))
                };
                // Steep ground is bare rock, banded like strata.
                let band = ((h / 1.1).floor() as i32).rem_euclid(3);
                let rock = match band {
                    0 => lin(0x6e6275),
                    1 => lin(0x7f7287),
                    _ => lin(0x635869),
                };
                c = mix(c, rock, smoothstep(0.86, 0.62, up));
                // Snow on the peaks around the rim.
                c = mix(
                    c,
                    lin(0xe9e6f2),
                    smoothstep(22.0, 27.0, h + tint * 4.0) * smoothstep(0.55, 0.8, up),
                );
                c
            }
        };
        c = mix(c, lin(0x2e2230), scar * 0.85);
        shade(c, 0.94 + tint * 0.12)
    }

    fn chunk_mesh(&self, ci: usize, cj: usize) -> Mesh {
        let cap = CHUNK * CHUNK * 6;
        let mut pos: Vec<[f32; 3]> = Vec::with_capacity(cap);
        let mut nor: Vec<[f32; 3]> = Vec::with_capacity(cap);
        let mut col: Vec<Col> = Vec::with_capacity(cap);
        for j in cj * CHUNK..(cj + 1) * CHUNK {
            for i in ci * CHUNK..(ci + 1) * CHUNK {
                let p = |i: usize, j: usize| {
                    let v = vert_pos(i, j);
                    Vec3::new(v.x, self.h[idx(i, j)], v.y)
                };
                let (a, b, c, d) = (p(i, j), p(i + 1, j), p(i + 1, j + 1), p(i, j + 1));
                let parity = (i / 2 + j / 2) % 2 == 0;
                let cell_k = idx(i, j);
                for (tri, ks) in [
                    ([a, d, c], [idx(i, j), idx(i, j + 1), idx(i + 1, j + 1)]),
                    ([a, c, b], [idx(i, j), idx(i + 1, j + 1), idx(i + 1, j)]),
                ] {
                    let n = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize_or(Vec3::Y);
                    let h = (tri[0].y + tri[1].y + tri[2].y) / 3.0;
                    // Color from the vertex nearest the triangle's middle, blended with its neighbors' char.
                    let k = ks[1];
                    let mut cc = self.color(k, h, n.y, parity, cell_k);
                    let scar = (self.scar[ks[0]] + self.scar[ks[1]] + self.scar[ks[2]]) / 3.0;
                    cc = mix(cc, lin(0x2e2230), (scar - self.scar[k]).max(0.0) * 0.8);
                    for v in tri {
                        pos.push(v.to_array());
                        nor.push(n.to_array());
                        col.push(cc);
                    }
                }
            }
        }
        let count = pos.len() as u32;
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
        mesh.insert_indices(Indices::U32((0..count).collect()));
        mesh
    }

    /// Top-down picture of the ground for the minimap, one pixel per cell,
    /// with hill shading. RGBA8, row 0 at -Z (north).
    pub fn map_pixels(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(CELLS * CELLS * 4);
        let light = Vec3::new(-0.6, 0.7, -0.4).normalize();
        for j in 0..CELLS {
            for i in 0..CELLS {
                let k = idx(i, j);
                let h = self.h[k];
                let dx = self.at(i + 1, j) - h;
                let dz = self.at(i, j + 1) - h;
                let n = Vec3::new(-dx, CELL, -dz).normalize();
                let parity = (i / 2 + j / 2) % 2 == 0;
                let mut c = self.color(k, h, n.y, parity, k);
                if h < WATER {
                    c = mix(c, lin(0x34d6e8), 0.55);
                }
                let lit = 0.55 + 0.6 * n.dot(light).max(0.0);
                let srgb = Color::LinearRgba(LinearRgba::new(c[0] * lit, c[1] * lit, c[2] * lit, 1.0)).to_srgba();
                out.extend_from_slice(&[
                    (srgb.red.clamp(0.0, 1.0) * 255.0) as u8,
                    (srgb.green.clamp(0.0, 1.0) * 255.0) as u8,
                    (srgb.blue.clamp(0.0, 1.0) * 255.0) as u8,
                    255,
                ]);
            }
        }
        out
    }
}

#[derive(Resource)]
pub struct TerrainMaterial(pub Handle<StandardMaterial>);

pub fn setup_material(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(TerrainMaterial(materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.93,
        reflectance: 0.18,
        ..default()
    })));
}

/// Spawns one entity per chunk for a freshly generated terrain.
pub fn spawn_chunks(commands: &mut Commands, meshes: &mut Assets<Mesh>, mat: &TerrainMaterial, terrain: &mut Terrain) {
    terrain.meshes.clear();
    for cj in 0..CHUNKS {
        for ci in 0..CHUNKS {
            let handle = meshes.add(terrain.chunk_mesh(ci, cj));
            terrain.meshes.push(handle.clone());
            commands.spawn((
                Mesh3d(handle),
                MeshMaterial3d(mat.0.clone()),
                Transform::IDENTITY,
                MatchEntity,
            ));
        }
    }
    terrain.dirty.fill(false);
}

/// Rebuilds the meshes of chunks that were edited.
pub fn rebuild(terrain: Option<ResMut<Terrain>>, mut meshes: ResMut<Assets<Mesh>>) {
    let Some(mut terrain) = terrain else { return };
    if terrain.meshes.is_empty() || !terrain.dirty.iter().any(|d| *d) {
        return;
    }
    for c in 0..CHUNKS * CHUNKS {
        if !terrain.dirty[c] {
            continue;
        }
        terrain.dirty[c] = false;
        let mesh = terrain.chunk_mesh(c % CHUNKS, c / CHUNKS);
        if let Some(mut m) = meshes.get_mut(&terrain.meshes[c]) {
            *m = mesh;
        }
    }
}

// ---------------------------------------------------------------------------
// Animated edits
// ---------------------------------------------------------------------------

/// The shape of a change to the ground.
#[derive(Clone, Copy, Debug)]
pub enum Shape {
    /// A bowl with a raised lip.
    Crater { center: Vec2, radius: f32, depth: f32 },
    /// A ridge along a segment.
    Wall {
        a: Vec2,
        b: Vec2,
        half_width: f32,
        height: f32,
    },
    /// A sharp spike.
    Spike { center: Vec2, radius: f32, height: f32 },
}

impl Shape {
    fn bounds(&self) -> (Vec2, f32) {
        match *self {
            Shape::Crater { center, radius, .. } => (center, radius * 1.45),
            Shape::Wall { a, b, half_width, .. } => ((a + b) * 0.5, a.distance(b) * 0.5 + half_width * 2.2),
            Shape::Spike { center, radius, .. } => (center, radius),
        }
    }

    /// Full height change at `p` for the whole edit.
    fn delta(&self, p: Vec2) -> f32 {
        match *self {
            Shape::Crater { center, radius, depth } => {
                let t = p.distance(center) / radius;
                if t < 1.0 {
                    -depth * (1.0 - t * t).powf(1.4)
                } else if t < 1.45 {
                    // The lip: thrown-out earth.
                    let u = (t - 1.0) / 0.45;
                    depth * 0.28 * (u * std::f32::consts::PI).sin().max(0.0)
                } else {
                    0.0
                }
            }
            Shape::Wall {
                a,
                b,
                half_width,
                height,
            } => {
                let ab = b - a;
                let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
                let d = p.distance(a + ab * t);
                // Flat top, steep flanks, tapered ends.
                let across = smoothstep(half_width * 2.0, half_width * 0.8, d);
                let along = smoothstep(0.0, 0.08, t) * smoothstep(1.0, 0.92, t);
                height * across * (0.35 + 0.65 * along)
            }
            Shape::Spike { center, radius, height } => {
                let t = p.distance(center) / radius;
                if t < 1.0 { height * (1.0 - t).powf(1.6) } else { 0.0 }
            }
        }
    }
}

/// An edit spread over time so the ground visibly moves.
pub struct TerrainOp {
    pub shape: Shape,
    pub duration: f32,
    pub elapsed: f32,
}

#[derive(Resource, Default)]
pub struct TerrainOps(pub Vec<TerrainOp>);

impl TerrainOps {
    pub fn add(&mut self, shape: Shape, duration: f32) {
        self.0.push(TerrainOp {
            shape,
            duration: duration.max(0.001),
            elapsed: 0.0,
        });
    }
}

/// Advances animated edits.
pub fn run_ops(time: Res<Time>, terrain: Option<ResMut<Terrain>>, mut ops: ResMut<TerrainOps>) {
    let Some(mut terrain) = terrain else {
        ops.0.clear();
        return;
    };
    let dt = time.delta_secs();
    for op in &mut ops.0 {
        let before = op.elapsed / op.duration;
        op.elapsed = (op.elapsed + dt).min(op.duration);
        let after = op.elapsed / op.duration;
        // Ease in and out so walls rise with weight.
        let e = |t: f32| t * t * (3.0 - 2.0 * t);
        let frac = e(after) - e(before);
        if frac <= 0.0 {
            continue;
        }
        let shape = op.shape;
        let (center, radius) = shape.bounds();
        terrain.edit(center, radius, |p| shape.delta(p) * frac);
    }
    ops.0.retain(|op| op.elapsed < op.duration);
}
