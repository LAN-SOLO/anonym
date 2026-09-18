// Kleine reine Helfer für die Oberfläche — keine Tauri-Abhängigkeit.

import { Category, Decision, Finding, Source, Strategy } from './api';
import { Dict, Lang } from './i18n';

export function categoryLabel(t: Dict, c: Category): string {
  return t.cat[c];
}

export function strategyLabel(t: Dict, s: Strategy): string {
  return t.strat[s];
}

export function sourceLabel(t: Dict, s: Source): string {
  return t.src[s];
}

export function baseName(path: string): string {
  const i = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return i >= 0 ? path.slice(i + 1) : path;
}

export function dirName(path: string): string {
  const i = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return i >= 0 ? path.slice(0, i) : '';
}

export function fmtDateTime(iso: string, lang: Lang): string {
  if (!iso) return '';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(lang === 'de' ? 'de-DE' : 'en-GB', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}

/** Wirksame Kategorie und Ersatz einer Fundstelle nach Entscheidung. */
export function effective(f: Finding, d: Decision | undefined): { accept: boolean; category: Category; replacement: string } {
  return {
    accept: d ? d.accept : true,
    category: d?.category ?? f.category,
    replacement: d?.replacement ?? f.replacement,
  };
}

/** Ausgabetext clientseitig aus Text + akzeptierten Fundstellen bauen (für die Vorschau). */
export function buildOutput(text: string, findings: Finding[], decisions: Record<number, Decision>): { text: string; marks: { start: number; end: number; category: Category; id: number }[] } {
  let out = '';
  let pos = 0;
  const marks: { start: number; end: number; category: Category; id: number }[] = [];
  for (const f of findings) {
    const e = effective(f, decisions[f.id]);
    if (!e.accept) continue;
    if (f.start < pos) continue;
    out += text.slice(pos, f.start);
    marks.push({ start: out.length, end: out.length + e.replacement.length, category: e.category, id: f.id });
    out += e.replacement;
    pos = f.end;
  }
  out += text.slice(pos);
  return { text: out, marks };
}

/** Kategorie-Farbe als CSS-Variable. */
export function categoryColor(c: Category): string {
  return `var(--cat-${c})`;
}

export function countBy<T extends string>(items: T[]): Record<string, number> {
  const out: Record<string, number> = {};
  for (const i of items) out[i] = (out[i] ?? 0) + 1;
  return out;
}
