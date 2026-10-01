//! Monster Race: split-screen kart racing on the backs of colossal beasts.
//! Every mesh, texture, glyph and sound is generated at startup.

// Bevy systems take many parameters and spell out long query types.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod audio;
mod beast;
mod camera;
mod course;
mod env;
mod fauna;
mod font;
mod fx;
mod game;
mod hud;
mod input;
mod items;
mod kart;
mod menu;
mod meshkit;
mod race;
mod track;
mod util;

use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode, WindowResolution},
};
use game::{PlayerSlot, Roster, Screen, Settings};
use input::Device;

/// Command-line options.
#[derive(Resource, Clone, Default)]
pub struct Args {
    /// No sound.
    pub mute: bool,
    /// Cheaper rendering for slower GPUs: no anti-aliasing or bloom, simpler shadows.
    pub low: bool,
    /// Write every synthesized sound to this directory as WAV files, then exit.
    pub dump_audio: Option<String>,
    pub size: (u32, u32),
    pub fullscreen: bool,
    pub novsync: bool,
    /// Skip the menus and start a race with this many players.
    pub race: Option<usize>,
    pub course: usize,
    pub laps: Option<u32>,
    pub racers: Option<usize>,
    /// Let the CPU drive the players' karts (used with --race): 1 drives like
    /// a CPU racer, 2 works the real drift button, to exercise the player's controls.
    pub autopilot: u8,
    /// Go straight to this screen (used with --shot).
    pub screen: Option<String>,
    /// Save a screenshot to this path, then exit.
    pub shot: Option<String>,
    /// Frames to wait before the screenshot.
    pub frames: u32,
    /// Moments of game (or race) time at which to take screenshots.
    pub times: Vec<f32>,
    /// Real seconds after launch at which to take screenshots (instead of `times`).
    pub waits: Vec<f32>,
    /// Fixed showcase camera: position, yaw and pitch.
    pub pose: Option<(Vec3, f32, f32)>,
}

fn parse_args() -> Args {
    let mut a = Args {
        frames: 60,
        size: (1600, 900),
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
            "--autopilot" => a.autopilot = 1,
            "--autopilot-drift" => a.autopilot = 2,
            "--race" => a.race = Some(next(&mut i).parse().unwrap_or(1)),
            "--course" => a.course = next(&mut i).parse().unwrap_or(0),
            "--laps" => a.laps = next(&mut i).parse().ok(),
            "--racers" => a.racers = next(&mut i).parse().ok(),
            "--screen" => a.screen = Some(next(&mut i)),
            "--shot" => a.shot = Some(next(&mut i)),
            "--frames" => a.frames = next(&mut i).parse().unwrap_or(60),
            "--time" => a.times = next(&mut i).split(',').filter_map(|x| x.parse().ok()).collect(),
            "--wait" => a.waits = next(&mut i).split(',').filter_map(|x| x.parse().ok()).collect(),
            "--size" => {
                let s = next(&mut i);
                if let Some((w, h)) = s.split_once('x') {
                    a.size = (w.parse().unwrap_or(1600), h.parse().unwrap_or(900));
                }
            }
            "--pose" => {
                let v: Vec<f32> = next(&mut i).split(',').filter_map(|x| x.parse().ok()).collect();
                if v.len() == 5 {
                    a.pose = Some((Vec3::new(v[0], v[1], v[2]), v[3].to_radians(), v[4].to_radians()));
                }
            }
            "-h" | "--help" => {
                println!(
                    "monster-race [--fullscreen] [--size WxH] [--mute] [--low]\n\
                     quick start: --race PLAYERS [--course N] [--laps N] [--racers N] [--autopilot]\n\
                     debug: --screen lobby|select | --pose x,y,z,yaw_deg,pitch_deg | --time SECS | --shot out.png [--frames N] | --novsync"
                );
                std::process::exit(0);
            }
            other => eprintln!("unknown option {other}"),
        }
        i += 1;
    }
    a.course = a.course.min(beast::COURSES.len() - 1);
    a
}

fn main() {
    let args = parse_args();
    if let Some(dir) = &args.dump_audio {
        audio::dump(dir);
        return;
    }
    let mut settings = Settings {
        course: args.course,
        ..default()
    };
    if let Some(laps) = args.laps {
        settings.laps = laps;
    }
    if let Some(racers) = args.racers {
        settings.racers = racers;
    }
    // Quick start: seat the requested number of players on whatever devices exist.
    let mut roster = Roster::default();
    if let Some(players) = args.race {
        for i in 0..players.min(game::MAX_PLAYERS) {
            // Seats start empty; `input::reattach` fills them with gamepads, then keyboards.
            roster.players.push(PlayerSlot {
                device: Device::Pad(Entity::PLACEHOLDER),
                color: i,
                ready: true,
            });
        }
    }
    let first = match (args.race, args.screen.as_deref()) {
        (Some(_), _) => Screen::Race,
        (None, Some("lobby")) => Screen::Lobby,
        (None, Some("select")) => Screen::Select,
        _ => Screen::Title,
    };

    let in_race = in_state(Screen::Race).and_then(resource_exists::<race::Race>);
    let has_course = resource_exists::<course::Course>;

    App::new()
        .insert_resource(args.clone())
        .insert_resource(settings)
        .insert_resource(roster)
        .init_resource::<game::Results>()
        .init_resource::<game::Cup>()
        .init_resource::<input::Inputs>()
        .init_resource::<fx::Fx>()
        .insert_resource(ClearColor(Color::srgb(0.75, 0.9, 0.97)))
        .add_plugins(
            DefaultPlugins
                .set(bevy::log::LogPlugin {
                    filter: std::env::var("MR_LOG").unwrap_or_default(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Monster Race".into(),
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
        .insert_state(first)
        .add_message::<course::LoadCourse>()
        .add_systems(
            Startup,
            (
                course::setup_mats,
                (
                    fx::setup,
                    font::setup,
                    camera::spawn,
                    kart::setup_assets,
                    items::setup_assets,
                    hud::setup_assets,
                ),
            )
                .chain(),
        )
        // Screens.
        .add_systems(OnEnter(Screen::Title), menu::title_enter)
        .add_systems(OnEnter(Screen::Lobby), menu::lobby_enter)
        .add_systems(OnEnter(Screen::Select), menu::select_enter)
        .add_systems(OnEnter(Screen::Results), menu::results_enter)
        .add_systems(OnEnter(Screen::Race), race::begin)
        .add_systems(OnExit(Screen::Race), race::end)
        .add_systems(
            Update,
            (
                (input::gather, input::reattach),
                (
                    menu::title_update.run_if(in_state(Screen::Title)),
                    menu::lobby_update.run_if(in_state(Screen::Lobby)),
                    menu::select_update.run_if(in_state(Screen::Select)),
                    menu::results_update.run_if(in_state(Screen::Results)),
                    race::pause.run_if(in_race.clone()),
                ),
                course::load_course,
                (
                    course::animate,
                    (
                        race::setup,
                        kart::control,
                        kart::drive,
                        kart::collide,
                        items::boxes,
                        items::items,
                        race::tick,
                    )
                        .chain()
                        .run_if(in_race.clone()),
                    (
                        course::follow_anchors,
                        env::scroll,
                        env::waves,
                        env::splashes,
                        env::geysers,
                        fauna::fly,
                        fauna::scuttle,
                        kart::dress,
                    ),
                    (camera::apply_env, camera::layout, camera::update).chain(),
                    hud::build_minimap,
                    (hud::sync, hud::update).chain().run_if(in_race.clone()),
                    fx::update,
                )
                    .chain()
                    .run_if(has_course),
                menu::pause_overlay,
                font::render_labels,
                (fullscreen_toggle, screenshot),
            )
                .chain(),
        )
        .run();
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
/// With several `--time` values (comma separated) it saves one per moment,
/// numbering the files.
fn screenshot(
    mut commands: Commands,
    args: Res<Args>,
    real: Res<Time<Real>>,
    mut time: ResMut<Time<Virtual>>,
    race: Option<Res<race::Race>>,
    mut frame: Local<u32>,
    mut shot: Local<usize>,
    mut timing: Local<(f32, u32)>,
    mut taken: Local<Option<f32>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(path) = &args.shot else { return };
    // Give the last capture time to be read back and written before quitting.
    if let Some(at) = *taken {
        if real.elapsed_secs() - at > 0.7 {
            exit.write(AppExit::Success);
        }
        return;
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
        let now = match &race {
            Some(race) if race.phase == race::Phase::Racing => race.time,
            Some(_) => 0.0,
            None => time.elapsed_secs(),
        };
        // Fast-forward to the moment, then settle at normal speed.
        let early = now < target;
        time.set_relative_speed(if early && target - now > 1.0 { 6.0 } else { 1.0 });
        if early {
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
