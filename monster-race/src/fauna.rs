//! The animals that share the beast's back: birds wheeling overhead, fish
//! leaping alongside, and the little scuttlers that wander across the road.

use crate::{
    beast::{Built, FaunaKind},
    course::{Course, CourseEntity, Mats},
    meshkit::{MeshBuilder, Skin},
    util::{lin, shade},
};
use bevy::{light::NotShadowCaster, prelude::*};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

#[derive(Component)]
pub struct Flyer {
    kind: FaunaKind,
    skin: Skin,
    rest: Vec3,
    seed: f32,
}

#[derive(Component)]
pub struct Wing(f32);

/// A critter crossing the road at track position `u`. Karts that hit it spin out.
#[derive(Component)]
pub struct Scuttler {
    pub u: f32,
    pub d: f32,
    phase: f32,
    speed: f32,
    /// Seconds left lying flat after being run over.
    pub squashed: f32,
}

fn critter_mesh(color: u32) -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let c = lin(color);
    b.ico_with(Vec3::new(0.0, 0.75, 0.0), Vec3::new(1.25, 0.6, 0.95), 1, |d| {
        (1.0, shade(c, 0.85 + d.y * 0.3))
    });
    for side in [-1.0f32, 1.0] {
        for k in 0..3 {
            let z = -0.55 + k as f32 * 0.5;
            let hip = Vec3::new(side * 0.9, 0.7, z);
            let knee = Vec3::new(side * 1.7, 1.05, z * 1.3);
            let foot = Vec3::new(side * 2.0, 0.0, z * 1.5);
            b.cyl(hip, knee, 0.14, 0.11, 4, shade(c, 0.75), false);
            b.cyl(knee, foot, 0.11, 0.05, 4, shade(c, 0.6), false);
        }
        // Eyes on stalks and a raised claw.
        let eye = Vec3::new(side * 0.35, 1.55, 0.75);
        b.cyl(Vec3::new(side * 0.3, 1.1, 0.7), eye, 0.07, 0.07, 4, c, false);
        b.ball(eye, 0.2, 0, lin(0x16131a));
        let claw = Vec3::new(side * 1.25, 1.0, 1.2);
        b.cyl(Vec3::new(side * 0.9, 0.8, 0.7), claw, 0.16, 0.2, 4, c, false);
        b.ico(
            claw + Vec3::new(0.0, 0.1, 0.3),
            Vec3::new(0.32, 0.26, 0.45),
            0,
            shade(c, 1.2),
        );
    }
    b
}

pub fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &Mats, built: &Built) {
    // Shared meshes.
    let mut body = MeshBuilder::new();
    body.ico(Vec3::ZERO, Vec3::new(0.7, 0.6, 2.0), 1, lin(0xf8f8f4));
    body.cone(
        Vec3::new(0.0, 0.1, -1.8),
        Vec3::new(0.0, 0.1, -2.9),
        0.28,
        4,
        lin(0xf2a23a),
    );
    body.tri_2(
        [
            Vec3::new(0.0, 0.0, 1.6),
            Vec3::new(-0.8, 0.1, 2.9),
            Vec3::new(0.8, 0.1, 2.9),
        ],
        lin(0xd8dde2),
    );
    let gull_body = meshes.add(body.build(true, false));
    let mut wing = MeshBuilder::new();
    wing.quad_2(
        [
            Vec3::new(0.0, 0.0, -0.8),
            Vec3::new(0.0, 0.0, 0.9),
            Vec3::new(3.0, 0.0, 0.7),
            Vec3::new(3.2, 0.0, -0.5),
        ],
        lin(0xf8f8f4),
    );
    wing.tri_2(
        [
            Vec3::new(3.2, 0.0, -0.5),
            Vec3::new(3.0, 0.0, 0.7),
            Vec3::new(5.6, 0.0, 0.6),
        ],
        lin(0x3a3f47),
    );
    let gull_wing = meshes.add(wing.build(true, false));

    let mut fish = MeshBuilder::new();
    fish.ico_with(Vec3::ZERO, Vec3::new(1.6, 2.6, 6.0), 1, |d| {
        (1.0, if d.y > 0.0 { lin(0x2b6cb0) } else { lin(0xdfe9f0) })
    });
    fish.tri_2(
        [
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(0.0, 3.4, 9.0),
            Vec3::new(0.0, -3.4, 9.0),
        ],
        lin(0x2b6cb0),
    );
    fish.tri_2(
        [
            Vec3::new(0.0, 2.2, -1.0),
            Vec3::new(0.0, 4.8, 1.5),
            Vec3::new(0.0, 2.2, 2.5),
        ],
        lin(0x245a94),
    );
    let fish = meshes.add(fish.build(true, false));

    let mut flutter = MeshBuilder::new();
    flutter.tri_2(
        [Vec3::ZERO, Vec3::new(1.1, 0.0, -0.9), Vec3::new(1.3, 0.0, 0.5)],
        lin(0xffb703),
    );
    flutter.tri_2(
        [Vec3::ZERO, Vec3::new(1.0, 0.0, 0.6), Vec3::new(0.5, 0.0, 1.1)],
        lin(0xfb8500),
    );
    let flutter_wing = meshes.add(flutter.build(true, false));
    let mut thorax = MeshBuilder::new();
    thorax.ico(Vec3::ZERO, Vec3::new(0.14, 0.14, 0.7), 0, lin(0x2a2522));
    let thorax = meshes.add(thorax.build(true, false));

    for f in &built.fauna {
        let flyer = Flyer {
            kind: f.kind,
            skin: f.skin,
            rest: f.rest,
            seed: f.seed,
        };
        match f.kind {
            FaunaKind::Gull => {
                commands
                    .spawn((
                        Mesh3d(gull_body.clone()),
                        MeshMaterial3d(mats.matte.clone()),
                        Transform::default(),
                        flyer,
                        CourseEntity,
                    ))
                    .with_children(|p| {
                        for side in [-1.0f32, 1.0] {
                            p.spawn((
                                Mesh3d(gull_wing.clone()),
                                MeshMaterial3d(mats.matte.clone()),
                                Transform::from_scale(Vec3::new(side, 1.0, 1.0)),
                                Wing(side),
                            ));
                        }
                    });
            }
            FaunaKind::Butterfly => {
                commands
                    .spawn((
                        Mesh3d(thorax.clone()),
                        MeshMaterial3d(mats.matte.clone()),
                        Transform::default(),
                        flyer,
                        NotShadowCaster,
                        CourseEntity,
                    ))
                    .with_children(|p| {
                        for side in [-1.0f32, 1.0] {
                            p.spawn((
                                Mesh3d(flutter_wing.clone()),
                                MeshMaterial3d(mats.matte.clone()),
                                Transform::from_scale(Vec3::new(side, 1.0, 1.0)),
                                Wing(side),
                                NotShadowCaster,
                            ));
                        }
                    });
            }
            FaunaKind::Fish => {
                commands.spawn((
                    Mesh3d(fish.clone()),
                    MeshMaterial3d(mats.gloss.clone()),
                    Transform::default(),
                    flyer,
                    CourseEntity,
                ));
            }
        }
    }

    let critter = meshes.add(critter_mesh(built.critter_color).build(true, false));
    for (i, &u) in built.track.critters.iter().enumerate() {
        for k in 0..3 {
            commands.spawn((
                Mesh3d(critter.clone()),
                MeshMaterial3d(mats.matte.clone()),
                Transform::default(),
                Scuttler {
                    u: u + k as f32 * 2.2,
                    d: 0.0,
                    phase: i as f32 * 1.7 + k as f32 * 2.1,
                    speed: 0.32 + 0.09 * ((i + k) % 3) as f32,
                    squashed: 0.0,
                },
                CourseEntity,
            ));
        }
    }
}

pub fn fly(
    time: Res<Time>,
    course: Res<Course>,
    mut flyers: Query<(&Flyer, &mut Transform, &Children)>,
    mut fish: Query<(&Flyer, &mut Transform), Without<Children>>,
    mut wings: Query<(&Wing, &mut Transform), (Without<Flyer>, Without<Scuttler>)>,
) {
    let t = time.elapsed_secs();
    for (f, mut tf, children) in &mut flyers {
        let anchor = f.skin.apply(&course.skin).transform_point3(f.rest);
        let (pos, dir, flap) = match f.kind {
            FaunaKind::Gull => {
                let radius = 28.0 + 22.0 * (f.seed * 3.1).sin().abs();
                let turn = if (f.seed * 7.0).sin() > 0.0 { 1.0 } else { -1.0 };
                let a = t * (0.22 + 0.08 * (f.seed * 1.3).sin()) * turn + f.seed;
                let bob = 5.0 * (t * 0.4 + f.seed).sin();
                let pos = anchor + Vec3::new(a.cos() * radius, bob, a.sin() * radius);
                let dir = Vec3::new(-a.sin(), 0.0, a.cos()) * turn;
                // Mostly gliding, with an occasional burst of flapping.
                let burst = ((t * 0.35 + f.seed).sin() * 3.0).clamp(0.0, 1.0);
                (pos, dir, 0.18 + 0.5 * burst * (t * 7.0 + f.seed).sin())
            }
            _ => {
                let a = t * 0.9 + f.seed;
                let pos = anchor
                    + Vec3::new(
                        (a * 0.7).sin() * 5.0 + (a * 1.9).sin() * 1.5,
                        2.5 + 1.5 * (a * 2.3).sin(),
                        (a * 0.9 + 1.0).cos() * 5.0,
                    );
                let dir = Vec3::new((a * 0.7).cos(), 0.0, -(a * 0.9 + 1.0).sin());
                (pos, dir, 0.9 * (t * 16.0 + f.seed).sin())
            }
        };
        tf.translation = pos;
        tf.look_to(dir.normalize_or(Vec3::Z), Vec3::Y);
        for child in children.iter() {
            if let Ok((w, mut wt)) = wings.get_mut(child) {
                wt.rotation = Quat::from_rotation_z(w.0 * flap);
            }
        }
    }
    // Fish leap in arcs out of the sea. They live in the world, not on the beast.
    for (f, mut tf) in &mut fish {
        if f.kind != FaunaKind::Fish {
            continue;
        }
        let period = 6.0 + (f.seed * 2.0).sin().abs() * 5.0;
        let phase = ((t + f.seed * 3.0) / period).fract();
        let jump = (phase / 0.28).min(1.0);
        let heading = f.seed * 2.4;
        let dir = Vec3::new(heading.cos(), 0.0, heading.sin());
        let arc = (jump * PI).sin();
        let pos = f.rest + dir * (jump - 0.5) * 60.0 + Vec3::Y * (arc * 26.0 - 6.0);
        let pitch = (1.0 - 2.0 * jump) * 1.1;
        tf.translation = pos;
        tf.rotation = Quat::from_rotation_y(FRAC_PI_2 - heading + PI) * Quat::from_rotation_x(pitch);
        tf.scale = Vec3::splat(if phase < 0.28 { 1.0 } else { 0.001 });
    }
}

pub fn scuttle(time: Res<Time>, course: Res<Course>, mut q: Query<(&mut Scuttler, &mut Transform)>) {
    let (t, dt) = (time.elapsed_secs(), time.delta_secs());
    for (mut s, mut tf) in &mut q {
        let loc = course.track.at(s.u);
        // Back and forth across the road, pausing at each verge.
        let swing = (t * s.speed + s.phase).sin();
        let eased = (swing * 1.5).clamp(-1.0, 1.0);
        s.d = eased * (loc.half - 0.5);
        s.squashed = (s.squashed - dt).max(0.0);
        let moving = (swing * 1.5).abs() < 1.0;
        let heading = if (t * s.speed + s.phase).cos() > 0.0 {
            FRAC_PI_2
        } else {
            -FRAC_PI_2
        };
        let wobble = if moving {
            (t * 14.0 + s.phase * TAU).sin() * 0.08
        } else {
            0.0
        };
        tf.translation = loc.at(s.d) + loc.n * 0.05;
        tf.rotation = loc.rotation(heading) * Quat::from_rotation_z(wobble);
        tf.scale = if s.squashed > 0.0 {
            Vec3::new(1.3, 0.15, 1.3)
        } else {
            Vec3::ONE
        };
    }
}
