// Plattform-Erkennung für Beschriftungen (keine Importe — läuft auch unter `node --test`).

/** macOS (WebKit-WebView meldet „Macintosh“ / „Mac OS X“). */
export function isMacUA(ua: string): boolean {
  return /Mac|iPhone|iPad/.test(ua);
}

export const isMac = typeof navigator !== 'undefined' && isMacUA(navigator.userAgent);

/** Beschriftung der Befehlstaste: ⌘ auf dem Mac, sonst Strg (deutsch) bzw. Ctrl. */
export function modKey(lang: 'de' | 'en', mac: boolean = isMac): string {
  return mac ? '⌘' : lang === 'de' ? 'Strg' : 'Ctrl';
}

/** Sprache beim ersten Start ohne Einstellungen: aus dem Browser/WebView, sonst Englisch. */
export function initialLang(navLang: string | undefined): 'de' | 'en' {
  return (navLang ?? '').toLowerCase().startsWith('de') ? 'de' : 'en';
}
