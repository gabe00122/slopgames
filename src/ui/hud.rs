//! In-raid heads-up display.

use egui::{Align2, Color32, FontId, LayerId, Order, Pos2, Rect, Stroke, StrokeKind, Vec2};

use super::style;
use crate::game::{FrameStats, Settings};
use crate::player::BodyPart;
use crate::inventory::EquipSlot;
use crate::raid::{Raid, EXTRACT_TIME};
use crate::weapons::{AttachmentId, Slot};

fn hud_painter(ctx: &egui::Context) -> egui::Painter {
    ctx.layer_painter(LayerId::new(Order::Foreground, egui::Id::new("hud")))
}

fn c3(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn draw_crosshair(p: &egui::Painter, center: Pos2, gap: f32, color: Color32) {
    let len = 7.0;
    let s = Stroke::new(2.0, color);
    p.line_segment([center + egui::vec2(gap, 0.0), center + egui::vec2(gap + len, 0.0)], s);
    p.line_segment([center - egui::vec2(gap, 0.0), center - egui::vec2(gap + len, 0.0)], s);
    p.line_segment([center + egui::vec2(0.0, gap), center + egui::vec2(0.0, gap + len)], s);
    p.line_segment([center - egui::vec2(0.0, gap), center - egui::vec2(0.0, gap + len)], s);
}

pub fn text(p: &egui::Painter, pos: Pos2, align: Align2, s: impl ToString, size: f32, color: Color32) -> Rect {
    // Drop shadow for readability over bright scenes.
    p.text(pos + egui::vec2(1.0, 1.0), align, s.to_string(), FontId::proportional(size), Color32::from_black_alpha(180));
    p.text(pos, align, s.to_string(), FontId::proportional(size), color)
}

fn scope_overlay(p: &egui::Painter, screen: Rect) {
    let c = screen.center();
    let r = screen.height() * 0.46;
    // Mask everything outside the lens with an annulus mesh.
    let outer = screen.width().max(screen.height()) * 1.5;
    let mut mesh = egui::Mesh::default();
    let n = 96;
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        let d = Vec2::new(a.cos(), a.sin());
        mesh.colored_vertex(c + d * r, Color32::BLACK);
        mesh.colored_vertex(c + d * outer, Color32::BLACK);
    }
    for i in 0..n as u32 {
        let j = (i + 1) % n as u32;
        let (a0, b0, a1, b1) = (i * 2, i * 2 + 1, j * 2, j * 2 + 1);
        mesh.add_triangle(a0, b0, b1);
        mesh.add_triangle(a0, b1, a1);
    }
    p.add(egui::Shape::mesh(mesh));
    // Soft vignette inside the lens edge.
    p.circle_stroke(c, r - 10.0, Stroke::new(22.0, Color32::from_black_alpha(90)));
    p.circle_stroke(c, r, Stroke::new(3.0, Color32::from_rgb(20, 20, 20)));
    let ink = Stroke::new(1.6, Color32::from_rgb(10, 10, 10));
    // PSO-style chevron and stadia lines.
    p.line_segment([c + egui::vec2(-10.0, 12.0), c], ink);
    p.line_segment([c + egui::vec2(10.0, 12.0), c], ink);
    p.line_segment([c + egui::vec2(0.0, 12.0), c + egui::vec2(0.0, r)], ink);
    p.line_segment([c + egui::vec2(-r, 0.0), c + egui::vec2(-24.0, 0.0)], ink);
    p.line_segment([c + egui::vec2(24.0, 0.0), c + egui::vec2(r, 0.0)], ink);
    for i in 1..=3 {
        let y = c.y + 24.0 * i as f32;
        p.line_segment([Pos2::new(c.x - 7.0, y), Pos2::new(c.x - 3.0, y)], ink);
        p.line_segment([Pos2::new(c.x + 3.0, y), Pos2::new(c.x + 7.0, y)], ink);
    }
}

fn body_diagram(p: &egui::Painter, origin: Pos2, raid: &Raid) {
    let body = &raid.player.body;
    let layout: [(BodyPart, [f32; 4]); 7] = [
        (BodyPart::Head, [34.0, 0.0, 28.0, 26.0]),
        (BodyPart::Thorax, [24.0, 30.0, 48.0, 40.0]),
        (BodyPart::Stomach, [26.0, 73.0, 44.0, 28.0]),
        (BodyPart::LeftArm, [2.0, 30.0, 18.0, 62.0]),
        (BodyPart::RightArm, [76.0, 30.0, 18.0, 62.0]),
        (BodyPart::LeftLeg, [24.0, 105.0, 21.0, 64.0]),
        (BodyPart::RightLeg, [51.0, 105.0, 21.0, 64.0]),
    ];
    let bg = Rect::from_min_size(origin - egui::vec2(10.0, 30.0), egui::vec2(200.0, 214.0));
    p.rect_filled(bg, 4.0, Color32::from_black_alpha(120));
    for (part, r) in layout {
        let rect = Rect::from_min_size(origin + egui::vec2(r[0], r[1]), egui::vec2(r[2], r[3]));
        let frac = body.part_hp(part) / part.max_hp();
        let col = style::hp_color(frac);
        p.rect_filled(rect, 2.0, col.gamma_multiply(0.85));
        p.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_black_alpha(200)), StrokeKind::Inside);
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            format!("{:.0}", body.part_hp(part)),
            FontId::proportional(11.0),
            Color32::from_rgb(15, 15, 15),
        );
    }
    let total = body.total();
    text(
        p,
        origin + egui::vec2(0.0, -24.0),
        Align2::LEFT_TOP,
        format!("HP {:.0} / {:.0}", total, body.max_total()),
        14.0,
        Color32::from_rgb(230, 230, 215),
    );
    let mut y = 40.0;
    let helmet = raid.equipment.armor_state(EquipSlot::Helmet);
    let armor = raid.equipment.armor_state(EquipSlot::Armor);
    for (label, a) in [("Helmet", helmet), ("Armor", armor)] {
        if let Some(a) = a {
            let d = a.kind.def();
            text(
                p,
                origin + egui::vec2(104.0, y),
                Align2::LEFT_TOP,
                format!("{label} C{}\n{:.0}/{:.0}", d.class, a.durability, d.max_durability),
                11.5,
                if a.durability > 0.0 { Color32::from_rgb(190, 200, 210) } else { style::BAD },
            );
            y += 34.0;
        }
    }
}

fn weapon_panel(p: &egui::Painter, screen: Rect, raid: &Raid) {
    let Some(w) = raid.weapon() else {
        text(p, Pos2::new(screen.right() - 20.0, screen.bottom() - 30.0), Align2::RIGHT_BOTTOM, "Unarmed", 16.0, style::TEXT_DIM);
        return;
    };
    let stats = w.stats();
    let right = screen.right() - 24.0;
    let bottom = screen.bottom() - 24.0;
    let bg = Rect::from_min_max(Pos2::new(right - 290.0, bottom - 108.0), Pos2::new(right + 12.0, bottom + 12.0));
    p.rect_filled(bg, 4.0, Color32::from_black_alpha(120));
    text(p, Pos2::new(right, bottom - 100.0), Align2::RIGHT_TOP, w.name(), 16.0, Color32::from_rgb(230, 225, 205));
    let mode = if stats.full_auto && w.auto_mode { "AUTO" } else { "SEMI" };
    let loaded = w.loaded.map(|a| a.def().short).unwrap_or("-");
    text(
        p,
        Pos2::new(right, bottom - 78.0),
        Align2::RIGHT_TOP,
        format!("{mode}  |  {}", loaded),
        13.0,
        style::TEXT_DIM,
    );
    let ammo_col = if w.rounds == 0 {
        style::BAD
    } else if w.rounds * 4 <= stats.mag_size {
        style::WARN
    } else {
        Color32::from_rgb(240, 238, 225)
    };
    text(
        p,
        Pos2::new(right - 70.0, bottom - 4.0),
        Align2::RIGHT_BOTTOM,
        format!("{}", w.rounds),
        38.0,
        ammo_col,
    );
    text(
        p,
        Pos2::new(right, bottom - 10.0),
        Align2::RIGHT_BOTTOM,
        format!("/ {}\n+{}", stats.mag_size, raid.reserve_for_active()),
        14.0,
        style::TEXT_DIM,
    );
    if let Some(next) = raid.next_reload_ammo() {
        text(
            p,
            Pos2::new(right - 280.0, bottom - 4.0),
            Align2::LEFT_BOTTOM,
            format!("R: reload {}\nT: change ammo", next.def().short),
            11.5,
            style::TEXT_DIM,
        );
    }
    if let Some(r) = raid.gun.reload {
        let frac = 1.0 - r.remaining / r.total.max(0.01);
        let c = screen.center() + egui::vec2(0.0, 60.0);
        let bar = Rect::from_center_size(c, egui::vec2(160.0, 6.0));
        p.rect_filled(bar, 2.0, Color32::from_black_alpha(160));
        let mut fill = bar;
        fill.set_width(160.0 * frac);
        p.rect_filled(fill, 2.0, style::ACCENT);
        text(p, c - egui::vec2(0.0, 8.0), Align2::CENTER_BOTTOM, format!("Reloading {}", r.ammo.def().short), 13.0, style::ACCENT);
    }
}

fn extract_panel(p: &egui::Painter, screen: Rect, raid: &Raid) {
    let show = raid.show_extracts > 0.0 || raid.extracting_at.is_some();
    if !show {
        return;
    }
    let player = &raid.player;
    let mut y = 64.0;
    text(p, Pos2::new(screen.right() - 16.0, y), Align2::RIGHT_TOP, "EXFILTRATION", 13.0, style::GOOD);
    y += 20.0;
    for e in &raid.extracts {
        let d = e.pos - player.pos;
        let dist = Vec2::new(d.x, d.z).length();
        // Compass bearing relative to where the player is facing.
        let fwd = player.forward_flat();
        let right = player.right_flat();
        let ang = d.dot(right).atan2(d.dot(fwd));
        let r = text(
            p,
            Pos2::new(screen.right() - 36.0, y),
            Align2::RIGHT_TOP,
            format!("{}  {:.0} m", e.name, dist),
            14.0,
            Color32::from_rgb(200, 240, 200),
        );
        // Direction arrow relative to the view (up = straight ahead).
        let c = Pos2::new(screen.right() - 22.0, r.center().y);
        let dir = Vec2::new(ang.sin(), -ang.cos());
        let perp = Vec2::new(-dir.y, dir.x);
        p.add(egui::Shape::convex_polygon(
            vec![c + dir * 8.0, c - dir * 6.0 + perp * 6.0, c - dir * 6.0 - perp * 6.0],
            Color32::from_rgb(140, 230, 150),
            Stroke::NONE,
        ));
        y += 19.0;
    }
}

pub fn draw_hud(ui: &mut egui::Ui, raid: &Raid, settings: &Settings, stats: &FrameStats, adapter: &str, inventory_open: bool) {
    let ctx = ui.ctx().clone();
    let screen = ctx.content_rect();
    let p = hud_painter(&ctx);
    let center = screen.center();
    let player = &raid.player;
    let alive = raid.dead.is_none();

    // --- Aiming overlays ---
    if alive && !inventory_open {
        let ads = raid.gun.ads;
        let sight = raid.weapon().and_then(|w| w.attachment(Slot::Sight));
        if raid.is_scoped() {
            scope_overlay(&p, screen);
        } else if ads > 0.85 {
            match sight {
                Some(AttachmentId::CobraRedDot) | Some(AttachmentId::Rmr) => {
                    p.circle_filled(center, 2.2, Color32::from_rgb(255, 40, 30));
                }
                Some(AttachmentId::Holo1p87) => {
                    p.circle_stroke(center, 14.0, Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 60, 40, 220)));
                    p.circle_filled(center, 1.8, Color32::from_rgb(255, 60, 40));
                }
                _ => {}
            }
        } else if ads < 0.5 {
            let spread = raid.weapon().map(|w| w.stats().hip_spread).unwrap_or(1.0);
            let moving = 1.0 + player.horizontal_speed() / 5.0;
            let gap = 3.0 + spread * moving * 5.0 * (1.0 - ads * 2.0);
            draw_crosshair(&p, center, gap, Color32::from_rgba_unmultiplied(230, 230, 220, 190));
        }

        if raid.hit_marker > 0.0 {
            let col = if raid.hit_marker_kill {
                Color32::from_rgb(255, 60, 50)
            } else {
                Color32::from_rgb(240, 240, 240)
            };
            let s = Stroke::new(2.0, col);
            for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let a = center + egui::vec2(dx * 6.0, dy * 6.0);
                let b = center + egui::vec2(dx * 13.0, dy * 13.0);
                p.line_segment([a, b], s);
            }
        }
    }

    // --- Damage feedback ---
    if raid.damage_flash > 0.0 {
        let a = (raid.damage_flash * 140.0) as u8;
        p.rect_stroke(screen, 0.0, Stroke::new(60.0, Color32::from_rgba_unmultiplied(170, 0, 0, a)), StrokeKind::Inside);
    }
    if raid.hit_indicator > 0.0 {
        if let Some(from) = raid.last_hit_from {
            let d = from - player.pos;
            let fwd = player.forward_flat();
            let right = player.right_flat();
            let ang = d.dot(right).atan2(d.dot(fwd));
            let r = 130.0;
            let dir = Vec2::new(ang.sin(), -ang.cos());
            let tip = center + dir * (r + 22.0);
            let perp = Vec2::new(-dir.y, dir.x);
            let base = center + dir * r;
            let a = (raid.hit_indicator.min(1.0) * 220.0) as u8;
            p.add(egui::Shape::convex_polygon(
                vec![tip, base + perp * 16.0, base - perp * 16.0],
                Color32::from_rgba_unmultiplied(220, 30, 20, a),
                Stroke::NONE,
            ));
        }
    }

    // --- Stamina ---
    let w = 220.0;
    let bar = Rect::from_min_size(Pos2::new(center.x - w / 2.0, screen.bottom() - 34.0), egui::vec2(w, 6.0));
    p.rect_filled(bar, 1.0, Color32::from_black_alpha(140));
    let mut fill = bar;
    fill.set_width(w * player.stamina / 100.0);
    p.rect_filled(fill, 1.0, Color32::from_rgb(210, 200, 150));

    // --- Panels ---
    body_diagram(&p, Pos2::new(24.0, screen.bottom() - 200.0), raid);
    weapon_panel(&p, screen, raid);

    // Raid clock (time remaining) and kills.
    let t = raid.time_left() as i32;
    text(
        &p,
        Pos2::new(screen.right() - 16.0, 12.0),
        Align2::RIGHT_TOP,
        format!("{:02}:{:02}", t / 60, t % 60),
        20.0,
        if t < 120 { style::BAD } else { style::ACCENT },
    );
    extract_panel(&p, screen, raid);

    // Extraction countdown.
    if let Some(i) = raid.extracting_at {
        let left = (EXTRACT_TIME - raid.extract_progress).max(0.0);
        text(
            &p,
            center + egui::vec2(0.0, -140.0),
            Align2::CENTER_CENTER,
            format!("EXTRACTING - {}  {:.1}", raid.extracts[i].name, left),
            22.0,
            style::GOOD,
        );
    }

    // Interaction prompt.
    if alive && !inventory_open {
        if let Some(i) = raid.interaction() {
            text(&p, center + egui::vec2(0.0, 48.0), Align2::CENTER_CENTER, raid.interaction_label(i), 16.0, Color32::WHITE);
        }
    }

    // Healing progress.
    if let Some(h) = raid.heal {
        let frac = 1.0 - h.remaining / h.total.max(0.01);
        let c = center + egui::vec2(0.0, 90.0);
        let bar = Rect::from_center_size(c, egui::vec2(180.0, 6.0));
        p.rect_filled(bar, 2.0, Color32::from_black_alpha(160));
        let mut fill = bar;
        fill.set_width(180.0 * frac);
        p.rect_filled(fill, 2.0, style::GOOD);
        text(
            &p,
            c - egui::vec2(0.0, 8.0),
            Align2::CENTER_BOTTOM,
            format!("Using {}", crate::inventory::ItemKind::Med(h.kind).name()),
            13.0,
            style::GOOD,
        );
    }
    if !inventory_open && alive {
        text(
            &p,
            Pos2::new(center.x, screen.bottom() - 14.0),
            Align2::CENTER_BOTTOM,
            "Tab inventory · F search · H heal · O exits",
            11.5,
            Color32::from_white_alpha(110),
        );
    }
    text(
        &p,
        Pos2::new(screen.right() - 16.0, 38.0),
        Align2::RIGHT_TOP,
        format!("Kills: {}", raid.kills),
        14.0,
        style::TEXT_DIM,
    );

    // Message feed (hidden behind the inventory).
    for (i, m) in raid.messages.iter().enumerate().filter(|_| !inventory_open) {
        let alpha = (m.ttl.min(1.0) * 255.0) as u8;
        let c = c3(m.color);
        text(
            &p,
            Pos2::new(24.0, screen.height() * 0.35 + i as f32 * 20.0),
            Align2::LEFT_TOP,
            &m.text,
            14.0,
            Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha),
        );
    }

    if settings.show_debug {
        let eye = player.eye_pos();
        let dir = player.look_dir();
        let target = raid
            .world
            .raycast(eye, dir, 8.0, |b| b.is_solid())
            .map(|h| format!("{} @ {} (dmg {:.0}%)", h.block.name(), h.pos, raid.world.damage_frac(h.pos) * 100.0))
            .unwrap_or_else(|| "-".into());
        let alive_scavs = raid.scavs.iter().filter(|s| s.alive()).count();
        let lines = [
            format!("{:.0} FPS  |  {}", stats.fps, adapter),
            format!(
                "pos {:.1} {:.1} {:.1}  yaw {:.2} pitch {:.2}",
                player.pos.x, player.pos.y, player.pos.z, player.yaw, player.pitch
            ),
            format!("chunks drawn {}  tris {}", stats.chunks_drawn, stats.triangles),
            format!("ground {}  crouch {}  noclip {}", player.on_ground, player.crouching, player.noclip),
            format!("looking at {target}"),
            format!("seed {}  scavs alive {}", raid.seed, alive_scavs),
        ];
        for (i, l) in lines.iter().enumerate() {
            p.text(
                Pos2::new(12.0, 12.0 + i as f32 * 17.0),
                Align2::LEFT_TOP,
                l,
                FontId::monospace(13.0),
                Color32::from_rgb(230, 230, 210),
            );
        }
    }
}

/// Shown when the player has died. Returns true when "continue" is clicked.
pub fn death_overlay(ui: &mut egui::Ui, raid: &Raid) -> bool {
    let ctx = ui.ctx().clone();
    let screen = ctx.content_rect();
    ctx.layer_painter(LayerId::new(Order::Background, egui::Id::new("death dim")))
        .rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(60, 0, 0, 120));
    let mut restart = false;
    egui::Window::new("KILLED IN ACTION")
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .resizable(false)
        .collapsible(false)
        .show(&ctx, |ui| {
            ui.set_min_width(360.0);
            if let Some(d) = &raid.dead {
                ui.label(egui::RichText::new(&d.cause).color(style::BAD).size(16.0));
            }
            let t = raid.time as i32;
            ui.label(format!("Time in raid: {:02}:{:02}", t / 60, t % 60));
            ui.label(format!("Kills: {}", raid.kills));
            ui.label(
                egui::RichText::new("Everything you brought into the raid is lost.")
                    .color(style::WARN),
            );
            ui.add_space(8.0);
            if style::big_button(ui, "Continue", 340.0).clicked() {
                restart = true;
            }
        });
    restart
}
