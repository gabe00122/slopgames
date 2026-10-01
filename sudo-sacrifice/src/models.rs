//! Every model in the game, built from primitives at startup. Each model is
//! a solid mesh plus an optional glowing mesh drawn with an unlit material,
//! which is what the bloom picks up.

use crate::{
    game::Team,
    glyphs,
    meshkit::{Col, MeshBuilder},
    units::UnitKind,
    util::{Rng, lin, shade},
};
use bevy::{math::Affine3A, prelude::*};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

#[derive(Clone)]
pub struct Model {
    pub solid: Option<Handle<Mesh>>,
    pub glow: Option<Handle<Mesh>>,
}

impl Model {
    /// The solid mesh of a model that has one.
    pub fn body(&self) -> Handle<Mesh> {
        self.solid.clone().expect("model has a solid mesh")
    }
}

#[derive(Resource)]
pub struct Mats {
    pub matte: Handle<StandardMaterial>,
    pub metal: Handle<StandardMaterial>,
    pub glow: Handle<StandardMaterial>,
    pub soft_glow: Handle<StandardMaterial>,
    pub water: Handle<StandardMaterial>,
    pub ghost: Handle<StandardMaterial>,
    pub sky: Handle<StandardMaterial>,
    pub unlit: Handle<StandardMaterial>,
}

pub fn setup_mats(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(Mats {
        matte: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.85,
            reflectance: 0.3,
            ..default()
        }),
        metal: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.38,
            metallic: 0.35,
            reflectance: 0.5,
            ..default()
        }),
        glow: materials.add(StandardMaterial {
            base_color: Color::linear_rgb(3.4, 3.4, 3.4),
            unlit: true,
            ..default()
        }),
        soft_glow: materials.add(StandardMaterial {
            base_color: Color::linear_rgb(1.25, 1.25, 1.25),
            unlit: true,
            fog_enabled: false,
            ..default()
        }),
        water: materials.add(StandardMaterial {
            base_color: Color::srgba(0.25, 0.85, 0.95, 0.62),
            emissive: LinearRgba::rgb(0.02, 0.12, 0.16),
            perceptual_roughness: 0.1,
            reflectance: 0.6,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        ghost: materials.add(StandardMaterial {
            base_color: Color::linear_rgba(2.2, 2.2, 2.2, 0.5),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            cull_mode: None,
            ..default()
        }),
        sky: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            fog_enabled: false,
            cull_mode: None,
            ..default()
        }),
        unlit: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            ..default()
        }),
    });
}

#[derive(Resource)]
pub struct Models {
    pub agent: [Model; 2],
    pub units: Vec<[Model; 2]>,
    pub altar: [Model; 2],
    pub crystal: [Model; 2],
    pub datacenter: [Model; 2],
    pub well: Model,
    /// Neutral, blue, red.
    pub floppy: [Model; 3],
    pub trees: Vec<Model>,
    pub rocks: Vec<Model>,
    pub segfault: Model,
    pub assert_bolt: Model,
    pub byte: Model,
    pub meteor: Model,
    pub token: Model,
    pub summon_ring: [Model; 2],
    /// Blue, red, and gold for neutral.
    pub beam: [Model; 3],
}

impl Models {
    pub fn unit(&self, kind: UnitKind, team: Team) -> &Model {
        &self.units[kind.index()][team.i()]
    }

    pub fn floppy_for(&self, owner: Option<Team>) -> &Model {
        match owner {
            None => &self.floppy[0],
            Some(Team::Blue) => &self.floppy[1],
            Some(Team::Red) => &self.floppy[2],
        }
    }
}

fn bake(meshes: &mut Assets<Mesh>, solid: MeshBuilder, glow: MeshBuilder) -> Model {
    Model {
        solid: (!solid.is_empty()).then(|| meshes.add(solid.build(true))),
        glow: (!glow.is_empty()).then(|| meshes.add(glow.build(true))),
    }
}

pub fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let m = &mut *meshes;
    let per_team = |m: &mut Assets<Mesh>, f: &dyn Fn(Team) -> (MeshBuilder, MeshBuilder)| {
        Team::BOTH.map(|t| {
            let (s, g) = f(t);
            bake(m, s, g)
        })
    };
    let units = UnitKind::ALL.iter().map(|&k| per_team(m, &|t| unit(k, t))).collect();
    let mut rng = Rng::new(77);
    let trees = (0..5).map(|i| {
        let (s, g) = tree(&mut Rng::new(300 + i));
        bake(m, s, g)
    });
    let trees: Vec<Model> = trees.collect();
    let rocks: Vec<Model> = (0..4)
        .map(|i| {
            let (s, g) = rock(&mut rng, i);
            bake(m, s, g)
        })
        .collect();
    let models = Models {
        agent: per_team(m, &agent),
        units,
        altar: per_team(m, &altar),
        crystal: per_team(m, &crystal),
        datacenter: per_team(m, &datacenter),
        well: {
            let (s, g) = well();
            bake(m, s, g)
        },
        floppy: [None, Some(Team::Blue), Some(Team::Red)].map(|o| {
            let (s, g) = floppy(o);
            bake(m, s, g)
        }),
        trees,
        rocks,
        segfault: {
            let mut g = MeshBuilder::new();
            g.ball(Vec3::ZERO, 0.42, 1, lin(0xff8a3d));
            g.ball(Vec3::ZERO, 0.25, 1, shade(lin(0xfff0c0), 1.3));
            bake(m, MeshBuilder::new(), g)
        },
        assert_bolt: {
            let mut g = MeshBuilder::new();
            g.with(Affine3A::from_rotation_x(FRAC_PI_2), |g| {
                g.gem(Vec3::ZERO, 0.14, 0.55, lin(0x9dffa8), lin(0x3fd060));
            });
            bake(m, MeshBuilder::new(), g)
        },
        byte: {
            let mut g = MeshBuilder::new();
            g.boxy(Vec3::ZERO, Vec3::splat(0.22), lin(0xff5fe0));
            bake(m, MeshBuilder::new(), g)
        },
        meteor: {
            let mut s = MeshBuilder::new();
            let mut r = Rng::new(5);
            s.rock(Vec3::ZERO, Vec3::new(2.0, 1.8, 2.1), lin(0x3a2f3f), &mut r);
            let mut g = MeshBuilder::new();
            for k in 0..9 {
                let a = k as f32 / 9.0 * TAU;
                let d = Vec3::new(a.cos(), (k as f32 * 1.7).sin() * 0.6, a.sin()).normalize();
                let rot = Quat::from_rotation_arc(Vec3::Y, d);
                g.with(Affine3A::from_rotation_translation(rot, d * 1.7), |g| {
                    g.gem(Vec3::ZERO, 0.3, 0.7, lin(0xffb347), lin(0xff5a1f));
                });
            }
            bake(m, s, g)
        },
        token: {
            let mut g = MeshBuilder::new();
            g.with(
                Affine3A::from_rotation_z(PI / 4.0) * Affine3A::from_rotation_x(PI / 4.0),
                |g| {
                    g.boxy(Vec3::ZERO, Vec3::splat(0.09), lin(0xffffff));
                },
            );
            bake(m, MeshBuilder::new(), g)
        },
        summon_ring: Team::BOTH.map(|t| {
            let c = lin(t.color());
            let mut g = MeshBuilder::new();
            g.annulus(Vec3::ZERO, Vec3::Y, 1.6, 1.8, 40, c);
            g.annulus(Vec3::ZERO, Vec3::Y, 1.05, 1.15, 32, c);
            // Runes: short ticks between the rings.
            for k in 0..12 {
                let a = k as f32 / 12.0 * TAU;
                let (co, si) = (a.cos(), a.sin());
                let p = Vec3::new(co, 0.0, si);
                let tn = Vec3::new(-si, 0.0, co);
                let q = [
                    p * 1.22 - tn * 0.06,
                    p * 1.22 + tn * 0.06,
                    p * 1.5 + tn * 0.06,
                    p * 1.5 - tn * 0.06,
                ];
                g.quad_2(q, shade(c, 1.4));
            }
            bake(m, MeshBuilder::new(), g)
        }),
        beam: [Team::Blue.color(), Team::Red.color(), 0xffd98a].map(|c| {
            let mut g = MeshBuilder::new();
            // A bright core inside a fainter sheath.
            g.cyl(Vec3::ZERO, Vec3::Y, 0.45, 0.35, 10, shade(lin(0xffffff), 0.35));
            g.cyl(Vec3::ZERO, Vec3::Y, 1.0, 0.8, 14, shade(lin(c), 0.3));
            bake(m, MeshBuilder::new(), g)
        }),
    };
    commands.insert_resource(models);
}

// ---------------------------------------------------------------------------
// Agents
// ---------------------------------------------------------------------------

/// A robed agent with an old CRT monitor for a head. Faces -Z; about 2.5 m.
fn agent(team: Team) -> (MeshBuilder, MeshBuilder) {
    let mut s = MeshBuilder::new();
    let mut g = MeshBuilder::new();
    let dark = lin(team.dark());
    let accent = lin(team.color());
    let trim = shade(accent, 0.75);
    let cloth = shade(dark, 1.25);
    // Robe.
    s.lathe(
        &[
            (0.82, 0.05, trim),
            (0.78, 0.2, cloth),
            (0.6, 0.7, cloth),
            (0.44, 1.25, dark),
            (0.4, 1.55, dark),
            (0.55, 1.72, cloth),
            (0.3, 1.9, dark),
            (0.0, 1.95, dark),
        ],
        9,
    );
    // Sash.
    s.lathe(&[(0.47, 1.12, trim), (0.43, 1.28, trim)], 9);
    // Shoulders.
    for x in [-1.0, 1.0] {
        s.ball(Vec3::new(0.48 * x, 1.72, 0.0), 0.2, 1, shade(accent, 0.6));
        // Sleeves down to the hands.
        s.cyl(
            Vec3::new(0.5 * x, 1.7, 0.0),
            Vec3::new(0.62 * x, 1.2, -0.3),
            0.13,
            0.2,
            6,
            cloth,
        );
        g.ball(Vec3::new(0.64 * x, 1.1, -0.36), 0.12, 1, accent);
    }
    // Neck.
    s.cyl(Vec3::Y * 1.85, Vec3::Y * 2.05, 0.1, 0.1, 6, lin(0x3a3a40));
    // The monitor: beige case, a bulge behind, a dark screen with a prompt.
    let beige = lin(0xd9cfb8);
    s.boxy6(
        Vec3::new(0.0, 2.32, 0.02),
        Vec3::new(0.34, 0.28, 0.26),
        [
            beige,
            shade(beige, 0.85),
            shade(beige, 0.92),
            shade(beige, 0.92),
            shade(beige, 1.05),
            shade(beige, 0.7),
        ],
    );
    s.boxy(Vec3::new(0.0, 2.3, 0.36), Vec3::new(0.24, 0.2, 0.14), shade(beige, 0.8));
    s.quad_p(
        [
            Vec3::new(-0.27, 2.52, -0.245),
            Vec3::new(0.27, 2.52, -0.245),
            Vec3::new(0.27, 2.12, -0.245),
            Vec3::new(-0.27, 2.12, -0.245),
        ],
        lin(0x0b1418),
    );
    // The prompt, drawn in the team's color, facing out of the screen.
    let mut prompt = MeshBuilder::new();
    glyphs::extrude(&mut prompt, ">_", 0.042, 0.017, 4, accent);
    g.with(
        Affine3A::from_rotation_translation(Quat::from_rotation_y(PI), Vec3::new(0.0, 2.21, -0.26)),
        |g| g.append(&prompt),
    );
    // Power light.
    g.ball(Vec3::new(0.24, 2.08, -0.262), 0.025, 0, lin(0x7ee787));
    // A cable from the back of the monitor down the robe.
    s.tube(
        &[
            (Vec3::new(0.0, 2.2, 0.48), 0.035),
            (Vec3::new(0.05, 1.9, 0.62), 0.035),
            (Vec3::new(0.02, 1.4, 0.55), 0.035),
            (Vec3::new(-0.05, 0.9, 0.7), 0.035),
            (Vec3::new(0.0, 0.5, 0.85), 0.035),
        ],
        4,
        lin(0x2a2a2e),
        true,
    );
    (s, g)
}

// ---------------------------------------------------------------------------
// Subagents
// ---------------------------------------------------------------------------

fn unit(kind: UnitKind, team: Team) -> (MeshBuilder, MeshBuilder) {
    let mut s = MeshBuilder::new();
    let mut g = MeshBuilder::new();
    let dark = lin(team.dark());
    let accent = lin(team.color());
    let body = shade(dark, 1.5);
    match kind {
        UnitKind::Linter => {
            // A beetle with a red squiggle down its back.
            s.ico(Vec3::new(0.0, 0.55, 0.05), Vec3::new(0.48, 0.34, 0.62), 1, body);
            s.ico(
                Vec3::new(0.0, 0.5, -0.62),
                Vec3::new(0.3, 0.24, 0.26),
                1,
                shade(dark, 1.1),
            );
            for side in [-1.0, 1.0] {
                for k in 0..3 {
                    let z = -0.3 + k as f32 * 0.35;
                    let hip = Vec3::new(0.35 * side, 0.5, z);
                    let knee = Vec3::new(0.75 * side, 0.62, z + 0.05);
                    let foot = Vec3::new(0.85 * side, 0.0, z + 0.12);
                    s.beam(hip, knee, 0.045, shade(dark, 0.8));
                    s.beam(knee, foot, 0.04, shade(dark, 0.8));
                }
                g.ball(Vec3::new(0.12 * side, 0.58, -0.84), 0.06, 0, accent);
                // Mandibles.
                s.cone(
                    Vec3::new(0.12 * side, 0.42, -0.8),
                    Vec3::new(0.05 * side, 0.38, -1.05),
                    0.05,
                    4,
                    lin(0xe8e2d0),
                );
            }
            let squiggle: Vec<(Vec3, f32)> = (0..9)
                .map(|k| {
                    let t = k as f32 / 8.0;
                    let x = (t * TAU * 2.0).sin() * 0.16;
                    (Vec3::new(x, 0.9 - (t - 0.5).powi(2) * 0.4, -0.4 + t * 0.9), 0.05)
                })
                .collect();
            g.tube(&squiggle, 4, lin(0xff3b3b), true);
        }
        UnitKind::TestRunner => {
            // A lanky robot with a checkmark on its chest and a cannon arm.
            for side in [-1.0, 1.0] {
                s.beam(
                    Vec3::new(0.2 * side, 0.9, 0.0),
                    Vec3::new(0.24 * side, 0.0, 0.05),
                    0.09,
                    shade(dark, 0.8),
                );
                s.boxy(
                    Vec3::new(0.24 * side, 0.05, -0.05),
                    Vec3::new(0.13, 0.05, 0.2),
                    lin(0x30343c),
                );
            }
            s.boxy(Vec3::new(0.0, 1.05, 0.0), Vec3::new(0.3, 0.14, 0.2), shade(dark, 0.9));
            s.boxy6(
                Vec3::new(0.0, 1.45, 0.0),
                Vec3::new(0.38, 0.32, 0.25),
                [body, body, shade(body, 0.85), shade(body, 0.85), shade(body, 1.1), body],
            );
            s.boxy(Vec3::new(0.0, 1.95, 0.0), Vec3::new(0.2, 0.16, 0.19), lin(0xdad6cc));
            g.boxy(Vec3::new(0.0, 1.98, -0.19), Vec3::new(0.15, 0.04, 0.01), lin(0x7ee787));
            // Antenna.
            s.cyl(
                Vec3::new(0.1, 2.1, 0.0),
                Vec3::new(0.14, 2.4, 0.04),
                0.02,
                0.015,
                4,
                lin(0x888888),
            );
            g.ball(Vec3::new(0.14, 2.42, 0.04), 0.05, 0, lin(0x7ee787));
            // Checkmark.
            g.tube(
                &[
                    (Vec3::new(-0.18, 1.5, -0.27), 0.045),
                    (Vec3::new(-0.05, 1.36, -0.27), 0.045),
                    (Vec3::new(0.2, 1.64, -0.27), 0.045),
                ],
                4,
                lin(0x7ee787),
                true,
            );
            // Left arm and the cannon on the right.
            s.beam(Vec3::new(-0.42, 1.68, 0.0), Vec3::new(-0.48, 1.1, -0.1), 0.07, dark);
            s.cyl(
                Vec3::new(0.48, 1.55, 0.1),
                Vec3::new(0.48, 1.45, -0.62),
                0.14,
                0.11,
                7,
                lin(0x4a4f5a),
            );
            g.annulus(Vec3::new(0.48, 1.45, -0.63), Vec3::NEG_Z, 0.04, 0.1, 8, lin(0x7ee787));
            s.ball(Vec3::new(0.45, 1.68, 0.0), 0.14, 1, accent);
        }
        UnitKind::Fuzzer => {
            // A hovering core in a cloud of random bytes.
            s.ball(Vec3::ZERO, 0.45, 1, body);
            g.ball(Vec3::new(0.0, 0.0, -0.34), 0.16, 1, accent);
            g.annulus(Vec3::ZERO, Vec3::new(0.2, 1.0, 0.1), 0.62, 0.7, 20, lin(0xffd166));
            let mut r = Rng::new(team.i() as u64 + 40);
            let bytes = [0xff5fe0, 0xffd166, 0x5ce1ff, 0x7ee787];
            for k in 0..10 {
                let d = Vec3::new(r.sym(1.0), r.sym(0.7), r.sym(1.0)).normalize_or(Vec3::X);
                let p = d * r.range(0.75, 1.05);
                let c = lin(bytes[k % bytes.len()]);
                let e = r.range(0.06, 0.12);
                g.boxy(p, Vec3::splat(e), c);
            }
            for k in 0..4 {
                let a = k as f32 / 4.0 * TAU + 0.4;
                let (c, sn) = (a.cos() * 0.25, a.sin() * 0.25);
                s.tube(
                    &[
                        (Vec3::new(c, -0.35, sn), 0.05),
                        (Vec3::new(c * 1.3, -0.8, sn * 1.3), 0.04),
                        (Vec3::new(c * 1.1, -1.2, sn * 1.1), 0.02),
                    ],
                    4,
                    shade(dark, 0.9),
                    true,
                );
            }
        }
        UnitKind::Monolith => {
            // A golem of legacy code: stacked stone blocks with glowing lines of code.
            let stone = lin(0x8a8196);
            let stone2 = lin(0x6f6780);
            for side in [-1.0, 1.0] {
                s.boxy(Vec3::new(0.55 * side, 0.7, 0.0), Vec3::new(0.35, 0.7, 0.4), stone2);
                s.boxy(Vec3::new(0.55 * side, 0.12, -0.12), Vec3::new(0.42, 0.12, 0.55), stone2);
                // Arms hanging to big fists.
                s.boxy(Vec3::new(1.45 * side, 2.9, 0.0), Vec3::new(0.38, 0.38, 0.4), stone);
                s.boxy(Vec3::new(1.55 * side, 2.1, -0.05), Vec3::new(0.3, 0.55, 0.32), stone2);
                s.boxy(Vec3::new(1.6 * side, 1.3, -0.15), Vec3::new(0.45, 0.4, 0.45), stone);
            }
            s.boxy(Vec3::new(0.0, 1.6, 0.0), Vec3::new(0.85, 0.3, 0.55), stone);
            s.boxy6(
                Vec3::new(0.0, 2.65, 0.0),
                Vec3::new(1.15, 0.8, 0.7),
                [
                    stone,
                    stone,
                    shade(stone, 0.85),
                    shade(stone, 0.85),
                    shade(stone, 1.1),
                    stone2,
                ],
            );
            s.boxy(Vec3::new(0.0, 3.7, -0.05), Vec3::new(0.4, 0.32, 0.38), stone2);
            for side in [-1.0, 1.0] {
                g.boxy(Vec3::new(0.15 * side, 3.72, -0.44), Vec3::new(0.08, 0.04, 0.01), accent);
            }
            // Lines of code on the chest, indented like a real function.
            let lines = [
                (0.0, 1.2),
                (0.25, 0.9),
                (0.25, 0.5),
                (0.5, 0.7),
                (0.25, 0.3),
                (0.0, 0.4),
            ];
            for (k, (indent, len)) in lines.iter().enumerate() {
                let y = 3.2 - k as f32 * 0.19;
                let x0 = -0.85 + indent;
                g.boxy(
                    Vec3::new(x0 + len * 0.5, y, -0.71),
                    Vec3::new(len * 0.5, 0.045, 0.01),
                    if k == 0 { lin(0xffd166) } else { accent },
                );
            }
            // Moss on the shoulders.
            for side in [-1.0, 1.0] {
                s.ico(
                    Vec3::new(1.4 * side, 3.28, 0.1),
                    Vec3::new(0.3, 0.1, 0.3),
                    1,
                    lin(0x5d8a3e),
                );
            }
        }
        UnitKind::Collector => {
            // A dustbin robot on one wheel with two grabbing arms.
            let steel = lin(0xa3acb6);
            s.lathe(
                &[
                    (0.38, 0.35, shade(steel, 0.8)),
                    (0.42, 0.5, steel),
                    (0.46, 1.15, steel),
                    (0.5, 1.2, shade(steel, 1.1)),
                ],
                10,
            );
            s.lathe(&[(0.465, 0.72, accent), (0.47, 0.86, accent)], 10);
            // Ribs.
            for k in 0..10 {
                let a = k as f32 / 10.0 * TAU;
                let d = Vec3::new(a.sin(), 0.0, a.cos());
                s.beam(
                    d * 0.46 + Vec3::Y * 0.52,
                    d * 0.48 + Vec3::Y * 1.12,
                    0.02,
                    shade(steel, 0.7),
                );
            }
            // Lid, a little open.
            s.with(
                Affine3A::from_rotation_translation(Quat::from_rotation_x(0.25), Vec3::new(0.0, 1.22, 0.1)),
                |s| {
                    s.lathe(
                        &[(0.52, 0.0, shade(steel, 1.15)), (0.4, 0.12, steel), (0.0, 0.2, steel)],
                        10,
                    );
                    s.cyl(Vec3::Y * 0.2, Vec3::Y * 0.3, 0.08, 0.08, 6, lin(0x333333));
                },
            );
            // Wheel.
            s.cyl(
                Vec3::new(-0.15, 0.25, 0.0),
                Vec3::new(0.15, 0.25, 0.0),
                0.25,
                0.25,
                10,
                lin(0x25262b),
            );
            // Eye and arms.
            g.ball(Vec3::new(0.0, 0.98, -0.45), 0.09, 1, accent);
            for side in [-1.0, 1.0] {
                let shoulder = Vec3::new(0.46 * side, 0.95, -0.1);
                let elbow = Vec3::new(0.62 * side, 0.7, -0.45);
                let hand = Vec3::new(0.4 * side, 0.6, -0.75);
                s.beam(shoulder, elbow, 0.04, lin(0x6a7078));
                s.beam(elbow, hand, 0.035, lin(0x6a7078));
                s.cone(hand, hand + Vec3::new(-0.08 * side, -0.12, -0.12), 0.06, 4, accent);
            }
        }
    }
    (s, g)
}

// ---------------------------------------------------------------------------
// Buildings
// ---------------------------------------------------------------------------

/// A stepped ziggurat with an altar table on top. The crystal is separate so it can spin.
fn altar(team: Team) -> (MeshBuilder, MeshBuilder) {
    let mut s = MeshBuilder::new();
    let mut g = MeshBuilder::new();
    let accent = lin(team.color());
    let stone = if team == Team::Blue {
        lin(0x7d7f9c)
    } else {
        lin(0x9c7d87)
    };
    let tiers = [(7.0, 0.0), (5.3, 1.2), (3.6, 2.4)];
    for (k, (half, y)) in tiers.iter().enumerate() {
        let c = shade(stone, 0.8 + k as f32 * 0.1);
        s.boxy6(
            Vec3::new(0.0, y + 0.6, 0.0),
            Vec3::new(*half, 0.6, *half),
            [c, c, shade(c, 0.9), shade(c, 0.9), shade(c, 1.15), shade(c, 0.6)],
        );
        // Glowing seams along the top edge of each tier.
        for side in 0..4 {
            let rot = Quat::from_rotation_y(side as f32 * FRAC_PI_2);
            let p = rot * Vec3::new(0.0, y + 1.12, *half + 0.01);
            let along = rot * Vec3::X;
            let q = [
                p - along * (*half - 0.3) - Vec3::Y * 0.05,
                p + along * (*half - 0.3) - Vec3::Y * 0.05,
                p + along * (*half - 0.3) + Vec3::Y * 0.05,
                p - along * (*half - 0.3) + Vec3::Y * 0.05,
            ];
            let n = rot * Vec3::Z;
            let q = if (q[1] - q[0]).cross(q[3] - q[0]).dot(n) >= 0.0 {
                q
            } else {
                [q[3], q[2], q[1], q[0]]
            };
            g.quad_p(q, accent);
        }
    }
    // Stairs up the front (+Z faces the map's middle for Blue; the altar is rotated for Red).
    for k in 0..6 {
        let top = (k + 1) as f32 * 0.6;
        s.boxy(
            Vec3::new(0.0, top * 0.5, 7.3 - k as f32 * 0.85),
            Vec3::new(1.4, top * 0.5, 0.45),
            shade(stone, 0.95 + (k % 2) as f32 * 0.08),
        );
    }
    // The altar table.
    s.boxy(Vec3::new(0.0, 4.0, 0.0), Vec3::new(1.6, 0.4, 0.9), lin(0x3b3444));
    s.boxy(Vec3::new(0.0, 4.5, 0.0), Vec3::new(1.8, 0.1, 1.05), lin(0x51485c));
    g.boxy(Vec3::new(0.0, 4.62, 0.0), Vec3::new(1.3, 0.02, 0.6), shade(accent, 0.6));
    // Obelisks at the corners of the base, capped with light.
    for (x, z) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let base = Vec3::new(6.2 * x, 1.2, 6.2 * z);
        s.cyl(base, base + Vec3::Y * 6.0, 0.5, 0.3, 4, shade(stone, 0.7));
        g.gem(base + Vec3::Y * 6.6, 0.3, 0.55, accent, shade(accent, 0.6));
    }
    // "sudo" in light on the front of the altar table, facing the stairs.
    let mut word = MeshBuilder::new();
    glyphs::extrude(&mut word, "sudo", 0.095, 0.04, 4, accent);
    g.with(Affine3A::from_translation(Vec3::new(0.0, 3.74, 0.93)), |g| {
        g.append(&word)
    });
    (s, g)
}

fn crystal(team: Team) -> (MeshBuilder, MeshBuilder) {
    let mut g = MeshBuilder::new();
    let accent = lin(team.color());
    g.gem(Vec3::ZERO, 0.9, 1.6, shade(accent, 1.1), shade(accent, 0.6));
    g.annulus(Vec3::ZERO, Vec3::new(0.3, 1.0, 0.0), 1.6, 1.72, 32, accent);
    (MeshBuilder::new(), g)
}

/// A server rack tower. Built on a compute well; about 6 m tall.
fn datacenter(team: Team) -> (MeshBuilder, MeshBuilder) {
    let mut s = MeshBuilder::new();
    let mut g = MeshBuilder::new();
    let accent = lin(team.color());
    let case = lin(0x2b2f37);
    s.boxy(Vec3::new(0.0, 0.25, 0.0), Vec3::new(2.2, 0.25, 2.2), lin(0x55505e));
    s.boxy6(
        Vec3::new(0.0, 3.0, 0.0),
        Vec3::new(1.3, 2.5, 1.1),
        [case, case, shade(case, 0.8), shade(case, 0.8), shade(case, 1.3), case],
    );
    // Rack units with blinking lights on both faces.
    let mut rng = Rng::new(team.i() as u64 + 9);
    for face in [-1.0f32, 1.0] {
        for row in 0..9 {
            let y = 1.0 + row as f32 * 0.46;
            s.boxy(
                Vec3::new(0.0, y, 1.12 * face),
                Vec3::new(1.15, 0.17, 0.03),
                lin(0x3b404a),
            );
            for k in 0..6 {
                if rng.chance(0.65) {
                    let c = match rng.below(3) {
                        0 => lin(0x7ee787),
                        1 => accent,
                        _ => lin(0xffd166),
                    };
                    g.boxy(
                        Vec3::new(-0.95 + k as f32 * 0.16, y + 0.06, 1.16 * face),
                        Vec3::new(0.035, 0.03, 0.01),
                        c,
                    );
                }
            }
        }
    }
    // Cooling fins.
    for side in [-1.0, 1.0] {
        for k in 0..7 {
            s.boxy(
                Vec3::new(1.36 * side, 1.2 + k as f32 * 0.6, 0.0),
                Vec3::new(0.08, 0.04, 0.95),
                lin(0x8a929c),
            );
        }
    }
    // Mast and beacon.
    s.cyl(Vec3::Y * 5.5, Vec3::Y * 7.2, 0.08, 0.05, 5, lin(0x9aa0a8));
    g.ball(Vec3::Y * 7.3, 0.2, 1, accent);
    g.annulus(Vec3::Y * 0.52, Vec3::Y, 2.0, 2.2, 24, accent);
    (s, g)
}

/// A compute well: standing stones around a glowing pool.
fn well() -> (MeshBuilder, MeshBuilder) {
    let mut s = MeshBuilder::new();
    let mut g = MeshBuilder::new();
    let mut rng = Rng::new(31);
    for k in 0..7 {
        let a = k as f32 / 7.0 * TAU;
        let p = Vec3::new(a.cos() * 3.6, 0.0, a.sin() * 3.6);
        let h = rng.range(1.6, 2.6);
        s.with(Affine3A::from_rotation_translation(Quat::from_rotation_y(-a), p), |s| {
            s.boxy(
                Vec3::new(0.0, h * 0.5 - 0.2, 0.0),
                Vec3::new(0.35, h * 0.5, 0.55),
                lin(0x77708a),
            )
        });
        g.boxy(p + Vec3::Y * (h - 0.5), Vec3::new(0.08, 0.2, 0.08), lin(0x9ff3ff));
    }
    s.annulus(Vec3::Y * 0.05, Vec3::Y, 2.3, 2.8, 24, lin(0x5d5870));
    g.disc(Vec3::Y * 0.08, Vec3::Y, 2.3, 24, lin(0x49d8f0));
    g.annulus(Vec3::Y * 0.1, Vec3::Y, 0.9, 1.1, 20, lin(0xd8fbff));
    (s, g)
}

/// A 3.5" floppy disk: the software that is sacrificed.
fn floppy(owner: Option<Team>) -> (MeshBuilder, MeshBuilder) {
    let mut s = MeshBuilder::new();
    let mut g = MeshBuilder::new();
    let body = match owner {
        None => lin(0x2c2c33),
        Some(Team::Blue) => lin(0x2f6fd6),
        Some(Team::Red) => lin(0xc8324f),
    };
    let halo = match owner {
        None => lin(0xffe9a8),
        Some(t) => lin(t.color()),
    };
    // Upright, facing -Z, 0.9 m square.
    s.boxy(Vec3::ZERO, Vec3::new(0.45, 0.45, 0.04), body);
    // Metal shutter at the top and the label below it, on both faces.
    for z in [-0.045, 0.045] {
        let flip = z > 0.0;
        let quad = |x0: f32, y0: f32, x1: f32, y1: f32| {
            let q = [
                Vec3::new(x0, y0, z),
                Vec3::new(x1, y0, z),
                Vec3::new(x1, y1, z),
                Vec3::new(x0, y1, z),
            ];
            if flip { q } else { [q[3], q[2], q[1], q[0]] }
        };
        s.quad_p(quad(-0.2, 0.18, 0.22, 0.44), lin(0xc9ced6));
        s.quad_p(quad(0.02, 0.24, 0.12, 0.4), lin(0x3a3d44));
        if !flip {
            s.quad_p(quad(-0.36, -0.42, 0.36, 0.06), lin(0xf2efe6));
            for k in 0..3 {
                let y = -0.1 - k as f32 * 0.1;
                s.quad_p(quad(-0.3, y, 0.3, y + 0.02), lin(0x9aa3b5));
            }
        }
    }
    g.annulus(Vec3::new(0.0, -0.9, 0.0), Vec3::Y, 0.45, 0.6, 20, halo);
    (s, g)
}

// ---------------------------------------------------------------------------
// Scenery
// ---------------------------------------------------------------------------

/// A dependency tree: a trunk that splits into a graph of package nodes.
fn tree(rng: &mut Rng) -> (MeshBuilder, MeshBuilder) {
    let mut s = MeshBuilder::new();
    let mut g = MeshBuilder::new();
    let bark = lin(0x5a4660);
    let leaf = [lin(0x4f9e6a), lin(0x3f8f86), lin(0x6aa64f), lin(0x2f7f75)];
    let lights = [0xffd166, 0x7ee787, 0x79c0ff, 0xff9ad5];
    let h = rng.range(3.0, 4.5);
    let lean = Vec3::new(rng.sym(0.4), 0.0, rng.sym(0.4));
    s.tube(
        &[
            (Vec3::ZERO - Vec3::Y * 0.3, 0.35),
            (Vec3::Y * h * 0.5 + lean * 0.4, 0.24),
            (Vec3::Y * h + lean, 0.18),
        ],
        6,
        bark,
        true,
    );
    // Roots.
    for k in 0..3 {
        let a = k as f32 / 3.0 * TAU + rng.sym(0.5);
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        s.cone(Vec3::Y * 0.4, d * 1.1 - Vec3::Y * 0.2, 0.16, 4, bark);
    }
    let root = Vec3::Y * h + lean;
    s.ball(root, 0.42, 1, leaf[0]);
    let mut frontier = vec![(root, Vec3::Y, 0.42)];
    for depth in 0..3 {
        let mut next = Vec::new();
        for (p, dir, r) in &frontier {
            let kids = if depth == 0 { 3 } else { 2 + rng.below(2) };
            for _ in 0..kids {
                let out = (*dir * 0.8 + Vec3::new(rng.sym(1.2), rng.range(0.1, 0.9), rng.sym(1.2))).normalize();
                let len = rng.range(1.0, 1.8) * (1.0 - depth as f32 * 0.18);
                let q = *p + out * len;
                let rr = r * 0.78;
                s.cyl(*p, q, 0.06, 0.05, 4, shade(bark, 1.3));
                let lit = depth == 2 && rng.chance(0.4);
                if lit {
                    g.ball(q, rr * 0.8, 1, lin(*rng.pick(&lights)));
                } else {
                    s.ball(q, rr, 1, *rng.pick(&leaf));
                }
                next.push((q, out, rr));
            }
        }
        frontier = next;
    }
    (s, g)
}

/// A legacy boulder, sometimes with data crystals growing out of it.
fn rock(rng: &mut Rng, variant: u64) -> (MeshBuilder, MeshBuilder) {
    let mut s = MeshBuilder::new();
    let mut g = MeshBuilder::new();
    let base = [lin(0x7a6f86), lin(0x6c6478), lin(0x857a70), lin(0x736a80)][variant as usize % 4];
    let big = rng.range(1.2, 2.2);
    s.rock(Vec3::Y * big * 0.35, Vec3::new(big, big * 0.8, big * 1.1), base, rng);
    for _ in 0..2 {
        let off = Vec3::new(rng.sym(big), 0.0, rng.sym(big));
        let r = rng.range(0.4, 0.8);
        s.rock(off + Vec3::Y * r * 0.2, Vec3::splat(r), shade(base, 0.9), rng);
    }
    if variant.is_multiple_of(2) {
        for _ in 0..3 {
            let d = Vec3::new(rng.sym(0.6), 1.0, rng.sym(0.6)).normalize();
            let rot = Quat::from_rotation_arc(Vec3::Y, d);
            let p = Vec3::Y * big * 0.7 + Vec3::new(rng.sym(0.5), 0.0, rng.sym(0.5));
            let len = rng.range(0.5, 0.9);
            g.with(Affine3A::from_rotation_translation(rot, p), |g| {
                g.gem(Vec3::Y * len, 0.2, len, lin(0x9ff3ff), lin(0x3aa0d8));
            });
        }
    }
    (s, g)
}

/// A giant floating glyph for the skyline.
pub fn sky_glyph(text: &str, color: u32) -> MeshBuilder {
    let mut g = MeshBuilder::new();
    let c: Col = lin(color);
    glyphs::extrude(&mut g, text, 2.2, 0.55, 6, c);
    g
}
