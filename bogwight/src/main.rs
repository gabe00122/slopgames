//! BOGWIGHT: a physics-driven side-scroller. You are the thing in the swamp;
//! the monster hunters who followed you in are about to regret it. Every
//! mesh and sound is generated at startup; physics is Avian 2D on the
//! z = 0 plane, rendered in 3D so lanterns, lightning and water can light
//! the scene for real.

// Bevy systems take many parameters and spell out long query types.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod audio;
mod autopilot;
mod boats;
mod camera;
mod fx;
mod game;
mod hud;
mod hunters;
mod input;
mod level;
mod menu;
mod meshkit;
mod models;
mod player;
mod props;
mod util;
mod water;
mod weather;
mod world;

use avian2d::prelude::*;
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode, WindowResolution},
};
use game::*;

/// Command-line options.
#[derive(Resource, Clone, Default)]
pub struct Args {
    pub mute: bool,
    /// Cheaper rendering: no MSAA, bloom or lantern shadows, less rain.
    pub low: bool,
    pub dump_audio: Option<String>,
    pub size: (u32, u32),
    pub fullscreen: bool,
    pub novsync: bool,
    /// Skip the title and start a night.
    pub play: bool,
    pub night: u32,
    pub seed: Option<u64>,
    /// Start the bogwight at this x.
    pub start: Option<f32>,
    /// The bogwight can't be hurt.
    pub god: bool,
    /// A crude bot plays (for testing).
    pub autopilot: bool,
    /// Hang off the nearest boat and pull it over (testing).
    pub grip_test: bool,
    pub timescale: f32,
    pub verbose: bool,
    pub exit_on_end: bool,
    /// Save a screenshot to this path, then exit.
    pub shot: Option<String>,
    pub frames: u32,
    /// Moments of play time at which to take screenshots.
    pub times: Vec<f32>,
    /// Real seconds after launch at which to take screenshots.
    pub waits: Vec<f32>,
    /// Take the screenshot when this happens: kill, spotted, flip, flash, dead, won.
    pub shot_on: Option<String>,
}

fn parse_args() -> Args {
    let mut a = Args {
        frames: 30,
        size: (1600, 900),
        timescale: 1.0,
        night: 1,
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
            "--night" => a.night = next(&mut i).parse().unwrap_or(1).max(1),
            "--seed" => a.seed = next(&mut i).parse().ok(),
            "--start" => a.start = next(&mut i).parse().ok(),
            "--god" => a.god = true,
            "--autopilot" => a.autopilot = true,
            "--grip-test" => a.grip_test = true,
            "--verbose" => a.verbose = true,
            "--exit-on-end" => a.exit_on_end = true,
            "--timescale" => a.timescale = next(&mut i).parse().unwrap_or(1.0),
            "--shot" => a.shot = Some(next(&mut i)),
            "--shot-on" => a.shot_on = Some(next(&mut i)),
            "--frames" => a.frames = next(&mut i).parse().unwrap_or(30),
            "--time" => a.times = next(&mut i).split(',').filter_map(|x| x.parse().ok()).collect(),
            "--wait" => a.waits = next(&mut i).split(',').filter_map(|x| x.parse().ok()).collect(),
            "--size" => {
                let s = next(&mut i);
                if let Some((w, h)) = s.split_once('x') {
                    a.size = (w.parse().unwrap_or(1600), h.parse().unwrap_or(900));
                }
            }
            "-h" | "--help" => {
                println!(
                    "bogwight [--fullscreen] [--size WxH] [--mute] [--low]\n\
                     quick start: --play [--night N] [--seed N]\n\
                     testing: --start X --god --autopilot --timescale X --verbose --exit-on-end\n\
                     screenshots: --shot out.png [--time T,T..|--wait S,S..|--shot-on kill|spotted|flip|flash|dead|won|over] [--frames N] [--novsync]\n\
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
    let first = if args.play { Screen::Playing } else { Screen::Title };
    let level_ready = resource_exists::<world::LevelInfo>;
    let playing = in_state(Screen::Playing);
    let session = Session {
        night: args.night,
        seed: args.seed.unwrap_or(0xb09_5eed),
        time: 0.0,
        outcome: None,
        hunters_total: 0,
        hunters_left: 0,
        stats: Stats::default(),
        demo: true,
    };

    App::new()
        .insert_resource(args.clone())
        .insert_resource(session)
        .insert_resource(ClearColor(Color::srgb(0.055, 0.075, 0.085)))
        .insert_resource(Gravity(Vec2::NEG_Y * GRAVITY))
        .insert_resource(SubstepCount(8))
        .init_resource::<input::Controls>()
        .init_resource::<player::Stealth>()
        .init_resource::<weather::Storm>()
        .init_resource::<fx::Fx>()
        .init_resource::<hud::Popups>()
        .init_resource::<hud::Banner>()
        .init_resource::<menu::Menu>()
        .init_resource::<water::Water>()
        .add_plugins(
            DefaultPlugins
                .set(bevy::log::LogPlugin {
                    filter: std::env::var("BW_LOG").unwrap_or_else(|_| "warn".into()),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "BOGWIGHT".into(),
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
        .add_plugins(
            PhysicsPlugins::default()
                .with_collision_hooks::<props::Hooks>()
                .set(PhysicsInterpolationPlugin::interpolate_all()),
        )
        .add_plugins(audio::SoundPlugin)
        .add_message::<Noise>()
        .add_message::<Popup>()
        .add_message::<HurtPlayer>()
        .add_message::<HitHunter>()
        .add_message::<Splash>()
        .insert_state(Screen::Boot)
        .add_systems(
            Startup,
            (
                (models::setup_mats, water::setup_materials),
                (models::setup, camera::spawn, weather::setup_lights, hud::spawn),
                (fx::setup, weather::setup_rain),
                move |mut next: ResMut<NextState<Screen>>| next.set(first),
            )
                .chain(),
        )
        .add_systems(OnEnter(Screen::Title), (new_night, menu::title_enter))
        .add_systems(OnExit(Screen::Title), menu::despawn::<menu::TitleRoot>)
        .add_systems(OnEnter(Screen::Playing), (new_night, hud::show))
        .add_systems(OnExit(Screen::Playing), hud::hide)
        .add_systems(OnEnter(Screen::Over), menu::over_enter)
        .add_systems(OnExit(Screen::Over), menu::despawn::<menu::OverRoot>)
        .add_systems(
            FixedUpdate,
            (
                water::simulate,
                autopilot::drive.run_if(|a: Res<Args>| a.autopilot),
                autopilot::grip_test.run_if(|a: Res<Args>| a.grip_test),
                (player::sense, player::movement, player::attack, player::carry),
                (hunters::perceive, hunters::act, hunters::apply_hits),
                boats::steer,
                water::buoyancy,
                hunters::bolts,
                (player::vitals, player::stealth),
                player::clear_presses,
            )
                .chain()
                .run_if(level_ready),
        )
        .add_systems(PreUpdate, input::read.run_if(|a: Res<Args>| !a.autopilot && !a.grip_test))
        .add_systems(
            Update,
            (
                (
                    menu::title_update.run_if(in_state(Screen::Title)),
                    menu::pause.run_if(playing.clone()),
                    menu::over_update.run_if(in_state(Screen::Over)),
                    fullscreen_toggle,
                ),
                (
                    (
                        hunters::bolt_hits,
                        hunters::drop_lanterns,
                        props::thrown,
                        props::lanterns,
                    ),
                    (props::ropes, props::campfires, props::flicker, props::mushrooms),
                    (fx::splashes, fx::update, fx::fireflies),
                    (world::sway, world::drift, water::update_meshes),
                    (weather::storm, weather::lighting, weather::rain),
                    (boats::animate_oars, player::animate, hunters::animate),
                    camera::follow,
                    (world::sky_follow, props::halos),
                    (hud::collect_popups, hud::update, hud::world_labels).run_if(playing.clone()),
                    (session_flow, status_print),
                )
                    .chain()
                    .run_if(level_ready),
                screenshot,
            )
                .chain(),
        )
        .run();
}

/// Tears down the old night and builds a new one: an empty swamp to drift
/// over behind the title, or a hunt for the player.
fn new_night(
    mut commands: Commands,
    screen: Res<State<Screen>>,
    args: Res<Args>,
    mut session: ResMut<Session>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    models: Res<models::Models>,
    mats: Res<models::Mats>,
    wmats: Res<water::WaterMaterials>,
    old: Query<Entity, With<LevelEntity>>,
    mut cams: Query<&mut camera::MainCam>,
    (mut banner, mut controls, mut popups): (ResMut<hud::Banner>, ResMut<input::Controls>, ResMut<hud::Popups>),
    mut time: ResMut<Time<Virtual>>,
    mut first: Local<bool>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    let demo = *screen.get() == Screen::Title;
    let (seed, night) = if demo {
        (0x0dea_db09, 2)
    } else {
        (session.seed, session.night)
    };
    let mut plan = level::plan(seed, night);
    if !demo
        && !*first
        && let Some(x) = args.start
    {
        let t = &plan.terrain;
        let x = x.clamp(2.0, t.len - 2.0);
        plan.start = if t.depth(x) > 1.4 {
            Vec2::new(x, -1.0)
        } else {
            Vec2::new(x, t.height(x) + 0.8)
        };
    }
    if !demo {
        *first = true;
    }
    world::spawn_level(
        &mut commands,
        &mut meshes,
        &mut materials,
        &models,
        &mats,
        &wmats,
        &plan,
        !demo,
        args.low,
    );
    session.hunters_total = plan.hunters.len() as u32;
    session.hunters_left = session.hunters_total;
    session.time = 0.0;
    session.outcome = None;
    session.stats = Stats::default();
    session.demo = demo;
    for mut c in &mut cams {
        c.snap = true;
    }
    controls.clear_presses();
    popups.clear();
    if !demo {
        banner.show(
            format!(
                "NIGHT {}. {} hunters followed you into the swamp.",
                session.night, session.hunters_total
            ),
            7.0,
        );
    }
    time.unpause();
    time.set_relative_speed(args.timescale);
    if args.verbose && !demo {
        println!(
            "night {} seed {seed:#x}: {:.0} m, {} hunters, {} boats; sections: {}",
            night,
            plan.terrain.len,
            plan.hunters.len(),
            plan.boats.len(),
            plan.sections
                .iter()
                .map(|s| format!("{:?}@{:.0}", s.kind, s.x0))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
}

/// The night's clock, and what happens when it's decided.
fn session_flow(
    time: Res<Time>,
    real: Res<Time<Real>>,
    args: Res<Args>,
    screen: Res<State<Screen>>,
    mut session: ResMut<Session>,
    mut player: Query<&mut player::Bogwight>,
    mut next: ResMut<NextState<Screen>>,
    mut exit: MessageWriter<AppExit>,
    mut sfx: MessageWriter<audio::Sfx>,
    mut decided: Local<Option<f32>>,
) {
    if *screen.get() != Screen::Playing {
        *decided = None;
        return;
    }
    if let Ok(mut bw) = player.single_mut() {
        bw.god = args.god;
        if session.live() && bw.dead && bw.dead_t > 1.2 {
            session.outcome = Some(Outcome::Lost);
            sfx.write(audio::Sfx::ui(audio::Sound::Defeat));
        }
    }
    if session.live() {
        session.time += time.delta_secs();
        if session.hunters_total > 0 && session.hunters_left == 0 {
            session.outcome = Some(Outcome::Won);
            sfx.write(audio::Sfx::ui(audio::Sound::Victory));
        }
        *decided = None;
        return;
    }
    let since = *decided.get_or_insert(real.elapsed_secs());
    if real.elapsed_secs() - since > 3.0 {
        if args.exit_on_end {
            let s = session.stats;
            println!(
                "{:?} night {} in {} — ambush {} drown {} thrown {} claw {} flipped {} spotted {}",
                session.outcome.unwrap(),
                session.night,
                util::format_time(session.time),
                s.ambushes,
                s.drowned,
                s.thrown,
                s.clawed,
                s.boats_flipped,
                s.spotted
            );
            exit.write(AppExit::Success);
        } else {
            next.set(Screen::Over);
        }
    }
}

/// `--verbose`: a status line every few seconds of play.
fn status_print(
    args: Res<Args>,
    session: Res<Session>,
    stealth: Res<player::Stealth>,
    player: Query<&player::Bogwight>,
    hunters: Query<&hunters::Hunter>,
    mut next: Local<f32>,
) {
    if !args.verbose || session.demo {
        return;
    }
    if session.time < *next {
        if session.time + 1.0 < *next - 5.0 {
            *next = 0.0;
        }
        return;
    }
    *next = session.time + 5.0;
    let mut states = [0; 6];
    for h in &hunters {
        states[h.state as usize] += 1;
    }
    if let Ok(bw) = player.single() {
        println!(
            "  [{}] pos=({:.1},{:.1}) {:?} hp={:.0} damp={:.2} vis={:.2} (light {:.2} x exposure {:.2}) left={} states(P,S,H,F,Held,D)={:?}",
            util::format_time(session.time),
            stealth.pos.x,
            stealth.pos.y,
            bw.medium,
            bw.hp,
            bw.damp,
            stealth.visibility,
            stealth.light,
            stealth.exposure,
            session.hunters_left,
            states
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
    session: Res<Session>,
    storm: Res<weather::Storm>,
    screen: Res<State<Screen>>,
    player: Query<&player::Bogwight>,
    mut seen: Local<Option<f32>>,
    mut frame: Local<u32>,
    mut shot: Local<usize>,
    mut timing: Local<(f32, u32)>,
    mut taken: Local<Option<f32>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(path) = &args.shot else { return };
    if let Some(at) = *taken {
        if real.elapsed_secs() - at > 0.8 {
            exit.write(AppExit::Success);
        }
        return;
    }
    if let Some(event) = &args.shot_on {
        let s = session.stats;
        let happening = (!session.demo || *screen.get() == Screen::Over)
            && match event.as_str() {
                "kill" => s.ambushes + s.drowned + s.thrown + s.clawed > 0,
                "spotted" => s.spotted > 0,
                "flip" => s.boats_flipped > 0,
                "flash" => storm.flash > 0.6,
                "dead" => player.iter().any(|b| b.dead),
                "won" => session.outcome == Some(Outcome::Won),
                "over" => *screen.get() == Screen::Over,
                _ => true,
            };
        if seen.is_none() {
            if !happening {
                return;
            }
            *seen = Some(real.elapsed_secs());
        }
        let delay = if event == "flash" { 0.0 } else { 0.4 };
        if seen.is_some_and(|t| real.elapsed_secs() - t < delay) {
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
        && (session.demo || session.time < target)
    {
        return;
    }
    *frame += 1;
    let frames = if args.shot_on.as_deref() == Some("flash") {
        1
    } else {
        args.frames
    };
    if *frame > frames / 2 && *frame < frames {
        timing.0 += real.delta_secs();
        timing.1 += 1;
    }
    if *frame < frames {
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
