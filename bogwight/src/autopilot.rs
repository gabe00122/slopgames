//! `--autopilot`: a crude bot that drives the bogwight through the level so
//! the whole loop can be tested unattended. It lurks under water toward the
//! nearest hunter, bursts up at them, claws, and drowns whoever it grabs.

use crate::{
    game::WATER_Y,
    hunters::{Hunter, State},
    input::Controls,
    level::Terrain,
    player::{Bogwight, Medium},
};
use avian2d::prelude::*;
use bevy::prelude::*;

#[derive(Default)]
pub struct Bot {
    stuck: f32,
    grab_cd: f32,
}

pub fn drive(
    time: Res<Time>,
    mut controls: ResMut<Controls>,
    terrain: Res<Terrain>,
    q: Query<(&Bogwight, &Position, &LinearVelocity)>,
    hunters: Query<(&Hunter, &Position)>,
    mut bot: Local<Bot>,
) {
    let dt = time.delta_secs();
    let Ok((bw, pos, vel)) = q.single() else { return };
    bot.grab_cd -= dt;
    controls.mv = Vec2::ZERO;
    let Some((h, hp)) = hunters
        .iter()
        .filter(|(h, _)| h.state != State::Dead)
        .min_by(|a, b| (a.1.0.x - pos.0.x).abs().total_cmp(&(b.1.0.x - pos.0.x).abs()))
    else {
        return;
    };
    let to = hp.0 - pos.0;
    let dist = to.length();
    if let Some(held) = bw.held {
        let alive = hunters.get(held).is_ok_and(|(h, _)| h.state == State::Held);
        if bw.medium == Medium::Water && alive {
            // Hold them under.
            controls.mv = Vec2::new(0.0, -1.0);
        } else {
            controls.grab = true;
        }
        return;
    }
    if dist < 1.4 {
        controls.attack = true;
        if bw.medium == Medium::Water && bot.grab_cd <= 0.0 && h.state != State::Held {
            controls.grab = true;
            bot.grab_cd = 1.5;
        }
    }
    // A boat's crew: hang off the side and roll it over.
    if bw.grip.is_some() {
        controls.mv = Vec2::new(0.0, -1.0);
        return;
    }
    if h.boat.is_some() && bw.medium == Medium::Water && to.x.abs() < 2.0 && to.y < 2.0 && bot.grab_cd <= 0.0 {
        controls.grab = true;
        bot.grab_cd = 1.0;
    }
    match bw.medium {
        Medium::Water => {
            let dir = to.x.signum();
            let ahead = terrain.height(pos.0.x + dir * 1.5);
            let target_dry = hp.0.y > WATER_Y + 0.3;
            if to.x.abs() > 2.2 && ahead < WATER_Y - 1.2 {
                // Lurk a couple of metres down, clear of the floor.
                let lurk = (-1.8f32).max(ahead + 0.8).min(WATER_Y - 0.6);
                let depth_err = (lurk - pos.0.y).clamp(-1.0, 1.0);
                controls.mv = Vec2::new(dir, depth_err * 0.8).normalize_or_zero();
            } else if target_dry && (ahead > WATER_Y - 1.2 || to.x.abs() < 3.0) {
                // Shallows or a bank ahead: come up and leap out at them
                // (straight up when they're overhead, on a dock or a boat).
                controls.mv = if to.x.abs() < 1.6 {
                    Vec2::new(to.x * 0.3, 1.0).normalize()
                } else {
                    Vec2::new(dir, 0.8).normalize()
                };
                if bw.depth < 0.5 {
                    controls.dash = true;
                }
            } else {
                controls.mv = to.normalize_or_zero();
                if hp.0.y > pos.0.y + 0.5 && to.x.abs() < 2.5 {
                    controls.dash = true;
                }
            }
        }
        _ => {
            controls.mv = Vec2::new(to.x.signum(), 0.0);
            if vel.x.abs() < 0.4 && bw.grounded {
                bot.stuck += dt;
            } else {
                bot.stuck = 0.0;
            }
            if bot.stuck > 0.4 || (to.y > 1.2 && to.x.abs() < 2.5) {
                controls.jump = true;
                bot.stuck = 0.0;
            }
        }
    }
}

/// `--grip-test`: teleports next to the nearest boat, grabs its gunwale and
/// pulls down, printing the boat's angle each second.
pub fn grip_test(
    time: Res<Time>,
    mut controls: ResMut<Controls>,
    mut q: Query<(&mut Bogwight, &mut Position), Without<crate::boats::Boat>>,
    boats: Query<(Entity, &Position, &Rotation, &crate::boats::Boat)>,
    mut t: Local<f32>,
    mut printed: Local<u32>,
) {
    let Ok((mut bw, mut pos)) = q.single_mut() else { return };
    *t += time.delta_secs();
    let Some((e, bp, rot, boat)) = boats
        .iter()
        .min_by(|a, b| a.1.0.distance(pos.0).total_cmp(&b.1.0.distance(pos.0)))
    else {
        return;
    };
    if *t > 1.0 && *t < 1.1 && bw.grip.is_none() {
        pos.0 = bp.0 + Vec2::new(1.9, -0.6);
        bw.grip = Some((e, Vec2::new(1.9, 0.3)));
    }
    controls.mv = if boat.flipped { Vec2::ZERO } else { Vec2::new(0.0, -1.0) };
    if *t as u32 > *printed {
        *printed = *t as u32;
        println!(
            "t={:.0} boat angle {:.0} deg flipped {} gripping {} player ({:.1},{:.1})",
            *t,
            rot.as_degrees(),
            boat.flipped,
            bw.grip.is_some(),
            pos.0.x,
            pos.0.y
        );
    }
}
