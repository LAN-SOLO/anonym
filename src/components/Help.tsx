import { useEffect, useState } from 'react';
import { Lang } from '../i18n';
import { modKey } from '../platform';

// Selbstständiges Hilfe-System: schwebender ?-Button, First-Run-Tutorial
// und durchsuchbares Handbuch. Inhalte liegen bewusst hier, nicht in i18n.ts.

const SEEN_KEY = 'anonym.tutorialSeen';

interface Step {
  title: string;
  body: string[];
}

interface Section {
  id: string;
  title: string;
  body: string[];
}

interface Content {
  labels: {
    fab: string;
    tutorial: string;
    manual: string;
    search: string;
    next: string;
    back: string;
    skip: string;
    done: string;
    stepOf: (n: number, total: number) => string;
    noResults: string;
  };
  tutorial: Step[];
  sections: Section[];
}

const de: Content = {
  labels: {
    fab: 'Hilfe & Handbuch',
    tutorial: 'Tutorial',
    manual: 'Handbuch',
    search: 'Handbuch durchsuchen …',
    next: 'Weiter',
    back: 'Zurück',
    skip: 'Überspringen',
    done: 'Los geht’s',
    stepOf: (n, total) => `Schritt ${n} von ${total}`,
    noResults: 'Keine Treffer',
  },
  tutorial: [
    {
      title: 'Willkommen bei anonym.',
      body: [
        'anonym findet Namen, Adressen, Kontonummern, Geburtsdaten und mehr in Texten und Tabellen und ersetzt sie durch stimmige Platzhalter — ohne Satzbau, Spalten oder Formatierung anzurühren.',
        'Es arbeitet ohne KI: Wörterbücher mit Vor- und Nachnamen und Orten, Muster für IBAN, Telefon, E-Mail, Kennzeichen und Aktenzeichen, Prüfziffern, die einen Treffer bestätigen. Dieselbe Datei ergibt dieselbe Ausgabe — heute, morgen und auf jedem Rechner.',
        'Alles bleibt auf Ihrem Rechner: kein Konto, keine Cloud, keine Telemetrie.',
        'Dieses Tutorial dauert zwei Minuten. Sie finden es jederzeit wieder über den ?-Knopf unten rechts.',
      ],
    },
    {
      title: 'Datei öffnen',
      body: [
        'Ziehen Sie eine oder mehrere Dateien ins Fenster oder klicken Sie auf „+ Datei“. anonym liest Text, Markdown, Logs, CSV/TSV, JSON, XML, HTML, E-Mails (EML), Untertitel, DOCX, XLSX und ODT/ODS.',
        '• Jede Datei wird sofort geprüft; die Zahl neben dem Namen ist die Anzahl der Fundstellen.',
        '• Alte Exporte ohne Unicode werden als Windows-1252 gelesen (andere Codepages: Einstellungen → Dateien) und in derselben Kodierung zurückgeschrieben.',
        '• Word- und Excel-Dateien öffnet anonym als Archiv und prüft die Textknoten (in Excel-Spalten mit typisierter Kopfzeile auch Zahlenzellen) — Formatierung, Formeln und Bilder bleiben unangetastet; nur Metadaten und das Vorschaubild der ersten Seite werden entfernt.',
      ],
    },
    {
      title: 'Fundstellen prüfen',
      body: [
        'Der Reiter „Fundstellen“ zeigt jede Stelle mit Kategorie, Original, Ersatzvorschlag und Quelle: Wörterbuch, Muster, Prüfziffer, Kontext (Anrede, Beschriftung wie „Kd-Nr.“), Spalte oder eigene Regel.',
        '• ✓ nimmt an, ✗ lehnt ab. Über die Kategorie-Chips filtern Sie und nehmen ganze Kategorien auf einmal an oder ab.',
        '• Klick auf den Ersatz macht ihn editierbar — Ihr Text ersetzt den Vorschlag.',
        '• Der Schild-Knopf setzt das Original auf die Ausnahmeliste: Firmenname, Produktname, Behörde — bleibt künftig in allen Dateien stehen.',
        '• Die Kategorie lässt sich pro Fundstelle umstellen, etwa wenn eine Nummer als Telefon erkannt wurde, aber ein Aktenzeichen ist. Der Ersatz für die neue Kategorie erscheint sofort in Tabelle und Vorschau.',
      ],
    },
    {
      title: 'Vorschau & Speichern',
      body: [
        'Der Reiter „Vorschau“ zeigt Original und Ausgabe nebeneinander, farbig nach Kategorie. Ein Klick auf eine Markierung nimmt die Fundstelle an oder lehnt sie ab.',
        '• „Anonymisiert speichern“ schreibt die Datei im selben Format als <name>.anonym.<endung> neben die Quelle (Zielordner und Namenszusatz: Einstellungen → Dateien).',
        '• „Exportieren als“ schreibt dieselbe anonymisierte Datei in ein anderes Format: TXT, Markdown, HTML, CSV, TSV, JSON, Excel (XLSX), Word (DOCX), ODT, ODS, PDF oder RTF — Name und Ort wählen Sie im Dateidialog. „Speichern unter …“ bietet dieselben Formate; die Endung entscheidet. Existiert die Ausgabe beim direkten Speichern schon, fragt anonym: Überschreiben oder anderer Name (abschaltbar in Einstellungen → Dateien); im Dateidialog fragt das System.',
        '• Daneben entsteht ein Bericht als JSON: was wurde wodurch ersetzt, wie oft, mit welcher Datumsverschiebung.',
        '• Die Quelle wird nie überschrieben.',
      ],
    },
    {
      title: 'Regelwerk',
      body: [
        'Im Reiter „Regelwerk“ steuern Sie, was passiert: pro Kategorie erkennen ja/nein und eine Strategie — Pseudonym (stimmiger Ersatz), Platzhalter wie [PERSON 3], Schwärzen, Maskieren oder Löschen.',
        '• Die Saat bestimmt alle Pseudonyme. Gleiche Saat + gleiche Datei = gleiche Ausgabe. Neue Saat = neue Namen. anonym erzeugt sie zufällig — behandeln Sie sie wie einen Schlüssel.',
        '• Namenswelten: deutsch, englisch, französisch, spanisch, türkisch, japanisch — oder Fantasy, Science-Fiction, Mittelalter für Spielwelten.',
        '• Ein Regelwerk ist eine JSON-Datei: exportieren, versionieren, im Team teilen, pro Empfänger eines anlegen.',
        '• masked (Vorabzugang oben rechts) schaltet Feinjustierung bis ins Zeichen, eigene Muster und Wortlisten, den Pseudonym-Speicher über Dateien und „Alle speichern“ frei.',
      ],
    },
  ],
  sections: [
    {
      id: 'principle',
      title: 'Ohne KI. Mit System.',
      body: [
        'anonym rät nicht, was ein Name sein könnte — es prüft. Drei Quellen tragen die Erkennung:',
        '• Wörterbücher: über 600 Vornamen mit Genus (deutsch, englisch, international), über 450 Nachnamen, Orte. Ein Vorname allein zählt, wenn er eindeutig ist („Anna“); mehrdeutige („Frank“, „Mark“, „Christian“) nur mit Nachname oder Anrede.',
        '• Muster: E-Mail, IBAN, Kreditkarte, Telefon, Datum (numerisch und ausgeschrieben, deutsch und englisch), IPv4/IPv6, deutsche Kennzeichen, Steuer-ID, USt-IdNr., Sozialversicherungsnummer, Straße + Hausnummer, PLZ + Ort.',
        '• Prüfziffern: IBAN (Mod 97), Kreditkarte (Luhn), Steuer-ID (Mod 11,10), Sozialversicherungsnummer. Ein Treffer mit gültiger Prüfziffer bekommt Konfidenz 95–99 %; eine 11-stellige Zahl ohne gültige Prüfziffer wird nicht als Steuer-ID gemeldet.',
        'Dazu Kontext: Anreden (Frau, Herr, Mr, Mme …) mit Titeln (Dr., Prof.) und Adelspartikeln (von, van, de), Beschriftungen wie „Kd-Nr.“, „Aktenzeichen“, „Customer ID“ — und Tabellenspalten, deren Kopfzeile eine Kategorie nennt.',
        'Bei Überlappungen gewinnt die Kategorie mit der höheren Priorität: IBAN vor Kreditkarte vor E-Mail vor Steuer-ID vor Datum vor Adresse vor Telefon vor Person. Deshalb wird „02.07.1981“ ein Datum und keine Telefonnummer.',
      ],
    },
    {
      id: 'categories',
      title: 'Kategorien',
      body: [
        '• Person: Vor- und Nachnamen. Pseudonym behält Genus (aus „Frau Berger“ wird nicht „Herr Vogt“) und Familien: derselbe Nachname wird in der ganzen Datei gleich ersetzt.',
        '• Adresse: Straße + Hausnummer (deutsch: „Musterstraße 12a“, „Am Hang 3“; englisch: „12 Baker Street“). Pseudonym: andere Straße aus der Namenswelt, neue Hausnummer. Der Ort bleibt.',
        '• PLZ: fünfstellig vor einem bekannten Ort oder mit „D-“. Pseudonym behält die ersten zwei Ziffern (Gebiet).',
        '• Datum: 14.03.2024, 2024-03-14, 3/14/2024, 14. März 2024, March 14, 2024. Pseudonym = Verschiebung um denselben Betrag für alle Datumsangaben; Abstände und Reihenfolge bleiben.',
        '• E-Mail: lokaler Teil aus den Pseudonymen der Person (anna.berger → lena.vogt), Domain bleibt. Unbekannte lokale Teile bekommen einen neuen Namen mit Zahl.',
        '• Telefon: national (0221 4711-0) und international (+49 160 …, +49 (0)221 …, 0049 221 …). Länder- und Ortsvorwahl bleiben, Rufnummer neu, Format erhalten; eine Durchwahl „-0“ bleibt.',
        '• IBAN: Land und Bankleitzahl bleiben, Kontonummer neu, Prüfziffer gültig, Gruppierung erhalten.',
        '• Kreditkarte: erste vier Ziffern bleiben, Rest neu, Luhn-Prüfziffer gültig.',
        '• Steuer-ID / USt-IdNr., Versicherungsnummer: neue Nummer mit gültiger Prüfziffer; Bereichsnummer bleibt.',
        '• Kennzeichen: Unterscheidungszeichen (Stadt) bleibt, Buchstaben und Ziffern neu.',
        '• IP-Adresse: IPv4 und IPv6 (auch verkürzt mit „::“). Netz (erste zwei Oktette bzw. Gruppen) bleibt, Host neu.',
        '• Kunden-/Aktenzeichen: Buchstaben und Trennzeichen bleiben, Ziffern neu in gleicher Länge — Kundennummer 10482 heißt überall dieselbe neue Nummer.',
      ],
    },
    {
      id: 'strategies',
      title: 'Strategien',
      body: [
        '• Pseudonym: stimmiger Ersatz, siehe Kategorien. Standard für alles.',
        '• Platzhalter: [PERSON 3] — Zähler je Original; dieselbe Person bekommt dieselbe Nummer. Vorlage mit {cat} und {n} anpassbar (masked).',
        '• Schwärzen: Balken █ in Originallänge. Zeichen und „Länge erhalten“ anpassbar (masked).',
        '• Maskieren: Buchstaben und Ziffern werden zu X, Trennzeichen bleiben; „letzte n sichtbar“ zeigt etwa die letzten vier Ziffern einer IBAN (masked).',
        '• Löschen: ersatzlos entfernen.',
        'Strategien gelten pro Kategorie im Regelwerk. Einzelne Fundstellen überschreiben Sie direkt in der Tabelle: Ersatz anklicken, eigenen Text eintragen.',
      ],
    },
    {
      id: 'formats',
      title: 'Formate',
      body: [
        '• Text: TXT, Markdown, Logs, JSON/JSONL, YAML, XML, HTML, SQL, INI/TOML, E-Mail (EML/MBOX), Untertitel (SRT/VTT), vCard, iCal, RTF, LaTeX. Die Datei wird als Ganzes geprüft; Zeilenenden und Kodierung bleiben.',
        '• CSV/TSV: Trennzeichen (; , Tab |) wird erkannt, Anführungszeichen bleiben. Spalten mit Kopfzeile wie Name, Vorname, E-Mail, Telefon, IBAN, Geb., PLZ, Kd-Nr. werden komplett typisiert — auch Nachnamen, die in keinem Wörterbuch stehen.',
        '• Feste Spaltenbreiten: Tabellen im Fließtext (Kopfzeile + Zeilen, durch zwei oder mehr Leerzeichen getrennt) werden ebenso typisiert.',
        '• DOCX: word/document.xml sowie Kopf-/Fußzeilen, Fuß-/Endnoten, Kommentare, gelöschter Text aus „Änderungen verfolgen“, Feldfunktionen (etwa HYPERLINK "mailto:…" in älteren Dokumenten und Serienbriefen) und externe Link-Ziele (z. B. mailto:anna@…). Jeder Formatierungslauf erscheint als eine Zeile der Vorschau. Ein Name, den Word über zwei Läufe verteilt hat (etwa nach einer Rechtschreibkorrektur), erscheint geteilt — Vorname und Nachname werden meist trotzdem einzeln erkannt.',
        '• XLSX: Textzellen (sharedStrings und Inline-Strings) und Kommentare aller Blätter sowie externe Link-Ziele. Spalten werden wie bei CSV über die Kopfzeile typisiert — dann werden auch Zahlenzellen ersetzt (Kundennummer als Zahl, Telefon, PLZ, Steuer-ID, Kartennummer) und Datumszellen als Excel-Seriennummer verschoben. Zahlenzellen ohne typisierte Kopfzeile bleiben unverändert.',
        '• ODT/ODS: alle Textknoten in content.xml und styles.xml (Kopf- und Fußzeilen) sowie externe Links (mailto:, https:).',
        '• Metadaten: Beim Speichern von DOCX, XLSX und ODF leert anonym Autor, „zuletzt geändert von“, Titel, Thema, Beschreibung, Stichwörter, Firma und Manager und ersetzt die Namen von Kommentar- und Überarbeitungs-Autoren durch „anonym“ — auch wenn sonst nichts ersetzt wurde. Das Vorschaubild der ersten Seite (LibreOffice legt es immer an, Word mit „Vorschaubild speichern“) zeigt das Original und wird entfernt.',
        '• Größe: Dateien bis 512 MB, je Archiv-Teil (entpackt) bis 256 MB, alle Textteile zusammen bis 1 GB — Schutz vor präparierten ZIP-Archiven.',
        '• Kodierungen: UTF-8 (mit und ohne BOM), UTF-16 mit BOM, sonst Windows-1252 als Rückfall; andere Codepages (ISO-8859-15, Windows-1250, Macintosh, KOI8-R) in Einstellungen → Dateien (masked).',
        '• Export: Jede anonymisierte Datei lässt sich zusätzlich als TXT, Markdown, HTML, CSV, TSV, JSON, XLSX, DOCX, ODT, ODS, PDF oder RTF schreiben. Tabellen (CSV, XLSX, ODS) werden in Textformaten zu Tabellen mit Spalten, in JSON zu einer Liste von Objekten je Zeile (Kopfzeile = Schlüssel); Texte werden in Tabellenformaten zu einer Zeile je Reihe. Alle Blätter einer Arbeitsmappe werden übernommen. PDF nutzt eine feste Schrift mit Windows-1252-Zeichensatz — Zeichen außerhalb (z. B. Kyrillisch, CJK) erscheinen als „?“.',
        'Alte Formate (DOC, XLS, WordPerfect, Lotus, dBase, EBCDIC …) als Eingabe, PDF-Textebene und Format-Beschreibungen per JSON folgen per Update.',
      ],
    },
    {
      id: 'rules',
      title: 'Regelwerk & Saat',
      body: [
        'Ein Regelwerk enthält Name, Saat, Namenswelt, Datumsverschiebung, die Einstellungen je Kategorie, Ausnahmeliste, eigene Muster und Wortlisten. Es liegt als rules.json im Datenordner und lässt sich exportieren und laden.',
        '• Saat: Zeichenkette, aus der alle Ersetzungen deterministisch abgeleitet werden. Gleiche Saat, gleiche Datei, gleiche Ausgabe — auch auf einem anderen Rechner. Andere Saat: alle Personen bekommen neue Pseudonyme.',
        '• Die Saat ist ein Schlüssel: Wer Saat und anonym kennt, kann Pseudonyme, Nummern und die Datumsverschiebung nachrechnen. Deshalb erzeugt anonym beim ersten Start eine zufällige Saat (96 Bit); „Neu“ neben dem Feld erzeugt eine weitere. Die frühere feste Voreinstellung „anonym“ wird automatisch ersetzt — außer ein gefüllter Pseudonym-Speicher hängt an ihr (dann weist das Regelwerk darauf hin). Exportierte Regelwerke enthalten die Saat: nur an Stellen weitergeben, die zurückübersetzen dürfen.',
        '• Datumsverschiebung: 0 = aus der Saat abgeleitet (±1 bis 45 Tage), sonst fester Wert in Tagen.',
        '• Ausnahmeliste: Phrasen, die immer stehen bleiben; Fundstellen darin werden verworfen. Groß-/Kleinschreibung spielt keine Rolle.',
        '• Eigene Muster (masked): reguläre Ausdrücke in Rust-Syntax; der ganze Treffer wird der gewählten Kategorie zugeordnet. Beispiel: PRJ-\\d{4} als Kunden-/Aktenzeichen.',
        '• Eigene Wortlisten (masked): Wörter oder Phrasen, die immer Treffer sind — Projektnamen, Codenamen. Für die Kategorie Person werden sie wie Nachnamen ersetzt.',
      ],
    },
    {
      id: 'store',
      title: 'Pseudonym-Speicher & Schlüsseldatei',
      body: [
        'Innerhalb einer Datei sind Pseudonyme immer konsistent. Über Dateien hinweg garantiert das der Pseudonym-Speicher (masked, Einstellungen → Pseudonym-Speicher): jede Ersetzung wird in pseudonyms.json gemerkt und bei der nächsten Datei wiederverwendet.',
        '• Damit bleiben Beziehungen zwischen Tabellen intakt: Kundennummer 10482 wird in allen Exporten dieselbe neue Nummer, Anna Berger überall dieselbe Lena Vogt.',
        '• Der Speicher ist zugleich die Schlüsseldatei. Wer sie hat, kann zurückübersetzen — exportieren Sie sie nur an Stellen, die das dürfen.',
        '• Rechtlich gilt: Solange eine Schlüsseldatei, Recover-Datei oder ein Bericht existiert, ist das Ergebnis pseudonymisiert, nicht anonymisiert. „Speicher löschen“ entfernt die Schlüsseldatei; anonymisiert ist das Ergebnis erst, wenn auch Recover-Dateien und Berichte gelöscht sind. Nach dem Löschen werden geöffnete Dateien neu analysiert — die angezeigten Ersatzwerte stammen dann nicht mehr aus dem alten Speicher.',
        '• Ändern Sie die Saat, passt der Speicher nicht mehr und wird beim nächsten Speichern geleert. „Neu“ und eine von Hand geänderte Saat (übernommen beim Verlassen des Felds oder mit Enter) fragen vorher, solange der Speicher Einträge hat; Abbrechen stellt die bisherige Saat wieder her. Ein geladenes Regelwerk und „Auf Standard zurücksetzen“ können ebenfalls eine andere Saat mitbringen — die Rückfrage zum Zurücksetzen nennt dann den Speicher, nach dem Laden erscheint ein Hinweis. Exportieren Sie die Schlüsseldatei vorher, wenn Sie sie brauchen.',
        '• „Schlüsseldatei laden …“ übernimmt Saat und Namenswelt der Datei ins aktive Regelwerk — sonst würde der geladene Speicher sofort als unpassend verworfen.',
      ],
    },
    {
      id: 'recover',
      title: 'Wiederherstellen (Recover-Datei)',
      body: [
        'Zu jeder Ausgabe schreibt anonym eine Recover-Datei <name>.anonym.recover.json (Einstellungen → Dateien). Sie enthält alle Ersetzungen dieser Datei: Original → Ersatz je Kategorie, dazu die Namensteile (Vorname, Nachname, Straße), die Datumsverschiebung und die Saat.',
        '• „Wiederherstellen …“ (Kopfzeile oder Taste R) übersetzt eine anonymisierte Datei zurück — auch wenn sie inzwischen weiterbearbeitet wurde: neue Zeilen, geänderte Absätze, andere Reihenfolge, anderes Format (z. B. die als XLSX exportierte CSV). Alles, was nicht aus der Anonymisierung stammt, bleibt unverändert.',
        '• Ablauf im Dialog: bearbeitete Datei wählen, Recover-Datei wählen (anonym schlägt die passende daneben vor), ggf. Passwort, Ausgabeformat wählen — wie die Datei oder eines der zwölf Exportformate (TXT, Markdown, HTML, CSV, TSV, JSON, XLSX, DOCX, ODT, ODS, PDF, RTF) — dann „Wiederherstellen …“ und Ziel bestätigen (Vorschlag <name>.recovered.<endung>). Das Ergebnis zeigt, wie viele Stellen zurückübersetzt wurden und wie viele Ersatzwerte nicht mehr vorkamen.',
        '• Wortweise: Ersatzwerte werden als ganze Wörter gesucht, längste zuerst, ohne Rücksicht auf Groß-/Kleinschreibung; die Schreibweise der Fundstelle bleibt (RADEMACHER → BERGER). Ein allein weiterverwendeter Nachname findet über die Namensteile zurück.',
        '• Nicht umkehrbar: Schwärzungen, Maskierungen und gelöschte Stellen — und Platzhalter, wenn derselbe Text für verschiedene Originale stand. Solche Ersatzwerte werden als „mehrdeutig“ übersprungen.',
        '• Passwort: In Einstellungen → Dateien lassen sich Recover-Dateien verschlüsseln (XChaCha20-Poly1305, Schlüssel per Argon2id). Das Passwort wird je Sitzung einmal abgefragt — zweimal einzugeben, gegen Tippfehler — und nie gespeichert. Ohne Passwort ist die Recover-Datei Klartext — behandeln Sie sie wie das Original. Ist die Verschlüsselung an, schreibt anonym die Originalwerte auch nicht in den Bericht daneben.',
        '• Rechtlich: Solange eine Recover-Datei existiert, ist die Ausgabe pseudonymisiert, nicht anonymisiert. Anonymisiert ist sie erst, wenn auch der Bericht (enthält ohne Verschlüsselung die Originalwerte) und der Pseudonym-Speicher gelöscht sind.',
      ],
    },
    {
      id: 'report',
      title: 'Bericht',
      body: [
        'Nach dem Speichern schreibt anonym <name>.anonym.report.json neben die Ausgabe (abschaltbar in Einstellungen → Dateien). Der Bericht enthält Quelle, Ausgabe, Format, Regelwerk, Namenswelt, Datumsverschiebung, Anzahl der Ersetzungen und die Liste „Original → Ersatz“ mit Häufigkeit.',
        'Der Reiter „Bericht“ zeigt dasselbe in der App. Ein PDF-Bericht folgt per Update.',
        'Achtung: Der Bericht enthält die Originalwerte. Er gehört in Ihre Akte, nicht zum Empfänger der anonymisierten Datei. Ausnahme: Sind Recover-Dateien verschlüsselt (Einstellungen → Dateien), bleibt in der Berichtsdatei die Spalte „Original“ leer — der Reiter „Bericht“ zeigt sie während der Sitzung weiter.',
      ],
    },
    {
      id: 'limits',
      title: 'Grenzen — ehrlich',
      body: [
        'Ohne KI gibt es kein Verstehen. anonym findet, was in Wörterbüchern und Mustern steht, und übersieht, was in keinem steht: den seltenen Nachnamen ohne Anrede, den Spitznamen, die Umschreibung („der Bäcker aus der Hauptstraße“), die eine Person trotzdem erkennbar macht.',
        '• Deshalb die Vorschau, deshalb der Blick vor dem Versand.',
        '• Nachnamen ohne Vorname oder Anrede werden im Fließtext nicht erkannt — in Tabellenspalten mit passender Kopfzeile schon. Helfen Sie mit eigenen Wortlisten nach.',
        '• Ein Vorname, der auch ein Wort ist („Mai“, „August“ als Monat), wird nur mit Nachname oder Anrede gezählt.',
        '• Excel-Zahlenzellen ohne typisierte Kopfzeile, Blattnamen, Kopf-/Fußzeilen von Excel-Blättern, Bilder, eingebettete Objekte und benutzerdefinierte Dokument-Eigenschaften (docProps/custom.xml) werden nicht geprüft.',
        '• Freitext hinter Namen („Anna Möbelhaus“) kann fälschlich als Nachname gelten — die Quelle „Kontext“ mit Konfidenz 70 % markiert solche Fälle; ein Klick lehnt ab.',
        '• Eine internationale Nummer, die nach der Ländervorwahl nur aus Dreiergruppen mit Punkten besteht („+49.221.471.100“), sieht aus wie eine Zahl mit Tausenderpunkten und wird nicht als Telefon gemeldet — ebenso mit Leerzeichen bei weniger als neun Ziffern. Ergänzen Sie bei Bedarf ein eigenes Muster.',
      ],
    },
    {
      id: 'news',
      title: 'Neu in 1.0',
      body: [
        '• Zufällige Saat je Installation statt der festen Voreinstellung — Pseudonyme lassen sich ohne Ihre Saat nicht nachrechnen. Knopf „Neu“ im Regelwerk.',
        '• DOCX/XLSX/ODF: Metadaten (Autor, Firma, Kommentar-Autoren) werden entfernt; gelöschter Text aus „Änderungen verfolgen“, Excel-Kommentare, ODT-Kopf-/Fußzeilen, Word-Feldfunktionen und mailto:-Links werden mitgeprüft; Vorschaubilder der ersten Seite fallen weg.',
        '• Entscheidungen (angenommen, abgelehnt, eigener Ersatz) bleiben an der richtigen Fundstelle, wenn eine neue Analyse die Liste verändert (z. B. nach einer Ausnahme).',
        '• Passwort für Recover-Dateien wird zweimal abgefragt; ohne Passwort wird nichts geschrieben.',
        '• Ausgabe, Bericht und Recover-Datei können die Quelle nicht überschreiben — auch nicht über Groß-/Kleinschreibung, Symlinks oder „..“.',
        '• Hinweise zur Datei und Fehlermeldungen erscheinen in der Sprache der Oberfläche.',
        '• Kategorie einer Fundstelle umgestellt: Tabelle und Vorschau zeigen sofort den neuen Ersatz — genau den, der beim Speichern geschrieben wird.',
        '• Sicherheitsabfragen vor „Auf Standard zurücksetzen“, vor einer neuen Saat bei gefülltem Pseudonym-Speicher und vor dem Entfernen eigener Muster und Wortlisten.',
        '• Windows: CSV/TSV-Exporte tragen ein UTF-8-BOM (Excel zeigt Umlaute richtig); Regelwerke, Schlüssel- und Recover-Dateien aus dem Windows-Editor (mit BOM) werden gelesen. Kurz gesperrte Dateien (Virenscanner, OneDrive) werden beim Speichern einige Male neu versucht.',
        '• Update-Fehler werden angezeigt statt still verworfen.',
        '• Internationale Telefonnummern (+49 160 …) und verkürzte IPv6-Adressen (2001:db8::…) werden erkannt; bei +49 (0)221 … und 0049 … bleibt die Vorwahl.',
        '• Saat von Hand ändern fragt bei gefülltem Pseudonym-Speicher nach; „Speicher löschen“ analysiert geöffnete Dateien neu.',
        '• Verschlüsselte Recover-Datei: der Bericht daneben enthält keine Originalwerte mehr.',
        '• „Alle speichern“ nennt bei fehlgeschlagenen Dateien den Grund, statt ihn mit der Erfolgsmeldung zu überdecken.',
        '• Esc schließt Dialoge auch aus einem Eingabefeld heraus und nur den obersten; bei offenem Handbuch lösen Tasten keine Aktionen dahinter aus.',
        '• Zahlen mit Vorzeichen und Tausendergruppen („+12.500.000“, „+1 250 000“) gelten nicht mehr als Telefonnummer; die USt-IdNr. bekommt eine gültige Prüfziffer.',
        `• ${modKey('de')} + , bei schon offenen Einstellungen oder über einem anderen Dialog öffnet nichts erneut — ungespeicherte Änderungen lassen sich weiter mit Esc verwerfen.`,
      ],
    },
    {
      id: 'shortcuts',
      title: 'Tastatur',
      body: ['• O — Datei öffnen', '• S — Anonymisiert speichern', '• R — Wiederherstellen', '• 1, 2, 3, 4 — Reiter Fundstellen, Vorschau, Regelwerk, Bericht', '• ? — Handbuch', `• ${modKey('de')} + , — Einstellungen`, '• Esc — Dialog schließen'],
    },
  ],
};

const en: Content = {
  labels: {
    fab: 'Help & manual',
    tutorial: 'Tutorial',
    manual: 'Manual',
    search: 'Search the manual …',
    next: 'Next',
    back: 'Back',
    skip: 'Skip',
    done: 'Let’s go',
    stepOf: (n, total) => `Step ${n} of ${total}`,
    noResults: 'No results',
  },
  tutorial: [
    {
      title: 'Welcome to anonym.',
      body: [
        'anonym finds names, addresses, account numbers, birth dates and more in texts and tables and replaces them with plausible substitutes — without touching sentence structure, columns or formatting.',
        'It works without AI: dictionaries of first names, surnames and places, patterns for IBAN, phone, email, licence plates and case numbers, checksums that confirm a hit. The same file gives the same output — today, tomorrow and on any machine.',
        'Everything stays on your computer: no account, no cloud, no telemetry.',
        'This tutorial takes two minutes. You can always reopen it with the ? button at the bottom right.',
      ],
    },
    {
      title: 'Open a file',
      body: [
        'Drop one or more files onto the window or click “+ File”. anonym reads text, Markdown, logs, CSV/TSV, JSON, XML, HTML, email (EML), subtitles, DOCX, XLSX and ODT/ODS.',
        '• Every file is analysed immediately; the number next to the name is the count of findings.',
        '• Legacy exports without Unicode are read as Windows-1252 (other code pages: Settings → Files) and written back in the same encoding.',
        '• Word and Excel files are opened as archives and their text nodes are checked (in Excel columns with a typed header numeric cells too) — formatting, formulas and images stay untouched; only metadata and the first-page preview image are removed.',
      ],
    },
    {
      title: 'Review findings',
      body: [
        'The “Findings” tab lists every hit with category, original, suggested replacement and source: dictionary, pattern, checksum, context (salutation, labels like “Customer no.”), column or custom rule.',
        '• ✓ accepts, ✗ rejects. The category chips filter the list and accept or reject whole categories at once.',
        '• Click the replacement to edit it — your text replaces the suggestion.',
        '• The shield button puts the original on the allowlist: company name, product, authority — stays untouched in every file from now on.',
        '• The category can be changed per finding, e.g. when a number was detected as a phone but is a case number. The replacement for the new category appears right away in table and preview.',
      ],
    },
    {
      title: 'Preview & save',
      body: [
        'The “Preview” tab shows original and output side by side, coloured by category. Clicking a highlight accepts or rejects that finding.',
        '• “Save anonymised” writes the file in the same format as <name>.anonym.<ext> next to the source (output folder and suffix: Settings → Files).',
        '• “Export as” writes the same anonymised file in another format: TXT, Markdown, HTML, CSV, TSV, JSON, Excel (XLSX), Word (DOCX), ODT, ODS, PDF or RTF — you choose name and location in the file dialog. “Save as …” offers the same formats; the extension decides. If the output already exists when saving directly, anonym asks: overwrite or another name (can be turned off under Settings → Files); in the file dialog the system asks.',
        '• A JSON report is written alongside: what was replaced by what, how often, with which date shift.',
        '• The source is never overwritten.',
      ],
    },
    {
      title: 'Rules',
      body: [
        'The “Rules” tab controls what happens: per category detect yes/no and a strategy — pseudonym (plausible substitute), placeholder like [PERSON 3], redact, mask or delete.',
        '• The seed drives all pseudonyms. Same seed + same file = same output. New seed = new names. anonym generates it at random — treat it like a key.',
        '• Name worlds: German, English, French, Spanish, Turkish, Japanese — or fantasy, science fiction and medieval for game worlds.',
        '• A rule set is a JSON file: export it, version it, share it with your team, keep one per recipient.',
        '• masked (early access, top right) unlocks fine-tuning down to the character, custom patterns and word lists, the pseudonym store across files and “Save all”.',
      ],
    },
  ],
  sections: [
    {
      id: 'principle',
      title: 'No AI. A system.',
      body: [
        'anonym does not guess what might be a name — it checks. Three sources drive detection:',
        '• Dictionaries: over 600 first names with gender (German, English, international), over 450 surnames, places. A first name on its own counts when it is unambiguous (“Anna”); ambiguous ones (“Frank”, “Mark”, “Christian”) only with a surname or salutation.',
        '• Patterns: email, IBAN, credit card, phone, dates (numeric and spelled out, German and English), IPv4/IPv6, German licence plates, tax ID, VAT ID, social insurance number, street + house number, postcode + town.',
        '• Checksums: IBAN (mod 97), credit card (Luhn), tax ID (mod 11,10), social insurance number. A hit with a valid checksum gets 95–99 % confidence; an 11-digit number without a valid checksum is not reported as a tax ID.',
        'Plus context: salutations (Frau, Herr, Mr, Mme …) with titles (Dr., Prof.) and particles (von, van, de), labels such as “Kd-Nr.”, “case no.”, “Customer ID” — and table columns whose header names a category.',
        'When hits overlap, the category with the higher priority wins: IBAN over credit card over email over tax ID over date over address over phone over person. That is why “02.07.1981” becomes a date, not a phone number.',
      ],
    },
    {
      id: 'categories',
      title: 'Categories',
      body: [
        '• Person: first names and surnames. The pseudonym keeps gender (“Frau Berger” never becomes “Herr Vogt”) and families: the same surname is replaced identically throughout the file.',
        '• Address: street + house number (German: “Musterstraße 12a”, “Am Hang 3”; English: “12 Baker Street”). Pseudonym: another street from the name world, a new number. The town stays.',
        '• Postcode: five digits before a known town or with “D-”. The pseudonym keeps the first two digits (area).',
        '• Date: 14.03.2024, 2024-03-14, 3/14/2024, 14. März 2024, March 14, 2024. Pseudonym = shift by the same amount for every date; intervals and order are preserved.',
        '• Email: local part built from the person’s pseudonyms (anna.berger → lena.vogt), domain stays. Unknown local parts get a new name with a number.',
        '• Phone: national (0221 4711-0) and international (+49 160 …, +49 (0)221 …, 0049 221 …). Country and area code stay, subscriber number is new, format preserved; an extension “-0” stays.',
        '• IBAN: country and bank code stay, account number is new, checksum valid, grouping preserved.',
        '• Credit card: first four digits stay, the rest is new, Luhn checksum valid.',
        '• Tax ID / VAT ID, insurance number: a new number with a valid checksum; the area number stays.',
        '• Licence plate: the district code stays, letters and digits are new.',
        '• IP address: IPv4 and IPv6 (also compressed with “::”). The network (first two octets or groups) stays, host is new.',
        '• Customer / case ID: letters and separators stay, digits are new with the same length — customer 10482 becomes the same new number everywhere.',
      ],
    },
    {
      id: 'strategies',
      title: 'Strategies',
      body: [
        '• Pseudonym: a plausible substitute, see Categories. Default for everything.',
        '• Placeholder: [PERSON 3] — counter per original; the same person gets the same number. Template with {cat} and {n} is adjustable (masked).',
        '• Redact: a bar █ of the original length. Character and “keep length” adjustable (masked).',
        '• Mask: letters and digits become X, separators stay; “keep last n” shows e.g. the last four digits of an IBAN (masked).',
        '• Delete: remove without substitute.',
        'Strategies apply per category in the rule set. Individual findings are overridden directly in the table: click the replacement and type your own text.',
      ],
    },
    {
      id: 'formats',
      title: 'Formats',
      body: [
        '• Text: TXT, Markdown, logs, JSON/JSONL, YAML, XML, HTML, SQL, INI/TOML, email (EML/MBOX), subtitles (SRT/VTT), vCard, iCal, RTF, LaTeX. The file is checked as a whole; line endings and encoding are preserved.',
        '• CSV/TSV: the delimiter (; , tab |) is detected, quotes are kept. Columns whose header reads name, first name, email, phone, IBAN, DOB, postcode, customer no. are typed completely — including surnames missing from every dictionary.',
        '• Fixed width: tables inside running text (header + rows separated by two or more spaces) are typed the same way.',
        '• DOCX: word/document.xml plus headers/footers, footnotes/endnotes, comments, deleted text from “Track changes”, field codes (such as HYPERLINK "mailto:…" in older documents and mail merges) and external link targets (e.g. mailto:anna@…). Each formatting run is one preview line. A name Word split across two runs (e.g. after a spelling correction) appears split — first and last name are usually still detected individually.',
        '• XLSX: text cells (shared strings and inline strings) and comments of all sheets plus external link targets. Columns are typed via the header like CSV — then numeric cells are replaced too (customer number stored as a number, phone, postcode, tax ID, card number) and date cells are shifted as Excel serial numbers. Numeric cells without a typed header stay unchanged.',
        '• ODT/ODS: all text nodes in content.xml and styles.xml (headers and footers) plus external links (mailto:, https:).',
        '• Metadata: when saving DOCX, XLSX and ODF, anonym clears author, “last modified by”, title, subject, description, keywords, company and manager and replaces the names of comment and revision authors with “anonym” — even if nothing else was replaced. The first-page preview image (LibreOffice always stores one, Word with “Save preview picture”) shows the original and is removed.',
        '• Size: files up to 512 MB, each archive part (unpacked) up to 256 MB, all text parts together up to 1 GB — protection against crafted ZIP archives.',
        '• Encodings: UTF-8 (with and without BOM), UTF-16 with BOM, otherwise Windows-1252 as fallback; other code pages (ISO-8859-15, Windows-1250, Macintosh, KOI8-R) under Settings → Files (masked).',
        '• Export: every anonymised file can additionally be written as TXT, Markdown, HTML, CSV, TSV, JSON, XLSX, DOCX, ODT, ODS, PDF or RTF. Tables (CSV, XLSX, ODS) become tables with columns in text formats and a list of objects per row in JSON (header = keys); texts become one row per line in table formats. All sheets of a workbook are carried over. PDF uses a fixed font with the Windows-1252 character set — characters outside it (e.g. Cyrillic, CJK) appear as “?”.',
        'Legacy formats (DOC, XLS, WordPerfect, Lotus, dBase, EBCDIC …) as input, the PDF text layer and JSON format descriptions follow via updates.',
      ],
    },
    {
      id: 'rules',
      title: 'Rule set & seed',
      body: [
        'A rule set holds name, seed, name world, date shift, the per-category settings, allowlist, custom patterns and word lists. It lives as rules.json in the data folder and can be exported and loaded.',
        '• Seed: a string from which every replacement is derived deterministically. Same seed, same file, same output — even on another machine. Another seed: every person gets new pseudonyms.',
        '• The seed is a key: whoever knows the seed and anonym can recompute pseudonyms, numbers and the date shift. So anonym generates a random seed (96 bits) on first start; “New” next to the field generates another one. The former fixed default “anonym” is replaced automatically — unless a filled pseudonym store depends on it (the rule set then says so). Exported rule sets contain the seed: share them only with places allowed to translate back.',
        '• Date shift: 0 = derived from the seed (±1 to 45 days), otherwise a fixed number of days.',
        '• Allowlist: phrases that always stay; findings inside them are discarded. Case does not matter.',
        '• Custom patterns (masked): regular expressions in Rust syntax; the whole match is assigned to the chosen category. Example: PRJ-\\d{4} as customer / case ID.',
        '• Custom word lists (masked): words or phrases that always count as findings — project names, code names. For the Person category they are replaced like surnames.',
      ],
    },
    {
      id: 'store',
      title: 'Pseudonym store & key file',
      body: [
        'Within one file, pseudonyms are always consistent. Across files, the pseudonym store guarantees it (masked, Settings → Pseudonym store): every replacement is remembered in pseudonyms.json and reused for the next file.',
        '• That keeps relations between tables intact: customer 10482 becomes the same new number in every export, Anna Berger is Lena Vogt everywhere.',
        '• The store doubles as the key file. Whoever has it can translate back — export it only to places that are allowed to.',
        '• Legally, as long as a key file, recover file or report exists the result is pseudonymised, not anonymised. “Clear store” removes the key file; the result is only anonymised once recover files and reports are deleted too. After clearing, open files are analysed again — the substitutes shown no longer come from the old store.',
        '• If you change the seed, the store no longer fits and is emptied on the next save. “New” and a seed edited by hand (applied when leaving the field or with Enter) ask first while the store has entries; cancelling restores the previous seed. A loaded rule set and “Reset to defaults” may bring a different seed as well — the reset confirmation then mentions the store, and a notice appears after loading. Export the key file first if you need it.',
        '• “Load key file …” copies the file’s seed and name world into the active rule set — otherwise the loaded store would be discarded right away as not matching.',
      ],
    },
    {
      id: 'recover',
      title: 'Recover (recover file)',
      body: [
        'For every output anonym writes a recover file <name>.anonym.recover.json (Settings → Files). It contains every replacement of that file: original → substitute per category, plus the name parts (first name, surname, street), the date shift and the seed.',
        '• “Recover …” (header or key R) translates an anonymised file back — even after it has been edited: new rows, changed paragraphs, different order, another format (e.g. the CSV exported as XLSX). Everything that did not come from the anonymisation stays as it is.',
        '• Flow in the dialog: choose the edited file, choose the recover file (anonym suggests the matching one next to it), password if needed, choose the output format — same as the file or one of the twelve export formats (TXT, Markdown, HTML, CSV, TSV, JSON, XLSX, DOCX, ODT, ODS, PDF, RTF) — then “Recover …” and confirm the target (suggestion <name>.recovered.<ext>). The result shows how many places were translated back and how many substitutes no longer occurred.',
        '• Word by word: substitutes are searched as whole words, longest first, ignoring case; the spelling of the occurrence is kept (RADEMACHER → BERGER). A surname reused on its own finds its way back via the name parts.',
        '• Not reversible: redactions, masks and deleted places — and placeholders when the same text stood for different originals. Such substitutes are skipped as “ambiguous”.',
        '• Password: under Settings → Files recover files can be encrypted (XChaCha20-Poly1305, key via Argon2id). The password is asked once per session — entered twice, against typos — and never stored. Without a password the recover file is plain text — treat it like the original. With encryption on, anonym also leaves the original values out of the report file next to it.',
        '• Legally: as long as a recover file exists, the output is pseudonymised, not anonymised. It is only anonymised once the report (which holds the original values unless encryption is on) and the pseudonym store are deleted too.',
      ],
    },
    {
      id: 'report',
      title: 'Report',
      body: [
        'After saving, anonym writes <name>.anonym.report.json next to the output (can be disabled under Settings → Files). The report contains source, output, format, rule set, name world, date shift, number of replacements and the list “original → replacement” with counts.',
        'The “Report” tab shows the same inside the app. A PDF report follows via update.',
        'Note: the report contains the original values. It belongs in your records, not with the recipient of the anonymised file. Exception: when recover files are encrypted (Settings → Files), the “original” column stays empty in the report file — the “Report” tab still shows it during the session.',
      ],
    },
    {
      id: 'limits',
      title: 'Limits — honestly',
      body: [
        'Without AI there is no understanding. anonym finds what dictionaries and patterns contain and misses what they don’t: the rare surname without salutation, the nickname, the description (“the baker from Main Street”) that still identifies a person.',
        '• Hence the preview, hence the look before sending.',
        '• Surnames without a first name or salutation are not detected in running text — in table columns with a matching header they are. Help with custom word lists.',
        '• A first name that is also a word (“May”, “August” as months) only counts with a surname or salutation.',
        '• Excel numeric cells without a typed header, sheet names, headers/footers of Excel sheets, images, embedded objects and custom document properties (docProps/custom.xml) are not checked.',
        '• Free text after a name (“Anna Furniture”) can be mistaken for a surname — the source “context” with 70 % confidence marks such cases; one click rejects.',
        '• An international number made only of three-digit groups with dots after the country code (“+49.221.471.100”) looks like a number with thousands separators and is not reported as a phone — likewise with spaces below nine digits. Add a custom pattern if needed.',
      ],
    },
    {
      id: 'news',
      title: 'New in 1.0',
      body: [
        '• Random seed per installation instead of the fixed default — pseudonyms cannot be recomputed without your seed. “New” button in the rule set.',
        '• DOCX/XLSX/ODF: metadata (author, company, comment authors) is removed; deleted text from “Track changes”, Excel comments, ODT headers/footers, Word field codes and mailto: links are checked too; first-page preview images are dropped.',
        '• Decisions (accepted, rejected, own replacement) stay on the right finding when a new analysis changes the list (e.g. after adding an allowlist entry).',
        '• The recover-file password is asked twice; without a password nothing is written.',
        '• Output, report and recover file cannot overwrite the source — not via case, symlinks or “..” either.',
        '• File notes and error messages appear in the interface language.',
        '• Changed a finding’s category: table and preview show the new replacement right away — exactly the one that is written on save.',
        '• Confirmation before “Reset to defaults”, before a new seed while the pseudonym store has entries and before removing custom patterns and word lists.',
        '• Windows: CSV/TSV exports carry a UTF-8 BOM (Excel shows umlauts correctly); rule sets, key and recover files saved by Windows Notepad (with BOM) are read. Briefly locked files (virus scanner, OneDrive) are retried a few times when saving.',
        '• Update errors are shown instead of being dropped silently.',
        '• International phone numbers (+49 160 …) and compressed IPv6 addresses (2001:db8::…) are detected; with +49 (0)221 … and 0049 … the area code stays.',
        '• Editing the seed by hand asks first while the pseudonym store has entries; “Clear store” analyses open files again.',
        '• Encrypted recover file: the report next to it no longer contains original values.',
        '• “Save all” names the reason when files fail instead of hiding it behind the success message.',
        '• Esc closes dialogs even from an input field, and only the topmost one; while the manual is open, keys do not trigger actions behind it.',
        '• Signed numbers with thousands groups (“+12.500.000”, “+1 250 000”) are no longer taken for phone numbers; the VAT ID gets a valid check digit.',
        `• ${modKey('en')} + , while Settings are already open or behind another dialog opens nothing again — unsaved changes can still be discarded with Esc.`,
      ],
    },
    {
      id: 'shortcuts',
      title: 'Keyboard',
      body: ['• O — open file', '• S — save anonymised', '• R — recover', '• 1, 2, 3, 4 — tabs Findings, Preview, Rules, Report', '• ? — manual', `• ${modKey('en')} + , — settings`, '• Esc — close dialog'],
    },
  ],
};

export function Help({ lang, openSignal = 0 }: { lang: Lang; openSignal?: number }) {
  // Sprache der Hilfe: folgt der App-Sprache, lässt sich aber im Kopf der
  // Hilfe jederzeit zwischen DE und EN umschalten
  const [helpLang, setHelpLang] = useState<'de' | 'en'>(lang === 'de' ? 'de' : 'en');
  useEffect(() => {
    setHelpLang(lang === 'de' ? 'de' : 'en');
  }, [lang]);
  const c = helpLang === 'de' ? de : en;
  const [mode, setMode] = useState<'closed' | 'tutorial' | 'manual'>(() => {
    try {
      return localStorage.getItem(SEEN_KEY) ? 'closed' : 'tutorial';
    } catch {
      return 'closed';
    }
  });
  const [step, setStep] = useState(0);
  const [sel, setSel] = useState(c.sections[0].id);
  const [q, setQ] = useState('');

  useEffect(() => {
    if (openSignal > 0) setMode('manual');
  }, [openSignal]);

  const close = () => {
    try {
      localStorage.setItem(SEEN_KEY, '1');
    } catch {
      /* Speicher nicht verfügbar */
    }
    setMode('closed');
    setStep(0);
  };

  useEffect(() => {
    if (mode === 'closed') return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') close();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [mode]);

  const query = q.trim().toLowerCase();
  const filtered = query ? c.sections.filter((s) => s.title.toLowerCase().includes(query) || s.body.some((p) => p.toLowerCase().includes(query))) : c.sections;
  const current = filtered.find((s) => s.id === sel) ?? filtered[0] ?? null;

  const para = (p: string, i: number) =>
    p.startsWith('• ') ? (
      <div key={i} className="hlp-li">
        {p.slice(2)}
      </div>
    ) : (
      <p key={i}>{p}</p>
    );

  return (
    <>
      <button className="hlp-fab" title={c.labels.fab} onClick={() => setMode('manual')}>
        ?
      </button>
      {mode !== 'closed' && (
        <div className="hlp-overlay" onClick={close}>
          <div className="hlp-modal" onClick={(e) => e.stopPropagation()}>
            <div className="hlp-head">
              <span className="hlp-brand">
                <span className="hlp-name">anonym</span>
                <span className="hlp-dot">.</span>
              </span>
              <button
                className={`hlp-tab ${mode === 'tutorial' ? 'active' : ''}`}
                onClick={() => {
                  setMode('tutorial');
                  setStep(0);
                }}
              >
                {c.labels.tutorial}
              </button>
              <button className={`hlp-tab ${mode === 'manual' ? 'active' : ''}`} onClick={() => setMode('manual')}>
                {c.labels.manual}
              </button>
              <span className="hlp-spacer" />
              <span className="hlp-lang" role="group" aria-label="Sprache / Language">
                <button className={`hlp-lang-btn ${helpLang === 'de' ? 'active' : ''}`} onClick={() => setHelpLang('de')}>
                  DE
                </button>
                <button className={`hlp-lang-btn ${helpLang === 'en' ? 'active' : ''}`} onClick={() => setHelpLang('en')}>
                  EN
                </button>
              </span>
              <button className="hlp-close" onClick={close}>
                ✕
              </button>
            </div>

            {mode === 'tutorial' && (
              <div className="hlp-tut">
                <div className="hlp-step-count">{c.labels.stepOf(step + 1, c.tutorial.length)}</div>
                <h2>{c.tutorial[step].title}</h2>
                {c.tutorial[step].body.map(para)}
                <div className="hlp-tut-nav">
                  <button className="hlp-ghost" onClick={close}>
                    {c.labels.skip}
                  </button>
                  <span className="hlp-dots">
                    {c.tutorial.map((_, i) => (
                      <span key={i} className={i === step ? 'on' : ''} />
                    ))}
                  </span>
                  {step > 0 && <button onClick={() => setStep(step - 1)}>{c.labels.back}</button>}
                  {step < c.tutorial.length - 1 ? (
                    <button className="hlp-primary" onClick={() => setStep(step + 1)}>
                      {c.labels.next}
                    </button>
                  ) : (
                    <button className="hlp-primary" onClick={close}>
                      {c.labels.done}
                    </button>
                  )}
                </div>
              </div>
            )}

            {mode === 'manual' && (
              <div className="hlp-body">
                <div className="hlp-toc">
                  <input type="text" placeholder={c.labels.search} value={q} onChange={(e) => setQ(e.target.value)} />
                  {filtered.length === 0 && <div className="hlp-empty">{c.labels.noResults}</div>}
                  {filtered.map((s) => (
                    <button key={s.id} className={`hlp-toc-item ${current?.id === s.id ? 'active' : ''}`} onClick={() => setSel(s.id)}>
                      {s.title}
                    </button>
                  ))}
                </div>
                <div className="hlp-content">
                  {current && (
                    <>
                      <h2>{current.title}</h2>
                      {current.body.map(para)}
                    </>
                  )}
                </div>
              </div>
            )}
          </div>
        </div>
      )}
    </>
  );
}
