//! In-raid heads-up display.

use egui::{Align2, Color32, FontId, LayerId, Order, Pos2, Stroke};

use super::style;
use crate::game::{FrameStats, Settings};
use crate::raid::Raid;

fn hud_painter(ctx: &egui::Context) -> egui::Painter {
    ctx.layer_painter(LayerId::new(Order::Foreground, egui::Id::new("hud")))
}

pub fn draw_crosshair(p: &egui::Painter, center: Pos2, gap: f32, color: Color32) {
    let len = 6.0;
    let s = Stroke::new(2.0, color);
    p.line_segment([center + egui::vec2(gap, 0.0), center + egui::vec2(gap + len, 0.0)], s);
    p.line_segment([center - egui::vec2(gap, 0.0), center - egui::vec2(gap + len, 0.0)], s);
    p.line_segment([center + egui::vec2(0.0, gap), center + egui::vec2(0.0, gap + len)], s);
    p.line_segment([center - egui::vec2(0.0, gap), center - egui::vec2(0.0, gap + len)], s);
}

pub fn draw_hud(ui: &mut egui::Ui, raid: &Raid, settings: &Settings, stats: &FrameStats, adapter: &str) {
    let ctx = ui.ctx().clone();
    let screen = ctx.content_rect();
    let p = hud_painter(&ctx);
    let center = screen.center();

    draw_crosshair(&p, center, 4.0, Color32::from_rgba_unmultiplied(230, 230, 220, 200));

    // Stamina bar.
    let player = &raid.player;
    let w = 220.0;
    let bar = egui::Rect::from_min_size(
        Pos2::new(center.x - w / 2.0, screen.bottom() - 34.0),
        egui::vec2(w, 6.0),
    );
    p.rect_filled(bar, 1.0, Color32::from_black_alpha(140));
    let mut fill = bar;
    fill.set_width(w * player.stamina / 100.0);
    p.rect_filled(fill, 1.0, Color32::from_rgb(210, 200, 150));

    // Raid clock.
    let t = raid.time as i32;
    p.text(
        Pos2::new(screen.right() - 16.0, 14.0),
        Align2::RIGHT_TOP,
        format!("{:02}:{:02}", t / 60, t % 60),
        FontId::monospace(18.0),
        style::ACCENT,
    );

    if settings.show_debug {
        let eye = player.eye_pos();
        let dir = player.look_dir();
        let target = raid
            .world
            .raycast(eye, dir, 8.0, |b| b.is_solid())
            .map(|h| format!("{} @ {}", h.block.name(), h.pos))
            .unwrap_or_else(|| "-".into());
        let lines = [
            format!("{:.0} FPS  |  {}", stats.fps, adapter),
            format!("pos {:.1} {:.1} {:.1}  yaw {:.2} pitch {:.2}", player.pos.x, player.pos.y, player.pos.z, player.yaw, player.pitch),
            format!("chunks drawn {}  tris {}", stats.chunks_drawn, stats.triangles),
            format!("ground {}  crouch {}  noclip {}", player.on_ground, player.crouching, player.noclip),
            format!("looking at {target}"),
            format!("seed {}", raid.seed),
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
