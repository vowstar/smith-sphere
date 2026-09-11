//! egui painter for the two planar circle charts.

use crate::palette::{Palette, trace_color};
use crate::scene::{ChartGrid, ChartVertex, PlottedTrace, PointRef, SelectionState};
use egui::{
    Align2, Color32, FontId, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2,
    pos2, vec2,
};
use smith_sphere_core::{Lang, Region};

/// Text the application supplies for one chart.
#[derive(Clone, Debug, Default)]
pub struct ChartLabels {
    pub title: String,
    pub subtitle: String,
    pub center: String,
    /// Optional note under the chart, e.g. where the selected point is.
    pub note: Option<String>,
    /// When set, grid labels are multiplied by this reference and shown in ohms.
    pub ohm_scale: Option<f64>,
}

/// Pointer state reported by a chart.
#[derive(Debug)]
pub struct ChartInteraction {
    pub hovered: Option<PointRef>,
    pub clicked: Option<PointRef>,
    pub response: Response,
}

/// Maximum number of valid samples for which every sample gets a marker.
pub const MARKER_LIMIT: usize = 240;
const HIT_RADIUS: f32 = 12.0;

struct Frame {
    center: Pos2,
    radius: f32,
    label_bounds: Rect,
}

impl Frame {
    fn to_screen(&self, p: [f64; 2]) -> Pos2 {
        pos2(
            self.center.x + p[0] as f32 * self.radius,
            self.center.y - p[1] as f32 * self.radius,
        )
    }
}

/// Paints one chart into `rect` and reports hover and click hits.
#[allow(clippy::too_many_arguments)]
pub fn paint_chart(
    ui: &mut Ui,
    rect: Rect,
    region: Region,
    grid: &ChartGrid,
    traces: &[PlottedTrace],
    selection: SelectionState,
    palette: &Palette,
    labels: &ChartLabels,
    lang: Lang,
) -> ChartInteraction {
    let response = ui.allocate_rect(rect, Sense::click());
    let painter = ui.painter_at(rect);

    let title_height = 64.0;
    let note_height = 36.0;
    let area = Rect::from_min_max(
        pos2(rect.left(), rect.top() + title_height),
        pos2(rect.right(), rect.bottom() - note_height),
    );
    let radius = ((area.width().min(area.height()) / 2.0) - 30.0).max(20.0);
    let frame = Frame {
        center: area.center(),
        radius,
        label_bounds: area.shrink(6.0),
    };

    let fill = match region {
        Region::Negative => palette.negative_fill,
        _ => palette.positive_fill,
    };

    painter.text(
        pos2(rect.left() + 12.0, rect.top() + 10.0),
        Align2::LEFT_TOP,
        &labels.title,
        FontId::proportional(15.0),
        palette.text,
    );
    // Allow two short lines without widening the chart on a narrow screen.
    let subtitle = painter.layout_job(egui::text::LayoutJob {
        wrap: egui::text::TextWrapping {
            max_width: rect.width() - 24.0,
            max_rows: 2,
            ..Default::default()
        },
        ..egui::text::LayoutJob::simple_singleline(
            labels.subtitle.clone(),
            FontId::proportional(11.0),
            palette.text_muted,
        )
    });
    painter.galley(
        pos2(rect.left() + 12.0, rect.top() + 30.0),
        subtitle,
        palette.text_muted,
    );

    painter.circle_filled(frame.center, radius, fill);
    paint_grid(&painter, &frame, region, grid, palette, labels.ohm_scale);
    painter.circle_stroke(frame.center, radius, Stroke::new(2.0, palette.boundary));
    paint_landmarks(&painter, &frame, region, palette, labels, lang);

    let hovered = response
        .hover_pos()
        .and_then(|pos| nearest_point(&frame, region, traces, pos));
    let clicked = if response.clicked() { hovered } else { None };

    for trace in traces {
        paint_trace(&painter, &frame, region, trace, selection, palette);
    }

    if let Some(note) = &labels.note {
        let note = painter.layout_job(egui::text::LayoutJob {
            wrap: egui::text::TextWrapping {
                max_width: rect.width() - 24.0,
                max_rows: 2,
                ..Default::default()
            },
            ..egui::text::LayoutJob::simple_singleline(
                note.clone(),
                FontId::proportional(11.0),
                palette.text_muted,
            )
        });
        painter.galley(
            pos2(rect.left() + 12.0, rect.bottom() - 27.0),
            note,
            palette.text_muted,
        );
    }

    ChartInteraction {
        hovered,
        clicked,
        response,
    }
}

fn paint_grid(
    painter: &egui::Painter,
    frame: &Frame,
    region: Region,
    grid: &ChartGrid,
    palette: &Palette,
    ohm_scale: Option<f64>,
) {
    let sign = if region == Region::Negative {
        -1.0
    } else {
        1.0
    };
    for curve in &grid.reactance {
        let stroke = grid_stroke(curve.major, palette);
        painter.add(Shape::line(
            curve.points.iter().map(|p| frame.to_screen(*p)).collect(),
            stroke,
        ));
    }
    for curve in &grid.resistance {
        let stroke = grid_stroke(curve.major, palette);
        painter.add(Shape::line(
            curve.points.iter().map(|p| frame.to_screen(*p)).collect(),
            stroke,
        ));
    }
    // Real axis, x = 0.
    painter.line_segment(
        [frame.to_screen([-1.0, 0.0]), frame.to_screen([1.0, 0.0])],
        Stroke::new(1.0, palette.grid_major),
    );

    let font = FontId::proportional(10.5);
    for curve in &grid.resistance {
        let r = curve.value;
        let position = frame.to_screen([(r - 1.0) / (r + 1.0), 0.0]);
        let text = format_grid_value(sign * r, ohm_scale, false);
        painter.text(
            position + vec2(2.0, -2.0),
            Align2::LEFT_BOTTOM,
            text,
            font.clone(),
            palette.text_muted,
        );
    }
    for curve in &grid.reactance {
        let x = curve.value;
        let rim = [(x * x - 1.0) / (x * x + 1.0), 2.0 * x / (x * x + 1.0)];
        let outward = [rim[0] * RIM_LABEL_SCALE, rim[1] * RIM_LABEL_SCALE];
        let anchor = if rim[0] < -0.2 {
            Align2::RIGHT_CENTER
        } else if rim[0] > 0.2 {
            Align2::LEFT_CENTER
        } else {
            Align2::CENTER_CENTER
        };
        let text = format_grid_value(x, ohm_scale, true);
        painter.text(
            frame.to_screen(outward),
            anchor,
            text,
            font.clone(),
            palette.text_muted,
        );
    }
}

/// Radial scale of the reactance labels just outside the rim.
const RIM_LABEL_SCALE: f64 = 1.07;

fn grid_stroke(major: bool, palette: &Palette) -> Stroke {
    if major {
        Stroke::new(1.0, palette.grid_major)
    } else {
        Stroke::new(0.7, palette.grid_minor)
    }
}

/// Formats a normalized grid value, optionally scaled to ohms.
#[must_use]
pub fn format_grid_value(value: f64, ohm_scale: Option<f64>, reactance: bool) -> String {
    let sign = if value < 0.0 {
        "−"
    } else if reactance {
        "+"
    } else {
        ""
    };
    let magnitude = value.abs();
    let (number, unit) = match ohm_scale {
        Some(z0) => (
            smith_sphere_core::format::significant(magnitude * z0, 4),
            " Ω",
        ),
        None => (smith_sphere_core::format::significant(magnitude, 3), ""),
    };
    if reactance {
        format!("{sign}j{number}{unit}")
    } else {
        format!("{sign}{number}{unit}")
    }
}

fn paint_landmarks(
    painter: &egui::Painter,
    frame: &Frame,
    region: Region,
    palette: &Palette,
    labels: &ChartLabels,
    lang: Lang,
) {
    let font = FontId::proportional(11.0);
    let short = frame.to_screen([-1.0, 0.0]);
    let open = frame.to_screen([1.0, 0.0]);
    painter.circle_filled(short, 3.0, palette.boundary);
    painter.circle_filled(open, 3.0, palette.boundary);
    painter.text(
        short + vec2(-6.0, 0.0),
        Align2::RIGHT_CENTER,
        lang.pick("短路 0", "short 0"),
        font.clone(),
        palette.text,
    );
    painter.text(
        open + vec2(6.0, 0.0),
        Align2::LEFT_CENTER,
        lang.pick("开路 ∞", "open ∞"),
        font.clone(),
        palette.text,
    );

    let center = frame.center;
    let center_color = if region == Region::Negative {
        palette.negative_edge
    } else {
        palette.positive_edge
    };
    painter.circle_stroke(center, 4.0, Stroke::new(1.5, center_color));
    painter.text(
        center + vec2(0.0, 8.0),
        Align2::CENTER_TOP,
        &labels.center,
        font.clone(),
        palette.text,
    );

    // Placed between the j1 (top) and j0.5 rim labels so nothing overlaps.
    let boundary_anchor = frame.to_screen([-0.40, 0.98]);
    painter.text(
        boundary_anchor + vec2(-4.0, -4.0),
        Align2::RIGHT_BOTTOM,
        lang.pick("R = 0 共享边界", "R = 0 shared rim"),
        FontId::proportional(10.5),
        palette.boundary,
    );
    // The ±j1 rim labels sit just outside the top and bottom of the rim, so
    // the region names go beside them on the same line instead of on top.
    let rim_font = FontId::proportional(10.5);
    let j1_half_width = painter
        .layout_no_wrap(
            format_grid_value(1.0, labels.ohm_scale, true),
            rim_font.clone(),
            palette.text_muted,
        )
        .size()
        .x
        / 2.0;
    let beside = vec2(j1_half_width + 8.0, 0.0);
    let top = frame.to_screen([0.0, RIM_LABEL_SCALE]);
    let bottom = frame.to_screen([0.0, -RIM_LABEL_SCALE]);
    painter.text(
        top + beside,
        Align2::LEFT_CENTER,
        lang.pick("感性 X > 0", "inductive X > 0"),
        rim_font.clone(),
        palette.text_muted,
    );
    painter.text(
        bottom + beside,
        Align2::LEFT_CENTER,
        lang.pick("容性 X < 0", "capacitive X < 0"),
        rim_font,
        palette.text_muted,
    );
}

fn nearest_point(
    frame: &Frame,
    region: Region,
    traces: &[PlottedTrace],
    pointer: Pos2,
) -> Option<PointRef> {
    let mut best: Option<(f32, PointRef)> = None;
    for trace in traces {
        for point in trace.valid_points() {
            let Some(position) = point.chart_position(region) else {
                continue;
            };
            let distance = frame.to_screen(position).distance(pointer);
            if distance <= HIT_RADIUS && best.is_none_or(|(d, _)| distance < d) {
                best = Some((
                    distance,
                    PointRef {
                        document: trace.document,
                        trace: trace.trace,
                        sample: point.sample,
                    },
                ));
            }
        }
    }
    best.map(|(_, point)| point)
}

fn paint_trace(
    painter: &egui::Painter,
    frame: &Frame,
    region: Region,
    trace: &PlottedTrace,
    selection: SelectionState,
    palette: &Palette,
) {
    let color = trace_color(trace.color_index);
    let stroke = Stroke::new(2.0, color);
    for segment in trace
        .segments
        .iter()
        .filter(|segment| segment.region == region)
    {
        let screen: Vec<(ChartVertex, Pos2)> = segment
            .vertices
            .iter()
            .map(|(vertex, position)| (*vertex, frame.to_screen(*position)))
            .collect();
        for pair in screen.windows(2) {
            let (va, a) = pair[0];
            let (vb, b) = pair[1];
            if va.is_interpolated() || vb.is_interpolated() {
                painter.add(Shape::dashed_line(&[a, b], stroke, 4.0, 3.0));
            } else {
                painter.line_segment([a, b], stroke);
            }
        }
        for (vertex, position) in &screen {
            if vertex.is_interpolated() {
                paint_diamond(painter, *position, 5.0, palette.interpolated);
            }
        }
        if screen.len() == 1 && !screen[0].0.is_interpolated() {
            painter.circle_filled(screen[0].1, 3.0, color);
        }
    }

    let draw_markers = trace.valid_points().count() <= MARKER_LIMIT;
    for point in trace.valid_points() {
        let Some(position) = point.chart_position(region) else {
            continue;
        };
        let screen = frame.to_screen(position);
        let reference = PointRef {
            document: trace.document,
            trace: trace.trace,
            sample: point.sample,
        };
        if draw_markers {
            painter.circle(screen, 2.6, color, Stroke::new(0.6, Color32::WHITE));
        }
        if selection.hovered == Some(reference) && selection.selected != Some(reference) {
            painter.circle_stroke(screen, 7.5, Stroke::new(1.5, color));
        }
        if selection.selected == Some(reference) {
            paint_selected_marker(painter, screen, color, palette);
            let label = point
                .frequency_hz
                .map(smith_sphere_core::format::frequency)
                .unwrap_or_else(|| trace.label.clone());
            let galley = painter.layout(
                label,
                FontId::proportional(11.0),
                palette.text,
                (frame.label_bounds.width() - 6.0).min(180.0),
            );
            let size = galley.size() + Vec2::splat(6.0);
            // Open the label away from the centre so it never covers the
            // centre marker text.
            let mut anchor = if screen.x < frame.center.x {
                screen + vec2(-12.0 - size.x, -12.0)
            } else {
                screen + vec2(12.0, -12.0)
            };
            anchor.x = anchor.x.clamp(
                frame.label_bounds.left(),
                (frame.label_bounds.right() - size.x).max(frame.label_bounds.left()),
            );
            anchor.y = anchor.y.clamp(
                frame.label_bounds.top(),
                (frame.label_bounds.bottom() - size.y).max(frame.label_bounds.top()),
            );
            let box_rect = Rect::from_min_size(anchor, size);
            painter.rect(
                box_rect,
                3.0,
                Color32::from_rgba_unmultiplied(255, 255, 255, 230),
                Stroke::new(1.0, color),
                StrokeKind::Outside,
            );
            painter.galley(anchor + Vec2::splat(3.0), galley, palette.text);
        }
    }
}

/// Selected-point marker: filled dot, dark ring, and four ticks so the state
/// does not rely on color alone.
pub fn paint_selected_marker(
    painter: &egui::Painter,
    screen: Pos2,
    color: Color32,
    palette: &Palette,
) {
    painter.circle(
        screen,
        7.0,
        Color32::WHITE,
        Stroke::new(1.5, palette.selection),
    );
    painter.circle_filled(screen, 3.5, color);
    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        let direction = vec2(dx, dy);
        painter.line_segment(
            [screen + direction * 9.0, screen + direction * 12.0],
            Stroke::new(1.0, palette.selection),
        );
    }
}

/// Hollow diamond used for interpolated boundary crossings.
pub fn paint_diamond(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    let points = vec![
        center + vec2(0.0, -size),
        center + vec2(size, 0.0),
        center + vec2(0.0, size),
        center + vec2(-size, 0.0),
    ];
    painter.add(Shape::closed_line(points, Stroke::new(1.5, color)));
}
