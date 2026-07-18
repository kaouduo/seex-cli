#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bom;
mod config;
mod extract;
mod monitor;
mod npnp;

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

use crate::config::{AppConfig, MonitorConfig, NpnpConfig};

const PATTERN_QUICK: &str = "regex:(?m)^(C\\d{3,})$";
const PATTERN_FULL: &str = "regex:编号[：:]\\s*(C\\d+)";
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
    let mut image = image::load_from_memory(include_bytes!("../assets/icons/seex.png"))
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Monitor,
    History,
    Export,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lang {
    En,
    Zh,
}

struct SeexApp {
    state: Arc<Mutex<MonitorState>>,
    _monitor: MonitorHandle,
    page: Page,
    lang: Lang,
    pin_on_top: bool,
    show_matched: bool,
    show_history: bool,
    match_quick: bool,
    match_full: bool,
    clear_confirm: bool,
    last_theme_check: Instant,
}

impl SeexApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        DARK_MODE.store(detect_dark_mode(), Ordering::Relaxed);
        install_style(&cc.egui_ctx);

        let config = AppConfig::load();
        let mut initial = MonitorState::new();
        apply_config_to_state(&config, &mut initial);
        initial.set_keyword(build_keyword(true, true));

        let state = Arc::new(Mutex::new(initial));
        let monitor = MonitorHandle::spawn(Arc::clone(&state));

        Self {
            state,
            _monitor: monitor,
            page: Page::Monitor,
            lang: Lang::En,
            pin_on_top: true,
            show_matched: true,
            show_history: true,
            match_quick: true,
            match_full: true,
            clear_confirm: false,
            last_theme_check: Instant::now(),
        }
    }

    fn t(&self, en: &'static str, zh: &'static str) -> &'static str {
        match self.lang {
            Lang::En => en,
            Lang::Zh => zh,
        }
    }

    fn render_top_bar(&mut self, ui: &mut Ui) {
        const ROW_HEIGHT: f32 = 44.0;
        const RIGHT_CONTROLS_WIDTH: f32 = 390.0;

        ui.set_height(ROW_HEIGHT);
        ui.spacing_mut().item_spacing = Vec2::new(10.0, 0.0);
        ui.horizontal(|ui| {
            ui.set_height(ROW_HEIGHT);
            top_bar_text(ui, 96.0, "SeEx", 30.0, primary(), true);
            top_bar_separator(ui, ROW_HEIGHT);
            self.nav_item(ui, Page::Monitor, self.t("Monitor", "监听"));
            self.nav_item(ui, Page::History, self.t("History", "历史"));
            self.nav_item(ui, Page::Export, self.t("Export", "导出"));

            ui.add_space((ui.available_width() - RIGHT_CONTROLS_WIDTH).max(0.0));
            self.language_toggle(ui);
            self.pin_toggle(ui);
            self.monitoring_label(ui);
        });
    }

    fn nav_item(&mut self, ui: &mut Ui, page: Page, label: &'static str) {
        let selected = self.page == page;
        if top_bar_button(ui, 132.0, label, selected).clicked() {
            self.page = page;
        }
    }

    fn page_title(&self, ui: &mut Ui, title: &'static str, desc: &'static str) {
        ui.horizontal(|ui| {
            ui.heading(title);
            ui.label(RichText::new(desc).color(muted()));
        });
        ui.separator();
    }

    fn monitor_page(&mut self, ui: &mut Ui) {
        self.page_title(
            ui,
            self.t("Monitor", "监听"),
            self.t("Clipboard event tracker", "剪贴板事件追踪器"),
        );

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
        if let Ok(mut state) = self.state.lock() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.strong(self.t("Export files", "导出文件"));
                    ui.label(
                        RichText::new(self.t(
                            "BOM CSV for component keeping.",
                            "用于元件管理的 BOM CSV 文件。",
                        ))
                        .color(muted()),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label(self.t("BOM CSV file:", "BOM CSV 文件:"));
                    if ui
                        .add(
                            TextEdit::singleline(&mut state.matched_save_path).desired_width(640.0),
                        )
                        .changed()
                    {
                        save_config = true;
                    }
                    if ui.button(self.t("Browse", "浏览")).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("CSV", &["csv"])
                            .set_file_name("matched.csv")
                            .save_file()
                        {
                            state.set_matched_save_path(path.to_string_lossy().into_owned());
                            save_config = true;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(self.t("Metadata language:", "元数据语言:"));
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
            });
        }
        if save_config {
            self.save_config();
        }

        ui.add_space(10.0);
        self.monitor_actions(ui);
        ui.add_space(10.0);
        self.matched_list(ui);
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
            ui.label(format!(
                "{}: {}",
                self.t("Matched", "匹配结果"),
                matched_count
            ));
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

        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.strong(self.t("Matched", "匹配结果"));
                ui.label(RichText::new(format!("{}", items.len())).color(primary()));
            });
            if !self.show_matched {
                return;
            }
            if items.is_empty() {
                ui.label(RichText::new(self.t("No matches yet", "暂无匹配结果")).color(muted()));
                return;
            }

            ScrollArea::vertical().max_height(230.0).show(ui, |ui| {
                let mut delete_index = None;
                for (idx, item) in items.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.monospace(&item.last_seen);
                        ui.label(RichText::new(&item.lcsc_id).strong().color(primary()));
                        ui.label(format!("x{}", item.count));
                        ui.label(RichText::new(first_line(&item.source_text, 72)).color(muted()));
                        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button(self.t("Delete", "删除")).clicked() {
                                delete_index = Some(idx);
                            }
                            if ui.small_button(self.t("Copy", "复制")).clicked() {
                                copy_to_clipboard(&item.lcsc_id);
                            }
                        });
                    });
                    ui.separator();
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
        self.page_title(
            ui,
            self.t("History", "历史"),
            self.t("Clipboard history", "剪贴板历史"),
        );
        let show_history = self.t("Show history", "显示历史");
        ui.checkbox(&mut self.show_history, show_history);

        let history = self
            .state
            .lock()
            .map(|state| state.history.clone())
            .unwrap_or_default();
        if history.is_empty() {
            ui.label(RichText::new(self.t("No history yet", "暂无历史记录")).color(muted()));
            return;
        }

        ScrollArea::vertical().show(ui, |ui| {
            let mut delete_index = None;
            for (idx, (time, content)) in history.iter().enumerate() {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.monospace(time);
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
    }

    fn export_page(&mut self, ui: &mut Ui) {
        self.page_title(
            ui,
            self.t("Export", "导出"),
            self.t("Component export integrations", "元件导出集成"),
        );

        let mut request = None;
        let mut save_config = false;

        if let Ok(mut state) = self.state.lock() {
            let matched_count = state.matched.len();
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.heading(self.t("EDA Library Export", "EDA 库导出"));
                    ui.label(
                        RichText::new(format!(
                            "{} {}",
                            matched_count,
                            self.t("items ready", "项待导出")
                        ))
                        .color(primary()),
                    );
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                matched_count > 0 && !state.npnp_running,
                                egui::Button::new(if state.npnp_running {
                                    self.t("Running...", "运行中...")
                                } else {
                                    self.t("Export Library", "导出库")
                                }),
                            )
                            .clicked()
                        {
                            request = Some(npnp::ExportRequest {
                                ids: state.get_unique_ids(),
                                target: state.npnp_target.clone(),
                                output_path: state.npnp_output_path.clone(),
                                mode: if state.npnp_target == "kicad" {
                                    state.npnp_kicad_mode.clone()
                                } else {
                                    state.npnp_mode.clone()
                                },
                                merge: state.npnp_merge,
                                append: state.npnp_append,
                                library_name: state.npnp_library_name.clone(),
                                parallel: state.npnp_parallel,
                                continue_on_error: state.npnp_continue_on_error,
                                lcsc_english: state.npnp_lcsc_english,
                                force: state.npnp_force,
                                project_relative: state.npnp_project_relative,
                            });
                        }
                    });
                });
                ui.label(
                    RichText::new(self.t(
                        "Altium and KiCad export through npnp",
                        "通过 npnp 导出 Altium 和 KiCad 库",
                    ))
                    .color(muted()),
                );
                if let Some(progress) = &state.npnp_progress {
                    if progress.determinate && progress.total > 0 {
                        let value = progress.current as f32 / progress.total as f32;
                        ui.add(egui::ProgressBar::new(value).text(&progress.message));
                    } else {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(&progress.message);
                        });
                    }
                }
                if let Some(result) = &state.npnp_last_result {
                    ui.separator();
                    render_export_result(ui, result);
                    let failed = extract_failed_ids(result);
                    ui.horizontal(|ui| {
                        if !failed.is_empty()
                            && ui
                                .button(self.t("Copy failed IDs", "复制失败 ID"))
                                .clicked()
                        {
                            copy_to_clipboard(&failed.join("\n"));
                        }
                        if ui.button(self.t("Copy details", "复制详情")).clicked() {
                            copy_to_clipboard(result);
                        }
                    });
                }
            });

            ui.add_space(10.0);
            ui.group(|ui| {
                ui.heading(self.t("Export Settings", "导出设置"));
                ui.horizontal(|ui| {
                    ui.label(self.t("EDA target:", "EDA 目标:"));
                    let mut target = state.npnp_target.clone();
                    ComboBox::from_id_salt("target")
                        .selected_text(if target == "kicad" { "KiCad" } else { "Altium" })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut target, "altium".to_string(), "Altium");
                            ui.selectable_value(&mut target, "kicad".to_string(), "KiCad");
                        });
                    if target != state.npnp_target {
                        state.set_npnp_target(target);
                        save_config = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(self.t("Export directory:", "导出目录:"));
                    if ui
                        .add(TextEdit::singleline(&mut state.npnp_output_path).desired_width(640.0))
                        .changed()
                    {
                        save_config = true;
                    }
                    if ui.button(self.t("Browse", "浏览")).clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            state.set_npnp_output_path(path.to_string_lossy().into_owned());
                            save_config = true;
                        }
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(self.t("Export scope:", "导出范围:"));
                    if state.npnp_target == "kicad" {
                        mode_button(
                            ui,
                            &mut state.npnp_kicad_mode,
                            "full",
                            self.t("Full", "完整"),
                        );
                        mode_button(
                            ui,
                            &mut state.npnp_kicad_mode,
                            "symbol",
                            self.t("Symbol", "符号"),
                        );
                        mode_button(
                            ui,
                            &mut state.npnp_kicad_mode,
                            "footprint",
                            self.t("Footprint", "封装"),
                        );
                        mode_button(ui, &mut state.npnp_kicad_mode, "3d", "3D");
                    } else {
                        mode_button(ui, &mut state.npnp_mode, "full", self.t("Full", "完整"));
                        mode_button(ui, &mut state.npnp_mode, "schlib", "SchLib");
                        mode_button(ui, &mut state.npnp_mode, "pcblib", "PcbLib");
                    }
                    save_config = true;
                });

                ui.horizontal(|ui| {
                    ui.label(self.t("Library name:", "库名称:"));
                    if ui
                        .add_enabled(
                            state.npnp_target == "kicad" || state.npnp_merge,
                            TextEdit::singleline(&mut state.npnp_library_name).desired_width(320.0),
                        )
                        .changed()
                    {
                        save_config = true;
                    }
                    ui.label(self.t("Parallel jobs:", "并行任务数:"));
                    let mut parallel = state.npnp_parallel as u32;
                    if ui
                        .add(egui::DragValue::new(&mut parallel).range(1..=64))
                        .changed()
                    {
                        state.set_npnp_parallel(parallel as usize);
                        save_config = true;
                    }
                });

                ui.horizontal_wrapped(|ui| {
                    if state.npnp_target == "altium" {
                        let mut merge = state.npnp_merge;
                        if ui.checkbox(&mut merge, self.t("Merge", "合并")).changed() {
                            state.set_npnp_merge(merge);
                            save_config = true;
                        }
                        let mut append = state.npnp_append;
                        if ui.checkbox(&mut append, self.t("Append", "追加")).changed() {
                            state.set_npnp_append(append);
                            save_config = true;
                        }
                    } else {
                        let mut project_relative = state.npnp_project_relative;
                        if ui
                            .checkbox(
                                &mut project_relative,
                                self.t("Project Relative", "项目相对路径"),
                            )
                            .changed()
                        {
                            state.set_npnp_project_relative(project_relative);
                            save_config = true;
                        }
                    }
                    let mut continue_on_error = state.npnp_continue_on_error;
                    if ui
                        .checkbox(
                            &mut continue_on_error,
                            self.t("Continue On Error", "出错继续"),
                        )
                        .changed()
                    {
                        state.set_npnp_continue_on_error(continue_on_error);
                        save_config = true;
                    }
                    let mut force = state.npnp_force;
                    if ui.checkbox(&mut force, self.t("Force", "强制")).changed() {
                        state.set_npnp_force(force);
                        save_config = true;
                    }
                    let mut english = state.npnp_lcsc_english;
                    if ui
                        .checkbox(&mut english, self.t("English metadata", "英文元数据"))
                        .changed()
                    {
                        state.set_npnp_lcsc_english(english);
                        state.bom_language =
                            if english { "english" } else { "chinese" }.to_string();
                        save_config = true;
                    }
                });
            });
        }

        if save_config {
            self.save_config();
        }
        if let Some(req) = request {
            npnp::spawn_export(Arc::clone(&self.state), req);
        }
    }

    fn language_toggle(&mut self, ui: &mut Ui) {
        if top_bar_button(ui, 72.0, "EN", self.lang == Lang::En).clicked() {
            self.lang = Lang::En;
        }
        if top_bar_button(ui, 72.0, "中文", self.lang == Lang::Zh).clicked() {
            self.lang = Lang::Zh;
        }
    }

    fn pin_toggle(&mut self, ui: &mut Ui) {
        let response = top_bar_icon_button(ui, 52.0, self.pin_on_top);
        let rect = response.rect;
        let clicked = response.clicked();
        let icon_color = if self.pin_on_top {
            Color32::WHITE
        } else {
            text_color()
        };
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "\u{1F4CC}",
            FontId::new(scaled_font(22.0), FontFamily::Proportional),
            icon_color,
        );

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
            snapshot_config(&state).save();
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
    state.set_npnp_output_path(config.npnp.output_path.clone());
    state.set_npnp_target(config.npnp.target.clone());
    state.set_npnp_mode(config.npnp.mode.clone());
    state.set_npnp_kicad_mode(config.npnp.kicad_mode.clone());
    state.set_npnp_merge(config.npnp.merge);
    state.set_npnp_append(config.npnp.append);
    state.set_npnp_library_name(config.npnp.library_name.clone());
    state.set_npnp_parallel(config.npnp.parallel);
    state.set_npnp_continue_on_error(config.npnp.continue_on_error);
    state.set_npnp_lcsc_english(config.npnp.lcsc_english);
    state.set_npnp_force(config.npnp.force);
    state.set_npnp_project_relative(config.npnp.project_relative);
    state.set_matched_save_path(config.monitor.matched_save_path.clone());
    state.set_bom_language(config.monitor.bom_language.clone());
}

fn snapshot_config(state: &MonitorState) -> AppConfig {
    AppConfig {
        npnp: NpnpConfig {
            target: state.npnp_target.clone(),
            output_path: state.npnp_output_path.clone(),
            mode: state.npnp_mode.clone(),
            kicad_mode: state.npnp_kicad_mode.clone(),
            merge: state.npnp_merge,
            append: state.npnp_append,
            library_name: state.npnp_library_name.clone(),
            parallel: state.npnp_parallel,
            continue_on_error: state.npnp_continue_on_error,
            lcsc_english: state.npnp_lcsc_english,
            force: state.npnp_force,
            project_relative: state.npnp_project_relative,
        },
        monitor: MonitorConfig {
            matched_save_path: state.matched_save_path.clone(),
            bom_language: state.bom_language.clone(),
        },
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

fn scaled_font(size: f32) -> f32 {
    size * FONT_SCALE_FROM_CURRENT
}

fn top_bar_text(ui: &mut Ui, width: f32, label: &str, size: f32, color: Color32, strong: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 44.0), egui::Sense::hover());
    let font_size = if strong { size + 1.0 } else { size };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::new(scaled_font(font_size), FontFamily::Proportional),
        color,
    );
}

fn top_bar_separator(ui: &mut Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(8.0, height), egui::Sense::hover());
    ui.painter().line_segment(
        [
            egui::pos2(rect.center().x, rect.top()),
            egui::pos2(rect.center().x, rect.bottom()),
        ],
        Stroke::new(1.0, border()),
    );
}

fn top_bar_button(ui: &mut Ui, width: f32, label: &str, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 44.0), egui::Sense::click());
    paint_top_bar_button(ui, rect, response.hovered(), selected);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::new(scaled_font(26.0), FontFamily::Proportional),
        if selected {
            Color32::WHITE
        } else {
            text_color()
        },
    );
    response
}

fn top_bar_icon_button(ui: &mut Ui, width: f32, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 44.0), egui::Sense::click());
    paint_top_bar_button(ui, rect, response.hovered(), selected);
    response
}

fn paint_top_bar_button(ui: &mut Ui, rect: egui::Rect, hovered: bool, selected: bool) {
    let fill = if selected {
        primary()
    } else if hovered {
        top_bar_hover()
    } else {
        top_bar_button_bg()
    };
    ui.painter().rect_filled(rect, 2.0, fill);
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

fn render_export_result(ui: &mut Ui, result: &str) {
    for line in result.lines().filter(|line| !line.trim().is_empty()) {
        let trimmed = line.trim();
        let color = if is_success_result_line(trimmed) {
            success()
        } else if is_failure_result_line(trimmed) {
            danger()
        } else if is_muted_result_line(trimmed) {
            muted()
        } else {
            text_color()
        };
        let text = if trimmed.starts_with("Generated:") || trimmed.starts_with("Output directory:")
        {
            RichText::new(trimmed).monospace().color(color)
        } else {
            RichText::new(trimmed).color(color)
        };
        ui.label(text);
    }
}

fn is_success_result_line(line: &str) -> bool {
    line == "Status: Success"
        || line.starts_with("Elapsed:")
        || line.starts_with("Stage totals:")
        || line.starts_with("3D models:")
        || line.starts_with("Generated:")
        || (line.contains("Success:") && line.contains("Failed: 0"))
}

fn is_failure_result_line(line: &str) -> bool {
    line == "Status: Failed"
        || line.starts_with("Failed IDs:")
        || line.starts_with("FAILED ")
        || line.starts_with("npnp ")
        || line.contains(" failed")
}

fn is_muted_result_line(line: &str) -> bool {
    line.starts_with("Targets:")
        || line.starts_with("Target:")
        || line.starts_with("Output directory:")
        || line.starts_with("Exit:")
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
        Color32::from_rgb(239, 116, 124)
    } else {
        Color32::from_rgb(232, 101, 108)
    }
}

fn app_bg() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(30, 32, 36)
    } else {
        Color32::from_rgb(243, 240, 235)
    }
}

fn surface() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(40, 43, 48)
    } else {
        Color32::from_rgb(255, 255, 255)
    }
}

fn border() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(76, 72, 76)
    } else {
        Color32::from_rgb(242, 221, 214)
    }
}

fn success() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(106, 190, 112)
    } else {
        Color32::from_rgb(80, 150, 82)
    }
}

fn danger() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(241, 99, 119)
    } else {
        Color32::from_rgb(220, 65, 84)
    }
}

fn muted() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(181, 170, 164)
    } else {
        Color32::from_rgb(125, 101, 96)
    }
}

fn text_color() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(239, 232, 226)
    } else {
        Color32::from_rgb(44, 24, 16)
    }
}

fn field_bg() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(48, 51, 57)
    } else {
        Color32::from_rgb(255, 248, 244)
    }
}

fn widget_hover_bg() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(64, 67, 74)
    } else {
        Color32::from_rgb(255, 236, 210)
    }
}

fn top_bar_button_bg() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(58, 61, 67)
    } else {
        Color32::from_rgb(232, 232, 232)
    }
}

fn top_bar_hover() -> Color32 {
    if is_dark_mode() {
        Color32::from_rgb(70, 74, 82)
    } else {
        Color32::from_rgb(244, 244, 244)
    }
}
