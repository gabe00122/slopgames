//! The building: rotunda, dome, galleries, vestibule and their lighting.

use crate::{
    content::{self, Gallery},
    kit::{Kit, Palette, at, matte},
    layout::*,
    meshes::{self, MeshData},
    placards::{Cell, PlacardAtlas},
    textures,
};
use bevy::{
    light::{FogVolume, NotShadowCaster, VolumetricLight},
    prelude::*,
};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Where the sunbeam through the oculus comes from (pointing toward the sun).
pub fn sun_dir() -> Vec3 {
    Vec3::new(0.18, 1.0, 0.3).normalize()
}

pub const OCULUS_R: f32 = 2.6;

/// Height of the oculus ring above the floor.
pub fn oculus_height() -> f32 {
    ROT_WALL_H + (ROT_APOTHEM * ROT_APOTHEM - OCULUS_R * OCULUS_R).sqrt()
}

#[derive(Resource)]
pub struct Sky {
    pub cubemap: Handle<Image>,
    pub interior: Handle<Image>,
}

/// Lights whose shadow maps are switched on only near the visitor.
#[derive(Component)]
pub struct ShadowLod;

pub fn build(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    pal: Res<Palette>,
    atlas: Res<PlacardAtlas>,
) {
    let galleries = content::galleries();
    let mut walls = Walls::build(&galleries);
    commands.insert_resource(Sky {
        cubemap: images.add(textures::sky(sun_dir())),
        interior: images.add(textures::interior()),
    });

    let mut kit = Kit {
        cmd: &mut commands,
        meshes: &mut meshes,
        mats: &mut mats,
        pal: &pal,
        cache: Default::default(),
    };
    let world = kit
        .cmd
        .spawn((Transform::IDENTITY, Visibility::default(), Name::new("Museum")))
        .id();

    rotunda(&mut kit, world, &mut images, &atlas, &mut walls);
    for (i, g) in galleries.iter().enumerate() {
        gallery(&mut kit, world, g, i, &atlas, &mut walls);
    }
    vestibule(&mut kit, world, &atlas);
    commands.insert_resource(walls);
}

fn plaster_mat(kit: &mut Kit, color: Color, rough: f32) -> Handle<StandardMaterial> {
    let (tex, normal) = (kit.pal.plaster.clone(), kit.pal.plaster_normal.clone());
    kit.mat(StandardMaterial {
        base_color: color,
        base_color_texture: Some(tex),
        normal_map_texture: Some(normal),
        perceptual_roughness: rough,
        ..default()
    })
}

/// A box in a side's local frame, with tiled UVs.
fn wall_box(
    kit: &mut Kit,
    parent: Entity,
    rot: Quat,
    center: Vec3,
    size: Vec3,
    mat: &Handle<StandardMaterial>,
) -> Entity {
    let mesh = kit.mesh(meshes::tiled_box(size, 2.0));
    kit.part(
        parent,
        &mesh,
        mat,
        Transform::from_translation(rot * center).with_rotation(rot),
    )
}

fn rotunda(kit: &mut Kit, world: Entity, images: &mut Assets<Image>, atlas: &PlacardAtlas, walls: &mut Walls) {
    let r = ROT_APOTHEM;
    let half_side = rot_side_len() / 2.0;
    let ivory = Color::srgb(0.93, 0.89, 0.8);
    let wall_mat = plaster_mat(kit, ivory, 0.8);
    let trim = kit.mat(StandardMaterial {
        base_color: Color::srgb(0.95, 0.94, 0.9),
        perceptual_roughness: 0.3,
        ..default()
    });
    let dark_marble = kit.mat(matte(Color::srgb(0.12, 0.13, 0.13), 0.2));

    // Floor: the octagon with the inlaid compass.
    {
        let mut m = MeshData::default();
        let circum = r / (PI / 8.0).cos();
        let pts: Vec<Vec3> = (0..8)
            .map(|k| {
                let a = (k as f32 + 0.5) * TAU / 8.0;
                Vec3::new(a.sin() * circum, 0.0, -a.cos() * circum)
            })
            .collect();
        let c = m.vert(Vec3::ZERO, Vec3::Y, Vec2::splat(0.5));
        let idx: Vec<u32> = pts
            .iter()
            .map(|p| m.vert(*p, Vec3::Y, Vec2::new(p.x / 36.0 + 0.5, p.z / 36.0 + 0.5)))
            .collect();
        for k in 0..8 {
            m.tri(c, idx[k], idx[(k + 1) % 8]);
        }
        let floor_tex = images.add(textures::rotunda_floor());
        let mat = kit.mat(StandardMaterial {
            base_color_texture: Some(floor_tex),
            perceptual_roughness: 0.16,
            reflectance: 0.6,
            ..default()
        });
        let mesh = kit.mesh(m.build());
        kit.part(world, &mesh, &mat, Transform::IDENTITY);
    }

    for side in 0..8 {
        let rot = side_rot(side);
        let o = OPENING_W / 2.0;
        let extra = WALL_T * (PI / 8.0).tan() + 0.02;
        let pier_w = half_side + extra - o;
        let zc = -(r + WALL_T / 2.0);
        for sx in [-1.0, 1.0] {
            let cx = sx * (o + pier_w / 2.0);
            wall_box(
                kit,
                world,
                rot,
                Vec3::new(cx, ROT_WALL_H / 2.0, zc),
                Vec3::new(pier_w, ROT_WALL_H, WALL_T),
                &wall_mat,
            );
            // Baseboard
            wall_box(
                kit,
                world,
                rot,
                Vec3::new(cx, 0.15, -r + 0.02),
                Vec3::new(pier_w, 0.3, 0.06),
                &dark_marble,
            );
            // Door frame (architrave) strip
            wall_box(
                kit,
                world,
                rot,
                Vec3::new(sx * (o + 0.2), OPENING_H / 2.0, -r + 0.04),
                Vec3::new(0.4, OPENING_H, 0.1),
                &trim,
            );
            // Engaged column flanking the opening
            let col = kit.node(
                world,
                Transform::from_translation(rot * Vec3::new(sx * (o + 1.15), 0.0, -r + 0.45)),
            );
            column(kit, col, 8.4, 0.36, &trim);
        }
        // Lintel above the opening
        let lh = ROT_WALL_H - OPENING_H;
        wall_box(
            kit,
            world,
            rot,
            Vec3::new(0.0, OPENING_H + lh / 2.0, zc),
            Vec3::new(OPENING_W, lh, WALL_T),
            &wall_mat,
        );
        wall_box(
            kit,
            world,
            rot,
            Vec3::new(0.0, OPENING_H + 0.2, -r + 0.04),
            Vec3::new(OPENING_W + 0.8, 0.4, 0.1),
            &trim,
        );
        // Cornice
        wall_box(
            kit,
            world,
            rot,
            Vec3::new(0.0, ROT_WALL_H - 0.35, -r + 0.2),
            Vec3::new(half_side * 2.0 + 0.3, 0.7, 0.4),
            &trim,
        );
        wall_box(
            kit,
            world,
            rot,
            Vec3::new(0.0, 8.8, -r + 0.08),
            Vec3::new(half_side * 2.0, 0.16, 0.16),
            &trim,
        );

        // Banner naming what lies beyond.
        let cell = if side == ENTRANCE_SIDE {
            Cell::Banner(7)
        } else {
            let gi = content::galleries().iter().position(|g| g.side == side).unwrap();
            Cell::Banner(gi)
        };
        let mat = atlas.material(kit.mats, cell, true);
        let quad = kit.mesh(Rectangle::new(6.4, 0.8));
        let e = kit.part(
            world,
            &quad,
            &mat,
            Transform::from_translation(rot * Vec3::new(0.0, 7.3, -r + 0.1)).with_rotation(rot),
        );
        kit.no_shadow(e);
    }

    // Ceiling ring where the round dome meets the octagon.
    let ceiling_mat = kit.mat(StandardMaterial {
        base_color: Color::srgb(0.9, 0.87, 0.8),
        perceptual_roughness: 0.8,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    let ring = kit.mesh(meshes::polygon_ring_ceiling(8, r + 0.3, r - 0.05, 128));
    kit.part(world, &ring, &ceiling_mat, at(0.0, ROT_WALL_H, 0.0));

    // Coffered dome (lower band) and smooth crown with the oculus.
    let (coffer_albedo, coffer_normal) = textures::coffer();
    let coffer_mat = kit.mat(StandardMaterial {
        base_color_texture: Some(images.add(coffer_albedo)),
        normal_map_texture: Some(images.add(coffer_normal)),
        perceptual_roughness: 0.75,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    let el_band = 52f32.to_radians();
    let dome_low = kit.mesh(meshes::dome(r, 0.0, el_band, 20, 112, 28.0, 5.0));
    kit.part(world, &dome_low, &coffer_mat, at(0.0, ROT_WALL_H, 0.0));
    let crown_mat = kit.mat(StandardMaterial {
        base_color: Color::srgb(0.86, 0.83, 0.76),
        base_color_texture: Some(kit.pal.plaster.clone()),
        perceptual_roughness: 0.85,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    let el_top = (OCULUS_R / r).acos();
    let dome_high = kit.mesh(meshes::dome(r, el_band, el_top, 16, 112, 14.0, 4.0));
    kit.part(world, &dome_high, &crown_mat, at(0.0, ROT_WALL_H, 0.0));
    let rim = kit.mesh(Torus::new(OCULUS_R - 0.12, OCULUS_R + 0.12).mesh().major_resolution(64));
    let bronze = kit.pal.bronze.clone();
    kit.part(world, &rim, &bronze, at(0.0, oculus_height() - 0.05, 0.0));
    // A band of gilding at the base of the dome.
    let band = kit.mesh(
        Torus::new(r - 0.2, r + 0.05)
            .mesh()
            .major_resolution(128)
            .minor_resolution(8),
    );
    let gold = kit.pal.gold.clone();
    kit.part(
        world,
        &band,
        &gold,
        at(0.0, ROT_WALL_H, 0.0).with_scale(Vec3::new(1.0, 0.6, 1.0)),
    );

    // Reflecting pool around the centerpiece.
    let rim_profile = [
        Vec2::new(POOL_R - 0.35, 0.3),
        Vec2::new(POOL_R - 0.35, 0.45),
        Vec2::new(POOL_R - 0.35, 0.45),
        Vec2::new(POOL_R, 0.45),
        Vec2::new(POOL_R, 0.45),
        Vec2::new(POOL_R + 0.08, 0.38),
        Vec2::new(POOL_R + 0.08, 0.0),
    ];
    let rim_mesh = kit.mesh(meshes::lathe(&rim_profile, 96));
    let marble = kit.pal.marble.clone();
    kit.part(world, &rim_mesh, &marble, Transform::IDENTITY);
    let water = kit.pal.water.clone();
    let disc = kit.mesh(Circle::new(POOL_R - 0.3).mesh().resolution(96));
    kit.part(
        world,
        &disc,
        &water,
        at(0.0, 0.32, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
    );

    // Benches facing the centerpiece on the diagonals between galleries.
    for k in 0..8 {
        if k % 2 == 0 {
            continue;
        }
        let a = (k as f32 + 0.5) * TAU / 8.0 - TAU / 16.0;
        let p = Vec3::new(a.sin(), 0.0, -a.cos()) * 9.0;
        let t = Transform::from_translation(p).with_rotation(Quat::from_rotation_y(-a));
        walls.add_bench(&t);
        let b = kit.node(world, t);
        bench(kit, b);
    }

    // Sunlight through the oculus: a distant, narrow spotlight whose beam is
    // clipped to a disc by the dome's shadow.
    let oculus = Vec3::new(0.0, oculus_height(), 0.0);
    let dist = 60.0;
    let pos = oculus + sun_dir() * dist;
    let spread = ((OCULUS_R + 1.2) / dist).atan();
    kit.cmd.spawn((
        SpotLight {
            color: Color::srgb(1.0, 0.94, 0.84),
            intensity: 4.0e9,
            range: 140.0,
            radius: 0.0,
            shadow_maps_enabled: true,
            inner_angle: spread * 0.8,
            outer_angle: spread,
            shadow_depth_bias: 0.05,
            ..default()
        },
        VolumetricLight,
        Transform::from_translation(pos).looking_at(oculus, Vec3::Y),
        Name::new("Sunbeam"),
    ));
    kit.cmd.spawn((
        FogVolume {
            density_factor: 0.0025,
            absorption: 0.05,
            scattering: 0.3,
            ..default()
        },
        Transform::from_xyz(0.0, 13.0, 0.0).with_scale(Vec3::new(2.0 * r, 26.0, 2.0 * r)),
    ));

    // Uplights washing the dome, and warm fill around the walls.
    for k in 0..8 {
        let a = (k as f32 + 0.5) * TAU / 8.0;
        let corner = Vec3::new(a.sin(), 0.0, -a.cos()) * (r / (PI / 8.0).cos() - 1.2);
        kit.cmd.spawn((
            SpotLight {
                color: Color::srgb(1.0, 0.86, 0.66),
                intensity: 1_600_000.0,
                range: 40.0,
                inner_angle: 0.5,
                outer_angle: 0.9,
                ..default()
            },
            Transform::from_translation(corner + Vec3::Y * 9.2).looking_at(Vec3::new(0.0, 22.0, 0.0), Vec3::Y),
        ));
        let mid = (k as f32) * TAU / 8.0;
        let wall_pt = Vec3::new(mid.sin(), 0.0, -mid.cos()) * (r - 3.0);
        kit.cmd.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.88, 0.72),
                intensity: 120_000.0,
                range: 16.0,
                radius: 0.3,
                ..default()
            },
            Transform::from_translation(wall_pt + Vec3::Y * 8.0),
        ));
    }
}

/// A classical column standing on `parent`'s origin.
fn column(kit: &mut Kit, parent: Entity, height: f32, radius: f32, mat: &Handle<StandardMaterial>) {
    let base = [
        Vec2::new(0.0, 0.0),
        Vec2::new(radius * 1.35, 0.0),
        Vec2::new(radius * 1.35, 0.18),
        Vec2::new(radius * 1.35, 0.18),
        Vec2::new(radius * 1.15, 0.26),
        Vec2::new(radius * 1.12, 0.34),
        Vec2::new(radius, 0.42),
        Vec2::new(radius, 0.42),
        Vec2::new(radius * 0.88, height - 0.5),
        Vec2::new(radius * 0.88, height - 0.5),
        Vec2::new(radius * 1.05, height - 0.42),
        Vec2::new(radius * 1.2, height - 0.2),
        Vec2::new(radius * 1.2, height - 0.2),
        Vec2::new(radius * 1.45, height - 0.2),
        Vec2::new(radius * 1.45, height - 0.2),
        Vec2::new(radius * 1.45, height),
        Vec2::new(radius * 1.45, height),
        Vec2::new(0.0, height),
    ];
    kit.lathe(parent, &base, mat, Transform::IDENTITY);
    kit.cube(
        parent,
        Vec3::new(radius * 3.0, 0.25, radius * 3.0),
        mat,
        at(0.0, height + 0.12, 0.0),
    );
}

fn bench(kit: &mut Kit, parent: Entity) {
    let seat = kit.pal.dark_wood.clone();
    let legs = kit.pal.marble.clone();
    kit.cube(parent, Vec3::new(2.6, 0.08, 0.6), &seat, at(0.0, 0.46, 0.0));
    for x in [-1.0, 1.0] {
        kit.cube(parent, Vec3::new(0.12, 0.42, 0.5), &legs, at(x, 0.21, 0.0));
    }
}

fn gallery(kit: &mut Kit, world: Entity, g: &Gallery, index: usize, atlas: &PlacardAtlas, walls: &mut Walls) {
    let rot = side_rot(g.side);
    let (w, l, h, r) = (GALLERY_W, g.length, g.height, ROT_APOTHEM);
    let mid = -(r + l / 2.0);
    let space = g.height > 10.0;

    let floor = kit.mesh(meshes::plane(w + 0.2, l, 2.0, true));
    let floor_mat = kit.pal.floor.clone();
    kit.part(
        world,
        &floor,
        &floor_mat,
        Transform::from_translation(rot * Vec3::new(0.0, 0.0, mid)).with_rotation(rot),
    );

    let wall_mat = plaster_mat(kit, g.wall, 0.9);
    let hw = w / 2.0 + WALL_T / 2.0;
    wall_box(
        kit,
        world,
        rot,
        Vec3::new(-hw, h / 2.0, mid - 0.05),
        Vec3::new(WALL_T, h, l - 0.1),
        &wall_mat,
    );
    wall_box(
        kit,
        world,
        rot,
        Vec3::new(hw, h / 2.0, mid - 0.05),
        Vec3::new(WALL_T, h, l - 0.1),
        &wall_mat,
    );
    wall_box(
        kit,
        world,
        rot,
        Vec3::new(0.0, h / 2.0, -(r + l + WALL_T / 2.0)),
        Vec3::new(w + 2.0 * WALL_T, h, WALL_T),
        &wall_mat,
    );
    if h > ROT_WALL_H {
        // Close the tall hall above the rotunda's wall.
        let extra = h - ROT_WALL_H;
        wall_box(
            kit,
            world,
            rot,
            Vec3::new(0.0, ROT_WALL_H + extra / 2.0, -(r + WALL_T / 2.0)),
            Vec3::new(w + 2.0 * WALL_T, extra, WALL_T),
            &wall_mat,
        );
    }
    let base_mat = kit.mat(matte(g.wall.darker(0.25), 0.4));
    for sx in [-1.0, 1.0] {
        wall_box(
            kit,
            world,
            rot,
            Vec3::new(sx * (w / 2.0 - 0.02), 0.12, mid),
            Vec3::new(0.05, 0.24, l),
            &base_mat,
        );
    }
    wall_box(
        kit,
        world,
        rot,
        Vec3::new(0.0, 0.12, -(r + l - 0.02)),
        Vec3::new(w, 0.24, 0.05),
        &base_mat,
    );

    // Ceiling
    let ceiling_color = if space {
        Color::srgb(0.03, 0.035, 0.06)
    } else {
        Color::srgb(0.92, 0.91, 0.88)
    };
    let ceiling_mat = plaster_mat(kit, ceiling_color, 0.9);
    wall_box(
        kit,
        world,
        rot,
        Vec3::new(0.0, h + 0.2, mid - WALL_T / 2.0 - 0.05),
        Vec3::new(w + 2.0 * WALL_T, 0.4, l + WALL_T - 0.1),
        &ceiling_mat,
    );

    let panel_len = l - 4.0;
    if space {
        // A field of stars on the dark ceiling of the space hall.
        let star = kit.glow(Color::srgb(0.9, 0.93, 1.0), 40.0);
        let star_mesh = kit.mesh(Sphere::new(0.035).mesh().ico(1).unwrap());
        for i in 0..260u32 {
            let hx = crate::textures::noise2(i as f32 * 3.1, 0.5, 0, 7) - 0.5;
            let hz = crate::textures::noise2(i as f32 * 1.7, 9.5, 0, 8);
            let big = crate::textures::noise2(i as f32 * 5.3, 2.5, 0, 9);
            let p = Vec3::new(hx * (w - 0.6), h - 0.02, -(r + 0.5 + hz * (l - 1.0)));
            let e = kit.part(
                world,
                &star_mesh,
                &star,
                Transform::from_translation(rot * p).with_scale(Vec3::splat(0.6 + big * 1.6)),
            );
            kit.no_shadow(e);
        }
        for k in 0..3 {
            let z = -(r + 6.0 + k as f32 * 13.0);
            kit.cmd.spawn((
                PointLight {
                    color: Color::srgb(0.75, 0.82, 1.0),
                    intensity: 260_000.0,
                    range: 22.0,
                    radius: 0.5,
                    ..default()
                },
                Transform::from_translation(rot * Vec3::new(0.0, h - 3.0, z)),
            ));
        }
    } else {
        // Light box running down the middle of the ceiling.
        let panel = kit.glow(Color::srgb(1.0, 0.97, 0.92), 18.0);
        let quad = kit.mesh(meshes::plane(2.4, panel_len, 1.0, false));
        let e = kit.part(
            world,
            &quad,
            &panel,
            Transform::from_translation(rot * Vec3::new(0.0, h - 0.01, mid)).with_rotation(rot),
        );
        kit.no_shadow(e);
        let frame = kit.pal.charcoal.clone();
        for sx in [-1.0, 1.0] {
            wall_box(
                kit,
                world,
                rot,
                Vec3::new(sx * 1.25, h - 0.06, mid),
                Vec3::new(0.1, 0.12, panel_len),
                &frame,
            );
        }
        kit.cmd.spawn((
            RectLight {
                color: Color::srgb(1.0, 0.95, 0.88),
                intensity: 180_000.0,
                width: 2.4,
                height: panel_len,
                range: 20.0,
            },
            Transform::from_translation(rot * Vec3::new(0.0, h - 0.05, mid))
                .with_rotation(rot * Quat::from_rotation_x(-FRAC_PI_2)),
        ));
    }
    // Spotlight tracks
    let track = kit.pal.black.clone();
    for sx in [-1.0, 1.0] {
        let e = wall_box(
            kit,
            world,
            rot,
            Vec3::new(sx * 2.4, h - 0.04, mid),
            Vec3::new(0.06, 0.06, l - 2.0),
            &track,
        );
        kit.no_shadow(e);
    }

    // Chair rail and crown moulding
    let rail = kit.mat(StandardMaterial {
        base_color: g.wall.lighter(0.08),
        perceptual_roughness: 0.5,
        ..default()
    });
    for sx in [-1.0, 1.0] {
        for (y, size) in [
            (1.0, Vec3::new(0.05, 0.07, l - 0.2)),
            (h - 0.12, Vec3::new(0.12, 0.24, l - 0.2)),
        ] {
            let e = wall_box(
                kit,
                world,
                rot,
                Vec3::new(sx * (w / 2.0 - size.x / 2.0), y, mid - 0.05),
                size,
                &rail,
            );
            kit.no_shadow(e);
        }
    }
    for (y, size) in [(1.0, Vec3::new(w, 0.07, 0.05)), (h - 0.12, Vec3::new(w, 0.24, 0.12))] {
        let e = wall_box(kit, world, rot, Vec3::new(0.0, y, -(r + l) + size.z / 2.0), size, &rail);
        kit.no_shadow(e);
    }
    // A gilded quotation high on the left wall.
    let quote_mat = atlas.material(kit.mats, Cell::Quote(index), true);
    let quote = kit.mesh(Rectangle::new(6.0, 2.25));
    let qy = if space { 7.0 } else { 4.3 };
    let e = kit.part(
        world,
        &quote,
        &quote_mat,
        Transform::from_translation(rot * Vec3::new(-w / 2.0 + 0.03, qy, -(r + 10.5)))
            .with_rotation(rot * Quat::from_rotation_y(FRAC_PI_2)),
    );
    kit.no_shadow(e);

    // Gallery introduction panel on the right, just inside the entrance.
    let intro_mat = atlas.material(kit.mats, Cell::Intro(index), false);
    let quad = kit.mesh(Rectangle::new(2.4, 1.5));
    kit.part(
        world,
        &quad,
        &intro_mat,
        Transform::from_translation(rot * Vec3::new(w / 2.0 - 0.03, 1.75, -(r + 3.2)))
            .with_rotation(rot * Quat::from_rotation_y(-FRAC_PI_2)),
    );

    // A bench midway for tired visitors (the space hall keeps its floor clear).
    if !space {
        let t = Transform::from_translation(rot * Vec3::new(0.0, 0.0, -(r + 12.5)))
            .with_rotation(rot * Quat::from_rotation_y(FRAC_PI_2));
        walls.add_bench(&t);
        let b = kit.node(world, t);
        bench(kit, b);
    }
}

fn vestibule(kit: &mut Kit, world: Entity, atlas: &PlacardAtlas) {
    let rot = side_rot(ENTRANCE_SIDE);
    let (w, l, h, r) = (VESTIBULE_W, VESTIBULE_L, VESTIBULE_H, ROT_APOTHEM);
    let mid = -(r + l / 2.0);
    let floor = kit.mesh(meshes::plane(w + 0.2, l, 2.0, true));
    let floor_mat = kit.pal.floor.clone();
    kit.part(
        world,
        &floor,
        &floor_mat,
        Transform::from_translation(rot * Vec3::new(0.0, 0.0, mid)).with_rotation(rot),
    );
    let wall_mat = plaster_mat(kit, Color::srgb(0.9, 0.86, 0.78), 0.85);
    let hw = w / 2.0 + WALL_T / 2.0;
    wall_box(
        kit,
        world,
        rot,
        Vec3::new(-hw, h / 2.0, mid - 0.05),
        Vec3::new(WALL_T, h, l - 0.1),
        &wall_mat,
    );
    wall_box(
        kit,
        world,
        rot,
        Vec3::new(hw, h / 2.0, mid - 0.05),
        Vec3::new(WALL_T, h, l - 0.1),
        &wall_mat,
    );
    wall_box(
        kit,
        world,
        rot,
        Vec3::new(0.0, h / 2.0, -(r + l + WALL_T / 2.0)),
        Vec3::new(w + 2.0 * WALL_T, h, WALL_T),
        &wall_mat,
    );
    let ceiling_mat = plaster_mat(kit, Color::srgb(0.92, 0.9, 0.86), 0.9);
    wall_box(
        kit,
        world,
        rot,
        Vec3::new(0.0, h + 0.2, mid - WALL_T / 2.0 - 0.05),
        Vec3::new(w + 2.0 * WALL_T, 0.4, l + WALL_T - 0.1),
        &ceiling_mat,
    );

    // Front doors (closed; the museum is yours alone).
    let door = kit.pal.dark_wood.clone();
    let brass = kit.pal.brass.clone();
    let trim = kit.pal.marble.clone();
    let far = -(r + l) + 0.08;
    wall_box(
        kit,
        world,
        rot,
        Vec3::new(0.0, 2.6, far + 0.02),
        Vec3::new(4.2, 5.2, 0.12),
        &trim,
    );
    for sx in [-1.0, 1.0] {
        wall_box(
            kit,
            world,
            rot,
            Vec3::new(sx * 0.92, 2.3, far + 0.1),
            Vec3::new(1.8, 4.6, 0.1),
            &door,
        );
        for y in [1.1, 3.4] {
            wall_box(
                kit,
                world,
                rot,
                Vec3::new(sx * 0.92, y, far + 0.16),
                Vec3::new(1.4, 1.6, 0.04),
                &door,
            );
        }
        wall_box(
            kit,
            world,
            rot,
            Vec3::new(sx * 0.25, 1.1, far + 0.25),
            Vec3::new(0.05, 0.9, 0.05),
            &brass,
        );
    }
    // Title banner over the arch into the rotunda, facing the doors.
    let mat = atlas.material(kit.mats, Cell::Banner(7), true);
    let quad = kit.mesh(Rectangle::new(6.4, 0.8));
    let e = kit.part(
        world,
        &quad,
        &mat,
        Transform::from_translation(rot * Vec3::new(0.0, 6.45, -(r + WALL_T + 0.02)))
            .with_rotation(rot * Quat::from_rotation_y(PI)),
    );
    kit.no_shadow(e);
    // Welcome panel
    let intro_mat = atlas.material(kit.mats, Cell::Intro(7), false);
    let quad = kit.mesh(Rectangle::new(2.4, 1.5));
    kit.part(
        world,
        &quad,
        &intro_mat,
        Transform::from_translation(rot * Vec3::new(-w / 2.0 + 0.03, 1.75, mid))
            .with_rotation(rot * Quat::from_rotation_y(FRAC_PI_2)),
    );
    // Potted plants in the corners
    for sx in [-1.0, 1.0] {
        let p = kit.node(
            world,
            Transform::from_translation(rot * Vec3::new(sx * (w / 2.0 - 0.8), 0.0, -(r + l - 1.0))),
        );
        plant(kit, p);
    }
    let panel = kit.glow(Color::srgb(1.0, 0.97, 0.92), 18.0);
    let quad = kit.mesh(meshes::plane(2.4, l - 3.0, 1.0, false));
    let e = kit.part(
        world,
        &quad,
        &panel,
        Transform::from_translation(rot * Vec3::new(0.0, h - 0.01, mid)).with_rotation(rot),
    );
    kit.no_shadow(e);
    kit.cmd.spawn((
        RectLight {
            color: Color::srgb(1.0, 0.95, 0.88),
            intensity: 120_000.0,
            width: 2.4,
            height: l - 3.0,
            range: 16.0,
        },
        Transform::from_translation(rot * Vec3::new(0.0, h - 0.05, mid))
            .with_rotation(rot * Quat::from_rotation_x(-FRAC_PI_2)),
    ));
}

fn plant(kit: &mut Kit, parent: Entity) {
    let pot = kit.pal.clay.clone();
    let soil = kit.pal.charcoal.clone();
    let leaf = kit.color(Color::srgb(0.12, 0.32, 0.12), 0.7);
    kit.lathe(
        parent,
        &[
            Vec2::new(0.0, 0.0),
            Vec2::new(0.3, 0.0),
            Vec2::new(0.42, 0.7),
            Vec2::new(0.42, 0.7),
            Vec2::new(0.38, 0.7),
            Vec2::new(0.38, 0.62),
        ],
        &pot,
        Transform::IDENTITY,
    );
    kit.cyl(parent, 0.38, 0.02, &soil, at(0.0, 0.64, 0.0));
    let trunk = kit.pal.wood.clone();
    kit.rod(
        parent,
        Vec3::new(0.0, 0.6, 0.0),
        Vec3::new(0.05, 1.6, 0.0),
        0.04,
        &trunk,
    );
    for i in 0..7 {
        let a = i as f32 * 2.4;
        let y = 1.4 + (i % 3) as f32 * 0.25;
        let p = Vec3::new(a.cos() * 0.3, y, a.sin() * 0.3);
        let e = kit.ball(
            parent,
            0.35,
            &leaf,
            Transform::from_translation(p).with_scale(Vec3::new(1.0, 0.7, 1.0)),
        );
        kit.cmd.entity(e).insert(NotShadowCaster);
    }
}

/// Shadow maps are expensive, so only the exhibit lights nearest the
/// visitor cast shadows.
pub fn shadow_lod(
    time: Res<Time>,
    mut timer: Local<f32>,
    player: Query<&Transform, With<crate::player::Player>>,
    mut spots: Query<(&GlobalTransform, &mut SpotLight), With<ShadowLod>>,
    mut points: Query<(&GlobalTransform, &mut PointLight), With<ShadowLod>>,
) {
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = 0.25;
    let Ok(p) = player.single() else { return };
    let eye = p.translation;
    let mut d: Vec<f32> = spots.iter().map(|(t, _)| t.translation().distance(eye)).collect();
    d.sort_by(f32::total_cmp);
    let max_shadows: usize = std::env::var("MUSEUM_SHADOWS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(6);
    let cutoff = if max_shadows == 0 {
        -1.0
    } else {
        d.get(max_shadows - 1).copied().unwrap_or(f32::MAX).min(22.0)
    };
    for (t, mut l) in &mut spots {
        let on = t.translation().distance(eye) <= cutoff;
        if l.shadow_maps_enabled != on {
            l.shadow_maps_enabled = on;
        }
    }
    for (t, mut l) in &mut points {
        let on = max_shadows > 0 && t.translation().distance(eye) < 12.0;
        if l.shadow_maps_enabled != on {
            l.shadow_maps_enabled = on;
        }
    }
}
