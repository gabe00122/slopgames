//! The swamp water: a wave-equation simulation over columns along the whole
//! level, the translucent meshes that draw it, and buoyancy and drag for
//! every floating body, applied as forces at sample points so boats rock,
//! list and capsize on their own.

use crate::{
    game::*,
    level::Terrain,
    meshkit::MeshBuilder,
    util::{lerp, lin, mix},
};
use avian2d::prelude::*;
use bevy::{
    asset::RenderAssetUsages,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
    prelude::*,
};

/// Column spacing of the wave simulation.
pub const COL: f32 = 0.2;
/// Deepest the water meshes reach.
const FLOOR: f32 = -14.0;

#[derive(Resource, Default)]
pub struct Water {
    h: Vec<f32>,
    v: Vec<f32>,
    wet: Vec<bool>,
    /// Column ranges `[start, end)` of each body of water.
    pub segments: Vec<(usize, usize)>,
    pub t: f32,
    /// Bank height under each column (for the mesh's lower edge).
    ground: Vec<f32>,
}

impl Water {
    pub fn new(terrain: &Terrain) -> Self {
        let n = (terrain.len / COL) as usize + 2;
        let ground: Vec<f32> = (0..n).map(|i| terrain.height(i as f32 * COL)).collect();
        let wet: Vec<bool> = ground.iter().map(|&g| g < WATER_Y - 0.01).collect();
        let mut segments = vec![];
        let mut i = 0;
        while i < n {
            if wet[i] {
                let s = i.saturating_sub(1);
                while i < n && wet[i] {
                    i += 1;
                }
                segments.push((s, (i + 1).min(n)));
            }
            i += 1;
        }
        let mut wet2 = wet.clone();
        for &(s, e) in &segments {
            for w in &mut wet2[s..e] {
                *w = true;
            }
        }
        Water {
            h: vec![0.0; n],
            v: vec![0.0; n],
            wet: wet2,
            segments,
            t: 0.0,
            ground,
        }
    }

    /// Always-on gentle chop, so the surface is never glassy.
    fn ambient(&self, x: f32) -> f32 {
        0.035 * (x * 0.9 + self.t * 1.3).sin() + 0.02 * (x * 2.3 - self.t * 2.1).sin()
    }

    fn col(&self, x: f32) -> Option<(usize, f32)> {
        if self.h.len() < 2 {
            return None;
        }
        let f = x / COL;
        if f < 0.0 || f >= (self.h.len() - 1) as f32 {
            return None;
        }
        let i = f.floor() as usize;
        Some((i, f - i as f32))
    }

    /// Height of the water surface at `x`, or `None` over dry land.
    pub fn surface(&self, x: f32) -> Option<f32> {
        let (i, f) = self.col(x)?;
        if !self.wet[i] && !self.wet[i + 1] {
            return None;
        }
        Some(WATER_Y + lerp(self.h[i], self.h[i + 1], f) + self.ambient(x))
    }

    /// How far below the surface `p` is (negative above it, or over land).
    pub fn depth(&self, p: Vec2) -> f32 {
        self.surface(p.x).map_or(-100.0, |s| s - p.y)
    }

    /// Pushes the surface at `x` with a vertical velocity change `dv` (m/s).
    pub fn disturb(&mut self, x: f32, dv: f32) {
        let Some((i, f)) = self.col(x) else { return };
        for (j, w) in [(i.saturating_sub(1), 0.25), (i, 1.0 - f), (i + 1, f), (i + 2, 0.25)] {
            if j < self.v.len() && self.wet[j] {
                self.v[j] += dv * w;
            }
        }
    }

    pub fn step(&mut self, dt: f32) {
        self.t += dt;
        const C2: f32 = 5.0 * 5.0 / (COL * COL);
        const K: f32 = 6.0;
        const DAMP: f32 = 1.4;
        let sub = 3;
        let h_dt = dt / sub as f32;
        for _ in 0..sub {
            for &(s, e) in &self.segments {
                for i in s..e {
                    let l = if i > s { self.h[i - 1] } else { self.h[i] };
                    let r = if i + 1 < e { self.h[i + 1] } else { self.h[i] };
                    let a = C2 * (l + r - 2.0 * self.h[i]) - K * self.h[i] - DAMP * self.v[i];
                    self.v[i] += a * h_dt;
                }
                for i in s..e {
                    self.h[i] = (self.h[i] + self.v[i] * h_dt).clamp(-1.0, 1.0);
                }
            }
        }
    }
}

pub fn simulate(time: Res<Time>, mut water: ResMut<Water>) {
    water.step(time.delta_secs());
}

/// Makes a body float. Buoyancy and drag are applied at each sample point,
/// weighted by how far under the surface that point is.
#[derive(Component)]
pub struct Floater {
    /// Sample points in the body's local frame.
    pub points: Vec<Vec2>,
    /// Half the vertical extent each point stands for.
    pub cell: f32,
    /// Upward force when fully submerged, as a multiple of the body's weight.
    pub buoyancy: f32,
    /// Linear drag in the body's local x and y (1/s).
    pub drag: Vec2,
    pub angular_drag: f32,
    /// How much of the body is under water (0..1), updated every step.
    pub submerged: f32,
    /// Splashes are reported when the body enters the water this hard.
    pub splashy: bool,
}

impl Floater {
    /// Sample points on a grid covering a `w` x `h` box.
    pub fn boxed(w: f32, h: f32, nx: usize, ny: usize, buoyancy: f32, drag: Vec2) -> Self {
        let mut points = vec![];
        for j in 0..ny {
            for i in 0..nx {
                points.push(Vec2::new(
                    (i as f32 + 0.5) / nx as f32 * w - w * 0.5,
                    (j as f32 + 0.5) / ny as f32 * h - h * 0.5,
                ));
            }
        }
        Floater {
            points,
            cell: h / ny as f32 * 0.5,
            buoyancy,
            drag,
            angular_drag: 2.0,
            submerged: 0.0,
            splashy: true,
        }
    }
}

pub fn buoyancy(
    mut water: ResMut<Water>,
    time: Res<Time>,
    mut bodies: Query<(Forces, &mut Floater, &ComputedMass, &ComputedAngularInertia)>,
    mut splash: MessageWriter<Splash>,
) {
    let dt = time.delta_secs();
    for (mut forces, mut fl, mass, inertia) in &mut bodies {
        let m = mass.value();
        if m <= 0.0 || !m.is_finite() {
            continue;
        }
        let pos = forces.position().0;
        let rot = *forces.rotation();
        let n = fl.points.len() as f32;
        let mut total = 0.0;
        let mut surface_x = None;
        for p in fl.points.clone() {
            let world = pos + rot * p;
            let Some(s) = water.surface(world.x) else {
                continue;
            };
            let frac = ((s - (world.y - fl.cell)) / (2.0 * fl.cell)).clamp(0.0, 1.0);
            if frac <= 0.0 {
                continue;
            }
            if frac < 1.0 {
                surface_x = Some(world.x);
            }
            total += frac / n;
            let lift = Vec2::Y * fl.buoyancy * m * GRAVITY * frac / n;
            let v = forces.velocity_at_point(world);
            let local_v = rot.inverse() * v;
            let drag = rot * (-local_v * fl.drag * (m / n) * frac);
            forces.apply_force_at_point(lift + drag, world);
        }
        if total > 0.0 {
            let w = forces.angular_velocity();
            forces.apply_torque(-w * fl.angular_drag * inertia.value() * total);
        }
        let vel = forces.linear_velocity();
        // Entering the water: a splash; moving along the surface: a wake.
        if fl.splashy && fl.submerged < 0.12 && total >= 0.12 && vel.y < -2.5 {
            let strength = -vel.y * m.sqrt() * 0.12;
            splash.write(Splash {
                pos: Vec2::new(pos.x, WATER_Y),
                strength,
            });
            water.disturb(pos.x, vel.y * 0.35 * (m / 60.0).sqrt().min(2.0));
        }
        if let Some(x) = surface_x {
            water.disturb(x, vel.y * 0.4 * dt * (m / 40.0).min(3.0));
            water.disturb(x - vel.x.signum() * 0.3, -vel.x.abs() * 0.05 * dt * (m / 40.0).min(3.0));
        }
        fl.submerged = total;
    }
}

/// The drawn water: one mesh per pool, rebuilt from the simulation each frame.
#[derive(Component)]
pub struct WaterMesh {
    pub segment: (usize, usize),
    pub handle: Handle<Mesh>,
}

fn water_colors() -> ([f32; 4], [f32; 4], [f32; 4], [f32; 4], [f32; 4]) {
    let mut surf_front = lin(0x6a9c88);
    surf_front[3] = 0.55;
    let mut surf_back = lin(0x1c3a32);
    surf_back[3] = 0.85;
    let mut front_top = lin(0x4f8a78);
    front_top[3] = 0.26;
    let mut front_bottom = lin(0x0c2219);
    front_bottom[3] = 0.78;
    let mut foam = lin(0xc4ead4);
    foam[3] = 0.6;
    (surf_front, surf_back, front_top, front_bottom, foam)
}

/// Builds the water mesh for columns `[s, e)`: a top sheet from the front edge
/// back to `Z_BACK`, a thin bright line along the front edge, and a front
/// face down to the pool floor.
fn water_mesh(water: &Water, s: usize, e: usize) -> Mesh {
    let (surf_front, surf_back, front_top, front_bottom, foam) = water_colors();
    let n = e - s;
    let mut pos: Vec<[f32; 3]> = Vec::with_capacity(n * 6);
    let mut col: Vec<[f32; 4]> = Vec::with_capacity(n * 6);
    let mut nor: Vec<[f32; 3]> = Vec::with_capacity(n * 6);
    for i in s..e {
        let x = i as f32 * COL;
        let g = water.ground[i].max(FLOOR);
        let depth = (WATER_Y - g).max(0.0);
        let k = (depth / 8.0).clamp(0.0, 1.0);
        // Top sheet: front, back.
        pos.push([x, 0.0, Z_WATER_FRONT]);
        pos.push([x, 0.0, Z_BACK]);
        col.push(surf_front);
        col.push(surf_back);
        // Foam line.
        pos.push([x, 0.0, Z_WATER_FRONT + 0.01]);
        pos.push([x, 0.0, Z_WATER_FRONT + 0.01]);
        col.push(foam);
        col.push(foam);
        // Front face: surface, floor.
        pos.push([x, 0.0, Z_WATER_FRONT]);
        pos.push([x, g, Z_WATER_FRONT]);
        col.push(front_top);
        col.push(mix(front_top, front_bottom, 0.3 + 0.7 * k));
        for _ in 0..4 {
            nor.push([0.0, 1.0, 0.0]);
        }
        nor.push([0.0, 0.0, 1.0]);
        nor.push([0.0, 0.0, 1.0]);
    }
    let mut idx = vec![];
    for j in 0..n as u32 - 1 {
        let a = j * 6;
        let b = a + 6;
        // Top sheet (double-sided material).
        idx.extend_from_slice(&[a, b, b + 1, a, b + 1, a + 1]);
        // Front face.
        idx.extend_from_slice(&[a + 4, a + 5, b + 5, a + 4, b + 5, b + 4]);
        // Foam.
        idx.extend_from_slice(&[a + 3, a + 2, b + 2, a + 3, b + 2, b + 3]);
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

#[derive(Resource)]
pub struct WaterMaterials {
    pub surface: Handle<StandardMaterial>,
    pub murk: Handle<StandardMaterial>,
    pub far: Handle<StandardMaterial>,
}

pub fn setup_materials(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(WaterMaterials {
        surface: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.06,
            reflectance: 0.7,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            double_sided: true,
            ..default()
        }),
        murk: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.9,
            reflectance: 0.1,
            ..default()
        }),
        far: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.12,
            reflectance: 0.6,
            ..default()
        }),
    });
}

/// Spawns the water meshes, the murky back wall of each pool, and the dark
/// water that stretches away behind the playfield.
pub fn spawn(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &WaterMaterials, water: &Water, len: f32) {
    for &(s, e) in &water.segments {
        let handle = meshes.add(water_mesh(water, s, e));
        commands.spawn((
            Mesh3d(handle.clone()),
            MeshMaterial3d(mats.surface.clone()),
            Transform::IDENTITY,
            NotShadowCaster,
            NotShadowReceiver,
            WaterMesh {
                segment: (s, e),
                handle,
            },
            LevelEntity,
        ));
        // The back wall: what you see behind you when under water.
        let mut b = MeshBuilder::new();
        let (x0, x1) = (s as f32 * COL, (e - 1) as f32 * COL);
        let top = lin(0x1d3b33);
        let bottom = lin(0x040a08);
        let steps = 6;
        for k in 0..steps {
            let y0 = WATER_Y + FLOOR * k as f32 / steps as f32;
            let y1 = WATER_Y + FLOOR * (k + 1) as f32 / steps as f32;
            let c0 = mix(top, bottom, (k as f32 / steps as f32).powf(0.7));
            let c1 = mix(top, bottom, ((k + 1) as f32 / steps as f32).powf(0.7));
            let i0 = b.v(Vec3::new(x0, y1, Z_BACK), c1);
            let i1 = b.v(Vec3::new(x1, y1, Z_BACK), c1);
            let i2 = b.v(Vec3::new(x1, y0, Z_BACK), c0);
            let i3 = b.v(Vec3::new(x0, y0, Z_BACK), c0);
            b.quad(i0, i1, i2, i3);
        }
        commands.spawn((
            Mesh3d(meshes.add(b.build(false))),
            MeshMaterial3d(mats.murk.clone()),
            Transform::IDENTITY,
            NotShadowCaster,
            LevelEntity,
        ));
    }
    // Far water behind the playfield, out to the horizon.
    let mut b = MeshBuilder::new();
    let near = lin(0x14241f);
    let far = lin(0x1f2f33);
    let (xa, xb) = (-400.0, len + 400.0);
    let i0 = b.v(Vec3::new(xa, WATER_Y - 0.03, Z_BACK), near);
    let i1 = b.v(Vec3::new(xb, WATER_Y - 0.03, Z_BACK), near);
    let i2 = b.v(Vec3::new(xb, WATER_Y - 0.03, -500.0), far);
    let i3 = b.v(Vec3::new(xa, WATER_Y - 0.03, -500.0), far);
    b.quad(i0, i1, i2, i3);
    commands.spawn((
        Mesh3d(meshes.add(b.build(false))),
        MeshMaterial3d(mats.far.clone()),
        Transform::IDENTITY,
        NotShadowCaster,
        LevelEntity,
    ));
}

/// Moves the water mesh vertices to the simulated surface.
pub fn update_meshes(water: Res<Water>, q: Query<&WaterMesh>, mut meshes: ResMut<Assets<Mesh>>) {
    for wm in &q {
        let Some(mut mesh) = meshes.get_mut(&wm.handle) else {
            continue;
        };
        let (s, e) = wm.segment;
        let mut heights = Vec::with_capacity(e - s);
        for i in s..e {
            let x = i as f32 * COL;
            heights.push(water.surface(x).unwrap_or(WATER_Y));
        }
        if let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) {
            for (j, &y) in heights.iter().enumerate() {
                let k = j * 6;
                pos[k][1] = y;
                pos[k + 1][1] = y;
                pos[k + 2][1] = y + 0.025;
                pos[k + 3][1] = y - 0.025;
                pos[k + 4][1] = y;
            }
        }
        if let Some(VertexAttributeValues::Float32x3(nor)) = mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL) {
            for j in 0..heights.len() {
                let l = heights[j.saturating_sub(1)];
                let r = heights[(j + 1).min(heights.len() - 1)];
                let n = Vec3::new(-(r - l) / (2.0 * COL), 1.0, 0.0).normalize();
                nor[j * 6] = n.to_array();
                nor[j * 6 + 1] = n.to_array();
            }
        }
    }
}
