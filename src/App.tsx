import { useCallback, useEffect, useRef, useState } from 'react';
import { open, save } from '@tauri-apps/plugin-dialog';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { Decision, ExportTarget, FILE_EXTENSIONS, Rules, Settings, UpdateInfo, api, defaultSettings } from './api';
import { dicts, Lang } from './i18n';
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

/** Texteingabe (Passwort) als Modal. */
function PromptModal({ title, label, okLabel, cancelLabel, password, onOk, onClose }: { title: string; label: string; okLabel: string; cancelLabel: string; password?: boolean; onOk: (v: string) => void; onClose: () => void }) {
  const [v, setV] = useState('');
  return (
    <div className="overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>{title}</h2>
        <label className="field">
          <span>{label}</span>
          <input type={password ? 'password' : 'text'} autoFocus value={v} onChange={(e) => setV(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && v && onOk(v)} />
        </label>
        <div className="btnrow">
          <button className="ghost" onClick={onClose}>
            {cancelLabel}
          </button>
          <button className="primary" disabled={!v} onClick={() => onOk(v)}>
            {okLabel}
          </button>
        </div>
      </div>
    </div>
  );
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
  const [prompt, setPrompt] = useState<{ title: string; label: string } | null>(null);
  const [showRecover, setShowRecover] = useState(false);
  /** Sitzungs-Passwort für Recover-Dateien (nie gespeichert). */
  const sessionPw = useRef<string | null>(null);
  const toastTimer = useRef<number | undefined>(undefined);
  const rulesTimer = useRef<number | undefined>(undefined);
  const rulesRef = useRef<Rules | null>(null);
  rulesRef.current = rules;

  const lang: Lang = settings?.language ?? 'de';
  const t = dicts[lang];
  const masked = settings?.masked ?? false;

  const showToast = useCallback((msg: string, isError = false) => {
    setToast(msg);
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), isError ? 6000 : 2400);
  }, []);
  const fail = useCallback((e: unknown) => showToast(String(e), true), [showToast]);

  // Bestätigung als Promise: 'ok' | 'alt' (dritte Schaltfläche) | 'cancel'
  type Answer = 'ok' | 'alt' | 'cancel';
  const confirmResolve = useRef<((a: Answer) => void) | null>(null);
  const ask = useCallback(
    (text: string, ok?: string, alt?: string) =>
      new Promise<Answer>((resolve) => {
        confirmResolve.current = resolve;
        setConfirm({ text, ok, alt });
      }),
    []
  );
  const onConfirm = useCallback((text: string, ok?: string) => ask(text, ok).then((a) => a === 'ok'), [ask]);

  // Texteingabe als Promise (null = abgebrochen)
  const promptResolve = useRef<((v: string | null) => void) | null>(null);
  const askText = useCallback(
    (title: string, label: string) =>
      new Promise<string | null>((resolve) => {
        promptResolve.current = resolve;
        setPrompt({ title, label });
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
      api
        .analyzeFile(path, r)
        .then((a) => {
          setFiles((fs) => fs.map((f) => (f.path === path ? { ...f, status: 'ready', analysis: a } : f)));
          // Entscheidungen behalten, deren Fundstelle (id + Text) noch existiert
          setDecisions((d) => {
            const old = d[path] ?? {};
            const next: Record<number, Decision> = {};
            for (const f of a.findings) {
              const prev = old[f.id];
              if (prev) next[f.id] = prev;
            }
            return { ...d, [path]: next };
          });
        })
        .catch((e) => setFiles((fs) => fs.map((f) => (f.path === path ? { ...f, status: 'error', error: String(e) } : f))));
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
              api.installUpdate().catch(() => setInstalling(false));
            }
          })
          .catch(() => {});
      })
      .catch(() => setSettings({ ...defaultSettings }));
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
      setFiles((fs) => {
        for (const f of fs) if (f.status !== 'idle') analyze(f.path, r);
        return fs;
      });
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
    setDecisions((d) => {
      const cur = { ...(d[path] ?? {}) };
      for (const id of ids) {
        if (patch === null) {
          delete cur[id];
          continue;
        }
        const prev = cur[id] ?? { id, accept: true, category: null, replacement: null };
        cur[id] = { ...prev, ...patch, id };
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

  // --- Speichern ---------------------------------------------------------------
  /** Speichern: im Ursprungsformat (ext = undefined), als Export (ext) oder per Dialog (chooseTarget). */
  const applyOne = useCallback(
    async (entry: FileEntry, chooseTarget: boolean, ext?: string): Promise<boolean> => {
      if (!rules || !settings || !entry.analysis) return false;
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
        if (!sel) return false;
        output = sel;
      } else if (settings.confirmOverwrite && (await api.pathExists(output))) {
        const a = await ask(t.confirmOverwriteText(baseName(output)), t.overwrite, t.otherName);
        if (a === 'cancel') return false;
        if (a === 'alt') {
          const sel = await pickPath();
          if (!sel) return false;
          output = sel;
        }
      }
      const list = Object.values(decisions[entry.path] ?? {});
      // Verschlüsselte Recover-Datei: Passwort einmal je Sitzung erfragen
      if (settings.writeRecover && settings.encryptRecover && !sessionPw.current) {
        const pw = await askText(t.setPassword, t.passwordPrompt);
        if (!pw) return false;
        sessionPw.current = pw;
      }
      try {
        const res = await api.applyFile(entry.path, rules, list, output, sessionPw.current ?? undefined);
        setFiles((fs) =>
          fs.map((f) => (f.path === entry.path ? { ...f, status: 'done', result: res, outputs: [...(f.outputs ?? []).filter((o) => o.output !== res.output), res] } : f))
        );
        return true;
      } catch (e) {
        fail(e);
        return false;
      }
    },
    [rules, settings, decisions, fail, ask, askText, t, targets]
  );

  const applySelected = useCallback(
    async (chooseTarget = false, ext?: string) => {
      const entry = files.find((f) => f.path === selected);
      if (!entry || !entry.analysis || busy) return;
      setBusy(true);
      const ok = await applyOne(entry, chooseTarget, ext);
      setBusy(false);
      if (ok) {
        showToast(ext ? t.exportedTo(`${baseName(entry.path)} → ${ext.toUpperCase()}`) : t.savedTo(baseName(entry.path)));
        setTab('report');
      }
    },
    [files, selected, busy, applyOne, showToast, t]
  );

  const applyAll = async () => {
    if (busy) return;
    setBusy(true);
    let n = 0;
    for (const f of files) {
      if (f.status === 'ready' || f.status === 'done') {
        if (await applyOne(f, false)) n++;
      }
    }
    setBusy(false);
    showToast(t.savedMany(n));
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
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (showSettings || confirm || prompt || showRecover) {
        if (e.key === 'Escape') {
          if (showSettings) {
            if (settingsBackup) setSettings(settingsBackup);
            setSettingsBackup(null);
            setShowSettings(false);
          }
          if (confirm) settleConfirm('cancel');
          if (prompt) settlePrompt(null);
          if (showRecover) setShowRecover(false);
        }
        return;
      }
      switch (e.key) {
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
  const curDecisions = current ? decisions[current.path] ?? {} : {};
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
                    {current.status === 'error' && <span className="chip mini bad">{current.error}</span>}
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
                            // {n}
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
                {tab === 'rules' && <RulesView rules={rules} masked={masked} t={t} onChange={changeRules} onToast={showToast} onFail={fail} />}
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
          onToast={showToast}
          onFail={fail}
        />
      )}

      {prompt && <PromptModal title={prompt.title} label={prompt.label} okLabel={t.ok2} cancelLabel={t.cancel} password onOk={(v) => settlePrompt(v)} onClose={() => settlePrompt(null)} />}

      {showRecover && (
        <RecoverModal
          targets={targets}
          sessionPassword={sessionPw.current}
          t={t}
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
              api.installUpdate().catch(() => setInstalling(false));
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
