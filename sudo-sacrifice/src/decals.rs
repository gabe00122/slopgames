//! Glowing marks drawn onto the ground: the aiming ring, spell warnings,
//! summoning circles and order markers. They are rebuilt into one mesh every
//! frame so they hug the ground even while it is being reshaped.

use crate::{
    meshkit::MeshBuilder,
    models::Mats,
    terrain::{Terrain, WATER},
    util::{lin, shade},
};
use bevy::{
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};
use std::f32::consts::TAU;

#[derive(Clone, Copy)]
enum Mark {
    Ring { center: Vec2, radius: f32 },
    Line { a: Vec2, b: Vec2, width: f32 },
}

#[derive(Clone, Copy)]
struct Decal {
    mark: Mark,
    color: u32,
    life: f32,
    max: f32,
}

#[derive(Resource, Default)]
pub struct Decals {
    list: Vec<Decal>,
}

impl Decals {
    /// A ring that fades over `life` seconds.
    pub fn ring(&mut self, center: Vec2, radius: f32, color: u32, life: f32) {
        self.list.push(Decal {
            mark: Mark::Ring { center, radius },
            color,
            life,
            max: life,
        });
    }

    /// A ring drawn for this frame only.
    pub fn ring_now(&mut self, center: Vec2, radius: f32, color: u32) {
        self.list.push(Decal {
            mark: Mark::Ring { center, radius },
            color,
            life: 0.0,
            max: 0.0,
        });
    }

    pub fn line_now(&mut self, a: Vec2, b: Vec2, width: f32, color: u32) {
        self.list.push(Decal {
            mark: Mark::Line { a, b, width },
            color,
            life: 0.0,
            max: 0.0,
        });
    }

    pub fn clear(&mut self) {
        self.list.clear();
    }
}

#[derive(Component)]
pub struct DecalMesh;

pub fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mats: Res<Mats>) {
    let mut b = MeshBuilder::new();
    b.disc(Vec3::new(0.0, -100.0, 0.0), Vec3::Y, 0.01, 3, [0.0; 4]);
    commands.spawn((
        Mesh3d(meshes.add(b.build(false))),
        MeshMaterial3d(mats.ghost.clone()),
        Transform::IDENTITY,
        NotShadowCaster,
        NotShadowReceiver,
        DecalMesh,
    ));
}

pub fn update(
    time: Res<Time>,
    terrain: Option<Res<Terrain>>,
    mut decals: ResMut<Decals>,
    mut meshes: ResMut<Assets<Mesh>>,
    q: Query<&Mesh3d, With<DecalMesh>>,
) {
    let Ok(handle) = q.single() else { return };
    let dt = time.delta_secs();
    let mut b = MeshBuilder::new();
    // A hidden triangle keeps the mesh valid when there is nothing to draw.
    b.disc(Vec3::new(0.0, -100.0, 0.0), Vec3::Y, 0.01, 3, [0.0; 4]);
    if let Some(terrain) = &terrain {
        let lift = |p: Vec2| Vec3::new(p.x, terrain.height_v(p).max(WATER) + 0.14, p.y);
        for d in &decals.list {
            let fade = if d.max > 0.0 {
                (d.life / d.max).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let pulse = if d.max > 0.0 { 1.0 + (1.0 - fade) * 0.08 } else { 1.0 };
            let c = shade(lin(d.color), fade);
            match d.mark {
                Mark::Ring { center, radius } => {
                    let r = radius * pulse;
                    let w = 0.12 + r * 0.012;
                    let n = ((r * 5.0) as usize).clamp(24, 110);
                    for k in 0..n {
                        let (a0, a1) = (k as f32 / n as f32 * TAU, (k + 1) as f32 / n as f32 * TAU);
                        let (d0, d1) = (Vec2::from_angle(a0), Vec2::from_angle(a1));
                        let q = [
                            lift(center + d0 * (r - w)),
                            lift(center + d0 * (r + w)),
                            lift(center + d1 * (r + w)),
                            lift(center + d1 * (r - w)),
                        ];
                        b.quad_p([q[3], q[2], q[1], q[0]], c);
                    }
                }
                Mark::Line { a, b: e, width } => {
                    let d = e - a;
                    let len = d.length().max(0.01);
                    let dir = d / len;
                    let side = Vec2::new(-dir.y, dir.x) * width;
                    let n = (len as usize).clamp(2, 60);
                    for k in 0..n {
                        let (p0, p1) = (a + d * (k as f32 / n as f32), a + d * ((k + 1) as f32 / n as f32));
                        let q = [lift(p0 - side), lift(p0 + side), lift(p1 + side), lift(p1 - side)];
                        b.quad_p(q, c);
                        b.quad_p([q[3], q[2], q[1], q[0]], c);
                    }
                }
            }
        }
    }
    if let Some(mut m) = meshes.get_mut(&handle.0) {
        *m = b.build(false);
    }
    for d in &mut decals.list {
        d.life -= dt;
    }
    decals.list.retain(|d| d.life > 0.0);
}
