// Upscale the low-resolution scene to the window with "sharp bilinear"
// filtering (crisp pixels without uneven nearest-neighbour columns), then
// apply the damage/pickup tint and a light vignette.

struct Post {
    tint: vec4f,
    size: vec4f,   // source w, h, destination w, h
    flags: vec4f,  // x: output is sRGB (linearise), y: vignette strength
}

@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> post: Post;

struct VsOut {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VsOut {
    let uv = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    var o: VsOut;
    o.pos = vec4f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    o.uv = uv;
    return o;
}

@fragment
fn fs(i: VsOut) -> @location(0) vec4f {
    let src = post.size.xy;
    let scale = post.size.zw / src;
    let texel = i.uv * src;
    let f = fract(texel) - 0.5;
    let snapped = floor(texel) + 0.5 + clamp(f * scale, vec2f(-0.5), vec2f(0.5));
    var c = textureSampleLevel(scene, samp, snapped / src, 0.0).rgb;
    c = mix(c, post.tint.rgb, post.tint.a);
    let q = i.uv - 0.5;
    c *= 1.0 - dot(q, q) * post.flags.y;
    if (post.flags.x > 0.5) {
        c = pow(max(c, vec3f(0.0)), vec3f(2.2));
    }
    return vec4f(c, 1.0);
}
