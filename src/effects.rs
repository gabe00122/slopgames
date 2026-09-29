//! Short-lived visual effects: debris, blood, tracers, muzzle flashes.

use glam::{Mat4, Vec3};

use crate::render::MeshBuilder;
use crate::rng::Rng;
use crate::world::World;

struct Particle {
    pos: Vec3,
    vel: Vec3,
    life: f32,
    max_life: f32,
    size: f32,
    color: [u8; 4],
    emissive: bool,
    gravity: f32,
}

struct Tracer {
    a: Vec3,
    b: Vec3,
    life: f32,
    color: [u8; 4],
}

#[derive(Default)]
pub struct Effects {
    particles: Vec<Particle>,
    tracers: Vec<Tracer>,
}

impl Effects {
    pub fn clear(&mut self) {
        self.particles.clear();
        self.tracers.clear();
    }

    pub fn debris(&mut self, pos: Vec3, normal: Vec3, color: [u8; 4], count: usize, rng: &mut Rng) {
        for _ in 0..count {
            let v = normal * rng.range_f32(1.0, 3.5)
                + Vec3::new(rng.range_f32(-1.5, 1.5), rng.range_f32(0.5, 3.0), rng.range_f32(-1.5, 1.5));
            let shade = rng.range_f32(0.75, 1.1);
            let c = [
                (color[0] as f32 * shade).min(255.0) as u8,
                (color[1] as f32 * shade).min(255.0) as u8,
                (color[2] as f32 * shade).min(255.0) as u8,
                255,
            ];
            let life = rng.range_f32(0.5, 1.1);
            self.particles.push(Particle {
                pos,
                vel: v,
                life,
                max_life: life,
                size: rng.range_f32(0.05, 0.12),
                color: c,
                emissive: false,
                gravity: 14.0,
            });
        }
    }

    /// Chunks flying off a destroyed block.
    pub fn block_break(&mut self, center: Vec3, color: [u8; 4], rng: &mut Rng) {
        for _ in 0..14 {
            let off = Vec3::new(rng.range_f32(-0.4, 0.4), rng.range_f32(-0.4, 0.4), rng.range_f32(-0.4, 0.4));
            let life = rng.range_f32(0.6, 1.3);
            self.particles.push(Particle {
                pos: center + off,
                vel: off * 5.0 + Vec3::Y * rng.range_f32(1.0, 3.0),
                life,
                max_life: life,
                size: rng.range_f32(0.1, 0.22),
                color,
                emissive: false,
                gravity: 16.0,
            });
        }
    }

    pub fn blood(&mut self, pos: Vec3, dir: Vec3, rng: &mut Rng) {
        for _ in 0..8 {
            let v = dir * rng.range_f32(0.5, 2.5)
                + Vec3::new(rng.range_f32(-1.0, 1.0), rng.range_f32(-0.5, 1.5), rng.range_f32(-1.0, 1.0));
            let life = rng.range_f32(0.3, 0.6);
            self.particles.push(Particle {
                pos,
                vel: v,
                life,
                max_life: life,
                size: rng.range_f32(0.04, 0.08),
                color: [140, 16, 16, 255],
                emissive: false,
                gravity: 12.0,
            });
        }
    }

    pub fn sparks(&mut self, pos: Vec3, normal: Vec3, rng: &mut Rng) {
        for _ in 0..4 {
            let v = normal * rng.range_f32(2.0, 5.0)
                + Vec3::new(rng.range_f32(-2.0, 2.0), rng.range_f32(-1.0, 2.0), rng.range_f32(-2.0, 2.0));
            self.particles.push(Particle {
                pos,
                vel: v,
                life: 0.12,
                max_life: 0.12,
                size: 0.03,
                color: [255, 220, 140, 255],
                emissive: true,
                gravity: 6.0,
            });
        }
    }

    pub fn muzzle_flash(&mut self, pos: Vec3) {
        self.particles.push(Particle {
            pos,
            vel: Vec3::ZERO,
            life: 0.05,
            max_life: 0.05,
            size: 0.22,
            color: [255, 214, 120, 255],
            emissive: true,
            gravity: 0.0,
        });
    }

    pub fn tracer(&mut self, a: Vec3, b: Vec3, color: [u8; 3]) {
        self.tracers.push(Tracer {
            a,
            b,
            life: 0.06,
            color: [color[0], color[1], color[2], 255],
        });
    }

    pub fn update(&mut self, dt: f32, world: &World) {
        for p in &mut self.particles {
            p.life -= dt;
            p.vel.y -= p.gravity * dt;
            let next = p.pos + p.vel * dt;
            if p.gravity > 0.0 && world.get(next.floor().as_ivec3()).is_solid() {
                p.vel *= -0.2;
            } else {
                p.pos = next;
            }
        }
        self.particles.retain(|p| p.life > 0.0);
        for t in &mut self.tracers {
            t.life -= dt;
        }
        self.tracers.retain(|t| t.life > 0.0);
    }

    pub fn build(&self, mb: &mut MeshBuilder) {
        for p in &self.particles {
            let s = p.size * (0.4 + 0.6 * (p.life / p.max_life));
            mb.add_cube(
                Mat4::from_translation(p.pos) * Mat4::from_scale(Vec3::splat(s)),
                p.color,
                p.emissive,
            );
        }
        for t in &self.tracers {
            // Draw only the leading part of the path so it reads as a moving round.
            let len = t.a.distance(t.b);
            let dir = (t.b - t.a) / len.max(1e-4);
            let progress = (1.0 - t.life / 0.06).clamp(0.0, 1.0);
            let head_d = (len * progress).max(len.min(2.0));
            let tail_d = (head_d - 3.0).max(0.0);
            mb.add_beam(t.a + dir * tail_d, t.a + dir * head_d, 0.018, t.color, true);
        }
    }
}
