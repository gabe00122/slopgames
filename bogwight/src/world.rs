//! Turns a level plan into entities: the ground's meshes and colliders, the
//! water, trees and reeds, the distant swamp, the sky, and everything that
//! moves.

use crate::{
    boats, fx,
    game::*,
    hunters,
    level::{self, DX, Plan, Terrain},
    meshkit::MeshBuilder,
    models::{Mats, Models},
    player, props,
    util::{Rng, fbm2, lin, mix, shade},
    water::{self, Water, WaterMaterials},
};
use avian2d::prelude::*;
use bevy::{
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};

/// Stretches of reeds the bogwight can hide in, and other facts about the
/// level that systems query often.
#[derive(Resource, Default)]
pub struct LevelInfo {
    pub reed_cover: Vec<(f32, f32)>,
    pub len: f32,
    pub sections: Vec<level::Section>,
}

impl LevelInfo {
    pub fn in_reeds(&self, x: f32) -> bool {
        self.reed_cover.iter().any(|&(a, b)| x >= a && x <= b)
    }

    pub fn section_at(&self, x: f32) -> Option<&level::Section> {
        self.sections.iter().find(|s| x >= s.x0 && x < s.x1)
    }
}

/// Gentle swaying for plants.
#[derive(Component)]
pub struct Sway {
    pub phase: f32,
    pub amp: f32,
    pub base: Quat,
}

/// Slow sideways drift for mist, wrapping around the level.
#[derive(Component)]
pub struct Drift {
    pub speed: f32,
    pub span: (f32, f32),
}

/// Sky pieces that follow the camera sideways.
#[derive(Component)]
pub struct SkyFollow {
    pub offset: Vec3,
    pub parallax: f32,
}

/// Rebuilds the world for a new night.
pub fn spawn_level(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    models: &Models,
    mats: &Mats,
    wmats: &WaterMaterials,
    plan: &Plan,
    with_player: bool,
    low: bool,
) {
    let t = &plan.terrain;
    let ground_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.55,
        reflectance: 0.35,
        ..default()
    });
    spawn_ground(commands, meshes, &ground_mat, t);
    let water = Water::new(t);
    water::spawn(commands, meshes, wmats, &water, t.len);
    commands.insert_resource(water);
    commands.insert_resource(t.clone());
    commands.insert_resource(LevelInfo {
        reed_cover: plan.reed_cover.clone(),
        len: t.len,
        sections: plan.sections.clone(),
    });

    let mut rng = Rng::new(t.len as u64 ^ 0xdec0);
    for d in &plan.trees {
        let y = t.height(d.x);
        // A tree in deep water would only show a bush floating on the surface.
        if t.depth(d.x) > 1.8 {
            continue;
        }
        commands.spawn((
            Mesh3d(models.trees[d.variant % models.trees.len()].clone()),
            MeshMaterial3d(mats.matte.clone()),
            Transform::from_xyz(d.x, y, d.z)
                .with_scale(Vec3::splat(d.scale))
                .with_rotation(Quat::from_rotation_y(rng.angle())),
            LevelEntity,
        ));
    }
    for d in &plan.reeds {
        let y = t.height(d.x);
        commands.spawn((
            Mesh3d(models.reeds[d.variant % models.reeds.len()].clone()),
            MeshMaterial3d(mats.cloth.clone()),
            Transform::from_xyz(d.x, y, d.z).with_scale(Vec3::splat(d.scale)),
            Sway {
                phase: rng.angle(),
                amp: 0.04,
                base: Quat::IDENTITY,
            },
            NotShadowCaster,
            LevelEntity,
        ));
    }
    for d in &plan.weeds {
        let y = t.height(d.x);
        commands.spawn((
            Mesh3d(models.weeds[d.variant % models.weeds.len()].clone()),
            MeshMaterial3d(mats.cloth.clone()),
            Transform::from_xyz(d.x, y, d.z).with_scale(Vec3::splat(d.scale)),
            Sway {
                phase: rng.angle(),
                amp: 0.12,
                base: Quat::IDENTITY,
            },
            NotShadowCaster,
            LevelEntity,
        ));
    }
    for &(x, z) in &plan.fungi {
        let y = t.height(x);
        let k = rng.below(2);
        let color = if k == 0 { lin(0x4affd0) } else { lin(0xb07aff) };
        commands
            .spawn((
                Mesh3d(models.fungus[k].clone()),
                MeshMaterial3d(mats.glow.clone()),
                Transform::from_xyz(x, y, z).with_scale(Vec3::splat(rng.range(0.8, 1.5))),
                NotShadowCaster,
                LevelEntity,
            ))
            .with_child((
                PointLight {
                    color: Color::linear_rgb(color[0], color[1], color[2]),
                    intensity: 9_000.0,
                    range: 4.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.4, 0.4),
            ));
    }
    // Stones and roots in the faces of the banks.
    let mut x = 1.0;
    while x < t.len - 1.0 {
        let h = t.height(x);
        if h > WATER_Y + 0.2 && rng.chance(0.5) {
            commands.spawn((
                Mesh3d(models.roots[rng.below(models.roots.len())].clone()),
                MeshMaterial3d(mats.matte.clone()),
                Transform::from_xyz(x, h - 0.05, Z_GROUND_FRONT + 0.02),
                NotShadowCaster,
                LevelEntity,
            ));
        }
        if rng.chance(0.45) {
            let s = rng.range(0.15, 0.45);
            commands.spawn((
                Mesh3d(models.rocks[rng.below(models.rocks.len())].clone()),
                MeshMaterial3d(mats.matte.clone()),
                Transform::from_xyz(x + rng.sym(0.8), h - rng.range(0.4, 3.0), Z_GROUND_FRONT - s * 0.4)
                    .with_scale(Vec3::new(s * 1.3, s, s))
                    .with_rotation(Quat::from_rotation_y(rng.angle())),
                NotShadowCaster,
                LevelEntity,
            ));
        }
        x += rng.range(1.2, 2.6);
    }
    for &(c, r) in &plan.rocks {
        let k = rng.below(models.rocks.len());
        commands.spawn((
            Mesh3d(models.rocks[k].clone()),
            MeshMaterial3d(mats.matte.clone()),
            Transform::from_xyz(c.x, c.y, 0.0)
                .with_scale(Vec3::new(r, r * 0.85, r.max(0.9)))
                .with_rotation(Quat::from_rotation_y(rng.angle())),
            RigidBody::Static,
            Collider::circle(r * 0.8),
            CollisionLayers::new(Layer::Ground, LayerMask::ALL),
            LevelEntity,
        ));
    }
    for &(x, z) in &plan.tents {
        let y = t.height(x);
        commands.spawn((
            Mesh3d(models.tent.clone()),
            MeshMaterial3d(mats.cloth.clone()),
            Transform::from_xyz(x, y - 0.05, z),
            LevelEntity,
        ));
    }
    for &p in &plan.campfires {
        props::spawn_campfire(commands, models, mats, p, low);
    }
    for &(p, scale) in &plan.mushrooms {
        props::spawn_mushroom(commands, models, mats, p, scale);
    }
    for dock in &plan.docks {
        props::spawn_dock(commands, models, mats, t, dock);
    }
    for bridge in &plan.bridges {
        props::spawn_bridge(commands, models, mats, bridge);
    }
    for &(p, dir) in &plan.lantern_posts {
        props::spawn_lantern_post(commands, models, mats, p, dir, low);
    }
    for p in &plan.props {
        props::spawn_prop(commands, models, mats, p);
    }
    for &c in &plan.fireflies {
        fx::spawn_fireflies(commands, models, mats, c, &mut rng);
    }
    let boats: Vec<Entity> = plan
        .boats
        .iter()
        .map(|b| boats::spawn_boat(commands, models, mats, b))
        .collect();
    for (i, h) in plan.hunters.iter().enumerate() {
        hunters::spawn_hunter(commands, models, mats, h, h.boat.map(|b| boats[b]), i as u64, low);
    }
    if with_player {
        player::spawn(commands, models, mats, plan.start);
    }
    spawn_scenery(commands, meshes, models, mats, t.len, &mut rng);
    // Walls at the ends of the swamp.
    for x in [-0.5, t.len + 0.5] {
        commands.spawn((
            RigidBody::Static,
            Collider::rectangle(1.0, 60.0),
            Transform::from_xyz(x, 10.0, 0.0),
            CollisionLayers::new(Layer::Ground, LayerMask::ALL),
            LevelEntity,
        ));
    }
}

const CHUNK: f32 = 32.0;
const GROUND_BOTTOM: f32 = -18.0;

fn ground_color(h: f32, zf: f32, x: f32) -> [f32; 4] {
    let n = fbm2(x * 0.6, zf * 3.0, 2, 17);
    if h > WATER_Y + 0.05 {
        // Moss at the front edge, mud behind.
        let moss = mix(lin(0x2f4424), lin(0x3d5530), n);
        let mud = mix(lin(0x3a2e22), lin(0x4a3a2a), n);
        let wet = shade(mix(mud, lin(0x2a2a20), 0.5), 0.8);
        let dry = mix(moss, mud, (zf * 1.2).clamp(0.0, 1.0) * 0.6 + n * 0.3);
        mix(wet, dry, ((h - WATER_Y) / 0.6).clamp(0.0, 1.0))
    } else {
        mix(lin(0x1f2a22), lin(0x2c3326), n)
    }
}

fn spawn_ground(commands: &mut Commands, meshes: &mut Assets<Mesh>, mat: &Handle<StandardMaterial>, t: &Terrain) {
    let chunks = (t.len / CHUNK).ceil() as usize;
    let zs = [Z_GROUND_FRONT, 0.4, -0.6, -2.0, -3.6, Z_BACK];
    for c in 0..chunks {
        let x0 = c as f32 * CHUNK;
        let x1 = ((c + 1) as f32 * CHUNK).min(t.len);
        let i0 = (x0 / DX) as usize;
        let i1 = ((x1 / DX) as usize).min(t.h.len() - 1);
        let mut b = MeshBuilder::new();
        // Top surface, a row of vertices per depth.
        let cols = i1 - i0 + 1;
        let base = 0u32;
        for i in i0..=i1 {
            let x = i as f32 * DX;
            let h = t.h[i];
            for (k, &z) in zs.iter().enumerate() {
                // The back rows sag and swell a little so the bank isn't a ribbon.
                let off = if k >= 3 {
                    (fbm2(x * 0.15, k as f32, 2, 3) - 0.5) * 0.6 * (k as f32 - 2.0) / 3.0
                } else {
                    0.0
                };
                let zf = (Z_GROUND_FRONT - z) / (Z_GROUND_FRONT - Z_BACK);
                b.v(Vec3::new(x, h + off, z), ground_color(h + off, zf, x));
            }
        }
        let rows = zs.len() as u32;
        for j in 0..cols as u32 - 1 {
            for k in 0..rows - 1 {
                let a = base + j * rows + k;
                let bb = base + (j + 1) * rows + k;
                b.quad(a, bb, bb + 1, a + 1);
            }
        }
        // Front face, down into the dark.
        let mut f = MeshBuilder::new();
        for i in i0..=i1 {
            let x = i as f32 * DX;
            let h = t.h[i];
            let wet = h <= WATER_Y;
            let n = fbm2(x * 0.35, 5.0, 3, 23);
            let top = if wet {
                lin(0x2c3a30)
            } else {
                mix(lin(0x4a3a28), lin(0x5a4630), n)
            };
            let band = if wet {
                lin(0x1e2a24)
            } else {
                mix(lin(0x33281c), lin(0x3e3022), n)
            };
            let deep = lin(0x15110d);
            let bottom = lin(0x080706);
            // Topsoil, a darker stratum, then the dark.
            f.v(Vec3::new(x, h, Z_GROUND_FRONT), top);
            f.v(Vec3::new(x, h - 0.35 - n * 0.3, Z_GROUND_FRONT), band);
            f.v(Vec3::new(x, h - 2.0 - n * 0.8, Z_GROUND_FRONT), deep);
            f.v(Vec3::new(x, GROUND_BOTTOM, Z_GROUND_FRONT), bottom);
        }
        for j in 0..cols as u32 - 1 {
            let a = j * 4;
            let n = a + 4;
            f.quad(a + 1, n + 1, n, a);
            f.quad(a + 2, n + 2, n + 1, a + 1);
            f.quad(a + 3, n + 3, n + 2, a + 2);
        }
        b.append(&f);
        commands.spawn((
            Mesh3d(meshes.add(b.build(false))),
            MeshMaterial3d(mat.clone()),
            Transform::IDENTITY,
            LevelEntity,
        ));
        // The collider: the surface as a polyline, overlapping neighbours by a sample.
        let pts: Vec<Vec2> = (i0..=i1.min(t.h.len() - 1))
            .map(|i| Vec2::new(i as f32 * DX, t.h[i]))
            .collect();
        commands.spawn((
            RigidBody::Static,
            Collider::polyline(pts, None),
            Friction::new(0.7),
            CollisionLayers::new(Layer::Ground, LayerMask::ALL),
            Transform::IDENTITY,
            LevelEntity,
        ));
    }
}

/// Distant hummocks and trees, drifting mist, the moon and the sky.
fn spawn_scenery(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    models: &Models,
    mats: &Mats,
    len: f32,
    rng: &mut Rng,
) {
    let count = (len / 5.0) as usize + 40;
    for _ in 0..count {
        let z = -rng.range(9.0, 110.0);
        let x = rng.range(-90.0, len + 90.0);
        let s = rng.range(3.0, 9.0);
        commands.spawn((
            Mesh3d(models.mound.clone()),
            MeshMaterial3d(mats.matte.clone()),
            Transform::from_xyz(x, -0.2, z).with_scale(Vec3::new(s, rng.range(0.8, 2.2), s * 0.6)),
            NotShadowCaster,
            LevelEntity,
        ));
        for _ in 0..rng.below(3) + 1 {
            commands.spawn((
                Mesh3d(models.trees[rng.below(models.trees.len())].clone()),
                MeshMaterial3d(mats.matte.clone()),
                Transform::from_xyz(x + rng.sym(s * 0.6), 0.0, z + rng.sym(2.0))
                    .with_scale(Vec3::splat(rng.range(1.0, 1.7)))
                    .with_rotation(Quat::from_rotation_y(rng.angle())),
                NotShadowCaster,
                LevelEntity,
            ));
        }
    }
    // Mist banks.
    for _ in 0..(len / 9.0) as usize {
        let z = -rng.range(2.0, 45.0);
        let s = rng.range(10.0, 26.0);
        commands.spawn((
            Mesh3d(models.quad.clone()),
            MeshMaterial3d(mats.mist.clone()),
            Transform::from_xyz(rng.range(-30.0, len + 30.0), rng.range(0.4, 2.5), z).with_scale(Vec3::new(
                s * 2.2,
                s * 0.45,
                1.0,
            )),
            Drift {
                speed: rng.range(0.2, 0.6),
                span: (-40.0, len + 40.0),
            },
            NotShadowCaster,
            NotShadowReceiver,
            LevelEntity,
        ));
    }
    // A low sheet of mist hugging the water in front.
    for _ in 0..(len / 18.0) as usize {
        let s = rng.range(8.0, 16.0);
        commands.spawn((
            Mesh3d(models.quad.clone()),
            MeshMaterial3d(mats.mist.clone()),
            Transform::from_xyz(rng.range(0.0, len), rng.range(0.1, 0.7), rng.range(1.6, 3.0)).with_scale(Vec3::new(
                s * 2.0,
                s * 0.18,
                1.0,
            )),
            Drift {
                speed: rng.range(0.3, 0.7),
                span: (-20.0, len + 20.0),
            },
            NotShadowCaster,
            NotShadowReceiver,
            LevelEntity,
        ));
    }
    // The moon, its halo, and the sky gradient behind everything.
    commands.spawn((
        Mesh3d(models.moon.clone()),
        MeshMaterial3d(mats.sky.clone()),
        Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::splat(26.0)),
        SkyFollow {
            offset: Vec3::new(-120.0, 170.0, -780.0),
            parallax: 0.97,
        },
        NotShadowCaster,
        LevelEntity,
    ));
    commands.spawn((
        Mesh3d(models.quad.clone()),
        MeshMaterial3d(mats.halo.clone()),
        Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::splat(120.0)),
        SkyFollow {
            offset: Vec3::new(-120.0, 170.0, -770.0),
            parallax: 0.97,
        },
        NotShadowCaster,
        LevelEntity,
    ));
    let top = lin(0x070b16);
    let mid = lin(0x16222c);
    let horizon = lin(0x2c3a38);
    let rows = [(-200.0, horizon), (0.0, horizon), (140.0, mid), (600.0, top)];
    let w = 3000.0;
    let mut sky = MeshBuilder::new();
    for k in 0..rows.len() - 1 {
        let (y0, c0) = rows[k];
        let (y1, c1) = rows[k + 1];
        let i0 = sky.v(Vec3::new(-w, y0, 0.0), c0);
        let i1 = sky.v(Vec3::new(w, y0, 0.0), c0);
        let i2 = sky.v(Vec3::new(w, y1, 0.0), c1);
        let i3 = sky.v(Vec3::new(-w, y1, 0.0), c1);
        sky.quad(i0, i1, i2, i3);
    }
    commands.spawn((
        Mesh3d(meshes.add(sky.build(false))),
        MeshMaterial3d(mats.sky.clone()),
        Transform::IDENTITY,
        SkyFollow {
            offset: Vec3::new(0.0, 0.0, -900.0),
            parallax: 1.0,
        },
        NotShadowCaster,
        NotShadowReceiver,
        LevelEntity,
    ));
}

pub fn sway(time: Res<Time>, mut q: Query<(&Sway, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (s, mut tf) in &mut q {
        let a = (t * 0.9 + s.phase).sin() * s.amp + (t * 2.3 + s.phase * 1.7).sin() * s.amp * 0.3;
        tf.rotation = s.base * Quat::from_rotation_z(a);
    }
}

pub fn drift(time: Res<Time>, mut q: Query<(&Drift, &mut Transform)>) {
    let dt = time.delta_secs();
    for (d, mut tf) in &mut q {
        tf.translation.x += d.speed * dt;
        if tf.translation.x > d.span.1 {
            tf.translation.x = d.span.0;
        }
    }
}

pub fn sky_follow(
    cam: Query<&Transform, (With<Camera3d>, Without<SkyFollow>)>,
    mut q: Query<(&SkyFollow, &mut Transform)>,
) {
    let Ok(c) = cam.single() else { return };
    for (s, mut tf) in &mut q {
        tf.translation = Vec3::new(c.translation.x * s.parallax, c.translation.y * 0.3, 0.0) + s.offset;
    }
}
