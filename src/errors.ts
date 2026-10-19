// Fehlermeldungen aus dem Rust-Kern kommen als feste deutsche Texte (oder als Code wie
// PASSWORD_WRONG). Hier werden sie in die Sprache der Oberfläche gebracht. Keine Importe:
// die Datei läuft auch direkt unter `node --test` (tests/errors.test.ts).

export type ErrLang = 'de' | 'en';

const CODES: Record<string, Record<ErrLang, string>> = {
  PASSWORD_REQUIRED: { de: 'Passwort erforderlich.', en: 'Password required.' },
  PASSWORD_WRONG: { de: 'Falsches Passwort.', en: 'Wrong password.' },
};

const NOUN: Record<string, string> = {
  Regelwerk: 'Rule set',
  'Recover-Datei': 'Recover file',
  Schlüsseldatei: 'Key file',
};
const NOUNS = 'Regelwerk|Recover-Datei|Schlüsseldatei';

/** Deutsch → Englisch, längere/spezifischere Muster zuerst. */
const EN: [RegExp, string | ((substring: string, ...args: string[]) => string)][] = [
  [new RegExp(`(${NOUNS}) nicht lesbar`, 'g'), (_m, n) => `${NOUN[n]} not readable`],
  [new RegExp(`(${NOUNS}): keine Datei oder größer als (\\d+) MB`, 'g'), (_m, n, mb) => `${NOUN[n]}: not a file or larger than ${mb} MB`],
  [/Datei nicht lesbar/g, 'File not readable'],
  [/Datei zu groß \((\d+) MB, Grenze (\d+) MB\)/g, 'File too large ($1 MB, limit $2 MB)'],
  [/Datei oder Ordner nicht gefunden/g, 'File or folder not found'],
  [/Datei wird von einem anderen Programm verwendet — bitte dort schließen/g, 'The file is in use by another program — please close it there'],
  [/Zugriff verweigert — ist die Datei in einem anderen Programm geöffnet oder schreibgeschützt\?/g, 'Access denied — is the file open in another program or read-only?'],
  [/Datenträger ist schreibgeschützt/g, 'The drive is write-protected'],
  [/Datenträger ist voll/g, 'The drive is full'],
  [/Ungültiger Datei- oder Ordnername/g, 'Invalid file or folder name'],
  [/Pfad oder Dateiname zu lang/g, 'Path or file name too long'],
  [/Ordner anlegen fehlgeschlagen/g, 'Could not create the folder'],
  [/Ungültiger Ausgabepfad/g, 'Invalid output path'],
  [/Schreiben fehlgeschlagen/g, 'Writing failed'],
  [/Speichern fehlgeschlagen/g, 'Saving failed'],
  [/Löschen fehlgeschlagen/g, 'Deleting failed'],
  [/Ausgabe darf die Quelle nicht überschreiben/g, 'The output must not overwrite the source'],
  [/Ausgabe darf die Recover-Datei nicht überschreiben/g, 'The output must not overwrite the recover file'],
  [/Keine gültige Schlüsseldatei: Saat fehlt/g, 'Not a valid key file: seed missing'],
  [/Keine gültige Schlüsseldatei/g, 'Not a valid key file'],
  [/Kein gültiges Regelwerk/g, 'Not a valid rule set'],
  [/Keine gültige Recover-Datei/g, 'Not a valid recover file'],
  [/Recover-Datei unlesbar/g, 'Recover file unreadable'],
  [/Recover-Datei beschädigt/g, 'Recover file damaged'],
  [/Schlüsselableitung fehlgeschlagen/g, 'Key derivation failed'],
  [/Verschlüsselung fehlgeschlagen/g, 'Encryption failed'],
  [/Adresse nicht erlaubt/g, 'Address not allowed'],
  [/Nur Ordner lassen sich öffnen/g, 'Only folders can be opened'],
  [/Update-Prüfung fehlgeschlagen/g, 'Update check failed'],
  [/Kein Update verfügbar/g, 'No update available'],
  [/Update fehlgeschlagen/g, 'Update failed'],
  [/Kein gültiges ZIP-Archiv/g, 'Not a valid ZIP archive'],
  [/Kein Dokumentinhalt im Archiv gefunden/g, 'No document content found in the archive'],
  [/Dokumentinhalt zu groß \(entpackt über (\d+) MB\)/g, 'Document content too large (over $1 MB unpacked)'],
  [/Archiv-Teil zu groß \((\d+) MB, Grenze (\d+) MB\)/g, 'Archive part too large ($1 MB, limit $2 MB)'],
  [/Archiv-Teil zu groß \(Grenze (\d+) MB\)/g, 'Archive part too large (limit $1 MB)'],
  [/Interner Fehler: Zeilenzahl der Vorschau passt nicht zum Dokument/g, 'Internal error: preview line count does not match the document'],
  [/Keine Tabellenblätter gefunden/g, 'No worksheets found'],
  [/(\S+\.xml) fehlt/g, '$1 missing'],
  [/^Keine Datei$/g, 'Not a file'],
  [/Nur in der Desktop-App verfügbar/g, 'Only available in the desktop app'],
];

/** Beliebigen Fehler (String, Error, Tauri-Fehlerobjekt) als Text in der UI-Sprache. */
export function errorText(lang: ErrLang, e: unknown): string {
  let msg: string;
  if (typeof e === 'string') msg = e;
  else if (e instanceof Error) msg = e.message;
  else if (e && typeof e === 'object' && 'message' in e && typeof (e as { message: unknown }).message === 'string') msg = (e as { message: string }).message;
  else msg = String(e);
  for (const code of Object.keys(CODES)) if (msg.includes(code)) return CODES[code][lang];
  if (lang === 'de') return msg;
  for (const [re, rep] of EN) msg = typeof rep === 'string' ? msg.replace(re, rep) : msg.replace(re, rep);
  return msg;
}
