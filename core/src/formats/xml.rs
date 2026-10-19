//! XML-Container (DOCX, XLSX, ODF): ZIP öffnen, Textknoten der relevanten Teile
//! einsammeln, als virtuellen Text (ein Knoten je Zeile) anbieten und beim
//! Schreiben die Knoten austauschen. Alles andere im Archiv bleibt Byte für Byte.

use super::{CellRef, Document, ForcedLine, Kind};
use crate::csv::{column_category, PersonHint};
use crate::formats::text::Encoding;
use crate::model::Category;
use regex::Regex;
use std::io::{Cursor, Read, Write};
use std::sync::OnceLock;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// Ein Textknoten: in welchem Archiv-Teil, welcher Byte-Bereich (ohne Tags),
/// und welche Zeile des virtuellen Textes.
#[derive(Debug, Clone)]
pub struct Node {
    pub part: usize,
    pub start: usize,
    pub end: usize,
}

pub struct Container {
    pub kind: Kind,
    pub bytes: Vec<u8>,
    /// Bearbeitete Teile: (Name, Inhalt als String)
    pub parts: Vec<(String, String)>,
    pub nodes: Vec<Node>,
}

fn re_docx() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // `w:delText` = gelöschter Text aus „Änderungen verfolgen“ — bleibt im Dokument und muss mit;
    // `w:instrText` = Feldfunktionen (z. B. `HYPERLINK "mailto:anna@…"`, ältere Dokumente/Serienbriefe)
    R.get_or_init(|| Regex::new(r"<w:(?:t|delText|instrText|delInstrText)(?:\s[^>]*)?>([^<]*)</w:(?:t|delText|instrText|delInstrText)>").unwrap())
}
/// Kommentare in XLSX (klassisch `<t>`, Threaded Comments `<text>`).
fn re_xlsx_comment() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"<(?:t|text)(?:\s[^>]*)?>([^<]*)</(?:t|text)>").unwrap())
}
/// Externe Beziehungen (Hyperlinks, `mailto:`) in `.rels`-Teilen: Wert von `Target`.
fn re_rel() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"<Relationship\b[^>]*>"#).unwrap())
}
fn re_rel_target() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"\sTarget="([^"]*)""#).unwrap())
}
/// Externe Links in ODF (`xlink:href="mailto:…"`, `https:…`) — interne Verweise (`#…`, `Pictures/…`) nicht.
fn re_odf_href() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"\sxlink:href="([A-Za-z][A-Za-z0-9+.-]*:[^"]*)""#).unwrap())
}

/// Obergrenze je Archiv-Teil (entpackt) — schützt vor ZIP-Bomben.
pub const MAX_PART_BYTES: u64 = 256 * 1024 * 1024;

/// Obergrenze für alle Textteile eines Dokuments zusammen (entpackt).
pub const MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;

/// Archiv-Eintrag mit Größenbremse lesen (die Größenangabe im ZIP kann lügen).
pub(crate) fn read_limited(r: &mut impl Read, declared: u64, name: &str) -> Result<Vec<u8>, String> {
    if declared > MAX_PART_BYTES {
        return Err(format!("{name}: Archiv-Teil zu groß ({} MB, Grenze {} MB)", declared / 1_048_576, MAX_PART_BYTES / 1_048_576));
    }
    let mut buf = Vec::new();
    r.take(MAX_PART_BYTES + 1).read_to_end(&mut buf).map_err(|e| format!("{name}: {e}"))?;
    if buf.len() as u64 > MAX_PART_BYTES {
        return Err(format!("{name}: Archiv-Teil zu groß (Grenze {} MB)", MAX_PART_BYTES / 1_048_576));
    }
    Ok(buf)
}

pub(crate) fn read_limited_string(r: &mut impl Read, declared: u64, name: &str) -> Result<String, String> {
    String::from_utf8(read_limited(r, declared, name)?).map_err(|e| format!("{name}: {e}"))
}

fn is_rels(name: &str) -> bool {
    name.ends_with(".rels")
}
fn re_xlsx() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"<t(?:\s[^>]*)?>([^<]*)</t>").unwrap())
}
fn re_odf() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r">([^<]+)<").unwrap())
}

pub fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        if let Some(j) = rest.find(';') {
            let ent = &rest[1..j];
            let decoded = match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                e if e.starts_with("#x") => u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32),
                e if e.starts_with('#') => e[1..].parse::<u32>().ok().and_then(char::from_u32),
                _ => None,
            };
            match decoded {
                Some(c) => {
                    out.push(c);
                    rest = &rest[j + 1..];
                }
                None => {
                    out.push('&');
                    rest = &rest[1..];
                }
            }
        } else {
            out.push_str(rest);
            rest = "";
        }
    }
    out.push_str(rest);
    out
}

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

fn part_names(archive: &mut ZipArchive<Cursor<&[u8]>>, kind: &Kind) -> Vec<String> {
    let mut names: Vec<String> = (0..archive.len()).filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string())).collect();
    names.sort();
    match kind {
        Kind::Docx => names
            .into_iter()
            .filter(|n| n == "word/document.xml" || n.starts_with("word/footnotes") || n.starts_with("word/endnotes") || n.starts_with("word/header") || n.starts_with("word/footer") || n.starts_with("word/comments") || (n.starts_with("word/_rels/") && is_rels(n)))
            .collect(),
        Kind::Xlsx => names
            .into_iter()
            .filter(|n| {
                n == "xl/sharedStrings.xml"
                    || (n.starts_with("xl/worksheets/sheet") && n.ends_with(".xml"))
                    || (n.starts_with("xl/comments") && n.ends_with(".xml"))
                    || (n.starts_with("xl/threadedComments/") && n.ends_with(".xml"))
                    || (n.starts_with("xl/worksheets/_rels/") && is_rels(n))
            })
            .collect(),
        _ => names.into_iter().filter(|n| n == "content.xml" || n == "styles.xml").collect(),
    }
}

pub fn open(bytes: Vec<u8>, kind: Kind) -> Result<Document, String> {
    let cursor = Cursor::new(bytes.as_slice());
    let mut archive = ZipArchive::new(cursor).map_err(|e| format!("Kein gültiges ZIP-Archiv: {e}"))?;
    let names = part_names(&mut archive, &kind);
    if names.is_empty() {
        return Err("Kein Dokumentinhalt im Archiv gefunden".into());
    }
    let mut parts: Vec<(String, String)> = Vec::new();
    let mut total = 0u64;
    for name in &names {
        let mut f = archive.by_name(name).map_err(|e| e.to_string())?;
        let size = f.size();
        let s = read_limited_string(&mut f, size, name)?;
        // Summe aller Textteile begrenzen — viele „kleine“ Bomben-Teile ergäben sonst Gigabytes
        total += s.len() as u64;
        if total > MAX_TOTAL_BYTES {
            return Err(format!("Dokumentinhalt zu groß (entpackt über {} MB)", MAX_TOTAL_BYTES / 1_048_576));
        }
        parts.push((name.clone(), s));
    }
    let re = match kind {
        Kind::Docx => re_docx(),
        Kind::Xlsx => re_xlsx(),
        _ => re_odf(),
    };
    let mut nodes = Vec::new();
    let mut text = String::new();
    let mut forced = Vec::new();
    let mut cells = Vec::new();
    if kind == Kind::Xlsx {
        xlsx_nodes(&parts, &mut nodes, &mut text, &mut forced, &mut cells);
        // Kommentare (klassisch + Threaded Comments) als eigene Zeilen
        for (pi, (name, content)) in parts.iter().enumerate() {
            if name.starts_with("xl/comments") || name.starts_with("xl/threadedComments/") {
                push_nodes(re_xlsx_comment(), pi, content, &mut nodes, &mut text);
            }
        }
    } else {
        for (pi, (name, content)) in parts.iter().enumerate() {
            if is_rels(name) {
                continue;
            }
            push_nodes(re, pi, content, &mut nodes, &mut text);
            if kind == Kind::Odf {
                push_nodes(re_odf_href(), pi, content, &mut nodes, &mut text);
            }
        }
    }
    // Externe Link-Ziele (DOCX/XLSX): `mailto:anna@…` in den Beziehungen
    for (pi, (name, content)) in parts.iter().enumerate() {
        if !is_rels(name) {
            continue;
        }
        for rel in re_rel().find_iter(content) {
            if !rel.as_str().contains("TargetMode=\"External\"") {
                continue;
            }
            if let Some(c) = re_rel_target().captures(rel.as_str()) {
                let g = c.get(1).unwrap();
                if g.as_str().trim().is_empty() {
                    continue;
                }
                nodes.push(Node { part: pi, start: rel.start() + g.start(), end: rel.start() + g.end() });
                text.push_str(&unescape(g.as_str()).replace(['\n', '\r'], " "));
                text.push('\n');
            }
        }
    }
    let mut notes = Vec::new();
    match kind {
        Kind::Docx => notes.push("docx-runs".into()),
        Kind::Xlsx => notes.push("xlsx-cells".into()),
        _ => {}
    }
    notes.push("metadata".into());
    Ok(Document {
        kind: kind.clone(),
        text,
        encoding: Encoding { name: "UTF-8".into(), bom: false, legacy: false },
        xml: Some(Container { kind, bytes, parts, nodes }),
        notes,
        forced,
        cells,
    })
}

/// Alle Treffer von `re` (Gruppe 1 = Textknoten) eines Teils als Zeilen aufnehmen.
fn push_nodes(re: &Regex, pi: usize, content: &str, nodes: &mut Vec<Node>, text: &mut String) {
    for c in re.captures_iter(content) {
        let g = c.get(1).unwrap();
        let raw = g.as_str();
        if raw.trim().is_empty() {
            continue;
        }
        nodes.push(Node { part: pi, start: g.start(), end: g.end() });
        text.push_str(&unescape(raw).replace(['\n', '\r'], " "));
        text.push('\n');
    }
}

/// Kategorien, die auch in Zahlenzellen sinnvoll sind (Datum = Excel-Seriennummer).
fn numeric_ok(c: Category) -> bool {
    matches!(c, Category::CustomerId | Category::Phone | Category::PostalCode | Category::TaxId | Category::Insurance | Category::CreditCard | Category::Date)
}

/// Spaltenindex aus einer Zellreferenz („B7“ → 1). Excel kennt höchstens 16 384 Spalten (XFD);
/// längere Buchstabenfolgen aus manipulierten Dateien werden gekappt statt überzulaufen.
pub(crate) fn col_index(cell_ref: &str) -> usize {
    let mut n = 0usize;
    for c in cell_ref.chars().take_while(|c| c.is_ascii_alphabetic()) {
        n = n.saturating_mul(26).saturating_add(c.to_ascii_uppercase() as usize - 'A' as usize + 1).min(16_384);
    }
    n.saturating_sub(1)
}

/// XLSX: sharedStrings als Knoten je `<t>` (gruppiert nach `<si>`), Blätter zellenweise —
/// Inline-Strings als Knoten, Zahlenzellen in typisierten Spalten ebenfalls. Spalten werden
/// über die Kopfzeile (erste Zeile) typisiert wie bei CSV.
fn xlsx_nodes(parts: &[(String, String)], nodes: &mut Vec<Node>, text: &mut String, forced: &mut Vec<ForcedLine>, cells: &mut Vec<CellRef>) {
    static SI: OnceLock<Regex> = OnceLock::new();
    static ROW: OnceLock<Regex> = OnceLock::new();
    static CELL: OnceLock<Regex> = OnceLock::new();
    static ATTR_R: OnceLock<Regex> = OnceLock::new();
    static ATTR_T: OnceLock<Regex> = OnceLock::new();
    static V: OnceLock<Regex> = OnceLock::new();
    let si = SI.get_or_init(|| Regex::new(r"(?s)<si>(.*?)</si>").unwrap());
    let row_re = ROW.get_or_init(|| Regex::new(r"(?s)<row\b[^>]*>(.*?)</row>").unwrap());
    let cell_re = CELL.get_or_init(|| Regex::new(r#"(?s)<c\b([^>]*?)(?:/>|>(.*?)</c>)"#).unwrap());
    let attr_r = ATTR_R.get_or_init(|| Regex::new(r#"\br="([A-Z]+)\d*""#).unwrap());
    let attr_t = ATTR_T.get_or_init(|| Regex::new(r#"\bt="([^"]*)""#).unwrap());
    let v_re = V.get_or_init(|| Regex::new(r"(?s)<v>([^<]*)</v>").unwrap());
    let t_re = re_xlsx();

    // 1) sharedStrings: je <si> die <t>-Knoten und der zusammengesetzte Text
    let mut shared_text: Vec<String> = Vec::new();
    let mut shared_nodes: Vec<Vec<usize>> = Vec::new();
    if let Some(pi) = parts.iter().position(|(n, _)| n == "xl/sharedStrings.xml") {
        let content = &parts[pi].1;
        for m in si.captures_iter(content) {
            let inner = m.get(1).unwrap();
            let mut full = String::new();
            let mut idxs = Vec::new();
            for c in t_re.captures_iter(inner.as_str()) {
                let g = c.get(1).unwrap();
                let raw = g.as_str();
                full.push_str(&unescape(raw));
                if raw.trim().is_empty() {
                    continue;
                }
                idxs.push(nodes.len());
                nodes.push(Node { part: pi, start: inner.start() + g.start(), end: inner.start() + g.end() });
                text.push_str(&unescape(raw).replace(['\n', '\r'], " "));
                text.push('\n');
            }
            shared_text.push(full);
            shared_nodes.push(idxs);
        }
    }
    let mut shared_forced: std::collections::HashSet<usize> = std::collections::HashSet::new();

    // 2) Blätter
    for (pi, (name, content)) in parts.iter().enumerate() {
        if !name.starts_with("xl/worksheets/") || is_rels(name) {
            continue;
        }
        let sheet_id = name.trim_start_matches("xl/worksheets/").trim_end_matches(".xml").to_string();
        // Kopfzeile: unter den ersten zehn Zeilen die mit den meisten typisierbaren Überschriften
        let mut header_row = 0usize;
        let mut best = 0usize;
        for (ri, r) in row_re.captures_iter(content).take(10).enumerate() {
            let mut typed_n = 0;
            for c in cell_re.captures_iter(&r[1]) {
                let typ = attr_t.captures(&c[1]).map(|a| a[1].to_string()).unwrap_or_default();
                let body = c.get(2).map(|b| b.as_str()).unwrap_or("");
                let cell_text = match typ.as_str() {
                    "s" => v_re.captures(body).and_then(|v| v[1].trim().parse::<usize>().ok()).and_then(|i| shared_text.get(i).cloned()).unwrap_or_default(),
                    "inlineStr" => t_re.captures_iter(body).map(|x| unescape(&x[1])).collect(),
                    _ => String::new(),
                };
                if column_category(&cell_text).is_some() {
                    typed_n += 1;
                }
            }
            if typed_n > best {
                best = typed_n;
                header_row = ri;
            }
        }
        let mut typed: Vec<Option<(Category, PersonHint)>> = Vec::new();
        for (ri, r) in row_re.captures_iter(content).enumerate() {
            let inner = r.get(1).unwrap();
            let mut col_seq = 0usize;
            for c in cell_re.captures_iter(inner.as_str()) {
                let attrs = &c[1];
                let col = attr_r.captures(attrs).map(|a| col_index(&a[1])).unwrap_or(col_seq);
                col_seq = col + 1;
                let typ = attr_t.captures(attrs).map(|a| a[1].to_string()).unwrap_or_default();
                let body = match c.get(2) {
                    Some(b) => b,
                    None => continue,
                };
                let body_off = inner.start() + body.start();
                // Zelltext ermitteln (für die Kopfzeile) und Knoten anlegen
                let (cell_text, own_nodes): (String, Vec<usize>) = match typ.as_str() {
                    "s" => {
                        let idx = v_re.captures(body.as_str()).and_then(|v| v[1].trim().parse::<usize>().ok());
                        match idx {
                            Some(i) if i < shared_text.len() => (shared_text[i].clone(), shared_nodes[i].clone()),
                            _ => (String::new(), vec![]),
                        }
                    }
                    "inlineStr" => {
                        let mut full = String::new();
                        let mut idxs = Vec::new();
                        for tc in t_re.captures_iter(body.as_str()) {
                            let g = tc.get(1).unwrap();
                            let raw = g.as_str();
                            full.push_str(&unescape(raw));
                            if raw.trim().is_empty() {
                                continue;
                            }
                            idxs.push(nodes.len());
                            nodes.push(Node { part: pi, start: body_off + g.start(), end: body_off + g.end() });
                            text.push_str(&unescape(raw).replace(['\n', '\r'], " "));
                            text.push('\n');
                        }
                        (full, idxs)
                    }
                    "b" | "e" | "str" => (String::new(), vec![]),
                    _ => {
                        // Zahl: nur in typisierten Spalten als Knoten aufnehmen
                        match (ri > header_row, typed.get(col).copied().flatten(), v_re.captures(body.as_str())) {
                            (true, Some((cat, _)), Some(v)) if numeric_ok(cat) => {
                                let g = v.get(1).unwrap();
                                let idx = nodes.len();
                                nodes.push(Node { part: pi, start: body_off + g.start(), end: body_off + g.end() });
                                text.push_str(g.as_str().trim());
                                text.push('\n');
                                (g.as_str().to_string(), vec![idx])
                            }
                            _ => (String::new(), vec![]),
                        }
                    }
                };
                if ri == header_row {
                    while typed.len() <= col {
                        typed.push(None);
                    }
                    typed[col] = column_category(&cell_text);
                    continue;
                }
                if ri < header_row {
                    continue;
                }
                for n in &own_nodes {
                    cells.push(CellRef { line: *n, column: format!("{sheet_id}:{col}") });
                }
                if let Some(Some((cat, hint))) = typed.get(col) {
                    for n in own_nodes {
                        // Shared Strings nur einmal typisieren (erste Spalte gewinnt)
                        if typ == "s" && !shared_forced.insert(n) {
                            continue;
                        }
                        forced.push(ForcedLine { line: n, category: *cat, hint: *hint });
                    }
                }
            }
        }
    }
}

/// Platzhalter für entfernte Personen-Metadaten.
pub const META_ANON: &str = "anonym";

fn element_re(tag: &str) -> Regex {
    let t = regex::escape(tag);
    Regex::new(&format!(r"(<{t}(?:\s[^>]*)?>)[^<]*(</{t}>)")).unwrap()
}
fn attr_re(attr: &str) -> Regex {
    let a = regex::escape(attr);
    Regex::new(&format!(r#"(\s{a}=")[^"]*(")"#)).unwrap()
}

struct Scrubber {
    clear: Vec<Regex>,
    anon: Vec<Regex>,
    initials: Vec<Regex>,
}

fn scrubber(clear: &[&str], anon_elems: &[&str], anon_attrs: &[&str], initial_attrs: &[&str]) -> Scrubber {
    Scrubber {
        clear: clear.iter().map(|t| element_re(t)).collect(),
        anon: anon_elems.iter().map(|t| element_re(t)).chain(anon_attrs.iter().map(|a| attr_re(a))).collect(),
        initials: initial_attrs.iter().map(|a| attr_re(a)).collect(),
    }
}

/// Personenbezogene Metadaten eines Archiv-Teils entfernen: Dokument-Eigenschaften
/// (Autor, zuletzt geändert von, Titel, Thema, Stichwörter, Firma, Manager), Autoren von
/// Kommentaren und Überarbeitungen, Personenlisten. `None` = Teil bleibt unverändert.
pub fn scrub_metadata(name: &str, content: &str) -> Option<String> {
    static CORE: OnceLock<Scrubber> = OnceLock::new();
    static APP: OnceLock<Scrubber> = OnceLock::new();
    static WORD: OnceLock<Scrubber> = OnceLock::new();
    static XL_COMMENTS: OnceLock<Scrubber> = OnceLock::new();
    static XL_PERSONS: OnceLock<Scrubber> = OnceLock::new();
    static ODF_META: OnceLock<Scrubber> = OnceLock::new();
    static ODF_CONTENT: OnceLock<Scrubber> = OnceLock::new();
    let sc = match name {
        "docProps/core.xml" => CORE.get_or_init(|| scrubber(&["dc:creator", "cp:lastModifiedBy", "dc:title", "dc:subject", "dc:description", "cp:keywords"], &[], &[], &[])),
        "docProps/app.xml" => APP.get_or_init(|| scrubber(&["Company", "Manager"], &[], &[], &[])),
        n if n.starts_with("word/") && n.ends_with(".xml") => WORD.get_or_init(|| scrubber(&[], &[], &["w:author", "w15:author", "w15:userId"], &["w:initials"])),
        n if n.starts_with("xl/comments") && n.ends_with(".xml") => XL_COMMENTS.get_or_init(|| scrubber(&[], &["author"], &[], &[])),
        n if n.starts_with("xl/persons/") && n.ends_with(".xml") => XL_PERSONS.get_or_init(|| scrubber(&[], &[], &["displayName", "userId"], &[])),
        "meta.xml" => ODF_META.get_or_init(|| scrubber(&["meta:initial-creator", "dc:creator", "meta:printed-by", "dc:title", "dc:subject", "dc:description", "meta:keyword"], &[], &[], &[])),
        "content.xml" | "styles.xml" => ODF_CONTENT.get_or_init(|| scrubber(&[], &["dc:creator"], &[], &[])),
        _ => return None,
    };
    let mut out = std::borrow::Cow::Borrowed(content);
    for re in &sc.clear {
        if let std::borrow::Cow::Owned(s) = re.replace_all(&out, "${1}${2}") {
            out = std::borrow::Cow::Owned(s);
        }
    }
    let anon = format!("${{1}}{META_ANON}${{2}}");
    for re in &sc.anon {
        if let std::borrow::Cow::Owned(s) = re.replace_all(&out, anon.as_str()) {
            out = std::borrow::Cow::Owned(s);
        }
    }
    for re in &sc.initials {
        if let std::borrow::Cow::Owned(s) = re.replace_all(&out, "${1}A${2}") {
            out = std::borrow::Cow::Owned(s);
        }
    }
    match out {
        std::borrow::Cow::Owned(s) if s != content => Some(s),
        _ => None,
    }
}

/// Teile, die `scrub_metadata` oder `drop_thumbnail_refs` umschreiben könnten.
fn may_hold_metadata(name: &str) -> bool {
    matches!(name, "docProps/core.xml" | "docProps/app.xml" | "meta.xml" | "content.xml" | "styles.xml" | "META-INF/manifest.xml" | "_rels/.rels" | "[Content_Types].xml")
        || (name.ends_with(".xml") && (name.starts_with("word/") || name.starts_with("xl/comments") || name.starts_with("xl/persons/")))
}

/// Vorschaubilder der ersten Seite (LibreOffice speichert sie immer, Word mit „Vorschaubild
/// speichern“) zeigen das Original samt Namen — sie fallen beim Neupacken weg.
fn is_thumbnail(name: &str) -> bool {
    name.starts_with("Thumbnails/") || name.starts_with("docProps/thumbnail.")
}

/// Verweise auf entfernte Vorschaubilder aus Manifest (ODF), Beziehungen und Inhaltstypen
/// (OOXML) streichen, damit Word/LibreOffice keine fehlende Datei melden.
fn drop_thumbnail_refs(name: &str, content: &str) -> Option<String> {
    static R: OnceLock<Regex> = OnceLock::new();
    if !matches!(name, "META-INF/manifest.xml" | "_rels/.rels" | "[Content_Types].xml") {
        return None;
    }
    let re = R.get_or_init(|| Regex::new(r#"<(?:manifest:file-entry|Relationship|Override)\b[^>]*="/?(?:Thumbnails/|docProps/thumbnail\.)[^"]*"[^>]*/>"#).unwrap());
    match re.replace_all(content, "") {
        std::borrow::Cow::Owned(s) => Some(s),
        _ => None,
    }
}

/// Neuen virtuellen Text (Zeile je Knoten) in die XML-Teile schreiben und das Archiv neu packen.
pub fn rebuild(c: &Container, old_text: &str, new_text: &str) -> Result<Vec<u8>, String> {
    let old_lines: Vec<&str> = old_text.split('\n').collect();
    let new_lines: Vec<&str> = new_text.split('\n').collect();
    if new_lines.len() != old_lines.len() {
        return Err("Interner Fehler: Zeilenzahl der Vorschau passt nicht zum Dokument".into());
    }
    // Je Teil: Knoten von hinten nach vorn ersetzen, damit Offsets gültig bleiben
    // (Textknoten und Link-Ziele eines Teils stehen nicht in Offset-Reihenfolge)
    let mut contents: Vec<String> = c.parts.iter().map(|(_, s)| s.clone()).collect();
    let mut order: Vec<usize> = (0..c.nodes.len()).collect();
    order.sort_by(|&a, &b| (c.nodes[b].part, c.nodes[b].start).cmp(&(c.nodes[a].part, c.nodes[a].start)));
    for li in order {
        let node = &c.nodes[li];
        if old_lines[li] == new_lines[li] {
            continue;
        }
        let part = &mut contents[node.part];
        let replacement = escape(new_lines[li]);
        part.replace_range(node.start..node.end, &replacement);
    }
    for (pi, (name, _)) in c.parts.iter().enumerate() {
        if let Some(clean) = scrub_metadata(name, &contents[pi]) {
            contents[pi] = clean;
        }
    }
    // Leerzeichen am Rand brauchen xml:space="preserve" (DOCX); vorhandene Attribute behalten
    let cursor = Cursor::new(c.bytes.as_slice());
    let mut archive = ZipArchive::new(cursor).map_err(|e| format!("Kein gültiges ZIP-Archiv: {e}"))?;
    let mut out = Cursor::new(Vec::with_capacity(c.bytes.len()));
    {
        let mut writer = ZipWriter::new(&mut out);
        for i in 0..archive.len() {
            let mut f = archive.by_index(i).map_err(|e| e.to_string())?;
            let name = f.name().to_string();
            let is_dir = f.is_dir();
            if is_thumbnail(&name) {
                continue;
            }
            let method = if name == "mimetype" { CompressionMethod::Stored } else { CompressionMethod::Deflated };
            let opts = SimpleFileOptions::default().compression_method(method).last_modified_time(f.last_modified().unwrap_or_default());
            if is_dir {
                writer.add_directory(name, opts).map_err(|e| e.to_string())?;
                continue;
            }
            let part_index = c.parts.iter().position(|(n, _)| *n == name);
            if part_index.is_none() && name != "mimetype" && !may_hold_metadata(&name) {
                // Unberührte Teile (Bilder, Stile …) komprimiert übernehmen: kein Entpacken,
                // Ausgabe bleibt so groß wie die Eingabe (ZIP-Bomben laufen nicht voll)
                writer.raw_copy_file(f).map_err(|e| e.to_string())?;
                continue;
            }
            writer.start_file(name.clone(), opts).map_err(|e| e.to_string())?;
            if let Some(pi) = part_index {
                writer.write_all(contents[pi].as_bytes()).map_err(|e| e.to_string())?;
            } else {
                let size = f.size();
                let buf = read_limited(&mut f, size, &name)?;
                // Metadaten (Autor, Firma, Personen) in Teilen ohne Textknoten leeren,
                // Verweise auf entfernte Vorschaubilder streichen
                let clean = std::str::from_utf8(&buf).ok().and_then(|s| {
                    let meta = scrub_metadata(&name, s);
                    let base = meta.as_deref().unwrap_or(s);
                    drop_thumbnail_refs(&name, base).or(meta)
                });
                match clean {
                    Some(clean) => writer.write_all(clean.as_bytes()).map_err(|e| e.to_string())?,
                    None => writer.write_all(&buf).map_err(|e| e.to_string())?,
                }
            }
        }
        writer.finish().map_err(|e| e.to_string())?;
    }
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_docx(paragraphs: &[&str]) -> Vec<u8> {
        let body: String = paragraphs.iter().map(|p| format!("<w:p><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>", escape(p))).collect();
        let doc = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><w:document xmlns:w=\"x\"><w:body>{body}</w:body></w:document>");
        let mut out = Cursor::new(Vec::new());
        {
            let mut w = ZipWriter::new(&mut out);
            let o = SimpleFileOptions::default();
            w.start_file("[Content_Types].xml", o).unwrap();
            w.write_all(b"<Types/>").unwrap();
            w.start_file("word/document.xml", o).unwrap();
            w.write_all(doc.as_bytes()).unwrap();
            w.finish().unwrap();
        }
        out.into_inner()
    }

    fn make_xlsx() -> Vec<u8> {
        let ss = ["Kd-Nr", "Name", "Geb.", "Anna Berger", "Zyxwacz Q"];
        let sst = format!("<sst xmlns=\"x\">{}</sst>", ss.iter().map(|s| format!("<si><t>{}</t></si>", escape(s))).collect::<String>());
        let sheet = "<worksheet xmlns=\"x\"><sheetData><row r=\"1\"><c r=\"A1\" t=\"s\"><v>0</v></c><c r=\"B1\" t=\"s\"><v>1</v></c><c r=\"C1\" t=\"s\"><v>2</v></c></row><row r=\"2\"><c r=\"A2\"><v>10482</v></c><c r=\"B2\" t=\"s\"><v>3</v></c><c r=\"C2\"><v>29774</v></c></row><row r=\"3\"><c r=\"A3\"><v>10483</v></c><c r=\"B3\" t=\"inlineStr\"><is><t>Zyxwacz Q</t></is></c><c r=\"C3\" s=\"1\"><v>40000.5</v></c></row></sheetData></worksheet>";
        let mut out = Cursor::new(Vec::new());
        {
            let mut w = ZipWriter::new(&mut out);
            let o = SimpleFileOptions::default();
            w.start_file("xl/workbook.xml", o).unwrap();
            w.write_all(b"<workbook/>").unwrap();
            w.start_file("xl/sharedStrings.xml", o).unwrap();
            w.write_all(sst.as_bytes()).unwrap();
            w.start_file("xl/worksheets/sheet1.xml", o).unwrap();
            w.write_all(sheet.as_bytes()).unwrap();
            w.finish().unwrap();
        }
        out.into_inner()
    }

    #[test]
    fn xlsx_header_in_second_row() {
        let ss = ["Hardware-Liste", "Abteilung", "User", "IT", "Joey Rösner"];
        let sst = format!("<sst xmlns=\"x\">{}</sst>", ss.iter().map(|s| format!("<si><t>{}</t></si>", escape(s))).collect::<String>());
        let sheet = "<worksheet xmlns=\"x\"><sheetData><row r=\"1\"><c r=\"A1\" t=\"s\"><v>0</v></c></row><row r=\"2\"><c r=\"A2\" t=\"s\"><v>1</v></c><c r=\"B2\" t=\"s\"><v>2</v></c></row><row r=\"3\"><c r=\"A3\" t=\"s\"><v>3</v></c><c r=\"B3\" t=\"s\"><v>4</v></c></row></sheetData></worksheet>";
        let mut out = Cursor::new(Vec::new());
        {
            let mut w = ZipWriter::new(&mut out);
            let o = SimpleFileOptions::default();
            w.start_file("xl/sharedStrings.xml", o).unwrap();
            w.write_all(sst.as_bytes()).unwrap();
            w.start_file("xl/worksheets/sheet1.xml", o).unwrap();
            w.write_all(sheet.as_bytes()).unwrap();
            w.finish().unwrap();
        }
        let doc = open(out.into_inner(), Kind::Xlsx).unwrap();
        let lines: Vec<&str> = doc.text.lines().collect();
        let forced: Vec<(&str, Category)> = doc.forced.iter().map(|f| (lines[f.line], f.category)).collect();
        assert_eq!(forced, vec![("Joey Rösner", Category::Person)], "{forced:?}");
        assert!(doc.cells.iter().any(|c| lines[c.line] == "IT" && c.column == "sheet1:0"));
    }

    #[test]
    fn xlsx_typed_columns_and_numbers() {
        let doc = open(make_xlsx(), Kind::Xlsx).unwrap();
        let lines: Vec<&str> = doc.text.lines().collect();
        assert!(lines.contains(&"10482") && lines.contains(&"10483") && lines.contains(&"29774") && lines.contains(&"40000.5"), "{lines:?}");
        assert!(lines.contains(&"Zyxwacz Q"));
        let forced: Vec<(String, Category)> = doc.forced.iter().map(|f| (lines[f.line].to_string(), f.category)).collect();
        assert!(forced.contains(&("10482".into(), Category::CustomerId)), "{forced:?}");
        assert!(forced.contains(&("Anna Berger".into(), Category::Person)));
        assert!(forced.contains(&("Zyxwacz Q".into(), Category::Person)));
        assert!(forced.contains(&("29774".into(), Category::Date)));
        // Kopfzeile selbst nicht typisiert
        assert!(!forced.iter().any(|(t, _)| t == "Name"));
        // Rückschreiben: Zahl ersetzen
        let new_text = doc.text.replace("10482", "84292").replace("29774", "29800");
        let rebuilt = rebuild(doc.xml.as_ref().unwrap(), &doc.text, &new_text).unwrap();
        let again = open(rebuilt, Kind::Xlsx).unwrap();
        assert!(again.text.contains("84292\n") && again.text.contains("29800\n"), "{}", again.text);
        assert!(again.xml.unwrap().parts.iter().any(|(_, c)| c.contains("<c r=\"A2\"><v>84292</v></c>")));
    }

    #[test]
    fn docx_roundtrip() {
        let bytes = make_docx(&["Sehr geehrte Frau Berger,", "Tom & Jerry <3", "  "]);
        let doc = open(bytes, Kind::Docx).unwrap();
        assert_eq!(doc.text, "Sehr geehrte Frau Berger,\nTom & Jerry <3\n");
        let new_text = "Sehr geehrte Frau Vogt,\nTim & Jerry <3\n";
        let rebuilt = rebuild(doc.xml.as_ref().unwrap(), &doc.text, new_text).unwrap();
        let again = open(rebuilt, Kind::Docx).unwrap();
        assert_eq!(again.text, new_text);
        let part = &again.xml.unwrap().parts[0].1;
        assert!(part.contains("Tim &amp; Jerry &lt;3"));
        assert!(part.contains("<w:t xml:space=\"preserve\">"));
    }

    fn zip_of(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        {
            let mut w = ZipWriter::new(&mut out);
            let o = SimpleFileOptions::default();
            for (n, c) in entries {
                w.start_file(*n, o).unwrap();
                w.write_all(c.as_bytes()).unwrap();
            }
            w.finish().unwrap();
        }
        out.into_inner()
    }

    fn part_of(bytes: &[u8], name: &str) -> String {
        let mut a = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut f = a.by_name(name).unwrap();
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        s
    }

    #[test]
    fn docx_deleted_text_links_and_metadata() {
        let doc = "<w:document><w:body><w:p><w:ins w:id=\"1\" w:author=\"Stefan Mau\"><w:r><w:t>Neu</w:t></w:r></w:ins><w:del w:id=\"2\" w:author=\"Stefan Mau\" w:initials=\"SM\"><w:r><w:delText>Anna Berger</w:delText></w:r></w:del><w:hyperlink r:id=\"rId5\"><w:r><w:t>Mail</w:t></w:r></w:hyperlink></w:p></w:body></w:document>";
        let rels = "<Relationships><Relationship Id=\"rId1\" Type=\"x/styles\" Target=\"styles.xml\"/><Relationship Id=\"rId5\" Type=\"x/hyperlink\" Target=\"mailto:anna.berger@example.org\" TargetMode=\"External\"/></Relationships>";
        let core = "<cp:coreProperties><dc:title>Brief an Anna Berger</dc:title><dc:creator>Stefan Mau</dc:creator><cp:lastModifiedBy>Stefan Mau</cp:lastModifiedBy><dcterms:created xsi:type=\"dcterms:W3CDTF\">2024-01-01T00:00:00Z</dcterms:created></cp:coreProperties>";
        let app = "<Properties><Company>Red6 GmbH</Company><Pages>1</Pages></Properties>";
        let bytes = zip_of(&[("word/document.xml", doc), ("word/_rels/document.xml.rels", rels), ("docProps/core.xml", core), ("docProps/app.xml", app)]);
        let d = open(bytes, Kind::Docx).unwrap();
        assert_eq!(d.text, "Neu\nAnna Berger\nMail\nmailto:anna.berger@example.org\n");
        let new_text = "Neu\nPaula Vogt\nMail\nmailto:paula.vogt@example.org\n";
        let out = rebuild(d.xml.as_ref().unwrap(), &d.text, new_text).unwrap();
        let docx = part_of(&out, "word/document.xml");
        assert!(docx.contains("<w:delText>Paula Vogt</w:delText>") && !docx.contains("Stefan") && docx.contains("w:initials=\"A\""), "{docx}");
        let rels2 = part_of(&out, "word/_rels/document.xml.rels");
        assert!(rels2.contains("Target=\"mailto:paula.vogt@example.org\"") && rels2.contains("Target=\"styles.xml\""), "{rels2}");
        let core2 = part_of(&out, "docProps/core.xml");
        assert!(!core2.contains("Stefan") && !core2.contains("Anna") && core2.contains("<dc:creator></dc:creator>") && core2.contains("2024-01-01"), "{core2}");
        assert!(!part_of(&out, "docProps/app.xml").contains("Red6"));
        // Unverändertes Dokument: Metadaten werden trotzdem entfernt
        let out2 = rebuild(d.xml.as_ref().unwrap(), &d.text, &d.text).unwrap();
        assert!(!part_of(&out2, "docProps/core.xml").contains("Stefan"));
    }

    #[test]
    fn xlsx_comments_are_checked_and_authors_removed() {
        let sheet = "<worksheet><sheetData><row r=\"1\"><c r=\"A1\" t=\"inlineStr\"><is><t>x</t></is></c></row></sheetData></worksheet>";
        let comments = "<comments><authors><author>Stefan Mau</author></authors><commentList><comment ref=\"A1\" authorId=\"0\"><text><r><t>Frau Berger anrufen</t></r></text></comment></commentList></comments>";
        let persons = "<personList><person displayName=\"Stefan Mau\" id=\"{1}\" userId=\"stefan@example.org\" providerId=\"AD\"/></personList>";
        let bytes = zip_of(&[("xl/worksheets/sheet1.xml", sheet), ("xl/comments1.xml", comments), ("xl/persons/person.xml", persons)]);
        let d = open(bytes, Kind::Xlsx).unwrap();
        assert!(d.text.contains("Frau Berger anrufen\n"), "{}", d.text);
        assert!(!d.text.contains("Stefan"));
        let new_text = d.text.replace("Berger", "Vogt");
        let out = rebuild(d.xml.as_ref().unwrap(), &d.text, &new_text).unwrap();
        let c = part_of(&out, "xl/comments1.xml");
        assert!(c.contains("<author>anonym</author>") && c.contains("Frau Vogt anrufen"), "{c}");
        let p = part_of(&out, "xl/persons/person.xml");
        assert!(!p.contains("Stefan") && !p.contains("stefan@") && p.contains("providerId=\"AD\""), "{p}");
    }

    #[test]
    fn odf_styles_links_and_meta() {
        let content = "<office:document-content><office:body><text:p>Hallo <text:a xlink:href=\"mailto:anna@example.org\">Mail</text:a></text:p><office:annotation><dc:creator>Stefan Mau</dc:creator><text:p>Notiz</text:p></office:annotation><text:a xlink:href=\"#Kapitel\">x</text:a></office:body></office:document-content>";
        let styles = "<office:document-styles><style:header><text:p>Kopf: Anna Berger</text:p></style:header></office:document-styles>";
        let meta = "<office:meta><meta:initial-creator>Stefan Mau</meta:initial-creator><dc:creator>Stefan Mau</dc:creator><meta:generator>LibreOffice</meta:generator></office:meta>";
        let bytes = zip_of(&[("mimetype", "application/vnd.oasis.opendocument.text"), ("content.xml", content), ("styles.xml", styles), ("meta.xml", meta)]);
        let d = open(bytes, Kind::Odf).unwrap();
        assert!(d.text.contains("mailto:anna@example.org\n") && d.text.contains("Kopf: Anna Berger\n"), "{}", d.text);
        assert!(!d.text.contains("#Kapitel"));
        let new_text = d.text.replace("anna@example.org", "paula@example.org").replace("Anna Berger", "Paula Vogt");
        let out = rebuild(d.xml.as_ref().unwrap(), &d.text, &new_text).unwrap();
        let c = part_of(&out, "content.xml");
        assert!(c.contains("xlink:href=\"mailto:paula@example.org\"") && c.contains("<dc:creator>anonym</dc:creator>") && c.contains("Hallo "), "{c}");
        assert!(part_of(&out, "styles.xml").contains("Kopf: Paula Vogt"));
        let m = part_of(&out, "meta.xml");
        assert!(!m.contains("Stefan") && m.contains("LibreOffice"), "{m}");
    }

    #[test]
    fn thumbnails_and_field_codes_do_not_survive() {
        let doc = "<w:document><w:body><w:p><w:r><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:instrText xml:space=\"preserve\"> HYPERLINK \"mailto:anna.berger@example.org\" </w:instrText></w:r><w:r><w:t>Mail</w:t></w:r></w:p></w:body></w:document>";
        let rels = "<Relationships><Relationship Id=\"rId1\" Type=\"x/officeDocument\" Target=\"word/document.xml\"/><Relationship Id=\"rId2\" Type=\"x/thumbnail\" Target=\"docProps/thumbnail.jpeg\"/></Relationships>";
        let types = "<Types><Default Extension=\"jpeg\" ContentType=\"image/jpeg\"/><Override PartName=\"/docProps/thumbnail.jpeg\" ContentType=\"image/jpeg\"/><Override PartName=\"/word/document.xml\" ContentType=\"x\"/></Types>";
        let bytes = zip_of(&[("[Content_Types].xml", types), ("_rels/.rels", rels), ("word/document.xml", doc), ("docProps/thumbnail.jpeg", "JPEG Anna Berger")]);
        let d = open(bytes, Kind::Docx).unwrap();
        assert!(d.text.contains("mailto:anna.berger@example.org"), "{}", d.text);
        let out = rebuild(d.xml.as_ref().unwrap(), &d.text, &d.text.replace("anna.berger", "paula.vogt")).unwrap();
        let mut a = ZipArchive::new(Cursor::new(out.as_slice())).unwrap();
        assert!(a.by_name("docProps/thumbnail.jpeg").is_err());
        drop(a);
        assert!(part_of(&out, "word/document.xml").contains("mailto:paula.vogt@example.org"));
        let r = part_of(&out, "_rels/.rels");
        assert!(!r.contains("thumbnail") && r.contains("word/document.xml"), "{r}");
        let t = part_of(&out, "[Content_Types].xml");
        assert!(!t.contains("thumbnail") && t.contains("/word/document.xml") && t.contains("Extension=\"jpeg\""), "{t}");
        // ODF: Thumbnails/ samt Manifest-Eintrag
        let manifest = "<manifest:manifest><manifest:file-entry manifest:full-path=\"/\" manifest:media-type=\"x\"/><manifest:file-entry manifest:full-path=\"Thumbnails/thumbnail.png\" manifest:media-type=\"image/png\"/><manifest:file-entry manifest:full-path=\"content.xml\" manifest:media-type=\"text/xml\"/></manifest:manifest>";
        let bytes = zip_of(&[("mimetype", "application/vnd.oasis.opendocument.text"), ("content.xml", "<office:document-content><text:p>Hallo</text:p></office:document-content>"), ("Thumbnails/thumbnail.png", "PNG"), ("META-INF/manifest.xml", manifest)]);
        let d = open(bytes, Kind::Odf).unwrap();
        let out = rebuild(d.xml.as_ref().unwrap(), &d.text, &d.text).unwrap();
        let mut a = ZipArchive::new(Cursor::new(out.as_slice())).unwrap();
        assert!(a.by_name("Thumbnails/thumbnail.png").is_err());
        drop(a);
        let m = part_of(&out, "META-INF/manifest.xml");
        assert!(!m.contains("Thumbnails") && m.contains("content.xml"), "{m}");
    }

    #[test]
    fn limits_and_hostile_input() {
        // Zellreferenz mit absurd vielen Buchstaben läuft nicht über
        assert_eq!(col_index("B7"), 1);
        assert_eq!(col_index("XFD1"), 16_383);
        assert_eq!(col_index(&"Z".repeat(64)), 16_383);
        // Größenbremse: gelogene Größenangabe und zu großer Inhalt
        let data = vec![b'a'; 16];
        assert!(read_limited(&mut data.as_slice(), MAX_PART_BYTES + 1, "x").is_err());
        assert_eq!(read_limited(&mut data.as_slice(), 0, "x").unwrap().len(), 16);
        // Kein ZIP → Fehler statt Absturz
        assert!(open(b"kein zip".to_vec(), Kind::Docx).is_err());
    }
}
