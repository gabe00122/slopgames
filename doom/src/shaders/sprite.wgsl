// Instanced textured quads: world sprites (depth tested against the walls)
// and the 2D overlay (alpha blended). Texture alpha: < 0.45 transparent,
// 0.5..1 emissive amount, 1 regularly lit.

@group(0) @binding(0) var tex: texture_2d_array<f32>;
@group(0) @binding(1) var samp: sampler;

struct QuadIn {
    @location(0) rect: vec4f,    // NDC x0, y0, x1, y1
    @location(1) uv: vec4f,      // u0, v0, u1, v1
    @location(2) color: vec4f,   // light multiplier, alpha
    @location(3) params: vec4f,  // layer, depth
}

struct VsOut {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
    @location(1) color: vec4f,
    @location(2) @interpolate(flat) layer: i32,
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, q: QuadIn) -> VsOut {
    var corners = array<vec2f, 6>(
        vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(0.0, 1.0),
        vec2f(0.0, 1.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0),
    );
    let c = corners[vi];
    var o: VsOut;
    o.pos = vec4f(mix(q.rect.xy, q.rect.zw, c), q.params.y, 1.0);
    o.uv = mix(q.uv.xy, q.uv.zw, c);
    o.color = q.color;
    o.layer = i32(q.params.x);
    return o;
}

fn texel(i: VsOut) -> vec3f {
    let t = textureSample(tex, samp, i.uv, i.layer);
    if (t.a < 0.45) {
        discard;
    }
    let e = clamp((1.0 - t.a) * 2.0, 0.0, 1.0);
    return mix(t.rgb * i.color.rgb, t.rgb, e);
}

@fragment
fn fs_sprite(i: VsOut) -> @location(0) vec4f {
    return vec4f(texel(i), 1.0);
}

@fragment
fn fs_ui(i: VsOut) -> @location(0) vec4f {
    return vec4f(texel(i), i.color.a);
}
