import { useEffect, useState } from 'react';
import { open, save } from '@tauri-apps/plugin-dialog';
import { FALLBACK_ENCODINGS, Settings, StoreInfo, UpdateInfo, api } from '../api';
import { Dict } from '../i18n';
import { IconExternalLink } from '../icons';

export const APP_VERSION = '0.1.0';

type SetTab = 'general' | 'files' | 'store' | 'app';

const WEBSITE = 'https://lan-solo.com/de/tools/anonym/';
const GITHUB = 'https://github.com/LAN-SOLO/anonym';

export function SettingsModal({
  settings,
  t,
  onClose,
  onSave,
  onLive,
  onConfirm,
  onToast,
  onFail,
}: {
  settings: Settings;
  t: Dict;
  onClose: () => void;
  onSave: (s: Settings) => void;
  onLive: (s: Settings) => void;
  onConfirm: (text: string) => Promise<boolean>;
  onToast: (msg: string) => void;
  onFail: (e: unknown) => void;
}) {
  const [tab, setTab] = useState<SetTab>('general');
  const [s, setS] = useState<Settings>({ ...settings });
  const [updState, setUpdState] = useState<'idle' | 'checking' | 'none' | 'error'>('idle');
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [installing, setInstalling] = useState(false);
  const [dataPath, setDataPath] = useState('');
  const [store, setStore] = useState<StoreInfo | null>(null);

  useEffect(() => {
    api.dataPath().then(setDataPath).catch(() => {});
    api.storeInfo().then(setStore).catch(() => {});
  }, []);

  const set = <K extends keyof Settings>(key: K, value: Settings[K]) =>
    setS((prev) => {
      const next = { ...prev, [key]: value };
      onLive(next);
      return next;
    });

  const pickDir = async () => {
    const sel = await open({ directory: true, multiple: false });
    if (typeof sel === 'string') set('outputDir', sel);
  };

  const checkUpdates = () => {
    setUpdState('checking');
    setUpdate(null);
    api
      .checkUpdate()
      .then((u) => {
        if (u) {
          setUpdate(u);
          setUpdState('idle');
        } else setUpdState('none');
      })
      .catch(() => setUpdState('error'));
  };

  const clearStore = async () => {
    if (!(await onConfirm(t.storeClearConfirm))) return;
    api
      .storeClear()
      .then(() => {
        onToast(t.storeCleared);
        return api.storeInfo().then(setStore);
      })
      .catch(onFail);
  };
  const exportStore = async () => {
    const sel = await save({ defaultPath: 'anonym-schluessel.json', filters: [{ name: 'JSON', extensions: ['json'] }] });
    if (!sel) return;
    api.storeExport(sel).then(() => onToast(t.saved)).catch(onFail);
  };
  const importStore = async () => {
    const sel = await open({ multiple: false, filters: [{ name: 'JSON', extensions: ['json'] }] });
    if (typeof sel !== 'string') return;
    api.storeImport(sel).then(setStore).catch(onFail);
  };

  const tabs: [SetTab, string][] = [
    ['general', t.setGeneral],
    ['files', t.setFiles],
    ['store', t.setStore],
    ['app', t.setApp],
  ];

  return (
    <div className="overlay" onClick={onClose}>
      <div className="modal settings" onClick={(e) => e.stopPropagation()}>
        <h2>{t.settings}</h2>
        <div className="set-tabs">
          {tabs.map(([id, label]) => (
            <button key={id} className={`chip ${tab === id ? 'active' : ''}`} onClick={() => setTab(id)}>
              {label}
            </button>
          ))}
        </div>

        {tab === 'general' && (
          <>
            <div className="row3">
              <label className="field grow1">
                <span>{t.language}</span>
                <select value={s.language} onChange={(e) => set('language', e.target.value as Settings['language'])}>
                  <option value="de">Deutsch</option>
                  <option value="en">English</option>
                </select>
              </label>
              <label className="field grow1">
                <span>{t.theme}</span>
                <select value={s.theme} onChange={(e) => set('theme', e.target.value as Settings['theme'])}>
                  <option value="dark">{t.themeDark}</option>
                  <option value="light">{t.themeLight}</option>
                </select>
              </label>
              <label className="field grow1">
                <span>{t.accent}</span>
                <select value={s.accent} onChange={(e) => set('accent', e.target.value as Settings['accent'])}>
                  <option value="blue">{t.accentBlue}</option>
                  <option value="emerald">{t.accentEmerald}</option>
                  <option value="violet">{t.accentViolet}</option>
                  <option value="amber">{t.accentAmber}</option>
                </select>
              </label>
            </div>
            <label className="check">
              <input type="checkbox" checked={s.confirmOverwrite} onChange={(e) => set('confirmOverwrite', e.target.checked)} />
              {t.confirmOverwrite}
            </label>
            <div className="sep" />
            <label className="check">
              <input type="checkbox" checked={s.masked} onChange={(e) => set('masked', e.target.checked)} />
              {t.maskedToggle} <span className="badge">masked</span>
            </label>
            <div className="note">{t.maskedExplain}</div>
            <div className="note">{t.maskedHint}</div>
          </>
        )}

        {tab === 'files' && (
          <>
            <div className="field">
              <span className="fieldlabel" style={{ marginTop: 0 }}>
                {t.outputDir}
              </span>
              <div className="pickrow">
                <input type="text" value={s.outputDir} placeholder={t.outputDirHint} onChange={(e) => set('outputDir', e.target.value)} />
                <button onClick={pickDir}>{t.pickFolder}</button>
                {s.outputDir && (
                  <button className="ghost" onClick={() => set('outputDir', '')}>
                    ×
                  </button>
                )}
              </div>
            </div>
            <div className="row3">
              <label className="field grow1">
                <span>{t.outputSuffix}</span>
                <input type="text" value={s.outputSuffix} onChange={(e) => set('outputSuffix', e.target.value)} />
              </label>
              <label className="field grow1">
                <span>{t.fallbackEncoding}</span>
                <select value={s.fallbackEncoding} disabled={!s.masked} onChange={(e) => set('fallbackEncoding', e.target.value)}>
                  {FALLBACK_ENCODINGS.map((e) => (
                    <option key={e} value={e}>
                      {e}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <div className="note">{t.outputSuffixHint.replace('{suffix}', s.outputSuffix || '.anonym')}</div>
            <div className="note">{t.fallbackEncodingHint}</div>
            <label className="check">
              <input type="checkbox" checked={s.writeReport} onChange={(e) => set('writeReport', e.target.checked)} />
              {t.writeReport}
            </label>
          </>
        )}

        {tab === 'store' && (
          <>
            <label className="check">
              <input type="checkbox" checked={s.useStore} disabled={!s.masked} onChange={(e) => set('useStore', e.target.checked)} />
              {t.useStore} <span className="badge">masked</span>
            </label>
            <div className="note">{t.useStoreHint}</div>
            {!s.masked && <div className="note">{t.storeLocked}</div>}
            {store && (
              <div className="storebox">
                <div>
                  <span className="dim">{t.storeEntries}:</span> {store.entries}
                  {store.seed && (
                    <>
                      {' · '}
                      <span className="dim">{t.storeSeed}:</span> {store.seed} · {store.world}
                    </>
                  )}
                </div>
                <div className="mono dim" style={{ fontSize: 11, marginTop: 4 }}>
                  {t.storePath}: {store.path}
                </div>
              </div>
            )}
            <div className="btnrow" style={{ justifyContent: 'flex-start' }}>
              <button disabled={!s.masked} onClick={exportStore}>
                {t.storeExport}
              </button>
              <button disabled={!s.masked} onClick={importStore}>
                {t.storeImport}
              </button>
              <button className="danger" disabled={!s.masked || !store || store.entries === 0} onClick={clearStore}>
                {t.storeClear}
              </button>
            </div>
          </>
        )}

        {tab === 'app' && (
          <>
            <div className="updatebox">
              <span>
                anonym {t.version} {APP_VERSION}
              </span>
              <button onClick={checkUpdates} disabled={updState === 'checking'}>
                {updState === 'checking' ? t.checking : t.checkUpdates}
              </button>
              {updState === 'none' && <span className="dim">{t.noUpdate}</span>}
              {updState === 'error' && <span className="dim">{t.updateError}</span>}
              {update && (
                <>
                  <span>
                    {t.updateAvailable} <strong>{update.version}</strong>
                  </span>
                  <button
                    className="primary"
                    disabled={installing}
                    onClick={() => {
                      setInstalling(true);
                      api.installUpdate().catch(() => setInstalling(false));
                    }}
                  >
                    {installing ? t.updateInstalling : t.installUpdate}
                  </button>
                </>
              )}
            </div>
            <label className="check" style={{ marginTop: 12 }}>
              <input type="checkbox" checked={s.autoUpdate} onChange={(e) => set('autoUpdate', e.target.checked)} />
              {t.autoUpdate}
            </label>
            <div className="note mono">
              {t.dataFolder}: {dataPath}
            </div>
            <div className="sep" />
            <div className="btnrow" style={{ justifyContent: 'flex-start' }}>
              <button onClick={() => api.openPath(WEBSITE).catch(onFail)}>
                <IconExternalLink size={11} /> {t.website}
              </button>
              <button onClick={() => api.openPath(GITHUB).catch(onFail)}>
                <IconExternalLink size={11} /> {t.github}
              </button>
            </div>
            <div className="note">{t.licenseNote}</div>
            <div className="note">{t.shortcuts}</div>
          </>
        )}

        <div className="btnrow">
          <button className="ghost" onClick={onClose}>
            {t.cancel}
          </button>
          <button className="primary" onClick={() => onSave(s)}>
            {t.saveSettings}
          </button>
        </div>
      </div>
    </div>
  );
}
