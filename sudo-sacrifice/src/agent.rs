//! The agents: the two wizards of this war. Each one walks, jumps, casts,
//! runs out of context (health) and comes back at its altar. The player's
//! agent and the CPU's are the same thing, steered through [`Intent`].

use crate::{
    audio::{Sfx, Sound},
    fx::{Fx, Puff},
    game::{Health, Knock, LOG_BAD, LOG_WARN, Log, Match, MatchEntity, Radius, SLOTS, Team},
    models::{Mats, Models},
    structures::{Datacenter, Obstacles},
    terrain::{Terrain, WATER},
    units::{Order, walk_step},
    util::{turn_toward, xz},
};
use bevy::{light::NotShadowCaster, prelude::*};

pub const MAX_CONTEXT: f32 = 220.0;
pub const MAX_TOKENS: f32 = 300.0;
pub const RESPAWN: f32 = 8.0;
const SPEED: f32 = 9.5;
const GRADE: f32 = 1.5;
const GRAVITY: f32 = 24.0;
const JUMP: f32 = 8.5;
/// Token income: a trickle, more per datacenter, more near your own buildings.
pub const REGEN_BASE: f32 = 2.5;
pub const REGEN_PER_DC: f32 = 1.6;
pub const REGEN_NEAR: f32 = 5.0;
pub const NEAR: f32 = 22.0;

#[derive(Component)]
pub struct Agent {
    pub team: Team,
    /// Driven by the keyboard and mouse.
    pub player: bool,
    pub tokens: f32,
    /// Software held, ready to sacrifice; the oldest goes first.
    pub software: Vec<String>,
    pub yaw: f32,
    pub vel: Vec2,
    pub vy: f32,
    /// Height above the ground.
    pub air: f32,
    pub cooldowns: [f32; 10],
    /// Global cooldown after any cast.
    pub gcd: f32,
    /// Seconds until respawn while compacted.
    pub dead: Option<f32>,
    pub cast_anim: f32,
    pub selected: usize,
    /// Current token income per second.
    pub regen: f32,
    /// The altar's position.
    pub home: Vec3,
    /// A complaint to show the player, and for how long.
    pub error: Option<(String, f32)>,
    pub step: f32,
}

/// What an agent wants to do this frame, set by the player's input or the CPU.
#[derive(Component, Default)]
pub struct Intent {
    /// Desired movement in world XZ, length up to 1.
    pub mv: Vec2,
    /// Heading to face.
    pub face: Option<f32>,
    /// Turn instantly (mouse look) rather than at walking pace.
    pub snap: bool,
    pub jump: bool,
    /// Slot index and target point.
    pub cast: Option<(usize, Vec3)>,
    pub order: Option<Order>,
    pub interact: bool,
    /// Where the agent is looking.
    pub aim: Vec3,
}

#[derive(Component)]
pub struct AgentBody;

/// Orbiting token cubes.
#[derive(Component)]
pub struct Orbiter(pub f32);

pub fn spawn_agent(
    commands: &mut Commands,
    models: &Models,
    mats: &Mats,
    team: Team,
    home: Vec3,
    pos: Vec3,
    yaw: f32,
    player: bool,
    software: Vec<String>,
) -> Entity {
    let model = &models.agent[team.i()];
    let root = commands
        .spawn((
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            Agent {
                team,
                player,
                tokens: 140.0,
                software,
                yaw,
                vel: Vec2::ZERO,
                vy: 0.0,
                air: 0.0,
                cooldowns: [0.0; 10],
                gcd: 0.0,
                dead: None,
                cast_anim: 0.0,
                selected: 0,
                regen: REGEN_BASE,
                home,
                error: None,
                step: 0.0,
            },
            Intent { aim: pos, ..default() },
            team,
            Health::new(MAX_CONTEXT),
            Radius(0.8),
            Knock::default(),
            MatchEntity,
        ))
        .id();
    commands.entity(root).with_children(|p| {
        p.spawn((
            Mesh3d(model.body()),
            MeshMaterial3d(mats.matte.clone()),
            Transform::default(),
            Visibility::default(),
            AgentBody,
        ))
        .with_children(|b| {
            if let Some(g) = &model.glow {
                b.spawn((Mesh3d(g.clone()), MeshMaterial3d(mats.glow.clone()), NotShadowCaster));
            }
            for k in 0..3 {
                if let Some(g) = &models.token.glow {
                    b.spawn((
                        Mesh3d(g.clone()),
                        MeshMaterial3d(mats.glow.clone()),
                        Transform::default(),
                        Orbiter(k as f32 / 3.0 * std::f32::consts::TAU),
                        NotShadowCaster,
                    ));
                }
            }
        });
    });
    root
}

pub fn move_agents(
    time: Res<Time>,
    terrain: Res<Terrain>,
    obstacles: Res<Obstacles>,
    mut fx: ResMut<Fx>,
    mut q: Query<(&mut Agent, &Intent, &mut Transform, &mut Knock)>,
) {
    let dt = time.delta_secs().min(0.05);
    for (mut a, intent, mut tf, mut knock) in &mut q {
        if a.dead.is_some() {
            knock.0 = Vec3::ZERO;
            continue;
        }
        if let Some(face) = intent.face {
            a.yaw = if intent.snap {
                face
            } else {
                turn_toward(a.yaw, face, dt * 7.0)
            };
        }
        let pos = xz(tf.translation);
        let ground = terrain.height_v(pos);
        let wet = if ground < WATER - 0.3 && a.air < 0.2 { 0.6 } else { 1.0 };
        let wish = intent.mv.clamp_length_max(1.0) * SPEED * wet;
        let accel = if a.air > 0.2 { 3.0 } else { 11.0 };
        a.vel = a.vel.lerp(wish, (dt * accel).min(1.0));
        let kn = Vec2::new(knock.0.x, knock.0.z);
        let delta = (a.vel + kn) * dt;
        let y_now = ground + a.air;
        let next = if a.air > 0.3 {
            // Airborne: may pass over anything lower than itself.
            let to = Terrain::clamp_inside(pos + delta, 4.0);
            if terrain.height_v(to) <= y_now + 0.3 {
                to
            } else {
                walk_step(&terrain, pos, delta, GRADE)
            }
        } else {
            walk_step(&terrain, pos, delta, GRADE)
        };
        let next = obstacles.push_out(next, 0.8);
        a.step += (next - pos).length();
        let g2 = terrain.height_v(next);
        // Vertical: jumps, knock-ups and gravity, measured against the ground.
        if intent.jump && a.air < 0.05 {
            a.vy = JUMP;
        }
        a.vy += knock.0.y;
        knock.0 = Vec3::ZERO;
        let mut y = y_now + a.vy * dt;
        a.vy -= GRAVITY * dt;
        if y <= g2 {
            if a.vy < -12.0 {
                fx.burst(Puff::Dust, Vec3::new(next.x, g2, next.y), Vec3::ZERO, 6, 3.0, 0.6, 0.6);
            }
            y = g2;
            a.vy = 0.0;
        }
        a.air = y - g2;
        tf.translation = Vec3::new(next.x, y, next.y);
        tf.rotation = Quat::from_rotation_y(a.yaw);
    }
}

/// Token income, context regeneration, cooldowns, compaction and respawning.
pub fn status(
    time: Res<Time>,
    terrain: Res<Terrain>,
    mut game: ResMut<Match>,
    mut log: ResMut<Log>,
    mut fx: ResMut<Fx>,
    mut sfx: MessageWriter<Sfx>,
    settings: Res<crate::game::Settings>,
    args: Res<crate::Args>,
    dcs: Query<(&Datacenter, &Transform)>,
    mut q: Query<(&mut Agent, &mut Health, &mut Transform, &mut Visibility), Without<Datacenter>>,
) {
    let dt = time.delta_secs();
    for (mut a, mut h, mut tf, mut vis) in &mut q {
        for cd in a.cooldowns.iter_mut() {
            *cd = (*cd - dt).max(0.0);
        }
        a.gcd = (a.gcd - dt).max(0.0);
        a.cast_anim = (a.cast_anim - dt * 2.5).max(0.0);
        let expired = a.error.as_mut().is_some_and(|(_, t)| {
            *t -= dt;
            *t <= 0.0
        });
        if expired {
            a.error = None;
        }
        let team = a.team;
        if let Some(t) = a.dead.as_mut() {
            *t -= dt;
            if *t <= 0.0 {
                // Back at the altar with a fresh context.
                a.dead = None;
                h.hp = h.max;
                h.shield = 4.0;
                // In front of the altar, facing the middle of the island.
                let out = (-xz(a.home)).normalize_or(Vec2::Y);
                tf.translation = terrain.ground(xz(a.home) + out * 11.0);
                a.yaw = crate::util::yaw_of(out);
                a.vel = Vec2::ZERO;
                a.vy = 0.0;
                a.air = 0.0;
                *vis = Visibility::Inherited;
                fx.burst(
                    Puff::Team(team),
                    tf.translation + Vec3::Y,
                    Vec3::Y * 3.0,
                    24,
                    4.0,
                    1.2,
                    0.25,
                );
                fx.flash(tf.translation + Vec3::Y * 2.0, team.color(), 300_000.0, 16.0, 0.8);
                sfx.write(Sfx::at(Sound::Respawn, tf.translation));
                if team == Team::Blue && !game.demo {
                    log.push("context restored. resuming session.", LOG_WARN);
                }
            }
            continue;
        }
        if !h.alive() {
            a.dead = Some(RESPAWN);
            *vis = Visibility::Hidden;
            game.stats[team.i()].compactions += 1;
            let at = tf.translation + Vec3::Y * 1.4;
            fx.burst(Puff::Glitch, at, Vec3::Y * 2.0, 40, 8.0, 1.2, 0.35);
            fx.burst(Puff::Team(team), at, Vec3::Y * 2.0, 20, 6.0, 1.0, 0.3);
            fx.flash(at, team.color(), 600_000.0, 20.0, 0.8);
            fx.shake(at, 0.6);
            sfx.write(Sfx::at(Sound::Compacted, at));
            if !game.demo {
                if team == Team::Blue {
                    log.push("context window exhausted. compacting...", LOG_BAD);
                } else {
                    log.push(format!("{} ran out of context", team.agent_name()), LOG_WARN);
                    game.announce(format!("{} COMPACTED", team.agent_name()), 2.5, 0x7ee787);
                }
            }
            continue;
        }
        // Income.
        let pos = tf.translation;
        let mut owned = 0;
        let mut near = pos.distance(a.home) < NEAR;
        for (dc, dtf) in &dcs {
            if dc.team == team && dc.online() {
                owned += 1;
                near |= dtf.translation.distance(pos) < NEAR;
            }
        }
        let cpu_bonus = if a.player {
            1.0
        } else {
            match settings.difficulty {
                crate::game::Difficulty::Easy => 0.8,
                crate::game::Difficulty::Normal => 1.0,
                crate::game::Difficulty::Hard => 1.25,
            }
        };
        a.regen = (REGEN_BASE + REGEN_PER_DC * owned as f32 + if near { REGEN_NEAR } else { 0.0 }) * cpu_bonus;
        a.tokens = (a.tokens + a.regen * dt).min(MAX_TOKENS);
        if args.rich && a.player {
            a.tokens = MAX_TOKENS;
        }
        // Context comes back slowly, and quickly at the altar.
        let heal = if pos.distance(a.home) < 16.0 {
            10.0
        } else if h.since_hit > 6.0 {
            1.5
        } else {
            0.0
        };
        h.hp = (h.hp + heal * dt).min(h.max);
    }
}

pub fn animate(
    time: Res<Time>,
    agents: Query<(&Agent, &Health, &Children)>,
    mut bodies: Query<(&mut Transform, &Children), (With<AgentBody>, Without<Orbiter>)>,
    mut orbiters: Query<(&mut Transform, &Orbiter), Without<AgentBody>>,
) {
    let t = time.elapsed_secs();
    for (a, h, children) in &agents {
        for child in children.iter() {
            let Ok((mut tf, kids)) = bodies.get_mut(child) else {
                continue;
            };
            // Hover and sway; lean into movement; rise when casting.
            let bob = (t * 2.2).sin() * 0.08 + 0.35;
            let local = Quat::from_rotation_y(-a.yaw) * Vec3::new(a.vel.x, 0.0, a.vel.y);
            let lean = Quat::from_rotation_x(local.z / SPEED * 0.25) * Quat::from_rotation_z(-local.x / SPEED * 0.2);
            let cast = a.cast_anim;
            tf.translation = Vec3::Y * (bob + cast * 0.25);
            tf.rotation = lean * Quat::from_rotation_x(-cast * 0.12);
            tf.scale = Vec3::splat(if h.flash > 0.0 { 1.06 } else { 1.0 });
            let spin = 1.6 + a.tokens / MAX_TOKENS * 2.5;
            for k in kids.iter() {
                if let Ok((mut otf, orb)) = orbiters.get_mut(k) {
                    let ang = orb.0 + t * spin;
                    let r = 1.05 + cast * 0.5;
                    otf.translation = Vec3::new(ang.cos() * r, 1.5 + (ang * 2.0).sin() * 0.25, ang.sin() * r);
                    otf.rotation = Quat::from_rotation_y(t * 3.0 + orb.0);
                    otf.scale = Vec3::splat(0.6 + a.tokens / MAX_TOKENS * 0.8);
                }
            }
        }
    }
}

/// Passes each agent's orders to its subagents.
pub fn orders(
    mut agents: Query<(&Agent, &mut Intent)>,
    mut units: Query<&mut crate::units::Unit>,
    mut sfx: MessageWriter<Sfx>,
    mut decals: ResMut<crate::decals::Decals>,
) {
    for (a, mut intent) in &mut agents {
        let Some(order) = intent.order.take() else { continue };
        if a.dead.is_some() {
            continue;
        }
        crate::units::give_order(&mut units, a.team, order);
        if a.player {
            sfx.write(Sfx::ui(Sound::Order));
            if let Order::Move(p) = order {
                decals.ring(xz(p), 2.5, 0xf2cc60, 0.9);
                decals.ring(xz(p), 1.2, 0xf2cc60, 0.9);
            }
        }
    }
}

/// Slot `i`'s cost, for affordability checks.
pub fn can_afford(a: &Agent, i: usize) -> bool {
    let s = SLOTS[i];
    a.tokens >= s.tokens() && a.software.len() >= s.software()
}
