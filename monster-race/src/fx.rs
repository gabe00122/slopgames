//! Particles: dust, drift sparks, spray and the like. A fixed pool of small
//! meshes is recycled, so bursts never allocate.

use crate::{
    course::Mats,
    meshkit::MeshBuilder,
    util::{Rng, lin},
};
use bevy::{
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Puff {
    Dust,
    /// Drift sparks; the level (1..=3) sets the color.
    Spark(u8),
    Water,
    Star,
    Flame,
    Leaf,
    Hit,
}

impl Puff {
    fn index(self) -> usize {
        match self {
            Puff::Dust => 0,
            Puff::Spark(1) => 1,
            Puff::Spark(2) => 2,
            Puff::Spark(_) => 3,
            Puff::Water => 4,
            Puff::Star => 6,
            Puff::Flame => 7,
            Puff::Leaf => 8,
            Puff::Hit => 9,
        }
    }

    /// `(color, glows, gravity, drag, grows)`.
    fn look(index: usize) -> (u32, bool, f32, f32, bool) {
        match index {
            0 => (0xdccba0, false, -1.5, 2.5, true),
            1 => (0x4cc9ff, true, 18.0, 1.0, false),
            2 => (0xff9f1c, true, 18.0, 1.0, false),
            3 => (0xff4fd8, true, 18.0, 1.0, false),
            4 => (0xe6f8ff, false, 26.0, 0.6, false),
            5 => (0x55555c, false, -3.0, 1.5, true),
            6 => (0xfff06a, true, 4.0, 1.5, false),
            7 => (0xff7b1c, true, -4.0, 2.0, false),
            8 => (0x5dbb4f, false, 9.0, 1.5, false),
            _ => (0xffffff, true, 10.0, 2.0, false),
        }
    }
}

const KINDS: usize = 10;
const POOL: usize = 420;

struct Spawn {
    kind: Puff,
    pos: Vec3,
    vel: Vec3,
    life: f32,
    size: f32,
}

/// Queue of particles to emit this frame, plus the shared camera-shake level.
#[derive(Resource)]
pub struct Fx {
    queue: Vec<Spawn>,
    rng: Rng,
    /// Screen shake shared by every view (beast horn, big landings). Decays.
    pub shake: f32,
}

impl Default for Fx {
    fn default() -> Self {
        Fx {
            queue: Vec::new(),
            rng: Rng::new(99),
            shake: 0.0,
        }
    }
}

impl Fx {
    pub fn puff(&mut self, kind: Puff, pos: Vec3, vel: Vec3, life: f32, size: f32) {
        if self.queue.len() < POOL {
            self.queue.push(Spawn {
                kind,
                pos,
                vel,
                life,
                size,
            });
        }
    }

    /// `count` particles thrown outward and upward from `pos`.
    pub fn burst(&mut self, kind: Puff, pos: Vec3, base: Vec3, count: usize, speed: f32, life: f32, size: f32) {
        for _ in 0..count {
            let dir = Vec3::new(self.rng.sym(1.0), self.rng.range(0.1, 1.0), self.rng.sym(1.0)).normalize_or(Vec3::Y);
            let v = base + dir * speed * self.rng.range(0.4, 1.0);
            let l = life * self.rng.range(0.6, 1.0);
            let s = size * self.rng.range(0.6, 1.2);
            self.puff(kind, pos, v, l, s);
        }
    }

    pub fn jitter(&mut self, amount: f32) -> Vec3 {
        Vec3::new(self.rng.sym(amount), self.rng.sym(amount), self.rng.sym(amount))
    }
}

#[derive(Component, Default)]
pub struct Particle {
    vel: Vec3,
    life: f32,
    max: f32,
    size: f32,
    gravity: f32,
    drag: f32,
    grows: bool,
}

#[derive(Resource)]
pub struct FxAssets {
    meshes: Vec<Handle<Mesh>>,
    pool: Vec<Entity>,
    next: usize,
}

pub fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mats: Res<Mats>) {
    let handles: Vec<Handle<Mesh>> = (0..KINDS)
        .map(|i| {
            let mut b = MeshBuilder::new();
            b.ball(Vec3::ZERO, 0.5, 0, lin(Puff::look(i).0));
            meshes.add(b.build(true, false))
        })
        .collect();
    let pool = (0..POOL)
        .map(|_| {
            commands
                .spawn((
                    Mesh3d(handles[0].clone()),
                    MeshMaterial3d(mats.unlit.clone()),
                    Transform::default(),
                    Visibility::Hidden,
                    Particle::default(),
                    NotShadowCaster,
                    NotShadowReceiver,
                ))
                .id()
        })
        .collect();
    commands.insert_resource(FxAssets {
        meshes: handles,
        pool,
        next: 0,
    });
}

pub fn update(
    time: Res<Time>,
    mats: Res<Mats>,
    mut fx: ResMut<Fx>,
    mut assets: ResMut<FxAssets>,
    mut q: Query<(
        &mut Particle,
        &mut Transform,
        &mut Visibility,
        &mut Mesh3d,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let dt = time.delta_secs();
    fx.shake = (fx.shake - dt * 1.4).max(0.0);
    for s in std::mem::take(&mut fx.queue) {
        let e = assets.pool[assets.next];
        assets.next = (assets.next + 1) % POOL;
        let Ok((mut p, mut tf, mut vis, mut mesh, mut mat)) = q.get_mut(e) else {
            continue;
        };
        let i = s.kind.index();
        let (_, glows, gravity, drag, grows) = Puff::look(i);
        *p = Particle {
            vel: s.vel,
            life: s.life,
            max: s.life,
            size: s.size,
            gravity,
            drag,
            grows,
        };
        tf.translation = s.pos;
        tf.scale = Vec3::splat(s.size);
        mesh.0 = assets.meshes[i].clone();
        mat.0 = if glows { mats.glow.clone() } else { mats.unlit.clone() };
        *vis = Visibility::Visible;
    }
    for (mut p, mut tf, mut vis, _, _) in &mut q {
        if p.life <= 0.0 {
            continue;
        }
        p.life -= dt;
        if p.life <= 0.0 {
            *vis = Visibility::Hidden;
            continue;
        }
        let (g, drag) = (p.gravity, p.drag);
        p.vel.y -= g * dt;
        p.vel *= 1.0 - (drag * dt).min(0.9);
        tf.translation += p.vel * dt;
        let f = p.life / p.max;
        let scale = if p.grows {
            p.size * (1.0 + (1.0 - f) * 1.6) * f.sqrt()
        } else {
            p.size * f
        };
        tf.scale = Vec3::splat(scale.max(0.001));
        tf.rotate_y(dt * 5.0);
    }
}
