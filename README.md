# anonym.

Anonymisieren ohne KI — Wörterbücher, Muster, Prüfziffern. Lokal, ohne Konto, ohne Telemetrie.

- **Erkennung:** Namen (über 600 Vornamen mit Genus, über 450 Nachnamen, Orte), Adressen, PLZ,
  Datumsangaben, E-Mail, Telefon, IBAN, Kreditkarte, Steuer-ID/USt-IdNr., Sozialversicherungsnummer,
  Kennzeichen, IP-Adressen, Kunden-/Aktenzeichen — per Wörterbuch, Muster, Prüfziffer (Mod 97, Luhn,
  Mod 11,10), Kontext (Anrede, Beschriftung) und Tabellenspalten (Kopfzeile, feste Spaltenbreiten).
- **Ersetzung:** stimmig statt geschwärzt — anderer Name mit gleichem Genus, Familien behalten den
  Nachnamen, IBAN mit gültiger Prüfziffer und gleicher Bank, Telefon mit gleicher Vorwahl, Datum um
  denselben Betrag verschoben. Alternativ Platzhalter `[PERSON 3]`, Schwärzen, Maskieren, Löschen.
- **Deterministisch:** gleiche Saat + gleiche Datei = gleiche Ausgabe. Kein Zufall, keine KI.
- **Formate:** Text/Markdown/Logs/JSON/XML/HTML/EML/SRT, CSV/TSV (Spalten-Typisierung), DOCX, XLSX,
  ODT/ODS — Ausgabe im selben Format, alte Codepages (Windows-1252, ISO-8859) bleiben erhalten.
- **Vorschau & Bericht:** jede Fundstelle vor dem Schreiben prüfen, annehmen, ablehnen, Ersatz
  ändern, Ausnahmen setzen; Bericht als JSON neben der Ausgabe.
- **Free** bleibt kostenlos; **masked** (12 €/Jahr, im Vorabzugang frei) bringt Feinjustierung bis ins
  Zeichen, Pseudonym-Speicher über Dateien (Schlüsseldatei), eigene Muster und Wortlisten, weitere
  Codepages und „Alle speichern“.

Status: Version 0.1 (Beta) — Website: https://lan-solo.com/de/tools/anonym/

## Entwicklung

```sh
pnpm install
pnpm tauri dev
cargo test --workspace
```

Der Rust-Kern (`core/`, Crate `anonym-core`) ist Tauri-frei und testbar: Datenmodell,
Wörterbücher (`core/data/`), Erkennung, Prüfziffern, Pseudonyme, Datums-Logik, Formate.
Die Kommandos in `src-tauri/src/commands.rs` folgen dem Vertrag in `src/api.ts`.

Eigene Namenswelten: `core/data/worlds/<id>.txt` mit den Abschnitten `[f]`, `[m]`, `[last]`,
`[street]` — ein Name je Zeile.

## Release-Build (lokal)

```sh
TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/anonym-updater.key)" \
TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" \
pnpm tauri build --bundles app,dmg
```

Windows- und Linux-Installer baut die GitHub-Action (`.github/workflows/build.yml`) — per Tag `v*`
als Release mit `latest.json` für den In-App-Updater, per `workflow_dispatch` als Test-Artefakte.
