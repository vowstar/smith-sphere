//! Colors shared by the three views.

use egui::Color32;

/// Visual constants for both hemispheres and the shared chrome.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub positive_fill: Color32,
    pub positive_edge: Color32,
    pub negative_fill: Color32,
    pub negative_edge: Color32,
    pub grid_major: Color32,
    pub grid_minor: Color32,
    pub boundary: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub selection: Color32,
    pub interpolated: Color32,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            positive_fill: Color32::from_rgb(242, 248, 254),
            positive_edge: Color32::from_rgb(107, 138, 164),
            negative_fill: Color32::from_rgb(244, 243, 252),
            negative_edge: Color32::from_rgb(134, 125, 171),
            grid_major: Color32::from_rgb(107, 138, 164),
            grid_minor: Color32::from_rgb(182, 206, 227),
            boundary: Color32::from_rgb(38, 105, 160),
            text: Color32::from_rgb(34, 73, 110),
            text_muted: Color32::from_rgb(82, 114, 139),
            selection: Color32::from_rgb(38, 105, 160),
            interpolated: Color32::from_rgb(82, 114, 139),
        }
    }
}

const TRACE_COLORS: [Color32; 6] = [
    Color32::from_rgb(52, 136, 214),
    Color32::from_rgb(40, 155, 152),
    Color32::from_rgb(205, 116, 86),
    Color32::from_rgb(144, 111, 195),
    Color32::from_rgb(81, 146, 109),
    Color32::from_rgb(190, 103, 151),
];

/// Deterministic color for a trace index, shared by all views.
#[must_use]
pub fn trace_color(index: usize) -> Color32 {
    TRACE_COLORS[index % TRACE_COLORS.len()]
}
