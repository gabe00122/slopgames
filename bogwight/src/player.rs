//! The bogwight: a dynamic body that swims like an eel and walks like a
//! drunk toad. Fluid, fast and nearly invisible under water; slow, heavy and
//! drying out on land. It claws, grabs, drowns and throws hunters, and can
//! hang off a boat's gunwale to roll it over.

use crate::{
    audio::{Sfx, Sound},
    boats::Boat,
    fx::{Fx, Look},
    game::*,
    hunters::{Hunter, State},
    input::Controls,
    models::{Mats, Models},
    props::{self, Bouncy, BridgePlank, Grabbable, Lantern, LightSource, Thrown},
    util::{damp, damp2, lerp, turn_toward},
    water::Water,
    weather::Storm,
    world::LevelInfo,
};
use avian2d::prelude::*;
use bevy::{light::NotShadowCaster, prelude::*};
use std::f32::consts::PI;

pub const R: f32 = 0.42;
const SWIM: f32 = 6.8;
const BURST: f32 = 12.5;
const WALK: f32 = 3.3;
const JUMP: f32 = 8.0;
/// Where the center floats when resting at the surface: eyes just out.
const SURFACE_DEPTH: f32 = 0.2;
pub const MAX_HP: f32 = 100.0;

/// What hunters can perceive of the bogwight, refreshed every step.
#[derive(Resource, Default)]
pub struct Stealth {
    pub pos: Vec2,
    pub vel: Vec2,
    pub alive: bool,
    /// 0 (invisible) to 1 (lit up in the open).
    pub visibility: f32,
    pub light: f32,
    pub exposure: f32,
    pub in_reeds: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Medium {
    Water,
    Land,
    Air,
}

#[derive(Component)]
pub struct Bogwight {
    pub hp: f32,
    /// Moisture: drains on land, and when it's gone the bogwight suffers.
    pub damp: f32,
    pub facing: f32,
    pub medium: Medium,
    pub submerged: f32,
    pub depth: f32,
    pub deep: bool,
    pub grounded: bool,
    was_grounded: bool,
    coyote: f32,
    pub ground: Option<Entity>,
    dash_cd: f32,
    pub dash_t: f32,
    attack_cd: f32,
    pub attack_t: f32,
    pub attack_dir: Vec2,
    pub held: Option<Entity>,
    pub grip: Option<(Entity, Vec2)>,
    since_hurt: f32,
    invuln: f32,
    pub dead: bool,
    pub dead_t: f32,
    step_t: f32,
    /// Time left in the window after bursting out of the water, when a
    /// strike counts as an ambush from below.
    pub leap: f32,
    last_vy: f32,
    /// Falling through a one-way platform (seconds left).
    pub dropping: f32,
    /// Landing impact for the squash animation.
    pub impact: f32,
    pub god: bool,
}

/// A body in the bogwight's claws remembers how it used to collide.
#[derive(Component)]
pub struct HeldBy {
    layers: CollisionLayers,
}

#[derive(Component)]
pub struct CreatureRig {
    rig: Entity,
    head: Entity,
    arm_f: Entity,
    arm_b: Entity,
    leg_f: Entity,
    leg_b: Entity,
    tail: [Entity; 3],
    yaw: f32,
    tilt: f32,
    land: f32,
    squash: f32,
    squash_v: f32,
    swim: f32,
    walk: f32,
}

pub fn spawn(commands: &mut Commands, models: &Models, mats: &Mats, at: Vec2) {
    let root = commands
        .spawn((
            Transform::from_xyz(at.x, at.y, 0.0),
            Visibility::default(),
            RigidBody::Dynamic,
            Collider::circle(R),
            LockedAxes::ROTATION_LOCKED,
            Mass(60.0),
            Friction::new(0.0).with_combine_rule(CoefficientCombine::Min),
            Restitution::new(0.0),
            GravityScale(0.0),
            CollisionLayers::new(
                Layer::Player,
                [Layer::Ground, Layer::Hunter, Layer::Prop, Layer::Boat, Layer::Bolt],
            ),
            SweptCcd::default(),
            LevelEntity,
        ))
        .id();
    let rig = commands
        .spawn((Transform::IDENTITY, Visibility::default(), ChildOf(root)))
        .id();
    let part = |commands: &mut Commands, mesh: &Handle<Mesh>, at: Vec3, parent: Entity| {
        commands
            .spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mats.wet.clone()),
                Transform::from_translation(at),
                ChildOf(parent),
            ))
            .id()
    };
    part(commands, &models.body, Vec3::ZERO, rig);
    let head = part(commands, &models.head, Vec3::new(0.36, 0.05, 0.0), rig);
    commands.spawn((
        Mesh3d(models.eyes.clone()),
        MeshMaterial3d(mats.glow.clone()),
        Transform::IDENTITY,
        NotShadowCaster,
        ChildOf(head),
    ));
    let arm_f = part(commands, &models.arm, Vec3::new(0.2, -0.05, 0.22), rig);
    let arm_b = part(commands, &models.arm, Vec3::new(0.2, -0.05, -0.22), rig);
    let leg_f = part(commands, &models.leg, Vec3::new(-0.22, -0.14, 0.15), rig);
    let leg_b = part(commands, &models.leg, Vec3::new(-0.22, -0.14, -0.15), rig);
    let t0 = part(commands, &models.tail[0], Vec3::new(-0.4, 0.0, 0.0), rig);
    let t1 = part(commands, &models.tail[1], Vec3::new(-0.32, 0.0, 0.0), t0);
    let t2 = part(commands, &models.tail[2], Vec3::new(-0.3, 0.0, 0.0), t1);
    commands.entity(root).insert((
        Bogwight {
            hp: MAX_HP,
            damp: 1.0,
            facing: 1.0,
            medium: Medium::Water,
            submerged: 1.0,
            depth: 1.0,
            deep: true,
            grounded: false,
            was_grounded: false,
            coyote: 0.0,
            ground: None,
            dash_cd: 0.0,
            dash_t: 0.0,
            attack_cd: 0.0,
            attack_t: 0.0,
            attack_dir: Vec2::X,
            held: None,
            grip: None,
            since_hurt: 10.0,
            invuln: 0.0,
            dead: false,
            dead_t: 0.0,
            step_t: 0.0,
            leap: 0.0,
            last_vy: 0.0,
            dropping: 0.0,
            impact: 0.0,
            god: false,
        },
        CreatureRig {
            rig,
            head,
            arm_f,
            arm_b,
            leg_f,
            leg_b,
            tail: [t0, t1, t2],
            yaw: 0.0,
            tilt: 0.0,
            land: 0.0,
            squash: 0.0,
            squash_v: 0.0,
            swim: 0.0,
            walk: 0.0,
        },
    ));
}

fn footing_filter(exclude: &[Entity]) -> SpatialQueryFilter {
    SpatialQueryFilter::from_mask(Layer::FOOTING).with_excluded_entities(exclude.iter().copied())
}

/// Water depth, footing, and which medium the bogwight is in.
pub fn sense(
    time: Res<Time>,
    water: Res<Water>,
    spatial: SpatialQuery,
    mut q: Query<(Entity, &mut Bogwight, &Position, &LinearVelocity)>,
) {
    let dt = time.delta_secs();
    for (e, mut bw, pos, vel) in &mut q {
        let depth = water.depth(pos.0);
        bw.depth = depth;
        bw.submerged = ((depth + R) / (2.0 * R)).clamp(0.0, 1.0);
        bw.deep = depth > R * 1.1;
        let mut exclude = vec![e];
        exclude.extend(bw.held);
        let filter = footing_filter(&exclude);
        let mut ground = None;
        for dx in [0.0, -0.26, 0.26] {
            let o = pos.0 + Vec2::new(dx, 0.0);
            let len = if dx == 0.0 { R + 0.12 } else { R * 0.8 + 0.1 };
            if let Some(hit) = spatial.cast_ray(o, Dir2::NEG_Y, len, true, &filter)
                && ground.is_none()
            {
                ground = Some(hit.entity);
            }
        }
        bw.was_grounded = bw.grounded;
        bw.grounded = ground.is_some() && vel.y < 3.0;
        bw.ground = ground;
        let before = bw.medium;
        bw.medium = if bw.submerged > 0.3 {
            Medium::Water
        } else if bw.grounded {
            Medium::Land
        } else {
            Medium::Air
        };
        if before == Medium::Water && bw.medium == Medium::Air && vel.y > 3.0 {
            bw.leap = 0.7;
        }
        if bw.medium != Medium::Air {
            bw.leap = (bw.leap - dt * 4.0).max(0.0);
        } else {
            bw.leap = (bw.leap - dt).max(0.0);
        }
    }
}

/// Swimming, walking, jumping, bursting, bouncing.
pub fn movement(
    time: Res<Time>,
    controls: Res<Controls>,
    session: Res<Session>,
    terrain: Res<crate::level::Terrain>,
    mut q: Query<(
        &mut Bogwight,
        &mut Position,
        &mut LinearVelocity,
        &mut GravityScale,
        &mut Friction,
    )>,
    others: Query<&LinearVelocity, Without<Bogwight>>,
    one_way: Query<(), With<props::OneWay>>,
    mut bouncy: Query<&mut Bouncy>,
    mut splash: MessageWriter<Splash>,
    mut noise: MessageWriter<Noise>,
    mut sfx: MessageWriter<Sfx>,
    mut fx: ResMut<Fx>,
    mut water: ResMut<Water>,
) {
    let dt = time.delta_secs();
    for (mut bw, mut pos, mut v, mut gs, mut friction) in &mut q {
        // Fell out of the world somehow: back to the surface.
        if pos.0.y < -25.0 {
            let x = pos.0.x.clamp(2.0, terrain.len - 2.0);
            pos.0 = Vec2::new(x, terrain.height(x).max(WATER_Y) + 1.0);
            v.0 = Vec2::ZERO;
        }
        bw.dash_cd -= dt;
        bw.attack_cd -= dt;
        bw.attack_t = (bw.attack_t - dt).max(0.0);
        bw.dropping -= dt;
        let mv = if bw.dead || !session.live() {
            Vec2::ZERO
        } else {
            controls.mv
        };
        let (jump, dash) = if bw.dead || !session.live() {
            (false, false)
        } else {
            (controls.jump, controls.dash)
        };
        let landed_from = bw.last_vy;
        bw.last_vy = v.y;
        if bw.dead {
            bw.dead_t += dt;
            friction.static_coefficient = 1.0;
            friction.dynamic_coefficient = 1.0;
            if bw.medium == Medium::Water {
                gs.0 = 0.15;
                v.0 *= (-2.0 * dt).exp();
            } else {
                gs.0 = 1.0;
            }
            continue;
        }
        if bw.grip.is_some() {
            gs.0 = 0.0;
            continue;
        }
        if bw.medium == Medium::Water {
            gs.0 = 0.0;
            friction.static_coefficient = 0.0;
            friction.dynamic_coefficient = 0.0;
            let buoy = if bw.deep { 1.0 } else { 1.3 * bw.submerged };
            v.y += GRAVITY * (buoy - 1.0) * dt;
            let carrying = if bw.held.is_some() { 0.8 } else { 1.0 };
            if (jump || dash) && bw.dash_cd <= 0.0 {
                let dir = if mv.length() > 0.2 {
                    mv.normalize()
                } else if !bw.deep {
                    Vec2::Y
                } else {
                    Vec2::new(bw.facing, 0.0)
                };
                v.0 = dir * BURST * carrying + v.0 * 0.15;
                bw.dash_t = 0.32;
                bw.dash_cd = 0.55;
                sfx.write(Sfx::at(Sound::Burst, pos.0));
                for _ in 0..10 {
                    let p = pos.0.extend(0.0) + Vec3::new(fx.rng.sym(0.3), fx.rng.sym(0.3), fx.rng.sym(0.3));
                    let bv = (-dir * fx.rng.range(1.0, 3.0)).extend(fx.rng.sym(0.5));
                    let size = fx.rng.range(0.04, 0.09);
                    fx.emit(Look::Bubble, p, bv, size, 1.4);
                }
                noise.write(Noise {
                    pos: pos.0,
                    radius: if bw.deep { 2.5 } else { 5.5 },
                });
            } else if bw.dash_t > 0.0 {
                bw.dash_t -= dt;
                v.0 *= (-0.8 * dt).exp();
            } else {
                let mut target = mv * SWIM * carrying;
                let diving = mv.y < -0.3;
                if !bw.deep && !diving {
                    // Hold at the surface, eyes out.
                    target.y = ((bw.depth - SURFACE_DEPTH) * 5.0).clamp(-2.5, 2.5);
                }
                if mv.length() > 0.1 || !bw.deep {
                    let rate = if mv.length() > 0.1 { 3.4 } else { 2.2 };
                    v.0 = damp2(v.0, target, rate, dt);
                } else {
                    v.0 *= (-1.6 * dt).exp();
                }
            }
            v.0 = v.0.clamp_length_max(15.0);
            if mv.x.abs() > 0.2 {
                bw.facing = mv.x.signum();
            } else if v.x.abs() > 0.8 {
                bw.facing = v.x.signum();
            }
            // Swimming along the surface leaves a wake.
            if !bw.deep && v.x.abs() > 1.0 {
                water.disturb(pos.0.x - v.x.signum() * 0.5, -v.x.abs() * 0.12 * dt * 10.0);
            }
            continue;
        }
        // On land or in the air.
        gs.0 = 1.0;
        if bw.medium == Medium::Land && !bw.was_grounded && landed_from < -6.0 {
            bw.impact = (-landed_from / 18.0).min(1.0);
            noise.write(Noise {
                pos: pos.0,
                radius: 4.0,
            });
            sfx.write(Sfx::at(Sound::Thud, pos.0));
            fx.burst(Look::Mud, pos.0.extend(0.0) - Vec3::Y * R, 6, 2.0, 0.05, 0.6);
        }
        let ground_vel = bw.ground.and_then(|g| others.get(g).ok()).map_or(Vec2::ZERO, |gv| gv.0);
        if bw.grounded {
            bw.coyote = 0.12;
        } else {
            bw.coyote -= dt;
        }
        let speed = WALK * if bw.held.is_some() { 0.7 } else { 1.0 };
        let target = mv.x * speed;
        if bw.grounded {
            let rel = damp(v.x - ground_vel.x, target, 9.0, dt);
            v.x = ground_vel.x + rel;
        } else if mv.x.abs() > 0.1 {
            // Keep a leap's momentum; steer gently.
            if !(target.signum() == v.x.signum() && v.x.abs() > target.abs()) {
                v.x = damp(v.x, target, 2.5, dt);
            }
        }
        // Down drops through a dock.
        if bw.grounded && mv.y < -0.6 && bw.ground.is_some_and(|g| one_way.get(g).is_ok()) {
            bw.dropping = 0.35;
        }
        if jump && bw.coyote > 0.0 && bw.dropping <= 0.0 {
            v.y = JUMP + ground_vel.y.max(0.0);
            bw.coyote = 0.0;
            sfx.write(Sfx::at(Sound::Jump, pos.0));
        }
        // Mushrooms throw you skyward.
        if bw.grounded
            && let Some(g) = bw.ground
            && let Ok(mut b) = bouncy.get_mut(g)
            && v.y <= 0.5
        {
            v.y = (-landed_from * 0.85).clamp(14.0, 18.0);
            b.squash = 1.0;
            bw.impact = 0.6;
            sfx.write(Sfx::at(Sound::Boing, pos.0));
        }
        let still = bw.grounded && mv.x.abs() < 0.1;
        friction.static_coefficient = if still { 1.5 } else { 0.0 };
        friction.dynamic_coefficient = if still { 1.2 } else { 0.0 };
        if mv.x.abs() > 0.2 {
            bw.facing = mv.x.signum();
        }
        if bw.grounded && (v.x - ground_vel.x).abs() > 1.2 {
            bw.step_t -= dt;
            if bw.step_t <= 0.0 {
                bw.step_t = 0.42;
                noise.write(Noise {
                    pos: pos.0,
                    radius: 3.2,
                });
                sfx.write(Sfx::at(Sound::Squelch, pos.0));
            }
        }
        v.0 = v.0.clamp_length_max(20.0);
        // Entering the water from the air.
        if bw.depth > -R && v.y < -3.0 {
            splash.write(Splash {
                pos: Vec2::new(pos.0.x, WATER_Y),
                strength: -v.y * 0.9,
            });
            water.disturb(pos.0.x, v.y * 0.5);
            noise.write(Noise {
                pos: pos.0,
                radius: 6.0,
            });
        }
    }
}

fn aim_dir(bw: &Bogwight, mv: Vec2) -> Vec2 {
    if bw.medium == Medium::Water && mv.length() > 0.2 {
        mv.normalize()
    } else {
        Vec2::new(bw.facing, (mv.y * 0.8).clamp(-0.6, 0.8)).normalize()
    }
}

/// Claw strikes, grabbing and throwing.
pub fn attack(
    mut commands: Commands,
    controls: Res<Controls>,
    spatial: SpatialQuery,
    session: Res<Session>,
    mut q: Query<(Entity, &mut Bogwight, &Position, &LinearVelocity)>,
    mut hunters: Query<(&mut Hunter, &Position, &CollisionLayers), Without<Bogwight>>,
    mut bodies: Query<
        (
            Forces,
            Option<&mut BridgePlank>,
            Option<&Lantern>,
            Has<Boat>,
            Option<&CollisionLayers>,
            Has<Grabbable>,
        ),
        (Without<Bogwight>, Without<Hunter>),
    >,
    mut hits: MessageWriter<HitHunter>,
    mut noise: MessageWriter<Noise>,
    mut sfx: MessageWriter<Sfx>,
    mut fx: ResMut<Fx>,
) {
    let Ok((me, mut bw, pos, vel)) = q.single_mut() else {
        return;
    };
    if bw.dead || !session.live() {
        return;
    }
    let aim = aim_dir(&bw, controls.mv);
    if controls.attack && bw.attack_cd <= 0.0 {
        bw.attack_cd = 0.38;
        bw.attack_t = 0.28;
        bw.attack_dir = aim;
        sfx.write(Sfx::at(Sound::Claw, pos.0));
        noise.write(Noise {
            pos: pos.0,
            radius: 4.5,
        });
        let center = pos.0 + aim * 0.72;
        let filter =
            SpatialQueryFilter::from_mask([Layer::Hunter, Layer::Prop, Layer::Boat]).with_excluded_entities([me]);
        let found = spatial.shape_intersections(&Collider::circle(0.8), center, 0.0, &filter);
        let mut struck = false;
        for e in found {
            if let Ok((h, hpos, _)) = hunters.get(e) {
                if h.state == State::Dead {
                    continue;
                }
                let from_below = bw.leap > 0.0 && hpos.0.y > pos.0.y - 0.3;
                let unaware = matches!(
                    h.state,
                    State::Patrol | State::Suspicious | State::Flounder | State::Held
                ) || (h.state == State::Hunt && h.unseen > 1.5);
                let ambush = unaware || from_below;
                hits.write(HitHunter {
                    entity: e,
                    damage: 1.0,
                    kind: if ambush { HitKind::Ambush } else { HitKind::Claw },
                    knock: aim * 4.0 + Vec2::Y * 2.0,
                });
                struck = true;
                continue;
            }
            let Ok((mut forces, plank, lantern, is_boat, _, _)) = bodies.get_mut(e) else {
                continue;
            };
            let bpos = forces.position().0;
            if let Some(mut plank) = plank {
                let rot = *forces.rotation();
                if props::cut_plank(&mut commands, &mut plank, bpos, &rot, center) {
                    sfx.write(Sfx::at(Sound::Snap, bpos));
                }
            }
            if let Some(l) = lantern
                && l.lit
            {
                props::smash_lantern(&mut commands, e, bpos, &mut fx, &mut sfx);
            }
            let j = if is_boat { 260.0 } else { 40.0 };
            forces.apply_linear_impulse_at_point(aim * j, center);
            struck = true;
        }
        if struck {
            fx.shake(0.15);
        }
    }
    if controls.grab {
        if let Some(held) = bw.held.take() {
            // Throw.
            let throw_v = aim * 13.0 + vel.0 * 0.5;
            release(&mut commands, held, &mut hunters);
            commands
                .entity(held)
                .insert((LinearVelocity(throw_v), Thrown { t: 1.4, speed: throw_v.length() }, CollidingEntities::default()));
            if let Ok((mut h, _, _)) = hunters.get_mut(held)
                && h.state != State::Dead
            {
                h.stun = 1.0;
            }
            sfx.write(Sfx::at(Sound::Throw, pos.0));
            noise.write(Noise {
                pos: pos.0,
                radius: 5.0,
            });
            return;
        }
        if bw.grip.take().is_some() {
            return;
        }
        let center = pos.0 + aim * 0.5;
        let filter = SpatialQueryFilter::from_mask([Layer::Hunter, Layer::Prop]).with_excluded_entities([me]);
        let found = spatial.shape_intersections(&Collider::circle(0.95), center, 0.0, &filter);
        let mut best: Option<(f32, Entity)> = None;
        for e in found {
            let p = if let Ok((_, hp, _)) = hunters.get(e) {
                hp.0
            } else if let Ok((forces, _, _, _, _, grabbable)) = bodies.get(e) {
                if !grabbable {
                    continue;
                }
                forces.position().0
            } else {
                continue;
            };
            let d = p.distance(center);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, e));
            }
        }
        if let Some((_, e)) = best {
            let layers = if let Ok((mut h, _, layers)) = hunters.get_mut(e) {
                if h.state != State::Dead {
                    h.state = State::Held;
                    h.alert = 1.2;
                    h.drown = 0.0;
                    h.struggle = 0.0;
                    h.aim = 0.0;
                    h.swing = 0.0;
                }
                *layers
            } else if let Ok((_, _, _, _, Some(layers), _)) = bodies.get(e) {
                *layers
            } else {
                CollisionLayers::default()
            };
            let mut filters = layers.filters;
            filters.remove(Layer::Player);
            commands
                .entity(e)
                .insert((HeldBy { layers }, CollisionLayers::new(layers.memberships, filters)));
            bw.held = Some(e);
            sfx.write(Sfx::at(Sound::Grab, pos.0));
            return;
        }
        // Nothing to grab: try the gunwale of a boat.
        let filter = SpatialQueryFilter::from_mask(Layer::Boat);
        for e in spatial.shape_intersections(&Collider::circle(1.1), pos.0, 0.0, &filter) {
            if let Ok((forces, ..)) = bodies.get(e) {
                let local = forces.rotation().inverse() * (pos.0 - forces.position().0);
                let grip = Vec2::new(local.x.clamp(-1.9, 2.0), 0.3);
                bw.grip = Some((e, grip));
                sfx.write(Sfx::at(Sound::Grab, pos.0));
                break;
            }
        }
    }
}

/// Lets go of a held body, restoring how it collides.
fn release(
    commands: &mut Commands,
    e: Entity,
    hunters: &mut Query<(&mut Hunter, &Position, &CollisionLayers), Without<Bogwight>>,
) {
    commands.entity(e).queue(|mut entity: EntityWorldMut| {
        if let Some(held) = entity.take::<HeldBy>() {
            entity.insert(held.layers);
        }
    });
    if let Ok((mut h, _, _)) = hunters.get_mut(e)
        && h.state == State::Held
    {
        h.state = State::Hunt;
        h.alert = 1.2;
    }
}

/// Keeps a held body in the claws (drowning hunters held under), and hangs
/// off a gripped boat, dragging its side down.
pub fn carry(
    mut commands: Commands,
    time: Res<Time>,
    controls: Res<Controls>,
    water: Res<Water>,
    mut q: Query<(Entity, &mut Bogwight, &Position, &mut LinearVelocity, &CollisionLayers)>,
    mut held_q: Query<(&Position, &mut LinearVelocity, Option<&mut Hunter>), (Without<Bogwight>, Without<Boat>)>,
    mut boats: Query<(Forces, &Boat), (Without<Bogwight>, Without<Hunter>)>,
    mut hits: MessageWriter<HitHunter>,
    mut hurt: MessageWriter<HurtPlayer>,
    mut noise: MessageWriter<Noise>,
    mut fx: ResMut<Fx>,
    mut sfx: MessageWriter<Sfx>,
) {
    let dt = time.delta_secs();
    let Ok((me, mut bw, pos, mut v, layers)) = q.single_mut() else {
        return;
    };
    // Hanging off a gunwale, the hull mustn't shove you away from it.
    let solid_boats = bw.grip.is_none();
    if layers.filters.has_all(Layer::Boat) != solid_boats {
        let mut filters = layers.filters;
        if solid_boats {
            filters.add(Layer::Boat);
        } else {
            filters.remove(Layer::Boat);
        }
        commands.entity(me).insert(CollisionLayers::new(layers.memberships, filters));
    }
    if let Some(e) = bw.held {
        let Ok((hpos, mut hv, hunter)) = held_q.get_mut(e) else {
            bw.held = None;
            return;
        };
        let hold = if bw.medium == Medium::Water {
            pos.0 + Vec2::new(bw.facing * 0.7, -0.35)
        } else {
            pos.0 + Vec2::new(bw.facing * 0.75, 0.3)
        };
        let pull = ((hold - hpos.0) * 14.0).clamp_length_max(20.0);
        hv.0 = pull + v.0;
        let mut drop = hpos.0.distance(pos.0) > 3.2 || bw.dead;
        if let Some(mut h) = hunter
            && h.state == State::Held
        {
            let head = hpos.0 + Vec2::Y * 0.6;
            if water.depth(head) > 0.05 {
                h.drown += dt;
                if fx.rng.chance(dt * 12.0) {
                    let p = head.extend(0.2);
                    let bv = Vec3::new(fx.rng.sym(0.5), fx.rng.range(0.5, 1.5), 0.0);
                    let size = fx.rng.range(0.04, 0.1);
                    fx.emit(Look::Bubble, p, bv, size, 1.5);
                }
                if h.drown > 1.6 {
                    hits.write(HitHunter {
                        entity: e,
                        damage: 99.0,
                        kind: HitKind::Drown,
                        knock: Vec2::ZERO,
                    });
                }
            } else {
                h.drown = (h.drown - dt * 0.5).max(0.0);
                h.struggle += dt;
                if h.struggle > 2.4 {
                    // Wriggles free with a knife in your arm.
                    hurt.write(HurtPlayer {
                        amount: 10.0,
                        from: hpos.0,
                        knock: 4.0,
                    });
                    h.stun = 0.4;
                    drop = true;
                }
            }
        }
        if drop {
            bw.held = None;
            commands.entity(e).queue(|mut entity: EntityWorldMut| {
                if let Some(held) = entity.take::<HeldBy>() {
                    entity.insert(held.layers);
                }
                if let Some(mut h) = entity.get_mut::<Hunter>()
                    && h.state == State::Held
                {
                    h.state = State::Hunt;
                }
            });
        }
    }
    if let Some((boat, local)) = bw.grip {
        let Ok((mut forces, b)) = boats.get_mut(boat) else {
            bw.grip = None;
            return;
        };
        let world = forces.position().0 + *forces.rotation() * local;
        if b.flipped || world.distance(pos.0) > 2.5 || bw.dead {
            bw.grip = None;
            return;
        }
        let hang = world + Vec2::new(0.0, -0.45);
        // Follow the gunwale, but never get flung by a spinning hull.
        v.0 = ((hang - pos.0) * 10.0 + forces.velocity_at_point(world)).clamp_length_max(6.0);
        // Hanging on drags that side down; pulling (down) or heaving (up)
        // rocks it harder.
        let mut f = Vec2::new(0.0, -1100.0);
        if controls.mv.y < -0.3 {
            f.y -= 1300.0;
        } else if controls.mv.y > 0.3 {
            f.y = 1300.0;
        }
        forces.apply_force_at_point(f, world);
        // The crew feel the boat list and hear the planks.
        if fx.rng.chance(dt * 1.5) {
            sfx.write(Sfx::at(Sound::Creak, world));
            noise.write(Noise { pos: world, radius: 4.0 });
        }
    }
}

/// Taking damage, healing, drying out, dying.
pub fn vitals(
    time: Res<Time>,
    storm: Res<Storm>,
    mut hurts: MessageReader<HurtPlayer>,
    mut q: Query<(&mut Bogwight, &Position, &mut LinearVelocity)>,
    mut fx: ResMut<Fx>,
    mut sfx: MessageWriter<Sfx>,
) {
    let dt = time.delta_secs();
    let Ok((mut bw, pos, mut v)) = q.single_mut() else {
        hurts.clear();
        return;
    };
    for h in hurts.read() {
        if bw.dead || bw.invuln > 0.0 {
            continue;
        }
        if !bw.god {
            bw.hp -= h.amount;
        }
        bw.invuln = 0.5;
        bw.since_hurt = 0.0;
        let away = (pos.0 - h.from).normalize_or(Vec2::Y);
        v.0 += away * h.knock + Vec2::Y * h.knock * 0.3;
        fx.hurt = 1.0;
        fx.shake(0.45);
        fx.burst(Look::Blood, pos.0.extend(0.3), 8, 2.5, 0.06, 0.8);
        sfx.write(Sfx::ui(Sound::Hurt));
    }
    bw.invuln -= dt;
    bw.since_hurt += dt;
    if bw.dead {
        return;
    }
    if bw.medium == Medium::Water {
        bw.damp = (bw.damp + dt * 0.5).min(1.0);
        if bw.since_hurt > 3.5 {
            bw.hp = (bw.hp + 6.0 * dt).min(MAX_HP);
        }
    } else {
        bw.damp -= dt / (16.0 * (1.0 + storm.rain * 0.5));
        if bw.damp <= 0.0 {
            bw.damp = 0.0;
            if !bw.god {
                bw.hp -= 5.0 * dt;
            }
        }
    }
    if bw.hp <= 0.0 {
        bw.hp = 0.0;
        bw.dead = true;
        bw.held = None;
        bw.grip = None;
        sfx.write(Sfx::ui(Sound::Death));
        fx.shake(0.8);
    }
}

/// How visible the bogwight is right now.
pub fn stealth(
    storm: Res<Storm>,
    level: Res<LevelInfo>,
    mut stealth: ResMut<Stealth>,
    q: Query<(&Bogwight, &Position, &LinearVelocity)>,
    lights: Query<(&LightSource, &GlobalTransform)>,
) {
    let Ok((bw, pos, vel)) = q.single() else {
        stealth.alive = false;
        return;
    };
    let mut light = 0.1;
    light += storm.flash * if bw.deep { 0.15 } else { 1.1 };
    for (ls, gt) in &lights {
        let d = gt.translation().truncate().distance(pos.0);
        if d < ls.radius {
            let k = 1.0 - d / ls.radius;
            light += ls.strength * k * k;
        }
    }
    let mut exposure = if bw.deep {
        if bw.depth > 2.0 { 0.05 } else { 0.12 }
    } else if bw.medium == Medium::Water {
        0.4
    } else {
        1.0
    };
    // Reeds only hide you close to the ground or water, not mid-leap.
    let in_reeds = level.in_reeds(pos.0.x) && (bw.grounded || bw.medium == Medium::Water);
    if in_reeds {
        exposure *= 0.4;
    }
    if bw.medium == Medium::Land && vel.x.abs() > 2.0 {
        exposure *= 1.2;
    }
    if bw.attack_t > 0.0 {
        exposure *= 1.5;
    }
    *stealth = Stealth {
        pos: pos.0,
        vel: vel.0,
        alive: !bw.dead,
        visibility: (light * exposure).clamp(0.0, 1.0),
        light,
        exposure,
        in_reeds,
    };
}

pub fn clear_presses(mut controls: ResMut<Controls>) {
    controls.clear_presses();
}

/// Poses the rig from the body's state: swimming undulation, a lurching
/// walk, claw swipes, squash on landing, a limp float in death.
pub fn animate(
    time: Res<Time>,
    mut q: Query<(&mut Bogwight, &mut CreatureRig, &LinearVelocity)>,
    mut parts: Query<&mut Transform>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    for (mut bw, mut rig, vel) in &mut q {
        let speed = vel.length();
        let swimming = bw.medium == Medium::Water || bw.leap > 0.0;
        let land_target = if swimming || bw.grip.is_some() { 0.0 } else { 1.0 };
        rig.land = damp(rig.land, land_target, 7.0, dt);
        let want_yaw = if bw.facing > 0.0 { 0.0 } else { PI };
        rig.yaw = turn_toward(rig.yaw, want_yaw, dt * if swimming { 12.0 } else { 8.0 });
        let swim_tilt = if speed > 1.0 {
            vel.y.atan2(vel.x.abs()).clamp(-1.3, 1.3)
        } else {
            0.0
        };
        let mut tilt_target = lerp(swim_tilt, 0.42, rig.land);
        if bw.grip.is_some() {
            tilt_target = 1.2;
        }
        if bw.dead {
            tilt_target = -0.3;
        }
        rig.tilt = damp(rig.tilt, tilt_target, 8.0, dt);
        // Squash and stretch on a spring.
        if bw.impact > 0.0 {
            rig.squash_v -= bw.impact * 9.0;
            bw.impact = 0.0;
        }
        let stretch = if swimming { (speed / 14.0).min(0.25) } else { 0.0 };
        let (s, sv) = (rig.squash, rig.squash_v);
        rig.squash_v = sv + (-(s - stretch) * 160.0 - sv * 11.0) * dt;
        rig.squash = (s + rig.squash_v * dt).clamp(-0.45, 0.45);
        rig.swim += dt * (2.5 + speed * 1.1);
        if bw.medium == Medium::Land {
            rig.walk += dt * vel.x.abs() * 3.6;
        }
        let roll = if bw.dead { PI * 0.85 } else { 0.0 };
        let sq = rig.squash;
        if let Ok(mut tf) = parts.get_mut(rig.rig) {
            tf.rotation =
                Quat::from_rotation_y(rig.yaw) * Quat::from_rotation_z(rig.tilt) * Quat::from_rotation_x(roll);
            tf.translation = Vec3::new(0.0, lerp(0.0, 0.1, rig.land), 0.0);
            tf.scale = Vec3::new(1.0 + sq, 1.0 - sq * 0.8, 1.0 - sq * 0.3);
        }
        let set = |parts: &mut Query<&mut Transform>, e: Entity, a: f32| {
            if let Ok(mut tf) = parts.get_mut(e) {
                tf.rotation = Quat::from_rotation_z(a);
            }
        };
        let limp = if bw.dead { 0.15 } else { 1.0 };
        // Tail.
        let amp = (0.22 + speed * 0.035).min(0.6) * (1.0 - rig.land * 0.7) * limp;
        for (k, &seg) in rig.tail.iter().enumerate() {
            let swim = (rig.swim - k as f32 * 0.9).sin() * amp * (1.0 + k as f32 * 0.35);
            let droop = -0.22 * rig.land * (k as f32 * 0.5 + 1.0) + (rig.walk + k as f32).sin() * 0.08 * rig.land;
            set(&mut parts, seg, swim + droop);
        }
        // Arms: swept back swimming, swinging on land, a swipe when striking.
        let stroke = rig.swim.sin() * 0.25;
        let walk_swing = rig.walk.sin() * 0.55;
        let mut af = lerp(-1.75 + stroke, 0.25 + walk_swing - rig.tilt, rig.land);
        let mut ab = lerp(-1.75 - stroke, 0.25 - walk_swing - rig.tilt, rig.land);
        if bw.attack_t > 0.0 {
            let k = 1.0 - bw.attack_t / 0.28;
            let e = 1.0 - (1.0 - k).powi(3);
            af = lerp(2.4, -0.9, e) - rig.tilt * 0.5;
            ab = lerp(ab, 0.8, 0.5);
        }
        if bw.held.is_some() || bw.grip.is_some() {
            af = 1.5 - rig.tilt * 0.6;
            ab = 1.3 - rig.tilt * 0.6;
        }
        if bw.dead {
            af = 0.6;
            ab = 0.4;
        }
        set(&mut parts, rig.arm_f, af);
        set(&mut parts, rig.arm_b, ab);
        // Legs: trailing and kicking in the water, stepping on land.
        let kick = (rig.swim * 1.1).sin() * 0.35;
        let lf = lerp(-1.3 + kick, rig.walk.sin() * 0.7 - rig.tilt, rig.land);
        let lb = lerp(-1.3 - kick, -rig.walk.sin() * 0.7 - rig.tilt, rig.land);
        set(&mut parts, rig.leg_f, lf * limp);
        set(&mut parts, rig.leg_b, lb * limp);
        // The head looks level while the body tilts, and bobs.
        let look = -rig.tilt * 0.35 * rig.land + (t * 2.0).sin() * 0.04;
        set(&mut parts, rig.head, look);
    }
}
