// Läuft mit `pnpm test` (node --test, Node ≥ 22.18 entfernt die TypeScript-Typen selbst).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { errorText } from '../src/errors.ts';
import { initialLang, isMacUA, modKey } from '../src/platform.ts';

const root = fileURLToPath(new URL('..', import.meta.url));
const GERMAN = /[äöüßÄÖÜ]|\bnicht\b|\bKeine?\b|fehlgeschlagen|\bDatei\b|\bfehlt\b|\bOrdner\b|Ungültig|Fehler|Regelwerk|Schlüssel|Grenze|darf|lesbar/;

function rustFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((n) => {
    const p = join(dir, n);
    return statSync(p).isDirectory() ? rustFiles(p) : p.endsWith('.rs') ? [p] : [];
  });
}

/** Fehlertexte aus dem Rust-Code (ohne Testmodule): Literale in Err(…), ok_or(…), map_err(…). */
function rustErrorLiterals(): string[] {
  const out: string[] = [];
  for (const f of [...rustFiles(join(root, 'core/src')), ...rustFiles(join(root, 'src-tauri/src'))]) {
    const src = readFileSync(f, 'utf8').split('#[cfg(test)]')[0];
    for (const line of src.split('\n')) {
      if (!/Err\(|ok_or\(|map_err\(/.test(line) || line.trim().startsWith('//')) continue;
      for (const m of line.matchAll(/"((?:[^"\\]|\\.)*)"/g)) {
        const lit = m[1].replace('{what}', 'Recover-Datei').replace(/\{[^}]*\}/g, '42');
        if (/[A-Za-zÄÖÜäöü]{3,}/.test(lit)) out.push(lit);
      }
    }
  }
  return out;
}

test('every backend error literal has an English translation', () => {
  const lits = rustErrorLiterals();
  assert.ok(lits.length > 30, `only ${lits.length} literals found`);
  const missing = lits.filter((l) => GERMAN.test(errorText('en', l)));
  assert.deepEqual(missing, []);
});

test('composite messages translate every part, German stays untouched', () => {
  const raw = 'Datei nicht lesbar: Datei oder Ordner nicht gefunden (os error 2)';
  assert.equal(errorText('en', raw), 'File not readable: File or folder not found (os error 2)');
  assert.equal(errorText('de', raw), raw);
  assert.equal(errorText('en', 'Recover-Datei: keine Datei oder größer als 64 MB'), 'Recover file: not a file or larger than 64 MB');
  assert.equal(errorText('en', 'Schlüsseldatei nicht lesbar: Zugriff verweigert — ist die Datei in einem anderen Programm geöffnet oder schreibgeschützt? (os error 13)'), 'Key file not readable: Access denied — is the file open in another program or read-only? (os error 13)');
  assert.equal(errorText('en', 'Datei zu groß (600 MB, Grenze 512 MB)'), 'File too large (600 MB, limit 512 MB)');
  assert.equal(errorText('en', 'word/document.xml fehlt'), 'word/document.xml missing');
});

test('codes and error objects', () => {
  assert.equal(errorText('en', 'PASSWORD_WRONG'), 'Wrong password.');
  assert.equal(errorText('de', 'PASSWORD_REQUIRED'), 'Passwort erforderlich.');
  assert.equal(errorText('en', new Error('Kein gültiges Regelwerk: expected value at line 1 column 1')), 'Not a valid rule set: expected value at line 1 column 1');
  assert.equal(errorText('en', { message: 'Adresse nicht erlaubt' }), 'Address not allowed');
});

test('shortcut labels and first-run language', () => {
  assert.equal(modKey('de', false), 'Strg');
  assert.equal(modKey('en', false), 'Ctrl');
  assert.equal(modKey('de', true), '⌘');
  assert.ok(isMacUA('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15'));
  assert.ok(!isMacUA('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Edg/129.0'));
  assert.equal(initialLang('de-AT'), 'de');
  assert.equal(initialLang('en-US'), 'en');
  assert.equal(initialLang(undefined), 'en');
});
