# anonym. 1.0 — Audit (Funktion & Sicherheit)

Stand: 2026-10-20 (Runde 3) · Basis Runde 1 `5126f76`, Runde 2 `a49da59`, Runde 3 `053d72f` · Version 1.0.0 (Test-Build für macOS und Windows)

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
- Ändert man die Saat, wird ein vorhandener Pseudonym-Speicher beim nächsten Speichern geleert
  (dokumentiert; „Neu“ fragt seit Runde 2, Tippen ins Saat-Feld seit Runde 3 bei gefülltem Speicher nach;
  ein geladenes Regelwerk oder „Auf Standard zurücksetzen“ fragt dazu nicht eigens).
- Unbekannte Betriebssystem-Fehler behalten die Systemmeldung (unter Windows in der Sprache des Systems);
  technische Details hinter einer übersetzten Meldung (serde, zip, HTTP) bleiben englisch.
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
16. Tastatur: O, S, R, 1–4, ?, ⌘+, (macOS) bzw. Strg+, (Windows), Esc. Einstellungen → App und
    Handbuch → Tastatur zeigen nur die Taste der eigenen Plattform.

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

## Runde 2 (2026-10-20)

Basis `a49da59`. Alles unten ist im Code behoben, nicht nur beschrieben.

**Behoben**
- Kategoriewechsel (Rest aus Runde 1): Tabelle und Vorschau zeigten bis zum Speichern den alten
  Ersatz. Neues Kommando `preview_replacements` rechnet den Ersatz über denselben Weg wie
  `apply_file` (gemeinsame Funktion `resolve` im Kern) — die Vorschau ist byte-gleich mit der
  Ausgabe. Test `category_change_preview_matches_output`; UI-Szenario prüft Hin- und Rückwechsel.
- Fehlermeldungen (L2): Kernfehler sind feste deutsche Texte; `src/errors.ts` übersetzt sie in der
  englischen Oberfläche (Toast, Fehler-Chip der Datei, Wiederherstellen-Dialog, Update-Prüfung).
  I/O-Fehler kommen als stabile Texte mit OS-Code (`io_text`: nicht gefunden, gesperrt, Zugriff
  verweigert, voll, schreibgeschützt, Name ungültig, Pfad zu lang) statt als Systemtext.
  `tests/errors.test.ts` liest alle `Err(…)`/`ok_or(…)`/`map_err(…)`-Literale aus dem Rust-Code und
  verlangt für jedes eine englische Übersetzung — neue Fehlertexte ohne Übersetzung lassen den Test rot werden.
  ZIP-Fehler bei Export/Neuaufbau tragen jetzt ebenfalls „Kein gültiges ZIP-Archiv: …“.
- Tastenkürzel (L6): „⌘/Strg+,“ war fest verdrahtet. Jetzt ⌘ auf dem Mac, „Strg“ (DE) bzw. „Ctrl“ (EN)
  sonst — in Einstellungen → App und im Handbuch (`src/platform.ts`).
- CSV/TSV-Export mit UTF-8-BOM (L4), damit Excel unter Windows Umlaute richtig liest; Regelwerk,
  Schlüssel-, Recover- und Store-Datei werden mit BOM (Windows-Editor) gelesen. Tests in `recover`,
  `store`, `formats`.
- Windows-Dateisystem (L5): Umbenennen der Temp-Datei wird bei Freigabe-/Sperrverletzung und
  „Zugriff verweigert“ (Virenscanner, OneDrive, Office) bis zu 8-mal mit wachsender Pause (≈2 s)
  wiederholt (`rename_retry`, nur `cfg!(windows)`). Temp-Namen tragen zusätzlich einen Zähler —
  vorher teilten sich zwei gleichzeitige Schreibvorgänge auf dieselbe Datei eine Temp-Datei
  (Test mit 8 Threads). Zielordner „C:“ wird zu „C:\“ (sonst aktueller Ordner auf C:).
  Umlaute/Leerzeichen im Namen: Test.
- Haupt-Thread (L8): `import_rules`, `export_rules`, `recover_info`, `store_info`, `store_export`,
  `store_import` laufen asynchron (bis 64 MB JSON); `set_rules`/`set_settings` bleiben bewusst
  synchron, damit sie in Reihenfolge geschrieben werden.
- Laufende Aufgaben (L9): Analyse-Ergebnisse tragen eine Folgenummer je Datei — ein verspätetes
  Ergebnis einer älteren Analyse (Regeländerung während der Prüfung) überschreibt nicht mehr das
  neuere. „Alle speichern“ überspringt Dateien, die währenddessen aus der Liste entfernt wurden.
  Wiederherstellen-Dialog: schnelles Umwählen übernimmt keine veraltete Recover-Vermutung/Info.
- Bestätigungen und Fehler (L11): „Auf Standard zurücksetzen“ (Regelwerk) lief ohne Rückfrage —
  jetzt Bestätigung; „Neu“ (Saat) fragt bei gefülltem Pseudonym-Speicher; Entfernen eines nicht
  leeren eigenen Musters / einer Wortliste fragt. `set_settings` meldete Schreibfehler nicht (still
  verworfen) — jetzt Fehler-Toast. `store_clear` meldet Fehler übersetzt.
- Updater (L13): Installationsfehler wurden still verschluckt (nur der Knopf ging zurück) — jetzt
  Toast in der UI-Sprache, auch beim automatischen Update. Version in Einstellungen → App kommt aus
  der App (`getVersion`), nicht aus einer Konstante. Updater-Endpunkt (HTTPS, GitHub latest.json)
  und Pubkey unverändert; der Versionsvergleich ist der semver-Vergleich des Plugins.
- Erststart (L3): Rust nimmt die Systemsprache über `sys-locale` (nicht `LANG`); die Web-Seite
  fällt ohne Backend auf `navigator.language` zurück statt fest auf Deutsch.
- Layout: Spalte „Feinjustierung“ im Regelwerk war ein `display:flex`-`<td>` und versetzte die
  Zeilenlinien; lange Fehler-Toasts brechen jetzt innerhalb des Fensters um. Standard-Dateinamen
  der Dialoge (Regelwerk, Schlüsseldatei) folgen der Sprache.

**Geprüft ohne Befund**
- L1: kein `window.confirm/alert/prompt` im Code; Bestätigungen sind eigene Modals (Promise).
  Die Mock-IPC bildet das Verhalten von plugin-dialog 2.7 nach (`window.confirm` → Promise).
- L7: Die Oberfläche rendert Dokumentinhalt nur als React-Text (kein `dangerouslySetInnerHTML`),
  CSP `default-src 'self'`; HTML-Export maskiert Inhalte.
- L10: Bericht-Zeit `chrono::Local` mit Offset, Anzeige in Ortszeit; Datumsverschiebung rechnet in
  Kalendertagen (`NaiveDate`, keine Sommerzeit-Effekte). Neu geschriebene ZIP-Teile (Export)
  tragen das DOS-Datum 1980-01-01 — keine Zeitzonen-Frage, und es verrät nichts.
- L12: UI-Szenario sucht in der englischen Oberfläche nach deutschen Wörtern (und umgekehrt) — ohne Treffer.

**Ausgeführt**
| Befehl | Ergebnis |
| --- | --- |
| `cargo test --workspace` | 48 grün (Kern 45, App 3) |
| `pnpm test` (`node --test`, neu) | 4 grün — Fehler-Übersetzung (inkl. Abgleich aller Rust-Fehlertexte), Kürzel, Startsprache |
| `pnpm build` | grün |
| `cargo check --target x86_64-pc-windows-msvc --workspace --tests` (vom Mac, ohne Linken) | grün |
| `cargo clippy --workspace --tests` | nur vorbestehende Stilhinweise |
| `cargo audit` | keine neuen Befunde; zip 2.4.2/4.6.1 RUSTSEC-2026-0367…0369 nur `extract()` (nicht genutzt, Fix erst in zip 9 = Major), sonst „unmaintained“-Hinweise des Linux-/Build-Unterbaus |
| UI-Szenario headless Chromium, Mock-IPC, 960×620 | 13 Schritte × 4 (DE/EN × dunkel/hell, je macOS- und Windows-User-Agent) = 52 grün: Datei öffnen, Kategoriewechsel mit Vorschau, Rückwechsel, Speichern, Speicherfehler (gesperrte Datei) übersetzt, Regelwerk-Tabelle, Muster-Entfernen-Abfrage, Zurücksetzen (Abbrechen/OK), neue Saat mit Abfrage, Einstellungen alle Reiter, Update-Fehler, Wiederherstellen-Fehler übersetzt, Handbuch-Kürzel; kein Überlauf, keine fremdsprachigen Texte |

**Nur auf echter Hardware prüfbar** (zusätzlich zur manuellen Liste oben)
- Windows: Datei während des Speicherns von Virenscanner/OneDrive gesperrt → Speichern gelingt nach
  kurzer Pause; dauerhaft in Excel geöffnet → Meldung „Datei wird von einem anderen Programm
  verwendet …“ (DE) bzw. „The file is in use by another program …“ (EN), keine `.anonym-tmp`-Reste.
- Windows: CSV-Export in Excel per Doppelklick öffnen → Umlaute korrekt; Regelwerk im Editor
  speichern (UTF-8 mit BOM) und laden.
- Windows: Einstellungen → App zeigt „Strg+,“; macOS „⌘+,“; Strg/⌘+, öffnet die Einstellungen.
- Zielordner „D:“ in den Einstellungen → Ausgabe landet in `D:\`.
- Update-Installation mit echtem Release (1.0.0 → 1.0.1) und ein absichtlich fehlschlagender
  Download → Fehlermeldung in der UI-Sprache.

## Runde 2 – Verifikation (unabhängige Zweitprüfung, 2026-10-20)

Geprüft: Diff `a49da59..2364fe0` mit Blick auf Plattform-Regressionen und neue Async-/Race-Fälle,
alle Tests/Typprüfungen neu ausgeführt, UI-Szenario erneut (plus ein neuer Schritt), L1–L13 an
Stellen nachgesehen, die Runde 2 nicht genannt hat.

**Bestätigt (gelesen und ausgeführt)**
- Kategorie-Vorschau: `preview_replacements` und `apply_file` gehen durch dasselbe `resolve`; der
  Test `category_change_preview_matches_output` läuft, das UI-Szenario zeigt den neuen Ersatz und
  den Rückwechsel.
- Fehler-Übersetzung: zusätzlich alle deutschsprachigen String-Literale außerhalb der Tests (nicht nur
  die in `Err/ok_or/map_err`-Zeilen) gegen `errorText('en', …)` geprüft — offen bleiben nur
  Erkennungsmuster, Ersatz-Dateistämme und Platzhalter, keine Fehlertexte.
- `rename_retry`, eindeutige Temp-Namen, BOM in CSV/TSV, BOM-tolerante JSON-Importe, async-Kommandos,
  Laufnummern bei Analyse/Wiederherstellen, „Alle speichern“ nach Entfernen, Bestätigungen.
- L1 (kein `window.confirm`), L3 (sys-locale + `navigator.language`), L7, L10, L13 (Endpoint/Pubkey,
  Version 1.0.0 in `tauri.conf.json`, `package.json`, beiden `Cargo.toml`) nachgesehen.
- zip 4.6.1 kommt nur über `tauri-plugin-updater` (signierte Update-Pakete), anonym ruft nirgends
  `extract()` auf — die Advisories bleiben ohne Wirkung.

**Behoben in dieser Verifikation**
- Kategorie-Vorschau rechnete zu oft: Der Effekt lief bei jeder Änderung an Entscheidungen oder an
  der Dateiliste (auch Status „analysiert“/„gespeichert“) neu und schickte für jede Datei mit
  Kategoriewechsel eine vollständige Neuanalyse ins Backend — beim Ablehnen einer beliebigen
  anderen Fundstelle ebenso wie bei „Alle speichern“ (n Dateien → n² Analysen). Jetzt rechnet eine
  Datei nur neu, wenn sich Regelwerk, Analyse, die Kategorie-Entscheidungen oder (nach einem
  Speichern) der Pseudonym-Speicher geändert haben; die Laufnummer gilt je Datei. UI-Szenario,
  neuer Schritt `previewCache`: Ablehnen + Annehmen einer anderen Fundstelle → vorher 2 zusätzliche
  Backend-Läufe, jetzt 0.
- Rückfrage-/Eingabe-Promise: Eine zweite Rückfrage, solange die erste offen ist (z. B. Doppelklick
  auf den Papierkorb), überschrieb deren Auflösung — die erste wartete für immer. Sie gilt jetzt als
  abgebrochen.
- `settings.json` wurde ohne BOM-Toleranz und ohne Bereinigung geladen: Mit dem Windows-Editor
  gespeichert (BOM) fielen alle Einstellungen still auf Standard zurück, und ein in einem früheren
  Testbuild gespeicherter Zielordner „D:“ blieb bis zum nächsten Speichern „D:“. Laden übergeht
  jetzt das BOM und bereinigt wie beim Speichern (Test `load_accepts_bom_and_sanitizes`).
- Handbuch/README: keine Änderung nötig (interne Korrekturen ohne neues sichtbares Verhalten).

**Ausgeführt**
| Befehl | Ergebnis |
| --- | --- |
| `cargo test --workspace` | 49 grün (Kern 45, App 4) |
| `pnpm test` | 4 grün |
| `pnpm build` | grün |
| `cargo check --target x86_64-pc-windows-msvc --workspace --tests` (vom Mac, ohne Linken, nach erzwungenem Neu-Check) | grün, keine Warnungen |
| `cargo clippy --workspace --tests` | nur die vorbestehenden Stilhinweise |
| `cargo audit` | unverändert (zip `extract()`-Advisories, „unmaintained“-Hinweise) |
| UI-Szenario headless Chromium, Mock-IPC (plugin-dialog 2.7), 960×620 | 14 Schritte × 4 Konfigurationen = 56 grün, kein Überlauf, keine fremdsprachigen Texte, keine Seitenfehler |

**Bekannt, nicht geändert**
- Saat von Hand eintippen leert einen gefüllten Speicher beim nächsten Speichern ohne Rückfrage
  (dokumentiert); „Speicher löschen“ stößt keine Neuanalyse an — angezeigte Pseudonyme stammen bis
  zur nächsten Analyse noch aus dem alten Speicher (geschrieben wird korrekt neu).
- `rename_retry` wiederholt unter Windows auch bei „Zugriff verweigert“ (5) — bei einer dauerhaft
  schreibgeschützten Zieldatei oder einem gleichnamigen Ordner kommt die Meldung erst nach ~2 s.

**Nur auf echter Hardware prüfbar**: unverändert die Listen oben (Runde 2 und Manuelle Testliste),
zusätzlich: `settings.json` im Windows-Editor speichern → Einstellungen bleiben erhalten.

## Runde 3 (2026-10-20)

Basis `053d72f`. Auftrag: jede Aussage aus Handbuch (DE/EN), README und den Texten der Oberfläche
gegen den Code prüfen, Abläufe Ende-zu-Ende durchspielen, Reste aus Runde 2 schließen.
Vorgehen: Aussagen-Liste aus `Help.tsx` (Tutorial + 11 Abschnitte, DE und EN getrennt gelesen),
README und `i18n.ts` (Hinweise, Rückfragen); jede gegen die Stelle im Code, wo möglich per
CLI-Lauf (`anonym-cli` mit Prüftexten, Pfad mit Leerzeichen und Umlaut) oder UI-Szenario.

**Falsche Aussagen — Code behoben**
- „Telefon: Vorwahl bleibt“ (Handbuch, README): stimmte nur national. `+49 160 95929195` (auch die
  Nummer auf lan-solo.com) wurde **gar nicht erkannt** — das Muster verlangte eine Vorwahl mit 0.
  Bei `+49 (0)221 47110` und `0049 221 4711` wurde die Ortsvorwahl ersetzt. Muster und Ersatz
  korrigiert; Tests `international_phone_numbers`, `numbers_keep_checksums` (erweitert).
  Gegenprobe ohne neue Fehltreffer: `+5 EUR`, `+2.1`, `+1500`, ISBN.
- „IPv4/IPv6“ (Handbuch): verkürzte IPv6-Adressen mit `::` wurden nicht (`2001:db8::8a2e:370:7334`)
  oder nur ab der Mitte erkannt (`fe80::1ff:…` → nur `1ff:fe23:4567:890a`, `fe80::` blieb stehen).
  Test `ipv6_full_and_compressed` (inkl. „12:30“/„12::30“ ohne Treffer).
- Verschlüsselung der Recover-Datei („wer das Passwort verliert, kann nicht mehr zurückübersetzen“):
  Der Bericht daneben stand weiter im Klartext mit allen Originalwerten — die Verschlüsselung schützte
  praktisch nichts. Bei verschlüsselter Recover-Datei bleibt `original` in der Berichtsdatei jetzt leer
  (Ersatz, Anzahl, Kategorien bleiben); der Reiter „Bericht“ zeigt die Werte in der Sitzung weiter.
  Test `encrypted_recover_keeps_originals_out_of_report_file`.
- „Esc — Dialog schließen“: Stand der Fokus in einem Eingabefeld (Passwortabfrage hat Autofokus,
  Einstellungen), passierte nichts. Eine Rückfrage über den Einstellungen („Speicher löschen“) schloss
  mit Esc beide und verwarf ungespeicherte Einstellungen. Jetzt schließt Esc immer genau den obersten Dialog.
- Tastenkürzel liefen hinter dem offenen Handbuch weiter (O öffnete den Dateidialog, R den
  Wiederherstellen-Dialog). Jetzt gesperrt, solange Handbuch/Tutorial offen sind; Groß-/Kleinschreibung
  egal (Feststelltaste).

**Falsche Aussagen — Text korrigiert (DE + EN)**
- „Speicher löschen macht aus Pseudonymisierung Anonymisierung“ (Handbuch, Einstellungen): Recover-
  Dateien und Berichte (Standard: an) enthalten die Originale weiter. Jetzt: anonymisiert erst, wenn
  auch diese gelöscht sind.
- Rückfrage „Speicher löschen? Danach lassen sich bisherige Ausgaben nicht mehr zurückübersetzen“:
  falsch — die Rückübersetzung läuft nur über Recover-Dateien, nicht über den Speicher. Neuer Text.
- Tutorial „Alte Exporte (Windows-1252, ISO-8859) werden erkannt“: es gibt keine Erkennung, nur die
  eingestellte Rückfall-Codepage. „Prüft nur die Textknoten“: XLSX-Zahlenzellen typisierter Spalten
  werden auch ersetzt, Metadaten und Vorschaubild entfernt.
- „Existiert die Ausgabe schon, fragt anonym“: nur beim direkten Speichern (abschaltbar); im
  Dateidialog („Speichern unter“, „Exportieren“) fragt das System.
- Grenzen: Kontext-Treffer haben 70 %, nicht 65 % (`best_conf.max(70)`).

**Reste aus Runde 2 — behoben**
- Saat von Hand: wird erst beim Verlassen des Felds oder mit Enter übernommen (vorher jeder
  Tastendruck nach 600 ms) und fragt bei gefülltem Pseudonym-Speicher wie „Neu“; Abbrechen oder
  ein leeres Feld stellt die bisherige Saat wieder her, Esc im Feld verwirft die Eingabe.
- „Speicher löschen“ analysiert offene Dateien neu (und rechnet Kategorie-Vorschauen neu) —
  angezeigte Pseudonyme stammen nicht mehr aus dem gelöschten Speicher.

**Bestätigt ohne Befund** (Code gelesen, per CLI ausgeführt): Wörterbuch-Größen (625 Vornamen,
477 Nachnamen), Prioritäten-Reihenfolge, Datumsformate (14.03.2024, 2024-03-14, 3/14/2024,
14. März 2024, March 14, 2024) und Verschiebung ±1…45 Tage, Genus und Familien-Nachname, E-Mail aus
Pseudonym bzw. Name+Zahl, IBAN/Karte/Steuer-ID/SV-Nr. mit gültiger Prüfziffer und den genannten
festen Teilen, ungültige 11-stellige Zahl ≠ Steuer-ID, Kennzeichen, PLZ (erste zwei Ziffern),
Kunden-/Aktenzeichen, mehrdeutige Vornamen („Frank“, „Mai“) allein ohne Treffer, CSV-Kopfzeile
typisiert unbekannte Nachnamen, Ausnahmeliste ohne Groß/Klein, Strategien (Zähler je Original,
Länge erhalten, letzte n sichtbar), Codepage-Liste, Dateinamen von Bericht/Recover/Wiederherstellung,
Endungsliste Frontend = Kern, Kürzel-Liste, „+ Datei“, Fundstellen-Zahl in der Seitenleiste,
„Alle speichern“ nur masked.

**Ende-zu-Ende-Abläufe** (UI-Szenario, headless Chromium, Mock-IPC mit plugin-dialog-2.7-Verhalten,
je DE/EN × dunkel/hell × macOS/Windows-User-Agent; die Mock-IPC schreibt Regelwerk/Einstellungen
jetzt in eine „Festplatte“ (sessionStorage), die einen Neustart übersteht, und verlangt wie
`apply_file` ein Passwort bei verschlüsselter Recover-Datei)
1. Erststart → Datei öffnen → Fundstellen → Kategoriewechsel → Vorschau → Speichern → Bericht (Bestand).
2. Abgebrochener Dateidialog bei Öffnen, „Speichern unter“ und „Exportieren“: keine Analyse, kein
   Schreiben, kein Fehler-Toast (`cancelDialog`).
3. Speichern bei vollem Datenträger, schreibgeschütztem Ordner (Zugriff verweigert), schreibgeschütztem
   Laufwerk, gesperrter Datei (Windows-Konfiguration): Meldung in der UI-Sprache (`diskErrors`, `save`).
   Zusätzlich echt auf macOS: `write_bytes` in einen Ordner mit Rechten 555 → „Zugriff verweigert“,
   keine Temp-Datei (Test `read_only_folder_gives_stable_error_and_no_temp`).
4. Saat tippen: kein Schreiben während des Tippens, Rückfrage beim Verlassen, Abbrechen stellt
   zurück, OK schreibt, leer stellt zurück (`seedTyping`).
5. „Speicher löschen“: Esc auf der Rückfrage lässt die Einstellungen offen, OK leert und analysiert
   neu (`storeClear`).
6. Verschlüsselung an → Speichern → Passwortabfrage, Esc aus dem Feld bricht ab (kein Schreiben);
   abweichende Wiederholung sperrt „Weiter“, gleiche schreibt mit Passwort (`escPassword`).
7. Handbuch offen: R/O/3 ohne Wirkung, Esc schließt, danach „R“ (groß) öffnet Wiederherstellen (`helpKeys`).
8. Neustart mitten in der Arbeit: Regelwerk-Name geändert, Seite neu geladen → Name, Saat, Sprache,
   Thema bleiben, Tutorial erscheint nicht erneut (`restartPrep`/`restartCheck`).
9. Regelwerk (Muster entfernen, Zurücksetzen), Einstellungen alle Reiter, Update-Fehler,
   Wiederherstellen-Fehler, Handbuch-Kürzel (Bestand).
Ergebnis: 22 Schritte × 4 = 88 grün, kein Überlauf bei 960×620, keine fremdsprachigen Texte, keine
Seitenfehler. Gegenprobe: dieselben neuen Schritte gegen den Stand `053d72f` → `seedTyping`,
`storeClear`, `escPassword`, `helpKeys`, `restartCheck` rot (`helpKeys` dort nur über „R“ groß;
 die Sperre hinter dem Handbuch ist in dieser Gegenprobe nicht sauber getrennt nachgewiesen, weil der
 vorige Schritt einen Dialog offen ließ).

**Ausgeführt**
| Befehl | Ergebnis |
| --- | --- |
| `cargo test --workspace` | 53 grün (Kern 48, App 5) |
| `pnpm test` | 4 grün |
| `pnpm build` | grün |
| `cargo check --target x86_64-pc-windows-msvc --workspace --tests` (vom Mac, ohne Linken) | grün |
| `cargo clippy --workspace --tests` | nur vorbestehende Stilhinweise (keiner in geänderten Zeilen) |
| `cargo audit` | keine Schwachstelle im Code von anonym; zip-`extract()`-Advisories wie in Runde 2; neu nur „unmaintained“ für `zvariant`/`zvariant_derive` (RUSTSEC-2026-0370…0372, Linux-D-Bus-Unterbau) |
| UI-Szenario (s. o.) | 88/88 grün |

**Bekannt, nicht geändert**
- `apply_file` schreibt Ausgabe → Speicher → Bericht → Recover-Datei nacheinander. Scheitert ein
  späterer Schritt (z. B. Datenträger läuft genau dann voll), liegt die Ausgabe schon da, die
  Recover-Datei fehlt; die Meldung erscheint, erneutes Speichern behebt es. Keine Transaktion über
  mehrere Dateien.
- Ein geladenes Regelwerk oder „Auf Standard zurücksetzen“ mit anderer Saat leert den Speicher beim
  nächsten Speichern ohne eigene Rückfrage (Handbuch nennt es; „Zurücksetzen“ fragt ohnehin).
- Export in ein anderes Format schreibt die Recover-Datei `<name>.anonym.recover.json` — bei
  gleichem Stamm dieselbe wie beim Speichern im Ursprungsformat (Inhalt gleich, solange Regelwerk
  und Entscheidungen gleich sind).
- Ein englischer Straßenname wird in der Namenswelt „deutsch“ zu „32 Brunnenstraße“ (Hausnummer vorn).

**Nur auf echter Hardware prüfbar** (zusätzlich zu den Listen oben)
- macOS und Windows: Esc in der Passwortabfrage und in den Einstellungen; Esc auf einer Rückfrage
  über den Einstellungen schließt nur die Rückfrage.
- Windows: schreibgeschützter Ordner/Datei (Eigenschaften → Schreibgeschützt) und volles Laufwerk
  (USB-Stick) → übersetzte Meldung, keine `.anonym-tmp`-Reste. Der Unix-Test deckt Windows nicht ab.
- Native Speichern-Dialoge fragen beim Überschreiben selbst (macOS NSSavePanel, Windows IFileSaveDialog) —
  einmal je Plattform bestätigen.
- Verschlüsselte Recover-Datei an → `<name>.anonym.report.json` im Editor öffnen: Feld `original` leer.
