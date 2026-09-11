//! egui painter for the rotatable sphere.

use crate::camera::Camera;
use crate::chart::{MARKER_LIMIT, paint_diamond, paint_selected_marker};
use crate::palette::{Palette, trace_color};
use crate::scene::{PlottedTrace, PointRef, SelectionState, SphereCurve, SphereCurveKind};
use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{Align2, Color32, FontId, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, pos2, vec2};
use smith_sphere_core::{Lang, SpherePoint};

/// Pointer state reported by the sphere view.
#[derive(Debug)]
pub struct SphereInteraction {
    pub hovered: Option<PointRef>,
    pub clicked: Option<PointRef>,
    /// The fixed selection exists but is on the far side of the sphere.
    pub selected_hidden: bool,
    pub response: Response,
}

const HIT_RADIUS: f32 = 11.0;
const BACK_ALPHA: u8 = 70;

struct Projector {
    camera: Camera,
    center: Pos2,
    radius: f32,
}

impl Projector {
    fn project(&self, point: SpherePoint) -> (Pos2, f32) {
        let [x, y, depth] = self.camera.view(point);
        (
            pos2(
                self.center.x + x * self.radius,
                self.center.y - y * self.radius,
            ),
            depth,
        )
    }
}

/// Paints the sphere, handles orbit and zoom gestures, and reports hits.
#[allow(clippy::too_many_arguments)]
pub fn paint_sphere(
    ui: &mut Ui,
    rect: Rect,
    camera: &mut Camera,
    grid: &[SphereCurve],
    traces: &[PlottedTrace],
    selection: SelectionState,
    palette: &Palette,
    show_landmarks: bool,
    lang: Lang,
) -> SphereInteraction {
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    if response.dragged() {
        let delta = response.drag_delta();
        camera.orbit(delta.x, delta.y);
    }
    if response.hovered() {
        let (scroll, zoom_delta) =
            ui.input(|input| (input.smooth_scroll_delta.y, input.zoom_delta()));
        if scroll != 0.0 {
            camera.zoom_by((1.0 + scroll * 0.0025).clamp(0.5, 2.0));
        }
        if zoom_delta != 1.0 {
            camera.zoom_by(zoom_delta);
        }
    }

    let painter = ui.painter_at(rect);
    let base_radius = rect.width().min(rect.height()) / 2.0 * 0.82;
    let projector = Projector {
        camera: *camera,
        center: rect.center(),
        radius: base_radius * camera.zoom,
    };

    // Far side first, faded, so it shows through the slightly translucent body.
    for curve in grid {
        let (stroke, _) = curve_style(curve.kind, palette);
        paint_polyline(&painter, &projector, &curve.points, fade(stroke), false);
    }
    for trace in traces {
        let stroke = Stroke::new(2.0, trace_color(trace.color_index));
        for run in &trace.sphere_runs {
            let points: Vec<SpherePoint> = run.iter().map(|(_, point)| *point).collect();
            paint_polyline(&painter, &projector, &points, fade(stroke), false);
        }
    }

    painter.add(Shape::mesh(sphere_mesh(&projector, palette)));
    painter.circle_stroke(
        projector.center,
        projector.radius,
        Stroke::new(1.0, palette.boundary.gamma_multiply(0.5)),
    );

    for curve in grid {
        let (stroke, _) = curve_style(curve.kind, palette);
        paint_polyline(&painter, &projector, &curve.points, stroke, true);
    }
    for trace in traces {
        let stroke = Stroke::new(2.2, trace_color(trace.color_index));
        for run in &trace.sphere_runs {
            let points: Vec<SpherePoint> = run.iter().map(|(_, point)| *point).collect();
            paint_polyline(&painter, &projector, &points, stroke, true);
        }
        for crossing in &trace.crossings {
            let (screen, depth) = projector.project(*crossing);
            if depth >= 0.0 {
                paint_diamond(&painter, screen, 5.0, palette.interpolated);
            }
        }
    }

    if show_landmarks {
        paint_landmark_labels(&painter, &projector, palette, lang);
    }

    let hovered = response
        .hover_pos()
        .and_then(|pos| nearest_point(&projector, traces, pos));
    let clicked = if response.clicked() { hovered } else { None };
    let mut selected_hidden = false;

    for trace in traces {
        let color = trace_color(trace.color_index);
        let draw_markers = trace.valid_points().count() <= MARKER_LIMIT;
        for point in trace.valid_points() {
            let (screen, depth) = projector.project(point.sphere);
            let reference = PointRef {
                document: trace.document,
                trace: trace.trace,
                sample: point.sample,
            };
            let front = depth >= 0.0;
            if draw_markers {
                if front {
                    painter.circle(screen, 2.6, color, Stroke::new(0.6, Color32::WHITE));
                } else {
                    painter.circle_filled(screen, 1.8, color.gamma_multiply(0.3));
                }
            }
            if selection.hovered == Some(reference)
                && selection.selected != Some(reference)
                && front
            {
                painter.circle_stroke(screen, 7.5, Stroke::new(1.5, color));
            }
            if selection.selected == Some(reference) {
                if front {
                    paint_selected_marker(&painter, screen, color, palette);
                } else {
                    selected_hidden = true;
                    painter.add(Shape::dashed_line(
                        &circle_points(screen, 7.0, 16),
                        Stroke::new(1.2, color.gamma_multiply(0.6)),
                        2.5,
                        2.0,
                    ));
                }
            }
        }
    }

    SphereInteraction {
        hovered,
        clicked,
        selected_hidden,
        response,
    }
}

fn fade(stroke: Stroke) -> Stroke {
    let [r, g, b, _] = stroke.color.to_array();
    Stroke::new(
        (stroke.width * 0.8).max(0.6),
        Color32::from_rgba_unmultiplied(r, g, b, BACK_ALPHA),
    )
}

fn curve_style(kind: SphereCurveKind, palette: &Palette) -> (Stroke, bool) {
    match kind {
        SphereCurveKind::Boundary => (Stroke::new(2.0, palette.boundary), true),
        SphereCurveKind::RealAxis | SphereCurveKind::UnitMagnitude => {
            (Stroke::new(1.0, palette.grid_major), true)
        }
        SphereCurveKind::Resistance(value) => {
            let color = if value < 0.0 {
                palette.negative_edge
            } else {
                palette.positive_edge
            };
            (Stroke::new(0.8, color), false)
        }
        SphereCurveKind::Reactance(_) => (Stroke::new(0.7, palette.grid_minor), false),
    }
}

/// Draws the parts of a polyline on the requested side of the sphere.
fn paint_polyline(
    painter: &egui::Painter,
    projector: &Projector,
    points: &[SpherePoint],
    stroke: Stroke,
    front: bool,
) {
    let projected: Vec<(Pos2, f32)> = points
        .iter()
        .map(|point| projector.project(*point))
        .collect();
    let mut run: Vec<Pos2> = Vec::new();
    for pair in projected.windows(2) {
        let (a, depth_a) = pair[0];
        let (b, depth_b) = pair[1];
        let midpoint_front = (depth_a + depth_b) >= 0.0;
        if midpoint_front == front {
            if run.is_empty() {
                run.push(a);
            }
            run.push(b);
        } else if !run.is_empty() {
            painter.add(Shape::line(std::mem::take(&mut run), stroke));
        }
    }
    if run.len() > 1 {
        painter.add(Shape::line(run, stroke));
    }
}

fn circle_points(center: Pos2, radius: f32, count: usize) -> Vec<Pos2> {
    (0..=count)
        .map(|index| {
            let angle = std::f32::consts::TAU * index as f32 / count as f32;
            center + vec2(angle.cos(), angle.sin()) * radius
        })
        .collect()
}

/// Shaded, hemisphere-tinted body built from the front-facing triangles of a
/// latitude/longitude tessellation.
fn sphere_mesh(projector: &Projector, palette: &Palette) -> Mesh {
    const RINGS: usize = 28;
    const SECTORS: usize = 56;
    let light = normalize([-0.35, 0.55, 0.76]);
    let mut mesh = Mesh::default();
    let mut depths = Vec::with_capacity((RINGS + 1) * (SECTORS + 1));

    // Poles on the w axis so the hemisphere boundary is a ring of vertices.
    let surface_point = |ring: usize, sector: usize| {
        let theta = std::f64::consts::PI * ring as f64 / RINGS as f64;
        let phi = std::f64::consts::TAU * sector as f64 / SECTORS as f64;
        SpherePoint {
            u: theta.sin() * phi.cos(),
            v: theta.sin() * phi.sin(),
            w: theta.cos(),
        }
    };
    let vertex_index = |ring: usize, sector: usize| (ring * (SECTORS + 1) + sector) as u32;

    for ring in 0..=RINGS {
        for sector in 0..=SECTORS {
            let point = surface_point(ring, sector);
            let [x, y, depth] = projector.camera.view(point);
            // Keep the surface pale so foreground traces retain their contrast.
            // Grid overlap and front/back visibility provide most of the depth.
            let shade = 0.99 + 0.01 * (x * light[0] + y * light[1] + depth * light[2]).max(0.0);
            let tint = if point.w >= 0.0 {
                palette.positive_fill
            } else {
                palette.negative_fill
            };
            let [r, g, b, _] = tint.to_array();
            let color = Color32::from_rgba_unmultiplied(
                (f32::from(r) * shade) as u8,
                (f32::from(g) * shade) as u8,
                (f32::from(b) * shade) as u8,
                238,
            );
            mesh.vertices.push(Vertex {
                pos: pos2(
                    projector.center.x + x * projector.radius,
                    projector.center.y - y * projector.radius,
                ),
                uv: WHITE_UV,
                color,
            });
            depths.push(depth);
        }
    }

    for ring in 0..RINGS {
        for sector in 0..SECTORS {
            let corners = [
                (ring, sector),
                (ring + 1, sector),
                (ring + 1, sector + 1),
                (ring, sector + 1),
            ];
            let average_depth: f32 = corners
                .iter()
                .map(|(r, s)| depths[vertex_index(*r, *s) as usize])
                .sum::<f32>()
                / 4.0;
            if average_depth < -0.01 {
                continue;
            }
            let [a, b, c, d] = corners.map(|(r, s)| vertex_index(r, s));
            mesh.add_triangle(a, b, c);
            mesh.add_triangle(a, c, d);
        }
    }
    mesh
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / length, v[1] / length, v[2] / length]
}

fn paint_landmark_labels(
    painter: &egui::Painter,
    projector: &Projector,
    palette: &Palette,
    lang: Lang,
) {
    let landmarks = [
        (SpherePoint::SHORT, lang.pick("短路 Z = 0", "short Z = 0")),
        (SpherePoint::OPEN, lang.pick("开路 Z = ∞", "open Z = ∞")),
        (SpherePoint::MATCH, "+Z0"),
        (SpherePoint::NEGATIVE_MATCH, "−Z0"),
        (SpherePoint::INDUCTIVE_UNIT, "+jZ0"),
        (SpherePoint::CAPACITIVE_UNIT, "−jZ0"),
    ];
    let font = FontId::proportional(11.0);
    for (point, label) in landmarks {
        let (screen, depth) = projector.project(point);
        if depth < -0.001 {
            continue;
        }
        painter.circle_filled(screen, 2.5, palette.boundary);
        let offset = vec2(6.0, -6.0);
        painter.text(
            screen + offset,
            Align2::LEFT_BOTTOM,
            label,
            font.clone(),
            palette.text,
        );
    }
}

fn nearest_point(
    projector: &Projector,
    traces: &[PlottedTrace],
    pointer: Pos2,
) -> Option<PointRef> {
    let mut best: Option<(f32, PointRef)> = None;
    for trace in traces {
        for point in trace.valid_points() {
            let (screen, depth) = projector.project(point.sphere);
            if depth < -0.02 {
                continue;
            }
            let distance = screen.distance(pointer);
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
