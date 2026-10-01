//! Item boxes and the things that come out of them.

use crate::{
    audio::{Sfx, Sound},
    course::{Course, Mats},
    fx::{Fx, Puff},
    game::Roster,
    input::rumble,
    kart::Kart,
    meshkit::MeshBuilder,
    race::{Phase, Race, RaceEntity},
    track::Edge,
    util::{Rng, damp, lin, wrap_angle},
};
use bevy::{input::gamepad::GamepadRumbleRequest, light::NotShadowCaster, prelude::*};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Item {
    /// One burst of speed.
    Berry,
    /// Three bursts.
    Berries,
    /// Fired straight ahead; bounces off walls.
    Seed,
    /// Chases the racer in front.
    Hornet,
    /// Dropped behind; spins out whoever runs over it.
    Burr,
    /// A while of invincible speed.
    Pollen,
    /// The beast bucks, throwing everyone ahead of you.
    Horn,
}

impl Item {
    pub const ALL: [Item; 7] = [
        Item::Berry,
        Item::Berries,
        Item::Seed,
        Item::Hornet,
        Item::Burr,
        Item::Pollen,
        Item::Horn,
    ];

    pub fn index(self) -> usize {
        Item::ALL.iter().position(|&i| i == self).unwrap()
    }

    pub fn name(self) -> &'static str {
        match self {
            Item::Berry => "DASH BERRY",
            Item::Berries => "BERRY BUNCH",
            Item::Seed => "SPIT SEED",
            Item::Hornet => "HORNET",
            Item::Burr => "BURR",
            Item::Pollen => "GOLDEN POLLEN",
            Item::Horn => "BEAST HORN",
        }
    }

    /// Picks an item for a racer in `place` of `total`. The further back, the better the odds.
    fn roll(place: usize, total: usize, rng: &mut Rng) -> Item {
        if total <= 1 {
            return Item::Berry;
        }
        let f = (place - 1) as f32 / (total - 1) as f32;
        // Weights in `Item::ALL` order.
        let weights: [f32; 7] = if f < 0.2 {
            [25.0, 3.0, 30.0, 4.0, 38.0, 0.0, 0.0]
        } else if f < 0.65 {
            [24.0, 14.0, 20.0, 20.0, 10.0, 6.0, 6.0]
        } else {
            [12.0, 28.0, 6.0, 18.0, 0.0, 20.0, 16.0]
        };
        let mut x = rng.f() * weights.iter().sum::<f32>();
        for (i, w) in weights.iter().enumerate() {
            if x < *w {
                return Item::ALL[i];
            }
            x -= w;
        }
        Item::Berry
    }
}

#[derive(Component)]
pub struct ItemBox {
    pub u: f32,
    pub d: f32,
    cooldown: f32,
    seed: f32,
}

#[derive(Component)]
pub struct Shot {
    kind: Item,
    u: f32,
    d: f32,
    yaw: f32,
    speed: f32,
    owner: usize,
    target: Option<usize>,
    life: f32,
    bounces: u8,
    age: f32,
}

#[derive(Component)]
pub struct Trap {
    u: f32,
    d: f32,
    owner: usize,
    age: f32,
}

#[derive(Resource)]
pub struct ItemAssets {
    crate_mesh: Handle<Mesh>,
    seed: Handle<Mesh>,
    hornet: Handle<Mesh>,
    burr: Handle<Mesh>,
}

pub fn setup_assets(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    // Item box: a bright faceted pod.
    let mut pod = MeshBuilder::new();
    let hues = [0xff5d8f, 0xffc20e, 0x2dc653, 0x3a86ff, 0x9d4edd, 0xfb7a00];
    let mut i = 0;
    pod.ico_with(Vec3::ZERO, Vec3::splat(1.25), 0, |_| {
        i += 1;
        (1.0, lin(hues[i % hues.len()]))
    });
    pod.ball(Vec3::ZERO, 0.62, 0, lin(0xffffff));

    let mut seed = MeshBuilder::new();
    seed.ico(Vec3::ZERO, Vec3::new(0.55, 0.55, 0.85), 1, lin(0x7ddc4a));
    seed.cone(
        Vec3::new(0.0, 0.0, -0.7),
        Vec3::new(0.0, 0.0, -1.5),
        0.3,
        6,
        lin(0xd7ff8a),
    );

    let mut hornet = MeshBuilder::new();
    hornet.ico_with(Vec3::ZERO, Vec3::new(0.55, 0.55, 1.1), 2, |d| {
        (
            1.0,
            if ((d.z + 1.0) * 3.0) as i32 % 2 == 0 {
                lin(0xffc20e)
            } else {
                lin(0x1c1c24)
            },
        )
    });
    hornet.cone(
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(0.0, -0.1, 1.8),
        0.16,
        5,
        lin(0x1c1c24),
    );
    hornet.ball(Vec3::new(0.0, 0.1, -1.0), 0.42, 1, lin(0x1c1c24));
    for side in [-1.0f32, 1.0] {
        hornet.tri_2(
            [
                Vec3::new(0.0, 0.4, -0.2),
                Vec3::new(side * 1.7, 0.9, 0.1),
                Vec3::new(side * 1.2, 0.8, 0.9),
            ],
            lin(0xdff6ff),
        );
        hornet.ball(Vec3::new(side * 0.22, 0.25, -1.3), 0.13, 0, lin(0xff4040));
    }

    let mut burr = MeshBuilder::new();
    burr.ball(Vec3::new(0.0, 0.7, 0.0), 0.62, 1, lin(0x8a5a2b));
    for k in 0..14 {
        let a = k as f32 * 2.399;
        let y = 1.0 - (k as f32 + 0.5) / 14.0 * 1.6;
        let r = (1.0 - y * y).max(0.0).sqrt();
        let dir = Vec3::new(a.cos() * r, y, a.sin() * r);
        burr.cone(
            Vec3::new(0.0, 0.7, 0.0) + dir * 0.5,
            Vec3::new(0.0, 0.7, 0.0) + dir * 1.2,
            0.17,
            4,
            lin(0xc98f4a),
        );
    }

    commands.insert_resource(ItemAssets {
        crate_mesh: meshes.add(pod.build(true, false)),
        seed: meshes.add(seed.build(true, false)),
        hornet: meshes.add(hornet.build(true, false)),
        burr: meshes.add(burr.build(true, false)),
    });
}

pub fn spawn_boxes(commands: &mut Commands, assets: &ItemAssets, mats: &Mats, course: &Course) {
    for (row, (u, ds)) in course.track.item_rows.iter().enumerate() {
        for (k, &d) in ds.iter().enumerate() {
            commands.spawn((
                Mesh3d(assets.crate_mesh.clone()),
                MeshMaterial3d(mats.ghost.clone()),
                Transform::default(),
                ItemBox {
                    u: *u,
                    d,
                    cooldown: 0.0,
                    seed: row as f32 * 1.3 + k as f32 * 0.7,
                },
                NotShadowCaster,
                RaceEntity,
            ));
        }
    }
}

/// Item boxes: spin, get collected, come back.
pub fn boxes(
    time: Res<Time>,
    course: Res<Course>,
    mut fx: ResMut<Fx>,
    mut sfx: MessageWriter<Sfx>,
    mut boxes: Query<(&mut ItemBox, &mut Transform, &mut Visibility)>,
    mut karts: Query<&mut Kart>,
) {
    let (t, dt) = (time.elapsed_secs(), time.delta_secs());
    let track = &course.track;
    for (mut b, mut tf, mut vis) in &mut boxes {
        let loc = track.at(b.u);
        tf.translation = loc.at(b.d) + loc.n * (1.9 + 0.25 * (t * 2.2 + b.seed).sin());
        tf.rotation = Quat::from_rotation_y(t * 1.6 + b.seed) * Quat::from_rotation_x(t * 1.1);
        if b.cooldown > 0.0 {
            b.cooldown -= dt;
            *vis = Visibility::Hidden;
            continue;
        }
        *vis = Visibility::Inherited;
        for mut k in &mut karts {
            if k.respawn > 0.0 || k.fallen || k.h > 3.0 {
                continue;
            }
            let ds = track.delta(k.u, b.u) * loc.len;
            if ds.abs() < 2.2 && (k.d - b.d).abs() < 2.0 {
                b.cooldown = 2.5;
                fx.burst(Puff::Star, tf.translation, Vec3::Y * 3.0, 10, 7.0, 0.45, 0.4);
                if k.item.is_none() && k.roulette <= 0.0 {
                    k.roulette = if k.is_human() { 1.5 } else { 0.4 };
                    if k.is_human() {
                        sfx.write(Sfx(Sound::ItemBox));
                    }
                }
                break;
            }
        }
    }
}

/// Roulette, item use, projectiles and traps.
pub fn items(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<ItemAssets>,
    mats: Res<Mats>,
    roster: Res<Roster>,
    mut course: ResMut<Course>,
    mut race: ResMut<Race>,
    mut fx: ResMut<Fx>,
    mut sfx: MessageWriter<Sfx>,
    mut rumbles: MessageWriter<GamepadRumbleRequest>,
    mut karts: Query<&mut Kart>,
    mut shots: Query<(Entity, &mut Shot, &mut Transform), Without<Trap>>,
    mut traps: Query<(Entity, &mut Trap, &mut Transform), Without<Shot>>,
) {
    let (t, dt) = (time.elapsed_secs(), time.delta_secs());
    let racing = matches!(race.phase, Phase::Racing | Phase::Finished);
    let mut all: Vec<Mut<Kart>> = karts.iter_mut().collect();
    let total = all.len();
    let progress: Vec<f32> = all.iter().map(|k| k.progress(&course.track)).collect();

    let mut horn: Option<usize> = None;
    for (i, k) in all.iter_mut().enumerate() {
        // Roulette.
        if k.roulette > 0.0 {
            k.roulette -= dt;
            if k.roulette <= 0.0 {
                let item = Item::roll(k.place, total, &mut race.rng);
                k.item = Some(item);
                k.item_uses = if item == Item::Berries { 3 } else { 1 };
                if k.is_human() {
                    sfx.write(Sfx(Sound::ItemGet));
                }
            }
            continue;
        }
        let Some(item) = k.item else { continue };
        if !k.input.item || !racing || k.spin > 0.0 || k.respawn > 0.0 || k.fallen || k.finished.is_some() {
            continue;
        }
        debug!("{} uses {}", k.name, item.name());
        k.item_uses = k.item_uses.saturating_sub(1);
        if k.item_uses == 0 {
            k.item = None;
        }
        let loc = course.track.at(k.u);
        let human = k.is_human();
        let mut sound = Sound::Use;
        match item {
            Item::Berry | Item::Berries => {
                k.give_boost(1.6);
                sound = Sound::Boost;
            }
            Item::Pollen => {
                k.star = 7.5;
                k.spin = 0.0;
                sound = Sound::Star;
            }
            Item::Burr => {
                commands.spawn((
                    Mesh3d(assets.burr.clone()),
                    MeshMaterial3d(mats.matte.clone()),
                    Transform::from_translation(k.pos),
                    Trap {
                        u: course.track.wrap(k.u - 4.0 / loc.len),
                        d: k.d,
                        owner: k.index,
                        age: 0.0,
                    },
                    RaceEntity,
                ));
            }
            Item::Seed | Item::Hornet => {
                // The hornet goes for whoever is directly ahead in the standings.
                let target = (item == Item::Hornet && k.place > 1).then_some(k.place - 1);
                commands.spawn((
                    Mesh3d(if item == Item::Seed {
                        assets.seed.clone()
                    } else {
                        assets.hornet.clone()
                    }),
                    MeshMaterial3d(mats.gloss.clone()),
                    Transform::from_translation(k.pos).with_scale(Vec3::splat(1.3)),
                    Shot {
                        kind: item,
                        u: course.track.wrap(k.u + 3.8 / loc.len),
                        d: k.d,
                        yaw: if item == Item::Seed { k.yaw } else { 0.0 },
                        speed: if item == Item::Seed {
                            k.speed.max(0.0) + 34.0
                        } else {
                            52.0
                        },
                        owner: k.index,
                        target,
                        life: if item == Item::Seed { 9.0 } else { 14.0 },
                        bounces: 0,
                        age: 0.0,
                    },
                    RaceEntity,
                ));
            }
            Item::Horn => {
                horn = Some(i);
                sound = Sound::Horn;
            }
        }
        if human || item == Item::Horn {
            sfx.write(Sfx(sound));
        }
    }

    // The beast horn: the beast bucks, and everyone ahead of the blower is thrown.
    if let Some(i) = horn {
        course.ctl.shudder = 1.0;
        fx.shake = 1.0;
        race.banner = Some((format!("{} SOUNDS THE BEAST HORN!", all[i].name), 3.0));
        for j in 0..total {
            if j == i || progress[j] < progress[i] || all[j].finished.is_some() {
                continue;
            }
            let k = &mut all[j];
            if k.hit(true) {
                k.speed *= 0.55;
                fx.burst(Puff::Hit, k.pos + Vec3::Y, Vec3::Y * 5.0, 10, 8.0, 0.5, 0.45);
            }
        }
        for k in all.iter() {
            if let Some(device) = k.device(&roster) {
                rumble(&mut rumbles, device, 1.0, 0.6, 0.7);
            }
        }
    }

    // Projectiles.
    for (entity, mut s, mut tf) in &mut shots {
        let track = &course.track;
        let loc = track.at(s.u);
        s.age += dt;
        s.life -= dt;
        let mut gone = s.life <= 0.0;
        if s.kind == Item::Hornet {
            // Home in across the road once the target is near; otherwise hold the middle.
            let aim = s
                .target
                .and_then(|place| all.iter().find(|k| k.place == place && k.finished.is_none()));
            let want = match aim {
                Some(k) if (track.delta(s.u, k.u) * loc.len).abs() < 70.0 => k.d,
                _ => 0.0,
            };
            s.d = damp(s.d, want, 4.0, dt);
            if let Some(k) = aim {
                s.speed = (k.speed + 20.0).max(46.0);
            }
        }
        let scale = (1.0 - s.d * loc.k).clamp(0.4, 2.5);
        let ds = s.speed * s.yaw.cos() * dt;
        s.u = track.wrap(s.u + ds / (loc.len * scale));
        s.d += s.speed * s.yaw.sin() * dt;
        s.yaw = wrap_angle(s.yaw - loc.k * ds / scale);
        let loc = track.at(s.u);
        let limit = loc.limit(s.d);
        if s.d.abs() > limit && !loc.gap {
            if loc.edge_at(s.d) == Edge::Fall || s.bounces >= 5 {
                gone = true;
            } else {
                s.d = s.d.signum() * limit;
                s.yaw = -s.yaw;
                s.bounces += 1;
            }
        }
        let pos = loc.at(s.d) + loc.n * 1.3;
        tf.translation = pos;
        tf.rotation = loc.rotation(s.yaw) * Quat::from_rotation_z(t * if s.kind == Item::Seed { 9.0 } else { 0.0 });
        if s.kind == Item::Hornet {
            tf.translation += loc.n * (0.5 + 0.3 * (t * 25.0).sin());
        }
        for k in all.iter_mut() {
            if (k.index == s.owner && s.age < 0.5) || k.respawn > 0.0 || k.fallen || k.h > 3.5 {
                continue;
            }
            let ds = track.delta(s.u, k.u) * loc.len;
            if ds.abs() < 2.4 && (k.d - s.d).abs() < 2.1 {
                gone = true;
                if k.hit(s.kind == Item::Hornet) {
                    fx.burst(Puff::Hit, k.pos + Vec3::Y, Vec3::Y * 5.0, 14, 8.0, 0.5, 0.45);
                    if k.is_human() {
                        sfx.write(Sfx(Sound::Hit));
                        if let Some(device) = k.device(&roster) {
                            rumble(&mut rumbles, device, 0.9, 0.6, 0.4);
                        }
                    }
                }
                break;
            }
        }
        if gone {
            fx.burst(Puff::Leaf, pos, Vec3::Y * 2.0, 8, 6.0, 0.4, 0.4);
            commands.entity(entity).despawn();
        }
    }

    // Traps.
    for (entity, mut trap, mut tf) in &mut traps {
        let track = &course.track;
        let loc = track.at(trap.u);
        trap.age += dt;
        tf.translation = loc.at(trap.d);
        tf.rotation = loc.rotation(0.0) * Quat::from_rotation_y(trap.age * 0.8);
        tf.scale = Vec3::splat((trap.age * 5.0).min(1.0) * 1.2);
        for k in all.iter_mut() {
            if (k.index == trap.owner && trap.age < 0.8) || !k.grounded || k.respawn > 0.0 {
                continue;
            }
            let ds = track.delta(trap.u, k.u) * loc.len;
            if ds.abs() < 2.1 && (k.d - trap.d).abs() < 2.0 {
                if k.hit(false) && k.is_human() {
                    sfx.write(Sfx(Sound::Hit));
                    if let Some(device) = k.device(&roster) {
                        rumble(&mut rumbles, device, 0.8, 0.5, 0.35);
                    }
                }
                fx.burst(Puff::Leaf, k.pos + Vec3::Y * 0.5, Vec3::Y * 4.0, 12, 7.0, 0.5, 0.45);
                commands.entity(entity).despawn();
                break;
            }
        }
    }
}
