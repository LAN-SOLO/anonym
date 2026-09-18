//! Deterministische, konsistente Pseudonyme. Gleiche Saat + gleiches Original =
//! gleicher Ersatz — innerhalb einer Datei und (mit gespeichertem Speicher) über
//! Dateien hinweg. Kein Zufall, keine KI: FNV-Hash → Index in die Namenswelt.

use crate::checksum;
use crate::dict::{Dictionaries, World};
use crate::model::{Category, Gender, NamePart};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

/// Persistierbarer Pseudonym-Speicher (Tarif masked: über Dateien hinweg).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Store {
    pub seed: String,
    pub world: String,
    /// `kind|original(klein)` → Ersatz
    pub entries: BTreeMap<String, String>,
    /// `kategorie|original(klein)` → Platzhalter-Nummer
    pub placeholders: BTreeMap<String, u32>,
}

pub fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Ziffernfolge aus einem Hash-Strom (splitmix64).
struct Digits {
    state: u64,
}

impl Digits {
    fn new(seed: u64) -> Digits {
        Digits { state: seed }
    }
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    fn digit(&mut self) -> u32 {
        (self.next_u64() % 10) as u32
    }
    fn digit_nonzero(&mut self) -> u32 {
        (self.next_u64() % 9 + 1) as u32
    }
    fn letter(&mut self) -> char {
        (b'A' + (self.next_u64() % 26) as u8) as char
    }
    fn index(&mut self, len: usize) -> usize {
        if len == 0 {
            0
        } else {
            (self.next_u64() % len as u64) as usize
        }
    }
}

pub struct Pseudonymizer {
    world: World,
    seed: String,
    pub store: Store,
    used: HashSet<String>,
    pub date_offset_days: i32,
    /// Feste Spaltenbreiten: so viele Zeichen darf ein Personenname insgesamt länger werden.
    pub slack: Option<usize>,
    /// Längenobergrenze für die nächste Wahl aus einer Liste (nur neue Zuordnungen).
    max_len: Option<usize>,
}

impl Pseudonymizer {
    pub fn new(world: World, seed: &str, date_shift_days: i32, store: Option<Store>) -> Pseudonymizer {
        let mut store = store.unwrap_or_default();
        if store.seed != seed || store.world != world.id {
            // Anderer Schlüssel → alter Speicher passt nicht mehr
            if !store.entries.is_empty() && (store.seed != seed) {
                store.entries.clear();
                store.placeholders.clear();
            }
            store.seed = seed.to_string();
            store.world = world.id.clone();
        }
        let used = store.entries.iter().map(|(k, v)| format!("{}|{}", k.split('|').next().unwrap_or(""), v.to_lowercase())).collect();
        let date_offset_days = if date_shift_days != 0 {
            date_shift_days
        } else {
            let h = fnv1a(&format!("{seed}|date-shift"));
            let days = (h % 45) as i32 + 1;
            if (h >> 8) % 2 == 0 {
                days
            } else {
                -days
            }
        };
        Pseudonymizer { world, seed: seed.to_string(), store, used, date_offset_days, slack: None, max_len: None }
    }

    fn key(kind: &str, original: &str) -> String {
        format!("{kind}|{}", original.trim().to_lowercase())
    }

    fn stream(&self, kind: &str, original: &str) -> Digits {
        Digits::new(fnv1a(&format!("{}|{kind}|{}", self.seed, original.trim().to_lowercase())))
    }

    /// Ersatz aus einer Liste — gemerkt, eindeutig, nie gleich dem Original.
    fn pick(&mut self, kind: &str, original: &str, list: &[String]) -> String {
        let key = Self::key(kind, original);
        if let Some(v) = self.store.entries.get(&key) {
            return v.clone();
        }
        if list.is_empty() {
            return original.to_string();
        }
        let mut d = self.stream(kind, original);
        let start = d.index(list.len());
        let mut chosen: Option<String> = None;
        for i in 0..list.len() {
            let cand = &list[(start + i) % list.len()];
            if cand.eq_ignore_ascii_case(original.trim()) {
                continue;
            }
            if self.max_len.map(|m| cand.chars().count() > m).unwrap_or(false) {
                continue;
            }
            let ukey = format!("{kind}|{}", cand.to_lowercase());
            if !self.used.contains(&ukey) {
                chosen = Some(cand.clone());
                break;
            }
        }
        // Liste erschöpft: Name mit Zähler
        let value = chosen.unwrap_or_else(|| {
            let n = self.store.entries.keys().filter(|k| k.starts_with(&format!("{kind}|"))).count() + 1;
            format!("{} {}", list[start], n)
        });
        self.used.insert(format!("{kind}|{}", value.to_lowercase()));
        self.store.entries.insert(key, value.clone());
        value
    }

    fn remember(&mut self, kind: &str, original: &str, value: String) -> String {
        let key = Self::key(kind, original);
        if let Some(v) = self.store.entries.get(&key) {
            return v.clone();
        }
        self.store.entries.insert(key, value.clone());
        value
    }

    pub fn first_name(&mut self, original: &str, gender: Gender) -> String {
        let kind = match gender {
            Gender::F => "first-f",
            Gender::M => "first-m",
            Gender::U => "first-u",
        };
        let list = self.world.first_names(gender).to_vec();
        let v = self.pick(kind, original, &list);
        match_case(original, &v)
    }

    pub fn last_name(&mut self, original: &str) -> String {
        let list = self.world.last.clone();
        let v = self.pick("last", original, &list);
        match_case(original, &v)
    }

    /// Vollständiger Personenname aus Teilen. Mit `slack` (feste Spaltenbreiten) werden
    /// neue Namen bevorzugt so gewählt, dass der Gesamtname höchstens `slack` Zeichen wächst.
    pub fn person(&mut self, parts: &[NamePart], separators: &[String]) -> String {
        let mut out = String::new();
        let mut slack = self.slack;
        for (i, p) in parts.iter().enumerate() {
            if i > 0 {
                out.push_str(separators.get(i - 1).map(String::as_str).unwrap_or(" "));
            }
            let (text, value) = match p {
                NamePart::First { text, gender } => {
                    self.max_len = slack.map(|s| text.chars().count() + s);
                    let v = self.first_name(text, *gender);
                    self.max_len = None;
                    (text, v)
                }
                NamePart::Last { text } => {
                    self.max_len = slack.map(|s| text.chars().count() + s);
                    let v = self.last_name(text);
                    self.max_len = None;
                    (text, v)
                }
                NamePart::Keep { text } => (text, text.clone()),
            };
            if let Some(s) = slack {
                let (ol, nl) = (text.chars().count(), value.chars().count());
                slack = Some((s + ol).saturating_sub(nl));
            }
            out.push_str(&value);
        }
        out
    }

    /// E-Mail: lokaler Teil aus bekannten Namen zusammensetzen, sonst deterministisch; Domain bleibt.
    pub fn email(&mut self, original: &str, d: &Dictionaries) -> String {
        let key = Self::key("email", original);
        if let Some(v) = self.store.entries.get(&key) {
            return v.clone();
        }
        let (local, domain) = match original.find('@') {
            Some(i) => (&original[..i], &original[i..]),
            None => (original, ""),
        };
        let mut new_local = String::new();
        let mut any_name = false;
        let mut idx = 0;
        let bytes = local.as_bytes();
        while idx < bytes.len() {
            let start = idx;
            while idx < bytes.len() && bytes[idx].is_ascii_alphanumeric() {
                idx += 1;
            }
            if idx > start {
                let token = &local[start..idx];
                let lower = token.to_lowercase();
                let mapped = ["first-f", "first-m", "first-u", "last"]
                    .iter()
                    .find_map(|k| self.store.entries.get(&format!("{k}|{lower}")).cloned());
                if let Some(m) = mapped {
                    any_name = true;
                    new_local.push_str(&ascii_fold(&m).to_lowercase());
                } else if let Some(fnm) = d.first_name(token) {
                    // Bekannter Vorname, der im Text selbst nicht vorkam
                    any_name = true;
                    let v = self.first_name(token, fnm.gender);
                    new_local.push_str(&ascii_fold(&v).to_lowercase());
                } else if d.is_last_name(token) {
                    any_name = true;
                    let v = self.last_name(token);
                    new_local.push_str(&ascii_fold(&v).to_lowercase());
                } else if token.chars().all(|c| c.is_ascii_digit()) {
                    let mut d = self.stream("email-digits", token);
                    for _ in 0..token.len() {
                        new_local.push(char::from_digit(d.digit(), 10).unwrap());
                    }
                } else {
                    new_local.push_str(token);
                }
            }
            if idx < bytes.len() {
                new_local.push(bytes[idx] as char);
                idx += 1;
            }
        }
        if !any_name {
            // Kein bekannter Name: lokalen Teil durch einen Welt-Namen ersetzen
            let list = self.world.last.clone();
            let name = self.pick("email-local", local, &list);
            let mut d = self.stream("email-num", local);
            new_local = format!("{}{}", ascii_fold(&name).to_lowercase(), d.index(90) + 10);
        }
        let v = format!("{new_local}{domain}");
        self.remember("email", original, v)
    }

    /// Telefon: erste Gruppe (Vorwahl) bleibt, Rest der Ziffern neu, Format erhalten.
    pub fn phone(&mut self, original: &str) -> String {
        let key = Self::key("phone", original);
        if let Some(v) = self.store.entries.get(&key) {
            return v.clone();
        }
        let groups: Vec<(usize, usize)> = digit_groups(original);
        let mut keep: HashSet<usize> = HashSet::new();
        if groups.len() >= 2 {
            keep.insert(0);
            if original.trim_start().starts_with('+') && groups.len() >= 3 {
                keep.insert(1);
            }
            // Durchwahl „-0“ / „-12“ am Ende bleibt
            let (ls, le) = groups[groups.len() - 1];
            if le - ls <= 2 && original[..ls].trim_end().ends_with('-') {
                keep.insert(groups.len() - 1);
            }
        }
        let mut d = self.stream("phone", original);
        let mut out = String::with_capacity(original.len());
        let mut pos = 0;
        for (gi, (gs, ge)) in groups.iter().enumerate() {
            out.push_str(&original[pos..*gs]);
            if keep.contains(&gi) {
                out.push_str(&original[*gs..*ge]);
            } else if groups.len() == 1 {
                // Eine einzige Ziffernfolge: erste vier Ziffern (Vorwahl) bleiben
                let seg = &original[*gs..*ge];
                let split = seg.len().min(4);
                out.push_str(&seg[..split]);
                for _ in split..seg.len() {
                    out.push(char::from_digit(d.digit(), 10).unwrap());
                }
            } else {
                for i in *gs..*ge {
                    let _ = i;
                    let dg = if i == *gs { d.digit_nonzero() } else { d.digit() };
                    out.push(char::from_digit(dg, 10).unwrap());
                }
            }
            pos = *ge;
        }
        out.push_str(&original[pos..]);
        self.remember("phone", original, out)
    }

    /// IBAN: Land und Bankleitzahl bleiben, Kontonummer neu, Prüfziffer gültig, Gruppierung erhalten.
    pub fn iban(&mut self, original: &str) -> String {
        let key = Self::key("iban", original);
        if let Some(v) = self.store.entries.get(&key) {
            return v.clone();
        }
        let compact: String = original.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_uppercase();
        if compact.len() < 15 {
            return original.to_string();
        }
        let country = &compact[..2];
        let bban = &compact[4..];
        let keep = if bban.len() >= 18 { 8 } else { 4 };
        let mut d = self.stream("iban", &compact);
        let mut new_bban = String::from(&bban[..keep.min(bban.len())]);
        for c in bban[keep.min(bban.len())..].chars() {
            if c.is_ascii_digit() {
                new_bban.push(char::from_digit(d.digit(), 10).unwrap());
            } else {
                new_bban.push(c);
            }
        }
        let new_compact = checksum::iban_with_check(country, &new_bban);
        let out = reflow(original, &new_compact);
        self.remember("iban", original, out)
    }

    /// Kreditkarte: erste vier Ziffern bleiben, Rest neu, Luhn gültig, Gruppierung erhalten.
    pub fn credit_card(&mut self, original: &str) -> String {
        let key = Self::key("card", original);
        if let Some(v) = self.store.entries.get(&key) {
            return v.clone();
        }
        let digits: Vec<u32> = original.chars().filter_map(|c| c.to_digit(10)).collect();
        if digits.len() < 12 {
            return original.to_string();
        }
        let mut d = self.stream("card", original);
        let mut new: Vec<u32> = digits[..4].to_vec();
        for _ in 4..digits.len() - 1 {
            new.push(d.digit());
        }
        let check = checksum::luhn_check_digit(&new);
        new.push(check);
        let compact: String = new.iter().map(|x| char::from_digit(*x, 10).unwrap()).collect();
        let out = reflow(original, &compact);
        self.remember("card", original, out)
    }

    /// Steuer-ID: gültige neue Nummer (ISO 7064 Mod 11,10), Gruppierung erhalten.
    pub fn tax_id(&mut self, original: &str) -> String {
        let key = Self::key("taxid", original);
        if let Some(v) = self.store.entries.get(&key) {
            return v.clone();
        }
        let compact: String = original.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        let mut d = self.stream("taxid", &compact);
        let out = if compact.len() == 11 && compact.chars().all(|c| c.is_ascii_digit()) {
            // Zehn Ziffern, genau eine doppelt, dann Prüfziffer
            let mut first_ten: Vec<u32>;
            loop {
                let mut pool: Vec<u32> = (0..10).collect();
                // Fisher-Yates mit Hash-Strom
                for i in (1..pool.len()).rev() {
                    let j = d.index(i + 1);
                    pool.swap(i, j);
                }
                first_ten = pool[..9].to_vec();
                let dup = first_ten[d.index(9)];
                let at = d.index(10);
                first_ten.insert(at, dup);
                if first_ten[0] != 0 {
                    break;
                }
            }
            let check = checksum::tax_id_check_digit(&first_ten);
            let mut s: String = first_ten.iter().map(|x| char::from_digit(*x, 10).unwrap()).collect();
            s.push(char::from_digit(check, 10).unwrap());
            reflow(original, &s)
        } else {
            // USt-IdNr. o. ä.: Buchstaben bleiben, Ziffern neu
            let mut s = String::new();
            for c in compact.chars() {
                if c.is_ascii_digit() {
                    s.push(char::from_digit(d.digit(), 10).unwrap());
                } else {
                    s.push(c);
                }
            }
            reflow(original, &s)
        };
        self.remember("taxid", original, out)
    }

    /// Sozialversicherungsnummer: Bereichsnummer bleibt, Rest neu, Prüfziffer gültig.
    pub fn insurance(&mut self, original: &str) -> String {
        let key = Self::key("svnr", original);
        if let Some(v) = self.store.entries.get(&key) {
            return v.clone();
        }
        let compact: String = original.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_uppercase();
        let mut d = self.stream("svnr", &compact);
        let out = if compact.len() == 12 {
            let mut s = String::from(&compact[..2]);
            // Geburtsdatum TTMMJJ plausibel
            let day = d.index(28) + 1;
            let month = d.index(12) + 1;
            let year = d.index(100);
            s.push_str(&format!("{day:02}{month:02}{year:02}"));
            s.push(d.letter());
            s.push_str(&format!("{:02}", d.index(100)));
            // Prüfziffer bestimmen
            let mut check = 0;
            for c in 0..10 {
                let cand = format!("{s}{c}");
                if checksum::svnr_valid(&cand) {
                    check = c;
                    break;
                }
            }
            s.push(char::from_digit(check, 10).unwrap());
            reflow(original, &s)
        } else {
            let mut s = String::new();
            for c in compact.chars() {
                if c.is_ascii_digit() {
                    s.push(char::from_digit(d.digit(), 10).unwrap());
                } else {
                    s.push(c);
                }
            }
            reflow(original, &s)
        };
        self.remember("svnr", original, out)
    }

    /// Kennzeichen: Unterscheidungszeichen (Stadt) bleibt, Buchstaben und Ziffern neu.
    pub fn plate(&mut self, original: &str) -> String {
        let key = Self::key("plate", original);
        if let Some(v) = self.store.entries.get(&key) {
            return v.clone();
        }
        let mut d = self.stream("plate", original);
        let sep = original.find(|c: char| c == '-' || c == ' ').unwrap_or(0);
        let mut out = String::from(&original[..sep]);
        for c in original[sep..].chars() {
            if c.is_ascii_uppercase() {
                out.push(d.letter());
            } else if c.is_ascii_digit() {
                out.push(char::from_digit(d.digit_nonzero(), 10).unwrap());
            } else {
                out.push(c);
            }
        }
        self.remember("plate", original, out)
    }

    /// IP: die ersten beiden Oktette (Netz) bleiben, Host neu.
    pub fn ip(&mut self, original: &str) -> String {
        let key = Self::key("ip", original);
        if let Some(v) = self.store.entries.get(&key) {
            return v.clone();
        }
        let mut d = self.stream("ip", original);
        let out = if original.contains(':') {
            let groups: Vec<&str> = original.split(':').collect();
            let mut parts: Vec<String> = Vec::new();
            for (i, g) in groups.iter().enumerate() {
                if i < 2 || g.is_empty() {
                    parts.push(g.to_string());
                } else {
                    parts.push(format!("{:x}", d.next_u64() % 0x10000));
                }
            }
            parts.join(":")
        } else {
            let octets: Vec<&str> = original.split('.').collect();
            let mut parts: Vec<String> = Vec::new();
            for (i, o) in octets.iter().enumerate() {
                if i < 2 {
                    parts.push(o.to_string());
                } else {
                    parts.push(format!("{}", d.index(254) + 1));
                }
            }
            parts.join(".")
        };
        self.remember("ip", original, out)
    }

    /// Kunden-/Aktenzeichen: Buchstaben und Trennzeichen bleiben, Ziffern neu (gleiche Länge).
    pub fn customer_id(&mut self, original: &str) -> String {
        let key = Self::key("id", original);
        if let Some(v) = self.store.entries.get(&key) {
            return v.clone();
        }
        let mut d = self.stream("id", original);
        let mut out = String::new();
        let mut first_digit = true;
        for c in original.chars() {
            if c.is_ascii_digit() {
                let dg = if first_digit { d.digit_nonzero() } else { d.digit() };
                first_digit = false;
                out.push(char::from_digit(dg, 10).unwrap());
            } else {
                out.push(c);
            }
        }
        self.remember("id", original, out)
    }

    /// Postleitzahl: Gebiet (erste zwei Ziffern) bleibt.
    pub fn postal_code(&mut self, original: &str) -> String {
        let key = Self::key("plz", original);
        if let Some(v) = self.store.entries.get(&key) {
            return v.clone();
        }
        let mut d = self.stream("plz", original);
        let mut out = String::new();
        let mut n = 0;
        for c in original.chars() {
            if c.is_ascii_digit() {
                n += 1;
                if n <= 2 {
                    out.push(c);
                } else {
                    out.push(char::from_digit(d.digit(), 10).unwrap());
                }
            } else {
                out.push(c);
            }
        }
        self.remember("plz", original, out)
    }

    /// Straße + Hausnummer: Straße aus der Welt, Hausnummer neu.
    pub fn address(&mut self, original: &str) -> String {
        let key = Self::key("address", original);
        if let Some(v) = self.store.entries.get(&key) {
            return v.clone();
        }
        // Straßenname = alles vor der ersten Ziffer (deutsch) bzw. nach der Nummer (englisch)
        let first_digit = original.find(|c: char| c.is_ascii_digit());
        let list = self.world.streets.clone();
        let mut d = self.stream("house", original);
        let number = d.index(120) + 1;
        let out = match first_digit {
            Some(0) => {
                // „12 Baker Street“
                let rest = original.trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ');
                let street = self.pick("street", rest, &list);
                format!("{number} {street}")
            }
            Some(i) => {
                let street_part = original[..i].trim_end();
                let street = self.pick("street", street_part, &list);
                format!("{street} {number}")
            }
            None => self.pick("street", original, &list),
        };
        self.remember("address", original, out)
    }

    /// Platzhalter-Nummer je Kategorie und Original.
    pub fn placeholder_no(&mut self, category: Category, original: &str) -> u32 {
        let key = format!("{}|{}", category.key(), original.trim().to_lowercase());
        if let Some(n) = self.store.placeholders.get(&key) {
            return *n;
        }
        let prefix = format!("{}|", category.key());
        let n = self.store.placeholders.keys().filter(|k| k.starts_with(&prefix)).count() as u32 + 1;
        self.store.placeholders.insert(key, n);
        n
    }

    /// Wort aus einer eigenen Liste: Ersatz aus der Nachnamen-Liste der Welt.
    pub fn custom_word(&mut self, original: &str) -> String {
        let list = self.world.last.clone();
        let v = self.pick("word", original, &list);
        match_case(original, &v)
    }
}

/// Ziffernfolgen (Byte-Bereiche) in einem String.
fn digit_groups(s: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(st) = start.take() {
            out.push((st, i));
        }
    }
    if let Some(st) = start {
        out.push((st, s.len()));
    }
    out
}

/// Neue Zeichenfolge in die Gruppierung (Leerzeichen, Bindestriche) des Originals gießen.
fn reflow(original: &str, compact: &str) -> String {
    let mut out = String::with_capacity(original.len());
    let mut it = compact.chars();
    for c in original.chars() {
        if c.is_ascii_alphanumeric() {
            match it.next() {
                Some(n) => out.push(n),
                None => break,
            }
        } else {
            out.push(c);
        }
    }
    for n in it {
        out.push(n);
    }
    out
}

/// Groß-/Kleinschreibung des Originals übernehmen (ANNA → LENA, anna → lena, Anna → Lena).
pub fn match_case(original: &str, value: &str) -> String {
    let letters: Vec<char> = original.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() >= 2 && letters.iter().all(|c| c.is_uppercase()) {
        value.to_uppercase()
    } else if !letters.is_empty() && letters.iter().all(|c| c.is_lowercase()) {
        value.to_lowercase()
    } else {
        value.to_string()
    }
}

/// Umlaute und Akzente für E-Mail-Adressen falten.
pub fn ascii_fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'ä' | 'á' | 'à' | 'â' | 'ã' | 'å' => out.push('a'),
            'Ä' | 'Á' | 'À' | 'Â' => out.push('A'),
            'ö' | 'ó' | 'ò' | 'ô' | 'õ' | 'ø' => out.push('o'),
            'Ö' | 'Ó' | 'Ò' | 'Ô' => out.push('O'),
            'ü' | 'ú' | 'ù' | 'û' => out.push('u'),
            'Ü' | 'Ú' | 'Ù' | 'Û' => out.push('U'),
            'é' | 'è' | 'ê' | 'ë' => out.push('e'),
            'É' | 'È' | 'Ê' => out.push('E'),
            'í' | 'ì' | 'î' | 'ï' => out.push('i'),
            'ç' => out.push('c'),
            'ñ' => out.push('n'),
            'ß' => out.push_str("ss"),
            'ı' => out.push('i'),
            'ş' => out.push('s'),
            'ğ' => out.push('g'),
            'ł' => out.push('l'),
            c if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' => out.push(c),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> Pseudonymizer {
        Pseudonymizer::new(World::builtin("de"), "test", 0, None)
    }

    #[test]
    fn deterministic_and_consistent() {
        let mut a = p();
        let mut b = p();
        let x = a.first_name("Anna", Gender::F);
        assert_eq!(x, b.first_name("Anna", Gender::F));
        assert_eq!(x, a.first_name("anna", Gender::F).to_lowercase().chars().next().map(|c| c.to_uppercase().collect::<String>()).unwrap() + &x[1..].to_string());
        assert_ne!(x, "Anna");
        let l1 = a.last_name("Berger");
        let l2 = a.last_name("Berger");
        assert_eq!(l1, l2);
        let d = Dictionaries::builtin();
        let email = a.email("anna.berger@example.org", &d);
        assert!(email.ends_with("@example.org"));
        assert!(email.starts_with(&format!("{}.{}", x.to_lowercase(), l1.to_lowercase())), "{email}");
        // Vorname nur in der Adresse, nicht im Text: trotzdem Pseudonym
        let mut c = p();
        let e2 = c.email("jonas.mueller-x@example.org", &d);
        assert!(!e2.starts_with("jonas"), "{e2}");
        assert!(e2.ends_with("-x@example.org"), "{e2}");
    }

    #[test]
    fn numbers_keep_checksums() {
        let mut a = p();
        let iban = a.iban("DE89 3704 0044 0532 0130 00");
        assert!(iban.starts_with("DE"));
        assert!(checksum::iban_valid(&iban), "{iban}");
        assert_eq!(&iban.replace(' ', "")[4..12], "37040044");
        assert_eq!(iban.len(), 27);
        let card = a.credit_card("4539 1488 0343 6467");
        assert!(checksum::luhn_valid(&card), "{card}");
        assert!(card.starts_with("4539 "));
        let tax = a.tax_id("86 095 742 719");
        assert!(checksum::tax_id_valid(&tax), "{tax}");
        assert_eq!(tax.len(), 14);
        let sv = a.insurance("65 170839 J 003");
        assert!(checksum::svnr_valid(&sv), "{sv}");
        assert!(sv.starts_with("65 "));
        let phone = a.phone("0221 4711-0");
        assert!(phone.starts_with("0221 "));
        assert!(phone.ends_with("-0"));
        assert_ne!(phone, "0221 4711-0");
        let intl = a.phone("+49 30 123456");
        assert!(intl.starts_with("+49 30 "));
        assert_eq!(a.ip("192.168.1.42").split('.').take(2).collect::<Vec<_>>(), vec!["192", "168"]);
        let id = a.customer_id("10482");
        assert_eq!(id.len(), 5);
        assert_ne!(id, "10482");
        assert!(a.postal_code("50667").starts_with("50"));
        let plate = a.plate("K-AB 1234");
        assert!(plate.starts_with("K-"));
    }

    #[test]
    fn date_offset_from_seed() {
        let a = Pseudonymizer::new(World::builtin("de"), "x", 0, None);
        assert!(a.date_offset_days != 0 && a.date_offset_days.abs() <= 45);
        let b = Pseudonymizer::new(World::builtin("de"), "x", 13, None);
        assert_eq!(b.date_offset_days, 13);
    }
}
