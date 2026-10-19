//! Persistenz im Config-Ordner: `rules.json` (aktives Regelwerk) und
//! `pseudonyms.json` (Pseudonym-Speicher über Dateien, Tarif masked).
//! JSON wird atomar geschrieben (Temp-Datei + rename).

use anonym_core::{Rules, Store};
use std::path::{Path, PathBuf};

pub const RULES_FILE: &str = "rules.json";
pub const STORE_FILE: &str = "pseudonyms.json";

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    anonym_core::formats::write_bytes(bytes, path)
}

pub fn rules_path(config_dir: &Path) -> PathBuf {
    config_dir.join(RULES_FILE)
}

pub fn store_path(config_dir: &Path) -> PathBuf {
    config_dir.join(STORE_FILE)
}

/// Aktives Regelwerk laden. Erster Start oder alte feste Standard-Saat „anonym“: zufällige
/// Saat erzeugen und speichern — eine öffentlich bekannte Saat machte Pseudonyme und die
/// Datumsverschiebung für jeden nachrechenbar. Ein gefüllter Pseudonym-Speicher hängt an
/// seiner Saat; dann bleibt sie (Hinweis im Regelwerk).
pub fn load_rules(config_dir: &Path) -> Rules {
    let mut rules: Rules = std::fs::read(rules_path(config_dir)).ok().and_then(|b| parse_rules(&b).ok()).unwrap_or_default();
    let store_in_use = load_store(config_dir).map(|s| !s.entries.is_empty() && s.seed == rules.seed).unwrap_or(false);
    if rules.seed == anonym_core::pseudo::LEGACY_DEFAULT_SEED && !store_in_use {
        rules.seed = anonym_core::pseudo::random_seed();
        let _ = save_rules(config_dir, &rules);
    }
    rules
}

/// Standard-Regelwerk mit frischer Zufalls-Saat.
pub fn fresh_rules() -> Rules {
    Rules { seed: anonym_core::pseudo::random_seed(), ..Rules::default() }
}

pub fn save_rules(config_dir: &Path, rules: &Rules) -> Result<(), String> {
    let json = serde_json::to_string_pretty(rules).map_err(|e| e.to_string())?;
    write_atomic(&rules_path(config_dir), json.as_bytes())
}

/// Regelwerk aus JSON-Bytes (ein UTF-8-BOM vom Windows-Editor wird übergangen).
pub fn parse_rules(bytes: &[u8]) -> Result<Rules, String> {
    serde_json::from_slice(anonym_core::formats::text::strip_bom(bytes)).map_err(|e| format!("Kein gültiges Regelwerk: {e}"))
}

pub fn load_store(config_dir: &Path) -> Option<Store> {
    std::fs::read(store_path(config_dir)).ok().and_then(|b| serde_json::from_slice(anonym_core::formats::text::strip_bom(&b)).ok())
}

pub fn save_store(config_dir: &Path, store: &Store) -> Result<(), String> {
    let json = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    write_atomic(&store_path(config_dir), json.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("anonym-store-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn legacy_seed_is_replaced_unless_store_depends_on_it() {
        // Erster Start: Zufalls-Saat, gespeichert
        let d = dir("first");
        let r = load_rules(&d);
        assert_ne!(r.seed, anonym_core::pseudo::LEGACY_DEFAULT_SEED);
        assert_eq!(load_rules(&d).seed, r.seed);
        // Alte Standard-Saat ohne Speicher: ersetzt
        let d = dir("legacy");
        save_rules(&d, &Rules::default()).unwrap();
        assert_ne!(load_rules(&d).seed, anonym_core::pseudo::LEGACY_DEFAULT_SEED);
        // Alte Standard-Saat mit gefülltem Speicher: bleibt, sonst wäre der Speicher wertlos
        let d = dir("store");
        save_rules(&d, &Rules::default()).unwrap();
        let mut st = Store { seed: "anonym".into(), world: "de".into(), ..Store::default() };
        st.entries.insert("last|berger".into(), "Vogt".into());
        save_store(&d, &st).unwrap();
        assert_eq!(load_rules(&d).seed, "anonym");
        // Eigene Saat bleibt immer
        let d = dir("own");
        save_rules(&d, &Rules { seed: "meine-saat".into(), ..Rules::default() }).unwrap();
        assert_eq!(load_rules(&d).seed, "meine-saat");
        // Regelwerk/Schlüsseldatei mit UTF-8-BOM (Windows-Editor)
        let json = serde_json::to_vec(&Rules { seed: "bom".into(), ..Rules::default() }).unwrap();
        assert_eq!(parse_rules(&[&[0xEF, 0xBB, 0xBF][..], &json].concat()).unwrap().seed, "bom");
        let d = dir("bom");
        let mut st = Store { seed: "s".into(), world: "de".into(), ..Store::default() };
        st.entries.insert("last|berger".into(), "Vogt".into());
        std::fs::write(store_path(&d), [&[0xEF, 0xBB, 0xBF][..], &serde_json::to_vec(&st).unwrap()].concat()).unwrap();
        assert_eq!(load_store(&d).unwrap().entries.len(), 1);
        for n in ["first", "legacy", "store", "own", "bom"] {
            let _ = std::fs::remove_dir_all(std::env::temp_dir().join(format!("anonym-store-test-{}-{n}", std::process::id())));
        }
    }
}
