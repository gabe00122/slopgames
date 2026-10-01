//! Particles (splashes, bubbles, sparks, embers, steam, blood), fireflies,
//! and screen shake. Particles come from a fixed pool of entities.

use crate::{
    audio::{Sfx, Sound},
    game::{LevelEntity, Splash},
    models::{Mats, Models},
    util::Rng,
    water::Water,
};
use bevy::{
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};

const POOL: usize = 600;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Look {
    Drop,
    Bubble,
    Blood,
    Spark,
    Ember,
    Steam,
    Mud,
}

impl Look {
    const ALL: [Look; 7] = [
        Look::Drop,
        Look::Bubble,
        Look::Blood,
        Look::Spark,
        Look::Ember,
        Look::Steam,
        Look::Mud,
    ];

    /// (gravity, drag, growth per second)
    fn motion(self) -> (f32, f32, f32) {
        match self {
            Look::Drop => (20.0, 0.3, 0.0),
            Look::Bubble => (-7.0, 3.0, 0.2),
            Look::Blood => (16.0, 0.8, 0.0),
            Look::Spark => (12.0, 1.0, -0.5),
            Look::Ember => (-0.6, 0.6, -0.3),
            Look::Steam => (-1.5, 1.5, 1.5),
            Look::Mud => (18.0, 0.5, 0.0),
        }
    }
}

struct Spawn {
    look: Look,
    pos: Vec3,
    vel: Vec3,
    size: f32,
    life: f32,
}

#[derive(Resource)]
pub struct Fx {
    queue: Vec<Spawn>,
    pub rng: Rng,
    shake: f32,
    /// Red flash when the bogwight is hurt (0..1).
    pub hurt: f32,
}

impl Default for Fx {
    fn default() -> Self {
        Fx {
            queue: Vec::new(),
            rng: Rng::new(0xf1),
            shake: 0.0,
            hurt: 0.0,
        }
    }
}

impl Fx {
    pub fn emit(&mut self, look: Look, pos: Vec3, vel: Vec3, size: f32, life: f32) {
        if self.queue.len() < 200 {
            self.queue.push(Spawn {
                look,
                pos,
                vel,
                size,
                life,
            });
        }
    }

    pub fn shake(&mut self, amount: f32) {
        self.shake = (self.shake + amount).min(1.2);
    }

    pub fn shake_amount(&self) -> f32 {
        self.shake
    }

    /// A burst in all directions.
    pub fn burst(&mut self, look: Look, pos: Vec3, n: usize, speed: f32, size: f32, life: f32) {
        for _ in 0..n {
            let d = self.rng.disc(1.0);
            let v = Vec3::new(d.x, d.y.abs() * 0.5 + d.y * 0.5 + 0.3, self.rng.sym(0.5)) * speed;
            let s = size * self.rng.range(0.6, 1.3);
            let l = life * self.rng.range(0.6, 1.2);
            self.emit(look, pos, v, s, l);
        }
    }
}

#[derive(Component, Default)]
pub struct Particle {
    vel: Vec3,
    life: f32,
    max: f32,
    size: f32,
    look: Option<Look>,
}

#[derive(Resource)]
pub struct FxAssets {
    pool: Vec<Entity>,
    next: usize,
    mats: Vec<Handle<StandardMaterial>>,
}

pub fn setup(
    mut commands: Commands,
    models: Res<Models>,
    mats: Res<Mats>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let looks: Vec<Handle<StandardMaterial>> = Look::ALL
        .iter()
        .map(|l| match l {
            Look::Drop => mats.drop.clone(),
            Look::Bubble => mats.bubble.clone(),
            Look::Blood => materials.add(StandardMaterial {
                base_color: Color::srgb(0.35, 0.03, 0.03),
                perceptual_roughness: 0.3,
                ..default()
            }),
            Look::Spark => materials.add(StandardMaterial {
                base_color: Color::linear_rgb(8.0, 4.0, 1.2),
                unlit: true,
                ..default()
            }),
            Look::Ember => materials.add(StandardMaterial {
                base_color: Color::linear_rgb(6.0, 1.8, 0.3),
                unlit: true,
                ..default()
            }),
            Look::Steam => materials.add(StandardMaterial {
                base_color: Color::srgba(0.8, 0.85, 0.85, 0.25),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
            Look::Mud => materials.add(StandardMaterial {
                base_color: Color::srgb(0.2, 0.15, 0.1),
                perceptual_roughness: 0.6,
                ..default()
            }),
        })
        .collect();
    let pool = (0..POOL)
        .map(|_| {
            commands
                .spawn((
                    Mesh3d(models.ball.clone()),
                    MeshMaterial3d(looks[0].clone()),
                    Transform::from_scale(Vec3::ZERO),
                    Visibility::Hidden,
                    Particle::default(),
                    NotShadowCaster,
                    NotShadowReceiver,
                ))
                .id()
        })
        .collect();
    commands.insert_resource(FxAssets {
        pool,
        next: 0,
        mats: looks,
    });
}

pub fn update(
    time: Res<Time>,
    water: Option<Res<Water>>,
    mut fx: ResMut<Fx>,
    mut assets: ResMut<FxAssets>,
    mut q: Query<(
        &mut Particle,
        &mut Transform,
        &mut Visibility,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let dt = time.delta_secs();
    fx.shake = (fx.shake - dt * 1.8).max(0.0);
    fx.hurt = (fx.hurt - dt * 2.5).max(0.0);
    for s in std::mem::take(&mut fx.queue) {
        let e = assets.pool[assets.next];
        assets.next = (assets.next + 1) % POOL;
        let Ok((mut p, mut tf, mut vis, mut mat)) = q.get_mut(e) else {
            continue;
        };
        *p = Particle {
            vel: s.vel,
            life: s.life,
            max: s.life,
            size: s.size,
            look: Some(s.look),
        };
        tf.translation = s.pos;
        tf.scale = Vec3::splat(s.size);
        let idx = Look::ALL.iter().position(|l| *l == s.look).unwrap_or(0);
        mat.0 = assets.mats[idx].clone();
        *vis = Visibility::Visible;
    }
    for (mut p, mut tf, mut vis, _) in &mut q {
        let Some(look) = p.look else { continue };
        p.life -= dt;
        if p.life <= 0.0 {
            p.look = None;
            *vis = Visibility::Hidden;
            continue;
        }
        let (gravity, drag, grow) = look.motion();
        p.vel.y -= gravity * dt;
        p.vel *= (-drag * dt).exp();
        if look == Look::Bubble {
            p.vel.x += (p.life * 17.0).sin() * dt * 2.0;
        }
        tf.translation += p.vel * dt;
        p.size = (p.size + grow * dt).max(0.0);
        let fade = (p.life / p.max).min(1.0);
        let s = match look {
            Look::Spark | Look::Ember => p.size * fade,
            Look::Steam => p.size * (0.5 + 0.5 * fade),
            Look::Drop => p.size * Vec2::new(p.vel.x, p.vel.y).length().clamp(1.0, 3.0).sqrt(),
            _ => p.size,
        };
        tf.scale = Vec3::splat(s);
        if let Some(w) = water.as_deref() {
            let surface = w.surface(tf.translation.x);
            match look {
                Look::Bubble => {
                    if surface.is_none_or(|s| tf.translation.y > s) {
                        p.life = 0.0;
                    }
                }
                Look::Drop | Look::Blood | Look::Mud
                    if p.vel.y < 0.0 && surface.is_some_and(|s| tf.translation.y < s) =>
                {
                    p.life = p.life.min(0.05);
                }
                _ => {}
            }
        }
    }
}

/// Splashes: drops thrown up, a ring of spray, a sound.
pub fn splashes(mut reader: MessageReader<Splash>, mut fx: ResMut<Fx>, mut sfx: MessageWriter<Sfx>) {
    for s in reader.read() {
        let n = (s.strength * 3.0).clamp(4.0, 40.0) as usize;
        for _ in 0..n {
            let a = fx.rng.sym(0.9);
            let speed = fx.rng.range(0.4, 1.0) * (2.0 + s.strength * 0.5).min(9.0);
            let v = Vec3::new(a.sin() * speed * 0.7, a.cos().abs() * speed, fx.rng.sym(1.5));
            let p = s.pos.extend(fx.rng.range(0.0, 1.2));
            let size = fx.rng.range(0.04, 0.09);
            let life = fx.rng.range(0.6, 1.2);
            fx.emit(Look::Drop, p, v, size, life);
        }
        for _ in 0..(n / 3) {
            let p = Vec3::new(
                s.pos.x + fx.rng.sym(0.4),
                s.pos.y - fx.rng.range(0.2, 0.8),
                fx.rng.sym(0.3),
            );
            let v = Vec3::new(fx.rng.sym(0.6), fx.rng.range(0.3, 1.0), 0.0);
            let size = fx.rng.range(0.04, 0.1);
            fx.emit(Look::Bubble, p, v, size, 1.5);
        }
        let sound = if s.strength > 7.0 {
            Sound::BigSplash
        } else {
            Sound::Splash
        };
        sfx.write(Sfx::at(sound, s.pos));
    }
}

#[derive(Component)]
pub struct Firefly {
    center: Vec3,
    phase: f32,
    speed: f32,
}

pub fn spawn_fireflies(commands: &mut Commands, models: &Models, mats: &Mats, center: Vec2, rng: &mut Rng) {
    for _ in 0..rng.below(5) + 6 {
        let c = Vec3::new(
            center.x + rng.sym(4.0),
            center.y + rng.range(-0.5, 1.5),
            rng.range(-2.5, 1.5),
        );
        commands.spawn((
            Mesh3d(models.ball.clone()),
            MeshMaterial3d(mats.glow.clone()),
            Transform::from_translation(c).with_scale(Vec3::splat(0.05)),
            Firefly {
                center: c,
                phase: rng.angle(),
                speed: rng.range(0.3, 0.8),
            },
            NotShadowCaster,
            NotShadowReceiver,
            LevelEntity,
        ));
    }
    commands.spawn((
        PointLight {
            color: Color::srgb(0.8, 1.0, 0.4),
            intensity: 4_000.0,
            range: 4.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(center.x, center.y + 0.5, 0.5),
        LevelEntity,
    ));
}

pub fn fireflies(time: Res<Time>, mut q: Query<(&Firefly, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (f, mut tf) in &mut q {
        let a = t * f.speed + f.phase;
        tf.translation = f.center
            + Vec3::new(
                a.sin() * 1.3 + (a * 2.3).sin() * 0.3,
                (a * 1.7).sin() * 0.6,
                (a * 0.7).cos() * 0.8,
            );
        let blink = ((t * 1.3 + f.phase * 3.0).sin() * 0.5 + 0.5).powi(3);
        tf.scale = Vec3::splat(0.02 + 0.05 * blink);
    }
}
