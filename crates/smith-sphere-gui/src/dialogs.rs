//! Input dialogs: manual impedance or reflection entry, pasted text, table
//! import questions, bundled examples, and the about page.

use crate::io::LoadedFile;
use crate::theme;
use egui::{Color32, ComboBox, Context, Id, Modal, RichText, TextEdit, Ui, vec2};
use smith_sphere_core::demo::Example;
use smith_sphere_core::expression::parse_complex;
use smith_sphere_core::parse::csv::{CsvLayout, build_csv_document};
use smith_sphere_core::{
    Complex, Document, FrequencyUnit, Impedance, Lang, Sample, Trace, TraceOrigin,
};

/// What a dialog asks the application to do after a frame.
pub enum DialogOutcome {
    Keep,
    Close,
    /// Add a manually entered trace to the manual-entry document.
    AddManualTrace(Trace),
    /// Parse pasted or imported text as if it were a file.
    LoadText(LoadedFile),
    /// Add a complete document.
    LoadDocument(Document),
}

/// The currently open dialog.
pub enum Dialog {
    Impedance(ImpedanceForm),
    Paste(PasteForm),
    Csv(CsvForm),
    Examples,
    About,
}

impl Dialog {
    pub fn show(&mut self, context: &Context, plot_z0: f64, lang: Lang) -> DialogOutcome {
        let (title, id) = match self {
            Self::Impedance(_) => (lang.pick("输入阻抗", "Enter Z"), "dialog_impedance"),
            Self::Paste(_) => (lang.pick("粘贴数据", "Paste data"), "dialog_paste"),
            Self::Csv(_) => (lang.pick("导入表格", "Import table"), "dialog_csv"),
            Self::Examples => (lang.pick("试用示例", "Examples"), "dialog_examples"),
            Self::About => (
                lang.pick("关于 SmithSphere", "About SmithSphere"),
                "dialog_about",
            ),
        };
        let width = (context.content_rect().width() - 68.0).clamp(220.0, 560.0);
        let mut outcome = DialogOutcome::Keep;
        let modal = Modal::new(Id::new(id))
            .frame(theme::section_frame(16))
            .backdrop_color(Color32::from_rgba_unmultiplied(224, 238, 250, 180))
            .show(context, |ui| {
                ui.set_width(width);
                ui.set_max_height((context.content_rect().height() - 68.0).max(180.0));
                ui.horizontal(|ui| {
                    ui.heading(title);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(lang.pick("关闭", "Close")).clicked() {
                            outcome = DialogOutcome::Close;
                        }
                    });
                });
                theme::separator(ui);
                let inner = egui::ScrollArea::vertical()
                    .max_height((context.content_rect().height() - 140.0).max(160.0))
                    .show(ui, |ui| match self {
                        Self::Impedance(form) => form.show(ui, plot_z0, lang),
                        Self::Paste(form) => form.show(ui, lang),
                        Self::Csv(form) => form.show(ui, plot_z0, lang),
                        Self::Examples => show_examples(ui, lang),
                        Self::About => show_about(ui, lang),
                    })
                    .inner;
                if !matches!(inner, DialogOutcome::Keep) {
                    outcome = inner;
                }
            });
        let painter = context.layer_painter(modal.response.layer_id);
        theme::dashed_border(&painter, modal.response.rect);
        if modal.should_close() && matches!(outcome, DialogOutcome::Keep) {
            outcome = DialogOutcome::Close;
        }
        outcome
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EntryMode {
    Impedance,
    Reflection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReflectionForm {
    RealImaginary,
    MagnitudePhase,
}

/// Manual entry of one impedance or reflection coefficient.
pub struct ImpedanceForm {
    mode: EntryMode,
    expression: String,
    resistance: String,
    reactance: String,
    reflection_form: ReflectionForm,
    reflection_a: String,
    reflection_b: String,
    reference: String,
    frequency: String,
    frequency_unit: FrequencyUnit,
    label: String,
    error: Option<String>,
}

impl ImpedanceForm {
    #[must_use]
    pub fn new(plot_z0: f64) -> Self {
        Self {
            mode: EntryMode::Impedance,
            expression: "25+j30".to_owned(),
            resistance: "25".to_owned(),
            reactance: "30".to_owned(),
            reflection_form: ReflectionForm::MagnitudePhase,
            reflection_a: "0.5".to_owned(),
            reflection_b: "45".to_owned(),
            reference: smith_sphere_core::format::significant(plot_z0, 6),
            frequency: String::new(),
            frequency_unit: FrequencyUnit::MHz,
            label: String::new(),
            error: None,
        }
    }

    fn show(&mut self, ui: &mut Ui, plot_z0: f64, lang: Lang) -> DialogOutcome {
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(
                &mut self.mode,
                EntryMode::Impedance,
                lang.pick("阻抗 Z = R + jX", "Impedance Z = R + jX"),
            );
            ui.selectable_value(
                &mut self.mode,
                EntryMode::Reflection,
                lang.pick("高级：反射系数 Γ", "Advanced: reflection Γ"),
            );
        });
        ui.add_space(4.0);
        let mut submit = false;
        match self.mode {
            EntryMode::Impedance => {
                ui.label(lang.pick(
                    "复数表达式（单位 Ω），例如 25+j30、-20-j5、j50：",
                    "Complex expression in Ω, such as 25+j30, -20-j5, j50:",
                ));
                let response =
                    ui.add(TextEdit::singleline(&mut self.expression).desired_width(f32::INFINITY));
                if response.changed()
                    && let Ok(z) = parse_complex(&self.expression, lang)
                {
                    self.resistance = smith_sphere_core::format::significant(z.re, 8);
                    self.reactance = smith_sphere_core::format::significant(z.im, 8);
                }
                submit |=
                    response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                ui.horizontal_wrapped(|ui| {
                    ui.label("R (Ω)");
                    let r = ui.add(TextEdit::singleline(&mut self.resistance).desired_width(100.0));
                    ui.label("X (Ω)");
                    // labels R and X are notation, the same in both languages
                    let x = ui.add(TextEdit::singleline(&mut self.reactance).desired_width(100.0));
                    if r.changed() || x.changed() {
                        let sign = if self.reactance.trim().starts_with('-') {
                            ""
                        } else {
                            "+"
                        };
                        self.expression =
                            format!("{}{sign}j{}", self.resistance.trim(), self.reactance.trim());
                    }
                });
                ui.small(lang.pick(
                    "R < 0 表示负电阻，会显示在负电阻区圆图。",
                    "R < 0 is negative resistance and appears on the negative chart.",
                ));
            }
            EntryMode::Reflection => {
                ui.label(lang.pick(
                    "反射系数 Γ = (Z − Z0)/(Z + Z0)，需要给出参考阻抗 Z0。",
                    "Reflection Γ = (Z − Z0)/(Z + Z0) needs a reference impedance Z0.",
                ));
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(
                        &mut self.reflection_form,
                        ReflectionForm::MagnitudePhase,
                        lang.pick("幅度 ∠ 相位 (°)", "magnitude ∠ phase (°)"),
                    );
                    ui.selectable_value(
                        &mut self.reflection_form,
                        ReflectionForm::RealImaginary,
                        lang.pick("实部 + j 虚部", "real + j imag"),
                    );
                });
                ui.horizontal_wrapped(|ui| {
                    let (a, b) = match self.reflection_form {
                        ReflectionForm::MagnitudePhase => ("|Γ|", "∠Γ (°)"),
                        ReflectionForm::RealImaginary => ("Re Γ", "Im Γ"),
                    };
                    ui.label(a);
                    ui.add(TextEdit::singleline(&mut self.reflection_a).desired_width(90.0));
                    ui.label(b);
                    ui.add(TextEdit::singleline(&mut self.reflection_b).desired_width(90.0));
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Z0 (Ω)");
                    ui.add(TextEdit::singleline(&mut self.reference).desired_width(70.0));
                });
                ui.small(lang.pick(
                    "|Γ| > 1 对应负电阻；Γ = 1 对应开路。",
                    "|Γ| > 1 is negative resistance; Γ = 1 is open circuit.",
                ));
            }
        }
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(lang.pick("频率（可选）", "Frequency (optional)"));
            ui.add(
                TextEdit::singleline(&mut self.frequency)
                    .desired_width(90.0)
                    .hint_text(lang.pick("例如 915", "e.g. 915")),
            );
            unit_combo(ui, "manual_unit", &mut self.frequency_unit);
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(lang.pick("名称", "Name"));
            ui.add(
                TextEdit::singleline(&mut self.label)
                    .desired_width(110.0)
                    .hint_text(lang.pick("可选", "optional")),
            );
        });
        if let Some(error) = &self.error {
            ui.colored_label(theme::CORAL, error);
        }
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            submit |= ui.button(lang.pick("添加到图中", "Add to plot")).clicked();
            let z0 = smith_sphere_core::format::significant(plot_z0, 6);
            ui.small(match lang {
                Lang::Chinese => format!("当前绘图 Z0 = {z0} Ω"),
                Lang::English => format!("current plot Z0 = {z0} Ω"),
            });
        });
        if submit {
            match self.build_trace(lang) {
                Ok(trace) => {
                    self.error = None;
                    return DialogOutcome::AddManualTrace(trace);
                }
                Err(error) => self.error = Some(error),
            }
        }
        DialogOutcome::Keep
    }

    fn build_trace(&self, lang: Lang) -> Result<Trace, String> {
        let frequency = if self.frequency.trim().is_empty() {
            None
        } else {
            let value = self
                .frequency
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite() && *value >= 0.0)
                .ok_or_else(|| {
                    lang.pick(
                        "频率必须是非负数字。",
                        "The frequency must be a non-negative number.",
                    )
                    .to_owned()
                })?;
            Some(value * self.frequency_unit.multiplier())
        };
        let (impedance, reference, origin, default_label) = match self.mode {
            EntryMode::Impedance => {
                let z = parse_complex(&self.expression, lang)?;
                (
                    Impedance::Finite(z),
                    50.0,
                    TraceOrigin::ManualImpedance,
                    smith_sphere_core::format::complex_ohms(z),
                )
            }
            EntryMode::Reflection => {
                let reference = self
                    .reference
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite() && *value > 0.0)
                    .ok_or_else(|| {
                        lang.pick("Z0 必须是正数。", "Z0 must be a positive number.")
                            .to_owned()
                    })?;
                let a = self
                    .reflection_a
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .ok_or_else(|| {
                        lang.pick(
                            "无法读取 Γ 的第一个分量。",
                            "Could not read the first Γ component.",
                        )
                        .to_owned()
                    })?;
                let b = self
                    .reflection_b
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .ok_or_else(|| {
                        lang.pick(
                            "无法读取 Γ 的第二个分量。",
                            "Could not read the second Γ component.",
                        )
                        .to_owned()
                    })?;
                let gamma = match self.reflection_form {
                    ReflectionForm::MagnitudePhase => {
                        if a < 0.0 {
                            return Err(lang
                                .pick("|Γ| 不能为负。", "|Γ| cannot be negative.")
                                .to_owned());
                        }
                        Complex::from_polar_deg(a, b)
                    }
                    ReflectionForm::RealImaginary => Complex::new(a, b),
                };
                let impedance = Impedance::from_reflection(gamma, reference);
                (
                    impedance,
                    reference,
                    TraceOrigin::ManualReflection,
                    format!("Γ = {}", smith_sphere_core::format::normalized(Some(gamma))),
                )
            }
        };
        let label = if self.label.trim().is_empty() {
            default_label
        } else {
            self.label.trim().to_owned()
        };
        let sample = Sample {
            frequency_hz: frequency,
            impedance: Some(impedance),
        };
        Ok(Trace::new(label, vec![sample], reference, origin))
    }
}

/// Which parser to apply to pasted text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteFormat {
    Auto,
    TouchstoneOnePort,
    TouchstoneTwoPort,
    Csv,
}

impl PasteFormat {
    fn label(self, lang: Lang) -> &'static str {
        match self {
            Self::Auto => lang.pick("自动识别", "Auto detect"),
            Self::TouchstoneOnePort => {
                lang.pick("Touchstone 1 端口 (.s1p)", "Touchstone one-port (.s1p)")
            }
            Self::TouchstoneTwoPort => {
                lang.pick("Touchstone 2 端口 (.s2p)", "Touchstone two-port (.s2p)")
            }
            Self::Csv => lang.pick("CSV / 表格", "CSV / table"),
        }
    }

    fn file_name(self, base: &str) -> String {
        match self {
            Self::Auto => base.to_owned(),
            Self::TouchstoneOnePort => format!("{base}.s1p"),
            Self::TouchstoneTwoPort => format!("{base}.s2p"),
            Self::Csv => format!("{base}.csv"),
        }
    }
}

/// Pasted Touchstone or table text.
pub struct PasteForm {
    text: String,
    format: PasteFormat,
    name: String,
}

impl Default for PasteForm {
    fn default() -> Self {
        Self {
            text: String::new(),
            format: PasteFormat::Auto,
            name: "pasted".to_owned(),
        }
    }
}

impl PasteForm {
    fn show(&mut self, ui: &mut Ui, lang: Lang) -> DialogOutcome {
        ui.label(lang.pick(
            "粘贴 Touchstone 1.x（.s1p/.s2p）内容，或带表头的表格（frequency、R、X）。",
            "Paste Touchstone 1.x (.s1p/.s2p), or a table with a header (frequency, R, X).",
        ));
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                ui.add(
                    TextEdit::multiline(&mut self.text)
                        .desired_width(f32::INFINITY)
                        .desired_rows(10)
                        .font(egui::TextStyle::Monospace)
                        .hint_text("# MHz S MA R 50\n100 0.5 45\n200 0.4 -30"),
                );
            });
        ui.horizontal_wrapped(|ui| {
            ui.label(lang.pick("格式", "Format"));
            ComboBox::from_id_salt("paste_format")
                .selected_text(self.format.label(lang))
                .show_ui(ui, |ui| {
                    for format in [
                        PasteFormat::Auto,
                        PasteFormat::TouchstoneOnePort,
                        PasteFormat::TouchstoneTwoPort,
                        PasteFormat::Csv,
                    ] {
                        ui.selectable_value(&mut self.format, format, format.label(lang));
                    }
                });
            ui.label(lang.pick("名称", "Name"));
            ui.add(TextEdit::singleline(&mut self.name).desired_width(140.0));
        });
        ui.add_space(6.0);
        if ui
            .add_enabled(
                !self.text.trim().is_empty(),
                egui::Button::new(lang.pick("导入", "Import")),
            )
            .clicked()
        {
            let base = if self.name.trim().is_empty() {
                "pasted"
            } else {
                self.name.trim()
            };
            return DialogOutcome::LoadText(LoadedFile {
                name: self.format.file_name(base),
                text: self.text.clone(),
            });
        }
        DialogOutcome::Keep
    }
}

/// Questions a table import still needs answered.
pub struct CsvForm {
    layout: CsvLayout,
    unit: Option<FrequencyUnit>,
    reference: String,
    error: Option<String>,
}

impl CsvForm {
    #[must_use]
    pub fn new(layout: CsvLayout, plot_z0: f64) -> Self {
        Self {
            layout,
            unit: None,
            reference: smith_sphere_core::format::significant(plot_z0, 6),
            error: None,
        }
    }

    fn show(&mut self, ui: &mut Ui, plot_z0: f64, lang: Lang) -> DialogOutcome {
        ui.label(RichText::new(&self.layout.file_name).strong());
        ui.label(self.layout.summary(lang));
        if let Some(header) = &self.layout.header {
            let joined = header.join(" | ");
            ui.small(match lang {
                Lang::Chinese => format!("表头：{joined}"),
                Lang::English => format!("Header: {joined}"),
            });
        }
        ui.add_space(4.0);
        if self.layout.needs_unit() {
            ui.horizontal_wrapped(|ui| {
                ui.label(lang.pick(
                    "频率单位（文件未声明，请选择）",
                    "Frequency unit (not stated, choose one)",
                ));
                let mut choice = self.unit;
                ComboBox::from_id_salt("csv_unit")
                    .selected_text(
                        choice
                            .map(FrequencyUnit::label)
                            .unwrap_or(lang.pick("请选择", "choose")),
                    )
                    .show_ui(ui, |ui| {
                        for unit in FrequencyUnit::ALL {
                            ui.selectable_value(&mut choice, Some(unit), unit.label());
                        }
                    });
                self.unit = choice;
            });
        }
        if self.layout.needs_reference() {
            ui.horizontal_wrapped(|ui| {
                ui.label(lang.pick(
                    "参考阻抗 Z0 (Ω)，用于把反射系数还原为阻抗",
                    "Reference Z0 (Ω), used to recover impedance from the reflection",
                ));
                ui.add(TextEdit::singleline(&mut self.reference).desired_width(80.0));
            });
        } else if self.layout.values.is_reflection() {
            ui.small(lang.pick(
                "表格声明了 Z0 列，将按该值还原阻抗。",
                "The table declares a Z0 column; impedance is recovered with it.",
            ));
        } else {
            let z0 = smith_sphere_core::format::significant(plot_z0, 6);
            ui.small(match lang {
                Lang::Chinese => {
                    format!("阻抗列直接给出物理阻抗，Γ 按当前绘图 Z0 = {z0} Ω 计算。")
                }
                Lang::English => {
                    format!("Impedance columns give the physical impedance directly; Γ uses the current plot Z0 = {z0} Ω.")
                }
            });
        }
        if let Some(error) = &self.error {
            ui.colored_label(theme::CORAL, error);
        }
        ui.add_space(6.0);
        let ready = (!self.layout.needs_unit() || self.unit.is_some())
            && (!self.layout.needs_reference() || !self.reference.trim().is_empty());
        if ui
            .add_enabled(ready, egui::Button::new(lang.pick("导入", "Import")))
            .clicked()
        {
            let reference = if self.layout.needs_reference() {
                match self.reference.trim().parse::<f64>() {
                    Ok(value) if value > 0.0 && value.is_finite() => Some(value),
                    _ => {
                        self.error = Some(
                            lang.pick("Z0 必须是正数。", "Z0 must be a positive number.")
                                .to_owned(),
                        );
                        return DialogOutcome::Keep;
                    }
                }
            } else {
                Some(plot_z0)
            };
            match build_csv_document(&self.layout, self.unit, reference, lang) {
                Ok(document) => return DialogOutcome::LoadDocument(document),
                Err(error) => self.error = Some(error.to_string()),
            }
        }
        DialogOutcome::Keep
    }
}

fn show_examples(ui: &mut Ui, lang: Lang) -> DialogOutcome {
    ui.label(lang.pick(
        "所有示例均为演示数据，由解析公式生成，不是实测结果。",
        "All examples are demo data generated by formulas, not measurements.",
    ));
    ui.add_space(4.0);
    let mut outcome = DialogOutcome::Keep;
    for example in Example::ALL {
        theme::section(ui, 10, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(lang.pick("载入", "Load")).clicked() {
                        outcome = DialogOutcome::LoadDocument(example.build());
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.label(RichText::new(example.title(lang)).strong());
                    });
                });
            });
            ui.small(example.summary(lang));
        });
    }
    outcome
}

fn show_about(ui: &mut Ui, lang: Lang) -> DialogOutcome {
    egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
        ui.label(lang.pick(
            "SmithSphere 史密斯球把完整的复阻抗平面映射到球面：正电阻半球对应传统史密斯圆图，负电阻半球对应负电阻区镜像圆图。",
            "SmithSphere maps the whole complex impedance plane onto a sphere: the positive hemisphere is the classic Smith chart, and the negative hemisphere is the mirrored negative chart.",
        ));
        theme::separator(ui);
        ui.label(RichText::new(lang.pick("术语", "Terms")).strong());
        ui.label(lang.pick(
            "Z = R + jX：物理阻抗，单位 Ω。z = Z/Z0：归一化阻抗。",
            "Z = R + jX is the physical impedance in Ω. z = Z/Z0 is the normalized impedance.",
        ));
        ui.label(lang.pick(
            "Γ = (Z − Z0)/(Z + Z0)：反射系数。Z = −Z0 时分母为零，Γ 发散；Γ = 0 时相位无定义。",
            "Γ = (Z − Z0)/(Z + Z0) is the reflection coefficient. At Z = −Z0 the denominator is zero and Γ diverges; at Γ = 0 the phase is undefined.",
        ));
        ui.label(lang.pick(
            "负电阻区圆图是负半平面的压缩镜像投影，其半径不是 |Γ|；圆心对应 Z = −Z0，不是匹配点。",
            "The negative chart is a compressed, mirrored projection of the negative half-plane. Its radius is not |Γ|; its centre is Z = −Z0, not a match point.",
        ));
        theme::separator(ui);
        ui.label(RichText::new(lang.pick("球面坐标", "Sphere coordinates")).strong());
        ui.monospace("d = r² + x² + 1\nu = (r² + x² − 1)/d\nv = 2x/d\nw = 2r/d");
        ui.label(lang.pick(
            "w > 0 为正电阻半球，w < 0 为负电阻半球；v > 0 感性，v < 0 容性。",
            "w > 0 is the positive hemisphere, w < 0 the negative one; v > 0 is inductive, v < 0 capacitive.",
        ));
        ui.monospace("p+ = (z − 1)/(z + 1) = (u + jv)/(1 + w)\np− = (conj z + 1)/(conj z − 1) = (u + jv)/(1 − w)");
        theme::separator(ui);
        ui.label(RichText::new(lang.pick("支持的输入", "Supported input")).strong());
        ui.label(lang.pick(
            "手动 R、X 或复数表达式；反射系数 Γ；CSV（frequency、R、X）；Touchstone 1.x 的 .s1p 与 .s2p（RI/MA/DB）。",
            "Manual R, X or a complex expression; reflection Γ; CSV (frequency, R, X); Touchstone 1.x .s1p and .s2p (RI/MA/DB).",
        ));
        ui.label(lang.pick(
            "暂不支持：Touchstone 2.0、混合模、三端口以上、任意负载终接计算、导纳网格与 Q 圆。",
            "Not supported: Touchstone 2.0, mixed mode, more than two ports, arbitrary terminations, admittance grids, and Q circles.",
        ));
        theme::separator(ui);
        ui.label(RichText::new(lang.pick("致谢", "Credits")).strong());
        ui.label(lang.pick(
            "界面字体 SmithSphere Sans 是 Adobe Source Han Sans CN 的改名子集，遵循 SIL Open Font License 1.1。使用 egui/eframe 构建。",
            "The interface font SmithSphere Sans is a renamed subset of Adobe Source Han Sans CN under the SIL Open Font License 1.1. Built with egui and eframe.",
        ));
        ui.label(lang.pick(
            "圆图来自 P. H. Smith（1939、1944），负电阻圆图来自 Lenzing 与 D'Elio（1963），球面表示来自 Zelley（IEEE Microwave Magazine，2007）和 Muller 等（IEEE MWCL，2011）。",
            "The chart is P. H. Smith (1939, 1944), the negative chart Lenzing and D'Elio (1963), and the sphere Zelley (IEEE Microwave Magazine, 2007) and Muller et al. (IEEE MWCL, 2011).",
        ));
        ui.hyperlink_to(
            lang.pick("Touchstone 规范", "Touchstone specification"),
            "https://ibis.org/touchstone_ver2.0/touchstone_ver2_0.pdf",
        );
    });
    DialogOutcome::Keep
}

/// Frequency unit selector.
pub fn unit_combo(ui: &mut Ui, id: &str, unit: &mut FrequencyUnit) {
    ComboBox::from_id_salt(id)
        .selected_text(unit.label())
        .width(70.0)
        .show_ui(ui, |ui| {
            for candidate in FrequencyUnit::ALL {
                ui.selectable_value(unit, candidate, candidate.label());
            }
        });
}

/// Small colored square used as a trace legend swatch.
pub fn swatch(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(12.0, 12.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, color);
}
