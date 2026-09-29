//! Vertex format shared by chunk meshes, entities and the weapon viewmodel.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec2, Vec3};

use super::atlas;
use crate::world::Tile;

/// 32-byte vertex.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    /// Face normal (snorm8). A zero normal marks emissive (unlit) geometry.
    pub normal: [i8; 4],
    /// sRGB tint colour; alpha is used for alpha testing.
    pub color: [u8; 4],
    /// x = ambient occlusion, y = skylight, z = crack amount.
    pub light: [u8; 4],
}

impl Vertex {
    pub const ATTRIBS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x2,
        2 => Snorm8x4,
        3 => Unorm8x4,
        4 => Unorm8x4
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

pub fn pack_normal(n: Vec3) -> [i8; 4] {
    let n = n.normalize_or_zero();
    [
        (n.x * 127.0).round() as i8,
        (n.y * 127.0).round() as i8,
        (n.z * 127.0).round() as i8,
        0,
    ]
}

/// CPU-side geometry builder for dynamic meshes (entities, effects, viewmodel).
#[derive(Default)]
pub struct MeshBuilder {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

/// Unit cube faces: (normal, 4 corners CCW from outside).
const CUBE_FACES: [([f32; 3], [[f32; 3]; 4]); 6] = [
    ([1.0, 0.0, 0.0], [[0.5, -0.5, 0.5], [0.5, -0.5, -0.5], [0.5, 0.5, -0.5], [0.5, 0.5, 0.5]]),
    ([-1.0, 0.0, 0.0], [[-0.5, -0.5, -0.5], [-0.5, -0.5, 0.5], [-0.5, 0.5, 0.5], [-0.5, 0.5, -0.5]]),
    ([0.0, 1.0, 0.0], [[-0.5, 0.5, 0.5], [0.5, 0.5, 0.5], [0.5, 0.5, -0.5], [-0.5, 0.5, -0.5]]),
    ([0.0, -1.0, 0.0], [[-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5], [-0.5, -0.5, 0.5]]),
    ([0.0, 0.0, 1.0], [[-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.5, 0.5, 0.5], [-0.5, 0.5, 0.5]]),
    ([0.0, 0.0, -1.0], [[0.5, -0.5, -0.5], [-0.5, -0.5, -0.5], [-0.5, 0.5, -0.5], [0.5, 0.5, -0.5]]),
];

impl MeshBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// A unit cube centred at the origin, transformed by `m`, flat coloured.
    pub fn add_cube(&mut self, m: Mat4, color: [u8; 4], emissive: bool) {
        self.add_cube_tiled(m, color, emissive, Tile::White);
    }

    /// A transformed unit cube textured with a single atlas tile.
    pub fn add_cube_tiled(&mut self, m: Mat4, color: [u8; 4], emissive: bool, tile: Tile) {
        let (uv0, uv1) = atlas::tile_uv(tile);
        let uvs = [
            Vec2::new(uv0.x, uv1.y),
            Vec2::new(uv1.x, uv1.y),
            Vec2::new(uv1.x, uv0.y),
            Vec2::new(uv0.x, uv0.y),
        ];
        for (n, corners) in CUBE_FACES.iter() {
            let normal = if emissive {
                [0, 0, 0, 0]
            } else {
                pack_normal(m.transform_vector3(Vec3::from(*n)))
            };
            let base = self.vertices.len() as u32;
            for (i, c) in corners.iter().enumerate() {
                let p = m.transform_point3(Vec3::from(*c));
                self.vertices.push(Vertex {
                    pos: p.to_array(),
                    uv: uvs[i].to_array(),
                    normal,
                    color,
                    light: [255, 255, 0, 0],
                });
            }
            self.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    /// Axis-aligned box between two corners.
    pub fn add_aabb(&mut self, min: Vec3, max: Vec3, color: [u8; 4], emissive: bool) {
        let center = (min + max) * 0.5;
        let size = (max - min).max(Vec3::splat(1e-4));
        self.add_cube(
            Mat4::from_translation(center) * Mat4::from_scale(size),
            color,
            emissive,
        );
    }

    /// A thin box stretched between two points (tracers, beams).
    pub fn add_beam(&mut self, a: Vec3, b: Vec3, thickness: f32, color: [u8; 4], emissive: bool) {
        let d = b - a;
        let len = d.length();
        if len < 1e-4 {
            return;
        }
        let dir = d / len;
        let rot = glam::Quat::from_rotation_arc(Vec3::Z, dir);
        let m = Mat4::from_scale_rotation_translation(
            Vec3::new(thickness, thickness, len),
            rot,
            (a + b) * 0.5,
        );
        self.add_cube(m, color, emissive);
    }
}
