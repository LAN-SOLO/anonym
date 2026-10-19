//! Tauri-Kommandos — der Vertrag steht in `src/api.ts`.

use crate::settings::{self, Settings};
use crate::state::AppState;
use crate::store;
use anonym_core::export::Target;
use anonym_core::recover::{self, RecoverResult};
use anonym_core::formats::io_text;
use anonym_core::formats::text::strip_bom;
use anonym_core::{Analysis, Decision, Options, ReplacementPreview, Report, Rules, WORLD_IDS};
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
    pub recover_path: Option<String>,
    pub report: Report,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverInfo {
    pub encrypted: bool,
    pub source: Option<String>,
    pub output: Option<String>,
    pub created: Option<String>,
    pub entries: usize,
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

/// Bleibt synchron (Haupt-Thread): Aufrufe werden so in Reihenfolge geschrieben. Ein
/// Schreibfehler geht an die Oberfläche, statt still verloren zu gehen.
#[tauri::command]
pub fn set_settings(st: State<'_, Shared>, settings: Settings) -> Result<(), String> {
    let settings = settings.sanitized();
    *st.settings.lock().unwrap() = settings.clone();
    settings::store(&st.config_dir, &settings)
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
    store::fresh_rules()
}

/// Neue zufällige Saat für das Regelwerk.
#[tauri::command]
pub fn new_seed() -> String {
    anonym_core::pseudo::random_seed()
}

// Datei-Kommandos laufen mit `async` nicht auf dem Haupt-Thread (bis 64 MB JSON).
#[tauri::command(async)]
pub fn import_rules(st: State<'_, Shared>, path: String) -> Result<Rules, String> {
    let bytes = read_small(Path::new(&path), "Regelwerk")?;
    let rules = store::parse_rules(&bytes)?;
    store::save_rules(&st.config_dir, &rules)?;
    *st.rules.lock().unwrap() = rules.clone();
    Ok(rules)
}

#[tauri::command(async)]
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

/// Ersatz für Fundstellen mit geänderter Kategorie (Vorschau in Tabelle und Reiter „Vorschau“).
#[tauri::command]
pub async fn preview_replacements(st: State<'_, Shared>, path: String, rules: Rules, decisions: Vec<Decision>) -> Result<Vec<ReplacementPreview>, String> {
    let st = st.inner().clone();
    let opts = options(&st);
    tauri::async_runtime::spawn_blocking(move || anonym_core::preview_replacements(Path::new(&path), &rules, &st.dicts, &opts, &decisions))
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
pub async fn apply_file(st: State<'_, Shared>, path: String, rules: Rules, decisions: Vec<Decision>, output: String, password: Option<String>) -> Result<ApplyResult, String> {
    let st = st.inner().clone();
    let opts = options(&st);
    let s = st.settings_clone();
    tauri::async_runtime::spawn_blocking(move || {
        let src = Path::new(&path);
        let out = Path::new(&output);
        // Weder Ausgabe noch Bericht noch Recover-Datei dürfen die Quelle treffen
        let report_target = report_path_for(out);
        let recover_target = anonym_core::recover_path_for(out);
        for target in [out, report_target.as_path(), recover_target.as_path()] {
            if same_file(src, target) {
                return Err("Ausgabe darf die Quelle nicht überschreiben".to_string());
            }
        }
        if s.write_recover && s.encrypt_recover && password.as_deref().map(str::is_empty).unwrap_or(true) {
            return Err("PASSWORD_REQUIRED".into());
        }
        let applied = anonym_core::apply_file(src, &rules, &st.dicts, &opts, &decisions, out)?;
        if s.masked && s.use_store {
            store::save_store(&st.config_dir, &applied.store)?;
        }
        let report_path = if s.write_report {
            let rp = report_target;
            let json = serde_json::to_string_pretty(&applied.report).map_err(|e| e.to_string())?;
            store::write_atomic(&rp, json.as_bytes())?;
            Some(rp.to_string_lossy().into_owned())
        } else {
            None
        };
        let recover_path = if s.write_recover {
            let rp = recover_target;
            let pw = if s.encrypt_recover { password.as_deref().filter(|p| !p.is_empty()) } else { None };
            let bytes = recover::seal(&applied.recover, pw)?;
            store::write_atomic(&rp, &bytes)?;
            Some(rp.to_string_lossy().into_owned())
        } else {
            None
        };
        Ok(ApplyResult { output: applied.output.to_string_lossy().into_owned(), report_path, recover_path, report: applied.report })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Zeigen zwei Pfade auf dieselbe Datei? Vorhandene Dateien über den kanonischen Pfad
/// (Symlinks, `..`, unter Windows Laufwerksbuchstaben und `/` vs. `\\`); sonst über den
/// kanonischen Ordner + Dateinamen. Windows und macOS vergleichen ohne Groß-/Kleinschreibung,
/// weil ihre Standard-Dateisysteme das auch tun.
pub(crate) fn same_file(a: &Path, b: &Path) -> bool {
    fn norm(p: &Path) -> Option<PathBuf> {
        if let Ok(c) = std::fs::canonicalize(p) {
            return Some(c);
        }
        let parent = p.parent().filter(|x| !x.as_os_str().is_empty()).unwrap_or(Path::new("."));
        Some(std::fs::canonicalize(parent).ok()?.join(p.file_name()?))
    }
    let (Some(x), Some(y)) = (norm(a), norm(b)) else { return a == b };
    if cfg!(any(windows, target_os = "macos")) {
        x.to_string_lossy().to_lowercase() == y.to_string_lossy().to_lowercase()
    } else {
        x == y
    }
}

fn report_path_for(out: &Path) -> PathBuf {
    let stem = out.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "ausgabe".into());
    out.with_file_name(format!("{stem}.report.json"))
}

/// JSON-Dateien (Recover-Datei, Schlüsseldatei, Regelwerk) mit Größenbremse lesen.
pub(crate) fn read_small(path: &Path, what: &str) -> Result<Vec<u8>, String> {
    const MAX: u64 = 64 * 1024 * 1024;
    let meta = std::fs::metadata(path).map_err(|e| format!("{what} nicht lesbar: {}", io_text(&e)))?;
    if !meta.is_file() || meta.len() > MAX {
        return Err(format!("{what}: keine Datei oder größer als 64 MB"));
    }
    std::fs::read(path).map(|b| strip_bom(&b).to_vec()).map_err(|e| format!("{what} nicht lesbar: {}", io_text(&e)))
}

// --- Rückübersetzung ---------------------------------------------------------

#[tauri::command(async)]
pub fn recover_info(path: String) -> Result<RecoverInfo, String> {
    let bytes = read_small(Path::new(&path), "Recover-Datei")?;
    if recover::is_sealed(&bytes) {
        return Ok(RecoverInfo { encrypted: true, source: None, output: None, created: None, entries: 0 });
    }
    let f = recover::open(&bytes, None)?;
    Ok(RecoverInfo { encrypted: false, source: Some(f.source), output: Some(f.output), created: Some(f.created), entries: f.entries.len() + f.parts.len() })
}

#[tauri::command]
pub fn suggest_recovered(path: String, ext: Option<String>) -> String {
    anonym_core::suggest_recovered(Path::new(&path), ext.as_deref()).to_string_lossy().into_owned()
}

#[tauri::command]
pub fn suggest_recover_key(path: String) -> String {
    // Zur bearbeiteten Datei passende Recover-Datei raten: <stem ohne .recovered>.recover.json daneben
    let p = Path::new(&path);
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    p.with_file_name(format!("{stem}.recover.json")).to_string_lossy().into_owned()
}

#[tauri::command]
pub async fn recover_file(st: State<'_, Shared>, path: String, key_path: String, password: Option<String>, output: String) -> Result<RecoverResult, String> {
    let st = st.inner().clone();
    let opts = options(&st);
    tauri::async_runtime::spawn_blocking(move || {
        if same_file(Path::new(&path), Path::new(&output)) {
            return Err("Ausgabe darf die Quelle nicht überschreiben".to_string());
        }
        if same_file(Path::new(&key_path), Path::new(&output)) {
            return Err("Ausgabe darf die Recover-Datei nicht überschreiben".to_string());
        }
        let bytes = read_small(Path::new(&key_path), "Recover-Datei")?;
        let key = recover::open(&bytes, password.as_deref())?;
        anonym_core::recover_file(Path::new(&path), &key, &opts, Path::new(&output))
    })
    .await
    .map_err(|e| e.to_string())?
}

// --- Pseudonym-Speicher -----------------------------------------------------

#[tauri::command(async)]
pub fn store_info(st: State<'_, Shared>) -> StoreInfo {
    let p = store::store_path(&st.config_dir);
    let s = store::load_store(&st.config_dir).unwrap_or_default();
    StoreInfo { path: p.to_string_lossy().into_owned(), entries: s.entries.len(), seed: s.seed, world: s.world }
}

#[tauri::command]
pub fn store_clear(st: State<'_, Shared>) -> Result<(), String> {
    let p = store::store_path(&st.config_dir);
    match std::fs::remove_file(&p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("Löschen fehlgeschlagen: {}", io_text(&e))),
    }
}

#[tauri::command(async)]
pub fn store_export(st: State<'_, Shared>, path: String) -> Result<(), String> {
    let s = store::load_store(&st.config_dir).unwrap_or_default();
    let json = serde_json::to_string_pretty(&s).map_err(|e| e.to_string())?;
    store::write_atomic(Path::new(&path), json.as_bytes())
}

#[tauri::command(async)]
pub fn store_import(st: State<'_, Shared>, path: String) -> Result<StoreInfo, String> {
    let bytes = read_small(Path::new(&path), "Schlüsseldatei")?;
    let s: anonym_core::Store = serde_json::from_slice(&bytes).map_err(|e| format!("Keine gültige Schlüsseldatei: {e}"))?;
    if s.seed.trim().is_empty() {
        return Err("Keine gültige Schlüsseldatei: Saat fehlt".into());
    }
    store::save_store(&st.config_dir, &s)?;
    // Der Speicher hängt an seiner Saat und Namenswelt: ins aktive Regelwerk übernehmen,
    // sonst würde er beim nächsten Lauf als „passt nicht“ geleert.
    {
        let mut rules = st.rules.lock().unwrap();
        rules.seed = s.seed.clone();
        if WORLD_IDS.contains(&s.world.as_str()) {
            rules.world = s.world.clone();
        }
        store::save_rules(&st.config_dir, &rules)?;
    }
    Ok(store_info(st))
}

// --- System -----------------------------------------------------------------

#[tauri::command]
pub fn path_exists(path: String) -> bool {
    Path::new(&path).exists()
}

/// Websites, die die App öffnen darf (Einstellungen → App).
const ALLOWED_URLS: [&str; 2] = ["https://lan-solo.com/", "https://github.com/LAN-SOLO/"];

/// Ordner im Dateimanager oder eine der festen Websites öffnen. Bewusst keine Dateien und
/// keine beliebigen URLs: Das WebView soll nichts starten können, was ausführbar ist.
#[tauri::command]
pub fn open_path(app: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    if path.starts_with("http://") || path.starts_with("https://") {
        if !ALLOWED_URLS.iter().any(|u| path.starts_with(u)) {
            return Err("Adresse nicht erlaubt".into());
        }
        return app.opener().open_url(path, None::<&str>).map_err(|e| e.to_string());
    }
    let p = Path::new(&path);
    if !p.is_dir() {
        return Err("Nur Ordner lassen sich öffnen".into());
    }
    app.opener().open_path(path, None::<&str>).map_err(|e| e.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_file_detects_aliases() {
        let dir = std::env::temp_dir().join(format!("anonym-same-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        let f = dir.join("brief.txt");
        std::fs::write(&f, "x").unwrap();
        assert!(same_file(&f, &dir.join("sub").join("..").join("brief.txt")));
        assert!(!same_file(&f, &dir.join("brief.anonym.txt")));
        // Noch nicht existierende Ausgabe im selben Ordner
        assert!(same_file(&dir.join("neu.txt"), &dir.join("sub").join("..").join("neu.txt")));
        if cfg!(any(windows, target_os = "macos")) {
            assert!(same_file(&f, &dir.join("BRIEF.TXT")));
        }
        assert_eq!(report_path_for(&dir.join("a.anonym.txt")).file_name().unwrap(), "a.anonym.report.json");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
