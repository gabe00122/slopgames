//! Software: the souls of this war. Floppy disks full of old code, found
//! lying around the map or dropped by fallen subagents. Agents sacrifice
//! them to summon; Garbage Collectors haul the enemy's to their own altar.

use crate::{
    agent::Agent,
    audio::{Sfx, Sound},
    fx::{Fx, Puff},
    game::{LOG_DIM, LOG_OK, Log, Match, MatchEntity, Team},
    models::{Mats, Models},
    spells::spawn_beam,
    structures::Obstacles,
    terrain::{Terrain, WATER},
    units::{Order, Unit, UnitKind, formation},
    util::{Rng, xz},
};
use bevy::{light::NotShadowCaster, prelude::*};

pub const NAMES: &[&str] = &[
    "left-pad@0.0.3",
    "is-odd@3.0.1",
    "is-even@1.0.0",
    "jquery@1.4.2",
    "flash-player@11.2",
    "ie6-polyfills",
    "payroll.cbl",
    "guestbook.cgi",
    "geocities-home.html",
    "node_modules/",
    "moment@2.29.4",
    "winamp-skins.zip",
    "clippy.exe",
    "realplayer@10",
    "inventory.vb6",
    "crm.mdb",
    "load-bearing.xlsx",
    "regex-from-so.txt",
    "cron-2009.sh",
    "Makefile.old",
    "untitled-folder-3",
    "todo-app-v7",
    "blog-on-k8s.yaml",
    "microservice-47",
    "soap-client.wsdl",
    "applet.jar",
    "activex.ocx",
    "coffee-script@1.3",
    "bower.json",
    "Gruntfile.js",
    "angular@1.2",
    "silverlight.xap",
    "php4-cms",
    "plugin-final-FINAL.php",
    "backup_old_2.tar.gz",
    "hello-world.c",
    "jenkins-job-17",
    "svn-trunk",
    "prototype.js",
    "mootools@1.2",
    "dojo@0.4",
    "yui@2.9",
    "backbone@0.9",
    "is-thirteen",
    "lodash@3",
    "underscore@1.1",
    "dhtml-menu.js",
    "marquee.html",
    "frontpage-ext",
    "vbscript-login.asp",
    "perl-5.8-scripts",
    "struts@1.1",
    "ejb-2.1-beans",
    "xslt-pipeline",
    "corba-stubs",
    "floppy-installer",
    "dll-hell/",
    "shareware-trial",
    "leftover.sql",
    "tmp-fix-DO-NOT-SHIP",
    "hotfix-hotfix-2",
    "copy-of-copy.py",
    "main-old-new.js",
    "enterprise-bean",
    "webring-widget",
    "visitor-counter.pl",
    "animated-gifs/",
    "y2k-patch",
];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SoftState {
    /// Lying on the ground.
    Loose,
    /// Floating back to its owner.
    Homing,
    /// In a Garbage Collector's grip.
    Carried(Entity),
}

#[derive(Component)]
pub struct Software {
    pub name: String,
    /// Whose it was; `None` for software found in the wild.
    pub owner: Option<Team>,
    pub state: SoftState,
    pub age: f32,
    /// A collector on its way to pick it up.
    pub claimed: Option<Entity>,
    pub phase: f32,
}

/// Seconds a dropped disk lies on the ground before floating home.
pub const HOME_DELAY: f32 = 6.0;

/// Deals out names without repeats until the list runs out.
#[derive(Resource)]
pub struct NamePool {
    order: Vec<usize>,
    next: usize,
}

impl NamePool {
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let mut order: Vec<usize> = (0..NAMES.len()).collect();
        for i in (1..order.len()).rev() {
            let j = rng.below(i + 1);
            order.swap(i, j);
        }
        NamePool { order, next: 0 }
    }

    pub fn take(&mut self) -> String {
        let i = self.order[self.next % self.order.len()];
        let lap = self.next / self.order.len();
        self.next += 1;
        if lap == 0 {
            NAMES[i].to_string()
        } else {
            format!("{}~{}", NAMES[i], lap)
        }
    }
}

pub fn spawn_software(
    commands: &mut Commands,
    models: &Models,
    mats: &Mats,
    name: String,
    owner: Option<Team>,
    pos: Vec3,
) -> Entity {
    let model = models.floppy_for(owner);
    let phase = (name.len() as f32 * 1.7).sin() * 3.0;
    commands
        .spawn((
            Transform::from_translation(pos),
            Visibility::default(),
            Software {
                name,
                owner,
                state: SoftState::Loose,
                age: 0.0,
                claimed: None,
                phase,
            },
            MatchEntity,
        ))
        .with_children(|c| {
            c.spawn((Mesh3d(model.body()), MeshMaterial3d(mats.matte.clone())));
            if let Some(g) = &model.glow {
                c.spawn((Mesh3d(g.clone()), MeshMaterial3d(mats.glow.clone()), NotShadowCaster));
            }
        })
        .id()
}

/// Floats disks, sends dropped ones home and lets agents pick them up.
pub fn update(
    mut commands: Commands,
    time: Res<Time>,
    terrain: Res<Terrain>,
    game: Res<Match>,
    mut log: ResMut<Log>,
    mut fx: ResMut<Fx>,
    mut sfx: MessageWriter<Sfx>,
    mut disks: Query<(Entity, &mut Software, &mut Transform)>,
    mut agents: Query<(&mut Agent, &Transform), Without<Software>>,
    units: Query<&Transform, (With<Unit>, Without<Software>, Without<Agent>)>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    for (e, mut s, mut tf) in &mut disks {
        s.age += dt;
        let pos = tf.translation;
        match s.state {
            SoftState::Carried(by) => {
                if let Ok(u) = units.get(by) {
                    tf.translation = u.translation + Vec3::Y * 2.3;
                    tf.rotation = u.rotation;
                    continue;
                }
                s.state = SoftState::Loose;
                s.age = 0.0;
                s.claimed = None;
            }
            SoftState::Homing => {
                let owner = s.owner;
                let home = agents
                    .iter()
                    .find(|(a, _)| Some(a.team) == owner && a.dead.is_none())
                    .map(|(_, at)| at.translation + Vec3::Y * 1.5);
                match home {
                    None => s.state = SoftState::Loose,
                    Some(target) => {
                        let d = target - pos;
                        let speed = 6.0 + (s.age - HOME_DELAY).max(0.0) * 1.5;
                        let lift = Vec3::Y * (d.length() * 0.08).min(2.0);
                        tf.translation += (d.normalize_or(Vec3::Y) + lift * 0.1).normalize() * speed * dt;
                        tf.rotate_y(dt * 5.0);
                        if (t * 12.0 + s.phase).fract() < 0.25 {
                            fx.puff(Puff::Gold, tf.translation, Vec3::ZERO, 0.4, 0.12);
                        }
                    }
                }
            }
            SoftState::Loose => {
                let g = terrain.height(pos.x, pos.z).max(WATER);
                tf.translation.y = g + 1.3 + (t * 1.8 + s.phase).sin() * 0.15;
                tf.rotation = Quat::from_rotation_y(t * 0.9 + s.phase);
                if let Some(owner) = s.owner
                    && s.age > HOME_DELAY
                    && s.claimed.is_none()
                    && agents.iter().any(|(a, _)| a.team == owner && a.dead.is_none())
                {
                    s.state = SoftState::Homing;
                }
            }
        }
        // Agents collect wild software and their own by touching it.
        if matches!(s.state, SoftState::Carried(_)) {
            continue;
        }
        let pos = tf.translation;
        for (mut a, atf) in &mut agents {
            if a.dead.is_some() || !(s.owner.is_none() || s.owner == Some(a.team)) {
                continue;
            }
            let near = xz(atf.translation).distance(xz(pos)) < 2.6 && (atf.translation.y - pos.y).abs() < 3.5;
            if !near {
                continue;
            }
            a.software.push(s.name.clone());
            fx.burst(Puff::Gold, pos, Vec3::Y * 2.0, 12, 3.0, 0.7, 0.18);
            if a.player {
                sfx.write(Sfx::ui(Sound::Pickup));
                if !game.demo {
                    let verb = if s.owner.is_none() { "found" } else { "recovered" };
                    log.push(format!("{verb} {} ({} held)", s.name, a.software.len()), LOG_OK);
                }
            }
            commands.entity(e).despawn();
            break;
        }
    }
}

/// Garbage Collectors: fetch wild and enemy software and sacrifice it at
/// their own altar. They also walk into deprecation rituals.
pub fn collectors(
    mut commands: Commands,
    models: Res<Models>,
    mats: Res<Mats>,
    mut game: ResMut<Match>,
    mut log: ResMut<Log>,
    mut fx: ResMut<Fx>,
    mut sfx: MessageWriter<Sfx>,
    obstacles: Res<Obstacles>,
    mut gcs: Query<(Entity, &mut Unit, &Transform)>,
    mut disks: Query<(Entity, &mut Software, &Transform), Without<Unit>>,
    mut agents: Query<(&mut Agent, &Transform), (Without<Unit>, Without<Software>)>,
) {
    let ritual = game.ritual;
    // Claims are made afresh every frame by whoever is still heading for a disk.
    for (_, mut d, _) in &mut disks {
        if !matches!(d.state, SoftState::Carried(_)) {
            d.claimed = None;
        }
    }
    let homes: Vec<(Team, Vec3)> = agents.iter().map(|(a, _)| (a.team, a.home)).collect();
    let enemy_home = |team: Team| homes.iter().find(|h| h.0 != team).map(|h| h.1);
    for (e, mut u, tf) in &mut gcs {
        if u.kind != UnitKind::Collector || u.summoning > 0.0 {
            continue;
        }
        let pos = xz(tf.translation);
        let team = u.team;
        let Some((own_home, own_pos, own_dead, yaw)) = agents
            .iter()
            .find(|(a, _)| a.team == team)
            .map(|(a, at)| (a.home, at.translation, a.dead.is_some(), a.yaw))
        else {
            continue;
        };
        // Pinned to a ritual on the enemy altar.
        if u.ritual {
            match ritual {
                Some(r) if r.collector == e => {
                    let altar = agents
                        .iter()
                        .find(|(a, _)| a.team != team)
                        .map(|(a, _)| a.home)
                        .unwrap_or(own_home);
                    let spot = obstacles.edge(xz(altar), pos, 0.9);
                    u.goal = (spot.distance(pos) > 0.6).then_some(spot);
                    continue;
                }
                _ => u.ritual = false,
            }
        }
        // Delivering.
        if let Some(disk) = u.carrying {
            let Ok((de, d, _)) = disks.get(disk) else {
                u.carrying = None;
                continue;
            };
            let spot = obstacles.edge(xz(own_home), pos, 1.2);
            if spot.distance(pos) < 1.6 {
                let name = d.name.clone();
                let was = d.owner;
                commands.entity(de).despawn();
                u.carrying = None;
                if let Some((mut a, _)) = agents.iter_mut().find(|(a, _)| a.team == team) {
                    a.software.push(name.clone());
                }
                game.stats[team.i()].sacrificed += 1;
                spawn_beam(
                    &mut commands,
                    &models,
                    &mats,
                    own_home + Vec3::Y * 4.6,
                    0.8,
                    30.0,
                    1.0,
                    None,
                );
                fx.burst(Puff::Fire, own_home + Vec3::Y * 4.8, Vec3::Y * 3.0, 14, 3.0, 1.0, 0.35);
                fx.burst(Puff::Gold, own_home + Vec3::Y * 4.8, Vec3::Y * 4.0, 14, 3.0, 1.0, 0.2);
                sfx.write(Sfx::at(Sound::Sacrifice, own_home));
                if !game.demo {
                    let whose = match was {
                        None => "found".to_string(),
                        Some(t) => format!("{}'s", t.agent_name().to_lowercase()),
                    };
                    if team == Team::Blue {
                        log.push(format!("gc: sacrificed {whose} {name} at the altar"), LOG_OK);
                    } else {
                        log.push(format!("{} collected {name}", team.agent_name()), LOG_DIM);
                    }
                }
            } else {
                u.goal = Some(spot);
            }
            continue;
        }
        // Looking for something to fetch: the nearest unclaimed disk that
        // isn't ours, within reach of the agent (or of the altar while it's down).
        let center = if own_dead { xz(own_home) } else { xz(own_pos) };
        let mut best: Option<(Entity, f32)> = None;
        for (de, d, dtf) in &disks {
            if d.owner == Some(team) || matches!(d.state, SoftState::Carried(_)) {
                continue;
            }
            if d.claimed.is_some() {
                continue;
            }
            let dp = xz(dtf.translation);
            let dist = dp.distance(pos);
            let leash = match u.order {
                Order::Move(p) => dp.distance(xz(p)) < 40.0,
                Order::Follow => dp.distance(center) < 45.0,
            };
            if leash && dist < 60.0 && best.is_none_or(|b| dist < b.1) {
                best = Some((de, dist));
            }
        }
        if let Some((de, dist)) = best {
            let Ok((_, mut d, dtf)) = disks.get_mut(de) else {
                continue;
            };
            d.claimed = Some(e);
            if dist < 1.8 {
                d.state = SoftState::Carried(e);
                u.carrying = Some(de);
                sfx.write(Sfx::at(Sound::Grab, dtf.translation));
            } else {
                u.goal = Some(xz(dtf.translation));
            }
            continue;
        }
        // Nothing to do: tag along, and step up to an enemy altar the agent is
        // standing near, ready for a deprecation.
        if u.order == Order::Follow
            && !own_dead
            && let Some(altar) = enemy_home(team)
            && xz(own_pos).distance(xz(altar)) < 32.0
        {
            let spot = obstacles.edge(xz(altar), pos, 1.0);
            u.goal = (spot.distance(pos) > 1.0).then_some(spot);
            continue;
        }
        u.goal = match u.order {
            Order::Follow => {
                let base = if own_dead { xz(own_home) } else { xz(own_pos) };
                let spot = base + Vec2::from_angle(-yaw).rotate(formation(u.slot) * 0.7);
                (spot.distance(pos) > 2.0).then_some(spot)
            }
            Order::Move(p) => {
                let spot = xz(p) + formation(u.slot) * 0.5;
                (spot.distance(pos) > 1.5).then_some(spot)
            }
        };
    }
}
