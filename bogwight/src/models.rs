//! Materials and every mesh in the game, built from primitives at startup.
//! Models face +X; z is depth (toward the camera is +z).

use crate::{
    meshkit::{Col, MeshBuilder},
    util::{Rng, lin, mix, shade, smoothstep},
};
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

#[derive(Resource)]
pub struct Mats {
    pub matte: Handle<StandardMaterial>,
    pub wet: Handle<StandardMaterial>,
    pub cloth: Handle<StandardMaterial>,
    pub glow: Handle<StandardMaterial>,
    pub halo: Handle<StandardMaterial>,
    pub halo_warm: Handle<StandardMaterial>,
    pub mist: Handle<StandardMaterial>,
    pub aim: Handle<StandardMaterial>,
    pub sky: Handle<StandardMaterial>,
    pub rain: Handle<StandardMaterial>,
    pub bubble: Handle<StandardMaterial>,
    pub drop: Handle<StandardMaterial>,
    pub bolt: Handle<StandardMaterial>,
}

/// A soft round spot, for halos and mist.
fn soft_image(size: u32, noisy: bool) -> Image {
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let u = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
            let d = (u * u + v * v).sqrt();
            let mut a = (1.0 - d).clamp(0.0, 1.0);
            a = a * a * (3.0 - 2.0 * a);
            if noisy {
                let n = crate::util::fbm2(u * 2.5 + 7.0, v * 2.5 + 3.0, 4, 11);
                a *= (n * 1.6 - 0.35).clamp(0.0, 1.0);
            }
            data.extend_from_slice(&[255, 255, 255, (a * 255.0) as u8]);
        }
    }
    let mut img = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = ImageSampler::linear();
    img
}

pub fn setup_mats(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let soft = images.add(soft_image(64, false));
    let cloud = images.add(soft_image(128, true));
    commands.insert_resource(Mats {
        matte: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.88,
            reflectance: 0.25,
            ..default()
        }),
        wet: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.32,
            reflectance: 0.55,
            ..default()
        }),
        cloth: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.95,
            reflectance: 0.15,
            cull_mode: None,
            double_sided: true,
            ..default()
        }),
        glow: materials.add(StandardMaterial {
            base_color: Color::linear_rgb(4.0, 4.0, 4.0),
            unlit: true,
            ..default()
        }),
        halo: materials.add(StandardMaterial {
            base_color: Color::linear_rgba(0.45, 0.5, 0.55, 1.0),
            base_color_texture: Some(soft.clone()),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            cull_mode: None,
            fog_enabled: false,
            ..default()
        }),
        halo_warm: materials.add(StandardMaterial {
            base_color: Color::linear_rgba(0.9, 0.42, 0.12, 1.0),
            base_color_texture: Some(soft.clone()),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            cull_mode: None,
            fog_enabled: false,
            ..default()
        }),
        mist: materials.add(StandardMaterial {
            base_color: Color::srgba(0.55, 0.66, 0.66, 0.09),
            base_color_texture: Some(cloud),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        }),
        aim: materials.add(StandardMaterial {
            base_color: Color::linear_rgba(3.0, 0.2, 0.1, 0.35),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            cull_mode: None,
            ..default()
        }),
        sky: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            fog_enabled: false,
            ..default()
        }),
        rain: materials.add(StandardMaterial {
            base_color: Color::srgba(0.7, 0.8, 0.9, 0.22),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        }),
        bubble: materials.add(StandardMaterial {
            base_color: Color::srgba(0.75, 0.95, 0.9, 0.35),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        drop: materials.add(StandardMaterial {
            base_color: Color::srgba(0.7, 0.85, 0.85, 0.7),
            perceptual_roughness: 0.1,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        bolt: materials.add(StandardMaterial {
            base_color: Color::linear_rgb(6.0, 7.0, 12.0),
            unlit: true,
            fog_enabled: false,
            ..default()
        }),
    });
}

/// Meshes for one hunter's look.
pub struct HunterMeshes {
    pub torso: Handle<Mesh>,
    pub head: Handle<Mesh>,
    pub arm: Handle<Mesh>,
    pub leg: Handle<Mesh>,
}

#[derive(Resource)]
pub struct Models {
    // The bogwight.
    pub body: Handle<Mesh>,
    pub head: Handle<Mesh>,
    pub eyes: Handle<Mesh>,
    pub arm: Handle<Mesh>,
    pub leg: Handle<Mesh>,
    pub tail: [Handle<Mesh>; 3],
    // Hunters.
    pub hunters: Vec<HunterMeshes>,
    pub warden: HunterMeshes,
    pub crossbow: Handle<Mesh>,
    pub lantern: Handle<Mesh>,
    pub lantern_glass: Handle<Mesh>,
    pub bolt: Handle<Mesh>,
    // Things.
    pub boat: Handle<Mesh>,
    pub oar: Handle<Mesh>,
    pub barrel: Handle<Mesh>,
    pub crate_: Handle<Mesh>,
    pub log: Handle<Mesh>,
    pub lily: [Handle<Mesh>; 2],
    pub trees: Vec<Handle<Mesh>>,
    pub reeds: Vec<Handle<Mesh>>,
    pub weeds: Vec<Handle<Mesh>>,
    pub rocks: Vec<Handle<Mesh>>,
    pub roots: Vec<Handle<Mesh>>,
    pub mushroom: Handle<Mesh>,
    pub mushroom_glow: Handle<Mesh>,
    pub fungus: Vec<Handle<Mesh>>,
    pub tent: Handle<Mesh>,
    pub campfire: Handle<Mesh>,
    pub flame: Handle<Mesh>,
    pub plank: Handle<Mesh>,
    pub post: Handle<Mesh>,
    pub dock_plank: Handle<Mesh>,
    pub lamp_post: Handle<Mesh>,
    pub rope: Handle<Mesh>,
    // Effects and sky.
    pub quad: Handle<Mesh>,
    pub ball: Handle<Mesh>,
    pub cube: Handle<Mesh>,
    pub moon: Handle<Mesh>,
    pub mound: Handle<Mesh>,
}

fn add(meshes: &mut Assets<Mesh>, b: &MeshBuilder, flat: bool) -> Handle<Mesh> {
    meshes.add(b.build(flat))
}

pub fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let mut rng = Rng::new(0x5a1d);
    let m = &mut meshes;
    let hunters = [
        (0x5a3d25, 0x2d2118, 0xd9a07a, 0),
        (0x4b5230, 0x1f1a14, 0x8d5a3b, 1),
        (0x6b2a22, 0x2a2016, 0xf0c8a0, 2),
        (0x3e4a52, 0x3a2a1a, 0xc58c64, 1),
        (0x6e5a3a, 0x221a12, 0xe6b48e, 0),
    ]
    .iter()
    .map(|&(coat, hat, skin, beard)| hunter_meshes(m, coat, hat, skin, beard, false))
    .collect();
    let warden = hunter_meshes(m, 0x7a1f1a, 0x141010, 0xd49a70, 2, true);
    let models = Models {
        body: add(m, &creature_body(), false),
        head: add(m, &creature_head(), false),
        eyes: add(m, &creature_eyes(), false),
        arm: add(m, &creature_arm(), false),
        leg: add(m, &creature_leg(), false),
        tail: [
            add(m, &creature_tail(0), false),
            add(m, &creature_tail(1), false),
            add(m, &creature_tail(2), false),
        ],
        hunters,
        warden,
        crossbow: add(m, &crossbow(), true),
        lantern: add(m, &lantern(), true),
        lantern_glass: add(m, &lantern_glass(), true),
        bolt: add(m, &bolt(), true),
        boat: add(m, &boat(), true),
        oar: add(m, &oar(), true),
        barrel: add(m, &barrel(), false),
        crate_: add(m, &crate_mesh(), true),
        log: add(m, &log(&mut rng), false),
        lily: [add(m, &lily(false), true), add(m, &lily(true), true)],
        trees: (0..4).map(|k| add(m, &tree(&mut Rng::new(900 + k)), true)).collect(),
        reeds: (0..3).map(|k| add(m, &reeds(&mut Rng::new(40 + k)), true)).collect(),
        weeds: (0..3).map(|k| add(m, &weeds(&mut Rng::new(70 + k)), true)).collect(),
        rocks: (0..3).map(|k| add(m, &rock(&mut Rng::new(20 + k)), true)).collect(),
        roots: (0..3).map(|k| add(m, &roots(&mut Rng::new(60 + k)), false)).collect(),
        mushroom: add(m, &mushroom(), false),
        mushroom_glow: add(m, &mushroom_glow(), true),
        fungus: (0..2).map(|k| add(m, &fungus(&mut Rng::new(5 + k), k), true)).collect(),
        tent: add(m, &tent(), true),
        campfire: add(m, &campfire(&mut rng), true),
        flame: add(m, &flame(), true),
        plank: add(m, &bridge_plank(), true),
        post: add(m, &post(), true),
        dock_plank: add(m, &dock_plank(), true),
        lamp_post: add(m, &lamp_post(), true),
        rope: add(m, &rope(), false),
        // A textured quad needs UVs, which the builder doesn't make.
        quad: m.add(Rectangle::new(1.0, 1.0)),
        ball: add(m, &ball(), false),
        cube: add(m, &cube(), true),
        moon: add(m, &moon(), false),
        mound: add(m, &mound(&mut rng), true),
    };
    commands.insert_resource(models);
}

// ---------------------------------------------------------------------------
// The bogwight
// ---------------------------------------------------------------------------

const SKIN: u32 = 0x2c4a2b;
const SKIN_DARK: u32 = 0x1b2e1c;
const BELLY: u32 = 0x8b9457;
const FIN: u32 = 0x7d3a26;
const BONE: u32 = 0xd8cfae;

fn skin_color(d: Vec3, p: Vec3) -> Col {
    let back = lin(SKIN);
    let belly = lin(BELLY);
    let mut c = mix(belly, back, smoothstep(-0.55, 0.2, d.y));
    // Blotches.
    let n = crate::util::noise2(p.x * 9.0 + 3.0, p.z * 9.0 + p.y * 7.0, 5);
    if n > 0.62 && d.y > -0.2 {
        c = mix(c, lin(SKIN_DARK), 0.7);
    }
    c
}

fn creature_body() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let radii = Vec3::new(0.47, 0.3, 0.27);
    b.ico_with(Vec3::ZERO, radii, 2, |d| {
        // Fuller at the shoulders, tapering to the tail.
        let k = 1.0 + 0.1 * d.x - 0.08 * (d.x * 3.0).sin().abs() * (d.x < 0.0) as i32 as f32;
        (k, skin_color(d, d * radii))
    });
    // A ridge of fins down the back.
    for i in 0..6 {
        let x = 0.3 - i as f32 * 0.13;
        let base_y = 0.29 * (1.0 - (x / 0.5).powi(2)).max(0.0).sqrt() - 0.02;
        let h = 0.16 - (i as f32 - 1.5).abs() * 0.022;
        let c = lin(FIN);
        let a = Vec3::new(x + 0.07, base_y, 0.0);
        let tip = Vec3::new(x - 0.07, base_y + h, 0.0);
        let bb = Vec3::new(x - 0.1, base_y, 0.0);
        b.tri_p([a, tip, bb], c);
        b.tri_p([bb, tip, a], shade(c, 0.8));
    }
    b
}

fn creature_head() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    // Skull: wide and flat, like a catfish crossed with a toad.
    let r = Vec3::new(0.25, 0.15, 0.22);
    b.ico_with(Vec3::new(0.17, 0.03, 0.0), r, 2, |d| (1.0, skin_color(d, d * r)));
    // Jaw.
    let jr = Vec3::new(0.22, 0.07, 0.19);
    b.ico_with(Vec3::new(0.15, -0.08, 0.0), jr, 1, |d| {
        (1.0, mix(lin(BELLY), lin(SKIN), smoothstep(-0.2, 0.8, d.y)))
    });
    // Brow bulges for the eyes.
    for z in [-0.12, 0.12] {
        b.ball(Vec3::new(0.24, 0.13, z), 0.07, 1, lin(SKIN_DARK));
    }
    // Teeth along the mouth.
    for i in 0..5 {
        let x = 0.2 + i as f32 * 0.045;
        for z in [-1.0f32, 1.0] {
            let zz = z * (0.17 - i as f32 * 0.02);
            b.cone(
                Vec3::new(x, -0.03, zz),
                Vec3::new(x + 0.01, -0.085, zz),
                0.014,
                3,
                lin(BONE),
            );
        }
    }
    // Barbels trailing from the chin.
    for z in [-0.1, 0.1] {
        b.tube(
            &[
                (Vec3::new(0.3, -0.1, z), 0.018),
                (Vec3::new(0.26, -0.22, z * 1.2), 0.012),
                (Vec3::new(0.16, -0.3, z * 1.3), 0.004),
            ],
            4,
            lin(SKIN_DARK),
            false,
        );
    }
    b
}

fn creature_eyes() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    for z in [-0.13, 0.13] {
        b.ball(Vec3::new(0.28, 0.15, z), 0.042, 1, lin(0xd6ff5a));
    }
    b
}

fn creature_arm() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let c = lin(SKIN);
    let elbow = Vec3::new(0.05, -0.27, 0.0);
    let wrist = Vec3::new(0.22, -0.45, 0.0);
    b.cyl(Vec3::ZERO, elbow, 0.075, 0.058, 6, c);
    b.ball(elbow, 0.058, 0, c);
    b.cyl(elbow, wrist, 0.058, 0.045, 6, shade(c, 0.9));
    b.ico(
        wrist + Vec3::new(0.03, -0.02, 0.0),
        Vec3::new(0.07, 0.05, 0.06),
        1,
        lin(SKIN_DARK),
    );
    for (k, z) in [-0.035f32, 0.0, 0.035].iter().enumerate() {
        let base = wrist + Vec3::new(0.07, -0.03, *z);
        let tip = base + Vec3::new(0.1, -0.08 - k as f32 * 0.01, z * 0.5);
        b.cone(base, tip, 0.016, 4, lin(BONE));
    }
    b
}

fn creature_leg() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let c = lin(SKIN);
    let knee = Vec3::new(0.12, -0.18, 0.0);
    let ankle = Vec3::new(-0.02, -0.33, 0.0);
    b.cyl(Vec3::ZERO, knee, 0.1, 0.07, 6, c);
    b.ball(knee, 0.07, 0, c);
    b.cyl(knee, ankle, 0.065, 0.045, 6, shade(c, 0.85));
    // A webbed foot.
    let d = lin(SKIN_DARK);
    b.ico(ankle + Vec3::new(0.08, -0.03, 0.0), Vec3::new(0.14, 0.03, 0.09), 1, d);
    for z in [-0.06f32, 0.0, 0.06] {
        b.cone(
            ankle + Vec3::new(0.18, -0.03, z),
            ankle + Vec3::new(0.27, -0.045, z * 1.4),
            0.012,
            3,
            lin(BONE),
        );
    }
    b
}

/// Tail segment `k`: 0 at the hips, 2 is the paddle fin.
fn creature_tail(k: usize) -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let (len, r0, r1) = [(0.32, 0.17, 0.11), (0.3, 0.11, 0.065), (0.24, 0.065, 0.02)][k];
    let c = lin(SKIN);
    b.tube(
        &[
            (Vec3::ZERO, r0),
            (Vec3::new(-len * 0.5, 0.0, 0.0), (r0 + r1) * 0.5),
            (Vec3::new(-len, 0.0, 0.0), r1),
        ],
        7,
        c,
        k == 2,
    );
    if k == 0 {
        b.ball(Vec3::ZERO, r0, 1, c);
    } else {
        b.ball(Vec3::ZERO, r0 * 0.95, 1, c);
    }
    // A membrane above and below, newt-style.
    let f = lin(FIN);
    let (h0, h1) = [(0.05, 0.1), (0.1, 0.16), (0.16, 0.2)][k];
    for s in [1.0f32, -0.7] {
        let p = [
            Vec3::new(0.0, s * r0 * 0.6, 0.0),
            Vec3::new(0.0, s * (r0 * 0.6 + h0), 0.0),
            Vec3::new(-len, s * (r1 * 0.6 + h1), 0.0),
            Vec3::new(-len, s * r1 * 0.6, 0.0),
        ];
        b.quad_2(p, if s > 0.0 { f } else { shade(f, 0.8) });
    }
    b
}

// ---------------------------------------------------------------------------
// Hunters
// ---------------------------------------------------------------------------

fn hunter_meshes(m: &mut Assets<Mesh>, coat: u32, hat: u32, skin: u32, beard: u32, warden: bool) -> HunterMeshes {
    HunterMeshes {
        torso: add(m, &hunter_torso(coat, warden), true),
        head: add(m, &hunter_head(hat, skin, beard, warden), true),
        arm: add(m, &hunter_arm(coat, skin), true),
        leg: add(m, &hunter_leg(), true),
    }
}

fn hunter_torso(coat: u32, warden: bool) -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let c = lin(coat);
    let bottom = if warden { -0.42 } else { -0.3 };
    // A long coat, flared at the hem.
    let rings: Vec<Vec<(Vec3, Col)>> = [
        (bottom, 0.25, 0.8),
        (-0.05, 0.2, 0.95),
        (0.2, 0.2, 1.0),
        (0.38, 0.17, 1.05),
        (0.46, 0.09, 0.9),
    ]
    .iter()
    .map(|&(y, r, k)| MeshBuilder::ring(Vec3::new(0.0, y, 0.0), Vec3::Y, Vec3::X, r, r * 0.85, 8, shade(c, k)))
    .collect();
    b.loft(&rings, true, true);
    // Belt and buckle.
    let belt = MeshBuilder::ring(
        Vec3::new(0.0, -0.02, 0.0),
        Vec3::Y,
        Vec3::X,
        0.205,
        0.18,
        8,
        lin(0x1c140e),
    );
    let belt2 = MeshBuilder::ring(
        Vec3::new(0.0, 0.04, 0.0),
        Vec3::Y,
        Vec3::X,
        0.205,
        0.18,
        8,
        lin(0x1c140e),
    );
    b.loft(&[belt, belt2], false, false);
    b.boxy(Vec3::new(0.2, 0.01, 0.0), Vec3::new(0.012, 0.03, 0.035), lin(0xc9a44a));
    // A bandolier of trophies and a pouch.
    b.beam(
        Vec3::new(-0.15, 0.4, 0.0),
        Vec3::new(0.16, -0.02, 0.0),
        0.02,
        lin(0x2a1d13),
    );
    b.boxy(
        Vec3::new(-0.12, -0.08, 0.17),
        Vec3::new(0.07, 0.07, 0.04),
        lin(0x3b2a1a),
    );
    if warden {
        // Epaulettes and a fur collar.
        b.ico(Vec3::new(0.0, 0.44, 0.0), Vec3::new(0.2, 0.06, 0.2), 1, lin(0x4a3a2c));
    }
    b
}

fn hunter_head(hat: u32, skin: u32, beard: u32, warden: bool) -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let s = lin(skin);
    b.ico(Vec3::new(0.02, 0.62, 0.0), Vec3::new(0.12, 0.14, 0.12), 1, s);
    b.cone(
        Vec3::new(0.12, 0.62, 0.0),
        Vec3::new(0.17, 0.6, 0.0),
        0.03,
        4,
        shade(s, 0.9),
    );
    for z in [-0.05, 0.05] {
        b.ball(Vec3::new(0.12, 0.65, z), 0.015, 0, lin(0x101010));
    }
    match beard {
        1 => b.ico(Vec3::new(0.07, 0.53, 0.0), Vec3::new(0.08, 0.08, 0.1), 1, lin(0x3a2616)),
        2 => b.ico(
            Vec3::new(0.08, 0.55, 0.0),
            Vec3::new(0.06, 0.05, 0.09),
            1,
            lin(0x6a6660),
        ),
        _ => {}
    }
    let h = lin(hat);
    let brim = if warden { 0.3 } else { 0.24 };
    b.cyl(
        Vec3::new(0.0, 0.71, 0.0),
        Vec3::new(0.0, 0.73, 0.0),
        brim,
        brim,
        10,
        shade(h, 0.9),
    );
    let crown = if warden { 0.26 } else { 0.15 };
    b.cyl(
        Vec3::new(0.0, 0.72, 0.0),
        Vec3::new(0.0, 0.72 + crown, 0.0),
        0.14,
        0.12,
        8,
        h,
    );
    b.cyl(
        Vec3::new(0.0, 0.73, 0.0),
        Vec3::new(0.0, 0.77, 0.0),
        0.143,
        0.14,
        8,
        lin(0x7a2a1c),
    );
    if warden {
        // A long feather.
        b.tube(
            &[
                (Vec3::new(-0.08, 0.76, 0.1), 0.03),
                (Vec3::new(-0.25, 0.95, 0.12), 0.04),
                (Vec3::new(-0.4, 1.02, 0.12), 0.0),
            ],
            4,
            lin(0xd8d0c0),
            false,
        );
    }
    b
}

fn hunter_arm(coat: u32, skin: u32) -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let c = lin(coat);
    b.cyl(Vec3::ZERO, Vec3::new(0.0, -0.48, 0.0), 0.068, 0.055, 6, shade(c, 0.9));
    b.ball(Vec3::new(0.0, -0.53, 0.0), 0.055, 1, lin(skin));
    b
}

fn hunter_leg() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    b.cyl(Vec3::ZERO, Vec3::new(0.0, -0.58, 0.0), 0.085, 0.07, 6, lin(0x3a3228));
    b.boxy(Vec3::new(0.04, -0.64, 0.0), Vec3::new(0.11, 0.06, 0.065), lin(0x1e1712));
    b
}

/// Held at the hand, pointing along +X.
fn crossbow() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let wood = lin(0x5b3b22);
    b.boxy(Vec3::new(0.12, 0.0, 0.0), Vec3::new(0.28, 0.035, 0.03), wood);
    b.boxy(
        Vec3::new(-0.12, -0.05, 0.0),
        Vec3::new(0.06, 0.07, 0.028),
        shade(wood, 0.8),
    );
    // The prod, bent forward, and its string.
    let steel = lin(0x7c7c80);
    b.tube(
        &[
            (Vec3::new(0.3, 0.26, 0.0), 0.012),
            (Vec3::new(0.39, 0.1, 0.0), 0.018),
            (Vec3::new(0.4, 0.0, 0.0), 0.02),
            (Vec3::new(0.39, -0.1, 0.0), 0.018),
            (Vec3::new(0.3, -0.26, 0.0), 0.012),
        ],
        5,
        steel,
        true,
    );
    b.beam(
        Vec3::new(0.3, 0.26, 0.0),
        Vec3::new(0.14, 0.0, 0.0),
        0.004,
        lin(0xc8c0a8),
    );
    b.beam(
        Vec3::new(0.3, -0.26, 0.0),
        Vec3::new(0.14, 0.0, 0.0),
        0.004,
        lin(0xc8c0a8),
    );
    b
}

/// Hangs from its handle at the origin.
fn lantern() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let iron = lin(0x2a2622);
    b.annulus(Vec3::new(0.0, -0.04, 0.0), Vec3::Z, 0.035, 0.05, 8, iron);
    b.cyl(
        Vec3::new(0.0, -0.09, 0.0),
        Vec3::new(0.0, -0.13, 0.0),
        0.02,
        0.08,
        6,
        iron,
    );
    b.boxy(Vec3::new(0.0, -0.15, 0.0), Vec3::new(0.085, 0.02, 0.085), iron);
    b.boxy(Vec3::new(0.0, -0.4, 0.0), Vec3::new(0.09, 0.025, 0.09), iron);
    for (x, z) in [(-0.08, -0.08), (0.08, -0.08), (0.08, 0.08), (-0.08, 0.08)] {
        b.boxy(Vec3::new(x, -0.275, z), Vec3::new(0.012, 0.11, 0.012), iron);
    }
    b
}

fn lantern_glass() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    b.boxy(Vec3::new(0.0, -0.275, 0.0), Vec3::new(0.07, 0.1, 0.07), lin(0xffb656));
    b
}

fn bolt() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    b.boxy(Vec3::ZERO, Vec3::new(0.25, 0.01, 0.01), lin(0x6a4a2a));
    b.cone(
        Vec3::new(0.24, 0.0, 0.0),
        Vec3::new(0.32, 0.0, 0.0),
        0.022,
        4,
        lin(0x9a9aa0),
    );
    b.quad_2(
        [
            Vec3::new(-0.25, 0.0, 0.0),
            Vec3::new(-0.15, 0.0, 0.0),
            Vec3::new(-0.2, 0.05, 0.0),
            Vec3::new(-0.27, 0.05, 0.0),
        ],
        lin(0xb03020),
    );
    b
}

// ---------------------------------------------------------------------------
// Things
// ---------------------------------------------------------------------------

/// A flat-bottomed skiff about 4.3 m long; the origin is its center of
/// mass, the floor is at y = -0.27.
fn boat() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let wood = lin(0x6a4a2e);
    let dark = lin(0x2e2014);
    // Side profile, counter-clockwise from the stern bottom.
    let profile = [
        Vec2::new(-1.75, -0.36),
        Vec2::new(1.65, -0.36),
        Vec2::new(2.25, 0.34),
        Vec2::new(-2.1, 0.3),
    ];
    let zf = 0.66;
    let zb = -0.66;
    // Near and far sides with plank bands.
    for (z, facing) in [(zf, 1.0f32), (zb, -1.0)] {
        for band in 0..4 {
            let t0 = band as f32 / 4.0;
            let t1 = (band + 1) as f32 / 4.0;
            let lerp2 = |a: Vec2, c: Vec2, t: f32| a + (c - a) * t;
            let l0 = lerp2(profile[0], profile[3], t0);
            let l1 = lerp2(profile[0], profile[3], t1);
            let r0 = lerp2(profile[1], profile[2], t0);
            let r1 = lerp2(profile[1], profile[2], t1);
            let c = shade(wood, 0.8 + 0.12 * ((band * 7) % 3) as f32);
            let p = [
                Vec3::new(l0.x, l0.y, z),
                Vec3::new(r0.x, r0.y, z),
                Vec3::new(r1.x, r1.y, z),
                Vec3::new(l1.x, l1.y, z),
            ];
            if facing > 0.0 {
                b.quad_p(p, c);
            } else {
                b.quad_p([p[1], p[0], p[3], p[2]], c);
            }
            // Seams.
            b.beam(
                Vec3::new(l0.x, l0.y, z + facing * 0.006),
                Vec3::new(r0.x, r0.y, z + facing * 0.006),
                0.008,
                dark,
            );
        }
        // Gunwale.
        b.beam(
            Vec3::new(profile[3].x, profile[3].y, z),
            Vec3::new(profile[2].x, profile[2].y, z),
            0.04,
            lin(0x4a3220),
        );
    }
    // Floor, bottom, bow and stern boards.
    b.boxy(Vec3::new(-0.05, -0.31, 0.0), Vec3::new(1.7, 0.05, zf), shade(wood, 0.7));
    let bow = [
        Vec3::new(profile[1].x, profile[1].y, zf),
        Vec3::new(profile[1].x, profile[1].y, zb),
        Vec3::new(profile[2].x, profile[2].y, zb),
        Vec3::new(profile[2].x, profile[2].y, zf),
    ];
    b.quad_2(bow, shade(wood, 0.75));
    let stern = [
        Vec3::new(profile[0].x, profile[0].y, zb),
        Vec3::new(profile[0].x, profile[0].y, zf),
        Vec3::new(profile[3].x, profile[3].y, zf),
        Vec3::new(profile[3].x, profile[3].y, zb),
    ];
    b.quad_2(stern, shade(wood, 0.75));
    // Ribs inside the far side and a bench.
    for i in 0..5 {
        let x = -1.4 + i as f32 * 0.7;
        b.beam(
            Vec3::new(x, -0.28, zb + 0.04),
            Vec3::new(x + 0.08, 0.3, zb + 0.04),
            0.025,
            dark,
        );
    }
    b.boxy(
        Vec3::new(-1.2, 0.02, 0.0),
        Vec3::new(0.18, 0.03, zf - 0.02),
        shade(wood, 0.9),
    );
    b
}

fn oar() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let wood = lin(0x7a5a38);
    b.cyl(
        Vec3::new(0.0, 0.5, 0.0),
        Vec3::new(0.0, -1.3, 0.0),
        0.025,
        0.025,
        5,
        wood,
    );
    b.boxy(
        Vec3::new(0.0, -1.45, 0.0),
        Vec3::new(0.09, 0.22, 0.015),
        shade(wood, 0.85),
    );
    b
}

fn barrel() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let wood = lin(0x6e4a2a);
    let hoop = lin(0x2a2a2c);
    let prof: Vec<(f32, f32, Col)> = (0..=8)
        .map(|i| {
            let t = i as f32 / 8.0;
            let y = -0.4 + t * 0.8;
            let r = 0.29 + 0.05 * (t * PI).sin();
            let c = if i == 1 || i == 7 {
                hoop
            } else {
                shade(wood, 0.85 + 0.15 * (t * 9.0).sin().abs())
            };
            (r, y, c)
        })
        .collect();
    b.lathe(&prof, 12);
    b
}

fn crate_mesh() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let wood = lin(0x8a6a40);
    let dark = lin(0x4a3620);
    b.boxy(Vec3::ZERO, Vec3::splat(0.39), wood);
    for z in [-0.4, 0.4] {
        b.beam(Vec3::new(-0.37, -0.37, z), Vec3::new(0.37, 0.37, z), 0.035, dark);
        for (a, c) in [
            (Vec3::new(-0.38, -0.38, z), Vec3::new(0.38, -0.38, z)),
            (Vec3::new(-0.38, 0.38, z), Vec3::new(0.38, 0.38, z)),
            (Vec3::new(-0.38, -0.38, z), Vec3::new(-0.38, 0.38, z)),
            (Vec3::new(0.38, -0.38, z), Vec3::new(0.38, 0.38, z)),
        ] {
            b.beam(a, c, 0.04, dark);
        }
    }
    b
}

fn log(rng: &mut Rng) -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let bark = lin(0x3a2c20);
    let path: Vec<(Vec3, f32)> = (0..=6)
        .map(|i| {
            let t = i as f32 / 6.0;
            (
                Vec3::new(-1.75 + t * 3.5, rng.sym(0.03), rng.sym(0.03)),
                0.28 + rng.sym(0.03),
            )
        })
        .collect();
    b.tube(&path, 9, bark, true);
    // Pale cut ends and a broken branch.
    b.disc(Vec3::new(1.76, 0.0, 0.0), Vec3::X, 0.25, 9, lin(0x9a8060));
    b.disc(Vec3::new(-1.76, 0.0, 0.0), -Vec3::X, 0.25, 9, lin(0x9a8060));
    b.cone(Vec3::new(0.4, 0.2, 0.0), Vec3::new(0.7, 0.62, 0.1), 0.07, 5, bark);
    // Moss on top.
    b.ico(Vec3::new(-0.3, 0.22, 0.0), Vec3::new(0.7, 0.08, 0.2), 1, lin(0x3d5a2a));
    b
}

fn lily(flower: bool) -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let green = lin(0x3f6b2e);
    let sides = 14;
    let r = 0.55;
    let center = b.v(Vec3::new(0.0, 0.03, 0.0), shade(green, 1.1));
    let ring: Vec<u32> = (0..=sides)
        .map(|k| {
            // Leave a notch.
            let t = 0.25 + k as f32 / sides as f32 * (TAU - 0.5);
            b.v(Vec3::new(t.cos() * r, 0.0, t.sin() * r), shade(green, 0.9))
        })
        .collect();
    for k in 0..sides {
        b.tri(center, ring[k + 1], ring[k]);
    }
    if flower {
        for k in 0..6 {
            let t = k as f32 / 6.0 * TAU;
            let tip = Vec3::new(t.cos() * 0.13, 0.14, t.sin() * 0.13);
            b.cone(
                Vec3::new(0.05, 0.04, 0.0),
                tip + Vec3::new(0.05, 0.0, 0.0),
                0.05,
                3,
                lin(0xe8d0e0),
            );
        }
        b.ball(Vec3::new(0.05, 0.08, 0.0), 0.04, 0, lin(0xf0c040));
    }
    b
}

/// A bald cypress with a flared base, knees and hanging moss.
fn tree(rng: &mut Rng) -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let bark = lin(0x3b3128);
    let h = rng.range(8.0, 11.0);
    let prof: Vec<(f32, f32, Col)> = [
        (1.0, -4.0),
        (1.0, 0.0),
        (0.62, 0.5),
        (0.4, 1.4),
        (0.3, 3.0),
        (0.22, h * 0.7),
        (0.08, h),
    ]
    .iter()
    .enumerate()
    .map(|(i, &(r, y))| (r, y, shade(bark, 0.7 + 0.08 * i as f32)))
    .collect();
    b.lathe(&prof, 9);
    // Knees poking up around the base.
    for _ in 0..4 {
        let a = rng.angle();
        let d = rng.range(1.1, 1.8);
        let p = Vec3::new(a.cos() * d, -0.2, a.sin() * d);
        b.cone(
            p,
            p + Vec3::new(0.0, rng.range(0.3, 0.6), 0.0),
            0.14,
            5,
            shade(bark, 0.8),
        );
    }
    // Branches, foliage clumps and moss.
    let foliage = lin(0x243a22);
    let moss = lin(0x6f7a62);
    let branches = rng.below(3) + 4;
    for i in 0..branches {
        let y = h * (0.45 + 0.5 * i as f32 / branches as f32);
        let a = rng.angle();
        let len = rng.range(1.2, 2.6) * (1.0 - 0.3 * i as f32 / branches as f32);
        let dir = Vec3::new(a.cos(), rng.range(0.15, 0.5), a.sin() * 0.6).normalize();
        let start = Vec3::new(0.0, y, 0.0);
        let end = start + dir * len;
        b.cyl(start, end, 0.12, 0.05, 5, bark);
        b.rock(end + Vec3::Y * 0.2, Vec3::new(len * 0.7, 0.45, len * 0.5), foliage, rng);
        for _ in 0..3 {
            let t = rng.range(0.3, 1.0);
            let p = start + (end - start) * t;
            let l = rng.range(0.8, 2.0);
            let w = rng.range(0.08, 0.16);
            b.quad_2(
                [
                    p + Vec3::new(-w, 0.0, 0.0),
                    p + Vec3::new(w, 0.0, 0.0),
                    p + Vec3::new(w * 0.3, -l, 0.0),
                    p + Vec3::new(-w * 0.3, -l, 0.0),
                ],
                shade(moss, rng.range(0.7, 1.0)),
            );
        }
    }
    b.rock(Vec3::new(0.0, h, 0.0), Vec3::new(1.3, 0.8, 1.0), foliage, rng);
    b
}

fn reeds(rng: &mut Rng) -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let base = lin(0x33401f);
    let tip = lin(0x8a8a4a);
    for _ in 0..rng.below(5) + 7 {
        let x = rng.sym(0.35);
        let z = rng.sym(0.2);
        let h = rng.range(1.2, 2.3);
        let lean = rng.sym(0.35);
        let w = rng.range(0.025, 0.045);
        let p0 = Vec3::new(x, -0.3, z);
        let p1 = Vec3::new(x + lean * 0.4, h * 0.55, z);
        let p2 = Vec3::new(x + lean, h, z);
        let c1 = mix(base, tip, 0.5);
        b.quad_2(
            [
                p0 + Vec3::X * -w,
                p0 + Vec3::X * w,
                p1 + Vec3::X * w * 0.7,
                p1 + Vec3::X * -w * 0.7,
            ],
            base,
        );
        b.quad_2(
            [
                p1 + Vec3::X * -w * 0.7,
                p1 + Vec3::X * w * 0.7,
                p2,
                p2 + Vec3::X * -0.005,
            ],
            c1,
        );
        if rng.chance(0.35) {
            let top = p1 + (p2 - p1) * 0.75;
            b.cyl(top, top + (p2 - p1).normalize() * 0.22, 0.04, 0.04, 5, lin(0x4a2e1a));
        } else {
            let _ = tip;
        }
    }
    b
}

fn weeds(rng: &mut Rng) -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let c = lin(0x2a3a1e);
    for _ in 0..rng.below(3) + 3 {
        let x = rng.sym(0.3);
        let h = rng.range(1.2, 3.0);
        let segs = 6;
        let mut prev = Vec3::new(x, -0.2, rng.sym(0.2));
        let w = rng.range(0.06, 0.12);
        let phase = rng.angle();
        for s in 1..=segs {
            let t = s as f32 / segs as f32;
            let p = Vec3::new(x + (t * 5.0 + phase).sin() * 0.15, -0.2 + t * h, prev.z);
            let ww = w * (1.0 - t * 0.7);
            b.quad_2(
                [
                    prev - Vec3::X * ww,
                    prev + Vec3::X * ww,
                    p + Vec3::X * ww * 0.8,
                    p - Vec3::X * ww * 0.8,
                ],
                shade(c, 0.7 + 0.5 * t),
            );
            prev = p;
        }
    }
    b
}

fn rock(rng: &mut Rng) -> MeshBuilder {
    let mut b = MeshBuilder::new();
    b.rock(Vec3::ZERO, Vec3::new(1.0, 0.8, 0.9), lin(0x4a4c44), rng);
    b.ico(Vec3::new(0.0, 0.62, 0.0), Vec3::new(0.7, 0.18, 0.6), 1, lin(0x35502a));
    b
}

/// Roots dangling down a bank's face from the origin.
fn roots(rng: &mut Rng) -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let c = lin(0x2a1e14);
    for _ in 0..rng.below(3) + 3 {
        let mut p = Vec3::new(rng.sym(0.4), 0.0, 0.0);
        let len = rng.range(0.5, 1.6);
        let mut path = vec![(p, rng.range(0.03, 0.06))];
        let steps = 5;
        for k in 1..=steps {
            p += Vec3::new(rng.sym(0.12), -len / steps as f32, rng.range(0.0, 0.03));
            path.push((p, 0.05 * (1.0 - k as f32 / steps as f32) + 0.006));
        }
        b.tube(&path, 4, shade(c, rng.range(0.8, 1.2)), false);
    }
    b
}

/// A springy toadstool: the cap's top is at y = 1.1.
fn mushroom() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let stalk = lin(0xcfc4a8);
    b.cyl(
        Vec3::new(0.0, -0.2, 0.0),
        Vec3::new(0.0, 0.85, 0.0),
        0.16,
        0.11,
        8,
        stalk,
    );
    let cap = lin(0x5a2a6a);
    let prof: Vec<(f32, f32, Col)> = vec![
        (0.1, 0.78, lin(0x3a2a3a)),
        (0.62, 0.8, shade(cap, 0.7)),
        (0.66, 0.88, cap),
        (0.55, 1.0, cap),
        (0.3, 1.08, shade(cap, 1.1)),
        (0.02, 1.1, shade(cap, 1.15)),
    ];
    b.lathe(&prof, 14);
    b
}

fn mushroom_glow() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let mut rng = Rng::new(77);
    for _ in 0..9 {
        let a = rng.angle();
        let r = rng.range(0.1, 0.5);
        let y = 1.1 - r * r * 0.8;
        b.ball(
            Vec3::new(a.cos() * r, y + 0.01, a.sin() * r),
            rng.range(0.035, 0.06),
            0,
            lin(0x55ffd8),
        );
    }
    // A glowing rim under the cap.
    b.annulus(Vec3::new(0.0, 0.79, 0.0), -Vec3::Y, 0.2, 0.6, 14, lin(0x2aa890));
    b
}

fn fungus(rng: &mut Rng, k: u64) -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let c = if k == 0 { lin(0x4affd0) } else { lin(0xb07aff) };
    for _ in 0..rng.below(4) + 5 {
        let x = rng.sym(0.35);
        let z = rng.sym(0.3);
        let h = rng.range(0.08, 0.3);
        b.cyl(
            Vec3::new(x, -0.1, z),
            Vec3::new(x, h, z),
            0.012,
            0.012,
            3,
            shade(c, 0.4),
        );
        b.ico(Vec3::new(x, h, z), Vec3::new(0.06, 0.03, 0.06), 0, c);
    }
    b
}

fn tent() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let canvas = lin(0x6b5d45);
    let (w, h, d) = (1.3, 1.7, 1.1);
    b.quad_2(
        [
            Vec3::new(-w, 0.0, d),
            Vec3::new(0.0, h, d),
            Vec3::new(0.0, h, -d),
            Vec3::new(-w, 0.0, -d),
        ],
        shade(canvas, 0.85),
    );
    b.quad_2(
        [
            Vec3::new(0.0, h, d),
            Vec3::new(w, 0.0, d),
            Vec3::new(w, 0.0, -d),
            Vec3::new(0.0, h, -d),
        ],
        canvas,
    );
    // Back wall and an open flap at the front.
    b.tri_p(
        [Vec3::new(-w, 0.0, -d), Vec3::new(0.0, h, -d), Vec3::new(w, 0.0, -d)],
        shade(canvas, 0.6),
    );
    b.tri_p(
        [
            Vec3::new(-w, 0.0, d),
            Vec3::new(-0.2, 0.0, d + 0.3),
            Vec3::new(0.0, h, d),
        ],
        shade(canvas, 0.75),
    );
    let pole = lin(0x4a3620);
    b.beam(
        Vec3::new(0.0, 0.0, d + 0.02),
        Vec3::new(0.0, h + 0.2, d + 0.02),
        0.03,
        pole,
    );
    b.beam(Vec3::new(0.0, h, -d), Vec3::new(0.0, h, d), 0.025, pole);
    // Pelts drying on a line.
    b.quad_2(
        [
            Vec3::new(w + 0.2, 1.2, 0.2),
            Vec3::new(w + 0.7, 1.2, 0.2),
            Vec3::new(w + 0.65, 0.6, 0.2),
            Vec3::new(w + 0.25, 0.65, 0.2),
        ],
        lin(0x5a4030),
    );
    b.beam(
        Vec3::new(w, 1.25, 0.2),
        Vec3::new(w + 1.0, 1.25, 0.2),
        0.01,
        lin(0xa89878),
    );
    b
}

fn campfire(rng: &mut Rng) -> MeshBuilder {
    let mut b = MeshBuilder::new();
    for k in 0..8 {
        let a = k as f32 / 8.0 * TAU;
        b.rock(
            Vec3::new(a.cos() * 0.45, 0.05, a.sin() * 0.35),
            Vec3::new(0.14, 0.1, 0.12),
            lin(0x55524a),
            rng,
        );
    }
    let wood = lin(0x2a1c12);
    b.beam(Vec3::new(-0.35, 0.05, -0.2), Vec3::new(0.3, 0.12, 0.2), 0.05, wood);
    b.beam(Vec3::new(0.35, 0.05, -0.2), Vec3::new(-0.3, 0.12, 0.2), 0.05, wood);
    b.ico(Vec3::new(0.0, 0.05, 0.0), Vec3::new(0.25, 0.05, 0.2), 1, lin(0x6a2a10));
    b
}

fn flame() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    b.cone(Vec3::ZERO, Vec3::new(0.0, 0.7, 0.0), 0.22, 6, lin(0xff7a1a));
    b.cone(
        Vec3::new(0.08, 0.0, 0.05),
        Vec3::new(0.1, 0.45, 0.05),
        0.12,
        5,
        lin(0xffc040),
    );
    b.cone(
        Vec3::new(-0.08, 0.0, -0.05),
        Vec3::new(-0.1, 0.5, 0.0),
        0.12,
        5,
        lin(0xffa020),
    );
    b
}

/// Bridge plank, 0.9 m long, the rope ties at its ends.
fn bridge_plank() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    b.boxy(Vec3::ZERO, Vec3::new(0.43, 0.045, 0.55), lin(0x7a5a38));
    b.beam(
        Vec3::new(-0.43, 0.05, 0.5),
        Vec3::new(0.43, 0.05, 0.5),
        0.015,
        lin(0x3a2a1a),
    );
    b
}

/// A post one metre tall from y = 0 down (scaled to length).
fn post() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    b.cyl(Vec3::ZERO, Vec3::new(0.0, -1.0, 0.0), 0.09, 0.1, 6, lin(0x3a2c1e));
    b
}

/// One metre of dock planking, top at y = 0.
fn dock_plank() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let wood = lin(0x6a5034);
    for i in 0..4 {
        let x = -0.5 + (i as f32 + 0.5) * 0.25;
        b.boxy(
            Vec3::new(x, -0.06, -0.25),
            Vec3::new(0.115, 0.06, 1.05),
            shade(wood, 0.8 + 0.1 * (i % 3) as f32),
        );
    }
    b.boxy(Vec3::new(0.0, -0.18, 0.75), Vec3::new(0.5, 0.05, 0.05), lin(0x3a2c1e));
    b
}

/// Post with an arm at the top, 2.8 m tall; the arm tip is at (0.9, 2.7).
fn lamp_post() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let wood = lin(0x3e2e20);
    b.cyl(Vec3::new(0.0, -0.3, 0.0), Vec3::new(0.0, 2.8, 0.0), 0.08, 0.07, 6, wood);
    b.beam(Vec3::new(-0.05, 2.7, 0.0), Vec3::new(0.95, 2.7, 0.0), 0.04, wood);
    b.beam(Vec3::new(0.0, 2.3, 0.0), Vec3::new(0.45, 2.7, 0.0), 0.03, wood);
    // A trophy skull nailed to it.
    b.ico(Vec3::new(0.0, 1.9, 0.09), Vec3::new(0.09, 0.1, 0.07), 1, lin(0xcfc4a0));
    b
}

/// A unit rope along +Y from 0 to 1.
fn rope() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    b.cyl(Vec3::ZERO, Vec3::Y, 0.018, 0.018, 4, lin(0x8a7650));
    b
}

fn ball() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    b.ball(Vec3::ZERO, 0.5, 1, [1.0; 4]);
    b
}

fn cube() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    b.boxy(Vec3::ZERO, Vec3::splat(0.5), [1.0; 4]);
    b
}

fn moon() -> MeshBuilder {
    let mut b = MeshBuilder::new();
    let c = lin(0xe8f0e0);
    b.disc(Vec3::ZERO, Vec3::Z, 1.0, 32, c);
    // Maria.
    let mut rng = Rng::new(3);
    for _ in 0..6 {
        let p = rng.disc(0.7);
        b.disc(
            Vec3::new(p.x, p.y, 0.01),
            Vec3::Z,
            rng.range(0.1, 0.25),
            10,
            shade(c, 0.82),
        );
    }
    b
}

/// A low hummock of distant ground.
fn mound(rng: &mut Rng) -> MeshBuilder {
    let mut b = MeshBuilder::new();
    b.ico_with(Vec3::ZERO, Vec3::new(1.0, 0.25, 0.6), 2, |d| {
        (rng.range(0.9, 1.1), mix(lin(0x1a221a), lin(0x2a3524), d.y.max(0.0)))
    });
    b
}

/// A rotation about Z that turns +Y toward `dir`.
pub fn rot_to(dir: Vec2) -> Quat {
    Quat::from_rotation_z(dir.y.atan2(dir.x) - FRAC_PI_2)
}
