//! CSV/TSV: Trennzeichen erkennen, Zellen mit Byte-Bereichen liefern (Quotes
//! bleiben außen vor), Spalten anhand der Kopfzeile typisieren.

use crate::model::Category;
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonHint {
    Full,
    First,
    Last,
}

#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub start: usize,
    pub end: usize,
}

/// Trennzeichen aus den ersten Zeilen raten: `;` `,` `\t` `|`.
pub fn detect_delimiter(text: &str) -> char {
    let sample: Vec<&str> = text.lines().take(10).collect();
    let mut best = (';', 0usize);
    for cand in [';', ',', '\t', '|'] {
        let counts: Vec<usize> = sample.iter().map(|l| l.matches(cand).count()).collect();
        let total: usize = counts.iter().sum();
        // Konsistenz belohnen: gleiche Anzahl je Zeile
        let consistent = counts.len() > 1 && counts.iter().all(|c| *c == counts[0]) && counts[0] > 0;
        let score = total * if consistent { 3 } else { 1 };
        if score > best.1 {
            best = (cand, score);
        }
    }
    best.0
}

/// Zellen je Zeile. Bei Quotes liegt der Bereich innerhalb der Anführungszeichen.
pub fn cells(text: &str, delim: char) -> Vec<Vec<Cell>> {
    let bytes = text.as_bytes();
    let d = delim as u8;
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    let mut row: Vec<Cell> = Vec::new();
    let mut i = 0;
    let n = bytes.len();
    while i <= n {
        if i == n {
            if !row.is_empty() || (n > 0 && bytes[n - 1] != b'\n') {
                row.push(Cell { start: n, end: n });
            }
            break;
        }
        if bytes[i] == b'"' {
            let start = i + 1;
            let mut j = start;
            loop {
                if j >= n {
                    break;
                }
                if bytes[j] == b'"' {
                    if j + 1 < n && bytes[j + 1] == b'"' {
                        j += 2;
                        continue;
                    }
                    break;
                }
                j += 1;
            }
            row.push(Cell { start, end: j.min(n) });
            i = (j + 1).min(n);
            // bis zum Trenner oder Zeilenende vorspulen
            while i < n && bytes[i] != d && bytes[i] != b'\n' {
                i += 1;
            }
        } else {
            let start = i;
            while i < n && bytes[i] != d && bytes[i] != b'\n' {
                i += 1;
            }
            let mut end = i;
            if end > start && bytes[end - 1] == b'\r' {
                end -= 1;
            }
            row.push(Cell { start, end });
        }
        if i < n && bytes[i] == d {
            i += 1;
            if i == n {
                row.push(Cell { start: n, end: n });
                break;
            }
            continue;
        }
        if i < n && bytes[i] == b'\n' {
            rows.push(std::mem::take(&mut row));
            i += 1;
            continue;
        }
        if i >= n {
            break;
        }
    }
    if !row.is_empty() {
        rows.push(row);
    }
    rows
}

/// Zeilen-Tokens, getrennt durch mindestens zwei Leerzeichen (Byte-Bereiche).
fn wide_tokens(line: &str, base: usize) -> Vec<Cell> {
    let mut cells = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    let n = bytes.len();
    while i < n {
        while i < n && (bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b'\r') {
            i += 1;
        }
        if i >= n {
            break;
        }
        let start = i;
        loop {
            // Ein einzelnes Leerzeichen gehört zum Token („Anna Berger“), zwei trennen
            while i < n && bytes[i] != b' ' && bytes[i] != b'\t' && bytes[i] != b'\r' {
                i += 1;
            }
            if i + 1 < n && bytes[i] == b' ' && bytes[i + 1] != b' ' && bytes[i + 1] != b'\t' {
                i += 1;
                continue;
            }
            break;
        }
        if i == start {
            i += 1;
            continue;
        }
        cells.push(Cell { start: base + start, end: base + i });
    }
    cells
}

/// Tabellen mit festen Spaltenbreiten: Kopfzeile mit ≥2 Spalten, darunter Zeilen,
/// deren Tokens sich den Kopfspalten zuordnen lassen. Endet an einer Leerzeile
/// oder einer Zeile mit weniger als zwei Tokens.
pub fn fixed_width_tables(text: &str) -> Vec<Vec<Vec<Cell>>> {
    let mut tables = Vec::new();
    let mut lines: Vec<(usize, &str)> = Vec::new();
    let mut pos = 0;
    for l in text.split('\n') {
        lines.push((pos, l));
        pos += l.len() + 1;
    }
    let mut i = 0;
    while i < lines.len() {
        let (base, line) = lines[i];
        let header = wide_tokens(line, base);
        let typed = header.iter().filter(|c| column_category(&text[c.start..c.end]).is_some()).count();
        if header.len() < 2 || typed == 0 {
            i += 1;
            continue;
        }
        let starts: Vec<usize> = header.iter().map(|c| c.start - base).collect();
        let mut rows: Vec<Vec<Cell>> = vec![header.clone()];
        let mut j = i + 1;
        while j < lines.len() {
            let (lb, ll) = lines[j];
            let toks = wide_tokens(ll, lb);
            if toks.len() < 2 || ll.trim().is_empty() {
                break;
            }
            // Tokens den Kopfspalten zuordnen (Spalte mit dem nächstliegenden Start links)
            let mut row: Vec<Option<Cell>> = vec![None; starts.len()];
            let mut ok = true;
            for t in toks {
                let off = t.start - lb;
                let col = starts.iter().rposition(|s| *s <= off + 1).unwrap_or(0);
                match &mut row[col] {
                    Some(c) => c.end = t.end,
                    None => row[col] = Some(t),
                }
                if starts.get(col + 1).map(|next| off >= *next + 2).unwrap_or(false) {
                    ok = false;
                }
            }
            if !ok {
                break;
            }
            rows.push(row.into_iter().map(|c| c.unwrap_or(Cell { start: lb, end: lb })).collect());
            j += 1;
        }
        if rows.len() >= 2 {
            tables.push(rows);
            i = j;
        } else {
            i += 1;
        }
    }
    tables
}

struct HeaderPatterns {
    list: Vec<(Regex, Category, PersonHint)>,
}

fn header_patterns() -> &'static HeaderPatterns {
    static P: OnceLock<HeaderPatterns> = OnceLock::new();
    P.get_or_init(|| {
        let mk = |re: &str, c: Category, h: PersonHint| (Regex::new(&format!("(?i)^(?:{re})$")).unwrap(), c, h);
        HeaderPatterns {
            list: vec![
                mk(r"vorname|vornamen|first ?name|given ?name|firstname", Category::Person, PersonHint::First),
                mk(r"nachname|familienname|zuname|last ?name|surname|family ?name|lastname", Category::Person, PersonHint::Last),
                mk(r"name|vor- und nachname|kunde|kundin|kundenname|ansprechpartner(?:in)?|mitarbeiter(?:in)?|patient(?:in)?|person|full ?name|customer ?name|contact|employee|mieter|halter|inhaber|versicherte[rn]?|schüler(?:in)?|student(?:in)?", Category::Person, PersonHint::Full),
                mk(r"e-?mail|email-?adresse|e-?mail-?adresse|mail|e-?mail ?address", Category::Email, PersonHint::Full),
                mk(r"telefon|tel\.?|telefonnummer|phone|phone ?number|mobil|mobile|handy|fax|festnetz|rufnummer", Category::Phone, PersonHint::Full),
                mk(r"iban|konto|kontonummer|konto-?nr\.?|bankkonto|account ?(?:no|number)?", Category::Iban, PersonHint::Full),
                mk(r"geb\.?|geburtsdatum|geburtstag|geb\.-?datum|birth ?date|birthday|dob|date of birth|datum|date|eintrittsdatum|austrittsdatum", Category::Date, PersonHint::Full),
                mk(r"straße|strasse|str\.?|anschrift|adresse|address|street|straße ?(?:und|/|,) ?(?:haus)?nr\.?", Category::Address, PersonHint::Full),
                mk(r"plz|postleitzahl|zip|zip ?code|postcode|postal ?code", Category::PostalCode, PersonHint::Full),
                mk(r"kd-?nr\.?|kunden-?nr\.?|kundennummer|kunden-?id|id|nr\.?|customer ?(?:id|no\.?|number)|mitglieds-?nr\.?|mitgliedsnummer|aktenzeichen|az\.?|personalnummer|personal-?nr\.?|vertragsnummer|vertrags-?nr\.?|mitarbeiternummer|mitarbeiter-?nr\.?|rechnungs-?nr\.?|rechnungsnummer|patienten-?nr\.?|fall-?nr\.?|fallnummer|case ?(?:id|no\.?)|employee ?(?:id|no\.?)|member ?(?:id|no\.?)|invoice ?(?:no\.?|number)|ticket", Category::CustomerId, PersonHint::Full),
                mk(r"steuer-?id|steuernummer|steuer-?nr\.?|tax ?id|tin|ust-?id|ust-?idnr\.?|vat ?(?:id|no\.?)", Category::TaxId, PersonHint::Full),
                mk(r"sv-?nr\.?|sozialversicherungsnummer|rentenversicherungsnummer|rv-?nr\.?|versichertennummer|versicherungsnummer|kvnr|insurance ?(?:no\.?|number|id)", Category::Insurance, PersonHint::Full),
                mk(r"kennzeichen|kfz-?kennzeichen|kfz|amtl\.? kennzeichen|plate|license ?plate|number ?plate", Category::Plate, PersonHint::Full),
                mk(r"ip|ip-?adresse|ip ?address|ipv4|ipv6|client-?ip|remote-?addr", Category::Ip, PersonHint::Full),
                mk(r"kreditkarte|kreditkartennummer|kartennummer|karten-?nr\.?|card ?(?:no\.?|number)|credit ?card|pan", Category::CreditCard, PersonHint::Full),
            ],
        }
    })
}

/// Kategorie für eine Spaltenüberschrift (leer = keine Typisierung).
pub fn column_category(header: &str) -> Option<(Category, PersonHint)> {
    let h = header.trim().trim_matches('"').trim();
    if h.is_empty() {
        return None;
    }
    header_patterns().list.iter().find(|(re, _, _)| re.is_match(h)).map(|(_, c, hint)| (*c, *hint))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delimiter_and_cells() {
        assert_eq!(detect_delimiter("a;b;c\n1;2;3\n"), ';');
        assert_eq!(detect_delimiter("a,b,c\n1,2,3\n"), ',');
        assert_eq!(detect_delimiter("a\tb\n1\t2\n"), '\t');
        let text = "Name;Ort\n\"Berger, Anna\";Köln\r\nJonas;\"Bonn\"\n";
        let rows = cells(text, ';');
        assert_eq!(rows.len(), 3);
        assert_eq!(&text[rows[1][0].start..rows[1][0].end], "Berger, Anna");
        assert_eq!(&text[rows[1][1].start..rows[1][1].end], "Köln");
        assert_eq!(&text[rows[2][1].start..rows[2][1].end], "Bonn");
    }

    #[test]
    fn wide_tokens_crlf() {
        let t = "Name  Ort\r\nAnna Berger  Köln\r\n";
        let tabs = fixed_width_tables(t);
        assert_eq!(tabs.len(), 1);
        assert_eq!(&t[tabs[0][1][0].start..tabs[0][1][0].end], "Anna Berger");
        assert_eq!(&t[tabs[0][1][1].start..tabs[0][1][1].end], "Köln");
        assert!(wide_tokens("\r", 0).is_empty());
    }

    #[test]
    fn fixed_width() {
        let text = "Brief\n\nKd-Nr.  Name          Ort    Geb.\n10482   Anna Berger   Köln   02.07.1981\n10483   Jonas Berger  Köln   19.11.2009\n\nGruß\n";
        let tables = fixed_width_tables(text);
        assert_eq!(tables.len(), 1);
        let t = &tables[0];
        assert_eq!(t.len(), 3);
        assert_eq!(&text[t[0][1].start..t[0][1].end], "Name");
        assert_eq!(&text[t[1][0].start..t[1][0].end], "10482");
        assert_eq!(&text[t[1][1].start..t[1][1].end], "Anna Berger");
        assert_eq!(&text[t[2][3].start..t[2][3].end], "19.11.2009");
    }

    #[test]
    fn headers() {
        assert_eq!(column_category("Vorname").unwrap(), (Category::Person, PersonHint::First));
        assert_eq!(column_category("E-Mail").unwrap().0, Category::Email);
        assert_eq!(column_category("Kd-Nr.").unwrap().0, Category::CustomerId);
        assert_eq!(column_category("Geb.").unwrap().0, Category::Date);
        assert!(column_category("Ort").is_none());
        assert!(column_category("Betrag").is_none());
    }
}
