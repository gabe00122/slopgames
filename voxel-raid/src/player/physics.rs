//! AABB vs voxel collision with auto step-up, shared by the player and AI.

use crate::world::World;
use glam::Vec3;

const EPS: f32 = 1e-3;
pub const STEP_HEIGHT: f32 = 1.05;

#[derive(Clone, Copy, Debug, Default)]
pub struct MoveResult {
    pub on_ground: bool,
    pub hit_ceiling: bool,
    pub hit_wall: bool,
    /// Height gained by auto-stepping this frame (for camera smoothing).
    pub stepped: f32,
}

/// Axis-aligned body: `pos` is the centre of the feet.
#[inline]
pub fn body_aabb(pos: Vec3, half_w: f32, height: f32) -> (Vec3, Vec3) {
    (
        Vec3::new(pos.x - half_w, pos.y, pos.z - half_w),
        Vec3::new(pos.x + half_w, pos.y + height, pos.z + half_w),
    )
}

pub fn collides(world: &World, pos: Vec3, half_w: f32, height: f32) -> bool {
    let (min, max) = body_aabb(pos, half_w, height);
    world.aabb_collides(min, max)
}

/// Push the body out of blocks along one axis after moving by `d` on that axis.
/// Returns true if a collision was resolved.
fn resolve_axis(world: &World, pos: &mut Vec3, axis: usize, d: f32, half_w: f32, height: f32) -> bool {
    if d == 0.0 {
        return false;
    }
    let (min, max) = body_aabb(*pos, half_w, height);
    let lo = min.floor().as_ivec3();
    let hi = (max - Vec3::splat(1e-5)).floor().as_ivec3();
    let mut hit = false;
    let mut limit = if d > 0.0 { f32::MAX } else { f32::MIN };
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                if world.get_xyz(x, y, z).is_solid() {
                    hit = true;
                    let c = [x, y, z][axis] as f32;
                    if d > 0.0 {
                        limit = limit.min(c);
                    } else {
                        limit = limit.max(c + 1.0);
                    }
                }
            }
        }
    }
    if !hit {
        return false;
    }
    match axis {
        0 => {
            pos.x = if d > 0.0 {
                limit - half_w - EPS
            } else {
                limit + half_w + EPS
            }
        }
        1 => pos.y = if d > 0.0 { limit - height - EPS } else { limit + 1e-4 },
        _ => {
            pos.z = if d > 0.0 {
                limit - half_w - EPS
            } else {
                limit + half_w + EPS
            }
        }
    }
    true
}

/// Integrate velocity with collision. `can_step` enables climbing one-block ledges
/// when walking into them while grounded.
pub fn move_body(
    world: &World,
    pos: &mut Vec3,
    vel: &mut Vec3,
    half_w: f32,
    height: f32,
    dt: f32,
    can_step: bool,
) -> MoveResult {
    let mut res = MoveResult::default();
    let disp = *vel * dt;
    let max_comp = disp.x.abs().max(disp.y.abs()).max(disp.z.abs());
    let steps = ((max_comp / 0.3).ceil() as i32).clamp(1, 32);
    let sub = disp / steps as f32;

    for _ in 0..steps {
        // Vertical first so ground contact is known for stepping.
        pos.y += sub.y;
        if resolve_axis(world, pos, 1, sub.y, half_w, height) {
            if sub.y < 0.0 {
                res.on_ground = true;
            } else {
                res.hit_ceiling = true;
            }
            vel.y = 0.0;
        }

        for axis in [0usize, 2usize] {
            let d = sub[axis];
            if d == 0.0 {
                continue;
            }
            let before = *pos;
            pos[axis] += d;
            if !collides(world, *pos, half_w, height) {
                continue;
            }
            // Try stepping up onto a ledge.
            if can_step {
                let mut up = before;
                up.y += STEP_HEIGHT;
                if !collides(world, up, half_w, height) {
                    up[axis] += d;
                    if !collides(world, up, half_w, height) {
                        let mut settle = up;
                        settle.y -= STEP_HEIGHT;
                        if collides(world, settle, half_w, height) {
                            resolve_axis(world, &mut settle, 1, -1.0, half_w, height);
                        }
                        if settle.y > before.y + 0.01 && !collides(world, settle, half_w, height) {
                            res.stepped += settle.y - before.y;
                            *pos = settle;
                            continue;
                        }
                    }
                }
            }
            resolve_axis(world, pos, axis, d, half_w, height);
            vel[axis] = 0.0;
            res.hit_wall = true;
        }
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Block;
    use glam::IVec3;

    fn flat_world() -> World {
        let mut w = World::new(IVec3::new(2, 1, 2));
        w.fill_gen(IVec3::new(0, 0, 0), IVec3::new(31, 4, 31), Block::Stone);
        w.finish_generation();
        w
    }

    #[test]
    fn falls_and_lands() {
        let w = flat_world();
        let mut pos = Vec3::new(10.5, 9.0, 10.5);
        let mut vel = Vec3::ZERO;
        let mut grounded = false;
        for _ in 0..200 {
            vel.y -= 24.0 / 60.0;
            let r = move_body(&w, &mut pos, &mut vel, 0.3, 1.8, 1.0 / 60.0, false);
            grounded |= r.on_ground;
        }
        assert!(grounded);
        assert!((pos.y - 5.0).abs() < 0.01, "pos.y = {}", pos.y);
    }

    #[test]
    fn blocked_by_wall_and_steps_up() {
        let mut w = flat_world();
        // Two-high wall at x = 14.
        w.fill_gen(IVec3::new(14, 5, 0), IVec3::new(14, 6, 31), Block::Brick);
        // One-high ledge at z = 20.
        w.fill_gen(IVec3::new(0, 5, 20), IVec3::new(12, 5, 31), Block::Brick);
        w.finish_generation();

        let mut pos = Vec3::new(12.0, 5.0, 10.5);
        let mut vel = Vec3::new(5.0, 0.0, 0.0);
        for _ in 0..60 {
            vel.x = 5.0;
            move_body(&w, &mut pos, &mut vel, 0.3, 1.8, 1.0 / 60.0, true);
        }
        assert!(pos.x < 14.0 - 0.29, "passed through wall: {}", pos.x);

        let mut pos = Vec3::new(5.5, 5.0, 18.0);
        let mut vel = Vec3::ZERO;
        let mut stepped = 0.0;
        for _ in 0..60 {
            vel.z = 4.0;
            vel.y -= 24.0 / 60.0;
            let r = move_body(&w, &mut pos, &mut vel, 0.3, 1.8, 1.0 / 60.0, true);
            stepped += r.stepped;
        }
        assert!(pos.z > 20.5);
        assert!((pos.y - 6.0).abs() < 0.05, "y = {}", pos.y);
        assert!(stepped > 0.9);
    }
}
