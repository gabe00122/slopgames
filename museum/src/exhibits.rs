//! Places every exhibit: its mount, model, placard, spotlight and collision.

use crate::{
    architecture::ShadowLod,
    content::{self, Mount, Slot},
    kit::{Kit, Palette, at},
    layout::{self, Walls},
    models::{self, Ctx},
    placards::{Cell, PlacardAtlas},
};
use bevy::prelude::*;

/// World-space facts about each exhibit, for the HUD, tour and map.
#[derive(Clone)]
pub struct ExhibitInfo {
    pub gallery: Option<usize>,
    pub focus: Vec3,
    pub radius: f32,
    /// Where a visitor would stand to admire it (eye position).
    pub view: Vec3,
    /// Point on the gallery's center line at the exhibit, for tour routing.
    pub aisle: Vec3,
    /// Center of the placard and the direction it faces.
    pub placard: (Vec3, Vec3),
}

#[derive(Resource)]
pub struct ExhibitIndex(pub Vec<ExhibitInfo>);

pub fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut walls: ResMut<Walls>,
    pal: Res<Palette>,
    atlas: Res<PlacardAtlas>,
) {
    let galleries = content::galleries();
    let list = content::exhibits();
    let mut infos = Vec::new();
    let mut kit = Kit {
        cmd: &mut commands,
        meshes: &mut meshes,
        mats: &mut mats,
        pal: &pal,
        cache: Default::default(),
    };
    let mut ctx = Ctx {
        images: &mut images,
        atlas: &atlas,
    };

    for (i, e) in list.iter().enumerate() {
        let t = layout::exhibit_transform(&galleries, e);
        let ceiling = e.gallery.map(|g| galleries[g].height).unwrap_or(26.0);
        let root = kit.cmd.spawn((t, Visibility::default(), Name::new(e.title))).id();
        let h = layout::mount_height(e.mount);
        mount(&mut kit, root, e.mount);
        let scale = models::scale(e.model);
        let top = kit.node(root, at(0.0, h, 0.0).with_scale(Vec3::splat(scale)));
        let mut b = models::build(&mut kit, &mut ctx, top, e.model, (ceiling - t.translation.y) / scale);
        b.center *= scale;
        b.radius *= scale;
        let focus = t.transform_point(Vec3::new(0.0, h, 0.0) + b.center);
        let forward = t.rotation * Vec3::Z;

        // Placard on a lectern in front and to the side.
        let front = match e.mount {
            Mount::Plinth(_, _, d) | Mount::Floor(_, d) => d / 2.0,
            Mount::Round(r, _) => r,
            Mount::Cables => 1.4,
        };
        let (lectern_local, lectern_yaw) = match e.slot {
            Slot::Center => (Vec3::new(0.0, 0.0, layout::POOL_R + 0.75), 0.0),
            Slot::Hang(..) => (Vec3::new(1.6, -t.translation.y, 1.2), -0.35),
            _ => (Vec3::new(0.85, 0.0, front + 0.5), -0.25),
        };
        let lectern = kit.node(
            root,
            yaw(at(lectern_local.x, lectern_local.y, lectern_local.z), lectern_yaw),
        );
        placard(&mut kit, lectern, &atlas, i);
        let lectern_world = t.transform_point(lectern_local);
        let lectern_rot = t.rotation * Quat::from_rotation_y(lectern_yaw) * Quat::from_rotation_x(-0.6);
        let placard_at = (
            lectern_world + t.rotation * Quat::from_rotation_y(lectern_yaw) * Vec3::new(0.0, 1.02, 0.0),
            lectern_rot * Vec3::Z,
        );
        walls.circles.push((Vec2::new(lectern_world.x, lectern_world.z), 0.3));

        // Collision around the display
        let r = layout::mount_radius(e.mount);
        if r > 0.0 && !matches!(e.slot, Slot::Center) {
            walls.circles.push((Vec2::new(t.translation.x, t.translation.z), r));
        }

        // Spotlight from the ceiling, in front of the piece.
        let spot_pos = match e.gallery {
            None => Vec3::new(0.0, 9.6, 9.0),
            Some(g) => {
                // Hang the light from the nearest ceiling track, a little
                // toward the entrance so it lights the front of the piece.
                let rot = layout::side_rot(galleries[g].side);
                let f = rot.inverse() * focus;
                let x = if f.x.abs() > 1.0 { 2.4 * f.x.signum() } else { 2.4 };
                let dz = if f.x.abs() > 1.0 { 1.2 } else { 3.0 };
                rot * Vec3::new(x, ceiling - 0.3, f.z + dz)
            }
        };
        let dist = spot_pos.distance(focus);
        let outer = (b.radius * 1.35 / dist).atan().clamp(0.12, 0.75);
        let lux = if matches!(e.slot, Slot::Center) { 900.0 } else { 520.0 };
        kit.cmd.spawn((
            SpotLight {
                color: Color::srgb(1.0, 0.95, 0.86),
                intensity: lux * 4.0 * std::f32::consts::PI * dist * dist,
                range: dist * 2.2,
                radius: 0.05,
                inner_angle: outer * 0.55,
                outer_angle: outer,
                shadow_maps_enabled: false,
                ..default()
            },
            ShadowLod,
            Transform::from_translation(spot_pos).looking_at(focus, Vec3::Y),
        ));
        // Visible fixture on the track
        if !matches!(e.slot, Slot::Center) {
            let can = kit.pal.black.clone();
            let dir = (focus - spot_pos).normalize();
            let e = kit.shape(
                root,
                Cylinder::new(0.07, 0.22).mesh().resolution(16),
                &can,
                Transform::IDENTITY,
            );
            let local = t.compute_affine().inverse();
            let p = local.transform_point3(spot_pos - dir * 0.12);
            let rot = t.rotation.inverse() * Quat::from_rotation_arc(Vec3::NEG_Y, dir);
            kit.insert(e, Transform::from_translation(p).with_rotation(rot));
        }

        // Viewpoint for the guided tour.
        let (view, aisle) = match e.slot {
            Slot::Center => (Vec3::new(0.0, 1.65, 9.0), Vec3::new(0.0, 0.0, 9.0)),
            Slot::Hang(x, d, _) => {
                let g = &galleries[e.gallery.unwrap()];
                let rot = layout::side_rot(g.side);
                let p = rot * Vec3::new(x * 0.3, 1.65, -(layout::ROT_APOTHEM + d - 8.0));
                (p, Vec3::new(p.x, 0.0, p.z))
            }
            _ => {
                let g = &galleries[e.gallery.unwrap()];
                let rot = layout::side_rot(g.side);
                let dist = (b.radius * 2.3).clamp(1.9, 6.0);
                let mut p = t.translation + forward * (front.max(0.6) + dist);
                p.y = 1.65;
                // Keep the viewpoint inside the gallery walls.
                let mut local = rot.inverse() * p;
                local.x = local.x.clamp(-5.0, 5.0);
                local.z = local.z.max(-(layout::ROT_APOTHEM + g.length - 1.0));
                let p = rot * local;
                let a = rot * Vec3::new(0.0, 0.0, local.z);
                (p, a)
            }
        };
        infos.push(ExhibitInfo {
            gallery: e.gallery,
            focus,
            radius: b.radius,
            view,
            aisle,
            placard: placard_at,
        });
    }
    commands.insert_resource(ExhibitIndex(infos));
}

fn yaw(t: Transform, a: f32) -> Transform {
    t.with_rotation(Quat::from_rotation_y(a))
}

fn mount(kit: &mut Kit, root: Entity, m: Mount) {
    let pal = kit.pal;
    match m {
        Mount::Plinth(w, h, d) => {
            kit.cube(
                root,
                Vec3::new(w, h - 0.06, d),
                &pal.plinth,
                at(0.0, (h - 0.06) / 2.0, 0.0),
            );
            kit.cube(
                root,
                Vec3::new(w + 0.04, 0.06, d + 0.04),
                &pal.plinth,
                at(0.0, h - 0.03, 0.0),
            );
            kit.cube(
                root,
                Vec3::new(w + 0.02, 0.08, d + 0.02),
                &pal.charcoal,
                at(0.0, 0.04, 0.0),
            );
        }
        Mount::Round(r, h) => {
            let profile = [
                Vec2::new(0.0, 0.0),
                Vec2::new(r + 0.06, 0.0),
                Vec2::new(r + 0.06, 0.08),
                Vec2::new(r + 0.06, 0.08),
                Vec2::new(r, 0.12),
                Vec2::new(r, h - 0.05),
                Vec2::new(r, h - 0.05),
                Vec2::new(r + 0.04, h),
                Vec2::new(r + 0.04, h),
                Vec2::new(0.0, h),
            ];
            kit.lathe(root, &profile, &pal.marble, Transform::IDENTITY);
        }
        Mount::Floor(w, d) => {
            kit.cube(root, Vec3::new(w, 0.12, d), &pal.charcoal, at(0.0, 0.06, 0.0));
            // Low rope barrier along the front
            let (hw, hd) = (w / 2.0 + 0.25, d / 2.0 + 0.35);
            let posts = [Vec3::new(-hw, 0.0, hd), Vec3::new(0.0, 0.0, hd), Vec3::new(hw, 0.0, hd)];
            for p in posts {
                kit.cyl(
                    root,
                    0.025,
                    0.9,
                    &pal.brass,
                    Transform::from_translation(p + Vec3::Y * 0.45),
                );
                kit.cyl(
                    root,
                    0.12,
                    0.03,
                    &pal.brass,
                    Transform::from_translation(p + Vec3::Y * 0.015),
                );
                kit.ball(root, 0.04, &pal.brass, Transform::from_translation(p + Vec3::Y * 0.92));
            }
            for k in 0..2 {
                let a = posts[k] + Vec3::Y * 0.85;
                let b = posts[k + 1] + Vec3::Y * 0.85;
                let pts = crate::meshes::sag(a, b, 0.14, 16);
                kit.shape(
                    root,
                    crate::meshes::tube(&pts, |_| 0.018, 8, false),
                    &pal.velvet,
                    Transform::IDENTITY,
                );
            }
        }
        Mount::Cables => {}
    }
}

fn placard(kit: &mut Kit, lectern: Entity, atlas: &PlacardAtlas, index: usize) {
    let pal = kit.pal;
    kit.cube(lectern, Vec3::new(0.05, 0.95, 0.05), &pal.black, at(0.0, 0.475, -0.05));
    kit.cube(lectern, Vec3::new(0.3, 0.02, 0.22), &pal.black, at(0.0, 0.01, -0.05));
    let panel = kit.node(
        lectern,
        Transform::from_xyz(0.0, 1.02, 0.0).with_rotation(Quat::from_rotation_x(-0.6)),
    );
    kit.cube(panel, Vec3::new(0.7, 0.46, 0.025), &pal.black, at(0.0, 0.0, -0.014));
    let mat = atlas.material(kit.mats, Cell::Exhibit(index), false);
    kit.shape(panel, Rectangle::new(0.64, 0.4), &mat, Transform::IDENTITY);
}
