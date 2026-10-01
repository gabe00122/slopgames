//! The beasts. Each one is a skeleton, a set of skinned meshes (its body plus
//! everything growing on it), a track draped over it, and an animator that
//! poses the skeleton every frame.

pub mod crab;
pub mod flora;
pub mod grazer;
pub mod whale;

use crate::{
    meshkit::{Col, MeshBuilder, Skin},
    track::Track,
    util::mix,
};
use bevy::{math::Affine3A, prelude::*};

/// Which shared material a mesh layer uses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mat {
    /// Rough, vertex-colored. Nearly everything.
    Matte,
    /// Shiny, vertex-colored: shell, eyes, wet things.
    Gloss,
    /// Unlit and bright enough to bloom: boost pads.
    Glow,
    /// Translucent water for pools.
    Water,
}

pub struct Layer {
    pub mesh: MeshBuilder,
    pub mat: Mat,
    pub flat: bool,
}

/// What the game tells the beast each frame.
#[derive(Default, Clone)]
pub struct BeastCtl {
    /// A point the beast's eyes follow (the race leader).
    pub look_at: Option<Vec3>,
    /// 0..1 spike set by the Beast Horn item; the beast shudders. Kept gentle:
    /// the ground must never drop faster than karts can follow, or it would
    /// fling every kart on the course. The horn tosses its victims itself.
    pub shudder: f32,
    /// Hold the plain walk (menus, countdown).
    pub calm: bool,
}

/// What the beast tells the game each frame.
#[derive(Default, Clone)]
pub struct AnimOut {
    /// Velocity of the ground relative to the beast (it walks on a treadmill).
    pub scroll: Vec3,
    /// Where legs break the water surface.
    pub splashes: Vec<Vec3>,
    /// Set for one frame when the beast starts doing something dramatic.
    pub banner: Option<&'static str>,
    /// Camera shake, 0..1.
    pub rumble: f32,
    /// The beast is blowing: geysers in the road are erupting.
    pub spout: bool,
}

pub trait Animator: Send + Sync + 'static {
    /// Writes the world transform of every bone for beast-time `t`.
    fn pose(&mut self, t: f32, dt: f32, ctl: &BeastCtl, out: &mut [Affine3A], info: &mut AnimOut);
}

#[derive(Clone, Copy)]
pub enum Ground {
    /// Open sea with the surface at `level` and the seabed at `floor`.
    Sea { level: f32, floor: f32 },
    /// Rolling grassland at `level`.
    Plain { level: f32 },
    /// Open sky over a sea of cloud at `floor`.
    Sky { floor: f32 },
}

#[derive(Clone, Copy)]
pub struct EnvDef {
    pub ground: Ground,
    pub sky_top: u32,
    pub sky_horizon: u32,
    pub fog: u32,
    pub fog_start: f32,
    pub fog_end: f32,
    pub sun_dir: Vec3,
    pub sun_color: u32,
    pub sun_lux: f32,
    pub ambient: f32,
    pub cloud_height: (f32, f32),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FaunaKind {
    /// Circles above its anchor.
    Gull,
    /// Flutters around its anchor.
    Butterfly,
    /// Leaps out of the sea near its anchor.
    Fish,
}

/// A living thing attached to a point on the beast.
pub struct Fauna {
    pub kind: FaunaKind,
    pub skin: Skin,
    pub rest: Vec3,
    pub seed: f32,
}

/// Where the showcase camera orbits.
#[derive(Clone, Copy)]
pub struct ShowView {
    pub center: Vec3,
    pub radius: f32,
    pub height: f32,
}

/// Everything a beast module hands to the game.
pub struct Built {
    /// Rest-pose world transform of each bone.
    pub rest: Vec<Affine3A>,
    pub track: Track,
    pub layers: Vec<Layer>,
    pub fauna: Vec<Fauna>,
    pub animator: Box<dyn Animator>,
    pub env: EnvDef,
    pub view: ShowView,
    /// Colors for the critters that cross the road.
    pub critter_color: u32,
}

pub struct CourseInfo {
    pub name: &'static str,
    pub beast: &'static str,
    pub blurb: &'static str,
    pub build: fn() -> Built,
}

pub const COURSES: &[CourseInfo] = &[
    CourseInfo {
        name: "CRABBACK CAY",
        beast: "OLD BRINE, THE ISLAND CRAB",
        blurb: "PALMS AND TIDE POOLS ON A WADING COLOSSUS. RACE OUT ALONG HIS ARMS AND LEAP BETWEEN THE CLAWS.",
        build: crab::build,
    },
    CourseInfo {
        name: "MOSSBACK RIDGE",
        beast: "THUNDERMOSS, THE GREAT GRAZER",
        blurb: "A FOREST ON A WALKING MOUNTAIN. UP THE NECK, AROUND THE CROWN AND OUT ALONG THE SWINGING TAIL.",
        build: grazer::build,
    },
    CourseInfo {
        name: "CLOUDBREAK REEF",
        beast: "GALEMOTHER, THE SKY WHALE",
        blurb: "A CORAL GARDEN ADRIFT ABOVE THE CLOUDS. RACE OUT ALONG HER FINS, OVER HER BLOWHOLE AND ACROSS HER FLUKES.",
        build: whale::build,
    },
];

/// Two-bone IK in the plane through `hip`, `foot` and the `bend` hint.
/// Returns the knee position; the foot is pulled in if out of reach.
pub fn ik_knee(hip: Vec3, foot: Vec3, l1: f32, l2: f32, bend: Vec3) -> (Vec3, Vec3) {
    let to = foot - hip;
    let d = to.length().clamp((l1 - l2).abs() + 0.01, l1 + l2 - 0.01);
    let dir = to.normalize_or(Vec3::NEG_Y);
    let a = (l1 * l1 - l2 * l2 + d * d) / (2.0 * d);
    let h = (l1 * l1 - a * a).max(0.0).sqrt();
    let perp = (bend - dir * bend.dot(dir)).normalize_or(Vec3::Y);
    let knee = hip + dir * a + perp * h;
    (knee, hip + dir * d)
}

/// World transform of a bone that points from `from` toward `to`, given the
/// direction it points in the rest pose and a shared bend-plane normal to fix
/// its twist.
pub fn aim_bone(from: Vec3, to: Vec3, rest_dir: Vec3, rest_side: Vec3, side: Vec3) -> Affine3A {
    let basis = |dir: Vec3, side: Vec3| {
        let d = dir.normalize_or(Vec3::Y);
        let s = (side - d * side.dot(d)).normalize_or(Vec3::X);
        Mat3::from_cols(d, s, d.cross(s))
    };
    let cur = basis(to - from, side);
    let rest = basis(rest_dir, rest_side);
    Affine3A::from_mat3_translation(cur * rest.transpose(), from)
}

/// A child bone rotated by `rot` about its own pivot, in its parent's frame.
pub fn child(parent: Affine3A, parent_pivot: Vec3, pivot: Vec3, rot: Quat) -> Affine3A {
    parent * Affine3A::from_rotation_translation(rot, pivot - parent_pivot)
}

pub fn pivot(p: Vec3) -> Affine3A {
    Affine3A::from_translation(p)
}

/// A lofted limb segment from `a` to `e`. `profile` lists `(fraction, rx, ry)`
/// cross-sections, with `ry` measured along `up`. Colors blend from `bottom`
/// on the underside to `top`.
pub fn limb(
    b: &mut MeshBuilder,
    a: Vec3,
    e: Vec3,
    up: Vec3,
    profile: &[(f32, f32, f32)],
    sides: usize,
    top: Col,
    bottom: Col,
) {
    let axis = (e - a).normalize_or(Vec3::Y);
    let rings: Vec<Vec<(Vec3, Col)>> = profile
        .iter()
        .map(|&(f, rx, ry)| {
            let mut ring = MeshBuilder::ring(a.lerp(e, f), axis, up, rx, ry, sides, top);
            for (k, v) in ring.iter_mut().enumerate() {
                let s = (k as f32 / sides as f32 * std::f32::consts::TAU).sin();
                v.1 = mix(bottom, top, (s * 0.8 + 0.55).clamp(0.0, 1.0));
            }
            ring
        })
        .collect();
    b.loft(&rings, true, true);
}
