//! Altars, compute wells and the datacenters built on them, plus the
//! deprecation ritual that decides the match.

use crate::{
    agent::{Agent, Intent},
    audio::{Sfx, Sound},
    fx::{Fx, Puff},
    game::{Health, LOG_BAD, LOG_OK, LOG_WARN, Log, Match, MatchEntity, RITUAL_TIME, Radius, Ritual, Team},
    models::{Mats, Models},
    spells::spawn_beam,
    units::{Unit, UnitKind},
    util::xz,
};
use bevy::{light::NotShadowCaster, prelude::*};

pub const DATACENTER_COST: f32 = 80.0;
pub const DEPRECATE_COST: f32 = 100.0;
pub const ALTAR_RADIUS: f32 = 7.6;
const DC_RADIUS: f32 = 2.4;
const DC_HP: f32 = 340.0;
const BUILD_TIME: f32 = 3.0;
/// How close an agent must be to a well to build, or to an enemy altar to deprecate it.
pub const WELL_REACH: f32 = 8.0;
pub const ALTAR_REACH: f32 = 24.0;
/// How close the Garbage Collector must be to the altar's center.
pub const GC_REACH: f32 = ALTAR_RADIUS + 5.0;

#[derive(Component)]
pub struct Altar {
    pub team: Team,
    pub base: Vec3,
    /// Seconds since it was deprecated.
    pub fall: f32,
}

#[derive(Component)]
pub struct AltarCrystal {
    pub team: Team,
    pub base: Vec3,
}

#[derive(Component)]
pub struct Well {
    pub datacenter: Option<Entity>,
}

#[derive(Component)]
pub struct Datacenter {
    pub team: Team,
    pub well: Entity,
    /// Construction progress, 0..1.
    pub rise: f32,
}

impl Datacenter {
    pub fn online(&self) -> bool {
        self.rise >= 1.0
    }
}

#[derive(Component)]
pub struct DcBody;

/// Round footprints nothing can walk through: `(center, radius)`.
#[derive(Resource, Default)]
pub struct Obstacles(pub Vec<(Vec2, f32)>);

impl Obstacles {
    /// Moves `p` (a body of radius `r`) out of any footprint it overlaps.
    pub fn push_out(&self, mut p: Vec2, r: f32) -> Vec2 {
        for &(c, cr) in &self.0 {
            let d = p - c;
            let min = cr + r;
            let l = d.length();
            if l < min {
                p = c + if l > 1e-4 { d / l } else { Vec2::X } * min;
            }
        }
        p
    }

    /// The point just outside the footprint at `center` nearest to `from`.
    pub fn edge(&self, center: Vec2, from: Vec2, margin: f32) -> Vec2 {
        let r = self
            .0
            .iter()
            .filter(|(c, _)| c.distance(center) < 0.5)
            .map(|(_, r)| *r)
            .fold(0.0, f32::max);
        center + (from - center).normalize_or(Vec2::Y) * (r + margin)
    }
}

pub fn spawn_altar(commands: &mut Commands, models: &Models, mats: &Mats, team: Team, pos: Vec3, facing: f32) {
    let model = &models.altar[team.i()];
    commands
        .spawn((
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(facing)),
            Visibility::default(),
            Altar {
                team,
                base: pos,
                fall: 0.0,
            },
            team,
            MatchEntity,
        ))
        .with_children(|c| {
            c.spawn((Mesh3d(model.body()), MeshMaterial3d(mats.matte.clone())));
            if let Some(g) = &model.glow {
                c.spawn((Mesh3d(g.clone()), MeshMaterial3d(mats.glow.clone()), NotShadowCaster));
            }
        });
    if let Some(g) = &models.crystal[team.i()].glow {
        let base = pos + Vec3::Y * 7.4;
        commands.spawn((
            Mesh3d(g.clone()),
            MeshMaterial3d(mats.glow.clone()),
            Transform::from_translation(base),
            AltarCrystal { team, base },
            NotShadowCaster,
            MatchEntity,
        ));
    }
}

pub fn spawn_well(commands: &mut Commands, models: &Models, mats: &Mats, pos: Vec3) {
    commands
        .spawn((
            Transform::from_translation(pos),
            Visibility::default(),
            Well { datacenter: None },
            MatchEntity,
        ))
        .with_children(|c| {
            c.spawn((Mesh3d(models.well.body()), MeshMaterial3d(mats.matte.clone())));
            if let Some(g) = &models.well.glow {
                c.spawn((Mesh3d(g.clone()), MeshMaterial3d(mats.glow.clone()), NotShadowCaster));
            }
        });
}

fn spawn_datacenter(
    commands: &mut Commands,
    models: &Models,
    mats: &Mats,
    team: Team,
    well: Entity,
    pos: Vec3,
) -> Entity {
    let model = &models.datacenter[team.i()];
    commands
        .spawn((
            Transform::from_translation(pos),
            Visibility::default(),
            Datacenter { team, well, rise: 0.0 },
            team,
            Health::new(DC_HP),
            Radius(DC_RADIUS),
            MatchEntity,
        ))
        .with_children(|c| {
            c.spawn((
                Mesh3d(model.body()),
                MeshMaterial3d(mats.metal.clone()),
                Transform::from_xyz(0.0, -7.5, 0.0),
                Visibility::default(),
                DcBody,
            ))
            .with_children(|b| {
                if let Some(g) = &model.glow {
                    b.spawn((Mesh3d(g.clone()), MeshMaterial3d(mats.glow.clone()), NotShadowCaster));
                }
            });
        })
        .id()
}

pub fn update_obstacles(
    mut obstacles: ResMut<Obstacles>,
    altars: Query<&Transform, With<Altar>>,
    dcs: Query<&Transform, With<Datacenter>>,
) {
    obstacles.0.clear();
    for tf in &altars {
        obstacles.0.push((xz(tf.translation), ALTAR_RADIUS));
    }
    for tf in &dcs {
        obstacles.0.push((xz(tf.translation), DC_RADIUS));
    }
}

/// What pressing the interact key would do right now.
#[derive(Clone, Debug, PartialEq)]
pub enum Interaction {
    Nothing,
    Build(Entity),
    Deprecate(Entity),
    /// Something is here, but it can't be done: why.
    Blocked(String),
}

/// Works out the interaction for an agent. Shared by the interact key and
/// the on-screen prompt.
pub fn interaction(
    agent: &Agent,
    pos: Vec3,
    game: &Match,
    wells: &[(Entity, bool, Vec3)],
    enemy_altar: Vec3,
    collectors: &[(Entity, Team, Vec3)],
) -> Interaction {
    if agent.dead.is_some() {
        return Interaction::Nothing;
    }
    if let Some((e, free, _)) = wells
        .iter()
        .filter(|(_, _, p)| xz(*p).distance(xz(pos)) < WELL_REACH)
        .min_by(|a, b| a.2.distance(pos).total_cmp(&b.2.distance(pos)))
    {
        if !free {
            return Interaction::Nothing;
        }
        if agent.tokens < DATACENTER_COST {
            return Interaction::Blocked(format!("PROVISIONING NEEDS {DATACENTER_COST:.0} TOKENS"));
        }
        return Interaction::Build(*e);
    }
    if xz(enemy_altar).distance(xz(pos)) < ALTAR_REACH {
        if game.ritual.is_some() {
            return Interaction::Blocked("A DEPRECATION IS ALREADY RUNNING".into());
        }
        let gc = collectors
            .iter()
            .filter(|(_, t, p)| *t == agent.team && xz(*p).distance(xz(enemy_altar)) < GC_REACH)
            .min_by(|a, b| a.2.distance(enemy_altar).total_cmp(&b.2.distance(enemy_altar)));
        let Some((gc, _, _)) = gc else {
            return Interaction::Blocked("BRING A GARBAGE COLLECTOR TO THE ALTAR".into());
        };
        if agent.tokens < DEPRECATE_COST {
            return Interaction::Blocked(format!("DEPRECATING NEEDS {DEPRECATE_COST:.0} TOKENS"));
        }
        return Interaction::Deprecate(*gc);
    }
    Interaction::Nothing
}

pub fn interact(
    mut commands: Commands,
    models: Res<Models>,
    mats: Res<Mats>,
    mut game: ResMut<Match>,
    mut log: ResMut<Log>,
    mut fx: ResMut<Fx>,
    mut sfx: MessageWriter<Sfx>,
    mut agents: Query<(&mut Agent, &mut Intent, &Transform)>,
    mut wells: Query<(Entity, &mut Well, &Transform)>,
    mut units: Query<(Entity, &mut Unit, &Transform)>,
) {
    let homes: Vec<(Team, Vec3)> = agents.iter().map(|(a, _, _)| (a.team, a.home)).collect();
    for (mut a, mut intent, tf) in &mut agents {
        if !std::mem::take(&mut intent.interact) || !game.live() {
            continue;
        }
        let team = a.team;
        let Some(enemy_altar) = homes.iter().find(|(t, _)| *t != team).map(|h| h.1) else {
            continue;
        };
        let well_list: Vec<(Entity, bool, Vec3)> = wells
            .iter()
            .map(|(e, w, t)| (e, w.datacenter.is_none(), t.translation))
            .collect();
        let gcs: Vec<(Entity, Team, Vec3)> = units
            .iter()
            .filter(|(_, u, _)| u.kind == UnitKind::Collector && u.summoning <= 0.0)
            .map(|(e, u, t)| (e, u.team, t.translation))
            .collect();
        match interaction(&a, tf.translation, &game, &well_list, enemy_altar, &gcs) {
            Interaction::Nothing => {}
            Interaction::Blocked(why) => {
                if a.player {
                    a.error = Some((why, 2.0));
                    sfx.write(Sfx::ui(Sound::Error));
                }
            }
            Interaction::Build(we) => {
                let Ok((_, mut w, wtf)) = wells.get_mut(we) else {
                    continue;
                };
                a.tokens -= DATACENTER_COST;
                let dc = spawn_datacenter(&mut commands, &models, &mats, team, we, wtf.translation);
                w.datacenter = Some(dc);
                game.stats[team.i()].datacenters += 1;
                fx.burst(Puff::Mana, wtf.translation + Vec3::Y, Vec3::Y * 4.0, 30, 5.0, 1.2, 0.25);
                fx.flash(wtf.translation + Vec3::Y * 3.0, team.color(), 300_000.0, 18.0, 0.8);
                sfx.write(Sfx::at(Sound::Build, wtf.translation));
                if !game.demo {
                    if team == Team::Blue {
                        log.push("$ terraform apply  (provisioning datacenter)", 0xe6edf3);
                    } else {
                        log.push(format!("{} is provisioning a datacenter", team.agent_name()), LOG_WARN);
                    }
                }
            }
            Interaction::Deprecate(gc) => {
                let Ok((_, mut u, _)) = units.get_mut(gc) else { continue };
                a.tokens -= DEPRECATE_COST;
                u.ritual = true;
                game.ritual = Some(Ritual {
                    by: team,
                    timer: RITUAL_TIME,
                    collector: gc,
                });
                sfx.write(Sfx::ui(Sound::Alarm));
                if !game.demo {
                    if team == Team::Blue {
                        log.push(
                            format!("$ sudo deprecate --altar {}", team.other().agent_name().to_lowercase()),
                            0xe6edf3,
                        );
                        game.announce("DEPRECATING THEIR ALTAR. HOLD THE GROUND!", 3.0, 0x7ee787);
                    } else {
                        log.push("WARNING: your altar is being deprecated!", LOG_BAD);
                        game.announce("YOUR ALTAR IS BEING DEPRECATED!", 3.5, 0xff6b6b);
                    }
                }
            }
        }
    }
}

/// Runs a deprecation ritual to its end, or aborts it.
pub fn ritual(
    mut commands: Commands,
    time: Res<Time>,
    models: Res<Models>,
    mats: Res<Mats>,
    mut game: ResMut<Match>,
    mut log: ResMut<Log>,
    mut fx: ResMut<Fx>,
    mut sfx: MessageWriter<Sfx>,
    agents: Query<(&Agent, &Transform)>,
    mut units: Query<(&mut Unit, &Transform, &Health)>,
    mut tick: Local<f32>,
) {
    let Some(mut r) = game.ritual else { return };
    let dt = time.delta_secs();
    let Some(altar) = agents.iter().find(|(a, _)| a.team != r.by).map(|(a, _)| a.home) else {
        return;
    };
    let by = agents.iter().find(|(a, _)| a.team == r.by);
    let gc_ok = units
        .get(r.collector)
        .is_ok_and(|(_, t, h)| h.alive() && xz(t.translation).distance(xz(altar)) < GC_REACH + 2.0);
    let agent_ok =
        by.is_some_and(|(a, t)| a.dead.is_none() && xz(t.translation).distance(xz(altar)) < ALTAR_REACH + 8.0);
    if !gc_ok || !agent_ok {
        game.ritual = None;
        if let Ok((mut u, _, _)) = units.get_mut(r.collector) {
            u.ritual = false;
        }
        sfx.write(Sfx::ui(Sound::Abort));
        if !game.demo {
            let why = if !gc_ok {
                "collector lost"
            } else {
                "agent left the altar"
            };
            if r.by == Team::Blue {
                log.push(format!("deprecation aborted: {why}"), LOG_BAD);
                game.announce("DEPRECATION ABORTED", 2.5, 0xff6b6b);
            } else {
                log.push(format!("deprecation of your altar aborted: {why}"), LOG_OK);
                game.announce("ALTAR SAVED", 2.5, 0x7ee787);
            }
        }
        return;
    }
    r.timer -= dt;
    *tick -= dt;
    // Glitching light swirls around the doomed altar.
    let prog = 1.0 - r.timer / RITUAL_TIME;
    let t = time.elapsed_secs();
    for k in 0..3 {
        let a = t * 2.5 + k as f32 * 2.1;
        let rad = 9.0 - prog * 4.0;
        let p = altar
            + Vec3::new(
                a.cos() * rad,
                2.0 + prog * 6.0 + (t * 3.0 + k as f32).sin(),
                a.sin() * rad,
            );
        let j = fx.jitter(2.0);
        fx.puff(Puff::Team(r.by), p, Vec3::Y * 1.5, 0.8, 0.3);
        fx.puff(Puff::Glitch, p, j, 0.5, 0.2);
    }
    if *tick <= 0.0 {
        *tick = 1.0;
        spawn_beam(
            &mut commands,
            &models,
            &mats,
            altar + Vec3::Y * 4.6,
            1.0 + prog * 1.5,
            50.0,
            0.8,
            Some(r.by),
        );
        fx.shake(altar, 0.2 + prog * 0.3);
        sfx.write(Sfx::at(Sound::Glitch, altar));
    }
    if r.timer > 0.0 {
        game.ritual = Some(r);
        return;
    }
    // Deprecated.
    game.ritual = None;
    game.winner = Some(r.by);
    if let Ok((_, gtf, _)) = units.get(r.collector) {
        let p = gtf.translation;
        fx.burst(Puff::Gold, p + Vec3::Y, Vec3::Y * 5.0, 30, 6.0, 1.5, 0.3);
        commands.entity(r.collector).despawn();
    }
    fx.burst(Puff::Glitch, altar + Vec3::Y * 5.0, Vec3::Y * 4.0, 120, 18.0, 2.5, 0.5);
    fx.burst(Puff::Fire, altar + Vec3::Y * 4.0, Vec3::Y * 6.0, 60, 14.0, 1.5, 1.2);
    fx.burst(Puff::Smoke, altar + Vec3::Y * 4.0, Vec3::Y * 3.0, 30, 8.0, 3.0, 2.5);
    fx.flash(altar + Vec3::Y * 8.0, r.by.color(), 4_000_000.0, 60.0, 1.5);
    fx.shake(altar, 1.5);
    spawn_beam(&mut commands, &models, &mats, altar, 5.0, 120.0, 2.5, Some(r.by));
    sfx.write(Sfx::ui(Sound::Deprecated));
    let loser = r.by.other();
    log.push(
        format!("{}'s altar deprecated. end of life.", loser.agent_name()),
        LOG_WARN,
    );
}

/// Construction, destruction and the fountains of free wells.
pub fn datacenters(
    mut commands: Commands,
    time: Res<Time>,
    game: Res<Match>,
    mut log: ResMut<Log>,
    mut fx: ResMut<Fx>,
    mut sfx: MessageWriter<Sfx>,
    mut dcs: Query<(Entity, &mut Datacenter, &Health, &Transform, &Children)>,
    mut bodies: Query<&mut Transform, (With<DcBody>, Without<Datacenter>)>,
    mut wells: Query<(&mut Well, &Transform), (Without<Datacenter>, Without<DcBody>)>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    for (e, mut dc, h, tf, children) in &mut dcs {
        let pos = tf.translation;
        if !h.alive() {
            if let Ok((mut w, _)) = wells.get_mut(dc.well) {
                w.datacenter = None;
            }
            fx.burst(Puff::Fire, pos + Vec3::Y * 3.0, Vec3::Y * 4.0, 30, 9.0, 1.2, 0.9);
            fx.burst(Puff::Debris, pos + Vec3::Y * 2.0, Vec3::Y * 7.0, 30, 9.0, 1.6, 0.4);
            fx.burst(Puff::Glitch, pos + Vec3::Y * 3.0, Vec3::Y * 3.0, 30, 8.0, 1.2, 0.3);
            fx.flash(pos + Vec3::Y * 3.0, 0xff8a3d, 1_200_000.0, 30.0, 0.8);
            fx.shake(pos, 0.6);
            sfx.write(Sfx::at(Sound::BigBoom, pos));
            if !game.demo {
                if dc.team == Team::Blue {
                    log.push("datacenter offline: connection reset by peer", LOG_BAD);
                } else {
                    log.push(format!("{}'s datacenter destroyed", dc.team.agent_name()), LOG_OK);
                }
            }
            commands.entity(e).despawn();
            continue;
        }
        if dc.rise < 1.0 {
            dc.rise = (dc.rise + dt / BUILD_TIME).min(1.0);
            if (t * 10.0).fract() < 0.5 {
                let j = fx.jitter(2.0).with_y(0.3);
                fx.puff(Puff::Dust, pos + j, Vec3::Y * 2.0, 0.8, 0.8);
            }
            if dc.rise >= 1.0 {
                fx.burst(
                    Puff::Team(dc.team),
                    pos + Vec3::Y * 6.0,
                    Vec3::Y * 3.0,
                    20,
                    5.0,
                    1.0,
                    0.25,
                );
                sfx.write(Sfx::at(Sound::Online, pos));
            }
        }
        let k = dc.rise;
        let ease = 1.0 - (1.0 - k).powi(3);
        for c in children.iter() {
            if let Ok(mut btf) = bodies.get_mut(c) {
                btf.translation.y = -7.5 * (1.0 - ease);
                btf.translation.x = if k < 1.0 { (t * 40.0).sin() * 0.05 } else { 0.0 };
            }
        }
    }
    // Free wells bubble with compute.
    for (w, wtf) in &wells {
        if w.datacenter.is_none() && fx.rand() < 0.5 {
            let j = fx.jitter(1.2);
            fx.puff(
                Puff::Mana,
                wtf.translation + Vec3::new(j.x, 0.2, j.z),
                Vec3::new(j.x * 0.3, 4.5 + j.y, j.z * 0.3),
                1.4,
                0.16,
            );
        }
    }
}

/// A deprecated altar sinks, tilting, into the ground.
pub fn altar_fall(
    time: Res<Time>,
    game: Res<Match>,
    mut fx: ResMut<Fx>,
    mut altars: Query<(&mut Altar, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (mut a, mut tf) in &mut altars {
        if game.winner != Some(a.team.other()) {
            continue;
        }
        a.fall += dt;
        let k = (a.fall / 6.0).min(1.0);
        let ease = k * k * (3.0 - 2.0 * k);
        let shake = if k < 1.0 {
            (a.fall * 37.0).sin() * 0.12 * (1.0 - k)
        } else {
            0.0
        };
        tf.translation = a.base + Vec3::new(shake, -ease * 5.2, shake * 0.7);
        let yaw = tf.rotation.to_euler(EulerRot::YXZ).0;
        tf.rotation = Quat::from_euler(EulerRot::YXZ, yaw, ease * 0.12, -ease * 0.08);
        if k < 1.0 && fx.rand() < 0.6 {
            let j = fx.jitter(8.0);
            fx.puff(
                Puff::Debris,
                a.base + Vec3::new(j.x, 1.0 + j.y.abs() * 0.3, j.z),
                Vec3::Y * 4.0,
                1.2,
                0.5,
            );
            let j = fx.jitter(7.0);
            fx.puff(Puff::Dust, a.base + Vec3::new(j.x, 0.5, j.z), Vec3::Y * 2.0, 2.0, 1.5);
            fx.puff(Puff::Glitch, a.base + Vec3::new(j.z, 3.0, j.x), Vec3::Y * 3.0, 1.0, 0.3);
        }
        if k < 1.0 {
            fx.shake(a.base, dt * 0.8);
        }
    }
}

pub fn crystals(time: Res<Time>, game: Res<Match>, mut q: Query<(&AltarCrystal, &mut Transform, &mut Visibility)>) {
    let t = time.elapsed_secs();
    for (c, mut tf, mut vis) in &mut q {
        let doomed = game.ritual.is_some_and(|r| r.by != c.team);
        let dead = game.winner.is_some_and(|w| w != c.team);
        if dead {
            *vis = Visibility::Hidden;
            continue;
        }
        let jitter = if doomed {
            Vec3::new((t * 37.0).sin(), (t * 53.0).sin(), (t * 41.0).cos()) * 0.25
        } else {
            Vec3::ZERO
        };
        tf.translation = c.base + Vec3::Y * ((t * 1.3).sin() * 0.35) + jitter;
        tf.rotation = Quat::from_rotation_y(t * if doomed { 3.0 } else { 0.6 });
        let flick = if doomed && (t * 9.0).fract() < 0.3 { 0.8 } else { 1.0 };
        tf.scale = Vec3::splat(flick);
    }
}
