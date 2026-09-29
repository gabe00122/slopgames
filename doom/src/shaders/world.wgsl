// Fullscreen pass: walls from the per-column hits, floors and ceilings by
// inverse projection per pixel, sky where the ceiling is open.

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var<storage, read> cells: array<u32>;
@group(0) @binding(2) var<storage, read> hits: array<Hit>;
@group(0) @binding(3) var surf: texture_2d_array<f32>;
@group(0) @binding(4) var sky: texture_2d<f32>;
@group(0) @binding(5) var samp: sampler;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let uv = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4f(uv * 2.0 - 1.0, 0.0, 1.0);
}

struct FsOut {
    @location(0) color: vec4f,
    @builtin(frag_depth) depth: f32,
}

fn dyn_light(p: vec3f) -> vec3f {
    var acc = vec3f(0.0);
    let n = u32(g.misc.z);
    for (var i = 0u; i < n; i++) {
        let l = g.lights[i];
        let k = max(1.0 - distance(p, l.pos_r.xyz) / l.pos_r.w, 0.0);
        acc += l.color.rgb * (k * k);
    }
    return acc;
}

fn shade(tc: vec4f, level: f32, dynamic: vec3f) -> vec3f {
    let lit = tc.rgb * min(vec3f(level) + dynamic, vec3f(1.6));
    // Texture alpha < 1 marks emissive texels.
    return mix(lit, tc.rgb, 1.0 - tc.a);
}

@fragment
fn fs(@builtin(position) fc: vec4f) -> FsOut {
    let vw = g.view.x;
    let vh = g.view.y;
    let eye = g.view.z;
    let time = g.view.w;
    let tan_h = g.right_tan.z;
    let tan_v = g.right_tan.w;
    let h = hits[min(u32(fc.x), u32(vw) - 1u)];
    let ndc_x = fc.x / vw * 2.0 - 1.0;
    let ndc_y = 1.0 - fc.y / vh * 2.0;
    let ray = g.cam.zw + g.right_tan.xy * (ndc_x * tan_h);

    let top = (1.0 - eye) / (h.dist * tan_v);
    let bot = (0.0 - eye) / (h.dist * tan_v);
    var out: FsOut;
    out.depth = 1.0;

    if (ndc_y <= top && ndc_y >= bot) {
        let v = (top - ndc_y) / (top - bot);
        let lod = log2(max(64.0 * h.dist * tan_v * 2.0 / vh, 1e-4));
        let tc = textureSampleLevel(surf, samp, vec2f(h.u, v), i32(h.tex), max(lod, 0.0));
        let z = eye + ndc_y * h.dist * tan_v;
        let level = light_level(h.light, h.dist) * select(1.0, 0.8, h.side == 1u);
        out.color = vec4f(shade(tc, level, dyn_light(vec3f(h.hx, h.hy, z))), 1.0);
        out.depth = h.dist / FAR;
        return out;
    }

    let is_floor = ndc_y < bot;
    let plane = select(1.0 - eye, -eye, is_floor);
    let d = plane / (ndc_y * tan_v);
    let wp = g.cam.xy + ray * d;
    let cx = i32(floor(wp.x));
    let cy = i32(floor(wp.y));
    let mw = i32(g.misc.x);
    let mh = i32(g.misc.y);
    var c = 0x80000000u;
    if (cx >= 0 && cy >= 0 && cx < mw && cy < mh) {
        c = cells[cy * mw + cx];
    }
    let floor_tex = (c >> 8u) & 0x7fu;
    let ceil_tex = (c >> 16u) & 0xffu;

    if (!is_floor && ceil_tex == SKY) {
        let su = atan2(ray.y, ray.x) / 6.2831853 * 4.0;
        let sv = clamp(1.0 - ndc_y * 0.95, 0.0, 0.999);
        out.color = vec4f(textureSampleLevel(sky, samp, vec2f(su, sv), 0.0).rgb, 1.0);
        return out;
    }

    let tex = select(ceil_tex, floor_tex, is_floor);
    var uv = wp;
    if (tex == F_NUKAGE || tex == F_LAVA) {
        let speed = select(0.08, 0.04, tex == F_LAVA);
        uv += vec2f(sin(wp.y * 2.5 + time * 1.7), cos(wp.x * 2.5 + time * 1.3)) * 0.035 + vec2f(time * speed, time * speed * 0.6);
    }
    let across = d * 2.0 * tan_h / vw;
    let along = d * d * tan_v * 2.0 / (vh * abs(plane));
    let lod = log2(max(64.0 * max(across, along * 0.4), 1e-4));
    let tc = textureSampleLevel(surf, samp, fract(uv), i32(tex), max(lod, 0.0));
    let level = light_level(unpack_light(c, cx, cy, time), d);
    let z = select(1.0, 0.0, is_floor);
    out.color = vec4f(shade(tc, level, dyn_light(vec3f(wp, z))), 1.0);
    return out;
}
