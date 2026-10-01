//! Subagents: what each kind is, how they are summoned, and how they
//! follow orders, walk the (changing) ground, pick fights and die.

use crate::{
    agent::Agent,
    audio::{Sfx, Sound},
    fx::{Fx, Puff},
    game::{Health, Hit, Knock, LOG_BAD, LOG_DIM, Log, Match, MatchEntity, Radius, Targets, Team},
    models::{Mats, Models},
    software::spawn_software,
    spells::{ProjKind, Projectile},
    structures::Obstacles,
    terrain::{Shape, Terrain, TerrainOps, WATER},
    util::{Rng, damp, dir_of, turn_toward, xz, yaw_of},
};
use bevy::{light::NotShadowCaster, prelude::*};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum UnitKind {
    Linter,
    TestRunner,
    Fuzzer,
    Monolith,
    Collector,
}

pub struct UnitDef {
    pub name: &'static str,
    pub blurb: &'static str,
    pub hp: f32,
    pub speed: f32,
    pub damage: f32,
    /// Attack reach, measured from the edge of the target.
    pub range: f32,
    pub cooldown: f32,
    /// How far it looks for a fight.
    pub aggro: f32,
    pub radius: f32,
    /// Hover height; zero for walkers.
    pub fly: f32,
    /// Splash radius of its attack.
    pub splash: f32,
    pub software: usize,
    pub tokens: f32,
    /// Height of its health bar above its feet.
    pub height: f32,
}

impl UnitKind {
    pub const ALL: [UnitKind; 5] = [
        UnitKind::Linter,
        UnitKind::TestRunner,
        UnitKind::Fuzzer,
        UnitKind::Monolith,
        UnitKind::Collector,
    ];

    pub fn index(self) -> usize {
        match self {
            UnitKind::Linter => 0,
            UnitKind::TestRunner => 1,
            UnitKind::Fuzzer => 2,
            UnitKind::Monolith => 3,
            UnitKind::Collector => 4,
        }
    }

    pub fn def(self) -> &'static UnitDef {
        &DEFS[self.index()]
    }

    pub fn fights(self) -> bool {
        self != UnitKind::Collector
    }
}

const DEFS: [UnitDef; 5] = [
    UnitDef {
        name: "LINTER",
        blurb: "Cheap, fast and petty. Swarms anything that looks wrong.",
        hp: 70.0,
        speed: 7.5,
        damage: 9.0,
        range: 0.7,
        cooldown: 0.75,
        aggro: 18.0,
        radius: 0.7,
        fly: 0.0,
        splash: 0.0,
        software: 1,
        tokens: 30.0,
        height: 1.4,
    },
    UnitDef {
        name: "TEST RUNNER",
        blurb: "Fires assertions from range. Fragile up close.",
        hp: 60.0,
        speed: 5.5,
        damage: 13.0,
        range: 17.0,
        cooldown: 1.25,
        aggro: 22.0,
        radius: 0.6,
        fly: 0.0,
        splash: 0.0,
        software: 1,
        tokens: 55.0,
        height: 2.7,
    },
    UnitDef {
        name: "FUZZER",
        blurb: "Flies. Drops random bytes that burst on whatever is below.",
        hp: 85.0,
        speed: 8.0,
        damage: 14.0,
        range: 4.0,
        cooldown: 1.5,
        aggro: 22.0,
        radius: 0.8,
        fly: 6.0,
        splash: 3.2,
        software: 2,
        tokens: 90.0,
        height: 1.4,
    },
    UnitDef {
        name: "MONOLITH",
        blurb: "Legacy code given legs. Slow, enormous, cracks the ground.",
        hp: 360.0,
        speed: 3.6,
        damage: 34.0,
        range: 1.4,
        cooldown: 1.9,
        aggro: 16.0,
        radius: 1.5,
        fly: 0.0,
        splash: 3.0,
        software: 3,
        tokens: 150.0,
        height: 4.8,
    },
    UnitDef {
        name: "GARBAGE COLLECTOR",
        blurb: "Carries enemy software to your altar. Needed to deprecate theirs.",
        hp: 60.0,
        speed: 7.0,
        damage: 0.0,
        range: 0.0,
        cooldown: 1.0,
        aggro: 0.0,
        radius: 0.6,
        fly: 0.0,
        splash: 0.0,
        software: 1,
        tokens: 40.0,
        height: 1.8,
    },
];

/// What a subagent has been told to do.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Order {
    /// Stay with the agent and defend it.
    Follow,
    /// Go to a point, fighting whatever is on the way, then hold there.
    Move(Vec3),
}

#[derive(Component)]
pub struct Unit {
    pub kind: UnitKind,
    pub team: Team,
    /// The software sacrificed to summon it; dropped again when it dies.
    pub souls: Vec<String>,
    pub order: Order,
    pub target: Option<Entity>,
    /// Where it is walking to this frame.
    pub goal: Option<Vec2>,
    pub cd: f32,
    pub yaw: f32,
    pub vel: Vec2,
    pub retarget: f32,
    /// Seconds left materializing; inactive and invulnerable meanwhile.
    pub summoning: f32,
    pub attack_anim: f32,
    pub step: f32,
    /// Which way it goes around obstacles, and for how long.
    pub detour: (f32, f32),
    /// Collector: the software it is carrying.
    pub carrying: Option<Entity>,
    /// Collector: pinned to an altar for a deprecation ritual.
    pub ritual: bool,
    /// Slot in the follow formation.
    pub slot: usize,
    /// Current hover height (fliers).
    pub hover: f32,
    pub pid: u32,
}

/// The visual node under a unit's root, animated separately.
#[derive(Component)]
pub struct UnitBody;

pub fn spawn_unit(
    commands: &mut Commands,
    models: &Models,
    mats: &Mats,
    kind: UnitKind,
    team: Team,
    pos: Vec3,
    yaw: f32,
    souls: Vec<String>,
    order: Order,
    slot: usize,
    pid: u32,
) -> Entity {
    let def = kind.def();
    let model = models.unit(kind, team);
    let root = commands
        .spawn((
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            Unit {
                kind,
                team,
                souls,
                order,
                target: None,
                goal: None,
                cd: 0.5,
                yaw,
                vel: Vec2::ZERO,
                retarget: 0.0,
                summoning: 1.1,
                attack_anim: 0.0,
                step: 0.0,
                detour: (1.0, 0.0),
                carrying: None,
                ritual: false,
                slot,
                hover: 0.0,
                pid,
            },
            team,
            Health::new(def.hp),
            Radius(def.radius),
            Knock::default(),
            MatchEntity,
        ))
        .id();
    commands.entity(root).with_children(|p| {
        let mut body = p.spawn((
            Mesh3d(model.body()),
            MeshMaterial3d(if kind == UnitKind::Collector {
                mats.metal.clone()
            } else {
                mats.matte.clone()
            }),
            Transform::from_scale(Vec3::splat(0.01)),
            UnitBody,
        ));
        if let Some(glow) = &model.glow {
            body.with_child((Mesh3d(glow.clone()), MeshMaterial3d(mats.glow.clone()), NotShadowCaster));
        }
    });
    root
}

/// Moves `from` by `delta` over the ground, refusing climbs steeper than
/// `grade` (rise over run) and sliding along whatever blocks it.
pub fn walk_step(t: &Terrain, from: Vec2, delta: Vec2, grade: f32) -> Vec2 {
    if delta.length_squared() < 1e-10 {
        return from;
    }
    let h0 = t.height_v(from);
    let ok = |to: Vec2| {
        let run = (to - from).length().max(0.05);
        let h = t.height_v(to);
        // Deep water can be left but not entered.
        Terrain::inside(to, 4.0) && h - h0 <= grade * run + 0.03 && (h > DEEP || h > h0)
    };
    let to = from + delta;
    if ok(to) {
        return to;
    }
    let tx = from + Vec2::new(delta.x, 0.0);
    let tz = from + Vec2::new(0.0, delta.y);
    let (first, second) = if delta.x.abs() > delta.y.abs() {
        (tx, tz)
    } else {
        (tz, tx)
    };
    if ok(first) {
        first
    } else if ok(second) {
        second
    } else {
        from
    }
}

/// Picks a walkable heading near `dir`: straight if possible, otherwise
/// turning toward `side` in growing steps. Returns the heading and whether it
/// had to turn.
pub fn steer(t: &Terrain, from: Vec2, dir: Vec2, look: f32, grade: f32, side: f32) -> (Vec2, bool) {
    let clear = |d: Vec2| {
        let h0 = t.height_v(from);
        // Probe a few points ahead.
        (1..=3).all(|k| {
            let p = from + d * (look * k as f32 / 3.0);
            let h = t.height_v(p);
            Terrain::inside(p, 4.0) && h - h0 <= grade * (look * k as f32 / 3.0) + 0.1 && (h > DEEP || h > h0)
        })
    };
    if clear(dir) {
        return (dir, false);
    }
    for step in 1..=6 {
        let a = step as f32 * 0.5;
        for s in [side, -side] {
            let d = Vec2::from_angle(a * s).rotate(dir);
            if clear(d) {
                return (d, true);
            }
        }
    }
    (dir, true)
}

pub const UNIT_GRADE: f32 = 1.15;
/// Water deeper than this stops walkers.
pub const DEEP: f32 = WATER - 2.2;

/// Decides what each subagent is doing: target and destination.
pub fn think(
    time: Res<Time>,
    targets: Res<Targets>,
    agents: Query<(&Agent, &Transform), Without<Unit>>,
    mut units: Query<(&mut Unit, &Transform)>,
) {
    let dt = time.delta_secs();
    let agent_of = |team: Team| agents.iter().find(|(a, _)| a.team == team);
    for (mut u, tf) in &mut units {
        let pos = tf.translation;
        if u.summoning > 0.0 || u.kind == UnitKind::Collector {
            continue;
        }
        let def = u.kind.def();
        let agent = agent_of(u.team);
        let anchor = match u.order {
            Order::Follow => agent.map(|(a, t)| if a.dead.is_some() { a.home } else { t.translation }),
            Order::Move(p) => Some(p),
        };
        // Drop targets that died or that we have been dragged too far from.
        if let Some(t) = u.target {
            match targets.get(t) {
                None => u.target = None,
                Some(tg) => {
                    let leash = anchor.map_or(0.0, |a| xz(tg.pos).distance(xz(a)));
                    if leash > def.aggro + 14.0 || tg.pos.distance(pos) > def.aggro * 1.8 {
                        u.target = None;
                    }
                }
            }
        }
        u.retarget -= dt;
        if u.retarget <= 0.0 {
            u.retarget = 0.4;
            let ground_only = u.kind == UnitKind::Linter || u.kind == UnitKind::Monolith;
            // Melee units can still hit fliers that come down low; keep it simple and let them try.
            let found = targets.nearest_enemy(u.team, pos, def.aggro, false).filter(|t| {
                !(ground_only && t.flying) && anchor.is_none_or(|a| xz(t.pos).distance(xz(a)) < def.aggro + 10.0)
            });
            // Following units also defend the agent from anything near it.
            let guard = if u.order == Order::Follow {
                agent.and_then(|(_, t)| {
                    targets
                        .nearest_enemy(u.team, t.translation, 14.0, ground_only)
                        .filter(|_| found.is_none())
                })
            } else {
                None
            };
            if let Some(f) = found.or(guard) {
                // Stick with the current target unless the new one is much closer.
                let keep = u
                    .target
                    .and_then(|t| targets.get(t))
                    .is_some_and(|cur| cur.pos.distance(pos) < f.pos.distance(pos) + 4.0);
                if !keep {
                    u.target = Some(f.entity);
                }
            }
        }
        // Where to go.
        u.goal = if let Some(t) = u.target.and_then(|t| targets.get(t)) {
            let d = xz(t.pos).distance(xz(pos));
            let reach = def.range + t.radius + def.radius;
            if d > reach * 0.9 { Some(xz(t.pos)) } else { None }
        } else {
            match u.order {
                Order::Follow => agent.and_then(|(a, t)| {
                    if a.dead.is_some() {
                        return Some(xz(a.home) + formation(u.slot) * 1.4);
                    }
                    let base = xz(t.translation);
                    let spot = base + Vec2::from_angle(-a.yaw).rotate(formation(u.slot));
                    (spot.distance(xz(pos)) > 2.0).then_some(spot)
                }),
                Order::Move(p) => {
                    let spot = xz(p) + formation(u.slot) * 0.8;
                    (spot.distance(xz(pos)) > 1.5).then_some(spot)
                }
            }
        };
    }
}

/// Formation offset for slot `i` around a leader facing -Z: fanned out to
/// both sides, leaving a clear lane behind the leader for the camera.
pub fn formation(i: usize) -> Vec2 {
    let ring = (i / 8) as f32;
    let k = i % 8;
    let side = if k.is_multiple_of(2) { 1.0 } else { -1.0 };
    // Degrees from straight ahead.
    let theta = (60.0 + (k / 2) as f32 * 23.0 + ring * 6.0).to_radians();
    let r = 5.5 + ring * 3.5;
    Vec2::new(side * theta.sin() * r, -theta.cos() * r)
}

/// Walks every subagent toward its goal over the ground, keeping them apart.
pub fn move_units(
    time: Res<Time>,
    terrain: Res<Terrain>,
    targets: Res<Targets>,
    obstacles: Res<Obstacles>,
    mut fx: ResMut<Fx>,
    mut units: Query<(Entity, &mut Unit, &mut Transform, &mut Knock, &Radius)>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    // Separation, computed from last frame's positions.
    let bodies: Vec<(Entity, Vec2, f32, bool)> = units
        .iter()
        .map(|(e, u, tf, _, r)| (e, xz(tf.translation), r.0, u.kind.def().fly > 0.0))
        .collect();
    for (e, mut u, mut tf, mut knock, radius) in &mut units {
        let def = u.kind.def();
        let pos = xz(tf.translation);
        if u.summoning > 0.0 {
            u.summoning -= dt;
            let g = terrain.height_v(pos);
            tf.translation.y = g + if def.fly > 0.0 {
                def.fly * (1.0 - u.summoning.max(0.0))
            } else {
                0.0
            };
            continue;
        }
        let mut wish = Vec2::ZERO;
        let flying = def.fly > 0.0;
        if let Some(goal) = u.goal {
            let to = goal - pos;
            let d = to.length();
            if d > 0.05 {
                let dir = to / d;
                u.detour.1 -= dt;
                let (dir, turned) = if flying {
                    (dir, false)
                } else {
                    steer(&terrain, pos, dir, 2.5, UNIT_GRADE, u.detour.0)
                };
                if turned && u.detour.1 <= 0.0 {
                    u.detour = (
                        if (e.index_u32() + (t as u32)).is_multiple_of(2) {
                            1.0
                        } else {
                            -1.0
                        },
                        1.5,
                    );
                }
                let slow = (d / 1.5).clamp(0.3, 1.0);
                wish = dir * def.speed * slow;
            }
        }
        // Push apart from neighbors of the same layer.
        let mut push = Vec2::ZERO;
        for (oe, op, or, ofly) in &bodies {
            if *oe == e || *ofly != flying {
                continue;
            }
            let d = pos - *op;
            let min = radius.0 + or;
            let l = d.length();
            if l < min && l > 1e-4 {
                push += d / l * (min - l) * 4.0;
            }
        }
        // Wading through the data lake is slow.
        let ground = terrain.height_v(pos);
        let wet = if !flying && ground < WATER - 0.3 { 0.55 } else { 1.0 };
        u.vel = u.vel.lerp(wish * wet, (dt * 8.0).min(1.0));
        let kn = Vec2::new(knock.0.x, knock.0.z);
        let delta = (u.vel + push + kn) * dt;
        let next = if flying {
            Terrain::clamp_inside(pos + delta, 5.0)
        } else {
            obstacles.push_out(walk_step(&terrain, pos, delta, UNIT_GRADE), radius.0)
        };
        knock.0 *= (1.0 - dt * 5.0).max(0.0);
        let moved = next - pos;
        // Face where it's going, or its target when standing still.
        let face = if moved.length() > def.speed * dt * 0.25 {
            Some(yaw_of(moved))
        } else {
            u.target.and_then(|t| targets.get(t)).map(|tg| yaw_of(xz(tg.pos) - pos))
        };
        if let Some(y) = face {
            u.yaw = turn_toward(u.yaw, y, dt * 7.0);
        }
        u.step += moved.length();
        let g = terrain.height_v(next);
        let y = if flying {
            u.hover = damp(u.hover, g.max(WATER) + def.fly, 3.0, dt);
            u.hover
        } else {
            g
        };
        tf.translation = Vec3::new(next.x, y, next.y);
        tf.rotation = Quat::from_rotation_y(u.yaw);
        if !flying && ground < WATER && moved.length() > 0.02 && (t * 7.0 + e.index_u32() as f32).fract() < 0.15 {
            fx.puff(
                Puff::Water,
                Vec3::new(next.x, WATER + 0.1, next.y),
                Vec3::Y * 2.0,
                0.5,
                0.4,
            );
        }
    }
}

/// Swings, shoots and bombs.
pub fn attack(
    mut commands: Commands,
    time: Res<Time>,
    targets: Res<Targets>,
    models: Res<Models>,
    mats: Res<Mats>,
    mut ops: ResMut<TerrainOps>,
    mut fx: ResMut<Fx>,
    mut hits: MessageWriter<Hit>,
    mut sfx: MessageWriter<Sfx>,
    mut units: Query<(&mut Unit, &Transform)>,
) {
    let dt = time.delta_secs();
    for (mut u, tf) in &mut units {
        u.cd -= dt;
        u.attack_anim = (u.attack_anim - dt * 3.0).max(0.0);
        if u.summoning > 0.0 || !u.kind.fights() {
            continue;
        }
        let def = u.kind.def();
        let Some(tg) = u.target.and_then(|t| targets.get(t)).copied() else {
            continue;
        };
        let pos = tf.translation;
        let d = xz(tg.pos).distance(xz(pos)) - tg.radius - def.radius;
        if d > def.range || u.cd > 0.0 {
            continue;
        }
        u.cd = def.cooldown;
        u.attack_anim = 1.0;
        let dir = (tg.pos - pos).normalize_or(Vec3::Z);
        match u.kind {
            UnitKind::Linter => {
                hits.write(Hit {
                    target: tg.entity,
                    amount: def.damage,
                    knock: dir * 1.5,
                });
                fx.burst(Puff::Spark, tg.pos + Vec3::Y * 0.8, Vec3::ZERO, 4, 4.0, 0.3, 0.18);
                sfx.write(Sfx::at(Sound::Bite, pos));
            }
            UnitKind::Monolith => {
                // Slams the ground: splash damage and a small dent.
                let at = tg.pos;
                for e in targets.enemies_near(u.team, at, def.splash) {
                    let k = (xz(e.pos) - xz(at)).normalize_or(Vec2::X);
                    hits.write(Hit {
                        target: e.entity,
                        amount: if e.entity == tg.entity {
                            def.damage
                        } else {
                            def.damage * 0.5
                        },
                        knock: Vec3::new(k.x, 0.4, k.y) * 7.0,
                    });
                }
                ops.add(
                    Shape::Crater {
                        center: xz(at),
                        radius: 2.0,
                        depth: 0.35,
                    },
                    0.15,
                );
                fx.burst(Puff::Dust, at, Vec3::Y, 10, 5.0, 0.8, 0.7);
                fx.shake(pos, 0.25);
                sfx.write(Sfx::at(Sound::Slam, pos));
            }
            UnitKind::TestRunner => {
                let muzzle = pos + Vec3::Y * 1.45 + tf.rotation * Vec3::new(0.48, 0.0, -0.7);
                let aim = tg.pos + Vec3::Y * if tg.flying { 0.0 } else { 0.9 };
                let v = (aim - muzzle).normalize_or(Vec3::Z) * 32.0;
                spawn_projectile(
                    &mut commands,
                    &models,
                    &mats,
                    Projectile::new(ProjKind::Assert, u.team, v, def.damage, 0.0, Some(tg.entity)),
                    muzzle,
                );
                sfx.write(Sfx::at(Sound::Zap, pos));
            }
            UnitKind::Fuzzer => {
                let v = Vec3::new(dir.x * 3.0, -2.0, dir.z * 3.0);
                spawn_projectile(
                    &mut commands,
                    &models,
                    &mats,
                    Projectile::new(ProjKind::Byte, u.team, v, def.damage, def.splash, None),
                    pos - Vec3::Y * 0.5,
                );
                sfx.write(Sfx::at(Sound::Blip, pos));
            }
            UnitKind::Collector => {}
        }
    }
}

pub fn spawn_projectile(commands: &mut Commands, models: &Models, mats: &Mats, p: Projectile, pos: Vec3) {
    let model = match p.kind {
        ProjKind::Segfault => &models.segfault,
        ProjKind::Assert => &models.assert_bolt,
        ProjKind::Byte => &models.byte,
        ProjKind::Meteor => &models.meteor,
    };
    let mut e = commands.spawn((
        Transform::from_translation(pos).looking_to(p.vel.normalize_or(Vec3::Z), Vec3::Y),
        Visibility::default(),
        MatchEntity,
        p,
    ));
    e.with_children(|c| {
        if let Some(s) = &model.solid {
            c.spawn((Mesh3d(s.clone()), MeshMaterial3d(mats.matte.clone())));
        }
        if let Some(g) = &model.glow {
            c.spawn((Mesh3d(g.clone()), MeshMaterial3d(mats.glow.clone()), NotShadowCaster));
        }
    });
}

/// Bobbing, stepping, squashing and the summoning rise.
pub fn animate(
    time: Res<Time>,
    units: Query<(&Unit, &Health, &Children)>,
    mut bodies: Query<&mut Transform, With<UnitBody>>,
) {
    let t = time.elapsed_secs();
    for (u, h, children) in &units {
        for child in children.iter() {
            let Ok(mut tf) = bodies.get_mut(child) else { continue };
            let grow = (1.0 - u.summoning / 1.1).clamp(0.0, 1.0);
            let pop = if u.summoning > 0.0 { grow.powf(0.6) } else { 1.0 };
            let phase = u.step * 1.6;
            let swing = u.attack_anim;
            let flash = if h.flash > 0.0 { 1.08 } else { 1.0 };
            let (offset, rot, scale) = match u.kind {
                UnitKind::Linter => (
                    Vec3::Y * (phase * 2.0).sin().abs() * 0.08,
                    Quat::from_rotation_x(-swing * 0.35),
                    Vec3::new(1.0, 1.0 - swing * 0.15, 1.0 + swing * 0.2),
                ),
                UnitKind::TestRunner => (
                    Vec3::Y * (phase).sin().abs() * 0.07,
                    Quat::from_rotation_z((phase).sin() * 0.05) * Quat::from_rotation_x(swing * 0.15),
                    Vec3::ONE,
                ),
                UnitKind::Fuzzer => (
                    Vec3::Y * (t * 2.3 + u.pid as f32).sin() * 0.3,
                    Quat::from_rotation_y(t * 2.0 + u.pid as f32) * Quat::from_rotation_x((t * 1.3).sin() * 0.15),
                    Vec3::splat(1.0 + swing * 0.15),
                ),
                UnitKind::Monolith => (
                    Vec3::Y * (phase * 0.8).sin().abs() * 0.12 - Vec3::Y * swing * 0.3,
                    Quat::from_rotation_z((phase * 0.8).sin() * 0.06) * Quat::from_rotation_x(-swing * 0.3),
                    Vec3::ONE,
                ),
                UnitKind::Collector => (
                    Vec3::Y * ((t * 5.0 + u.pid as f32).sin() * 0.03),
                    Quat::from_rotation_z((phase * 1.5).sin() * 0.08),
                    Vec3::ONE,
                ),
            };
            tf.translation = offset;
            tf.rotation = rot;
            tf.scale = scale * pop * flash;
        }
    }
}

/// Removes dead subagents, dropping the software they were made from.
pub fn deaths(
    mut commands: Commands,
    models: Res<Models>,
    mats: Res<Mats>,
    mut fx: ResMut<Fx>,
    mut log: ResMut<Log>,
    mut game: ResMut<Match>,
    mut sfx: MessageWriter<Sfx>,
    mut rng: Local<Option<Rng>>,
    units: Query<(Entity, &Unit, &Health, &Transform)>,
) {
    let rng = rng.get_or_insert_with(|| Rng::new(4242));
    for (e, u, h, tf) in &units {
        if h.alive() {
            continue;
        }
        let pos = tf.translation;
        fx.burst(Puff::Glitch, pos + Vec3::Y * 0.6, Vec3::Y * 2.0, 18, 6.0, 0.9, 0.3);
        fx.burst(Puff::Smoke, pos + Vec3::Y * 0.4, Vec3::Y, 6, 2.0, 1.2, 0.8);
        sfx.write(Sfx::at(Sound::Crash, pos));
        game.stats[u.team.i()].lost += 1;
        let ground = pos - Vec3::Y * u.kind.def().fly.min(pos.y);
        for (k, name) in u.souls.iter().enumerate() {
            let off = rng.disc(1.2) * k as f32;
            spawn_software(
                &mut commands,
                &models,
                &mats,
                name.clone(),
                Some(u.team),
                Vec3::new(ground.x + off.x, pos.y, ground.z + off.y),
            );
        }
        if !game.demo && u.team == Team::Blue {
            log.push(
                format!(
                    "process {} ({}) exited with code 137",
                    u.pid,
                    u.kind.def().name.to_lowercase()
                ),
                LOG_BAD,
            );
        } else if !game.demo && u.kind == UnitKind::Monolith {
            log.push(
                format!("{}'s monolith was decommissioned", Team::Red.agent_name()),
                LOG_DIM,
            );
        }
        commands.entity(e).despawn();
    }
}

/// Applies an order from an agent to its subagents.
pub fn give_order(units: &mut Query<&mut Unit>, team: Team, order: Order) {
    for mut u in units.iter_mut() {
        if u.team == team && u.kind != UnitKind::Collector {
            u.order = order;
            u.target = None;
            u.retarget = 0.0;
        } else if u.team == team && order == Order::Follow && !u.ritual {
            u.order = Order::Follow;
        }
    }
}

/// Where a summoned subagent appears: a little ahead of the agent.
pub fn summon_point(terrain: &Terrain, pos: Vec3, yaw: f32) -> Vec3 {
    let p = xz(pos) + dir_of(yaw) * 3.2;
    terrain.ground(Terrain::clamp_inside(p, 6.0))
}
