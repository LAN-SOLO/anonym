//! Datumsangaben erkennen, verschieben und im Originalstil zurückschreiben.

use chrono::{Datelike, Duration, NaiveDate};
use regex::Regex;
use std::sync::OnceLock;

const MONTHS_DE: [(&str, u32); 12] = [
    ("januar", 1),
    ("februar", 2),
    ("märz", 3),
    ("april", 4),
    ("mai", 5),
    ("juni", 6),
    ("juli", 7),
    ("august", 8),
    ("september", 9),
    ("oktober", 10),
    ("november", 11),
    ("dezember", 12),
];
const MONTHS_EN: [(&str, u32); 12] = [
    ("january", 1),
    ("february", 2),
    ("march", 3),
    ("april", 4),
    ("may", 5),
    ("june", 6),
    ("july", 7),
    ("august", 8),
    ("september", 9),
    ("october", 10),
    ("november", 11),
    ("december", 12),
];

pub struct DatePatterns {
    pub numeric: Vec<Regex>,
    pub textual_de: Regex,
    pub textual_en_mdy: Regex,
    pub textual_en_dmy: Regex,
}

pub fn patterns() -> &'static DatePatterns {
    static P: OnceLock<DatePatterns> = OnceLock::new();
    P.get_or_init(|| DatePatterns {
        numeric: vec![
            Regex::new(r"\b(\d{1,2})\.(\d{1,2})\.(\d{4}|\d{2})\b").unwrap(),
            Regex::new(r"\b(\d{4})-(\d{2})-(\d{2})\b").unwrap(),
            Regex::new(r"\b(\d{1,2})/(\d{1,2})/(\d{4})\b").unwrap(),
        ],
        textual_de: Regex::new(r"\b(\d{1,2})\.[ ]?(Januar|Februar|März|Maerz|April|Mai|Juni|Juli|August|September|Oktober|November|Dezember|Jan|Feb|Mär|Apr|Jun|Jul|Aug|Sep|Sept|Okt|Nov|Dez)\.?[ ](\d{4})\b").unwrap(),
        textual_en_mdy: Regex::new(r"\b(January|February|March|April|May|June|July|August|September|October|November|December|Jan|Feb|Mar|Apr|Jun|Jul|Aug|Sep|Sept|Oct|Nov|Dec)\.?[ ](\d{1,2})(?:st|nd|rd|th)?,?[ ](\d{4})\b").unwrap(),
        textual_en_dmy: Regex::new(r"\b(\d{1,2})(?:st|nd|rd|th)?[ ](January|February|March|April|May|June|July|August|September|October|November|December|Jan|Feb|Mar|Apr|Jun|Jul|Aug|Sep|Sept|Oct|Nov|Dec)\.?,?[ ](\d{4})\b").unwrap(),
    })
}

fn month_from_name(name: &str) -> Option<(u32, bool)> {
    let lower = name.to_lowercase().replace("maerz", "märz");
    for (n, m) in MONTHS_DE.iter().chain(MONTHS_EN.iter()) {
        if *n == lower {
            return Some((*m, false));
        }
        if lower.len() >= 3 && n.starts_with(&lower) {
            return Some((*m, true));
        }
    }
    None
}

fn month_name(m: u32, german: bool, abbreviated: bool, original: &str) -> String {
    let table = if german { &MONTHS_DE } else { &MONTHS_EN };
    let full = table[(m - 1) as usize].0;
    let mut name = if abbreviated { full.chars().take(original.trim_end_matches('.').chars().count().max(3)).collect::<String>() } else { full.to_string() };
    // Groß-/Kleinschreibung wie im Original
    if original.chars().next().map(|c| c.is_uppercase()).unwrap_or(true) {
        let mut cs = name.chars();
        name = match cs.next() {
            Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(),
            None => name,
        };
    }
    name
}

fn year_from(s: &str) -> Option<i32> {
    let y: i32 = s.parse().ok()?;
    Some(if s.len() == 2 {
        if y < 50 {
            2000 + y
        } else {
            1900 + y
        }
    } else {
        y
    })
}

fn pad(n: u32, like: &str) -> String {
    if like.len() >= 2 {
        format!("{n:02}")
    } else {
        n.to_string()
    }
}

fn plausible(d: NaiveDate) -> bool {
    (1900..=2100).contains(&d.year())
}

/// Ein erkanntes Datum um `days` verschieben; `None`, wenn der Text kein gültiges Datum ist.
pub fn shift(text: &str, days: i32) -> Option<String> {
    let p = patterns();
    let delta = Duration::days(days as i64);
    // TT.MM.JJJJ
    if let Some(c) = p.numeric[0].captures(text) {
        if c.get(0)?.as_str() == text {
            let (d, m, y) = (&c[1], &c[2], &c[3]);
            let date = NaiveDate::from_ymd_opt(year_from(y)?, m.parse().ok()?, d.parse().ok()?)?;
            if !plausible(date) {
                return None;
            }
            let n = date + delta;
            let year = if y.len() == 2 { format!("{:02}", n.year() % 100) } else { n.year().to_string() };
            return Some(format!("{}.{}.{}", pad(n.day(), d), pad(n.month(), m), year));
        }
    }
    if let Some(c) = p.numeric[1].captures(text) {
        if c.get(0)?.as_str() == text {
            let date = NaiveDate::from_ymd_opt(c[1].parse().ok()?, c[2].parse().ok()?, c[3].parse().ok()?)?;
            if !plausible(date) {
                return None;
            }
            let n = date + delta;
            return Some(format!("{}-{:02}-{:02}", n.year(), n.month(), n.day()));
        }
    }
    if let Some(c) = p.numeric[2].captures(text) {
        if c.get(0)?.as_str() == text {
            let a: u32 = c[1].parse().ok()?;
            let b: u32 = c[2].parse().ok()?;
            // US-Schreibweise MM/TT/JJJJ, außer der erste Wert kann kein Monat sein
            let (m, d, mdy) = if a > 12 { (b, a, false) } else { (a, b, true) };
            let date = NaiveDate::from_ymd_opt(c[3].parse().ok()?, m, d)?;
            if !plausible(date) {
                return None;
            }
            let n = date + delta;
            return Some(if mdy {
                format!("{}/{}/{}", pad(n.month(), &c[1]), pad(n.day(), &c[2]), n.year())
            } else {
                format!("{}/{}/{}", pad(n.day(), &c[1]), pad(n.month(), &c[2]), n.year())
            });
        }
    }
    if let Some(c) = p.textual_de.captures(text) {
        if c.get(0)?.as_str() == text {
            let (m, abbr) = month_from_name(&c[2])?;
            let date = NaiveDate::from_ymd_opt(c[3].parse().ok()?, m, c[1].parse().ok()?)?;
            let n = date + delta;
            let m_end = c.get(2)?.end();
            let dot = if text[m_end..].starts_with('.') { "." } else { "" };
            let d_end = c.get(1)?.end();
            let space = if text[d_end + 1..].starts_with(' ') { " " } else { "" };
            return Some(format!("{}.{}{}{} {}", n.day(), space, month_name(n.month(), true, abbr, &c[2]), dot, n.year()));
        }
    }
    if let Some(c) = p.textual_en_mdy.captures(text) {
        if c.get(0)?.as_str() == text {
            let (m, abbr) = month_from_name(&c[1])?;
            let date = NaiveDate::from_ymd_opt(c[3].parse().ok()?, m, c[2].parse().ok()?)?;
            let n = date + delta;
            let comma = if text.contains(',') { "," } else { "" };
            let dot = if text[c.get(1)?.end()..].starts_with('.') { "." } else { "" };
            return Some(format!("{}{} {}{} {}", month_name(n.month(), false, abbr, &c[1]), dot, n.day(), comma, n.year()));
        }
    }
    if let Some(c) = p.textual_en_dmy.captures(text) {
        if c.get(0)?.as_str() == text {
            let (m, abbr) = month_from_name(&c[2])?;
            let date = NaiveDate::from_ymd_opt(c[3].parse().ok()?, m, c[1].parse().ok()?)?;
            let n = date + delta;
            return Some(format!("{} {} {}", n.day(), month_name(n.month(), false, abbr, &c[2]), n.year()));
        }
    }
    None
}

/// Prüft, ob ein Regex-Treffer ein gültiges Datum ist (für die Erkennung).
pub fn is_valid(text: &str) -> bool {
    shift(text, 0).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shifts() {
        assert_eq!(shift("14.03.2024", 13).unwrap(), "27.03.2024");
        assert_eq!(shift("02.07.1981", 13).unwrap(), "15.07.1981");
        assert_eq!(shift("19.11.2009", 13).unwrap(), "02.12.2009");
        assert_eq!(shift("2024-03-14", -14).unwrap(), "2024-02-29");
        assert_eq!(shift("3/14/2024", 1).unwrap(), "3/15/2024");
        assert_eq!(shift("14. März 2024", 20).unwrap(), "3. April 2024");
        assert_eq!(shift("March 14, 2024", 20).unwrap(), "April 3, 2024");
        assert_eq!(shift("14 March 2024", 20).unwrap(), "3 April 2024");
        assert_eq!(shift("1.2.03", 0).unwrap(), "1.2.03");
        assert!(shift("31.02.2024", 1).is_none());
        assert!(shift("99.99.9999", 1).is_none());
    }
}
