//! The world around the beast: sky, sun, clouds, and the sea or plain it
//! walks through. The beast stays near the origin and the scenery scrolls
//! past, like a treadmill.

use crate::{
    beast::{EnvDef, Ground, flora},
    course::{Course, CourseEntity, Mats},
    fx::{Fx, Puff},
    meshkit::MeshBuilder,
    util::{Rng, hex, lin, mix, noise2, shade, smoothstep},
};
use bevy::{
    asset::RenderAssetUsages,
    light::{CascadeShadowConfigBuilder, GlobalAmbientLight, NotShadowCaster, NotShadowReceiver},
    math::Affine3A,
    mesh::PrimitiveTopology,
    prelude::*,
};
use std::f32::consts::TAU;

/// Moves with the ground under the beast and wraps around to stay in range.
#[derive(Component)]
pub struct Scrolling {
    pub half: Vec2,
    pub factor: f32,
    pub wind: Vec3,
}

/// A foam ring where a leg breaks the water.
#[derive(Component)]
pub struct Splash(pub usize);

const SPLASHES: usize = 12;
const TILE: f32 = 900.0;
const GRID: i32 = 5;

pub fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &Mats, env: &EnvDef, rng: &mut Rng, low: bool) {
    commands.insert_resource(ClearColor(hex(env.fog)));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.82, 0.9, 1.0),
        brightness: env.ambient,
        ..default()
    });

    // Sun.
    let sun_dir = env.sun_dir.normalize();
    commands.spawn((
        DirectionalLight {
            illuminance: env.sun_lux,
            color: hex(env.sun_color),
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: if low { 2 } else { 4 },
            minimum_distance: 1.0,
            first_cascade_far_bound: if low { 90.0 } else { 45.0 },
            maximum_distance: if low { 500.0 } else { 900.0 },
            ..default()
        }
        .build(),
        Transform::IDENTITY.looking_to(sun_dir, Vec3::Y),
        CourseEntity,
    ));
    let mut sun = MeshBuilder::new();
    sun.ball(Vec3::ZERO, 210.0, 2, lin(0xfff6d8));
    commands.spawn((
        Mesh3d(meshes.add(sun.build(false, false))),
        MeshMaterial3d(mats.glow.clone()),
        Transform::from_translation(-sun_dir * 6200.0),
        NotShadowCaster,
        CourseEntity,
    ));

    // Sky dome with a bright halo around the sun.
    let mut sky = MeshBuilder::new();
    let (top, horizon, haze) = (lin(env.sky_top), lin(env.sky_horizon), lin(env.fog));
    sky.ico_with(Vec3::ZERO, Vec3::splat(7000.0), 4, |d| {
        let mut c = if d.y >= 0.0 {
            mix(horizon, top, smoothstep(0.0, 0.55, d.y).powf(0.8))
        } else {
            mix(horizon, haze, smoothstep(0.0, -0.08, d.y))
        };
        let toward = d.dot(-sun_dir).max(0.0);
        c = mix(c, lin(0xfffbea), toward.powf(9.0) * 0.4 + toward.powf(90.0) * 0.5);
        (1.0, c)
    });
    commands.spawn((
        Mesh3d(meshes.add(sky.build(false, false))),
        MeshMaterial3d(mats.sky.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        CourseEntity,
    ));

    // Clouds drift past a little slower than the ground, which sells the height.
    for _ in 0..34 {
        let mut cloud = MeshBuilder::new();
        let size = rng.range(60.0, 150.0);
        for k in 0..(4 + rng.below(4)) {
            let off = Vec3::new(rng.sym(1.3), rng.sym(0.12), rng.sym(0.8)) * size;
            let r = size * rng.range(0.4, 0.75) * if k == 0 { 1.2 } else { 1.0 };
            cloud.ico_with(off, Vec3::new(r * 1.3, r * 0.5, r), 1, |d| {
                (
                    1.0,
                    // Unlit and a little over 1.0, so clouds stay white after tone mapping.
                    shade(
                        mix(lin(0xc9d9ea), lin(0xffffff), (d.y * 0.7 + 0.55).clamp(0.0, 1.0)),
                        1.45,
                    ),
                )
            });
        }
        let half = Vec2::splat(3400.0);
        commands.spawn((
            Mesh3d(meshes.add(cloud.build(true, false))),
            MeshMaterial3d(mats.unlit.clone()),
            Transform::from_xyz(
                rng.sym(half.x),
                rng.range(env.cloud_height.0, env.cloud_height.1),
                rng.sym(half.y),
            ),
            Scrolling {
                half,
                factor: 0.5,
                wind: Vec3::new(rng.range(1.0, 4.0), 0.0, rng.range(-1.0, 1.0)),
            },
            NotShadowCaster,
            NotShadowReceiver,
            CourseEntity,
        ));
    }

    match env.ground {
        Ground::Sea { level, floor } => spawn_sea(commands, meshes, mats, rng, level, floor),
        Ground::Plain { level } => spawn_plain(commands, meshes, mats, rng, level),
        Ground::Sky { floor } => spawn_sky(commands, meshes, mats, rng, floor),
    }
}

fn tile_positions() -> impl Iterator<Item = Vec3> {
    (0..GRID * GRID).map(|i| {
        let (ix, iz) = (i % GRID - GRID / 2, i / GRID - GRID / 2);
        Vec3::new(ix as f32 * TILE, 0.0, iz as f32 * TILE)
    })
}

fn tile_scroll() -> Scrolling {
    Scrolling {
        half: Vec2::splat(TILE * GRID as f32 * 0.5),
        factor: 1.0,
        wind: Vec3::ZERO,
    }
}

fn spawn_sea(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &Mats, rng: &mut Rng, level: f32, floor: f32) {
    // Surface: a patch of faceted, moving swell around the crab, and flat sea beyond it.
    let half = WAVE_SIZE * 0.5;
    let far = 9000.0;
    let mut flat = MeshBuilder::new();
    let white = [1.0, 1.0, 1.0, 1.0];
    for (x0, x1, z0, z1) in [
        (-far, far, -far, -half),
        (-far, far, half, far),
        (-far, -half, -half, half),
        (half, far, -half, half),
    ] {
        flat.quad_p(
            [
                Vec3::new(x0, 0.0, z0),
                Vec3::new(x0, 0.0, z1),
                Vec3::new(x1, 0.0, z1),
                Vec3::new(x1, 0.0, z0),
            ],
            white,
        );
    }
    commands.spawn((
        Mesh3d(meshes.add(flat.build(true, false))),
        MeshMaterial3d(mats.sea.clone()),
        Transform::from_xyz(0.0, level, 0.0),
        NotShadowCaster,
        CourseEntity,
    ));
    let mut swell = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    let (pos, nor, col) = wave_mesh(0.0, Vec2::ZERO);
    swell.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    swell.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    swell.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    commands.spawn((
        Mesh3d(meshes.add(swell)),
        MeshMaterial3d(mats.sea.clone()),
        Transform::from_xyz(0.0, level, 0.0),
        Waves { drift: Vec2::ZERO },
        NotShadowCaster,
        CourseEntity,
    ));

    // Seabed tiles: mottled sand, boulders and coral heads, seen through the water.
    let mut bed = MeshBuilder::new();
    let cells = 12;
    let cell = TILE / cells as f32;
    for i in 0..cells {
        for k in 0..cells {
            let (x, z) = (-TILE * 0.5 + i as f32 * cell, -TILE * 0.5 + k as f32 * cell);
            let n = noise2(i as f32 * 0.5, k as f32 * 0.5, cells / 2, 5);
            let c = mix(lin(0x3f9f96), lin(0x8fc4a8), n);
            bed.quad_p(
                [
                    Vec3::new(x, 0.0, z),
                    Vec3::new(x, 0.0, z + cell),
                    Vec3::new(x + cell, 0.0, z + cell),
                    Vec3::new(x + cell, 0.0, z),
                ],
                c,
            );
        }
    }
    for _ in 0..26 {
        let p = Vec3::new(rng.sym(TILE * 0.47), 0.0, rng.sym(TILE * 0.47));
        bed.xf = Affine3A::from_translation(p);
        match rng.below(3) {
            0 => {
                let s = rng.range(8.0, 22.0);
                flora::boulders(&mut bed, s, lin(0x5d6d73), rng);
            }
            1 => {
                let c = lin([0xff6f91u32, 0xff9671, 0xc86bfa, 0xffc75f][rng.below(4)]);
                flora::coral(&mut bed, rng.range(14.0, 30.0), c, rng);
            }
            _ => {
                for _ in 0..3 {
                    bed.xf = Affine3A::from_translation(p + Vec3::new(rng.sym(10.0), 26.0, rng.sym(10.0)));
                    flora::strand(&mut bed, 26.0, lin(0x2f7d4f), rng);
                }
            }
        }
    }
    let bed = meshes.add(bed.build(true, false));

    // Foam flecks on the surface.
    let mut foam = MeshBuilder::new();
    for _ in 0..46 {
        let p = Vec3::new(rng.sym(TILE * 0.5), 0.0, rng.sym(TILE * 0.5));
        let (w, l) = (rng.range(1.0, 3.0), rng.range(5.0, 16.0));
        let a = rng.sym(0.5);
        let (dx, dz) = (
            Vec3::new(a.cos(), 0.0, a.sin()) * l,
            Vec3::new(-a.sin(), 0.0, a.cos()) * w,
        );
        foam.quad_p(
            [p - dx - dz, p - dx + dz, p + dx + dz * 0.4, p + dx - dz * 0.4],
            [1.0, 1.0, 1.0, 1.0],
        );
    }
    let foam = meshes.add(foam.build(true, false));

    for p in tile_positions() {
        commands.spawn((
            Mesh3d(bed.clone()),
            MeshMaterial3d(mats.matte.clone()),
            Transform::from_translation(p + Vec3::Y * floor),
            tile_scroll(),
            NotShadowCaster,
            CourseEntity,
        ));
        commands.spawn((
            Mesh3d(foam.clone()),
            MeshMaterial3d(mats.foam.clone()),
            Transform::from_translation(p + Vec3::Y * (level + 1.6)),
            tile_scroll(),
            NotShadowCaster,
            NotShadowReceiver,
            CourseEntity,
        ));
    }

    // Sea stacks and islets drifting by, clear of the crab's path.
    for k in 0..12 {
        let mut islet = MeshBuilder::new();
        let r = rng.range(40.0, 110.0);
        let h = rng.range(30.0, 130.0);
        islet.ico_with(
            Vec3::new(0.0, (level + floor) * 0.5, 0.0),
            Vec3::new(r, h - floor, r),
            2,
            |d| {
                let k = 0.8 + 0.35 * noise2(d.x * 3.0 + d.y * 2.0, d.z * 3.0, 0, 11);
                let c = if d.y > 0.55 {
                    lin(0x6dbb4c)
                } else {
                    mix(lin(0x6b6f76), lin(0xa49a86), d.y * 0.5 + 0.5)
                };
                (k, shade(c, 0.85 + 0.2 * k))
            },
        );
        for _ in 0..(2 + rng.below(4)) {
            let a = rng.angle();
            let rr = r * rng.range(0.0, 0.35);
            islet.xf = Affine3A::from_translation(Vec3::new(
                a.cos() * rr,
                (level + floor) * 0.5 + (h - floor) * 0.86,
                a.sin() * rr,
            ));
            flora::palm(&mut islet, rng.range(18.0, 30.0), rng);
        }
        let side = if k % 2 == 0 { 1.0 } else { -1.0 };
        let half = Vec2::new(2600.0, 2600.0);
        commands.spawn((
            Mesh3d(meshes.add(islet.build(true, false))),
            MeshMaterial3d(mats.matte.clone()),
            Transform::from_xyz(rng.sym(half.x), 0.0, side * rng.range(520.0, 2300.0)),
            Scrolling {
                half,
                factor: 1.0,
                wind: Vec3::ZERO,
            },
            CourseEntity,
        ));
    }

    // Foam rings around the legs.
    let mut ring = MeshBuilder::new();
    let n = 18;
    for k in 0..n {
        let (a0, a1) = (k as f32 / n as f32 * TAU, (k + 1) as f32 / n as f32 * TAU);
        let p = |a: f32, r: f32| Vec3::new(a.cos() * r, 0.0, a.sin() * r);
        ring.quad_p(
            [p(a0, 10.0), p(a1, 10.0), p(a1, 17.0), p(a0, 17.0)],
            [1.0, 1.0, 1.0, 1.0],
        );
    }
    let ring = meshes.add(ring.build(true, false));
    for i in 0..SPLASHES {
        commands.spawn((
            Mesh3d(ring.clone()),
            MeshMaterial3d(mats.foam.clone()),
            Transform::from_xyz(0.0, level + 0.4, 0.0),
            Visibility::Hidden,
            Splash(i),
            NotShadowCaster,
            NotShadowReceiver,
            CourseEntity,
        ));
    }
}

fn spawn_plain(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &Mats, rng: &mut Rng, level: f32) {
    // Rolling grassland tiles. The noise is periodic so tiles meet seamlessly.
    let height = |x: f32, z: f32| {
        let (u, v) = (x / TILE + 0.5, z / TILE + 0.5);
        (noise2(u * 4.0, v * 4.0, 4, 21) - 0.5) * 34.0 + (noise2(u * 9.0, v * 9.0, 9, 22) - 0.5) * 12.0
    };
    let mut land = MeshBuilder::new();
    let cells = 20;
    let cell = TILE / cells as f32;
    for i in 0..cells {
        for k in 0..cells {
            let (x, z) = (-TILE * 0.5 + i as f32 * cell, -TILE * 0.5 + k as f32 * cell);
            let n = noise2(i as f32 * 0.35, k as f32 * 0.35, 7, 31);
            let dry = noise2(i as f32 * 0.2 + 3.0, k as f32 * 0.2, 4, 33);
            let c = mix(
                mix(lin(0x7fbf4f), lin(0x5aa545), n),
                lin(0xc9c46a),
                smoothstep(0.55, 0.85, dry),
            );
            let p = |x: f32, z: f32| Vec3::new(x, height(x, z), z);
            land.quad_p(
                [p(x, z), p(x, z + cell), p(x + cell, z + cell), p(x + cell, z)],
                shade(c, 0.92 + 0.16 * rng.f()),
            );
        }
    }
    // Little forests and farmsteads: ordinary-sized, so they look like moss from the beast.
    for _ in 0..22 {
        let (cx, cz) = (rng.sym(TILE * 0.45), rng.sym(TILE * 0.45));
        for _ in 0..(8 + rng.below(14)) {
            let (x, z) = (cx + rng.sym(60.0), cz + rng.sym(60.0));
            land.xf = Affine3A::from_translation(Vec3::new(x, height(x, z), z));
            if rng.chance(0.6) {
                flora::conifer(&mut land, rng.range(16.0, 30.0), rng);
            } else {
                flora::broadleaf(&mut land, rng.range(14.0, 24.0), lin(0x3f9a46), rng);
            }
        }
    }
    for _ in 0..5 {
        let (x, z) = (rng.sym(TILE * 0.45), rng.sym(TILE * 0.45));
        land.xf =
            Affine3A::from_rotation_translation(Quat::from_rotation_y(rng.angle()), Vec3::new(x, height(x, z), z));
        land.boxy(Vec3::new(0.0, 4.0, 0.0), Vec3::new(7.0, 4.0, 5.0), lin(0xe8d9b5));
        land.quad_2(
            [
                Vec3::new(-8.0, 8.0, -6.0),
                Vec3::new(-8.0, 8.0, 6.0),
                Vec3::new(0.0, 13.0, 6.0),
                Vec3::new(0.0, 13.0, -6.0),
            ],
            lin(0xb5523a),
        );
        land.quad_2(
            [
                Vec3::new(8.0, 8.0, 6.0),
                Vec3::new(8.0, 8.0, -6.0),
                Vec3::new(0.0, 13.0, -6.0),
                Vec3::new(0.0, 13.0, 6.0),
            ],
            lin(0xa3452f),
        );
    }
    for _ in 0..14 {
        let (x, z) = (rng.sym(TILE * 0.47), rng.sym(TILE * 0.47));
        land.xf = Affine3A::from_translation(Vec3::new(x, height(x, z), z));
        let s = rng.range(6.0, 20.0);
        flora::boulders(&mut land, s, lin(0x8d8a80), rng);
    }
    let land = meshes.add(land.build(true, false));
    for p in tile_positions() {
        commands.spawn((
            Mesh3d(land.clone()),
            MeshMaterial3d(mats.matte.clone()),
            Transform::from_translation(p + Vec3::Y * level),
            tile_scroll(),
            NotShadowCaster,
            CourseEntity,
        ));
    }
    // A skirt of far green so the horizon never shows a gap beyond the tiles.
    let mut far = MeshBuilder::new();
    far.quad_p(
        [
            Vec3::new(-9000.0, 0.0, -9000.0),
            Vec3::new(-9000.0, 0.0, 9000.0),
            Vec3::new(9000.0, 0.0, 9000.0),
            Vec3::new(9000.0, 0.0, -9000.0),
        ],
        lin(0x6fb04a),
    );
    commands.spawn((
        Mesh3d(meshes.add(far.build(true, false))),
        MeshMaterial3d(mats.matte.clone()),
        Transform::from_xyz(0.0, level - 24.0, 0.0),
        NotShadowCaster,
        CourseEntity,
    ));

    // A ring of distant mountains. Too far to scroll.
    let mut peaks = MeshBuilder::new();
    let count = 46;
    for k in 0..count {
        let a = k as f32 / count as f32 * TAU + rng.sym(0.04);
        let dist = rng.range(4200.0, 5200.0);
        let h = rng.range(450.0, 1300.0);
        let r = h * rng.range(0.9, 1.5);
        let base = Vec3::new(a.cos() * dist, level - 40.0, a.sin() * dist);
        let rock = mix(lin(0x5f7a8c), lin(0x7d93a3), rng.f());
        peaks.cone(base, base + Vec3::Y * h, r, 7, rock);
        if h > 800.0 {
            peaks.cone(
                base + Vec3::Y * h * 0.72,
                base + Vec3::Y * (h + 2.0),
                r * 0.29,
                7,
                lin(0xf4f8fb),
            );
        }
    }
    commands.spawn((
        Mesh3d(meshes.add(peaks.build(true, false))),
        MeshMaterial3d(mats.matte.clone()),
        NotShadowCaster,
        CourseEntity,
    ));
}

const WAVE_SIZE: f32 = 2600.0;
const WAVE_CELLS: usize = 64;

/// The moving patch of sea. `drift` is how far the water has slid past the beast.
#[derive(Component)]
pub struct Waves {
    drift: Vec2,
}

/// Faceted swell: positions, flat normals and tints for every triangle.
fn wave_mesh(t: f32, drift: Vec2) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 4]>) {
    let n = WAVE_CELLS;
    let step = WAVE_SIZE / n as f32;
    let half = WAVE_SIZE * 0.5;
    // Heights on the grid; the swell dies away toward the edge to meet the flat sea.
    let mut grid = vec![Vec3::ZERO; (n + 1) * (n + 1)];
    for i in 0..=n {
        for k in 0..=n {
            let (x, z) = (-half + i as f32 * step, -half + k as f32 * step);
            let (wx, wz) = (x - drift.x, z - drift.y);
            let edge = 1.0 - (x.abs().max(z.abs()) / half);
            let calm = smoothstep(0.0, 0.25, edge);
            let h = 1.0 * (wx * 0.021 + wz * 0.013 + t * 0.9).sin()
                + 0.7 * (wx * -0.017 + wz * 0.034 + t * 1.3).sin()
                + 0.45 * (wx * 0.05 + wz * -0.041 + t * 1.9).sin();
            // Jitter the lattice a little so facets aren't a regular grid.
            let j = noise2(i as f32 * 1.7, k as f32 * 1.7, 0, 77) - 0.5;
            let inside = if i > 0 && i < n && k > 0 && k < n { 1.0 } else { 0.0 };
            grid[i * (n + 1) + k] = Vec3::new(x + j * step * 0.5 * inside, h * calm, z - j * step * 0.4 * inside);
        }
    }
    let count = n * n * 6;
    let (mut pos, mut nor, mut col) = (
        Vec::with_capacity(count),
        Vec::with_capacity(count),
        Vec::with_capacity(count),
    );
    let mut tri = |a: Vec3, b: Vec3, c: Vec3| {
        let normal = (b - a).cross(c - a).normalize_or(Vec3::Y).to_array();
        // Crests catch a little white.
        let lift = (((a.y + b.y + c.y) / 3.0 - 1.2) * 0.35).clamp(0.0, 0.35);
        let tint = [1.0 + lift * 2.0, 1.0 + lift * 1.6, 1.0 + lift, 1.0];
        for p in [a, b, c] {
            pos.push(p.to_array());
            nor.push(normal);
            col.push(tint);
        }
    };
    for i in 0..n {
        for k in 0..n {
            let a = grid[i * (n + 1) + k];
            let b = grid[i * (n + 1) + k + 1];
            let c = grid[(i + 1) * (n + 1) + k + 1];
            let d = grid[(i + 1) * (n + 1) + k];
            tri(a, b, c);
            tri(a, c, d);
        }
    }
    (pos, nor, col)
}

pub fn waves(
    time: Res<Time>,
    course: Res<Course>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut q: Query<(&Mesh3d, &mut Waves)>,
) {
    for (mesh, mut waves) in &mut q {
        waves.drift += Vec2::new(course.out.scroll.x, course.out.scroll.z) * time.delta_secs();
        let Some(mut mesh) = meshes.get_mut(&mesh.0) else {
            continue;
        };
        let (pos, nor, col) = wave_mesh(time.elapsed_secs(), waves.drift);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    }
}

fn spawn_sky(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &Mats, rng: &mut Rng, floor: f32) {
    // A sea of cloud far below: lumpy tiles that drift past.
    let mut tile = MeshBuilder::new();
    let (top, shadow) = (shade(lin(0xfff6ec), 1.4), shade(lin(0xcbb2de), 1.2));
    for _ in 0..34 {
        let p = Vec3::new(rng.sym(TILE * 0.5), rng.sym(14.0), rng.sym(TILE * 0.5));
        let r = rng.range(70.0, 150.0);
        tile.ico_with(p, Vec3::new(r * 1.4, r * 0.45, r), 1, |d| {
            (
                rng.range(0.9, 1.08),
                mix(shadow, top, (d.y * 0.7 + 0.5).clamp(0.0, 1.0)),
            )
        });
    }
    let tile = meshes.add(tile.build(true, false));
    for p in tile_positions() {
        commands.spawn((
            Mesh3d(tile.clone()),
            MeshMaterial3d(mats.unlit.clone()),
            Transform::from_translation(p + Vec3::Y * floor),
            tile_scroll(),
            NotShadowCaster,
            NotShadowReceiver,
            CourseEntity,
        ));
    }
    let mut under = MeshBuilder::new();
    under.quad_p(
        [
            Vec3::new(-9000.0, 0.0, -9000.0),
            Vec3::new(-9000.0, 0.0, 9000.0),
            Vec3::new(9000.0, 0.0, 9000.0),
            Vec3::new(9000.0, 0.0, -9000.0),
        ],
        shadow,
    );
    commands.spawn((
        Mesh3d(meshes.add(under.build(true, false))),
        MeshMaterial3d(mats.unlit.clone()),
        Transform::from_xyz(0.0, floor - 30.0, 0.0),
        NotShadowCaster,
        NotShadowReceiver,
        CourseEntity,
    ));

    // Floating islands at every height, some near enough to pass close by.
    for k in 0..18 {
        let mut isle = MeshBuilder::new();
        let r = rng.range(25.0, 90.0);
        // An upturned cone of rock with a grassy cap.
        isle.ico_with(Vec3::ZERO, Vec3::new(r, r * 0.35, r), 1, |d| {
            let c = if d.y > 0.3 { lin(0x7cc45a) } else { lin(0xb89a82) };
            (rng.range(0.9, 1.1), shade(c, 0.85 + 0.25 * d.y.max(0.0)))
        });
        isle.cone(
            Vec3::new(0.0, -r * 0.2, 0.0),
            Vec3::new(rng.sym(r * 0.3), -r * 1.7, rng.sym(r * 0.3)),
            r * 0.8,
            7,
            lin(0x9a7f6c),
        );
        for _ in 0..(1 + rng.below(4)) {
            let a = rng.angle();
            let rr = r * rng.range(0.0, 0.6);
            isle.xf = Affine3A::from_translation(Vec3::new(a.cos() * rr, r * 0.3, a.sin() * rr));
            if rng.chance(0.5) {
                flora::broadleaf(&mut isle, rng.range(14.0, 26.0), lin(0xf2a7c3), rng);
            } else {
                flora::conifer(&mut isle, rng.range(16.0, 28.0), rng);
            }
        }
        let side = if k % 2 == 0 { 1.0 } else { -1.0 };
        let half = Vec2::new(2600.0, 2600.0);
        commands.spawn((
            Mesh3d(meshes.add(isle.build(true, false))),
            MeshMaterial3d(mats.matte.clone()),
            Transform::from_xyz(
                side * rng.range(380.0, 2200.0),
                rng.range(-280.0, 180.0),
                rng.sym(half.y),
            ),
            Scrolling {
                half,
                factor: 1.0,
                wind: Vec3::ZERO,
            },
            NotShadowCaster,
            CourseEntity,
        ));
    }
    // Towering cumulus on the horizon.
    let mut towers = MeshBuilder::new();
    for k in 0..30 {
        let a = k as f32 / 30.0 * TAU + rng.sym(0.08);
        let dist = rng.range(4200.0, 5600.0);
        let base = Vec3::new(a.cos() * dist, floor, a.sin() * dist);
        let h = rng.range(300.0, 1100.0);
        for j in 0..4 {
            let f = j as f32 / 3.0;
            let r = h * (0.45 - 0.2 * f) * rng.range(0.8, 1.2);
            towers.ico_with(
                base + Vec3::Y * h * f * 0.9,
                Vec3::new(r * 1.3, r * 0.8, r * 1.3),
                1,
                |d| (1.0, mix(shadow, top, (d.y * 0.6 + 0.5 + f * 0.2).clamp(0.0, 1.0))),
            );
        }
    }
    commands.spawn((
        Mesh3d(meshes.add(towers.build(true, false))),
        MeshMaterial3d(mats.unlit.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        CourseEntity,
    ));
}

/// Spray over any erupting geyser in the road.
pub fn geysers(course: Res<Course>, mut fx: ResMut<Fx>, mut tick: Local<u32>) {
    if !course.out.spout {
        return;
    }
    *tick += 1;
    for &u in &course.track.geysers {
        let loc = course.track.at(u);
        let base = loc.p + loc.n * 0.5;
        for _ in 0..5 {
            let j = fx.jitter(1.4);
            fx.puff(Puff::Water, base + j, loc.n * 34.0 + j * 3.0, 1.4, 2.2);
        }
        if tick.is_multiple_of(3) {
            fx.burst(Puff::Water, base, Vec3::ZERO, 3, 8.0, 0.6, 1.2);
        }
    }
}

pub fn scroll(time: Res<Time>, course: Res<Course>, mut q: Query<(&mut Transform, &Scrolling)>) {
    let dt = time.delta_secs();
    for (mut t, s) in &mut q {
        t.translation += (course.out.scroll * s.factor + s.wind) * dt;
        let p = &mut t.translation;
        p.x = (p.x + s.half.x).rem_euclid(s.half.x * 2.0) - s.half.x;
        p.z = (p.z + s.half.y).rem_euclid(s.half.y * 2.0) - s.half.y;
    }
}

pub fn splashes(time: Res<Time>, course: Res<Course>, mut q: Query<(&Splash, &mut Transform, &mut Visibility)>) {
    let t = time.elapsed_secs();
    for (s, mut tf, mut vis) in &mut q {
        match course.out.splashes.get(s.0) {
            Some(p) => {
                tf.translation.x = p.x;
                tf.translation.z = p.z;
                let pulse = 1.0 + 0.2 * (t * 2.6 + s.0 as f32 * 1.9).sin();
                tf.scale = Vec3::new(pulse, 1.0, pulse);
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}
