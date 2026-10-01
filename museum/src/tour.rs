//! Guided tour: glides the visitor from exhibit to exhibit, pausing at each.

use crate::{
    content,
    exhibits::ExhibitIndex,
    layout::{self, Area, ROT_APOTHEM},
    player::{Control, EYE, Player},
};
use bevy::prelude::*;

const SPEED: f32 = 2.6;
const DWELL: f32 = 11.0;

#[derive(Resource, Default)]
pub struct Tour {
    pub active: bool,
    pub stop: usize,
    pub focus: Option<usize>,
    path: Vec<Vec3>,
    dwell: f32,
}

/// Point just inside the entrance of an area, on its center line.
fn doorway(area: Area, inside: bool) -> Option<Vec3> {
    let galleries = content::galleries();
    let (side, d) = match area {
        Area::Rotunda => return None,
        Area::Vestibule => (layout::ENTRANCE_SIDE, if inside { 2.0 } else { -2.5 }),
        Area::Gallery(g) => (galleries[g].side, if inside { 2.0 } else { -2.5 }),
    };
    Some(layout::side_rot(side) * Vec3::new(0.0, 0.0, -(ROT_APOTHEM + d)))
}

/// Route from `from` to exhibit `to`, following aisles and skirting the pool.
fn route(index: &ExhibitIndex, from: Vec3, to: usize) -> Vec<Vec3> {
    let galleries = content::galleries();
    let target = &index.0[to];
    let here = layout::locate(&galleries, from);
    let there = target.gallery.map(Area::Gallery).unwrap_or(Area::Rotunda);
    let mut pts = vec![Vec3::new(from.x, 0.0, from.z)];
    if here != there {
        if here != Area::Rotunda {
            pts.extend(doorway(here, true));
            pts.extend(doorway(here, false));
        }
        if let Some(p) = doorway(there, false) {
            pts.push(p);
        }
        if there != Area::Rotunda {
            pts.extend(doorway(there, true));
        }
    }
    pts.push(target.aisle);
    pts.push(Vec3::new(target.view.x, 0.0, target.view.z));
    // Walk around the reflecting pool rather than through it.
    let mut out: Vec<Vec3> = Vec::new();
    for p in pts {
        if let Some(prev) = out.last().copied() {
            let (a, b) = (Vec2::new(prev.x, prev.z), Vec2::new(p.x, p.z));
            let ab = b - a;
            let t = (-a.dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            let closest = a + ab * t;
            if closest.length() < layout::POOL_R + 1.2 && ab.length() > 0.5 {
                let mid = (a.normalize_or(Vec2::X) + b.normalize_or(Vec2::X))
                    .normalize_or(Vec2::new(-ab.y, ab.x).normalize());
                let detour = mid * (layout::POOL_R + 2.6);
                out.push(Vec3::new(detour.x, 0.0, detour.y));
            }
        }
        if out.last().is_none_or(|l| l.distance(p) > 0.05) {
            out.push(p);
        }
    }
    out
}

fn start_leg(tour: &mut Tour, index: &ExhibitIndex, from: Vec3) {
    tour.path = route(index, from, tour.stop);
    tour.dwell = 0.0;
    tour.focus = None;
}

/// Starts the tour at `stop`, or at the exhibit nearest to `here`.
pub fn begin(tour: &mut Tour, index: &ExhibitIndex, here: Vec3, stop: Option<usize>) {
    tour.active = true;
    tour.stop = stop.unwrap_or_else(|| {
        (0..index.0.len())
            .min_by(|a, b| {
                index.0[*a]
                    .view
                    .distance(here)
                    .total_cmp(&index.0[*b].view.distance(here))
            })
            .unwrap_or(0)
    });
    start_leg(tour, index, here);
}

pub fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut control: ResMut<Control>,
    mut tour: ResMut<Tour>,
    index: Res<ExhibitIndex>,
    player: Single<&Transform, With<Player>>,
) {
    if !control.started || control.paused {
        return;
    }
    let n = index.0.len();
    if keys.just_pressed(KeyCode::KeyT) || (tour.active && keys.just_pressed(KeyCode::Escape)) {
        tour.active = !tour.active;
        control.touring = tour.active;
        if tour.active {
            begin(&mut tour, &index, player.translation, None);
        } else {
            tour.focus = None;
        }
        return;
    }
    if !tour.active {
        return;
    }
    let step = if keys.any_just_pressed([KeyCode::KeyN, KeyCode::ArrowRight]) {
        1
    } else if keys.any_just_pressed([KeyCode::KeyP, KeyCode::ArrowLeft]) {
        n - 1
    } else {
        0
    };
    if step != 0 {
        tour.stop = (tour.stop + step) % n;
        start_leg(&mut tour, &index, player.translation);
    }
}

pub fn drive(
    time: Res<Time>,
    mut tour: ResMut<Tour>,
    index: Res<ExhibitIndex>,
    mut q: Query<(&mut Player, &mut Transform)>,
) {
    if !tour.active {
        return;
    }
    let Ok((mut p, mut t)) = q.single_mut() else { return };
    let dt = time.delta_secs().min(0.05);
    let target = index.0[tour.stop].clone();
    let pos = Vec3::new(t.translation.x, 0.0, t.translation.z);

    // Walk along the path.
    // Keep the exhibit a little left of center, clear of the info panel.
    let to_focus = (target.focus - t.translation).normalize_or(Vec3::NEG_Z);
    let right = to_focus.cross(Vec3::Y).normalize_or(Vec3::X);
    let mut look_at = target.focus + right * target.radius * 0.45;
    if tour.path.len() > 1 {
        let next = tour.path[1];
        let to = next - pos;
        let d = to.length();
        let step = SPEED * dt;
        let new = if d <= step {
            tour.path.remove(0);
            next
        } else {
            pos + to / d * step
        };
        tour.path[0] = new;
        t.translation = Vec3::new(new.x, EYE, new.z);
        let remaining: f32 =
            tour.path.windows(2).map(|w| w[0].distance(w[1])).sum::<f32>() + new.distance(tour.path[0]);
        if remaining > 3.0 && d > 0.01 {
            look_at = Vec3::new(new.x, EYE, new.z) + to / d * 5.0;
        }
    } else {
        tour.focus = Some(tour.stop);
        tour.dwell += dt;
        if tour.dwell > DWELL {
            tour.stop = (tour.stop + 1) % index.0.len();
            let from = t.translation;
            start_leg(&mut tour, &index, from);
        }
    }

    // Turn smoothly toward whatever we should be looking at.
    let dir = (look_at - t.translation).normalize_or(Vec3::NEG_Z);
    let want_yaw = (-dir.x).atan2(-dir.z);
    let want_pitch = dir.y.clamp(-1.0, 1.0).asin();
    let mut dy = want_yaw - p.yaw;
    while dy > std::f32::consts::PI {
        dy -= std::f32::consts::TAU;
    }
    while dy < -std::f32::consts::PI {
        dy += std::f32::consts::TAU;
    }
    let k = (2.5 * dt).min(1.0);
    p.yaw += dy * k;
    p.pitch += (want_pitch - p.pitch) * k;
    t.rotation = Quat::from_euler(EulerRot::YXZ, p.yaw, p.pitch, 0.0);
}
