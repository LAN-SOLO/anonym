//! XML-Container (DOCX, XLSX, ODF): ZIP öffnen, Textknoten der relevanten Teile
//! einsammeln, als virtuellen Text (ein Knoten je Zeile) anbieten und beim
//! Schreiben die Knoten austauschen. Alles andere im Archiv bleibt Byte für Byte.

use super::{Document, ForcedLine, Kind};
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
    R.get_or_init(|| Regex::new(r"<w:t(?:\s[^>]*)?>([^<]*)</w:t>").unwrap())
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
        Kind::Docx => names.into_iter().filter(|n| n == "word/document.xml" || n.starts_with("word/footnotes") || n.starts_with("word/endnotes") || n.starts_with("word/header") || n.starts_with("word/footer") || n.starts_with("word/comments")).collect(),
        Kind::Xlsx => names.into_iter().filter(|n| n == "xl/sharedStrings.xml" || (n.starts_with("xl/worksheets/sheet") && n.ends_with(".xml")) || n == "xl/comments1.xml").collect(),
        _ => names.into_iter().filter(|n| n == "content.xml").collect(),
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
    for name in &names {
        let mut f = archive.by_name(name).map_err(|e| e.to_string())?;
        let mut s = String::new();
        f.read_to_string(&mut s).map_err(|e| format!("{name}: {e}"))?;
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
    if kind == Kind::Xlsx {
        xlsx_nodes(&parts, &mut nodes, &mut text, &mut forced);
    } else {
        for (pi, (_, content)) in parts.iter().enumerate() {
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
    }
    let mut notes = Vec::new();
    match kind {
        Kind::Docx => notes.push("Word: Jeder Formatierungslauf ist eine Zeile — ein Name, den Word über zwei Läufe verteilt hat, erscheint geteilt.".into()),
        Kind::Xlsx => notes.push("Excel: Textzellen aller Blätter; Zahlenzellen nur in Spalten, deren Kopfzeile eine Kategorie nennt (Kd-Nr., Telefon, PLZ, Geb. …).".into()),
        _ => {}
    }
    Ok(Document {
        kind: kind.clone(),
        text,
        encoding: Encoding { name: "UTF-8".into(), bom: false, legacy: false },
        xml: Some(Container { kind, bytes, parts, nodes }),
        notes,
        forced,
    })
}

/// Kategorien, die auch in Zahlenzellen sinnvoll sind (Datum = Excel-Seriennummer).
fn numeric_ok(c: Category) -> bool {
    matches!(c, Category::CustomerId | Category::Phone | Category::PostalCode | Category::TaxId | Category::Insurance | Category::CreditCard | Category::Date)
}

fn col_index(cell_ref: &str) -> usize {
    let mut n = 0usize;
    for c in cell_ref.chars().take_while(|c| c.is_ascii_alphabetic()) {
        n = n * 26 + (c.to_ascii_uppercase() as usize - 'A' as usize + 1);
    }
    n.saturating_sub(1)
}

/// XLSX: sharedStrings als Knoten je `<t>` (gruppiert nach `<si>`), Blätter zellenweise —
/// Inline-Strings als Knoten, Zahlenzellen in typisierten Spalten ebenfalls. Spalten werden
/// über die Kopfzeile (erste Zeile) typisiert wie bei CSV.
fn xlsx_nodes(parts: &[(String, String)], nodes: &mut Vec<Node>, text: &mut String, forced: &mut Vec<ForcedLine>) {
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
        if !name.starts_with("xl/worksheets/") {
            continue;
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
                        match (ri > 0, typed.get(col).copied().flatten(), v_re.captures(body.as_str())) {
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
                if ri == 0 {
                    while typed.len() <= col {
                        typed.push(None);
                    }
                    typed[col] = column_category(&cell_text);
                    continue;
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

/// Neuen virtuellen Text (Zeile je Knoten) in die XML-Teile schreiben und das Archiv neu packen.
pub fn rebuild(c: &Container, old_text: &str, new_text: &str) -> Result<Vec<u8>, String> {
    let old_lines: Vec<&str> = old_text.split('\n').collect();
    let new_lines: Vec<&str> = new_text.split('\n').collect();
    if new_lines.len() != old_lines.len() {
        return Err("Interner Fehler: Zeilenzahl der Vorschau passt nicht zum Dokument".into());
    }
    // Je Teil: Knoten rückwärts ersetzen, damit Offsets gültig bleiben
    let mut contents: Vec<String> = c.parts.iter().map(|(_, s)| s.clone()).collect();
    for (li, node) in c.nodes.iter().enumerate().rev() {
        if old_lines[li] == new_lines[li] {
            continue;
        }
        let part = &mut contents[node.part];
        let replacement = escape(new_lines[li]);
        part.replace_range(node.start..node.end, &replacement);
    }
    // Leerzeichen am Rand brauchen xml:space="preserve" (DOCX); vorhandene Attribute behalten
    let cursor = Cursor::new(c.bytes.as_slice());
    let mut archive = ZipArchive::new(cursor).map_err(|e| e.to_string())?;
    let mut out = Cursor::new(Vec::with_capacity(c.bytes.len()));
    {
        let mut writer = ZipWriter::new(&mut out);
        for i in 0..archive.len() {
            let mut f = archive.by_index(i).map_err(|e| e.to_string())?;
            let name = f.name().to_string();
            let is_dir = f.is_dir();
            let method = if name == "mimetype" { CompressionMethod::Stored } else { CompressionMethod::Deflated };
            let opts = SimpleFileOptions::default().compression_method(method).last_modified_time(f.last_modified().unwrap_or_default());
            if is_dir {
                writer.add_directory(name, opts).map_err(|e| e.to_string())?;
                continue;
            }
            writer.start_file(name.clone(), opts).map_err(|e| e.to_string())?;
            if let Some(pi) = c.parts.iter().position(|(n, _)| *n == name) {
                writer.write_all(contents[pi].as_bytes()).map_err(|e| e.to_string())?;
            } else {
                let mut buf = Vec::new();
                f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
                writer.write_all(&buf).map_err(|e| e.to_string())?;
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
}
