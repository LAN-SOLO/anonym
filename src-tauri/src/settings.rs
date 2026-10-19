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
    /// Recover-Datei (Schlüssel zur Rückübersetzung) neben die Ausgabe schreiben.
    pub write_recover: bool,
    /// Recover-Dateien mit Passwort verschlüsseln (Passwort wird je Sitzung abgefragt, nie gespeichert).
    pub encrypt_recover: bool,
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
            write_recover: true,
            encrypt_recover: false,
            confirm_overwrite: true,
        }
    }
}

impl Settings {
    /// Werte aus dem WebView auf Gültiges begrenzen. Der Namenszusatz wird Teil eines
    /// Dateinamens und darf keine Pfadtrenner oder unter Windows verbotene Zeichen tragen.
    pub fn sanitized(mut self) -> Settings {
        let d = Settings::default();
        if !matches!(self.language.as_str(), "de" | "en") {
            self.language = d.language.clone();
        }
        if !matches!(self.theme.as_str(), "dark" | "light") {
            self.theme = d.theme.clone();
        }
        if !matches!(self.accent.as_str(), "blue" | "emerald" | "violet" | "amber") {
            self.accent = d.accent.clone();
        }
        let suffix: String = self.output_suffix.chars().filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && !c.is_control()).collect();
        let suffix = suffix.trim().trim_end_matches('.').to_string();
        self.output_suffix = if suffix.is_empty() || suffix.chars().all(|c| c == '.') { d.output_suffix.clone() } else { suffix };
        if !anonym_core::formats::text::FALLBACKS.contains(&self.fallback_encoding.as_str()) {
            self.fallback_encoding = d.fallback_encoding.clone();
        }
        if !self.write_recover {
            self.encrypt_recover = false;
        }
        // „C:“ ist unter Windows der aktuelle Ordner auf C:, nicht das Laufwerk — Stamm meinen
        let dir = self.output_dir.trim();
        let b = dir.as_bytes();
        self.output_dir = if b.len() == 2 && b[1] == b':' && b[0].is_ascii_alphabetic() { format!("{dir}\\") } else { dir.to_string() };
        self
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

pub fn store(config_dir: &std::path::Path, settings: &Settings) -> Result<(), String> {
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    crate::store::write_atomic(&settings_path(config_dir), json.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_suffix_and_enums() {
        let s = Settings { output_suffix: "/../x".into(), theme: "neon".into(), fallback_encoding: "utf-7".into(), ..Settings::default() }.sanitized();
        assert_eq!(s.output_suffix, "..x".trim_end_matches('.'));
        assert_eq!(s.theme, "dark");
        assert_eq!(s.fallback_encoding, "windows-1252");
        assert_eq!(Settings { output_suffix: "..".into(), ..Settings::default() }.sanitized().output_suffix, ".anonym");
        assert_eq!(Settings { output_suffix: "_anon".into(), ..Settings::default() }.sanitized().output_suffix, "_anon");
        assert_eq!(Settings { output_suffix: "C:\\a|b".into(), ..Settings::default() }.sanitized().output_suffix, "Cab");
        assert_eq!(Settings { output_dir: " D: ".into(), ..Settings::default() }.sanitized().output_dir, "D:\\");
        assert_eq!(Settings { output_dir: "D:\\Export".into(), ..Settings::default() }.sanitized().output_dir, "D:\\Export");
        assert_eq!(Settings { output_dir: "/Users/a b/Ausgabe".into(), ..Settings::default() }.sanitized().output_dir, "/Users/a b/Ausgabe");
    }
}
