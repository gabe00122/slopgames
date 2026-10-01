//! The CPU agent. It thinks a few times a second: stay alive, keep summoning,
//! claim compute wells, pick up software, fight what comes near, and when the
//! army is big enough, march on the enemy's datacenters and then its altar.
//! It steers the same [`Intent`] the player's input does.

use crate::{
    agent::{Agent, Intent},
    game::{Difficulty, Health, Kind, Match, SLOTS, Settings, Slot, Targets, Team},
    software::{SoftState, Software},
    spells::{Spell, wall_ends},
    structures::{
        ALTAR_RADIUS, DATACENTER_COST, DEPRECATE_COST, Datacenter, GC_REACH, Interaction, Obstacles, Well, interaction,
    },
    terrain::Terrain,
    units::{Order, Unit, UnitKind, steer},
    util::{Rng, xz, yaw_of},
};
use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Goal {
    Idle,
    Collect(Entity),
    Build(Entity),
    Fight,
    Retreat,
    Push,
    Defend,
}

#[derive(Component)]
pub struct Brain {
    pub think: f32,
    pub goal: Goal,
    pub dest: Option<Vec2>,
    /// What to look at and shoot at.
    pub focus: Option<Vec3>,
    rng: Rng,
    strafe: f32,
    strafe_t: f32,
    order: Option<Order>,
    order_t: f32,
    stuck_at: Vec2,
    stuck_t: f32,
    detour: f32,
    detour_t: f32,
    pushing: bool,
    /// The next subagent to summon, once it's affordable.
    plan: Option<UnitKind>,
}

impl Brain {
    pub fn new(seed: u64) -> Self {
        Brain {
            think: 0.5,
            goal: Goal::Idle,
            dest: None,
            focus: None,
            rng: Rng::new(seed),
            strafe: 1.0,
            strafe_t: 0.0,
            order: None,
            order_t: 0.0,
            stuck_at: Vec2::ZERO,
            stuck_t: 0.0,
            detour: 1.0,
            detour_t: 0.0,
            pushing: false,
            plan: None,
        }
    }
}

struct Tuning {
    think: f32,
    aim_err: f32,
    eager: f32,
    push_at: f32,
    /// No marching on the enemy before this much match time.
    first_push: f32,
}

fn tuning(d: Difficulty) -> Tuning {
    match d {
        Difficulty::Easy => Tuning {
            think: 0.9,
            aim_err: 3.5,
            eager: 0.4,
            push_at: 11.0,
            first_push: 240.0,
        },
        Difficulty::Normal => Tuning {
            think: 0.55,
            aim_err: 1.8,
            eager: 0.7,
            push_at: 8.0,
            first_push: 120.0,
        },
        Difficulty::Hard => Tuning {
            think: 0.35,
            aim_err: 0.7,
            eager: 0.95,
            push_at: 7.0,
            first_push: 0.0,
        },
    }
}

fn slot_of(s: Slot) -> usize {
    SLOTS.iter().position(|x| *x == s).unwrap_or(0)
}

/// How much a subagent counts toward an army.
fn worth(k: UnitKind) -> f32 {
    match k {
        UnitKind::Linter => 1.0,
        UnitKind::TestRunner => 1.3,
        UnitKind::Fuzzer => 2.0,
        UnitKind::Monolith => 3.5,
        UnitKind::Collector => 0.0,
    }
}

pub fn think(
    time: Res<Time>,
    game: Res<Match>,
    settings: Res<Settings>,
    terrain: Res<Terrain>,
    targets: Res<Targets>,
    obstacles: Res<Obstacles>,
    mut agents: Query<(Entity, &Agent, &mut Intent, &Transform, &Health, Option<&mut Brain>)>,
    units: Query<(Entity, &Unit, &Transform, &Health)>,
    wells: Query<(Entity, &Well, &Transform)>,
    dcs: Query<(&Datacenter, &Transform, &Health)>,
    disks: Query<(Entity, &Software, &Transform)>,
) {
    let dt = time.delta_secs();
    let tune = tuning(settings.difficulty);
    let infos: Vec<(Team, Vec3, f32, bool, Vec3)> = agents
        .iter()
        .map(|(_, a, _, t, h, _)| (a.team, t.translation, h.frac(), a.dead.is_none(), a.home))
        .collect();
    for (me, agent, mut intent, tf, health, brain) in &mut agents {
        let Some(mut b) = brain else { continue };
        if agent.dead.is_some() || !game.live() {
            intent.mv = Vec2::ZERO;
            b.dest = None;
            continue;
        }
        let team = agent.team;
        let pos = tf.translation;
        let p2 = xz(pos);
        let Some(&(_, enemy_pos, enemy_hp, enemy_alive, enemy_home)) = infos.iter().find(|i| i.0 != team) else {
            continue;
        };
        b.think -= dt;
        b.order_t -= dt;
        if b.think <= 0.0 {
            b.think = tune.think * b.rng.range(0.8, 1.2);
            decide(
                &mut b,
                me,
                agent,
                &mut intent,
                pos,
                health,
                (enemy_pos, enemy_hp, enemy_alive, enemy_home),
                &game,
                &tune,
                &terrain,
                &targets,
                &obstacles,
                &units,
                &wells,
                &dcs,
                &disks,
            );
        }
        // Every frame: walk toward the destination, face the focus.
        let mut mv = Vec2::ZERO;
        if let Some(dest) = b.dest {
            let to = dest - p2;
            let d = to.length();
            if d > 1.2 {
                b.detour_t -= dt;
                let (dir, turned) = steer(&terrain, p2, to / d, 3.0, 1.4, b.detour);
                if turned && b.detour_t <= 0.0 {
                    b.detour = if b.rng.chance(0.5) { 1.0 } else { -1.0 };
                    b.detour_t = 2.0;
                }
                mv = dir * (d / 3.0).min(1.0);
            }
        }
        if b.goal == Goal::Fight || b.goal == Goal::Defend {
            b.strafe_t -= dt;
            if b.strafe_t <= 0.0 {
                b.strafe_t = b.rng.range(0.8, 2.0);
                b.strafe = if b.rng.chance(0.5) { 1.0 } else { -1.0 };
            }
            if let Some(f) = b.focus {
                let to = (xz(f) - p2).normalize_or(Vec2::X);
                mv += Vec2::new(-to.y, to.x) * b.strafe * 0.6;
            }
        }
        // Unstick: if it wants to move but isn't, hop and try the other way round.
        let want = mv.length() > 0.3;
        if want && p2.distance(b.stuck_at) < 0.4 {
            b.stuck_t += dt;
        } else {
            b.stuck_t = 0.0;
            b.stuck_at = p2;
        }
        intent.jump = b.stuck_t > 0.7;
        if b.stuck_t > 1.2 {
            b.detour = -b.detour;
            b.detour_t = 2.5;
            b.stuck_t = 0.0;
        }
        intent.mv = mv.clamp_length_max(1.0);
        intent.snap = false;
        intent.face = match b.focus {
            Some(f) if f.distance(pos) < 70.0 => Some(yaw_of(xz(f) - p2)),
            _ if mv.length() > 0.1 => Some(yaw_of(mv)),
            _ => None,
        };
        intent.aim = b.focus.unwrap_or(pos);
    }
}

#[allow(clippy::too_many_arguments)]
fn decide(
    b: &mut Brain,
    me: Entity,
    agent: &Agent,
    intent: &mut Intent,
    pos: Vec3,
    health: &Health,
    enemy: (Vec3, f32, bool, Vec3),
    game: &Match,
    tune: &Tuning,
    terrain: &Terrain,
    targets: &Targets,
    obstacles: &Obstacles,
    units: &Query<(Entity, &Unit, &Transform, &Health)>,
    wells: &Query<(Entity, &Well, &Transform)>,
    dcs: &Query<(&Datacenter, &Transform, &Health)>,
    disks: &Query<(Entity, &Software, &Transform)>,
) {
    let (enemy_pos, enemy_hp, enemy_alive, enemy_home) = enemy;
    let team = agent.team;
    let p2 = xz(pos);
    let home = agent.home;
    let hp = health.frac();

    // --- The situation ---
    let mine: Vec<(Entity, &Unit, Vec3, f32)> = units
        .iter()
        .filter(|(_, u, _, _)| u.team == team)
        .map(|(e, u, t, h)| (e, u, t.translation, h.frac()))
        .collect();
    let army: f32 = mine.iter().map(|m| worth(m.1.kind)).sum();
    let collectors = mine.iter().filter(|m| m.1.kind == UnitKind::Collector).count();
    let threats: Vec<&crate::game::Target> = targets.enemies_near(team, pos, 34.0).collect();
    let threat_worth: f32 = threats
        .iter()
        .map(|t| match t.kind {
            Kind::Unit(k) => worth(k),
            Kind::Agent => 3.0,
            Kind::Datacenter => 0.0,
        })
        .sum();
    let enemy_near = enemy_alive && enemy_pos.distance(pos) < 42.0;
    let enemy_ritual = game.ritual.is_some_and(|r| r.by != team);
    let my_ritual = game.ritual.is_some_and(|r| r.by == team);

    // --- Pick a goal ---
    let wells_list: Vec<(Entity, bool, Vec3)> = wells
        .iter()
        .map(|(e, w, t)| (e, w.datacenter.is_none(), t.translation))
        .collect();
    let gcs: Vec<(Entity, Team, Vec3)> = mine
        .iter()
        .filter(|m| m.1.kind == UnitKind::Collector && m.1.summoning <= 0.0)
        .map(|m| (m.0, team, m.2))
        .collect();
    let can = interaction(agent, pos, game, &wells_list, enemy_home, &gcs);

    b.goal = if enemy_ritual {
        Goal::Defend
    } else if hp < 0.3 && (enemy_near || threat_worth > 2.0) {
        Goal::Retreat
    } else if my_ritual {
        Goal::Push
    } else if enemy_near || threat_worth > 1.5 {
        Goal::Fight
    } else if game.time >= tune.first_push
        && (b.pushing
            || army >= tune.push_at
            || (game.time > 300.0 && army >= tune.push_at * 0.6)
            || (agent.tokens > 240.0 && agent.software.is_empty() && army >= 3.0))
    {
        Goal::Push
    } else {
        Goal::Idle
    };
    // Pushes end when the army is spent.
    b.pushing = matches!(b.goal, Goal::Push | Goal::Fight) && army >= tune.push_at * 0.45 && !enemy_ritual;
    if b.goal == Goal::Push {
        b.pushing = true;
    }

    // Economy when nothing is pressing.
    if b.goal == Goal::Idle {
        if let Some((we, _, wp)) = wells_list
            .iter()
            .filter(|(_, free, wp)| {
                *free && targets.enemies_near(team, *wp, 22.0).next().is_none() && xz(*wp).distance(p2) < 110.0
            })
            .min_by(|a, b2| {
                let score = |w: &(Entity, bool, Vec3)| w.2.distance(pos) + 0.6 * w.2.distance(home);
                score(a).total_cmp(&score(b2))
            })
        {
            b.goal = Goal::Build(*we);
            b.dest = Some(xz(*wp));
            if matches!(can, Interaction::Build(e) if e == *we) {
                intent.interact = true;
            }
        } else if let Some((de, dp)) = disks
            .iter()
            .filter(|(_, s, _)| s.owner.is_none() && s.state == SoftState::Loose)
            .map(|(e, _, t)| (e, t.translation))
            .filter(|(_, t)| t.distance(pos) < 60.0)
            .min_by(|a, c| a.1.distance(pos).total_cmp(&c.1.distance(pos)))
        {
            b.goal = Goal::Collect(de);
            b.dest = Some(xz(dp));
        }
    }

    b.focus = None;
    match b.goal {
        Goal::Defend => {
            let gc = game.ritual.and_then(|r| targets.get(r.collector)).map(|t| t.pos);
            b.dest = Some(obstacles.edge(xz(home), p2, 6.0));
            b.focus = gc.or(threats.first().map(|t| t.pos));
            order(b, intent, Order::Move(home), 0.0);
        }
        Goal::Retreat => {
            b.dest = Some(obstacles.edge(xz(home), p2, 4.0));
            order(b, intent, Order::Follow, 0.0);
            b.focus = threats.first().map(|t| t.pos);
        }
        Goal::Fight => {
            // Hold a comfortable range from the enemy agent; otherwise close in on the nearest threat.
            let focus = if enemy_near {
                enemy_pos
            } else {
                threats.first().map_or(enemy_pos, |t| t.pos)
            };
            b.focus = Some(focus);
            let d = xz(focus).distance(p2);
            let away = (p2 - xz(focus)).normalize_or(Vec2::X);
            b.dest = Some(if d > 26.0 {
                xz(focus) + away * 22.0
            } else if d < 14.0 {
                p2 + away * 8.0
            } else {
                p2
            });
            let theirs = threat_worth.max(0.5);
            if army >= theirs * 0.8 {
                order(b, intent, Order::Move(focus), 10.0);
            } else {
                order(b, intent, Order::Follow, 0.0);
            }
        }
        Goal::Push => {
            // Datacenters first, then the altar.
            let dc = dcs
                .iter()
                .filter(|(d, _, h)| d.team != team && h.alive())
                .map(|(_, t, _)| t.translation)
                .min_by(|a, c| a.distance(pos).total_cmp(&c.distance(pos)));
            let objective = match dc {
                Some(d) if d.distance(pos) < 90.0 || d.distance(enemy_home) > 40.0 => d,
                _ => enemy_home,
            };
            let at_altar = objective == enemy_home;
            let stand = if at_altar {
                obstacles.edge(xz(enemy_home), p2, 5.0)
            } else {
                xz(objective) + (p2 - xz(objective)).normalize_or(Vec2::X) * 9.0
            };
            b.dest = Some(stand);
            b.focus = threats.first().map(|t| t.pos).or(Some(objective + Vec3::Y * 2.0));
            order(b, intent, Order::Move(objective), 10.0);
            if at_altar && !my_ritual {
                match &can {
                    Interaction::Deprecate(_) => intent.interact = true,
                    _ => {
                        // Make sure a collector comes along.
                        let gc_close = gcs.iter().any(|g| xz(g.2).distance(xz(enemy_home)) < GC_REACH);
                        if !gc_close && collectors == 0 && agent.tokens >= 40.0 && !agent.software.is_empty() {
                            intent.cast = Some((slot_of(Slot::Summon(UnitKind::Collector)), pos));
                        }
                    }
                }
            }
            let _ = (DEPRECATE_COST, ALTAR_RADIUS);
        }
        Goal::Idle => {
            // Loiter between home and the middle, near our own datacenters.
            let anchor = dcs
                .iter()
                .filter(|(d, _, _)| d.team == team)
                .map(|(_, t, _)| xz(t.translation))
                .min_by(|a, c| a.distance(p2).total_cmp(&c.distance(p2)))
                .unwrap_or(xz(home) * 0.6);
            if b.dest.is_none_or(|d| d.distance(p2) < 3.0) {
                b.dest = Some(anchor + b.rng.disc(12.0));
            }
            order(b, intent, Order::Follow, 0.0);
        }
        Goal::Collect(_) | Goal::Build(_) => {
            order(b, intent, Order::Follow, 0.0);
        }
    }

    // --- Summon, saving up for whatever matters more ---
    let sw = agent.software.len();
    let near_altar = xz(enemy_home).distance(p2) < 40.0;
    let gc_along = gcs.iter().any(|g| xz(g.2).distance(p2) < 35.0);
    let saving = if near_altar && !my_ritual && gc_along && !enemy_ritual {
        DEPRECATE_COST
    } else if matches!(b.goal, Goal::Build(_)) {
        DATACENTER_COST
    } else {
        0.0
    };
    let pressed = matches!(b.goal, Goal::Fight | Goal::Defend | Goal::Retreat);
    let reserve = saving + if pressed { 10.0 } else { 0.0 };
    // Under pressure, don't wait on something expensive: anything now beats a Monolith later.
    if b.plan
        .is_some_and(|k| k.def().software > sw || (pressed && k.def().tokens > agent.tokens + 15.0))
    {
        b.plan = None;
    }
    if b.plan.is_none() && sw > 0 {
        let want_gc = collectors == 0 || (collectors < 2 && army > 10.0);
        b.plan = Some(if want_gc {
            UnitKind::Collector
        } else {
            let options = [
                (UnitKind::Linter, 3.0),
                (UnitKind::TestRunner, 3.0),
                (UnitKind::Fuzzer, 2.0),
                (UnitKind::Monolith, 1.5),
            ];
            let allowed: Vec<(UnitKind, f32)> = options
                .into_iter()
                .filter(|(k, _)| k.def().software <= sw && !(pressed && k.def().tokens > agent.tokens + 15.0))
                .collect();
            let total: f32 = allowed.iter().map(|o| o.1).sum();
            let mut roll = b.rng.f() * total;
            let mut pick = UnitKind::Linter;
            for (k, w) in allowed {
                if roll < w {
                    pick = k;
                    break;
                }
                roll -= w;
            }
            pick
        });
    }
    let plan_cost = b.plan.map_or(0.0, |k| k.def().tokens);
    // Stopping a deprecation beats everything: hit the collector with whatever is ready.
    if enemy_ritual && let Some(gc) = game.ritual.and_then(|r| targets.get(r.collector)) {
        let now = |s: Spell| agent.cooldowns[slot_of(Slot::Spell(s))] <= 0.0 && agent.tokens >= s.def().cost;
        let d = gc.pos.distance(pos);
        let spell = if now(Spell::ForcePush) && d < 70.0 {
            Some(Spell::ForcePush)
        } else if now(Spell::ForkBomb) && d < 55.0 {
            Some(Spell::ForkBomb)
        } else if now(Spell::Segfault) && d < 60.0 {
            Some(Spell::Segfault)
        } else {
            None
        };
        if let Some(s) = spell {
            let err = b.rng.disc(tune.aim_err * 0.5);
            intent.cast = Some((slot_of(Slot::Spell(s)), terrain.ground(xz(gc.pos) + err)));
            return;
        }
    }
    // Deprecate as soon as it's possible.
    if matches!(can, Interaction::Deprecate(_)) && !matches!(b.goal, Goal::Retreat | Goal::Defend) {
        intent.interact = true;
    }
    if let Some(k) = b.plan
        && agent.tokens >= plan_cost + reserve
        && intent.cast.is_none()
    {
        intent.cast = Some((slot_of(Slot::Summon(k)), pos));
        b.plan = None;
        return;
    }

    // --- Spells ---
    if intent.cast.is_some() || !b.rng.chance(tune.eager) {
        return;
    }
    // Keep enough back for the next summon, unless in real trouble.
    // Under pressure a new subagent is worth more than a spell.
    let desperate = matches!(b.goal, Goal::Defend | Goal::Retreat);
    let hold = if sw > 0 {
        plan_cost * if desperate { 1.0 } else { 0.6 }
    } else {
        0.0
    };
    let budget = agent.tokens - reserve - hold;
    let ready = |s: Spell| {
        let i = slot_of(Slot::Spell(s));
        agent.cooldowns[i] <= 0.0 && budget >= s.def().cost
    };
    let cast = |s: Spell, at: Vec3, b: &mut Brain, intent: &mut Intent| {
        let err = b.rng.disc(tune.aim_err);
        let at = terrain.ground(xz(at) + err);
        intent.cast = Some((slot_of(Slot::Spell(s)), at));
    };
    let hurt_friends = mine.iter().filter(|m| m.3 < 0.6 && m.2.distance(pos) < 14.0).count();
    let hotfix_ok = agent.cooldowns[slot_of(Slot::Spell(Spell::Hotfix))] <= 0.0 && agent.tokens >= 40.0;
    if hotfix_ok && (hp < 0.5 || (hurt_friends >= 3 && ready(Spell::Hotfix))) {
        cast(Spell::Hotfix, pos, b, intent);
        return;
    }
    // The biggest cluster of enemies near the focus.
    let cluster = |r: f32| -> Option<(Vec3, usize)> {
        threats
            .iter()
            .map(|t| {
                let n = threats.iter().filter(|o| o.pos.distance(t.pos) < r).count();
                (t.pos, n)
            })
            .max_by_key(|c| c.1)
    };
    if let Some((c, n)) = cluster(9.0)
        && ready(Spell::ForcePush)
        && c.distance(pos) > 12.0
        && c.distance(pos) < 65.0
        && (n >= 4 || (enemy_alive && enemy_hp < 0.35 && enemy_pos.distance(c) < 6.0))
    {
        cast(Spell::ForcePush, c, b, intent);
        return;
    }
    if let Some((c, n)) = cluster(10.0)
        && ready(Spell::ForkBomb)
        && n >= 3
        && c.distance(pos) > 11.0
        && c.distance(pos) < 50.0
    {
        cast(Spell::ForkBomb, c, b, intent);
        return;
    }
    // A wall between us and a charge we can't take.
    let charging = threats
        .iter()
        .filter(|t| matches!(t.kind, Kind::Unit(UnitKind::Linter | UnitKind::Monolith)) && t.pos.distance(pos) < 16.0)
        .count();
    if ready(Spell::Firewall) && charging >= 3 && army < threat_worth {
        let c = threats[0].pos;
        let mid = pos.lerp(c, 0.5);
        let (a, e) = wall_ends(pos, mid);
        if a.distance(p2) > 3.0 && e.distance(p2) > 3.0 {
            cast(Spell::Firewall, mid, b, intent);
            return;
        }
    }
    // Pot shots at the enemy agent, or at whatever is closest.
    if ready(Spell::Segfault) {
        let target = if enemy_near && enemy_pos.distance(pos) < 55.0 {
            Some(enemy_pos)
        } else {
            threats
                .iter()
                .filter(|t| t.pos.distance(pos) < 50.0)
                .min_by(|a, c| a.pos.distance(pos).total_cmp(&c.pos.distance(pos)))
                .map(|t| t.pos)
        };
        if let Some(t) = target
            && terrain.line_of_sight(pos + Vec3::Y * 2.0, t + Vec3::Y)
        {
            cast(Spell::Segfault, t, b, intent);
        }
    }
    let _ = me;
}

/// Issues an order to the subagents unless it's the one already standing.
/// `slack` is how far a move order's point may drift before it's reissued.
fn order(b: &mut Brain, intent: &mut Intent, o: Order, slack: f32) {
    let same = match (b.order, o) {
        (Some(Order::Follow), Order::Follow) => true,
        (Some(Order::Move(a)), Order::Move(c)) => a.distance(c) <= slack,
        _ => false,
    };
    if same && b.order_t > 0.0 {
        return;
    }
    if !same || b.order_t <= 0.0 {
        b.order = Some(o);
        b.order_t = 6.0;
        intent.order = Some(o);
    }
}
