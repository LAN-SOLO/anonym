// Vertrag zwischen React-UI und Tauri-Backend (Kommandos in src-tauri/src/commands.rs).
// Typen spiegeln anonym-core/src/model.rs (serde camelCase / kebab-case).

import { invoke } from '@tauri-apps/api/core';

export const isTauri = '__TAURI_INTERNALS__' in window;

export type Category =
  | 'person'
  | 'address'
  | 'postal-code'
  | 'date'
  | 'email'
  | 'phone'
  | 'iban'
  | 'credit-card'
  | 'tax-id'
  | 'insurance'
  | 'plate'
  | 'ip'
  | 'customer-id';

export const CATEGORIES: Category[] = [
  'person',
  'address',
  'postal-code',
  'date',
  'email',
  'phone',
  'iban',
  'credit-card',
  'tax-id',
  'insurance',
  'plate',
  'ip',
  'customer-id',
];

export type Strategy = 'pseudonym' | 'placeholder' | 'redact' | 'mask' | 'delete';
export const STRATEGIES: Strategy[] = ['pseudonym', 'placeholder', 'redact', 'mask', 'delete'];

export type Source = 'dictionary' | 'pattern' | 'checksum' | 'context' | 'column' | 'custom';
export type Gender = 'm' | 'f' | 'u';

export type NamePart = { first: { text: string; gender: Gender } } | { last: { text: string } } | { keep: { text: string } };

export interface CategoryRule {
  enabled: boolean;
  strategy: Strategy;
  redactChar: string;
  maskChar: string;
  keepLast: number;
  keepLength: boolean;
  placeholder: string;
}

export interface CustomPattern {
  name: string;
  pattern: string;
  category: Category;
}

export interface CustomWords {
  name: string;
  category: Category;
  words: string[];
}

export interface Rules {
  name: string;
  seed: string;
  world: string;
  dateShiftDays: number;
  categories: Record<Category, CategoryRule>;
  allowlist: string[];
  customPatterns: CustomPattern[];
  customWords: CustomWords[];
  columnTyping: boolean;
}

export interface Finding {
  id: number;
  bstart: number;
  bend: number;
  start: number;
  end: number;
  line: number;
  text: string;
  category: Category;
  source: Source;
  confidence: number;
  replacement: string;
  parts?: NamePart[];
}

export interface Decision {
  id: number;
  accept: boolean;
  category?: Category | null;
  replacement?: string | null;
}

export interface Analysis {
  path: string;
  format: string;
  encoding: string;
  text: string;
  lines: number;
  findings: Finding[];
  notes: string[];
}

export interface ReportEntry {
  category: Category;
  original: string;
  replacement: string;
  count: number;
}

export interface Report {
  tool: string;
  version: string;
  created: string;
  source: string;
  output: string;
  format: string;
  rules: string;
  world: string;
  dateShiftDays: number;
  replaced: number;
  rejected: number;
  byCategory: Record<string, number>;
  entries: ReportEntry[];
}

export interface ApplyResult {
  output: string;
  reportPath: string | null;
  recoverPath: string | null;
  report: Report;
}

export interface RecoverInfo {
  encrypted: boolean;
  source: string | null;
  output: string | null;
  created: string | null;
  entries: number;
}

export interface RecoverResult {
  output: string;
  format: string;
  restored: number;
  notFound: number;
  ambiguous: number;
  byKind: Record<string, number>;
}

export interface ExportTarget {
  ext: string;
  label: string;
}

export interface StoreInfo {
  path: string;
  entries: number;
  seed: string;
  world: string;
}

export interface Settings {
  language: 'de' | 'en';
  theme: 'dark' | 'light';
  accent: 'blue' | 'emerald' | 'violet' | 'amber';
  autoUpdate: boolean;
  /** Tarif „masked“ — im Vorabzugang frei umschaltbar. */
  masked: boolean;
  outputDir: string;
  outputSuffix: string;
  fallbackEncoding: string;
  useStore: boolean;
  writeReport: boolean;
  writeRecover: boolean;
  encryptRecover: boolean;
  confirmOverwrite: boolean;
}

export interface UpdateInfo {
  version: string;
  notes: string | null;
  date: string | null;
}

export const defaultSettings: Settings = {
  language: 'de',
  theme: 'dark',
  accent: 'blue',
  autoUpdate: false,
  masked: false,
  outputDir: '',
  outputSuffix: '.anonym',
  fallbackEncoding: 'windows-1252',
  useStore: false,
  writeReport: true,
  writeRecover: true,
  encryptRecover: false,
  confirmOverwrite: true,
};

export const FALLBACK_ENCODINGS = ['windows-1252', 'iso-8859-15', 'windows-1250', 'iso-8859-2', 'macintosh', 'koi8-r'];

export const FILE_EXTENSIONS = [
  'txt', 'md', 'markdown', 'log', 'csv', 'tsv', 'tab', 'json', 'jsonl', 'ndjson', 'yaml', 'yml', 'xml', 'html', 'htm', 'srt', 'vtt', 'eml', 'mbox', 'sql', 'ini', 'cfg', 'conf', 'toml', 'rtf', 'tex', 'rst', 'adoc', 'vcf', 'ics', 'docx', 'docm', 'dotx', 'xlsx', 'xlsm', 'xltx', 'odt', 'ods', 'ott', 'ots',
];

function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri) return Promise.reject(new Error('Nur in der Desktop-App verfügbar'));
  return invoke<T>(cmd, args);
}

export const api = {
  // Einstellungen
  getSettings: () => call<Settings>('get_settings'),
  setSettings: (settings: Settings) => call<void>('set_settings', { settings }),
  dataPath: () => call<string>('data_path'),

  // Regelwerk
  getRules: () => call<Rules>('get_rules'),
  setRules: (rules: Rules) => call<void>('set_rules', { rules }),
  defaultRules: () => call<Rules>('default_rules'),
  importRules: (path: string) => call<Rules>('import_rules', { path }),
  exportRules: (rules: Rules, path: string) => call<void>('export_rules', { rules, path }),
  listWorlds: () => call<string[]>('list_worlds'),

  // Analyse & Anwendung
  analyzeFile: (path: string, rules: Rules) => call<Analysis>('analyze_file', { path, rules }),
  suggestOutput: (path: string, ext?: string) => call<string>('suggest_output', { path, ext: ext ?? null }),
  exportTargets: () => call<ExportTarget[]>('export_targets'),
  applyFile: (path: string, rules: Rules, decisions: Decision[], output: string, password?: string) =>
    call<ApplyResult>('apply_file', { path, rules, decisions, output, password: password ?? null }),

  // Rückübersetzung
  recoverInfo: (path: string) => call<RecoverInfo>('recover_info', { path }),
  suggestRecovered: (path: string) => call<string>('suggest_recovered', { path }),
  suggestRecoverKey: (path: string) => call<string>('suggest_recover_key', { path }),
  recoverFile: (path: string, keyPath: string, password: string | null, output: string) =>
    call<RecoverResult>('recover_file', { path, keyPath, password, output }),

  // Pseudonym-Speicher
  storeInfo: () => call<StoreInfo>('store_info'),
  storeClear: () => call<void>('store_clear'),
  storeExport: (path: string) => call<void>('store_export', { path }),
  storeImport: (path: string) => call<StoreInfo>('store_import', { path }),

  // System
  pathExists: (path: string) => call<boolean>('path_exists', { path }),
  openPath: (path: string) => call<void>('open_path', { path }),
  checkUpdate: () => call<UpdateInfo | null>('check_update'),
  installUpdate: () => call<void>('install_update'),
};
