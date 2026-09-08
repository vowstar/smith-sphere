//! Application state, layout, and the linkage between the three views.

use crate::dialogs::{CsvForm, Dialog, DialogOutcome, ImpedanceForm, PasteForm, swatch};
use crate::io::{self, LoadedFile, PendingFile};
use crate::{fonts, theme};
use egui::{Align, Color32, Frame, Layout, Rect, RichText, Stroke, Ui, vec2};
use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
use smith_sphere_core::demo::Example;
use smith_sphere_core::parse::csv::inspect_csv;
use smith_sphere_core::parse::touchstone::parse_touchstone;
use smith_sphere_core::parse::{DetectedFormat, detect_format};
use smith_sphere_core::{DataSource, Document, Region, Trace, TraceOrigin, format};
use smith_sphere_render::scene::{ChartGrid, SphereCurve};
use smith_sphere_render::{
    Camera, ChartLabels, GridDetail, Palette, PlottedSample, PlottedTrace, PointRef,
    SelectionState, chart_grid, paint_chart, paint_sphere, plot_document, sphere_grid, trace_color,
};

const WIDE_LAYOUT_THRESHOLD: f32 = 940.0;
const SIDE_PANEL_WIDTH: f32 = 318.0;
const TOOLBAR_HEIGHT: f32 = 34.0;
const MIN_CHART_HEIGHT: f32 = 250.0;
const MIN_SPHERE_HEIGHT: f32 = 200.0;
const PROJECT_URL: &str = "https://github.com/vowstar/smith-sphere";
const MANUAL_DOCUMENT_NAME: &str = "手动输入";

/// Settings that survive restarts.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    plot_z0: f64,
    detailed_grid: bool,
    ohm_labels: bool,
    show_landmarks: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            plot_z0: 50.0,
            detailed_grid: false,
            ohm_labels: false,
            show_landmarks: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NoticeKind {
    Info,
    Error,
}

#[derive(Clone, Debug)]
struct Notice {
    kind: NoticeKind,
    message: String,
    hint: Option<String>,
}

/// Interactive workspace shared by native and browser builds.
pub struct SmithSphereApp {
    documents: Vec<Document>,
    preferences: Preferences,
    z0_text: String,
    selection: SelectionState,
    camera: Camera,
    camera_target: Option<Camera>,
    dialog: Option<Dialog>,
    notice: Option<Notice>,
    pending_file: PendingFile,
    palette: Palette,
    chart_grid: ChartGrid,
    sphere_grid: Vec<SphereCurve>,
    plotted: Vec<PlottedTrace>,
    /// Hover hit collected while the views are painted; committed to
    /// `selection.hovered` at the end of the frame so panels drawn before the
    /// views still see the latest preview.
    frame_hover: Option<PointRef>,
    selected_hidden: bool,
    advanced_open: bool,
    /// Window size requested through `SMITH_SPHERE_WINDOW`, applied on the
    /// first frame because eframe restores the persisted size at startup.
    #[cfg(not(target_arch = "wasm32"))]
    startup_size: Option<[f32; 2]>,
    #[cfg(all(feature = "capture", not(target_arch = "wasm32")))]
    capture: Option<CaptureState>,
}

#[cfg(all(feature = "capture", not(target_arch = "wasm32")))]
struct CaptureState {
    path: std::path::PathBuf,
    frames: u32,
    requested: bool,
}

impl Default for SmithSphereApp {
    fn default() -> Self {
        let preferences = Preferences::default();
        let mut app = Self {
            documents: Vec::new(),
            z0_text: format::significant(preferences.plot_z0, 6),
            preferences,
            selection: SelectionState::default(),
            camera: Camera::HOME,
            camera_target: None,
            dialog: None,
            notice: None,
            pending_file: PendingFile::default(),
            palette: Palette::default(),
            chart_grid: chart_grid(GridDetail::Basic),
            sphere_grid: sphere_grid(GridDetail::Basic),
            plotted: Vec::new(),
            frame_hover: None,
            selected_hidden: false,
            advanced_open: false,
            #[cfg(not(target_arch = "wasm32"))]
            startup_size: None,
            #[cfg(all(feature = "capture", not(target_arch = "wasm32")))]
            capture: None,
        };
        app.rebuild_grids();
        app
    }
}

impl SmithSphereApp {
    /// Creates the application and restores persisted preferences.
    #[must_use]
    pub fn new(context: &eframe::CreationContext<'_>) -> Self {
        theme::install(&context.egui_ctx);
        fonts::install(&context.egui_ctx);
        let mut app = Self::default();
        if let Some(storage) = context.storage
            && let Some(preferences) = eframe::get_value::<Preferences>(storage, eframe::APP_KEY)
        {
            app.preferences = preferences;
            app.z0_text = format::significant(app.preferences.plot_z0, 6);
        }
        app.rebuild_grids();
        #[cfg(not(target_arch = "wasm32"))]
        app.apply_startup_environment();
        app
    }

    /// Reads `SMITH_SPHERE_STARTUP`, `SMITH_SPHERE_SELECT`, `SMITH_SPHERE_VIEW`
    /// and `SMITH_SPHERE_CAPTURE` so layouts can be verified without a person
    /// clicking through the interface.
    #[cfg(not(target_arch = "wasm32"))]
    fn apply_startup_environment(&mut self) {
        self.startup_size = std::env::var("SMITH_SPHERE_WINDOW").ok().and_then(|value| {
            let (w, h) = value.split_once('x')?;
            Some([w.parse::<f32>().ok()?, h.parse::<f32>().ok()?])
        });
        if let Ok(startup) = std::env::var("SMITH_SPHERE_STARTUP") {
            for item in startup
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
            {
                match item {
                    "rlc" => self.add_document(Example::SeriesRlc.build()),
                    "negative" => self.add_document(Example::NegativeResistance.build()),
                    "crossing" => self.add_document(Example::BoundaryCrossing.build()),
                    "landmarks" => self.add_document(Example::Landmarks.build()),
                    path => match std::fs::read(path) {
                        Ok(bytes) => {
                            let name = std::path::Path::new(path)
                                .file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_else(|| path.to_owned());
                            self.ingest_file(LoadedFile::from_bytes(name, &bytes));
                        }
                        Err(error) => self.set_error(format!("无法读取 {path}：{error}"), None),
                    },
                }
            }
        }
        if let Some(index) = std::env::var("SMITH_SPHERE_SELECT")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            && let Some(document) = self.documents.first()
        {
            let trace = document.primary_trace_index();
            self.select(Some(PointRef {
                document: 0,
                trace,
                sample: index,
            }));
        }
        match std::env::var("SMITH_SPHERE_VIEW").as_deref() {
            Ok("positive") => self.camera = self.camera.positive_view(),
            Ok("negative") => self.camera = self.camera.negative_view(),
            _ => {}
        }
        #[cfg(feature = "capture")]
        if let Ok(path) = std::env::var("SMITH_SPHERE_CAPTURE") {
            self.capture = Some(CaptureState {
                path: path.into(),
                frames: 0,
                requested: false,
            });
        }
    }

    fn rebuild_grids(&mut self) {
        let detail = if self.preferences.detailed_grid {
            GridDetail::Detailed
        } else {
            GridDetail::Basic
        };
        self.chart_grid = chart_grid(detail);
        self.sphere_grid = sphere_grid(detail);
    }

    fn replot(&mut self) {
        let mut color = 0;
        let z0 = self.preferences.plot_z0;
        self.plotted.clear();
        for (index, document) in self.documents.iter().enumerate() {
            let traces = plot_document(index, document, z0, color);
            color += traces.len();
            self.plotted.extend(traces);
        }
        if let Some(selected) = self.selection.selected
            && self.plotted_point(selected).is_none()
        {
            self.selection.selected = None;
        }
        self.selection.hovered = None;
    }

    fn plotted_point(&self, reference: PointRef) -> Option<&PlottedSample> {
        self.plotted
            .iter()
            .find(|trace| trace.document == reference.document && trace.trace == reference.trace)
            .and_then(|trace| trace.point(reference.sample))
    }

    fn plotted_trace(&self, document: usize, trace: usize) -> Option<&PlottedTrace> {
        self.plotted
            .iter()
            .find(|plotted| plotted.document == document && plotted.trace == trace)
    }

    /// The point shown in the details panel: hover preview wins over the fixed
    /// selection.
    fn current_point(&self) -> Option<(PointRef, bool)> {
        if let Some(hovered) = self.selection.hovered
            && self.plotted_point(hovered).is_some()
        {
            return Some((hovered, true));
        }
        self.selection
            .selected
            .filter(|selected| self.plotted_point(*selected).is_some())
            .map(|point| (point, false))
    }

    fn select(&mut self, reference: Option<PointRef>) {
        self.selection.selected =
            reference.filter(|reference| self.plotted_point(*reference).is_some());
    }

    fn set_error(&mut self, message: String, hint: Option<String>) {
        self.notice = Some(Notice {
            kind: NoticeKind::Error,
            message,
            hint,
        });
    }

    fn set_info(&mut self, message: String) {
        self.notice = Some(Notice {
            kind: NoticeKind::Info,
            message,
            hint: None,
        });
    }

    fn add_document(&mut self, document: Document) {
        let index = self.documents.len();
        let trace_index = document.primary_trace_index();
        let sample_count: usize = document
            .visible_traces()
            .map(|(_, trace)| trace.valid_sample_count())
            .sum();
        let name = document.name.clone();
        let kind = document.source.kind_label();
        self.documents.push(document);
        self.replot();
        let first_valid = self.documents[index]
            .traces
            .get(trace_index)
            .and_then(|trace| {
                trace
                    .samples
                    .iter()
                    .position(|sample| sample.impedance.is_some())
            });
        if let Some(sample) = first_valid {
            self.select(Some(PointRef {
                document: index,
                trace: trace_index,
                sample,
            }));
        }
        self.set_info(format!(
            "已载入 {name}（{kind}），共 {sample_count} 个有效数据点。"
        ));
    }

    fn add_manual_trace(&mut self, trace: Trace) {
        let existing = self
            .documents
            .iter()
            .position(|document| matches!(document.source, DataSource::Manual));
        let (document_index, trace_index) = match existing {
            Some(index) => {
                self.documents[index].traces.push(trace);
                (index, self.documents[index].traces.len() - 1)
            }
            None => {
                self.documents.push(Document::new(
                    MANUAL_DOCUMENT_NAME,
                    DataSource::Manual,
                    vec![trace],
                ));
                (self.documents.len() - 1, 0)
            }
        };
        self.replot();
        self.select(Some(PointRef {
            document: document_index,
            trace: trace_index,
            sample: 0,
        }));
        let label = self.documents[document_index].traces[trace_index]
            .label
            .clone();
        self.set_info(format!("已添加 {label}。"));
    }

    fn remove_document(&mut self, index: usize) {
        if index < self.documents.len() {
            self.documents.remove(index);
            self.selection = SelectionState::default();
            self.replot();
        }
    }

    /// Parses a file or pasted text and adds it, or opens the table dialog
    /// when the file lacks information.
    fn ingest_file(&mut self, file: LoadedFile) {
        let format = detect_format(Some(&file.name), &file.text);
        let format = if matches!(format, DetectedFormat::Csv) && !file.name.contains('.') {
            detect_format(None, &file.text)
        } else {
            format
        };
        match format {
            DetectedFormat::Touchstone { ports } => {
                match parse_touchstone(&file.text, &file.name, ports) {
                    Ok(document) => self.add_document(document),
                    Err(error) => self.set_error(error.message, error.hint),
                }
            }
            DetectedFormat::Csv => match inspect_csv(&file.text, &file.name) {
                Ok(layout) => {
                    if layout.needs_unit() || layout.needs_reference() {
                        self.dialog =
                            Some(Dialog::Csv(CsvForm::new(layout, self.preferences.plot_z0)));
                    } else {
                        match smith_sphere_core::parse::csv::build_csv_document(
                            &layout,
                            None,
                            Some(self.preferences.plot_z0),
                        ) {
                            Ok(document) => self.add_document(document),
                            Err(error) => self.set_error(error.message, error.hint),
                        }
                    }
                }
                Err(error) => self.set_error(error.message, error.hint),
            },
        }
    }

    fn apply_z0_text(&mut self) {
        match self.z0_text.trim().parse::<f64>() {
            Ok(value) if value.is_finite() && value > 0.0 => {
                if (value - self.preferences.plot_z0).abs() > f64::EPSILON {
                    self.preferences.plot_z0 = value;
                    self.replot();
                }
            }
            _ => {
                self.z0_text = format::significant(self.preferences.plot_z0, 6);
                self.set_error(
                    "绘图参考阻抗 Z0 必须是正实数。".to_owned(),
                    Some("第一版仅支持正实数 Z0。".to_owned()),
                );
            }
        }
    }

    /// Trace that the frequency slider follows.
    fn slider_target(&self) -> Option<(usize, usize)> {
        if let Some(selected) = self.selection.selected
            && self
                .documents
                .get(selected.document)
                .and_then(|d| d.traces.get(selected.trace))
                .is_some_and(Trace::has_frequency_axis)
        {
            return Some((selected.document, selected.trace));
        }
        self.documents
            .iter()
            .enumerate()
            .find_map(|(index, document)| {
                let trace = document.primary_trace_index();
                document
                    .traces
                    .get(trace)
                    .filter(|trace| trace.has_frequency_axis())
                    .map(|_| (index, trace))
            })
    }

    fn step_selection(&mut self, direction: isize) {
        let Some((document, trace)) = self.slider_target() else {
            return;
        };
        let Some(plotted) = self.plotted_trace(document, trace) else {
            return;
        };
        let current = self
            .selection
            .selected
            .filter(|selected| selected.document == document && selected.trace == trace)
            .map(|selected| selected.sample as isize);
        let count = plotted.points.len() as isize;
        let mut index = current.map_or(0, |current| current + direction);
        while (0..count).contains(&index) {
            if plotted.points[index as usize].is_some() {
                self.select(Some(PointRef {
                    document,
                    trace,
                    sample: index as usize,
                }));
                return;
            }
            index += direction;
        }
    }

    fn handle_keyboard(&mut self, context: &egui::Context) {
        if self.dialog.is_some() || context.memory(|memory| memory.focused().is_some()) {
            return;
        }
        let (left, right) = context.input(|input| {
            (
                input.key_pressed(egui::Key::ArrowLeft),
                input.key_pressed(egui::Key::ArrowRight),
            )
        });
        if left {
            self.step_selection(-1);
        }
        if right {
            self.step_selection(1);
        }
    }

    fn animate_camera(&mut self, context: &egui::Context) {
        if let Some(target) = self.camera_target {
            self.camera = self.camera.approach(target, 0.25);
            if self.camera.distance_to(target) < 0.3 {
                self.camera = target;
                self.camera_target = None;
            }
            context.request_repaint();
        }
    }

    fn process_incoming_files(&mut self, context: &egui::Context) {
        let pending = self.pending_file.borrow_mut().take();
        if let Some(file) = pending {
            self.ingest_file(file);
        }
        for file in io::dropped_files(context, &self.pending_file) {
            self.ingest_file(file);
        }
    }

    fn open_file_dialog(&mut self, context: &egui::Context) {
        match io::pick_file(context, &self.pending_file) {
            Ok(Some(file)) => self.ingest_file(file),
            Ok(None) => {}
            Err(error) => self.set_error(error, None),
        }
    }

    fn show_dialog(&mut self, context: &egui::Context) {
        let Some(dialog) = self.dialog.as_mut() else {
            return;
        };
        let outcome = dialog.show(context, self.preferences.plot_z0);
        match outcome {
            DialogOutcome::Keep => {}
            DialogOutcome::Close => self.dialog = None,
            DialogOutcome::AddManualTrace(trace) => {
                self.add_manual_trace(trace);
            }
            DialogOutcome::LoadText(file) => {
                self.dialog = None;
                self.ingest_file(file);
            }
            DialogOutcome::LoadDocument(document) => {
                self.dialog = None;
                self.add_document(document);
            }
        }
    }

    fn show_header(&mut self, ui: &mut Ui) {
        let narrow = ui.available_width() < 720.0;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("SmithSphere").size(18.0).strong());
            ui.label(
                RichText::new("史密斯球")
                    .size(15.0)
                    .color(theme::TEXT_MUTED),
            );
            if !narrow {
                ui.add_space(12.0);
            }
            if ui.button("打开文件").clicked() {
                self.open_file_dialog(ui.ctx());
            }
            if ui.button("输入阻抗").clicked() {
                self.dialog = Some(Dialog::Impedance(ImpedanceForm::new(
                    self.preferences.plot_z0,
                )));
            }
            if ui.button("试用示例").clicked() {
                self.dialog = Some(Dialog::Examples);
            }
            if ui.button("粘贴数据").clicked() {
                self.dialog = Some(Dialog::Paste(PasteForm::default()));
            }
            ui.add_space(8.0);
            ui.label("绘图 Z0");
            let response =
                ui.add(egui::TextEdit::singleline(&mut self.z0_text).desired_width(56.0));
            ui.label("Ω");
            if response.lost_focus() {
                self.apply_z0_text();
            }
            if narrow {
                ui.toggle_value(&mut self.advanced_open, "高级设置");
                if ui.button("关于").clicked() {
                    self.dialog = Some(Dialog::About);
                }
            } else {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("关于").clicked() {
                        self.dialog = Some(Dialog::About);
                    }
                    ui.toggle_value(&mut self.advanced_open, "高级设置");
                });
            }
        });
    }

    fn show_notice(&mut self, ui: &mut Ui) {
        let Some(notice) = self.notice.clone() else {
            return;
        };
        let (fill, stroke, prefix) = match notice.kind {
            NoticeKind::Info => (Color32::from_rgb(234, 243, 239), theme::OK, "提示"),
            NoticeKind::Error => (Color32::from_rgb(250, 236, 236), theme::CORAL, "无法载入"),
        };
        Frame::new()
            .fill(fill)
            .stroke(Stroke::new(1.0, stroke))
            .corner_radius(6)
            .inner_margin(8)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button("关闭").clicked() {
                        self.notice = None;
                    }
                    ui.label(
                        RichText::new(format!("{prefix}：{}", notice.message)).color(theme::TEXT),
                    );
                });
                if let Some(hint) = &notice.hint {
                    ui.small(format!("下一步：{hint}"));
                }
            });
    }

    fn show_welcome(&mut self, ui: &mut Ui) {
        ui.add_space(ui.available_height() * 0.12);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("史密斯球").size(30.0).strong());
            ui.label(
                RichText::new(
                    "把完整的复阻抗平面映射到球面；用两张圆图分别精确读取负电阻区和正电阻区。",
                )
                .color(theme::TEXT_MUTED),
            );
            ui.add_space(18.0);
            let compact = ui.available_width() < 700.0;
            let card = |ui: &mut Ui, title: &str, body: &str| -> bool {
                let width = if compact {
                    ui.available_width().min(420.0)
                } else {
                    210.0
                };
                let mut clicked = false;
                Frame::new()
                    .fill(theme::SURFACE)
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .corner_radius(8)
                    .inner_margin(14)
                    .show(ui, |ui| {
                        ui.set_width(width);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(title).size(16.0).strong());
                            ui.small(body);
                            ui.add_space(6.0);
                            clicked = ui.button("开始").clicked();
                        });
                    });
                clicked
            };
            let mut open = false;
            let mut manual = false;
            let mut examples = false;
            if compact {
                open = card(
                    ui,
                    "打开文件",
                    ".s1p / .s2p（Touchstone 1.x）或 CSV（frequency、R、X）。",
                );
                manual = card(
                    ui,
                    "输入阻抗",
                    "手动输入 R、X，或粘贴 25+j30 这样的表达式。",
                );
                examples = card(
                    ui,
                    "试用示例",
                    "内置的演示数据：RLC 扫频、负电阻、跨越 R = 0。",
                );
            } else {
                ui.horizontal(|ui| {
                    open = card(
                        ui,
                        "打开文件",
                        ".s1p / .s2p（Touchstone 1.x）或 CSV（frequency、R、X）。",
                    );
                    manual = card(
                        ui,
                        "输入阻抗",
                        "手动输入 R、X，或粘贴 25+j30 这样的表达式。",
                    );
                    examples = card(
                        ui,
                        "试用示例",
                        "内置的演示数据：RLC 扫频、负电阻、跨越 R = 0。",
                    );
                });
            }
            if open {
                self.open_file_dialog(ui.ctx());
            }
            if manual {
                self.dialog = Some(Dialog::Impedance(ImpedanceForm::new(
                    self.preferences.plot_z0,
                )));
            }
            if examples {
                self.dialog = Some(Dialog::Examples);
            }
            ui.add_space(12.0);
            ui.small("也可以把文件直接拖入窗口。示例均标注为演示数据，不是实测结果。");
        });
    }

    fn sphere_toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("三维史密斯球").strong());
            if ui.small_button("复位视角").clicked() {
                self.camera_target = Some(Camera {
                    zoom: 1.0,
                    ..Camera::HOME
                });
            }
            if ui.small_button("查看正区").clicked() {
                self.camera_target = Some(self.camera.positive_view());
            }
            if ui.small_button("查看负区").clicked() {
                self.camera_target = Some(self.camera.negative_view());
            }
            let locate_enabled = self.selection.selected.is_some();
            let locate_label = if self.selected_hidden {
                "定位选中点（被遮挡）"
            } else {
                "定位选中点"
            };
            if ui
                .add_enabled(locate_enabled, egui::Button::new(locate_label).small())
                .clicked()
                && let Some(selected) = self.selection.selected
                && let Some(point) = self.plotted_point(selected)
            {
                self.camera_target = Some(self.camera.looking_at(point.sphere));
            }
            if ui.small_button("放大").clicked() {
                self.camera.zoom_by(1.2);
            }
            if ui.small_button("缩小").clicked() {
                self.camera.zoom_by(1.0 / 1.2);
            }
            if self.camera.yaw_deg.abs() > 90.0 {
                ui.label(
                    RichText::new("背面视角：左右与负区圆图相反（负圆是镜像投影）")
                        .small()
                        .color(theme::AMBER),
                );
            }
        });
    }

    fn sphere_view(&mut self, ui: &mut Ui, rect: Rect) {
        let mut camera = self.camera;
        let interaction = paint_sphere(
            ui,
            rect,
            &mut camera,
            &self.sphere_grid,
            &self.plotted,
            self.selection,
            &self.palette,
            self.preferences.show_landmarks,
        );
        if camera != self.camera {
            self.camera = camera;
            self.camera_target = None;
        }
        self.selected_hidden = interaction.selected_hidden;
        if interaction.response.hovered() {
            self.frame_hover = interaction.hovered;
        }
        if let Some(clicked) = interaction.clicked {
            self.select(Some(clicked));
        }
        let legend = ui.painter_at(rect);
        let legend_font = egui::FontId::proportional(11.0);
        let base = rect.left_bottom() + vec2(8.0, -8.0);
        legend.rect_filled(
            Rect::from_min_size(base + vec2(0.0, -11.0), vec2(11.0, 11.0)),
            2.0,
            self.palette.positive_fill,
        );
        legend.text(
            base + vec2(15.0, 0.0),
            egui::Align2::LEFT_BOTTOM,
            "R > 0 正电阻半球",
            legend_font.clone(),
            theme::TEXT_MUTED,
        );
        legend.rect_filled(
            Rect::from_min_size(base + vec2(118.0, -11.0), vec2(11.0, 11.0)),
            2.0,
            self.palette.negative_fill,
        );
        legend.text(
            base + vec2(133.0, 0.0),
            egui::Align2::LEFT_BOTTOM,
            "R < 0 负电阻半球",
            legend_font.clone(),
            theme::TEXT_MUTED,
        );
        legend.text(
            rect.right_bottom() + vec2(-8.0, -8.0),
            egui::Align2::RIGHT_BOTTOM,
            "拖动旋转 · 滚轮缩放",
            legend_font,
            theme::TEXT_MUTED,
        );
    }

    fn chart_labels(&self, region: Region) -> ChartLabels {
        let z0 = format::significant(self.preferences.plot_z0, 6);
        let ohm_scale = self
            .preferences
            .ohm_labels
            .then_some(self.preferences.plot_z0);
        let current = self
            .current_point()
            .and_then(|(reference, _)| self.plotted_point(reference).map(|point| point.region));
        match region {
            Region::Negative => ChartLabels {
                title: "负电阻区 / R < 0".to_owned(),
                subtitle: "镜像压缩投影：半径不是 |Γ|；网格为归一化值".to_owned(),
                center: format!("Z = −Z0 = −{z0} Ω（Γ 发散）"),
                note: match current {
                    Some(Region::Positive) => Some("当前点位于正电阻区，见右图".to_owned()),
                    Some(Region::Boundary) => Some("当前点在 R = 0 边界，两图同步显示".to_owned()),
                    _ => Some("◇ 边界交点为插值，不是采样点".to_owned()),
                },
                ohm_scale,
            },
            _ => ChartLabels {
                title: "正电阻区 / R > 0".to_owned(),
                subtitle: "传统史密斯圆图：半径 = |Γ|；网格为归一化值".to_owned(),
                center: format!("匹配 Z = Z0 = {z0} Ω"),
                note: match current {
                    Some(Region::Negative) => Some("当前点位于负电阻区，见左图".to_owned()),
                    Some(Region::Boundary) => Some("当前点在 R = 0 边界，两图同步显示".to_owned()),
                    _ => Some("◇ 边界交点为插值，不是采样点".to_owned()),
                },
                ohm_scale,
            },
        }
    }

    fn chart_view(&mut self, ui: &mut Ui, rect: Rect, region: Region) {
        let labels = self.chart_labels(region);
        ui.painter_at(rect).rect(
            rect,
            8.0,
            theme::SURFACE,
            Stroke::new(1.0, theme::BORDER),
            egui::StrokeKind::Inside,
        );
        let interaction = paint_chart(
            ui,
            rect,
            region,
            &self.chart_grid,
            &self.plotted,
            self.selection,
            &self.palette,
            &labels,
        );
        if interaction.response.hovered() {
            self.frame_hover = interaction.hovered;
        }
        if let Some(clicked) = interaction.clicked {
            self.select(Some(clicked));
        }
    }

    fn show_frequency_slider(&mut self, ui: &mut Ui) {
        let Some((document, trace)) = self.slider_target() else {
            return;
        };
        let Some(plotted) = self.plotted_trace(document, trace) else {
            return;
        };
        let count = plotted.points.len();
        if count < 2 {
            return;
        }
        let selected_index = self
            .selection
            .selected
            .filter(|selected| selected.document == document && selected.trace == trace)
            .map(|selected| selected.sample);
        let mut index = selected_index.unwrap_or(0);
        let label = plotted
            .point(index)
            .and_then(|point| point.frequency_hz)
            .map(format::frequency);
        let first = plotted
            .valid_points()
            .next()
            .and_then(|point| point.frequency_hz)
            .map(format::frequency)
            .unwrap_or_default();
        let last = plotted
            .valid_points()
            .last()
            .and_then(|point| point.frequency_hz)
            .map(format::frequency)
            .unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label(RichText::new("频率").strong());
            ui.label(RichText::new(label.unwrap_or_else(|| "断点".to_owned())).monospace());
            ui.small(format!("{first} – {last}，{count} 点"));
        });
        let response = ui.add(
            egui::Slider::new(&mut index, 0..=count - 1)
                .show_value(false)
                .step_by(1.0)
                .clamping(egui::SliderClamping::Always),
        );
        ui.small("滑块吸附采样点；方向键 ← → 逐点移动。");
        if response.changed() && Some(index) != selected_index {
            let plotted = self.plotted_trace(document, trace);
            let valid = plotted
                .is_some_and(|plotted| plotted.points.get(index).is_some_and(Option::is_some));
            if valid {
                self.select(Some(PointRef {
                    document,
                    trace,
                    sample: index,
                }));
            } else {
                let direction = if selected_index.is_some_and(|selected| index < selected) {
                    -1
                } else {
                    1
                };
                self.select(Some(PointRef {
                    document,
                    trace,
                    sample: index,
                }));
                self.step_selection(direction);
            }
        }
    }

    fn show_details(&self, ui: &mut Ui) {
        ui.label(RichText::new("当前点").strong());
        let Some((reference, preview)) = self.current_point() else {
            ui.small("悬停可预览，点击固定选中；也可以用频率滑块或方向键选择。");
            return;
        };
        let Some(point) = self.plotted_point(reference) else {
            return;
        };
        let Some(document) = self.documents.get(reference.document) else {
            return;
        };
        let Some(trace) = document.traces.get(reference.trace) else {
            return;
        };
        let color = self
            .plotted_trace(reference.document, reference.trace)
            .map(|t| trace_color(t.color_index))
            .unwrap_or(theme::TEXT);
        let state = if preview {
            "预览（悬停）"
        } else {
            "已选中（点击固定）"
        };
        ui.horizontal(|ui| {
            swatch(ui, color);
            ui.label(RichText::new(state).color(if preview {
                theme::TEXT_MUTED
            } else {
                theme::TEXT
            }));
        });
        let z0 = self.preferences.plot_z0;
        let gamma = point.impedance.reflection(z0);
        let (magnitude, phase) = format::reflection(gamma);
        let region = match point.region {
            Region::Positive => "正电阻区（右图）",
            Region::Negative => "负电阻区（左图）",
            Region::Boundary => "R = 0 共享边界（两图）",
        };
        egui::Grid::new("details_grid")
            .num_columns(2)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                ui.label("频率");
                ui.label(
                    point
                        .frequency_hz
                        .map(format::frequency)
                        .unwrap_or_else(|| "无频率".to_owned()),
                );
                ui.end_row();
                ui.label("Z");
                ui.label(format::impedance(point.impedance));
                ui.end_row();
                ui.label("z = Z/Z0");
                ui.label(format::normalized(point.normalized.finite()));
                ui.end_row();
                ui.label("|Γ|");
                ui.label(magnitude);
                ui.end_row();
                ui.label("∠Γ");
                ui.label(phase);
                ui.end_row();
                ui.label("绘图 Z0");
                ui.label(format!("{} Ω", format::significant(z0, 6)));
                ui.end_row();
                ui.label("区域");
                ui.label(region);
                ui.end_row();
                ui.label("轨迹");
                ui.label(trace_description(trace));
                ui.end_row();
                ui.label("来源");
                ui.label(document.source.kind_label());
                ui.end_row();
            });
        if (trace.source_z0 - z0).abs() > 1e-9 {
            ui.small(format!(
                "数据来源的参考阻抗为 {} Ω；物理阻抗按来源还原后再以绘图 Z0 归一化。",
                format::significant(trace.source_z0, 6)
            ));
        }
        if let Some(gamma) = gamma.finite()
            && gamma.abs() > 1.0
        {
            ui.small("|Γ| > 1：负电阻，反射增益。这不等于电路一定不稳定，需结合外部网络判断。");
        }
        if matches!(gamma, smith_sphere_core::Reflection::Divergent) {
            ui.small("Z = −Z0：反射系数分母为零，Γ 发散；球面与负圆位置仍然确定。");
        }
    }

    fn show_documents(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("数据").strong());
        let mut remove = None;
        let mut replot = false;
        for index in 0..self.documents.len() {
            let color_start = self
                .plotted
                .iter()
                .find(|trace| trace.document == index)
                .map(|trace| trace.color_index);
            let document = &mut self.documents[index];
            Frame::new().fill(theme::SURFACE).stroke(Stroke::new(1.0, theme::BORDER)).corner_radius(6).inner_margin(8).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&document.name).strong());
                    ui.label(RichText::new(document.source.kind_label()).small().color(theme::AMBER));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button("移除").clicked() {
                            remove = Some(index);
                        }
                    });
                });
                ui.small(document.source.detail());
                if document.has_variants() {
                    ui.horizontal(|ui| {
                        let mut selected = document.variant_selection.unwrap_or(0);
                        for (trace_index, trace) in document.traces.iter().enumerate() {
                            if ui.selectable_value(&mut selected, trace_index, &trace.label).changed() {
                                replot = true;
                            }
                        }
                        document.select_variant(selected);
                    });
                    ui.small("S11 是端口 1 的反射（端口 2 接匹配负载），S22 反之；它们对应各端口的等效阻抗，不是 Z 参数矩阵里的 Z11/Z22。");
                } else {
                    for (offset, (_, trace)) in document.visible_traces().enumerate() {
                        ui.horizontal(|ui| {
                            swatch(ui, trace_color(color_start.unwrap_or(0) + offset));
                            ui.small(format!("{}（{} 点，来源 Z0 {} Ω）", trace.label, trace.valid_sample_count(), format::significant(trace.source_z0, 6)));
                        });
                    }
                }
                for note in &document.notes {
                    ui.small(RichText::new(note).color(theme::TEXT_MUTED));
                }
            });
        }
        if let Some(index) = remove {
            self.remove_document(index);
        } else if replot {
            self.selection = SelectionState::default();
            self.replot();
        }
    }

    fn show_advanced(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("高级设置").strong());
        let mut changed = false;
        changed |= ui
            .checkbox(&mut self.preferences.detailed_grid, "详细网格")
            .changed();
        ui.checkbox(
            &mut self.preferences.ohm_labels,
            "网格以 Ω 标注（× 绘图 Z0）",
        );
        ui.checkbox(&mut self.preferences.show_landmarks, "球面显示特征点标签");
        ui.small("导纳网格、Q 圆与任意终接计算不在本版范围内。");
        if changed {
            self.rebuild_grids();
        }
    }

    fn show_side_panel(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(4.0);
                self.show_frequency_slider(ui);
                ui.separator();
                self.show_details(ui);
                ui.separator();
                self.show_documents(ui);
                if self.advanced_open {
                    ui.separator();
                    self.show_advanced(ui);
                }
                ui.add_space(8.0);
                ui.hyperlink_to("项目主页", PROJECT_URL);
            });
    }

    fn show_wide_workspace(&mut self, ui: &mut Ui) {
        let available = ui.available_rect_before_wrap();
        let gap = 10.0;
        let width = available.width();
        let height = available.height() - TOOLBAR_HEIGHT - 2.0 * gap;
        let chart_width = (width - gap) / 2.0;
        let mut chart_height = chart_width.min(height * 0.56).max(MIN_CHART_HEIGHT);
        let mut sphere_height = height - chart_height - gap;
        if sphere_height < MIN_SPHERE_HEIGHT {
            sphere_height = MIN_SPHERE_HEIGHT;
            chart_height = (height - sphere_height - gap).max(120.0);
        }

        let toolbar_rect = Rect::from_min_size(available.min, vec2(width, TOOLBAR_HEIGHT));
        let mut toolbar = ui.new_child(egui::UiBuilder::new().max_rect(toolbar_rect));
        self.sphere_toolbar(&mut toolbar);

        let sphere_rect = Rect::from_min_size(
            available.min + vec2(0.0, TOOLBAR_HEIGHT + gap),
            vec2(width, sphere_height),
        );
        ui.painter_at(sphere_rect).rect(
            sphere_rect,
            8.0,
            theme::SURFACE,
            Stroke::new(1.0, theme::BORDER),
            egui::StrokeKind::Inside,
        );
        self.sphere_view(ui, sphere_rect);

        let charts_top = sphere_rect.bottom() + gap;
        let negative_rect = Rect::from_min_size(
            egui::pos2(available.left(), charts_top),
            vec2(chart_width, chart_height),
        );
        let positive_rect = Rect::from_min_size(
            egui::pos2(available.left() + chart_width + gap, charts_top),
            vec2(chart_width, chart_height),
        );
        self.chart_view(ui, negative_rect, Region::Negative);
        self.chart_view(ui, positive_rect, Region::Positive);
        ui.allocate_rect(
            Rect::from_min_max(
                available.min,
                egui::pos2(available.right(), positive_rect.bottom()),
            ),
            egui::Sense::hover(),
        );
    }

    fn show_narrow_workspace(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let width = ui.available_width();
                self.sphere_toolbar(ui);
                let sphere_height = (width * 0.9).clamp(240.0, 420.0);
                let (sphere_rect, _) =
                    ui.allocate_exact_size(vec2(width, sphere_height), egui::Sense::hover());
                ui.painter_at(sphere_rect).rect(
                    sphere_rect,
                    8.0,
                    theme::SURFACE,
                    Stroke::new(1.0, theme::BORDER),
                    egui::StrokeKind::Inside,
                );
                self.sphere_view(ui, sphere_rect);
                ui.add_space(6.0);
                self.show_frequency_slider(ui);
                ui.add_space(6.0);
                let chart_height = (width * 1.02).clamp(300.0, 520.0);
                let (negative_rect, _) =
                    ui.allocate_exact_size(vec2(width, chart_height), egui::Sense::hover());
                self.chart_view(ui, negative_rect, Region::Negative);
                ui.add_space(6.0);
                let (positive_rect, _) =
                    ui.allocate_exact_size(vec2(width, chart_height), egui::Sense::hover());
                self.chart_view(ui, positive_rect, Region::Positive);
                ui.add_space(8.0);
                self.show_details(ui);
                ui.separator();
                self.show_documents(ui);
                if self.advanced_open {
                    ui.separator();
                    self.show_advanced(ui);
                }
                ui.add_space(8.0);
                ui.hyperlink_to("项目主页", PROJECT_URL);
            });
    }

    #[cfg(all(feature = "capture", not(target_arch = "wasm32")))]
    fn handle_capture(&mut self, context: &egui::Context) {
        let Some(capture) = self.capture.as_mut() else {
            return;
        };
        capture.frames += 1;
        context.request_repaint();
        if capture.frames == 12 && !capture.requested {
            capture.requested = true;
            context.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let image = context.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            let path = capture.path.clone();
            let result = write_png(&path, &image);
            match result {
                Ok(()) => log::info!("wrote capture to {}", path.display()),
                Err(error) => log::error!("capture failed: {error}"),
            }
            self.capture = None;
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

#[cfg(all(feature = "capture", not(target_arch = "wasm32")))]
fn write_png(path: &std::path::Path, image: &egui::ColorImage) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|error| error.to_string())?;
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        image.width() as u32,
        image.height() as u32,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    let bytes: Vec<u8> = image
        .pixels
        .iter()
        .flat_map(|pixel| pixel.to_srgba_unmultiplied())
        .collect();
    writer
        .write_image_data(&bytes)
        .map_err(|error| error.to_string())
}

fn trace_description(trace: &Trace) -> String {
    match &trace.origin {
        TraceOrigin::TwoPortS(variant) => format!(
            "{}：端口 {} 的反射，另一端口匹配终接",
            variant.label(),
            variant.port_number()
        ),
        TraceOrigin::OnePortS => "S11：单端口反射".to_owned(),
        TraceOrigin::OnePortZ => "Z11：单端口归一化阻抗".to_owned(),
        TraceOrigin::OnePortY => "Y11：单端口归一化导纳".to_owned(),
        TraceOrigin::ImpedanceColumns => format!("{}：表格 R、X 列", trace.label),
        TraceOrigin::ReflectionColumns => format!("{}：表格反射系数列", trace.label),
        TraceOrigin::ManualImpedance | TraceOrigin::ManualReflection => {
            format!("{}：手动输入", trace.label)
        }
        TraceOrigin::Demo => format!("{}：演示数据", trace.label),
    }
}

impl eframe::App for SmithSphereApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.preferences);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(size) = self.startup_size.take() {
            context.send_viewport_cmd(egui::ViewportCommand::InnerSize(vec2(size[0], size[1])));
        }
        self.process_incoming_files(&context);
        self.animate_camera(&context);
        self.handle_keyboard(&context);
        self.frame_hover = None;

        egui::Panel::top("header")
            .frame(
                Frame::new()
                    .fill(theme::SURFACE)
                    .inner_margin(egui::Margin::symmetric(12, 8)),
            )
            .resizable(false)
            .show(ui, |ui| {
                self.show_header(ui);
            });

        let wide = context.content_rect().width() >= WIDE_LAYOUT_THRESHOLD;
        let has_data = !self.documents.is_empty();
        if wide && has_data {
            egui::Panel::right("details")
                .exact_size(SIDE_PANEL_WIDTH)
                .resizable(false)
                .frame(Frame::new().fill(theme::SURFACE_RAISED).inner_margin(10))
                .show(ui, |ui| self.show_side_panel(ui));
        }

        egui::CentralPanel::default()
            .frame(Frame::new().fill(theme::CANVAS).inner_margin(10))
            .show(ui, |ui| {
                self.show_notice(ui);
                if !has_data {
                    self.show_welcome(ui);
                } else if wide {
                    self.show_wide_workspace(ui);
                } else {
                    self.show_narrow_workspace(ui);
                }
            });

        self.show_dialog(&context);

        if self.selection.hovered != self.frame_hover {
            self.selection.hovered = self.frame_hover;
            context.request_repaint();
        }

        #[cfg(all(feature = "capture", not(target_arch = "wasm32")))]
        self.handle_capture(&context);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, RawInput};
    use smith_sphere_core::demo::Example;

    fn app_with(example: Example) -> SmithSphereApp {
        let mut app = SmithSphereApp::default();
        app.add_document(example.build());
        app
    }

    #[test]
    fn loading_an_example_selects_its_first_point() {
        let app = app_with(Example::SeriesRlc);
        assert_eq!(
            app.selection.selected,
            Some(PointRef {
                document: 0,
                trace: 0,
                sample: 0
            })
        );
        assert_eq!(app.plotted.len(), 1);
    }

    #[test]
    fn landmarks_show_all_traces_and_offer_no_slider() {
        let app = app_with(Example::Landmarks);
        assert_eq!(app.plotted.len(), 6);
        assert!(app.slider_target().is_none());
    }

    #[test]
    fn arrow_stepping_skips_gaps_and_stays_in_range() {
        let mut app = SmithSphereApp::default();
        let samples = vec![
            smith_sphere_core::Sample::new(1.0, smith_sphere_core::Impedance::new(10.0, 0.0)),
            smith_sphere_core::Sample::gap(Some(2.0)),
            smith_sphere_core::Sample::new(3.0, smith_sphere_core::Impedance::new(30.0, 0.0)),
        ];
        app.add_document(Document::new(
            "g",
            DataSource::Manual,
            vec![Trace::new(
                "Z",
                samples,
                50.0,
                TraceOrigin::ImpedanceColumns,
            )],
        ));
        app.step_selection(1);
        assert_eq!(app.selection.selected.map(|p| p.sample), Some(2));
        app.step_selection(1);
        assert_eq!(app.selection.selected.map(|p| p.sample), Some(2));
        app.step_selection(-1);
        assert_eq!(app.selection.selected.map(|p| p.sample), Some(0));
    }

    #[test]
    fn changing_plot_z0_keeps_physical_impedance() {
        let mut app = app_with(Example::SeriesRlc);
        let before = app
            .plotted_point(PointRef {
                document: 0,
                trace: 0,
                sample: 3,
            })
            .expect("point")
            .impedance;
        app.z0_text = "75".to_owned();
        app.apply_z0_text();
        let after = app
            .plotted_point(PointRef {
                document: 0,
                trace: 0,
                sample: 3,
            })
            .expect("point");
        assert_eq!(before, after.impedance);
        assert_eq!(app.preferences.plot_z0, 75.0);
        app.z0_text = "-5".to_owned();
        app.apply_z0_text();
        assert_eq!(app.preferences.plot_z0, 75.0);
        assert!(
            app.notice
                .as_ref()
                .is_some_and(|notice| notice.kind == NoticeKind::Error)
        );
    }

    #[test]
    fn touchstone_two_port_can_switch_between_ports() {
        let mut app = SmithSphereApp::default();
        let text =
            "# GHz S RI R 50\n1 0.1 0.2 0.9 0 0.8 0 -0.3 0.4\n2 0.2 0.2 0.9 0 0.8 0 -0.3 -0.4\n";
        app.ingest_file(LoadedFile {
            name: "amp.s2p".to_owned(),
            text: text.to_owned(),
        });
        assert_eq!(app.documents.len(), 1);
        assert!(app.documents[0].has_variants());
        assert_eq!(app.plotted.len(), 1);
        app.documents[0].select_variant(1);
        app.replot();
        assert_eq!(app.plotted[0].trace, 1);
    }

    #[test]
    fn csv_without_unit_opens_the_import_dialog() {
        let mut app = SmithSphereApp::default();
        app.ingest_file(LoadedFile {
            name: "sweep.csv".to_owned(),
            text: "freq,R,X\n1,25,30\n2,30,10\n".to_owned(),
        });
        assert!(matches!(app.dialog, Some(Dialog::Csv(_))));
        assert!(app.documents.is_empty());
    }

    #[test]
    fn unsupported_touchstone_reports_an_error_instead_of_plotting() {
        let mut app = SmithSphereApp::default();
        app.ingest_file(LoadedFile {
            name: "x.s1p".to_owned(),
            text: "[Version] 2.0\n# GHz S MA R 50\n1 0.5 0\n".to_owned(),
        });
        assert!(app.documents.is_empty());
        assert!(app.notice.as_ref().is_some_and(
            |notice| notice.kind == NoticeKind::Error && notice.message.contains("2.0")
        ));
    }

    #[test]
    fn workspace_renders_at_desktop_and_narrow_sizes() {
        for width in [1280.0, 960.0, 800.0, 390.0] {
            let mut app = app_with(Example::BoundaryCrossing);
            app.add_document(Example::NegativeResistance.build());
            let context = egui::Context::default();
            theme::install(&context);
            let output = context.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 820.0))),
                    ..Default::default()
                },
                |ui| {
                    if width >= WIDE_LAYOUT_THRESHOLD {
                        egui::Panel::right("details")
                            .exact_size(SIDE_PANEL_WIDTH)
                            .show(ui, |ui| app.show_side_panel(ui));
                        egui::CentralPanel::default().show(ui, |ui| app.show_wide_workspace(ui));
                    } else {
                        egui::CentralPanel::default().show(ui, |ui| app.show_narrow_workspace(ui));
                    }
                },
            );
            output.drop_without_applying_deltas();
        }
    }
}
