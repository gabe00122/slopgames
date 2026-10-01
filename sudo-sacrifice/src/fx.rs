//! Particles, light flashes and screen shake. Particles come from a fixed
//! pool of small meshes that is recycled, so bursts never allocate.

use crate::{
    game::Team,
    meshkit::MeshBuilder,
    models::Mats,
    util::{Rng, lin},
};
use bevy::{
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Puff {
    Dust,
    Spark,
    Water,
    Fire,
    Smoke,
    /// Random bright bytes.
    Glitch,
    Heal,
    Mana,
    Team(Team),
    Gold,
    Debris,
}

/// `(color, glows, gravity, drag, grows, cube)`.
const LOOKS: [(u32, bool, f32, f32, bool, bool); 15] = [
    (0xc9b48a, false, -1.0, 2.5, true, false),
    (0xffd27a, true, 14.0, 1.0, false, false),
    (0xd8f6ff, false, 20.0, 0.6, false, false),
    (0xff7a2a, true, -6.0, 2.0, false, false),
    (0x4a4250, false, -2.0, 1.5, true, false),
    (0xff4fd8, true, 6.0, 1.2, false, true),
    (0x4fe3ff, true, 6.0, 1.2, false, true),
    (0xffd166, true, 6.0, 1.2, false, true),
    (0x7ee787, true, -3.0, 1.5, false, false),
    (0x9ff3ff, true, -1.5, 0.4, false, true),
    (0x3cc8ff, true, 2.0, 1.2, false, true),
    (0xff4d6d, true, 2.0, 1.2, false, true),
    (0xffe9a8, true, -2.0, 1.2, false, false),
    (0x6a5a64, false, 22.0, 0.3, false, true),
    (0xffc94a, true, -7.0, 2.0, false, false),
];

const POOL: usize = 1100;
const LIGHTS: usize = 10;

struct Spawn {
    look: usize,
    pos: Vec3,
    vel: Vec3,
    life: f32,
    size: f32,
}

struct Flash {
    pos: Vec3,
    color: Color,
    intensity: f32,
    range: f32,
    life: f32,
}

/// Queue of particles to emit this frame, plus the shared camera shake.
#[derive(Resource)]
pub struct Fx {
    queue: Vec<Spawn>,
    flashes: Vec<Flash>,
    rng: Rng,
    /// Screen shake. Decays.
    pub shake: f32,
    /// Where the camera is, for distance falloff.
    pub listener: Vec3,
}

impl Default for Fx {
    fn default() -> Self {
        Fx {
            queue: Vec::new(),
            flashes: Vec::new(),
            rng: Rng::new(99),
            shake: 0.0,
            listener: Vec3::ZERO,
        }
    }
}

impl Fx {
    fn look_of(&mut self, kind: Puff) -> usize {
        match kind {
            Puff::Dust => 0,
            Puff::Spark => 1,
            Puff::Water => 2,
            Puff::Fire => {
                if self.rng.chance(0.4) {
                    14
                } else {
                    3
                }
            }
            Puff::Smoke => 4,
            Puff::Glitch => 5 + self.rng.below(3),
            Puff::Heal => 8,
            Puff::Mana => 9,
            Puff::Team(Team::Blue) => 10,
            Puff::Team(Team::Red) => 11,
            Puff::Gold => 12,
            Puff::Debris => 13,
        }
    }

    pub fn puff(&mut self, kind: Puff, pos: Vec3, vel: Vec3, life: f32, size: f32) {
        if self.queue.len() < POOL / 2 {
            let look = self.look_of(kind);
            self.queue.push(Spawn {
                look,
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

    /// Particles scattered over a disc on the ground.
    pub fn scatter(&mut self, kind: Puff, center: Vec3, radius: f32, count: usize, up: f32, life: f32, size: f32) {
        for _ in 0..count {
            let d = self.rng.disc(radius);
            let v = Vec3::new(self.rng.sym(1.0), up * self.rng.range(0.5, 1.0), self.rng.sym(1.0));
            let l = life * self.rng.range(0.6, 1.0);
            let s = size * self.rng.range(0.7, 1.2);
            self.puff(kind, center + Vec3::new(d.x, 0.2, d.y), v, l, s);
        }
    }

    pub fn jitter(&mut self, amount: f32) -> Vec3 {
        Vec3::new(self.rng.sym(amount), self.rng.sym(amount), self.rng.sym(amount))
    }

    pub fn rand(&mut self) -> f32 {
        self.rng.f()
    }

    /// Shakes the camera, less the further away `pos` is.
    pub fn shake(&mut self, pos: Vec3, amount: f32) {
        let d = pos.distance(self.listener);
        let k = (1.0 - d / 90.0).clamp(0.0, 1.0);
        self.shake = (self.shake + amount * k * k).min(1.6);
    }

    /// A brief point light.
    pub fn flash(&mut self, pos: Vec3, color: u32, intensity: f32, range: f32, life: f32) {
        if self.flashes.len() < LIGHTS {
            self.flashes.push(Flash {
                pos,
                color: crate::util::hex(color),
                intensity,
                range,
                life,
            });
        }
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
    spin: f32,
}

#[derive(Component, Default)]
pub struct FlashLight {
    life: f32,
    max: f32,
    peak: f32,
}

#[derive(Resource)]
pub struct FxAssets {
    meshes: Vec<Handle<Mesh>>,
    pool: Vec<Entity>,
    next: usize,
    lights: Vec<Entity>,
    next_light: usize,
}

pub fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mats: Res<Mats>) {
    let handles: Vec<Handle<Mesh>> = LOOKS
        .iter()
        .map(|look| {
            let mut b = MeshBuilder::new();
            if look.5 {
                b.boxy(Vec3::ZERO, Vec3::splat(0.4), lin(look.0));
            } else {
                b.ball(Vec3::ZERO, 0.5, 0, lin(look.0));
            }
            meshes.add(b.build(true))
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
    let lights = (0..LIGHTS)
        .map(|_| {
            commands
                .spawn((
                    PointLight {
                        intensity: 0.0,
                        range: 10.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::default(),
                    FlashLight::default(),
                ))
                .id()
        })
        .collect();
    commands.insert_resource(FxAssets {
        meshes: handles,
        pool,
        next: 0,
        lights,
        next_light: 0,
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
    mut lights: Query<(&mut PointLight, &mut Transform, &mut FlashLight), Without<Particle>>,
) {
    let dt = time.delta_secs();
    fx.shake = (fx.shake - dt * 1.6).max(0.0);
    for s in std::mem::take(&mut fx.queue) {
        let e = assets.pool[assets.next];
        assets.next = (assets.next + 1) % POOL;
        let Ok((mut p, mut tf, mut vis, mut mesh, mut mat)) = q.get_mut(e) else {
            continue;
        };
        let (_, glows, gravity, drag, grows, _) = LOOKS[s.look];
        let spin = fx.rng.sym(6.0);
        *p = Particle {
            vel: s.vel,
            life: s.life,
            max: s.life,
            size: s.size,
            gravity,
            drag,
            grows,
            spin,
        };
        tf.translation = s.pos;
        tf.scale = Vec3::splat(s.size);
        mesh.0 = assets.meshes[s.look].clone();
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
            p.size * f.sqrt()
        };
        tf.scale = Vec3::splat(scale.max(0.001));
        let spin = p.spin;
        tf.rotate_y(dt * spin);
        tf.rotate_x(dt * spin * 0.7);
    }
    for f in std::mem::take(&mut fx.flashes) {
        let e = assets.lights[assets.next_light];
        assets.next_light = (assets.next_light + 1) % LIGHTS;
        if let Ok((mut light, mut tf, mut fl)) = lights.get_mut(e) {
            light.color = f.color;
            light.range = f.range;
            light.intensity = f.intensity;
            tf.translation = f.pos;
            *fl = FlashLight {
                life: f.life,
                max: f.life,
                peak: f.intensity,
            };
        }
    }
    for (mut light, _, mut fl) in &mut lights {
        if fl.life <= 0.0 {
            if light.intensity != 0.0 {
                light.intensity = 0.0;
            }
            continue;
        }
        fl.life -= dt;
        let k = (fl.life / fl.max).clamp(0.0, 1.0);
        light.intensity = fl.peak * k * k;
    }
}
