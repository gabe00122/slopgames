//! The Museum of Human Achievement: a walkable 3D museum built with Bevy.
//! Every mesh, texture and placard is generated at startup.

mod anim;
mod architecture;
mod content;
mod exhibits;
mod hud;
mod kit;
mod layout;
mod map;
mod meshes;
mod models;
mod placards;
mod player;
mod textures;
mod tour;

use bevy::{
    light::GlobalAmbientLight,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::{PresentMode, WindowResolution},
};

/// Command-line options.
#[derive(Resource, Clone, Default)]
pub struct Args {
    /// Cheaper rendering: no SSAO, TAA or volumetric light.
    pub low: bool,
    /// Start pose: eye position, yaw and pitch.
    pub pose: Option<(Vec3, f32, f32)>,
    /// Save a screenshot to this path after warm-up, then exit.
    pub shot: Option<String>,
    /// Start on the guided tour at this exhibit (used with --shot).
    pub exhibit: Option<usize>,
    /// Stand in front of this exhibit's placard (used with --shot).
    pub placard: Option<usize>,
    /// Open the map (used with --shot).
    pub map: bool,
    pub novsync: bool,
    /// Start the guided tour immediately (used with --shot).
    pub tour: bool,
    /// Show the title card (used with --shot).
    pub title: bool,
    /// Speed up time (used with --shot).
    pub timescale: f32,
    pub frames: u32,
    pub size: (u32, u32),
}

fn parse_args() -> Args {
    let mut a = Args {
        frames: 150,
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
            "--low" => a.low = true,
            "--map" => a.map = true,
            "--novsync" => a.novsync = true,
            "--tour" => a.tour = true,
            "--title" => a.title = true,
            "--timescale" => a.timescale = next(&mut i).parse().unwrap_or(1.0),
            "--placard" => a.placard = next(&mut i).parse().ok(),
            "--shot" => a.shot = Some(next(&mut i)),
            "--frames" => a.frames = next(&mut i).parse().unwrap_or(150),
            "--exhibit" => a.exhibit = next(&mut i).parse().ok(),
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
                    "museum [--low] [--size WxH]\n\
                     debug: --pose x,y,z,yaw_deg,pitch_deg | --exhibit N | --map | --shot out.png [--frames N]"
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
    App::new()
        .insert_resource(args.clone())
        .insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.025)))
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(1.0, 0.95, 0.88),
            brightness: 18.0,
            ..default()
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: content::MUSEUM_NAME.into(),
                resolution: WindowResolution::new(args.size.0, args.size.1),
                present_mode: if args.novsync {
                    PresentMode::AutoNoVsync
                } else {
                    PresentMode::AutoVsync
                },
                ..default()
            }),
            ..default()
        }))
        .add_plugins(anim::AnimPlugin)
        .init_resource::<tour::Tour>()
        .add_systems(
            Startup,
            (
                kit::setup_palette,
                placards::setup_atlas,
                architecture::build,
                exhibits::spawn,
                player::spawn,
                hud::spawn,
                map::spawn,
                debug_start,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                player::grab_cursor,
                tour::controls,
                player::look,
                player::walk,
                tour::drive,
                hud::update_focus,
                hud::update_panel,
                hud::update_status,
                map::toggle,
                map::update,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (placards::retire_atlas_camera, architecture::shadow_lod, screenshot),
        )
        .run();
}

/// Applies debug start options.
fn debug_start(
    args: Res<Args>,
    mut control: ResMut<player::Control>,
    mut tour: ResMut<tour::Tour>,
    mut map: ResMut<map::MapOpen>,
    mut time: ResMut<Time<Virtual>>,
    index: Res<exhibits::ExhibitIndex>,
    mut q: Query<(&mut player::Player, &mut Transform)>,
) {
    if args.shot.is_none() {
        return;
    }
    control.started = !args.title;
    map.0 = args.map;
    time.set_relative_speed(args.timescale);
    if args.tour {
        let here = q.single().unwrap().1.translation;
        control.touring = true;
        tour::begin(&mut tour, &index, here, args.exhibit);
        return;
    }
    if let Some(i) = args.exhibit {
        let e = &index.0[i.min(index.0.len() - 1)];
        let (mut p, mut t) = q.single_mut().unwrap();
        let dir = (e.focus - e.view).normalize();
        p.yaw = (-dir.x).atan2(-dir.z);
        p.pitch = dir.y.asin();
        t.translation = e.view;
        t.rotation = Quat::from_euler(EulerRot::YXZ, p.yaw, p.pitch, 0.0);
        tour.focus = Some(i);
    }
    if let Some(i) = args.placard {
        let (pos, normal) = index.0[i.min(index.0.len() - 1)].placard;
        let (mut p, mut t) = q.single_mut().unwrap();
        t.translation = pos + normal * 0.75;
        let dir = -normal;
        p.yaw = (-dir.x).atan2(-dir.z);
        p.pitch = dir.y.asin();
        t.rotation = Quat::from_euler(EulerRot::YXZ, p.yaw, p.pitch, 0.0);
    }
}

fn screenshot(
    mut commands: Commands,
    args: Res<Args>,
    time: Res<Time<Real>>,
    mut frame: Local<u32>,
    mut timing: Local<(f32, u32)>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(path) = &args.shot else { return };
    *frame += 1;
    if *frame > args.frames / 2 && *frame < args.frames {
        timing.0 += time.delta_secs();
        timing.1 += 1;
    }
    if *frame == args.frames && timing.1 > 0 {
        let ms = timing.0 / timing.1 as f32 * 1000.0;
        println!("frame time: {ms:.2} ms ({:.0} fps)", 1000.0 / ms);
    }
    if *frame == args.frames {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path.clone()));
    }
    if *frame == args.frames + 20 {
        exit.write(AppExit::Success);
    }
}
