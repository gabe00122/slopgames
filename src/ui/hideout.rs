//! Hideout HUD (build hotbar) and station upgrade / crafting window.

use egui::{Align2, Color32, FontId, LayerId, Order, Pos2, Rect, RichText, Stroke, StrokeKind};

use super::hud::{draw_crosshair, text};
use super::style;
use crate::hideout::world::{HideoutInteract, HideoutSession};
use crate::hideout::{can_afford, have, recipes, HideoutState, StationKind};
use crate::inventory::{Grid, ItemKind};
use crate::world::Block;

pub fn draw_hud(ui: &mut egui::Ui, session: &HideoutSession, state: &HideoutState) {
    let ctx = ui.ctx().clone();
    let screen = ctx.content_rect();
    let p = ctx.layer_painter(LayerId::new(Order::Foreground, egui::Id::new("hideout hud")));
    let center = screen.center();
    let overlay_open = session.open_station.is_some() || session.stash_open;
    if !overlay_open {
        draw_crosshair(&p, center, 3.0, Color32::from_rgba_unmultiplied(230, 230, 220, 200));
        let prompt = match session.interaction() {
            Some(HideoutInteract::Stash) => Some("[E/F] Open stash".to_string()),
            Some(HideoutInteract::Station(s)) => Some(format!(
                "[E/F] Use {} (level {}/{})",
                s.name(),
                state.level(s),
                s.max_level()
            )),
            None => None,
        };
        if let Some(t) = prompt {
            text(
                &p,
                center + egui::vec2(0.0, 44.0),
                Align2::CENTER_CENTER,
                t,
                16.0,
                Color32::WHITE,
            );
        }
    }

    // Hotbar.
    let blocks = Block::buildable();
    let slot = 50.0;
    let total = slot * blocks.len() as f32;
    let origin = Pos2::new(center.x - total / 2.0, screen.bottom() - slot - 26.0);
    for (i, b) in blocks.iter().enumerate() {
        let r = Rect::from_min_size(
            origin + egui::vec2(i as f32 * slot, 0.0),
            egui::vec2(slot - 4.0, slot - 4.0),
        );
        p.rect_filled(r, 3.0, Color32::from_black_alpha(160));
        let c = crate::render::atlas::tile_color(b.info().tiles[1]);
        p.rect_filled(r.shrink(8.0), 2.0, Color32::from_rgb(c[0], c[1], c[2]));
        let key = match i {
            9 => "0".to_string(),
            10 => "-".to_string(),
            11 => "=".to_string(),
            n => (n + 1).to_string(),
        };
        p.text(
            r.left_top() + egui::vec2(3.0, 1.0),
            Align2::LEFT_TOP,
            key,
            FontId::monospace(10.0),
            Color32::from_white_alpha(200),
        );
        if i == session.selected {
            p.rect_stroke(r, 3.0, Stroke::new(2.5, style::ACCENT), StrokeKind::Outside);
        }
    }
    text(
        &p,
        origin + egui::vec2(total / 2.0, -10.0),
        Align2::CENTER_BOTTOM,
        session.selected_block().name(),
        15.0,
        style::ACCENT,
    );
    text(
        &p,
        Pos2::new(center.x, screen.bottom() - 8.0),
        Align2::CENTER_BOTTOM,
        "LMB remove · RMB place · 1-0/scroll select block · E/F use · Tab stash · Esc menu",
        11.5,
        Color32::from_white_alpha(130),
    );

    // Station overview.
    text(
        &p,
        Pos2::new(16.0, 14.0),
        Align2::LEFT_TOP,
        "HIDEOUT",
        18.0,
        style::ACCENT,
    );
    for (i, s) in StationKind::ALL.iter().enumerate() {
        text(
            &p,
            Pos2::new(16.0, 40.0 + i as f32 * 18.0),
            Align2::LEFT_TOP,
            format!("{}  Lv {}/{}", s.name(), state.level(*s), s.max_level()),
            13.5,
            style::TEXT_DIM,
        );
    }
    for (i, (m, ttl)) in session.messages.iter().enumerate() {
        let a = (ttl.min(1.0) * 255.0) as u8;
        text(
            &p,
            Pos2::new(16.0, screen.height() * 0.35 + i as f32 * 20.0),
            Align2::LEFT_TOP,
            m,
            14.0,
            Color32::from_rgba_unmultiplied(230, 220, 180, a),
        );
    }
}

pub enum StationAction {
    Upgrade,
    Craft(usize),
    Close,
}

fn cost_rows(ui: &mut egui::Ui, stash: &Grid, cost: &[(ItemKind, u32)]) {
    for (k, n) in cost {
        let h = have(stash, *k);
        let ok = h >= *n;
        let text = if *k == ItemKind::Roubles {
            format!(
                "{}   {} / {}",
                k.name(),
                super::inventory::value_label(h as u64),
                super::inventory::value_label(*n as u64)
            )
        } else {
            format!("{}   {} / {}", k.name(), h, n)
        };
        ui.label(RichText::new(text).color(if ok { style::GOOD } else { style::BAD }));
    }
}

pub fn station_window(
    ui: &mut egui::Ui,
    station: StationKind,
    state: &HideoutState,
    stash: &Grid,
    status: Option<&(String, bool)>,
) -> Option<StationAction> {
    let mut action = None;
    let ctx = ui.ctx().clone();
    let level = state.level(station);
    egui::Window::new(format!("{} - level {}/{}", station.name(), level, station.max_level()))
        .id(egui::Id::new("station window"))
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .resizable(false)
        .collapsible(false)
        .default_width(620.0)
        .show(&ctx, |ui| {
            ui.set_min_width(620.0);
            ui.label(RichText::new(station.description()).color(style::TEXT_DIM));
            ui.separator();
            if level < station.max_level() {
                if let Some(cost) = station.upgrade_cost(level + 1) {
                    ui.label(RichText::new(format!("UPGRADE TO LEVEL {}", level + 1)).color(style::ACCENT));
                    cost_rows(ui, stash, &cost);
                    let unlocks = recipes()
                        .into_iter()
                        .filter(|r| r.station == station && r.level == level + 1)
                        .map(|r| r.name())
                        .collect::<Vec<_>>()
                        .join(", ");
                    ui.label(
                        RichText::new(format!("Unlocks: {unlocks}"))
                            .size(12.0)
                            .color(style::TEXT_DIM),
                    );
                    let afford = can_afford(stash, &cost);
                    if ui
                        .add_enabled(afford, egui::Button::new(format!("Upgrade to level {}", level + 1)))
                        .clicked()
                    {
                        action = Some(StationAction::Upgrade);
                    }
                }
            } else {
                ui.label(RichText::new("Fully upgraded").color(style::GOOD));
            }
            ui.separator();
            ui.label(RichText::new("CRAFTING").color(style::ACCENT));
            egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                for (i, r) in recipes().iter().enumerate() {
                    if r.station != station {
                        continue;
                    }
                    let unlocked = level >= r.level;
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_width(580.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(r.name()).strong().color(if unlocked {
                                Color32::from_rgb(230, 225, 205)
                            } else {
                                style::TEXT_DIM
                            }));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if unlocked {
                                    let afford = can_afford(stash, &r.inputs);
                                    if ui.add_enabled(afford, egui::Button::new("Craft")).clicked() {
                                        action = Some(StationAction::Craft(i));
                                    }
                                } else {
                                    ui.label(RichText::new(format!("Requires level {}", r.level)).color(style::WARN));
                                }
                            });
                        });
                        if unlocked {
                            cost_rows(ui, stash, &r.inputs);
                        }
                    });
                }
            });
            if let Some((msg, err)) = status {
                ui.label(RichText::new(msg).color(if *err { style::BAD } else { style::GOOD }));
            }
            ui.add_space(4.0);
            if ui.button("Close (E / Esc)").clicked() {
                action = Some(StationAction::Close);
            }
        });
    action
}
