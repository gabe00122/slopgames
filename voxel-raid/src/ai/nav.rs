//! A* pathfinding over walkable voxel cells (feet positions).

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use glam::{IVec3, Vec3};

use crate::world::World;

/// A cell is walkable if there is a solid block below and two free cells for the body.
pub fn walkable(world: &World, p: IVec3) -> bool {
    world.get(p - IVec3::Y).is_solid() && !world.get(p).is_solid() && !world.get(p + IVec3::Y).is_solid()
}

/// Find a walkable cell at or near a position (searching a few cells down/up).
pub fn snap_to_walkable(world: &World, pos: Vec3) -> Option<IVec3> {
    let base = pos.floor().as_ivec3();
    for dy in [0, -1, 1, -2, 2, -3, -4] {
        let p = base + IVec3::new(0, dy, 0);
        if walkable(world, p) {
            return Some(p);
        }
    }
    None
}

#[derive(Copy, Clone, PartialEq)]
struct Node {
    f: f32,
    p: IVec3,
}

impl Eq for Node {}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f.partial_cmp(&self.f).unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

const DIRS: [(i32, i32); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];

/// Neighbour in horizontal direction (dx, dz) from p, allowing a step up of 1
/// and drops of up to 3 blocks.
fn step_target(world: &World, p: IVec3, dx: i32, dz: i32) -> Option<IVec3> {
    let n = IVec3::new(p.x + dx, p.y, p.z + dz);
    if walkable(world, n) {
        return Some(n);
    }
    // Step up (needs headroom above the current cell).
    let up = n + IVec3::Y;
    if walkable(world, up) && !world.get(p + IVec3::new(0, 2, 0)).is_solid() {
        return Some(up);
    }
    // Drop down.
    if !world.get(n).is_solid() && !world.get(n + IVec3::Y).is_solid() {
        for d in 1..=3 {
            let down = n - IVec3::new(0, d, 0);
            if walkable(world, down) {
                return Some(down);
            }
            if world.get(down).is_solid() {
                break;
            }
        }
    }
    None
}

/// Returns a list of feet positions (block centres) from start to goal.
pub fn find_path(world: &World, start: Vec3, goal: Vec3, max_nodes: usize) -> Option<Vec<Vec3>> {
    let s = snap_to_walkable(world, start)?;
    let g = snap_to_walkable(world, goal)?;
    if s == g {
        return Some(vec![goal]);
    }
    let h = |p: IVec3| (p - g).as_vec3().length();
    let mut open = BinaryHeap::new();
    let mut came: HashMap<IVec3, IVec3> = HashMap::new();
    let mut cost: HashMap<IVec3, f32> = HashMap::new();
    cost.insert(s, 0.0);
    open.push(Node { f: h(s), p: s });
    let mut expanded = 0;
    let mut best = (h(s), s);
    while let Some(Node { p, .. }) = open.pop() {
        if p == g {
            best = (0.0, g);
            break;
        }
        expanded += 1;
        if expanded > max_nodes {
            break;
        }
        let pc = cost[&p];
        for (dx, dz) in DIRS {
            let diag = dx != 0 && dz != 0;
            if diag {
                // No corner cutting: both orthogonal neighbours must be passable at this level.
                let a = IVec3::new(p.x + dx, p.y, p.z);
                let b = IVec3::new(p.x, p.y, p.z + dz);
                let clear = |c: IVec3| !world.get(c).is_solid() && !world.get(c + IVec3::Y).is_solid();
                if !clear(a) || !clear(b) {
                    continue;
                }
            }
            let Some(n) = step_target(world, p, dx, dz) else {
                continue;
            };
            if diag && n.y != p.y {
                continue;
            }
            let step = if diag { 1.414 } else { 1.0 } + (n.y - p.y).abs() as f32 * 0.6;
            let nc = pc + step;
            if cost.get(&n).is_none_or(|c| nc < *c) {
                cost.insert(n, nc);
                came.insert(n, p);
                let hn = h(n);
                if hn < best.0 {
                    best = (hn, n);
                }
                open.push(Node { f: nc + hn, p: n });
            }
        }
    }
    // Reconstruct towards the goal, or the closest point reached.
    let end = best.1;
    if end == s {
        return None;
    }
    let mut path = vec![end];
    let mut cur = end;
    while let Some(prev) = came.get(&cur) {
        if *prev == s {
            break;
        }
        path.push(*prev);
        cur = *prev;
    }
    path.reverse();
    Some(
        path.into_iter()
            .map(|c| Vec3::new(c.x as f32 + 0.5, c.y as f32, c.z as f32 + 0.5))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Block;

    #[test]
    fn path_goes_around_wall_and_up_step() {
        let mut w = World::new(IVec3::new(2, 1, 2));
        w.fill_gen(IVec3::new(0, 0, 0), IVec3::new(31, 2, 31), Block::Stone);
        // Wall with a gap at z = 20.
        w.fill_gen(IVec3::new(10, 3, 0), IVec3::new(10, 5, 19), Block::Brick);
        w.fill_gen(IVec3::new(10, 3, 21), IVec3::new(10, 5, 31), Block::Brick);
        // A one-block step on the far side.
        w.fill_gen(IVec3::new(14, 3, 0), IVec3::new(31, 3, 31), Block::Planks);
        w.finish_generation();
        let path = find_path(&w, Vec3::new(3.5, 3.0, 3.5), Vec3::new(20.5, 4.0, 3.5), 5000).expect("path");
        let last = *path.last().unwrap();
        assert!((last - Vec3::new(20.5, 4.0, 3.5)).length() < 0.01);
        assert!(path
            .iter()
            .any(|p| (p.z - 20.5).abs() < 0.01 && (p.x - 10.5).abs() < 0.01));
    }
}
