//! Formate: Eine Datei wird zu einem „virtuellen Text“ (für Erkennung und
//! Vorschau) plus der Information, wie Änderungen zurückgeschrieben werden.

pub mod text;
pub mod xml;

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// Reiner Text (auch Markdown, JSON, YAML, XML, HTML, Logs, EML, SRT …)
    Text,
    /// Tabelle mit Trennzeichen
    Csv { delimiter: char },
    /// Word (document.xml, `<w:t>`)
    Docx,
    /// Excel (sharedStrings.xml + Inline-Strings der Blätter)
    Xlsx,
    /// OpenDocument Text/Tabelle (content.xml, alle Textknoten)
    Odf,
}

impl Kind {
    pub fn label(&self) -> String {
        match self {
            Kind::Text => "Text".into(),
            Kind::Csv { delimiter } => match delimiter {
                '\t' => "TSV".into(),
                d => format!("CSV ({d})"),
            },
            Kind::Docx => "DOCX".into(),
            Kind::Xlsx => "XLSX".into(),
            Kind::Odf => "ODF".into(),
        }
    }
}

/// Geöffnete Datei: virtueller Text + Rückschreib-Information.
pub struct Document {
    pub kind: Kind,
    pub text: String,
    pub encoding: text::Encoding,
    /// Nur bei XML-Containern: Ursprungsbytes + Textknoten-Karte
    pub xml: Option<xml::Container>,
    pub notes: Vec<String>,
}

pub fn extension(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Datei öffnen und in den virtuellen Text überführen.
pub fn open(path: &Path, fallback_encoding: &str) -> Result<Document, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Datei nicht lesbar: {e}"))?;
    let ext = extension(path);
    match ext.as_str() {
        "docx" | "docm" | "dotx" => xml::open(bytes, Kind::Docx),
        "xlsx" | "xlsm" | "xltx" => xml::open(bytes, Kind::Xlsx),
        "odt" | "ods" | "ott" | "ots" | "odp" => xml::open(bytes, Kind::Odf),
        _ => {
            let (text, encoding) = text::decode(&bytes, fallback_encoding);
            let kind = match ext.as_str() {
                "csv" | "tsv" | "tab" => Kind::Csv { delimiter: if ext == "csv" { crate::csv::detect_delimiter(&text) } else { '\t' } },
                _ => Kind::Text,
            };
            let mut notes = Vec::new();
            if encoding.legacy {
                notes.push(format!("Kodierung {} erkannt — Ausgabe in derselben Kodierung.", encoding.name));
            }
            Ok(Document { kind, text, encoding, xml: None, notes })
        }
    }
}

/// Ausgabe im Ursprungsformat als Bytes.
pub fn render(doc: &Document, new_text: &str) -> Result<Vec<u8>, String> {
    match &doc.xml {
        Some(container) => xml::rebuild(container, &doc.text, new_text),
        None => Ok(text::encode(new_text, &doc.encoding)),
    }
}

/// Bytes atomar schreiben (Temp-Datei + rename).
pub fn write_bytes(bytes: &[u8], out: &Path) -> Result<(), String> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Ordner anlegen fehlgeschlagen: {e}"))?;
    }
    let tmp = out.with_extension("anonym-tmp");
    std::fs::write(&tmp, bytes).map_err(|e| format!("Schreiben fehlgeschlagen: {e}"))?;
    std::fs::rename(&tmp, out).map_err(|e| format!("Umbenennen fehlgeschlagen: {e}"))
}

/// Ausgabe schreiben: neuer virtueller Text → Datei im Ursprungsformat.
pub fn write(doc: &Document, new_text: &str, out: &Path) -> Result<(), String> {
    let bytes = render(doc, new_text)?;
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Ordner anlegen fehlgeschlagen: {e}"))?;
    }
    let tmp = out.with_extension("anonym-tmp");
    std::fs::write(&tmp, bytes).map_err(|e| format!("Schreiben fehlgeschlagen: {e}"))?;
    std::fs::rename(&tmp, out).map_err(|e| format!("Umbenennen fehlgeschlagen: {e}"))
}

/// Endungen, die anonym öffnet (für Dateidialoge).
pub const EXTENSIONS: [&str; 40] = [
    "txt", "md", "markdown", "log", "csv", "tsv", "tab", "json", "jsonl", "ndjson", "yaml", "yml", "xml", "html", "htm", "srt", "vtt", "eml", "mbox", "sql", "ini", "cfg", "conf", "toml", "rtf", "tex", "rst", "adoc", "vcf", "ics", "docx", "docm", "dotx", "xlsx", "xlsm", "xltx", "odt", "ods", "ott", "ots",
];
