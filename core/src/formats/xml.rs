//! XML-Container (DOCX, XLSX, ODF): ZIP öffnen, Textknoten der relevanten Teile
//! einsammeln, als virtuellen Text (ein Knoten je Zeile) anbieten und beim
//! Schreiben die Knoten austauschen. Alles andere im Archiv bleibt Byte für Byte.

use super::{Document, Kind};
use crate::formats::text::Encoding;
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
    let mut notes = Vec::new();
    match kind {
        Kind::Docx => notes.push("Word: Jeder Formatierungslauf ist eine Zeile — ein Name, den Word über zwei Läufe verteilt hat, erscheint geteilt.".into()),
        Kind::Xlsx => notes.push("Excel: Nur Textzellen werden geprüft — Zahlen- und Datumszellen bleiben unverändert.".into()),
        _ => {}
    }
    Ok(Document {
        kind: kind.clone(),
        text,
        encoding: Encoding { name: "UTF-8".into(), bom: false, legacy: false },
        xml: Some(Container { kind, bytes, parts, nodes }),
        notes,
    })
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
