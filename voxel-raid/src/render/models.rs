//! Box-built models: humanoids and modular weapons (world and viewmodel).

use glam::{Mat4, Vec3};

use super::MeshBuilder;
use crate::player::BodyPart;
use crate::weapons::ballistics::{crouch_scale, HUMANOID_PARTS};
use crate::weapons::{AttachmentId, ReceiverId, Slot, Weapon};

const METAL: [u8; 4] = [46, 47, 50, 255];
const METAL_LIGHT: [u8; 4] = [70, 72, 76, 255];
const POLYMER: [u8; 4] = [30, 30, 32, 255];
const WOOD: [u8; 4] = [130, 74, 38, 255];
const PLUM: [u8; 4] = [104, 50, 40, 255];

fn cube(mb: &mut MeshBuilder, m: Mat4, center: [f32; 3], size: [f32; 3], color: [u8; 4]) {
    mb.add_cube(
        m * Mat4::from_translation(Vec3::from(center)) * Mat4::from_scale(Vec3::from(size)),
        color,
        false,
    );
}

fn cube_rx(mb: &mut MeshBuilder, m: Mat4, center: [f32; 3], size: [f32; 3], rx: f32, color: [u8; 4]) {
    mb.add_cube(
        m * Mat4::from_translation(Vec3::from(center)) * Mat4::from_rotation_x(rx) * Mat4::from_scale(Vec3::from(size)),
        color,
        false,
    );
}

/// Visible barrel length beyond the handguard.
fn barrel_len(w: &Weapon) -> f32 {
    match w.receiver {
        ReceiverId::Grach => 0.0,
        ReceiverId::Vityaz => 0.1,
        _ => match w.attachment(Slot::Barrel) {
            Some(AttachmentId::Ak74Barrel206) => 0.04,
            Some(AttachmentId::AkmBarrel520) => 0.3,
            Some(_) => 0.2,
            None => 0.0,
        },
    }
}

fn muzzle_len(a: Option<AttachmentId>) -> f32 {
    match a {
        Some(AttachmentId::Pbs4Suppressor) | Some(AttachmentId::Pbs1Suppressor) => 0.22,
        Some(AttachmentId::Suppressor9mm) => 0.17,
        Some(AttachmentId::Dtk1Compensator) => 0.09,
        Some(_) => 0.065,
        None => 0.0,
    }
}

/// Height of the sight line above the model origin (for ADS alignment).
pub fn sight_height(w: &Weapon) -> f32 {
    match (w.receiver, w.attachment(Slot::Sight)) {
        (ReceiverId::Grach, Some(_)) => 0.062,
        (ReceiverId::Grach, None) => 0.047,
        (_, Some(AttachmentId::CobraRedDot)) => 0.098,
        (_, Some(AttachmentId::Holo1p87)) => 0.1,
        (_, Some(AttachmentId::Pso1Scope)) => 0.104,
        (_, _) => 0.07,
    }
}

/// Muzzle position in model space.
pub fn muzzle_point(w: &Weapon) -> Vec3 {
    if w.receiver == ReceiverId::Grach {
        let z = -0.165 - muzzle_len(w.attachment(Slot::Muzzle));
        return Vec3::new(0.0, 0.02, z);
    }
    let z = -0.43 - barrel_len(w) - muzzle_len(w.attachment(Slot::Muzzle));
    Vec3::new(0.0, 0.005, z)
}

/// Build a weapon model. Origin at the receiver, barrel along -Z.
pub fn weapon_model(mb: &mut MeshBuilder, w: &Weapon, m: Mat4) {
    weapon_model_opts(mb, w, m, false);
}

/// `hide_stock` is used when aiming down sights (the stock is against the cheek).
pub fn weapon_model_opts(mb: &mut MeshBuilder, w: &Weapon, m: Mat4, hide_stock: bool) {
    if w.receiver == ReceiverId::Grach {
        pistol_model(mb, w, m);
        return;
    }
    let is_ak = matches!(w.receiver, ReceiverId::Ak74n | ReceiverId::Akm);
    let furniture = match w.receiver {
        ReceiverId::Akm => WOOD,
        ReceiverId::Ak74n => PLUM,
        _ => POLYMER,
    };
    // Receiver, dust cover, handguard, gas tube.
    cube(mb, m, [0.0, 0.0, -0.05], [0.05, 0.08, 0.36], METAL);
    cube(mb, m, [0.0, 0.045, -0.06], [0.044, 0.014, 0.3], METAL_LIGHT);
    cube(mb, m, [0.0, -0.006, -0.33], [0.058, 0.064, 0.2], furniture);
    if is_ak {
        cube(mb, m, [0.0, 0.037, -0.33], [0.03, 0.026, 0.21], furniture);
    }
    // Barrel + front sight.
    let bl = barrel_len(w);
    if bl > 0.0 {
        cube(mb, m, [0.0, 0.005, -0.43 - bl / 2.0], [0.022, 0.022, bl], METAL);
        cube(mb, m, [0.0, 0.045, -0.43 - bl + 0.03], [0.01, 0.05, 0.012], METAL);
    } else if w.attachment(Slot::Barrel).is_none() && w.receiver.has_slot(Slot::Barrel) {
        // No barrel: nothing sticks out.
    }
    // Muzzle device.
    let mz = w.attachment(Slot::Muzzle);
    let ml = muzzle_len(mz);
    if ml > 0.0 && (bl > 0.0 || w.receiver == ReceiverId::Vityaz) {
        let z0 = -0.43 - bl;
        let (thick, color) = match mz {
            Some(AttachmentId::Pbs4Suppressor)
            | Some(AttachmentId::Pbs1Suppressor)
            | Some(AttachmentId::Suppressor9mm) => (0.05, [36, 36, 38, 255]),
            Some(AttachmentId::Dtk1Compensator) => (0.038, [40, 40, 42, 255]),
            _ => (0.032, METAL_LIGHT),
        };
        cube(mb, m, [0.0, 0.005, z0 - ml / 2.0], [thick, thick, ml], color);
    }
    // Rear sight with a notch.
    cube(mb, m, [0.0, 0.054, 0.02], [0.032, 0.006, 0.02], METAL);
    cube(mb, m, [-0.011, 0.062, 0.02], [0.01, 0.012, 0.02], METAL);
    cube(mb, m, [0.011, 0.062, 0.02], [0.01, 0.012, 0.02], METAL);
    // Pistol grip.
    cube_rx(mb, m, [0.0, -0.085, 0.08], [0.034, 0.1, 0.045], 0.3, POLYMER);
    // Stock.
    let stock = if hide_stock { None } else { w.attachment(Slot::Stock) };
    match stock {
        Some(AttachmentId::AkWoodStock) => cube_rx(mb, m, [0.0, -0.035, 0.27], [0.045, 0.085, 0.26], -0.12, WOOD),
        Some(AttachmentId::Ak74PolymerStock) => cube_rx(mb, m, [0.0, -0.035, 0.27], [0.045, 0.085, 0.26], -0.12, PLUM),
        Some(AttachmentId::ZhukovStock) => {
            cube(mb, m, [0.0, -0.01, 0.22], [0.04, 0.04, 0.16], POLYMER);
            cube(mb, m, [0.0, -0.035, 0.33], [0.045, 0.1, 0.06], POLYMER);
        }
        Some(AttachmentId::Pp19FoldingStock) => {
            cube(mb, m, [0.0, 0.0, 0.24], [0.018, 0.06, 0.22], METAL);
            cube(mb, m, [0.0, -0.02, 0.35], [0.03, 0.09, 0.02], METAL);
        }
        Some(AttachmentId::Pp19CqbStock) => cube_rx(mb, m, [0.0, -0.03, 0.25], [0.045, 0.08, 0.2], -0.08, POLYMER),
        _ => {}
    }
    // Magazine.
    match w.attachment(Slot::Magazine) {
        Some(AttachmentId::Rpk16Drum95) | Some(AttachmentId::AkmDrum75) => {
            cube(mb, m, [0.0, -0.1, -0.1], [0.012, 0.06, 0.04], METAL);
            cube(mb, m, [0.0, -0.17, -0.1], [0.09, 0.13, 0.13], [52, 52, 50, 255]);
        }
        Some(a) => {
            let (len, color) = match a {
                AttachmentId::Ak74Mag45 | AttachmentId::AkmMag40 => (0.23, [58, 50, 44, 255]),
                AttachmentId::Ak74Mag30 => (0.17, PLUM),
                AttachmentId::AkmMag30 => (0.17, [150, 72, 40, 255]),
                AttachmentId::Pp19Mag20 => (0.12, POLYMER),
                _ => (0.17, POLYMER),
            };
            let curve = if is_ak { 0.38 } else { 0.1 };
            cube_rx(
                mb,
                m,
                [0.0, -0.04 - len / 2.0, -0.12 - len * 0.2],
                [0.034, len, 0.07],
                curve,
                color,
            );
        }
        None => {}
    }
    // Foregrip.
    match w.attachment(Slot::Grip) {
        Some(AttachmentId::AfgGrip) => cube(mb, m, [0.0, -0.05, -0.34], [0.03, 0.03, 0.09], POLYMER),
        Some(_) => cube_rx(mb, m, [0.0, -0.08, -0.34], [0.028, 0.08, 0.032], 0.15, POLYMER),
        None => {}
    }
    // Sight.
    match w.attachment(Slot::Sight) {
        Some(AttachmentId::CobraRedDot) => {
            cube(mb, m, [0.0, 0.058, -0.04], [0.036, 0.016, 0.08], METAL);
            cube(mb, m, [0.0, 0.09, -0.03], [0.044, 0.05, 0.03], METAL_LIGHT);
        }
        Some(AttachmentId::Holo1p87) => {
            cube(mb, m, [0.0, 0.06, -0.04], [0.04, 0.02, 0.09], METAL);
            cube(mb, m, [0.0, 0.1, -0.04], [0.056, 0.056, 0.07], [40, 42, 40, 255]);
        }
        Some(AttachmentId::Pso1Scope) => {
            cube(mb, m, [0.022, 0.06, -0.02], [0.02, 0.05, 0.08], METAL);
            cube(mb, m, [0.0, 0.104, -0.02], [0.036, 0.036, 0.26], [28, 28, 30, 255]);
            cube(mb, m, [0.0, 0.104, -0.16], [0.046, 0.046, 0.03], [28, 28, 30, 255]);
            cube(mb, m, [0.0, 0.13, -0.02], [0.02, 0.02, 0.025], [28, 28, 30, 255]);
        }
        _ => {}
    }
}

fn pistol_model(mb: &mut MeshBuilder, w: &Weapon, m: Mat4) {
    cube(mb, m, [0.0, 0.02, -0.07], [0.03, 0.036, 0.19], METAL);
    cube(mb, m, [0.0, -0.012, -0.06], [0.028, 0.022, 0.15], POLYMER);
    cube_rx(mb, m, [0.0, -0.07, 0.02], [0.03, 0.1, 0.045], 0.25, POLYMER);
    cube(mb, m, [0.0, 0.043, -0.155], [0.006, 0.012, 0.008], METAL);
    cube(mb, m, [0.0, 0.043, 0.015], [0.02, 0.01, 0.008], METAL);
    if let Some(AttachmentId::Suppressor9mm) = w.attachment(Slot::Muzzle) {
        cube(
            mb,
            m,
            [0.0, 0.02, -0.165 - 0.085],
            [0.036, 0.036, 0.17],
            [36, 36, 38, 255],
        );
    }
    if w.attachment(Slot::Sight).is_some() {
        cube(mb, m, [0.0, 0.048, -0.03], [0.024, 0.024, 0.03], METAL_LIGHT);
    }
    if let Some(AttachmentId::PistolMag33) = w.attachment(Slot::Magazine) {
        cube(mb, m, [0.0, -0.16, 0.035], [0.026, 0.1, 0.035], POLYMER);
    }
}

#[derive(Clone, Copy)]
pub struct HumanoidLook {
    pub jacket: [u8; 4],
    pub pants: [u8; 4],
    pub skin: [u8; 4],
    pub hat: Option<[u8; 4]>,
    pub helmet: bool,
    pub armor: bool,
}

pub struct HumanoidPose {
    pub feet: Vec3,
    pub yaw: f32,
    pub crouch: f32,
    pub walk_phase: f32,
    pub moving: bool,
    pub aiming: bool,
    pub aim_pitch: f32,
    pub dead: bool,
    pub hit_flash: bool,
}

fn tint(c: [u8; 4], flash: bool) -> [u8; 4] {
    if flash {
        [255, (c[1] as u32 / 3) as u8, (c[2] as u32 / 3) as u8, 255]
    } else {
        c
    }
}

pub fn humanoid(mb: &mut MeshBuilder, pose: &HumanoidPose, look: &HumanoidLook, weapon: Option<&Weapon>) {
    let s = crouch_scale(pose.crouch);
    let base = if pose.dead {
        Mat4::from_translation(pose.feet + Vec3::Y * 0.14)
            * Mat4::from_rotation_y(pose.yaw)
            * Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2)
    } else {
        Mat4::from_translation(pose.feet) * Mat4::from_rotation_y(pose.yaw) * Mat4::from_scale(Vec3::new(1.0, s, 1.0))
    };
    let swing = if pose.moving && !pose.dead {
        (pose.walk_phase * 3.2).sin() * 0.6
    } else {
        0.0
    };
    let f = pose.hit_flash;
    for (part, c, h) in HUMANOID_PARTS {
        let center = Vec3::from(c);
        let size = Vec3::from(h) * 2.0;
        let (color, pivot, angle) = match part {
            BodyPart::Head => (look.skin, None, 0.0),
            BodyPart::Thorax => (if look.armor { [58, 66, 48, 255] } else { look.jacket }, None, 0.0),
            BodyPart::Stomach => (look.jacket, None, 0.0),
            BodyPart::LeftArm => {
                let a = if pose.aiming { 1.35 } else { -swing };
                (look.jacket, Some(Vec3::new(c[0], 1.46, 0.0)), a)
            }
            BodyPart::RightArm => {
                let a = if pose.aiming { 1.2 } else { swing };
                (look.jacket, Some(Vec3::new(c[0], 1.46, 0.0)), a)
            }
            BodyPart::LeftLeg => (look.pants, Some(Vec3::new(c[0], 0.86, 0.0)), swing),
            BodyPart::RightLeg => (look.pants, Some(Vec3::new(c[0], 0.86, 0.0)), -swing),
        };
        let m = match pivot {
            Some(p) if angle != 0.0 => {
                base * Mat4::from_translation(p)
                    * Mat4::from_rotation_x(angle)
                    * Mat4::from_translation(center - p)
                    * Mat4::from_scale(size)
            }
            _ => base * Mat4::from_translation(center) * Mat4::from_scale(size),
        };
        mb.add_cube(m, tint(color, f), false);
    }
    // Face detail and headgear.
    let face =
        base * Mat4::from_translation(Vec3::new(0.0, 1.66, -0.131)) * Mat4::from_scale(Vec3::new(0.16, 0.03, 0.01));
    mb.add_cube(face, [30, 26, 24, 255], false);
    if look.helmet {
        let m = base * Mat4::from_translation(Vec3::new(0.0, 1.74, 0.0)) * Mat4::from_scale(Vec3::new(0.3, 0.14, 0.3));
        mb.add_cube(m, tint([72, 80, 60, 255], f), false);
    } else if let Some(hat) = look.hat {
        let m = base * Mat4::from_translation(Vec3::new(0.0, 1.75, 0.0)) * Mat4::from_scale(Vec3::new(0.28, 0.1, 0.28));
        mb.add_cube(m, tint(hat, f), false);
    }
    // Gun held at the chest (or lying next to the corpse).
    if let Some(w) = weapon {
        let gm = if pose.dead {
            Mat4::from_translation(pose.feet + Vec3::new(0.0, 0.05, 0.0))
                * Mat4::from_rotation_y(pose.yaw + 1.2)
                * Mat4::from_translation(Vec3::new(0.5, 0.0, 0.0))
                * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2)
        } else if pose.aiming {
            base * Mat4::from_translation(Vec3::new(0.1, 1.38, -0.4)) * Mat4::from_rotation_x(pose.aim_pitch)
        } else {
            base * Mat4::from_translation(Vec3::new(0.12, 1.1, -0.2)) * Mat4::from_rotation_x(-0.5)
        };
        weapon_model(mb, w, gm);
    }
}

pub struct ViewmodelParams {
    pub ads: f32,
    pub kick: f32,
    /// Reload progress 0..1 while reloading.
    pub reload: Option<f32>,
    pub draw: f32,
    pub sprint: f32,
    pub walk_phase: f32,
    pub moving: f32,
    pub flash: bool,
    pub sway: glam::Vec2,
    pub time: f32,
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// First-person weapon + hands in view space.
pub fn viewmodel(mb: &mut MeshBuilder, w: &Weapon, p: &ViewmodelParams) {
    let pistol = w.receiver == ReceiverId::Grach;
    let sh = sight_height(w);
    let hip = if pistol {
        Vec3::new(0.15, -0.17, -0.42)
    } else {
        Vec3::new(0.19, -0.21, -0.52)
    };
    let ads_pos = Vec3::new(0.0, -sh, if pistol { -0.4 } else { -0.46 });
    let a = smooth(p.ads);
    let mut pos = hip.lerp(ads_pos, a);
    let bob = p.moving * (1.0 - 0.85 * a);
    pos.x += (p.walk_phase * 1.6).sin() * 0.014 * bob;
    pos.y -= (p.walk_phase * 3.2).sin().abs() * 0.012 * bob;
    // Idle breathing.
    pos.y += (p.time * 1.7).sin() * 0.002 * (1.0 - 0.6 * a);
    pos.z += p.kick * if pistol { 0.03 } else { 0.045 };
    pos.y -= p.draw * 0.7;
    pos.x += p.sway.x * (1.0 - 0.7 * a);
    pos.y += p.sway.y * (1.0 - 0.7 * a);
    let reload = p.reload.map(|f| (f * std::f32::consts::PI).sin()).unwrap_or(0.0);
    let sprint = p.sprint * (1.0 - a);
    let kick_pitch = p.kick * if pistol { 0.12 } else { 0.05 } * (1.0 - 0.4 * a);
    let m = Mat4::from_translation(pos)
        * Mat4::from_rotation_x(kick_pitch - reload * 0.55 - sprint * 0.3)
        * Mat4::from_rotation_z(reload * 0.6)
        * Mat4::from_rotation_y(sprint * 0.7);
    weapon_model_opts(mb, w, m, p.ads > 0.7);

    let glove = [52, 50, 44, 255];
    let sleeve = [72, 78, 58, 255];
    // Right hand on the pistol grip, forearm back to the bottom-right of the screen.
    let grip = if pistol {
        Vec3::new(0.0, -0.07, 0.02)
    } else {
        Vec3::new(0.0, -0.085, 0.08)
    };
    mb.add_cube(
        m * Mat4::from_translation(grip + Vec3::new(0.012, 0.0, 0.0)) * Mat4::from_scale(Vec3::new(0.05, 0.075, 0.07)),
        glove,
        false,
    );
    let rg = m.transform_point3(grip + Vec3::new(0.02, -0.03, 0.04));
    mb.add_beam(rg, rg + Vec3::new(0.12, -0.2, 0.3), 0.075, sleeve, false);
    // Left hand on the handguard (or supporting the pistol grip).
    let support = if pistol {
        Vec3::new(-0.025, -0.085, 0.0)
    } else {
        Vec3::new(-0.012, -0.04, -0.33)
    };
    mb.add_cube(
        m * Mat4::from_translation(support) * Mat4::from_scale(Vec3::new(0.06, 0.055, 0.1)),
        glove,
        false,
    );
    let lg = m.transform_point3(support + Vec3::new(-0.02, -0.03, 0.03));
    mb.add_beam(lg, lg + Vec3::new(-0.2, -0.22, 0.28), 0.075, sleeve, false);

    if p.flash {
        let mp = m.transform_point3(muzzle_point(w));
        let size = if pistol { 0.05 } else { 0.08 };
        mb.add_cube(
            Mat4::from_translation(mp) * Mat4::from_rotation_z(p.time * 40.0) * Mat4::from_scale(Vec3::splat(size)),
            [255, 214, 120, 255],
            true,
        );
        mb.add_cube(
            Mat4::from_translation(mp) * Mat4::from_scale(Vec3::new(size * 0.4, size * 0.4, size * 2.2)),
            [255, 240, 190, 255],
            true,
        );
    }
}

/// Slowly rotating side view of a weapon for the modding screen.
pub fn showcase(mb: &mut MeshBuilder, w: &Weapon, time: f32) {
    let pistol = w.receiver == ReceiverId::Grach;
    let scale = if pistol { 2.6 } else { 1.0 };
    // Centre the model along its length (muzzle to stock butt).
    let front = muzzle_point(w).z;
    let back = if pistol {
        0.05
    } else if w.attachment(Slot::Stock).is_some() {
        0.38
    } else {
        0.14
    };
    let center_z = (front + back) * 0.5;
    let m = Mat4::from_translation(Vec3::new(0.0, 0.0, -2.5))
        * Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2 + (time * 0.5).sin() * 0.35)
        * Mat4::from_rotation_x(0.06)
        * Mat4::from_scale(Vec3::splat(scale))
        * Mat4::from_translation(Vec3::new(0.0, 0.03, -center_z));
    weapon_model(mb, w, m);
}
