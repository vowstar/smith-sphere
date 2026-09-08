//! Visual language shared by the native and browser editions.

use egui::{Color32, Context, CornerRadius, FontId, Stroke, TextStyle, Theme, vec2};

pub const CANVAS: Color32 = Color32::from_rgb(244, 245, 247);
pub const SURFACE: Color32 = Color32::from_rgb(255, 255, 255);
pub const SURFACE_RAISED: Color32 = Color32::from_rgb(248, 249, 251);
pub const BORDER: Color32 = Color32::from_rgb(217, 221, 227);
pub const TEXT: Color32 = Color32::from_rgb(28, 34, 44);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(95, 102, 112);
pub const ACCENT: Color32 = Color32::from_rgb(74, 111, 165);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(223, 232, 243);
pub const AMBER: Color32 = Color32::from_rgb(158, 104, 22);
pub const CORAL: Color32 = Color32::from_rgb(178, 72, 72);
pub const OK: Color32 = Color32::from_rgb(52, 120, 96);

/// Applies the product theme to a fresh egui context.
pub fn install(context: &Context) {
    context.set_theme(Theme::Light);
    let mut style = (*context.style_of(Theme::Light)).clone();
    style.spacing.item_spacing = vec2(8.0, 7.0);
    style.spacing.button_padding = vec2(11.0, 6.0);
    style.spacing.interact_size.y = 32.0;
    style.spacing.slider_width = 180.0;
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(19.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(14.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(14.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(12.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(13.0, egui::FontFamily::Monospace),
    );

    let visuals = &mut style.visuals;
    visuals.dark_mode = false;
    visuals.override_text_color = Some(TEXT);
    visuals.weak_text_color = Some(TEXT_MUTED);
    visuals.panel_fill = CANVAS;
    visuals.window_fill = SURFACE;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.window_shadow.color = Color32::from_black_alpha(28);
    visuals.extreme_bg_color = Color32::from_rgb(238, 241, 245);
    visuals.text_edit_bg_color = Some(Color32::from_rgb(250, 251, 252));
    visuals.faint_bg_color = Color32::from_rgb(247, 248, 250);
    visuals.code_bg_color = Color32::from_rgb(240, 243, 247);
    visuals.hyperlink_color = ACCENT;
    visuals.warn_fg_color = AMBER;
    visuals.error_fg_color = CORAL;
    visuals.selection.bg_fill = ACCENT_SOFT;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.slider_trailing_fill = true;
    visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);

    let radius = CornerRadius::same(6);
    visuals.widgets.noninteractive.bg_fill = SURFACE_RAISED;
    visuals.widgets.noninteractive.weak_bg_fill = SURFACE;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.noninteractive.corner_radius = radius;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);

    visuals.widgets.inactive.bg_fill = SURFACE_RAISED;
    visuals.widgets.inactive.weak_bg_fill = SURFACE_RAISED;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.corner_radius = radius;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(236, 241, 248);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(236, 241, 248);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.hovered.corner_radius = radius;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.25, ACCENT);

    visuals.widgets.active.bg_fill = ACCENT_SOFT;
    visuals.widgets.active.weak_bg_fill = ACCENT_SOFT;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.corner_radius = radius;
    visuals.widgets.active.fg_stroke = Stroke::new(1.25, TEXT);

    context.set_style_of(Theme::Light, style);
}
