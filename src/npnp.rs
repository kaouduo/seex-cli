use ::npnp::LcedaClient;
use ::npnp::batch::{BatchOptions, BatchSummary, export_batch};
use ::npnp::cli::KicadVersion;
use ::npnp::kicad::{
    KicadBatchOptions, KicadBatchSummary, KicadExportOptions, KicadLibraryOptions,
    export_kicad_batch,
};
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
        let started_at = Instant::now();
        let input_path = if is_kicad_target(&req) {
            None
        } else {
            Some(create_temp_input_path())
        };
        let result: Result<String, String> = (|| {
            if let Some(path) = &input_path {
                write_ids_file(path, &req.ids)?;
            }
            set_progress(&state, running_message(&req), false, 0, req.ids.len());
            if !is_kicad_target(&req) {
                let input_path = input_path
                    .as_ref()
                    .ok_or_else(|| "missing npnp batch input path".to_string())?;
                return match run_altium_cli(&req, input_path) {
                    Ok(message) => Ok(message),
                    Err(CliExportError::NotFound) => run_altium_api(&req, input_path, started_at),
                    Err(err) => Err(err.to_string()),
                };
            }

            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|err| err.to_string())?;
            runtime.block_on(async {
                let summary = export_kicad_batch(build_kicad_batch_options(&req))
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(format_kicad_summary(&req, &summary, started_at.elapsed()))
            })
        })();

        if let Some(path) = &input_path {
            let _ = fs::remove_file(path);
        }

        let message = match result {
            Ok(message) => message,
            Err(err) => format_error(&req, &err, started_at.elapsed()),
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
    let mut file = BufWriter::new(fs::File::create(path).map_err(|e| e.to_string())?);
    for id in ids {
        writeln!(file, "{}", id).map_err(|e| e.to_string())?;
    }
    file.flush().map_err(|e| e.to_string())
}

fn run_altium_api(
    req: &ExportRequest,
    input_path: &Path,
    started_at: Instant,
) -> Result<String, String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|err| err.to_string())?;
    runtime.block_on(async {
        let client = LcedaClient::new();
        let summary = export_batch(&client, build_batch_options(req, &input_path.to_path_buf()))
            .await
            .map_err(|e| e.to_string())?;
        Ok(format_altium_summary(req, &summary, started_at.elapsed()))
    })
}

fn run_altium_cli(req: &ExportRequest, input_path: &Path) -> Result<String, CliExportError> {
    let mut command = npnp_cli_command();
    configure_hidden_command(&mut command);
    command
        .arg("altium")
        .arg("batch")
        .arg("--input")
        .arg(input_path)
        .arg("--output")
        .arg(normalize_output_path(&req.output_path));

    match normalize_altium_mode(&req.mode) {
        "schlib" => {
            command.arg("--schlib");
        }
        "pcblib" => {
            command.arg("--pcblib");
        }
        _ => {
            command.arg("--full");
        }
    }

    if req.merge {
        command.arg("--merge");
        command.arg("--library-name").arg(resolve_library_name(req));
    }
    if req.append && req.merge {
        command.arg("--append");
    }
    if req.parallel > 0 {
        command
            .arg("--parallel")
            .arg(req.parallel.max(1).to_string());
    }
    if req.continue_on_error {
        command.arg("--continue-on-error");
    }
    if req.lcsc_english {
        command.arg("--english-metadata");
    }
    if req.force {
        command.arg("--force");
    }

    let started_at = Instant::now();
    let output = command.output().map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            CliExportError::NotFound
        } else {
            CliExportError::Io(err.to_string())
        }
    })?;
    let elapsed = started_at.elapsed();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        Ok(format_altium_cli_summary(
            req,
            output.status,
            elapsed,
            &stdout,
            &stderr,
        ))
    } else {
        Err(CliExportError::Failed(format_command_output(
            output.status,
            elapsed,
            &stdout,
            &stderr,
        )))
    }
}

#[cfg(windows)]
fn configure_hidden_command(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn configure_hidden_command(_command: &mut Command) {}

fn npnp_cli_command() -> Command {
    if let Ok(current_exe) = std::env::current_exe() {
        for ancestor in current_exe.ancestors() {
            let local_npnp = ancestor
                .join("npnp")
                .join("target")
                .join("release")
                .join(if cfg!(windows) { "npnp.exe" } else { "npnp" });
            if local_npnp.exists() {
                return Command::new(local_npnp);
            }
        }
    }

    Command::new("npnp")
}

enum CliExportError {
    NotFound,
    Io(String),
    Failed(String),
}

impl std::fmt::Display for CliExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "npnp executable was not found on PATH"),
            Self::Io(err) => write!(f, "failed to run npnp CLI: {err}"),
            Self::Failed(output) => write!(f, "npnp CLI failed\n{output}"),
        }
    }
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

fn format_altium_summary(req: &ExportRequest, summary: &BatchSummary, elapsed: Duration) -> String {
    let mut lines = vec![
        "Status: Success".to_string(),
        "Target: Altium".to_string(),
        format!("Elapsed: {}", format_duration(elapsed)),
        format!(
            "npnp Altium batch complete. Total: {} | Skipped: {} | Success: {} | Failed: {}",
            summary.total, summary.skipped, summary.success, summary.failed,
        ),
    ];

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

fn format_altium_cli_summary(
    req: &ExportRequest,
    status: ExitStatus,
    elapsed: Duration,
    stdout: &str,
    stderr: &str,
) -> String {
    let parsed = parse_altium_cli_output(stdout, stderr);
    let mut lines = vec![
        "Status: Success".to_string(),
        "Target: Altium".to_string(),
        format!("Elapsed: {}", format_duration(elapsed)),
    ];

    if let Some(summary) = parsed.summary {
        lines.push(summary);
    } else {
        lines.push(format!("Exit: {}", status_label(status)));
    }

    lines.push(format!(
        "Targets: {} | Merge: {} | Parallel: {}",
        mode_label(req),
        merge_label(req),
        req.parallel.max(1),
    ));

    lines.push(format!(
        "Output directory: {}",
        normalize_output_path(&req.output_path)
    ));

    for path in parsed.generated_files {
        lines.push(format!("Generated: {path}"));
    }
    if !parsed.failed_ids.is_empty() {
        lines.push(format!("Failed IDs: {}", parsed.failed_ids.join(", ")));
    }

    lines.join("\n")
}

fn format_kicad_summary(
    req: &ExportRequest,
    summary: &KicadBatchSummary,
    elapsed: Duration,
) -> String {
    let mut lines = vec![
        "Status: Success".to_string(),
        "Target: KiCad".to_string(),
        format!("Elapsed: {}", format_duration(elapsed)),
        format!(
            "npnp KiCad batch complete. Total: {} | Skipped: {} | Success: {} | Failed: {}",
            summary.total, summary.skipped, summary.success, summary.failed,
        ),
    ];

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
    lines.push(format!(
        "Stage totals: fetch {} | metadata {} | symbol {} | footprint {} | 3d {}",
        format_duration(summary.timings.component_fetch),
        format_duration(summary.timings.metadata),
        format_duration(summary.timings.symbol),
        format_duration(summary.timings.footprint),
        format_duration(summary.timings.model_3d),
    ));
    lines.push(format!(
        "3D models: written {} | skipped {} | missing {}",
        summary.models.written, summary.models.skipped, summary.models.missing
    ));

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

fn format_command_output(
    status: ExitStatus,
    elapsed: Duration,
    stdout: &str,
    stderr: &str,
) -> String {
    let parsed = parse_altium_cli_output(stdout, stderr);
    let mut lines = vec![
        "Status: Failed".to_string(),
        format!("Exit: {}", status_label(status)),
        format!("Elapsed: {}", format_duration(elapsed)),
    ];

    if let Some(summary) = parsed.summary {
        lines.push(summary);
    }
    if !parsed.failed_ids.is_empty() {
        lines.push(format!("Failed IDs: {}", parsed.failed_ids.join(", ")));
    }
    for line in parsed.error_lines {
        lines.push(line);
    }

    lines.join("\n")
}

struct ParsedCliOutput {
    summary: Option<String>,
    generated_files: Vec<String>,
    failed_ids: Vec<String>,
    error_lines: Vec<String>,
}

fn parse_altium_cli_output(stdout: &str, stderr: &str) -> ParsedCliOutput {
    let mut summary = None;
    let mut generated_files = Vec::new();
    let mut failed_ids = Vec::new();
    let mut error_lines = Vec::new();

    for line in clean_command_lines(stdout) {
        if line.starts_with("Altium batch export complete.") {
            summary = Some(line);
        } else if let Some(path) = line.strip_prefix("Generated:") {
            generated_files.push(path.trim().to_string());
        } else if line.to_ascii_lowercase().starts_with("failed ids:") {
            failed_ids.extend(crate::extract::extract_lcsc_ids(&line));
        } else if line.starts_with("FAILED ") {
            failed_ids.extend(crate::extract::extract_lcsc_ids(&line));
            error_lines.push(line);
        }
    }

    for line in clean_command_lines(stderr) {
        if line.starts_with('[') || line.starts_with("Batch mode:") || line.starts_with("Output:") {
            continue;
        }
        if line.starts_with("Library:") || line.starts_with("Append mode:") {
            continue;
        }
        if line.starts_with("Writing merged library files") {
            continue;
        }
        if line.starts_with("FAILED ") {
            failed_ids.extend(crate::extract::extract_lcsc_ids(&line));
            error_lines.push(line);
        } else if line.to_ascii_lowercase().starts_with("failed ids:") {
            failed_ids.extend(crate::extract::extract_lcsc_ids(&line));
        } else if !line.trim().is_empty() {
            error_lines.push(line);
        }
    }

    failed_ids.sort();
    failed_ids.dedup();

    ParsedCliOutput {
        summary,
        generated_files,
        failed_ids,
        error_lines,
    }
}

fn clean_command_lines(output: &str) -> Vec<String> {
    output
        .replace('\r', "\n")
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn status_label(status: ExitStatus) -> String {
    match status.code() {
        Some(code) => code.to_string(),
        None => "terminated".to_string(),
    }
}

fn format_duration(duration: Duration) -> String {
    let total_ms = duration.as_millis();
    if total_ms < 60_000 {
        return format!("{}.{:02}s", total_ms / 1_000, (total_ms % 1_000) / 10);
    }

    let total_secs = duration.as_secs();
    let hours = total_secs / 3_600;
    let minutes = (total_secs % 3_600) / 60;
    let seconds = total_secs % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

fn format_error(req: &ExportRequest, error: &str, elapsed: Duration) -> String {
    let target = if is_kicad_target(req) {
        "KiCad"
    } else {
        "Altium"
    };
    let failed_ids = crate::extract::extract_lcsc_ids(error);
    let mut lines = vec![
        "Status: Failed".to_string(),
        format!("Target: {target}"),
        format!("Elapsed: {}", format_duration(elapsed)),
    ];
    if !failed_ids.is_empty() {
        lines.push(format!("Failed IDs: {}", failed_ids.join(", ")));
    } else {
        lines.push(error.to_string());
    }
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
