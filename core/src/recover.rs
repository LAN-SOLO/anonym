//! Recover-Datei: Schlüssel zur Rückübersetzung einer anonymisierten Datei — auch
//! nachdem sie weiterbearbeitet oder in ein anderes Format gebracht wurde. Die
//! Rückübersetzung arbeitet wortweise (Ersatz → Original, längste zuerst) auf dem
//! virtuellen Text der bearbeiteten Datei und schreibt sie im selben Format zurück.

use crate::model::{Category, Report};
use crate::pseudo::{match_case, Pseudonymizer};
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit, OsRng};
use chacha20poly1305::{AeadCore, XChaCha20Poly1305, XNonce};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverEntry {
    /// Kategorie (Bericht) oder Speicher-Art (`first-f`, `last`, `email`, …) für Teil-Namen
    pub kind: String,
    pub original: String,
    pub replacement: String,
}

/// Inhalt einer Recover-Datei (Klartext).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RecoverFile {
    pub tool: String,
    pub version: String,
    pub created: String,
    pub source: String,
    pub output: String,
    pub seed: String,
    pub world: String,
    pub date_shift_days: i32,
    /// Vollständige Ersetzungen aus dem Bericht (Original → Ersatz je Kategorie).
    pub entries: Vec<RecoverEntry>,
    /// Teil-Zuordnungen aus dem Pseudonym-Speicher (Vorname, Nachname, Straße …), damit
    /// auch getrennt weiterverwendete Namensteile zurückfinden.
    pub parts: Vec<RecoverEntry>,
}

impl Default for RecoverFile {
    fn default() -> Self {
        RecoverFile {
            tool: "anonym".into(),
            version: crate::VERSION.into(),
            created: String::new(),
            source: String::new(),
            output: String::new(),
            seed: String::new(),
            world: String::new(),
            date_shift_days: 0,
            entries: Vec::new(),
            parts: Vec::new(),
        }
    }
}

/// Verschlüsselte Hülle (XChaCha20-Poly1305, Schlüssel per Argon2id aus dem Passwort).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Sealed {
    tool: String,
    encrypted: bool,
    kdf: String,
    salt: String,
    nonce: String,
    ciphertext: String,
}

/// Recover-Datei aus Bericht und Pseudonym-Lauf bauen.
pub fn build(report: &Report, pseudo: &Pseudonymizer, seed: &str, world: &str) -> RecoverFile {
    let entries = report
        .entries
        .iter()
        .filter(|e| !e.replacement.is_empty() && e.replacement != e.original)
        .map(|e| RecoverEntry { kind: e.category.key().to_string(), original: e.original.clone(), replacement: e.replacement.clone() })
        .collect();
    let mut parts: Vec<RecoverEntry> = pseudo
        .touched
        .iter()
        .filter_map(|k| pseudo.store.entries.get(k).map(|v| (k, v)))
        .filter_map(|(k, v)| {
            let (kind, original) = k.split_once('|')?;
            if v.is_empty() || v.eq_ignore_ascii_case(original) {
                return None;
            }
            Some(RecoverEntry { kind: kind.to_string(), original: original.to_string(), replacement: v.clone() })
        })
        .collect();
    parts.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.original.cmp(&b.original)));
    RecoverFile {
        created: report.created.clone(),
        source: report.source.clone(),
        output: report.output.clone(),
        seed: seed.to_string(),
        world: world.to_string(),
        date_shift_days: report.date_shift_days,
        entries,
        parts,
        ..RecoverFile::default()
    }
}

fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    argon2::Argon2::default().hash_password_into(password.as_bytes(), salt, &mut key).map_err(|e| format!("Schlüsselableitung fehlgeschlagen: {e}"))?;
    Ok(key)
}

/// Als JSON serialisieren — mit Passwort verschlüsselt, sonst Klartext.
pub fn seal(file: &RecoverFile, password: Option<&str>) -> Result<Vec<u8>, String> {
    let plain = serde_json::to_vec_pretty(file).map_err(|e| e.to_string())?;
    let Some(pw) = password.filter(|p| !p.is_empty()) else {
        return Ok(plain);
    };
    let b64 = base64::engine::general_purpose::STANDARD;
    let mut salt = [0u8; 16];
    use chacha20poly1305::aead::rand_core::RngCore;
    OsRng.fill_bytes(&mut salt);
    let key = derive_key(pw, &salt)?;
    let cipher = XChaCha20Poly1305::new((&key).into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ct = cipher.encrypt(&nonce, plain.as_ref()).map_err(|_| "Verschlüsselung fehlgeschlagen".to_string())?;
    let sealed = Sealed { tool: "anonym".into(), encrypted: true, kdf: "argon2id".into(), salt: b64.encode(salt), nonce: b64.encode(nonce), ciphertext: b64.encode(ct) };
    serde_json::to_vec_pretty(&sealed).map_err(|e| e.to_string())
}

/// Ist die Datei verschlüsselt?
pub fn is_sealed(bytes: &[u8]) -> bool {
    serde_json::from_slice::<Sealed>(bytes).map(|s| s.encrypted).unwrap_or(false)
}

/// Recover-Datei lesen (Klartext oder mit Passwort entschlüsseln).
pub fn open(bytes: &[u8], password: Option<&str>) -> Result<RecoverFile, String> {
    if let Ok(sealed) = serde_json::from_slice::<Sealed>(bytes) {
        if sealed.encrypted {
            let pw = password.filter(|p| !p.is_empty()).ok_or("PASSWORD_REQUIRED")?;
            let b64 = base64::engine::general_purpose::STANDARD;
            let salt = b64.decode(&sealed.salt).map_err(|_| "Recover-Datei beschädigt (salt)")?;
            let nonce = b64.decode(&sealed.nonce).map_err(|_| "Recover-Datei beschädigt (nonce)")?;
            let ct = b64.decode(&sealed.ciphertext).map_err(|_| "Recover-Datei beschädigt (ciphertext)")?;
            let key = derive_key(pw, &salt)?;
            let cipher = XChaCha20Poly1305::new((&key).into());
            let plain = cipher.decrypt(XNonce::from_slice(&nonce), ct.as_ref()).map_err(|_| "PASSWORD_WRONG".to_string())?;
            return serde_json::from_slice(&plain).map_err(|e| format!("Recover-Datei unlesbar: {e}"));
        }
    }
    let f: RecoverFile = serde_json::from_slice(bytes).map_err(|e| format!("Keine gültige Recover-Datei: {e}"))?;
    if f.tool != "anonym" {
        return Err("Keine gültige Recover-Datei".into());
    }
    Ok(f)
}

/// Ergebnis einer Rückübersetzung.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverResult {
    pub output: String,
    pub format: String,
    /// Anzahl zurückübersetzter Stellen
    pub restored: u32,
    /// Ersatzwerte aus der Recover-Datei, die in der Datei nicht (mehr) vorkamen
    pub not_found: u32,
    /// Mehrdeutige Ersatzwerte (z. B. Balken, Platzhalter mit gleichem Text), die übersprungen wurden
    pub ambiguous: u32,
    pub by_kind: BTreeMap<String, u32>,
}

/// Umkehr-Zuordnung: Ersatz (klein) → (Original, Art). Mehrdeutige Ersatzwerte fallen weg.
fn reverse_map(key: &RecoverFile) -> (HashMap<String, (String, String)>, u32) {
    let mut map: HashMap<String, (String, String)> = HashMap::new();
    let mut conflicts: HashSet<String> = HashSet::new();
    for e in key.entries.iter().chain(key.parts.iter()) {
        let r = e.replacement.trim();
        if r.is_empty() || r.chars().all(|c| !c.is_alphanumeric()) {
            continue; // Balken, Sterne — nicht umkehrbar
        }
        // Aufgefüllte Ersatzwerte (feste Spaltenbreiten) zusätzlich mit ihrem Füll-Leerraum,
        // damit die Spaltenbreite exakt zurückkommt; die getrimmte Form fängt Bearbeitungen ab.
        let mut variants = vec![r.to_string()];
        if e.replacement != r {
            variants.push(e.replacement.clone());
        }
        for v in variants {
            let k = v.to_lowercase();
            match map.get(&k) {
                Some((o, _)) if !o.eq_ignore_ascii_case(&e.original) => {
                    conflicts.insert(k);
                }
                Some(_) => {}
                None => {
                    map.insert(k, (e.original.clone(), e.kind.clone()));
                }
            }
        }
    }
    for c in &conflicts {
        map.remove(c);
    }
    (map, conflicts.len() as u32)
}

/// Text zurückübersetzen: eine Alternation aller Ersatzwerte (längste zuerst), ein Durchlauf.
pub fn restore_text(text: &str, key: &RecoverFile) -> (String, RecoverResult) {
    let (map, ambiguous) = reverse_map(key);
    let mut result = RecoverResult { output: String::new(), format: String::new(), restored: 0, not_found: 0, ambiguous, by_kind: BTreeMap::new() };
    if map.is_empty() {
        result.not_found = 0;
        return (text.to_string(), result);
    }
    let mut alts: Vec<&String> = map.keys().collect();
    alts.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()).then(a.cmp(b)));
    let pattern: String = alts
        .iter()
        .map(|a| {
            let esc = regex::escape(a);
            let lb = if a.chars().next().map(|c| c.is_alphanumeric()).unwrap_or(false) { r"\b" } else { "" };
            let rb = if a.chars().last().map(|c| c.is_alphanumeric()).unwrap_or(false) { r"\b" } else { "" };
            format!("{lb}{esc}{rb}")
        })
        .collect::<Vec<_>>()
        .join("|");
    let re = match Regex::new(&format!("(?i)(?:{pattern})")) {
        Ok(r) => r,
        Err(_) => return (text.to_string(), result),
    };
    let mut seen: HashSet<String> = HashSet::new();
    let out = re.replace_all(text, |c: &regex::Captures| {
        let found = c.get(0).unwrap().as_str();
        let k = found.to_lowercase();
        match map.get(&k) {
            Some((orig, kind)) => {
                seen.insert(k);
                result.restored += 1;
                *result.by_kind.entry(kind.clone()).or_insert(0) += 1;
                // Groß-/Kleinschreibung der Fundstelle übernehmen (nur bei Wörtern)
                if found.chars().any(|c| c.is_alphabetic()) {
                    match_case(found, orig)
                } else {
                    orig.clone()
                }
            }
            None => found.to_string(),
        }
    });
    let distinct: HashSet<String> = map.keys().map(|k| k.trim().to_string()).collect();
    let seen_trim: HashSet<String> = seen.iter().map(|k| k.trim().to_string()).collect();
    result.not_found = distinct.len().saturating_sub(seen_trim.len()) as u32;
    (out.into_owned(), result)
}

/// Kategorie-Label für die Anzeige (Speicher-Arten auf Kategorien abbilden).
pub fn kind_category(kind: &str) -> Option<Category> {
    Category::from_key(kind).or(match kind {
        "first-f" | "first-m" | "first-u" | "last" | "word" => Some(Category::Person),
        "email" | "email-local" => Some(Category::Email),
        "phone" => Some(Category::Phone),
        "iban" => Some(Category::Iban),
        "card" => Some(Category::CreditCard),
        "taxid" => Some(Category::TaxId),
        "svnr" => Some(Category::Insurance),
        "plate" => Some(Category::Plate),
        "ip" => Some(Category::Ip),
        "id" => Some(Category::CustomerId),
        "plz" => Some(Category::PostalCode),
        "address" | "street" => Some(Category::Address),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> RecoverFile {
        RecoverFile {
            entries: vec![
                RecoverEntry { kind: "person".into(), original: "Anna Berger".into(), replacement: "Paula Rademacher".into() },
                RecoverEntry { kind: "date".into(), original: "14.03.2024".into(), replacement: "23.04.2024".into() },
                RecoverEntry { kind: "customer-id".into(), original: "10482".into(), replacement: "84292".into() },
                RecoverEntry { kind: "person".into(), original: "Max Roth".into(), replacement: "████████".into() },
            ],
            parts: vec![
                RecoverEntry { kind: "first-f".into(), original: "anna".into(), replacement: "Paula".into() },
                RecoverEntry { kind: "last".into(), original: "berger".into(), replacement: "Rademacher".into() },
                RecoverEntry { kind: "email".into(), original: "anna.berger@example.org".into(), replacement: "paula.rademacher@example.org".into() },
            ],
            ..RecoverFile::default()
        }
    }

    #[test]
    fn restores_edited_text() {
        let edited = "Frau RADEMACHER (Paula Rademacher) rief am 23.04.2024 an, Kd-Nr 84292, neu: Kd-Nr 99999.\nMail: paula.rademacher@example.org — Paulas Kollegin Rademacher-Vogt.\n";
        let (out, r) = restore_text(edited, &key());
        assert_eq!(out, "Frau BERGER (Anna Berger) rief am 14.03.2024 an, Kd-Nr 10482, neu: Kd-Nr 99999.\nMail: anna.berger@example.org — Paulas Kollegin Berger-Vogt.\n");
        assert_eq!(r.restored, 6);
        // „Paula“ allein kam nur als „Paulas“ vor (Wortgrenze) — bleibt unberührt und gilt als nicht gefunden
        assert_eq!(r.not_found, 1);
        assert_eq!(r.ambiguous, 0);
        assert_eq!(r.by_kind["person"], 1);
    }

    #[test]
    fn seal_and_open() {
        let k = key();
        let plain = seal(&k, None).unwrap();
        assert!(!is_sealed(&plain));
        assert_eq!(open(&plain, None).unwrap(), k);
        let enc = seal(&k, Some("geheim")).unwrap();
        assert!(is_sealed(&enc));
        assert_eq!(open(&enc, None).unwrap_err(), "PASSWORD_REQUIRED");
        assert_eq!(open(&enc, Some("falsch")).unwrap_err(), "PASSWORD_WRONG");
        assert_eq!(open(&enc, Some("geheim")).unwrap(), k);
        assert!(!String::from_utf8_lossy(&enc).contains("Anna"));
    }
}
