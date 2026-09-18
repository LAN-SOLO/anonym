//! App-Einstellungen als JSON im Config-Ordner der App (`settings.json`).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// "de" | "en"
    pub language: String,
    /// "dark" | "light"
    pub theme: String,
    /// "blue" | "emerald" | "violet" | "amber"
    pub accent: String,
    pub auto_update: bool,
    /// Tarif „masked“ — im Vorabzugang frei umschaltbar, keine Lizenzprüfung.
    pub masked: bool,
    /// Zielordner für Ausgaben (leer = neben der Quelle).
    pub output_dir: String,
    /// Namenszusatz der Ausgabe (`.anonym` → `brief.anonym.txt`).
    pub output_suffix: String,
    /// Codepage für Dateien, die kein UTF-8 sind.
    pub fallback_encoding: String,
    /// Pseudonym-Speicher über Dateien hinweg nutzen (masked).
    pub use_store: bool,
    /// Bericht (JSON) neben die Ausgabe schreiben.
    pub write_report: bool,
    /// Vor dem Überschreiben vorhandener Ausgaben nachfragen (nur UI).
    pub confirm_overwrite: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            language: if sys_locale_is_german() { "de" } else { "en" }.into(),
            theme: "dark".into(),
            accent: "blue".into(),
            auto_update: false,
            masked: false,
            output_dir: String::new(),
            output_suffix: ".anonym".into(),
            fallback_encoding: "windows-1252".into(),
            use_store: false,
            write_report: true,
            confirm_overwrite: true,
        }
    }
}

fn sys_locale_is_german() -> bool {
    sys_locale::get_locale().map(|l| l.to_lowercase().starts_with("de")).unwrap_or(false)
}

pub fn config_dir(app: &tauri::AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| dirs::config_dir().unwrap_or_default().join("anonym"));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn settings_path(config_dir: &std::path::Path) -> PathBuf {
    config_dir.join("settings.json")
}

pub fn load(config_dir: &std::path::Path) -> Settings {
    std::fs::read_to_string(settings_path(config_dir))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn store(config_dir: &std::path::Path, settings: &Settings) {
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = crate::store::write_atomic(&settings_path(config_dir), json.as_bytes());
    }
}
