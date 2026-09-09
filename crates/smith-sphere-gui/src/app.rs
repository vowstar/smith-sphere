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
use smith_sphere_core::{DataSource, Document, Lang, Region, Trace, TraceOrigin, format};
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
    lang: Lang,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            plot_z0: 50.0,
            detailed_grid: false,
            ohm_labels: false,
            show_landmarks: true,
            lang: Lang::Chinese,
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
        if let Some(lang) = system_language() {
            app.preferences.lang = lang;
        }
        if let Some(storage) = context.storage
            && let Some(preferences) = eframe::get_value::<Preferences>(storage, eframe::APP_KEY)
        {
            app.preferences = preferences;
            if !(app.preferences.plot_z0.is_finite() && app.preferences.plot_z0 > 0.0) {
                app.preferences.plot_z0 = Preferences::default().plot_z0;
            }
            app.z0_text = format::significant(app.preferences.plot_z0, 6);
        }
        app.rebuild_grids();
        #[cfg(not(target_arch = "wasm32"))]
        app.apply_startup_environment();
        app
    }

    /// Reads `SMITH_SPHERE_STARTUP`, `SMITH_SPHERE_SELECT`, `SMITH_SPHERE_VIEW`, `SMITH_SPHERE_LANG`
    /// and `SMITH_SPHERE_CAPTURE` so layouts can be verified without a person
    /// clicking through the interface.
    #[cfg(not(target_arch = "wasm32"))]
    fn apply_startup_environment(&mut self) {
        if let Ok(code) = std::env::var("SMITH_SPHERE_LANG")
            && let Some(lang) = Lang::from_code(&code)
        {
            self.preferences.lang = lang;
        }
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
                        Err(error) => {
                            let lang = self.lang();
                            self.set_error(
                                match lang {
                                    Lang::Chinese => format!("无法读取 {path}：{error}"),
                                    Lang::English => format!("Could not read {path}: {error}"),
                                },
                                None,
                            )
                        }
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

    fn lang(&self) -> Lang {
        self.preferences.lang
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
        let lang = self.lang();
        let name = document.display_name(lang);
        let kind = document.source.kind_label(lang);
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
        self.set_info(match lang {
            Lang::Chinese => {
                format!("已载入 {name}（{kind}），共 {sample_count} 个有效数据点。")
            }
            Lang::English => {
                format!("Loaded {name} ({kind}), {sample_count} valid data points.")
            }
        });
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
        let lang = self.lang();
        self.set_info(match lang {
            Lang::Chinese => format!("已添加 {label}。"),
            Lang::English => format!("Added {label}."),
        });
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
        let lang = self.lang();
        let format = detect_format(Some(&file.name), &file.text);
        let format = if matches!(format, DetectedFormat::Csv) && !file.name.contains('.') {
            detect_format(None, &file.text)
        } else {
            format
        };
        match format {
            DetectedFormat::Touchstone { ports } => {
                match parse_touchstone(&file.text, &file.name, ports, lang) {
                    Ok(document) => self.add_document(document),
                    Err(error) => self.set_error(error.message, error.hint),
                }
            }
            DetectedFormat::Csv => match inspect_csv(&file.text, &file.name, lang) {
                Ok(layout) => {
                    if layout.needs_unit() || layout.needs_reference() {
                        self.dialog =
                            Some(Dialog::Csv(CsvForm::new(layout, self.preferences.plot_z0)));
                    } else {
                        match smith_sphere_core::parse::csv::build_csv_document(
                            &layout,
                            None,
                            Some(self.preferences.plot_z0),
                            lang,
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
                let lang = self.lang();
                self.set_error(
                    lang.pick(
                        "绘图参考阻抗 Z0 必须是正实数。",
                        "The plotting reference impedance Z0 must be a positive real number.",
                    )
                    .to_owned(),
                    Some(
                        lang.pick(
                            "第一版仅支持正实数 Z0。",
                            "This version supports only a positive real Z0.",
                        )
                        .to_owned(),
                    ),
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
        let lang = self.lang();
        match io::pick_file(context, &self.pending_file, lang) {
            Ok(Some(file)) => self.ingest_file(file),
            Ok(None) => {}
            Err(error) => self.set_error(error, None),
        }
    }

    fn show_dialog(&mut self, context: &egui::Context) {
        let Some(dialog) = self.dialog.as_mut() else {
            return;
        };
        let lang = self.preferences.lang;
        let outcome = dialog.show(context, self.preferences.plot_z0, lang);
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
        let lang = self.lang();
        let narrow = ui.available_width() < 720.0;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("SmithSphere").size(18.0).strong());
            ui.label(
                RichText::new(lang.pick("史密斯球", "Impedance sphere"))
                    .size(15.0)
                    .color(theme::TEXT_MUTED),
            );
            if !narrow {
                ui.add_space(12.0);
            }
            if ui.button(lang.pick("打开文件", "Open file")).clicked() {
                self.open_file_dialog(ui.ctx());
            }
            if ui.button(lang.pick("输入阻抗", "Enter Z")).clicked() {
                self.dialog = Some(Dialog::Impedance(ImpedanceForm::new(
                    self.preferences.plot_z0,
                )));
            }
            if ui.button(lang.pick("试用示例", "Examples")).clicked() {
                self.dialog = Some(Dialog::Examples);
            }
            if ui.button(lang.pick("粘贴数据", "Paste data")).clicked() {
                self.dialog = Some(Dialog::Paste(PasteForm::default()));
            }
            ui.add_space(8.0);
            ui.label(lang.pick("绘图 Z0", "Plot Z0"));
            let response =
                ui.add(egui::TextEdit::singleline(&mut self.z0_text).desired_width(56.0));
            ui.label("Ω");
            if response.lost_focus() {
                self.apply_z0_text();
            }
            if narrow {
                self.language_selector(ui);
                ui.toggle_value(&mut self.advanced_open, lang.pick("高级设置", "Advanced"));
                if ui.button(lang.pick("关于", "About")).clicked() {
                    self.dialog = Some(Dialog::About);
                }
            } else {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(lang.pick("关于", "About")).clicked() {
                        self.dialog = Some(Dialog::About);
                    }
                    ui.toggle_value(&mut self.advanced_open, lang.pick("高级设置", "Advanced"));
                    self.language_selector(ui);
                });
            }
        });
    }

    fn language_selector(&mut self, ui: &mut Ui) {
        let current = self.preferences.lang;
        egui::ComboBox::from_id_salt("language")
            .selected_text(current.endonym())
            .width(96.0)
            .show_ui(ui, |ui| {
                for candidate in Lang::ALL {
                    ui.selectable_value(&mut self.preferences.lang, candidate, candidate.endonym());
                }
            });
    }

    fn show_notice(&mut self, ui: &mut Ui) {
        let lang = self.lang();
        let Some(notice) = self.notice.clone() else {
            return;
        };
        let (fill, stroke, prefix) = match notice.kind {
            NoticeKind::Info => (
                Color32::from_rgb(234, 243, 239),
                theme::OK,
                lang.pick("提示", "Note"),
            ),
            NoticeKind::Error => (
                Color32::from_rgb(250, 236, 236),
                theme::CORAL,
                lang.pick("无法载入", "Could not load"),
            ),
        };
        Frame::new()
            .fill(fill)
            .stroke(Stroke::new(1.0, stroke))
            .corner_radius(6)
            .inner_margin(8)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button(lang.pick("关闭", "Close")).clicked() {
                        self.notice = None;
                    }
                    ui.label(
                        RichText::new(format!(
                            "{prefix}{}{}",
                            lang.pick("：", ": "),
                            notice.message
                        ))
                        .color(theme::TEXT),
                    );
                });
                if let Some(hint) = &notice.hint {
                    ui.small(match lang {
                        Lang::Chinese => format!("下一步：{hint}"),
                        Lang::English => format!("Next: {hint}"),
                    });
                }
            });
    }

    fn show_welcome(&mut self, ui: &mut Ui) {
        let lang = self.lang();
        ui.add_space(ui.available_height() * 0.12);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(lang.pick("史密斯球", "Impedance sphere"))
                    .size(30.0)
                    .strong(),
            );
            ui.label(
                RichText::new(lang.pick(
                    "把完整的复阻抗平面映射到球面；用两张圆图分别精确读取负电阻区和正电阻区。",
                    "Maps the whole complex impedance plane onto a sphere and reads both halves on two planar charts.",
                ))
                .color(theme::TEXT_MUTED),
            );
            ui.add_space(18.0);
            let compact = ui.available_width() < 700.0;
            const CARD_WIDTH: f32 = 210.0;
            let frame = Frame::new()
                .fill(theme::SURFACE)
                .stroke(Stroke::new(1.0, theme::BORDER))
                .corner_radius(8)
                .inner_margin(14);
            let card = |ui: &mut Ui, title: &str, body: &str| -> bool {
                // Leave room for the frame margin and stroke so a full-width
                // card does not overflow and clip on a narrow screen.
                let width = if compact {
                    (ui.available_width() - 32.0).clamp(160.0, 420.0)
                } else {
                    CARD_WIDTH
                };
                let mut clicked = false;
                frame.show(ui, |ui| {
                    ui.set_width(width);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(title).size(16.0).strong());
                            ui.small(body);
                            ui.add_space(6.0);
                            clicked = ui.button(lang.pick("开始", "Start")).clicked();
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
                    lang.pick("打开文件", "Open file"),
                    lang.pick(
                        ".s1p / .s2p（Touchstone 1.x）或 CSV（frequency、R、X）。",
                        ".s1p / .s2p (Touchstone 1.x) or CSV (frequency, R, X).",
                    ),
                );
                manual = card(
                    ui,
                    lang.pick("输入阻抗", "Enter Z"),
                    lang.pick(
                        "手动输入 R、X，或粘贴 25+j30 这样的表达式。",
                        "Type R, X by hand, or paste an expression such as 25+j30.",
                    ),
                );
                examples = card(
                    ui,
                    lang.pick("试用示例", "Examples"),
                    lang.pick(
                        "内置的演示数据：RLC 扫频、负电阻、跨越 R = 0。",
                        "Built-in demo data: RLC sweep, negative resistance, R = 0 crossing.",
                    ),
                );
            } else {
                ui.horizontal(|ui| {
                    // A horizontal row takes the full width, so the outer
                    // centered layout cannot centre it. Pad the row by hand.
                    let outer = CARD_WIDTH + frame.total_margin().sum().x;
                    let row = 3.0 * outer + 2.0 * ui.spacing().item_spacing.x;
                    ui.add_space(((ui.available_width() - row) / 2.0).max(0.0));
                    open = card(
                        ui,
                        lang.pick("打开文件", "Open file"),
                        lang.pick(
                            ".s1p / .s2p（Touchstone 1.x）或 CSV（frequency、R、X）。",
                            ".s1p / .s2p (Touchstone 1.x) or CSV (frequency, R, X).",
                        ),
                    );
                    manual = card(
                        ui,
                        lang.pick("输入阻抗", "Enter Z"),
                        lang.pick(
                            "手动输入 R、X，或粘贴 25+j30 这样的表达式。",
                            "Type R, X by hand, or paste an expression such as 25+j30.",
                        ),
                    );
                    examples = card(
                        ui,
                        lang.pick("试用示例", "Examples"),
                        lang.pick(
                            "内置的演示数据：RLC 扫频、负电阻、跨越 R = 0。",
                            "Built-in demo data: RLC sweep, negative resistance, R = 0 crossing.",
                        ),
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
            ui.small(lang.pick(
                "也可以把文件直接拖入窗口。示例均标注为演示数据，不是实测结果。",
                "You can also drop a file onto the window. Examples are labeled demo data, not measurements.",
            ));
        });
    }

    fn sphere_toolbar(&mut self, ui: &mut Ui) {
        let lang = self.lang();
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(lang.pick("三维史密斯球", "Sphere")).strong());
            if ui.small_button(lang.pick("复位视角", "Reset view")).clicked() {
                self.camera_target = Some(Camera {
                    zoom: 1.0,
                    ..Camera::HOME
                });
            }
            if ui.small_button(lang.pick("查看正区", "Positive")).clicked() {
                self.camera_target = Some(self.camera.positive_view());
            }
            if ui.small_button(lang.pick("查看负区", "Negative")).clicked() {
                self.camera_target = Some(self.camera.negative_view());
            }
            let locate_enabled = self.selection.selected.is_some();
            let locate_label = if self.selected_hidden {
                lang.pick("定位选中点（被遮挡）", "Locate point (hidden)")
            } else {
                lang.pick("定位选中点", "Locate point")
            };
            if ui
                .add_enabled(locate_enabled, egui::Button::new(locate_label).small())
                .clicked()
                && let Some(selected) = self.selection.selected
                && let Some(point) = self.plotted_point(selected)
            {
                self.camera_target = Some(self.camera.looking_at(point.sphere));
            }
            if ui.small_button(lang.pick("放大", "Zoom in")).clicked() {
                self.camera.zoom_by(1.2);
            }
            if ui.small_button(lang.pick("缩小", "Zoom out")).clicked() {
                self.camera.zoom_by(1.0 / 1.2);
            }
            if self.camera.yaw_deg.abs() > 90.0 {
                ui.label(
                    RichText::new(lang.pick(
                        "背面视角：左右与负区圆图相反（负圆是镜像投影）",
                        "Back view: left and right are mirrored from the negative chart (it is a mirrored projection)",
                    ))
                        .small()
                        .color(theme::AMBER),
                );
            }
        });
    }

    fn sphere_view(&mut self, ui: &mut Ui, rect: Rect) {
        let lang = self.lang();
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
            lang,
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
        // The legend flows left to right from the measured label widths so it
        // stays correct in either language.
        let legend = ui.painter_at(rect);
        let legend_font = egui::FontId::proportional(11.0);
        let base = rect.left_bottom() + vec2(8.0, -8.0);
        let mut swatch_left = base;
        for (fill, label) in [
            (
                self.palette.positive_fill,
                lang.pick("R > 0 正电阻半球", "R > 0 positive hemisphere"),
            ),
            (
                self.palette.negative_fill,
                lang.pick("R < 0 负电阻半球", "R < 0 negative hemisphere"),
            ),
        ] {
            legend.rect_filled(
                Rect::from_min_size(swatch_left + vec2(0.0, -11.0), vec2(11.0, 11.0)),
                2.0,
                fill,
            );
            let text_rect = legend.text(
                swatch_left + vec2(15.0, 0.0),
                egui::Align2::LEFT_BOTTOM,
                label,
                legend_font.clone(),
                theme::TEXT_MUTED,
            );
            swatch_left = egui::pos2(text_rect.right() + 16.0, base.y);
        }
        // The rotate hint needs room to the right of the legend; on a narrow
        // sphere it would overlap, so it is shown only when the view is wide.
        if rect.width() > 560.0 {
            legend.text(
                rect.right_bottom() + vec2(-8.0, -8.0),
                egui::Align2::RIGHT_BOTTOM,
                lang.pick("拖动旋转，滚轮缩放", "drag to rotate, wheel to zoom"),
                legend_font,
                theme::TEXT_MUTED,
            );
        }
    }

    fn chart_labels(&self, region: Region) -> ChartLabels {
        let lang = self.lang();
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
                title: lang.pick("负电阻区 / R < 0", "Negative region / R < 0").to_owned(),
                subtitle: lang
                    .pick(
                        "镜像压缩投影：半径不是 |Γ|；网格为归一化值",
                        "Mirrored, compressed projection: the radius is not |Γ|; the grid is normalized",
                    )
                    .to_owned(),
                center: match lang {
                    Lang::Chinese => format!("Z = −Z0 = −{z0} Ω（Γ 发散）"),
                    Lang::English => format!("Z = −Z0 = −{z0} Ω (Γ divergent)"),
                },
                note: match current {
                    Some(Region::Positive) => Some(
                        lang.pick("当前点位于正电阻区，见右图", "The point is in the positive region, see the right chart").to_owned(),
                    ),
                    Some(Region::Boundary) => Some(
                        lang.pick("当前点在 R = 0 边界，两图同步显示", "The point is on the R = 0 rim, shown on both charts").to_owned(),
                    ),
                    _ => Some(
                        lang.pick("◇ 边界交点为插值，不是采样点", "◇ the boundary crossing is interpolated, not a sample").to_owned(),
                    ),
                },
                ohm_scale,
            },
            _ => ChartLabels {
                title: lang.pick("正电阻区 / R > 0", "Positive region / R > 0").to_owned(),
                subtitle: lang
                    .pick(
                        "传统史密斯圆图：半径 = |Γ|；网格为归一化值",
                        "Classic Smith chart: the radius = |Γ|; the grid is normalized",
                    )
                    .to_owned(),
                center: match lang {
                    Lang::Chinese => format!("匹配 Z = Z0 = {z0} Ω"),
                    Lang::English => format!("match Z = Z0 = {z0} Ω"),
                },
                note: match current {
                    Some(Region::Negative) => Some(
                        lang.pick("当前点位于负电阻区，见左图", "The point is in the negative region, see the left chart").to_owned(),
                    ),
                    Some(Region::Boundary) => Some(
                        lang.pick("当前点在 R = 0 边界，两图同步显示", "The point is on the R = 0 rim, shown on both charts").to_owned(),
                    ),
                    _ => Some(
                        lang.pick("◇ 边界交点为插值，不是采样点", "◇ the boundary crossing is interpolated, not a sample").to_owned(),
                    ),
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
            self.lang(),
        );
        if interaction.response.hovered() {
            self.frame_hover = interaction.hovered;
        }
        if let Some(clicked) = interaction.clicked {
            self.select(Some(clicked));
        }
    }

    fn show_frequency_slider(&mut self, ui: &mut Ui) {
        let lang = self.lang();
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
        // Wrapped, so a long range such as "5 kHz – 650 MHz, 201 points" cannot
        // widen the side panel and push its content out of view.
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(lang.pick("频率", "Frequency")).strong());
            ui.label(
                RichText::new(label.unwrap_or_else(|| lang.pick("断点", "break").to_owned()))
                    .monospace(),
            );
            ui.small(match lang {
                Lang::Chinese => format!("{first} – {last}，{count} 点"),
                Lang::English => format!("{first} – {last}, {count} points"),
            });
        });
        let response = ui.add(
            egui::Slider::new(&mut index, 0..=count - 1)
                .show_value(false)
                .step_by(1.0)
                .clamping(egui::SliderClamping::Always),
        );
        ui.small(lang.pick(
            "滑块吸附采样点；方向键 ← → 逐点移动。",
            "The slider snaps to samples; the ← → keys step one point.",
        ));
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
        let lang = self.lang();
        ui.label(RichText::new(lang.pick("当前点", "Current point")).strong());
        let Some((reference, preview)) = self.current_point() else {
            ui.small(lang.pick(
                "悬停可预览，点击固定选中；也可以用频率滑块或方向键选择。",
                "Hover to preview, click to fix the selection; the slider and arrow keys also select.",
            ));
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
            lang.pick("预览（悬停）", "Preview (hover)")
        } else {
            lang.pick("已选中（点击固定）", "Selected (click to fix)")
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
        let (magnitude, phase) = format::reflection(gamma, lang);
        let region = match point.region {
            Region::Positive => lang.pick("正电阻区（右图）", "positive region (right)"),
            Region::Negative => lang.pick("负电阻区（左图）", "negative region (left)"),
            Region::Boundary => lang.pick("R = 0 共享边界（两图）", "R = 0 shared rim (both)"),
        };
        egui::Grid::new("details_grid")
            .num_columns(2)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                ui.label(lang.pick("频率", "Frequency"));
                ui.label(
                    point
                        .frequency_hz
                        .map(format::frequency)
                        .unwrap_or_else(|| lang.pick("无频率", "no frequency").to_owned()),
                );
                ui.end_row();
                ui.label("Z");
                ui.label(format::impedance(point.impedance, lang));
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
                ui.label(lang.pick("绘图 Z0", "Plot Z0"));
                ui.label(format!("{} Ω", format::significant(z0, 6)));
                ui.end_row();
                ui.label(lang.pick("区域", "Region"));
                ui.label(region);
                ui.end_row();
                ui.label(lang.pick("轨迹", "Trace"));
                ui.label(trace_description(trace, lang));
                ui.end_row();
                ui.label(lang.pick("来源", "Source"));
                ui.label(document.source.kind_label(lang));
                ui.end_row();
            });
        if (trace.source_z0 - z0).abs() > 1e-9 {
            let source = format::significant(trace.source_z0, 6);
            ui.small(match lang {
                Lang::Chinese => format!(
                    "数据来源的参考阻抗为 {source} Ω；物理阻抗按来源还原后再以绘图 Z0 归一化。"
                ),
                Lang::English => format!(
                    "The source reference is {source} Ω; the physical impedance is recovered from the source, then normalized by the plot Z0."
                ),
            });
        }
        if let Some(gamma) = gamma.finite()
            && gamma.abs() > 1.0
        {
            ui.small(lang.pick(
                "|Γ| > 1：负电阻，反射增益。这不等于电路一定不稳定，需结合外部网络判断。",
                "|Γ| > 1 means negative resistance and reflection gain. That alone does not make the circuit unstable; judge it with the external network.",
            ));
        }
        if matches!(gamma, smith_sphere_core::Reflection::Divergent) {
            ui.small(lang.pick(
                "Z = −Z0：反射系数分母为零，Γ 发散；球面与负圆位置仍然确定。",
                "At Z = −Z0 the reflection denominator is zero and Γ diverges; the sphere and negative-chart positions are still defined.",
            ));
        }
    }

    fn show_documents(&mut self, ui: &mut Ui) {
        let lang = self.lang();
        ui.label(RichText::new(lang.pick("数据", "Data")).strong());
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
                    ui.label(RichText::new(document.display_name(lang)).strong());
                    ui.label(RichText::new(document.source.kind_label(lang)).small().color(theme::AMBER));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button(lang.pick("移除", "Remove")).clicked() {
                            remove = Some(index);
                        }
                    });
                });
                ui.small(document.source.detail(lang));
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
                    ui.small(lang.pick(
                        "S11 是端口 1 的反射（端口 2 接匹配负载），S22 反之；它们对应各端口的等效阻抗，不是 Z 参数矩阵里的 Z11/Z22。",
                        "S11 is port 1 reflection with port 2 matched, and S22 the reverse; they are each port\u{2019}s equivalent impedance, not Z11/Z22 of the Z matrix.",
                    ));
                } else {
                    for (offset, (_, trace)) in document.visible_traces().enumerate() {
                        ui.horizontal(|ui| {
                            swatch(ui, trace_color(color_start.unwrap_or(0) + offset));
                            let count = trace.valid_sample_count();
                            let source = format::significant(trace.source_z0, 6);
                            ui.small(match lang {
                                Lang::Chinese => format!("{}（{count} 点，来源 Z0 {source} Ω）", trace.label),
                                Lang::English => format!("{} ({count} points, source Z0 {source} Ω)", trace.label),
                            });
                        });
                    }
                }
                for note in &document.notes {
                    ui.small(RichText::new(note.text(lang)).color(theme::TEXT_MUTED));
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
        let lang = self.lang();
        ui.label(RichText::new(lang.pick("高级设置", "Advanced")).strong());
        let mut changed = false;
        changed |= ui
            .checkbox(
                &mut self.preferences.detailed_grid,
                lang.pick("详细网格", "Detailed grid"),
            )
            .changed();
        ui.checkbox(
            &mut self.preferences.ohm_labels,
            lang.pick(
                "网格以 Ω 标注（× 绘图 Z0）",
                "Label the grid in Ω (× plot Z0)",
            ),
        );
        ui.checkbox(
            &mut self.preferences.show_landmarks,
            lang.pick("球面显示特征点标签", "Show landmark labels on the sphere"),
        );
        ui.small(lang.pick(
            "导纳网格、Q 圆与任意终接计算不在本版范围内。",
            "Admittance grids, Q circles, and arbitrary terminations are outside this version.",
        ));
        if changed {
            self.rebuild_grids();
        }
    }

    fn show_side_panel(&mut self, ui: &mut Ui) {
        let lang = self.lang();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
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
                ui.hyperlink_to(lang.pick("项目主页", "Project home"), PROJECT_URL);
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
        let lang = self.lang();
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
                ui.hyperlink_to(lang.pick("项目主页", "Project home"), PROJECT_URL);
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

/// The interface language implied by the operating system or browser locale,
/// used only when the user has not chosen one before.
#[cfg(not(target_arch = "wasm32"))]
fn system_language() -> Option<Lang> {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(key)
            && let Some(lang) = Lang::from_code(&value)
        {
            return Some(lang);
        }
    }
    None
}

/// The interface language implied by the browser locale.
#[cfg(target_arch = "wasm32")]
fn system_language() -> Option<Lang> {
    let navigator = web_sys::window()?.navigator();
    if let Some(language) = navigator.language()
        && let Some(lang) = Lang::from_code(&language)
    {
        return Some(lang);
    }
    let languages = navigator.languages();
    for value in languages.iter() {
        if let Some(code) = value.as_string()
            && let Some(lang) = Lang::from_code(&code)
        {
            return Some(lang);
        }
    }
    None
}

fn trace_description(trace: &Trace, lang: Lang) -> String {
    let label = &trace.label;
    match &trace.origin {
        TraceOrigin::TwoPortS(variant) => {
            let name = variant.label();
            let port = variant.port_number();
            match lang {
                Lang::Chinese => format!("{name}：端口 {port} 的反射，另一端口匹配终接"),
                Lang::English => format!("{name}: port {port} reflection, the other port matched"),
            }
        }
        TraceOrigin::OnePortS => lang
            .pick("S11：单端口反射", "S11: one-port reflection")
            .to_owned(),
        TraceOrigin::OnePortZ => lang
            .pick(
                "Z11：单端口归一化阻抗",
                "Z11: one-port normalized impedance",
            )
            .to_owned(),
        TraceOrigin::OnePortY => lang
            .pick(
                "Y11：单端口归一化导纳",
                "Y11: one-port normalized admittance",
            )
            .to_owned(),
        TraceOrigin::ImpedanceColumns => match lang {
            Lang::Chinese => format!("{label}：表格 R、X 列"),
            Lang::English => format!("{label}: table R, X columns"),
        },
        TraceOrigin::ReflectionColumns => match lang {
            Lang::Chinese => format!("{label}：表格反射系数列"),
            Lang::English => format!("{label}: table reflection columns"),
        },
        TraceOrigin::ManualImpedance | TraceOrigin::ManualReflection => match lang {
            Lang::Chinese => format!("{label}：手动输入"),
            Lang::English => format!("{label}: manual entry"),
        },
        TraceOrigin::Demo => match lang {
            Lang::Chinese => format!("{label}：演示数据"),
            Lang::English => format!("{label}: demo data"),
        },
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
