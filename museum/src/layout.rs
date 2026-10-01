//! Floor plan: an octagonal rotunda with galleries radiating from its sides.
//!
//! Each gallery has a local frame whose origin is the rotunda center, with
//! local -Z pointing outward along the gallery and +X to the visitor's right
//! as they walk in.

use crate::content::{Exhibit, Gallery, Mount, Slot};
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

/// Distance from the rotunda center to the middle of each wall.
pub const ROT_APOTHEM: f32 = 16.0;
pub const ROT_WALL_H: f32 = 10.0;
pub const OPENING_W: f32 = 7.0;
pub const OPENING_H: f32 = 6.0;
pub const WALL_T: f32 = 0.5;
pub const GALLERY_W: f32 = 12.0;
pub const POOL_R: f32 = 4.2;
/// The entrance vestibule opens from the south side.
pub const ENTRANCE_SIDE: usize = 4;
pub const VESTIBULE_L: f32 = 12.0;
pub const VESTIBULE_W: f32 = 10.0;
pub const VESTIBULE_H: f32 = 7.0;

pub fn side_rot(side: usize) -> Quat {
    Quat::from_rotation_y(-(side as f32) * FRAC_PI_4)
}

pub fn rot_side_len() -> f32 {
    2.0 * ROT_APOTHEM * (PI / 8.0).tan()
}

/// Local position of an exhibit root and its yaw (so local +Z faces the
/// viewer) in gallery space.
pub fn slot_local(g: &Gallery, slot: Slot) -> (Vec3, f32) {
    let s = |d: f32| -(ROT_APOTHEM + d);
    match slot {
        Slot::Left(d) => (Vec3::new(-3.4, 0.0, s(d)), FRAC_PI_2),
        Slot::Right(d) => (Vec3::new(3.4, 0.0, s(d)), -FRAC_PI_2),
        Slot::End => (Vec3::new(0.0, 0.0, s(g.length - 4.5)), 0.0),
        Slot::Hang(x, d, y) => (Vec3::new(x, y, s(d)), 0.0),
        Slot::Center => (Vec3::ZERO, 0.0),
    }
}

/// World transform of an exhibit's root (on the floor, or at its hanging
/// point for suspended pieces).
pub fn exhibit_transform(galleries: &[Gallery], e: &Exhibit) -> Transform {
    match e.gallery {
        None => Transform::IDENTITY,
        Some(gi) => {
            let g = &galleries[gi];
            let (p, yaw) = slot_local(g, e.slot);
            let rot = side_rot(g.side);
            Transform::from_translation(rot * p).with_rotation(rot * Quat::from_rotation_y(yaw))
        }
    }
}

pub fn mount_height(m: Mount) -> f32 {
    match m {
        Mount::Plinth(_, h, _) => h,
        Mount::Round(_, h) => h,
        Mount::Floor(..) => 0.12,
        Mount::Cables => 0.0,
    }
}

/// Radius used to keep visitors from walking through the display.
pub fn mount_radius(m: Mount) -> f32 {
    match m {
        Mount::Plinth(w, _, d) => (w.max(d)) / 2.0 + 0.1,
        Mount::Round(r, _) => r + 0.1,
        Mount::Floor(w, d) => (w.max(d)) / 2.0 + 0.1,
        Mount::Cables => 0.0,
    }
}

/// 2D collision geometry on the XZ plane.
#[derive(Resource, Default)]
pub struct Walls {
    pub segments: Vec<(Vec2, Vec2)>,
    pub circles: Vec<(Vec2, f32)>,
}

fn xz(v: Vec3) -> Vec2 {
    Vec2::new(v.x, v.z)
}

impl Walls {
    /// Keeps visitors off a bench (2.6 m long, along local X).
    pub fn add_bench(&mut self, t: &Transform) {
        for x in [-0.8, 0.0, 0.8] {
            self.circles.push((xz(t.transform_point(Vec3::new(x, 0.0, 0.0))), 0.35));
        }
    }

    pub fn add_local(&mut self, rot: Quat, a: Vec3, b: Vec3) {
        self.segments.push((xz(rot * a), xz(rot * b)));
    }

    pub fn build(galleries: &[Gallery]) -> Self {
        let mut w = Walls::default();
        let half_side = rot_side_len() / 2.0;
        let r = ROT_APOTHEM;
        for side in 0..8 {
            let rot = side_rot(side);
            let o = OPENING_W / 2.0;
            w.add_local(rot, Vec3::new(-half_side - 0.1, 0.0, -r), Vec3::new(-o, 0.0, -r));
            w.add_local(rot, Vec3::new(o, 0.0, -r), Vec3::new(half_side + 0.1, 0.0, -r));
            for sx in [-1.0, 1.0] {
                w.circles
                    .push((xz(rot * Vec3::new(sx * (o + 1.15), 0.0, -r + 0.45)), 0.5));
            }
        }
        let mut hall = |side: usize, width: f32, length: f32| {
            let rot = side_rot(side);
            let hw = width / 2.0;
            let far = -(r + length);
            w.add_local(rot, Vec3::new(-hw, 0.0, -r), Vec3::new(-hw, 0.0, far));
            w.add_local(rot, Vec3::new(hw, 0.0, -r), Vec3::new(hw, 0.0, far));
            w.add_local(rot, Vec3::new(-hw, 0.0, far), Vec3::new(hw, 0.0, far));
        };
        for g in galleries {
            hall(g.side, GALLERY_W, g.length);
        }
        hall(ENTRANCE_SIDE, VESTIBULE_W, VESTIBULE_L);
        w.circles.push((Vec2::ZERO, POOL_R + 0.25));
        w
    }

    /// Pushes a circle of radius `r` at `p` out of all walls.
    pub fn resolve(&self, mut p: Vec2, r: f32) -> Vec2 {
        for _ in 0..4 {
            for (a, b) in &self.segments {
                let ab = *b - *a;
                let t = ((p - *a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
                let closest = *a + ab * t;
                let d = p - closest;
                let dist = d.length();
                if dist < r {
                    let n = if dist > 1e-5 {
                        d / dist
                    } else {
                        Vec2::new(-ab.y, ab.x).normalize()
                    };
                    p = closest + n * r;
                }
            }
            for (c, cr) in &self.circles {
                let d = p - *c;
                let dist = d.length();
                if dist < r + cr {
                    let n = if dist > 1e-5 { d / dist } else { Vec2::X };
                    p = *c + n * (r + cr);
                }
            }
        }
        p
    }
}

/// Which part of the museum a point is in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Area {
    Rotunda,
    Vestibule,
    Gallery(usize),
}

pub fn locate(galleries: &[Gallery], p: Vec3) -> Area {
    for (i, g) in galleries.iter().enumerate() {
        let l = side_rot(g.side).inverse() * p;
        if l.x.abs() < GALLERY_W / 2.0 + 0.5 && l.z < -ROT_APOTHEM && l.z > -(ROT_APOTHEM + g.length + 1.0) {
            return Area::Gallery(i);
        }
    }
    let l = side_rot(ENTRANCE_SIDE).inverse() * p;
    if l.x.abs() < VESTIBULE_W / 2.0 + 0.5 && l.z < -ROT_APOTHEM {
        return Area::Vestibule;
    }
    Area::Rotunda
}

/// True if the straight line between two points crosses a wall.
pub fn blocked(walls: &Walls, a: Vec3, b: Vec3) -> bool {
    let (p, q) = (xz(a), xz(b));
    let cross = |u: Vec2, v: Vec2| u.x * v.y - u.y * v.x;
    walls.segments.iter().any(|(c, d)| {
        let r = q - p;
        let s = *d - *c;
        let den = cross(r, s);
        if den.abs() < 1e-8 {
            return false;
        }
        let t = cross(*c - p, s) / den;
        let u = cross(*c - p, r) / den;
        (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)
    })
}
