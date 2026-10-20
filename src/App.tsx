import { useCallback, useEffect, useRef, useState } from 'react';
import { open, save } from '@tauri-apps/plugin-dialog';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { Decision, ExportTarget, FILE_EXTENSIONS, Rules, Settings, UpdateInfo, api, defaultSettings } from './api';
import { dicts, Lang, noteText } from './i18n';
import { errorText } from './errors';
import { initialLang } from './platform';
import { Findings } from './components/Findings';
import { Help } from './components/Help';
import { Preview } from './components/Preview';
import { ReportView } from './components/ReportView';
import { RecoverModal } from './components/RecoverModal';
import { RulesView } from './components/RulesView';
import { SettingsModal } from './components/SettingsModal';
import { FileEntry, Sidebar } from './components/Sidebar';
import { IconGear, IconRefresh } from './icons';
import { baseName, effective } from './util';

type Tab = 'findings' | 'preview' | 'rules' | 'report';

/** Bestätigungsdialog (window.confirm ist im WebView nicht verlässlich). */
function ConfirmModal({ text, okLabel, cancelLabel, altLabel, onOk, onAlt, onClose }: { text: string; okLabel: string; cancelLabel: string; altLabel?: string; onOk: () => void; onAlt?: () => void; onClose: () => void }) {
  return (
    <div className="overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="note" style={{ marginTop: 0, fontSize: 12.5, color: 'var(--text)' }}>
          {text}
        </div>
        <div className="btnrow">
          <button className="ghost" onClick={onClose} autoFocus>
            {cancelLabel}
          </button>
          {altLabel && onAlt && <button onClick={onAlt}>{altLabel}</button>}
          <button className="primary" onClick={onOk}>
            {okLabel}
          </button>
        </div>
      </div>
    </div>
  );
}

/** Texteingabe (Passwort) als Modal — mit `repeatLabel` zweimal einzugeben (Tippfehler-Schutz). */
function PromptModal({ title, label, repeatLabel, mismatch, okLabel, cancelLabel, password, onOk, onClose }: { title: string; label: string; repeatLabel?: string; mismatch?: string; okLabel: string; cancelLabel: string; password?: boolean; onOk: (v: string) => void; onClose: () => void }) {
  const [v, setV] = useState('');
  const [v2, setV2] = useState('');
  const differs = !!repeatLabel && v2 !== '' && v !== v2;
  const ok = !!v && (!repeatLabel || v === v2);
  const submit = () => ok && onOk(v);
  return (
    <div className="overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>{title}</h2>
        <label className="field">
          <span>{label}</span>
          <input type={password ? 'password' : 'text'} autoFocus value={v} onChange={(e) => setV(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && submit()} />
        </label>
        {repeatLabel && (
          <label className="field">
            <span>{repeatLabel}</span>
            <input type={password ? 'password' : 'text'} value={v2} onChange={(e) => setV2(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && submit()} />
          </label>
        )}
        {differs && <div className="note">{mismatch}</div>}
        <div className="btnrow">
          <button className="ghost" onClick={onClose}>
            {cancelLabel}
          </button>
          <button className="primary" disabled={!ok} onClick={submit}>
            {okLabel}
          </button>
        </div>
      </div>
    </div>
  );
}

/** Stabile Kennung einer Fundstelle über Analysen hinweg: Byte-Position + Text. */
function keyOf(f: { bstart: number; text: string }): string {
  return `${f.bstart}\u0000${f.text}`;
}

function supported(path: string): boolean {
  const ext = path.split('.').pop()?.toLowerCase() ?? '';
  return FILE_EXTENSIONS.includes(ext);
}

export default function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [settingsBackup, setSettingsBackup] = useState<Settings | null>(null);
  const [rules, setRules] = useState<Rules | null>(null);
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [decisions, setDecisions] = useState<Record<string, Record<number, Decision>>>({});
  const [selected, setSelected] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>('findings');
  const [showSettings, setShowSettings] = useState(false);
  const [confirm, setConfirm] = useState<{ text: string; ok?: string; alt?: string } | null>(null);
  const [helpSignal, setHelpSignal] = useState(0);
  const [toast, setToast] = useState<string | null>(null);
  const [updateAvail, setUpdateAvail] = useState<UpdateInfo | null>(null);
  const [installing, setInstalling] = useState(false);
  const [busy, setBusy] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [targets, setTargets] = useState<ExportTarget[]>([]);
  const [exportExt, setExportExt] = useState('xlsx');
  const [prompt, setPrompt] = useState<{ title: string; label: string; repeat?: boolean } | null>(null);
  const [showRecover, setShowRecover] = useState(false);
  /** Sitzungs-Passwort für Recover-Dateien (nie gespeichert). */
  const sessionPw = useRef<string | null>(null);
  const toastTimer = useRef<number | undefined>(undefined);
  const rulesTimer = useRef<number | undefined>(undefined);
  const rulesRef = useRef<Rules | null>(null);
  rulesRef.current = rules;
  const filesRef = useRef<FileEntry[]>([]);
  filesRef.current = files;

  const lang: Lang = settings?.language ?? initialLang(navigator.language);
  const t = dicts[lang];
  const masked = settings?.masked ?? false;
  const langRef = useRef<Lang>(lang);
  langRef.current = lang;
  /** Ersatz für Fundstellen mit umgestellter Kategorie, je Datei: Schlüssel (Position + Text) → Ersatz. */
  const [catPreview, setCatPreview] = useState<Record<string, Record<string, string>>>({});
  /** Laufende Analyse je Datei: nur das Ergebnis der jüngsten wird übernommen. */
  const analyzeSeq = useRef<Record<string, number>>({});

  const showToast = useCallback((msg: string, isError = false) => {
    setToast(msg);
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), isError ? 6000 : 2400);
  }, []);
  // Fehler aus dem Kern sind deutsch formuliert: in die Sprache der Oberfläche übersetzen
  const lastError = useRef<string | null>(null);
  const fail = useCallback(
    (e: unknown) => {
      lastError.current = errorText(langRef.current, e);
      showToast(lastError.current, true);
    },
    [showToast]
  );

  // Bestätigung als Promise: 'ok' | 'alt' (dritte Schaltfläche) | 'cancel'
  type Answer = 'ok' | 'alt' | 'cancel';
  const confirmResolve = useRef<((a: Answer) => void) | null>(null);
  const ask = useCallback(
    (text: string, ok?: string, alt?: string) =>
      new Promise<Answer>((resolve) => {
        // Eine noch offene Rückfrage (z. B. Doppelklick) gilt als abgebrochen, statt ewig zu hängen
        confirmResolve.current?.('cancel');
        confirmResolve.current = resolve;
        setConfirm({ text, ok, alt });
      }),
    []
  );
  const onConfirm = useCallback((text: string, ok?: string) => ask(text, ok).then((a) => a === 'ok'), [ask]);

  // Texteingabe als Promise (null = abgebrochen)
  const promptResolve = useRef<((v: string | null) => void) | null>(null);
  const askText = useCallback(
    (title: string, label: string, repeat = false) =>
      new Promise<string | null>((resolve) => {
        promptResolve.current?.(null);
        promptResolve.current = resolve;
        setPrompt({ title, label, repeat });
      }),
    []
  );
  const settlePrompt = (v: string | null) => {
    setPrompt(null);
    promptResolve.current?.(v);
    promptResolve.current = null;
  };
  const settleConfirm = (a: Answer) => {
    setConfirm(null);
    confirmResolve.current?.(a);
    confirmResolve.current = null;
  };

  // --- Analyse ---------------------------------------------------------------
  const analyze = useCallback(
    (path: string, r: Rules) => {
      setFiles((fs) => fs.map((f) => (f.path === path ? { ...f, status: 'analyzing', error: undefined } : f)));
      const seq = (analyzeSeq.current[path] ?? 0) + 1;
      analyzeSeq.current[path] = seq;
      const stale = () => analyzeSeq.current[path] !== seq;
      api
        .analyzeFile(path, r)
        .then((a) => {
          // Eine spätere Analyse (geändertes Regelwerk) läuft schon: dieses Ergebnis ist veraltet
          if (stale()) return;
          // Fundstellen werden nach jeder Analyse neu durchnummeriert (neue Ausnahme, neues
          // Muster …): Entscheidungen über Position + Text der Fundstelle auf die neuen ids
          // übertragen, nicht über die id — sonst träfe ein „Ablehnen“ plötzlich eine andere Stelle.
          setFiles((fs) => fs.map((f) => (f.path === path ? { ...f, status: 'ready', analysis: a } : f)));
          setDecisions((d) => {
            const old = d[path] ?? {};
            const byKey = new Map<string, Decision>();
            for (const dec of Object.values(old)) if (dec.key) byKey.set(dec.key, dec);
            const next: Record<number, Decision> = {};
            for (const f of a.findings) {
              const prev = byKey.get(keyOf(f));
              if (prev) next[f.id] = { ...prev, id: f.id };
            }
            return { ...d, [path]: next };
          });
        })
        .catch((e) => {
          if (stale()) return;
          setFiles((fs) => fs.map((f) => (f.path === path ? { ...f, status: 'error', error: errorText('de', e) } : f)));
        });
    },
    []
  );

  const addFiles = useCallback(
    (paths: string[]) => {
      const r = rulesRef.current;
      if (!r) return;
      const fresh: string[] = [];
      for (const p of paths) {
        if (!supported(p)) {
          showToast(t.unsupported(baseName(p)), true);
          continue;
        }
        fresh.push(p);
      }
      if (fresh.length === 0) return;
      setFiles((fs) => {
        const known = new Set(fs.map((f) => f.path));
        const add = fresh.filter((p) => !known.has(p)).map<FileEntry>((p) => ({ path: p, status: 'idle' }));
        return [...fs, ...add];
      });
      setSelected((s) => s ?? fresh[0]);
      for (const p of fresh) analyze(p, r);
      setTab((tb) => (tb === 'rules' ? 'findings' : tb));
    },
    [analyze, showToast, t]
  );

  const pickFiles = useCallback(async () => {
    const sel = await open({ multiple: true, filters: [{ name: t.allFiles, extensions: FILE_EXTENSIONS }] });
    if (!sel) return;
    addFiles(Array.isArray(sel) ? sel : [sel]);
  }, [addFiles, t]);

  // --- Laden -------------------------------------------------------------------
  useEffect(() => {
    api
      .getSettings()
      .then((s) => {
        setSettings(s);
        api
          .checkUpdate()
          .then((u) => {
            if (!u) return;
            setUpdateAvail(u);
            if (s.autoUpdate) {
              setInstalling(true);
              api.installUpdate().catch((e) => {
                setInstalling(false);
                fail(e);
              });
            }
          })
          .catch(() => {});
      })
      .catch(() => setSettings({ ...defaultSettings, language: initialLang(navigator.language) }));
    api.getRules().then(setRules).catch(fail);
    api.exportTargets().then(setTargets).catch(() => {});
  }, [fail]);

  // Drag & Drop aus dem Finder/Explorer
  useEffect(() => {
    let off: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((ev) => {
        const p = ev.payload;
        if (p.type === 'enter' || p.type === 'over') setDragging(true);
        else if (p.type === 'leave') setDragging(false);
        else if (p.type === 'drop') {
          setDragging(false);
          addFiles(p.paths);
        }
      })
      .then((f) => (off = f))
      .catch(() => {});
    return () => off?.();
  }, [addFiles]);

  // Darstellung aus den Einstellungen auf <html> spiegeln
  useEffect(() => {
    if (!settings) return;
    document.documentElement.setAttribute('data-theme', settings.theme);
    document.documentElement.setAttribute('data-accent', settings.accent);
    document.documentElement.lang = settings.language;
  }, [settings]);

  // Auswahl: erste Datei, wenn nichts gewählt
  useEffect(() => {
    if (files.length === 0) {
      setSelected(null);
      return;
    }
    if (!selected || !files.some((f) => f.path === selected)) setSelected(files[0].path);
  }, [files, selected]);

  // --- Regelwerk ---------------------------------------------------------------
  const reanalyzeAll = useCallback(
    (r: Rules) => {
      // Seiteneffekte nicht im State-Updater (StrictMode ruft Updater doppelt auf)
      for (const f of filesRef.current) if (f.status !== 'idle') analyze(f.path, r);
    },
    [analyze]
  );

  const changeRules = (r: Rules) => {
    setRules(r);
    window.clearTimeout(rulesTimer.current);
    rulesTimer.current = window.setTimeout(() => {
      api
        .setRules(r)
        .then(() => reanalyzeAll(r))
        .catch(fail);
    }, 600);
  };

  const addAllow = (text: string) => {
    if (!rules) return;
    const word = text.trim();
    if (!word || rules.allowlist.some((a) => a.toLowerCase() === word.toLowerCase())) return;
    const r = { ...rules, allowlist: [...rules.allowlist, word] };
    setRules(r);
    api
      .setRules(r)
      .then(() => {
        showToast(t.addedToAllowlist(word));
        reanalyzeAll(r);
      })
      .catch(fail);
  };

  // --- Entscheidungen ---------------------------------------------------------
  const decide = (path: string, ids: number[], patch: Partial<Decision> | null) => {
    const findings = filesRef.current.find((f) => f.path === path)?.analysis?.findings ?? [];
    setDecisions((d) => {
      const cur = { ...(d[path] ?? {}) };
      for (const id of ids) {
        if (patch === null) {
          delete cur[id];
          continue;
        }
        const f = findings.find((x) => x.id === id);
        const prev = cur[id] ?? { id, accept: true, category: null, replacement: null };
        cur[id] = { ...prev, ...patch, id, key: f ? keyOf(f) : prev.key, bstart: f ? f.bstart : prev.bstart, text: f ? f.text : prev.text };
      }
      return { ...d, [path]: cur };
    });
  };
  const toggle = (path: string, id: number) => {
    const f = files.find((x) => x.path === path)?.analysis?.findings.find((x) => x.id === id);
    if (!f) return;
    const e = effective(f, decisions[path]?.[id]);
    decide(path, [id], { accept: !e.accept });
  };

  // Kategoriewechsel: den neuen Ersatz vom Backend rechnen lassen (gleicher Weg wie beim
  // Speichern), damit Tabelle und Vorschau schon vorher zeigen, was geschrieben wird
  // Je Datei nur neu rechnen, wenn sich etwas Relevantes geändert hat (Regelwerk, Analyse,
  // Kategorie-Entscheidungen, Pseudonym-Speicher nach einem Speichern): jede Anfrage liest und
  // analysiert die Datei vollständig. Laufnummer je Datei: ein älteres Ergebnis wird verworfen.
  const previewTimer = useRef<number | undefined>(undefined);
  const previewReq = useRef<Record<string, { rules: Rules; analysis: object; sig: string; seq: number }>>({});
  const storeGen = useRef(0);
  useEffect(() => {
    if (!rules) return;
    window.clearTimeout(previewTimer.current);
    previewTimer.current = window.setTimeout(() => {
      for (const f of filesRef.current) {
        if (!f.analysis) continue;
        const ds = Object.values(decisions[f.path] ?? {});
        if (!ds.some((d) => d.category && d.accept && d.replacement == null)) {
          delete previewReq.current[f.path];
          setCatPreview((p) => {
            if (!(f.path in p)) return p;
            const { [f.path]: _drop, ...rest } = p;
            return rest;
          });
          continue;
        }
        // Nur Kategorie-Entscheidungen beeinflussen den neuen Ersatz (Reihenfolge im Pseudonymisierer)
        const sig =
          `${storeGen.current}\u0002` +
          ds
            .filter((d) => d.category)
            .map((d) => `${d.key ?? d.id}\u0001${d.category}\u0001${d.accept ? 1 : 0}\u0001${d.replacement == null ? 0 : 1}`)
            .sort()
            .join('\u0002');
        const last = previewReq.current[f.path];
        if (last && last.rules === rules && last.analysis === f.analysis && last.sig === sig) continue;
        const seq = (last?.seq ?? 0) + 1;
        const path = f.path;
        previewReq.current[path] = { rules, analysis: f.analysis, sig, seq };
        const list = ds.map(({ key: _key, preview: _preview, ...d }) => d);
        api
          .previewReplacements(path, rules, list)
          .then((res) => {
            if (previewReq.current[path]?.seq !== seq) return;
            const m: Record<string, string> = {};
            for (const r of res) m[`${keyOf(r)}\u0000${r.category}`] = r.replacement;
            setCatPreview((p) => ({ ...p, [path]: m }));
          })
          .catch(() => {
            // Beim nächsten Anlass erneut versuchen (Tabelle zeigt bis dahin den alten Vorschlag)
            if (previewReq.current[path]?.seq === seq) previewReq.current[path] = { ...previewReq.current[path], sig: '' };
          });
      }
    }, 150);
    return () => window.clearTimeout(previewTimer.current);
  }, [decisions, rules, files]);

  // --- Speichern ---------------------------------------------------------------
  /** Speichern: im Ursprungsformat (ext = undefined), als Export (ext) oder per Dialog (chooseTarget). */
  const applyOne = useCallback(
    async (entry: FileEntry, chooseTarget: boolean, ext?: string): Promise<string | null> => {
      if (!rules || !settings || !entry.analysis) return null;
      let output = await api.suggestOutput(entry.path, ext);
      const srcExt = entry.path.split('.').pop()?.toLowerCase() ?? '';
      // Dateidialog: gewünschtes Format zuerst, dann Ursprungsformat, dann alle Exporte
      const pickPath = async (): Promise<string | null> => {
        const all = [
          ...(srcExt ? [{ name: `${srcExt.toUpperCase()} (${t.outputTo})`, extensions: [srcExt] }] : []),
          ...targets.filter((x) => x.ext !== srcExt).map((x) => ({ name: x.label, extensions: [x.ext] })),
        ];
        const filters = ext ? [...all.filter((f) => f.extensions[0] === ext), ...all.filter((f) => f.extensions[0] !== ext)] : all;
        const sel = await save({ defaultPath: output, filters });
        return sel || null;
      };
      if (chooseTarget || ext) {
        const sel = await pickPath();
        if (!sel) return null;
        output = sel;
      } else if (settings.confirmOverwrite && (await api.pathExists(output))) {
        const a = await ask(t.confirmOverwriteText(baseName(output)), t.overwrite, t.otherName);
        if (a === 'cancel') return null;
        if (a === 'alt') {
          const sel = await pickPath();
          if (!sel) return null;
          output = sel;
        }
      }
      // `key` ist nur für die Oberfläche; bstart + text gehen mit, damit das Backend die
      // Entscheidung auch dann richtig zuordnet, wenn eine neue Analyse noch läuft (ids veraltet)
      const list = Object.values(decisions[entry.path] ?? {}).map(({ key: _key, preview: _preview, ...d }) => d);
      // Verschlüsselte Recover-Datei: Passwort einmal je Sitzung erfragen
      if (settings.writeRecover && settings.encryptRecover && !sessionPw.current) {
        const pw = await askText(t.setPassword, t.passwordPrompt, true);
        if (!pw) return null;
        sessionPw.current = pw;
      }
      try {
        const res = await api.applyFile(entry.path, rules, list, output, sessionPw.current ?? undefined);
        // Pseudonym-Speicher kann gewachsen sein: Vorschauen für Kategoriewechsel neu rechnen
        storeGen.current++;
        setFiles((fs) =>
          fs.map((f) => (f.path === entry.path ? { ...f, status: 'done', result: res, outputs: [...(f.outputs ?? []).filter((o) => o.output !== res.output), res] } : f))
        );
        return res.output;
      } catch (e) {
        fail(e);
        return null;
      }
    },
    [rules, settings, decisions, fail, ask, askText, t, targets]
  );

  const applySelected = useCallback(
    async (chooseTarget = false, ext?: string) => {
      const entry = files.find((f) => f.path === selected);
      if (!entry || !entry.analysis || busy) return;
      setBusy(true);
      const written = await applyOne(entry, chooseTarget, ext);
      setBusy(false);
      if (written) {
        showToast(ext ? t.exportedTo(`${baseName(entry.path)} → ${ext.toUpperCase()}`) : t.savedTo(baseName(written)));
        setTab('report');
      }
    },
    [files, selected, busy, applyOne, showToast, t]
  );

  const applyAll = async () => {
    if (busy) return;
    setBusy(true);
    let n = 0;
    lastError.current = null;
    for (const f of files) {
      // Während des Speicherns aus der Liste entfernt: nicht mehr schreiben
      if (!filesRef.current.some((x) => x.path === f.path)) continue;
      if (f.status === 'ready' || f.status === 'done') {
        if (await applyOne(f, false)) n++;
        // Passwortabfrage abgebrochen: nicht für jede weitere Datei erneut fragen
        else if (settings?.writeRecover && settings.encryptRecover && !sessionPw.current) break;
      }
    }
    setBusy(false);
    // Fehlgeschlagene Dateien: Grund nicht durch die Erfolgsmeldung überdecken
    if (lastError.current) showToast(`${t.savedMany(n)} ${lastError.current}`, true);
    else showToast(t.savedMany(n));
  };

  // --- Wiederherstellen ----------------------------------------------------------
  const recoverFlow = useCallback(() => setShowRecover(true), []);

  // --- Einstellungen -----------------------------------------------------------
  const saveSettings = (s: Settings) => {
    api
      .setSettings(s)
      .then(() => {
        setSettings(s);
        setSettingsBackup(null);
        setShowSettings(false);
        if (rules) reanalyzeAll(rules);
      })
      .catch(fail);
  };
  const toggleMasked = () => {
    if (!settings) return;
    saveSettings({ ...settings, masked: !settings.masked });
  };

  // --- Tastaturkürzel ------------------------------------------------------------
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if ((e.metaKey || e.ctrlKey) && e.key === ',') {
        e.preventDefault();
        if (settings) {
          setSettingsBackup(settings);
          setShowSettings(true);
        }
        return;
      }
      // Offener Dialog: Esc schließt nur den obersten (Rückfrage über Einstellungen schließt
      // nicht beide) — auch wenn der Fokus in einem Eingabefeld steht (Passwort, Einstellungen)
      if (showSettings || confirm || prompt || showRecover) {
        if (e.key === 'Escape') {
          if (confirm) settleConfirm('cancel');
          else if (prompt) settlePrompt(null);
          else if (showRecover) setShowRecover(false);
          else if (showSettings) {
            if (settingsBackup) setSettings(settingsBackup);
            setSettingsBackup(null);
            setShowSettings(false);
          }
        }
        return;
      }
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      // Handbuch/Tutorial offen: keine Aktionen dahinter auslösen (Esc schließt es selbst)
      if (document.querySelector('.hlp-overlay')) return;
      switch (e.key.length === 1 ? e.key.toLowerCase() : e.key) {
        case 'o':
          pickFiles();
          break;
        case 's':
          applySelected(false);
          break;
        case 'r':
          recoverFlow();
          break;
        case '1':
          setTab('findings');
          break;
        case '2':
          setTab('preview');
          break;
        case '3':
          setTab('rules');
          break;
        case '4':
          setTab('report');
          break;
        case '?':
          setHelpSignal((n) => n + 1);
          break;
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [settings, settingsBackup, showSettings, confirm, prompt, showRecover, pickFiles, applySelected, recoverFlow]);

  if (!settings || !rules) return null;

  const current = selected ? files.find((f) => f.path === selected) ?? null : null;
  const analysis = current?.analysis;
  const curDecisions: Record<number, Decision> = {};
  if (current) {
    const pv = catPreview[current.path] ?? {};
    for (const [id, d] of Object.entries(decisions[current.path] ?? {})) {
      const r = d.category && d.key ? pv[`${d.key}\u0000${d.category}`] : undefined;
      curDecisions[Number(id)] = r === undefined ? d : { ...d, preview: r };
    }
  }
  const accepted = analysis ? analysis.findings.filter((f) => effective(f, curDecisions[f.id]).accept).length : 0;

  const tabs: [Tab, string][] = [
    ['findings', t.tabFindings],
    ['preview', t.tabPreview],
    ['rules', t.tabRules],
    ['report', t.tabReport],
  ];

  return (
    <div className={`app ${dragging ? 'dragging' : ''}`}>
      <header className="header">
        <span className="brand">
          <span className="name">anonym</span>
          <span className="dot">.</span>
        </span>
        <span className="tagline">{t.tagline}</span>
        <span className="grow" />
        <button className={`ghost masked-toggle ${masked ? 'on' : ''}`} title={t.maskedHint} onClick={toggleMasked}>
          {masked ? <span className="badge">{t.maskedBadge}</span> : <span className="badge dimmed">masked</span>}
        </button>
        <button className="ghost" title={t.recoverHint} onClick={recoverFlow}>
          <IconRefresh size={12} /> {t.recover}
        </button>
        <button className="ghost icon hdr-help" title={t.help} onClick={() => setHelpSignal((n) => n + 1)}>
          ?
        </button>
        <button
          className="ghost icon"
          title={t.settings}
          onClick={() => {
            setSettingsBackup(settings);
            setShowSettings(true);
          }}
        >
          <IconGear size={14} />
        </button>
      </header>

      <div className="main">
        <Sidebar files={files} selected={selected} masked={masked} busy={busy} t={t} onSelect={setSelected} onAdd={pickFiles} onRemove={(p) => setFiles((fs) => fs.filter((f) => f.path !== p))} onClear={() => setFiles([])} onApplyAll={applyAll} />

        <div className="view">
          {files.length === 0 && tab !== 'rules' ? (
            <div className="onboard">
              <h2>{t.onboardTitle}</h2>
              <p>{t.onboardText}</p>
              <button className="primary" onClick={pickFiles}>
                {t.openFile}
              </button>
              <div className="note">{t.supported}</div>
              <button className="ghost" style={{ marginTop: 8 }} onClick={() => setTab('rules')}>
                {t.tabRules} →
              </button>
            </div>
          ) : (
            <>
              <div className="viewbar">
                {current ? (
                  <>
                    <h1 title={current.path}>{baseName(current.path)}</h1>
                    {analysis && (
                      <>
                        <span className="chip mini dim">{analysis.format}</span>
                        <span className="chip mini dim">{analysis.encoding}</span>
                        <span className="sub">
                          {analysis.lines} {t.lines} · {analysis.findings.length} {t.findings} · {accepted} {t.accepted}
                        </span>
                      </>
                    )}
                    {current.status === 'analyzing' && <span className="sub">{t.stAnalyzing}</span>}
                    {current.status === 'error' && <span className="chip mini bad">{errorText(lang, current.error)}</span>}
                  </>
                ) : (
                  <h1>{t.tabRules}</h1>
                )}
                <span className="grow" />
                <div className="tabs">
                  {tabs.map(([id, label]) => (
                    <button key={id} className={`chip ${tab === id ? 'active' : ''}`} onClick={() => setTab(id)}>
                      {label}
                    </button>
                  ))}
                </div>
              </div>
              <div className="viewbody">
                {tab === 'findings' && analysis && current && (
                  <>
                    {analysis.notes.length > 0 && (
                      <div className="notes">
                        {analysis.notes.map((n, i) => (
                          <div key={i} className="note">
                            // {noteText(t, n)}
                          </div>
                        ))}
                      </div>
                    )}
                    {analysis.findings.length === 0 ? (
                      <div className="empty">{t.noFindings}</div>
                    ) : (
                      <Findings findings={analysis.findings} decisions={curDecisions} t={t} onDecide={(ids, patch) => decide(current.path, ids, patch)} onAllow={addAllow} />
                    )}
                  </>
                )}
                {tab === 'preview' && analysis && current && <Preview text={analysis.text} findings={analysis.findings} decisions={curDecisions} t={t} onToggle={(id) => toggle(current.path, id)} />}
                {tab === 'rules' && <RulesView rules={rules} masked={masked} t={t} onChange={changeRules} onConfirm={onConfirm} onToast={showToast} onFail={fail} />}
                {tab === 'report' && <ReportView result={current?.result} outputs={current?.outputs ?? []} t={t} lang={lang} onOpen={(p) => api.openPath(p).catch(fail)} />}
                {(tab === 'findings' || tab === 'preview') && current && !analysis && current.status !== 'error' && <div className="empty">{t.stAnalyzing}</div>}
              </div>
              {current && analysis && tab !== 'rules' && (
                <div className="actionbar">
                  <button className="ghost" onClick={() => analyze(current.path, rules)}>
                    {t.reanalyze}
                  </button>
                  <span className="grow" />
                  {current.result && (
                    <span className="dim mono" title={current.result.output}>
                      {t.outputTo}: {baseName(current.result.output)}
                    </span>
                  )}
                  <span className="exportgroup" title={t.exportHint}>
                    <span className="lbl">{t.exportAs}</span>
                    <select value={exportExt} onChange={(e) => setExportExt(e.target.value)}>
                      {targets.map((x) => (
                        <option key={x.ext} value={x.ext}>
                          {x.label}
                        </option>
                      ))}
                    </select>
                    <button disabled={busy} onClick={() => applySelected(false, exportExt)}>
                      {t.exportBtn}
                    </button>
                  </span>
                  <button disabled={busy} onClick={() => applySelected(true)}>
                    {t.applyAs}
                  </button>
                  <button className="primary" disabled={busy} onClick={() => applySelected(false)}>
                    {busy ? t.applying : t.applyFile}
                  </button>
                </div>
              )}
            </>
          )}
        </div>
      </div>

      {dragging && <div className="dropzone">{t.dropHint}</div>}

      {showSettings && (
        <SettingsModal
          settings={settings}
          t={t}
          onClose={() => {
            if (settingsBackup) setSettings(settingsBackup);
            setSettingsBackup(null);
            setShowSettings(false);
          }}
          onSave={saveSettings}
          onLive={(s) => setSettings(s)}
          onConfirm={onConfirm}
          onStoreCleared={() => {
            // Angezeigte Pseudonyme stammen aus dem alten Speicher: neu analysieren
            storeGen.current++;
            reanalyzeAll(rules);
          }}
          onRulesReload={() => api.getRules().then((r) => {
            setRules(r);
            reanalyzeAll(r);
          }).catch(fail)}
          onToast={showToast}
          onFail={fail}
        />
      )}

      {prompt && <PromptModal title={prompt.title} label={prompt.label} repeatLabel={prompt.repeat ? t.passwordRepeat : undefined} mismatch={t.passwordMismatch} okLabel={t.ok2} cancelLabel={t.cancel} password onOk={(v) => settlePrompt(v)} onClose={() => settlePrompt(null)} />}

      {showRecover && (
        <RecoverModal
          targets={targets}
          sessionPassword={sessionPw.current}
          t={t}
          lang={lang}
          onClose={() => setShowRecover(false)}
          onDone={() => {}}
          onFail={fail}
        />
      )}

      {confirm && <ConfirmModal text={confirm.text} okLabel={confirm.ok ?? t.ok} cancelLabel={t.cancel} altLabel={confirm.alt} onOk={() => settleConfirm('ok')} onAlt={() => settleConfirm('alt')} onClose={() => settleConfirm('cancel')} />}

      {updateAvail && (
        <div className="upd-banner">
          <span>
            {t.updateBanner} <strong>{updateAvail.version}</strong>
          </span>
          <button
            className="primary"
            disabled={installing}
            onClick={() => {
              setInstalling(true);
              api.installUpdate().catch((e) => {
                setInstalling(false);
                fail(e);
              });
            }}
          >
            {installing ? t.updateInstalling : t.updateInstall}
          </button>
          <button className="ghost" onClick={() => setUpdateAvail(null)}>
            {t.updateLater}
          </button>
        </div>
      )}

      <Help lang={lang} openSignal={helpSignal} />
      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}
