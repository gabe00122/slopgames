// One invocation per screen column: DDA through the grid until a wall or a
// door panel is hit. Results feed the wall/floor pass and double as a depth
// buffer for sprites (written there as frag_depth).

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var<storage, read> cells: array<u32>;
@group(0) @binding(2) var<storage, read> doors: array<f32>;
@group(0) @binding(3) var<storage, read_write> hits: array<Hit>;

fn cell_light(x: i32, y: i32) -> f32 {
    let w = i32(g.misc.x);
    return unpack_light(cells[y * w + x], x, y, g.view.w);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3u) {
    let col = gid.x;
    if (col >= u32(g.view.x)) {
        return;
    }
    let mw = i32(g.misc.x);
    let mh = i32(g.misc.y);
    let pos = g.cam.xy;
    let ndc_x = (f32(col) + 0.5) / g.view.x * 2.0 - 1.0;
    // Forward component is 1, so the DDA parameter is the perpendicular distance.
    let ray = g.cam.zw + g.right_tan.xy * (ndc_x * g.right_tan.z);

    var map = vec2i(floor(pos));
    let dd = vec2f(
        select(abs(1.0 / ray.x), 1e30, abs(ray.x) < 1e-9),
        select(abs(1.0 / ray.y), 1e30, abs(ray.y) < 1e-9),
    );
    let stp = vec2i(select(1, -1, ray.x < 0.0), select(1, -1, ray.y < 0.0));
    var side_d = vec2f(
        select((f32(map.x) + 1.0 - pos.x) * dd.x, (pos.x - f32(map.x)) * dd.x, ray.x < 0.0),
        select((f32(map.y) + 1.0 - pos.y) * dd.y, (pos.y - f32(map.y)) * dd.y, ray.y < 0.0),
    );

    var hit: Hit;
    hit.dist = FAR;
    hit.u = 0.0;
    hit.tex = 1u;
    hit.side = 0u;
    hit.light = 0.5;
    hit.hx = pos.x + ray.x * FAR;
    hit.hy = pos.y + ray.y * FAR;
    hit.pad = 0.0;

    if (map.x < 0 || map.y < 0 || map.x >= mw || map.y >= mh) {
        hits[col] = hit;
        return;
    }

    var t_enter = 0.0;
    var side = 0u;
    var prev_door = false;
    var last_light = cell_light(map.x, map.y);

    for (var i = 0; i < 256; i++) {
        let idx = map.y * mw + map.x;
        let dv = doors[idx];
        prev_door = dv >= 0.0;
        if (prev_door) {
            // Sliding door panel across the middle of the cell.
            let plane_x = dv >= 2.0;
            let open = select(dv, dv - 2.0, plane_x);
            var t = -1.0;
            var along = 0.0;
            if (plane_x && abs(ray.x) > 1e-9) {
                t = (f32(map.x) + 0.5 - pos.x) / ray.x;
                along = pos.y + ray.y * t - f32(map.y);
            } else if (!plane_x && abs(ray.y) > 1e-9) {
                t = (f32(map.y) + 0.5 - pos.y) / ray.y;
                along = pos.x + ray.x * t - f32(map.x);
            }
            if (t >= t_enter && t <= min(side_d.x, side_d.y) && along >= open && along <= 1.0) {
                hit.dist = t;
                hit.u = along - open;
                hit.tex = (cells[idx] & 0xffu) - 1u;
                hit.side = select(1u, 0u, plane_x);
                hit.light = cell_light(map.x, map.y);
                hit.hx = pos.x + ray.x * t;
                hit.hy = pos.y + ray.y * t;
                break;
            }
        }

        if (side_d.x < side_d.y) {
            t_enter = side_d.x;
            side_d.x += dd.x;
            map.x += stp.x;
            side = 0u;
        } else {
            t_enter = side_d.y;
            side_d.y += dd.y;
            map.y += stp.y;
            side = 1u;
        }
        if (map.x < 0 || map.y < 0 || map.x >= mw || map.y >= mh || t_enter > FAR) {
            hit.dist = min(t_enter, FAR);
            break;
        }

        let nidx = map.y * mw + map.x;
        let wall = cells[nidx] & 0xffu;
        if (wall != 0u && doors[nidx] < 0.0) {
            var wx: f32;
            if (side == 0u) {
                wx = pos.y + t_enter * ray.y;
            } else {
                wx = pos.x + t_enter * ray.x;
            }
            var u = fract(wx);
            // Flip so textures always read left to right.
            if ((side == 0u && ray.x < 0.0) || (side == 1u && ray.y > 0.0)) {
                u = 1.0 - u;
            }
            hit.dist = t_enter;
            hit.u = u;
            hit.tex = select(wall - 1u, TEX_DOORTRAK, prev_door);
            hit.side = side;
            hit.light = last_light;
            hit.hx = pos.x + ray.x * t_enter;
            hit.hy = pos.y + ray.y * t_enter;
            break;
        }
        last_light = cell_light(map.x, map.y);
    }
    hits[col] = hit;
}
