//! Erkennung: Wörterbücher, Muster, Prüfziffern, Kontext (Anrede, Beschriftung),
//! Tabellenspalten und eigene Regeln. Liefert rohe Treffer mit Byte-Offsets;
//! Überlappungen werden nach Priorität aufgelöst.

use crate::checksum;
use crate::csv;
use crate::dates;
use crate::dict::Dictionaries;
use crate::model::{Category, CustomPattern, CustomWords, Gender, NamePart, Rules, Source};
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct RawMatch {
    pub bstart: usize,
    pub bend: usize,
    pub category: Category,
    pub source: Source,
    pub confidence: u8,
    pub parts: Vec<NamePart>,
    /// Trenner zwischen den Namensteilen (Leerzeichen, „, “).
    pub seps: Vec<String>,
    /// Zelle einer Tabelle mit festen Spaltenbreiten: Ersatz auf Originalbreite auffüllen;
    /// bis zu diesem Byte-Offset dürfen Füll-Leerzeichen verbraucht werden.
    pub pad: bool,
    pub pad_end: Option<usize>,
}

impl RawMatch {
    fn simple(bstart: usize, bend: usize, category: Category, source: Source, confidence: u8) -> RawMatch {
        RawMatch { bstart, bend, category, source, confidence, parts: vec![], seps: vec![], pad: false, pad_end: None }
    }
}

struct Patterns {
    email: Regex,
    iban: Regex,
    card: Regex,
    phone: Regex,
    ipv4: Regex,
    ipv6: Regex,
    plate: Regex,
    tax_id: Regex,
    vat_id: Regex,
    svnr: Regex,
    address_de: Regex,
    address_prep: Regex,
    address_en: Regex,
    postal: Regex,
    postal_d: Regex,
    customer_id: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        email: Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}").unwrap(),
        iban: Regex::new(r"\b[A-Z]{2}\d{2}(?:[ ]?[A-Z0-9]{4}){2,7}(?:[ ]?[A-Z0-9]{1,4})?\b").unwrap(),
        card: Regex::new(r"\b(?:\d[ -]?){12,18}\d\b").unwrap(),
        phone: Regex::new(r"(?:\+\d{1,3}[ .-]?(?:\(0\)[ ]?)?)?(?:\(0?\d{1,5}\)|0\d{1,5})[ ./-]?\d{2,8}(?:[ ./-]\d{1,8}){0,4}(?:-\d{1,3})?").unwrap(),
        ipv4: Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap(),
        ipv6: Regex::new(r"\b(?:[0-9a-fA-F]{1,4}:){3,7}[0-9a-fA-F]{1,4}\b").unwrap(),
        plate: Regex::new(r"\b[A-ZÄÖÜ]{1,3}-[A-Z]{1,2}[ ]?\d{1,4}[EH]?\b").unwrap(),
        tax_id: Regex::new(r"\b\d{2}[ ]?\d{3}[ ]?\d{3}[ ]?\d{3}\b").unwrap(),
        vat_id: Regex::new(r"\bDE[ ]?\d{9}\b").unwrap(),
        svnr: Regex::new(r"\b\d{2}[ ]?\d{6}[ ]?[A-Z][ ]?\d{3}\b").unwrap(),
        address_de: Regex::new(r"\b(?:[A-ZÄÖÜ][\w]*[ -])?[A-ZÄÖÜ][\w]*?(?:[Ss]traße|[Ss]trasse|[Ss]tr\.|[Ww]eg|[Aa]llee|[Pp]latz|[Gg]asse|[Rr]ing|[Dd]amm|[Uu]fer|[Cc]haussee|[Ss]teig|[Pp]fad|[Mm]arkt)[ ]+\d{1,4}(?:[ ]?[a-zA-Z])?\b").unwrap(),
        address_prep: Regex::new(r"\b(?:Am|An der|An den|Im|In der|Auf dem|Auf der|Unter den|Zum|Zur|Hinter dem|Bei der)[ ][A-ZÄÖÜ][\w]+(?:[ ][A-ZÄÖÜ][\w]+)?[ ]+\d{1,4}(?:[ ]?[a-zA-Z])?\b").unwrap(),
        address_en: Regex::new(r"\b\d{1,5}[ ][A-Z][a-z]+(?:[ ][A-Z][a-z]+)?[ ](?:Street|St\.|Avenue|Ave\.|Road|Rd\.|Lane|Ln\.|Drive|Dr\.|Boulevard|Blvd\.|Way|Court|Ct\.|Place|Pl\.|Square|Close)\b").unwrap(),
        postal: Regex::new(r"\b(\d{5})[ ]+([A-ZÄÖÜ][a-zäöüß]+(?:[ -][A-ZÄÖÜ][a-zäöüß]+)?)").unwrap(),
        postal_d: Regex::new(r"\bD-(\d{5})\b").unwrap(),
        customer_id: Regex::new(r"(?i)\b(?:kd\.?-?nr\.?|kunden-?nr\.?|kundennummer|kunden-?id|aktenzeichen|az\.|vertrags-?nr\.?|vertragsnummer|mitglieds-?nr\.?|mitgliedsnummer|personalnummer|personal-?nr\.?|rechnungs-?nr\.?|rechnungsnummer|auftrags-?nr\.?|auftragsnummer|patienten-?nr\.?|fall-?nr\.?|ticket-?nr\.?|customer[ ]?(?:no|id|number)\.?|account[ ]?(?:no|id|number)\.?|case[ ]?(?:no|id|number)\.?|invoice[ ]?(?:no|number)\.?|member[ ]?(?:no|id)\.?|employee[ ]?(?:no|id)\.?|ticket[ ]?(?:no|id)\.?|order[ ]?(?:no|id|number)\.?)[ ]*[:#]?[ ]*([A-Za-z0-9][A-Za-z0-9/_-]{2,})").unwrap(),
    })
}

// --- Tokenizer ---------------------------------------------------------------

#[derive(Debug, Clone)]
struct Tok {
    start: usize,
    end: usize,
}

const ABBREV: [&str; 14] = ["dr", "prof", "hr", "fr", "mr", "mrs", "ms", "ing", "med", "dipl", "sr", "sra", "sig", "st"];

fn tokenize(text: &str) -> Vec<Tok> {
    let mut toks = Vec::new();
    let bytes = text.as_bytes();
    let mut iter = text.char_indices().peekable();
    while let Some((i, c)) = iter.next() {
        if !c.is_alphanumeric() {
            continue;
        }
        let start = i;
        let mut end = i + c.len_utf8();
        while let Some(&(j, d)) = iter.peek() {
            if d.is_alphanumeric() || d == '\'' || d == '’' || (d == '-' && j + 1 < bytes.len() && text[j + 1..].chars().next().map(|n| n.is_alphanumeric()).unwrap_or(false)) {
                end = j + d.len_utf8();
                iter.next();
            } else {
                break;
            }
        }
        // Abkürzungspunkt (Dr., Prof., Hr.) gehört zum Token
        if end < bytes.len() && bytes[end] == b'.' && ABBREV.contains(&text[start..end].to_lowercase().as_str()) {
            end += 1;
            iter.next();
        }
        toks.push(Tok { start, end });
    }
    toks
}

fn is_capitalized(s: &str) -> bool {
    let mut cs = s.chars();
    match cs.next() {
        Some(c) if c.is_uppercase() => cs.next().map(|d| d.is_lowercase()).unwrap_or(true) || s.chars().filter(|c| c.is_alphabetic()).count() <= 1,
        _ => false,
    }
}

fn is_all_caps(s: &str) -> bool {
    let letters: Vec<char> = s.chars().filter(|c| c.is_alphabetic()).collect();
    letters.len() >= 2 && letters.iter().all(|c| c.is_uppercase())
}

/// Nur Leerraum (ohne Zeilenumbruch) zwischen zwei Tokens?
fn adjacent(text: &str, a: &Tok, b: &Tok) -> bool {
    let gap = &text[a.end..b.start];
    !gap.is_empty() && gap.len() <= 3 && gap.chars().all(|c| c == ' ' || c == '\t' || c == '\u{a0}')
}

const SALUTATIONS: [&str; 22] = [
    "herr", "herrn", "frau", "hr.", "fr.", "mr", "mr.", "mrs", "mrs.", "ms", "ms.", "miss", "monsieur", "madame", "mme", "señor", "señora", "sr.", "sra.", "sig.", "sayın", "fräulein",
];
const TITLES: [&str; 12] = ["dr", "dr.", "prof", "prof.", "dipl.", "ing.", "med.", "doktor", "professor", "dipl.-ing.", "mag.", "phd"];
const PARTICLES: [&str; 22] = ["von", "van", "de", "der", "den", "zu", "zur", "da", "di", "du", "la", "le", "del", "della", "el", "al", "bin", "ibn", "ter", "ten", "af", "y"];
const STOPWORDS: [&str; 164] = [
    "und", "oder", "aber", "der", "die", "das", "den", "dem", "des", "ein", "eine", "einen", "einem", "einer", "ich", "du", "er", "sie", "es", "wir", "ihr", "am", "an", "im", "in", "um", "mit", "von", "für", "nach", "bei", "zu", "zum", "zur", "vom", "aus", "auf", "bis", "über", "unter", "vor", "seit", "ohne", "gegen", "durch", "wenn", "als", "auch", "nur", "noch", "schon", "sehr", "hat", "ist", "war", "wird", "sind", "haben", "gmbh", "ag", "kg", "ohg", "ug", "str", "str.", "straße", "herr", "herrn", "frau", "uhr", "euro", "eur", "nr", "nr.", "tel", "tel.", "fax", "team", "firma", "kunde", "kundin", "montag", "dienstag", "mittwoch", "donnerstag", "freitag", "samstag", "sonntag", "januar", "februar", "märz", "april", "mai", "juni", "juli", "august", "september", "oktober", "november", "dezember", "the", "and", "or", "but", "of", "on", "at", "to", "for", "with", "from", "by", "is", "are", "was", "were", "has", "have", "had", "will", "would", "should", "can", "could", "may", "might", "ltd", "inc", "llc", "co", "corp", "street", "avenue", "road", "dear", "hi", "hello", "regards", "thanks", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday", "january", "february", "march", "june", "july", "october", "december", "sehr", "geehrte", "geehrter", "liebe", "lieber", "hallo", "name", "vorname", "nachname", "ort", "datum",
];

fn is_stop(s: &str) -> bool {
    STOPWORDS.contains(&s.to_lowercase().as_str())
}

fn surname_like(d: &Dictionaries, s: &str) -> Option<u8> {
    if d.is_last_name(s) || s.split('-').all(|p| d.is_last_name(p)) {
        return Some(92);
    }
    if d.first_name(s).is_some() {
        return Some(85);
    }
    if is_capitalized(s) && !is_stop(s) && !is_all_caps(s) && s.chars().count() >= 3 && !d.is_city(s) {
        return Some(65);
    }
    None
}

fn build_person(text: &str, toks: &[Tok], d: &Dictionaries, gender_hint: Gender) -> RawMatch {
    let mut parts = Vec::new();
    let mut seps = Vec::new();
    let n = toks.len();
    for (i, t) in toks.iter().enumerate() {
        let s = &text[t.start..t.end];
        let lower = s.to_lowercase();
        let part = if PARTICLES.contains(&lower.as_str()) {
            NamePart::Keep { text: s.to_string() }
        } else if i == n - 1 && n > 1 {
            NamePart::Last { text: s.to_string() }
        } else if let Some(fnm) = d.first_name(s) {
            NamePart::First { text: s.to_string(), gender: if fnm.gender == Gender::U { gender_hint } else { fnm.gender } }
        } else if n == 1 {
            NamePart::Last { text: s.to_string() }
        } else {
            NamePart::First { text: s.to_string(), gender: gender_hint }
        };
        parts.push(part);
        if i + 1 < n {
            seps.push(text[t.end..toks[i + 1].start].to_string());
        }
    }
    RawMatch { bstart: toks[0].start, bend: toks[n - 1].end, category: Category::Person, source: Source::Dictionary, confidence: 90, parts, seps, pad: false, pad_end: None }
}

/// Personen im Fließtext: Vorname (+ Nachname) aus dem Wörterbuch, Anrede + Name.
fn detect_persons(text: &str, d: &Dictionaries, out: &mut Vec<RawMatch>) {
    let toks = tokenize(text);
    let n = toks.len();
    let mut i = 0;
    while i < n {
        let s = &text[toks[i].start..toks[i].end];
        let lower = s.to_lowercase();
        if SALUTATIONS.contains(&lower.as_str()) {
            let gender = if matches!(lower.as_str(), "frau" | "fr." | "mrs" | "mrs." | "ms" | "ms." | "miss" | "madame" | "mme" | "señora" | "sra." | "fräulein") {
                Gender::F
            } else {
                Gender::M
            };
            let mut j = i + 1;
            while j < n && adjacent(text, &toks[j - 1], &toks[j]) && TITLES.contains(&text[toks[j].start..toks[j].end].to_lowercase().as_str()) {
                j += 1;
            }
            let mut collected: Vec<Tok> = Vec::new();
            while j < n && collected.len() < 4 && adjacent(text, &toks[j - 1], &toks[j]) {
                let w = &text[toks[j].start..toks[j].end];
                let wl = w.to_lowercase();
                if PARTICLES.contains(&wl.as_str()) && j + 1 < n && is_capitalized(&text[toks[j + 1].start..toks[j + 1].end]) {
                    collected.push(toks[j].clone());
                    j += 1;
                    continue;
                }
                if is_capitalized(w) && !is_stop(w) && !is_all_caps(w) {
                    collected.push(toks[j].clone());
                    j += 1;
                } else {
                    break;
                }
            }
            // Ein einzelnes „von“ o. ä. am Ende zählt nicht
            while collected.last().map(|t| PARTICLES.contains(&text[t.start..t.end].to_lowercase().as_str())).unwrap_or(false) {
                collected.pop();
            }
            if !collected.is_empty() {
                let mut m = build_person(text, &collected, d, gender);
                m.source = Source::Context;
                m.confidence = 88;
                out.push(m);
                i = j;
                continue;
            }
            i += 1;
            continue;
        }
        if is_capitalized(s) {
            if let Some(fnm) = d.first_name(s) {
                let mut collected = vec![toks[i].clone()];
                let mut j = i + 1;
                let mut best_conf = 0u8;
                while j < n && collected.len() < 4 && adjacent(text, &toks[j - 1], &toks[j]) {
                    let w = &text[toks[j].start..toks[j].end];
                    let wl = w.to_lowercase();
                    if PARTICLES.contains(&wl.as_str()) && j + 1 < n && is_capitalized(&text[toks[j + 1].start..toks[j + 1].end]) {
                        collected.push(toks[j].clone());
                        j += 1;
                        continue;
                    }
                    match surname_like(d, w) {
                        Some(c) => {
                            best_conf = best_conf.max(c);
                            collected.push(toks[j].clone());
                            j += 1;
                            // Nach einem Nachnamen aus dem Wörterbuch ist Schluss, sonst hängt „Anna Berger Köln“ zusammen
                            if d.is_last_name(w) {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                while collected.last().map(|t| PARTICLES.contains(&text[t.start..t.end].to_lowercase().as_str())).unwrap_or(false) {
                    collected.pop();
                }
                if collected.len() >= 2 {
                    let mut m = build_person(text, &collected, d, fnm.gender);
                    m.confidence = best_conf.max(70);
                    m.source = if best_conf >= 85 { Source::Dictionary } else { Source::Context };
                    out.push(m);
                    i = j;
                    continue;
                }
                if !fnm.ambiguous {
                    let mut m = build_person(text, &collected, d, fnm.gender);
                    m.confidence = 70;
                    out.push(m);
                }
            }
        }
        i += 1;
    }
}

fn prev_char_ok(text: &str, start: usize) -> bool {
    text[..start].chars().next_back().map(|c| !c.is_alphanumeric()).unwrap_or(true)
}
fn next_char_ok(text: &str, end: usize) -> bool {
    text[end..].chars().next().map(|c| !c.is_alphanumeric()).unwrap_or(true)
}

fn detect_patterns(text: &str, d: &Dictionaries, out: &mut Vec<RawMatch>) {
    let p = patterns();
    for m in p.email.find_iter(text) {
        out.push(RawMatch::simple(m.start(), m.end(), Category::Email, Source::Pattern, 98));
    }
    for m in p.iban.find_iter(text) {
        let compact: String = m.as_str().chars().filter(|c| !c.is_whitespace()).collect();
        if checksum::iban_valid(&compact) {
            out.push(RawMatch::simple(m.start(), m.end(), Category::Iban, Source::Checksum, 99));
        } else if checksum::iban_length(&compact[..2]).map(|l| l == compact.len()).unwrap_or(false) {
            out.push(RawMatch::simple(m.start(), m.end(), Category::Iban, Source::Pattern, 60));
        }
    }
    for m in p.card.find_iter(text) {
        let digits = m.as_str().chars().filter(|c| c.is_ascii_digit()).count();
        if (13..=19).contains(&digits) && checksum::luhn_valid(m.as_str()) {
            out.push(RawMatch::simple(m.start(), m.end(), Category::CreditCard, Source::Checksum, 95));
        }
    }
    for m in p.tax_id.find_iter(text) {
        if checksum::tax_id_valid(m.as_str()) {
            out.push(RawMatch::simple(m.start(), m.end(), Category::TaxId, Source::Checksum, 95));
        }
    }
    for m in p.vat_id.find_iter(text) {
        out.push(RawMatch::simple(m.start(), m.end(), Category::TaxId, Source::Pattern, 85));
    }
    for m in p.svnr.find_iter(text) {
        if checksum::svnr_valid(m.as_str()) {
            out.push(RawMatch::simple(m.start(), m.end(), Category::Insurance, Source::Checksum, 95));
        }
    }
    for m in p.ipv4.find_iter(text) {
        if m.as_str().split('.').all(|o| o.parse::<u32>().map(|v| v <= 255).unwrap_or(false)) {
            out.push(RawMatch::simple(m.start(), m.end(), Category::Ip, Source::Pattern, 90));
        }
    }
    for m in p.ipv6.find_iter(text) {
        if m.as_str().chars().any(|c| c.is_ascii_alphabetic()) || m.as_str().matches(':').count() >= 5 {
            out.push(RawMatch::simple(m.start(), m.end(), Category::Ip, Source::Pattern, 80));
        }
    }
    let dp = dates::patterns();
    for re in dp.numeric.iter().chain([&dp.textual_de, &dp.textual_en_mdy, &dp.textual_en_dmy]) {
        for m in re.find_iter(text) {
            if dates::is_valid(m.as_str()) {
                out.push(RawMatch::simple(m.start(), m.end(), Category::Date, Source::Pattern, 90));
            }
        }
    }
    for m in p.phone.find_iter(text) {
        let digits = m.as_str().chars().filter(|c| c.is_ascii_digit()).count();
        if (7..=16).contains(&digits) && prev_char_ok(text, m.start()) && next_char_ok(text, m.end()) {
            out.push(RawMatch::simple(m.start(), m.end(), Category::Phone, Source::Pattern, 75));
        }
    }
    for m in p.plate.find_iter(text) {
        out.push(RawMatch::simple(m.start(), m.end(), Category::Plate, Source::Pattern, 70));
    }
    for re in [&p.address_de, &p.address_prep, &p.address_en] {
        for m in re.find_iter(text) {
            out.push(RawMatch::simple(m.start(), m.end(), Category::Address, Source::Pattern, 85));
        }
    }
    for c in p.postal.captures_iter(text) {
        let city = c.get(2).unwrap().as_str();
        let known = d.is_city(city) || city.split([' ', '-']).next().map(|w| d.is_city(w)).unwrap_or(false);
        if known {
            let g = c.get(1).unwrap();
            out.push(RawMatch::simple(g.start(), g.end(), Category::PostalCode, Source::Dictionary, 90));
        }
    }
    for c in p.postal_d.captures_iter(text) {
        let g = c.get(1).unwrap();
        out.push(RawMatch::simple(g.start(), g.end(), Category::PostalCode, Source::Pattern, 80));
    }
    for c in p.customer_id.captures_iter(text) {
        let g = c.get(1).unwrap();
        if g.as_str().chars().any(|c| c.is_ascii_digit()) {
            out.push(RawMatch::simple(g.start(), g.end(), Category::CustomerId, Source::Context, 85));
        }
    }
}

fn detect_custom(text: &str, patterns: &[CustomPattern], words: &[CustomWords], out: &mut Vec<RawMatch>, notes: &mut Vec<String>) {
    for cp in patterns {
        if cp.pattern.trim().is_empty() {
            continue;
        }
        match Regex::new(&cp.pattern) {
            Ok(re) => {
                for m in re.find_iter(text) {
                    if m.end() > m.start() {
                        out.push(RawMatch::simple(m.start(), m.end(), cp.category, Source::Custom, 90));
                    }
                }
            }
            Err(e) => notes.push(format!("Muster „{}“ ungültig: {}", cp.name, e)),
        }
    }
    for cw in words {
        let list: Vec<String> = cw.words.iter().map(|w| w.trim()).filter(|w| !w.is_empty()).map(regex::escape).collect();
        if list.is_empty() {
            continue;
        }
        let re = Regex::new(&format!(r"(?i)\b(?:{})\b", list.join("|"))).unwrap();
        for m in re.find_iter(text) {
            let mut rm = RawMatch::simple(m.start(), m.end(), cw.category, Source::Custom, 90);
            if cw.category == Category::Person {
                rm.parts = vec![NamePart::Last { text: m.as_str().to_string() }];
            }
            out.push(rm);
        }
    }
}

/// Personen-Zelle aus einer typisierten Spalte in Namensteile zerlegen.
fn person_from_cell(text: &str, start: usize, end: usize, d: &Dictionaries, hint: csv::PersonHint) -> RawMatch {
    let cell = &text[start..end];
    let toks = tokenize(cell);
    let mut parts = Vec::new();
    let mut seps = Vec::new();
    let comma = cell.contains(',');
    let n = toks.len();
    for (i, t) in toks.iter().enumerate() {
        let s = &cell[t.start..t.end];
        let lower = s.to_lowercase();
        let first = d.first_name(s);
        let part = match hint {
            _ if PARTICLES.contains(&lower.as_str()) => NamePart::Keep { text: s.to_string() },
            csv::PersonHint::First => NamePart::First { text: s.to_string(), gender: first.map(|f| f.gender).unwrap_or(Gender::U) },
            csv::PersonHint::Last => NamePart::Last { text: s.to_string() },
            csv::PersonHint::Full => {
                if comma {
                    // „Berger, Anna“: vor dem Komma Nachname
                    let before_comma = cell[..t.start].matches(',').count() == 0;
                    if before_comma {
                        NamePart::Last { text: s.to_string() }
                    } else {
                        NamePart::First { text: s.to_string(), gender: first.map(|f| f.gender).unwrap_or(Gender::U) }
                    }
                } else if n == 1 {
                    if first.is_some() {
                        NamePart::First { text: s.to_string(), gender: first.unwrap().gender }
                    } else {
                        NamePart::Last { text: s.to_string() }
                    }
                } else if i == n - 1 {
                    NamePart::Last { text: s.to_string() }
                } else {
                    NamePart::First { text: s.to_string(), gender: first.map(|f| f.gender).unwrap_or(Gender::U) }
                }
            }
        };
        parts.push(part);
        if i + 1 < n {
            seps.push(cell[t.end..toks[i + 1].start].to_string());
        }
    }
    if toks.is_empty() {
        return RawMatch::simple(start, end, Category::Person, Source::Column, 80);
    }
    RawMatch { bstart: start + toks[0].start, bend: start + toks[n - 1].end, category: Category::Person, source: Source::Column, confidence: 85, parts, seps, pad: false, pad_end: None }
}

fn detect_columns(text: &str, delimiter: char, d: &Dictionaries, out: &mut Vec<RawMatch>) {
    let rows = csv::cells(text, delimiter);
    typed_rows(text, &rows, d, out, false);
}

/// Tabellen mit festen Spaltenbreiten im Fließtext (Kopfzeile + Zeilen mit ≥2 Leerzeichen als Trenner).
fn detect_fixed_width(text: &str, d: &Dictionaries, out: &mut Vec<RawMatch>) {
    for table in csv::fixed_width_tables(text) {
        typed_rows(text, &table, d, out, true);
    }
}

fn typed_rows(text: &str, rows: &[Vec<csv::Cell>], d: &Dictionaries, out: &mut Vec<RawMatch>, pad: bool) {
    if rows.len() < 2 {
        return;
    }
    let header = &rows[0];
    let typed: Vec<Option<(Category, csv::PersonHint)>> = header.iter().map(|c| csv::column_category(text[c.start..c.end].trim())).collect();
    if typed.iter().all(|t| t.is_none()) {
        return;
    }
    for row in rows.iter().skip(1) {
        for (ci, cell) in row.iter().enumerate() {
            let Some(Some((cat, hint))) = typed.get(ci) else { continue };
            let raw = &text[cell.start..cell.end];
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            let s = cell.start + (raw.len() - raw.trim_start().len());
            let e = s + trimmed.len();
            let mut m = if *cat == Category::Person { person_from_cell(text, s, e, d, *hint) } else { RawMatch::simple(s, e, *cat, Source::Column, 80) };
            m.pad = pad;
            if pad {
                // Bis zwei Zeichen vor der nächsten Spalte darf aufgefüllt bzw. verbraucht werden
                m.pad_end = row.get(ci + 1).map(|n| n.start.saturating_sub(2)).filter(|pe| *pe >= m.bend);
            }
            out.push(m);
        }
    }
}

/// Alle Treffer für einen Text — bereits nach Priorität entflochten und nach Position sortiert.
pub fn detect(text: &str, rules: &Rules, d: &Dictionaries, csv_delimiter: Option<char>, notes: &mut Vec<String>) -> Vec<RawMatch> {
    let mut raw: Vec<RawMatch> = Vec::new();
    detect_persons(text, d, &mut raw);
    detect_patterns(text, d, &mut raw);
    detect_custom(text, &rules.custom_patterns, &rules.custom_words, &mut raw, notes);
    if rules.column_typing {
        match csv_delimiter {
            Some(delim) => detect_columns(text, delim, d, &mut raw),
            None => detect_fixed_width(text, d, &mut raw),
        }
    }

    // Abgeschaltete Kategorien
    raw.retain(|m| rules.rule(m.category).enabled);

    // Ausnahmeliste: alles, was in einer geschützten Phrase liegt, fällt weg
    let mut protected: Vec<(usize, usize)> = Vec::new();
    for phrase in rules.allowlist.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
        if let Ok(re) = Regex::new(&format!(r"(?i){}", regex::escape(phrase))) {
            protected.extend(re.find_iter(text).map(|m| (m.start(), m.end())));
        }
    }
    if !protected.is_empty() {
        raw.retain(|m| !protected.iter().any(|(s, e)| m.bstart < *e && m.bend > *s));
    }

    // Überlappungen: höhere Priorität gewinnt, dann Spalten-Typisierung (kennt die Zelle
    // und ihre Breite), dann höhere Konfidenz, dann längerer Treffer
    raw.sort_by(|a, b| {
        b.category
            .priority()
            .cmp(&a.category.priority())
            .then((b.source == Source::Column).cmp(&(a.source == Source::Column)))
            .then(b.confidence.cmp(&a.confidence))
            .then((b.bend - b.bstart).cmp(&(a.bend - a.bstart)))
            .then(a.bstart.cmp(&b.bstart))
    });
    let mut accepted: Vec<RawMatch> = Vec::new();
    for m in raw {
        if m.bend <= m.bstart {
            continue;
        }
        if accepted.iter().any(|a| m.bstart < a.bend && m.bend > a.bstart) {
            continue;
        }
        accepted.push(m);
    }
    accepted.sort_by_key(|m| m.bstart);
    accepted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str) -> Vec<(Category, String)> {
        let d = Dictionaries::builtin();
        let mut notes = Vec::new();
        detect(text, &Rules::default(), &d, None, &mut notes).into_iter().map(|m| (m.category, text[m.bstart..m.bend].to_string())).collect()
    }

    #[test]
    fn letter_example() {
        let text = "Sehr geehrte Frau Berger,\nvielen Dank für Ihren Anruf vom 14.03.2024. Die Gutschrift\ngeht auf Ihr Konto DE89 3704 0044 0532 0130 00.\nRückfragen an anna.berger@example.org oder 0221 4711-0.\n\nKd-Nr.  Name          Ort    Geb.\n10482   Anna Berger   Köln   02.07.1981\n10483   Jonas Berger  Köln   19.11.2009\n";
        let found = run(text);
        let cats: Vec<Category> = found.iter().map(|f| f.0).collect();
        assert!(found.contains(&(Category::Person, "Berger".into())), "{found:?}");
        assert!(found.contains(&(Category::Date, "14.03.2024".into())));
        assert!(found.contains(&(Category::Iban, "DE89 3704 0044 0532 0130 00".into())));
        assert!(found.contains(&(Category::Email, "anna.berger@example.org".into())));
        assert!(found.contains(&(Category::Phone, "0221 4711-0".into())));
        assert!(found.contains(&(Category::CustomerId, "10482".into())), "{found:?}");
        assert!(found.contains(&(Category::Person, "Anna Berger".into())));
        assert!(found.contains(&(Category::Person, "Jonas Berger".into())));
        assert!(found.contains(&(Category::Date, "02.07.1981".into())));
        assert!(!cats.contains(&Category::Address));
    }

    #[test]
    fn addresses_and_more() {
        let found = run("Anna wohnt in der Musterstraße 12a, 50667 Köln. Kennzeichen K-AB 1234, IP 192.168.1.42, Steuer-ID 86 095 742 719, SV 65 170839 J 003. Karte 4539 1488 0343 6467. Am Hang 3 in Bonn.");
        assert!(found.contains(&(Category::Address, "Musterstraße 12a".into())), "{found:?}");
        assert!(found.contains(&(Category::PostalCode, "50667".into())));
        assert!(found.contains(&(Category::Plate, "K-AB 1234".into())));
        assert!(found.contains(&(Category::Ip, "192.168.1.42".into())));
        assert!(found.contains(&(Category::TaxId, "86 095 742 719".into())));
        assert!(found.contains(&(Category::Insurance, "65 170839 J 003".into())));
        assert!(found.contains(&(Category::CreditCard, "4539 1488 0343 6467".into())));
        assert!(found.contains(&(Category::Address, "Am Hang 3".into())));
        assert!(found.contains(&(Category::Person, "Anna".into())));
    }

    #[test]
    fn salutation_titles_and_ambiguity() {
        let found = run("Herr Dr. Hans Peter Müller und Frau von Berg kamen. Frank kam nicht, aber Frank Walter schon. Mark my words.");
        assert!(found.contains(&(Category::Person, "Hans Peter Müller".into())), "{found:?}");
        assert!(found.contains(&(Category::Person, "von Berg".into())), "{found:?}");
        assert!(found.contains(&(Category::Person, "Frank Walter".into())));
        assert!(!found.contains(&(Category::Person, "Frank".into())));
        assert!(!found.iter().any(|f| f.1 == "Mark"), "{found:?}");
    }

    #[test]
    fn allowlist_and_disabled() {
        let d = Dictionaries::builtin();
        let mut rules = Rules::default();
        rules.allowlist.push("Anna Berger GmbH".into());
        rules.categories.get_mut(&Category::Email).unwrap().enabled = false;
        let text = "Die Anna Berger GmbH schreibt an info@example.org. Anna Berger selbst auch.";
        let mut notes = Vec::new();
        let found: Vec<String> = detect(text, &rules, &d, None, &mut notes).into_iter().map(|m| text[m.bstart..m.bend].to_string()).collect();
        assert_eq!(found, vec!["Anna Berger".to_string()], "{found:?}");
    }

    #[test]
    fn csv_columns() {
        let d = Dictionaries::builtin();
        let text = "Kd-Nr;Nachname;Vorname;Geb.;Ort\n10482;Zyxwacz;Anna;02.07.1981;Köln\n";
        let mut notes = Vec::new();
        let found: Vec<(Category, String)> = detect(text, &Rules::default(), &d, Some(';'), &mut notes).into_iter().map(|m| (m.category, text[m.bstart..m.bend].to_string())).collect();
        assert!(found.contains(&(Category::CustomerId, "10482".into())), "{found:?}");
        assert!(found.contains(&(Category::Person, "Zyxwacz".into())), "{found:?}");
        assert!(found.contains(&(Category::Person, "Anna".into())));
        assert!(found.contains(&(Category::Date, "02.07.1981".into())));
        assert!(!found.iter().any(|f| f.1 == "Köln"));
    }
}

