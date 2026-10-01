//! Procedural models for every exhibit. Each builder spawns its parts under
//! `top` (the surface of the plinth, +Z facing the visitor) and returns a
//! bounding sphere used for "look at" detection.

use crate::{
    anim::*,
    architecture::ShadowLod,
    content::Model,
    kit::{Kit, at, v2},
    meshes::{self, MeshData},
    placards::{Cell, PlacardAtlas},
    textures::{self, fbm3},
};
use bevy::{light::NotShadowCaster, prelude::*};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub struct Bounds {
    pub center: Vec3,
    pub radius: f32,
}

fn bounds(x: f32, y: f32, z: f32, radius: f32) -> Bounds {
    Bounds {
        center: Vec3::new(x, y, z),
        radius,
    }
}

pub struct Ctx<'a> {
    pub images: &'a mut Assets<Image>,
    pub atlas: &'a PlacardAtlas,
}

type Mat = Handle<StandardMaterial>;

fn hash01(i: u32, salt: u32) -> f32 {
    let mut h = i.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xffff) as f32 / 65536.0
}

/// Faceted, lightly jittered version of a mesh, for knapped stone.
fn knapped(mut mesh: Mesh, amount: f32, seed: u32) -> Mesh {
    use bevy::mesh::VertexAttributeValues;
    if let Some(VertexAttributeValues::Float32x3(ps)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) {
        for p in ps.iter_mut() {
            let v = Vec3::from(*p);
            let q = (v * 1000.0).round() / 1000.0;
            let d = Vec3::new(
                fbm3(q * 9.0, 2, seed) - 0.5,
                fbm3(q * 9.0 + 5.0, 2, seed) - 0.5,
                fbm3(q * 9.0 + 9.0, 2, seed) - 0.5,
            );
            *p = (v + d * amount).into();
        }
    }
    mesh.duplicate_vertices();
    mesh.compute_flat_normals();
    mesh
}

/// Small objects are shown enlarged so they read from a distance.
pub fn scale(model: Model) -> f32 {
    use Model::*;
    match model {
        Transistor => 1.7,
        Microchip => 1.35,
        Cuneiform => 1.4,
        Microscope => 1.4,
        Penicillin => 1.2,
        Platonic => 1.3,
        HandAxe => 1.15,
        Vaccine => 1.1,
        _ => 1.0,
    }
}

pub fn build(kit: &mut Kit, ctx: &mut Ctx, top: Entity, model: Model, ceiling: f32) -> Bounds {
    use Model::*;
    match model {
        Armillary => armillary(kit, ctx, top),
        HandAxe => hand_axe(kit, top),
        Fire => fire(kit, top),
        CaveArt => cave_art(kit, ctx, top),
        Wheat => wheat(kit, top),
        Wheel => wheel(kit, top),
        Cuneiform => cuneiform(kit, top),
        Pyramid => pyramid(kit, top),
        Platonic => platonic(kit, top),
        Parthenon => parthenon(kit, top),
        Antikythera => antikythera(kit, top),
        PrintingPress => printing_press(kit, top),
        Orrery => orrery(kit, top),
        Telescope => telescope(kit, ctx, top),
        Ship => ship(kit, top),
        NewtonCannon => newton_cannon(kit, ctx, top),
        WrightFlyer => wright_flyer(kit, top, ceiling),
        Sputnik => sputnik(kit, top, ceiling),
        SaturnV => saturn_v(kit, top),
        Voyager => voyager(kit, top, ceiling),
        Jwst => jwst(kit, top),
        SteamEngine => steam_engine(kit, top),
        Locomotive => locomotive(kit, top),
        PeriodicTable => periodic_table(kit, ctx, top),
        LightBulb => light_bulb(kit, top),
        Radio => radio(kit, top),
        Microscope => microscope(kit, top),
        Vaccine => vaccine(kit, top),
        TreeOfLife => tree_of_life(kit, top),
        Penicillin => penicillin(kit, ctx, top),
        Dna => dna(kit, top),
        AnalyticalEngine => analytical_engine(kit, top),
        Eniac => eniac(kit, top),
        Transistor => transistor(kit, top),
        Microchip => microchip(kit, top),
        Web => web(kit, top),
    }
}

/// Cables from a suspended model up to the ceiling.
fn cables(kit: &mut Kit, parent: Entity, points: &[Vec3], ceiling_above: f32) {
    let steel = kit.pal.steel.clone();
    for p in points {
        let e = kit.rod(parent, *p, Vec3::new(p.x, ceiling_above, p.z), 0.006, &steel);
        kit.no_shadow(e);
    }
}

fn torus(kit: &mut Kit, major: f32, minor: f32) -> Handle<Mesh> {
    kit.mesh(
        Torus::new(major - minor, major + minor)
            .mesh()
            .major_resolution(96)
            .minor_resolution(12),
    )
}

// ------------------------------------------------------------------ Rotunda

fn armillary(kit: &mut Kit, ctx: &mut Ctx, top: Entity) -> Bounds {
    let pal = kit.pal;
    let stand = [
        v2(0.0, 0.0),
        v2(0.5, 0.0),
        v2(0.5, 0.1),
        v2(0.5, 0.1),
        v2(0.26, 0.22),
        v2(0.12, 0.4),
        v2(0.09, 0.75),
        v2(0.15, 0.82),
        v2(0.15, 0.82),
        v2(0.0, 0.86),
    ];
    kit.lathe(top, &stand, &pal.bronze, Transform::IDENTITY);
    let c = kit.node(top, at(0.0, 2.3, 0.0));
    let r = 1.45;
    // Fixed meridian (vertical) and horizon rings.
    let meridian = torus(kit, r, 0.045);
    kit.part(
        c,
        &meridian,
        &pal.gold,
        Transform::from_rotation(Quat::from_rotation_x(FRAC_PI_2)),
    );
    let horizon = torus(kit, r + 0.08, 0.05);
    kit.part(c, &horizon, &pal.brass, Transform::from_scale(Vec3::new(1.0, 1.6, 1.0)));
    // Tilted celestial assembly rotating about the polar axis.
    let tilt = kit.node(c, Transform::from_rotation(Quat::from_rotation_z(0.68)));
    kit.rod(
        tilt,
        Vec3::new(0.0, -r - 0.1, 0.0),
        Vec3::new(0.0, r + 0.1, 0.0),
        0.025,
        &pal.steel,
    );
    for s in [-1.0, 1.0] {
        kit.ball(tilt, 0.06, &pal.gold, at(0.0, s * (r + 0.12), 0.0));
    }
    let spin = kit.node(tilt, Transform::IDENTITY);
    kit.insert(spin, Spin::y(0.18));
    let equator = torus(kit, r - 0.12, 0.035);
    kit.part(spin, &equator, &pal.gold, Transform::IDENTITY);
    for (y, rr) in [
        (0.52, ((r - 0.12) * (r - 0.12) - 0.27f32).sqrt()),
        (-0.52, ((r - 0.12) * (r - 0.12) - 0.27f32).sqrt()),
    ] {
        let m = torus(kit, rr, 0.02);
        kit.part(spin, &m, &pal.brass, at(0.0, y, 0.0));
    }
    let colure = torus(kit, r - 0.14, 0.03);
    kit.part(
        spin,
        &colure,
        &pal.brass,
        Transform::from_rotation(Quat::from_rotation_x(FRAC_PI_2)),
    );
    kit.part(
        spin,
        &colure,
        &pal.brass,
        Transform::from_rotation(Quat::from_rotation_z(FRAC_PI_2)),
    );
    // The zodiac band, tilted by the obliquity of the ecliptic.
    let band = torus(kit, r - 0.13, 0.07);
    kit.part(
        spin,
        &band,
        &pal.bronze,
        Transform::from_rotation(Quat::from_rotation_x(23.4f32.to_radians())).with_scale(Vec3::new(1.0, 0.35, 1.0)),
    );
    // A gilded Earth at the heart.
    let globe_tex = ctx.images.add(textures::globe(true));
    let globe_mat = kit.mat(StandardMaterial {
        base_color_texture: Some(globe_tex),
        metallic: 0.6,
        perceptual_roughness: 0.3,
        ..default()
    });
    let g = kit.ball(tilt, 0.34, &globe_mat, Transform::IDENTITY);
    kit.insert(g, Spin::y(0.5));
    bounds(0.0, 2.3, 0.0, 1.6)
}

// --------------------------------------------------------------------- Dawn

fn vitrine(kit: &mut Kit, parent: Entity, size: Vec3) {
    let glass = kit.pal.glass.clone();
    let brass = kit.pal.brass.clone();
    let e = kit.cube(parent, size, &glass, at(0.0, size.y / 2.0, 0.0));
    kit.no_shadow(e);
    // Thin frame along the top edges
    for (s, t) in [
        (Vec3::new(size.x, 0.015, 0.015), Vec3::new(0.0, size.y, size.z / 2.0)),
        (Vec3::new(size.x, 0.015, 0.015), Vec3::new(0.0, size.y, -size.z / 2.0)),
        (Vec3::new(0.015, 0.015, size.z), Vec3::new(size.x / 2.0, size.y, 0.0)),
        (Vec3::new(0.015, 0.015, size.z), Vec3::new(-size.x / 2.0, size.y, 0.0)),
    ] {
        kit.cube(parent, s, &brass, Transform::from_translation(t));
    }
}

fn hand_axe(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    kit.cyl(top, 0.16, 0.03, &pal.black, at(0.0, 0.015, 0.0));
    let turn = kit.node(top, at(0.0, 0.03, 0.0));
    kit.insert(turn, Spin::y(0.35));
    kit.rod(turn, Vec3::ZERO, Vec3::new(0.0, 0.08, 0.0), 0.01, &pal.black);
    let profile = [
        v2(0.0, 0.0),
        v2(0.07, 0.015),
        v2(0.11, 0.07),
        v2(0.125, 0.14),
        v2(0.11, 0.22),
        v2(0.075, 0.3),
        v2(0.035, 0.36),
        v2(0.0, 0.39),
    ];
    let axe = knapped(meshes::lathe(&profile, 9), 0.018, 5);
    let flint = kit.mat(StandardMaterial {
        base_color: Color::srgb(0.32, 0.27, 0.22),
        perceptual_roughness: 0.32,
        reflectance: 0.5,
        ..default()
    });
    let axe = kit.mesh(axe);
    kit.part(
        turn,
        &axe,
        &flint,
        at(0.0, 0.07, 0.0).with_scale(Vec3::new(1.0, 1.0, 0.36)),
    );
    // An older Oldowan chopper and a sharp flake beside it.
    let chopper = kit.mesh(knapped(Sphere::new(0.07).mesh().ico(1).unwrap().into(), 0.02, 9));
    kit.part(
        top,
        &chopper,
        &pal.sandstone,
        at(-0.3, 0.05, 0.18).with_scale(Vec3::new(1.2, 0.8, 1.0)),
    );
    let flake = kit.mesh(knapped(Sphere::new(0.05).mesh().ico(1).unwrap().into(), 0.012, 13));
    kit.part(
        top,
        &flake,
        &flint,
        at(0.3, 0.015, 0.2).with_scale(Vec3::new(1.4, 0.25, 0.8)),
    );
    vitrine(kit, top, Vec3::new(0.95, 0.62, 0.95));
    bounds(0.0, 0.25, 0.0, 0.45)
}

fn fire(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let stone = kit.mesh(knapped(Sphere::new(0.12).mesh().ico(1).unwrap().into(), 0.03, 21));
    for i in 0..14 {
        let a = i as f32 / 14.0 * TAU;
        let t = Transform::from_xyz(a.cos() * 0.6, 0.05, a.sin() * 0.6)
            .with_rotation(Quat::from_rotation_y(a * 3.1))
            .with_scale(Vec3::new(1.3, 0.75, 1.0));
        kit.part(top, &stone, &pal.stone, t);
    }
    let ash = kit.color(Color::srgb(0.1, 0.09, 0.085), 0.95);
    kit.cyl(top, 0.5, 0.02, &ash, at(0.0, 0.01, 0.0));
    let log = kit.color(Color::srgb(0.16, 0.1, 0.07), 0.9);
    for i in 0..5 {
        let a = i as f32 / 5.0 * TAU + 0.3;
        kit.rod(
            top,
            Vec3::new(a.cos() * 0.42, 0.02, a.sin() * 0.42),
            Vec3::new(a.cos() * 0.05, 0.42, a.sin() * 0.05),
            0.045,
            &log,
        );
    }
    let ember = kit.glow(Color::srgb(1.0, 0.35, 0.08), 12.0);
    let ember_mesh = kit.mesh(Sphere::new(0.03).mesh().ico(1).unwrap());
    for i in 0..24 {
        let a = hash01(i, 1) * TAU;
        let d = hash01(i, 2) * 0.35;
        let e = kit.part(
            top,
            &ember_mesh,
            &ember,
            at(a.cos() * d, 0.03, a.sin() * d).with_scale(Vec3::splat(0.6 + hash01(i, 3))),
        );
        kit.no_shadow(e);
    }
    let outer = kit.glow(Color::srgb(1.0, 0.36, 0.06), 30.0);
    let inner = kit.glow(Color::srgb(1.0, 0.75, 0.3), 60.0);
    for i in 0..7 {
        let a = i as f32 * 2.1;
        let d = if i == 0 { 0.0 } else { 0.12 };
        let h = if i == 0 { 0.75 } else { 0.4 + hash01(i, 5) * 0.25 };
        let base = Vec3::new(1.0, h, 1.0);
        let f = kit.shape(
            top,
            Cone {
                radius: 0.11,
                height: 1.0,
            }
            .mesh()
            .resolution(12),
            &outer,
            at(a.cos() * d, 0.1 + h / 2.0, a.sin() * d),
        );
        kit.insert(
            f,
            (
                Flicker {
                    base,
                    seed: i as f32 * 1.7,
                },
                NotShadowCaster,
            ),
        );
        let g = kit.shape(
            top,
            Cone {
                radius: 0.06,
                height: 1.0,
            }
            .mesh()
            .resolution(12),
            &inner,
            at(a.cos() * d, 0.08 + h * 0.3, a.sin() * d),
        );
        kit.insert(
            g,
            (
                Flicker {
                    base: base * Vec3::new(1.0, 0.55, 1.0),
                    seed: i as f32 * 2.9 + 1.0,
                },
                NotShadowCaster,
            ),
        );
    }
    let light = kit.cmd.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.55, 0.22),
            intensity: 90_000.0,
            range: 9.0,
            radius: 0.15,
            shadow_maps_enabled: false,
            ..default()
        },
        FlickerLight { base: 90_000.0 },
        ShadowLod,
        at(0.0, 0.55, 0.0),
        ChildOf(top),
    ));
    let _ = light;
    bounds(0.0, 0.4, 0.0, 0.8)
}

fn cave_art(kit: &mut Kit, ctx: &mut Ctx, top: Entity) -> Bounds {
    let pal = kit.pal;
    let (nx, ny) = (72, 40);
    let (w, h) = (3.0, 1.8);
    let height = |x: f32, y: f32| {
        let p = Vec3::new(x, y, 0.0);
        (fbm3(p * 1.4, 5, 3) - 0.5) * 0.35 + (fbm3(p * 6.0, 3, 4) - 0.5) * 0.04 - ((x / 1.5).powi(2)) * 0.12
    };
    let mut m = MeshData::default();
    let mut idx = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            let x = -w / 2.0 + w * i as f32 / nx as f32;
            let y = h * j as f32 / ny as f32;
            let z = height(x, y);
            let e = 0.01;
            let n = Vec3::new(
                -(height(x + e, y) - height(x - e, y)) / (2.0 * e),
                -(height(x, y + e) - height(x, y - e)) / (2.0 * e),
                1.0,
            )
            .normalize();
            idx.push(m.vert(
                Vec3::new(x, y, z),
                n,
                Vec2::new(i as f32 / nx as f32, 1.0 - j as f32 / ny as f32),
            ));
        }
    }
    for j in 0..ny {
        for i in 0..nx {
            let a = idx[j * (nx + 1) + i];
            let b = idx[j * (nx + 1) + i + 1];
            let c = idx[(j + 1) * (nx + 1) + i + 1];
            let d = idx[(j + 1) * (nx + 1) + i];
            m.quad(a, b, c, d);
        }
    }
    let tex = ctx.images.add(textures::cave_painting());
    let rock = kit.mat(StandardMaterial {
        base_color_texture: Some(tex),
        perceptual_roughness: 0.9,
        ..default()
    });
    let mesh = kit.mesh(m.build());
    kit.part(top, &mesh, &rock, at(0.0, 0.05, -0.1));
    let back = kit.color(Color::srgb(0.3, 0.25, 0.2), 0.95);
    kit.cube(top, Vec3::new(3.1, 1.9, 0.3), &back, at(0.0, 0.95, -0.45));
    // A stone oil lamp, as the painters would have used.
    kit.cyl(top, 0.08, 0.05, &pal.stone, at(0.5, 0.025, 0.35));
    let flame = kit.glow(Color::srgb(1.0, 0.6, 0.2), 40.0);
    let f = kit.shape(
        top,
        Cone {
            radius: 0.02,
            height: 1.0,
        }
        .mesh()
        .resolution(8),
        &flame,
        at(0.5, 0.1, 0.35),
    );
    kit.insert(
        f,
        (
            Flicker {
                base: Vec3::new(1.0, 0.08, 1.0),
                seed: 3.3,
            },
            NotShadowCaster,
        ),
    );
    kit.cmd.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.6, 0.3),
            intensity: 6000.0,
            range: 4.0,
            radius: 0.02,
            ..default()
        },
        FlickerLight { base: 6000.0 },
        at(0.5, 0.18, 0.35),
        ChildOf(top),
    ));
    bounds(0.0, 0.9, 0.0, 1.5)
}

fn wheat(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let pot = [
        v2(0.0, 0.0),
        v2(0.12, 0.0),
        v2(0.19, 0.12),
        v2(0.2, 0.22),
        v2(0.15, 0.33),
        v2(0.16, 0.36),
        v2(0.16, 0.36),
        v2(0.14, 0.36),
        v2(0.14, 0.3),
    ];
    kit.lathe(top, &pot, &pal.clay, at(-0.15, 0.0, -0.1));
    let bundle = kit.node(top, at(-0.15, 0.3, -0.1));
    kit.insert(
        bundle,
        Swing {
            axis: Vec3::X,
            amplitude: 0.025,
            freq: 0.22,
            phase: 0.0,
            base: Quat::IDENTITY,
        },
    );
    let straw = kit.metal(Color::srgb(0.85, 0.7, 0.35), 0.6);
    let grain = kit.color(Color::srgb(0.84, 0.62, 0.26), 0.55);
    let ear = kit.mesh(Sphere::new(1.0).mesh().uv(10, 8));
    for i in 0..40u32 {
        let a = i as f32 * 2.399;
        let tilt = 0.06 + hash01(i, 1) * 0.28;
        let len = 0.62 + hash01(i, 2) * 0.3;
        let dir = Vec3::new(tilt.sin() * a.cos(), tilt.cos(), tilt.sin() * a.sin());
        let base = Vec3::new(a.cos(), 0.0, a.sin()) * 0.03 * hash01(i, 3);
        let tip = base + dir * len;
        let s = kit.rod(bundle, base, tip, 0.0045, &straw);
        kit.no_shadow(s);
        let rot = Quat::from_rotation_arc(Vec3::Y, dir);
        kit.part(
            bundle,
            &ear,
            &grain,
            Transform::from_translation(tip + dir * 0.05)
                .with_rotation(rot)
                .with_scale(Vec3::new(0.016, 0.065, 0.02)),
        );
        // awns
        let e = kit.rod(
            bundle,
            tip + dir * 0.09,
            tip + dir * 0.2 + Vec3::new(0.01, 0.0, 0.0),
            0.0015,
            &straw,
        );
        kit.no_shadow(e);
    }
    // A basket of harvested grain
    let basket = [
        v2(0.0, 0.0),
        v2(0.16, 0.0),
        v2(0.22, 0.1),
        v2(0.22, 0.1),
        v2(0.2, 0.1),
        v2(0.14, 0.02),
    ];
    kit.lathe(top, &basket, &pal.wood, at(0.3, 0.0, 0.25));
    kit.cyl(top, 0.19, 0.02, &grain, at(0.3, 0.07, 0.25));
    bounds(-0.1, 0.65, 0.0, 0.6)
}

fn wheel(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    kit.cube(top, Vec3::new(0.14, 0.72, 0.14), &pal.dark_wood, at(0.0, 0.36, -0.28));
    kit.cube(top, Vec3::new(0.5, 0.06, 0.3), &pal.dark_wood, at(0.0, 0.03, -0.28));
    kit.rod(
        top,
        Vec3::new(0.0, 0.7, -0.3),
        Vec3::new(0.0, 0.7, 0.14),
        0.035,
        &pal.wood,
    );
    let hub = kit.node(top, at(0.0, 0.7, 0.0));
    kit.insert(hub, Spin::z(-0.3));
    let face = Quat::from_rotation_x(FRAC_PI_2);
    let r = 0.6;
    kit.shape(
        hub,
        Cylinder::new(r, 0.09).mesh().resolution(48),
        &pal.wood,
        Transform::from_rotation(face),
    );
    kit.shape(
        hub,
        Cylinder::new(0.13, 0.22).mesh().resolution(24),
        &pal.dark_wood,
        Transform::from_rotation(face),
    );
    let seam = kit.color(Color::srgb(0.12, 0.07, 0.04), 0.9);
    for x in [-0.2f32, 0.2] {
        let chord = 2.0 * (r * r - x * x).sqrt();
        for z in [-0.046, 0.046] {
            kit.cube(hub, Vec3::new(0.012, chord - 0.01, 0.004), &seam, at(x, 0.0, z));
        }
    }
    for y in [-0.3f32, 0.3] {
        let chord = 2.0 * (r * r - y * y).sqrt() - 0.12;
        kit.cube(hub, Vec3::new(chord, 0.08, 0.035), &pal.dark_wood, at(0.0, y, 0.062));
        for x in [-0.36f32, -0.12, 0.12, 0.36] {
            if x.abs() < chord / 2.0 {
                kit.shape(
                    hub,
                    Cylinder::new(0.014, 0.02).mesh().resolution(10),
                    &seam,
                    at(x, y, 0.085).with_rotation(face),
                );
            }
        }
    }
    bounds(0.0, 0.7, 0.0, 0.75)
}

// ------------------------------------------------------------------ Ancient

fn cuneiform(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let clay = kit.color(Color::srgb(0.72, 0.56, 0.4), 0.9);
    let dark = kit.color(Color::srgb(0.46, 0.34, 0.24), 0.95);
    kit.cube(top, Vec3::new(0.5, 0.04, 0.2), &pal.dark_wood, at(0.0, 0.02, -0.05));
    let tab = kit.node(
        top,
        Transform::from_xyz(0.0, 0.22, -0.02).with_rotation(Quat::from_rotation_x(-0.28)),
    );
    let body = kit.mesh(Sphere::new(1.0).mesh().uv(24, 16));
    kit.part(tab, &body, &clay, Transform::from_scale(Vec3::new(0.24, 0.17, 0.04)));
    kit.cube(tab, Vec3::new(0.42, 0.29, 0.06), &clay, Transform::IDENTITY);
    let wedge = kit.mesh(
        Cone {
            radius: 0.009,
            height: 0.028,
        }
        .mesh()
        .resolution(3),
    );
    for row in 0..7 {
        let y = 0.11 - row as f32 * 0.036;
        kit.cube(tab, Vec3::new(0.38, 0.0025, 0.004), &dark, at(0.0, y - 0.018, 0.031));
        let mut x = -0.17;
        let mut k = row * 31;
        while x < 0.17 {
            k += 1;
            if hash01(k, 7) > 0.18 {
                let horizontal = hash01(k, 8) > 0.55;
                let rot = if horizontal {
                    Quat::from_rotation_z(FRAC_PI_2)
                } else {
                    Quat::IDENTITY
                };
                kit.part(
                    tab,
                    &wedge,
                    &dark,
                    Transform::from_xyz(x, y, 0.03).with_rotation(rot * Quat::from_rotation_x(0.2)),
                );
            }
            x += 0.022 + hash01(k, 9) * 0.02;
        }
    }
    kit.rod(
        top,
        Vec3::new(-0.2, 0.012, 0.2),
        Vec3::new(0.12, 0.012, 0.28),
        0.008,
        &pal.wood,
    );
    // A second, smaller tablet lying flat
    let t2 = kit.node(
        top,
        Transform::from_xyz(0.33, 0.02, 0.2).with_rotation(Quat::from_rotation_y(0.4)),
    );
    kit.cube(t2, Vec3::new(0.16, 0.035, 0.12), &clay, Transform::IDENTITY);
    bounds(0.0, 0.22, 0.0, 0.35)
}

fn pyramid(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let sand = kit.color(Color::srgb(0.86, 0.73, 0.5), 0.95);
    kit.cube(top, Vec3::new(1.9, 0.03, 1.9), &sand, at(0.0, 0.015, 0.0));
    let limestone = kit.color(Color::srgb(0.9, 0.86, 0.76), 0.6);
    let base = 0.03;
    let pyr = |kit: &mut Kit, x: f32, z: f32, b: f32, mat: &Mat, cap: Option<&Mat>| {
        let h = b * 0.636;
        kit.shape(top, meshes::pyramid(b, h), mat, at(x, base, z));
        if let Some(cap) = cap {
            let cb = b * 0.13;
            kit.shape(
                top,
                meshes::pyramid(cb, cb * 0.636),
                cap,
                at(x, base + h - cb * 0.636 + 0.001, z),
            );
        }
    };
    pyr(kit, 0.42, 0.3, 0.9, &limestone, Some(&pal.gold));
    pyr(kit, -0.4, -0.45, 0.84, &pal.sandstone, Some(&limestone));
    pyr(kit, -0.62, 0.48, 0.4, &pal.sandstone, None);
    for i in 0..3 {
        pyr(kit, 0.98, 0.0 + i as f32 * 0.18, 0.12, &pal.sandstone, None);
    }
    bounds(0.0, 0.3, 0.0, 1.0)
}

fn platonic(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let leather = kit.color(Color::srgb(0.38, 0.12, 0.08), 0.5);
    kit.cube(top, Vec3::new(0.38, 0.06, 0.28), &leather, at(0.0, 0.03, 0.22));
    kit.cube(top, Vec3::new(0.36, 0.05, 0.265), &pal.ivory, at(0.012, 0.032, 0.22));
    let ring = kit.node(top, at(0.0, 0.55, -0.05));
    kit.insert(ring, Spin::y(0.22));
    let lapis = kit.mat(StandardMaterial {
        base_color: Color::srgb(0.1, 0.2, 0.6),
        perceptual_roughness: 0.15,
        ..default()
    });
    let solids: [(Mesh, Mat); 5] = [
        (meshes::tetrahedron(0.13), pal.gold.clone()),
        (Cuboid::from_length(0.15).into(), pal.marble.clone()),
        (meshes::octahedron(0.13), pal.chrome.clone()),
        (meshes::dodecahedron(0.12), pal.bronze.clone()),
        (meshes::icosahedron(0.12), lapis),
    ];
    for (i, (mesh, mat)) in solids.into_iter().enumerate() {
        let a = i as f32 / 5.0 * TAU;
        let base = Vec3::new(a.cos() * 0.36, 0.0, a.sin() * 0.36);
        let n = kit.node(ring, Transform::from_translation(base));
        kit.insert(
            n,
            Bob {
                amplitude: 0.035,
                freq: 0.25 + i as f32 * 0.04,
                base,
            },
        );
        let s = kit.shape(n, mesh, &mat, Transform::IDENTITY);
        kit.insert(
            s,
            Spin {
                axis: Vec3::new(1.0, 1.0, 0.3).normalize(),
                speed: 0.5 + i as f32 * 0.1,
            },
        );
    }
    bounds(0.0, 0.5, 0.0, 0.55)
}

fn parthenon(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let marble = kit.color(Color::srgb(0.9, 0.87, 0.8), 0.45);
    let (l, w) = (1.9, 0.84);
    for (k, grow) in [(0, 0.12), (1, 0.06), (2, 0.0)] {
        kit.cube(
            top,
            Vec3::new(l + grow, 0.03, w + grow),
            &marble,
            at(0.0, 0.015 + k as f32 * 0.03, 0.0),
        );
    }
    let sty = 0.09;
    let col_h = 0.4;
    let col = kit.mesh(meshes::lathe(
        &[v2(0.0, 0.0), v2(0.03, 0.0), v2(0.024, col_h), v2(0.0, col_h)],
        16,
    ));
    let cap = kit.mesh(Cuboid::new(0.07, 0.018, 0.07));
    let place = |kit: &mut Kit, x: f32, z: f32| {
        kit.part(top, &col, &marble, at(x, sty, z));
        kit.part(top, &cap, &marble, at(x, sty + col_h + 0.009, z));
    };
    let (ex, ez) = (l / 2.0 - 0.05, w / 2.0 - 0.05);
    for i in 0..17 {
        let x = -ex + 2.0 * ex * i as f32 / 16.0;
        place(kit, x, ez);
        place(kit, x, -ez);
    }
    for i in 1..7 {
        let z = -ez + 2.0 * ez * i as f32 / 7.0;
        place(kit, ex, z);
        place(kit, -ex, z);
    }
    let ent_y = sty + col_h + 0.018;
    kit.cube(
        top,
        Vec3::new(l - 0.02, 0.05, w - 0.02),
        &marble,
        at(0.0, ent_y + 0.025, 0.0),
    );
    let frieze = kit.color(Color::srgb(0.78, 0.74, 0.68), 0.5);
    kit.cube(
        top,
        Vec3::new(l - 0.01, 0.035, w - 0.01),
        &frieze,
        at(0.0, ent_y + 0.068, 0.0),
    );
    let roof = meshes::prism(
        &[v2(-w / 2.0 - 0.01, 0.0), v2(w / 2.0 + 0.01, 0.0), v2(0.0, 0.13)],
        l + 0.02,
    );
    kit.shape(
        top,
        roof,
        &marble,
        at(0.0, ent_y + 0.086, 0.0).with_rotation(Quat::from_rotation_y(FRAC_PI_2)),
    );
    kit.cube(
        top,
        Vec3::new(l - 0.45, col_h - 0.02, w - 0.3),
        &pal.ivory,
        at(0.0, sty + col_h / 2.0, 0.0),
    );
    bounds(0.0, 0.35, 0.0, 1.05)
}

fn antikythera(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    kit.cube(top, Vec3::new(1.0, 0.8, 0.04), &pal.dark_wood, at(0.0, 0.42, -0.14));
    kit.cube(top, Vec3::new(1.0, 0.04, 0.3), &pal.dark_wood, at(0.0, 0.02, -0.02));
    for x in [-0.5, 0.5] {
        kit.cube(top, Vec3::new(0.04, 0.8, 0.3), &pal.dark_wood, at(x, 0.42, -0.02));
    }
    let patina = kit.metal(Color::srgb(0.36, 0.48, 0.38), 0.6);
    let bronze = pal.bronze.clone();
    let brass = pal.brass.clone();
    // (center, radius, teeth, material, z)
    let main_c = Vec3::new(-0.05, 0.43, 0.0);
    let w0 = 0.25;
    let mut gears: Vec<(Vec3, f32, usize, Mat, f32, f32)> = Vec::new(); // center, radius, teeth, mat, z, speed
    gears.push((main_c, 0.3, 64, bronze.clone(), 0.0, w0));
    let mesh_with = |c: Vec3, r: f32, ang: f32, r2: f32| c + Vec3::new(ang.cos(), ang.sin(), 0.0) * (r + r2 - 0.01);
    let g2 = mesh_with(main_c, 0.3, 0.35, 0.1);
    gears.push((g2, 0.1, 22, brass.clone(), 0.0, -w0 * 0.3 / 0.1));
    gears.push((g2, 0.065, 15, patina.clone(), 0.025, -w0 * 0.3 / 0.1));
    let g4 = mesh_with(g2, 0.065, 1.7, 0.09);
    gears.push((g4, 0.09, 20, bronze.clone(), 0.025, w0 * 0.3 / 0.1 * 0.065 / 0.09));
    let g5 = mesh_with(main_c, 0.3, 3.5, 0.08);
    gears.push((g5, 0.08, 18, patina.clone(), 0.0, -w0 * 0.3 / 0.08));
    let g6 = mesh_with(main_c, 0.3, 4.6, 0.06);
    gears.push((g6, 0.06, 14, brass.clone(), 0.0, -w0 * 0.3 / 0.06));
    let g7 = mesh_with(g5, 0.08, 2.5, 0.07);
    gears.push((g7, 0.07, 16, bronze.clone(), 0.0, w0 * 0.3 / 0.07));
    for (i, (c, r, teeth, mat, z, speed)) in gears.into_iter().enumerate() {
        let n = kit.node(top, Transform::from_translation(c + Vec3::Z * z));
        kit.insert(n, Spin::z(speed));
        let hole = if i == 0 { r * 0.8 } else { 0.012 };
        kit.shape(n, meshes::gear(r, teeth, 0.014, hole), &mat, Transform::IDENTITY);
        kit.rod(
            n,
            Vec3::new(0.0, 0.0, -0.13 - z),
            Vec3::new(0.0, 0.0, 0.03),
            0.008,
            &pal.steel,
        );
        if i == 0 {
            for k in 0..4 {
                let a = k as f32 * FRAC_PI_2 + 0.4;
                kit.rod(
                    n,
                    Vec3::ZERO,
                    Vec3::new(a.cos(), a.sin(), 0.0) * (r * 0.82),
                    0.014,
                    &mat,
                );
            }
            kit.shape(
                n,
                Cylinder::new(0.04, 0.03).mesh().resolution(20),
                &mat,
                Transform::from_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            );
        }
    }
    // The corroded fragment as it came out of the sea.
    let lump = kit.mesh(knapped(Sphere::new(0.14).mesh().ico(2).unwrap().into(), 0.05, 41));
    let corroded = kit.color(Color::srgb(0.3, 0.36, 0.3), 0.9);
    kit.part(
        top,
        &lump,
        &corroded,
        at(0.62, 0.06, 0.22).with_scale(Vec3::new(1.3, 0.45, 1.0)),
    );
    kit.shape(
        top,
        meshes::gear(0.09, 20, 0.012, 0.0),
        &corroded,
        Transform::from_xyz(0.6, 0.1, 0.24).with_rotation(Quat::from_rotation_x(-1.2)),
    );
    bounds(0.0, 0.43, 0.0, 0.6)
}

// ---------------------------------------------------------------- Discovery

fn printing_press(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let wood = pal.dark_wood.clone();
    for x in [-0.45, 0.45] {
        kit.cube(top, Vec3::new(0.13, 1.95, 0.15), &wood, at(x, 0.975, 0.0));
        kit.cube(top, Vec3::new(0.16, 0.1, 0.9), &wood, at(x, 0.05, 0.0));
    }
    kit.cube(top, Vec3::new(1.12, 0.22, 0.22), &wood, at(0.0, 1.78, 0.0));
    kit.cube(top, Vec3::new(1.05, 0.14, 0.2), &wood, at(0.0, 1.2, 0.0));
    kit.cube(top, Vec3::new(1.05, 0.12, 0.2), &wood, at(0.0, 0.45, 0.0));
    // Carriage and bed with a locked-up forme of type and a sheet of paper.
    kit.cube(top, Vec3::new(0.78, 0.07, 1.2), &pal.wood, at(0.0, 0.7, 0.15));
    kit.cube(top, Vec3::new(0.42, 0.035, 0.32), &pal.iron, at(0.0, 0.752, 0.0));
    kit.cube(top, Vec3::new(0.4, 0.004, 0.3), &pal.ivory, at(0.0, 0.772, 0.0));
    // The screw and platen move together.
    kit.rod(
        top,
        Vec3::new(0.0, 1.13, 0.0),
        Vec3::new(0.0, 1.67, 0.0),
        0.045,
        &pal.brass,
    );
    let platen = kit.node(top, at(0.0, 0.95, 0.0));
    kit.insert(
        platen,
        Slide {
            axis: Vec3::Y,
            amplitude: 0.07,
            freq: 0.2,
            phase: 0.0,
            base: Vec3::new(0.0, 0.88, 0.0),
        },
    );
    kit.cube(platen, Vec3::new(0.5, 0.06, 0.38), &pal.wood, Transform::IDENTITY);
    kit.rod(
        platen,
        Vec3::new(0.0, 0.03, 0.0),
        Vec3::new(0.0, 0.22, 0.0),
        0.03,
        &pal.iron,
    );
    let bar = kit.node(top, at(0.0, 1.05, 0.0));
    kit.insert(
        bar,
        Swing {
            axis: Vec3::Y,
            amplitude: 0.7,
            freq: 0.2,
            phase: FRAC_PI_2,
            base: Quat::IDENTITY,
        },
    );
    kit.rod(bar, Vec3::ZERO, Vec3::new(0.6, 0.02, 0.18), 0.018, &pal.iron);
    kit.ball(bar, 0.035, &pal.wood, at(0.6, 0.02, 0.18));
    // Printed sheets and ink balls on a side table
    kit.cube(top, Vec3::new(0.3, 0.6, 0.35), &pal.wood, at(0.72, 0.3, 0.3));
    kit.cube(top, Vec3::new(0.26, 0.05, 0.34), &pal.ivory, at(0.72, 0.625, 0.3));
    for z in [0.2, 0.4] {
        kit.ball(top, 0.06, &pal.charcoal, at(0.72, 0.72, z));
        kit.rod(
            top,
            Vec3::new(0.72, 0.72, z),
            Vec3::new(0.72, 0.9, z + 0.02),
            0.012,
            &pal.wood,
        );
    }
    bounds(0.0, 1.0, 0.0, 1.0)
}

fn orrery(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    kit.rod(top, Vec3::ZERO, Vec3::new(0.0, 0.58, 0.0), 0.02, &pal.brass);
    let sun_y = 0.64;
    let sun = kit.glow(Color::srgb(1.0, 0.78, 0.35), 25.0);
    let s = kit.ball(top, 0.08, &sun, at(0.0, sun_y, 0.0));
    kit.no_shadow(s);
    kit.cmd.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.85, 0.6),
            intensity: 1500.0,
            range: 1.6,
            radius: 0.08,
            ..default()
        },
        at(0.0, sun_y, 0.0),
        ChildOf(top),
    ));
    let ring = torus(kit, 0.78, 0.012);
    kit.part(top, &ring, &pal.brass, at(0.0, 0.18, 0.0));
    for k in 0..3 {
        let a = k as f32 / 3.0 * TAU;
        kit.rod(
            top,
            Vec3::new(a.cos() * 0.1, 0.0, a.sin() * 0.1),
            Vec3::new(a.cos() * 0.78, 0.18, a.sin() * 0.78),
            0.008,
            &pal.brass,
        );
    }
    let planets: [(f32, f32, Color, f32); 6] = [
        (0.16, 0.018, Color::srgb(0.6, 0.58, 0.55), 1.6),
        (0.24, 0.028, Color::srgb(0.9, 0.82, 0.6), 1.2),
        (0.33, 0.03, Color::srgb(0.2, 0.4, 0.85), 1.0),
        (0.43, 0.022, Color::srgb(0.8, 0.3, 0.15), 0.8),
        (0.56, 0.058, Color::srgb(0.82, 0.68, 0.5), 0.44),
        (0.7, 0.046, Color::srgb(0.9, 0.82, 0.58), 0.33),
    ];
    for (i, (r, size, c, speed)) in planets.into_iter().enumerate() {
        let arm_y = 0.22 + i as f32 * 0.04;
        let n = kit.node(
            top,
            Transform::from_xyz(0.0, arm_y, 0.0).with_rotation(Quat::from_rotation_y(i as f32 * 1.9)),
        );
        kit.insert(n, Spin::y(speed * 0.45));
        kit.rod(n, Vec3::ZERO, Vec3::new(r, 0.0, 0.0), 0.004, &pal.brass);
        kit.rod(
            n,
            Vec3::new(r, 0.0, 0.0),
            Vec3::new(r, sun_y - arm_y - size, 0.0),
            0.003,
            &pal.brass,
        );
        let mat = kit.color(c, 0.6);
        kit.ball(n, size, &mat, at(r, sun_y - arm_y, 0.0));
        if i == 2 {
            let m = kit.node(n, at(r, sun_y - arm_y, 0.0));
            kit.insert(m, Spin::y(2.5));
            kit.ball(m, 0.009, &pal.white, at(0.055, 0.0, 0.0));
        }
        if i == 5 {
            let ring = kit.mesh(Torus::new(0.06, 0.09).mesh().major_resolution(32).minor_resolution(6));
            let rm = kit.color(Color::srgb(0.85, 0.78, 0.6), 0.5);
            kit.part(
                n,
                &ring,
                &rm,
                at(r, sun_y - arm_y, 0.0)
                    .with_rotation(Quat::from_rotation_x(0.45))
                    .with_scale(Vec3::new(1.0, 0.15, 1.0)),
            );
        }
    }
    bounds(0.0, 0.55, 0.0, 0.8)
}

fn telescope(kit: &mut Kit, ctx: &mut Ctx, top: Entity) -> Bounds {
    let pal = kit.pal;
    let head = Vec3::new(0.0, 1.05, 0.0);
    for k in 0..3 {
        let a = k as f32 / 3.0 * TAU + 0.5;
        kit.rod(
            top,
            head,
            Vec3::new(a.cos() * 0.42, 0.0, a.sin() * 0.42),
            0.02,
            &pal.wood,
        );
    }
    kit.ball(top, 0.04, &pal.brass, Transform::from_translation(head));
    let dir = Vec3::new(-0.78, 0.58, -0.22).normalize();
    let scope = kit.node(
        top,
        Transform::from_translation(head + Vec3::Y * 0.04).with_rotation(Quat::from_rotation_arc(Vec3::Y, dir)),
    );
    let leather = kit.mat(StandardMaterial {
        base_color: Color::srgb(0.42, 0.16, 0.08),
        perceptual_roughness: 0.45,
        ..default()
    });
    let tube = [
        v2(0.0, -0.5),
        v2(0.022, -0.5),
        v2(0.022, -0.5),
        v2(0.026, -0.42),
        v2(0.034, 0.75),
        v2(0.034, 0.75),
        v2(0.0, 0.75),
    ];
    kit.lathe(scope, &tube, &leather, Transform::IDENTITY);
    let band = kit.mesh(Torus::new(0.028, 0.04).mesh().major_resolution(24).minor_resolution(6));
    for y in [-0.42, -0.1, 0.25, 0.6, 0.73] {
        kit.part(
            scope,
            &band,
            &pal.gold,
            at(0.0, y, 0.0).with_scale(Vec3::new(1.0, 0.6, 1.0)),
        );
    }
    // The Moon and Jupiter with its four Galilean moons, as Galileo saw them.
    let moon_tex = ctx
        .images
        .add(textures::generate(256, 128, textures::COLOR_TILE, |u, v| {
            let d = Vec3::new(
                (u * TAU).cos() * (v * PI).sin(),
                (v * PI).cos(),
                (u * TAU).sin() * (v * PI).sin(),
            );
            let n = fbm3(d * 3.0, 5, 61);
            let maria = if fbm3(d * 1.2 + 3.0, 3, 62) > 0.55 { 0.55 } else { 1.0 };
            Vec3::splat((0.45 + n * 0.4) * maria).extend(1.0)
        }));
    let moon_mat = kit.mat(StandardMaterial {
        base_color_texture: Some(moon_tex),
        perceptual_roughness: 0.95,
        ..default()
    });
    let moon = kit.ball(top, 0.26, &moon_mat, at(-0.75, 2.35, -0.35));
    kit.insert(
        moon,
        (
            Bob {
                amplitude: 0.05,
                freq: 0.12,
                base: Vec3::new(-0.75, 2.35, -0.35),
            },
            Spin::y(0.1),
        ),
    );
    let jup_tex = ctx
        .images
        .add(textures::generate(256, 128, textures::COLOR_TILE, |u, v| {
            let band = ((v * 14.0 + fbm3(Vec3::new(u * 8.0, v * 3.0, 0.0), 3, 71) * 1.5) * PI).sin();
            let c1 = Vec3::new(0.85, 0.72, 0.55);
            let c2 = Vec3::new(0.6, 0.42, 0.3);
            (c1 + (c2 - c1) * (band * 0.5 + 0.5)).extend(1.0)
        }));
    let jup_mat = kit.mat(StandardMaterial {
        base_color_texture: Some(jup_tex),
        perceptual_roughness: 0.7,
        ..default()
    });
    let jup = kit.node(top, at(0.75, 2.1, -0.4));
    kit.ball(jup, 0.12, &jup_mat, Transform::IDENTITY);
    for (k, r) in [0.2f32, 0.27, 0.35, 0.45].into_iter().enumerate() {
        let orbit = kit.node(jup, Transform::from_rotation(Quat::from_rotation_y(k as f32 * 1.3)));
        kit.insert(orbit, Spin::y(1.2 / (k as f32 + 1.0)));
        kit.ball(orbit, 0.018, &pal.ivory, at(r, 0.0, 0.0));
    }
    bounds(0.0, 1.4, -0.2, 1.3)
}

fn ship(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let sea = kit.mat(StandardMaterial {
        base_color: Color::srgb(0.03, 0.12, 0.2),
        perceptual_roughness: 0.08,
        reflectance: 0.8,
        ..default()
    });
    kit.cube(top, Vec3::new(1.72, 0.04, 0.92), &sea, at(0.0, 0.02, 0.0));
    let roll = kit.node(
        top,
        Transform::from_xyz(0.0, 0.08, 0.0).with_rotation(Quat::from_rotation_y(0.55)),
    );
    kit.insert(
        roll,
        Swing {
            axis: Vec3::X,
            amplitude: 0.035,
            freq: 0.18,
            phase: 0.0,
            base: Quat::from_rotation_y(0.55),
        },
    );
    let pitch = kit.node(roll, Transform::IDENTITY);
    kit.insert(
        pitch,
        Swing {
            axis: Vec3::Z,
            amplitude: 0.02,
            freq: 0.11,
            phase: 1.0,
            base: Quat::IDENTITY,
        },
    );

    // Hull lofted from U-shaped stations.
    let (n, m) = (28, 16);
    let len = 1.2;
    let half_beam = |s: f32| -> f32 {
        if s >= 0.45 {
            0.19 * (1.0 - ((s - 0.45) / 0.55).powi(2)).max(0.0).sqrt()
        } else {
            0.19 * (1.0 - ((0.45 - s) / 0.62).powi(2)).max(0.0).sqrt()
        }
    };
    let depth = |s: f32| 0.08 + 0.1 * (1.0 - (2.0 * s - 1.0).powi(4)).max(0.0);
    let sheer = |s: f32| 0.24 + 0.5 * (s - 0.5).powi(2);
    let mut hull = MeshData::default();
    let mut rows = Vec::new();
    for i in 0..=n {
        let s = i as f32 / n as f32;
        let x = -len / 2.0 + len * s;
        let (w, d, h) = (half_beam(s), depth(s), sheer(s));
        let mut row = Vec::new();
        for j in 0..=m {
            let phi = -FRAC_PI_2 + PI * j as f32 / m as f32;
            let z = w * phi.sin().signum() * phi.sin().abs().powf(0.55);
            let y = h - d * phi.cos().powf(0.7);
            let p = Vec3::new(x, y, z);
            let outward = (p - Vec3::new(x, h, 0.0)).normalize_or(Vec3::NEG_Y);
            row.push(hull.vert(p, outward, Vec2::new(s * 3.0, j as f32 / m as f32)));
        }
        rows.push(row);
    }
    for i in 0..n {
        for j in 0..m {
            hull.quad(rows[i][j], rows[i + 1][j], rows[i + 1][j + 1], rows[i][j + 1]);
        }
    }
    let mut hull_mesh = hull.build();
    hull_mesh.compute_smooth_normals();
    let hull_mat = kit.mat(StandardMaterial {
        base_color: Color::srgb(0.42, 0.28, 0.18),
        base_color_texture: Some(pal.wood_tex.clone()),
        perceptual_roughness: 0.6,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    kit.shape(pitch, hull_mesh, &hull_mat, Transform::IDENTITY);
    // Deck planks
    let mut deck = MeshData::default();
    for i in 0..n {
        let s0 = i as f32 / n as f32;
        let s1 = (i + 1) as f32 / n as f32;
        let x0 = -len / 2.0 + len * s0;
        let x1 = -len / 2.0 + len * s1;
        let p = [
            Vec3::new(x0, sheer(s0) - 0.012, -half_beam(s0)),
            Vec3::new(x1, sheer(s1) - 0.012, -half_beam(s1)),
            Vec3::new(x1, sheer(s1) - 0.012, half_beam(s1)),
            Vec3::new(x0, sheer(s0) - 0.012, half_beam(s0)),
        ];
        deck.flat_quad(
            p,
            Vec3::Y,
            [
                Vec2::new(s0 * 4.0, 0.0),
                Vec2::new(s1 * 4.0, 0.0),
                Vec2::new(s1 * 4.0, 0.4),
                Vec2::new(s0 * 4.0, 0.4),
            ],
        );
    }
    kit.shape(pitch, deck.build(), &pal.wood, Transform::IDENTITY);
    // Castles fore and aft
    kit.cube(pitch, Vec3::new(0.24, 0.12, 0.22), &hull_mat, at(-0.46, 0.39, 0.0));
    kit.cube(pitch, Vec3::new(0.16, 0.08, 0.18), &hull_mat, at(0.44, 0.36, 0.0));
    // Masts, yards and sails
    let canvas = kit.mat(StandardMaterial {
        base_color: Color::srgb(0.9, 0.86, 0.74),
        perceptual_roughness: 0.9,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let red = pal.red.clone();
    for (x, h, sails) in [(0.32f32, 0.72f32, 2), (0.0, 0.92, 2), (-0.34, 0.6, 0)] {
        let base = sheer(x / len + 0.5);
        kit.rod(
            pitch,
            Vec3::new(x, base - 0.1, 0.0),
            Vec3::new(x, base + h, 0.0),
            0.011,
            &pal.dark_wood,
        );
        for k in 0..sails {
            let y0 = base + 0.18 + k as f32 * h * 0.42;
            let sh = h * 0.36;
            let sw = 0.34 - k as f32 * 0.08;
            let brace = kit.node(
                pitch,
                Transform::from_xyz(x, 0.0, 0.0).with_rotation(Quat::from_rotation_y(0.35)),
            );
            kit.rod(
                brace,
                Vec3::new(0.0, y0 + sh, -sw / 2.0 - 0.03),
                Vec3::new(0.0, y0 + sh, sw / 2.0 + 0.03),
                0.006,
                &pal.dark_wood,
            );
            kit.cube(brace, Vec3::new(0.006, sh, sw), &canvas, at(0.02, y0 + sh / 2.0, 0.0));
            if k == 0 {
                let d = (sh * sh + sw * sw).sqrt() * 0.8;
                let ang = sh.atan2(sw);
                for sgn in [-1.0, 1.0] {
                    kit.cube(
                        brace,
                        Vec3::new(0.004, 0.02, d),
                        &red,
                        Transform::from_xyz(0.026, y0 + sh / 2.0, 0.0).with_rotation(Quat::from_rotation_x(sgn * ang)),
                    );
                }
            }
        }
        if sails == 0 {
            // Lateen mizzen
            let tri = meshes::prism(&[v2(-0.2, 0.0), v2(0.18, 0.0), v2(0.12, 0.42)], 0.005);
            kit.shape(pitch, tri, &canvas, at(x, base + 0.12, 0.0));
        }
        let flag = kit.color(Color::srgb(0.9, 0.75, 0.2), 0.7);
        kit.cube(
            pitch,
            Vec3::new(0.08, 0.04, 0.004),
            &flag,
            at(x + 0.04, base + h + 0.01, 0.0),
        );
    }
    kit.rod(
        pitch,
        Vec3::new(0.55, sheer(0.96), 0.0),
        Vec3::new(0.78, sheer(1.0) + 0.14, 0.0),
        0.008,
        &pal.dark_wood,
    );
    bounds(0.0, 0.55, 0.0, 0.8)
}

fn newton_cannon(kit: &mut Kit, ctx: &mut Ctx, top: Entity) -> Bounds {
    let pal = kit.pal;
    let cy = 0.58;
    let er = 0.3;
    kit.rod(top, Vec3::ZERO, Vec3::new(0.0, cy - er + 0.02, 0.0), 0.03, &pal.brass);
    let tex = ctx.images.add(textures::globe(false));
    let earth = kit.mat(StandardMaterial {
        base_color_texture: Some(tex),
        perceptual_roughness: 0.5,
        ..default()
    });
    let center = Vec3::new(0.0, cy, 0.0);
    let e = kit.ball(top, er, &earth, Transform::from_translation(center));
    kit.insert(e, Spin::y(0.05));
    let rock = kit.color(Color::srgb(0.45, 0.42, 0.36), 0.9);
    kit.shape(
        top,
        Cone {
            radius: 0.07,
            height: 0.09,
        }
        .mesh()
        .resolution(16),
        &rock,
        at(0.0, cy + er + 0.03, 0.0),
    );
    let peak = center + Vec3::Y * (er + 0.075);
    kit.rod(top, peak, peak + Vec3::new(0.05, 0.004, 0.0), 0.007, &pal.iron);
    // Trajectories for increasing launch speeds: ellipses with the apogee at
    // the mountaintop, until one never comes down.
    let r0 = er + 0.08;
    let trail = kit.glow(Color::srgb(1.0, 0.85, 0.5), 4.0);
    for e in [0.75f32, 0.5, 0.25] {
        let mut pts = Vec::new();
        let mut phi = FRAC_PI_2;
        loop {
            let r = r0 * (1.0 - e) / (1.0 - e * (phi - FRAC_PI_2).cos());
            let p = center + Vec3::new(phi.cos() * r, phi.sin() * r, 0.0);
            pts.push(p);
            if r < er || pts.len() > 400 {
                break;
            }
            phi -= 0.02;
        }
        let t = kit.shape(
            top,
            meshes::tube(&pts, |_| 0.004, 6, false),
            &trail,
            Transform::IDENTITY,
        );
        kit.no_shadow(t);
    }
    let orbit = torus(kit, r0, 0.004);
    let o = kit.part(
        top,
        &orbit,
        &trail,
        Transform::from_translation(center).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
    );
    kit.no_shadow(o);
    let ball_orbit = kit.node(top, Transform::from_translation(center));
    kit.insert(ball_orbit, Spin::z(-0.9));
    kit.ball(ball_orbit, 0.016, &pal.iron, at(0.0, r0, 0.0));
    bounds(0.0, cy, 0.0, 0.5)
}

// -------------------------------------------------------------------- Space

fn wright_flyer(kit: &mut Kit, top: Entity, ceiling: f32) -> Bounds {
    let pal = kit.pal;
    let canvas = kit.mat(StandardMaterial {
        base_color: Color::srgb(0.9, 0.86, 0.76),
        perceptual_roughness: 0.85,
        ..default()
    });
    let spruce = kit.color(Color::srgb(0.72, 0.58, 0.4), 0.6);
    let span = 6.1;
    for y in [0.45, -0.45] {
        kit.cube(top, Vec3::new(span, 0.03, 0.95), &canvas, at(0.0, y, 0.0));
    }
    for x in [0.3f32, 1.05, 1.8, 2.55] {
        for sx in [-1.0, 1.0] {
            for z in [-0.4, 0.4] {
                kit.rod(
                    top,
                    Vec3::new(sx * x, -0.45, z),
                    Vec3::new(sx * x, 0.45, z),
                    0.012,
                    &spruce,
                );
            }
        }
    }
    // Skids running forward to the canard elevator
    for x in [-0.35, 0.35] {
        kit.rod(top, Vec3::new(x, -0.62, -0.5), Vec3::new(x, -0.62, 1.7), 0.014, &spruce);
        kit.rod(top, Vec3::new(x, -0.62, 1.7), Vec3::new(x, -0.1, 2.05), 0.014, &spruce);
        kit.rod(top, Vec3::new(x, -0.62, 0.2), Vec3::new(x, -0.45, 0.2), 0.012, &spruce);
        kit.rod(top, Vec3::new(x, -0.45, 0.4), Vec3::new(x, -0.1, 2.05), 0.01, &spruce);
        kit.rod(top, Vec3::new(x, 0.45, 0.4), Vec3::new(x, -0.1, 2.05), 0.01, &spruce);
    }
    for y in [0.12, -0.22] {
        kit.cube(top, Vec3::new(2.0, 0.02, 0.42), &canvas, at(0.0, y, 2.05));
    }
    // Twin rudders
    for x in [-0.25, 0.25] {
        kit.cube(top, Vec3::new(0.02, 0.9, 0.38), &canvas, at(x, 0.0, -2.0));
    }
    for (a, b) in [
        (Vec3::new(-0.4, 0.45, -0.45), Vec3::new(-0.25, 0.45, -1.82)),
        (Vec3::new(0.4, 0.45, -0.45), Vec3::new(0.25, 0.45, -1.82)),
        (Vec3::new(-0.4, -0.45, -0.45), Vec3::new(-0.25, -0.45, -1.82)),
        (Vec3::new(0.4, -0.45, -0.45), Vec3::new(0.25, -0.45, -1.82)),
    ] {
        kit.rod(top, a, b, 0.01, &spruce);
    }
    // Engine and chain-driven pusher propellers
    kit.cube(top, Vec3::new(0.3, 0.18, 0.28), &pal.iron, at(0.25, -0.34, 0.0));
    kit.cube(top, Vec3::new(0.12, 0.1, 0.5), &pal.ivory, at(-0.3, -0.4, 0.05));
    for sx in [-1.0f32, 1.0] {
        let hub = kit.node(top, at(sx * 1.2, 0.0, -0.62));
        kit.insert(hub, Spin::z(sx * 9.0));
        for k in 0..2 {
            let a = k as f32 * PI;
            kit.cube(
                hub,
                Vec3::new(1.3, 0.1, 0.02),
                &spruce,
                Transform::from_rotation(Quat::from_rotation_z(a) * Quat::from_rotation_x(0.35))
                    .with_translation(Vec3::ZERO),
            );
        }
        kit.rod(
            top,
            Vec3::new(0.25, -0.3, 0.0),
            Vec3::new(sx * 1.2, 0.0, -0.6),
            0.005,
            &pal.iron,
        );
    }
    cables(
        kit,
        top,
        &[
            Vec3::new(-2.2, 0.47, 0.0),
            Vec3::new(2.2, 0.47, 0.0),
            Vec3::new(0.0, 0.1, 2.05),
        ],
        ceiling,
    );
    bounds(0.0, 0.0, 0.0, 3.1)
}

fn sputnik(kit: &mut Kit, top: Entity, ceiling: f32) -> Bounds {
    let pal = kit.pal;
    let body = kit.node(top, Transform::IDENTITY);
    kit.insert(body, Spin::y(0.18));
    kit.ball(body, 0.29, &pal.chrome, Transform::IDENTITY);
    let seam = kit.mesh(Torus::new(0.285, 0.3).mesh().major_resolution(48).minor_resolution(6));
    kit.part(
        body,
        &seam,
        &pal.steel,
        Transform::from_rotation(Quat::from_rotation_x(FRAC_PI_2)).with_scale(Vec3::new(1.0, 0.4, 1.0)),
    );
    for (sx, sy, len) in [
        (1.0f32, 1.0f32, 2.4f32),
        (-1.0, 1.0, 2.9),
        (1.0, -1.0, 2.9),
        (-1.0, -1.0, 2.4),
    ] {
        let base = Vec3::new(sx * 0.13, sy * 0.13, 0.22);
        let dir = Vec3::new(sx * 0.42, sy * 0.42, -1.0).normalize();
        kit.rod(body, base, base + dir * len, 0.005, &pal.steel);
        kit.shape(
            body,
            Cylinder::new(0.02, 0.06).mesh().resolution(12),
            &pal.steel,
            Transform::from_translation(base).with_rotation(Quat::from_rotation_arc(Vec3::Y, dir)),
        );
    }
    cables(kit, top, &[Vec3::new(0.0, 0.29, 0.0)], ceiling);
    bounds(0.0, 0.0, 0.0, 1.2)
}

fn voyager(kit: &mut Kit, top: Entity, ceiling: f32) -> Bounds {
    let pal = kit.pal;
    let craft = kit.node(top, Transform::IDENTITY);
    kit.insert(craft, Spin::y(0.1));
    let dish = [
        v2(0.0, -0.03),
        v2(0.3, -0.01),
        v2(0.6, 0.06),
        v2(0.92, 0.18),
        v2(0.92, 0.18),
        v2(0.92, 0.2),
        v2(0.92, 0.2),
        v2(0.6, 0.08),
        v2(0.3, 0.01),
        v2(0.0, -0.01),
    ];
    let white = kit.color(Color::srgb(0.92, 0.92, 0.9), 0.5);
    let d = kit.node(
        craft,
        Transform::from_xyz(0.0, 0.0, 0.12).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
    );
    kit.lathe(d, &dish, &white, Transform::IDENTITY);
    for k in 0..3 {
        let a = k as f32 / 3.0 * TAU;
        kit.rod(
            d,
            Vec3::new(a.cos() * 0.8, 0.17, a.sin() * 0.8),
            Vec3::new(0.0, 0.55, 0.0),
            0.008,
            &white,
        );
    }
    kit.cyl(d, 0.05, 0.12, &white, at(0.0, 0.58, 0.0));
    // Ten-sided bus wrapped in foil
    let foil = kit.metal(Color::srgb(0.85, 0.65, 0.3), 0.45);
    kit.shape(
        craft,
        meshes::prism(&meshes::regular_polygon(10, 0.45, 0.0), 0.24),
        &foil,
        at(0.0, 0.0, -0.18),
    );
    kit.shape(
        craft,
        meshes::prism(&meshes::regular_polygon(10, 0.46, 0.0), 0.04),
        &pal.charcoal,
        at(0.0, 0.0, -0.05),
    );
    // The Golden Record on the side of the bus
    let rec = kit.node(
        craft,
        Transform::from_xyz(-0.47, -0.05, -0.18).with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
    );
    kit.cyl(rec, 0.16, 0.012, &pal.gold, Transform::IDENTITY);
    kit.cyl(rec, 0.03, 0.016, &pal.aluminium, Transform::IDENTITY);
    // RTG boom with three power units
    let a = Vec3::new(0.4, -0.15, -0.25);
    let b = Vec3::new(1.9, -0.55, -0.25);
    kit.rod(craft, a, b, 0.02, &pal.steel);
    let fin = kit.color(Color::srgb(0.18, 0.18, 0.2), 0.5);
    for k in 0..3 {
        let p = a.lerp(b, 0.45 + k as f32 * 0.2);
        let dir = (b - a).normalize();
        let rt = kit.node(
            craft,
            Transform::from_translation(p).with_rotation(Quat::from_rotation_arc(Vec3::Y, dir)),
        );
        kit.cyl(rt, 0.09, 0.22, &fin, Transform::IDENTITY);
        for f in 0..6 {
            let fa = f as f32 / 6.0 * TAU;
            kit.cube(
                rt,
                Vec3::new(0.08, 0.2, 0.01),
                &fin,
                Transform::from_xyz(fa.cos() * 0.11, 0.0, fa.sin() * 0.11).with_rotation(Quat::from_rotation_y(-fa)),
            );
        }
    }
    // Science boom and scan platform
    let s0 = Vec3::new(-0.4, 0.15, -0.25);
    let s1 = Vec3::new(-1.6, 0.35, -0.25);
    kit.rod(craft, s0, s1, 0.02, &pal.steel);
    kit.cube(
        craft,
        Vec3::new(0.26, 0.2, 0.22),
        &foil,
        Transform::from_translation(s1),
    );
    for z in [0.08, -0.08] {
        kit.shape(
            craft,
            Cylinder::new(0.04, 0.22).mesh().resolution(16),
            &pal.charcoal,
            Transform::from_translation(s1 + Vec3::new(0.0, 0.18, z)),
        );
    }
    // Magnetometer boom
    kit.rod(
        craft,
        Vec3::new(-0.1, 0.4, -0.25),
        Vec3::new(-1.3, 3.0, -0.25),
        0.008,
        &pal.steel,
    );
    cables(kit, top, &[Vec3::new(0.0, 0.95, 0.0)], ceiling);
    bounds(0.0, 0.2, 0.0, 1.8)
}

fn jwst(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    kit.cube(top, Vec3::new(0.34, 0.2, 0.34), &pal.charcoal, at(0.0, 0.1, 0.0));
    let panel = kit.metal(Color::srgb(0.12, 0.16, 0.3), 0.3);
    kit.cube(
        top,
        Vec3::new(0.7, 0.01, 0.3),
        &panel,
        Transform::from_xyz(0.0, 0.06, 0.4).with_rotation(Quat::from_rotation_x(0.3)),
    );
    let shield = kit.metal(Color::srgb(0.78, 0.7, 0.82), 0.22);
    let hex = [
        v2(1.08, 0.0),
        v2(0.52, 0.66),
        v2(-0.52, 0.66),
        v2(-1.08, 0.0),
        v2(-0.52, -0.66),
        v2(0.52, -0.66),
    ];
    for k in 0..5 {
        let s = 1.0 - k as f32 * 0.035;
        let pts: Vec<Vec2> = hex.iter().map(|p| *p * s).collect();
        kit.shape(
            top,
            meshes::prism(&pts, 0.004),
            &shield,
            Transform::from_xyz(0.0, 0.24 + k as f32 * 0.025, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
        );
    }
    kit.cube(top, Vec3::new(0.1, 0.3, 0.1), &pal.charcoal, at(0.0, 0.5, -0.05));
    let mirror = kit.node(
        top,
        Transform::from_xyz(0.0, 0.98, 0.0).with_rotation(Quat::from_rotation_x(-0.3)),
    );
    kit.cube(mirror, Vec3::new(0.85, 0.8, 0.05), &pal.black, at(0.0, 0.0, -0.04));
    let gold = kit.metal(Color::srgb(1.0, 0.78, 0.36), 0.1);
    let r = 0.078;
    let seg = kit.mesh(meshes::prism(&meshes::regular_polygon(6, r, 0.0), 0.014));
    for q in -2i32..=2 {
        for rr in -2i32..=2 {
            let s = -q - rr;
            if s.abs() > 2 || (q == 0 && rr == 0) {
                continue;
            }
            let x = 1.5 * r * q as f32 * 1.06;
            let y = 3f32.sqrt() * r * (rr as f32 + q as f32 / 2.0) * 1.06;
            kit.part(mirror, &seg, &gold, at(x, y, 0.0));
        }
    }
    let tip = Vec3::new(0.0, 0.0, 0.6);
    for a in [FRAC_PI_2, FRAC_PI_2 + TAU / 3.0, FRAC_PI_2 + 2.0 * TAU / 3.0] {
        kit.rod(
            mirror,
            Vec3::new(a.cos(), a.sin(), 0.0) * 0.38,
            tip,
            0.006,
            &pal.charcoal,
        );
    }
    kit.shape(
        mirror,
        Cylinder::new(0.045, 0.02).mesh().resolution(6),
        &gold,
        Transform::from_translation(tip).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
    );
    bounds(0.0, 0.8, 0.0, 1.1)
}

fn saturn_v(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let deck = kit.color(Color::srgb(0.35, 0.35, 0.36), 0.8);
    kit.cube(top, Vec3::new(1.8, 0.3, 1.8), &deck, at(0.0, 0.15, 0.0));
    for k in 0..4 {
        let a = k as f32 * FRAC_PI_2 + PI / 4.0;
        kit.cube(
            top,
            Vec3::new(0.12, 0.35, 0.12),
            &pal.steel,
            at(a.cos() * 0.6, 0.45, a.sin() * 0.6),
        );
    }
    let yb = 0.95;
    let white = kit.color(Color::srgb(0.93, 0.93, 0.9), 0.5);
    let black = pal.black.clone();
    let body = [
        v2(0.0, yb),
        v2(0.505, yb),
        v2(0.505, yb),
        v2(0.505, yb + 6.7),
        v2(0.505, yb + 6.7),
        v2(0.33, yb + 7.25),
        v2(0.33, yb + 7.25),
        v2(0.33, yb + 9.05),
        v2(0.33, yb + 9.05),
        v2(0.195, yb + 9.95),
        v2(0.195, yb + 9.95),
        v2(0.195, yb + 10.35),
        v2(0.195, yb + 10.35),
        v2(0.02, yb + 10.72),
        v2(0.0, yb + 10.72),
    ];
    kit.lathe(top, &body, &white, Transform::IDENTITY);
    let band = |kit: &mut Kit, y0: f32, y1: f32, r: f32| {
        kit.shape(
            top,
            Cylinder::new(r + 0.004, y1 - y0).mesh().resolution(48),
            &black,
            at(0.0, yb + (y0 + y1) / 2.0, 0.0),
        );
    };
    band(kit, 3.7, 4.2, 0.505);
    band(kit, 6.0, 6.25, 0.505);
    band(kit, 7.25, 7.55, 0.33);
    band(kit, 8.95, 9.05, 0.33);
    // The S-IC roll pattern: black panels near the base
    for k in 0..4 {
        let a = k as f32 * FRAC_PI_2;
        kit.cube(
            top,
            Vec3::new(0.32, 1.4, 0.02),
            &black,
            Transform::from_xyz(a.sin() * 0.5, yb + 1.0, a.cos() * 0.5).with_rotation(Quat::from_rotation_y(a)),
        );
    }
    let sm = kit.metal(Color::srgb(0.8, 0.8, 0.82), 0.3);
    kit.shape(
        top,
        Cylinder::new(0.2, 0.4).mesh().resolution(40),
        &sm,
        at(0.0, yb + 10.15, 0.0),
    );
    kit.rod(
        top,
        Vec3::new(0.0, yb + 10.7, 0.0),
        Vec3::new(0.0, yb + 11.1, 0.0),
        0.015,
        &pal.red,
    );
    kit.shape(
        top,
        Cone {
            radius: 0.04,
            height: 0.16,
        }
        .mesh()
        .resolution(16),
        &pal.red,
        at(0.0, yb + 11.18, 0.0),
    );
    // Fins and F-1 engines
    let fin = meshes::prism(&[v2(0.45, 0.0), v2(0.85, 0.0), v2(0.85, 0.16), v2(0.45, 0.85)], 0.035);
    let fin = kit.mesh(fin);
    for k in 0..4 {
        let a = k as f32 * FRAC_PI_2 + PI / 4.0;
        kit.part(
            top,
            &fin,
            &black,
            Transform::from_xyz(0.0, yb - 0.05, 0.0).with_rotation(Quat::from_rotation_y(a)),
        );
    }
    let bell = kit.mesh(meshes::lathe(
        &[
            v2(0.05, 0.0),
            v2(0.07, -0.15),
            v2(0.1, -0.38),
            v2(0.13, -0.58),
            v2(0.13, -0.58),
            v2(0.11, -0.58),
        ],
        24,
    ));
    let engine = kit.metal(Color::srgb(0.2, 0.2, 0.22), 0.5);
    for (x, z) in [(0.0, 0.0), (0.26, 0.26), (-0.26, 0.26), (0.26, -0.26), (-0.26, -0.26)] {
        kit.part(top, &bell, &engine, at(x, yb, z));
    }
    bounds(0.0, 3.5, 0.0, 3.5)
}

// ----------------------------------------------------------------- Industry

fn spoked_wheel(kit: &mut Kit, parent: Entity, r: f32, width: f32, spokes: usize, rim: &Mat, spoke: &Mat) {
    kit.shape(
        parent,
        meshes::radial_extrude(|_| r, 64, width, r * 0.86),
        rim,
        Transform::IDENTITY,
    );
    kit.shape(
        parent,
        Cylinder::new(r * 0.14, width * 1.6).mesh().resolution(20),
        spoke,
        Transform::from_rotation(Quat::from_rotation_x(FRAC_PI_2)),
    );
    for k in 0..spokes {
        let a = k as f32 / spokes as f32 * TAU;
        kit.rod(
            parent,
            Vec3::ZERO,
            Vec3::new(a.cos(), a.sin(), 0.0) * r * 0.88,
            r * 0.035,
            spoke,
        );
    }
}

fn steam_engine(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let green = kit.mat(StandardMaterial {
        base_color: Color::srgb(0.08, 0.26, 0.16),
        perceptual_roughness: 0.3,
        ..default()
    });
    let red = kit.mat(StandardMaterial {
        base_color: Color::srgb(0.5, 0.06, 0.05),
        perceptual_roughness: 0.35,
        ..default()
    });
    let iron = pal.iron.clone();
    let brass = pal.brass.clone();
    let steel = pal.steel.clone();
    let c = Vec3::new(0.5, 0.78, 0.0);
    kit.cube(top, Vec3::new(1.55, 0.14, 0.5), &iron, at(-0.35, 0.07, 0.0));
    // Cylinder
    kit.cube(top, Vec3::new(0.45, 0.5, 0.3), &iron, at(-0.75, 0.39, 0.0));
    let cyl = kit.node(
        top,
        Transform::from_xyz(-0.75, c.y, 0.0).with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
    );
    kit.cyl(cyl, 0.18, 0.5, &green, Transform::IDENTITY);
    for y in [-0.26, 0.26] {
        kit.cyl(cyl, 0.2, 0.03, &brass, at(0.0, y, 0.0));
    }
    kit.rod(
        top,
        Vec3::new(-0.75, c.y + 0.18, 0.0),
        Vec3::new(-0.75, 1.35, -0.2),
        0.04,
        &pal.copper,
    );
    // Crosshead guides
    for dy in [-0.075, 0.075] {
        kit.cube(top, Vec3::new(0.6, 0.025, 0.1), &steel, at(-0.18, c.y + dy, 0.0));
    }
    kit.cube(top, Vec3::new(0.08, 0.55, 0.14), &iron, at(0.12, 0.42, 0.0));
    // Bearing pedestal and crankshaft
    kit.cube(top, Vec3::new(0.18, 0.78, 0.16), &iron, at(c.x, 0.39, -0.18));
    kit.rod(top, c - Vec3::Z * 0.28, c + Vec3::Z * 0.3, 0.03, &steel);
    // Flywheel + crank (rotated by the linkage system)
    let wheel = kit.node(top, Transform::from_translation(c));
    let rim_node = kit.node(wheel, at(0.0, 0.0, 0.22));
    spoked_wheel(kit, rim_node, 0.62, 0.1, 6, &red, &red);
    kit.cube(wheel, Vec3::new(0.2, 0.06, 0.04), &iron, at(0.08, 0.0, 0.03));
    kit.shape(
        wheel,
        Cylinder::new(0.022, 0.08).mesh().resolution(12),
        &steel,
        Transform::from_xyz(0.16, 0.0, 0.0).with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
    );
    let rod = kit.node(top, Transform::from_translation(c));
    kit.cube(rod, Vec3::new(0.6, 0.045, 0.035), &steel, Transform::IDENTITY);
    let piston = kit.node(top, Transform::from_translation(c));
    kit.cube(piston, Vec3::new(0.1, 0.1, 0.12), &brass, Transform::IDENTITY);
    kit.rod(piston, Vec3::ZERO, Vec3::new(-0.72, 0.0, 0.0), 0.018, &steel);
    kit.cmd.spawn((
        SliderCrank {
            speed: 1.5,
            crank: 0.16,
            rod: 0.6,
            center: c,
            wheel,
            rod_entity: rod,
            piston,
            phase: 0.0,
        },
        Transform::IDENTITY,
        ChildOf(top),
    ));
    // Watt's centrifugal governor
    let gx = Vec3::new(-0.35, 0.14, -0.15);
    kit.rod(top, gx, gx + Vec3::Y * 0.85, 0.012, &steel);
    let gov = kit.node(top, Transform::from_translation(gx + Vec3::Y * 0.8));
    kit.insert(gov, Spin::y(3.0));
    kit.ball(gov, 0.025, &brass, at(0.0, 0.03, 0.0));
    for s in [-1.0, 1.0] {
        kit.rod(
            gov,
            Vec3::new(0.0, 0.02, 0.0),
            Vec3::new(s * 0.14, -0.14, 0.0),
            0.006,
            &steel,
        );
        kit.ball(gov, 0.045, &brass, at(s * 0.14, -0.14, 0.0));
    }
    bounds(0.0, 0.7, 0.0, 1.1)
}

fn locomotive(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let yellow = kit.mat(StandardMaterial {
        base_color: Color::srgb(0.95, 0.72, 0.12),
        perceptual_roughness: 0.35,
        ..default()
    });
    let black = pal.black.clone();
    let steel = pal.steel.clone();
    for i in 0..12 {
        kit.cube(
            top,
            Vec3::new(0.09, 0.03, 0.62),
            &pal.dark_wood,
            at(-1.2 + i as f32 * 0.22, 0.015, 0.0),
        );
    }
    for z in [-0.22, 0.22] {
        kit.cube(top, Vec3::new(2.5, 0.04, 0.035), &steel, at(0.0, 0.05, z));
    }
    let rail = 0.07;
    let body = kit.node(top, at(-0.1, 0.0, 0.0));
    // Boiler, firebox, chimney
    let boiler = kit.node(
        body,
        Transform::from_xyz(0.35, 0.58, 0.0).with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
    );
    kit.cyl(boiler, 0.18, 0.95, &yellow, Transform::IDENTITY);
    let band = kit.mesh(Torus::new(0.175, 0.19).mesh().major_resolution(32).minor_resolution(6));
    for y in [-0.4, -0.13, 0.13, 0.4] {
        kit.part(boiler, &band, &pal.brass, at(0.0, y, 0.0));
    }
    kit.cube(body, Vec3::new(0.28, 0.42, 0.4), &pal.copper, at(-0.24, 0.5, 0.0));
    let white = kit.color(Color::srgb(0.95, 0.95, 0.93), 0.4);
    kit.lathe(
        body,
        &[
            v2(0.0, 0.0),
            v2(0.07, 0.0),
            v2(0.05, 0.08),
            v2(0.045, 0.85),
            v2(0.07, 0.92),
            v2(0.07, 0.92),
            v2(0.0, 0.92),
        ],
        &white,
        at(0.78, 0.62, 0.0),
    );
    // Driving wheels with inclined cylinders and linkage
    let wheel_c = Vec3::new(0.55, rail + 0.32, 0.0);
    let angle = -0.55f32;
    for (side, phase) in [(-1.0f32, 0.0f32), (1.0, FRAC_PI_2)] {
        let z = side * 0.3;
        let frame = kit.node(
            body,
            Transform::from_xyz(0.0, 0.0, z).with_rotation(Quat::from_rotation_z(angle)),
        );
        let local_c = Quat::from_rotation_z(-angle) * wheel_c;
        let w = kit.node(frame, Transform::from_translation(local_c));
        spoked_wheel(kit, w, 0.32, 0.05, 12, &yellow, &yellow);
        kit.cube(w, Vec3::new(0.13, 0.04, 0.03), &black, at(0.06, 0.0, side * 0.035));
        let rod = kit.node(frame, Transform::from_translation(local_c));
        kit.cube(rod, Vec3::new(0.5, 0.03, 0.02), &steel, Transform::IDENTITY);
        let piston = kit.node(frame, Transform::from_translation(local_c));
        kit.rod(piston, Vec3::ZERO, Vec3::new(-0.3, 0.0, 0.0), 0.012, &steel);
        let cyl_c = local_c + Vec3::new(-0.12 - 0.5 - 0.32, 0.0, 0.0);
        let cyl = kit.node(
            frame,
            Transform::from_translation(cyl_c).with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
        );
        kit.cyl(cyl, 0.055, 0.3, &yellow, Transform::IDENTITY);
        kit.cmd.spawn((
            SliderCrank {
                speed: -1.4,
                crank: 0.12,
                rod: 0.5,
                center: local_c,
                wheel: w,
                rod_entity: rod,
                piston,
                phase,
            },
            Transform::IDENTITY,
            ChildOf(frame),
        ));
    }
    kit.rod(body, wheel_c - Vec3::Z * 0.32, wheel_c + Vec3::Z * 0.32, 0.025, &steel);
    for side in [-1.0f32, 1.0] {
        let w = kit.node(body, Transform::from_xyz(-0.15, rail + 0.18, side * 0.26));
        kit.insert(w, Spin::z(-1.4 * 0.32 / 0.18));
        spoked_wheel(kit, w, 0.18, 0.04, 10, &yellow, &yellow);
    }
    // Tender with a water barrel
    kit.cube(top, Vec3::new(0.55, 0.06, 0.46), &pal.wood, at(-0.85, rail + 0.2, 0.0));
    for x in [-1.02, -0.68] {
        for z in [-0.24, 0.24] {
            let w = kit.node(top, Transform::from_xyz(x, rail + 0.12, z));
            kit.insert(w, Spin::z(-1.4 * 0.32 / 0.12));
            spoked_wheel(kit, w, 0.12, 0.035, 8, &black, &black);
        }
    }
    let barrel = kit.node(
        top,
        Transform::from_xyz(-0.85, rail + 0.4, 0.0).with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
    );
    kit.cyl(barrel, 0.16, 0.45, &pal.wood, Transform::IDENTITY);
    for y in [-0.15, 0.15] {
        kit.cyl(barrel, 0.165, 0.02, &black, at(0.0, y, 0.0));
    }
    bounds(0.0, 0.55, 0.0, 1.3)
}

pub const ELEMENTS: [&str; 118] = [
    "H", "He", "Li", "Be", "B", "C", "N", "O", "F", "Ne", "Na", "Mg", "Al", "Si", "P", "S", "Cl", "Ar", "K", "Ca",
    "Sc", "Ti", "V", "Cr", "Mn", "Fe", "Co", "Ni", "Cu", "Zn", "Ga", "Ge", "As", "Se", "Br", "Kr", "Rb", "Sr", "Y",
    "Zr", "Nb", "Mo", "Tc", "Ru", "Rh", "Pd", "Ag", "Cd", "In", "Sn", "Sb", "Te", "I", "Xe", "Cs", "Ba", "La", "Ce",
    "Pr", "Nd", "Pm", "Sm", "Eu", "Gd", "Tb", "Dy", "Ho", "Er", "Tm", "Yb", "Lu", "Hf", "Ta", "W", "Re", "Os", "Ir",
    "Pt", "Au", "Hg", "Tl", "Pb", "Bi", "Po", "At", "Rn", "Fr", "Ra", "Ac", "Th", "Pa", "U", "Np", "Pu", "Am", "Cm",
    "Bk", "Cf", "Es", "Fm", "Md", "No", "Lr", "Rf", "Db", "Sg", "Bh", "Hs", "Mt", "Ds", "Rg", "Cn", "Nh", "Fl", "Mc",
    "Lv", "Ts", "Og",
];

/// Category color for atomic number `z`.
pub fn element_color(z: u32) -> Color {
    let hex = match z {
        3 | 11 | 19 | 37 | 55 | 87 => 0xd9534f,
        4 | 12 | 20 | 38 | 56 | 88 => 0xef9f4a,
        2 | 10 | 18 | 36 | 54 | 86 | 118 => 0xb57bb0,
        9 | 17 | 35 | 53 | 85 | 117 => 0x8f86d6,
        1 | 6 | 7 | 8 | 15 | 16 | 34 => 0x5f9ed6,
        5 | 14 | 32 | 33 | 51 | 52 => 0x4fb3a9,
        13 | 31 | 49 | 50 | 81..=84 | 113..=116 => 0x86b86a,
        57..=71 => 0xe0876a,
        89..=103 => 0xd67a8e,
        _ => 0xd9b35c,
    };
    Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// (row, column) of an element in the standard 18-column layout, with the
/// f-block in rows 8 and 9.
pub fn element_cell(z: u32) -> (u32, u32) {
    match z {
        1 => (0, 0),
        2 => (0, 17),
        3..=4 => (1, z - 3),
        5..=10 => (1, z + 7),
        11..=12 => (2, z - 11),
        13..=18 => (2, z - 1),
        19..=36 => (3, z - 19),
        37..=54 => (4, z - 37),
        55..=56 => (5, z - 55),
        57..=71 => (8, z - 57 + 2),
        72..=86 => (5, z - 72 + 3),
        87..=88 => (6, z - 87),
        89..=103 => (9, z - 89 + 2),
        _ => (6, z - 104 + 3),
    }
}

fn periodic_table(kit: &mut Kit, ctx: &mut Ctx, top: Entity) -> Bounds {
    let pal = kit.pal;
    let pitch = 0.172;
    let size = 0.158;
    let x0 = -pitch * 8.5;
    let y0 = 2.12;
    kit.cube(
        top,
        Vec3::new(18.0 * pitch + 0.12, 1.86, 0.05),
        &pal.black,
        at(0.0, y0 - 0.86, -0.06),
    );
    for x in [-1.3, 1.3] {
        kit.cube(top, Vec3::new(0.08, 0.35, 0.5), &pal.black, at(x, 0.175, -0.06));
    }
    let block = kit.mesh(Cuboid::new(size, size, 0.04));
    let face = kit.mesh(Rectangle::new(size, size));
    for z in 1..=118u32 {
        let (row, col) = element_cell(z);
        let gap = if row >= 8 { 0.08 } else { 0.0 };
        let p = Vec3::new(x0 + col as f32 * pitch, y0 - row as f32 * pitch - gap, 0.0);
        let body = kit.color(element_color(z).darker(0.15), 0.4);
        kit.part(top, &block, &body, Transform::from_translation(p));
        let mat = ctx.atlas.material(kit.mats, Cell::Element(z as usize - 1), false);
        kit.part(top, &face, &mat, Transform::from_translation(p + Vec3::Z * 0.0205));
        // Mendeleev's successful predictions get a gilded frame.
        if matches!(z, 21 | 31 | 32) {
            for (s, o) in [
                (
                    Vec3::new(size + 0.02, 0.01, 0.01),
                    Vec3::new(0.0, size / 2.0 + 0.005, 0.03),
                ),
                (
                    Vec3::new(size + 0.02, 0.01, 0.01),
                    Vec3::new(0.0, -size / 2.0 - 0.005, 0.03),
                ),
                (
                    Vec3::new(0.01, size + 0.02, 0.01),
                    Vec3::new(size / 2.0 + 0.005, 0.0, 0.03),
                ),
                (
                    Vec3::new(0.01, size + 0.02, 0.01),
                    Vec3::new(-size / 2.0 - 0.005, 0.0, 0.03),
                ),
            ] {
                kit.cube(top, s, &pal.gold, Transform::from_translation(p + o));
            }
        }
    }
    bounds(0.0, 1.3, 0.0, 1.6)
}

fn light_bulb(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let mut screw = vec![
        v2(0.0, 0.0),
        v2(0.03, 0.0),
        v2(0.03, 0.02),
        v2(0.03, 0.02),
        v2(0.09, 0.03),
        v2(0.09, 0.05),
        v2(0.09, 0.05),
    ];
    for k in 0..6 {
        let y = 0.05 + k as f32 * 0.028;
        screw.push(v2(0.1, y));
        screw.push(v2(0.113, y + 0.014));
    }
    screw.push(v2(0.1, 0.22));
    screw.push(v2(0.1, 0.22));
    screw.push(v2(0.0, 0.22));
    kit.lathe(top, &screw, &pal.brass, Transform::IDENTITY);
    kit.cyl(top, 0.092, 0.03, &pal.black, at(0.0, 0.035, 0.0));
    let bulb = [
        v2(0.098, 0.22),
        v2(0.11, 0.26),
        v2(0.16, 0.34),
        v2(0.24, 0.46),
        v2(0.27, 0.56),
        v2(0.26, 0.66),
        v2(0.2, 0.76),
        v2(0.11, 0.81),
        v2(0.0, 0.825),
    ];
    let glass = kit.mat(StandardMaterial {
        base_color: Color::srgba(1.0, 0.97, 0.9, 0.1),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.03,
        reflectance: 0.8,
        ..default()
    });
    let g = kit.lathe(top, &bulb, &glass, Transform::IDENTITY);
    kit.no_shadow(g);
    let stem = kit.mat(StandardMaterial {
        base_color: Color::srgba(0.9, 0.95, 0.95, 0.4),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let s = kit.lathe(
        top,
        &[v2(0.0, 0.22), v2(0.05, 0.22), v2(0.018, 0.4), v2(0.0, 0.41)],
        &stem,
        Transform::IDENTITY,
    );
    kit.no_shadow(s);
    for x in [-0.05f32, 0.05] {
        kit.rod(
            top,
            Vec3::new(x * 0.3, 0.3, 0.0),
            Vec3::new(x, 0.52, 0.0),
            0.003,
            &pal.steel,
        );
    }
    let pts: Vec<Vec3> = (0..=40)
        .map(|i| {
            let t = i as f32 / 40.0 * PI;
            Vec3::new(-0.05 * t.cos(), 0.52 + 0.1 * t.sin(), 0.02 * (t * 2.0).sin())
        })
        .collect();
    let filament = kit.glow(Color::srgb(1.0, 0.72, 0.4), 150.0);
    let f = kit.shape(
        top,
        meshes::tube(&pts, |_| 0.004, 6, false),
        &filament,
        Transform::IDENTITY,
    );
    kit.no_shadow(f);
    kit.cmd.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.72, 0.45),
            intensity: 40_000.0,
            range: 7.0,
            radius: 0.05,
            ..default()
        },
        at(0.0, 0.57, 0.0),
        ChildOf(top),
    ));
    bounds(0.0, 0.45, 0.0, 0.45)
}

fn radio(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let steel = kit.metal(Color::srgb(0.55, 0.2, 0.15), 0.5);
    let h = 3.0;
    let corner = |k: usize, y: f32| {
        let s = 0.28 + (0.04 - 0.28) * (y / h);
        let a = k as f32 * FRAC_PI_2 + PI / 4.0;
        Vec3::new(a.cos() * s, y, a.sin() * s)
    };
    for k in 0..4 {
        kit.rod(top, corner(k, 0.0), corner(k, h), 0.012, &steel);
        for lvl in 0..9 {
            let y0 = lvl as f32 * h / 9.0;
            let y1 = (lvl + 1) as f32 * h / 9.0;
            let (a, b) = if lvl % 2 == 0 {
                (corner(k, y0), corner((k + 1) % 4, y1))
            } else {
                (corner((k + 1) % 4, y0), corner(k, y1))
            };
            let e = kit.rod(top, a, b, 0.006, &steel);
            kit.no_shadow(e);
        }
    }
    kit.rod(
        top,
        Vec3::new(0.0, h, 0.0),
        Vec3::new(0.0, h + 0.5, 0.0),
        0.01,
        &pal.steel,
    );
    let tip = kit.glow(Color::srgb(1.0, 0.2, 0.15), 20.0);
    kit.ball(top, 0.03, &tip, at(0.0, h + 0.5, 0.0));
    let wave = kit.mat(StandardMaterial {
        base_color: Color::srgba(0.5, 0.85, 1.0, 0.5),
        emissive: LinearRgba::rgb(1.5, 4.0, 6.0),
        alpha_mode: AlphaMode::Add,
        unlit: true,
        ..default()
    });
    let ring = kit.mesh(Torus::new(0.97, 1.0).mesh().major_resolution(96).minor_resolution(6));
    for k in 0..3 {
        let e = kit.part(top, &ring, &wave, at(0.0, h + 0.3, 0.0));
        kit.insert(
            e,
            (
                Pulse {
                    period: 3.0,
                    offset: k as f32,
                    max: 1.5,
                },
                NotShadowCaster,
            ),
        );
    }
    // Marconi's spark transmitter on a small table
    kit.cube(top, Vec3::new(0.5, 0.5, 0.35), &pal.dark_wood, at(0.55, 0.25, 0.55));
    kit.cube(top, Vec3::new(0.34, 0.14, 0.24), &pal.wood, at(0.55, 0.57, 0.55));
    for x in [0.47f32, 0.63] {
        kit.rod(
            top,
            Vec3::new(x, 0.64, 0.55),
            Vec3::new(x, 0.76, 0.55),
            0.008,
            &pal.brass,
        );
        kit.ball(
            top,
            0.03,
            &pal.brass,
            at(x + if x < 0.55 { 0.035 } else { -0.035 }, 0.76, 0.55),
        );
    }
    let on = kit.glow(Color::srgb(0.6, 0.7, 1.0), 80.0);
    let off = kit.mat(StandardMaterial {
        base_color: Color::srgba(0.0, 0.0, 0.0, 0.0),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let spark = kit.ball(top, 0.014, &off, at(0.55, 0.76, 0.55));
    kit.insert(
        spark,
        (
            Blink {
                on,
                off,
                rate: 0.35,
                seed: 7,
                state: false,
            },
            NotShadowCaster,
        ),
    );
    bounds(0.0, 1.6, 0.0, 1.6)
}

// --------------------------------------------------------------------- Life

fn microscope(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let brass = pal.brass.clone();
    let black = pal.black.clone();
    let m = kit.node(top, at(-0.15, 0.0, 0.0));
    kit.lathe(
        m,
        &[
            v2(0.0, 0.0),
            v2(0.13, 0.0),
            v2(0.13, 0.03),
            v2(0.13, 0.03),
            v2(0.05, 0.05),
            v2(0.0, 0.05),
        ],
        &black,
        Transform::IDENTITY,
    );
    kit.rod(
        m,
        Vec3::new(0.0, 0.04, -0.05),
        Vec3::new(0.0, 0.2, -0.05),
        0.018,
        &brass,
    );
    kit.cube(m, Vec3::new(0.16, 0.012, 0.16), &black, at(0.0, 0.22, 0.02));
    kit.cube(m, Vec3::new(0.1, 0.003, 0.03), &pal.glass, at(0.0, 0.228, 0.02));
    kit.shape(
        m,
        Cylinder::new(0.035, 0.006).mesh().resolution(24),
        &pal.chrome,
        Transform::from_xyz(0.0, 0.11, 0.02).with_rotation(Quat::from_rotation_x(0.5)),
    );
    let arm: Vec<Vec3> = (0..=16)
        .map(|i| {
            let t = i as f32 / 16.0;
            Vec3::new(0.0, 0.2 + t * 0.26, -0.05 - (t * PI).sin() * 0.07)
        })
        .collect();
    kit.shape(m, meshes::tube(&arm, |_| 0.016, 10, true), &brass, Transform::IDENTITY);
    let body = kit.node(
        m,
        Transform::from_xyz(0.0, 0.26, 0.02).with_rotation(Quat::from_rotation_x(-0.18)),
    );
    kit.lathe(
        body,
        &[
            v2(0.0, 0.0),
            v2(0.012, 0.0),
            v2(0.02, 0.03),
            v2(0.022, 0.05),
            v2(0.022, 0.05),
            v2(0.03, 0.06),
            v2(0.03, 0.3),
            v2(0.03, 0.3),
            v2(0.024, 0.3),
            v2(0.024, 0.36),
            v2(0.024, 0.36),
            v2(0.0, 0.36),
        ],
        &brass,
        Transform::IDENTITY,
    );
    kit.cyl(body, 0.027, 0.03, &black, at(0.0, 0.345, 0.0));
    kit.shape(
        m,
        Cylinder::new(0.022, 0.03).mesh().resolution(16),
        &brass,
        Transform::from_xyz(0.035, 0.33, -0.06).with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
    );
    // A glass dome with the "animalcules" it revealed
    let dome_c = Vec3::new(0.3, 0.2, 0.08);
    kit.cyl(top, 0.2, 0.03, &pal.dark_wood, at(dome_c.x, 0.015, dome_c.z));
    let g = kit.ball(top, 0.19, &pal.glass, Transform::from_translation(dome_c));
    kit.no_shadow(g);
    let colors = [
        Color::srgb(0.4, 0.9, 0.5),
        Color::srgb(0.9, 0.6, 0.3),
        Color::srgb(0.5, 0.7, 1.0),
    ];
    for i in 0..10u32 {
        let c = colors[i as usize % 3];
        let mat = kit.mat(StandardMaterial {
            base_color: c.with_alpha(0.7),
            emissive: (c.to_linear() * 0.8).into(),
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        let p = dome_c + Vec3::new(hash01(i, 1) - 0.5, hash01(i, 2) - 0.5, hash01(i, 3) - 0.5) * 0.2;
        let n = kit.node(top, Transform::from_translation(p));
        kit.insert(
            n,
            Bob {
                amplitude: 0.01,
                freq: 0.3 + hash01(i, 4) * 0.4,
                base: p,
            },
        );
        let shape: Mesh = match i % 3 {
            0 => Capsule3d::new(0.01, 0.035).into(),
            1 => Sphere::new(0.015).mesh().uv(12, 8),
            _ => meshes::tube(&meshes::helix(0.008, 0.06, 3.0, 0.0, 30), |_| 0.003, 5, false),
        };
        let e = kit.shape(n, shape, &mat, Transform::IDENTITY);
        kit.insert(
            e,
            (
                Spin {
                    axis: Vec3::new(hash01(i, 5), 1.0, hash01(i, 6)).normalize(),
                    speed: 0.6,
                },
                NotShadowCaster,
            ),
        );
    }
    bounds(0.0, 0.25, 0.0, 0.5)
}

fn vaccine(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let c = Vec3::new(0.0, 0.32, -0.1);
    let virus = kit.node(top, Transform::from_translation(c));
    kit.insert(
        virus,
        (
            Spin::y(0.3),
            Bob {
                amplitude: 0.02,
                freq: 0.2,
                base: c,
            },
        ),
    );
    let coat = kit.mat(StandardMaterial {
        base_color: Color::srgb(0.55, 0.2, 0.22),
        perceptual_roughness: 0.6,
        ..default()
    });
    let tubule = kit.color(Color::srgb(0.75, 0.42, 0.38), 0.6);
    let scale = Vec3::new(0.2, 0.15, 0.12);
    kit.ball(virus, 1.0, &coat, Transform::from_scale(scale));
    let bump = kit.mesh(Capsule3d::new(0.012, 0.03));
    for i in 0..70u32 {
        let y = 1.0 - (i as f32 + 0.5) / 70.0 * 2.0;
        let rr = (1.0 - y * y).sqrt();
        let a = i as f32 * 2.399963;
        let unit = Vec3::new(a.cos() * rr, y, a.sin() * rr);
        let p = unit * scale;
        let n = (unit / scale).normalize();
        kit.part(
            virus,
            &bump,
            &tubule,
            Transform::from_translation(p)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, n) * Quat::from_rotation_x(FRAC_PI_2)),
        );
    }
    vitrine(kit, top, Vec3::new(0.7, 0.62, 0.62));
    // The bifurcated needle that eradicated smallpox, on a velvet pad
    kit.cube(
        top,
        Vec3::new(0.32, 0.015, 0.14),
        &pal.velvet,
        at(-0.3, 0.0075 + 0.62 * 0.0, 0.46),
    );
    kit.rod(
        top,
        Vec3::new(-0.42, 0.02, 0.46),
        Vec3::new(-0.18, 0.02, 0.46),
        0.003,
        &pal.chrome,
    );
    for dz in [-0.004, 0.004] {
        kit.rod(
            top,
            Vec3::new(-0.18, 0.02, 0.46),
            Vec3::new(-0.16, 0.02, 0.46 + dz),
            0.0015,
            &pal.chrome,
        );
    }
    let vial = kit.lathe(
        top,
        &[
            v2(0.0, 0.0),
            v2(0.03, 0.0),
            v2(0.03, 0.08),
            v2(0.012, 0.1),
            v2(0.012, 0.12),
            v2(0.0, 0.12),
        ],
        &pal.glass,
        at(0.25, 0.0, 0.45),
    );
    kit.no_shadow(vial);
    let liquid = kit.color(Color::srgb(0.9, 0.85, 0.6), 0.2);
    kit.cyl(top, 0.026, 0.05, &liquid, at(0.25, 0.026, 0.45));
    kit.cyl(top, 0.015, 0.02, &pal.red, at(0.25, 0.125, 0.45));
    bounds(0.0, 0.32, 0.0, 0.45)
}

fn tree_of_life(kit: &mut Kit, top: Entity) -> Bounds {
    let bronze = kit.pal.bronze.clone();
    let leaves = [
        kit.glow(Color::srgb(0.4, 0.9, 0.5), 6.0),
        kit.glow(Color::srgb(0.4, 0.7, 1.0), 6.0),
        kit.glow(Color::srgb(1.0, 0.7, 0.3), 6.0),
    ];
    let joint = kit.mesh(Sphere::new(1.0).mesh().uv(12, 8));
    let leaf = kit.mesh(Sphere::new(0.035).mesh().ico(2).unwrap());
    struct B {
        base: Vec3,
        dir: Vec3,
        len: f32,
        r: f32,
        depth: u32,
        clade: usize,
        id: u32,
    }
    let mut stack = vec![B {
        base: Vec3::ZERO,
        dir: Vec3::Y,
        len: 0.55,
        r: 0.05,
        depth: 6,
        clade: 0,
        id: 1,
    }];
    while let Some(b) = stack.pop() {
        let end = b.base + b.dir * b.len;
        kit.rod(top, b.base, end, b.r, &bronze);
        kit.part(
            top,
            &joint,
            &bronze,
            Transform::from_translation(end).with_scale(Vec3::splat(b.r)),
        );
        if b.depth == 0 {
            let e = kit.part(
                top,
                &leaf,
                &leaves[b.clade],
                Transform::from_translation(end + b.dir * 0.03),
            );
            kit.no_shadow(e);
            continue;
        }
        let n = if b.depth == 6 {
            3
        } else if hash01(b.id, 3) > 0.7 {
            3
        } else {
            2
        };
        let side = b.dir.any_orthonormal_vector();
        for k in 0..n {
            let around = Quat::from_axis_angle(b.dir, k as f32 / n as f32 * TAU + hash01(b.id, 4 + k) * 1.5);
            let spread = 0.45 + hash01(b.id, 9 + k) * 0.25;
            let axis = around * side;
            let mut dir = Quat::from_axis_angle(axis, spread) * b.dir;
            dir = (dir + Vec3::Y * 0.35).normalize();
            stack.push(B {
                base: end,
                dir,
                len: b.len * (0.74 + hash01(b.id, 20 + k) * 0.12),
                r: b.r * 0.68,
                depth: b.depth - 1,
                clade: if b.depth == 6 { k as usize % 3 } else { b.clade },
                id: b.id * 3 + k,
            });
        }
    }
    bounds(0.0, 1.0, 0.0, 1.0)
}

fn penicillin(kit: &mut Kit, ctx: &mut Ctx, top: Entity) -> Bounds {
    let pal = kit.pal;
    let dish = [
        v2(0.0, 0.0),
        v2(0.32, 0.0),
        v2(0.32, 0.0),
        v2(0.32, 0.045),
        v2(0.32, 0.045),
        v2(0.31, 0.045),
        v2(0.31, 0.045),
        v2(0.31, 0.006),
        v2(0.31, 0.006),
        v2(0.0, 0.006),
    ];
    let d = kit.lathe(top, &dish, &pal.glass, Transform::IDENTITY);
    kit.no_shadow(d);
    let agar = kit.color(Color::srgb(0.8, 0.7, 0.42), 0.4);
    kit.cyl(top, 0.305, 0.02, &agar, at(0.0, 0.016, 0.0));
    let tex = ctx.images.add(textures::petri_dish());
    let plate = kit.mat(StandardMaterial {
        base_color_texture: Some(tex),
        perceptual_roughness: 0.35,
        ..default()
    });
    kit.shape(
        top,
        Circle::new(0.305).mesh().resolution(64),
        &plate,
        Transform::from_xyz(0.0, 0.0265, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
    );
    let mold = kit.color(Color::srgb(0.82, 0.88, 0.78), 0.95);
    let fuzz = kit.mesh(knapped(Sphere::new(0.06).mesh().ico(3).unwrap().into(), 0.012, 51));
    // The colony sits where the texture draws it: u,v = (0.375, 0.4).
    let pos = Vec3::new((0.375 - 0.5) * 0.61, 0.028, (0.4 - 0.5) * 0.61);
    kit.part(
        top,
        &fuzz,
        &mold,
        Transform::from_translation(pos).with_scale(Vec3::new(1.0, 0.35, 1.0)),
    );
    let lid = kit.lathe(
        top,
        &[v2(0.0, 0.03), v2(0.33, 0.03), v2(0.33, 0.03), v2(0.33, 0.0)],
        &pal.glass,
        Transform::from_xyz(0.28, 0.13, -0.08).with_rotation(Quat::from_rotation_z(0.45)),
    );
    kit.no_shadow(lid);
    // A rack of culture tubes
    kit.cube(top, Vec3::new(0.4, 0.1, 0.1), &pal.wood, at(-0.3, 0.05, -0.42));
    let colors = [
        Color::srgb(0.8, 0.7, 0.4),
        Color::srgb(0.7, 0.8, 0.4),
        Color::srgb(0.9, 0.6, 0.4),
        Color::srgb(0.8, 0.75, 0.5),
        Color::srgb(0.7, 0.6, 0.35),
    ];
    for (k, c) in colors.into_iter().enumerate() {
        let x = -0.46 + k as f32 * 0.08;
        let t = kit.lathe(
            top,
            &[v2(0.0, 0.0), v2(0.018, 0.01), v2(0.018, 0.22), v2(0.0, 0.22)],
            &pal.glass,
            at(x, 0.02, -0.42),
        );
        kit.no_shadow(t);
        let m = kit.color(c, 0.3);
        kit.cyl(top, 0.015, 0.07, &m, at(x, 0.06, -0.42));
    }
    bounds(0.0, 0.1, 0.0, 0.45)
}

fn dna(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let helix = kit.node(top, at(0.0, 0.15, 0.0));
    kit.insert(helix, Spin::y(0.25));
    let (radius, height, turns) = (0.38, 2.6, 2.6);
    let offset = 0.75 * PI;
    let backbone = kit.mat(StandardMaterial {
        base_color: Color::srgb(0.9, 0.9, 0.88),
        metallic: 0.3,
        perceptual_roughness: 0.2,
        ..default()
    });
    for phase in [0.0, offset] {
        let pts = meshes::helix(radius, height, turns, phase, 260);
        kit.shape(
            helix,
            meshes::tube(&pts, |_| 0.032, 10, true),
            &backbone,
            Transform::IDENTITY,
        );
    }
    let bases = [
        kit.color(Color::srgb(0.88, 0.32, 0.3), 0.3),
        kit.color(Color::srgb(0.3, 0.5, 0.9), 0.3),
        kit.color(Color::srgb(0.3, 0.75, 0.42), 0.3),
        kit.color(Color::srgb(0.95, 0.78, 0.25), 0.3),
    ];
    let node = kit.mesh(Sphere::new(0.05).mesh().uv(16, 10));
    let rungs = (turns * 10.5) as usize;
    for i in 0..rungs {
        let t = (i as f32 + 0.5) / rungs as f32;
        let a = t * turns * TAU;
        let y = t * height;
        let p1 = Vec3::new(a.cos() * radius, y, a.sin() * radius);
        let p2 = Vec3::new((a + offset).cos() * radius, y, (a + offset).sin() * radius);
        let pair = (hash01(i as u32, 77) * 4.0) as usize % 4;
        let (m1, m2) = match pair {
            0 => (0, 1),
            1 => (1, 0),
            2 => (2, 3),
            _ => (3, 2),
        };
        let mid = (p1 + p2) / 2.0;
        kit.rod(helix, p1, mid - (p2 - p1).normalize() * 0.01, 0.018, &bases[m1]);
        kit.rod(helix, p2, mid + (p2 - p1).normalize() * 0.01, 0.018, &bases[m2]);
        kit.part(helix, &node, &pal.chrome, Transform::from_translation(p1));
        kit.part(helix, &node, &pal.chrome, Transform::from_translation(p2));
    }
    bounds(0.0, 1.45, 0.0, 1.4)
}

// -------------------------------------------------------------- Information

fn analytical_engine(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let brass = pal.brass.clone();
    let steel = pal.steel.clone();
    kit.cyl(top, 0.56, 0.03, &brass, at(0.0, 0.015, 0.0));
    kit.cyl(top, 0.56, 0.03, &brass, at(0.0, 1.06, 0.0));
    for k in 0..6 {
        let a = k as f32 / 6.0 * TAU;
        kit.rod(
            top,
            Vec3::new(a.cos() * 0.52, 0.0, a.sin() * 0.52),
            Vec3::new(a.cos() * 0.52, 1.05, a.sin() * 0.52),
            0.014,
            &steel,
        );
    }
    let big = kit.mesh(meshes::gear(0.15, 30, 0.014, 0.014));
    let small = kit.mesh(meshes::gear(0.1, 20, 0.012, 0.012));
    let flat = Quat::from_rotation_x(FRAC_PI_2);
    let column = |kit: &mut Kit, p: Vec3, gear: &Handle<Mesh>, speed: f32, mat: &Mat| {
        let c = kit.node(top, Transform::from_translation(p));
        kit.insert(c, Spin::y(speed));
        kit.rod(c, Vec3::new(0.0, 0.03, 0.0), Vec3::new(0.0, 1.04, 0.0), 0.012, &steel);
        for k in 0..12 {
            kit.part(
                c,
                gear,
                mat,
                Transform::from_xyz(0.0, 0.1 + k as f32 * 0.075, 0.0)
                    .with_rotation(flat * Quat::from_rotation_z(k as f32 * 0.3)),
            );
        }
    };
    column(kit, Vec3::ZERO, &big, 0.3, &brass);
    for k in 0..8 {
        let a = k as f32 / 8.0 * TAU;
        let speed = if k % 2 == 0 { -0.45 } else { 0.45 };
        let mat = if k % 2 == 0 { pal.bronze.clone() } else { brass.clone() };
        column(kit, Vec3::new(a.cos() * 0.35, 0.0, a.sin() * 0.35), &small, speed, &mat);
    }
    // Punched cards, as for Jacquard's loom
    let card = kit.color(Color::srgb(0.85, 0.78, 0.6), 0.9);
    let hole = pal.black.clone();
    for i in 0..10 {
        let t = Transform::from_xyz(0.7, 0.03 + i as f32 * 0.012, 0.1)
            .with_rotation(Quat::from_rotation_y(if i % 2 == 0 { 0.05 } else { -0.05 }));
        kit.cube(top, Vec3::new(0.2, 0.006, 0.34), &card, t);
        let c = kit.node(top, t);
        if i == 9 {
            for h in 0..24u32 {
                if hash01(h, 2) > 0.5 {
                    kit.cube(
                        c,
                        Vec3::new(0.012, 0.007, 0.012),
                        &hole,
                        at(-0.08 + (h % 6) as f32 * 0.032, 0.0, -0.12 + (h / 6) as f32 * 0.08),
                    );
                }
            }
        }
    }
    bounds(0.0, 0.55, 0.0, 0.8)
}

fn eniac(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let cabinet = kit.color(Color::srgb(0.08, 0.08, 0.09), 0.5);
    let panel = kit.color(Color::srgb(0.15, 0.15, 0.16), 0.5);
    let on = kit.glow(Color::srgb(1.0, 0.45, 0.15), 30.0);
    let off = kit.color(Color::srgb(0.25, 0.08, 0.05), 0.4);
    let knob = pal.ivory.clone();
    let lamp = kit.mesh(Sphere::new(0.013).mesh().ico(1).unwrap());
    let knob_mesh = kit.mesh(Cylinder::new(0.016, 0.02).mesh().resolution(10));
    let mut seed = 0;
    for c in 0..4 {
        let x = -1.26 + c as f32 * 0.84;
        kit.cube(top, Vec3::new(0.8, 2.1, 0.5), &cabinet, at(x, 1.05, -0.1));
        kit.cube(top, Vec3::new(0.7, 0.9, 0.02), &panel, at(x, 1.5, 0.16));
        for row in 0..6 {
            for col in 0..10 {
                seed += 1;
                let p = Vec3::new(x - 0.3 + col as f32 * 0.066, 1.2 + row as f32 * 0.11, 0.18);
                let e = kit.part(top, &lamp, &off, Transform::from_translation(p));
                kit.insert(
                    e,
                    (
                        Blink {
                            on: on.clone(),
                            off: off.clone(),
                            rate: 0.3,
                            seed,
                            state: false,
                        },
                        NotShadowCaster,
                    ),
                );
            }
        }
        kit.cube(top, Vec3::new(0.7, 0.55, 0.02), &panel, at(x, 0.6, 0.16));
        for row in 0..3 {
            for col in 0..5 {
                kit.part(
                    top,
                    &knob_mesh,
                    &knob,
                    Transform::from_xyz(x - 0.24 + col as f32 * 0.12, 0.45 + row as f32 * 0.15, 0.18)
                        .with_rotation(Quat::from_rotation_x(FRAC_PI_2)),
                );
            }
        }
    }
    // Patch cables looping between panels
    let cable_colors = [
        Color::srgb(0.1, 0.1, 0.1),
        Color::srgb(0.6, 0.1, 0.1),
        Color::srgb(0.1, 0.2, 0.5),
    ];
    for k in 0..6 {
        let x0 = -1.26 + (k % 3) as f32 * 0.84 + 0.2;
        let a = Vec3::new(x0, 1.0 - (k / 3) as f32 * 0.12, 0.19);
        let b = Vec3::new(x0 + 0.6, 0.95 - (k / 3) as f32 * 0.12, 0.19);
        let m = kit.color(cable_colors[k % 3], 0.6);
        let mut pts = meshes::sag(a, b, 0.22, 20);
        for p in pts.iter_mut() {
            p.z += 0.02;
        }
        kit.shape(top, meshes::tube(&pts, |_| 0.009, 6, false), &m, Transform::IDENTITY);
    }
    // A tray of vacuum tubes
    kit.cube(top, Vec3::new(0.5, 0.04, 0.3), &pal.aluminium, at(1.55, 0.6, 0.35));
    kit.cube(top, Vec3::new(0.06, 0.6, 0.06), &pal.charcoal, at(1.55, 0.3, 0.35));
    let tube_glass = kit.mat(StandardMaterial {
        base_color: Color::srgba(0.9, 0.9, 0.95, 0.25),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.05,
        ..default()
    });
    let filament = kit.glow(Color::srgb(1.0, 0.45, 0.1), 40.0);
    for i in 0..8 {
        let p = Vec3::new(1.37 + (i % 4) as f32 * 0.12, 0.62, 0.28 + (i / 4) as f32 * 0.14);
        let t = kit.lathe(
            top,
            &[
                v2(0.0, 0.0),
                v2(0.03, 0.0),
                v2(0.03, 0.13),
                v2(0.02, 0.16),
                v2(0.0, 0.165),
            ],
            &tube_glass,
            Transform::from_translation(p),
        );
        kit.no_shadow(t);
        kit.cube(
            top,
            Vec3::new(0.03, 0.06, 0.01),
            &pal.steel,
            Transform::from_translation(p + Vec3::Y * 0.07),
        );
        let f = kit.cube(
            top,
            Vec3::new(0.006, 0.05, 0.006),
            &filament,
            Transform::from_translation(p + Vec3::new(0.0, 0.07, 0.012)),
        );
        kit.no_shadow(f);
    }
    bounds(0.0, 1.1, 0.0, 1.9)
}

fn transistor(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    kit.cube(top, Vec3::new(0.36, 0.06, 0.36), &pal.black, at(0.0, 0.03, 0.0));
    kit.cube(top, Vec3::new(0.24, 0.04, 0.24), &pal.copper, at(0.0, 0.08, 0.0));
    let germanium = kit.metal(Color::srgb(0.45, 0.47, 0.5), 0.25);
    kit.cube(top, Vec3::new(0.2, 0.05, 0.2), &germanium, at(0.0, 0.125, 0.0));
    let amber = kit.mat(StandardMaterial {
        base_color: Color::srgba(0.95, 0.75, 0.4, 0.55),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.1,
        ..default()
    });
    let base = 0.15;
    let wedge = meshes::prism(&[v2(-0.07, 0.16), v2(0.07, 0.16), v2(0.0, 0.0)], 0.12);
    kit.shape(top, wedge, &amber, at(0.0, base, 0.0));
    let slope = (0.07f32).atan2(0.16);
    let len = (0.07f32 * 0.07 + 0.16 * 0.16).sqrt();
    for s in [-1.0f32, 1.0] {
        let mid = Vec3::new(s * 0.035 + s * 0.004, base + 0.08, 0.0);
        kit.cube(
            top,
            Vec3::new(0.004, len - 0.008, 0.08),
            &pal.gold,
            Transform::from_translation(mid).with_rotation(Quat::from_rotation_z(s * slope)),
        );
        let wire: Vec<Vec3> = (0..=12)
            .map(|i| {
                let t = i as f32 / 12.0;
                Vec3::new(
                    s * (0.07 + t * 0.12),
                    base + 0.14 - t * 0.12 + (t * PI).sin() * 0.05,
                    0.03,
                )
            })
            .collect();
        kit.shape(
            top,
            meshes::tube(&wire, |_| 0.003, 5, false),
            &pal.copper,
            Transform::IDENTITY,
        );
    }
    // Spring pressing the wedge down, held by a bent strip
    let spring = meshes::helix(0.022, 0.1, 6.0, 0.0, 120)
        .into_iter()
        .map(|p| p + Vec3::Y * (base + 0.16))
        .collect::<Vec<_>>();
    kit.shape(
        top,
        meshes::tube(&spring, |_| 0.003, 6, false),
        &pal.steel,
        Transform::IDENTITY,
    );
    let clip = [
        Vec3::new(0.0, base + 0.26, 0.0),
        Vec3::new(0.0, base + 0.29, 0.0),
        Vec3::new(0.0, base + 0.3, -0.03),
        Vec3::new(0.0, base + 0.3, -0.15),
        Vec3::new(0.0, base + 0.25, -0.16),
        Vec3::new(0.0, 0.07, -0.16),
    ];
    kit.shape(
        top,
        meshes::tube(&clip, |_| 0.006, 8, true),
        &pal.steel,
        Transform::IDENTITY,
    );
    bounds(0.0, 0.2, 0.0, 0.35)
}

fn microchip(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let ceramic = kit.color(Color::srgb(0.9, 0.9, 0.88), 0.4);
    let stripe = kit.color(Color::srgb(0.45, 0.45, 0.48), 0.4);
    let (w, d) = (0.6, 0.22);
    kit.cube(top, Vec3::new(w, 0.05, d), &ceramic, at(0.0, 0.1, 0.0));
    for x in [-0.12, 0.12] {
        kit.cube(top, Vec3::new(0.04, 0.052, d + 0.002), &stripe, at(x, 0.1, 0.0));
    }
    kit.cube(top, Vec3::new(0.12, 0.004, 0.1), &pal.gold, at(0.0, 0.127, 0.0));
    for i in 0..8 {
        let x = -w / 2.0 + 0.05 + i as f32 * (w - 0.1) / 7.0;
        for s in [-1.0, 1.0] {
            kit.cube(
                top,
                Vec3::new(0.022, 0.006, 0.04),
                &pal.chrome,
                at(x, 0.09, s * (d / 2.0 + 0.02)),
            );
            kit.cube(
                top,
                Vec3::new(0.022, 0.09, 0.006),
                &pal.chrome,
                at(x, 0.045, s * (d / 2.0 + 0.04)),
            );
        }
    }
    // The die, floating above, with glowing wiring
    let c = Vec3::new(0.0, 0.48, 0.0);
    let die = kit.node(top, Transform::from_translation(c));
    kit.insert(
        die,
        (
            Bob {
                amplitude: 0.02,
                freq: 0.2,
                base: c,
            },
            Swing {
                axis: Vec3::Y,
                amplitude: 0.5,
                freq: 0.05,
                phase: 0.0,
                base: Quat::from_rotation_x(0.55),
            },
        ),
    );
    let silicon = kit.metal(Color::srgb(0.18, 0.2, 0.28), 0.25);
    kit.cube(die, Vec3::new(0.4, 0.015, 0.5), &silicon, Transform::IDENTITY);
    let trace = kit.glow(Color::srgb(0.3, 0.85, 1.0), 6.0);
    let gold_trace = kit.glow(Color::srgb(1.0, 0.75, 0.3), 5.0);
    for i in 0..90u32 {
        let horizontal = hash01(i, 1) > 0.5;
        let len = 0.04 + hash01(i, 2) * 0.16;
        let x = (hash01(i, 3) - 0.5) * 0.34;
        let z = (hash01(i, 4) - 0.5) * 0.44;
        let size = if horizontal {
            Vec3::new(len, 0.002, 0.004)
        } else {
            Vec3::new(0.004, 0.002, len)
        };
        let clamp = |v: f32, half: f32, lim: f32| v.clamp(-lim + half, lim - half);
        let p = Vec3::new(clamp(x, size.x / 2.0, 0.19), 0.0085, clamp(z, size.z / 2.0, 0.24));
        let mat = if hash01(i, 5) > 0.7 { &gold_trace } else { &trace };
        let e = kit.cube(die, size, mat, Transform::from_translation(p));
        kit.no_shadow(e);
    }
    for i in 0..16 {
        let t = i as f32 / 16.0 * 4.0;
        let (x, z) = match t as i32 {
            0 => (-0.18 + t.fract() * 0.36, -0.23),
            1 => (0.18, -0.23 + t.fract() * 0.46),
            2 => (0.18 - t.fract() * 0.36, 0.23),
            _ => (-0.18, 0.23 - t.fract() * 0.46),
        };
        kit.cube(die, Vec3::new(0.018, 0.003, 0.018), &pal.gold, at(x, 0.009, z));
    }
    bounds(0.0, 0.35, 0.0, 0.45)
}

fn web(kit: &mut Kit, top: Entity) -> Bounds {
    let pal = kit.pal;
    let ring = kit.glow(Color::srgb(0.4, 0.7, 1.0), 8.0);
    let t = torus(kit, 0.9, 0.012);
    kit.part(top, &t, &ring, at(0.0, 0.02, 0.0));
    let c = Vec3::new(0.0, 1.3, 0.0);
    let globe = kit.node(top, Transform::from_translation(c));
    kit.insert(globe, Spin::y(0.15));
    let inner = kit.mat(StandardMaterial {
        base_color: Color::srgba(0.05, 0.12, 0.3, 0.55),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.1,
        reflectance: 0.7,
        ..default()
    });
    let g = kit.ball(globe, 0.72, &inner, Transform::IDENTITY);
    kit.no_shadow(g);
    let n = 80;
    let pts: Vec<Vec3> = (0..n)
        .map(|i| {
            let y = 1.0 - (i as f32 + 0.5) / n as f32 * 2.0;
            let r = (1.0 - y * y).sqrt();
            let a = i as f32 * 2.399963;
            Vec3::new(a.cos() * r, y, a.sin() * r) * 0.8
        })
        .collect();
    let node = kit.glow(Color::srgb(0.5, 0.9, 1.0), 25.0);
    let node_mesh = kit.mesh(Sphere::new(0.02).mesh().ico(2).unwrap());
    for p in &pts {
        let e = kit.part(globe, &node_mesh, &node, Transform::from_translation(*p));
        kit.no_shadow(e);
    }
    let link = kit.glow(Color::srgb(0.25, 0.5, 1.0), 3.0);
    let mut edges = Vec::new();
    for i in 0..n {
        let mut near: Vec<(f32, usize)> = (0..n)
            .filter(|&j| j != i)
            .map(|j| (pts[i].distance(pts[j]), j))
            .collect();
        near.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        for &(_, j) in near.iter().take(3) {
            let key = (i.min(j), i.max(j));
            if !edges.contains(&key) {
                edges.push(key);
            }
        }
    }
    for (a, b) in &edges {
        let e = kit.rod(globe, pts[*a], pts[*b], 0.0035, &link);
        kit.no_shadow(e);
    }
    let packet = kit.glow(Color::srgb(1.0, 1.0, 1.0), 60.0);
    let packet_mesh = kit.mesh(Sphere::new(0.012).mesh().ico(1).unwrap());
    for k in 0..30u32 {
        let (a, b) = edges[(hash01(k, 1) * edges.len() as f32) as usize % edges.len()];
        let e = kit.part(globe, &packet_mesh, &packet, Transform::from_translation(pts[a]));
        kit.insert(
            e,
            (
                Travel {
                    from: pts[a],
                    to: pts[b],
                    speed: 0.5 + hash01(k, 2),
                    offset: hash01(k, 3),
                },
                NotShadowCaster,
            ),
        );
    }
    let _ = pal;
    bounds(0.0, 1.3, 0.0, 0.95)
}
