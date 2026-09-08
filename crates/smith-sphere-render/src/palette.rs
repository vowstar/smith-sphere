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
            positive_fill: Color32::from_rgb(220, 231, 240),
            positive_edge: Color32::from_rgb(120, 146, 176),
            negative_fill: Color32::from_rgb(234, 226, 238),
            negative_edge: Color32::from_rgb(150, 128, 170),
            grid_major: Color32::from_rgb(132, 140, 150),
            grid_minor: Color32::from_rgb(178, 184, 192),
            boundary: Color32::from_rgb(52, 60, 72),
            text: Color32::from_rgb(28, 34, 44),
            text_muted: Color32::from_rgb(96, 104, 116),
            selection: Color32::from_rgb(20, 24, 32),
            interpolated: Color32::from_rgb(110, 110, 110),
        }
    }
}

const TRACE_COLORS: [Color32; 6] = [
    Color32::from_rgb(45, 98, 168),
    Color32::from_rgb(196, 122, 36),
    Color32::from_rgb(118, 82, 168),
    Color32::from_rgb(28, 132, 132),
    Color32::from_rgb(160, 66, 126),
    Color32::from_rgb(110, 110, 40),
];

/// Deterministic color for a trace index, shared by all views.
#[must_use]
pub fn trace_color(index: usize) -> Color32 {
    TRACE_COLORS[index % TRACE_COLORS.len()]
}
