//! First-person visitor: camera, walking, looking and bumping into walls.

use crate::{Args, architecture::Sky, layout::Walls};
use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    camera::{Exposure, Hdr},
    core_pipeline::tonemapping::Tonemapping,
    input::mouse::AccumulatedMouseMotion,
    light::{GeneratedEnvironmentMapLight, Skybox, VolumetricFog},
    pbr::ScreenSpaceAmbientOcclusion,
    post_process::bloom::Bloom,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use std::f32::consts::FRAC_PI_2;

pub const EYE: f32 = 1.65;
const RADIUS: f32 = 0.32;

#[derive(Component)]
pub struct Player {
    pub yaw: f32,
    pub pitch: f32,
    pub bob: f32,
    pub velocity: Vec2,
}

/// Whether the visitor is walking freely (vs. paused or on the tour).
#[derive(Resource, Default)]
pub struct Control {
    pub started: bool,
    pub paused: bool,
    pub touring: bool,
    pub sensitivity: f32,
}

impl Control {
    pub fn walking(&self) -> bool {
        self.started && !self.paused && !self.touring
    }
}

pub fn spawn(mut commands: Commands, sky: Res<Sky>, args: Res<Args>) {
    let (pos, yaw, pitch) = args.pose.unwrap_or((Vec3::new(0.0, EYE, 26.5), 0.0, 0.02));
    let mut e = commands.spawn((
        Camera3d::default(),
        Hdr,
        Projection::Perspective(PerspectiveProjection {
            fov: 72f32.to_radians(),
            near: 0.05,
            ..default()
        }),
        Tonemapping::AgX,
        Exposure { ev100: 7.4 },
        Bloom {
            intensity: 0.12,
            ..Bloom::NATURAL
        },
        Skybox {
            image: Some(sky.cubemap.clone()),
            brightness: 900.0,
            ..default()
        },
        GeneratedEnvironmentMapLight {
            environment_map: sky.interior.clone(),
            intensity: 70.0,
            ..default()
        },
        Transform::from_translation(pos).with_rotation(Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0)),
        Player {
            yaw,
            pitch,
            bob: 0.0,
            velocity: Vec2::ZERO,
        },
        IsDefaultUiCamera,
        Name::new("Visitor"),
    ));
    if args.low {
        e.insert(Msaa::Sample4);
    } else {
        e.insert((
            Msaa::Off,
            TemporalAntiAliasing::default(),
            ScreenSpaceAmbientOcclusion::default(),
            VolumetricFog {
                ambient_intensity: 0.0,
                step_count: 48,
                ..default()
            },
        ));
    }
    commands.insert_resource(Control {
        sensitivity: 1.0,
        ..default()
    });
}

pub fn look(
    motion: Res<AccumulatedMouseMotion>,
    control: Res<Control>,
    cursor: Single<&CursorOptions>,
    mut q: Query<(&mut Player, &mut Transform)>,
) {
    if !control.walking() || cursor.grab_mode == CursorGrabMode::None {
        return;
    }
    let Ok((mut p, mut t)) = q.single_mut() else { return };
    let d = motion.delta * 0.0022 * control.sensitivity;
    p.yaw -= d.x;
    p.pitch = (p.pitch - d.y).clamp(-FRAC_PI_2 + 0.05, FRAC_PI_2 - 0.05);
    t.rotation = Quat::from_euler(EulerRot::YXZ, p.yaw, p.pitch, 0.0);
}

pub fn walk(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    control: Res<Control>,
    walls: Res<Walls>,
    mut q: Query<(&mut Player, &mut Transform)>,
) {
    let Ok((mut p, mut t)) = q.single_mut() else { return };
    let dt = time.delta_secs().min(0.05);
    let mut wish = Vec2::ZERO;
    if control.walking() {
        let fwd = Vec2::new(-p.yaw.sin(), -p.yaw.cos());
        let right = Vec2::new(-fwd.y, fwd.x);
        if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
            wish += fwd;
        }
        if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
            wish -= fwd;
        }
        if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
            wish += right;
        }
        if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
            wish -= right;
        }
        let speed = if keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
            6.5
        } else {
            3.2
        };
        wish = wish.normalize_or_zero() * speed;
    }
    if control.touring {
        return;
    }
    let accel = if wish == Vec2::ZERO { 10.0 } else { 12.0 };
    p.velocity = p.velocity.lerp(wish, (accel * dt).min(1.0));
    let old = Vec2::new(t.translation.x, t.translation.z);
    let moved = walls.resolve(old + p.velocity * dt, RADIUS);
    let speed = (moved - old).length() / dt.max(1e-4);
    p.bob += speed * dt * 2.1;
    let bob = (p.bob).sin() * 0.035 * (speed / 3.2).min(1.5);
    t.translation = Vec3::new(moved.x, EYE + bob, moved.y);
}

pub fn grab_cursor(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut control: ResMut<Control>,
    mut cursor: Single<&mut CursorOptions>,
    args: Res<Args>,
) {
    if args.shot.is_some() {
        return;
    }
    if mouse.just_pressed(MouseButton::Left) && (!control.started || control.paused) {
        control.started = true;
        control.paused = false;
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    if keys.just_pressed(KeyCode::Escape) && control.started && !control.paused && !control.touring {
        control.paused = true;
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        control.sensitivity = (control.sensitivity / 1.2).max(0.2);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        control.sensitivity = (control.sensitivity * 1.2).min(5.0);
    }
}
