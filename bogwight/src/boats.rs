//! Wooden skiffs. A boat is a dynamic body floating on buoyancy forces; its
//! crew stand in it on friction alone. While anyone aboard can paddle it's
//! pushed along its patrol, or toward the bogwight once they've spotted it.
//! Rock it hard enough and it goes over.

use crate::{
    audio::{Sfx, Sound},
    game::*,
    hunters::{Hunter, HunterKind, State},
    level::BoatSpawn,
    models::{Mats, Models},
    water::Floater,
};
use avian2d::prelude::*;
use bevy::prelude::*;

#[derive(Component)]
pub struct Boat {
    pub patrol: (f32, f32),
    pub dir: f32,
    pub flipped: bool,
    tipped: f32,
    oar: Entity,
    oar_phase: f32,
    creak: f32,
    pub crew: usize,
}

pub fn spawn_boat(commands: &mut Commands, models: &Models, mats: &Mats, s: &BoatSpawn) -> Entity {
    let mut points = vec![];
    for j in 0..3 {
        for i in 0..9 {
            // Sample the trapezoid hull.
            let v = (j as f32 + 0.5) / 3.0;
            let y = -0.36 + v * 0.68;
            let x0 = -1.75 - 0.35 * v;
            let x1 = 1.65 + 0.6 * v;
            let u = (i as f32 + 0.5) / 9.0;
            points.push(Vec2::new(x0 + (x1 - x0) * u, y));
        }
    }
    let oar = commands
        .spawn((
            Mesh3d(models.oar.clone()),
            MeshMaterial3d(mats.matte.clone()),
            Transform::from_xyz(-1.85, 0.3, 0.62),
        ))
        .id();
    let e = commands
        .spawn((
            Transform::from_xyz(s.x, 0.08, 0.0),
            Visibility::default(),
            RigidBody::Dynamic,
            Collider::compound(vec![
                (
                    Vec2::new(-0.05, -0.3),
                    Rotation::IDENTITY,
                    Collider::rectangle(3.45, 0.12),
                ),
                (
                    Vec2::new(-1.93, -0.03),
                    Rotation::radians(0.49),
                    Collider::rectangle(0.1, 0.72),
                ),
                (
                    Vec2::new(1.95, -0.01),
                    Rotation::radians(-0.71),
                    Collider::rectangle(0.1, 0.9),
                ),
            ]),
            Mass(110.0),
            CenterOfMass(Vec2::new(0.0, -0.22)),
            Friction::new(0.8),
            Restitution::new(0.05),
            CollisionLayers::new(
                Layer::Boat,
                [
                    Layer::Ground,
                    Layer::Player,
                    Layer::Hunter,
                    Layer::Prop,
                    Layer::Bolt,
                    Layer::Boat,
                ],
            ),
            Floater {
                points,
                cell: 0.114,
                buoyancy: 4.3,
                drag: Vec2::new(0.35, 3.0),
                angular_drag: 2.5,
                submerged: 0.0,
                splashy: true,
            },
            Boat {
                patrol: s.patrol,
                dir: 1.0,
                flipped: false,
                tipped: 0.0,
                oar,
                oar_phase: 0.0,
                creak: 0.0,
                crew: 0,
            },
            LevelEntity,
        ))
        .with_child((
            Mesh3d(models.boat.clone()),
            MeshMaterial3d(mats.matte.clone()),
            Transform::IDENTITY,
        ))
        .id();
    commands.entity(oar).insert(ChildOf(e));
    e
}

pub fn steer(
    time: Res<Time>,
    stealth: Res<crate::player::Stealth>,
    mut boats: Query<(Entity, &mut Boat, Forces, &mut Floater)>,
    hunters: Query<&Hunter>,
    mut sfx: MessageWriter<Sfx>,
    mut popups: MessageWriter<Popup>,
    mut noises: MessageWriter<Noise>,
    mut session: ResMut<Session>,
) {
    let dt = time.delta_secs();
    for (e, mut boat, mut forces, mut floater) in &mut boats {
        let pos = forces.position().0;
        let rot = *forces.rotation();
        let angle = rot.as_radians();
        let vel = forces.linear_velocity();
        let spin = forces.angular_velocity();
        // Capsizing.
        if angle.abs() > 1.7 {
            boat.tipped += dt;
        } else {
            boat.tipped = 0.0;
        }
        if boat.tipped > 0.6 && !boat.flipped {
            boat.flipped = true;
            // Swamped: it floats low and stays over.
            floater.buoyancy = 2.2;
            session.stats.boats_flipped += 1;
            popups.write(Popup::new(pos + Vec2::Y * 1.2, "CAPSIZED", 0x5ad8ff, true));
            sfx.write(Sfx::at(Sound::Capsize, pos));
            noises.write(Noise { pos, radius: 12.0 });
        }
        boat.creak -= dt;
        if spin.abs() > 1.4 && boat.creak <= 0.0 {
            sfx.write(Sfx::at(Sound::Creak, pos));
            boat.creak = 0.8;
        }
        let crew: Vec<&Hunter> = hunters
            .iter()
            .filter(|h| h.boat == Some(e) && matches!(h.state, State::Patrol | State::Suspicious | State::Hunt))
            .collect();
        boat.crew = crew.len();
        if crew.is_empty() || boat.flipped || angle.abs() > 0.6 || floater.submerged < 0.15 {
            continue;
        }
        let (lo, hi) = (boat.patrol.0 - 4.0, boat.patrol.1 + 4.0);
        let hunting = crew.iter().find(|h| h.state == State::Hunt);
        let target = if let Some(h) = hunting {
            let seen = if h.unseen < 0.5 { stealth.pos } else { h.last_seen };
            let shooter = crew
                .iter()
                .any(|h| h.kind.has_crossbow() && h.kind != HunterKind::Warden);
            if shooter {
                // Hold off at crossbow range.
                let side = (pos.x - seen.x).signum();
                seen.x + side * 7.0
            } else {
                seen.x
            }
        } else {
            let end = if boat.dir > 0.0 { boat.patrol.1 } else { boat.patrol.0 };
            if (end - pos.x).abs() < 1.0 || (end - pos.x).signum() != boat.dir {
                boat.dir = -boat.dir;
            }
            end
        }
        .clamp(lo, hi);
        let push = ((target - pos.x) * 90.0 - vel.x * 130.0).clamp(-380.0, 380.0);
        let at = pos + rot * Vec2::new(0.0, -0.25);
        forces.apply_force_at_point(Vec2::new(push, 0.0), at);
        boat.oar_phase += dt * (1.0 + vel.x.abs() * 1.5);
    }
}

pub fn animate_oars(boats: Query<&Boat>, mut oars: Query<&mut Transform>) {
    for b in &boats {
        if let Ok(mut tf) = oars.get_mut(b.oar) {
            let a = if b.crew > 0 && !b.flipped {
                b.oar_phase.sin() * 0.45 - 0.35
            } else {
                -0.9
            };
            tf.rotation = Quat::from_rotation_z(a) * Quat::from_rotation_x(0.35);
        }
    }
}
