//! Menus: pause menu (main menu and raid summary are added with the raid loop).

use super::style;

pub enum PauseAction {
    Resume,
    Quit,
}

pub fn pause_menu(ui: &mut egui::Ui) -> Option<PauseAction> {
    let mut action = None;
    let ctx = ui.ctx().clone();
    let screen = ctx.content_rect();
    ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("pause dim")))
        .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(140));
    egui::Window::new("Paused")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .resizable(false)
        .collapsible(false)
        .show(&ctx, |ui| {
            ui.vertical_centered(|ui| {
                if style::big_button(ui, "Resume", 240.0).clicked() {
                    action = Some(PauseAction::Resume);
                }
                if style::big_button(ui, "Quit game", 240.0).clicked() {
                    action = Some(PauseAction::Quit);
                }
            });
        });
    action
}
