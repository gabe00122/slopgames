//! Hitscan ballistics: block penetration/destruction and body-part hit detection.

use glam::{IVec3, Vec3};

use super::AmmoType;
use crate::player::BodyPart;
use crate::world::{Block, World};

/// A yaw-rotated box belonging to a body part.
#[derive(Clone, Copy, Debug)]
pub struct Hitbox {
    pub part: BodyPart,
    pub center: Vec3,
    pub half: Vec3,
    pub yaw: f32,
}

impl Hitbox {}

/// Local layout of a humanoid (feet at origin, facing -Z): (part, centre, half-extent).
pub const HUMANOID_PARTS: [(BodyPart, [f32; 3], [f32; 3]); 7] = [
    (BodyPart::Head, [0.0, 1.64, 0.0], [0.13, 0.14, 0.13]),
    (BodyPart::Thorax, [0.0, 1.32, 0.0], [0.23, 0.18, 0.14]),
    (BodyPart::Stomach, [0.0, 1.0, 0.0], [0.21, 0.14, 0.13]),
    (BodyPart::LeftArm, [-0.31, 1.2, 0.0], [0.07, 0.3, 0.08]),
    (BodyPart::RightArm, [0.31, 1.2, 0.0], [0.07, 0.3, 0.08]),
    (BodyPart::LeftLeg, [-0.11, 0.43, 0.0], [0.09, 0.43, 0.1]),
    (BodyPart::RightLeg, [0.11, 0.43, 0.0], [0.09, 0.43, 0.1]),
];

/// Vertical squash factor for a crouch amount in 0..1.
pub fn crouch_scale(crouch: f32) -> f32 {
    1.0 - 0.28 * crouch.clamp(0.0, 1.0)
}

pub fn humanoid_hitboxes(feet: Vec3, yaw: f32, crouch: f32) -> [Hitbox; 7] {
    let s = crouch_scale(crouch);
    let rot = glam::Quat::from_rotation_y(yaw);
    HUMANOID_PARTS.map(|(part, c, h)| {
        let local = Vec3::new(c[0], c[1] * s, c[2]);
        Hitbox {
            part,
            center: feet + rot * local,
            half: Vec3::new(h[0], h[1] * s, h[2]),
            yaw,
        }
    })
}

/// Ray vs yaw-rotated box. Returns the entry distance.
pub fn ray_hitbox(origin: Vec3, dir: Vec3, hb: &Hitbox) -> Option<f32> {
    let inv = glam::Quat::from_rotation_y(-hb.yaw);
    let o = inv * (origin - hb.center);
    let d = inv * dir;
    let mut tmin = f32::NEG_INFINITY;
    let mut tmax = f32::INFINITY;
    for i in 0..3 {
        if d[i].abs() < 1e-8 {
            if o[i].abs() > hb.half[i] {
                return None;
            }
        } else {
            let t1 = (-hb.half[i] - o[i]) / d[i];
            let t2 = (hb.half[i] - o[i]) / d[i];
            let (a, b) = if t1 < t2 { (t1, t2) } else { (t2, t1) };
            tmin = tmin.max(a);
            tmax = tmax.min(b);
            if tmin > tmax {
                return None;
            }
        }
    }
    if tmax < 0.0 {
        return None;
    }
    Some(tmin.max(0.0))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TargetId {
    Player,
    Scav(usize),
}

pub struct TargetBoxes {
    pub target: TargetId,
    pub boxes: [Hitbox; 7],
}

#[derive(Clone, Copy, Debug)]
pub struct BlockImpact {
    pub pos: IVec3,
    pub point: Vec3,
    pub normal: IVec3,
    pub block: Block,
    pub destroyed: bool,
    pub penetrated: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct EntityImpact {
    pub target: TargetId,
    pub part: BodyPart,
    pub point: Vec3,
    /// Damage and armor penetration remaining after passing through blocks.
    pub damage: f32,
    pub pen: f32,
}

#[derive(Clone, Debug)]
pub struct ShotOutcome {
    pub end: Vec3,
    pub blocks: Vec<BlockImpact>,
    pub entity: Option<EntityImpact>,
}

/// Fire one hitscan bullet. Blocks along the path are damaged (and possibly
/// destroyed); the bullet passes through blocks whose penetration resistance
/// is below its remaining power, losing damage and penetration as it goes.
pub fn fire(
    world: &mut World,
    origin: Vec3,
    dir: Vec3,
    ammo: AmmoType,
    max_range: f32,
    targets: &[TargetBoxes],
    exclude: Option<TargetId>,
) -> ShotOutcome {
    let dir = dir.normalize_or_zero();
    let def = ammo.def();

    // Closest body part along the ray.
    let mut ent: Option<(f32, TargetId, BodyPart)> = None;
    for t in targets {
        if Some(t.target) == exclude {
            continue;
        }
        for hb in &t.boxes {
            if let Some(tt) = ray_hitbox(origin, dir, hb) {
                if tt <= max_range && ent.is_none_or(|(best, _, _)| tt < best) {
                    ent = Some((tt, t.target, hb.part));
                }
            }
        }
    }
    let ent_t = ent.map(|e| e.0).unwrap_or(f32::INFINITY);
    let scan_to = max_range.min(ent_t);

    let mut hits = Vec::new();
    world.traverse(origin, dir, scan_to, |h| {
        if h.block.is_solid() {
            hits.push(h);
        }
        hits.len() < 16
    });

    let mut damage = def.damage;
    let mut pen = def.armor_pen;
    let mut power = def.block_pen;
    let mut blocks = Vec::new();
    let mut stopped: Option<f32> = None;
    for h in hits {
        if h.t > ent_t {
            break;
        }
        let resist = h.block.info().pen_resist;
        let passes = power > resist;
        let point = origin + dir * h.t;
        let block_dmg = if passes {
            def.block_damage * 0.6
        } else {
            def.block_damage
        };
        let destroyed = world.damage_block(h.pos, block_dmg);
        blocks.push(BlockImpact {
            pos: h.pos,
            point,
            normal: h.normal,
            block: h.block,
            destroyed,
            penetrated: passes,
        });
        if passes {
            let f = (1.0 - 0.6 * resist / power.max(1.0)).clamp(0.1, 1.0);
            damage *= f;
            pen *= f;
            power -= resist;
        } else {
            stopped = Some(h.t);
            break;
        }
    }

    match (stopped, ent) {
        (Some(t), _) => ShotOutcome {
            end: origin + dir * t,
            blocks,
            entity: None,
        },
        (None, Some((t, target, part))) => {
            let point = origin + dir * t;
            ShotOutcome {
                end: point,
                blocks,
                entity: Some(EntityImpact {
                    target,
                    part,
                    point,
                    damage,
                    pen,
                }),
            }
        }
        (None, None) => ShotOutcome {
            end: origin + dir * max_range,
            blocks,
            entity: None,
        },
    }
}

/// Random direction within a cone of half-angle `spread_deg` around `dir`.
pub fn apply_spread(dir: Vec3, spread_deg: f32, rng: &mut crate::rng::Rng) -> Vec3 {
    let dir = dir.normalize();
    let up = if dir.y.abs() > 0.99 { Vec3::X } else { Vec3::Y };
    let right = dir.cross(up).normalize();
    let up = right.cross(dir);
    let r = rng.f32().sqrt() * spread_deg.to_radians();
    let a = rng.f32() * std::f32::consts::TAU;
    (dir + right * (r.tan() * a.cos()) + up * (r.tan() * a.sin())).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;
    use glam::IVec3;

    fn wall_world(b: Block) -> World {
        let mut w = World::new(IVec3::new(2, 1, 2));
        w.fill_gen(IVec3::new(0, 0, 0), IVec3::new(31, 2, 31), Block::Stone);
        w.fill_gen(IVec3::new(10, 3, 0), IVec3::new(10, 8, 31), b);
        w.finish_generation();
        w
    }

    fn target_at(x: f32) -> Vec<TargetBoxes> {
        vec![TargetBoxes {
            target: TargetId::Scav(0),
            boxes: humanoid_hitboxes(Vec3::new(x, 3.0, 5.5), 0.0, 0.0),
        }]
    }

    #[test]
    fn headshot_detected() {
        let mut w = wall_world(Block::Air);
        let targets = target_at(14.5);
        let out = fire(
            &mut w,
            Vec3::new(2.0, 4.64, 5.5),
            Vec3::X,
            AmmoType::Ps545,
            100.0,
            &targets,
            None,
        );
        let e = out.entity.expect("should hit");
        assert_eq!(e.part, BodyPart::Head);
    }

    #[test]
    fn ap_penetrates_brick_but_fmj_does_not() {
        let targets = target_at(14.5);
        let mut w = wall_world(Block::Brick);
        let fmj = fire(
            &mut w,
            Vec3::new(2.0, 4.3, 5.5),
            Vec3::X,
            AmmoType::Ps545,
            100.0,
            &targets,
            None,
        );
        assert!(fmj.entity.is_none());
        assert!(!fmj.blocks[0].penetrated);

        let mut w = wall_world(Block::Brick);
        let ap = fire(
            &mut w,
            Vec3::new(2.0, 4.3, 5.5),
            Vec3::X,
            AmmoType::Bs545,
            100.0,
            &targets,
            None,
        );
        let e = ap.entity.expect("AP should pass through one brick");
        assert!(e.damage < AmmoType::Bs545.def().damage);
    }

    #[test]
    fn fmj_passes_planks_hp_does_not() {
        let targets = target_at(14.5);
        let mut w = wall_world(Block::Planks);
        assert!(fire(
            &mut w,
            Vec3::new(2.0, 4.3, 5.5),
            Vec3::X,
            AmmoType::Ps545,
            100.0,
            &targets,
            None
        )
        .entity
        .is_some());
        let mut w = wall_world(Block::Planks);
        assert!(fire(
            &mut w,
            Vec3::new(2.0, 4.3, 5.5),
            Vec3::X,
            AmmoType::Hp545,
            100.0,
            &targets,
            None
        )
        .entity
        .is_none());
    }

    #[test]
    fn sustained_fire_destroys_blocks() {
        let mut w = wall_world(Block::Planks);
        let mut destroyed = false;
        for _ in 0..20 {
            let out = fire(
                &mut w,
                Vec3::new(2.0, 4.3, 5.5),
                Vec3::X,
                AmmoType::Hp545,
                100.0,
                &[],
                None,
            );
            destroyed |= out.blocks.iter().any(|b| b.destroyed);
        }
        assert!(destroyed);
        assert_eq!(w.get(IVec3::new(10, 4, 5)), Block::Air);
    }

    #[test]
    fn spread_stays_in_cone() {
        let mut rng = Rng::new(5);
        for _ in 0..200 {
            let d = apply_spread(Vec3::Z, 2.0, &mut rng);
            assert!(d.angle_between(Vec3::Z).to_degrees() <= 2.01);
        }
    }
}
