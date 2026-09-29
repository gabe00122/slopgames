//! Procedural sprite art: monsters, projectiles, pickups, decorations, the
//! first-person weapons, the status-bar face and the font atlas.
//!
//! Everything is painted into 128x128 layers of one texture array. For world
//! sprites a layer spans one map unit (the wall height) and the bottom row sits
//! on the floor. Alpha < 0.5 is transparent; alpha in [0.5, 1) marks emissive
//! ("fullbright") pixels, alpha 1 is regularly lit.

use crate::font;
use crate::math::{hash2, Rng};
use crate::textures::Image;

pub const SPR: usize = 128;

type Rgb = [f32; 3];

fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

fn scale(c: Rgb, s: f32) -> Rgb {
    [c[0] * s, c[1] * s, c[2] * s]
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn solid(c: Rgb) -> [f32; 4] {
    [c[0], c[1], c[2], 1.0]
}

/// Emissive pixel; `e` = 1 is fully bright regardless of sector light.
fn glow(c: Rgb, e: f32) -> [f32; 4] {
    [c[0], c[1], c[2], 1.0 - 0.5 * e.clamp(0.0, 1.0)]
}

// ---------------------------------------------------------------------------
// Painter primitives
// ---------------------------------------------------------------------------

const LIGHT: [f32; 3] = [-0.42, -0.55, 0.72];

fn shade_px(c: Rgb, n: [f32; 3], shade: f32) -> [f32; 4] {
    let l = (n[0] * LIGHT[0] + n[1] * LIGHT[1] + n[2] * LIGHT[2]).max(0.0);
    let b = 1.0 + shade * (0.3 + 0.9 * l - 1.0);
    solid(scale(c, b))
}

fn put(img: &mut Image, x: i32, y: i32, c: [f32; 4]) {
    if x >= 0 && y >= 0 && (x as usize) < img.w && (y as usize) < img.h {
        let i = y as usize * img.w + x as usize;
        img.px[i] = c;
    }
}

fn opaque(img: &Image, x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && (x as usize) < img.w && (y as usize) < img.h && img.get(x as usize, y as usize)[3] >= 0.5
}

fn bbox(img: &Image, x0: f32, y0: f32, x1: f32, y1: f32) -> (i32, i32, i32, i32) {
    (
        (x0.floor() as i32).max(0),
        (y0.floor() as i32).max(0),
        (x1.ceil() as i32).min(img.w as i32 - 1),
        (y1.ceil() as i32).min(img.h as i32 - 1),
    )
}

/// Pseudo-3D shaded ellipse (a sphere seen from the front).
fn ellipse(img: &mut Image, cx: f32, cy: f32, rx: f32, ry: f32, col: Rgb, shade: f32) {
    let (x0, y0, x1, y1) = bbox(img, cx - rx, cy - ry, cx + rx, cy + ry);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = (x as f32 + 0.5 - cx) / rx;
            let dy = (y as f32 + 0.5 - cy) / ry;
            let d2 = dx * dx + dy * dy;
            if d2 <= 1.0 {
                put(img, x, y, shade_px(col, [dx, dy, (1.0 - d2).sqrt()], shade));
            }
        }
    }
}

/// Flat ellipse with an explicit pixel value (used for dark holes, glows...).
fn ellipse_flat(img: &mut Image, cx: f32, cy: f32, rx: f32, ry: f32, c: [f32; 4]) {
    let (x0, y0, x1, y1) = bbox(img, cx - rx, cy - ry, cx + rx, cy + ry);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = (x as f32 + 0.5 - cx) / rx;
            let dy = (y as f32 + 0.5 - cy) / ry;
            if dx * dx + dy * dy <= 1.0 {
                put(img, x, y, c);
            }
        }
    }
}

/// Radial glowing blob, fully emissive, `inner` at the center fading to `outer`.
fn fireball(img: &mut Image, cx: f32, cy: f32, r: f32, inner: Rgb, outer: Rgb, seed: u32) {
    let (x0, y0, x1, y1) = bbox(img, cx - r * 1.3, cy - r * 1.3, cx + r * 1.3, cy + r * 1.3);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let ang = dy.atan2(dx);
            let wobble = 1.0 + 0.22 * ((ang * 5.0 + seed as f32).sin() * 0.6 + hash2(x, y, seed) * 0.4);
            let d = (dx * dx + dy * dy).sqrt() / (r * wobble);
            if d <= 1.0 {
                put(img, x, y, glow(mix(inner, outer, d.powf(1.5)), 1.0));
            }
        }
    }
}

/// Tapered capsule from a to b, shaded like a cylinder.
#[allow(clippy::too_many_arguments)]
fn limb(img: &mut Image, ax: f32, ay: f32, bx: f32, by: f32, ra: f32, rb: f32, col: Rgb, shade: f32) {
    let r = ra.max(rb);
    let (x0, y0, x1, y1) = bbox(img, ax.min(bx) - r, ay.min(by) - r, ax.max(bx) + r, ay.max(by) + r);
    let (vx, vy) = (bx - ax, by - ay);
    let len2 = (vx * vx + vy * vy).max(1e-4);
    let inv = 1.0 / len2.sqrt();
    let (nx, ny) = (-vy * inv, vx * inv);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (px, py) = (x as f32 + 0.5 - ax, y as f32 + 0.5 - ay);
            let t = ((px * vx + py * vy) / len2).clamp(0.0, 1.0);
            let rr = ra + (rb - ra) * t;
            let (qx, qy) = (px - vx * t, py - vy * t);
            let d = (qx * qx + qy * qy).sqrt();
            if d <= rr {
                let s = ((qx * nx + qy * ny) / rr).clamp(-1.0, 1.0);
                let n = [s * nx, s * ny, (1.0 - s * s).max(0.0).sqrt()];
                put(img, x, y, shade_px(col, n, shade));
            }
        }
    }
}

/// Even-odd polygon fill.
fn poly(img: &mut Image, pts: &[(f32, f32)], c: [f32; 4]) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &(x, y) in pts {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let (bx0, by0, bx1, by1) = bbox(img, x0, y0, x1, y1);
    for y in by0..=by1 {
        for x in bx0..=bx1 {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut inside = false;
            let mut j = pts.len() - 1;
            for i in 0..pts.len() {
                let (xi, yi) = pts[i];
                let (xj, yj) = pts[j];
                if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
                    inside = !inside;
                }
                j = i;
            }
            if inside {
                put(img, x, y, c);
            }
        }
    }
}

fn rect(img: &mut Image, x0: f32, y0: f32, x1: f32, y1: f32, c: [f32; 4]) {
    for y in (y0.round() as i32)..(y1.round() as i32) {
        for x in (x0.round() as i32)..(x1.round() as i32) {
            put(img, x, y, c);
        }
    }
}

/// Vertical gradient box with bevelled edges — crates, boxes, gun parts.
fn block(img: &mut Image, x0: f32, y0: f32, x1: f32, y1: f32, col: Rgb) {
    let (xa, ya, xb, yb) = (x0.round() as i32, y0.round() as i32, x1.round() as i32, y1.round() as i32);
    for y in ya..yb {
        for x in xa..xb {
            let mut s = 1.0 - 0.25 * (y - ya) as f32 / (yb - ya).max(1) as f32;
            if y == ya || x == xa {
                s *= 1.3;
            }
            if y == yb - 1 || x == xb - 1 {
                s *= 0.6;
            }
            put(img, x, y, solid(scale(col, s)));
        }
    }
}

/// Dark 1px outline around opaque regions, the classic sprite look.
fn outline(img: &mut Image) {
    let src = img.clone();
    for y in 0..img.h as i32 {
        for x in 0..img.w as i32 {
            if opaque(&src, x, y) {
                continue;
            }
            let mut acc = [0.0f32; 3];
            let mut n = 0.0;
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                if opaque(&src, x + dx, y + dy) {
                    let p = src.get((x + dx) as usize, (y + dy) as usize);
                    if p[3] > 0.99 {
                        acc = [acc[0] + p[0], acc[1] + p[1], acc[2] + p[2]];
                        n += 1.0;
                    }
                }
            }
            if n > 0.0 {
                put(img, x, y, solid(scale(acc, 0.28 / n)));
            }
        }
    }
}

/// Multiplicative per-pixel noise on lit pixels for a gritty, hand-drawn feel.
fn grit(img: &mut Image, amount: f32, seed: u32) {
    for y in 0..img.h {
        for x in 0..img.w {
            let p = img.get(x, y);
            if p[3] > 0.99 {
                let s = 1.0 - amount + 2.0 * amount * hash2(x as i32, y as i32, seed);
                img.set(x, y, [p[0] * s, p[1] * s, p[2] * s, 1.0]);
            }
        }
    }
}

fn tint(img: &mut Image, c: Rgb, t: f32) {
    for p in img.px.iter_mut() {
        if p[3] >= 0.5 {
            let m = mix([p[0], p[1], p[2]], c, t);
            *p = [m[0], m[1], m[2], p[3]];
        }
    }
}

fn blank() -> Image {
    Image::new(SPR, SPR)
}

/// Draws `src` onto `dst` (opaque pixels only) with an offset.
fn blit(dst: &mut Image, src: &Image, ox: i32, oy: i32) {
    for y in 0..src.h as i32 {
        for x in 0..src.w as i32 {
            let p = src.get(x as usize, y as usize);
            if p[3] >= 0.5 {
                put(dst, x + ox, y + oy, p);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Monsters
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Species {
    Zombie,
    Imp,
}

#[derive(Clone, Copy)]
struct Pose {
    bob: f32,
    lean: f32,
    lfoot: (f32, f32),
    rfoot: (f32, f32),
    lhand: (f32, f32),
    rhand: (f32, f32),
    mouth: f32,
    /// Radius of a fireball held in the right hand (imp), 0 = none.
    held_ball: f32,
    flash: bool,
}

impl Pose {
    fn stand(species: Species) -> Pose {
        let (lh, rh) = match species {
            Species::Zombie => ((58.0, 80.0), (70.0, 80.0)),
            Species::Imp => ((44.0, 90.0), (84.0, 90.0)),
        };
        Pose {
            bob: 0.0,
            lean: 0.0,
            lfoot: (55.0, 125.0),
            rfoot: (73.0, 125.0),
            lhand: lh,
            rhand: rh,
            mouth: 0.0,
            held_ball: 0.0,
            flash: false,
        }
    }

    fn walk(species: Species, frame: usize) -> Pose {
        let mut p = Pose::stand(species);
        let swing = match species {
            Species::Zombie => 1.5,
            Species::Imp => 4.0,
        };
        match frame {
            1 => {
                p.lfoot = (53.0, 116.0);
                p.bob = -1.5;
                p.lhand.1 += swing;
                p.rhand.1 -= swing;
            }
            3 => {
                p.rfoot = (75.0, 116.0);
                p.bob = -1.5;
                p.lhand.1 -= swing;
                p.rhand.1 += swing;
            }
            _ => p.bob = 1.0,
        }
        p
    }
}

struct Palette {
    skin: Rgb,
    torso: Rgb,
    legs: Rgb,
    feet: Rgb,
    hair: Rgb,
    eyes: Rgb,
}

fn knee(hip: (f32, f32), foot: (f32, f32), out: f32) -> (f32, f32) {
    let raise = (125.0 - foot.1).max(0.0);
    ((hip.0 + foot.0) * 0.5 + out * (2.5 + raise * 0.5), (hip.1 + foot.1) * 0.5 - raise * 0.35)
}

fn elbow(sh: (f32, f32), hand: (f32, f32), out: f32) -> (f32, f32) {
    ((sh.0 + hand.0) * 0.5 + out * 6.0, (sh.1 + hand.1) * 0.5 + 1.0)
}

fn spike(img: &mut Image, x: f32, y: f32, dx: f32, dy: f32, w: f32) {
    let (nx, ny) = (-dy, dx);
    let l = (nx * nx + ny * ny).sqrt().max(1e-3);
    let (nx, ny) = (nx / l * w, ny / l * w);
    poly(img, &[(x - nx, y - ny), (x + dx, y + dy), (x + nx, y + ny)], solid(rgb(222, 206, 170)));
}

fn draw_humanoid(img: &mut Image, sp: Species, pal: &Palette, p: &Pose) {
    let cx = 64.0 + p.lean;
    let hip_y = 86.0 + p.bob;
    let sh_y = 58.0 + p.bob;
    let lhip = (57.0 + p.lean * 0.3, hip_y);
    let rhip = (71.0 + p.lean * 0.3, hip_y);
    let lsh = (49.0 + p.lean, sh_y);
    let rsh = (79.0 + p.lean, sh_y);
    let thick = if sp == Species::Imp { 1.1 } else { 1.0 };

    // Legs.
    for (hip, foot, out) in [(lhip, p.lfoot, -1.0), (rhip, p.rfoot, 1.0)] {
        let k = knee(hip, foot, out);
        limb(img, hip.0, hip.1, k.0, k.1, 6.5 * thick, 5.5 * thick, pal.legs, 1.0);
        limb(img, k.0, k.1, foot.0, foot.1 - 4.0, 5.5 * thick, 4.5 * thick, pal.legs, 1.0);
        ellipse(img, foot.0 + out * 1.0, foot.1 - 2.5, 6.0, 3.5, pal.feet, 0.9);
        if sp == Species::Imp {
            for c in [-3.0, 0.0, 3.0] {
                spike(img, foot.0 + c + out, foot.1 - 1.0, c * 0.4, 2.5, 1.0);
            }
        }
    }

    // Torso.
    match sp {
        Species::Zombie => {
            ellipse(img, cx, 72.0 + p.bob, 15.5, 17.0, pal.torso, 1.0);
            // Armor plates and belt.
            ellipse(img, cx - 6.0, 66.0 + p.bob, 5.5, 6.0, scale(pal.torso, 1.12), 0.8);
            ellipse(img, cx + 6.0, 66.0 + p.bob, 5.5, 6.0, scale(pal.torso, 1.12), 0.8);
            rect(img, cx - 14.0, 83.0 + p.bob, cx + 14.0, 87.0 + p.bob, solid(rgb(56, 40, 26)));
            rect(img, cx - 2.0, 83.0 + p.bob, cx + 2.0, 87.0 + p.bob, solid(rgb(170, 150, 90)));
        }
        Species::Imp => {
            ellipse(img, cx, 72.0 + p.bob, 14.5, 17.0, pal.torso, 1.0);
            // Abs and pecs.
            for (dx, dy) in [(-5.0, -8.0), (5.0, -8.0)] {
                ellipse(img, cx + dx, 72.0 + dy + p.bob, 6.0, 4.5, scale(pal.torso, 1.1), 0.9);
            }
            for row in 0..3 {
                for s in [-1.0, 1.0] {
                    ellipse(img, cx + s * 3.2, 72.0 + p.bob + row as f32 * 4.5, 2.8, 2.0, scale(pal.torso, 1.05), 0.9);
                }
            }
        }
    }

    // Shoulders and arms.
    for (sh, hand, out) in [(lsh, p.lhand, -1.0), (rsh, p.rhand, 1.0)] {
        let e = elbow(sh, hand, out);
        let arm_col = if sp == Species::Zombie { pal.torso } else { pal.skin };
        ellipse(img, sh.0, sh.1 + 2.0, 6.5, 6.0, arm_col, 1.0);
        limb(img, sh.0, sh.1 + 2.0, e.0, e.1, 5.0 * thick, 4.5 * thick, arm_col, 1.0);
        limb(img, e.0, e.1, hand.0, hand.1, 4.5 * thick, 3.8 * thick, pal.skin, 1.0);
        ellipse(img, hand.0, hand.1, 4.0, 4.0, pal.skin, 0.9);
        if sp == Species::Imp {
            spike(img, sh.0 + out * 1.0, sh.1 - 3.0, out * 4.0, -7.0, 1.8);
            spike(img, sh.0 + out * 5.0, sh.1 - 1.0, out * 6.0, -4.0, 1.5);
            for c in [-2.5, 0.0, 2.5] {
                spike(img, hand.0 + c, hand.1 + 3.0, c * 0.5, 4.0, 0.9);
            }
        }
    }

    // Head.
    let hx = cx;
    let hy = 44.0 + p.bob;
    ellipse(img, hx, hy + 8.0, 4.0, 4.0, pal.skin, 0.8); // neck
    ellipse(img, hx, hy, 8.0, 9.5, pal.skin, 1.0);
    match sp {
        Species::Zombie => {
            // Hair cap.
            for y in (hy - 10.0) as i32..(hy - 3.0) as i32 {
                for x in (hx - 9.0) as i32..(hx + 9.0) as i32 {
                    let dx = (x as f32 + 0.5 - hx) / 8.3;
                    let dy = (y as f32 + 0.5 - hy) / 9.8;
                    if dx * dx + dy * dy <= 1.0 {
                        put(img, x, y, solid(scale(pal.hair, 0.8 + 0.4 * hash2(x, y, 5))));
                    }
                }
            }
            for s in [-1.0, 1.0] {
                rect(img, hx + s * 3.0 - 1.0, hy - 0.5, hx + s * 3.0 + 1.0, hy + 1.0, glow(pal.eyes, 0.8));
            }
            // Bloody mouth.
            let mh = 1.0 + p.mouth * 3.0;
            rect(img, hx - 2.5, hy + 4.5, hx + 2.5, hy + 4.5 + mh, solid(rgb(60, 10, 10)));
        }
        Species::Imp => {
            rect(img, hx - 7.0, hy - 3.0, hx + 7.0, hy - 1.0, solid(scale(pal.skin, 0.55)));
            for s in [-1.0, 1.0] {
                ellipse_flat(img, hx + s * 3.3, hy, 2.0, 1.4, glow(pal.eyes, 1.0));
                spike(img, hx + s * 5.0, hy - 6.0, s * 3.0, -6.0, 1.4);
            }
            spike(img, hx, hy - 8.5, 0.0, -5.0, 1.3);
            let mh = 1.5 + p.mouth * 3.5;
            rect(img, hx - 3.5, hy + 4.0, hx + 3.5, hy + 4.0 + mh, solid(rgb(50, 8, 6)));
            for t in 0..3 {
                let tx = hx - 2.5 + t as f32 * 2.5;
                spike(img, tx, hy + 4.0, 0.0, 1.8, 0.8);
            }
        }
    }

    // Held items in front of the body.
    if sp == Species::Zombie {
        let gx = (p.lhand.0 + p.rhand.0) * 0.5;
        let gy = (p.lhand.1 + p.rhand.1) * 0.5 - 2.0;
        block(img, gx - 6.0, gy - 5.0, gx + 6.0, gy + 5.0, rgb(70, 70, 76));
        ellipse_flat(img, gx, gy - 1.0, 3.2, 3.2, solid(rgb(110, 110, 116)));
        ellipse_flat(img, gx, gy - 1.0, 2.0, 2.0, solid(rgb(8, 8, 8)));
        for (dx, dy) in [(p.lhand.0 - gx, p.lhand.1 - gy), (p.rhand.0 - gx, p.rhand.1 - gy)] {
            ellipse(img, gx + dx * 1.2, gy + dy + 2.0, 3.5, 3.0, pal.skin, 0.8);
        }
        if p.flash {
            fireball(img, gx, gy - 2.0, 7.0, rgb(255, 255, 220), rgb(255, 170, 40), 3);
            for (dx, dy) in [(9.0, 0.0), (-9.0, 0.0), (0.0, -9.0), (6.0, 6.0), (-6.0, -6.0)] {
                limb(img, gx, gy - 2.0, gx + dx, gy - 2.0 + dy, 2.0, 0.5, rgb(255, 220, 120), 0.0);
            }
            // Limb uses lit pixels; make the spikes emissive.
            for y in (gy - 14.0) as i32..(gy + 10.0) as i32 {
                for x in (gx - 12.0) as i32..(gx + 12.0) as i32 {
                    if opaque(img, x, y) {
                        let q = img.get(x as usize, y as usize);
                        if q[0] > 0.9 && q[1] > 0.6 && q[2] < 0.6 {
                            put(img, x, y, glow([q[0], q[1], q[2]], 1.0));
                        }
                    }
                }
            }
        }
    }
    if p.held_ball > 0.0 {
        fireball(img, p.rhand.0, p.rhand.1 - p.held_ball * 0.6, p.held_ball, rgb(255, 250, 200), rgb(255, 110, 20), 9);
    }
}

fn zombie_palette() -> Palette {
    Palette {
        skin: rgb(198, 150, 118),
        torso: rgb(84, 104, 60),
        legs: rgb(78, 70, 48),
        feet: rgb(44, 34, 26),
        hair: rgb(58, 40, 24),
        eyes: rgb(255, 40, 20),
    }
}

fn imp_palette() -> Palette {
    Palette {
        skin: rgb(138, 92, 58),
        torso: rgb(128, 84, 52),
        legs: rgb(118, 78, 48),
        feet: rgb(90, 60, 38),
        hair: rgb(0, 0, 0),
        eyes: rgb(255, 200, 40),
    }
}

fn humanoid_frame(sp: Species, p: &Pose) -> Image {
    let pal = if sp == Species::Zombie { zombie_palette() } else { imp_palette() };
    let mut img = blank();
    draw_humanoid(&mut img, sp, &pal, p);
    grit(&mut img, 0.08, 17);
    if sp == Species::Imp {
        // Mottled skin.
        for y in 0..SPR {
            for x in 0..SPR {
                let q = img.get(x, y);
                if q[3] > 0.99 && crate::math::fbm(x as f32, y as f32, 128.0, 16, 2, 33) > 0.62 {
                    img.set(x, y, [q[0] * 0.75, q[1] * 0.7, q[2] * 0.7, 1.0]);
                }
            }
        }
    }
    outline(&mut img);
    img
}

fn draw_demon(walk: usize, mouth: f32) -> Image {
    let mut img = blank();
    let pink = rgb(214, 118, 128);
    let dark = rgb(150, 70, 80);
    let lift = |f: usize, side: usize| if f == 1 + 2 * side { 9.0 } else { 0.0 };
    let bob = if walk % 2 == 1 { -2.0 } else { 1.0 };
    // Legs.
    for (side, x) in [(0usize, 46.0f32), (1, 82.0)] {
        let out = if side == 0 { -1.0 } else { 1.0 };
        let fy = 125.0 - lift(walk, side);
        limb(&mut img, x, 96.0 + bob, x + out * 5.0, 108.0 - lift(walk, side) * 0.5, 10.0, 8.0, dark, 1.0);
        limb(&mut img, x + out * 5.0, 108.0 - lift(walk, side) * 0.5, x + out * 2.0, fy - 4.0, 8.0, 6.5, dark, 1.0);
        ellipse(&mut img, x + out * 2.0, fy - 3.0, 8.0, 4.0, rgb(70, 50, 44), 0.9);
    }
    // Body.
    ellipse(&mut img, 64.0, 84.0 + bob, 33.0, 25.0, pink, 1.0);
    // Little arms.
    for s in [-1.0f32, 1.0] {
        let hand_y = 100.0 + bob + if walk % 2 == 1 { s * 3.0 } else { 0.0 };
        limb(&mut img, 64.0 + s * 28.0, 78.0 + bob, 64.0 + s * 34.0, hand_y, 6.0, 4.5, pink, 1.0);
        for c in [-2.0, 0.0, 2.0] {
            spike(&mut img, 64.0 + s * 34.0 + c, hand_y + 3.0, c * 0.4, 3.5, 0.9);
        }
    }
    // Head and jaw.
    ellipse(&mut img, 64.0, 70.0 + bob, 24.0, 19.0, mix(pink, rgb(255, 255, 255), 0.08), 1.0);
    rect(&mut img, 44.0, 58.0 + bob, 84.0, 61.0 + bob, solid(scale(pink, 0.55)));
    for s in [-1.0f32, 1.0] {
        let hx = 64.0 + s * 17.0;
        poly(
            &mut img,
            &[(hx - 4.0, 56.0 + bob), (hx + s * 7.0, 42.0 + bob), (hx + 4.0, 55.0 + bob)],
            solid(rgb(230, 214, 180)),
        );
        ellipse_flat(&mut img, 64.0 + s * 9.0, 64.0 + bob, 3.5, 2.2, glow(rgb(255, 220, 60), 1.0));
    }
    // Mouth with teeth.
    let my = 80.0 + bob;
    let mh = 3.0 + mouth * 11.0;
    ellipse_flat(&mut img, 64.0, my, 18.0, mh, solid(rgb(80, 6, 10)));
    if mouth > 0.3 {
        ellipse_flat(&mut img, 64.0, my + mh * 0.4, 9.0, mh * 0.4, solid(rgb(200, 60, 80)));
    }
    for i in 0..7 {
        let tx = 64.0 - 14.0 + i as f32 * 4.7;
        let w = 1.0 - ((tx - 64.0) / 18.0).powi(2);
        let top = my - mh * w.max(0.0).sqrt();
        let bot = my + mh * w.max(0.0).sqrt();
        poly(&mut img, &[(tx - 2.0, top), (tx, top + 4.5), (tx + 2.0, top)], solid(rgb(250, 245, 225)));
        poly(&mut img, &[(tx - 2.0, bot), (tx, bot - 4.0), (tx + 2.0, bot)], solid(rgb(250, 245, 225)));
    }
    grit(&mut img, 0.07, 23);
    outline(&mut img);
    img
}

fn draw_caco(mouth: f32, eye_closed: bool, charge: bool) -> Image {
    let mut img = blank();
    let red = rgb(200, 40, 36);
    // Horns behind the head.
    for s in [-1.0f32, 1.0] {
        poly(
            &mut img,
            &[(64.0 + s * 18.0, 40.0), (64.0 + s * 34.0, 8.0), (64.0 + s * 30.0, 36.0)],
            solid(rgb(226, 214, 190)),
        );
    }
    ellipse(&mut img, 64.0, 64.0, 38.0, 36.0, red, 1.0);
    // Blotches.
    let mut rng = Rng::new(7);
    for _ in 0..14 {
        let a = rng.range(0.0, std::f32::consts::TAU);
        let r = rng.range(8.0, 30.0);
        let (x, y) = (64.0 + a.cos() * r, 64.0 + a.sin() * r * 0.9);
        ellipse(&mut img, x, y, rng.range(2.0, 4.0), rng.range(2.0, 3.5), scale(red, 0.7), 0.6);
    }
    // Eye.
    if eye_closed {
        ellipse(&mut img, 64.0, 50.0, 12.0, 10.0, scale(red, 0.85), 0.9);
        rect(&mut img, 54.0, 50.0, 74.0, 52.0, solid(rgb(60, 6, 6)));
    } else {
        ellipse(&mut img, 64.0, 50.0, 12.0, 10.0, rgb(240, 240, 220), 0.7);
        ellipse_flat(&mut img, 64.0, 51.0, 6.5, 6.5, solid(rgb(40, 170, 60)));
        ellipse_flat(&mut img, 64.0, 51.0, 3.0, 3.5, solid(rgb(4, 10, 4)));
        put(&mut img, 62, 48, solid(rgb(255, 255, 255)));
    }
    // Mouth.
    let mh = 5.0 + mouth * 11.0;
    ellipse_flat(&mut img, 64.0, 80.0, 22.0, mh, solid(rgb(60, 4, 8)));
    if charge {
        fireball(&mut img, 64.0, 80.0, mh * 0.8, rgb(255, 210, 255), rgb(190, 60, 230), 13);
    } else {
        ellipse_flat(&mut img, 64.0, 80.0 + mh * 0.5, 10.0, mh * 0.35, solid(rgb(60, 150, 60)));
    }
    for i in 0..8 {
        let tx = 64.0 - 17.0 + i as f32 * 4.9;
        let w = (1.0 - ((tx - 64.0) / 22.0).powi(2)).max(0.0).sqrt();
        let top = 80.0 - mh * w;
        let bot = 80.0 + mh * w;
        poly(&mut img, &[(tx - 2.2, top), (tx, top + 5.0), (tx + 2.2, top)], solid(rgb(245, 240, 220)));
        poly(&mut img, &[(tx - 2.2, bot), (tx, bot - 4.5), (tx + 2.2, bot)], solid(rgb(245, 240, 220)));
    }
    grit(&mut img, 0.06, 29);
    outline(&mut img);
    img
}

/// Collapse a standing frame into a bloody heap over `n` frames.
fn death_frames(base: &Image, pivot_y: f32, n: usize, seed: u32) -> Vec<Image> {
    let mut out = Vec::new();
    let mut rng = Rng::new(seed as u64);
    let splats: Vec<(f32, f32, f32)> =
        (0..24).map(|_| (rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(1.0, 3.0))).collect();
    for k in 0..n {
        let t = (k + 1) as f32 / n as f32;
        let sy = 1.0 - 0.82 * t.powf(1.3);
        let sx = 1.0 + 0.45 * t;
        let tilt = 0.25 * t * (1.0 - t) * 4.0;
        let mut img = blank();
        // Blood pool behind the body.
        if k >= 1 {
            let rx = 10.0 + 26.0 * t;
            ellipse_flat(&mut img, 64.0, pivot_y - 1.0, rx, 2.0 + 3.0 * t, solid(rgb(110, 4, 6)));
            ellipse_flat(&mut img, 60.0, pivot_y - 1.5, rx * 0.6, 1.5 + 2.0 * t, solid(rgb(150, 10, 10)));
        }
        for y in 0..SPR as i32 {
            for x in 0..SPR as i32 {
                let dy = pivot_y - (y as f32 + 0.5);
                let dx = x as f32 + 0.5 - 64.0 - dy * tilt;
                let sxp = 64.0 + dx / sx;
                let syp = pivot_y - dy / sy;
                if sxp < 0.0 || syp < 0.0 || sxp >= SPR as f32 || syp >= SPR as f32 {
                    continue;
                }
                let p = base.get(sxp as usize, syp as usize);
                if p[3] >= 0.5 {
                    let dim = 1.0 - 0.3 * t;
                    put(&mut img, x, y, [p[0] * dim, p[1] * dim * 0.95, p[2] * dim * 0.95, 1.0]);
                }
            }
        }
        // Gore on top.
        let count = (splats.len() as f32 * t) as usize;
        for &(ux, uy, r) in &splats[..count] {
            let x = 64.0 + ux * 24.0 * sx;
            let y = pivot_y - 6.0 - (uy + 1.0) * 30.0 * sy;
            if opaque(&img, x as i32, y as i32) {
                ellipse_flat(&mut img, x, y, r, r * 0.8, solid(rgb(140, 8, 8)));
            }
        }
        out.push(img);
    }
    out
}

// ---------------------------------------------------------------------------
// Projectiles and effects
// ---------------------------------------------------------------------------

fn fx_ball(r: f32, inner: Rgb, outer: Rgb, seed: u32) -> Image {
    let mut img = blank();
    fireball(&mut img, 64.0, 64.0, r, inner, outer, seed);
    // Flame licks.
    let mut rng = Rng::new(seed as u64);
    for _ in 0..6 {
        let a = rng.range(0.0, std::f32::consts::TAU);
        let d = r * rng.range(0.8, 1.2);
        fireball(&mut img, 64.0 + a.cos() * d, 64.0 + a.sin() * d, r * 0.35, mix(inner, outer, 0.5), outer, seed + 1);
    }
    img
}

fn explosion(stage: usize) -> Image {
    let mut img = blank();
    let mut rng = Rng::new(40 + stage as u64);
    let r = [14.0, 24.0, 30.0][stage];
    let (inner, outer) = match stage {
        0 => (rgb(255, 255, 230), rgb(255, 190, 60)),
        1 => (rgb(255, 230, 120), rgb(240, 90, 20)),
        _ => (rgb(230, 110, 30), rgb(120, 20, 10)),
    };
    for i in 0..10 {
        let a = rng.range(0.0, std::f32::consts::TAU);
        let d = if i == 0 { 0.0 } else { r * rng.range(0.2, 0.7) };
        let rr = r * rng.range(0.4, 0.65);
        fireball(&mut img, 64.0 + a.cos() * d, 64.0 + a.sin() * d, rr, inner, outer, 50 + i);
    }
    if stage == 2 {
        // Break up into wisps.
        for y in 0..SPR {
            for x in 0..SPR {
                if crate::math::fbm(x as f32, y as f32, 128.0, 16, 2, 7) < 0.42 {
                    img.set(x, y, [0.0; 4]);
                }
            }
        }
    }
    img
}

fn puff(stage: usize) -> Image {
    let mut img = blank();
    let r = [4.0, 6.0, 8.0][stage];
    let g = [200, 150, 110][stage];
    ellipse(&mut img, 64.0, 64.0 - stage as f32 * 3.0, r, r, rgb(g, g, g), 0.7);
    if stage == 0 {
        fireball(&mut img, 64.0, 64.0, 3.0, rgb(255, 255, 200), rgb(255, 200, 60), 2);
    }
    img
}

fn blood(stage: usize) -> Image {
    let mut img = blank();
    let mut rng = Rng::new(60);
    for _ in 0..9 {
        let a = rng.range(0.0, std::f32::consts::TAU);
        let d = rng.range(0.0, 3.0 + stage as f32 * 4.0);
        let r = rng.range(1.5, 3.0) * (1.0 - stage as f32 * 0.2);
        let y = 64.0 + a.sin() * d + stage as f32 * 4.0;
        ellipse(&mut img, 64.0 + a.cos() * d, y, r, r, rgb(170, 10, 10), 0.8);
    }
    img
}

// ---------------------------------------------------------------------------
// Pickups and decorations
// ---------------------------------------------------------------------------

fn medkit(big: bool) -> Image {
    let mut img = blank();
    let (w, h) = if big { (30.0, 20.0) } else { (18.0, 12.0) };
    let (x0, y0) = (64.0 - w / 2.0, 126.0 - h);
    block(&mut img, x0, y0, x0 + w, y0 + h, rgb(225, 225, 220));
    let (cx, cy) = (64.0, y0 + h / 2.0);
    let a = h * 0.32;
    let b = h * 0.12;
    rect(&mut img, cx - a, cy - b, cx + a, cy + b, solid(rgb(200, 20, 20)));
    rect(&mut img, cx - b, cy - a, cx + b, cy + a, solid(rgb(200, 20, 20)));
    if big {
        rect(&mut img, 58.0, y0 - 3.0, 70.0, y0, solid(rgb(90, 90, 90)));
    }
    outline(&mut img);
    img
}

fn clip() -> Image {
    let mut img = blank();
    block(&mut img, 58.0, 108.0, 70.0, 126.0, rgb(60, 62, 66));
    for i in 0..3 {
        let x = 60.0 + i as f32 * 3.5;
        ellipse(&mut img, x + 1.0, 106.0, 1.6, 3.0, rgb(220, 170, 60), 0.8);
    }
    outline(&mut img);
    img
}

fn shells(boxed: bool) -> Image {
    let mut img = blank();
    if boxed {
        block(&mut img, 48.0, 110.0, 80.0, 126.0, rgb(150, 40, 30));
        rect(&mut img, 50.0, 115.0, 78.0, 119.0, solid(rgb(220, 190, 60)));
        for i in 0..6 {
            let x = 51.0 + i as f32 * 4.8;
            ellipse(&mut img, x + 1.5, 108.0, 2.0, 2.5, rgb(200, 30, 20), 0.8);
        }
    } else {
        for i in 0..4 {
            let y = 124.0 - (i % 2) as f32 * 4.0;
            let x = 52.0 + i as f32 * 6.0;
            limb(&mut img, x, y, x + 8.0, y - 2.0, 2.2, 2.2, rgb(200, 30, 20), 0.9);
            ellipse(&mut img, x + 8.5, y - 2.0, 2.0, 2.2, rgb(220, 180, 70), 0.8);
        }
    }
    outline(&mut img);
    img
}

fn draw_rocket_ammo(boxed: bool) -> Image {
    let mut img = blank();
    if boxed {
        block(&mut img, 44.0, 108.0, 84.0, 126.0, rgb(120, 90, 50));
        for i in 0..5 {
            let x = 48.0 + i as f32 * 8.0;
            ellipse(&mut img, x, 106.0, 3.0, 4.0, rgb(180, 40, 30), 0.8);
        }
        rect(&mut img, 44.0, 116.0, 84.0, 118.0, solid(rgb(60, 40, 20)));
    } else {
        limb(&mut img, 44.0, 121.0, 80.0, 121.0, 3.5, 3.5, rgb(110, 120, 90), 0.9);
        ellipse(&mut img, 82.0, 121.0, 4.0, 3.5, rgb(190, 40, 30), 0.9);
        poly(&mut img, &[(42.0, 116.0), (48.0, 121.0), (42.0, 126.0)], solid(rgb(80, 80, 80)));
    }
    outline(&mut img);
    img
}

fn armor(col: Rgb) -> Image {
    let mut img = blank();
    let pts = [
        (46.0, 98.0),
        (54.0, 94.0),
        (58.0, 100.0),
        (70.0, 100.0),
        (74.0, 94.0),
        (82.0, 98.0),
        (80.0, 126.0),
        (48.0, 126.0),
    ];
    poly(&mut img, &pts, solid(col));
    // Plates with shading.
    for (x, y) in [(56.0, 108.0), (72.0, 108.0), (56.0, 119.0), (72.0, 119.0)] {
        ellipse(&mut img, x, y, 6.5, 5.0, col, 1.0);
    }
    rect(&mut img, 63.0, 101.0, 65.0, 126.0, solid(scale(col, 0.5)));
    outline(&mut img);
    img
}

fn potion(bright: bool) -> Image {
    let mut img = blank();
    let liquid = if bright { rgb(120, 170, 255) } else { rgb(60, 110, 240) };
    ellipse(&mut img, 64.0, 117.0, 8.0, 9.0, rgb(200, 220, 240), 0.8);
    ellipse_flat(&mut img, 64.0, 119.0, 6.5, 6.5, glow(liquid, 0.6));
    rect(&mut img, 61.0, 104.0, 67.0, 110.0, solid(rgb(200, 220, 240)));
    rect(&mut img, 60.0, 102.0, 68.0, 104.0, solid(rgb(140, 100, 60)));
    outline(&mut img);
    img
}

fn soulsphere(bright: bool) -> Image {
    let mut img = blank();
    let (inner, outer) = if bright {
        (rgb(220, 240, 255), rgb(60, 110, 255))
    } else {
        (rgb(170, 200, 255), rgb(40, 70, 220))
    };
    fireball(&mut img, 64.0, 96.0, 17.0, inner, outer, 71);
    // Ghostly face.
    for s in [-1.0f32, 1.0] {
        ellipse_flat(&mut img, 64.0 + s * 5.5, 92.0, 2.5, 3.0, glow(rgb(20, 40, 140), 1.0));
    }
    ellipse_flat(&mut img, 64.0, 102.0, 4.0, 2.0, glow(rgb(20, 40, 140), 1.0));
    img
}

fn weapon_pickup(kind: usize) -> Image {
    let mut img = blank();
    let steel = rgb(96, 98, 106);
    match kind {
        // Shotgun.
        0 => {
            limb(&mut img, 30.0, 118.0, 98.0, 114.0, 2.2, 2.2, steel, 0.9);
            block(&mut img, 56.0, 116.0, 72.0, 121.0, rgb(120, 76, 40));
            poly(&mut img, &[(24.0, 116.0), (40.0, 116.0), (38.0, 124.0), (20.0, 126.0)], solid(rgb(110, 70, 36)));
        }
        // Chaingun.
        1 => {
            for i in 0..3 {
                let y = 112.0 + i as f32 * 3.0;
                limb(&mut img, 50.0, y, 96.0, y, 1.6, 1.6, steel, 0.9);
            }
            block(&mut img, 34.0, 108.0, 58.0, 124.0, rgb(80, 80, 88));
            block(&mut img, 40.0, 120.0, 48.0, 127.0, rgb(50, 50, 54));
        }
        // Rocket launcher.
        _ => {
            limb(&mut img, 28.0, 114.0, 100.0, 114.0, 6.0, 6.0, rgb(90, 104, 80), 1.0);
            ellipse_flat(&mut img, 100.0, 114.0, 3.0, 5.0, solid(rgb(20, 20, 20)));
            block(&mut img, 54.0, 102.0, 66.0, 108.0, rgb(60, 60, 64));
            block(&mut img, 48.0, 119.0, 56.0, 127.0, rgb(50, 50, 54));
        }
    }
    outline(&mut img);
    img
}

fn keycard(col: Rgb, bright: bool) -> Image {
    let mut img = blank();
    let c = if bright { mix(col, rgb(255, 255, 255), 0.35) } else { col };
    block(&mut img, 57.0, 112.0, 71.0, 126.0, c);
    rect(&mut img, 59.0, 115.0, 69.0, 118.0, glow(mix(c, rgb(255, 255, 255), 0.5), if bright { 1.0 } else { 0.4 }));
    outline(&mut img);
    img
}

fn barrel(frame: usize) -> Image {
    let mut img = blank();
    let body = rgb(92, 108, 84);
    for y in 86..126 {
        for x in 46..82 {
            let dx = (x as f32 + 0.5 - 64.0) / 18.0;
            let n = [dx, 0.0, (1.0 - dx * dx).max(0.0).sqrt()];
            let band = matches!(y, 90..=92 | 104..=106 | 119..=121);
            let c = if band { scale(body, 0.7) } else { body };
            put(&mut img, x, y, shade_px(c, n, 1.0));
        }
    }
    ellipse_flat(&mut img, 64.0, 86.0, 18.0, 5.0, solid(rgb(60, 70, 54)));
    ellipse_flat(&mut img, 64.0, 86.5, 15.0, 3.8, glow(rgb(90, 230, 60), 0.8));
    let mut rng = Rng::new(80 + frame as u64);
    for _ in 0..3 {
        ellipse_flat(&mut img, rng.range(54.0, 74.0), rng.range(84.5, 88.0), 2.0, 1.2, glow(rgb(180, 255, 140), 1.0));
    }
    // Nukage drip.
    rect(&mut img, 70.0, 90.0, 72.0, 96.0 + frame as f32 * 3.0, glow(rgb(90, 230, 60), 0.7));
    grit(&mut img, 0.08, 81);
    outline(&mut img);
    img
}

fn tech_lamp() -> Image {
    let mut img = blank();
    block(&mut img, 54.0, 118.0, 74.0, 126.0, rgb(80, 82, 90));
    limb(&mut img, 64.0, 118.0, 64.0, 34.0, 3.0, 2.5, rgb(110, 112, 120), 1.0);
    block(&mut img, 56.0, 26.0, 72.0, 34.0, rgb(90, 92, 100));
    fireball(&mut img, 64.0, 18.0, 9.0, rgb(255, 255, 255), rgb(160, 200, 255), 91);
    outline(&mut img);
    img
}

fn torch(frame: usize, flame: Rgb) -> Image {
    let mut img = blank();
    limb(&mut img, 64.0, 126.0, 64.0, 84.0, 2.5, 2.0, rgb(90, 70, 50), 1.0);
    block(&mut img, 57.0, 80.0, 71.0, 86.0, rgb(80, 80, 86));
    let mut rng = Rng::new(100 + frame as u64);
    for i in 0..5 {
        let y = 74.0 - i as f32 * 5.0 + rng.range(-1.5, 1.5);
        let r = 7.0 - i as f32 * 1.2;
        fireball(&mut img, 64.0 + rng.range(-2.0, 2.0), y, r, rgb(255, 255, 210), flame, 110 + frame as u32 * 5 + i);
    }
    outline(&mut img);
    img
}

fn skull_pole() -> Image {
    let mut img = blank();
    limb(&mut img, 64.0, 126.0, 64.0, 70.0, 2.5, 2.2, rgb(110, 80, 56), 1.0);
    let bone = rgb(222, 210, 180);
    ellipse(&mut img, 64.0, 62.0, 9.0, 10.0, bone, 1.0);
    rect(&mut img, 59.0, 69.0, 69.0, 75.0, solid(scale(bone, 0.9)));
    for s in [-1.0f32, 1.0] {
        ellipse_flat(&mut img, 64.0 + s * 4.0, 62.0, 2.6, 3.0, solid(rgb(20, 10, 8)));
    }
    for i in 0..4 {
        rect(&mut img, 60.0 + i as f32 * 2.5, 71.0, 61.0 + i as f32 * 2.5, 75.0, solid(rgb(30, 20, 16)));
    }
    ellipse_flat(&mut img, 64.0, 80.0, 6.0, 3.0, solid(rgb(140, 10, 10)));
    outline(&mut img);
    img
}

// ---------------------------------------------------------------------------
// First-person weapons (drawn bottom-center, pointing into the screen)
// ---------------------------------------------------------------------------

const SKIN: [f32; 3] = [0.78, 0.56, 0.42];
const SLEEVE: [f32; 3] = [0.27, 0.35, 0.2];

/// Perspective tube running into the screen: `w0` wide at `y0` (far end),
/// `w1` wide at `y1` (near end), shaded like a cylinder lit from above-left.
#[allow(clippy::too_many_arguments)]
fn tube(img: &mut Image, cx: f32, y0: f32, y1: f32, w0: f32, w1: f32, col: Rgb, shade: f32) {
    for y in y0.round() as i32..y1.round() as i32 {
        let t = (y as f32 - y0) / (y1 - y0).max(1.0);
        let hw = (w0 + (w1 - w0) * t) * 0.5;
        for x in (cx - hw).floor() as i32..=(cx + hw).ceil() as i32 {
            let s = (x as f32 + 0.5 - cx) / hw;
            if s.abs() <= 1.0 {
                put(img, x, y, shade_px(col, [s, -0.35, (1.0 - s * s).max(0.0).sqrt()], shade));
            }
        }
    }
}

/// Arm reaching in from below the screen, ending in a hand wrapped around
/// something `grip_w` wide centred at (x, y).
fn gripping_hand(img: &mut Image, from: (f32, f32), x: f32, y: f32, grip_w: f32) {
    limb(img, from.0, from.1, x, y + 8.0, 11.0, 8.5, SLEEVE, 1.0);
    ellipse(img, x, y + 3.0, grip_w * 0.5 + 3.0, 7.0, SKIN, 1.0);
    // Fingers curl over the left edge, the thumb over the right.
    for i in 0..3 {
        ellipse(img, x - grip_w * 0.5 - 1.0, y - 3.0 + i as f32 * 4.0, 3.0, 2.3, SKIN, 0.9);
    }
    limb(img, x + grip_w * 0.5 + 2.0, y + 6.0, x + grip_w * 0.5, y - 4.0, 2.8, 2.4, SKIN, 0.9);
}

fn pistol(fire: bool) -> Image {
    let mut img = blank();
    let dy = if fire { 4.0 } else { 0.0 };
    let steel = rgb(78, 80, 90);
    gripping_hand(&mut img, (84.0, 140.0), 65.0, 110.0 + dy, 16.0);
    // Frame and slide, narrowing with distance.
    tube(&mut img, 64.0, 96.0 + dy, 110.0 + dy, 17.0, 19.0, scale(steel, 0.8), 0.8);
    tube(&mut img, 64.0, 72.0 + dy, 100.0 + dy, 13.0, 18.0, steel, 1.0);
    for i in 0..4 {
        rect(&mut img, 57.0, 90.0 + dy + i as f32 * 2.5, 71.0, 91.0 + dy + i as f32 * 2.5, solid(scale(steel, 0.55)));
    }
    // Sights.
    rect(&mut img, 58.5, 69.5 + dy, 61.0, 73.0 + dy, solid(rgb(40, 40, 46)));
    rect(&mut img, 67.0, 69.5 + dy, 69.5, 73.0 + dy, solid(rgb(40, 40, 46)));
    rect(&mut img, 63.0, 67.0 + dy, 65.0, 72.0 + dy, solid(rgb(150, 150, 160)));
    ellipse_flat(&mut img, 64.0, 74.0 + dy, 2.0, 1.2, solid(rgb(10, 10, 12)));
    grit(&mut img, 0.05, 131);
    outline(&mut img);
    img
}

fn shotgun(frame: usize) -> Image {
    // 0 idle, 1 fire, 2/3 pump.
    let mut img = blank();
    let (dy, pump) = match frame {
        1 => (5.0, 0.0),
        2 => (9.0, 12.0),
        3 => (5.0, 6.0),
        _ => (0.0, 0.0),
    };
    let steel = rgb(70, 72, 82);
    let wood = rgb(132, 82, 42);
    // Receiver at the bottom, then barrel over magazine tube.
    tube(&mut img, 64.0, 108.0 + dy, 132.0, 26.0, 32.0, scale(steel, 0.85), 1.0);
    rect(&mut img, 70.0, 114.0 + dy, 76.0, 122.0 + dy, solid(rgb(20, 20, 24)));
    tube(&mut img, 64.0, 60.0 + dy, 112.0 + dy, 10.0, 18.0, steel, 1.0);
    rect(&mut img, 63.0, 62.0 + dy, 65.0, 110.0 + dy, solid(scale(steel, 1.45)));
    ellipse_flat(&mut img, 64.0, 60.5 + dy, 2.5, 1.6, solid(rgb(12, 12, 14)));
    ellipse_flat(&mut img, 64.0, 58.5 + dy, 1.3, 1.3, solid(rgb(200, 200, 190)));
    // Pump.
    let py = 84.0 + dy + pump;
    tube(&mut img, 64.0, py, py + 20.0, 19.0, 23.0, wood, 1.0);
    for i in 0..5 {
        let y = py + 3.0 + i as f32 * 3.4;
        let hw = 9.5 + i as f32 * 0.5;
        rect(&mut img, 64.0 - hw, y, 64.0 + hw, y + 1.0, solid(scale(wood, 0.6)));
    }
    gripping_hand(&mut img, (34.0, 146.0), 64.0, py + 14.0, 22.0);
    grit(&mut img, 0.05, 121);
    outline(&mut img);
    img
}

fn chaingun(spin: usize) -> Image {
    let mut img = blank();
    let steel = rgb(84, 86, 96);
    // Housing.
    tube(&mut img, 64.0, 90.0, 132.0, 34.0, 46.0, scale(steel, 0.8), 1.0);
    rect(&mut img, 50.0, 100.0, 78.0, 103.0, solid(scale(steel, 0.5)));
    // Rotating barrel cluster: draw the far side first.
    let mut barrels: Vec<(f32, f32)> = (0..6)
        .map(|i| {
            let a = i as f32 / 6.0 * std::f32::consts::TAU + spin as f32 * 0.52;
            (a.cos(), a.sin())
        })
        .collect();
    barrels.sort_by(|a, b| a.1.total_cmp(&b.1));
    for (c, s) in barrels {
        let top = (64.0 + c * 7.0, 58.0 + s * 3.5);
        let bot = (64.0 + c * 12.0, 94.0 + s * 5.0);
        limb(&mut img, top.0, top.1, bot.0, bot.1, 2.4, 3.6, scale(steel, 0.9 + 0.2 * s), 1.0);
        ellipse_flat(&mut img, top.0, top.1, 1.6, 1.2, solid(rgb(10, 10, 12)));
    }
    tube(&mut img, 64.0, 70.0, 75.0, 20.0, 21.0, scale(steel, 1.1), 1.0);
    gripping_hand(&mut img, (30.0, 150.0), 44.0, 116.0, 12.0);
    gripping_hand(&mut img, (98.0, 150.0), 85.0, 116.0, 12.0);
    grit(&mut img, 0.05, 141);
    outline(&mut img);
    img
}

fn launcher(fire: bool) -> Image {
    let mut img = blank();
    let dy = if fire { 7.0 } else { 0.0 };
    let body = rgb(88, 104, 78);
    tube(&mut img, 64.0, 60.0 + dy, 132.0, 22.0, 38.0, body, 1.0);
    for y in [74.0, 96.0, 118.0] {
        let t = (y - 60.0) / 72.0;
        let hw = (22.0 + 16.0 * t) * 0.5;
        rect(&mut img, 64.0 - hw, y + dy, 64.0 + hw, y + dy + 2.0, solid(scale(body, 0.6)));
    }
    ellipse_flat(&mut img, 64.0, 61.0 + dy, 11.0, 5.0, solid(rgb(44, 50, 40)));
    ellipse_flat(&mut img, 64.0, 61.5 + dy, 8.0, 3.5, solid(rgb(6, 6, 6)));
    // Sight block on top.
    block(&mut img, 60.0, 80.0 + dy, 68.0, 94.0 + dy, rgb(56, 56, 60));
    gripping_hand(&mut img, (26.0, 150.0), 44.0, 112.0 + dy, 10.0);
    grit(&mut img, 0.05, 151);
    outline(&mut img);
    img
}

fn muzzle_flash(cx: f32, cy: f32, r: f32, seed: u32) -> Image {
    let mut img = blank();
    fireball(&mut img, cx, cy, r, rgb(255, 255, 230), rgb(255, 160, 40), seed);
    let mut rng = Rng::new(seed as u64);
    for _ in 0..7 {
        let a = rng.range(0.0, std::f32::consts::TAU);
        let d = r * rng.range(0.9, 1.5);
        fireball(&mut img, cx + a.cos() * d, cy + a.sin() * d * 0.8, r * 0.4, rgb(255, 240, 180), rgb(255, 120, 20), seed + 3);
    }
    img
}

// ---------------------------------------------------------------------------
// Status bar face, font, UI
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Expr {
    Normal,
    Ouch,
    Grin,
    Dead,
}

fn face(tier: usize, look: i32, expr: Expr) -> Image {
    let mut img = Image::new(32, 32);
    let skin = if expr == Expr::Dead { rgb(150, 120, 100) } else { rgb(206, 144, 104) };
    let hair = rgb(92, 60, 30);
    // Head and ears.
    ellipse(&mut img, 16.0, 17.0, 11.0, 13.5, skin, 0.9);
    ellipse(&mut img, 5.5, 18.0, 2.0, 3.0, skin, 0.9);
    ellipse(&mut img, 26.5, 18.0, 2.0, 3.0, skin, 0.9);
    for y in 3..10 {
        for x in 4..28 {
            let dx = (x as f32 + 0.5 - 16.0) / 11.5;
            let dy = (y as f32 + 0.5 - 17.0) / 14.0;
            if dx * dx + dy * dy <= 1.0 {
                put(&mut img, x, y, solid(scale(hair, 0.8 + 0.4 * hash2(x, y, 3))));
            }
        }
    }
    rect(&mut img, 6.0, 9.0, 8.0, 15.0, solid(hair));
    rect(&mut img, 24.0, 9.0, 26.0, 15.0, solid(hair));

    let brow = solid(rgb(70, 40, 20));
    let eye_y = 15.0;
    match expr {
        Expr::Dead => {
            for s in [-1.0f32, 1.0] {
                rect(&mut img, 16.0 + s * 5.0 - 2.5, eye_y, 16.0 + s * 5.0 + 2.5, eye_y + 1.0, brow);
            }
        }
        _ => {
            let squint = if tier >= 4 { 1.2 } else { 2.0 };
            let open = if expr == Expr::Ouch { 2.8 } else { squint };
            for s in [-1.0f32, 1.0] {
                let ex = 16.0 + s * 5.0;
                ellipse_flat(&mut img, ex, eye_y, 3.0, open, solid(rgb(240, 240, 235)));
                let px = ex + look as f32 * 1.4;
                rect(&mut img, px - 1.0, eye_y - 1.0, px + 1.0, eye_y + 1.0, solid(rgb(40, 60, 120)));
                // Angry brows, lower on the inside.
                let inner = if expr == Expr::Ouch { -1.5 } else { 1.0 };
                poly(
                    &mut img,
                    &[(ex - s * 3.5, 11.0), (ex + s * 3.5, 11.0 + inner), (ex + s * 3.5, 12.5 + inner), (ex - s * 3.5, 12.5)],
                    brow,
                );
            }
        }
    }
    // Nose.
    rect(&mut img, 15.0, 16.0, 17.0, 21.0, solid(scale(skin, 0.75)));
    // Mouth.
    match expr {
        Expr::Grin => {
            rect(&mut img, 10.0, 23.0, 22.0, 27.0, solid(rgb(60, 10, 10)));
            rect(&mut img, 11.0, 23.0, 21.0, 25.0, solid(rgb(245, 245, 235)));
        }
        Expr::Ouch => ellipse_flat(&mut img, 16.0, 25.0, 3.5, 3.0, solid(rgb(60, 10, 10))),
        _ => rect(&mut img, 12.0, 24.0, 20.0, 25.5, solid(rgb(110, 50, 40))),
    }
    // Damage: bruises and blood grow with the tier.
    let mut rng = Rng::new(900 + tier as u64);
    if tier >= 2 {
        ellipse_flat(&mut img, 21.0, 18.5, 3.0, 1.5, solid(rgb(120, 70, 110)));
    }
    let splats = [0, 1, 3, 6, 10][tier.min(4)] + if expr == Expr::Dead { 10 } else { 0 };
    for _ in 0..splats {
        let (x, y) = (rng.range(7.0, 25.0), rng.range(6.0, 28.0));
        if opaque(&img, x as i32, y as i32) {
            ellipse_flat(&mut img, x, y, rng.range(0.8, 2.0), rng.range(1.0, 2.6), solid(rgb(170, 10, 10)));
        }
    }
    outline(&mut img);
    img
}

fn font_layer() -> Image {
    let mut img = blank();
    for code in 0x20u8..=0x7F {
        let (cx, cy) = ((code % 16) as usize * 8, (code / 16) as usize * 8);
        for y in 0..8 {
            for x in 0..8 {
                if font::pixel(code, x, y) {
                    img.set(cx + x, cy + y, [1.0, 1.0, 1.0, 1.0]);
                }
            }
        }
    }
    img
}

fn statusbar_tile() -> Image {
    let mut img = blank();
    for y in 0..SPR {
        for x in 0..SPR {
            let g = crate::math::fbm(x as f32, y as f32, 128.0, 16, 4, 501);
            let c = mix(rgb(70, 66, 60), rgb(126, 118, 104), g);
            img.set(x, y, solid(c));
        }
    }
    img
}

// ---------------------------------------------------------------------------
// Catalogue
// ---------------------------------------------------------------------------

/// A monster's animation layers.
#[derive(Clone, Debug, Default)]
pub struct MonsterSprites {
    pub walk: Vec<u32>,
    pub attack: Vec<u32>,
    pub pain: u32,
    pub death: Vec<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct WeaponSprites {
    pub frames: Vec<u32>,
    pub flash: Vec<u32>,
}

/// Every sprite layer plus named indices into it.
pub struct Sprites {
    pub layers: Vec<Image>,
    pub zombie: MonsterSprites,
    pub imp: MonsterSprites,
    pub demon: MonsterSprites,
    pub caco: MonsterSprites,
    pub imp_ball: Vec<u32>,
    pub caco_ball: Vec<u32>,
    pub rocket: Vec<u32>,
    pub explosion: Vec<u32>,
    pub puff: Vec<u32>,
    pub blood: Vec<u32>,
    pub stimpack: u32,
    pub medikit: u32,
    pub clip: u32,
    pub shells: u32,
    pub shell_box: u32,
    pub rocket_ammo: u32,
    pub rocket_box: u32,
    pub green_armor: u32,
    pub blue_armor: u32,
    pub potion: Vec<u32>,
    pub soulsphere: Vec<u32>,
    pub shotgun_pickup: u32,
    pub chaingun_pickup: u32,
    pub launcher_pickup: u32,
    pub keys: [Vec<u32>; 3],
    pub barrel: Vec<u32>,
    pub lamp: u32,
    pub torch_red: Vec<u32>,
    pub torch_blue: Vec<u32>,
    pub skull_pole: u32,
    pub pistol: WeaponSprites,
    pub shotgun: WeaponSprites,
    pub chaingun: WeaponSprites,
    pub launcher: WeaponSprites,
    /// Face sheet: 4x4 grid of 32x32 faces per layer.
    pub face_layers: [u32; 2],
    pub font: u32,
    pub statusbar: u32,
}

/// Face slot in the face sheet: 5 health tiers x 3 look directions, then extras.
pub fn face_index(tier: usize, look: i32) -> usize {
    tier.min(4) * 3 + (look + 1) as usize
}
pub const FACE_OUCH: usize = 15;
pub const FACE_GRIN: usize = 16;
pub const FACE_DEAD: usize = 17;

struct Builder {
    layers: Vec<Image>,
}

impl Builder {
    fn add(&mut self, img: Image) -> u32 {
        self.layers.push(img);
        (self.layers.len() - 1) as u32
    }

    fn add_all(&mut self, imgs: Vec<Image>) -> Vec<u32> {
        imgs.into_iter().map(|i| self.add(i)).collect()
    }
}

fn humanoid_set(b: &mut Builder, sp: Species) -> MonsterSprites {
    let walk: Vec<u32> = (0..4).map(|f| b.add(humanoid_frame(sp, &Pose::walk(sp, f)))).collect();
    let attack = match sp {
        Species::Zombie => {
            let mut aim = Pose::stand(sp);
            aim.lhand = (59.0, 74.0);
            aim.rhand = (69.0, 74.0);
            aim.bob = -0.5;
            let mut fire = aim;
            fire.flash = true;
            vec![b.add(humanoid_frame(sp, &aim)), b.add(humanoid_frame(sp, &fire))]
        }
        Species::Imp => {
            let mut a = Pose::stand(sp);
            a.rhand = (88.0, 42.0);
            a.held_ball = 5.0;
            a.mouth = 0.6;
            let mut b2 = a;
            b2.rhand = (92.0, 32.0);
            b2.held_ball = 7.5;
            b2.lean = -2.0;
            let mut c = Pose::stand(sp);
            c.rhand = (72.0, 66.0);
            c.lean = 3.0;
            c.mouth = 1.0;
            vec![b.add(humanoid_frame(sp, &a)), b.add(humanoid_frame(sp, &b2)), b.add(humanoid_frame(sp, &c))]
        }
    };
    let mut pain_pose = Pose::stand(sp);
    pain_pose.lean = -4.0;
    pain_pose.mouth = 1.0;
    pain_pose.bob = -1.0;
    let mut pain = humanoid_frame(sp, &pain_pose);
    tint(&mut pain, rgb(255, 60, 40), 0.25);
    let pain = b.add(pain);
    let base = humanoid_frame(sp, &pain_pose);
    let death = b.add_all(death_frames(&base, 126.0, 5, if sp == Species::Zombie { 1 } else { 2 }));
    MonsterSprites { walk, attack, pain, death }
}

pub fn build() -> Sprites {
    let mut b = Builder { layers: Vec::new() };

    let zombie = humanoid_set(&mut b, Species::Zombie);
    let imp = humanoid_set(&mut b, Species::Imp);

    let demon = {
        let walk = (0..4).map(|f| b.add(draw_demon(f, 0.15))).collect();
        let attack = vec![b.add(draw_demon(0, 0.5)), b.add(draw_demon(0, 1.0)), b.add(draw_demon(2, 0.3))];
        let mut p = draw_demon(0, 0.8);
        tint(&mut p, rgb(255, 60, 40), 0.25);
        let pain = b.add(p);
        let death = b.add_all(death_frames(&draw_demon(0, 1.0), 126.0, 5, 3));
        MonsterSprites { walk, attack, pain, death }
    };

    let caco = {
        let idle = draw_caco(0.2, false, false);
        let walk = vec![b.add(idle.clone()), b.add(draw_caco(0.35, false, false))];
        let attack = vec![b.add(draw_caco(0.6, false, true)), b.add(draw_caco(1.0, false, true)), b.add(draw_caco(0.5, false, false))];
        let mut p = draw_caco(0.8, true, false);
        tint(&mut p, rgb(255, 80, 60), 0.2);
        let pain = b.add(p);
        // Floating corpse drops and splats: pivot at the sphere's bottom.
        let death = b.add_all(death_frames(&draw_caco(1.0, true, false), 100.0, 5, 4));
        MonsterSprites { walk, attack, pain, death }
    };

    let imp_ball = vec![
        b.add(fx_ball(9.0, rgb(255, 250, 200), rgb(255, 100, 20), 1)),
        b.add(fx_ball(10.0, rgb(255, 240, 170), rgb(240, 80, 10), 2)),
    ];
    let caco_ball = vec![
        b.add(fx_ball(10.0, rgb(255, 220, 255), rgb(200, 50, 240), 3)),
        b.add(fx_ball(11.0, rgb(255, 200, 255), rgb(170, 40, 220), 4)),
    ];
    let rocket = vec![
        b.add(fx_ball(6.0, rgb(255, 255, 240), rgb(255, 170, 60), 5)),
        b.add(fx_ball(7.0, rgb(255, 250, 220), rgb(255, 140, 40), 6)),
    ];
    let explosion = b.add_all((0..3).map(explosion).collect());
    let puff = b.add_all((0..3).map(puff).collect());
    let blood = b.add_all((0..3).map(blood).collect());

    let stimpack = b.add(medkit(false));
    let medikit = b.add(medkit(true));
    let clip = b.add(clip());
    let shells_ = b.add(shells(false));
    let shell_box = b.add(shells(true));
    let rocket_ammo = b.add(draw_rocket_ammo(false));
    let rocket_box = b.add(draw_rocket_ammo(true));
    let green_armor = b.add(armor(rgb(60, 170, 60)));
    let blue_armor = b.add(armor(rgb(60, 90, 230)));
    let potion = vec![b.add(potion(false)), b.add(potion(true))];
    let soulsphere = vec![b.add(soulsphere(false)), b.add(soulsphere(true))];
    let shotgun_pickup = b.add(weapon_pickup(0));
    let chaingun_pickup = b.add(weapon_pickup(1));
    let launcher_pickup = b.add(weapon_pickup(2));
    let key_cols = [rgb(230, 30, 30), rgb(50, 90, 255), rgb(250, 210, 30)];
    let keys = key_cols.map(|c| vec![b.add(keycard(c, false)), b.add(keycard(c, true))]);
    let barrel = vec![b.add(barrel(0)), b.add(barrel(1))];
    let lamp = b.add(tech_lamp());
    let torch_red = (0..3).map(|f| b.add(torch(f, rgb(255, 60, 20)))).collect();
    let torch_blue = (0..3).map(|f| b.add(torch(f, rgb(60, 110, 255)))).collect();
    let skull_pole = b.add(skull_pole());

    let pistol = WeaponSprites {
        frames: vec![b.add(pistol(false)), b.add(pistol(true))],
        flash: vec![b.add(muzzle_flash(64.0, 62.0, 8.0, 7))],
    };
    let shotgun = WeaponSprites {
        frames: (0..4).map(|f| b.add(shotgun(f))).collect(),
        flash: vec![b.add(muzzle_flash(64.0, 50.0, 12.0, 8))],
    };
    let chaingun = WeaponSprites {
        frames: vec![b.add(chaingun(0)), b.add(chaingun(1))],
        flash: vec![b.add(muzzle_flash(64.0, 52.0, 10.0, 9)), b.add(muzzle_flash(64.0, 50.0, 12.0, 10))],
    };
    let launcher = WeaponSprites {
        frames: vec![b.add(launcher(false)), b.add(launcher(true))],
        flash: vec![b.add(muzzle_flash(64.0, 50.0, 14.0, 11))],
    };

    // Face sheets.
    let mut sheets = [blank(), blank()];
    let mut faces: Vec<Image> = Vec::new();
    for tier in 0..5 {
        for look in -1..=1 {
            faces.push(face(tier, look, Expr::Normal));
        }
    }
    faces.push(face(2, 0, Expr::Ouch));
    faces.push(face(0, 0, Expr::Grin));
    faces.push(face(4, 0, Expr::Dead));
    for (i, f) in faces.iter().enumerate() {
        let sheet = &mut sheets[i / 16];
        let slot = i % 16;
        blit(sheet, f, (slot % 4) as i32 * 32, (slot / 4) as i32 * 32);
    }
    let [s0, s1] = sheets;
    let face_layers = [b.add(s0), b.add(s1)];
    let font = b.add(font_layer());
    let statusbar = b.add(statusbar_tile());

    Sprites {
        layers: b.layers,
        zombie,
        imp,
        demon,
        caco,
        imp_ball,
        caco_ball,
        rocket,
        explosion,
        puff,
        blood,
        stimpack,
        medikit,
        clip,
        shells: shells_,
        shell_box,
        rocket_ammo,
        rocket_box,
        green_armor,
        blue_armor,
        potion,
        soulsphere,
        shotgun_pickup,
        chaingun_pickup,
        launcher_pickup,
        keys,
        barrel,
        lamp,
        torch_red,
        torch_blue,
        skull_pole,
        pistol,
        shotgun,
        chaingun,
        launcher,
        face_layers,
        font,
        statusbar,
    }
}
