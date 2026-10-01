//! Spells and summoning. An agent's intent to cast arrives here, is checked
//! against tokens, software and cooldowns, and becomes projectiles, walls,
//! craters, fork bombs or freshly summoned subagents.

use crate::{
    agent::{Agent, Intent},
    audio::{Sfx, Sound},
    decals::Decals,
    fx::{Fx, Puff},
    game::{Health, Hit, LOG_DIM, LOG_OK, Log, Match, MatchEntity, SLOTS, Slot, Targets, Team},
    models::{Mats, Models},
    terrain::{Shape, Terrain, TerrainOps},
    units::{Order, Unit, spawn_projectile, spawn_unit, summon_point},
    util::{Rng, xz},
};
use bevy::{light::NotShadowCaster, prelude::*};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Spell {
    Segfault,
    Hotfix,
    Firewall,
    ForcePush,
    ForkBomb,
}

pub struct SpellDef {
    pub name: &'static str,
    pub blurb: &'static str,
    pub cost: f32,
    pub cooldown: f32,
    /// How far from the agent it can be aimed; zero for self-cast.
    pub range: f32,
    /// Size of the aiming ring.
    pub radius: f32,
}

impl Spell {
    pub fn def(self) -> &'static SpellDef {
        match self {
            Spell::Segfault => &SpellDef {
                name: "SEGFAULT",
                blurb: "Hurls a core dump. Burns what it hits and blasts a small crater.",
                cost: 18.0,
                cooldown: 0.45,
                range: 60.0,
                radius: 3.5,
            },
            Spell::Hotfix => &SpellDef {
                name: "HOTFIX",
                blurb: "Patches you and every subagent within 14 m. Ship it.",
                cost: 40.0,
                cooldown: 3.0,
                range: 0.0,
                radius: 14.0,
            },
            Spell::Firewall => &SpellDef {
                name: "FIREWALL",
                blurb: "Raises a burning wall of earth across the target. Blocks walkers.",
                cost: 55.0,
                cooldown: 6.0,
                range: 45.0,
                radius: 8.0,
            },
            Spell::ForcePush => &SpellDef {
                name: "FORCE PUSH",
                blurb: "Drops a commit from orbit. Leaves a crater you could lose a team in.",
                cost: 110.0,
                cooldown: 12.0,
                range: 70.0,
                radius: 9.0,
            },
            Spell::ForkBomb => &SpellDef {
                name: "FORK BOMB",
                blurb: "The ground forks itself: spikes erupt again and again for 5 s.",
                cost: 90.0,
                cooldown: 14.0,
                range: 55.0,
                radius: 10.0,
            },
        }
    }
}

/// Summoning cooldown, per kind.
const SUMMON_CD: f32 = 0.8;
/// Most subagents one agent can run at once.
pub const PROCESS_LIMIT: usize = 22;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProjKind {
    Segfault,
    Assert,
    Byte,
    Meteor,
}

#[derive(Component)]
pub struct Projectile {
    pub kind: ProjKind,
    pub team: Team,
    pub vel: Vec3,
    pub damage: f32,
    pub splash: f32,
    pub target: Option<Entity>,
    pub life: f32,
    pub gravity: f32,
}

impl Projectile {
    pub fn new(kind: ProjKind, team: Team, vel: Vec3, damage: f32, splash: f32, target: Option<Entity>) -> Self {
        Projectile {
            kind,
            team,
            vel,
            damage,
            splash,
            target,
            life: if kind == ProjKind::Meteor { 10.0 } else { 4.0 },
            gravity: match kind {
                ProjKind::Segfault => SEGFAULT_G,
                ProjKind::Byte => 16.0,
                _ => 0.0,
            },
        }
    }
}

const SEGFAULT_G: f32 = 14.0;
const SEGFAULT_SPEED: f32 = 42.0;

/// Something blew up; scenery nearby is destroyed.
#[derive(Message, Clone, Copy)]
pub struct Blast {
    pub pos: Vec3,
    pub radius: f32,
}

/// A burning wall.
#[derive(Component)]
pub struct FireZone {
    pub team: Team,
    pub a: Vec2,
    pub b: Vec2,
    pub life: f32,
    pub tick: f32,
}

/// Ground that keeps forking spikes.
#[derive(Component)]
pub struct ForkZone {
    pub team: Team,
    pub center: Vec2,
    pub radius: f32,
    pub life: f32,
    pub next: f32,
}

/// The rune circle a subagent is summoned from.
#[derive(Component)]
pub struct SummonCircle {
    pub life: f32,
}

/// A column of light that fades (summoning and sacrifices).
#[derive(Component)]
pub struct Beam {
    pub life: f32,
    pub max: f32,
    pub radius: f32,
    pub height: f32,
}

pub fn spawn_beam(
    commands: &mut Commands,
    models: &Models,
    mats: &Mats,
    pos: Vec3,
    radius: f32,
    height: f32,
    life: f32,
    tint: Option<Team>,
) {
    let model = &models.beam[tint.map_or(2, |t| t.i())];
    if let Some(g) = &model.glow {
        commands.spawn((
            Mesh3d(g.clone()),
            MeshMaterial3d(mats.ghost.clone()),
            Transform::from_translation(pos).with_scale(Vec3::new(radius, height, radius)),
            Beam {
                life,
                max: life,
                radius,
                height,
            },
            NotShadowCaster,
            MatchEntity,
        ));
    }
}

/// The ballistic launch velocity that lands on `to`.
fn lob(from: Vec3, to: Vec3, speed: f32, g: f32) -> Vec3 {
    let d = to - from;
    let t = (d.length() / speed).max(0.12);
    Vec3::new(d.x / t, d.y / t + 0.5 * g * t, d.z / t)
}

/// Where a spell aimed at `target` actually lands: pulled in to its range.
pub fn clamp_aim(terrain: &Terrain, from: Vec3, target: Vec3, range: f32) -> Vec3 {
    let d = xz(target) - xz(from);
    if d.length() <= range {
        return target;
    }
    terrain.ground(Terrain::clamp_inside(xz(from) + d.normalize() * range, 5.0))
}

pub fn cast(
    mut commands: Commands,
    terrain: Res<Terrain>,
    models: Res<Models>,
    mats: Res<Mats>,
    targets: Res<Targets>,
    mut game: ResMut<Match>,
    mut log: ResMut<Log>,
    mut fx: ResMut<Fx>,
    mut ops: ResMut<TerrainOps>,
    mut decals: ResMut<Decals>,
    mut sfx: MessageWriter<Sfx>,
    mut hits: MessageWriter<Hit>,
    mut agents: Query<(Entity, &mut Agent, &mut Intent, &Transform, &Health)>,
    units: Query<(&Unit, &Transform)>,
    mut pid: Local<u32>,
) {
    if *pid == 0 {
        *pid = 3100;
    }
    for (me, mut agent, mut intent, tf, health) in &mut agents {
        let Some((slot_i, aim)) = intent.cast.take() else {
            continue;
        };
        if agent.dead.is_some() || !health.alive() || !game.live() {
            continue;
        }
        let slot = SLOTS[slot_i];
        let pos = tf.translation;
        let is_player = agent.player;
        let fail = |agent: &mut Agent, msg: &str, sfx: &mut MessageWriter<Sfx>| {
            if is_player {
                agent.error = Some((msg.to_string(), 1.6));
                sfx.write(Sfx::ui(Sound::Error));
            }
        };
        if agent.cooldowns[slot_i] > 0.0 || agent.gcd > 0.0 {
            continue;
        }
        if agent.tokens < slot.tokens() {
            fail(&mut agent, "NOT ENOUGH TOKENS", &mut sfx);
            continue;
        }
        if agent.software.len() < slot.software() {
            fail(&mut agent, "NOT ENOUGH SOFTWARE TO SACRIFICE", &mut sfx);
            continue;
        }
        let team = agent.team;
        match slot {
            Slot::Summon(kind) => {
                let count = units.iter().filter(|(u, _)| u.team == team).count();
                if count >= PROCESS_LIMIT {
                    fail(&mut agent, "PROCESS LIMIT REACHED", &mut sfx);
                    continue;
                }
                let def = kind.def();
                agent.tokens -= def.tokens;
                let souls: Vec<String> = agent.software.drain(..def.software).collect();
                let at = summon_point(&terrain, pos, agent.yaw);
                *pid += 1 + (*pid % 7);
                // The lowest free formation slot.
                let taken: Vec<usize> = units
                    .iter()
                    .filter(|(u, _)| u.team == team)
                    .map(|(u, _)| u.slot)
                    .collect();
                let slot_n = (0..).find(|s| !taken.contains(s)).unwrap_or(0);
                spawn_unit(
                    &mut commands,
                    &models,
                    &mats,
                    kind,
                    team,
                    at,
                    agent.yaw,
                    souls.clone(),
                    Order::Follow,
                    slot_n,
                    *pid,
                );
                if let Some(g) = &models.summon_ring[team.i()].glow {
                    let n = terrain.normal(at.x, at.z);
                    commands.spawn((
                        Mesh3d(g.clone()),
                        MeshMaterial3d(mats.ghost.clone()),
                        Transform::from_translation(at + n * 0.2)
                            .with_rotation(Quat::from_rotation_arc(Vec3::Y, n))
                            .with_scale(Vec3::splat(0.01)),
                        SummonCircle { life: 0.0 },
                        NotShadowCaster,
                        MatchEntity,
                    ));
                }
                fx.burst(Puff::Gold, at + Vec3::Y * 0.3, Vec3::Y * 3.0, 16, 3.5, 1.1, 0.22);
                fx.burst(Puff::Team(team), at + Vec3::Y * 0.3, Vec3::Y * 4.0, 16, 3.0, 1.0, 0.18);
                fx.flash(at + Vec3::Y * 1.5, team.color(), 250_000.0, 14.0, 0.6);
                spawn_beam(&mut commands, &models, &mats, at, 0.7, 9.0, 0.8, Some(team));
                // The sacrifice itself happens on the altar.
                spawn_beam(
                    &mut commands,
                    &models,
                    &mats,
                    agent.home + Vec3::Y * 4.6,
                    0.6,
                    40.0,
                    1.2,
                    Some(team),
                );
                fx.burst(Puff::Fire, agent.home + Vec3::Y * 4.8, Vec3::Y * 2.0, 12, 2.5, 0.9, 0.3);
                sfx.write(Sfx::at(Sound::Summon, at));
                let st = &mut game.stats[team.i()];
                st.summoned += 1;
                if !game.demo {
                    let names = souls.join(" ");
                    if team == Team::Blue {
                        log.push(format!("$ sudo sacrifice {names}"), 0xe6edf3);
                        log.push(format!("  spawned {} (pid {})", def.name, *pid), LOG_OK);
                    } else {
                        log.push(
                            format!("{} sacrificed {names} -> {}", team.agent_name(), def.name),
                            LOG_DIM,
                        );
                    }
                }
                agent.cooldowns[slot_i] = SUMMON_CD;
                agent.gcd = 0.3;
                agent.cast_anim = 1.0;
            }
            Slot::Spell(spell) => {
                let def = spell.def();
                let target = if def.range > 0.0 {
                    clamp_aim(&terrain, pos, aim, def.range)
                } else {
                    pos
                };
                agent.tokens -= def.cost;
                agent.cooldowns[slot_i] = def.cooldown;
                agent.gcd = 0.25;
                agent.cast_anim = 1.0;
                game.stats[team.i()].spells += 1;
                let hand = pos + Vec3::Y * 1.3 + tf.rotation * Vec3::new(0.64, 0.0, -0.5);
                match spell {
                    Spell::Segfault => {
                        let v = lob(hand, target, SEGFAULT_SPEED, SEGFAULT_G);
                        spawn_projectile(
                            &mut commands,
                            &models,
                            &mats,
                            Projectile::new(ProjKind::Segfault, team, v, 26.0, 4.0, None),
                            hand,
                        );
                        sfx.write(Sfx::at(Sound::Cast, pos));
                    }
                    Spell::Hotfix => {
                        hits.write(Hit {
                            target: me,
                            amount: -45.0,
                            knock: Vec3::ZERO,
                        });
                        fx.burst(Puff::Heal, pos + Vec3::Y, Vec3::Y * 2.0, 20, 3.0, 1.2, 0.2);
                        for t in targets.friends_near(team, pos, def.radius) {
                            if t.entity != me {
                                hits.write(Hit {
                                    target: t.entity,
                                    amount: -45.0,
                                    knock: Vec3::ZERO,
                                });
                                fx.burst(Puff::Heal, t.pos + Vec3::Y, Vec3::Y * 2.0, 6, 2.0, 1.0, 0.18);
                            }
                        }
                        decals.ring(xz(pos), def.radius, 0x7ee787, 0.8);
                        fx.flash(pos + Vec3::Y * 2.0, 0x7ee787, 200_000.0, 16.0, 0.6);
                        sfx.write(Sfx::at(Sound::Heal, pos));
                    }
                    Spell::Firewall => {
                        let (a, b) = wall_ends(pos, target);
                        ops.add(
                            Shape::Wall {
                                a,
                                b,
                                half_width: 1.3,
                                height: 6.0,
                            },
                            0.9,
                        );
                        commands.spawn((
                            FireZone {
                                team,
                                a,
                                b,
                                life: 7.0,
                                tick: 0.0,
                            },
                            MatchEntity,
                        ));
                        for k in 0..=12 {
                            let p = a.lerp(b, k as f32 / 12.0);
                            fx.burst(Puff::Dust, terrain.ground(p), Vec3::Y * 2.0, 3, 4.0, 1.2, 0.9);
                        }
                        fx.shake(target, 0.5);
                        sfx.write(Sfx::at(Sound::Rumble, target));
                    }
                    Spell::ForcePush => {
                        let back = (xz(pos) - xz(target)).normalize_or(Vec2::X);
                        let start = target + Vec3::new(back.x * 26.0, 70.0, back.y * 26.0);
                        let v = (target - start) / 1.5;
                        spawn_projectile(
                            &mut commands,
                            &models,
                            &mats,
                            Projectile::new(ProjKind::Meteor, team, v, 80.0, 9.0, None),
                            start,
                        );
                        decals.ring(xz(target), def.radius, 0xff8a3d, 1.5);
                        decals.ring(xz(target), def.radius * 0.5, 0xff8a3d, 1.5);
                        sfx.write(Sfx::at(Sound::Incoming, target));
                    }
                    Spell::ForkBomb => {
                        commands.spawn((
                            ForkZone {
                                team,
                                center: xz(target),
                                radius: def.radius,
                                life: 5.0,
                                next: 0.0,
                            },
                            MatchEntity,
                        ));
                        decals.ring(xz(target), def.radius, 0xff4fd8, 5.0);
                        sfx.write(Sfx::at(Sound::Fork, target));
                    }
                }
            }
        }
    }
}

/// The two ends of a firewall raised at `target` by an agent at `from`:
/// across the line between them.
pub fn wall_ends(from: Vec3, target: Vec3) -> (Vec2, Vec2) {
    let d = (xz(target) - xz(from)).normalize_or(Vec2::Y);
    let across = Vec2::new(-d.y, d.x);
    let c = xz(target);
    (c - across * 8.0, c + across * 8.0)
}

/// Flies projectiles and blows them up.
pub fn projectiles(
    mut commands: Commands,
    time: Res<Time>,
    terrain: Res<Terrain>,
    targets: Res<Targets>,
    mut ops: ResMut<TerrainOps>,
    mut fx: ResMut<Fx>,
    mut chars: ResMut<TerrainChar>,
    mut hits: MessageWriter<Hit>,
    mut blasts: MessageWriter<Blast>,
    mut sfx: MessageWriter<Sfx>,
    mut q: Query<(Entity, &mut Projectile, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (e, mut p, mut tf) in &mut q {
        p.life -= dt;
        // Assertions curve toward their target.
        if p.kind == ProjKind::Assert
            && let Some(t) = p.target.and_then(|t| targets.get(t))
        {
            let want = (t.pos + Vec3::Y * if t.flying { 0.0 } else { 0.9 } - tf.translation).normalize_or(Vec3::Z);
            let speed = p.vel.length();
            p.vel = p.vel.normalize_or(Vec3::Z).lerp(want, (dt * 6.0).min(1.0)).normalize() * speed;
        }
        let g = p.gravity;
        p.vel.y -= g * dt;
        let pos = tf.translation + p.vel * dt;
        tf.translation = pos;
        if p.vel.length_squared() > 0.01 {
            tf.look_to(p.vel.normalize(), Vec3::Y);
        }
        if p.kind == ProjKind::Meteor {
            tf.rotate_local_z(dt * 3.0);
        }
        // Trails.
        let (j1, j2) = (fx.jitter(1.5), fx.jitter(1.5));
        match p.kind {
            ProjKind::Segfault => fx.puff(Puff::Fire, pos, j1 * 0.4, 0.35, 0.35),
            ProjKind::Meteor => {
                fx.puff(Puff::Fire, pos + j1, Vec3::Y * 2.0, 0.6, 1.6);
                fx.puff(Puff::Smoke, pos + j2, Vec3::Y, 1.4, 1.8);
            }
            ProjKind::Byte => {
                if fx.rand() < 0.3 {
                    fx.puff(Puff::Glitch, pos, Vec3::ZERO, 0.3, 0.12);
                }
            }
            ProjKind::Assert => {}
        }
        let ground = terrain.height(pos.x, pos.z);
        let mut hit_unit = None;
        if p.kind != ProjKind::Meteor {
            hit_unit = targets
                .0
                .iter()
                .filter(|t| t.team != p.team)
                .find(|t| {
                    let center = t.pos + Vec3::Y * if t.flying { 0.0 } else { t.radius.min(1.2) };
                    center.distance(pos) < t.radius + 0.5
                })
                .copied();
        }
        let expired = p.life <= 0.0 || !Terrain::inside(xz(pos), 0.0);
        if pos.y > ground && hit_unit.is_none() && !expired {
            continue;
        }
        let at = if pos.y <= ground {
            Vec3::new(pos.x, ground, pos.z)
        } else {
            pos
        };
        commands.entity(e).despawn();
        match p.kind {
            ProjKind::Assert => {
                if let Some(t) = hit_unit {
                    hits.write(Hit {
                        target: t.entity,
                        amount: p.damage,
                        knock: p.vel.normalize_or(Vec3::Z) * 1.5,
                    });
                }
                fx.burst(Puff::Heal, at, Vec3::ZERO, 6, 4.0, 0.3, 0.14);
            }
            ProjKind::Byte => {
                splash(&targets, &mut hits, p.team, at, p.splash, p.damage, 3.0);
                fx.burst(Puff::Glitch, at, Vec3::Y * 2.0, 12, 5.0, 0.6, 0.22);
                chars.0.push((xz(at), 1.6, 0.25));
                sfx.write(Sfx::at(Sound::Pop, at));
            }
            ProjKind::Segfault => {
                splash(&targets, &mut hits, p.team, at, p.splash, p.damage, 6.0);
                ops.add(
                    Shape::Crater {
                        center: xz(at),
                        radius: 3.0,
                        depth: 0.8,
                    },
                    0.12,
                );
                chars.0.push((xz(at), 3.6, 0.5));
                fx.burst(Puff::Fire, at + Vec3::Y * 0.5, Vec3::Y * 2.0, 16, 7.0, 0.6, 0.55);
                fx.burst(Puff::Debris, at, Vec3::Y * 5.0, 8, 7.0, 1.0, 0.3);
                fx.burst(Puff::Smoke, at + Vec3::Y, Vec3::Y, 5, 2.0, 1.4, 1.0);
                fx.flash(at + Vec3::Y * 1.5, 0xff8a3d, 400_000.0, 16.0, 0.35);
                fx.shake(at, 0.3);
                blasts.write(Blast { pos: at, radius: 2.2 });
                sfx.write(Sfx::at(Sound::Boom, at));
            }
            ProjKind::Meteor => {
                splash(&targets, &mut hits, p.team, at, p.splash, p.damage, 16.0);
                ops.add(
                    Shape::Crater {
                        center: xz(at),
                        radius: 9.5,
                        depth: 4.2,
                    },
                    0.3,
                );
                chars.0.push((xz(at), 12.0, 0.9));
                fx.burst(Puff::Fire, at + Vec3::Y, Vec3::Y * 6.0, 50, 16.0, 1.0, 1.4);
                fx.burst(Puff::Debris, at, Vec3::Y * 12.0, 40, 16.0, 1.8, 0.6);
                fx.burst(Puff::Smoke, at + Vec3::Y * 2.0, Vec3::Y * 3.0, 24, 8.0, 2.5, 2.4);
                fx.scatter(Puff::Dust, at, 12.0, 30, 3.0, 2.0, 1.4);
                fx.flash(at + Vec3::Y * 4.0, 0xffa050, 3_000_000.0, 45.0, 0.9);
                fx.shake(at, 1.3);
                blasts.write(Blast { pos: at, radius: 10.0 });
                sfx.write(Sfx::at(Sound::BigBoom, at));
            }
        }
    }
}

/// Damage falling off from full at the center to half at the edge.
fn splash(
    targets: &Targets,
    hits: &mut MessageWriter<Hit>,
    team: Team,
    at: Vec3,
    radius: f32,
    damage: f32,
    knock: f32,
) {
    for t in targets.enemies_near(team, at, radius) {
        let d = (t.pos.distance(at) - t.radius).max(0.0);
        let k = 1.0 - 0.5 * (d / radius).clamp(0.0, 1.0);
        let dir = (xz(t.pos) - xz(at)).normalize_or(Vec2::X);
        hits.write(Hit {
            target: t.entity,
            amount: damage * k,
            knock: Vec3::new(dir.x, 0.5, dir.y) * knock * k,
        });
    }
}

/// Charring requested this frame: `(center, radius, amount)`.
#[derive(Resource, Default)]
pub struct TerrainChar(pub Vec<(Vec2, f32, f32)>);

pub fn apply_char(mut chars: ResMut<TerrainChar>, mut terrain: ResMut<Terrain>) {
    for (c, r, a) in chars.0.drain(..) {
        terrain.char(c, r, a);
    }
}

/// Burning walls and fork bombs.
pub fn zones(
    mut commands: Commands,
    time: Res<Time>,
    terrain: Res<Terrain>,
    targets: Res<Targets>,
    mut ops: ResMut<TerrainOps>,
    mut chars: ResMut<TerrainChar>,
    mut fx: ResMut<Fx>,
    mut hits: MessageWriter<Hit>,
    mut blasts: MessageWriter<Blast>,
    mut sfx: MessageWriter<Sfx>,
    mut fires: Query<(Entity, &mut FireZone)>,
    mut forks: Query<(Entity, &mut ForkZone)>,
    mut rng: Local<Option<Rng>>,
) {
    let rng = rng.get_or_insert_with(|| Rng::new(808));
    let dt = time.delta_secs();
    for (e, mut z) in &mut fires {
        z.life -= dt;
        z.tick -= dt;
        if z.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        // Flames along the crest.
        let n = if z.life > 1.0 { 5 } else { 2 };
        for _ in 0..n {
            let p = z.a.lerp(z.b, rng.f()) + rng.disc(0.6);
            let g = terrain.ground(p);
            let size = rng.range(0.3, 0.6);
            fx.puff(
                Puff::Fire,
                g + Vec3::Y * 0.2,
                Vec3::new(rng.sym(0.4), rng.range(3.0, 6.0), rng.sym(0.4)),
                0.55,
                size,
            );
        }
        if rng.chance(0.15) {
            let p = z.a.lerp(z.b, rng.f());
            fx.puff(Puff::Smoke, terrain.ground(p) + Vec3::Y * 2.0, Vec3::Y * 2.0, 1.5, 0.8);
        }
        if z.tick <= 0.0 {
            z.tick = 0.4;
            let ab = z.b - z.a;
            for t in targets.0.iter().filter(|t| t.team != z.team && !t.flying) {
                let p = xz(t.pos);
                let s = ((p - z.a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
                if p.distance(z.a + ab * s) < 2.4 + t.radius {
                    hits.write(Hit {
                        target: t.entity,
                        amount: 7.0,
                        knock: Vec3::ZERO,
                    });
                }
            }
        }
    }
    for (e, mut z) in &mut forks {
        z.life -= dt;
        z.next -= dt;
        if z.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        if z.next > 0.0 {
            continue;
        }
        z.next = 0.2;
        let c = z.center + rng.disc(z.radius);
        let r = rng.range(1.4, 2.3);
        let h = rng.range(1.4, 3.2);
        ops.add(
            Shape::Spike {
                center: c,
                radius: r,
                height: h,
            },
            0.18,
        );
        chars.0.push((c, r, 0.15));
        let g = terrain.ground(c);
        for t in targets.enemies_near(z.team, g, 2.8) {
            hits.write(Hit {
                target: t.entity,
                amount: 13.0,
                knock: Vec3::Y * 6.0 + (t.pos - g).normalize_or(Vec3::X) * 3.0,
            });
        }
        fx.burst(Puff::Glitch, g + Vec3::Y * 0.5, Vec3::Y * 5.0, 8, 5.0, 0.7, 0.28);
        fx.burst(Puff::Debris, g, Vec3::Y * 6.0, 4, 4.0, 0.9, 0.3);
        fx.shake(g, 0.12);
        blasts.write(Blast { pos: g, radius: 1.8 });
        if rng.chance(0.35) {
            sfx.write(Sfx::at(Sound::Glitch, g));
        }
    }
}

pub fn circles(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut SummonCircle, &mut Transform)>) {
    let dt = time.delta_secs();
    for (e, mut c, mut tf) in &mut q {
        c.life += dt;
        if c.life > 1.8 {
            commands.entity(e).despawn();
            continue;
        }
        // Open quickly, spin, then close.
        let open = (c.life / 0.3).min(1.0);
        let close = ((1.8 - c.life) / 0.4).clamp(0.0, 1.0);
        tf.scale = Vec3::splat((open * close).max(0.01) * 1.2);
        tf.rotate_local_y(dt * (4.0 - c.life * 1.5));
    }
}

pub fn beams(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Beam, &mut Transform)>) {
    let dt = time.delta_secs();
    for (e, mut b, mut tf) in &mut q {
        b.life -= dt;
        if b.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let k = b.life / b.max;
        let r = b.radius * k.powf(0.7);
        tf.scale = Vec3::new(r, b.height * (1.0 + (1.0 - k) * 0.3), r);
    }
}
