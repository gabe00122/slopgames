//! Split-screen cameras. Each of the four 3D cameras renders to its own
//! texture, and the window camera lays those textures out as panes; how many
//! are live and what each one follows is set by [`Views`]. Rendering to
//! textures (rather than viewports of the window) keeps each view's HDR,
//! bloom and tone mapping to itself.

use crate::{
    Args,
    course::Course,
    fx::Fx,
    kart::{Driver, Kart, VMAX, marker_layer},
    race::{INTRO, Phase, Race},
    util::{damp, damp_v, hex, smoothstep, wrap_angle},
};
use bevy::{
    camera::{Hdr, RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    post_process::bloom::Bloom,
    prelude::*,
    render::render_resource::{Extent3d, TextureFormat},
    window::PrimaryWindow,
};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ViewMode {
    /// Slow orbit around the whole beast (menus).
    Showcase,
    /// Behind a kart.
    Chase(Entity),
    /// A closer orbit that watches the race (the spare quadrant with three players).
    Overview,
}

/// The live views, in slot order. One fills the window, two split it left
/// and right, three or four take a quadrant each.
#[derive(Resource)]
pub struct Views {
    pub modes: Vec<ViewMode>,
}

#[derive(Component)]
pub struct ViewCam {
    pub slot: usize,
    image: Handle<Image>,
    /// Smoothed heading relative to the track, for the chase view.
    yaw: f32,
    pos: Vec3,
    look: Vec3,
    up: Vec3,
    fov: f32,
    /// Whether the smoothed state has been initialised for the current target.
    live: bool,
    orbit: f32,
}

/// The window camera: draws the view panes and all full-window UI.
#[derive(Component)]
pub struct OverlayCam;

/// The UI image that shows a view's texture in the window.
#[derive(Component)]
pub struct ViewPane(usize);

/// A dark line between panes. The flag is true for the vertical one.
#[derive(Component)]
pub struct Divider(bool);

const INK: Color = Color::srgb(0.1, 0.08, 0.18);

pub fn spawn(mut commands: Commands, args: Res<Args>, mut images: ResMut<Assets<Image>>) {
    for slot in 0..4 {
        let image = images.add(Image::new_target_texture(64, 64, TextureFormat::Rgba8UnormSrgb, None));
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            ImageNode::new(image.clone()),
            ViewPane(slot),
            GlobalZIndex(-10),
        ));
        commands.spawn((
            Camera3d::default(),
            Camera {
                order: slot as isize,
                is_active: slot == 0,
                ..default()
            },
            RenderTarget::Image(image.clone().into()),
            RenderLayers::from_layers(&[0, 1, 2, 3, 4]),
            Hdr,
            if args.low { Msaa::Off } else { Msaa::Sample4 },
            Tonemapping::AcesFitted,
            Bloom {
                intensity: if args.low { 0.0 } else { 0.1 },
                ..Bloom::NATURAL
            },
            Projection::Perspective(PerspectiveProjection {
                fov: 60f32.to_radians(),
                near: 0.4,
                far: 30000.0,
                ..default()
            }),
            DistanceFog::default(),
            Transform::from_xyz(0.0, 200.0, 600.0).looking_at(Vec3::ZERO, Vec3::Y),
            ViewCam {
                slot,
                image,
                yaw: 0.0,
                pos: Vec3::ZERO,
                look: Vec3::ZERO,
                up: Vec3::Y,
                fov: 60.0,
                live: false,
                orbit: 0.0,
            },
        ));
    }
    for vertical in [true, false] {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: if vertical { percent(50) } else { percent(0) },
                top: if vertical { percent(0) } else { percent(50) },
                width: if vertical { Val::VMin(0.6) } else { percent(100) },
                height: if vertical { percent(100) } else { Val::VMin(0.6) },
                margin: if vertical {
                    UiRect::left(Val::VMin(-0.3))
                } else {
                    UiRect::top(Val::VMin(-0.3))
                },
                ..default()
            },
            BackgroundColor(INK),
            Visibility::Hidden,
            Divider(vertical),
            GlobalZIndex(-9),
        ));
    }
    commands.spawn((
        Camera2d,
        Camera {
            order: 10,
            clear_color: ClearColorConfig::Custom(INK),
            ..default()
        },
        IsDefaultUiCamera,
        OverlayCam,
    ));
    commands.insert_resource(Views {
        modes: vec![ViewMode::Showcase],
    });
}

/// The window rectangle of view `slot` when `count` views are live, as
/// fractions `(x, y, w, h)`.
pub fn view_rect(slot: usize, count: usize) -> (f32, f32, f32, f32) {
    match count {
        0 | 1 => (0.0, 0.0, 1.0, 1.0),
        2 => (slot as f32 * 0.5, 0.0, 0.5, 1.0),
        _ => ((slot % 2) as f32 * 0.5, (slot / 2) as f32 * 0.5, 0.5, 0.5),
    }
}

/// Sizes each live view's texture to its share of the window and places its pane.
pub fn layout(
    views: Res<Views>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    karts: Query<&Kart>,
    mut cams: Query<(&ViewCam, &mut Camera, &mut RenderLayers)>,
    mut panes: Query<(&ViewPane, &mut Node, &mut Visibility), Without<Divider>>,
    mut dividers: Query<(&Divider, &mut Visibility), Without<ViewPane>>,
) {
    let size = window.physical_size();
    if size.x == 0 || size.y == 0 {
        return;
    }
    let count = views.modes.len();
    for (view, mut cam, mut layers) in &mut cams {
        let active = view.slot < count;
        if cam.is_active != active {
            cam.is_active = active;
        }
        if !active {
            continue;
        }
        // Every view shows the other players' markers but not its own.
        let own = match views.modes[view.slot] {
            ViewMode::Chase(e) => karts.get(e).ok().and_then(|k| match k.driver {
                Driver::Human(slot) => Some(marker_layer(slot)),
                Driver::Cpu => None,
            }),
            _ => None,
        };
        let want = RenderLayers::from_layers(&(0..=4).filter(|&l| Some(l) != own).collect::<Vec<_>>());
        if *layers != want {
            *layers = want;
        }
        let (_, _, w, h) = view_rect(view.slot, count);
        let want = Extent3d {
            width: ((w * size.x as f32) as u32).max(8),
            height: ((h * size.y as f32) as u32).max(8),
            depth_or_array_layers: 1,
        };
        if images
            .get(&view.image)
            .is_some_and(|i| i.texture_descriptor.size != want)
            && let Some(mut image) = images.get_mut(&view.image)
        {
            image.resize(want);
        }
    }
    for (pane, mut node, mut vis) in &mut panes {
        let active = pane.0 < count;
        let want = if active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        if active {
            let (x, y, w, h) = view_rect(pane.0, count);
            let rect = (
                percent(x * 100.0),
                percent(y * 100.0),
                percent(w * 100.0),
                percent(h * 100.0),
            );
            if (node.left, node.top, node.width, node.height) != rect {
                (node.left, node.top, node.width, node.height) = rect;
            }
        }
    }
    for (divider, mut vis) in &mut dividers {
        let want = if count >= if divider.0 { 2 } else { 3 } {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }
}

/// Matches each camera's fog to the loaded course.
pub fn apply_env(
    course: Res<Course>,
    mut applied: Local<Option<usize>>,
    mut cams: Query<&mut DistanceFog, With<ViewCam>>,
) {
    if *applied == Some(course.index) {
        return;
    }
    *applied = Some(course.index);
    for mut fog in &mut cams {
        *fog = DistanceFog {
            color: hex(course.env.fog),
            falloff: FogFalloff::Linear {
                start: course.env.fog_start,
                end: course.env.fog_end,
            },
            ..default()
        };
    }
}

pub fn update(
    time: Res<Time>,
    real: Res<Time<Real>>,
    args: Res<Args>,
    views: Res<Views>,
    course: Res<Course>,
    race: Option<Res<Race>>,
    fx: Res<Fx>,
    karts: Query<&Kart>,
    mut cams: Query<(&mut ViewCam, &mut Transform, &mut Projection)>,
) {
    let dt = time.delta_secs();
    let t = real.elapsed_secs();
    let show = course.view;
    let leader = karts.iter().min_by_key(|k| k.place).map(|k| k.pos);
    for (mut cam, mut tf, mut projection) in &mut cams {
        let Some(&mode) = views.modes.get(cam.slot) else {
            cam.live = false;
            continue;
        };
        let mut fov = 58.0;
        // The orbit shot, used by the showcase and blended out of during the intro.
        let orbit = |angle: f32, radius: f32, height: f32, center: Vec3| {
            let pos = center + Vec3::new(angle.cos() * radius, height, angle.sin() * radius);
            (pos, center)
        };
        let (pos, look, up) = match mode {
            ViewMode::Showcase => {
                if let Some((pos, yaw, pitch)) = args.pose {
                    tf.translation = pos;
                    tf.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
                    continue;
                }
                let (pos, look) = orbit(t * 0.11 + 0.9, show.radius, show.height, show.center);
                (pos, look, Vec3::Y)
            }
            ViewMode::Overview => {
                let focus = leader.map_or(show.center, |p| show.center.lerp(p, 0.35));
                let (pos, look) = orbit(t * 0.16 + 2.0, show.radius * 0.72, show.height * 0.9, focus);
                (pos, look, Vec3::Y)
            }
            ViewMode::Chase(entity) => {
                let Ok(k) = karts.get(entity) else { continue };
                let loc = course.track.at(k.u);
                let right = loc.r.normalize_or(Vec3::X);
                if !cam.live {
                    cam.yaw = k.yaw;
                    cam.orbit = 0.0;
                }
                let finished = k.finished.is_some();
                if finished {
                    cam.orbit += dt * 0.5;
                }
                // Follow the heading loosely; lean into drifts.
                let want = k.yaw + k.drift as f32 * 0.22;
                let diff = wrap_angle(want - cam.yaw);
                cam.yaw = wrap_angle(cam.yaw + diff * (1.0 - (-dt * if k.spin > 0.0 { 1.0 } else { 5.5 }).exp()));
                let a = cam.yaw + cam.orbit;
                let fwd = (loc.t * a.cos() + right * a.sin()).normalize_or(Vec3::Z);
                // Tilt partway with the ground so the horizon shows the beast rolling.
                let up = (loc.n * 0.55 + Vec3::Y * 0.45).normalize();
                let speed = (k.speed / VMAX).clamp(0.0, 1.4);
                let boost = if k.boost > 0.0 { 1.0 } else { 0.0 };
                let dist = 8.6 + 1.6 * speed + if finished { 3.0 } else { 0.0 };
                let lift = Vec3::Y * (k.h * 0.75 + (k.depth - 0.9).clamp(0.0, 3.0));
                let base = loc.at(k.d) + lift;
                // The camera rides the road too: it sits over the ribbon behind
                // the kart, so it follows crests and never ends up inside a hill.
                let scale = (1.0 - k.d * loc.k).clamp(0.4, 2.5);
                let behind = course.track.at(k.u - dist * a.cos() / (loc.len * scale));
                let reach = behind.half + behind.sh[0].min(behind.sh[1]);
                let across = (k.d - dist * a.sin()).clamp(-reach, reach);
                let mut pos = behind.at(across) + lift + up * 4.1;
                let mut look = base + fwd * 6.0 + up * 1.4;
                fov = 60.0 + 7.0 * speed + 9.0 * boost;
                if k.fallen {
                    // Watch the fall from where we were.
                    pos = cam.pos;
                    look = k.pos;
                }
                // Blend from the intro sweep into the chase position.
                if let Some(race) = &race
                    && race.phase == Phase::Intro
                {
                    let f = smoothstep(INTRO - 1.6, INTRO - 0.1, race.timer);
                    let sweep = 2.2 + race.timer * 0.5 + cam.slot as f32 * 0.4;
                    let (opos, olook) = orbit(sweep, show.radius * 0.85, show.height * 0.8, show.center);
                    pos = opos.lerp(pos, f);
                    look = olook.lerp(look, f);
                    fov = 58.0 + (fov - 58.0) * f;
                }
                (pos, look, up)
            }
        };
        if !cam.live {
            cam.pos = pos;
            cam.look = look;
            cam.up = up;
            cam.fov = fov;
            cam.live = true;
        }
        // A touch of smoothing takes the edge off bumps without lagging behind.
        let rate = if matches!(mode, ViewMode::Chase(_)) { 14.0 } else { 3.0 };
        cam.pos = damp_v(cam.pos, pos, rate, dt);
        cam.look = damp_v(cam.look, look, rate * 1.3, dt);
        cam.up = damp_v(cam.up, up, 4.0, dt).normalize_or(Vec3::Y);
        cam.fov = damp(cam.fov, fov, 4.0, dt);

        let shake = (fx.shake + course.out.rumble * 0.6).min(1.2);
        let jitter = if shake > 0.01 {
            Vec3::new((t * 53.0).sin(), (t * 71.0 + 1.0).sin(), (t * 61.0 + 2.0).sin()) * shake * 0.55
        } else {
            Vec3::ZERO
        };
        *tf = Transform::from_translation(cam.pos + jitter).looking_at(cam.look, cam.up);
        if let Projection::Perspective(p) = &mut *projection {
            // Narrow split-screen views get a taller field of view so the road stays readable.
            let widen = if p.aspect_ratio < 1.2 { 1.22 } else { 1.0 };
            p.fov = (cam.fov * widen).to_radians();
        }
    }
}
