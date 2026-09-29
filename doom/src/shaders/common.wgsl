// Shared declarations. Constants such as FAR, TEX_DOORTRAK and SKY are
// prepended by the renderer from the Rust definitions.

struct Light {
    pos_r: vec4f,  // xyz position, w radius
    color: vec4f,
}

struct Globals {
    cam: vec4f,        // pos.xy, dir.xy
    right_tan: vec4f,  // right.xy, tan_h, tan_v
    view: vec4f,       // view width, view height, eye height, time
    misc: vec4f,       // map width, map height, light count, unused
    lights: array<Light, MAX_LIGHTS>,
}

// One entry per screen column, written by the raycast compute pass.
struct Hit {
    dist: f32,
    u: f32,
    tex: u32,
    side: u32,
    light: f32,
    hx: f32,
    hy: f32,
    pad: f32,
}

// Must match `math::hash2` on the CPU (sector light flicker).
fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    var h = (bitcast<u32>(x) * 0x8da6b343u) ^ (bitcast<u32>(y) * 0xd8163841u) ^ (seed * 0xcb1ab31fu);
    h ^= h >> 13u;
    h *= 0x5bd1e995u;
    h ^= h >> 15u;
    return f32(h & 0x00ffffffu) / 16777216.0;
}

fn unpack_light(c: u32, x: i32, y: i32, time: f32) -> f32 {
    var l = f32(c >> 24u) / 255.0;
    if ((c & 0x8000u) != 0u && hash2(x, y, u32(time * 10.0)) > 0.6) {
        l *= 0.45;
    }
    return l;
}

// Doom-style diminishing light, quantised into bands like a colormap.
fn light_level(sector: f32, dist: f32) -> f32 {
    let fall = 1.0 / (1.0 + 0.018 * dist * dist);
    return floor(sector * (0.3 + 0.85 * fall) * 20.0 + 0.5) / 20.0;
}
