//! Menus: main menu, pause menu and raid summary.

use egui::{Color32, RichText};

use super::inventory::value_label;
use super::style;
use crate::inventory::EquipSlot;
use crate::raid::{OutcomeKind, RaidOutcome};
use crate::save::Profile;

pub enum PauseAction {
    Resume,
    LeaveRaid,
    BackToMenu,
    Quit,
}

pub enum MainMenuAction {
    StartRaid,
    Stash,
    Hideout,
    Quit,
    EmergencyKit,
    ResetProfile,
}

fn dim_background(ctx: &egui::Context, alpha: u8) {
    let screen = ctx.content_rect();
    ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("menu dim")))
        .rect_filled(screen, 0.0, Color32::from_black_alpha(alpha));
}

pub fn pause_menu(ui: &mut egui::Ui, in_raid: bool, settings: &mut crate::game::Settings) -> Option<PauseAction> {
    let mut action = None;
    let ctx = ui.ctx().clone();
    dim_background(&ctx, 140);
    egui::Window::new("Paused")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .resizable(false)
        .collapsible(false)
        .show(&ctx, |ui| {
            ui.vertical_centered(|ui| {
                if style::big_button(ui, "Resume", 280.0).clicked() {
                    action = Some(PauseAction::Resume);
                }
                if in_raid {
                    if style::big_button(ui, "Leave raid", 280.0).clicked() {
                        action = Some(PauseAction::LeaveRaid);
                    }
                    ui.label(
                        RichText::new("Leaving counts as Missing In Action:\neverything you brought in is lost.")
                            .size(11.5)
                            .color(style::WARN),
                    );
                } else if style::big_button(ui, "Back to main menu", 280.0).clicked() {
                    action = Some(PauseAction::BackToMenu);
                }
                if style::big_button(ui, "Quit game", 280.0).clicked() {
                    action = Some(PauseAction::Quit);
                }
                ui.add_space(8.0);
                ui.separator();
                ui.add(egui::Slider::new(&mut settings.sensitivity, 0.0005..=0.006).text("Mouse sensitivity"));
                ui.add(egui::Slider::new(&mut settings.fov_deg, 55.0..=100.0).text("FOV"));
                ui.checkbox(&mut settings.show_debug, "Debug overlay (F3)");
            });
        });
    action
}

fn loadout_line(ui: &mut egui::Ui, profile: &Profile, slot: EquipSlot) {
    let item = profile.equipment.slot(slot);
    ui.horizontal(|ui| {
        ui.add_sized(
            [120.0, 16.0],
            egui::Label::new(RichText::new(slot.name()).color(style::TEXT_DIM)),
        );
        match item {
            Some(i) => {
                let extra = match &i.weapon {
                    Some(w) => format!("  ({}/{})", w.rounds, w.capacity()),
                    None => i.durability.map(|d| format!("  ({:.0} dur.)", d)).unwrap_or_default(),
                };
                ui.label(format!("{}{}", i.name(), extra));
            }
            None => {
                ui.label(RichText::new("—").color(style::TEXT_DIM));
            }
        }
    });
}

pub fn main_menu(
    ui: &mut egui::Ui,
    profile: &Profile,
    notice: Option<&str>,
    confirm_reset: &mut bool,
    hideout_available: bool,
) -> Option<MainMenuAction> {
    let mut action = None;
    egui::Panel::left("main_left")
        .exact_size(430.0)
        .frame(
            egui::Frame::NONE
                .fill(Color32::from_rgba_unmultiplied(14, 15, 14, 235))
                .inner_margin(28.0),
        )
        .show(ui, |ui| {
            ui.add_space(30.0);
            ui.label(
                RichText::new("VOXEL")
                    .size(54.0)
                    .strong()
                    .color(Color32::from_rgb(228, 222, 200)),
            );
            ui.label(RichText::new("RAID").size(54.0).strong().color(style::ACCENT));
            ui.label(RichText::new("extract or die trying").size(15.0).color(style::TEXT_DIM));
            ui.add_space(40.0);
            let w = 360.0;
            if style::big_button(ui, "START RAID", w).clicked() {
                action = Some(MainMenuAction::StartRaid);
            }
            ui.add_space(6.0);
            if style::big_button(ui, "STASH", w).clicked() {
                action = Some(MainMenuAction::Stash);
            }
            ui.add_space(6.0);
            let hideout = ui.add_enabled(
                hideout_available,
                egui::Button::new(RichText::new("HIDEOUT").size(19.0)).min_size([w, 44.0].into()),
            );
            if hideout.clicked() {
                action = Some(MainMenuAction::Hideout);
            }
            ui.add_space(6.0);
            if style::big_button(ui, "QUIT", w).clicked() {
                action = Some(MainMenuAction::Quit);
            }
            ui.add_space(30.0);
            if let Some(n) = notice {
                ui.label(RichText::new(n).color(style::WARN));
            }
            ui.add_space(20.0);
            ui.collapsing("Profile options", |ui| {
                ui.checkbox(confirm_reset, "I really want to wipe my profile");
                if ui
                    .add_enabled(
                        *confirm_reset,
                        egui::Button::new(RichText::new("Wipe profile").color(style::BAD)),
                    )
                    .clicked()
                {
                    action = Some(MainMenuAction::ResetProfile);
                }
            });
        });

    egui::Panel::right("main_right")
        .exact_size(430.0)
        .frame(
            egui::Frame::NONE
                .fill(Color32::from_rgba_unmultiplied(14, 15, 14, 235))
                .inner_margin(24.0),
        )
        .show(ui, |ui| {
            ui.add_space(30.0);
            ui.label(RichText::new("PMC PROFILE").color(style::ACCENT));
            let s = &profile.stats;
            let rate = if s.raids > 0 {
                s.survived as f32 / s.raids as f32 * 100.0
            } else {
                0.0
            };
            egui::Grid::new("stats")
                .num_columns(2)
                .spacing([24.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Raids");
                    ui.label(s.raids.to_string());
                    ui.end_row();
                    ui.label("Survived / KIA / MIA");
                    ui.label(format!("{} / {} / {}", s.survived, s.killed, s.mia));
                    ui.end_row();
                    ui.label("Survival rate");
                    ui.label(format!("{rate:.0}%"));
                    ui.end_row();
                    ui.label("Kills");
                    ui.label(s.kills.to_string());
                    ui.end_row();
                    ui.label("Loot extracted");
                    ui.label(value_label(s.loot_value));
                    ui.end_row();
                    ui.label("Stash value");
                    ui.label(value_label(profile.stash.total_value()));
                    ui.end_row();
                });
            ui.add_space(20.0);
            ui.label(RichText::new("LOADOUT").color(style::ACCENT));
            for slot in EquipSlot::ALL {
                loadout_line(ui, profile, slot);
            }
            ui.label(format!("Gear value: {}", value_label(profile.equipment.total_value())));
            if !profile.equipment.has_any_weapon() {
                ui.add_space(8.0);
                ui.label(RichText::new("You have no weapon equipped!").color(style::WARN));
            }
            let has_weapon_anywhere = profile.equipment.has_any_weapon()
                || profile
                    .stash
                    .items
                    .iter()
                    .any(|p| matches!(p.item.kind, crate::inventory::ItemKind::Weapon(_)));
            if !has_weapon_anywhere {
                ui.add_space(8.0);
                ui.label("Broke? The Fence will spot you a basic kit.");
                if ui.button("Request emergency kit").clicked() {
                    action = Some(MainMenuAction::EmergencyKit);
                }
            }
        });
    action
}

/// Returns true when the player clicks continue.
pub fn raid_summary(ui: &mut egui::Ui, outcome: &RaidOutcome) -> bool {
    let mut cont = false;
    let ctx = ui.ctx().clone();
    egui::Window::new("Raid summary")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .resizable(false)
        .collapsible(false)
        .title_bar(false)
        .show(&ctx, |ui| {
            ui.set_min_width(460.0);
            ui.vertical_centered(|ui| {
                let (title, color, detail) = match &outcome.kind {
                    OutcomeKind::Survived(e) => ("SURVIVED", style::GOOD, format!("Extracted at {e}")),
                    OutcomeKind::Killed(c) => ("KILLED IN ACTION", style::BAD, c.clone()),
                    OutcomeKind::MissingInAction => (
                        "MISSING IN ACTION",
                        style::WARN,
                        "You did not reach an extraction point in time.".to_string(),
                    ),
                };
                ui.add_space(10.0);
                ui.label(RichText::new(title).size(34.0).strong().color(color));
                ui.label(RichText::new(detail).size(15.0));
                ui.add_space(14.0);
            });
            let t = outcome.time as i32;
            egui::Grid::new("summary")
                .num_columns(2)
                .spacing([30.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Time in raid");
                    ui.label(format!("{:02}:{:02}", t / 60, t % 60));
                    ui.end_row();
                    ui.label("Kills");
                    ui.label(outcome.kills.to_string());
                    ui.end_row();
                    ui.label("Gear brought in");
                    ui.label(value_label(outcome.value_in));
                    ui.end_row();
                    match outcome.kind {
                        OutcomeKind::Survived(_) => {
                            ui.label("Gear brought out");
                            ui.label(value_label(outcome.value_out));
                            ui.end_row();
                            let gain = outcome.value_out as i64 - outcome.value_in as i64;
                            ui.label("Profit");
                            ui.label(
                                RichText::new(format!(
                                    "{}{}",
                                    if gain >= 0 { "+" } else { "-" },
                                    value_label(gain.unsigned_abs())
                                ))
                                .color(if gain >= 0 {
                                    style::GOOD
                                } else {
                                    style::BAD
                                }),
                            );
                            ui.end_row();
                        }
                        _ => {
                            ui.label("Gear lost");
                            ui.label(RichText::new(value_label(outcome.value_in)).color(style::BAD));
                            ui.end_row();
                        }
                    }
                });
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                if style::big_button(ui, "Continue", 300.0).clicked() {
                    cont = true;
                }
            });
        });
    cont
}
