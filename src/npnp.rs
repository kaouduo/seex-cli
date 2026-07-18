use ::npnp::LcedaClient;
use ::npnp::batch::{BatchOptions, BatchSummary, export_batch};
use ::npnp::cli::KicadVersion;
use ::npnp::kicad::{
    KicadBatchOptions, KicadBatchSummary, KicadExportOptions, KicadLibraryOptions,
    export_kicad_batch,
};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::monitor::{ExportProgressState, MonitorState};

const DEFAULT_LIBRARY_NAME: &str = "SeExMerged";
const DEFAULT_OUTPUT_DIR: &str = "npnp_export";

pub struct ExportRequest {
    pub ids: Vec<String>,
    pub target: String,
    pub output_path: String,
    pub mode: String,
    pub merge: bool,
    pub append: bool,
    pub library_name: String,
    pub parallel: usize,
    pub continue_on_error: bool,
    pub lcsc_english: bool,
    pub force: bool,
    pub project_relative: bool,
}

pub fn spawn_export(state: Arc<Mutex<MonitorState>>, req: ExportRequest) {
    if let Ok(mut s) = state.lock() {
        s.npnp_running = true;
        s.npnp_last_result = None;
        s.npnp_progress = Some(ExportProgressState {
            determinate: false,
            current: 0,
            total: req.ids.len(),
            message: "Preparing npnp export...".to_string(),
        });
    }

    std::thread::spawn(move || {
        let input_path = create_temp_input_path();
        let result: Result<String, String> = (|| {
            write_ids_file(&input_path, &req.ids)?;
            set_progress(&state, running_message(&req), false, 0, req.ids.len());
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|err| err.to_string())?;
            runtime.block_on(async {
                if is_kicad_target(&req) {
                    let summary = export_kicad_batch(build_kicad_batch_options(&req))
                        .await
                        .map_err(|e| e.to_string())?;
                    Ok(format_kicad_summary(&req, &summary))
                } else {
                    let client = LcedaClient::new();
                    let summary = export_batch(&client, build_batch_options(&req, &input_path))
                        .await
                        .map_err(|e| e.to_string())?;
                    Ok(format_altium_summary(&req, &summary))
                }
            })
        })();

        let _ = fs::remove_file(&input_path);

        let message = match result {
            Ok(message) => message,
            Err(err) => format_error(&req, &err),
        };

        if let Ok(mut s) = state.lock() {
            s.npnp_running = false;
            s.npnp_progress = None;
            s.npnp_last_result = Some(message.clone());
            s.add_debug_log(message);
        }
    });
}

fn set_progress(
    state: &Arc<Mutex<MonitorState>>,
    message: impl Into<String>,
    determinate: bool,
    current: usize,
    total: usize,
) {
    if let Ok(mut s) = state.lock() {
        s.npnp_progress = Some(ExportProgressState {
            determinate,
            current,
            total,
            message: message.into(),
        });
    }
}

fn running_message(req: &ExportRequest) -> String {
    let count = req.ids.len();
    if is_kicad_target(req) {
        format!("Running npnp KiCad batch for {} items...", count)
    } else if req.merge && req.append {
        format!("Appending into merged npnp library ({} items)...", count)
    } else if req.merge {
        format!("Merging {} npnp items...", count)
    } else {
        format!("Running npnp batch for {} items...", count)
    }
}

fn create_temp_input_path() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "seex_npnp_ids_{}_{}.txt",
        std::process::id(),
        stamp
    ))
}

fn write_ids_file(path: &PathBuf, ids: &[String]) -> Result<(), String> {
    let mut file = fs::File::create(path).map_err(|e| e.to_string())?;
    for id in ids {
        writeln!(file, "{}", id).map_err(|e| e.to_string())?;
    }
    file.sync_all().map_err(|e| e.to_string())
}

fn build_batch_options(req: &ExportRequest, input_path: &PathBuf) -> BatchOptions {
    let mode = normalize_altium_mode(&req.mode);
    let append = req.append && req.merge;
    BatchOptions {
        input: input_path.clone(),
        output: PathBuf::from(normalize_output_path(&req.output_path)),
        schlib: mode == "schlib",
        pcblib: mode == "pcblib",
        full: mode == "full",
        merge: req.merge,
        append,
        library_name: effective_library_name(req),
        parallel: req.parallel.max(1),
        continue_on_error: req.continue_on_error,
        english_metadata: req.lcsc_english,
        force: req.force,
    }
}

fn build_kicad_batch_options(req: &ExportRequest) -> KicadBatchOptions {
    let mode = normalize_kicad_mode(&req.mode);
    KicadBatchOptions {
        ids: req.ids.clone(),
        library: KicadLibraryOptions {
            output: PathBuf::from(normalize_output_path(&req.output_path)),
            library_name: Some(resolve_library_name(req)),
            symbol_lib: None,
            footprint_lib: None,
            model_lib: None,
        },
        export: KicadExportOptions {
            symbol: mode == "symbol",
            footprint: mode == "footprint",
            model_3d: mode == "3d",
            full: mode == "full",
            force: req.force,
            project_relative: req.project_relative,
            kicad_version: KicadVersion::V6,
        },
        english_metadata: req.lcsc_english,
        parallel: req.parallel.max(1),
        continue_on_error: req.continue_on_error,
    }
}

fn is_kicad_target(req: &ExportRequest) -> bool {
    normalize_target(&req.target) == "kicad"
}

fn normalize_target(target: &str) -> &str {
    match target.trim().to_ascii_lowercase().as_str() {
        "kicad" => "kicad",
        _ => "altium",
    }
}

fn normalize_altium_mode(mode: &str) -> &str {
    match mode.trim().to_ascii_lowercase().as_str() {
        "schlib" => "schlib",
        "pcblib" => "pcblib",
        _ => "full",
    }
}

fn normalize_kicad_mode(mode: &str) -> &str {
    match mode.trim().to_ascii_lowercase().as_str() {
        "symbol" => "symbol",
        "footprint" => "footprint",
        "3d" => "3d",
        _ => "full",
    }
}

fn normalize_output_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        DEFAULT_OUTPUT_DIR.to_string()
    } else {
        trimmed.to_string()
    }
}

fn effective_library_name(req: &ExportRequest) -> Option<String> {
    if !is_kicad_target(req) && !req.merge {
        return None;
    }

    Some(resolve_library_name(req))
}

fn resolve_library_name(req: &ExportRequest) -> String {
    let trimmed = req.library_name.trim();
    if trimmed.is_empty() {
        DEFAULT_LIBRARY_NAME.to_string()
    } else {
        trimmed.to_string()
    }
}

fn format_altium_summary(req: &ExportRequest, summary: &BatchSummary) -> String {
    let mut lines = vec![format!(
        "npnp Altium batch complete. Total: {} | Skipped: {} | Success: {} | Failed: {}",
        summary.total, summary.skipped, summary.success, summary.failed,
    )];

    lines.push(format!(
        "Targets: {} | Merge: {} | Parallel: {}",
        mode_label(req),
        merge_label(req),
        req.parallel.max(1),
    ));

    lines.push(format!(
        "Continue on error: {} | LCSC English: {} | Force: {}",
        yes_no(req.continue_on_error),
        yes_no(req.lcsc_english),
        yes_no(req.force),
    ));

    if req.merge {
        lines.push(format!(
            "Library name: {}",
            effective_library_name(req).unwrap_or_else(|| DEFAULT_LIBRARY_NAME.to_string())
        ));
    }

    if !summary.failed_ids.is_empty() {
        lines.push(format!("Failed IDs: {}", summary.failed_ids.join(", ")));
    }

    if summary.generated_files.is_empty() {
        lines.push(format!("Output directory: {}", summary.output.display()));
        if !req.merge {
            lines.push(format!("Folders: {}", target_folders(req).join(", ")));
        }
    } else {
        for path in &summary.generated_files {
            lines.push(format!("Generated: {}", path.display()));
        }
    }

    lines.join("\n")
}

fn format_kicad_summary(req: &ExportRequest, summary: &KicadBatchSummary) -> String {
    let mut lines = vec![format!(
        "npnp KiCad batch complete. Total: {} | Success: {} | Failed: {}",
        summary.total, summary.success, summary.failed,
    )];

    lines.push(format!(
        "Targets: {} | Parallel: {}",
        mode_label(req),
        req.parallel.max(1),
    ));

    lines.push(format!(
        "Continue on error: {} | LCSC English: {} | Force: {} | Project Relative: {}",
        yes_no(req.continue_on_error),
        yes_no(req.lcsc_english),
        yes_no(req.force),
        yes_no(req.project_relative),
    ));

    lines.push(format!("Library name: {}", resolve_library_name(req)));

    if !summary.failed_ids.is_empty() {
        lines.push(format!("Failed IDs: {}", summary.failed_ids.join(", ")));
    }

    if !summary.failure_details.is_empty() {
        lines.push("Failure details:".to_string());
        for detail in &summary.failure_details {
            lines.push(format!("- {}", detail));
        }
    }

    if summary.generated_files.is_empty() {
        lines.push(format!("Output directory: {}", summary.output.display()));
        lines.push(format!("Folders: {}", target_folders(req).join(", ")));
    } else {
        for path in &summary.generated_files {
            lines.push(format!("Generated: {}", path.display()));
        }
    }

    lines.join("\n")
}

fn format_error(req: &ExportRequest, error: &str) -> String {
    let target = if is_kicad_target(req) {
        "KiCad"
    } else {
        "Altium"
    };
    let mut lines = vec![format!("npnp {target} export failed"), error.to_string()];
    lines.push(format!("Targets: {}", mode_label(req)));
    lines.push(format!(
        "Output directory: {}",
        normalize_output_path(&req.output_path)
    ));
    lines.push(format!(
        "Library name: {}",
        effective_library_name(req).unwrap_or_else(|| DEFAULT_LIBRARY_NAME.to_string())
    ));
    lines.join("\n")
}

fn mode_label(req: &ExportRequest) -> &'static str {
    if is_kicad_target(req) {
        match normalize_kicad_mode(&req.mode) {
            "symbol" => "Symbol",
            "footprint" => "Footprint",
            "3d" => "3D",
            _ => "Full",
        }
    } else {
        match normalize_altium_mode(&req.mode) {
            "schlib" => "SchLib",
            "pcblib" => "PcbLib",
            _ => "Full",
        }
    }
}

fn merge_label(req: &ExportRequest) -> &'static str {
    match (req.merge, req.append) {
        (true, true) => "ON (append)",
        (true, false) => "ON",
        _ => "OFF",
    }
}

fn target_folders(req: &ExportRequest) -> Vec<&'static str> {
    if is_kicad_target(req) {
        match normalize_kicad_mode(&req.mode) {
            "symbol" => vec!["*.kicad_sym"],
            "footprint" => vec!["*.pretty"],
            "3d" => vec!["*.3dshapes"],
            _ => vec!["*.kicad_sym", "*.pretty", "*.3dshapes"],
        }
    } else {
        match normalize_altium_mode(&req.mode) {
            "schlib" => vec!["schlib"],
            "pcblib" => vec!["pcblib"],
            _ => vec!["schlib", "pcblib"],
        }
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "ON" } else { "OFF" }
}
