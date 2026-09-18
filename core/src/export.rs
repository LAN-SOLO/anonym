//! Export der anonymisierten Ausgabe in andere Formate. Aus der Ausgabe im
//! Ursprungsformat wird eine neutrale Zwischenform (Tabellen oder Textzeilen),
//! die jeder Schreiber in sein Format gießt — ohne externe Bibliotheken.

use crate::formats::xml::{escape, unescape};
use crate::formats::Kind;
use regex::Regex;
use std::io::{Cursor, Read, Write};
use std::sync::OnceLock;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// Zielformate des Exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Txt,
    Md,
    Html,
    Csv,
    Tsv,
    Json,
    Xlsx,
    Docx,
    Odt,
    Ods,
    Pdf,
    Rtf,
}

impl Target {
    pub const ALL: [Target; 12] = [Target::Txt, Target::Md, Target::Html, Target::Csv, Target::Tsv, Target::Json, Target::Xlsx, Target::Docx, Target::Odt, Target::Ods, Target::Pdf, Target::Rtf];

    pub fn from_ext(ext: &str) -> Option<Target> {
        Some(match ext.to_lowercase().as_str() {
            "txt" | "log" => Target::Txt,
            "md" | "markdown" => Target::Md,
            "html" | "htm" => Target::Html,
            "csv" => Target::Csv,
            "tsv" | "tab" => Target::Tsv,
            "json" => Target::Json,
            "xlsx" => Target::Xlsx,
            "docx" => Target::Docx,
            "odt" => Target::Odt,
            "ods" => Target::Ods,
            "pdf" => Target::Pdf,
            "rtf" => Target::Rtf,
            _ => return None,
        })
    }

    pub fn ext(self) -> &'static str {
        match self {
            Target::Txt => "txt",
            Target::Md => "md",
            Target::Html => "html",
            Target::Csv => "csv",
            Target::Tsv => "tsv",
            Target::Json => "json",
            Target::Xlsx => "xlsx",
            Target::Docx => "docx",
            Target::Odt => "odt",
            Target::Ods => "ods",
            Target::Pdf => "pdf",
            Target::Rtf => "rtf",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Target::Txt => "Text (TXT)",
            Target::Md => "Markdown (MD)",
            Target::Html => "HTML",
            Target::Csv => "CSV",
            Target::Tsv => "TSV",
            Target::Json => "JSON",
            Target::Xlsx => "Excel (XLSX)",
            Target::Docx => "Word (DOCX)",
            Target::Odt => "OpenDocument Text (ODT)",
            Target::Ods => "OpenDocument Tabelle (ODS)",
            Target::Pdf => "PDF",
            Target::Rtf => "Rich Text (RTF)",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub name: String,
    pub rows: Vec<Vec<String>>,
}

/// Neutrale Zwischenform.
#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    Tables(Vec<Table>),
    Text(Vec<String>),
}

// --- Lesen der Ausgabe -------------------------------------------------------

fn re(s: &str) -> Regex {
    Regex::new(s).unwrap()
}

/// Aus der anonymisierten Ausgabe (Bytes im Ursprungsformat + virtueller Text) die Zwischenform bauen.
pub fn extract(kind: &Kind, bytes: &[u8], text: &str) -> Result<Content, String> {
    match kind {
        Kind::Text => Ok(Content::Text(text.lines().map(|l| l.trim_end_matches('\r').to_string()).collect())),
        Kind::Csv { delimiter } => {
            let rows = crate::csv::cells(text, *delimiter);
            let rows: Vec<Vec<String>> = rows
                .into_iter()
                .filter(|r| !(r.len() == 1 && text[r[0].start..r[0].end].trim().is_empty()))
                .map(|r| r.into_iter().map(|c| text[c.start..c.end].replace("\"\"", "\"")).collect())
                .collect();
            Ok(Content::Tables(vec![Table { name: "Tabelle1".into(), rows }]))
        }
        Kind::Docx => Ok(Content::Text(docx_paragraphs(bytes)?)),
        Kind::Xlsx => Ok(Content::Tables(xlsx_tables(bytes)?)),
        Kind::Odf => odf_content(bytes),
    }
}

fn zip_part(bytes: &[u8], name: &str) -> Result<Option<String>, String> {
    let mut a = ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut f = match a.by_name(name) {
        Ok(f) => f,
        Err(_) => return Ok(None),
    };
    let mut s = String::new();
    f.read_to_string(&mut s).map_err(|e| e.to_string())?;
    Ok(Some(s))
}

fn zip_names(bytes: &[u8]) -> Result<Vec<String>, String> {
    let a = ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    Ok(a.file_names().map(|s| s.to_string()).collect())
}

fn strip_tags(s: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = R.get_or_init(|| re(r"<[^>]*>"));
    unescape(&r.replace_all(s, ""))
}

fn docx_paragraphs(bytes: &[u8]) -> Result<Vec<String>, String> {
    let doc = zip_part(bytes, "word/document.xml")?.ok_or("word/document.xml fehlt")?;
    static P: OnceLock<Regex> = OnceLock::new();
    static T: OnceLock<Regex> = OnceLock::new();
    let p = P.get_or_init(|| re(r"(?s)<w:p[ >].*?</w:p>|<w:p/>"));
    let t = T.get_or_init(|| re(r"(?s)<w:t(?:\s[^>]*)?>([^<]*)</w:t>|<w:tab/>|<w:br/>"));
    let mut out = Vec::new();
    for m in p.find_iter(&doc) {
        let mut line = String::new();
        for c in t.captures_iter(m.as_str()) {
            match c.get(1) {
                Some(g) => line.push_str(&unescape(g.as_str())),
                None => line.push(if c.get(0).unwrap().as_str() == "<w:tab/>" { '\t' } else { ' ' }),
            }
        }
        out.push(line);
    }
    Ok(out)
}

fn col_index(cell_ref: &str) -> usize {
    let mut n = 0usize;
    for c in cell_ref.chars().take_while(|c| c.is_ascii_alphabetic()) {
        n = n * 26 + (c.to_ascii_uppercase() as usize - 'A' as usize + 1);
    }
    n.saturating_sub(1)
}

fn xlsx_tables(bytes: &[u8]) -> Result<Vec<Table>, String> {
    let shared: Vec<String> = match zip_part(bytes, "xl/sharedStrings.xml")? {
        Some(s) => {
            let si = re(r"(?s)<si>(.*?)</si>");
            let t = re(r"(?s)<t(?:\s[^>]*)?>([^<]*)</t>");
            si.captures_iter(&s).map(|c| t.captures_iter(&c[1]).map(|x| unescape(&x[1])).collect::<String>()).collect()
        }
        None => Vec::new(),
    };
    // Blattnamen aus workbook.xml (Reihenfolge = rId-Reihenfolge, meist sheetN)
    let wb = zip_part(bytes, "xl/workbook.xml")?.unwrap_or_default();
    let sheet_re = re(r#"<sheet\s[^>]*name="([^"]*)""#);
    let names: Vec<String> = sheet_re.captures_iter(&wb).map(|c| unescape(&c[1])).collect();
    let mut sheet_files: Vec<String> = zip_names(bytes)?.into_iter().filter(|n| n.starts_with("xl/worksheets/sheet") && n.ends_with(".xml")).collect();
    sheet_files.sort_by_key(|n| n.trim_start_matches("xl/worksheets/sheet").trim_end_matches(".xml").parse::<u32>().unwrap_or(0));
    let row_re = re(r#"(?s)<row\b[^>]*>(.*?)</row>"#);
    let cell_re = re(r#"(?s)<c\b([^>]*?)(?:/>|>(.*?)</c>)"#);
    let attr_r = re(r#"\br="([A-Z]+)\d*""#);
    let attr_t = re(r#"\bt="([^"]*)""#);
    let v_re = re(r"(?s)<v>([^<]*)</v>");
    let is_re = re(r"(?s)<t(?:\s[^>]*)?>([^<]*)</t>");
    let mut tables = Vec::new();
    for (i, file) in sheet_files.iter().enumerate() {
        let xml = zip_part(bytes, file)?.unwrap_or_default();
        let mut rows: Vec<Vec<String>> = Vec::new();
        for r in row_re.captures_iter(&xml) {
            let mut row: Vec<String> = Vec::new();
            for c in cell_re.captures_iter(&r[1]) {
                let attrs = &c[1];
                let col = attr_r.captures(attrs).map(|a| col_index(&a[1])).unwrap_or(row.len());
                let typ = attr_t.captures(attrs).map(|a| a[1].to_string()).unwrap_or_default();
                let inner = c.get(2).map(|g| g.as_str()).unwrap_or("");
                let value = match typ.as_str() {
                    "s" => v_re.captures(inner).and_then(|v| v[1].trim().parse::<usize>().ok()).and_then(|i| shared.get(i).cloned()).unwrap_or_default(),
                    "inlineStr" => is_re.captures_iter(inner).map(|x| unescape(&x[1])).collect(),
                    "b" => v_re.captures(inner).map(|v| if &v[1] == "1" { "WAHR".to_string() } else { "FALSCH".to_string() }).unwrap_or_default(),
                    _ => v_re.captures(inner).map(|v| unescape(&v[1])).unwrap_or_default(),
                };
                while row.len() < col {
                    row.push(String::new());
                }
                if row.len() == col {
                    row.push(value);
                } else {
                    row[col] = value;
                }
            }
            rows.push(row);
        }
        tables.push(Table { name: names.get(i).cloned().unwrap_or_else(|| format!("Tabelle{}", i + 1)), rows });
    }
    if tables.is_empty() {
        return Err("Keine Tabellenblätter gefunden".into());
    }
    Ok(tables)
}

fn odf_content(bytes: &[u8]) -> Result<Content, String> {
    let xml = zip_part(bytes, "content.xml")?.ok_or("content.xml fehlt")?;
    if xml.contains("<office:spreadsheet") {
        let table_re = re(r#"(?s)<table:table\b([^>]*)>(.*?)</table:table>"#);
        let name_re = re(r#"table:name="([^"]*)""#);
        let row_re = re(r"(?s)<table:table-row\b[^>]*>(.*?)</table:table-row>");
        let cell_re = re(r#"(?s)<table:(?:covered-)?table-cell\b([^>]*?)(?:/>|>(.*?)</table:(?:covered-)?table-cell>)"#);
        let rep_re = re(r#"table:number-columns-repeated="(\d+)""#);
        let mut tables = Vec::new();
        for t in table_re.captures_iter(&xml) {
            let name = name_re.captures(&t[1]).map(|c| unescape(&c[1])).unwrap_or_default();
            let mut rows = Vec::new();
            for r in row_re.captures_iter(&t[2]) {
                let mut row: Vec<String> = Vec::new();
                for c in cell_re.captures_iter(&r[1]) {
                    let rep: usize = rep_re.captures(&c[1]).and_then(|x| x[1].parse().ok()).unwrap_or(1);
                    let val = c.get(2).map(|g| strip_tags(g.as_str())).unwrap_or_default();
                    for _ in 0..rep.min(1000) {
                        row.push(val.clone());
                    }
                }
                while row.last().map(|v| v.is_empty()).unwrap_or(false) {
                    row.pop();
                }
                rows.push(row);
            }
            while rows.last().map(|r| r.is_empty()).unwrap_or(false) {
                rows.pop();
            }
            tables.push(Table { name, rows });
        }
        Ok(Content::Tables(tables))
    } else {
        let p_re = re(r"(?s)<text:(?:p|h)\b[^>]*>(.*?)</text:(?:p|h)>");
        Ok(Content::Text(p_re.captures_iter(&xml).map(|c| strip_tags(&c[1].replace("<text:tab/>", "\t").replace("<text:line-break/>", " "))).collect()))
    }
}

// --- Schreiben ----------------------------------------------------------------

/// Zwischenform in das Zielformat schreiben.
pub fn render(content: &Content, target: Target, title: &str) -> Result<Vec<u8>, String> {
    Ok(match target {
        Target::Txt => txt(content).into_bytes(),
        Target::Md => md(content).into_bytes(),
        Target::Html => html(content, title).into_bytes(),
        Target::Csv => delimited(content, ';').into_bytes(),
        Target::Tsv => delimited(content, '\t').into_bytes(),
        Target::Json => json(content, title)?,
        Target::Xlsx => xlsx(content)?,
        Target::Docx => docx(content)?,
        Target::Odt => odf(content, false)?,
        Target::Ods => odf(content, true)?,
        Target::Pdf => pdf(content, title),
        Target::Rtf => rtf(content).into_bytes(),
    })
}

fn table_as_lines(t: &Table) -> Vec<String> {
    // Spaltenbreiten wie bei fester Breite: zwei Leerzeichen Abstand
    let cols = t.rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut widths = vec![0usize; cols];
    for r in &t.rows {
        for (i, c) in r.iter().enumerate() {
            widths[i] = widths[i].max(c.chars().count());
        }
    }
    t.rows
        .iter()
        .map(|r| {
            let mut line = String::new();
            for (i, w) in widths.iter().enumerate() {
                let c = r.get(i).map(String::as_str).unwrap_or("");
                line.push_str(c);
                if i + 1 < widths.len() {
                    line.extend(std::iter::repeat(' ').take(w - c.chars().count() + 2));
                }
            }
            line.trim_end().to_string()
        })
        .collect()
}

fn txt(content: &Content) -> String {
    match content {
        Content::Text(lines) => lines.join("\n") + "\n",
        Content::Tables(tables) => tables
            .iter()
            .map(|t| {
                let mut s = String::new();
                if tables.len() > 1 {
                    s.push_str(&format!("== {}\n", t.name));
                }
                s.push_str(&table_as_lines(t).join("\n"));
                s.push('\n');
                s
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn md(content: &Content) -> String {
    match content {
        Content::Text(lines) => lines.iter().map(|l| l.trim_end().to_string()).collect::<Vec<_>>().join("\n") + "\n",
        Content::Tables(tables) => tables
            .iter()
            .map(|t| {
                let cols = t.rows.iter().map(|r| r.len()).max().unwrap_or(0);
                let mut s = String::new();
                if tables.len() > 1 {
                    s.push_str(&format!("## {}\n\n", t.name));
                }
                let cell = |c: &str| c.replace('|', "\\|").replace('\n', " ");
                for (ri, r) in t.rows.iter().enumerate() {
                    s.push('|');
                    for i in 0..cols {
                        s.push(' ');
                        s.push_str(&cell(r.get(i).map(String::as_str).unwrap_or("")));
                        s.push_str(" |");
                    }
                    s.push('\n');
                    if ri == 0 {
                        s.push('|');
                        for _ in 0..cols {
                            s.push_str(" --- |");
                        }
                        s.push('\n');
                    }
                }
                s
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn html(content: &Content, title: &str) -> String {
    let mut s = format!("<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\" />\n<title>{}</title>\n<style>body{{font-family:system-ui,sans-serif;margin:2rem}}table{{border-collapse:collapse}}td,th{{border:1px solid #999;padding:4px 8px;text-align:left}}th{{background:#eee}}</style>\n</head>\n<body>\n", escape(title));
    match content {
        Content::Text(lines) => {
            for l in lines {
                if l.trim().is_empty() {
                    continue;
                }
                s.push_str(&format!("<p>{}</p>\n", escape(l)));
            }
        }
        Content::Tables(tables) => {
            for t in tables {
                if tables.len() > 1 {
                    s.push_str(&format!("<h2>{}</h2>\n", escape(&t.name)));
                }
                s.push_str("<table>\n");
                for (ri, r) in t.rows.iter().enumerate() {
                    let tag = if ri == 0 { "th" } else { "td" };
                    s.push_str("<tr>");
                    for c in r {
                        s.push_str(&format!("<{tag}>{}</{tag}>", escape(c)));
                    }
                    s.push_str("</tr>\n");
                }
                s.push_str("</table>\n");
            }
        }
    }
    s.push_str("</body>\n</html>\n");
    s
}

fn delimited(content: &Content, delim: char) -> String {
    let quote = |c: &str| {
        if c.contains(delim) || c.contains('"') || c.contains('\n') || c.contains('\r') {
            format!("\"{}\"", c.replace('"', "\"\""))
        } else {
            c.to_string()
        }
    };
    let rows: Vec<Vec<String>> = match content {
        Content::Text(lines) => lines.iter().map(|l| vec![l.clone()]).collect(),
        Content::Tables(tables) => tables.iter().flat_map(|t| t.rows.iter().cloned()).collect(),
    };
    let mut s = String::new();
    for r in rows {
        s.push_str(&r.iter().map(|c| quote(c)).collect::<Vec<_>>().join(&delim.to_string()));
        s.push_str("\r\n");
    }
    s
}

fn json(content: &Content, title: &str) -> Result<Vec<u8>, String> {
    let value = match content {
        Content::Text(lines) => serde_json::json!({ "title": title, "lines": lines }),
        Content::Tables(tables) => {
            let mut map = serde_json::Map::new();
            for t in tables {
                let header: Vec<String> = t.rows.first().cloned().unwrap_or_default();
                let records: Vec<serde_json::Value> = t.rows
                    .iter()
                    .skip(1)
                    .map(|r| {
                        let mut o = serde_json::Map::new();
                        for (i, c) in r.iter().enumerate() {
                            let key = header.get(i).filter(|h| !h.trim().is_empty()).cloned().unwrap_or_else(|| format!("col{}", i + 1));
                            o.insert(key, serde_json::Value::String(c.clone()));
                        }
                        serde_json::Value::Object(o)
                    })
                    .collect();
                map.insert(t.name.clone(), serde_json::Value::Array(records));
            }
            if tables.len() == 1 {
                map.into_iter().next().map(|(_, v)| v).unwrap_or(serde_json::Value::Array(vec![]))
            } else {
                serde_json::Value::Object(map)
            }
        }
    };
    serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())
}

fn zip_build(entries: &[(&str, &[u8])], mimetype: Option<&str>) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    {
        let mut w = ZipWriter::new(&mut out);
        if let Some(m) = mimetype {
            w.start_file("mimetype", SimpleFileOptions::default().compression_method(CompressionMethod::Stored)).map_err(|e| e.to_string())?;
            w.write_all(m.as_bytes()).map_err(|e| e.to_string())?;
        }
        for (name, data) in entries {
            w.start_file(*name, SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)).map_err(|e| e.to_string())?;
            w.write_all(data).map_err(|e| e.to_string())?;
        }
        w.finish().map_err(|e| e.to_string())?;
    }
    Ok(out.into_inner())
}

fn is_number(s: &str) -> bool {
    let t = s.trim();
    !t.is_empty() && t.len() <= 15 && t.parse::<f64>().is_ok() && !t.starts_with('+') && !(t.len() > 1 && t.starts_with('0') && !t.starts_with("0."))
}

fn col_name(mut n: usize) -> String {
    let mut s = String::new();
    n += 1;
    while n > 0 {
        let r = (n - 1) % 26;
        s.insert(0, (b'A' + r as u8) as char);
        n = (n - 1) / 26;
    }
    s
}

fn content_tables(content: &Content, name: &str) -> Vec<Table> {
    match content {
        Content::Tables(t) => t.clone(),
        Content::Text(lines) => vec![Table { name: name.into(), rows: lines.iter().map(|l| vec![l.clone()]).collect() }],
    }
}

fn xlsx(content: &Content) -> Result<Vec<u8>, String> {
    let tables = content_tables(content, "Text");
    let mut sheets_xml = Vec::new();
    for t in &tables {
        let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>");
        for (ri, r) in t.rows.iter().enumerate() {
            s.push_str(&format!("<row r=\"{}\">", ri + 1));
            for (ci, c) in r.iter().enumerate() {
                let cell_ref = format!("{}{}", col_name(ci), ri + 1);
                if c.is_empty() {
                    continue;
                }
                if ri > 0 && is_number(c) {
                    s.push_str(&format!("<c r=\"{cell_ref}\"><v>{}</v></c>", c.trim()));
                } else {
                    let sp = if c.starts_with(' ') || c.ends_with(' ') { " xml:space=\"preserve\"" } else { "" };
                    s.push_str(&format!("<c r=\"{cell_ref}\" t=\"inlineStr\"><is><t{sp}>{}</t></is></c>", escape(c)));
                }
            }
            s.push_str("</row>");
        }
        s.push_str("</sheetData></worksheet>");
        sheets_xml.push(s);
    }
    let mut wb = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets>");
    let mut wb_rels = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">");
    let mut ct = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>");
    for (i, t) in tables.iter().enumerate() {
        let name: String = t.name.chars().filter(|c| !"[]:*?/\\".contains(*c)).take(31).collect();
        let name = if name.trim().is_empty() { format!("Tabelle{}", i + 1) } else { name };
        wb.push_str(&format!("<sheet name=\"{}\" sheetId=\"{}\" r:id=\"rId{}\"/>", escape(&name), i + 1, i + 1));
        wb_rels.push_str(&format!("<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet{}.xml\"/>", i + 1, i + 1));
        ct.push_str(&format!("<Override PartName=\"/xl/worksheets/sheet{}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>", i + 1));
    }
    wb.push_str("</sheets></workbook>");
    wb_rels.push_str(&format!("<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/></Relationships>", tables.len() + 1));
    ct.push_str("</Types>");
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>";
    let styles = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><fonts count=\"1\"><font><sz val=\"11\"/><name val=\"Calibri\"/></font></fonts><fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill></fills><borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs><cellXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/></cellXfs><cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles></styleSheet>";
    let mut entries: Vec<(String, Vec<u8>)> = vec![
        ("[Content_Types].xml".into(), ct.into_bytes()),
        ("_rels/.rels".into(), rels.as_bytes().to_vec()),
        ("xl/workbook.xml".into(), wb.into_bytes()),
        ("xl/_rels/workbook.xml.rels".into(), wb_rels.into_bytes()),
        ("xl/styles.xml".into(), styles.as_bytes().to_vec()),
    ];
    for (i, s) in sheets_xml.into_iter().enumerate() {
        entries.push((format!("xl/worksheets/sheet{}.xml", i + 1), s.into_bytes()));
    }
    let refs: Vec<(&str, &[u8])> = entries.iter().map(|(n, d)| (n.as_str(), d.as_slice())).collect();
    zip_build(&refs, None)
}

fn docx_para(text: &str) -> String {
    let sp = if text.contains("  ") || text.starts_with(' ') || text.ends_with(' ') { " xml:space=\"preserve\"" } else { "" };
    let mut s = String::from("<w:p><w:r>");
    let parts: Vec<&str> = text.split('\t').collect();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            s.push_str("<w:tab/>");
        }
        s.push_str(&format!("<w:t{sp}>{}</w:t>", escape(p)));
    }
    s.push_str("</w:r></w:p>");
    s
}

fn docx(content: &Content) -> Result<Vec<u8>, String> {
    let mut body = String::new();
    match content {
        Content::Text(lines) => {
            for l in lines {
                body.push_str(&docx_para(l));
            }
        }
        Content::Tables(tables) => {
            for t in tables {
                if tables.len() > 1 {
                    body.push_str(&docx_para(&t.name));
                }
                let cols = t.rows.iter().map(|r| r.len()).max().unwrap_or(1);
                body.push_str("<w:tbl><w:tblPr><w:tblStyle w:val=\"TableGrid\"/><w:tblW w:w=\"0\" w:type=\"auto\"/><w:tblBorders><w:top w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"999999\"/><w:left w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"999999\"/><w:bottom w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"999999\"/><w:right w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"999999\"/><w:insideH w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"999999\"/><w:insideV w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"999999\"/></w:tblBorders></w:tblPr><w:tblGrid>");
                for _ in 0..cols {
                    body.push_str("<w:gridCol/>");
                }
                body.push_str("</w:tblGrid>");
                for (ri, r) in t.rows.iter().enumerate() {
                    body.push_str("<w:tr>");
                    for i in 0..cols {
                        let c = r.get(i).map(String::as_str).unwrap_or("");
                        let para = if ri == 0 { docx_para(c).replace("<w:r>", "<w:r><w:rPr><w:b/></w:rPr>") } else { docx_para(c) };
                        body.push_str(&format!("<w:tc>{para}</w:tc>"));
                    }
                    body.push_str("</w:tr>");
                }
                body.push_str("</w:tbl>");
                body.push_str(&docx_para(""));
            }
        }
    }
    let doc = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1417\" w:right=\"1417\" w:bottom=\"1134\" w:left=\"1417\" w:header=\"708\" w:footer=\"708\" w:gutter=\"0\"/></w:sectPr></w:body></w:document>");
    let ct = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/></Types>";
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/></Relationships>";
    zip_build(&[("[Content_Types].xml", ct.as_bytes()), ("_rels/.rels", rels.as_bytes()), ("word/document.xml", doc.as_bytes())], None)
}

fn odf_text_p(text: &str) -> String {
    // Mehrfache Leerzeichen und Tabs als ODF-Elemente
    let mut s = String::from("<text:p>");
    let mut spaces = 0;
    let flush = |s: &mut String, spaces: &mut usize| {
        if *spaces == 1 {
            s.push(' ');
        } else if *spaces > 1 {
            s.push(' ');
            s.push_str(&format!("<text:s text:c=\"{}\"/>", *spaces - 1));
        }
        *spaces = 0;
    };
    for c in text.chars() {
        match c {
            ' ' => spaces += 1,
            '\t' => {
                flush(&mut s, &mut spaces);
                s.push_str("<text:tab/>");
            }
            c => {
                flush(&mut s, &mut spaces);
                s.push_str(&escape(&c.to_string()));
            }
        }
    }
    flush(&mut s, &mut spaces);
    s.push_str("</text:p>");
    s
}

fn odf(content: &Content, spreadsheet: bool) -> Result<Vec<u8>, String> {
    let ns = "xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" xmlns:table=\"urn:oasis:names:tc:opendocument:xmlns:table:1.0\" xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\" office:version=\"1.2\"";
    let mime = if spreadsheet { "application/vnd.oasis.opendocument.spreadsheet" } else { "application/vnd.oasis.opendocument.text" };
    let mut body = String::new();
    if spreadsheet {
        body.push_str("<office:spreadsheet>");
        for (i, t) in content_tables(content, "Text").iter().enumerate() {
            let name = if t.name.trim().is_empty() { format!("Tabelle{}", i + 1) } else { t.name.clone() };
            body.push_str(&format!("<table:table table:name=\"{}\"><table:table-column/>", escape(&name)));
            for (ri, r) in t.rows.iter().enumerate() {
                body.push_str("<table:table-row>");
                for c in r {
                    if ri > 0 && is_number(c) {
                        body.push_str(&format!("<table:table-cell office:value-type=\"float\" office:value=\"{}\"><text:p>{}</text:p></table:table-cell>", c.trim(), escape(c.trim())));
                    } else {
                        body.push_str(&format!("<table:table-cell office:value-type=\"string\">{}</table:table-cell>", odf_text_p(c)));
                    }
                }
                body.push_str("</table:table-row>");
            }
            body.push_str("</table:table>");
        }
        body.push_str("</office:spreadsheet>");
    } else {
        body.push_str("<office:text>");
        match content {
            Content::Text(lines) => {
                for l in lines {
                    body.push_str(&odf_text_p(l));
                }
            }
            Content::Tables(tables) => {
                for (i, t) in tables.iter().enumerate() {
                    if tables.len() > 1 {
                        body.push_str(&odf_text_p(&t.name));
                    }
                    let cols = t.rows.iter().map(|r| r.len()).max().unwrap_or(1);
                    body.push_str(&format!("<table:table table:name=\"T{}\"><table:table-column table:number-columns-repeated=\"{cols}\"/>", i + 1));
                    for r in &t.rows {
                        body.push_str("<table:table-row>");
                        for ci in 0..cols {
                            body.push_str(&format!("<table:table-cell office:value-type=\"string\">{}</table:table-cell>", odf_text_p(r.get(ci).map(String::as_str).unwrap_or(""))));
                        }
                        body.push_str("</table:table-row>");
                    }
                    body.push_str("</table:table>");
                }
            }
        }
        body.push_str("</office:text>");
    }
    let content_xml = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<office:document-content {ns}><office:body>{body}</office:body></office:document-content>");
    let manifest = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<manifest:manifest xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\" manifest:version=\"1.2\"><manifest:file-entry manifest:full-path=\"/\" manifest:version=\"1.2\" manifest:media-type=\"{mime}\"/><manifest:file-entry manifest:full-path=\"content.xml\" manifest:media-type=\"text/xml\"/></manifest:manifest>");
    zip_build(&[("content.xml", content_xml.as_bytes()), ("META-INF/manifest.xml", manifest.as_bytes())], Some(mime))
}

/// Minimales PDF: Courier, A4, Zeilenumbruch nach Breite, Seitenzahlen. Zeichen außerhalb
/// von Windows-1252 werden durch „?“ ersetzt (Standard-14-Schriften kennen kein Unicode).
fn pdf(content: &Content, title: &str) -> Vec<u8> {
    const FONT_SIZE: f32 = 9.0;
    const LINE_H: f32 = 11.5;
    const MARGIN: f32 = 50.0;
    const PAGE_W: f32 = 595.28;
    const PAGE_H: f32 = 841.89;
    let max_chars = ((PAGE_W - 2.0 * MARGIN) / (FONT_SIZE * 0.6)) as usize; // Courier: 0,6 em
    let lines_per_page = ((PAGE_H - 2.0 * MARGIN) / LINE_H) as usize;

    let raw: Vec<String> = match content {
        Content::Text(l) => l.clone(),
        Content::Tables(tables) => {
            let mut v = Vec::new();
            for t in tables {
                if tables.len() > 1 {
                    v.push(format!("== {}", t.name));
                }
                v.extend(table_as_lines(t));
                v.push(String::new());
            }
            v
        }
    };
    let mut lines: Vec<String> = Vec::new();
    for l in raw {
        let l = l.replace('\t', "    ");
        let chars: Vec<char> = l.chars().collect();
        if chars.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut i = 0;
        while i < chars.len() {
            let end = (i + max_chars).min(chars.len());
            lines.push(chars[i..end].iter().collect());
            i = end;
        }
    }
    let pages: Vec<&[String]> = if lines.is_empty() { vec![&[]] } else { lines.chunks(lines_per_page).collect() };

    let esc = |s: &str| -> Vec<u8> {
        let (bytes, _, _) = encoding_rs::WINDOWS_1252.encode(s);
        let mut out = Vec::with_capacity(bytes.len());
        for b in bytes.iter() {
            match b {
                b'(' | b')' | b'\\' => {
                    out.push(b'\\');
                    out.push(*b);
                }
                b'\r' | b'\n' => out.push(b' '),
                _ => out.push(*b),
            }
        }
        out
    };

    let mut objects: Vec<Vec<u8>> = Vec::new();
    // 1 = Katalog, 2 = Seitenbaum, 3 = Schrift, dann je Seite: Seite + Inhalt
    let n_pages = pages.len();
    let mut kids = String::new();
    for i in 0..n_pages {
        kids.push_str(&format!("{} 0 R ", 4 + i * 2));
    }
    objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    objects.push(format!("<< /Type /Pages /Kids [{kids}] /Count {n_pages} >>").into_bytes());
    objects.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding >>".to_vec());
    for (pi, page) in pages.iter().enumerate() {
        let mut stream: Vec<u8> = Vec::new();
        stream.extend_from_slice(format!("BT /F1 {FONT_SIZE} Tf {LINE_H} TL {MARGIN} {} Td\n", PAGE_H - MARGIN).as_bytes());
        for l in page.iter() {
            stream.extend_from_slice(b"(");
            stream.extend_from_slice(&esc(l));
            stream.extend_from_slice(b") Tj T*\n");
        }
        stream.extend_from_slice(b"ET\n");
        let footer = format!("{} - {}/{}", title, pi + 1, n_pages);
        stream.extend_from_slice(format!("BT /F1 7 Tf {MARGIN} {} Td (", MARGIN / 2.0).as_bytes());
        stream.extend_from_slice(&esc(&footer));
        stream.extend_from_slice(b") Tj ET\n");
        let page_obj = format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W} {PAGE_H}] /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>", 5 + pi * 2);
        objects.push(page_obj.into_bytes());
        let mut content_obj = format!("<< /Length {} >>\nstream\n", stream.len()).into_bytes();
        content_obj.extend_from_slice(&stream);
        content_obj.extend_from_slice(b"\nendstream");
        objects.push(content_obj);
    }
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(obj);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for o in offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
    out
}

fn rtf(content: &Content) -> String {
    let esc = |s: &str| -> String {
        let mut o = String::new();
        for c in s.chars() {
            match c {
                '\\' => o.push_str("\\\\"),
                '{' => o.push_str("\\{"),
                '}' => o.push_str("\\}"),
                '\t' => o.push_str("\\tab "),
                c if (c as u32) < 128 => o.push(c),
                c => o.push_str(&format!("\\u{}?", c as u32 as i32 as i16 as i32)),
            }
        }
        o
    };
    let mut s = String::from("{\\rtf1\\ansi\\ansicpg1252\\deff0{\\fonttbl{\\f0\\fmodern Courier New;}}\\f0\\fs18\n");
    match content {
        Content::Text(lines) => {
            for l in lines {
                s.push_str(&esc(l));
                s.push_str("\\par\n");
            }
        }
        Content::Tables(tables) => {
            for t in tables {
                if tables.len() > 1 {
                    s.push_str(&format!("\\b {}\\b0\\par\n", esc(&t.name)));
                }
                for l in table_as_lines(t) {
                    s.push_str(&esc(&l));
                    s.push_str("\\par\n");
                }
                s.push_str("\\par\n");
            }
        }
    }
    s.push('}');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Content {
        Content::Tables(vec![Table {
            name: "Kunden".into(),
            rows: vec![vec!["Kd-Nr".into(), "Name".into(), "Betrag".into()], vec!["10482".into(), "Anna Berger".into(), "120,50".into()], vec!["10483".into(), "Jonas & Co".into(), "33".into()]],
        }])
    }

    #[test]
    fn writers_produce_output() {
        let t = table();
        for target in Target::ALL {
            let bytes = render(&t, target, "Test").unwrap();
            assert!(!bytes.is_empty(), "{target:?}");
        }
        let text = Content::Text(vec!["Hallo Welt".into(), "".into(), "Zeile (mit) Klammern & Umlauten äöü".into()]);
        for target in Target::ALL {
            let bytes = render(&text, target, "Test").unwrap();
            assert!(!bytes.is_empty(), "{target:?}");
        }
        assert!(String::from_utf8(render(&t, Target::Csv, "x").unwrap()).unwrap().starts_with("Kd-Nr;Name;Betrag\r\n10482;"));
        assert!(String::from_utf8(render(&t, Target::Md, "x").unwrap()).unwrap().contains("| --- |"));
        assert!(String::from_utf8(render(&t, Target::Html, "x").unwrap()).unwrap().contains("<td>Jonas &amp; Co</td>"));
        let pdf = render(&text, Target::Pdf, "x").unwrap();
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF\n"));
    }

    #[test]
    fn xlsx_roundtrip() {
        let t = table();
        let bytes = render(&t, Target::Xlsx, "x").unwrap();
        let back = extract(&Kind::Xlsx, &bytes, "").unwrap();
        match back {
            Content::Tables(ts) => {
                assert_eq!(ts[0].name, "Kunden");
                assert_eq!(ts[0].rows[1], vec!["10482", "Anna Berger", "120,50"]);
                assert_eq!(ts[0].rows[2][2], "33");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn docx_and_odf_roundtrip() {
        let text = Content::Text(vec!["Sehr geehrte Frau Berger,".into(), "Zeile\tmit Tab".into()]);
        let bytes = render(&text, Target::Docx, "x").unwrap();
        assert_eq!(extract(&Kind::Docx, &bytes, "").unwrap(), text);
        let bytes = render(&text, Target::Odt, "x").unwrap();
        assert_eq!(extract(&Kind::Odf, &bytes, "").unwrap(), text);
        let t = table();
        let bytes = render(&t, Target::Ods, "x").unwrap();
        assert_eq!(extract(&Kind::Odf, &bytes, "").unwrap(), t);
        // mimetype liegt unkomprimiert an erster Stelle
        let mut a = ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
        let f = a.by_index(0).unwrap();
        assert_eq!(f.name(), "mimetype");
        assert_eq!(f.compression(), CompressionMethod::Stored);
    }
}
