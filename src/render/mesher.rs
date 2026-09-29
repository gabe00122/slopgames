//! Chunk meshing: face culling, per-vertex ambient occlusion and soft skylight.

use super::atlas::tile_uv;
use super::vertex::Vertex;
use crate::world::{Block, World, CHUNK_SIZE};
use glam::IVec3;

#[derive(Default)]
pub struct ChunkMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

struct FaceDir {
    n: IVec3,
    u: IVec3,
    v: IVec3,
    /// 0 = top tile, 1 = side, 2 = bottom.
    tile_slot: usize,
}

// u x v = n, u = texture right, v = texture up (as seen from outside).
const FACES: [FaceDir; 6] = [
    FaceDir { n: IVec3::new(1, 0, 0), u: IVec3::new(0, 0, -1), v: IVec3::new(0, 1, 0), tile_slot: 1 },
    FaceDir { n: IVec3::new(-1, 0, 0), u: IVec3::new(0, 0, 1), v: IVec3::new(0, 1, 0), tile_slot: 1 },
    FaceDir { n: IVec3::new(0, 1, 0), u: IVec3::new(1, 0, 0), v: IVec3::new(0, 0, -1), tile_slot: 0 },
    FaceDir { n: IVec3::new(0, -1, 0), u: IVec3::new(1, 0, 0), v: IVec3::new(0, 0, 1), tile_slot: 2 },
    FaceDir { n: IVec3::new(0, 0, 1), u: IVec3::new(1, 0, 0), v: IVec3::new(0, 1, 0), tile_slot: 1 },
    FaceDir { n: IVec3::new(0, 0, -1), u: IVec3::new(-1, 0, 0), v: IVec3::new(0, 1, 0), tile_slot: 1 },
];

const CORNERS: [(i32, i32); 4] = [(-1, -1), (1, -1), (1, 1), (-1, 1)];
const AO_CURVE: [f32; 4] = [0.42, 0.62, 0.8, 1.0];

#[inline]
fn face_visible(block: Block, neighbor: Block) -> bool {
    if neighbor.is_air() {
        return true;
    }
    if neighbor.is_opaque() {
        return false;
    }
    // Transparent neighbour: hide faces between identical transparent blocks.
    neighbor != block
}

pub fn mesh_chunk(world: &World, cpos: IVec3) -> ChunkMesh {
    let mut mesh = ChunkMesh::default();
    let Some(chunk) = world.chunk(cpos) else {
        return mesh;
    };
    if chunk.is_empty() {
        return mesh;
    }
    let origin = cpos * CHUNK_SIZE;
    for ly in 0..CHUNK_SIZE {
        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let l = IVec3::new(lx, ly, lz);
                let block = chunk.get(l);
                if block.is_air() {
                    continue;
                }
                let p = origin + l;
                let info = block.info();
                let crack = world.damage_frac(p);
                let crack_u8 = if crack > 0.0 { (crack * 255.0) as u8 } else { 0 };
                for face in FACES.iter() {
                    let np = p + face.n;
                    let neighbor = world.get(np);
                    if !face_visible(block, neighbor) {
                        continue;
                    }
                    let (uv0, uv1) = tile_uv(info.tiles[face.tile_slot]);
                    let base = mesh.vertices.len() as u32;
                    let mut ao_vals = [0f32; 4];
                    for (ci, (a, b)) in CORNERS.iter().enumerate() {
                        let su = face.u * *a;
                        let sv = face.v * *b;
                        // Ambient occlusion from the three blocks around this corner.
                        let s1 = world.get(np + su).is_opaque();
                        let s2 = world.get(np + sv).is_opaque();
                        let cc = world.get(np + su + sv).is_opaque();
                        let level = if s1 && s2 {
                            0
                        } else {
                            3 - (s1 as usize + s2 as usize + cc as usize)
                        };
                        let ao = if info.emissive { 1.0 } else { AO_CURVE[level] };
                        ao_vals[ci] = ao;

                        // Soft skylight: average sky exposure of the open cells at this corner.
                        let mut sky_sum = 0.0;
                        let mut sky_n = 0.0;
                        for (cell, open) in [
                            (np, true),
                            (np + su, !s1),
                            (np + sv, !s2),
                            (np + su + sv, !cc && !(s1 && s2)),
                        ] {
                            if open {
                                sky_n += 1.0;
                                if world.sky_at(cell) {
                                    sky_sum += 1.0;
                                }
                            }
                        }
                        let sky = if sky_n > 0.0 { sky_sum / sky_n } else { 0.0 };

                        let corner = p.as_vec3()
                            + glam::Vec3::splat(0.5)
                            + face.n.as_vec3() * 0.5
                            + su.as_vec3() * 0.5
                            + sv.as_vec3() * 0.5;
                        let tu = (*a as f32 + 1.0) * 0.5;
                        let tv = (1.0 - *b as f32) * 0.5;
                        let normal = if info.emissive {
                            [0, 0, 0, 0]
                        } else {
                            [face.n.x as i8 * 127, face.n.y as i8 * 127, face.n.z as i8 * 127, 0]
                        };
                        mesh.vertices.push(Vertex {
                            pos: corner.to_array(),
                            uv: [uv0.x + (uv1.x - uv0.x) * tu, uv0.y + (uv1.y - uv0.y) * tv],
                            normal,
                            color: [255, 255, 255, 255],
                            light: [(ao * 255.0) as u8, (sky * 255.0) as u8, crack_u8, 0],
                        });
                    }
                    // Flip the quad diagonal to avoid AO interpolation artifacts.
                    if ao_vals[0] + ao_vals[2] < ao_vals[1] + ao_vals[3] {
                        mesh.indices.extend_from_slice(&[
                            base + 1,
                            base + 2,
                            base + 3,
                            base + 1,
                            base + 3,
                            base,
                        ]);
                    } else {
                        mesh.indices.extend_from_slice(&[
                            base,
                            base + 1,
                            base + 2,
                            base,
                            base + 2,
                            base + 3,
                        ]);
                    }
                }
            }
        }
    }
    mesh
}

/// Mesh many chunks in parallel using scoped threads.
pub fn mesh_chunks_parallel(world: &World, coords: &[IVec3]) -> Vec<(IVec3, ChunkMesh)> {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 16);
    let per = coords.len().div_ceil(threads).max(1);
    let mut out = Vec::with_capacity(coords.len());
    std::thread::scope(|s| {
        let handles: Vec<_> = coords
            .chunks(per)
            .map(|part| {
                s.spawn(move || {
                    part.iter()
                        .map(|c| (*c, mesh_chunk(world, *c)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for h in handles {
            if let Ok(v) = h.join() {
                out.extend(v);
            }
        }
    });
    out
}
