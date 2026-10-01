//! The storm: moonlight and murk, rain streaking past the camera and
//! pocking the water, and lightning that flickers across the whole swamp,
//! lighting up anything that isn't under water for the hunters to see.

use crate::{
    audio::{Sfx, Sound},
    fx::Fx,
    game::*,
    meshkit::MeshBuilder,
    models::Mats,
    util::{Rng, hex, lerp, lin},
    water::Water,
};
use bevy::{
    asset::RenderAssetUsages,
    light::{GlobalAmbientLight, NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
    prelude::*,
};

#[derive(Resource)]
pub struct Storm {
    /// Current lightning brightness (0 is none, ~1 is a full flash).
    pub flash: f32,
    pub rain: f32,
    next: f32,
    t: f32,
    pulses: Vec<(f32, f32)>,
    active: bool,
    thunder: Vec<(f32, bool)>,
    bolt: Option<Entity>,
    rng: Rng,
    time: f32,
    /// A strike is coming: the sky flickers faintly first.
    pub warning: f32,
}

impl Default for Storm {
    fn default() -> Self {
        Storm {
            flash: 0.0,
            rain: 0.8,
            next: 7.0,
            t: 0.0,
            pulses: vec![],
            active: false,
            thunder: vec![],
            bolt: None,
            rng: Rng::new(0x57_0e),
            time: 0.0,
            warning: 0.0,
        }
    }
}

#[derive(Component)]
pub struct Moon;

pub fn setup_lights(mut commands: Commands) {
    commands.insert_resource(GlobalAmbientLight {
        color: hex(0x7890b0),
        brightness: 260.0,
        ..default()
    });
    commands.spawn((
        DirectionalLight {
            illuminance: 2_600.0,
            color: hex(0xa8c0ff),
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::IDENTITY.looking_to(Vec3::new(0.35, -0.8, -0.45), Vec3::Y),
        Moon,
    ));
}

/// Lightning timing and thunder.
pub fn storm(
    mut commands: Commands,
    time: Res<Time>,
    mut storm: ResMut<Storm>,
    mut meshes: ResMut<Assets<Mesh>>,
    mats: Res<Mats>,
    cam: Query<&Transform, With<Camera3d>>,
    mut sfx: MessageWriter<Sfx>,
    mut fx: ResMut<Fx>,
) {
    let dt = time.delta_secs();
    storm.time += dt;
    let st = storm.time;
    storm.rain = (0.75 + 0.25 * (st * 0.05).sin()).clamp(0.0, 1.0);
    storm.next -= dt;
    storm.warning = if storm.next < 1.2 && storm.next > 0.0 {
        // Distant flickers over the horizon.
        let k = (storm.next * 23.0).sin().max(0.0) * (st * 41.0).sin().max(0.0);
        k * 0.12
    } else {
        0.0
    };
    if !storm.active && storm.next <= 0.0 {
        storm.active = true;
        storm.t = 0.0;
        let mut pulses = vec![(0.0, 0.45)];
        let n = storm.rng.below(3) + 2;
        let mut at = 0.08;
        for _ in 0..n {
            pulses.push((at, storm.rng.range(0.6, 1.15)));
            at += storm.rng.range(0.07, 0.2);
        }
        storm.pulses = pulses;
        let near = storm.rng.chance(0.3);
        let delay = if near {
            storm.rng.range(0.05, 0.3)
        } else {
            storm.rng.range(0.9, 2.6)
        };
        storm.thunder.push((delay, near));
        if let Ok(c) = cam.single() {
            let x = c.translation.x + storm.rng.sym(28.0);
            let z = if near {
                -storm.rng.range(10.0, 18.0)
            } else {
                -storm.rng.range(40.0, 110.0)
            };
            let mesh = bolt_mesh(&mut storm.rng, Vec3::new(x, 0.0, z));
            let e = commands
                .spawn((
                    Mesh3d(meshes.add(mesh.build(false))),
                    MeshMaterial3d(mats.bolt.clone()),
                    Transform::IDENTITY,
                    NotShadowCaster,
                    NotShadowReceiver,
                    LevelEntity,
                ))
                .id();
            storm.bolt = Some(e);
        }
        storm.next = storm.rng.range(10.0, 24.0);
    }
    if storm.active {
        storm.t += dt;
        let t = storm.t;
        storm.flash = storm
            .pulses
            .iter()
            .filter(|(t0, _)| t >= *t0)
            .map(|&(t0, a)| a * (-(t - t0) * 13.0).exp())
            .sum::<f32>();
        if t > 1.2 {
            storm.active = false;
            storm.flash = 0.0;
            if let Some(b) = storm.bolt.take() {
                commands.entity(b).despawn();
            }
        } else if let Some(b) = storm.bolt {
            let vis = if storm.flash > 0.25 {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
            commands.entity(b).insert(vis);
        }
    } else {
        storm.flash = storm.warning;
    }
    let mut due = vec![];
    for th in storm.thunder.iter_mut() {
        th.0 -= dt;
        if th.0 <= 0.0 {
            due.push(th.1);
        }
    }
    storm.thunder.retain(|th| th.0 > 0.0);
    for near in due {
        sfx.write(Sfx::ui(if near { Sound::ThunderNear } else { Sound::Thunder }));
        fx.shake(if near { 0.5 } else { 0.15 });
    }
}

/// A jagged, branching bolt from the clouds down to `base`.
fn bolt_mesh(rng: &mut Rng, base: Vec3) -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let c = lin(0xe0e8ff);
    let top = base + Vec3::new(rng.sym(10.0), 60.0, 0.0);
    let branch = |b: &mut MeshBuilder, from: Vec3, to: Vec3, width: f32, depth: u32, rng: &mut Rng| {
        let steps = 14;
        let mut prev = from;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let mut p = from.lerp(to, t);
            if i < steps {
                p += Vec3::new(rng.sym(2.2), rng.sym(0.8), 0.0);
            }
            b.beam(prev, p, width * (1.0 - t * 0.5), c);
            if depth > 0 && rng.chance(0.18) {
                let end = p + Vec3::new(rng.sym(12.0), -rng.range(6.0, 16.0), 0.0);
                let mut q = p;
                for k in 1..=6 {
                    let tt = k as f32 / 6.0;
                    let r = p.lerp(end, tt) + Vec3::new(rng.sym(1.2), rng.sym(0.6), 0.0);
                    b.beam(q, r, width * 0.4 * (1.0 - tt * 0.6), c);
                    q = r;
                }
            }
            prev = p;
        }
    };
    branch(&mut b, top, base, 0.35, 1, rng);
    b
}

/// Moon, ambient light and fog follow the lightning and the camera's depth.
pub fn lighting(
    storm: Res<Storm>,
    mut moon: Query<&mut DirectionalLight, With<Moon>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
    mut cams: Query<(&Transform, &mut DistanceFog), With<Camera3d>>,
    water: Option<Res<Water>>,
) {
    let f = storm.flash;
    for mut m in &mut moon {
        m.illuminance = 2_600.0 + f * 40_000.0;
    }
    ambient.brightness = 260.0 + f * 700.0;
    for (tf, mut fog) in &mut cams {
        let under = water
            .as_deref()
            .and_then(|w| w.surface(tf.translation.x))
            .is_some_and(|s| tf.translation.y < s);
        let base = if under {
            Vec3::new(0.02, 0.07, 0.06)
        } else {
            Vec3::new(0.055, 0.075, 0.085)
        };
        let lit = Vec3::new(0.2, 0.24, 0.32);
        let c = base.lerp(lit, (f * 0.8).min(1.0));
        fog.color = Color::srgb(c.x, c.y, c.z);
        fog.falloff = if under {
            FogFalloff::Linear { start: 14.0, end: 45.0 }
        } else {
            FogFalloff::Linear {
                start: lerp(24.0, 40.0, f.min(1.0)),
                end: lerp(150.0, 260.0, f.min(1.0)),
            }
        };
        clear.0 = fog.color;
    }
}

const DROPS: usize = 900;

#[derive(Resource)]
pub struct Rain {
    handle: Handle<Mesh>,
    drops: Vec<Vec3>,
    rng: Rng,
    count: usize,
}

fn rain_mesh(n: usize) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; n * 4]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; n * 4]);
    let mut idx = Vec::with_capacity(n * 6);
    for i in 0..n as u32 {
        let a = i * 4;
        idx.extend_from_slice(&[a, a + 1, a + 2, a, a + 2, a + 3]);
    }
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

pub fn setup_rain(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mats: Res<Mats>, args: Res<crate::Args>) {
    let count = if args.low { DROPS / 2 } else { DROPS };
    let handle = meshes.add(rain_mesh(count));
    commands.spawn((
        Mesh3d(handle.clone()),
        MeshMaterial3d(mats.rain.clone()),
        Transform::IDENTITY,
        NotShadowCaster,
        NotShadowReceiver,
    ));
    let mut rng = Rng::new(0x4a1);
    let drops = (0..count)
        .map(|_| Vec3::new(rng.sym(24.0), rng.range(-14.0, 16.0), rng.range(-9.0, 6.0)))
        .collect();
    commands.insert_resource(Rain {
        handle,
        drops,
        rng,
        count,
    });
}

pub fn rain(
    time: Res<Time>,
    storm: Res<Storm>,
    mut rain: ResMut<Rain>,
    mut meshes: ResMut<Assets<Mesh>>,
    cam: Query<&Transform, With<Camera3d>>,
    mut water: Option<ResMut<Water>>,
    terrain: Option<Res<crate::level::Terrain>>,
) {
    let Ok(c) = cam.single() else { return };
    let dt = time.delta_secs();
    let center = c.translation;
    let wind = -3.0;
    let fall = Vec3::new(wind, -19.0, 0.0);
    let active = (rain.count as f32 * storm.rain) as usize;
    let Rain { handle, drops, rng, .. } = &mut *rain;
    let Some(mut mesh) = meshes.get_mut(&*handle) else {
        return;
    };
    let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) else {
        return;
    };
    let dir = fall.normalize();
    let len = 0.55;
    for (i, d) in drops.iter_mut().enumerate() {
        // Keep drops in a box around the camera's view of the playfield.
        let rel_x = d.x - (center.x);
        if rel_x < -24.0 {
            d.x += 48.0;
        } else if rel_x > 24.0 {
            d.x -= 48.0;
        }
        *d += fall * dt;
        let in_play = d.z > Z_BACK && d.z < Z_WATER_FRONT;
        // Drops stop at the water or the ground, whatever their depth, so
        // none streak across the cut-away view under the surface.
        let ground = terrain.as_deref().map_or(WATER_Y, |t| t.height(d.x));
        let surface = water.as_deref().and_then(|w| w.surface(d.x)).unwrap_or(WATER_Y);
        let floor = ground.max(surface);
        let hit = d.y < floor;
        if hit
            && in_play
            && let Some(w) = water.as_deref_mut()
            && rng.chance(0.3)
        {
            w.disturb(d.x, -0.25);
        }
        if hit || d.y < center.y - 14.0 {
            d.x = center.x + rng.sym(24.0);
            d.y = center.y + rng.range(10.0, 16.0);
            d.z = rng.range(-9.0, 6.0);
        }
        let k = i * 4;
        if i >= active {
            for j in 0..4 {
                pos[k + j] = [0.0; 3];
            }
            continue;
        }
        let w = 0.01 + (d.z + 9.0).max(0.0) * 0.0006;
        let side = Vec3::new(-dir.y, dir.x, 0.0) * w;
        let a = *d;
        let b = *d - dir * len;
        pos[k] = (a - side).to_array();
        pos[k + 1] = (a + side).to_array();
        pos[k + 2] = (b + side).to_array();
        pos[k + 3] = (b - side).to_array();
    }
}
