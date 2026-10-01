//! The monster hunters: their physical bodies and rigs, what they notice,
//! what they do about it, their crossbow bolts, and how they die.

use crate::{
    audio::{Sfx, Sound},
    fx::{Fx, Look},
    game::*,
    level::HunterSpawn,
    models::{Mats, Models},
    player::Stealth,
    props::{self, CarriedLantern, Grabbable, LightSource},
    util::{Rng, damp, turn_toward},
    water::{Floater, Water},
};
use avian2d::prelude::*;
use bevy::{
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};
use std::f32::consts::{FRAC_PI_2, PI};

/// Half the height of a hunter's capsule: the feet are this far below its center.
pub const HALF_H: f32 = 0.78;
const SHOULDER: f32 = 0.36;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HunterKind {
    Lantern,
    Crossbow,
    /// The leader: a big lantern, a fast crossbow, and a lot of health.
    Warden,
}

impl HunterKind {
    pub fn hp(self) -> f32 {
        match self {
            HunterKind::Warden => 7.0,
            _ => 3.0,
        }
    }
    pub fn has_lantern(self) -> bool {
        matches!(self, HunterKind::Lantern | HunterKind::Warden)
    }
    pub fn has_crossbow(self) -> bool {
        matches!(self, HunterKind::Crossbow | HunterKind::Warden)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Patrol,
    /// Heard or glimpsed something: turns, looks, walks over.
    Suspicious,
    /// Knows where the bogwight is and is trying to kill it.
    Hunt,
    /// In over their head, paddling for the shore.
    Flounder,
    /// In the bogwight's claws.
    Held,
    Dead,
}

#[derive(Component)]
pub struct Hunter {
    pub kind: HunterKind,
    pub state: State,
    pub hp: f32,
    /// 0 is calm, 1 is certain; it rises while they see or hear the bogwight.
    pub alert: f32,
    pub facing: f32,
    pub patrol: (f32, f32),
    pub goal: f32,
    pub wait: f32,
    pub guard: bool,
    pub boat: Option<Entity>,
    pub last_seen: Vec2,
    pub unseen: f32,
    pub sees: bool,
    pub cooldown: f32,
    /// Crossbow draw, 0..1.
    pub aim: f32,
    pub aim_dir: Vec2,
    /// Melee swing timer (counts down; the blow lands halfway).
    pub swing: f32,
    pub stun: f32,
    pub drown: f32,
    pub struggle: f32,
    pub grounded: bool,
    pub ground: Option<Entity>,
    pub walk: f32,
    pub speed: f32,
    pub off_boat: f32,
    pub dry: f32,
    /// How long they've been floundering; the swamp takes them eventually.
    pub swim: f32,
    pub look_timer: f32,
    pub dead_time: f32,
    pub scale: f32,
    pub rng: Rng,
}

/// Entities of a hunter's animated parts.
#[derive(Component)]
pub struct HunterRig {
    pub rig: Entity,
    pub arm_front: Entity,
    pub arm_back: Entity,
    pub leg_front: Entity,
    pub leg_back: Entity,
    pub lantern: Option<Entity>,
    pub aim_line: Option<Entity>,
    pub yaw: f32,
}

#[derive(Component)]
pub struct AimLine;

/// A hunter who should let go of their lantern (they died holding it).
#[derive(Component)]
pub struct DropLantern;

pub fn hunter_layers() -> CollisionLayers {
    CollisionLayers::new(
        Layer::Hunter,
        [Layer::Ground, Layer::Player, Layer::Prop, Layer::Boat, Layer::Hunter],
    )
}

pub fn spawn_hunter(
    commands: &mut Commands,
    models: &Models,
    mats: &Mats,
    s: &HunterSpawn,
    boat: Option<Entity>,
    id: u64,
    low: bool,
) -> Entity {
    let warden = s.kind == HunterKind::Warden;
    let look = if warden {
        &models.warden
    } else {
        &models.hunters[(id as usize * 7 + 3) % models.hunters.len()]
    };
    let k = if warden { 1.12 } else { 1.0 };
    let mut rng = Rng::new(id.wrapping_mul(0x9e37) ^ 0x4a11);
    let root = commands
        .spawn((
            Transform::from_xyz(s.pos.x, s.pos.y, 0.0),
            Visibility::default(),
            RigidBody::Dynamic,
            Collider::capsule(0.28 * k, 1.0 * k),
            LockedAxes::ROTATION_LOCKED,
            Mass(70.0 * k),
            Friction::new(0.7),
            Restitution::new(0.0),
            hunter_layers(),
            Floater {
                points: (0..6).map(|i| Vec2::new(0.0, (-0.7 + i as f32 * 0.28) * k)).collect(),
                cell: 0.14 * k,
                buoyancy: 1.1,
                drag: Vec2::new(2.2, 2.8),
                angular_drag: 3.0,
                submerged: 0.0,
                splashy: true,
            },
            Grabbable,
            LevelEntity,
        ))
        .id();
    let rig = commands
        .spawn((
            Transform::from_rotation(Quat::from_rotation_y(if s.facing > 0.0 { 0.0 } else { PI }))
                .with_scale(Vec3::splat(k)),
            Visibility::default(),
            ChildOf(root),
        ))
        .id();
    let part = |commands: &mut Commands, mesh: &Handle<Mesh>, at: Vec3, parent: Entity| {
        commands
            .spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mats.matte.clone()),
                Transform::from_translation(at),
                ChildOf(parent),
            ))
            .id()
    };
    part(commands, &look.torso, Vec3::ZERO, rig);
    part(commands, &look.head, Vec3::ZERO, rig);
    let leg_front = part(commands, &look.leg, Vec3::new(0.0, -0.08, 0.1), rig);
    let leg_back = part(commands, &look.leg, Vec3::new(0.0, -0.08, -0.1), rig);
    let arm_front = part(commands, &look.arm, Vec3::new(0.0, SHOULDER, 0.21), rig);
    let arm_back = part(commands, &look.arm, Vec3::new(0.0, SHOULDER, -0.21), rig);
    if s.kind.has_crossbow() {
        commands.spawn((
            Mesh3d(models.crossbow.clone()),
            MeshMaterial3d(mats.matte.clone()),
            Transform::from_xyz(0.0, -0.52, 0.03).with_rotation(Quat::from_rotation_z(-FRAC_PI_2)),
            ChildOf(arm_front),
        ));
    }
    let lantern = s.kind.has_lantern().then(|| {
        let mut e = commands.spawn((
            Transform::from_xyz(0.0, -0.53, 0.0),
            Visibility::default(),
            ChildOf(arm_back),
            LightSource {
                strength: if warden { 1.3 } else { 1.0 },
                radius: if warden { 10.0 } else { 8.0 },
            },
        ));
        let (light, glass) = props::lantern_parts(&mut e, models, mats, low);
        let id = e.id();
        commands.entity(id).insert(CarriedLantern {
            lit: true,
            light,
            glass,
        });
        id
    });
    let aim_line = s.kind.has_crossbow().then(|| {
        commands
            .spawn((
                Mesh3d(models.cube.clone()),
                MeshMaterial3d(mats.aim.clone()),
                Transform::from_scale(Vec3::ZERO),
                Visibility::Hidden,
                AimLine,
                NotShadowCaster,
                NotShadowReceiver,
                LevelEntity,
            ))
            .id()
    });
    let goal = if rng.chance(0.5) { s.patrol.0 } else { s.patrol.1 };
    commands.entity(root).insert((
        Hunter {
            kind: s.kind,
            state: State::Patrol,
            hp: s.kind.hp(),
            alert: 0.0,
            facing: s.facing,
            patrol: s.patrol,
            goal,
            wait: rng.range(0.0, 2.0),
            guard: s.guard,
            boat,
            last_seen: s.pos,
            unseen: 99.0,
            sees: false,
            cooldown: 0.0,
            aim: 0.0,
            aim_dir: Vec2::X,
            swing: 0.0,
            stun: 0.0,
            drown: 0.0,
            struggle: 0.0,
            grounded: false,
            ground: None,
            walk: rng.angle(),
            speed: 0.0,
            off_boat: 0.0,
            dry: 0.0,
            swim: 0.0,
            look_timer: rng.range(2.0, 6.0),
            dead_time: 0.0,
            scale: k,
            rng,
        },
        HunterRig {
            rig,
            arm_front,
            arm_back,
            leg_front,
            leg_back,
            lantern,
            aim_line,
            yaw: if s.facing > 0.0 { 0.0 } else { PI },
        },
    ));
    root
}

fn ground_filter() -> SpatialQueryFilter {
    SpatialQueryFilter::from_mask([Layer::Ground, Layer::Prop, Layer::Boat])
}

/// Is it safe to take a step in `dir`: solid ground ahead, no drop, no deep water?
fn safe_ahead(spatial: &SpatialQuery, water: &Water, pos: Vec2, dir: f32, exclude: Entity) -> bool {
    let probe = pos + Vec2::new(dir * 0.75, 0.0);
    let filter = ground_filter().with_excluded_entities([exclude]);
    match spatial.cast_ray(probe, Dir2::NEG_Y, HALF_H + 2.2, true, &filter) {
        None => false,
        Some(hit) => {
            let ground_y = probe.y - hit.distance;
            if ground_y < pos.y - HALF_H - 1.2 {
                return false;
            }
            water.surface(probe.x).map_or(0.0, |s| s - ground_y) < 1.05
        }
    }
}

/// What each hunter sees and hears this step.
pub fn perceive(
    time: Res<Time>,
    stealth: Res<Stealth>,
    spatial: SpatialQuery,
    mut noises: MessageReader<Noise>,
    mut hunters: Query<(Entity, &mut Hunter, &Position)>,
    mut sfx: MessageWriter<Sfx>,
    mut popups: MessageWriter<Popup>,
    mut session: ResMut<Session>,
) {
    let dt = time.delta_secs();
    let noises: Vec<Noise> = noises.read().copied().collect();
    let mut whistles: Vec<(Vec2, Vec2)> = vec![];
    for (_, mut h, pos) in &mut hunters {
        if matches!(h.state, State::Dead | State::Held) {
            continue;
        }
        let eye = pos.0 + Vec2::Y * 0.6 * h.scale;
        h.sees = false;
        if stealth.alive {
            let to = stealth.pos - eye;
            let d = to.length();
            let facing_ok = to.x * h.facing > 0.0 || d < 1.3;
            let mut range = 2.0 + 17.0 * stealth.visibility;
            if h.kind == HunterKind::Warden {
                range *= 1.25;
            }
            if !facing_ok {
                range *= 0.25;
            }
            if h.state == State::Flounder {
                range *= 0.4;
            }
            // On the hunt they're scanning hard.
            if h.state == State::Hunt {
                range += 3.0;
            }
            if d < range {
                let blocked = d > 0.5
                    && spatial
                        .cast_ray(
                            eye,
                            Dir2::new(to).unwrap_or(Dir2::X),
                            d,
                            true,
                            &SpatialQueryFilter::from_mask(Layer::Ground),
                        )
                        .is_some_and(|hit| hit.distance < d - 0.4);
                if !blocked {
                    h.sees = true;
                    let gain = (1.0 - d / range).max(0.2) * 2.6;
                    let boost = if h.state == State::Hunt { 2.0 } else { 1.0 };
                    h.alert += gain * boost * dt;
                    h.last_seen = stealth.pos;
                    h.unseen = 0.0;
                }
            }
        }
        if !h.sees {
            h.unseen += dt;
        }
        for n in &noises {
            let d = n.pos.distance(eye);
            if d < n.radius {
                h.alert += 0.3 * (1.0 - d / n.radius) + 0.06;
                if h.state != State::Hunt || h.unseen > 1.0 {
                    h.last_seen = n.pos;
                }
            }
        }
        if !h.sees {
            let decay = match h.state {
                State::Hunt => 0.05,
                _ => 0.1,
            };
            h.alert -= decay * dt;
        }
        h.alert = h.alert.clamp(0.0, 1.4);
        match h.state {
            State::Patrol if h.alert > 0.3 => {
                h.state = State::Suspicious;
                sfx.write(Sfx::at(Sound::Huh, pos.0));
            }
            State::Suspicious if h.alert >= 1.0 => {
                h.state = State::Hunt;
                whistles.push((pos.0, h.last_seen));
                session.stats.spotted += 1;
                sfx.write(Sfx::at(Sound::Whistle, pos.0));
                popups.write(Popup::new(pos.0 + Vec2::Y * 1.6, "!", 0xff5040, true));
            }
            State::Suspicious if h.alert < 0.08 => {
                h.state = State::Patrol;
            }
            State::Hunt if h.alert < 0.4 => {
                h.state = State::Suspicious;
            }
            _ => {}
        }
    }
    // A whistle brings everyone nearby.
    for (at, seen) in whistles {
        for (_, mut h, pos) in &mut hunters {
            if matches!(h.state, State::Dead | State::Held | State::Hunt) {
                continue;
            }
            if pos.0.distance(at) < 16.0 {
                h.alert = h.alert.max(0.8);
                h.last_seen = seen;
                h.unseen = 0.5;
                if h.state == State::Patrol {
                    h.state = State::Suspicious;
                }
            }
        }
    }
}

/// Where the nearest wadeable shore is from `x`, as a direction.
fn shore_dir(water: &Water, x: f32) -> f32 {
    for step in 1..60 {
        let d = step as f32 * 0.8;
        for dir in [1.0, -1.0] {
            if water.surface(x + dir * d).is_none() {
                return dir;
            }
        }
    }
    1.0
}

/// Movement and attacks.
pub fn act(
    mut commands: Commands,
    time: Res<Time>,
    stealth: Res<Stealth>,
    water: Res<Water>,
    spatial: SpatialQuery,
    models: Res<Models>,
    mats: Res<Mats>,
    mut hunters: Query<(Entity, &mut Hunter, &Position, &mut LinearVelocity)>,
    others: Query<&LinearVelocity, Without<Hunter>>,
    mut hurt: MessageWriter<HurtPlayer>,
    mut hits: MessageWriter<HitHunter>,
    mut sfx: MessageWriter<Sfx>,
) {
    let dt = time.delta_secs();
    for (e, mut h, pos, mut vel) in &mut hunters {
        h.cooldown -= dt;
        h.stun -= dt;
        h.look_timer -= dt;
        if matches!(h.state, State::Dead) {
            h.dead_time += dt;
            continue;
        }
        // Lost to the swamp: fallen out of the world, or floundered too long.
        if h.state == State::Flounder {
            h.swim += dt;
        } else {
            h.swim = 0.0;
        }
        if pos.0.y < -25.0 || h.swim > 30.0 {
            hits.write(HitHunter {
                entity: e,
                damage: 99.0,
                kind: HitKind::Drown,
                knock: Vec2::ZERO,
            });
            continue;
        }
        // Footing.
        let filter = ground_filter().with_excluded_entities([e]);
        let hit = spatial.cast_ray(pos.0, Dir2::NEG_Y, HALF_H * h.scale + 0.18, true, &filter);
        h.grounded = hit.is_some();
        h.ground = hit.map(|hit| hit.entity);
        let ground_vel = h.ground.and_then(|g| others.get(g).ok()).map_or(Vec2::ZERO, |v| v.0);
        if let Some(boat) = h.boat {
            if h.ground == Some(boat) {
                h.off_boat = 0.0;
            } else {
                h.off_boat += dt;
                if h.off_boat > 1.2 {
                    h.boat = None;
                }
            }
        }
        let on_boat = h.boat.is_some() && h.ground == h.boat;
        // In over their head?
        let shoulders = pos.0 + Vec2::Y * 0.45 * h.scale;
        let feet = pos.0 - Vec2::Y * HALF_H * h.scale;
        let deep = water.depth(shoulders) > 0.05 && !(h.grounded && water.depth(feet) < 1.2);
        if h.state == State::Held {
            continue;
        }
        if deep && h.state != State::Flounder {
            h.state = State::Flounder;
            h.aim = 0.0;
            h.swing = 0.0;
        }
        if h.state == State::Flounder {
            if !deep {
                h.dry += dt;
                if h.dry > 0.5 {
                    h.state = State::Hunt;
                    h.alert = 1.0;
                    h.dry = 0.0;
                }
            } else {
                h.dry = 0.0;
            }
            let dir = shore_dir(&water, pos.0.x);
            h.facing = dir;
            vel.x = damp(vel.x, dir * 1.4, 2.0, dt);
            if h.rng.chance(dt * 2.0) {
                vel.y += h.rng.range(0.5, 1.5);
            }
            h.speed = 1.0;
            continue;
        }
        if h.stun > 0.0 {
            continue;
        }
        let to_player = stealth.pos - pos.0;
        let d = to_player.length();
        let mut target = 0.0f32;
        let mut face = h.facing;
        match h.state {
            State::Patrol => {
                if h.guard || on_boat {
                    if h.look_timer <= 0.0 {
                        // Glance behind, then back.
                        face = -h.facing;
                        h.look_timer = h.rng.range(2.5, 6.0);
                    }
                } else if h.wait > 0.0 {
                    h.wait -= dt;
                    if h.look_timer <= 0.0 {
                        face = -h.facing;
                        h.look_timer = h.rng.range(1.5, 3.0);
                    }
                } else {
                    let dx = h.goal - pos.0.x;
                    if dx.abs() < 0.3 {
                        h.wait = h.rng.range(1.5, 4.5);
                        h.goal = if (h.goal - h.patrol.0).abs() < 0.5 {
                            h.patrol.1
                        } else {
                            h.patrol.0
                        };
                    } else {
                        face = dx.signum();
                        target = face * 1.1;
                    }
                }
            }
            State::Suspicious => {
                let dx = h.last_seen.x - pos.0.x;
                if dx.abs() > 0.3 {
                    face = dx.signum();
                }
                if !h.guard && !on_boat && dx.abs() > 1.5 {
                    target = face * 0.9;
                }
            }
            State::Hunt => {
                let knows = if h.unseen < 0.5 { stealth.pos } else { h.last_seen };
                let dx = knows.x - pos.0.x;
                if dx.abs() > 0.2 {
                    face = dx.signum();
                }
                let dist = knows.distance(pos.0);
                let melee = h.kind == HunterKind::Lantern || (h.kind == HunterKind::Warden && dist < 2.0);
                if !on_boat {
                    if melee {
                        if dist > 1.0 {
                            target = face * 2.3;
                        }
                    } else if dist < 4.5 {
                        target = -face * 1.6;
                    } else if dist > 12.0 {
                        target = face * 1.8;
                    }
                }
                // Attacks.
                if stealth.alive && melee && d < 1.4 && h.cooldown <= 0.0 && h.swing <= 0.0 {
                    h.swing = 0.5;
                    h.cooldown = 1.1;
                    sfx.write(Sfx::at(Sound::Swing, pos.0));
                }
                if h.kind.has_crossbow() && !melee && stealth.alive {
                    let can = h.unseen < 0.4 && (2.0..17.0).contains(&d);
                    if can && h.cooldown <= 0.0 {
                        let draw = if h.kind == HunterKind::Warden { 0.6 } else { 0.95 };
                        if h.aim == 0.0 {
                            sfx.write(Sfx::at(Sound::Creak, pos.0));
                        }
                        h.aim = (h.aim + dt / draw).min(1.0);
                        if h.aim >= 1.0 {
                            let from = pos.0 + Vec2::new(face * 0.6, SHOULDER * h.scale);
                            let dir = aim_at(from, stealth.pos + stealth.vel * (d / 26.0) * 0.5, &mut h.rng);
                            spawn_bolt(&mut commands, &models, &mats, from, dir * 26.0);
                            sfx.write(Sfx::at(Sound::Twang, pos.0));
                            h.aim = 0.0;
                            h.cooldown = if h.kind == HunterKind::Warden { 1.3 } else { 2.1 };
                        }
                    } else if !can {
                        h.aim = (h.aim - dt * 1.5).max(0.0);
                    }
                    let from = pos.0 + Vec2::new(face * 0.6, SHOULDER * h.scale);
                    h.aim_dir = (stealth.pos - from).normalize_or(Vec2::X * face);
                }
            }
            _ => {}
        }
        if h.swing > 0.0 {
            let before = h.swing;
            h.swing -= dt;
            if before > 0.25 && h.swing <= 0.25 && stealth.alive {
                let dx = stealth.pos.x - pos.0.x;
                if d < 1.75 && dx * h.facing > -0.3 {
                    let amount = if h.kind == HunterKind::Warden { 18.0 } else { 12.0 };
                    hurt.write(HurtPlayer {
                        amount,
                        from: pos.0,
                        knock: 7.0,
                    });
                }
            }
            target = 0.0;
        }
        // Don't walk into deep water or off a ledge.
        if target != 0.0 && !safe_ahead(&spatial, &water, pos.0, target.signum(), e) {
            target = 0.0;
        }
        h.facing = face;
        // Wading is slow.
        if water.depth(feet) > 0.3 {
            target *= 0.6;
        }
        if h.grounded && !on_boat {
            let rel = vel.x - ground_vel.x;
            vel.x = ground_vel.x + damp(rel, target, 10.0, dt);
        }
        h.speed = (vel.x - ground_vel.x).abs();
    }
}

/// A launch direction for a bolt (speed 26, reduced gravity) toward `to`,
/// on the low arc, with a little spread.
fn aim_at(from: Vec2, to: Vec2, rng: &mut Rng) -> Vec2 {
    let g = GRAVITY * BOLT_GRAVITY;
    let v = 26.0f32;
    let d = to - from;
    let dx = d.x.abs().max(0.1);
    let dy = d.y;
    let disc = v.powi(4) - g * (g * dx * dx + 2.0 * dy * v * v);
    let angle = if disc >= 0.0 {
        ((v * v - disc.sqrt()) / (g * dx)).atan()
    } else {
        dy.atan2(dx)
    };
    let angle = angle + rng.sym(0.035);
    Vec2::new(angle.cos() * d.x.signum(), angle.sin())
}

pub const BOLT_GRAVITY: f32 = 0.35;

#[derive(Component)]
pub struct Bolt {
    pub life: f32,
    pub stuck: bool,
}

pub fn spawn_bolt(commands: &mut Commands, models: &Models, mats: &Mats, from: Vec2, vel: Vec2) {
    commands.spawn((
        Mesh3d(models.bolt.clone()),
        MeshMaterial3d(mats.matte.clone()),
        Transform::from_xyz(from.x, from.y, 0.0).with_rotation(Quat::from_rotation_z(vel.to_angle())),
        RigidBody::Dynamic,
        Collider::rectangle(0.5, 0.05),
        Mass(0.15),
        GravityScale(BOLT_GRAVITY),
        LinearVelocity(vel),
        LockedAxes::ROTATION_LOCKED,
        SweptCcd::default(),
        CollisionEventsEnabled,
        CollisionLayers::new(Layer::Bolt, [Layer::Ground, Layer::Player, Layer::Prop, Layer::Boat]),
        Bolt {
            life: 10.0,
            stuck: false,
        },
        NotShadowCaster,
        LevelEntity,
    ));
}

pub fn bolts(
    mut commands: Commands,
    time: Res<Time>,
    water: Res<Water>,
    mut q: Query<(Entity, &mut Bolt, &mut LinearVelocity, &mut Rotation, &Position)>,
) {
    let dt = time.delta_secs();
    for (e, mut b, mut v, mut rot, pos) in &mut q {
        b.life -= dt;
        if b.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        if water.depth(pos.0) > 0.0 {
            v.0 *= (-5.0 * dt).exp();
        }
        if v.length_squared() > 1.0 {
            *rot = Rotation::radians(v.to_angle());
        }
    }
}

/// Bolts that hit the bogwight hurt it; bolts that hit anything else stick.
pub fn bolt_hits(
    mut commands: Commands,
    mut events: MessageReader<CollisionStart>,
    mut bolts: Query<(&mut Bolt, &LinearVelocity, &GlobalTransform)>,
    player: Query<(), With<crate::player::Bogwight>>,
    bodies: Query<(&GlobalTransform, &RigidBody), Without<Bolt>>,
    mut hurt: MessageWriter<HurtPlayer>,
    mut sfx: MessageWriter<Sfx>,
    mut fx: ResMut<Fx>,
) {
    for ev in events.read() {
        for (a, b, body_b) in [
            (ev.collider1, ev.collider2, ev.body2),
            (ev.collider2, ev.collider1, ev.body1),
        ] {
            let Ok((mut bolt, v, gt)) = bolts.get_mut(a) else {
                continue;
            };
            if bolt.stuck {
                continue;
            }
            let pos = gt.translation().truncate();
            if player.get(b).is_ok() {
                if v.length() > 5.0 {
                    hurt.write(HurtPlayer {
                        amount: 16.0,
                        from: pos - v.0.normalize_or_zero(),
                        knock: 3.0,
                    });
                    fx.burst(Look::Blood, pos.extend(0.2), 6, 2.0, 0.06, 0.8);
                }
                commands.entity(a).despawn();
                bolt.stuck = true;
                continue;
            }
            bolt.stuck = true;
            bolt.life = 8.0;
            sfx.write(Sfx::at(Sound::Thunk, pos));
            commands
                .entity(a)
                .remove::<(RigidBody, Collider, SweptCcd, CollisionEventsEnabled, LinearVelocity)>();
            // Stick into moving things so the bolt rides along.
            let target = body_b.unwrap_or(b);
            if let Ok((tgt, rb)) = bodies.get(target)
                && *rb == RigidBody::Dynamic
            {
                let rel = tgt.affine().inverse() * gt.affine();
                commands
                    .entity(a)
                    .insert((ChildOf(target), Transform::from_matrix(rel.into())));
            }
        }
    }
}

/// Damage, knockback and death.
pub fn apply_hits(
    mut commands: Commands,
    mut hits: MessageReader<HitHunter>,
    mut hunters: Query<(&mut Hunter, &Position, &mut LinearVelocity, &mut AngularVelocity)>,
    mut session: ResMut<Session>,
    mut sfx: MessageWriter<Sfx>,
    mut popups: MessageWriter<Popup>,
    mut noises: MessageWriter<Noise>,
    mut fx: ResMut<Fx>,
) {
    for hit in hits.read() {
        let Ok((mut h, pos, mut vel, mut ang)) = hunters.get_mut(hit.entity) else {
            continue;
        };
        if h.state == State::Dead {
            continue;
        }
        let fatal = match hit.kind {
            HitKind::Ambush | HitKind::Drown => true,
            _ => {
                h.hp -= hit.damage;
                h.hp <= 0.0
            }
        };
        vel.0 += hit.knock;
        if hit.kind != HitKind::Drown {
            fx.burst(Look::Blood, pos.0.extend(0.3), 10, 3.0, 0.06, 0.9);
            sfx.write(Sfx::at(Sound::Hit, pos.0));
        }
        if !fatal {
            h.stun = 0.35;
            h.alert = 1.2;
            h.last_seen = pos.0 - hit.knock.normalize_or_zero();
            if h.state != State::Held && h.state != State::Flounder {
                h.state = State::Hunt;
            }
            continue;
        }
        h.state = State::Dead;
        h.hp = 0.0;
        h.aim = 0.0;
        ang.0 = h.rng.sym(4.0) + hit.knock.x.signum() * -2.0;
        commands
            .entity(hit.entity)
            .insert((LockedAxes::new(), Friction::new(0.9), DropLantern));
        session.hunters_left = session.hunters_left.saturating_sub(1);
        let (text, color) = match hit.kind {
            HitKind::Ambush => {
                session.stats.ambushes += 1;
                ("AMBUSH", 0xc8ff4a)
            }
            HitKind::Drown => {
                session.stats.drowned += 1;
                ("DROWNED", 0x5ad8ff)
            }
            HitKind::Impact => {
                session.stats.thrown += 1;
                ("SPLAT", 0xffb040)
            }
            HitKind::Claw => {
                session.stats.clawed += 1;
                ("SLAIN", 0xff7050)
            }
        };
        popups.write(Popup::new(pos.0 + Vec2::Y * 1.2, text, color, true));
        if hit.kind == HitKind::Drown {
            sfx.write(Sfx::at(Sound::Gurgle, pos.0));
            noises.write(Noise {
                pos: pos.0,
                radius: 2.5,
            });
        } else {
            sfx.write(Sfx::at(Sound::Scream, pos.0));
            noises.write(Noise {
                pos: pos.0,
                radius: if hit.kind == HitKind::Ambush { 7.0 } else { 11.0 },
            });
        }
    }
}

/// Dead hunters let go of their lanterns, which become loose bodies.
pub fn drop_lanterns(
    mut commands: Commands,
    models: Res<Models>,
    mats: Res<Mats>,
    args: Res<crate::Args>,
    mut q: Query<(Entity, &mut HunterRig, &LinearVelocity), With<DropLantern>>,
    carried: Query<(&CarriedLantern, &GlobalTransform)>,
) {
    for (e, mut rig, vel) in &mut q {
        commands.entity(e).remove::<DropLantern>();
        let Some(l) = rig.lantern.take() else { continue };
        let Ok((c, gt)) = carried.get(l) else { continue };
        let p = gt.translation().truncate();
        let body = props::spawn_lantern_body(&mut commands, &models, &mats, p, c.lit, args.low);
        commands
            .entity(body)
            .insert(LinearVelocity(vel.0 + Vec2::new(0.0, 1.5)));
        commands.entity(l).despawn();
    }
}

/// Poses the rig: facing, walking, aiming, swinging, flailing.
pub fn animate(
    time: Res<Time>,
    stealth: Res<Stealth>,
    mut hunters: Query<(&mut Hunter, &mut HunterRig, &GlobalTransform)>,
    mut parts: Query<&mut Transform, Without<AimLine>>,
    mut lines: Query<(&mut Transform, &mut Visibility), With<AimLine>>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    for (mut h, mut rig, gt) in &mut hunters {
        let want = if h.facing > 0.0 { 0.0 } else { PI };
        rig.yaw = turn_toward(rig.yaw, want, dt * 9.0);
        if h.state != State::Dead {
            let speed = h.speed;
            h.walk += dt * speed * 4.5;
        }
        let walk = h.walk;
        let stride = (h.speed / 1.2).min(1.0);
        let (mut lf, mut lb) = (walk.sin() * 0.55 * stride, -walk.sin() * 0.55 * stride);
        let (mut af, mut ab);
        match h.state {
            State::Flounder | State::Held => {
                let k = t * 13.0 + h.walk;
                af = 2.6 + k.sin() * 0.6;
                ab = 2.4 + (k + 1.3).sin() * 0.6;
                lf = (k * 1.3).sin() * 0.8;
                lb = -(k * 1.3).sin() * 0.8;
            }
            State::Dead => {
                af = 0.4;
                ab = -0.3;
                lf = 0.2;
                lb = -0.1;
            }
            _ => {
                ab = -walk.sin() * 0.35 * stride;
                af = walk.sin() * 0.35 * stride;
                if h.kind.has_lantern() {
                    ab = if h.state == State::Hunt { 1.9 } else { 1.25 } + (t * 1.7).sin() * 0.05;
                }
                if h.kind.has_crossbow() {
                    af = if h.state == State::Hunt {
                        // Point along the aim, in the rig's facing frame.
                        let d = Vec2::new(h.aim_dir.x * h.facing, h.aim_dir.y);
                        d.y.atan2(d.x.max(0.05)) + FRAC_PI_2
                    } else {
                        0.75
                    };
                }
                if h.swing > 0.0 {
                    let k = 1.0 - h.swing / 0.5;
                    af = if k < 0.5 {
                        2.8 * (k / 0.5)
                    } else {
                        2.8 - 3.6 * ((k - 0.5) / 0.5)
                    };
                }
            }
        }
        let set = |parts: &mut Query<&mut Transform, Without<AimLine>>, e: Entity, a: f32| {
            if let Ok(mut tf) = parts.get_mut(e) {
                tf.rotation = Quat::from_rotation_z(a);
            }
        };
        set(&mut parts, rig.leg_front, lf);
        set(&mut parts, rig.leg_back, lb);
        set(&mut parts, rig.arm_front, af);
        set(&mut parts, rig.arm_back, ab);
        if let Some(l) = rig.lantern {
            // Hang straight down whatever the arm does (and swing a little).
            set(&mut parts, l, -ab + (t * 2.0 + walk).sin() * 0.08 * stride);
        }
        if let Ok(mut tf) = parts.get_mut(rig.rig) {
            tf.rotation = Quat::from_rotation_y(rig.yaw);
        }
        if let Some(line) = rig.aim_line
            && let Ok((mut tf, mut vis)) = lines.get_mut(line)
        {
            if h.aim > 0.05 && h.state == State::Hunt && stealth.alive {
                let from = gt.translation().truncate() + Vec2::new(h.facing * 0.6, SHOULDER * h.scale);
                let to = stealth.pos;
                let d = to - from;
                let len = d.length();
                tf.translation = ((from + to) * 0.5).extend(0.3);
                tf.rotation = Quat::from_rotation_z(d.to_angle());
                let w = 0.012 + 0.03 * h.aim * h.aim;
                tf.scale = Vec3::new(len, w, w);
                *vis = Visibility::Visible;
            } else {
                *vis = Visibility::Hidden;
            }
        }
    }
}
