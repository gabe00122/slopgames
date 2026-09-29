//! Voxel world: chunk storage, block access, damage, raycasting.

pub mod block;
pub mod gen;

pub use block::{Block, ContainerKind, Tile};

use glam::{IVec3, Vec3};
use std::collections::{HashMap, HashSet};

pub const CHUNK_SIZE: i32 = 16;
const CHUNK_VOL: usize = (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize;

#[derive(Clone)]
pub struct Chunk {
    blocks: Vec<Block>,
    non_air: u32,
}

impl Chunk {
    fn new() -> Self {
        Self {
            blocks: vec![Block::Air; CHUNK_VOL],
            non_air: 0,
        }
    }

    #[inline]
    fn idx(l: IVec3) -> usize {
        (l.y * CHUNK_SIZE * CHUNK_SIZE + l.z * CHUNK_SIZE + l.x) as usize
    }

    #[inline]
    pub fn get(&self, l: IVec3) -> Block {
        self.blocks[Self::idx(l)]
    }

    fn set(&mut self, l: IVec3, b: Block) {
        let i = Self::idx(l);
        let old = self.blocks[i];
        if old.is_air() && !b.is_air() {
            self.non_air += 1;
        } else if !old.is_air() && b.is_air() {
            self.non_air -= 1;
        }
        self.blocks[i] = b;
    }

    pub fn is_empty(&self) -> bool {
        self.non_air == 0
    }
}

/// Result of a voxel raycast.
#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    /// Block coordinate that was hit.
    pub pos: IVec3,
    /// Normal of the face that was entered (zero if the ray started inside the block).
    pub normal: IVec3,
    /// Distance along the (normalized) ray at which the block was entered.
    pub t: f32,
    pub block: Block,
}

pub struct World {
    pub chunks_dim: IVec3,
    pub size: IVec3,
    chunks: Vec<Chunk>,
    /// Highest sky-blocking block per column (-1 if none). Used for skylight.
    heightmap: Vec<i32>,
    damage: HashMap<IVec3, f32>,
    dirty: HashSet<IVec3>,
}

impl World {
    pub fn new(chunks_dim: IVec3) -> Self {
        let count = (chunks_dim.x * chunks_dim.y * chunks_dim.z) as usize;
        let size = chunks_dim * CHUNK_SIZE;
        Self {
            chunks_dim,
            size,
            chunks: vec![Chunk::new(); count],
            heightmap: vec![-1; (size.x * size.z) as usize],
            damage: HashMap::new(),
            dirty: HashSet::new(),
        }
    }

    #[inline]
    fn chunk_index(&self, c: IVec3) -> Option<usize> {
        if c.x < 0
            || c.y < 0
            || c.z < 0
            || c.x >= self.chunks_dim.x
            || c.y >= self.chunks_dim.y
            || c.z >= self.chunks_dim.z
        {
            return None;
        }
        Some((c.y * self.chunks_dim.x * self.chunks_dim.z + c.z * self.chunks_dim.x + c.x) as usize)
    }

    pub fn chunk(&self, c: IVec3) -> Option<&Chunk> {
        self.chunk_index(c).map(|i| &self.chunks[i])
    }

    pub fn chunk_coords(&self) -> impl Iterator<Item = IVec3> + '_ {
        let d = self.chunks_dim;
        (0..d.y).flat_map(move |y| (0..d.z).flat_map(move |z| (0..d.x).map(move |x| IVec3::new(x, y, z))))
    }

    #[inline]
    pub fn in_bounds(&self, p: IVec3) -> bool {
        p.x >= 0 && p.y >= 0 && p.z >= 0 && p.x < self.size.x && p.y < self.size.y && p.z < self.size.z
    }

    /// Block at a world position. Outside the map horizontally and below the
    /// floor there is an indestructible wall; above the map is open air.
    #[inline]
    pub fn get(&self, p: IVec3) -> Block {
        if p.y >= self.size.y {
            return Block::Air;
        }
        if p.y < 0 || p.x < 0 || p.z < 0 || p.x >= self.size.x || p.z >= self.size.z {
            return Block::Bedrock;
        }
        let c = IVec3::new(p.x >> 4, p.y >> 4, p.z >> 4);
        let i = (c.y * self.chunks_dim.x * self.chunks_dim.z + c.z * self.chunks_dim.x + c.x) as usize;
        self.chunks[i].get(p & 15)
    }

    #[inline]
    pub fn get_xyz(&self, x: i32, y: i32, z: i32) -> Block {
        self.get(IVec3::new(x, y, z))
    }

    /// Set a block during world generation (no dirty tracking).
    pub fn set_gen(&mut self, p: IVec3, b: Block) {
        if !self.in_bounds(p) {
            return;
        }
        let c = IVec3::new(p.x >> 4, p.y >> 4, p.z >> 4);
        if let Some(i) = self.chunk_index(c) {
            self.chunks[i].set(p & 15, b);
        }
    }

    pub fn fill_gen(&mut self, min: IVec3, max_inclusive: IVec3, b: Block) {
        for y in min.y..=max_inclusive.y {
            for z in min.z..=max_inclusive.z {
                for x in min.x..=max_inclusive.x {
                    self.set_gen(IVec3::new(x, y, z), b);
                }
            }
        }
    }

    /// Call once generation is complete.
    pub fn finish_generation(&mut self) {
        for z in 0..self.size.z {
            for x in 0..self.size.x {
                self.recompute_column(x, z);
            }
        }
        self.dirty.clear();
        self.damage.clear();
    }

    fn recompute_column(&mut self, x: i32, z: i32) -> i32 {
        let mut h = -1;
        for y in (0..self.size.y).rev() {
            let b = self.get_xyz(x, y, z);
            if b.is_opaque() || b == Block::Leaves {
                h = y;
                break;
            }
        }
        self.heightmap[(z * self.size.x + x) as usize] = h;
        h
    }

    /// Is the air cell at `p` lit by the sky (nothing opaque above it)?
    #[inline]
    pub fn sky_at(&self, p: IVec3) -> bool {
        if p.x < 0 || p.z < 0 || p.x >= self.size.x || p.z >= self.size.z {
            return true;
        }
        p.y > self.heightmap[(p.z * self.size.x + p.x) as usize]
    }

    /// Topmost solid block in a column, or -1.
    pub fn top_solid_y(&self, x: i32, z: i32) -> i32 {
        for y in (0..self.size.y).rev() {
            if self.get_xyz(x, y, z).is_solid() {
                return y;
            }
        }
        -1
    }

    fn mark_dirty_around(&mut self, p: IVec3) {
        let c = IVec3::new(p.x >> 4, p.y >> 4, p.z >> 4);
        self.dirty.insert(c);
        let l = p & 15;
        for axis in 0..3 {
            if l[axis] == 0 {
                let mut n = c;
                n[axis] -= 1;
                self.dirty.insert(n);
            }
            if l[axis] == CHUNK_SIZE - 1 {
                let mut n = c;
                n[axis] += 1;
                self.dirty.insert(n);
            }
        }
    }

    /// Set a block at runtime, marking affected chunks for remeshing.
    pub fn set(&mut self, p: IVec3, b: Block) {
        if !self.in_bounds(p) {
            return;
        }
        let c = IVec3::new(p.x >> 4, p.y >> 4, p.z >> 4);
        let Some(i) = self.chunk_index(c) else { return };
        if self.chunks[i].get(p & 15) == b {
            return;
        }
        self.chunks[i].set(p & 15, b);
        self.damage.remove(&p);
        self.mark_dirty_around(p);

        let old_h = self.heightmap[(p.z * self.size.x + p.x) as usize];
        let new_h = self.recompute_column(p.x, p.z);
        if old_h != new_h {
            // Skylight below this column changed: remesh chunk columns around it.
            let top = old_h.max(new_h).max(p.y) >> 4;
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let cx = (p.x + dx) >> 4;
                    let cz = (p.z + dz) >> 4;
                    for cy in 0..=top {
                        self.dirty.insert(IVec3::new(cx, cy, cz));
                    }
                }
            }
        }
    }

    /// Apply damage to a block. Returns true if the block was destroyed.
    pub fn damage_block(&mut self, p: IVec3, amount: f32) -> bool {
        let b = self.get(p);
        if b.is_air() || b.indestructible() || !self.in_bounds(p) {
            return false;
        }
        let hp = b.info().hp;
        let entry = self.damage.entry(p).or_insert(0.0);
        let before_stage = (*entry / hp * 5.0) as i32;
        *entry += amount;
        if *entry >= hp {
            self.set(p, Block::Air);
            return true;
        }
        let after_stage = (*entry / hp * 5.0) as i32;
        if after_stage != before_stage {
            self.mark_dirty_around(p);
        }
        false
    }

    /// Fraction of block HP lost (0 = intact), used for crack rendering.
    pub fn damage_frac(&self, p: IVec3) -> f32 {
        match self.damage.get(&p) {
            Some(d) => {
                let hp = self.get(p).info().hp;
                if hp.is_finite() && hp > 0.0 {
                    (d / hp).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            }
            None => 0.0,
        }
    }

    /// Take the set of chunks that need remeshing.
    pub fn take_dirty(&mut self) -> Vec<IVec3> {
        let v: Vec<IVec3> = self
            .dirty
            .drain()
            .filter(|c| {
                c.x >= 0
                    && c.y >= 0
                    && c.z >= 0
                    && c.x < self.chunks_dim.x
                    && c.y < self.chunks_dim.y
                    && c.z < self.chunks_dim.z
            })
            .collect();
        v
    }

    /// Walk the voxels along a ray (Amanatides & Woo DDA), calling `visit`
    /// for each non-air block. `visit` returns false to stop.
    pub fn traverse(&self, origin: Vec3, dir: Vec3, max_t: f32, mut visit: impl FnMut(RayHit) -> bool) {
        let dir = dir.normalize_or_zero();
        if dir == Vec3::ZERO {
            return;
        }
        let mut p = origin.floor().as_ivec3();
        let step = IVec3::new(
            if dir.x > 0.0 { 1 } else { -1 },
            if dir.y > 0.0 { 1 } else { -1 },
            if dir.z > 0.0 { 1 } else { -1 },
        );
        let inv = |d: f32| if d.abs() < 1e-9 { f32::INFINITY } else { 1.0 / d.abs() };
        let t_delta = Vec3::new(inv(dir.x), inv(dir.y), inv(dir.z));
        let boundary = |o: f32, cell: i32, d: f32| -> f32 {
            if d > 0.0 {
                (cell as f32 + 1.0 - o) / d
            } else if d < 0.0 {
                (o - cell as f32) / -d
            } else {
                f32::INFINITY
            }
        };
        let mut t_max = Vec3::new(
            boundary(origin.x, p.x, dir.x),
            boundary(origin.y, p.y, dir.y),
            boundary(origin.z, p.z, dir.z),
        );
        let mut t = 0.0;
        let mut normal = IVec3::ZERO;
        loop {
            let b = self.get(p);
            if !b.is_air()
                && !visit(RayHit {
                    pos: p,
                    normal,
                    t,
                    block: b,
                })
            {
                return;
            }
            if t_max.x < t_max.y && t_max.x < t_max.z {
                p.x += step.x;
                t = t_max.x;
                t_max.x += t_delta.x;
                normal = IVec3::new(-step.x, 0, 0);
            } else if t_max.y < t_max.z {
                p.y += step.y;
                t = t_max.y;
                t_max.y += t_delta.y;
                normal = IVec3::new(0, -step.y, 0);
            } else {
                p.z += step.z;
                t = t_max.z;
                t_max.z += t_delta.z;
                normal = IVec3::new(0, 0, -step.z);
            }
            if t > max_t {
                return;
            }
            if p.y >= self.size.y && step.y > 0 {
                return;
            }
        }
    }

    /// First block along the ray accepted by `filter`.
    pub fn raycast(&self, origin: Vec3, dir: Vec3, max_t: f32, filter: impl Fn(Block) -> bool) -> Option<RayHit> {
        let mut result = None;
        self.traverse(origin, dir, max_t, |hit| {
            if filter(hit.block) {
                result = Some(hit);
                false
            } else {
                true
            }
        });
        result
    }

    /// Is there an unobstructed line of sight between two points?
    pub fn line_of_sight(&self, a: Vec3, b: Vec3) -> bool {
        let d = b - a;
        let len = d.length();
        if len < 1e-4 {
            return true;
        }
        self.raycast(a, d, len, |blk| blk.blocks_vision()).is_none()
    }

    /// Does an axis-aligned box overlap any solid block?
    pub fn aabb_collides(&self, min: Vec3, max: Vec3) -> bool {
        let lo = min.floor().as_ivec3();
        let hi = (max - Vec3::splat(1e-4)).floor().as_ivec3();
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                for x in lo.x..=hi.x {
                    if self.get_xyz(x, y, z).is_solid() {
                        return true;
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_set_and_bounds() {
        let mut w = World::new(IVec3::new(2, 2, 2));
        assert_eq!(w.get(IVec3::new(5, 5, 5)), Block::Air);
        assert_eq!(w.get(IVec3::new(-1, 5, 5)), Block::Bedrock);
        assert_eq!(w.get(IVec3::new(5, 100, 5)), Block::Air);
        w.set(IVec3::new(17, 3, 4), Block::Brick);
        assert_eq!(w.get(IVec3::new(17, 3, 4)), Block::Brick);
        assert!(!w.take_dirty().is_empty());
    }

    #[test]
    fn raycast_hits_block() {
        let mut w = World::new(IVec3::new(2, 2, 2));
        w.set_gen(IVec3::new(10, 5, 5), Block::Stone);
        w.finish_generation();
        let hit = w
            .raycast(Vec3::new(2.5, 5.5, 5.5), Vec3::X, 50.0, |b| b.is_solid())
            .expect("should hit");
        assert_eq!(hit.pos, IVec3::new(10, 5, 5));
        assert_eq!(hit.normal, IVec3::new(-1, 0, 0));
        assert!((hit.t - 7.5).abs() < 1e-4);
    }

    #[test]
    fn damage_destroys_block() {
        let mut w = World::new(IVec3::new(1, 1, 1));
        let p = IVec3::new(3, 3, 3);
        w.set_gen(p, Block::Planks);
        w.finish_generation();
        assert!(!w.damage_block(p, 30.0));
        assert!(w.damage_frac(p) > 0.4);
        assert!(w.damage_block(p, 40.0));
        assert_eq!(w.get(p), Block::Air);
    }

    #[test]
    fn skylight_heightmap() {
        let mut w = World::new(IVec3::new(1, 1, 1));
        w.set_gen(IVec3::new(4, 10, 4), Block::Concrete);
        w.finish_generation();
        assert!(!w.sky_at(IVec3::new(4, 5, 4)));
        assert!(w.sky_at(IVec3::new(4, 11, 4)));
        w.set(IVec3::new(4, 10, 4), Block::Air);
        assert!(w.sky_at(IVec3::new(4, 5, 4)));
    }
}
