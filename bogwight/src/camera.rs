//! A perspective camera looking at the playfield from the front, so the
//! background layers drift by with real parallax. It trails the bogwight,
//! leads it a little, and pulls back when things get fast.

use crate::{
    Args,
    fx::Fx,
    game::Session,
    player::Bogwight,
    util::{damp_v, noise2},
    world::LevelInfo,
};
use avian2d::prelude::LinearVelocity;
use bevy::{
    anti_alias::fxaa::Fxaa, camera::Hdr, core_pipeline::tonemapping::Tonemapping, post_process::bloom::Bloom,
    prelude::*,
};

#[derive(Component)]
pub struct MainCam {
    pub pos: Vec3,
    /// Snap to the target next frame (after a level change).
    pub snap: bool,
}

pub fn spawn(mut commands: Commands, args: Res<Args>) {
    let mut e = commands.spawn((
        Camera3d::default(),
        Hdr,
        Tonemapping::TonyMcMapface,
        Bloom {
            intensity: if args.low { 0.0 } else { 0.22 },
            ..Bloom::NATURAL
        },
        Projection::Perspective(PerspectiveProjection {
            fov: 40f32.to_radians(),
            near: 0.3,
            far: 2000.0,
            ..default()
        }),
        DistanceFog {
            color: Color::srgb(0.055, 0.075, 0.085),
            falloff: FogFalloff::Linear {
                start: 24.0,
                end: 150.0,
            },
            ..default()
        },
        Transform::from_xyz(20.0, 2.0, 18.0),
        MainCam {
            pos: Vec3::new(20.0, 2.0, 18.0),
            snap: true,
        },
        IsDefaultUiCamera,
    ));
    if args.low {
        e.insert((Msaa::Off, Fxaa::default()));
    } else {
        e.insert(Msaa::Sample4);
    }
}

pub fn follow(
    time: Res<Time>,
    real: Res<Time<Real>>,
    session: Option<Res<Session>>,
    level: Option<Res<LevelInfo>>,
    terrain: Option<Res<crate::level::Terrain>>,
    fx: Res<Fx>,
    player: Query<(&Transform, &LinearVelocity), (With<Bogwight>, Without<MainCam>)>,
    mut cam: Query<(&mut Transform, &mut MainCam)>,
) {
    let Ok((mut tf, mut cam)) = cam.single_mut() else {
        return;
    };
    let dt = real.delta_secs().min(0.05);
    let len = level.as_ref().map_or(400.0, |l| l.len);
    let demo = session.as_ref().is_none_or(|s| s.demo);
    let target = if let Ok((p, v)) = player.single()
        && !demo
    {
        let speed = v.length();
        let lead = Vec3::new((v.x * 0.35).clamp(-3.0, 3.0), 1.3 + (v.y * 0.08).clamp(-0.8, 0.8), 0.0);
        let dist = 16.5 + (speed * 0.25).min(4.0);
        Vec3::new(p.translation.x, p.translation.y, dist) + lead
    } else {
        // Attract mode: drift along the swamp.
        let t = time.elapsed_secs();
        let span = (len - 40.0).max(10.0);
        let x = 20.0 + (t * 2.2) % span;
        Vec3::new(x, 2.6, 19.0)
    };
    let mut target = target;
    target.x = target.x.clamp(9.0, len - 9.0);
    // Don't spend the screen on solid ground.
    if let Some(t) = terrain.as_deref() {
        target.y = target.y.max(t.height(target.x) + 2.2);
    }
    if cam.snap {
        cam.pos = target;
        cam.snap = false;
    }
    cam.pos = damp_v(cam.pos, target, 3.2, dt);
    let shake = fx.shake_amount();
    let t = real.elapsed_secs();
    let jitter = Vec3::new(noise2(t * 25.0, 0.0, 3) - 0.5, noise2(0.0, t * 25.0, 5) - 0.5, 0.0) * shake * shake * 1.2;
    tf.translation = cam.pos + jitter;
    let look = cam.pos + Vec3::new(0.0, -0.9, -cam.pos.z) + jitter * 0.5;
    tf.look_at(look, Vec3::Y);
}
