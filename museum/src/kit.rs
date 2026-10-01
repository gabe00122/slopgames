//! Shared materials and a small helper for assembling models out of parts.

use crate::{meshes, textures};
use bevy::{light::NotShadowCaster, prelude::*};

/// Common materials, created once at startup.
#[derive(Resource)]
pub struct Palette {
    pub gold: Handle<StandardMaterial>,
    pub brass: Handle<StandardMaterial>,
    pub bronze: Handle<StandardMaterial>,
    pub copper: Handle<StandardMaterial>,
    pub steel: Handle<StandardMaterial>,
    pub chrome: Handle<StandardMaterial>,
    pub iron: Handle<StandardMaterial>,
    pub aluminium: Handle<StandardMaterial>,
    pub white: Handle<StandardMaterial>,
    pub ivory: Handle<StandardMaterial>,
    pub black: Handle<StandardMaterial>,
    pub charcoal: Handle<StandardMaterial>,
    pub red: Handle<StandardMaterial>,
    pub wood: Handle<StandardMaterial>,
    pub dark_wood: Handle<StandardMaterial>,
    pub stone: Handle<StandardMaterial>,
    pub sandstone: Handle<StandardMaterial>,
    pub marble: Handle<StandardMaterial>,
    pub clay: Handle<StandardMaterial>,
    pub glass: Handle<StandardMaterial>,
    pub water: Handle<StandardMaterial>,
    pub plinth: Handle<StandardMaterial>,
    pub velvet: Handle<StandardMaterial>,
    pub floor: Handle<StandardMaterial>,
    pub plaster: Handle<Image>,
    pub plaster_normal: Handle<Image>,
    pub wood_tex: Handle<Image>,
    /// Unit meshes shared by every box, cylinder, rod and ball so that
    /// identical parts batch together.
    pub unit_cube: Handle<Mesh>,
    pub unit_cyl: Handle<Mesh>,
    pub unit_rod: Handle<Mesh>,
    pub unit_ball: Handle<Mesh>,
}

pub fn metal(c: Color, rough: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: c,
        metallic: 1.0,
        perceptual_roughness: rough,
        ..default()
    }
}

pub fn matte(c: Color, rough: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: c,
        perceptual_roughness: rough,
        ..default()
    }
}

pub fn glow(c: Color, strength: f32) -> StandardMaterial {
    let l = c.to_linear();
    StandardMaterial {
        base_color: c,
        emissive: LinearRgba::rgb(l.red * strength, l.green * strength, l.blue * strength),
        ..default()
    }
}

pub fn setup_palette(
    mut commands: Commands,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (plaster, plaster_normal) = textures::plaster();
    let plaster = images.add(plaster);
    let plaster_normal = images.add(plaster_normal);
    let wood_tex = images.add(textures::wood());
    let (floor_tex, floor_normal) = textures::marble_floor();
    let floor = mats.add(StandardMaterial {
        base_color_texture: Some(images.add(floor_tex)),
        normal_map_texture: Some(images.add(floor_normal)),
        perceptual_roughness: 0.18,
        reflectance: 0.6,
        ..default()
    });

    let mut m = |s: StandardMaterial| mats.add(s);
    let palette = Palette {
        gold: m(metal(Color::srgb(1.0, 0.77, 0.34), 0.22)),
        brass: m(metal(Color::srgb(0.86, 0.66, 0.32), 0.32)),
        bronze: m(metal(Color::srgb(0.55, 0.36, 0.2), 0.45)),
        copper: m(metal(Color::srgb(0.93, 0.55, 0.4), 0.3)),
        steel: m(metal(Color::srgb(0.62, 0.63, 0.66), 0.35)),
        chrome: m(metal(Color::srgb(0.92, 0.92, 0.94), 0.06)),
        iron: m(metal(Color::srgb(0.16, 0.16, 0.17), 0.6)),
        aluminium: m(metal(Color::srgb(0.85, 0.86, 0.88), 0.2)),
        white: m(matte(Color::srgb(0.9, 0.9, 0.88), 0.5)),
        ivory: m(matte(Color::srgb(0.93, 0.9, 0.82), 0.6)),
        black: m(matte(Color::srgb(0.03, 0.03, 0.035), 0.45)),
        charcoal: m(matte(Color::srgb(0.12, 0.12, 0.13), 0.7)),
        red: m(matte(Color::srgb(0.6, 0.08, 0.06), 0.5)),
        wood: m(StandardMaterial {
            base_color_texture: Some(wood_tex.clone()),
            perceptual_roughness: 0.55,
            ..default()
        }),
        dark_wood: m(StandardMaterial {
            base_color: Color::srgb(0.45, 0.36, 0.32),
            base_color_texture: Some(wood_tex.clone()),
            perceptual_roughness: 0.4,
            ..default()
        }),
        stone: m(matte(Color::srgb(0.5, 0.48, 0.45), 0.9)),
        sandstone: m(matte(Color::srgb(0.82, 0.68, 0.46), 0.85)),
        marble: m(matte(Color::srgb(0.93, 0.92, 0.89), 0.25)),
        clay: m(matte(Color::srgb(0.66, 0.4, 0.26), 0.85)),
        glass: m(StandardMaterial {
            base_color: Color::srgba(0.85, 0.92, 0.95, 0.12),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.04,
            reflectance: 0.9,
            ..default()
        }),
        water: m(StandardMaterial {
            base_color: Color::srgb(0.02, 0.05, 0.06),
            perceptual_roughness: 0.02,
            reflectance: 1.0,
            ..default()
        }),
        plinth: m(StandardMaterial {
            base_color: Color::srgb(0.94, 0.93, 0.9),
            base_color_texture: Some(plaster.clone()),
            perceptual_roughness: 0.7,
            ..default()
        }),
        velvet: m(matte(Color::srgb(0.45, 0.04, 0.06), 0.95)),
        floor,
        plaster,
        plaster_normal,
        wood_tex,
        unit_cube: meshes.add(Cuboid::from_length(1.0)),
        unit_cyl: meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(32)),
        unit_rod: meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(12)),
        unit_ball: meshes.add(Sphere::new(1.0).mesh().uv(32, 18)),
    };
    commands.insert_resource(palette);
}

/// Bundles the asset stores needed to spawn model parts.
pub struct Kit<'a, 'w, 's> {
    pub cmd: &'a mut Commands<'w, 's>,
    pub meshes: &'a mut Assets<Mesh>,
    pub mats: &'a mut Assets<StandardMaterial>,
    pub pal: &'a Palette,
    /// Materials already created, keyed by their parameters.
    pub cache: std::collections::HashMap<String, H<StandardMaterial>>,
}

pub type H<T> = Handle<T>;

impl Kit<'_, '_, '_> {
    pub fn mesh(&mut self, m: impl Into<Mesh>) -> H<Mesh> {
        self.meshes.add(m)
    }

    pub fn mat(&mut self, m: StandardMaterial) -> H<StandardMaterial> {
        self.mats.add(m)
    }

    fn cached(&mut self, key: String, make: impl FnOnce() -> StandardMaterial) -> H<StandardMaterial> {
        if let Some(h) = self.cache.get(&key) {
            return h.clone();
        }
        let h = self.mats.add(make());
        self.cache.insert(key, h.clone());
        h
    }

    pub fn color(&mut self, c: Color, rough: f32) -> H<StandardMaterial> {
        self.cached(format!("c{:?}{rough}", c.to_srgba()), || matte(c, rough))
    }

    pub fn metal(&mut self, c: Color, rough: f32) -> H<StandardMaterial> {
        self.cached(format!("m{:?}{rough}", c.to_srgba()), || metal(c, rough))
    }

    pub fn glow(&mut self, c: Color, strength: f32) -> H<StandardMaterial> {
        self.cached(format!("g{:?}{strength}", c.to_srgba()), || glow(c, strength))
    }

    fn unit(
        &mut self,
        parent: Entity,
        mesh: H<Mesh>,
        size: Vec3,
        mat: &H<StandardMaterial>,
        mut t: Transform,
    ) -> Entity {
        t.scale *= size;
        self.part(parent, &mesh, mat, t)
    }

    /// Empty transform node for grouping and animating parts.
    pub fn node(&mut self, parent: Entity, t: Transform) -> Entity {
        self.cmd.spawn((t, Visibility::default(), ChildOf(parent))).id()
    }

    pub fn part(&mut self, parent: Entity, mesh: &H<Mesh>, mat: &H<StandardMaterial>, t: Transform) -> Entity {
        self.cmd
            .spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), t, ChildOf(parent)))
            .id()
    }

    pub fn shape(&mut self, parent: Entity, shape: impl Into<Mesh>, mat: &H<StandardMaterial>, t: Transform) -> Entity {
        let mesh = self.mesh(shape);
        self.part(parent, &mesh, mat, t)
    }

    /// Box of the given size.
    /// Box of the given size. (Scaled, so don't parent other parts to it.)
    pub fn cube(&mut self, parent: Entity, size: Vec3, mat: &H<StandardMaterial>, t: Transform) -> Entity {
        self.unit(parent, self.pal.unit_cube.clone(), size, mat, t)
    }

    /// Cylinder standing on Y.
    pub fn cyl(&mut self, parent: Entity, r: f32, h: f32, mat: &H<StandardMaterial>, t: Transform) -> Entity {
        self.unit(parent, self.pal.unit_cyl.clone(), Vec3::new(r, h, r), mat, t)
    }

    pub fn ball(&mut self, parent: Entity, r: f32, mat: &H<StandardMaterial>, t: Transform) -> Entity {
        self.unit(parent, self.pal.unit_ball.clone(), Vec3::splat(r), mat, t)
    }

    /// A rod of radius `r` between two points.
    pub fn rod(&mut self, parent: Entity, a: Vec3, b: Vec3, r: f32, mat: &H<StandardMaterial>) -> Entity {
        let d = b - a;
        let len = d.length();
        let t = Transform::from_translation((a + b) / 2.0)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, d / len.max(1e-6)));
        self.unit(parent, self.pal.unit_rod.clone(), Vec3::new(r, len, r), mat, t)
    }

    pub fn lathe(&mut self, parent: Entity, profile: &[Vec2], mat: &H<StandardMaterial>, t: Transform) -> Entity {
        self.shape(parent, meshes::lathe(profile, 48), mat, t)
    }

    /// Marks an entity so it doesn't cast shadows (glass, glowing parts).
    pub fn no_shadow(&mut self, e: Entity) -> Entity {
        self.cmd.entity(e).insert(NotShadowCaster);
        e
    }

    pub fn insert(&mut self, e: Entity, b: impl Bundle) -> Entity {
        self.cmd.entity(e).insert(b);
        e
    }
}

pub fn v2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

pub fn at(x: f32, y: f32, z: f32) -> Transform {
    Transform::from_xyz(x, y, z)
}
