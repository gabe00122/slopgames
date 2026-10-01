//! Builds a match: the island, its lakes and forests, the sky, both altars,
//! the compute wells, the software lying around and the two agents.
//! The map is point-symmetric so neither side has the better ground.

use crate::{
    agent::spawn_agent,
    fx::{Fx, Puff},
    game::{MatchEntity, Team},
    meshkit::MeshBuilder,
    models::{Mats, Models, sky_glyph},
    software::{NamePool, spawn_software},
    spells::Blast,
    structures::{spawn_altar, spawn_well},
    terrain::{HALF, Paving, Terrain, TerrainMaterial, WATER, spawn_chunks, vert_pos},
    util::{Rng, fbm2, hex, lin, mix, noise2, smoothstep, xz},
};
use bevy::{
    light::{CascadeShadowConfigBuilder, GlobalAmbientLight, NotShadowCaster, NotShadowReceiver},
    prelude::*,
};
use std::f32::consts::TAU;

/// Blue's altar; Red's is its mirror image through the center.
pub const BLUE_ALTAR: Vec2 = Vec2::new(-62.0, 66.0);
/// Blue's side of the wells; mirrored for Red, plus one in the middle.
const WELLS: [Vec2; 3] = [Vec2::new(-26.0, 76.0), Vec2::new(-80.0, 24.0), Vec2::new(-30.0, 30.0)];

pub fn altar_pos(team: Team) -> Vec2 {
    match team {
        Team::Blue => BLUE_ALTAR,
        Team::Red => -BLUE_ALTAR,
    }
}

pub fn well_positions() -> Vec<Vec2> {
    let mut v: Vec<Vec2> = WELLS.to_vec();
    v.extend(WELLS.iter().map(|w| -*w));
    v.push(Vec2::ZERO);
    v
}

/// Scenery that sits on the ground and follows it when it moves.
#[derive(Component)]
pub struct Decor {
    pub at: Vec2,
    pub sink: f32,
    pub radius: f32,
}

/// Slowly turning glyphs in the sky.
#[derive(Component)]
pub struct SkyGlyph {
    pub spin: f32,
    pub base: Vec3,
    pub phase: f32,
}

/// The lake surface.
#[derive(Component)]
pub struct Lake;

/// One half of the island; the terrain averages it with its mirror image.
fn raw_height(p: Vec2, seed: u32) -> f32 {
    let s = seed;
    let hills = (fbm2(p.x / 60.0, p.y / 60.0, 4, s) - 0.45) * 26.0;
    let ridge = {
        let n = fbm2(p.x / 34.0 + 7.0, p.y / 34.0 - 3.0, 3, s + 11);
        (1.0 - (n * 2.0 - 1.0).abs()).powi(3) * 9.0
    };
    let bumps = (noise2(p.x / 8.0, p.y / 8.0, s + 23) - 0.5) * 1.6;
    // Lakes in the low parts of a broad wave.
    let basin = (fbm2(p.x / 45.0 - 13.0, p.y / 45.0 + 5.0, 2, s + 31) - 0.5) * 18.0;
    let basin = basin.min(0.0) * 1.3;
    let inland = 4.0 + hills + ridge + bumps + basin;
    // The island: a rounded square whose coast wanders. Beyond it the ground
    // drops into deep water; in places the coast rises into cliffs instead.
    let r = (p.x.abs().powi(4) + p.y.abs().powi(4)).powf(0.25);
    let coast = HALF - 19.0 + (fbm2(p.x / 26.0, p.y / 26.0, 3, s + 41) - 0.5) * 16.0;
    let t = r - coast;
    let cliffs = smoothstep(0.52, 0.66, fbm2(p.x / 38.0 + 3.0, p.y / 38.0 - 9.0, 2, s + 51));
    let ridge_up = cliffs * smoothstep(-18.0, -4.0, t) * smoothstep(10.0, 0.0, t) * (16.0 + ridge * 1.5);
    let sea = smoothstep(-8.0, 10.0, t);
    let h = inland + ridge_up;
    h + (-9.0 - h) * sea * (1.0 - cliffs * 0.6)
}

pub fn height_at(p: Vec2, seed: u32) -> f32 {
    let mut h = 0.5 * (raw_height(p, seed) + raw_height(-p, seed));
    // Flatten the plazas under the altars and around the wells.
    let mut flats: Vec<(Vec2, f32, f32)> = vec![(BLUE_ALTAR, 12.0, 26.0), (-BLUE_ALTAR, 12.0, 26.0)];
    for w in well_positions() {
        flats.push((w, 5.0, 13.0));
    }
    for (c, inner, outer) in flats {
        let target = 0.5 * (raw_height(c, seed) + raw_height(-c, seed));
        let target = target.clamp(2.0, 9.0);
        let k = smoothstep(outer, inner, p.distance(c));
        h = h + (target - h) * k;
    }
    // Keep a dry, gentle path between the altars through the middle.
    let axis = (-BLUE_ALTAR).normalize();
    let along = p.dot(axis);
    let off = (p - axis * along).length();
    let path = smoothstep(14.0, 4.0, off) * smoothstep(110.0, 80.0, along.abs());
    h = h + (h.max(1.5) - h) * path;
    h
}

pub fn build_terrain(seed: u32) -> Terrain {
    let mut t = Terrain::generate(|p| height_at(p, seed), seed);
    let n = crate::terrain::CELLS + 1;
    for j in 0..n {
        for i in 0..n {
            let p = vert_pos(i, j);
            let k = j * n + i;
            for team in Team::BOTH {
                let a = altar_pos(team);
                let d = (p - a).abs();
                if d.x.max(d.y) < 11.0 {
                    t.paving[k] = Paving::Altar(team.i());
                }
                t.lock[k] = t.lock[k].max(smoothstep(16.0, 12.0, p.distance(a)));
            }
            for w in well_positions() {
                let d = p.distance(w);
                if d < 4.6 {
                    t.paving[k] = Paving::Well;
                }
                t.lock[k] = t.lock[k].max(smoothstep(8.0, 5.5, d));
            }
        }
    }
    t
}

/// Spawns everything for a new match.
pub fn spawn_world(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    models: &Models,
    mats: &Mats,
    terrain_mat: &TerrainMaterial,
    seed: u64,
    blue_player: bool,
    low: bool,
) -> [Entity; 2] {
    let mut terrain = build_terrain(seed as u32);
    spawn_chunks(commands, meshes, terrain_mat, &mut terrain);
    let mut rng = Rng::new(seed ^ 0x5eed);
    let mut names = NamePool::new(seed);

    // Lighting: a low evening sun.
    commands.insert_resource(ClearColor(hex(0x2a2346)));
    commands.insert_resource(GlobalAmbientLight {
        color: hex(0xb8a8ff),
        brightness: 380.0,
        ..default()
    });
    let sun_dir = Vec3::new(0.55, -0.42, 0.62).normalize();
    commands.spawn((
        DirectionalLight {
            illuminance: 9_500.0,
            color: hex(0xffd2a8),
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: if low { 2 } else { 4 },
            minimum_distance: 0.5,
            first_cascade_far_bound: if low { 40.0 } else { 22.0 },
            maximum_distance: if low { 160.0 } else { 260.0 },
            ..default()
        }
        .build(),
        Transform::IDENTITY.looking_to(sun_dir, Vec3::Y),
        MatchEntity,
    ));
    // A cool rim light from the other side.
    commands.spawn((
        DirectionalLight {
            illuminance: 1_600.0,
            color: hex(0x8fa8ff),
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::IDENTITY.looking_to(Vec3::new(-0.5, -0.6, -0.4), Vec3::Y),
        MatchEntity,
    ));

    spawn_sky(commands, meshes, mats, sun_dir, &mut rng);

    // The lake surface over the whole map; the ground pokes through it.
    let mut lake = MeshBuilder::new();
    let s = 1300.0;
    lake.quad_p(
        [
            Vec3::new(-s, 0.0, s),
            Vec3::new(s, 0.0, s),
            Vec3::new(s, 0.0, -s),
            Vec3::new(-s, 0.0, -s),
        ],
        [1.0; 4],
    );
    commands.spawn((
        Mesh3d(meshes.add(lake.build(true))),
        MeshMaterial3d(mats.water.clone()),
        Transform::from_xyz(0.0, WATER, 0.0),
        NotShadowCaster,
        Lake,
        MatchEntity,
    ));

    // Altars face each other.
    for team in Team::BOTH {
        let a = altar_pos(team);
        let to_center = -a;
        let facing = to_center.x.atan2(to_center.y);
        spawn_altar(commands, models, mats, team, terrain.ground(a), facing);
    }
    let wells = well_positions();
    for w in &wells {
        spawn_well(commands, models, mats, terrain.ground(*w));
    }

    // Scenery, placed in mirrored pairs.
    let clear = |p: Vec2| {
        let far_from = |c: Vec2, r: f32| p.distance(c) > r;
        far_from(BLUE_ALTAR, 20.0)
            && far_from(-BLUE_ALTAR, 20.0)
            && wells.iter().all(|w| far_from(*w, 9.0))
            && Terrain::inside(p, 10.0)
    };
    let mut placed = 0;
    let mut tries = 0;
    while placed < 70 && tries < 2000 {
        tries += 1;
        let p = Vec2::new(rng.sym(HALF - 12.0), rng.sym(HALF - 12.0));
        let h = terrain.height_v(p);
        let up = terrain.normal(p.x, p.y).y;
        // Forests like the gentle middle heights.
        let forest = fbm2(p.x / 40.0, p.y / 40.0, 2, seed as u32 + 71);
        if !clear(p) || h < WATER + 0.8 || h > 18.0 || up < 0.8 || forest < 0.45 {
            continue;
        }
        let variant = rng.below(models.trees.len());
        let yaw = rng.angle();
        let scale = rng.range(0.85, 1.3);
        for q in [p, -p] {
            spawn_decor(commands, models, mats, &terrain, q, true, variant, yaw, scale);
        }
        placed += 1;
    }
    placed = 0;
    tries = 0;
    while placed < 30 && tries < 2000 {
        tries += 1;
        let p = Vec2::new(rng.sym(HALF - 12.0), rng.sym(HALF - 12.0));
        if !clear(p) || terrain.height_v(p) < WATER - 1.5 {
            continue;
        }
        let variant = rng.below(models.rocks.len());
        let yaw = rng.angle();
        let scale = rng.range(0.7, 1.5);
        for q in [p, -p] {
            spawn_decor(commands, models, mats, &terrain, q, false, variant, yaw, scale);
        }
        placed += 1;
    }

    // Wild software, in mirrored pairs.
    placed = 0;
    tries = 0;
    while placed < 8 && tries < 2000 {
        tries += 1;
        let p = Vec2::new(rng.sym(HALF - 20.0), rng.sym(HALF - 20.0));
        let h = terrain.height_v(p);
        if !clear(p) || h < WATER + 0.3 || h > 16.0 || p.length() < 12.0 {
            continue;
        }
        for q in [p, -p] {
            spawn_software(
                commands,
                models,
                mats,
                names.take(),
                None,
                terrain.ground(q) + Vec3::Y * 1.3,
            );
        }
        placed += 1;
    }
    // A few in the middle, worth fighting over.
    for k in 0..3 {
        let a = k as f32 / 3.0 * TAU + 0.5;
        let p = Vec2::from_angle(a) * 7.0;
        spawn_software(
            commands,
            models,
            mats,
            names.take(),
            None,
            terrain.ground(p) + Vec3::Y * 1.3,
        );
    }

    // The agents, in front of their altars.
    let mut agents = [Entity::PLACEHOLDER; 2];
    for team in Team::BOTH {
        let home = terrain.ground(altar_pos(team));
        let front = altar_pos(team) + (-altar_pos(team)).normalize() * 17.0;
        let dir = -altar_pos(team);
        let yaw = (-dir.x).atan2(-dir.y);
        let software: Vec<String> = (0..5).map(|_| names.take()).collect();
        agents[team.i()] = spawn_agent(
            commands,
            models,
            mats,
            team,
            home,
            terrain.ground(front),
            yaw,
            team == Team::Blue && blue_player,
            software,
        );
    }

    commands.insert_resource(names);
    commands.insert_resource(terrain);
    agents
}

fn spawn_decor(
    commands: &mut Commands,
    models: &Models,
    mats: &Mats,
    terrain: &Terrain,
    p: Vec2,
    tree: bool,
    variant: usize,
    yaw: f32,
    scale: f32,
) {
    let model = if tree {
        &models.trees[variant]
    } else {
        &models.rocks[variant]
    };
    let sink = if tree { 0.2 } else { 0.4 };
    commands
        .spawn((
            Transform::from_translation(terrain.ground(p) - Vec3::Y * sink)
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::splat(scale)),
            Visibility::default(),
            Decor {
                at: p,
                sink,
                radius: if tree { 1.2 } else { 1.8 } * scale,
            },
            MatchEntity,
        ))
        .with_children(|c| {
            c.spawn((Mesh3d(model.body()), MeshMaterial3d(mats.matte.clone())));
            if let Some(g) = &model.glow {
                c.spawn((Mesh3d(g.clone()), MeshMaterial3d(mats.glow.clone()), NotShadowCaster));
            }
        });
}

fn spawn_sky(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &Mats, sun_dir: Vec3, rng: &mut Rng) {
    // Dusk dome with a halo around the sun and a scatter of stars overhead.
    let mut sky = MeshBuilder::new();
    let (top, mid, horizon, below) = (lin(0x141033), lin(0x3b2d6e), lin(0xff9a6b), lin(0x2a2346));
    sky.ico_with(Vec3::ZERO, Vec3::splat(1400.0), 4, |d| {
        let mut c = if d.y >= 0.0 {
            let t = smoothstep(0.0, 0.5, d.y).powf(0.7);
            if t < 0.35 {
                mix(horizon, mid, t / 0.35)
            } else {
                mix(mid, top, (t - 0.35) / 0.65)
            }
        } else {
            mix(horizon, below, smoothstep(0.0, -0.12, d.y))
        };
        let toward = d.dot(-sun_dir).max(0.0);
        c = mix(c, lin(0xfff0d0), toward.powf(8.0) * 0.5 + toward.powf(60.0) * 0.5);
        (1.0, c)
    });
    commands.spawn((
        Mesh3d(meshes.add(sky.build(false))),
        MeshMaterial3d(mats.sky.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        MatchEntity,
    ));
    let mut stars = MeshBuilder::new();
    for _ in 0..260 {
        let d = Vec3::new(rng.sym(1.0), rng.range(0.25, 1.0), rng.sym(1.0)).normalize();
        stars.ball(d * 1300.0, rng.range(1.5, 3.5), 0, lin(0xfff6e8));
    }
    commands.spawn((
        Mesh3d(meshes.add(stars.build(true))),
        MeshMaterial3d(mats.soft_glow.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        MatchEntity,
    ));
    // The sun itself.
    let mut sun = MeshBuilder::new();
    sun.ball(Vec3::ZERO, 60.0, 2, lin(0xfff1d6));
    commands.spawn((
        Mesh3d(meshes.add(sun.build(false))),
        MeshMaterial3d(mats.soft_glow.clone()),
        Transform::from_translation(-sun_dir * 1250.0),
        NotShadowCaster,
        MatchEntity,
    ));
    // Giant floating glyphs around the island.
    let glyphs = [
        ("{", 0xb9a5ff),
        ("}", 0xb9a5ff),
        (";", 0x7fe3d0),
        ("</>", 0xffb38a),
        ("#", 0x9fd1ff),
        ("$_", 0x7ee787),
        ("=>", 0xff9ad5),
        ("[]", 0xffe08a),
        ("*", 0xb9a5ff),
        ("()", 0x7fe3d0),
    ];
    for (k, (text, color)) in glyphs.iter().enumerate() {
        let a = k as f32 / glyphs.len() as f32 * TAU + rng.sym(0.2);
        let r = rng.range(HALF + 30.0, HALF + 90.0);
        let base = Vec3::new(a.cos() * r, rng.range(45.0, 90.0), a.sin() * r);
        let mesh = sky_glyph(text, *color);
        commands.spawn((
            Mesh3d(meshes.add(mesh.build(false))),
            MeshMaterial3d(mats.soft_glow.clone()),
            Transform::from_translation(base).with_scale(Vec3::splat(rng.range(1.2, 2.2))),
            SkyGlyph {
                spin: rng.range(0.05, 0.15) * if rng.chance(0.5) { 1.0 } else { -1.0 },
                base,
                phase: rng.angle(),
            },
            NotShadowCaster,
            NotShadowReceiver,
            MatchEntity,
        ));
    }
}

/// Keeps scenery on the ground as it moves, and destroys what blasts hit.
pub fn decor(
    mut commands: Commands,
    terrain: Res<Terrain>,
    mut fx: ResMut<Fx>,
    mut blasts: MessageReader<Blast>,
    mut seen: Local<u64>,
    mut q: Query<(Entity, &Decor, &mut Transform)>,
) {
    let hits: Vec<Blast> = blasts.read().copied().collect();
    for b in &hits {
        for (e, d, tf) in &q {
            if d.at.distance(xz(b.pos)) < b.radius + d.radius * 0.5 {
                let p = tf.translation + Vec3::Y * 1.5;
                fx.burst(Puff::Debris, p, Vec3::Y * 4.0, 10, 6.0, 1.2, 0.35);
                fx.burst(Puff::Glitch, p + Vec3::Y, Vec3::Y * 2.0, 8, 4.0, 0.8, 0.2);
                commands.entity(e).despawn();
            }
        }
    }
    if *seen == terrain.version {
        return;
    }
    *seen = terrain.version;
    for (_, d, mut tf) in &mut q {
        let y = terrain.height_v(d.at) - d.sink;
        if (tf.translation.y - y).abs() > 0.01 {
            tf.translation.y = y;
        }
    }
}

pub fn sky_glyphs(time: Res<Time>, mut q: Query<(&SkyGlyph, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (g, mut tf) in &mut q {
        tf.rotation = Quat::from_rotation_y(t * g.spin + g.phase);
        tf.translation = g.base + Vec3::Y * (t * 0.3 + g.phase).sin() * 3.0;
    }
}
