// Voxel / entity shader: directional sun + skylight + AO, alpha test,
// procedural crack overlay for damaged blocks, and distance fog.

struct Globals {
    view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    sun_dir: vec4<f32>,     // xyz = direction towards the sun
    fog_color: vec4<f32>,   // linear rgb
    fog: vec4<f32>,         // x = start, y = end, z = time, w = ambient boost
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var atlas_tex: texture_2d<f32>;
@group(1) @binding(1) var atlas_samp: sampler;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) normal: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) light: vec4<f32>,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) light: vec3<f32>,
    @location(4) world_pos: vec3<f32>,
};

@vertex
fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    o.clip = g.view_proj * vec4<f32>(v.pos, 1.0);
    o.uv = v.uv;
    o.normal = v.normal.xyz;
    o.color = v.color;
    o.light = v.light.xyz;
    o.world_pos = v.pos;
    return o;
}

fn hash21(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(123.34, 456.21));
    q = q + dot(q, q + 45.32);
    return fract(q.x * q.y);
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    // Manual LOD + clamping inside the tile so mipmapped filtering never
    // bleeds neighbouring atlas tiles into block edges.
    let tile_size = 1.0 / 16.0;
    let tile_origin = floor(in.uv / tile_size) * tile_size;
    let texel = in.uv * 256.0;
    let rho = max(length(dpdx(texel)), length(dpdy(texel)));
    let lod = clamp(log2(max(rho, 1e-6)), 0.0, 4.0);
    let margin = 0.5 * exp2(ceil(lod)) / 256.0;
    let uv = clamp(in.uv, tile_origin + vec2<f32>(margin), tile_origin + vec2<f32>(tile_size - margin));
    let tex = textureSampleLevel(atlas_tex, atlas_samp, uv, lod);
    if (tex.a * in.color.a < 0.5) {
        discard;
    }
    // Vertex colours are sRGB; convert to linear.
    let tint = pow(in.color.rgb, vec3<f32>(2.2));
    var base = tex.rgb * tint;

    // Crack overlay: darken a procedural set of texels as damage increases.
    let crack = in.light.z;
    if (crack > 0.01) {
        let tile_px = floor(fract(in.uv * 16.0) * 16.0);
        let cell = floor(tile_px / 2.0);
        let h = hash21(cell + floor(in.uv * 16.0) * 17.0);
        let line_mask = abs(fract((tile_px.x + tile_px.y * 0.7) / 5.0) - 0.5) < 0.12;
        let mask = select(0.0, 1.0, h < crack * 0.8 || (line_mask && h < crack * 1.4));
        base = base * (1.0 - 0.65 * mask);
    }

    var lit = 1.0;
    let nlen = length(in.normal);
    if (nlen > 0.5) {
        let n = in.normal / nlen;
        let sun = max(dot(n, g.sun_dir.xyz), 0.0);
        let sky = in.light.y;
        let ao = in.light.x;
        // Side faces get slightly different ambient so geometry reads well.
        let face_amb = 0.9 + 0.1 * n.y;
        lit = ao * (face_amb * (0.28 + g.fog.w + 0.32 * sky) + 0.62 * sun * sky);
    } else {
        lit = 1.25;
    }
    var col = base * lit;

    let d = distance(in.world_pos, g.cam_pos.xyz);
    let f = clamp((d - g.fog.x) / max(g.fog.y - g.fog.x, 0.001), 0.0, 1.0);
    col = mix(col, g.fog_color.rgb, f * f);
    return vec4<f32>(col, 1.0);
}
