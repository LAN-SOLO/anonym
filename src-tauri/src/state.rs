use crate::settings::Settings;
use anonym_core::{Dictionaries, Rules};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct AppState {
    pub settings: Mutex<Settings>,
    /// Aktives Regelwerk (persistiert als rules.json im Config-Ordner).
    pub rules: Mutex<Rules>,
    pub dicts: Dictionaries,
    pub config_dir: PathBuf,
}

impl AppState {
    pub fn settings_clone(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }
    pub fn rules_clone(&self) -> Rules {
        self.rules.lock().unwrap().clone()
    }
}
