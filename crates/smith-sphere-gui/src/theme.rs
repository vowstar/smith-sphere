//! Visual language shared by the native and browser editions.

use egui::{
    Color32, Context, CornerRadius, FontId, Frame, InnerResponse, Painter, Pos2, Rect, Sense,
    Shape, Stroke, TextStyle, Theme, Ui, vec2,
};

pub const CANVAS: Color32 = Color32::from_rgb(247, 251, 255);
pub const SURFACE: Color32 = Color32::from_rgb(255, 255, 255);
pub const SURFACE_RAISED: Color32 = Color32::from_rgb(242, 248, 254);
pub const BORDER: Color32 = Color32::from_rgb(184, 209, 232);
pub const CONTROL_BORDER: Color32 = Color32::from_rgb(107, 138, 164);
pub const TEXT: Color32 = Color32::from_rgb(34, 73, 110);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(82, 114, 139);
pub const ACCENT: Color32 = Color32::from_rgb(52, 136, 214);
pub const ACCENT_TEXT: Color32 = Color32::from_rgb(38, 105, 160);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(234, 245, 255);
pub const AMBER: Color32 = Color32::from_rgb(138, 101, 31);
pub const CORAL: Color32 = Color32::from_rgb(163, 61, 61);

/// Shared decorative stroke, in logical pixels, outside the coordinate field.
pub fn dashed_rule(painter: &Painter, start: Pos2, end: Pos2) {
    painter.add(Shape::dashed_line(
        &[start, end],
        Stroke::new(1.0, BORDER),
        3.0,
        3.0,
    ));
}

pub fn dashed_border(painter: &Painter, rect: Rect) {
    let rect = rect.shrink(0.5);
    for [start, end] in [
        [rect.left_top(), rect.right_top()],
        [rect.right_top(), rect.right_bottom()],
        [rect.right_bottom(), rect.left_bottom()],
        [rect.left_bottom(), rect.left_top()],
    ] {
        dashed_rule(painter, start, end);
    }
}

pub fn plot_surface(painter: &Painter, rect: Rect) {
    painter.rect_filled(rect, 2.0, SURFACE);
    dashed_border(painter, rect);
}

pub fn section_frame(margin: i8) -> Frame {
    Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, Color32::TRANSPARENT))
        .corner_radius(2)
        .inner_margin(margin)
}

pub fn section<R>(
    ui: &mut Ui,
    margin: i8,
    contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    let response = section_frame(margin).show(ui, contents);
    dashed_border(ui.painter(), response.response.rect);
    response
}

/// A decorative divider with the same rhythm as section outlines.
pub fn separator(ui: &mut Ui) {
    let horizontal = !ui.layout().main_dir().is_horizontal();
    let available = ui.available_size_before_wrap();
    let size = if horizontal {
        vec2(available.x, 8.0)
    } else {
        vec2(8.0, ui.spacing().interact_size.y)
    };
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let [start, end] = if horizontal {
        [rect.left_center(), rect.right_center()]
    } else {
        [rect.center_top(), rect.center_bottom()]
    };
    dashed_rule(ui.painter(), start, end);
}

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
    visuals.window_shadow.color = Color32::from_rgba_unmultiplied(34, 73, 110, 14);
    visuals.extreme_bg_color = ACCENT_SOFT;
    visuals.text_edit_bg_color = Some(SURFACE);
    visuals.faint_bg_color = CANVAS;
    visuals.code_bg_color = SURFACE_RAISED;
    visuals.hyperlink_color = ACCENT_TEXT;
    visuals.warn_fg_color = AMBER;
    visuals.error_fg_color = CORAL;
    visuals.selection.bg_fill = ACCENT_SOFT;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT_TEXT);
    visuals.slider_trailing_fill = true;
    visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);

    let radius = CornerRadius::same(3);
    visuals.widgets.noninteractive.bg_fill = SURFACE_RAISED;
    visuals.widgets.noninteractive.weak_bg_fill = SURFACE;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.noninteractive.corner_radius = radius;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);

    visuals.widgets.inactive.bg_fill = SURFACE_RAISED;
    visuals.widgets.inactive.weak_bg_fill = SURFACE_RAISED;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, CONTROL_BORDER);
    visuals.widgets.inactive.corner_radius = radius;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);

    visuals.widgets.hovered.bg_fill = ACCENT_SOFT;
    visuals.widgets.hovered.weak_bg_fill = ACCENT_SOFT;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.hovered.corner_radius = radius;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.25, ACCENT_TEXT);

    visuals.widgets.active.bg_fill = ACCENT_SOFT;
    visuals.widgets.active.weak_bg_fill = ACCENT_SOFT;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.corner_radius = radius;
    visuals.widgets.active.fg_stroke = Stroke::new(1.25, TEXT);

    context.set_style_of(Theme::Light, style);
}
