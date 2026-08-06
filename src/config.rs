use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_FILENAME: &str = "export_config.json";

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    #[serde(alias = "syft")]
    pub export: ExportConfig,
    pub monitor: MonitorConfig,
    pub ui_language: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            export: ExportConfig::default(),
            monitor: MonitorConfig::default(),
            ui_language: "english".to_string(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MonitorConfig {
    pub matched_save_path: String,
    pub bom_language: String,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            matched_save_path: String::new(),
            bom_language: "chinese".to_string(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportConfig {
    pub target: String,
    pub output_path: String,
    pub mode: String,
    pub kicad_mode: String,
    pub merge: bool,
    pub append: bool,
    pub library_name: String,
    pub parallel: usize,
    pub continue_on_error: bool,
    pub lcsc_english: bool,
    pub force: bool,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            target: "altium".to_string(),
            output_path: "export".to_string(),
            mode: "full".to_string(),
            kicad_mode: "full".to_string(),
            merge: false,
            append: false,
            library_name: "SeExMerged".to_string(),
            parallel: 4,
            continue_on_error: false,
            lcsc_english: false,
            force: false,
        }
    }
}

impl AppConfig {
    fn config_path() -> PathBuf {
        ProjectDirs::from("com", "ref42", "SeExEgui")
            .map(|dirs| dirs.config_dir().join(CONFIG_FILENAME))
            .or_else(|| {
                std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|d| d.join(CONFIG_FILENAME)))
            })
            .unwrap_or_else(|| PathBuf::from(CONFIG_FILENAME))
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, content);
        }
    }
}
