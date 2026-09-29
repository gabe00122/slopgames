//! A muted, military-looking egui theme.

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle};

pub const ACCENT: Color32 = Color32::from_rgb(196, 176, 112);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(96, 86, 54);
pub const GOOD: Color32 = Color32::from_rgb(120, 200, 110);
pub const WARN: Color32 = Color32::from_rgb(230, 180, 60);
pub const BAD: Color32 = Color32::from_rgb(220, 70, 60);
pub const TEXT_DIM: Color32 = Color32::from_rgb(150, 150, 140);
pub const PANEL: Color32 = Color32::from_rgb(20, 22, 21);

pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        style.text_styles = [
            (TextStyle::Heading, FontId::new(22.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(14.5, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(14.5, FontFamily::Proportional)),
            (TextStyle::Small, FontId::new(11.5, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.5, FontFamily::Monospace)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);

        let v = &mut style.visuals;
        v.window_fill = Color32::from_rgba_unmultiplied(22, 24, 23, 245);
        v.panel_fill = PANEL;
        v.extreme_bg_color = Color32::from_rgb(12, 13, 12);
        v.faint_bg_color = Color32::from_rgb(28, 30, 29);
        v.window_corner_radius = CornerRadius::same(3);
        v.menu_corner_radius = CornerRadius::same(3);
        v.window_stroke = Stroke::new(1.0, Color32::from_rgb(60, 62, 56));
        v.selection.bg_fill = ACCENT_DIM;
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.hyperlink_color = ACCENT;

        let r = CornerRadius::same(2);
        v.widgets.noninteractive.bg_fill = Color32::from_rgb(26, 28, 27);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(200, 200, 190));
        v.widgets.inactive.bg_fill = Color32::from_rgb(40, 42, 38);
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(40, 42, 38);
        v.widgets.inactive.corner_radius = r;
        v.widgets.hovered.bg_fill = Color32::from_rgb(64, 62, 48);
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(64, 62, 48);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
        v.widgets.hovered.corner_radius = r;
        v.widgets.active.bg_fill = ACCENT_DIM;
        v.widgets.active.weak_bg_fill = ACCENT_DIM;
        v.widgets.active.corner_radius = r;
    });
}

/// A large menu button.
pub fn big_button(ui: &mut egui::Ui, text: &str, width: f32) -> egui::Response {
    ui.add_sized(
        [width, 44.0],
        egui::Button::new(
            egui::RichText::new(text)
                .size(19.0)
                .color(Color32::from_rgb(225, 220, 200)),
        ),
    )
}

pub fn hp_color(frac: f32) -> Color32 {
    if frac <= 0.0 {
        Color32::from_rgb(40, 20, 20)
    } else if frac < 0.35 {
        BAD
    } else if frac < 0.7 {
        WARN
    } else {
        GOOD
    }
}
