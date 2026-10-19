# anonym. 1.0 — Audit (Funktion & Sicherheit)

Stand: 2026-10-19 · Basis `5126f76` · Version 1.0.0 (Test-Build für macOS und Windows)

„Getestet“ heißt hier: tatsächlich ausgeführt — Rust-Unit-/Integrationstest, CLI-Lauf oder die
Web-Oberfläche in headless Chrome gegen eine Mock-IPC-Schicht. „Statisch geprüft“ heißt: Code
gelesen, Aufrufkette Frontend → `invoke` → Rust-Kommando abgeglichen, aber nicht ausgeführt.
Native Teile (Dateidialoge, Drag & Drop, Updater, Fenster) brauchen den manuellen Test unten.

## Tests

| Befehl | Ergebnis |
| --- | --- |
| `cargo test --workspace` | 43 Tests grün (Kern 40, App 3); dreimal hintereinander stabil |
| `pnpm build` (`tsc` + Vite) | fehlerfrei |
| `cargo clippy --workspace --all-targets` | nur Stil-Hinweise aus dem Bestand (`repeat_n`, `is_multiple_of`), keine Fehler |
| `pnpm audit` | 0 Schwachstellen (vorher 1 hoch: `source-map-js`, nur Build-Werkzeug — aktualisiert) |
| `cargo audit` | nicht installiert — nicht gelaufen |
| UI-Szenario (headless Chrome, Mock-IPC) | Datei öffnen, Fundstellen, Ablehnen, Ausnahme → Neu-Analyse, Speichern mit Passwort (Wiederholung), Bericht, Regelwerk/Saat „Neu“, Vorschau, Einstellungen, DE/EN, Dunkel/Hell |

## Funktionsumfang und Teststatus

| Funktion | Status |
| --- | --- |
| Erkennung: Wörterbuch-Namen, Anrede/Titel, Adressen, PLZ, Datum, E-Mail, Telefon, IBAN, Karte, Steuer-ID/USt-IdNr., SV-Nr., Kennzeichen, IP, Kunden-/Aktenzeichen | getestet (Kern-Tests `detect::*`, `checksum::*`) |
| Spalten-Typisierung CSV/TSV, feste Breiten, XLSX-Kopfzeile in Zeile 1–10, Mehrheits-Personenspalten | getestet |
| Strategien Pseudonym / Platzhalter / Schwärzen / Maskieren / Löschen | getestet (`apply::tests::strategies`) |
| Deterministische Pseudonyme, Familien-Nachnamen, Prüfziffern bleiben gültig, Datumsverschiebung | getestet |
| Formate lesen/schreiben: Text (UTF-8/BOM/UTF-16/Codepages), CSV, DOCX, XLSX, ODT/ODS | getestet |
| DOCX: gelöschter Text (Änderungen verfolgen), Link-Ziele (`mailto:`), Metadaten entfernen | getestet (neu: `docx_deleted_text_links_and_metadata`, `docx_leaves_no_trace`) |
| XLSX: Kommentare prüfen, Kommentar-Autoren/Personenliste entfernen | getestet (neu) — `xl/comments1.xml` war vorher zwar gelesen, aber nie geprüft |
| ODF: Kopf-/Fußzeilen (`styles.xml`), Links, `meta.xml` | getestet (neu) |
| Export in 12 Formate (TXT, MD, HTML, CSV, TSV, JSON, XLSX, DOCX, ODT, ODS, PDF, RTF) | getestet (Erzeugung + XLSX/DOCX/ODF-Rundreise); Darstellung in Office/PDF-Viewern nur manuell |
| Recover-Datei: schreiben, Klartext/verschlüsselt, Rückübersetzung nach Bearbeitung, Formatwechsel | getestet |
| Recover-Dialog (Datei, Recover-Datei-Vorschlag, Passwort, Format) | statisch geprüft (IPC-Namen/Argumente stimmen) |
| Bericht (JSON + Reiter) | getestet (UI-Szenario: Reiter, Werte aus Mock) |
| Fundstellen: annehmen/ablehnen, Kategorie, eigener Ersatz, Ausnahmeliste, Filter, Massenaktionen | getestet (UI-Szenario: Ablehnen + Ausnahme + Neu-Analyse) |
| Vorschau zweispaltig, Klick schaltet um, „nur geänderte Zeilen“ | getestet (Rendering), Klick statisch |
| Regelwerk: Name, Saat + „Neu“, Namenswelt, Datumsverschiebung, Kategorien, Ausnahmen, eigene Muster/Wortlisten, Import/Export | Saat getestet (UI + Rust), Rest statisch |
| Pseudonym-Speicher (masked): nutzen, exportieren, laden (übernimmt Saat), löschen | Kern getestet, Kommandos statisch |
| Einstellungen: Sprache, Thema, Akzent, Zielordner, Namenszusatz, Codepage, Bericht, Recover, Verschlüsselung, Überschreiben-Rückfrage, masked, Auto-Update | Bereinigung getestet (`sanitizes_suffix_and_enums`), UI teilweise im Szenario |
| Mehrere Dateien, „Alle speichern“ (masked) | statisch |
| Tastatur: O, S, R, 1–4, ?, ⌘/Strg+, , Esc | statisch |
| Drag & Drop, Dateidialoge, Ordner öffnen, Updater | nur manuell (nativ) |
| Handbuch + Tutorial DE/EN, Sprachumschalter | Inhalte aktualisiert; Öffnen im Szenario sichtbar |

## Behobene Fehler

1. **Entscheidungen an falscher Fundstelle** (hoch): Nach jeder neuen Analyse (Ausnahme, Regeländerung)
   werden Fundstellen neu nummeriert; Entscheidungen hingen an der id. Beispiel: IBAN abgelehnt, dann
   Datum auf die Ausnahmeliste → die Ablehnung wanderte auf die E-Mail, die IBAN wurde ersetzt.
   Jetzt über Byte-Position + Text zugeordnet (UI-Szenario reproduziert und bestätigt).
2. **XLSX-Kommentare nie anonymisiert**: `xl/comments1.xml` wurde gelesen, aber nicht in Knoten
   überführt. Jetzt alle `xl/comments*.xml` und Threaded Comments.
3. **Passwortpflicht nach dem Schreiben geprüft**: Bei „verschlüsseln“ ohne Passwort war die Ausgabe
   schon geschrieben (und der Speicher gesichert), nur die Recover-Datei fehlte. Jetzt vorher.
4. **Passwort-Tippfehler** sperrte die Rückübersetzung: Eingabe jetzt doppelt.
5. **„Alle speichern“** fragte nach abgebrochener Passwortabfrage für jede Datei erneut.
6. **Hinweise nur auf Deutsch** in der englischen Oberfläche → Codes aus dem Kern, Übersetzung im UI.
7. **?-Knopf verdeckte „Anonymisiert speichern“** rechts unten.
8. **Toast** nannte die Quelle statt der geschriebenen Datei.
9. **Schlüsseldatei laden** war mit anderer Saat wirkungslos (Speicher wurde beim nächsten Lauf geleert)
   → Saat und Namenswelt werden ins Regelwerk übernommen.
10. Neu-Analyse aller Dateien lief als Seiteneffekt in einem State-Updater (im Dev-Modus doppelt).

## Sicherheit

| Schwere | Befund | Status |
| --- | --- | --- |
| hoch | **Öffentliche Standard-Saat „anonym“**: Mit Saat + Open-Source-Werkzeug sind Pseudonyme per Wörterbuch, Nummern per Durchprobieren und die Datumsverschiebung exakt nachrechenbar — Geburtsdaten wären sofort rückrechenbar. | behoben: Zufalls-Saat (96 Bit) beim ersten Start / statt „anonym“ (außer ein gefüllter Speicher hängt daran → Warnung im Regelwerk), Knopf „Neu“, Hinweis im Handbuch |
| hoch | **Metadaten-Leck in DOCX/XLSX/ODF**: Autor, zuletzt geändert von, Titel, Firma, Kommentar-/Überarbeitungs-Autoren, gelöschter Text und `mailto:`-Links blieben Byte für Byte erhalten. | behoben + Test, der jede Archiv-Datei der Ausgabe auf Reste prüft |
| mittel | **`open_path` öffnete beliebige Pfade/URLs** aus dem WebView (bei XSS: Programm starten). | behoben: nur Ordner und zwei feste https-Präfixe; `opener:default` aus den Capabilities entfernt (Rust-seitiger Aufruf braucht es nicht) |
| mittel | **Quelle überschreibbar**: Vergleich `src == out` per Zeichenkette; `C:\a.txt` vs `c:/A.txt`, Symlinks, `..` und Bericht/Recover-Pfad = Quelle gingen durch; Recover konnte die Recover-Datei überschreiben. | behoben: kanonischer Vergleich (Windows/macOS ohne Groß/Klein), alle drei Ziele geprüft |
| mittel | **ZIP-Bombe / Riesendateien**: entpackte Teile ohne Grenze in den Speicher. | behoben: 256 MB je Teil (gegen gelogene Größenangaben per `take`), 512 MB Eingabe, 64 MB für JSON-Schlüsseldateien |
| mittel | **Panik bei manipulierter Recover-Datei** (`XNonce::from_slice` mit falscher Länge). | behoben + Test |
| niedrig | Spaltenreferenz mit vielen Buchstaben → Integer-Überlauf (Debug-Panik). | behoben (gekappt bei XFD) |
| niedrig | Namenszusatz aus den Einstellungen ungeprüft (Pfadtrenner). | behoben (`Settings::sanitized`) |
| niedrig | Schlüssel nach Argon2id im Speicher stehen gelassen. | behoben (volatiles Überschreiben beim Drop) |
| niedrig | Temp-Datei `<stamm>.tmp` konnte Nachbardateien treffen, blieb bei Fehlern liegen. | behoben (`.<name>.<pid>.anonym-tmp`, Aufräumen) |
| niedrig | Export-Titel (HTML/PDF-Fußzeile) aus dem Quellnamen — der kann Namen tragen. | behoben (Ausgabename) |
| info | Bericht und Recover-Datei (Standard: Klartext) liegen neben der Ausgabe und enthalten die Originale. | bewusst so; Hinweise in Einstellungen und Handbuch. Empfehlung: Verschlüsselung nutzen, Ausgabeordner trennen |
| info | CSP `default-src 'self'`, kein `dangerouslySetInnerHTML`, kein Markdown/HTML-Rendering von Dateiinhalt (React-Text), Rust-Regex (linear, kein ReDoS) | geprüft, in Ordnung |
| info | Updater: Pubkey gesetzt, Endpoint https (GitHub Releases), Signatur-Pflicht durch Tauri | geprüft |
| info | Krypto: XChaCha20-Poly1305 mit zufälliger 192-Bit-Nonce und frischem 16-Byte-Salz je Datei, Argon2id v19 (19 MiB, t=2, p=1) | geprüft; Parameter stehen nicht in der Datei (fest, für Lesbarkeit alter Dateien) |
| info | TLS: keine eigene HTTP-Logik, nur Tauri-Updater (rustls/Systemstandard) | geprüft |
| info | localStorage: nur `anonym.tutorialSeen`; Passwort nur im React-Ref der Sitzung | geprüft |

CI (`.github/workflows/build.yml`): für `workflow_dispatch` mit 1.0.0 nichts Blockierendes gefunden —
MSI verlangt numerische Version (1.0.0 passt), Signatur/Notarisierung unverändert, `cargo test` läuft
auch unter Windows (Tests nutzen `temp_dir()` und vergleichen nur Dateinamen).

## Bekannte Grenzen

- Nachnamen ohne Vorname/Anrede im Fließtext werden nicht erkannt (Wortlisten helfen).
- Nicht geprüft: XLSX-Zahlenzellen ohne typisierte Kopfzeile, Blattnamen, Kopf-/Fußzeilen von
  Excel-Blättern, Bilder, eingebettete Objekte, `docProps/custom.xml`.
- DOCX: Ein über zwei Formatierungsläufe verteilter Name erscheint geteilt.
- Kategoriewechsel einer Fundstelle: neuer Ersatz erst beim Speichern sichtbar (Tabelle/Vorschau zeigen den alten).
- Fehlermeldungen aus dem Rust-Kern sind deutsch, auch in der englischen Oberfläche.
- Ändert man die Saat, wird ein vorhandener Pseudonym-Speicher beim nächsten Speichern geleert (dokumentiert).
- Linux-Builds entstehen in der CI, wurden hier nicht betrachtet.

## Manuelle Testliste (macOS und Windows)

Je Plattform mit dem CI-Artefakt 1.0.0, frisches Benutzerkonto oder gelöschter Datenordner
(macOS `~/Library/Application Support/com.lan-solo.anonym`, Windows `%APPDATA%\com.lan-solo.anonym`).

1. Start: Fenster erscheint, Tutorial läuft, Sprache folgt dem System; macOS: kein Gatekeeper-Dialog
   (notarisiert); Windows: Installer (NSIS und MSI) installiert und startet.
2. Regelwerk: Saat ist 24 Hex-Zeichen (nicht „anonym“); „Neu“ ändert sie; Neustart behält sie.
3. Datei öffnen per Dialog **und** per Drag & Drop (Finder/Explorer): TXT, CSV (;), XLSX, DOCX, ODT.
   Windows: Pfad mit Laufwerksbuchstaben, Leerzeichen und Umlauten (`C:\Users\…\Prüfung Kunden.xlsx`).
4. Fundstellen: IBAN ablehnen, dann ein anderes Original per Schild auf die Ausnahmeliste → IBAN bleibt abgelehnt.
5. Speichern (S): `<name>.anonym.<ext>`, Bericht und Recover-Datei daneben; Toast nennt die Ausgabe.
   Zweites Speichern → Rückfrage „Überschreiben / Anderer Name“.
6. Windows: Ausgabe in Excel geöffnet lassen und erneut speichern → verständliche Fehlermeldung (Datei gesperrt), keine Temp-Reste.
7. „Speichern unter …“ auf die Quelle selbst (auch mit anderer Groß-/Kleinschreibung) → abgelehnt.
8. DOCX mit „Änderungen verfolgen“, Kommentar und mailto-Link anonymisieren, in Word öffnen:
   Datei öffnet ohne Reparatur, Autor in Datei → Eigenschaften leer, Kommentar-Autor „anonym“, Link-Ziel ersetzt.
9. XLSX mit Kommentar: in Excel öffnen ohne Reparaturhinweis, Kommentartext anonymisiert.
10. Export: jede der 12 Endungen einmal, Datei öffnet in der Standard-App.
11. Einstellungen → Dateien: „verschlüsseln“ an → Speichern fragt Passwort zweimal; abweichend = Weiter gesperrt.
12. Wiederherstellen (R): bearbeitete Ausgabe + Recover-Datei (Vorschlag erscheint), falsches Passwort → „Falsches Passwort“, richtiges → Ergebnis; „Ordner öffnen“ öffnet Finder/Explorer.
13. masked an: Pseudonym-Speicher nutzen, zwei Dateien speichern (gleiche Person = gleicher Ersatz), Schlüsseldatei exportieren, löschen, wieder laden (Regelwerk übernimmt Saat), „Alle speichern“.
14. Einstellungen → App: Version 1.0.0, „Auf Updates prüfen“ (ohne Release: Fehlermeldung oder „aktuell“), Website/GitHub öffnen den Browser.
15. Hell/Dunkel und alle vier Akzente, DE/EN umschalten (auch im Handbuch), Fenster auf Mindestgröße 960×620: nichts verdeckt.
16. Tastatur: O, S, R, 1–4, ?, ⌘/Strg+, , Esc.

## Verifikation (unabhängige Zweitprüfung, 2026-10-19)

Ausgangsstand 8123f01, skeptisch gegengeprüft: Diff 5126f76..8123f01 gelesen, alle Suiten selbst
ausgeführt, zehn fehleranfällige Abläufe einzeln nachvollzogen.

**Bestätigt:** Versionsstand 1.0.0 in `package.json`, `tauri.conf.json`, beiden `Cargo.toml`,
`Cargo.lock` und `APP_VERSION`; IPC-Namen/Argumente von `new_seed`, `store_import`, `apply_file`,
`recover_file`, `open_path` passen zu den Rust-Signaturen; Hinweis-Codes (`docx-runs`, `xlsx-cells`,
`metadata`, `encoding␟…`, `pattern␟…`) werden vom Kern genau so erzeugt und in DE/EN übersetzt;
`same_file` vergleicht unter Windows beide Seiten kanonisch (`\\?\`-Präfix auf beiden Seiten).

**Widerlegt / ergänzt:**
- „cargo audit nicht installiert“ stimmt nicht — `cargo audit` lief. Befund: rustls 0.23.45
  (RUSTSEC-2026-0346…0349, Updater-HTTPS) → per `cargo update -p rustls` auf 0.23.46 (nur Lockfile).
  Offen und ohne Auswirkung: zip 2.x/4.x RUSTSEC-2026-0367…0369 betreffen nur `extract()`, das
  anonym nicht aufruft; glib/zvariant/unic/proc-macro-error = Linux-GTK-Unterbau bzw. „unmaintained“.
- Die Entscheidungs-Zuordnung war nur in der Oberfläche repariert. Speichern, während nach einer
  Regeländerung (Ausnahme, 600-ms-Verzögerung) die neue Analyse noch lief, schickte weiterhin alte ids
  an `apply_file` — die dort frisch nummerierten Fundstellen trafen dann die falsche Stelle.
  Jetzt trägt jede Entscheidung `bstart` + `text`; der Kern ordnet darüber zu (id nur noch als
  Rückfall). Test `decisions_follow_position_not_stale_id`.
- Metadaten-Leck übersehen: Vorschaubilder der ersten Seite (`Thumbnails/thumbnail.png`, das
  LibreOffice immer schreibt; `docProps/thumbnail.*` bei Word) wurden unverändert kopiert und zeigten
  das Original. Werden jetzt entfernt, samt Manifest-, `.rels`- und Content-Types-Verweis.
- DOCX-Feldfunktionen (`w:instrText`, z. B. `HYPERLINK "mailto:anna@…"`) wurden nicht geprüft — jetzt
  mitgeprüft. Test `thumbnails_and_field_codes_do_not_survive`.
- ZIP-Bombe nur je Teil begrenzt: viele Teile à 256 MB summierten sich. Jetzt höchstens 1 GB
  Textteile je Dokument; unberührte Teile (Bilder usw.) werden komprimiert per `raw_copy_file`
  übernommen statt entpackt — die Ausgabe wächst nicht über die Eingabe hinaus.
- „Ordner öffnen“ bei Dateien im Laufwerks-Stamm übergab unter Windows `C:` (= aktueller Ordner
  auf C:) bzw. unter macOS einen leeren Pfad; `dirName` behält jetzt die Wurzel (`C:\`, `/`).

**Ausgeführt:** `cargo test --workspace` 45/45 grün (Kern 42, App 3); `pnpm build` grün;
`cargo clippy --workspace --all-targets` nur vorbestehende Stilwarnungen; `pnpm audit` 0;
`cargo audit` siehe oben. CLI-Durchlauf (`anonym-cli --apply --export=docx,odt,pdf,xlsx`) mit drei
echten Office-Dateien (Kopien im Scratch-Ordner, Pfad mit Leerzeichen und Umlaut): alle Ausgaben
ZIP-intakt, jedes XML wohlgeformt, Eintragsliste identisch zur Quelle, `dc:creator`/`lastModifiedBy`
leer, DOCX/ODT von macOS `textutil` lesbar. Scratch-Dateien danach gelöscht.

**Nicht selbst geprüft:** echtes Tauri-Fenster, native Dialoge, Drag & Drop, Updater, Installer,
Öffnen in Word/Excel/LibreOffice ohne Reparaturhinweis, Windows-Laufzeit — siehe manuelle Testliste
oben (Punkt 8 zusätzlich: ODT aus LibreOffice öffnen, Vorschaubild im Finder/Explorer ist generisch).
