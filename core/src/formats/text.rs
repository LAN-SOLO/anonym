//! Textkodierungen: UTF-8 (mit/ohne BOM), UTF-16 (BOM), sonst eine ältere Codepage
//! als Rückfall. Die Ausgabe wird in derselben Kodierung geschrieben.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoding {
    pub name: String,
    pub bom: bool,
    /// Alte Codepage (kein Unicode) — Hinweis in der Oberfläche
    pub legacy: bool,
}

pub const FALLBACKS: [&str; 6] = ["windows-1252", "iso-8859-15", "windows-1250", "iso-8859-2", "macintosh", "koi8-r"];

pub fn decode(bytes: &[u8], fallback: &str) -> (String, Encoding) {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return (String::from_utf8_lossy(&bytes[3..]).into_owned(), Encoding { name: "UTF-8".into(), bom: true, legacy: false });
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (s, _, _) = encoding_rs::UTF_16LE.decode(&bytes[2..]);
        return (s.into_owned(), Encoding { name: "UTF-16LE".into(), bom: true, legacy: false });
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (s, _, _) = encoding_rs::UTF_16BE.decode(&bytes[2..]);
        return (s.into_owned(), Encoding { name: "UTF-16BE".into(), bom: true, legacy: false });
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return (s.to_string(), Encoding { name: "UTF-8".into(), bom: false, legacy: false });
    }
    let enc = encoding_rs::Encoding::for_label(fallback.as_bytes()).unwrap_or(encoding_rs::WINDOWS_1252);
    let (s, _, _) = enc.decode(bytes);
    (s.into_owned(), Encoding { name: enc.name().to_string(), bom: false, legacy: true })
}

pub fn encode(text: &str, enc: &Encoding) -> Vec<u8> {
    match enc.name.as_str() {
        "UTF-8" => {
            let mut v = Vec::with_capacity(text.len() + 3);
            if enc.bom {
                v.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
            }
            v.extend_from_slice(text.as_bytes());
            v
        }
        "UTF-16LE" => {
            let mut v = vec![0xFF, 0xFE];
            for u in text.encode_utf16() {
                v.extend_from_slice(&u.to_le_bytes());
            }
            v
        }
        "UTF-16BE" => {
            let mut v = vec![0xFE, 0xFF];
            for u in text.encode_utf16() {
                v.extend_from_slice(&u.to_be_bytes());
            }
            v
        }
        name => {
            let e = encoding_rs::Encoding::for_label(name.as_bytes()).unwrap_or(encoding_rs::WINDOWS_1252);
            let (bytes, _, _) = e.encode(text);
            bytes.into_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_legacy() {
        let latin = [b'K', 0xF6, b'l', b'n'];
        let (s, e) = decode(&latin, "windows-1252");
        assert_eq!(s, "Köln");
        assert!(e.legacy);
        assert_eq!(encode(&s, &e), latin.to_vec());
        let (s2, e2) = decode("Köln".as_bytes(), "windows-1252");
        assert_eq!(s2, "Köln");
        assert!(!e2.legacy);
        let bom = [0xEF, 0xBB, 0xBF, b'x'];
        let (s3, e3) = decode(&bom, "windows-1252");
        assert_eq!(s3, "x");
        assert_eq!(encode(&s3, &e3), bom.to_vec());
    }
}
