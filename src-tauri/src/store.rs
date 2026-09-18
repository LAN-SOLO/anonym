//! Persistenz im Config-Ordner: `rules.json` (aktives Regelwerk) und
//! `pseudonyms.json` (Pseudonym-Speicher über Dateien, Tarif masked).
//! JSON wird atomar geschrieben (Temp-Datei + rename).

use anonym_core::{Rules, Store};
use std::path::{Path, PathBuf};

pub const RULES_FILE: &str = "rules.json";
pub const STORE_FILE: &str = "pseudonyms.json";

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Ordner anlegen fehlgeschlagen: {e}"))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(|e| format!("Schreiben fehlgeschlagen: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("Umbenennen fehlgeschlagen: {e}"))
}

pub fn rules_path(config_dir: &Path) -> PathBuf {
    config_dir.join(RULES_FILE)
}

pub fn store_path(config_dir: &Path) -> PathBuf {
    config_dir.join(STORE_FILE)
}

pub fn load_rules(config_dir: &Path) -> Rules {
    std::fs::read_to_string(rules_path(config_dir))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_rules(config_dir: &Path, rules: &Rules) -> Result<(), String> {
    let json = serde_json::to_string_pretty(rules).map_err(|e| e.to_string())?;
    write_atomic(&rules_path(config_dir), json.as_bytes())
}

pub fn read_rules(path: &Path) -> Result<Rules, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("Regelwerk nicht lesbar: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("Kein gültiges Regelwerk: {e}"))
}

pub fn load_store(config_dir: &Path) -> Option<Store> {
    std::fs::read_to_string(store_path(config_dir)).ok().and_then(|s| serde_json::from_str(&s).ok())
}

pub fn save_store(config_dir: &Path, store: &Store) -> Result<(), String> {
    let json = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    write_atomic(&store_path(config_dir), json.as_bytes())
}
