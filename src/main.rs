#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod altium;
mod batch;
mod bom;
mod cli;
mod config;
mod easyeda;
mod error;
mod exporter;
mod extract;
mod kicad;
mod lceda;
mod lcsc;
mod metadata;
mod monitor;
mod util;

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use arboard::Clipboard;
use eframe::egui::{
    self, Color32, ComboBox, Context, FontData, FontDefinitions, FontFamily, FontId, Frame, Layout,
    Margin, RichText, ScrollArea, Stroke, TextEdit, TextStyle, Theme, Ui, Vec2, ViewportBuilder,
};
use monitor::{MonitorHandle, MonitorState};

use crate::config::{AppConfig, ExportConfig, MonitorConfig};

const PATTERN_QUICK: &str = "regex:(?m)^(C\\d{3,})$";
const PATTERN_FULL: &str = "regex:编号[：:]\\s*(C\\d+)";
const RELEASES_API_URL: &str = "https://api.github.com/repos/ref42/seex/releases/latest";
const FONT_SCALE_FROM_CURRENT: f32 = 0.75;
static DARK_MODE: AtomicBool = AtomicBool::new(false);

fn main() -> eframe::Result {
    set_windows_app_id();

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size(Vec2::new(1080.0, 900.0))
            .with_min_inner_size(Vec2::new(1080.0, 720.0))
            .with_icon(load_app_icon())
            .with_always_on_top(),
        ..Default::default()
    };

    eframe::run_native(
        app_title(),
        options,
        Box::new(|cc| Ok(Box::new(SeexApp::new(cc)))),
    )
}

fn app_title() -> &'static str {
    concat!("SeEx ", env!("CARGO_PKG_VERSION"))
}

#[cfg(windows)]
fn set_windows_app_id() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let app_id: Vec<u16> = OsStr::new("Ref42.SeEx")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let _ =
            windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(app_id.as_ptr());
    }
}

#[cfg(not(windows))]
fn set_windows_app_id() {}

fn load_app_icon() -> egui::IconData {
    let mut image = image::load_from_memory(include_bytes!(concat!(env!("OUT_DIR"), "/seex.png")))
        .expect("embedded SeEx icon must decode")
        .into_rgba8();
    let max_edge = 256;
    let source_edge = image.width().max(image.height());
    if source_edge > max_edge {
        let scale = max_edge as f32 / source_edge as f32;
        let width = ((image.width() as f32 * scale).round() as u32).max(1);
        let height = ((image.height() as f32 * scale).round() as u32).max(1);
        image = image::imageops::resize(&image, width, height, image::imageops::Lanczos3);
    }
    let (width, height) = image.dimensions();
    egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    }
}

fn load_image_texture(ctx: &Context, name: &'static str, bytes: &[u8]) -> egui::TextureHandle {
    let image = image::load_from_memory(bytes)
        .expect("embedded image must decode")
        .into_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let pixels = image.into_raw();
    ctx.load_texture(
        name,
        egui::ColorImage::from_rgba_unmultiplied(size, &pixels),
        egui::TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
    )
}

fn fit_image_size(size: Vec2, max_size: Vec2) -> Vec2 {
    let scale = (max_size.x / size.x).min(max_size.y / size.y).min(1.0);
    size * scale
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Monitor,
    History,
    Export,
    More,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lang {
    En,
    Zh,
}

#[derive(Clone, Default)]
struct UpdateState {
    checking: bool,
    checked: bool,
    latest_version: Option<String>,
    release_url: Option<String>,
    error: Option<String>,
}

#[derive(serde::Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
}

struct SeexApp {
    state: Arc<Mutex<MonitorState>>,
    _monitor: MonitorHandle,
    donate_pay: egui::TextureHandle,
    donate_wechat: egui::TextureHandle,
    page: Page,
    lang: Lang,
    pin_on_top: bool,
    show_matched: bool,
    show_history: bool,
    match_quick: bool,
    match_full: bool,
    clear_confirm: bool,
    last_theme_check: Instant,
    update_state: Arc<Mutex<UpdateState>>,
    show_donate: bool,
    show_failed_ids: bool,
    schematic_stroke_color: [u8; 3],
    schematic_fill_color: [u8; 3],
    schematic_stroke_hex: String,
    schematic_fill_hex: String,
    active_color_picker: Option<u8>,
    manual_import_text: String,
    manual_import_status: Option<String>,
}

impl SeexApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let config = AppConfig::load();
        install_fonts(&cc.egui_ctx);
        DARK_MODE.store(detect_dark_mode(), Ordering::Relaxed);
        install_style(&cc.egui_ctx);

        let mut initial = MonitorState::new();
        apply_config_to_state(&config, &mut initial);
        initial.set_keyword(build_keyword(true, true));

        let state = Arc::new(Mutex::new(initial));
        let monitor = MonitorHandle::spawn(Arc::clone(&state));
        let donate_pay = load_image_texture(
            &cc.egui_ctx,
            "donate-pay",
            include_bytes!("../assets/donate/pay1.jpg"),
        );
        let donate_wechat = load_image_texture(
            &cc.egui_ctx,
            "donate-wechat",
            include_bytes!("../assets/donate/pay2.jpg"),
        );

        Self {
            state,
            _monitor: monitor,
            donate_pay,
            donate_wechat,
            page: Page::Monitor,
            lang: if config.ui_language.eq_ignore_ascii_case("chinese") {
                Lang::Zh
            } else {
                Lang::En
            },
            pin_on_top: true,
            show_matched: true,
            show_history: true,
            match_quick: true,
            match_full: true,
            clear_confirm: false,
            last_theme_check: Instant::now(),
            update_state: Arc::new(Mutex::new(UpdateState::default())),
            show_donate: config.show_donate,
            show_failed_ids: false,
            schematic_stroke_color: config.export.schematic_stroke_color,
            schematic_fill_color: config.export.schematic_fill_color,
            schematic_stroke_hex: color_to_hex(config.export.schematic_stroke_color),
            schematic_fill_hex: color_to_hex(config.export.schematic_fill_color),
            active_color_picker: None,
            manual_import_text: String::new(),
            manual_import_status: None,
        }
    }

    fn t(&self, en: &'static str, zh: &'static str) -> &'static str {
        match self.lang {
            Lang::En => en,
            Lang::Zh => zh,
        }
    }

    fn render_top_bar(&mut self, ui: &mut Ui) {
        const ROW_HEIGHT: f32 = 42.0;
        const RIGHT_CONTROLS_WIDTH: f32 = 380.0;

        ui.set_height(ROW_HEIGHT);
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
        ui.horizontal(|ui| {
            ui.set_height(ROW_HEIGHT);
            self.nav_item(ui, Page::Monitor, self.t("Monitor", "监听"));
            self.nav_item(ui, Page::History, self.t("History", "历史"));
            self.nav_item(ui, Page::Export, self.t("Export", "导出"));
            self.nav_item(ui, Page::More, self.t("More", "更多"));

            ui.add_space((ui.available_width() - RIGHT_CONTROLS_WIDTH).max(12.0));
            self.language_toggle(ui);
            self.pin_toggle(ui);
            self.monitoring_label(ui);
        });
    }

    fn nav_item(&mut self, ui: &mut Ui, page: Page, label: &'static str) {
        let selected = self.page == page;
        if top_bar_button(ui, 116.0, label, selected).clicked() {
            self.page = page;
        }
    }

    fn monitor_page(&mut self, ui: &mut Ui) {
        // The manual-import content naturally settles at roughly this height;
        // use it as the shared baseline for the neighboring export panel.
        const MONITOR_PANEL_HEIGHT: f32 = 267.0;
        let mut save_config = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(self.t("Match mode:", "匹配模式:"));
            if ui
                .selectable_label(self.match_quick, self.t("Quick ID", "快速 ID"))
                .clicked()
            {
                self.match_quick = !self.match_quick;
                self.set_keyword_from_modes();
            }
            if ui
                .selectable_label(self.match_full, self.t("Full Info", "完整信息"))
                .clicked()
            {
                self.match_full = !self.match_full;
                self.set_keyword_from_modes();
            }
            if let Ok(mut state) = self.state.lock() {
                let monitoring_label = if state.monitoring {
                    self.t("Monitoring", "监听中")
                } else {
                    self.t("Paused", "已暂停")
                };
                if ui
                    .add(egui::Button::new(monitoring_label).selected(state.monitoring))
                    .clicked()
                {
                    state.monitoring = !state.monitoring;
                    if state.monitoring {
                        state.last_content.clear();
                        state.initialized = false;
                    }
                }
            }
        });

        ui.add_space(10.0);
        ui.columns(2, |columns| {
            self.manual_import_panel(&mut columns[0], MONITOR_PANEL_HEIGHT);

            if let Ok(mut state) = self.state.lock() {
                section_with_min_height(&mut columns[1], MONITOR_PANEL_HEIGHT, |ui| {
                    ui.horizontal(|ui| {
                        section_header(
                            ui,
                            self.t("Export files", "导出文件"),
                            self.t(
                                "BOM CSV for component keeping.",
                                "用于元件管理的 BOM CSV 文件。",
                            ),
                        );
                        ui.with_layout(Layout::right_to_left(egui::Align::TOP), |ui| {
                            if ui.button(self.t("Browse", "浏览")).clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("CSV", &["csv"])
                                    .set_file_name("matched.csv")
                                    .save_file()
                                {
                                    state
                                        .set_matched_save_path(path.to_string_lossy().into_owned());
                                    save_config = true;
                                }
                            }
                        });
                    });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        fixed_label(ui, self.t("BOM CSV file:", "BOM CSV 文件:"));
                        let input_width = ui.available_width().max(80.0);
                        if ui
                            .add_sized(
                                Vec2::new(input_width, 30.0),
                                TextEdit::singleline(&mut state.matched_save_path),
                            )
                            .changed()
                        {
                            save_config = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        fixed_label(ui, self.t("Metadata language:", "元数据语言:"));
                        if ui
                            .selectable_label(
                                state.bom_language != "english",
                                self.t("Chinese", "中文"),
                            )
                            .clicked()
                        {
                            state.set_bom_language("chinese".to_string());
                            save_config = true;
                        }
                        if ui
                            .selectable_label(
                                state.bom_language == "english",
                                self.t("English", "英文"),
                            )
                            .clicked()
                        {
                            state.set_bom_language("english".to_string());
                            save_config = true;
                        }
                    });
                    ui.add_space(18.0);
                });
            }
        });
        if save_config {
            self.save_config();
        }

        ui.add_space(10.0);
        self.monitor_actions(ui);
        ui.add_space(10.0);
        self.matched_list(ui);
    }

    fn manual_import_panel(&mut self, ui: &mut Ui, min_height: f32) {
        let paste_label = self.t("Paste Text for Import", "粘贴文本导入");
        let import_label = self.t("Import IDs", "导入 ID");
        let hint = self.t(
            "Paste component IDs, LCSC links, or copied product text here.",
            "可粘贴元件 ID、立创链接或复制的商品信息。",
        );
        let input_hint = self.t(
            "Example: C2040, C2927505, or an LCSC product URL",
            "示例：C2040、C2927505 或立创商品链接",
        );

        section_with_min_height(ui, min_height, |ui| {
            section_header(ui, self.t("Manual import", "手动导入"), hint);
            ui.add_space(8.0);
            ui.add(
                TextEdit::multiline(&mut self.manual_import_text)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    .hint_text(input_hint),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if ui.button(paste_label).clicked() {
                    match Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
                        Ok(text) => {
                            self.manual_import_text = text;
                            self.manual_import_status = Some(
                                self.t(
                                    "Clipboard text loaded. Review it, then click Import IDs.",
                                    "已读取剪贴板文本，请确认后点击“导入 ID”。",
                                )
                                .to_string(),
                            );
                        }
                        Err(_) => {
                            self.manual_import_status = Some(self.t(
                                "Clipboard is busy or unavailable. Paste directly into the box.",
                                "剪贴板被占用或不可用，请直接粘贴到输入框。",
                            ).to_string());
                        }
                    }
                }
                if ui.button(import_label).clicked() {
                    let text = std::mem::take(&mut self.manual_import_text);
                    let result = self
                        .state
                        .lock()
                        .map(|mut state| {
                            let imported = extract::extract_lcsc_ids(&text);
                            let count = imported.len();
                            state.import_text(text);
                            count
                        })
                        .unwrap_or(0);
                    self.manual_import_status = Some(if result == 0 {
                        self.t("No LCSC component IDs found.", "未找到立创元件 ID。")
                            .to_string()
                    } else {
                        format!(
                            "{} {}",
                            result,
                            self.t("unique ID(s) imported.", "个唯一 ID 已导入。")
                        )
                    });
                }
                if let Some(status) = &self.manual_import_status {
                    ui.label(RichText::new(status).color(muted()));
                }
            });
        });
    }

    fn monitor_actions(&mut self, ui: &mut Ui) {
        let (matched_count, bom_running, bom_result) = self
            .state
            .lock()
            .map(|state| {
                (
                    state.matched.len(),
                    state.bom_running,
                    state.bom_last_result.clone(),
                )
            })
            .unwrap_or((0, false, None));

        ui.horizontal_wrapped(|ui| {
            metric_pill(ui, self.t("Matched", "匹配结果"), matched_count, primary());
            if ui.button(self.t("Copy IDs", "复制 ID")).clicked() {
                let ids = self
                    .state
                    .lock()
                    .map(|state| state.get_unique_ids())
                    .unwrap_or_default();
                if !ids.is_empty() {
                    copy_to_clipboard(&ids.join("\n"));
                }
            }
            if ui
                .add_enabled(
                    matched_count > 0 && !bom_running,
                    egui::Button::new(if bom_running {
                        self.t("Exporting...", "导出中...")
                    } else {
                        self.t("Export BOM CSV", "导出 BOM CSV")
                    }),
                )
                .clicked()
            {
                bom::spawn_save_matched(Arc::clone(&self.state));
            }
            if !self.clear_confirm {
                if ui.button(self.t("Clear All", "清空全部")).clicked() {
                    self.clear_confirm = true;
                }
            } else {
                ui.label(RichText::new(self.t("Sure?", "确定吗?")).color(danger()));
                if ui.button(self.t("Yes", "是")).clicked() {
                    if let Ok(mut state) = self.state.lock() {
                        state.clear_all();
                    }
                    self.clear_confirm = false;
                }
                if ui.button(self.t("No", "否")).clicked() {
                    self.clear_confirm = false;
                }
            }
            let show_list = self.t("Show list", "显示列表");
            ui.checkbox(&mut self.show_matched, show_list);
        });

        if let Some(result) = bom_result {
            ui.label(RichText::new(result).color(if bom_running { muted() } else { success() }));
        }
    }

    fn matched_list(&mut self, ui: &mut Ui) {
        let items = self
            .state
            .lock()
            .map(|state| state.matched.clone())
            .unwrap_or_default();

        section(ui, |ui| {
            section_header_inline(ui, self.t("Matched", "匹配结果"), |ui| {
                count_chip(ui, items.len(), primary());
            });
            if !self.show_matched {
                return;
            }
            if items.is_empty() {
                empty_state(ui, self.t("No matches yet", "暂无匹配结果"));
                return;
            }

            ui.add_space(8.0);
            ScrollArea::vertical()
                .max_height(list_height(ui, 0.24, 3, 54.0))
                .min_scrolled_height(list_height(ui, 0.24, 3, 54.0))
                .show(ui, |ui| {
                    let mut delete_index = None;
                    for (idx, item) in items.iter().enumerate() {
                        row_frame(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&item.lcsc_id).strong().color(primary()));
                                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.small_button(self.t("Delete", "删除")).clicked() {
                                        delete_index = Some(idx);
                                    }
                                    if ui.small_button(self.t("Copy", "复制")).clicked() {
                                        copy_to_clipboard(&item.lcsc_id);
                                    }
                                });
                            });
                        });
                    }
                    if let Some(idx) = delete_index {
                        if let Ok(mut state) = self.state.lock() {
                            state.delete_matched(idx);
                        }
                    }
                });
        });
    }

    fn history_page(&mut self, ui: &mut Ui) {
        let history = self
            .state
            .lock()
            .map(|state| state.history.clone())
            .unwrap_or_default();

        section(ui, |ui| {
            section_header_inline(ui, self.t("History", "历史"), |ui| {
                count_chip(ui, history.len(), primary());
            });
            let show_history = self.t("Show history", "显示历史");
            ui.checkbox(&mut self.show_history, show_history);

            if history.is_empty() {
                empty_state(ui, self.t("No history yet", "暂无历史记录"));
                return;
            }

            if !self.show_history {
                return;
            }

            ui.add_space(8.0);
            ScrollArea::vertical()
                .max_height(list_height(ui, 0.55, 6, 52.0))
                .min_scrolled_height(list_height(ui, 0.55, 6, 52.0))
                .show(ui, |ui| {
                    let mut delete_index = None;
                    for (idx, (time, content)) in history.iter().enumerate() {
                        row_frame(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.monospace(RichText::new(time).color(muted()));
                                ui.label(first_line(content, 110));
                                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.small_button(self.t("Delete", "删除")).clicked() {
                                        delete_index = Some(idx);
                                    }
                                    if ui.small_button(self.t("Copy", "复制")).clicked() {
                                        copy_to_clipboard(content);
                                    }
                                });
                            });
                        });
                    }
                    if let Some(idx) = delete_index {
                        if let Ok(mut state) = self.state.lock() {
                            state.delete_history(idx);
                        }
                    }
                });
        });
    }

    fn export_page(&mut self, ui: &mut Ui) {
        let mut request = None;
        let mut save_config = false;
        let mut schematic_stroke_color = self.schematic_stroke_color;
        let mut schematic_fill_color = self.schematic_fill_color;
        let mut schematic_stroke_hex = self.schematic_stroke_hex.clone();
        let mut schematic_fill_hex = self.schematic_fill_hex.clone();
        let mut active_color_picker = self.active_color_picker;
        let mut toggle_failed_ids = false;
        let show_failed_ids = self.show_failed_ids;
        let mut show_failed_ids_popup = self.show_failed_ids;

        if let Ok(mut state) = self.state.lock() {
            let matched_count = state.matched.len();
            section(ui, |ui| {
                section_header_inline(ui, self.t("EDA Library Export", "EDA 库导出"), |ui| {
                    metric_pill(ui, self.t("Ready", "待导出"), matched_count, primary());
                });
                ui.label(
                    RichText::new(self.t(
                        "Altium and KiCad export through export",
                        "通过 export 导出 Altium 和 KiCad 库",
                    ))
                    .color(muted()),
                );
                ui.horizontal(|ui| {
                    if let Some(result) = &state.export_last_result {
                        if render_export_metrics(ui, result, self.lang) {
                            toggle_failed_ids = true;
                        }
                    }
                    if let Some(progress) = &state.export_progress {
                        let value = if progress.total > 0 {
                            progress.current as f32 / progress.total as f32
                        } else {
                            0.0
                        };
                        let width = (ui.available_width() - 160.0).max(120.0);
                        ui.add_sized(
                            [width, 34.0],
                            egui::ProgressBar::new(value).text(&progress.message),
                        );
                    }
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                matched_count > 0 && !state.export_running,
                                egui::Button::new(if state.export_running {
                                    self.t("Running...", "运行中...")
                                } else {
                                    self.t("Export Library", "导出库")
                                }),
                            )
                            .clicked()
                        {
                            request = Some(exporter::ExportRequest {
                                ids: state.get_unique_ids(),
                                target: state.export_target.clone(),
                                output_path: state.export_output_path.clone(),
                                mode: if state.export_target == "kicad" {
                                    state.export_kicad_mode.clone()
                                } else {
                                    state.export_mode.clone()
                                },
                                merge: state.export_merge,
                                append: state.export_append,
                                library_name: state.export_library_name.clone(),
                                parallel: state.export_parallel,
                                continue_on_error: state.export_continue_on_error,
                                lcsc_english: state.export_lcsc_english,
                                force: state.export_force,
                                schematic_stroke_color: self.schematic_stroke_color,
                                schematic_fill_color: self.schematic_fill_color,
                            });
                        }
                    });
                });
                if let Some(result) = &state.export_last_result {
                    let failed = extract_failed_ids(result);
                    if show_failed_ids && !failed.is_empty() {
                        let title = self.t("Failed IDs", "失败 ID");
                        let ids = failed.join(", ");
                        let viewport_center = ui
                            .ctx()
                            .input(|input| input.viewport().inner_rect)
                            .map(|rect| rect.center())
                            .unwrap_or_else(|| ui.max_rect().center());
                        egui::Window::new(title)
                            .collapsible(false)
                            .resizable(true)
                            .default_width(420.0)
                            .default_pos(viewport_center - Vec2::new(210.0, 45.0))
                            .open(&mut show_failed_ids_popup)
                            .show(ui.ctx(), |ui| {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&ids).monospace().color(danger()),
                                    )
                                    .selectable(true),
                                );
                            });
                    }
                }
            });

            ui.add_space(10.0);
            section(ui, |ui| {
                section_header(
                    ui,
                    self.t("Export Settings", "导出设置"),
                    self.t("Defaults are saved automatically.", "设置会自动保存。"),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    fixed_label(ui, self.t("EDA target:", "EDA 目标:"));
                    let mut target = state.export_target.clone();
                    ComboBox::from_id_salt("target")
                        .selected_text(if target == "kicad" { "KiCad" } else { "Altium" })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut target, "altium".to_string(), "Altium");
                            ui.selectable_value(&mut target, "kicad".to_string(), "KiCad");
                        });
                    if target != state.export_target {
                        state.set_export_target(target);
                        save_config = true;
                    }
                });

                ui.horizontal(|ui| {
                    if color_setting(
                        ui,
                        self.t("Stroke color:", "描边颜色:"),
                        &mut schematic_stroke_color,
                        &mut schematic_stroke_hex,
                        0,
                        &mut active_color_picker,
                    ) {
                        save_config = true;
                    }
                    if color_setting(
                        ui,
                        self.t("Fill color:", "填充颜色:"),
                        &mut schematic_fill_color,
                        &mut schematic_fill_hex,
                        1,
                        &mut active_color_picker,
                    ) {
                        save_config = true;
                    }
                });

                ui.horizontal(|ui| {
                    fixed_label(ui, self.t("Export directory:", "导出目录:"));
                    if ui
                        .add(
                            TextEdit::singleline(&mut state.export_output_path)
                                .desired_width(640.0),
                        )
                        .changed()
                    {
                        save_config = true;
                    }
                    if ui.button(self.t("Browse", "浏览")).clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            state.set_export_output_path(path.to_string_lossy().into_owned());
                            save_config = true;
                        }
                    }
                });

                ui.horizontal(|ui| {
                    fixed_label(ui, self.t("Export scope:", "导出范围:"));
                    if state.export_target == "kicad" {
                        mode_button(
                            ui,
                            &mut state.export_kicad_mode,
                            "full",
                            self.t("Full", "完整"),
                        );
                        mode_button(
                            ui,
                            &mut state.export_kicad_mode,
                            "symbol",
                            self.t("Symbol", "符号"),
                        );
                        mode_button(
                            ui,
                            &mut state.export_kicad_mode,
                            "footprint",
                            self.t("Footprint", "封装"),
                        );
                        mode_button(ui, &mut state.export_kicad_mode, "3d", "3D");
                    } else {
                        mode_button(ui, &mut state.export_mode, "full", self.t("Full", "完整"));
                        mode_button(ui, &mut state.export_mode, "schlib", "SchLib");
                        mode_button(ui, &mut state.export_mode, "pcblib", "PcbLib");
                        mode_button(ui, &mut state.export_mode, "3d", "3D");
                    }
                    save_config = true;
                });

                ui.horizontal(|ui| {
                    fixed_label(ui, self.t("Library name:", "库名称:"));
                    if ui
                        .add_enabled(
                            state.export_target == "kicad" || state.export_merge,
                            TextEdit::singleline(&mut state.export_library_name)
                                .desired_width(320.0),
                        )
                        .changed()
                    {
                        save_config = true;
                    }
                    ui.add_space(12.0);
                    ui.label(RichText::new(self.t("Parallel jobs:", "并行任务数:")).color(muted()));
                    let mut parallel = state.export_parallel as u32;
                    if ui
                        .add(egui::DragValue::new(&mut parallel).range(1..=64))
                        .changed()
                    {
                        state.set_export_parallel(parallel as usize);
                        save_config = true;
                    }
                });

                ui.horizontal_wrapped(|ui| {
                    if state.export_target == "altium" && state.export_mode != "3d" {
                        let mut merge = state.export_merge;
                        if ui.checkbox(&mut merge, self.t("Merge", "合并")).changed() {
                            state.set_export_merge(merge);
                            save_config = true;
                        }
                        let mut append = state.export_append;
                        if ui.checkbox(&mut append, self.t("Append", "追加")).changed() {
                            state.set_export_append(append);
                            save_config = true;
                        }
                    }
                    let mut continue_on_error = state.export_continue_on_error;
                    if ui
                        .checkbox(
                            &mut continue_on_error,
                            self.t("Continue On Error", "出错继续"),
                        )
                        .changed()
                    {
                        state.set_export_continue_on_error(continue_on_error);
                        save_config = true;
                    }
                    let mut force = state.export_force;
                    if ui.checkbox(&mut force, self.t("Force", "强制")).changed() {
                        state.set_export_force(force);
                        save_config = true;
                    }
                    let mut english = state.export_lcsc_english;
                    if ui
                        .checkbox(&mut english, self.t("English metadata", "英文元数据"))
                        .changed()
                    {
                        state.set_export_lcsc_english(english);
                        state.bom_language =
                            if english { "english" } else { "chinese" }.to_string();
                        save_config = true;
                    }
                });
            });
        }

        if schematic_stroke_color != self.schematic_stroke_color {
            self.schematic_stroke_color = schematic_stroke_color;
            save_config = true;
        }
        if schematic_fill_color != self.schematic_fill_color {
            self.schematic_fill_color = schematic_fill_color;
            save_config = true;
        }
        self.schematic_stroke_hex = schematic_stroke_hex;
        self.schematic_fill_hex = schematic_fill_hex;
        self.active_color_picker = active_color_picker;
        if toggle_failed_ids {
            show_failed_ids_popup = !show_failed_ids_popup;
        }
        self.show_failed_ids = show_failed_ids_popup;
        if save_config {
            self.save_config();
        }
        if let Some(req) = request {
            exporter::spawn_export(Arc::clone(&self.state), req);
        }
    }

    fn more_page(&mut self, ui: &mut Ui) {
        const MORE_PANEL_HEIGHT: f32 = 190.0;

        ui.columns(2, |columns| {
            section_with_min_height(&mut columns[0], MORE_PANEL_HEIGHT, |ui| {
                section_header(
                    ui,
                    self.t("Preferences", "偏好设置"),
                    self.t("Choose what SeEx displays.", "选择显示内容。"),
                );
                ui.add_space(8.0);

                let mut show_donate = self.show_donate;
                if ui
                    .checkbox(
                        &mut show_donate,
                        self.t("Show donation QR codes", "显示赞助二维码"),
                    )
                    .changed()
                {
                    self.show_donate = show_donate;
                    self.save_config();
                }
            });

            section_with_min_height(&mut columns[1], MORE_PANEL_HEIGHT, |ui| {
                section_header(
                    ui,
                    self.t("Updates", "更新"),
                    self.t("Keep SeEx up to date", "检查 SeEx 是否有新版本"),
                );
                ui.add_space(8.0);

                let update = self
                    .update_state
                    .lock()
                    .map(|state| state.clone())
                    .unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{}: {}",
                        self.t("Current version", "当前版本"),
                        env!("CARGO_PKG_VERSION")
                    ));
                    if update.checking {
                        ui.spinner();
                        ui.label(self.t("Checking...", "检查中..."));
                    } else if ui.button(self.t("Check for updates", "检查更新")).clicked() {
                        self.start_update_check(ui.ctx());
                    }
                });

                if let Some(error) = update.error {
                    ui.label(RichText::new(error).color(Color32::from_rgb(190, 70, 70)));
                } else if let Some(latest) = update.latest_version {
                    if version_is_newer(&latest, env!("CARGO_PKG_VERSION")) {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{}: {}",
                                    self.t("Update available", "发现新版本"),
                                    latest
                                ))
                                .color(Color32::from_rgb(35, 145, 85)),
                            );
                            if let Some(url) = update.release_url {
                                ui.hyperlink_to(self.t("Download update", "下载更新"), url);
                            }
                        });
                    } else if update.checked {
                        ui.label(
                            RichText::new(self.t("You are up to date.", "当前已是最新版本。"))
                                .color(Color32::from_rgb(35, 145, 85)),
                        );
                    }
                }
            });
        });

        if self.show_donate {
            ui.add_space(12.0);
            ui.columns(2, |columns| {
                for (column, texture) in columns
                    .iter_mut()
                    .zip([&self.donate_pay, &self.donate_wechat])
                {
                    column.vertical_centered(|ui| {
                        let image_size =
                            fit_image_size(texture.size_vec2(), Vec2::new(420.0, 620.0));
                        ui.image((texture.id(), image_size));
                    });
                }
            });
        }
    }

    fn start_update_check(&self, ctx: &Context) {
        if let Ok(mut state) = self.update_state.lock() {
            if state.checking {
                return;
            }
            *state = UpdateState {
                checking: true,
                ..UpdateState::default()
            };
        }

        let state = Arc::clone(&self.update_state);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = (|| -> Result<GitHubRelease, String> {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| error.to_string())?;
                runtime.block_on(async {
                    reqwest::Client::new()
                        .get(RELEASES_API_URL)
                        .header("User-Agent", "SeEx")
                        .send()
                        .await
                        .map_err(|error| error.to_string())?
                        .error_for_status()
                        .map_err(|error| error.to_string())?
                        .json::<GitHubRelease>()
                        .await
                        .map_err(|error| error.to_string())
                })
            })();

            if let Ok(mut state) = state.lock() {
                state.checking = false;
                state.checked = true;
                match result {
                    Ok(release) => {
                        state.latest_version =
                            Some(release.tag_name.trim_start_matches('v').to_string());
                        state.release_url = Some(release.html_url);
                    }
                    Err(error) => state.error = Some(format!("Update check failed: {error}")),
                }
            }
            ctx.request_repaint();
        });
    }

    fn language_toggle(&mut self, ui: &mut Ui) {
        if top_bar_button(ui, 72.0, "EN", self.lang == Lang::En).clicked() {
            self.lang = Lang::En;
            self.save_config();
        }
        if top_bar_button(ui, 72.0, "中文", self.lang == Lang::Zh).clicked() {
            self.lang = Lang::Zh;
            self.save_config();
        }
    }

    fn pin_toggle(&mut self, ui: &mut Ui) {
        let response = top_bar_button(ui, 82.0, self.t("PIN", "置顶"), self.pin_on_top);
        let clicked = response.clicked();

        response.on_hover_text(self.t("Keep window on top", "窗口置顶"));

        if clicked {
            self.pin_on_top = !self.pin_on_top;
            let window_level = if self.pin_on_top {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            };
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::WindowLevel(window_level));
        }
    }

    fn set_keyword_from_modes(&mut self) {
        let keyword = build_keyword(self.match_quick, self.match_full);
        if let Ok(mut state) = self.state.lock() {
            state.set_keyword(keyword);
        }
    }

    fn save_config(&self) {
        if let Ok(state) = self.state.lock() {
            snapshot_config(
                &state,
                self.lang,
                self.show_donate,
                self.schematic_stroke_color,
                self.schematic_fill_color,
            )
            .save();
        }
    }

    fn sync_system_theme(&mut self, ctx: &Context) {
        if self.last_theme_check.elapsed() < Duration::from_secs(2) {
            return;
        }
        self.last_theme_check = Instant::now();

        let dark_mode = detect_dark_mode();
        if dark_mode != is_dark_mode() {
            DARK_MODE.store(dark_mode, Ordering::Relaxed);
            install_style(ctx);
            ctx.request_repaint();
        }
    }
}

impl eframe::App for SeexApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.sync_system_theme(ui.ctx());
        ui.ctx().request_repaint_after(Duration::from_millis(250));
        ui.painter().rect_filled(ui.max_rect(), 0, app_bg());

        egui::Panel::top("top_bar")
            .exact_size(64.0)
            .show_separator_line(true)
            .frame(
                Frame::new()
                    .fill(surface())
                    .stroke(Stroke::new(1.0, border()))
                    .inner_margin(Margin::symmetric(14, 5)),
            )
            .show(ui, |ui| self.render_top_bar(ui));

        egui::Panel::bottom("status_bar")
            .exact_size(52.0)
            .show_separator_line(true)
            .frame(
                Frame::new()
                    .fill(surface())
                    .stroke(Stroke::new(1.0, border()))
                    .inner_margin(Margin::symmetric(14, 5)),
            )
            .show(ui, |ui| self.status_bar_ui(ui));

        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(app_bg())
                    .inner_margin(Margin::symmetric(24, 18)),
            )
            .show(ui, |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        match self.page {
                            Page::Monitor => self.monitor_page(ui),
                            Page::History => self.history_page(ui),
                            Page::Export => self.export_page(ui),
                            Page::More => self.more_page(ui),
                        }
                        ui.add_space(12.0);
                    });
            });
    }
}

impl SeexApp {
    fn monitoring_label(&self, ui: &mut Ui) {
        let monitoring = self
            .state
            .lock()
            .map(|state| state.monitoring)
            .unwrap_or(false);
        let label = if monitoring {
            self.t("● Listening", "● 监听中")
        } else {
            self.t("● Paused", "● 已暂停")
        };
        ui.label(RichText::new(label).color(if monitoring { success() } else { danger() }));
    }

    fn status_bar_ui(&self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if let Ok(state) = self.state.lock() {
                ui.label(format!(
                    "{} {} | {} {}",
                    self.t("History:", "历史:"),
                    state.history.len(),
                    self.t("Matched:", "匹配:"),
                    state.matched.len()
                ));
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("Copyright (c) Ref42. All rights reserved.").color(muted()),
                    );
                });
            }
        });
    }
}

fn install_style(ctx: &Context) {
    let theme = if is_dark_mode() {
        Theme::Dark
    } else {
        Theme::Light
    };
    ctx.set_theme(theme);
    ctx.set_visuals(if is_dark_mode() {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    });
    let mut style = (*ctx.style_of(theme)).clone();
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(scaled_font(18.0), FontFamily::Proportional),
        ),
        (
            TextStyle::Body,
            FontId::new(scaled_font(26.0), FontFamily::Proportional),
        ),
        (
            TextStyle::Button,
            FontId::new(scaled_font(26.0), FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(scaled_font(36.0), FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(scaled_font(26.0), FontFamily::Monospace),
        ),
    ]
    .into();
    style.spacing.item_spacing = Vec2::new(12.0, 12.0);
    style.spacing.button_padding = Vec2::new(16.0, 8.0);
    style.spacing.interact_size = Vec2::new(80.0, 40.0);
    style.spacing.slider_width = 260.0;
    style.spacing.combo_width = 260.0;
    style.spacing.text_edit_width = 640.0;
    style.visuals.override_text_color = Some(text_color());
    style.visuals.window_fill = app_bg();
    style.visuals.panel_fill = surface();
    style.visuals.extreme_bg_color = field_bg();
    style.visuals.widgets.inactive.bg_fill = field_bg();
    style.visuals.widgets.hovered.bg_fill = widget_hover_bg();
    style.visuals.widgets.active.bg_fill = primary();
    style.visuals.selection.bg_fill = primary();
    ctx.set_style_of(theme, style);
}

fn install_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "source_han_sans_sc".to_string(),
        FontData::from_static(include_bytes!("../assets/fonts/SourceHanSansSC.otf")).into(),
    );
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, "source_han_sans_sc".to_string());
    }

    ctx.set_fonts(fonts);
}

fn apply_config_to_state(config: &AppConfig, state: &mut MonitorState) {
    state.set_export_output_path(config.export.output_path.clone());
    state.set_export_target(config.export.target.clone());
    state.set_export_mode(config.export.mode.clone());
    state.set_export_kicad_mode(config.export.kicad_mode.clone());
    state.set_export_merge(config.export.merge);
    state.set_export_append(config.export.append);
    state.set_export_library_name(config.export.library_name.clone());
    state.set_export_parallel(config.export.parallel);
    state.set_export_continue_on_error(config.export.continue_on_error);
    state.set_export_lcsc_english(config.export.lcsc_english);
    state.set_export_force(config.export.force);
    state.set_matched_save_path(config.monitor.matched_save_path.clone());
    state.set_bom_language(config.monitor.bom_language.clone());
}

fn snapshot_config(
    state: &MonitorState,
    language: Lang,
    show_donate: bool,
    schematic_stroke_color: [u8; 3],
    schematic_fill_color: [u8; 3],
) -> AppConfig {
    AppConfig {
        export: ExportConfig {
            target: state.export_target.clone(),
            output_path: state.export_output_path.clone(),
            mode: state.export_mode.clone(),
            kicad_mode: state.export_kicad_mode.clone(),
            merge: state.export_merge,
            append: state.export_append,
            library_name: state.export_library_name.clone(),
            parallel: state.export_parallel,
            continue_on_error: state.export_continue_on_error,
            lcsc_english: state.export_lcsc_english,
            force: state.export_force,
            schematic_stroke_color,
            schematic_fill_color,
        },
        monitor: MonitorConfig {
            matched_save_path: state.matched_save_path.clone(),
            bom_language: state.bom_language.clone(),
        },
        ui_language: match language {
            Lang::En => "english".to_string(),
            Lang::Zh => "chinese".to_string(),
        },
        show_donate,
    }
}

fn build_keyword(match_quick: bool, match_full: bool) -> String {
    let mut parts = Vec::new();
    if match_full {
        parts.push(PATTERN_FULL);
    }
    if match_quick {
        parts.push(PATTERN_QUICK);
    }
    parts.join("||")
}

fn mode_button(ui: &mut Ui, mode: &mut String, value: &str, label: &str) {
    if ui.selectable_label(mode == value, label).clicked() {
        *mode = value.to_string();
    }
}

fn section(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    let width = ui.available_width();
    ui.allocate_ui_with_layout(
        Vec2::new(width, 0.0),
        Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_width(width);
            Frame::new()
                .fill(surface())
                .stroke(Stroke::new(1.0, border()))
                .inner_margin(Margin::symmetric(18, 16))
                .show(ui, |ui| {
                    ui.set_width((width - 36.0).max(0.0));
                    ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);
                    add_contents(ui);
                });
        },
    );
}

fn section_with_min_height(ui: &mut Ui, min_height: f32, add_contents: impl FnOnce(&mut Ui)) {
    let width = ui.available_width();
    ui.allocate_ui_with_layout(
        Vec2::new(width, min_height),
        Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_width(width);
            ui.set_min_height(min_height);
            Frame::new()
                .fill(surface())
                .stroke(Stroke::new(1.0, border()))
                .inner_margin(Margin::symmetric(18, 16))
                .show(ui, |ui| {
                    ui.set_width((width - 36.0).max(0.0));
                    ui.set_min_height((min_height - 32.0).max(0.0));
                    ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);
                    add_contents(ui);
                });
        },
    );
}

fn row_frame(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(row_bg())
        .stroke(Stroke::new(1.0, subtle_border()))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, add_contents);
    ui.add_space(6.0);
}

fn list_height(ui: &Ui, viewport_fraction: f32, minimum_rows: usize, row_height: f32) -> f32 {
    let viewport_height = ui.ctx().viewport_rect().height();
    let target_height = viewport_height * viewport_fraction;
    let minimum_height = minimum_rows as f32 * row_height;
    target_height
        .max(minimum_height)
        .min((viewport_height * 0.72).max(minimum_height))
}

fn section_header(ui: &mut Ui, title: &str, desc: &str) {
    ui.vertical(|ui| {
        ui.label(RichText::new(title).strong().color(text_color()));
        ui.label(RichText::new(desc).color(muted()));
    });
}

fn section_header_inline(ui: &mut Ui, title: &str, right: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).strong().color(text_color()));
        ui.with_layout(Layout::right_to_left(egui::Align::Center), right);
    });
}

fn fixed_label(ui: &mut Ui, label: &str) {
    ui.add_sized(
        Vec2::new(180.0, 30.0),
        egui::Label::new(RichText::new(label).color(muted())),
    );
}

fn color_setting(
    ui: &mut Ui,
    label: &str,
    color: &mut [u8; 3],
    hex_text: &mut String,
    picker_id: u8,
    active_picker: &mut Option<u8>,
) -> bool {
    let mut changed = false;
    let mut opened_this_frame = false;
    ui.horizontal(|ui| {
        fixed_label(ui, label);
        let swatch = Color32::from_rgb(color[0], color[1], color[2]);
        if ui
            .add(
                egui::Button::new("")
                    .min_size(Vec2::new(28.0, 24.0))
                    .fill(swatch),
            )
            .clicked()
        {
            *active_picker = if *active_picker == Some(picker_id) {
                None
            } else {
                Some(picker_id)
            };
            opened_this_frame = true;
        }
        if ui
            .add_sized(
                Vec2::new(100.0, 28.0),
                TextEdit::singleline(hex_text)
                    .char_limit(7)
                    .hint_text("#RRGGBB"),
            )
            .changed()
        {
            if let Some(parsed) = parse_hex_color(hex_text) {
                *color = parsed;
                changed = true;
            }
        }
    });

    if *active_picker == Some(picker_id) {
        let window = egui::Window::new(label)
            .id(egui::Id::new(("schematic-color", picker_id)))
            .collapsible(false)
            .resizable(false)
            .default_width(260.0)
            .show(ui.ctx(), |ui| {
                ui.label("Pick a swatch:");
                ui.horizontal_wrapped(|ui| {
                    for palette_color in [
                        [0, 0, 0],
                        [255, 255, 255],
                        [220, 53, 69],
                        [255, 193, 7],
                        [25, 135, 84],
                        [13, 110, 253],
                        [111, 66, 193],
                        [11, 56, 104],
                    ] {
                        let response =
                            ui.add(egui::Button::new("").min_size(Vec2::splat(24.0)).fill(
                                Color32::from_rgb(
                                    palette_color[0],
                                    palette_color[1],
                                    palette_color[2],
                                ),
                            ));
                        if response.clicked() {
                            *color = palette_color;
                            changed = true;
                        }
                    }
                });
                ui.spacing_mut().slider_width = ui.available_width().max(180.0);
                let mut color32 = Color32::from_rgb(color[0], color[1], color[2]);
                if egui::color_picker::color_picker_color32(
                    ui,
                    &mut color32,
                    egui::color_picker::Alpha::Opaque,
                ) {
                    *color = [color32.r(), color32.g(), color32.b()];
                    changed = true;
                }
                ui.horizontal(|ui| {
                    ui.label("Hex");
                    if ui
                        .add_sized(
                            Vec2::new(110.0, 28.0),
                            TextEdit::singleline(hex_text).char_limit(7),
                        )
                        .changed()
                    {
                        if let Some(parsed) = parse_hex_color(hex_text) {
                            *color = parsed;
                            changed = true;
                        }
                    }
                    if ui.button("Done").clicked() {
                        *active_picker = None;
                    }
                });
            });

        if !opened_this_frame {
            if let Some(window) = window {
                let clicked_outside = ui.ctx().input(|input| {
                    input.pointer.any_click()
                        && input
                            .pointer
                            .interact_pos()
                            .is_some_and(|position| !window.response.rect.contains(position))
                });
                if clicked_outside {
                    *active_picker = None;
                }
            }
        }
    }

    if changed {
        *hex_text = color_to_hex(*color);
    }
    changed
}

fn color_to_hex(color: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2])
}

fn parse_hex_color(value: &str) -> Option<[u8; 3]> {
    let value = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if value.len() != 6 {
        return None;
    }
    Some([
        u8::from_str_radix(&value[0..2], 16).ok()?,
        u8::from_str_radix(&value[2..4], 16).ok()?,
        u8::from_str_radix(&value[4..6], 16).ok()?,
    ])
}

fn empty_state(ui: &mut Ui, label: &str) {
    ui.add_space(8.0);
    ui.label(RichText::new(label).color(muted()).italics());
}

fn metric_pill(ui: &mut Ui, label: &str, value: usize, color: Color32) {
    let _ = metric_pill_text(ui, &format!("{label} {value}"), color);
}

fn metric_pill_text(ui: &mut Ui, text: &str, color: Color32) -> egui::Response {
    let font = FontId::new(scaled_font(25.0), FontFamily::Proportional);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, color);
    let size = galley.size() + Vec2::new(20.0, 10.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    ui.painter().rect_filled(rect, 12.0, chip_bg(color));
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, color);
    response
}

fn count_chip(ui: &mut Ui, value: usize, color: Color32) {
    chip(ui, &value.to_string(), color);
}

fn chip(ui: &mut Ui, label: &str, color: Color32) {
    let font = FontId::new(scaled_font(20.0), FontFamily::Proportional);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, color);
    let size = galley.size() + Vec2::new(18.0, 8.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(rect, 12.0, chip_bg(color));
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, color);
}

fn scaled_font(size: f32) -> f32 {
    size * FONT_SCALE_FROM_CURRENT
}

fn version_is_newer(latest: &str, current: &str) -> bool {
    let parse = |version: &str| {
        version
            .trim_start_matches('v')
            .split('.')
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    parse(latest) > parse(current)
}

fn top_bar_button(ui: &mut Ui, width: f32, label: &str, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 44.0), egui::Sense::click());
    let pressed = response.is_pointer_button_down_on();
    paint_top_bar_button(ui, rect, response.hovered(), pressed, selected);
    let text_rect = if pressed { rect.shrink(1.0) } else { rect };
    ui.painter().text(
        text_rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::new(scaled_font(24.0), FontFamily::Proportional),
        if selected {
            Color32::WHITE
        } else {
            text_color()
        },
    );
    response
}

fn paint_top_bar_button(
    ui: &mut Ui,
    rect: egui::Rect,
    hovered: bool,
    pressed: bool,
    selected: bool,
) {
    let fill = if selected {
        primary()
    } else if hovered {
        top_bar_hover()
    } else {
        top_bar_button_bg()
    };
    ui.painter()
        .rect_filled(if pressed { rect.shrink(1.0) } else { rect }, 7.0, fill);
}

fn copy_to_clipboard(text: &str) {
    if let Ok(mut clipboard) = Clipboard::new() {
        let _ = clipboard.set_text(text.to_string());
    }
}

fn first_line(content: &str, max_chars: usize) -> String {
    let mut line = content.lines().next().unwrap_or_default().to_string();
    if line.chars().count() > max_chars {
        line = line.chars().take(max_chars).collect::<String>();
        line.push_str("...");
    }
    line
}

fn render_export_metrics(ui: &mut Ui, result: &str, lang: Lang) -> bool {
    let mut metrics = Vec::new();
    for line in result.lines().filter(|line| !line.trim().is_empty()) {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("Elapsed:") {
            metrics.push(("Elapsed", value.trim()));
        } else if trimmed.contains("Total:") {
            for key in ["Total", "Success", "Skipped", "Failed"] {
                if let Some((_, value)) = trimmed.split_once(&format!("{key}:")) {
                    metrics.push((key, value.split('|').next().unwrap_or(value).trim()));
                }
            }
        }
    }

    let labels = match lang {
        Lang::En => ["Elapsed", "Total", "Success", "Skipped", "Failed"],
        Lang::Zh => ["耗时", "总数", "成功", "跳过", "失败"],
    };
    let keys = ["Elapsed", "Total", "Success", "Skipped", "Failed"];
    let mut failed_clicked = false;
    ui.horizontal(|ui| {
        for (index, label) in labels.iter().enumerate() {
            let value = metrics
                .iter()
                .find(|(key, _)| *key == keys[index])
                .map(|(_, value)| *value)
                .unwrap_or("0");
            let color = if keys[index] == "Elapsed" || keys[index] == "Success" {
                success()
            } else if keys[index] == "Failed" && value != "0" {
                danger()
            } else {
                text_color()
            };
            let response = metric_pill_text(ui, &format!("{label} {value}"), color);
            if keys[index] == "Failed" && value != "0" && response.clicked() {
                failed_clicked = true;
            }
        }
    });
    failed_clicked
}

fn extract_failed_ids(result: &str) -> Vec<String> {
    result
        .lines()
        .find(|line| line.trim().to_ascii_lowercase().starts_with("failed ids:"))
        .map(|line| {
            extract::extract_lcsc_ids(line)
                .into_iter()
                .collect::<Vec<String>>()
        })
        .unwrap_or_default()
}

fn detect_dark_mode() -> bool {
    matches!(dark_light::detect(), Ok(dark_light::Mode::Dark))
}

fn is_dark_mode() -> bool {
    DARK_MODE.load(Ordering::Relaxed)
}

fn primary() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(74, 174, 214)
    } else {
        Color32::from_rgb(33, 137, 182)
    }
}

fn app_bg() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(24, 27, 31)
    } else {
        Color32::from_rgb(246, 248, 250)
    }
}

fn surface() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(32, 36, 42)
    } else {
        Color32::from_rgb(255, 255, 255)
    }
}

fn row_bg() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(38, 43, 50)
    } else {
        Color32::from_rgb(250, 252, 253)
    }
}

fn border() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(55, 62, 72)
    } else {
        Color32::from_rgb(224, 229, 235)
    }
}

fn subtle_border() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(47, 54, 63)
    } else {
        Color32::from_rgb(235, 239, 244)
    }
}

fn success() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(84, 193, 135)
    } else {
        Color32::from_rgb(38, 152, 98)
    }
}

fn danger() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(245, 109, 120)
    } else {
        Color32::from_rgb(210, 65, 82)
    }
}

fn muted() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(166, 176, 188)
    } else {
        Color32::from_rgb(99, 111, 125)
    }
}

fn text_color() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(235, 239, 244)
    } else {
        Color32::from_rgb(27, 34, 43)
    }
}

fn field_bg() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(38, 43, 50)
    } else {
        Color32::from_rgb(250, 252, 253)
    }
}

fn widget_hover_bg() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(48, 55, 64)
    } else {
        Color32::from_rgb(239, 246, 250)
    }
}

fn top_bar_button_bg() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(36, 41, 48)
    } else {
        Color32::from_rgb(246, 248, 250)
    }
}

fn top_bar_hover() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(48, 55, 64)
    } else {
        Color32::from_rgb(235, 241, 246)
    }
}

fn chip_bg(color: Color32) -> Color32 {
    if is_dark_mode() {
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 42)
    } else {
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 24)
    }
}
