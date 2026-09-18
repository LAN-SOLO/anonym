//! anonym-core — regelbasierte Anonymisierung ohne KI: Wörterbücher, Muster,
//! Prüfziffern; konsistente, deterministische Pseudonyme; Formate, die im
//! Ursprungsformat zurückgeschrieben werden. Tauri-frei und testbar.

pub mod apply;
pub mod checksum;
pub mod csv;
pub mod dates;
pub mod detect;
pub mod dict;
pub mod export;
pub mod formats;
pub mod model;
pub mod pseudo;
pub mod recover;

pub use dict::{Dictionaries, World, WORLD_IDS};
pub use model::*;
pub use pseudo::Store;

use formats::{Document, Kind};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Laufzeit-Optionen, die nicht ins Regelwerk gehören.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Codepage für Dateien, die kein UTF-8 sind (`windows-1252`, …).
    pub fallback_encoding: String,
    /// Pseudonym-Speicher über Dateien hinweg (Tarif masked).
    pub store: Option<Store>,
}

struct Prepared {
    doc: Document,
    findings: Vec<Finding>,
    pseudo: pseudo::Pseudonymizer,
}

fn prepare(path: &Path, rules: &Rules, dicts: &Dictionaries, opts: &Options) -> Result<Prepared, String> {
    let fallback = if opts.fallback_encoding.is_empty() { "windows-1252" } else { opts.fallback_encoding.as_str() };
    let mut doc = formats::open(path, fallback)?;
    let delimiter = match doc.kind {
        Kind::Csv { delimiter } => Some(delimiter),
        _ => None,
    };
    let mut notes = Vec::new();
    let raw = detect::detect(&doc.text, rules, dicts, delimiter, &doc.forced, &mut notes);
    doc.notes.extend(notes);
    let mut pseudo = pseudo::Pseudonymizer::new(World::builtin(&rules.world), &rules.seed, rules.date_shift_days, opts.store.clone());

    // Byte-Offsets → UTF-16 und Zeilen, in einem Durchlauf
    let text = &doc.text;
    let mut findings = Vec::with_capacity(raw.len());
    let mut byte_pos = 0usize;
    let mut u16_pos = 0usize;
    let mut line = 1u32;
    let advance = |target: usize, byte_pos: &mut usize, u16_pos: &mut usize, line: &mut u32| {
        for c in text[*byte_pos..target].chars() {
            *u16_pos += c.len_utf16();
            if c == '\n' {
                *line += 1;
            }
        }
        *byte_pos = target;
    };
    // Personen zuerst ersetzen, damit E-Mail-Adressen die Pseudonyme kennen,
    // auch wenn der Name erst später im Text steht.
    let mut order: Vec<usize> = (0..raw.len()).collect();
    // Zellen fester Spaltenbreiten zuerst: dort zählt die Länge des Pseudonyms.
    order.sort_by_key(|&i| (raw[i].category != Category::Person, !raw[i].pad, i));
    let mut replacements: Vec<String> = vec![String::new(); raw.len()];
    let mut ends: Vec<usize> = raw.iter().map(|m| m.bend).collect();
    for i in order {
        let m = &raw[i];
        let original = &text[m.bstart..m.bend];
        let rule = rules.rule(m.category);
        pseudo.slack = match (m.pad, m.pad_end) {
            (true, Some(pe)) => Some(text[m.bend..pe].bytes().filter(|b| *b == b' ').count()),
            (true, None) => Some(0),
            _ => None,
        };
        let mut r = apply::replacement(original, m.category, &m.parts, &m.seps, &rule, &mut pseudo, dicts);
        pseudo.slack = None;
        // Feste Spaltenbreiten: kürzeren Ersatz auffüllen, längeren in die Füll-Leerzeichen
        // hinein wachsen lassen, damit die Spalten stehen bleiben
        if m.pad {
            let (ol, rl) = (original.chars().count(), r.chars().count());
            if rl < ol && m.pad_end.is_some() {
                r.extend(std::iter::repeat(' ').take(ol - rl));
            } else if rl > ol {
                if let Some(pe) = m.pad_end {
                    let avail = text[m.bend..pe].bytes().filter(|b| *b == b' ').count();
                    ends[i] = m.bend + (rl - ol).min(avail);
                }
            }
        }
        replacements[i] = r;
    }
    for (i, m) in raw.iter().enumerate() {
        advance(m.bstart, &mut byte_pos, &mut u16_pos, &mut line);
        let start = u16_pos;
        let start_line = line;
        advance(ends[i], &mut byte_pos, &mut u16_pos, &mut line);
        let end = u16_pos;
        let original = &text[m.bstart..m.bend];
        let replacement = std::mem::take(&mut replacements[i]);
        findings.push(Finding {
            id: i as u32,
            bstart: m.bstart,
            bend: ends[i],
            start,
            end,
            line: start_line,
            text: original.to_string(),
            category: m.category,
            source: m.source,
            confidence: m.confidence,
            replacement,
            parts: m.parts.clone(),
        });
    }
    Ok(Prepared { doc, findings, pseudo })
}

/// Datei analysieren: virtueller Text + Fundstellen mit Ersatzvorschlag.
pub fn analyze(path: &Path, rules: &Rules, dicts: &Dictionaries, opts: &Options) -> Result<Analysis, String> {
    let p = prepare(path, rules, dicts, opts)?;
    let lines = p.doc.text.matches('\n').count() as u32 + if p.doc.text.ends_with('\n') || p.doc.text.is_empty() { 0 } else { 1 };
    Ok(Analysis {
        path: path.to_string_lossy().into_owned(),
        format: p.doc.kind.label(),
        encoding: p.doc.encoding.name.clone(),
        text: p.doc.text,
        lines,
        findings: p.findings,
        notes: p.doc.notes,
    })
}

/// Ergebnis einer Anwendung: Bericht + aktualisierter Pseudonym-Speicher.
pub struct Applied {
    pub report: Report,
    pub store: Store,
    pub output: PathBuf,
    /// Schlüssel zur Rückübersetzung dieser Ausgabe.
    pub recover: recover::RecoverFile,
}

/// Standard-Ausgabepfad: `<name>.anonym.<ext>` neben der Quelle (oder im Zielordner).
pub fn suggest_output(path: &Path, out_dir: Option<&Path>, suffix: &str) -> PathBuf {
    suggest_output_as(path, out_dir, suffix, None)
}

/// Ausgabepfad mit anderer Endung (Export in ein anderes Format).
pub fn suggest_output_as(path: &Path, out_dir: Option<&Path>, suffix: &str, target_ext: Option<&str>) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "datei".into());
    let ext = target_ext.map(str::to_string).or_else(|| path.extension().map(|e| e.to_string_lossy().into_owned()));
    let suffix = if suffix.is_empty() { ".anonym" } else { suffix };
    // Bereits anonymisierte Datei erneut speichern/exportieren: Zusatz nicht verdoppeln
    let stem = if stem.ends_with(suffix) { stem[..stem.len() - suffix.len()].to_string() } else { stem };
    let name = match ext {
        Some(e) => format!("{stem}{suffix}.{e}"),
        None => format!("{stem}{suffix}"),
    };
    let dir = out_dir.map(Path::to_path_buf).unwrap_or_else(|| path.parent().map(Path::to_path_buf).unwrap_or_default());
    dir.join(name)
}

/// Datei anonymisieren: Analyse wiederholen (deterministisch), Entscheidungen
/// anwenden, im Ursprungsformat schreiben, Bericht erzeugen.
pub fn apply_file(path: &Path, rules: &Rules, dicts: &Dictionaries, opts: &Options, decisions: &[Decision], output: &Path) -> Result<Applied, String> {
    let mut p = prepare(path, rules, dicts, opts)?;
    let by_id: HashMap<u32, &Decision> = decisions.iter().map(|d| (d.id, d)).collect();
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut entries: HashMap<(Category, String), (String, u32)> = HashMap::new();
    let mut by_category: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
    let mut rejected = 0u32;
    for f in &p.findings {
        let d = by_id.get(&f.id);
        if let Some(d) = d {
            if !d.accept {
                rejected += 1;
                continue;
            }
        }
        let category = d.and_then(|d| d.category).unwrap_or(f.category);
        let replacement = match d.and_then(|d| d.replacement.clone()) {
            Some(r) => r,
            None if category != f.category => apply::replacement(&f.text, category, &[], &[], &rules.rule(category), &mut p.pseudo, dicts),
            None => f.replacement.clone(),
        };
        *by_category.entry(category.key().to_string()).or_insert(0) += 1;
        entries.entry((category, f.text.clone())).and_modify(|e| e.1 += 1).or_insert((replacement.clone(), 1));
        edits.push((f.bstart, f.bend, replacement));
    }
    let new_text = apply::splice(&p.doc.text, &edits);
    let native = formats::render(&p.doc, &new_text)?;
    // Andere Endung als die Quelle → Export in dieses Format
    let src_ext = formats::extension(path);
    let out_ext = formats::extension(output);
    let exported = match export::Target::from_ext(&out_ext) {
        Some(target) if out_ext != src_ext => {
            let content = export::extract(&p.doc.kind, &native, &new_text)?;
            let title = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            Some((target, export::render(&content, target, &title)?))
        }
        _ => None,
    };
    let format_label = match &exported {
        Some((t, _)) => format!("{} → {}", p.doc.kind.label(), t.label()),
        None => p.doc.kind.label(),
    };
    formats::write_bytes(exported.map(|(_, b)| b).as_deref().unwrap_or(&native), output)?;

    let mut list: Vec<ReportEntry> = entries
        .into_iter()
        .map(|((category, original), (replacement, count))| ReportEntry { category, original, replacement, count })
        .collect();
    list.sort_by(|a, b| a.category.cmp(&b.category).then(a.original.cmp(&b.original)));
    let report = Report {
        tool: "anonym".into(),
        version: VERSION.into(),
        created: chrono::Local::now().to_rfc3339(),
        source: path.to_string_lossy().into_owned(),
        output: output.to_string_lossy().into_owned(),
        format: format_label,
        rules: rules.name.clone(),
        world: rules.world.clone(),
        date_shift_days: p.pseudo.date_offset_days,
        replaced: edits.len() as u32,
        rejected,
        by_category,
        entries: list,
    };
    let recover = recover::build(&report, &p.pseudo, &rules.seed, &rules.world);
    Ok(Applied { report, store: p.pseudo.store, output: output.to_path_buf(), recover })
}

/// Pfad der Recover-Datei zu einer Ausgabe: `<name>.anonym.recover.json`.
pub fn recover_path_for(output: &Path) -> PathBuf {
    let stem = output.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "ausgabe".into());
    output.with_file_name(format!("{stem}.recover.json"))
}

/// Standard-Ausgabepfad einer Rückübersetzung: `<name>.recovered.<ext>` (Zusatz `.anonym` entfällt).
pub fn suggest_recovered(path: &Path) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "datei".into());
    let stem = stem.strip_suffix(".anonym").map(str::to_string).unwrap_or(stem);
    let name = match path.extension() {
        Some(e) => format!("{stem}.recovered.{}", e.to_string_lossy()),
        None => format!("{stem}.recovered"),
    };
    path.with_file_name(name)
}

/// Bearbeitete anonymisierte Datei mit Recover-Datei zurückübersetzen und im selben Format schreiben.
pub fn recover_file(path: &Path, key: &recover::RecoverFile, opts: &Options, output: &Path) -> Result<recover::RecoverResult, String> {
    let fallback = if opts.fallback_encoding.is_empty() { "windows-1252" } else { opts.fallback_encoding.as_str() };
    let doc = formats::open(path, fallback)?;
    let (new_text, mut result) = recover::restore_text(&doc.text, key);
    let bytes = formats::render(&doc, &new_text)?;
    formats::write_bytes(&bytes, output)?;
    result.output = output.to_string_lossy().into_owned();
    result.format = doc.kind.label();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str, content: &[u8]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("anonym-core-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    const LETTER: &str = "Sehr geehrte Frau Berger,\nvielen Dank für Ihren Anruf vom 14.03.2024. Die Gutschrift\ngeht auf Ihr Konto DE89 3704 0044 0532 0130 00.\nRückfragen an anna.berger@example.org oder 0221 4711-0.\n\nKd-Nr.  Name          Ort    Geb.\n10482   Anna Berger   Köln   02.07.1981\n10483   Jonas Berger  Köln   19.11.2009\n";

    #[test]
    fn end_to_end_text() {
        let path = tmp("brief.txt", LETTER.as_bytes());
        let dicts = Dictionaries::builtin();
        let rules = Rules::default();
        let opts = Options::default();
        let a = analyze(&path, &rules, &dicts, &opts).unwrap();
        assert_eq!(a.format, "Text");
        assert!(a.findings.len() >= 9, "{:?}", a.findings.iter().map(|f| (&f.text, f.category)).collect::<Vec<_>>());
        let out = suggest_output(&path, None, "");
        assert!(out.to_string_lossy().ends_with("brief.anonym.txt"));
        // Pfadtrenner sind plattformabhängig — nur den Dateinamen prüfen
        let name = |p: PathBuf| p.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(name(suggest_output_as(Path::new("/x/brief.anonym.txt"), None, "", Some("xlsx"))), "brief.anonym.xlsx");
        assert_eq!(name(suggest_output(Path::new("/x/brief.anonym.txt"), None, "")), "brief.anonym.txt");
        let applied = apply_file(&path, &rules, &dicts, &opts, &[], &out).unwrap();
        let result = std::fs::read_to_string(&out).unwrap();
        assert!(!result.contains("Berger"), "{result}");
        assert!(!result.contains("anna.berger"), "{result}");
        // E-Mail nutzt die Pseudonyme der Person (Vorname steht erst später im Text)
        let first = result.lines().nth(6).unwrap().split_whitespace().nth(1).unwrap().to_lowercase();
        assert!(result.contains(&format!("{first}.")), "{result}");
        assert!(!result.contains("DE89 3704 0044 0532 0130 00"));
        assert!(result.contains("Sehr geehrte Frau "));
        assert!(result.contains("Köln"));
        assert!(result.contains("@example.org"));
        // Familie behält denselben Nachnamen
        let surname_line = result.lines().find(|l| l.starts_with("Sehr geehrte Frau ")).unwrap();
        let surname = surname_line.trim_start_matches("Sehr geehrte Frau ").trim_end_matches(',');
        assert_eq!(result.matches(surname).count(), 3, "{result}");
        // Feste Spaltenbreiten: „Köln“ steht in beiden Datenzeilen an derselben Stelle wie im Original
        let col = |l: &str| l.find("Köln").unwrap();
        let orig_col = col(LETTER.lines().nth(6).unwrap());
        assert_eq!(col(result.lines().nth(6).unwrap()), orig_col, "{result}");
        assert_eq!(col(result.lines().nth(7).unwrap()), orig_col, "{result}");
        assert_eq!(applied.report.rejected, 0);
        assert!(applied.report.replaced >= 9);
        // Determinismus
        let again = analyze(&path, &rules, &dicts, &opts).unwrap();
        assert_eq!(again.findings, a.findings);
    }

    #[test]
    fn export_other_formats() {
        let path = tmp("export.csv", "Kd-Nr;Name;E-Mail\n10482;Anna Berger;anna@example.org\n".as_bytes());
        let dicts = Dictionaries::builtin();
        let rules = Rules::default();
        let opts = Options::default();
        for ext in ["xlsx", "docx", "odt", "ods", "pdf", "html", "md", "json", "txt", "rtf", "tsv"] {
            let out = suggest_output_as(&path, None, "", Some(ext));
            assert!(out.to_string_lossy().ends_with(&format!("export.anonym.{ext}")));
            let applied = apply_file(&path, &rules, &dicts, &opts, &[], &out).unwrap();
            assert!(applied.report.format.contains("→"), "{ext}: {}", applied.report.format);
            let bytes = std::fs::read(&out).unwrap();
            assert!(!bytes.is_empty(), "{ext}");
            if ext == "xlsx" {
                let back = export::extract(&formats::Kind::Xlsx, &bytes, "").unwrap();
                match back {
                    export::Content::Tables(t) => {
                        assert_eq!(t[0].rows[0], vec!["Kd-Nr", "Name", "E-Mail"]);
                        assert_ne!(t[0].rows[1][1], "Anna Berger");
                        assert_ne!(t[0].rows[1][0], "10482");
                    }
                    _ => panic!(),
                }
            }
        }
        // Text-Quelle nach XLSX: eine Zeile je Reihe
        let tpath = tmp("export.txt", LETTER.as_bytes());
        let out = suggest_output_as(&tpath, None, "", Some("xlsx"));
        apply_file(&tpath, &rules, &dicts, &opts, &[], &out).unwrap();
        let back = export::extract(&formats::Kind::Xlsx, &std::fs::read(&out).unwrap(), "").unwrap();
        match back {
            export::Content::Tables(t) => assert!(t[0].rows.len() >= 8),
            _ => panic!(),
        }
    }

    #[test]
    fn recover_after_editing() {
        let path = tmp("rec.txt", LETTER.as_bytes());
        let dicts = Dictionaries::builtin();
        let rules = Rules::default();
        let opts = Options::default();
        let out = suggest_output(&path, None, "");
        let applied = apply_file(&path, &rules, &dicts, &opts, &[], &out).unwrap();
        assert!(applied.recover.entries.len() >= 9);
        assert!(applied.recover.parts.iter().any(|p| p.kind == "last" && p.original == "berger"));
        // Weiterbearbeitung: Zeile angehängt, Nachname allein verwendet, Groß-/Kleinschreibung geändert
        let anon = std::fs::read_to_string(&out).unwrap();
        let surname = anon.lines().next().unwrap().trim_start_matches("Sehr geehrte Frau ").trim_end_matches(',').to_string();
        let edited = format!("{anon}\nNachtrag: {} hat zurückgerufen. Neue Kd-Nr 55555.\n", surname.to_uppercase());
        let edited_path = tmp("rec.anonym.txt", edited.as_bytes());
        let recovered = suggest_recovered(&edited_path);
        assert!(recovered.to_string_lossy().ends_with("rec.recovered.txt"));
        let key_bytes = recover::seal(&applied.recover, Some("pw")).unwrap();
        let key = recover::open(&key_bytes, Some("pw")).unwrap();
        let r = recover_file(&edited_path, &key, &opts, &recovered).unwrap();
        let text = std::fs::read_to_string(&recovered).unwrap();
        assert!(text.starts_with(LETTER.trim_end()), "{text}");
        assert!(text.contains("Nachtrag: BERGER hat zurückgerufen. Neue Kd-Nr 55555."), "{text}");
        assert!(r.restored >= 10 && r.ambiguous == 0, "{r:?}");
        // Anderes Format: CSV → XLSX exportiert, dann zurück
        let csv = tmp("rec2.csv", "Kd-Nr;Name;E-Mail\n10482;Anna Berger;anna@example.org\n".as_bytes());
        let xlsx_out = suggest_output_as(&csv, None, "", Some("xlsx"));
        let applied2 = apply_file(&csv, &rules, &dicts, &opts, &[], &xlsx_out).unwrap();
        let rec2 = suggest_recovered(&xlsx_out);
        let r2 = recover_file(&xlsx_out, &applied2.recover, &opts, &rec2).unwrap();
        assert!(r2.restored >= 3, "{r2:?}");
        let back = export::extract(&formats::Kind::Xlsx, &std::fs::read(&rec2).unwrap(), "").unwrap();
        match back {
            export::Content::Tables(t) => assert_eq!(t[0].rows[1], vec!["10482", "Anna Berger", "anna@example.org"]),
            _ => panic!(),
        }
    }

    #[test]
    fn decisions_and_csv() {
        let path = tmp("kunden.csv", "Kd-Nr;Name;E-Mail;Geb.\n10482;Anna Berger;anna@example.org;02.07.1981\n".as_bytes());
        let dicts = Dictionaries::builtin();
        let rules = Rules::default();
        let opts = Options::default();
        let a = analyze(&path, &rules, &dicts, &opts).unwrap();
        assert_eq!(a.format, "CSV (;)");
        let date = a.findings.iter().find(|f| f.category == Category::Date).unwrap();
        let name = a.findings.iter().find(|f| f.category == Category::Person).unwrap();
        let decisions = vec![
            Decision { id: date.id, accept: false, category: None, replacement: None },
            Decision { id: name.id, accept: true, category: None, replacement: Some("Max Mustermann".into()) },
        ];
        let out = suggest_output(&path, None, "");
        let applied = apply_file(&path, &rules, &dicts, &opts, &decisions, &out).unwrap();
        let result = std::fs::read_to_string(&out).unwrap();
        assert!(result.contains("02.07.1981"));
        assert!(result.contains("Max Mustermann"));
        assert!(!result.contains("10482"));
        assert_eq!(applied.report.rejected, 1);
        assert_eq!(result.lines().count(), 2);
        assert_eq!(result.lines().nth(1).unwrap().split(';').count(), 4);
    }
}
