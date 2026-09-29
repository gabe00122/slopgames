//! 2D overlay: first-person weapon, status bar, automap, messages and menus.
//! Everything is emitted as textured quads in internal-resolution pixels.

use crate::font;
use crate::game::{Game, Phase, Weapon, WeaponPhase};
use crate::map::Key;
use crate::render::{Light, Quad, View};
use crate::sprites::{face_index, Sprites, FACE_DEAD, FACE_GRIN, FACE_OUCH, SPR};

/// Status bar design size in HUD units (`View::u` pixels each).
pub const BAR_W: f32 = 532.0;
pub const BAR_H: f32 = 64.0;

const RED: [f32; 4] = [0.86, 0.08, 0.05, 1.0];
const LABEL: [f32; 4] = [0.86, 0.84, 0.78, 1.0];
const YELLOW: [f32; 4] = [1.0, 0.82, 0.2, 1.0];
const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

struct Ui<'a> {
    q: Vec<Quad>,
    w: f32,
    h: f32,
    spr: &'a Sprites,
}

fn glyph_uv(ch: u8) -> [f32; 4] {
    let s = 1.0 / SPR as f32;
    let (x, y) = ((ch % 16) as f32 * 8.0, (ch / 16) as f32 * 8.0);
    [x * s, y * s, (x + 8.0) * s, (y + 8.0) * s]
}

impl Ui<'_> {
    fn image(&mut self, x: f32, y: f32, w: f32, h: f32, layer: u32, uv: [f32; 4], color: [f32; 4]) {
        let x0 = x / self.w * 2.0 - 1.0;
        let y0 = 1.0 - y / self.h * 2.0;
        let x1 = (x + w) / self.w * 2.0 - 1.0;
        let y1 = 1.0 - (y + h) / self.h * 2.0;
        self.q.push(Quad { rect: [x0, y0, x1, y1], uv, color, params: [layer as f32, 0.0, 0.0, 0.0] });
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        // Sample the middle of the solid glyph so edges never bleed.
        let s = 1.0 / SPR as f32;
        let (gx, gy) = ((font::SOLID % 16) as f32 * 8.0, (font::SOLID / 16) as f32 * 8.0);
        let uv = [(gx + 2.0) * s, (gy + 2.0) * s, (gx + 6.0) * s, (gy + 6.0) * s];
        self.image(x, y, w, h, self.spr.font, uv, color);
    }

    fn text(&mut self, x: f32, y: f32, scale: f32, s: &str, color: [f32; 4]) {
        let adv = 8.0 * scale;
        for (i, ch) in s.bytes().enumerate() {
            if ch != b' ' {
                self.image(x + i as f32 * adv, y, adv, adv, self.spr.font, glyph_uv(ch), color);
            }
        }
    }

    fn text_shadow(&mut self, x: f32, y: f32, scale: f32, s: &str, color: [f32; 4]) {
        let o = scale.max(1.0);
        self.text(x + o, y + o, scale, s, [0.0, 0.0, 0.0, 0.85 * color[3]]);
        self.text(x, y, scale, s, color);
    }

    fn centered(&mut self, cx: f32, y: f32, scale: f32, s: &str, color: [f32; 4]) {
        let w = s.len() as f32 * 8.0 * scale;
        self.text_shadow((cx - w * 0.5).round(), y, scale, s, color);
    }

    fn right(&mut self, rx: f32, y: f32, scale: f32, s: &str, color: [f32; 4]) {
        let w = s.len() as f32 * 8.0 * scale;
        self.text_shadow(rx - w, y, scale, s, color);
    }
}

pub fn build(game: &Game, view: &View, spr: &Sprites, lights: &[Light]) -> Vec<Quad> {
    let mut ui = Ui { q: Vec::new(), w: view.w, h: view.h, spr };
    let u = view.u;
    match game.phase {
        Phase::Title => title(&mut ui, game, view),
        Phase::Playing => {
            weapon(&mut ui, game, view, lights);
            if game.show_automap {
                automap(&mut ui, game, view);
            } else if !game.player.dead {
                // Crosshair.
                let (cx, cy) = ((view.w * 0.5).floor(), (view.view_h * 0.5).floor());
                ui.rect(cx - u, cy, 3.0 * u, u, [1.0, 1.0, 1.0, 0.55]);
                ui.rect(cx, cy - u, u, 3.0 * u, [1.0, 1.0, 1.0, 0.55]);
            }
            status_bar(&mut ui, game, view);
            for (i, (m, t)) in game.messages.iter().enumerate() {
                let a = t.min(1.0);
                ui.text_shadow(4.0 * u, (4.0 + i as f32 * 10.0) * u, u, &m.to_uppercase(), [0.95, 0.85, 0.55, a]);
            }
            if game.player.dead {
                ui.centered(view.w * 0.5, view.view_h * 0.35, 4.0 * u, "YOU DIED", RED);
                if game.player.death_t > 1.2 {
                    ui.centered(view.w * 0.5, view.view_h * 0.35 + 44.0 * u, u, "PRESS FIRE TO TRY AGAIN", LABEL);
                }
            }
            if game.paused {
                pause(&mut ui, view);
            }
        }
        Phase::Intermission => intermission(&mut ui, game, view),
        Phase::Victory => victory(&mut ui, game, view),
    }
    ui.q
}

fn weapon(ui: &mut Ui, game: &Game, view: &View, lights: &[Light]) {
    let p = &game.player;
    if p.dead && p.death_t > 0.5 {
        return;
    }
    let spr = ui.spr;
    let ws = match p.weapon {
        Weapon::Pistol => &spr.pistol,
        Weapon::Shotgun => &spr.shotgun,
        Weapon::Chaingun => &spr.chaingun,
        Weapon::Launcher => &spr.launcher,
    };
    let (frame, flash) = match p.phase {
        WeaponPhase::Firing(t) => match p.weapon {
            Weapon::Pistol => (usize::from(t < 0.12), t < 0.07),
            Weapon::Shotgun => {
                let f = if t < 0.12 {
                    1
                } else if t < 0.3 {
                    0
                } else if t < 0.5 {
                    2
                } else if t < 0.7 {
                    3
                } else {
                    0
                };
                (f, t < 0.08)
            }
            Weapon::Chaingun => (p.chaingun_frame, t < 0.06),
            Weapon::Launcher => (usize::from(t < 0.2), t < 0.1),
        },
        _ => (0, false),
    };
    let lower = match p.phase {
        WeaponPhase::Lowering(t) => t / 0.18,
        WeaponPhase::Raising(t) => 1.0 - t / 0.18,
        _ => 0.0,
    } + if p.dead { p.death_t * 2.0 } else { 0.0 };
    let size = (view.view_h * 0.8).round();
    let bob = p.bob;
    let bx = (p.bob_phase.cos() * 12.0 * view.u * bob).round();
    let by = ((p.bob_phase.sin()).abs() * 9.0 * view.u * bob).round();
    let x = ((view.w - size) * 0.5).round() + bx;
    let y = view.view_h - size + by + size * 0.04 + lower.min(1.0) * size * 0.6;
    let mut light = game.weapon_light(lights);
    if p.muzzle_t > 0.0 {
        light = light.map(|c| (c + 0.35).min(1.6));
    }
    ui.image(x, y, size, size, ws.frames[frame.min(ws.frames.len() - 1)], [0.0, 0.0, 1.0, 1.0], [light[0], light[1], light[2], 1.0]);
    if flash && !ws.flash.is_empty() {
        let fi = if p.weapon == Weapon::Chaingun { p.chaingun_frame } else { 0 };
        ui.image(x, y, size, size, ws.flash[fi.min(ws.flash.len() - 1)], [0.0, 0.0, 1.0, 1.0], WHITE);
    }
}

fn status_bar(ui: &mut Ui, game: &Game, view: &View) {
    let u = view.u;
    let p = &game.player;
    let y0 = view.h - BAR_H * u;
    let tile_w = SPR as f32 * u;
    let mut x = 0.0;
    while x < view.w {
        ui.image(x, y0, tile_w, BAR_H * u, ui.spr.statusbar, [0.0, 0.0, 1.0, 0.5], [0.85, 0.85, 0.85, 1.0]);
        x += tile_w;
    }
    ui.rect(0.0, y0, view.w, u, [0.1, 0.09, 0.08, 1.0]);
    let bx = ((view.w - BAR_W * u) * 0.5).floor();
    let at = |v: f32| bx + v * u;
    let inset = [0.0, 0.0, 0.0, 0.32];
    for (x0, x1) in [(4.0, 84.0), (88.0, 192.0), (196.0, 244.0), (316.0, 420.0), (424.0, 440.0), (444.0, 528.0)] {
        ui.rect(at(x0), y0 + 4.0 * u, (x1 - x0) * u, 56.0 * u, inset);
    }
    let big = 3.0 * u;

    // Ammo for the current weapon.
    let ammo = p.ammo[p.weapon.ammo_type()];
    ui.right(at(80.0), y0 + 10.0 * u, big, &ammo.to_string(), RED);
    ui.centered(at(44.0), y0 + 46.0 * u, u, "AMMO", LABEL);

    ui.right(at(188.0), y0 + 10.0 * u, big, &format!("{}%", p.health), RED);
    ui.centered(at(140.0), y0 + 46.0 * u, u, "HEALTH", LABEL);

    // Owned weapons.
    for (i, w) in Weapon::ALL.iter().enumerate() {
        let owned = p.owned[w.index()];
        let col = if *w == p.weapon {
            WHITE
        } else if owned {
            YELLOW
        } else {
            [0.35, 0.33, 0.3, 1.0]
        };
        let (gx, gy) = (at(202.0 + (i % 2) as f32 * 22.0), y0 + (7.0 + (i / 2) as f32 * 18.0) * u);
        ui.text_shadow(gx, gy, 2.0 * u, &(i + 1).to_string(), col);
    }
    ui.centered(at(220.0), y0 + 46.0 * u, u, "ARMS", LABEL);

    // Face.
    let idx = if p.dead {
        FACE_DEAD
    } else if p.ouch_t > 0.0 {
        FACE_OUCH
    } else if p.grin_t > 0.0 {
        FACE_GRIN
    } else {
        let tier = match p.health {
            80.. => 0,
            60..=79 => 1,
            40..=59 => 2,
            20..=39 => 3,
            _ => 4,
        };
        face_index(tier, p.face_look)
    };
    let slot = idx % 16;
    let s = 32.0 / SPR as f32;
    let (fu, fv) = ((slot % 4) as f32 * s, (slot / 4) as f32 * s);
    ui.rect(at(248.0), y0 + 2.0 * u, 64.0 * u, 62.0 * u, [0.0, 0.0, 0.0, 0.45]);
    ui.image(at(248.0), y0, 64.0 * u, 64.0 * u, ui.spr.face_layers[idx / 16], [fu, fv, fu + s, fv + s], WHITE);

    ui.right(at(416.0), y0 + 10.0 * u, big, &format!("{}%", p.armor), RED);
    ui.centered(at(368.0), y0 + 46.0 * u, u, "ARMOR", LABEL);

    // Keycards.
    for k in [Key::Red, Key::Blue, Key::Yellow] {
        let i = k as usize;
        let y = y0 + (8.0 + i as f32 * 17.0) * u;
        let col = match k {
            Key::Red => [0.9, 0.12, 0.1, 1.0],
            Key::Blue => [0.2, 0.35, 1.0, 1.0],
            Key::Yellow => [1.0, 0.85, 0.15, 1.0],
        };
        if p.keys[i] {
            ui.rect(at(426.0), y, 12.0 * u, 13.0 * u, [0.0, 0.0, 0.0, 1.0]);
            ui.rect(at(427.0), y + u, 10.0 * u, 11.0 * u, col);
        } else {
            ui.rect(at(427.0), y + u, 10.0 * u, 11.0 * u, [0.0, 0.0, 0.0, 0.3]);
        }
    }

    // Ammo inventory.
    for (i, name) in ["BULL", "SHEL", "RCKT"].iter().enumerate() {
        let y = y0 + (10.0 + i as f32 * 16.0) * u;
        ui.text_shadow(at(450.0), y, u, name, LABEL);
        ui.right(at(524.0), y, u, &format!("{}", p.ammo[i]), YELLOW);
    }
}

fn automap(ui: &mut Ui, game: &Game, view: &View) {
    let u = view.u;
    ui.rect(0.0, 0.0, view.w, view.view_h, [0.02, 0.02, 0.03, 0.82]);
    let cs = 6.0 * u;
    let p = game.player.pos;
    let (cx, cy) = (view.w * 0.5, view.view_h * 0.5);
    let map = &game.map;
    for y in 0..map.h {
        for x in 0..map.w {
            if !game.cell_seen(x, y) {
                continue;
            }
            let sx = (cx + (x as f32 - p.x) * cs).round();
            let sy = (cy + (y as f32 - p.y) * cs).round();
            if sx < -cs || sy < -cs || sx > view.w || sy > view.view_h {
                continue;
            }
            let c = map.cell(x, y);
            let col = if let Some(di) = c.door {
                let d = &map.doors[di];
                match d.key {
                    Some(Key::Red) => [0.95, 0.15, 0.1, 1.0],
                    Some(Key::Blue) => [0.25, 0.4, 1.0, 1.0],
                    Some(Key::Yellow) => [1.0, 0.85, 0.2, 1.0],
                    None if d.secret => [0.55, 0.5, 0.45, 1.0],
                    None => [0.85, 0.65, 0.2, 1.0],
                }
            } else if c.exit {
                [0.2, 1.0, 0.3, 1.0]
            } else if c.wall != 0 {
                [0.55, 0.5, 0.45, 1.0]
            } else if c.hazard > 0 {
                [0.15, 0.3, 0.1, 1.0]
            } else {
                [0.12, 0.12, 0.15, 1.0]
            };
            let (w, h) = if c.wall != 0 { (cs, cs) } else { (cs - u, cs - u) };
            let (sx, sy, w, h) = clip(sx, sy, w, h, view.w, view.view_h);
            if w > 0.0 && h > 0.0 {
                ui.rect(sx, sy, w, h, col);
            }
        }
    }
    // Player arrow: a dot and a trail of dots pointing forward.
    let d = game.player.dir();
    ui.rect(cx - 2.0 * u, cy - 2.0 * u, 4.0 * u, 4.0 * u, WHITE);
    for k in 1..=4 {
        let t = k as f32 * 2.0 * u;
        ui.rect((cx + d.x * t - u).round(), (cy + d.y * t - u).round(), 2.0 * u, 2.0 * u, [1.0, 0.9, 0.3, 1.0]);
    }
    ui.text_shadow(4.0 * u, view.view_h - 12.0 * u, u, &format!("{}  -  TAB TO CLOSE", game.level_name()), LABEL);
}

fn clip(x: f32, y: f32, w: f32, h: f32, mw: f32, mh: f32) -> (f32, f32, f32, f32) {
    let x0 = x.max(0.0);
    let y0 = y.max(0.0);
    let x1 = (x + w).min(mw);
    let y1 = (y + h).min(mh);
    (x0, y0, x1 - x0, y1 - y0)
}

fn controls(ui: &mut Ui, view: &View, y: f32) {
    let u = view.u;
    let lines = [
        "WASD MOVE   MOUSE TURN   SHIFT RUN",
        "CLICK/CTRL FIRE   E/SPACE USE   1-4 WEAPONS",
        "TAB MAP   F2 RESOLUTION   F11 FULLSCREEN   ESC PAUSE",
    ];
    for (i, l) in lines.iter().enumerate() {
        ui.centered(view.w * 0.5, y + i as f32 * 12.0 * u, u, l, [0.8, 0.78, 0.72, 1.0]);
    }
}

fn title(ui: &mut Ui, game: &Game, view: &View) {
    let u = view.u;
    ui.rect(0.0, 0.0, view.w, view.h, [0.0, 0.0, 0.0, 0.45]);
    let big = (view.w / (8.0 * 8.0 * 1.15)).floor().clamp(2.0, 8.0 * u);
    let y = (view.h * 0.2).round();
    ui.centered(view.w * 0.5 + big, y + big, big, "HELLCAST", [0.2, 0.0, 0.0, 1.0]);
    ui.centered(view.w * 0.5, y, big, "HELLCAST", [0.9, 0.12, 0.05, 1.0]);
    ui.centered(view.w * 0.5, y + 10.0 * big, u, "A RAYCASTING DOOM-LIKE IN RUST + WGPU", LABEL);
    if (game.title_t * 2.0) as i32 % 2 == 0 {
        ui.centered(view.w * 0.5, (view.h * 0.55).round(), 2.0 * u, "CLICK TO START", YELLOW);
    }
    controls(ui, view, (view.h * 0.75).round());
}

fn pause(ui: &mut Ui, view: &View) {
    let u = view.u;
    ui.rect(0.0, 0.0, view.w, view.h, [0.0, 0.0, 0.0, 0.55]);
    ui.centered(view.w * 0.5, (view.h * 0.25).round(), 4.0 * u, "PAUSED", RED);
    ui.centered(view.w * 0.5, (view.h * 0.25).round() + 44.0 * u, u, "CLICK TO RESUME  -  ESC AGAIN TO QUIT", LABEL);
    controls(ui, view, (view.h * 0.6).round());
}

fn fmt_time(t: f32) -> String {
    let s = t as i32;
    format!("{}:{:02}", s / 60, s % 60)
}

fn pct(a: i32, b: i32) -> String {
    if b == 0 { "100%".into() } else { format!("{}%", a * 100 / b) }
}

fn intermission(ui: &mut Ui, game: &Game, view: &View) {
    let u = view.u;
    ui.rect(0.0, 0.0, view.w, view.h, [0.05, 0.0, 0.0, 0.8]);
    let s = &game.stats;
    let y = (view.h * 0.18).round();
    ui.centered(view.w * 0.5, y, 2.0 * u, game.level_name(), YELLOW);
    ui.centered(view.w * 0.5, y + 22.0 * u, 2.0 * u, "FINISHED", RED);
    let rows = [
        ("KILLS", pct(s.kills, s.total_kills)),
        ("ITEMS", pct(s.items, s.total_items)),
        ("SECRETS", pct(s.secrets, s.total_secrets)),
        ("TIME", fmt_time(s.time)),
    ];
    let x0 = view.w * 0.5 - 120.0 * u;
    for (i, (k, v)) in rows.iter().enumerate() {
        let yy = y + (64.0 + i as f32 * 26.0) * u;
        ui.text_shadow(x0, yy, 2.0 * u, k, RED);
        ui.right(x0 + 240.0 * u, yy, 2.0 * u, v, LABEL);
    }
    if (game.time * 2.0) as i32 % 2 == 0 {
        ui.centered(view.w * 0.5, view.h - 40.0 * u, u, "PRESS FIRE TO CONTINUE", YELLOW);
    }
}

fn victory(ui: &mut Ui, game: &Game, view: &View) {
    let u = view.u;
    ui.rect(0.0, 0.0, view.w, view.h, [0.08, 0.0, 0.0, 0.7]);
    let y = (view.h * 0.25).round();
    ui.centered(view.w * 0.5, y, 4.0 * u, "VICTORY", RED);
    ui.centered(view.w * 0.5, y + 48.0 * u, u, "THE HELL GATE IS SEALED. THE DEMONS ARE SILENT.", LABEL);
    ui.centered(view.w * 0.5, y + 62.0 * u, u, "FOR NOW.", LABEL);
    if (game.time * 2.0) as i32 % 2 == 0 {
        ui.centered(view.w * 0.5, view.h - 40.0 * u, u, "PRESS FIRE TO RETURN TO THE TITLE", YELLOW);
    }
}
