import { useEffect, useState } from 'react';
import { open, save } from '@tauri-apps/plugin-dialog';
import { ExportTarget, FILE_EXTENSIONS, RecoverInfo, RecoverResult, api } from '../api';
import { Dict } from '../i18n';
import { baseName, dirName } from '../util';

/** Wiederherstellen: bearbeitete anonymisierte Datei + Recover-Datei → Ausgabe in wählbarem Format. */
export function RecoverModal({
  targets,
  sessionPassword,
  t,
  onClose,
  onDone,
  onFail,
}: {
  targets: ExportTarget[];
  sessionPassword: string | null;
  t: Dict;
  onClose: () => void;
  onDone: (r: RecoverResult) => void;
  onFail: (e: unknown) => void;
}) {
  const [file, setFile] = useState('');
  const [key, setKey] = useState('');
  const [info, setInfo] = useState<RecoverInfo | null>(null);
  const [password, setPassword] = useState(sessionPassword ?? '');
  const [format, setFormat] = useState('');
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<RecoverResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const srcExt = file.split('.').pop()?.toLowerCase() ?? '';

  // Recover-Datei neben der gewählten Datei vorschlagen
  useEffect(() => {
    if (!file || key) return;
    api
      .suggestRecoverKey(file)
      .then(async (guess) => {
        if (await api.pathExists(guess)) setKey(guess);
      })
      .catch(() => {});
  }, [file, key]);

  useEffect(() => {
    if (!key) {
      setInfo(null);
      return;
    }
    api
      .recoverInfo(key)
      .then((i) => {
        setInfo(i);
        setError(null);
      })
      .catch((e) => {
        setInfo(null);
        setError(String(e));
      });
  }, [key]);

  const pickFile = async () => {
    const sel = await open({ multiple: false, title: t.recoverPickFile, filters: [{ name: t.allFiles, extensions: FILE_EXTENSIONS }] });
    if (typeof sel === 'string') {
      setFile(sel);
      setKey('');
      setResult(null);
    }
  };
  const pickKey = async () => {
    const sel = await open({ multiple: false, title: t.recoverPickKey, defaultPath: file ? dirName(file) : undefined, filters: [{ name: t.recoverKeyFile, extensions: ['json'] }] });
    if (typeof sel === 'string') setKey(sel);
  };

  const run = async () => {
    if (!file || !key || busy) return;
    const ext = format || undefined;
    const suggested = await api.suggestRecovered(file, ext);
    const outExt = ext ?? srcExt;
    const all = [
      ...(srcExt ? [{ name: `${srcExt.toUpperCase()} (${t.recoverSame})`, extensions: [srcExt] }] : []),
      ...targets.filter((x) => x.ext !== srcExt).map((x) => ({ name: x.label, extensions: [x.ext] })),
    ];
    const filters = [...all.filter((f) => f.extensions[0] === outExt), ...all.filter((f) => f.extensions[0] !== outExt)];
    const output = await save({ defaultPath: suggested, filters });
    if (!output) return;
    setBusy(true);
    setError(null);
    try {
      const r = await api.recoverFile(file, key, info?.encrypted ? password : null, output);
      setResult(r);
      onDone(r);
    } catch (e) {
      const msg = String(e);
      setError(msg.includes('PASSWORD_WRONG') ? t.recoverPasswordWrong : msg.includes('PASSWORD_REQUIRED') ? t.recoverPassword : msg);
      if (!msg.includes('PASSWORD')) onFail(e);
    } finally {
      setBusy(false);
    }
  };

  const ready = !!file && !!key && !!info && (!info.encrypted || password.length > 0);

  return (
    <div className="overlay" onClick={onClose}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <h2>{t.recoverTitle}</h2>
        <div className="note" style={{ marginTop: 0 }}>
          {t.recoverHint}
        </div>

        <div className="field">
          <span className="fieldlabel">{t.recoverPickFile}</span>
          <div className="pickrow">
            <span className={`pickval mono ${file ? '' : 'dim'}`} title={file}>
              {file ? baseName(file) : '—'}
            </span>
            <button onClick={pickFile}>{t.pickFile}</button>
          </div>
          <div className="note" style={{ margin: '0 0 8px' }}>
            {t.recoverPickFileHint}
          </div>
        </div>

        <div className="field">
          <span className="fieldlabel">{t.recoverPickKey}</span>
          <div className="pickrow">
            <span className={`pickval mono ${key ? '' : 'dim'}`} title={key}>
              {key ? baseName(key) : '—'}
            </span>
            <button onClick={pickKey}>{t.pickFile}</button>
          </div>
          <div className="note" style={{ margin: '0 0 8px' }}>
            {info
              ? info.encrypted
                ? `// ${t.recoverEncrypted}`
                : `// ${t.recoverEntries(info.entries)}${info.source ? ` · ${baseName(info.source)} → ${baseName(info.output ?? '')}` : ''}`
              : t.recoverKeyHint}
          </div>
        </div>

        {info?.encrypted && (
          <label className="field">
            <span>{t.recoverPassword}</span>
            <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && ready && run()} />
          </label>
        )}

        <label className="field">
          <span>{t.recoverFormat}</span>
          <select value={format} onChange={(e) => setFormat(e.target.value)}>
            <option value="">{srcExt ? `${srcExt.toUpperCase()} — ${t.recoverSame}` : t.recoverSame}</option>
            {targets
              .filter((x) => x.ext !== srcExt)
              .map((x) => (
                <option key={x.ext} value={x.ext}>
                  {x.label}
                </option>
              ))}
          </select>
        </label>

        {error && <div className="chip mini bad" style={{ display: 'inline-block', marginBottom: 8 }}>{error}</div>}

        {result && (
          <>
            <div className="sep" />
            <div className="kpis" style={{ gridTemplateColumns: 'repeat(3, 1fr)', margin: '0 0 10px' }}>
              <div className="kpi">
                <div className="k">{t.recoverRestored}</div>
                <div className="v">{result.restored}</div>
              </div>
              <div className="kpi">
                <div className="k">{t.recoverNotFound}</div>
                <div className="v" style={{ color: 'var(--text-dim)' }}>
                  {result.notFound}
                </div>
              </div>
              <div className="kpi">
                <div className="k">{t.recoverAmbiguous}</div>
                <div className="v" style={{ color: 'var(--text-dim)' }}>
                  {result.ambiguous}
                </div>
              </div>
            </div>
            <div className="pathrow">
              <span className="chip mini dim">{result.format}</span>
              <span className="mono">{baseName(result.output)}</span>
              <button onClick={() => api.openPath(dirName(result.output)).catch(onFail)}>{t.reportOpenFolder}</button>
            </div>
          </>
        )}

        <div className="btnrow">
          <button className="ghost" onClick={onClose}>
            {result ? t.close : t.cancel}
          </button>
          <button className="primary" disabled={!ready || busy} onClick={run}>
            {busy ? t.applying : t.recoverRun}
          </button>
        </div>
      </div>
    </div>
  );
}
