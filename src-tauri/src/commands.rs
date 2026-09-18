//! Tauri-Kommandos — der Vertrag steht in `src/api.ts`.

use crate::settings::{self, Settings};
use crate::state::AppState;
use crate::store;
use anonym_core::export::Target;
use anonym_core::{Analysis, Decision, Options, Report, Rules, WORLD_IDS};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, State};

type Shared = Arc<AppState>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfoDto {
    pub version: String,
    pub notes: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub output: String,
    pub report_path: Option<String>,
    pub report: Report,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTarget {
    pub ext: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreInfo {
    pub path: String,
    pub entries: usize,
    pub seed: String,
    pub world: String,
}

fn options(st: &Shared) -> Options {
    let s = st.settings_clone();
    Options {
        fallback_encoding: s.fallback_encoding.clone(),
        store: if s.masked && s.use_store { store::load_store(&st.config_dir) } else { None },
    }
}

// --- Einstellungen ----------------------------------------------------------

#[tauri::command]
pub fn get_settings(st: State<'_, Shared>) -> Settings {
    st.settings_clone()
}

#[tauri::command]
pub fn set_settings(st: State<'_, Shared>, settings: Settings) {
    settings::store(&st.config_dir, &settings);
    *st.settings.lock().unwrap() = settings;
}

#[tauri::command]
pub fn data_path(st: State<'_, Shared>) -> String {
    st.config_dir.to_string_lossy().into_owned()
}

// --- Regelwerk --------------------------------------------------------------

#[tauri::command]
pub fn get_rules(st: State<'_, Shared>) -> Rules {
    st.rules_clone()
}

#[tauri::command]
pub fn set_rules(st: State<'_, Shared>, rules: Rules) -> Result<(), String> {
    store::save_rules(&st.config_dir, &rules)?;
    *st.rules.lock().unwrap() = rules;
    Ok(())
}

#[tauri::command]
pub fn default_rules() -> Rules {
    Rules::default()
}

#[tauri::command]
pub fn import_rules(st: State<'_, Shared>, path: String) -> Result<Rules, String> {
    let rules = store::read_rules(Path::new(&path))?;
    store::save_rules(&st.config_dir, &rules)?;
    *st.rules.lock().unwrap() = rules.clone();
    Ok(rules)
}

#[tauri::command]
pub fn export_rules(rules: Rules, path: String) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&rules).map_err(|e| e.to_string())?;
    store::write_atomic(Path::new(&path), json.as_bytes())
}

#[tauri::command]
pub fn list_worlds() -> Vec<String> {
    WORLD_IDS.iter().map(|s| s.to_string()).collect()
}

// --- Analyse & Anwendung ----------------------------------------------------

#[tauri::command]
pub async fn analyze_file(st: State<'_, Shared>, path: String, rules: Rules) -> Result<Analysis, String> {
    let st = st.inner().clone();
    let opts = options(&st);
    tauri::async_runtime::spawn_blocking(move || anonym_core::analyze(Path::new(&path), &rules, &st.dicts, &opts))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn suggest_output(st: State<'_, Shared>, path: String, ext: Option<String>) -> String {
    let s = st.settings_clone();
    let dir = if s.output_dir.trim().is_empty() { None } else { Some(PathBuf::from(s.output_dir.trim())) };
    anonym_core::suggest_output_as(Path::new(&path), dir.as_deref(), &s.output_suffix, ext.as_deref()).to_string_lossy().into_owned()
}

#[tauri::command]
pub fn export_targets() -> Vec<ExportTarget> {
    Target::ALL.iter().map(|t| ExportTarget { ext: t.ext().into(), label: t.label().into() }).collect()
}

#[tauri::command]
pub async fn apply_file(st: State<'_, Shared>, path: String, rules: Rules, decisions: Vec<Decision>, output: String) -> Result<ApplyResult, String> {
    let st = st.inner().clone();
    let opts = options(&st);
    let s = st.settings_clone();
    tauri::async_runtime::spawn_blocking(move || {
        let src = Path::new(&path);
        let out = Path::new(&output);
        if src == out {
            return Err("Ausgabe darf die Quelle nicht überschreiben".to_string());
        }
        let applied = anonym_core::apply_file(src, &rules, &st.dicts, &opts, &decisions, out)?;
        if s.masked && s.use_store {
            store::save_store(&st.config_dir, &applied.store)?;
        }
        let report_path = if s.write_report {
            let rp = report_path_for(out);
            let json = serde_json::to_string_pretty(&applied.report).map_err(|e| e.to_string())?;
            store::write_atomic(&rp, json.as_bytes())?;
            Some(rp.to_string_lossy().into_owned())
        } else {
            None
        };
        Ok(ApplyResult { output: applied.output.to_string_lossy().into_owned(), report_path, report: applied.report })
    })
    .await
    .map_err(|e| e.to_string())?
}

fn report_path_for(out: &Path) -> PathBuf {
    let stem = out.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "ausgabe".into());
    out.with_file_name(format!("{stem}.report.json"))
}

// --- Pseudonym-Speicher -----------------------------------------------------

#[tauri::command]
pub fn store_info(st: State<'_, Shared>) -> StoreInfo {
    let p = store::store_path(&st.config_dir);
    let s = store::load_store(&st.config_dir).unwrap_or_default();
    StoreInfo { path: p.to_string_lossy().into_owned(), entries: s.entries.len(), seed: s.seed, world: s.world }
}

#[tauri::command]
pub fn store_clear(st: State<'_, Shared>) -> Result<(), String> {
    let p = store::store_path(&st.config_dir);
    if p.exists() {
        std::fs::remove_file(&p).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn store_export(st: State<'_, Shared>, path: String) -> Result<(), String> {
    let s = store::load_store(&st.config_dir).unwrap_or_default();
    let json = serde_json::to_string_pretty(&s).map_err(|e| e.to_string())?;
    store::write_atomic(Path::new(&path), json.as_bytes())
}

#[tauri::command]
pub fn store_import(st: State<'_, Shared>, path: String) -> Result<StoreInfo, String> {
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Schlüsseldatei nicht lesbar: {e}"))?;
    let s: anonym_core::Store = serde_json::from_str(&text).map_err(|e| format!("Keine gültige Schlüsseldatei: {e}"))?;
    store::save_store(&st.config_dir, &s)?;
    Ok(store_info(st))
}

// --- System -----------------------------------------------------------------

#[tauri::command]
pub fn path_exists(path: String) -> bool {
    Path::new(&path).exists()
}

#[tauri::command]
pub fn open_path(app: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    if path.starts_with("http://") || path.starts_with("https://") {
        app.opener().open_url(path, None::<&str>).map_err(|e| e.to_string())
    } else {
        app.opener().open_path(path, None::<&str>).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateInfoDto>, String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => Ok(Some(UpdateInfoDto {
            version: update.version.clone(),
            notes: update.body.clone(),
            date: update.date.map(|d| d.to_string()),
        })),
        Ok(None) => Ok(None),
        Err(e) => Err(format!("Update-Prüfung fehlgeschlagen: {e}")),
    }
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("Update-Prüfung fehlgeschlagen: {e}"))?
        .ok_or("Kein Update verfügbar")?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| format!("Update fehlgeschlagen: {e}"))?;
    app.restart();
}
