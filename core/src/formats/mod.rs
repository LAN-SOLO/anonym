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
    /// Zeilen des virtuellen Texts, die durch eine typisierte Tabellenspalte
    /// bereits einer Kategorie zugeordnet sind (XLSX: Kopfzeile → Spalte).
    pub forced: Vec<ForcedLine>,
    /// Zeile des virtuellen Texts → Tabellenspalte (XLSX), für die Mehrheits-Typisierung.
    pub cells: Vec<CellRef>,
}

/// Zuordnung einer Zeile des virtuellen Texts zu einer Tabellenspalte.
#[derive(Debug, Clone)]
pub struct CellRef {
    pub line: usize,
    /// Spaltenkennung, z. B. `sheet1:B`
    pub column: String,
}

/// Eine per Spaltenkopf typisierte Zeile des virtuellen Texts.
#[derive(Debug, Clone, Copy)]
pub struct ForcedLine {
    pub line: usize,
    pub category: crate::model::Category,
    pub hint: crate::csv::PersonHint,
}

pub fn extension(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Datei öffnen und in den virtuellen Text überführen.
/// Größte Eingabedatei — darüber wird die Vorschau im Fenster unbrauchbar und der Speicher knapp.
pub const MAX_INPUT_BYTES: u64 = 512 * 1024 * 1024;

pub fn open(path: &Path, fallback_encoding: &str) -> Result<Document, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("Datei nicht lesbar: {}", io_text(&e)))?;
    if !meta.is_file() {
        return Err("Keine Datei".into());
    }
    if meta.len() > MAX_INPUT_BYTES {
        return Err(format!("Datei zu groß ({} MB, Grenze {} MB)", meta.len() / 1_048_576, MAX_INPUT_BYTES / 1_048_576));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("Datei nicht lesbar: {}", io_text(&e)))?;
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
                notes.push(format!("encoding\u{1f}{}", encoding.name));
            }
            Ok(Document { kind, text, encoding, xml: None, notes, forced: Vec::new(), cells: Vec::new() })
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

/// Bytes atomar schreiben: Temp-Datei im selben Ordner + rename. Die Temp-Datei trägt den
/// vollen Dateinamen (kein Zusammenstoß mit Nachbardateien gleichen Stamms) und wird bei
/// einem Fehler wieder entfernt. Unter Windows halten Virenscanner, OneDrive oder die Vorschau
/// eine frisch geschriebene Datei oft kurz offen: das Umbenennen wird dann einige Male wiederholt.
pub fn write_bytes(bytes: &[u8], out: &Path) -> Result<(), String> {
    let parent = out.parent().filter(|p| !p.as_os_str().is_empty());
    if let Some(parent) = parent {
        std::fs::create_dir_all(parent).map_err(|e| format!("Ordner anlegen fehlgeschlagen: {}", io_text(&e)))?;
    }
    let name = out.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or("Ungültiger Ausgabepfad")?;
    // Prozess-id + Zähler: Zwei gleichzeitige Schreibvorgänge auf dieselbe Datei (z. B.
    // Regelwerk aus zwei Kommandos) teilen sich nie eine Temp-Datei.
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = out.with_file_name(format!(".{name}.{}-{seq}.anonym-tmp", std::process::id()));
    if let Err(e) = std::fs::write(&tmp, bytes) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("Schreiben fehlgeschlagen: {}", io_text(&e)));
    }
    if let Err(e) = rename_retry(&tmp, out) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("Speichern fehlgeschlagen: {}", io_text(&e)));
    }
    Ok(())
}

/// `rename` mit Wiederholung bei vorübergehenden Sperren (nur Windows; anderswo ein Versuch).
pub fn rename_retry(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut wait = 25u64;
    let mut attempt = 0;
    loop {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) if cfg!(windows) && attempt < 8 && is_transient_lock(&e) => {
                std::thread::sleep(std::time::Duration::from_millis(wait));
                wait = (wait * 2).min(400);
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

/// Windows: Zugriff verweigert (5), Freigabe- (32) bzw. Sperrverletzung (33) — typisch für
/// Virenscanner, OneDrive, Indexer oder eine in Office geöffnete Datei.
fn is_transient_lock(e: &std::io::Error) -> bool {
    matches!(e.raw_os_error(), Some(5) | Some(32) | Some(33))
}

/// I/O-Fehler als feste deutsche Meldung (die Oberfläche übersetzt sie) mit OS-Code.
/// Unbekannte Fälle behalten die Systemmeldung.
pub fn io_text(e: &std::io::Error) -> String {
    use std::io::ErrorKind as K;
    let code = e.raw_os_error();
    let known = if cfg!(windows) {
        match code {
            Some(2) | Some(3) => Some("Datei oder Ordner nicht gefunden"),
            Some(32) | Some(33) => Some("Datei wird von einem anderen Programm verwendet — bitte dort schließen"),
            Some(5) => Some("Zugriff verweigert — ist die Datei in einem anderen Programm geöffnet oder schreibgeschützt?"),
            Some(19) => Some("Datenträger ist schreibgeschützt"),
            Some(39) | Some(112) => Some("Datenträger ist voll"),
            Some(123) | Some(161) => Some("Ungültiger Datei- oder Ordnername"),
            Some(206) => Some("Pfad oder Dateiname zu lang"),
            _ => None,
        }
    } else {
        match code {
            Some(2) => Some("Datei oder Ordner nicht gefunden"),
            Some(1) | Some(13) => Some("Zugriff verweigert — ist die Datei in einem anderen Programm geöffnet oder schreibgeschützt?"),
            Some(28) => Some("Datenträger ist voll"),
            Some(30) => Some("Datenträger ist schreibgeschützt"),
            Some(63) if cfg!(target_os = "macos") => Some("Pfad oder Dateiname zu lang"),
            Some(36) if cfg!(target_os = "linux") => Some("Pfad oder Dateiname zu lang"),
            _ => None,
        }
    };
    let known = known.or(match e.kind() {
        K::NotFound => Some("Datei oder Ordner nicht gefunden"),
        K::PermissionDenied => Some("Zugriff verweigert — ist die Datei in einem anderen Programm geöffnet oder schreibgeschützt?"),
        _ => None,
    });
    match (known, code) {
        (Some(t), Some(c)) => format!("{t} (os error {c})"),
        (Some(t), None) => t.to_string(),
        (None, _) => e.to_string(),
    }
}

/// Ausgabe schreiben: neuer virtueller Text → Datei im Ursprungsformat.
pub fn write(doc: &Document, new_text: &str, out: &Path) -> Result<(), String> {
    write_bytes(&render(doc, new_text)?, out)
}

/// Endungen, die anonym öffnet (für Dateidialoge).
pub const EXTENSIONS: [&str; 40] = [
    "txt", "md", "markdown", "log", "csv", "tsv", "tab", "json", "jsonl", "ndjson", "yaml", "yml", "xml", "html", "htm", "srt", "vtt", "eml", "mbox", "sql", "ini", "cfg", "conf", "toml", "rtf", "tex", "rst", "adoc", "vcf", "ics", "docx", "docm", "dotx", "xlsx", "xlsm", "xltx", "odt", "ods", "ott", "ots",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_errors_are_stable_and_write_leaves_no_temp() {
        let dir = std::env::temp_dir().join(format!("anonym-io-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let missing = dir.join("gibt-es-nicht.txt");
        let e = std::fs::read(&missing).unwrap_err();
        assert!(io_text(&e).starts_with("Datei oder Ordner nicht gefunden (os error "), "{}", io_text(&e));
        let err = open(&missing, "windows-1252").err().unwrap();
        assert!(err.starts_with("Datei nicht lesbar: Datei oder Ordner nicht gefunden"), "{err}");
        // Umlaute und Leerzeichen im Namen, Ziel existiert schon (wird ersetzt)
        let out = dir.join("Prüfung Kunden.anonym.txt");
        write_bytes(b"eins", &out).unwrap();
        write_bytes(b"zwei", &out).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), b"zwei");
        // Gleichzeitige Schreibvorgänge auf dieselbe Datei: jeder mit eigener Temp-Datei
        let threads: Vec<_> = (0..8u8)
            .map(|i| {
                let out = out.clone();
                std::thread::spawn(move || write_bytes(&[b'a' + i; 64], &out))
            })
            .collect();
        for t in threads {
            t.join().unwrap().unwrap();
        }
        let last = std::fs::read(&out).unwrap();
        assert!(last.len() == 64 && last.iter().all(|b| *b == last[0]));
        // Ziel ist ein Ordner: Fehler, keine Temp-Datei bleibt liegen
        let blocked = dir.join("ordner.txt");
        std::fs::create_dir_all(blocked.join("x")).unwrap();
        assert!(write_bytes(b"x", &blocked).unwrap_err().starts_with("Speichern fehlgeschlagen: "));
        let leftovers = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().ends_with(".anonym-tmp")).count();
        assert_eq!(leftovers, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn strips_utf8_bom() {
        assert_eq!(text::strip_bom(b"\xEF\xBB\xBF{}"), b"{}");
        assert_eq!(text::strip_bom(b"{}"), b"{}");
    }
}
