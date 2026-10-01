//! sudo sacrifice: a Sacrifice-like where coding agents sacrifice old
//! software at their altars to summon subagents, and reshape the ground with
//! spells. Every mesh and sound is generated at startup.

// Bevy systems take many parameters and spell out long query types.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod agent;
mod ai;
mod audio;
mod decals;
mod fx;
mod game;
mod glyphs;
mod hud;
mod menu;
mod meshkit;
mod models;
mod player;
mod software;
mod spells;
mod structures;
mod terrain;
mod units;
mod util;
mod world;

use agent::Agent;
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode, WindowResolution},
};
use game::{Difficulty, Health, Hit, Log, Match, MatchEntity, Radius, Screen, Settings, Target, Targets, Team};

/// Command-line options.
#[derive(Resource, Clone, Default)]
pub struct Args {
    pub mute: bool,
    /// Cheaper rendering: no MSAA or bloom, fewer shadow cascades.
    pub low: bool,
    pub dump_audio: Option<String>,
    pub size: (u32, u32),
    pub fullscreen: bool,
    pub novsync: bool,
    /// Skip the title and start a session.
    pub play: bool,
    /// The CPU plays Blue too.
    pub autopilot: u8,
    pub seed: Option<u64>,
    pub difficulty: Option<Difficulty>,
    /// Game speed multiplier, for testing.
    pub timescale: f32,
    /// Print the session log to stdout.
    pub verbose: bool,
    /// Quit when a session ends (with --play).
    pub exit_on_end: bool,
    /// Save a screenshot to this path, then exit.
    pub shot: Option<String>,
    pub frames: u32,
    /// Moments of match time at which to take screenshots.
    pub times: Vec<f32>,
    /// Real seconds after launch at which to take screenshots.
    pub waits: Vec<f32>,
    /// Fixed camera: position, yaw and pitch.
    pub pose: Option<(Vec3, f32, f32)>,
    /// Select this slot at the start (for screenshots of the aiming marks).
    pub slot: Option<usize>,
    /// The player never runs out of tokens (testing).
    pub rich: bool,
    /// Line up one of each subagent in front of the player, with a passive rival.
    pub gallery: bool,
    /// Take the screenshot when this happens instead: `ritual` or `dead`.
    pub shot_on: Option<String>,
}

fn parse_args() -> Args {
    let mut a = Args {
        frames: 40,
        size: (1600, 900),
        timescale: 1.0,
        ..default()
    };
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    let next = |i: &mut usize| {
        *i += 1;
        argv.get(*i).cloned().unwrap_or_default()
    };
    while i < argv.len() {
        match argv[i].as_str() {
            "--mute" => a.mute = true,
            "--low" => a.low = true,
            "--dump-audio" => a.dump_audio = Some(next(&mut i)),
            "--fullscreen" => a.fullscreen = true,
            "--novsync" => a.novsync = true,
            "--play" => a.play = true,
            "--autopilot" => a.autopilot = 1,
            "--verbose" => a.verbose = true,
            "--rich" => a.rich = true,
            "--gallery" => a.gallery = true,
            "--shot-on" => a.shot_on = Some(next(&mut i)),
            "--exit-on-end" => a.exit_on_end = true,
            "--seed" => a.seed = next(&mut i).parse().ok(),
            "--difficulty" => {
                a.difficulty = match next(&mut i).as_str() {
                    "easy" | "junior" => Some(Difficulty::Easy),
                    "hard" | "staff" => Some(Difficulty::Hard),
                    _ => Some(Difficulty::Normal),
                }
            }
            "--timescale" => a.timescale = next(&mut i).parse().unwrap_or(1.0),
            "--shot" => a.shot = Some(next(&mut i)),
            "--frames" => a.frames = next(&mut i).parse().unwrap_or(40),
            "--time" => a.times = next(&mut i).split(',').filter_map(|x| x.parse().ok()).collect(),
            "--wait" => a.waits = next(&mut i).split(',').filter_map(|x| x.parse().ok()).collect(),
            "--slot" => a.slot = next(&mut i).parse().ok(),
            "--size" => {
                let s = next(&mut i);
                if let Some((w, h)) = s.split_once('x') {
                    a.size = (w.parse().unwrap_or(1600), h.parse().unwrap_or(900));
                }
            }
            "--pose" => {
                let v: Vec<f32> = next(&mut i).split(',').filter_map(|x| x.trim().parse().ok()).collect();
                if v.len() == 5 {
                    a.pose = Some((Vec3::new(v[0], v[1], v[2]), v[3].to_radians(), v[4].to_radians()));
                }
            }
            "-h" | "--help" => {
                println!(
                    "sudo-sacrifice [--fullscreen] [--size WxH] [--mute] [--low]\n\
                     quick start: --play [--difficulty easy|normal|hard] [--seed N]\n\
                     testing: --autopilot --timescale X --verbose --exit-on-end\n\
                     screenshots: --shot out.png [--time T,T..|--wait S,S..] [--frames N] [--pose x,y,z,yaw,pitch] [--slot N] [--novsync]\n\
                     --dump-audio DIR"
                );
                std::process::exit(0);
            }
            other => eprintln!("unknown option {other}"),
        }
        i += 1;
    }
    a
}

fn main() {
    let args = parse_args();
    if let Some(dir) = &args.dump_audio {
        audio::dump(dir);
        return;
    }
    let settings = Settings {
        difficulty: args.difficulty.unwrap_or(Difficulty::Normal),
        seed: args.seed.unwrap_or(1),
    };
    let first = if args.play { Screen::Playing } else { Screen::Title };
    let has_match = resource_exists::<terrain::Terrain>.and_then(resource_exists::<Match>);
    let playing = in_state(Screen::Playing);

    App::new()
        .insert_resource(args.clone())
        .insert_resource(settings)
        .insert_resource(ClearColor(util::hex(0x2a2346)))
        .init_resource::<Targets>()
        .init_resource::<Log>()
        .init_resource::<fx::Fx>()
        .init_resource::<decals::Decals>()
        .init_resource::<terrain::TerrainOps>()
        .init_resource::<spells::TerrainChar>()
        .init_resource::<structures::Obstacles>()
        .init_resource::<player::Look>()
        .init_resource::<player::Control>()
        .init_resource::<menu::Menu>()
        .add_plugins(
            DefaultPlugins
                .set(bevy::log::LogPlugin {
                    filter: std::env::var("SS_LOG").unwrap_or_else(|_| "warn".into()),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "sudo sacrifice".into(),
                        resolution: WindowResolution::new(args.size.0, args.size.1),
                        present_mode: if args.novsync {
                            PresentMode::AutoNoVsync
                        } else {
                            PresentMode::AutoVsync
                        },
                        mode: if args.fullscreen {
                            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
                        } else {
                            WindowMode::Windowed
                        },
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(audio::SoundPlugin)
        .add_message::<Hit>()
        .add_message::<spells::Blast>()
        .insert_state(Screen::Boot)
        .add_systems(
            Startup,
            (
                models::setup_mats,
                terrain::setup_material,
                (
                    models::setup,
                    fx::setup,
                    decals::setup,
                    player::spawn_camera,
                    hud::setup_minimap,
                ),
                move |mut next: ResMut<NextState<Screen>>| next.set(first),
            )
                .chain(),
        )
        .add_systems(OnEnter(Screen::Title), (new_match, menu::title_enter))
        .add_systems(OnEnter(Screen::Playing), (new_match, hud::spawn).chain())
        .add_systems(OnEnter(Screen::Over), menu::over_enter)
        .add_systems(
            Update,
            (
                (
                    player::cursor,
                    menu::title_update.run_if(in_state(Screen::Title)),
                    menu::pause.run_if(playing.clone()),
                    menu::over_update.run_if(in_state(Screen::Over)),
                    fullscreen_toggle,
                ),
                (
                    snapshot,
                    player::input,
                    ai::think,
                    (agent::orders, spells::cast, structures::interact),
                    (structures::update_obstacles, units::think, software::collectors),
                    (units::move_units, agent::move_agents),
                    (units::attack, spells::projectiles, spells::zones),
                    game::apply_hits,
                    (
                        units::deaths,
                        agent::status,
                        structures::datacenters,
                        structures::ritual,
                    ),
                    software::update,
                    (terrain::run_ops, spells::apply_char, terrain::rebuild, world::decor),
                    (
                        units::animate,
                        agent::animate,
                        structures::crystals,
                        structures::altar_fall,
                        world::sky_glyphs,
                        spells::beams,
                        spells::circles,
                        player::aim_marks,
                    ),
                    (decals::update, player::camera, fx::update),
                    (hud::update, hud::world_labels, hud::minimap).run_if(playing.clone()),
                    (hud::age_log, match_flow, echo_log, status_print),
                )
                    .chain()
                    .run_if(has_match),
                screenshot,
            )
                .chain(),
        )
        .run();
}

/// Tears down the old match and builds a new one: a CPU-versus-CPU demo
/// behind the title, or a session for the player.
fn new_match(
    mut commands: Commands,
    screen: Res<State<Screen>>,
    args: Res<Args>,
    mut settings: ResMut<Settings>,
    mut meshes: ResMut<Assets<Mesh>>,
    models: Res<models::Models>,
    mats: Res<models::Mats>,
    tmat: Res<terrain::TerrainMaterial>,
    old: Query<Entity, With<MatchEntity>>,
    (mut ops, mut decals, mut log): (ResMut<terrain::TerrainOps>, ResMut<decals::Decals>, ResMut<Log>),
    mut control: ResMut<player::Control>,
    mut look: ResMut<player::Look>,
    mut time: ResMut<Time<Virtual>>,
    mut cams: Query<&mut player::MainCam>,
    mut demos: Local<u64>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    ops.0.clear();
    decals.clear();
    log.lines.clear();
    let demo = *screen.get() == Screen::Title;
    let seed = if demo {
        *demos += 1;
        settings.seed.wrapping_add(*demos * 7919) ^ 0xde70
    } else {
        settings.seed
    };
    let player = !demo && args.autopilot == 0;
    let agents = world::spawn_world(
        &mut commands,
        &mut meshes,
        &models,
        &mats,
        &tmat,
        seed,
        player,
        args.low,
    );
    for team in Team::BOTH {
        if (team == Team::Red && !(args.gallery && !demo)) || !player {
            commands
                .entity(agents[team.i()])
                .insert(ai::Brain::new(seed ^ ((team.i() as u64 + 1) * 0x9e37)));
        }
    }
    if args.gallery && !demo {
        // One of each, in an arc in front of Blue's starting spot.
        let terrain = world::build_terrain(seed as u32);
        let home = world::altar_pos(Team::Blue);
        let fwd = (-home).normalize();
        let side = Vec2::new(-fwd.y, fwd.x);
        let start = home + fwd * 12.0;
        for (k, kind) in units::UnitKind::ALL.iter().enumerate() {
            let p = start + fwd * 11.0 + side * (k as f32 - 2.0) * 3.6;
            let at = terrain.ground(p);
            let e = units::spawn_unit(
                &mut commands,
                &models,
                &mats,
                *kind,
                Team::Blue,
                at,
                util::yaw_of(-fwd),
                vec!["left-pad@0.0.3".into()],
                units::Order::Move(at),
                k,
                4000 + k as u32,
            );
            let _ = e;
        }
        software::spawn_software(
            &mut commands,
            &models,
            &mats,
            "is-odd@3.0.1".into(),
            None,
            terrain.ground(start + fwd * 6.0 - side * 3.0) + Vec3::Y * 1.3,
        );
    }
    commands.insert_resource(Match::new(demo));
    if !demo {
        let rival = Team::Red.agent_name().to_lowercase();
        log.push(
            format!(
                "$ sudo sacrifice --rival {rival} --level {}",
                settings.difficulty.name().to_lowercase()
            ),
            0xe6edf3,
        );
        log.push("[sudo] password for orchestrator: ********", game::LOG_DIM);
        log.push(
            "find software. provision datacenters. deprecate their altar.",
            game::LOG_WARN,
        );
    }
    // Face the enemy altar.
    let dir = -world::altar_pos(Team::Blue);
    look.yaw = util::yaw_of(dir);
    look.pitch = 0.2;
    control.paused = false;
    time.unpause();
    time.set_relative_speed(args.timescale);
    for mut c in &mut cams {
        c.live = false;
    }
    // Keep the next session's map different from this one's.
    if !demo {
        settings.seed = settings.seed.wrapping_add(1);
    }
}

/// Gathers everything that can be attacked into [`Targets`].
fn snapshot(
    mut targets: ResMut<Targets>,
    agents: Query<(Entity, &Agent, &Transform, &Health, &Radius)>,
    units: Query<(Entity, &units::Unit, &Transform, &Health, &Radius)>,
    dcs: Query<(Entity, &structures::Datacenter, &Transform, &Health, &Radius)>,
) {
    targets.0.clear();
    for (e, a, tf, h, r) in &agents {
        if a.dead.is_none() && h.alive() {
            targets.0.push(Target {
                entity: e,
                team: a.team,
                pos: tf.translation,
                radius: r.0,
                kind: game::Kind::Agent,
                flying: false,
            });
        }
    }
    for (e, u, tf, h, r) in &units {
        if u.summoning <= 0.0 && h.alive() {
            targets.0.push(Target {
                entity: e,
                team: u.team,
                pos: tf.translation,
                radius: r.0,
                kind: game::Kind::Unit(u.kind),
                flying: u.kind.def().fly > 0.0,
            });
        }
    }
    for (e, d, tf, h, r) in &dcs {
        if h.alive() {
            targets.0.push(Target {
                entity: e,
                team: d.team,
                pos: tf.translation + Vec3::Y * 1.5,
                radius: r.0,
                kind: game::Kind::Datacenter,
                flying: false,
            });
        }
    }
}

/// The match clock, and what happens when a match is decided.
fn match_flow(
    time: Res<Time>,
    real: Res<Time<Real>>,
    args: Res<Args>,
    screen: Res<State<Screen>>,
    mut game: ResMut<Match>,
    mut next: ResMut<NextState<Screen>>,
    mut exit: MessageWriter<AppExit>,
    mut decided: Local<Option<f32>>,
) {
    if game.live() {
        game.time += time.delta_secs();
        *decided = None;
        return;
    }
    let since = decided.get_or_insert(real.elapsed_secs());
    let wait = real.elapsed_secs() - *since;
    match screen.get() {
        Screen::Playing if wait > 3.0 => {
            if args.exit_on_end {
                println!(
                    "winner: {:?} after {}",
                    game.winner.unwrap_or(Team::Blue),
                    util::format_time(game.time)
                );
                exit.write(AppExit::Success);
            } else {
                next.set(Screen::Over);
            }
        }
        Screen::Title if wait > 8.0 => next.set(Screen::Title),
        _ => {}
    }
}

/// `--verbose`: prints the session log as it grows.
fn echo_log(args: Res<Args>, log: Res<Log>, game: Res<Match>, mut printed: Local<usize>) {
    if !args.verbose {
        return;
    }
    if log.total < *printed {
        *printed = 0;
    }
    let fresh = (log.total - *printed).min(log.lines.len());
    for line in &log.lines[log.lines.len() - fresh..] {
        println!("[{}] {}", util::format_time(game.time), line.text);
    }
    *printed = log.total;
}

/// `--verbose`: a status line per agent every 20 s of match time.
fn status_print(
    args: Res<Args>,
    game: Res<Match>,
    agents: Query<(&Agent, &Health, &Transform, Option<&ai::Brain>)>,
    units: Query<&units::Unit>,
    dcs: Query<&structures::Datacenter>,
    mut next: Local<f32>,
) {
    if !args.verbose || game.time < *next {
        return;
    }
    *next = game.time + 20.0;
    for (a, h, tf, brain) in &agents {
        let mut counts = [0; 5];
        for u in units.iter().filter(|u| u.team == a.team) {
            counts[u.kind.index()] += 1;
        }
        let dc = dcs.iter().filter(|d| d.team == a.team).count();
        println!(
            "  [{}] {:?} goal={:?} hp={:.0} tokens={:.0} sw={} units(L,T,F,M,G)={:?} dc={} pos=({:.0},{:.0}){}",
            util::format_time(game.time),
            a.team,
            brain.map(|b| b.goal),
            h.hp,
            a.tokens,
            a.software.len(),
            counts,
            dc,
            tf.translation.x,
            tf.translation.z,
            if a.dead.is_some() { " DEAD" } else { "" }
        );
    }
}

fn fullscreen_toggle(keys: Res<ButtonInput<KeyCode>>, mut window: Single<&mut Window, With<PrimaryWindow>>) {
    if keys.just_pressed(KeyCode::F11) {
        window.mode = match window.mode {
            WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            _ => WindowMode::Windowed,
        };
    }
}

/// `--shot`: waits for the requested moment, saves a screenshot and exits.
/// With several moments it saves one file per moment, numbered.
fn screenshot(
    mut commands: Commands,
    args: Res<Args>,
    real: Res<Time<Real>>,
    game: Option<Res<Match>>,
    mut agents: Query<&mut Agent>,
    mut seen: Local<Option<f32>>,
    mut frame: Local<u32>,
    mut shot: Local<usize>,
    mut timing: Local<(f32, u32)>,
    mut taken: Local<Option<f32>>,
    mut exit: MessageWriter<AppExit>,
) {
    if let Some(slot) = args.slot {
        for mut a in &mut agents {
            if a.player {
                a.selected = slot.min(9);
            }
        }
    }
    let Some(path) = &args.shot else { return };
    if let Some(at) = *taken {
        if real.elapsed_secs() - at > 0.8 {
            exit.write(AppExit::Success);
        }
        return;
    }
    // Waiting for an event: shoot a moment after it starts.
    if let Some(event) = &args.shot_on {
        let happening = match event.as_str() {
            "ritual" => game.as_ref().is_some_and(|g| g.ritual.is_some()),
            "dead" => agents.iter().any(|a| a.team == Team::Blue && a.dead.is_some()),
            _ => true,
        };
        if seen.is_none() {
            if !happening {
                return;
            }
            *seen = Some(real.elapsed_secs());
        }
        if seen.is_some_and(|t| real.elapsed_secs() - t < 1.0) {
            return;
        }
    }
    let count = args.times.len().max(args.waits.len()).max(1);
    if let Some(&at) = args.waits.get(*shot)
        && real.elapsed_secs() < at
    {
        return;
    }
    if let Some(&target) = args.times.get(*shot)
        && *frame == 0
    {
        let now = game.as_ref().map_or(0.0, |g| g.time);
        if now < target {
            return;
        }
    }
    *frame += 1;
    if *frame > args.frames / 2 && *frame < args.frames {
        timing.0 += real.delta_secs();
        timing.1 += 1;
    }
    if *frame < args.frames {
        return;
    }
    let last = *shot + 1 >= count;
    let file = if count > 1 {
        path.replace(".png", &format!("_{}.png", *shot))
    } else {
        path.clone()
    };
    commands.spawn(Screenshot::primary_window()).observe(save_to_disk(file));
    if last {
        if timing.1 > 0 {
            let ms = timing.0 / timing.1 as f32 * 1000.0;
            println!("frame time: {ms:.2} ms ({:.0} fps)", 1000.0 / ms);
        }
        *taken = Some(real.elapsed_secs());
    } else {
        *shot += 1;
        *frame = 0;
    }
}
