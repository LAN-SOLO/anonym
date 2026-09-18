//! Datenmodell: Kategorien, Strategien, Regelwerk, Fundstellen, Entscheidungen.
//! Alles serde-camelCase — der Vertrag steht in `src/api.ts`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Kategorien personenbezogener Angaben. Reihenfolge = Anzeige-Reihenfolge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    Person,
    Address,
    PostalCode,
    Date,
    Email,
    Phone,
    Iban,
    CreditCard,
    TaxId,
    Insurance,
    Plate,
    Ip,
    CustomerId,
}

impl Category {
    pub const ALL: [Category; 13] = [
        Category::Person,
        Category::Address,
        Category::PostalCode,
        Category::Date,
        Category::Email,
        Category::Phone,
        Category::Iban,
        Category::CreditCard,
        Category::TaxId,
        Category::Insurance,
        Category::Plate,
        Category::Ip,
        Category::CustomerId,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Category::Person => "person",
            Category::Address => "address",
            Category::PostalCode => "postal-code",
            Category::Date => "date",
            Category::Email => "email",
            Category::Phone => "phone",
            Category::Iban => "iban",
            Category::CreditCard => "credit-card",
            Category::TaxId => "tax-id",
            Category::Insurance => "insurance",
            Category::Plate => "plate",
            Category::Ip => "ip",
            Category::CustomerId => "customer-id",
        }
    }

    pub fn from_key(k: &str) -> Option<Category> {
        Category::ALL.iter().copied().find(|c| c.key() == k)
    }

    /// Wort im Platzhalter `[PERSON 3]`.
    pub fn placeholder_word(self) -> &'static str {
        match self {
            Category::Person => "PERSON",
            Category::Address => "ADRESSE",
            Category::PostalCode => "PLZ",
            Category::Date => "DATUM",
            Category::Email => "EMAIL",
            Category::Phone => "TELEFON",
            Category::Iban => "IBAN",
            Category::CreditCard => "KARTE",
            Category::TaxId => "STEUER-ID",
            Category::Insurance => "SVNR",
            Category::Plate => "KENNZEICHEN",
            Category::Ip => "IP",
            Category::CustomerId => "ID",
        }
    }

    /// Bei überlappenden Treffern gewinnt die höhere Priorität.
    pub fn priority(self) -> u8 {
        match self {
            Category::Iban => 100,
            Category::CreditCard => 95,
            Category::Email => 94,
            Category::TaxId => 90,
            Category::Insurance => 89,
            Category::Ip => 85,
            Category::Date => 80,
            Category::Address => 75,
            Category::Plate => 70,
            Category::CustomerId => 65,
            Category::Phone => 60,
            Category::PostalCode => 55,
            Category::Person => 50,
        }
    }
}

/// Was mit einem Treffer passiert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Strategy {
    /// Stimmiger Ersatz (Pseudonym, neue Nummer mit gültiger Prüfziffer, verschobenes Datum).
    Pseudonym,
    /// `[PERSON 3]` — Platzhalter mit Zähler je Original.
    Placeholder,
    /// Schwärzen: Balken in Originallänge.
    Redact,
    /// Maskieren: Zeichen ersetzen, optional die letzten n behalten (`XXXX XXXX 1234`).
    Mask,
    /// Ersatzlos entfernen.
    Delete,
}

impl Strategy {
    pub const ALL: [Strategy; 5] = [Strategy::Pseudonym, Strategy::Placeholder, Strategy::Redact, Strategy::Mask, Strategy::Delete];
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CategoryRule {
    pub enabled: bool,
    pub strategy: Strategy,
    /// Zeichen für `Redact` (Standard █).
    pub redact_char: String,
    /// Zeichen für `Mask` (Standard X).
    pub mask_char: String,
    /// Bei `Mask`: so viele Zeichen am Ende sichtbar lassen.
    pub keep_last: usize,
    /// Bei `Redact`: Länge des Originals erhalten (sonst fester Balken).
    pub keep_length: bool,
    /// Vorlage für `Placeholder`: `{cat}` = Kategorie, `{n}` = Zähler.
    pub placeholder: String,
}

impl Default for CategoryRule {
    fn default() -> Self {
        CategoryRule {
            enabled: true,
            strategy: Strategy::Pseudonym,
            redact_char: "█".into(),
            mask_char: "X".into(),
            keep_last: 0,
            keep_length: true,
            placeholder: "[{cat} {n}]".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomPattern {
    pub name: String,
    /// Regulärer Ausdruck (Rust-Regex-Syntax). Der ganze Treffer wird ersetzt.
    pub pattern: String,
    pub category: Category,
}

impl Default for CustomPattern {
    fn default() -> Self {
        CustomPattern { name: String::new(), pattern: String::new(), category: Category::CustomerId }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomWords {
    pub name: String,
    pub category: Category,
    /// Wörter/Phrasen, die immer als Treffer dieser Kategorie gelten (ohne Groß/Klein).
    pub words: Vec<String>,
}

impl Default for CustomWords {
    fn default() -> Self {
        CustomWords { name: String::new(), category: Category::Person, words: Vec::new() }
    }
}

/// Ein Regelwerk — als JSON kopier- und teilbar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Rules {
    pub name: String,
    /// Saat für alle deterministischen Ersetzungen. Gleiche Saat + gleiche Datei = gleiche Ausgabe.
    pub seed: String,
    /// Namenswelt für Pseudonyme (`de`, `en`, `fr`, `es`, `tr`, `ja`, `fantasy`, `scifi`, `medieval`).
    pub world: String,
    /// Datumsverschiebung in Tagen; 0 = aus der Saat ableiten (±1…45 Tage).
    pub date_shift_days: i32,
    pub categories: BTreeMap<Category, CategoryRule>,
    /// Ausnahmeliste: bleibt immer stehen (Firmenname, Produkte, Behörden).
    pub allowlist: Vec<String>,
    pub custom_patterns: Vec<CustomPattern>,
    pub custom_words: Vec<CustomWords>,
    /// Tabellenspalten anhand der Kopfzeile typisieren (CSV/TSV).
    pub column_typing: bool,
}

impl Default for Rules {
    fn default() -> Self {
        let mut categories = BTreeMap::new();
        for c in Category::ALL {
            categories.insert(c, CategoryRule::default());
        }
        Rules {
            name: "Standard".into(),
            seed: "anonym".into(),
            world: "de".into(),
            date_shift_days: 0,
            categories,
            allowlist: Vec::new(),
            custom_patterns: Vec::new(),
            custom_words: Vec::new(),
            column_typing: true,
        }
    }
}

impl Rules {
    pub fn rule(&self, c: Category) -> CategoryRule {
        self.categories.get(&c).cloned().unwrap_or_default()
    }
}

/// Woher ein Treffer stammt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    Dictionary,
    Pattern,
    Checksum,
    Context,
    Column,
    Custom,
}

/// Namensbestandteil eines Personen-Treffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Gender {
    M,
    F,
    U,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NamePart {
    /// Vorname mit Genus.
    First { text: String, gender: Gender },
    Last { text: String },
    /// Anrede, Titel, Adelspartikel — bleibt stehen.
    Keep { text: String },
}

/// Ein Treffer im (virtuellen) Text. Offsets in Bytes (`bstart`/`bend`) für Rust
/// und in UTF-16-Einheiten (`start`/`end`) für die Oberfläche.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: u32,
    pub bstart: usize,
    pub bend: usize,
    pub start: usize,
    pub end: usize,
    pub line: u32,
    pub text: String,
    pub category: Category,
    pub source: Source,
    pub confidence: u8,
    pub replacement: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<NamePart>,
}

/// Entscheidung der Nutzerin je Fundstelle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Decision {
    pub id: u32,
    pub accept: bool,
    pub category: Option<Category>,
    pub replacement: Option<String>,
}

impl Default for Decision {
    fn default() -> Self {
        Decision { id: 0, accept: true, category: None, replacement: None }
    }
}

/// Ergebnis einer Analyse.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    pub path: String,
    pub format: String,
    pub encoding: String,
    pub text: String,
    pub lines: u32,
    pub findings: Vec<Finding>,
    /// Hinweise (z. B. „Word-Datei: Text über Formatierungsläufe geteilt“).
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportEntry {
    pub category: Category,
    pub original: String,
    pub replacement: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub tool: String,
    pub version: String,
    pub created: String,
    pub source: String,
    pub output: String,
    pub format: String,
    pub rules: String,
    pub world: String,
    pub date_shift_days: i32,
    pub replaced: u32,
    pub rejected: u32,
    pub by_category: BTreeMap<String, u32>,
    pub entries: Vec<ReportEntry>,
}
