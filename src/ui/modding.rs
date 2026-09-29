//! Weapon modding screen: swap attachments per slot with a live stats panel.

use egui::{Color32, RichText};

use super::style;
use crate::weapons::{swap_attachment, AmmoType, AttachmentId, PartSource, Slot, Weapon, WeaponStats};

#[derive(Default)]
pub struct ModdingUi {
    status: Option<(String, bool)>,
}

pub enum ModdingEvent {
    Close,
    /// Rounds that were unloaded from a swapped magazine.
    Unloaded(AmmoType, u32),
}

/// One row in the stats panel: (label, value, better-when-higher, bar range, format).
struct StatRow {
    label: &'static str,
    get: fn(&WeaponStats) -> f32,
    higher_better: bool,
    max: f32,
    fmt: fn(f32) -> String,
}

const ROWS: [StatRow; 11] = [
    StatRow {
        label: "Ergonomics",
        get: |s| s.ergonomics,
        higher_better: true,
        max: 100.0,
        fmt: |v| format!("{v:.0}"),
    },
    StatRow {
        label: "Vertical recoil",
        get: |s| s.vertical_recoil,
        higher_better: false,
        max: 1.6,
        fmt: |v| format!("{v:.2}°"),
    },
    StatRow {
        label: "Horizontal recoil",
        get: |s| s.horizontal_recoil,
        higher_better: false,
        max: 0.7,
        fmt: |v| format!("{v:.2}°"),
    },
    StatRow {
        label: "Accuracy (MOA)",
        get: |s| s.spread * 60.0,
        higher_better: false,
        max: 30.0,
        fmt: |v| format!("{v:.1}"),
    },
    StatRow {
        label: "Hip-fire spread",
        get: |s| s.hip_spread,
        higher_better: false,
        max: 3.5,
        fmt: |v| format!("{v:.2}°"),
    },
    StatRow {
        label: "ADS time",
        get: |s| s.ads_time,
        higher_better: false,
        max: 0.7,
        fmt: |v| format!("{:.0} ms", v * 1000.0),
    },
    StatRow {
        label: "Reload time",
        get: |s| s.reload_time,
        higher_better: false,
        max: 4.5,
        fmt: |v| format!("{v:.2} s"),
    },
    StatRow {
        label: "Magazine",
        get: |s| s.mag_size as f32,
        higher_better: true,
        max: 95.0,
        fmt: |v| format!("{v:.0} rds"),
    },
    StatRow {
        label: "Sight zoom",
        get: |s| s.zoom,
        higher_better: true,
        max: 4.0,
        fmt: |v| format!("{v:.1}x"),
    },
    StatRow {
        label: "Loudness",
        get: |s| s.loudness,
        higher_better: false,
        max: 110.0,
        fmt: |v| format!("{v:.0} m"),
    },
    StatRow {
        label: "Weight",
        get: |s| s.weight,
        higher_better: false,
        max: 6.5,
        fmt: |v| format!("{v:.2} kg"),
    },
];

fn stat_bar(ui: &mut egui::Ui, row: &StatRow, cur: f32, cmp: Option<f32>) {
    {
        ui.label(RichText::new(row.label).color(style::TEXT_DIM));
        let (rect, _) = ui.allocate_exact_size(egui::vec2(120.0, 10.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, 2.0, Color32::from_rgb(40, 42, 40));
        let frac = (cur / row.max).clamp(0.0, 1.0);
        let mut fill = rect;
        fill.set_width(rect.width() * frac);
        p.rect_filled(fill, 2.0, style::ACCENT_DIM);
        let mut text = (row.fmt)(cur);
        let mut color = Color32::from_rgb(225, 222, 205);
        if let Some(other) = cmp {
            let ofrac = (other / row.max).clamp(0.0, 1.0);
            let x = rect.left() + rect.width() * ofrac;
            let better = if row.higher_better { other > cur } else { other < cur };
            let col = if better { style::GOOD } else { style::BAD };
            if (other - cur).abs() > 1e-4 {
                p.line_segment(
                    [egui::pos2(x, rect.top() - 2.0), egui::pos2(x, rect.bottom() + 2.0)],
                    egui::Stroke::new(2.0, col),
                );
                text = format!("{} → {}", (row.fmt)(cur), (row.fmt)(other));
                color = col;
            }
        }
        ui.label(RichText::new(text).color(color).monospace());
        ui.end_row();
    }
}

/// Draws the modding screen. `baseline` is what the stats are compared against
/// when nothing is hovered (the receiver with default attachments).
pub fn modding_screen(
    ui: &mut egui::Ui,
    state: &mut ModdingUi,
    weapon: &mut Weapon,
    parts: &mut dyn PartSource,
) -> Vec<ModdingEvent> {
    let mut events = Vec::new();
    let mut hover: Option<(Slot, Option<AttachmentId>)> = None;
    let mut change: Option<(Slot, Option<AttachmentId>)> = None;
    let receiver = weapon.receiver;

    egui::Panel::left("mod_left").exact_size(400.0).show(ui, |ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("WEAPON MODDING").color(style::ACCENT).size(13.0));
        ui.heading(weapon.name());
        ui.label(
            RichText::new(format!(
                "Caliber {}  ·  {}",
                weapon.caliber().name(),
                if receiver.def().full_auto {
                    "Semi / Auto"
                } else {
                    "Semi"
                }
            ))
            .color(style::TEXT_DIM),
        );
        ui.separator();
        for slot in Slot::ALL {
            if !receiver.has_slot(slot) {
                continue;
            }
            let current = weapon.attachment(slot);
            let options: Vec<AttachmentId> = AttachmentId::ALL
                .iter()
                .copied()
                .filter(|a| a.def().slot == slot && a.fits(receiver) && parts.count(*a) > 0 && Some(*a) != current)
                .collect();
            ui.add_space(4.0);
            ui.label(
                RichText::new(slot.name().to_uppercase())
                    .size(11.5)
                    .color(style::TEXT_DIM),
            );
            let current_name = current.map(|a| a.def().name).unwrap_or("— empty —");
            egui::ComboBox::from_id_salt(("slot", slot as usize))
                .width(360.0)
                .selected_text(current_name)
                .show_ui(ui, |ui| {
                    if current.is_some() {
                        let r = ui.selectable_label(false, "— remove —");
                        if r.hovered() {
                            hover = Some((slot, None));
                        }
                        if r.clicked() {
                            change = Some((slot, None));
                        }
                    }
                    if options.is_empty() {
                        ui.label(RichText::new("No compatible parts available").color(style::TEXT_DIM));
                    }
                    for a in options {
                        let label = format!("{}  (x{})", a.def().name, parts.count(a));
                        let r = ui.selectable_label(false, label);
                        if r.hovered() {
                            hover = Some((slot, Some(a)));
                        }
                        if r.clicked() {
                            change = Some((slot, Some(a)));
                        }
                    }
                });
        }
        ui.add_space(10.0);
        ui.separator();
        if let Some((msg, err)) = &state.status {
            ui.label(RichText::new(msg).color(if *err { style::BAD } else { style::GOOD }));
        }
        if !weapon.stats().operable {
            ui.label(RichText::new("⚠ Weapon is inoperable without a barrel").color(style::WARN));
        }
        ui.add_space(6.0);
        if style::big_button(ui, "Done", 370.0).clicked() {
            events.push(ModdingEvent::Close);
        }
        ui.label(
            RichText::new("Hover a part to preview its effect on the stats.")
                .color(style::TEXT_DIM)
                .size(11.5),
        );
    });

    // Stats for the hovered option (preview) or against the defaults.
    let current = weapon.stats();
    let preview = hover.map(|(slot, a)| {
        let mut w = weapon.clone();
        w.set_attachment(slot, a);
        w.stats()
    });

    egui::Panel::right("mod_right").exact_size(430.0).show(ui, |ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("STATISTICS").color(style::ACCENT).size(13.0));
        if preview.is_some() {
            ui.label(RichText::new("Previewing change").color(style::WARN));
        } else {
            ui.label(RichText::new("Current configuration").color(style::TEXT_DIM));
        }
        ui.add_space(6.0);
        egui::Grid::new("mod_stats")
            .num_columns(3)
            .spacing([10.0, 8.0])
            .show(ui, |ui| {
                for row in &ROWS {
                    let cur = (row.get)(&current);
                    let cmp = preview.as_ref().map(|p| (row.get)(p));
                    stat_bar(ui, row, cur, cmp);
                }
            });
        ui.add_space(8.0);
        ui.separator();
        ui.label(format!("Fire rate: {:.0} rpm", current.fire_rate));
        let loaded = weapon
            .loaded
            .filter(|_| weapon.rounds > 0)
            .map(|a| format!("{} x{}", a.def().name, weapon.rounds))
            .unwrap_or_else(|| "Empty".into());
        ui.label(format!("Loaded: {loaded}"));
        ui.label(format!("Value: {} RUB", weapon.value()));
    });

    if let Some((slot, a)) = change {
        match swap_attachment(weapon, slot, a, parts) {
            Ok(r) => {
                if let Some((ammo, n)) = r.unloaded {
                    events.push(ModdingEvent::Unloaded(ammo, n));
                }
                let msg = match a {
                    Some(a) => format!("Installed {}", a.def().name),
                    None => format!("Removed {}", slot.name()),
                };
                state.status = Some((msg, false));
            }
            Err(e) => state.status = Some((e, true)),
        }
    }
    events
}
