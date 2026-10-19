import { useState } from 'react';
import { open, save } from '@tauri-apps/plugin-dialog';
import { CATEGORIES, Category, CategoryRule, Rules, STRATEGIES, Strategy, api } from '../api';
import { Dict } from '../i18n';
import { IconTrash } from '../icons';
import { categoryLabel, strategyLabel } from '../util';

const WORLDS = ['de', 'en', 'fr', 'es', 'tr', 'ja', 'fantasy', 'scifi', 'medieval'] as const;

export function RulesView({
  rules,
  masked,
  t,
  onChange,
  onConfirm,
  onToast,
  onFail,
}: {
  rules: Rules;
  masked: boolean;
  t: Dict;
  onChange: (r: Rules) => void;
  onConfirm: (text: string, ok?: string) => Promise<boolean>;
  onToast: (msg: string) => void;
  onFail: (e: unknown) => void;
}) {
  const [allow, setAllow] = useState(rules.allowlist.join('\n'));
  const [wordDrafts, setWordDrafts] = useState<Record<number, string>>({});

  const patch = (p: Partial<Rules>) => onChange({ ...rules, ...p });
  const patchCat = (c: Category, p: Partial<CategoryRule>) => patch({ categories: { ...rules.categories, [c]: { ...rules.categories[c], ...p } } });

  const commitAllow = () =>
    patch({
      allowlist: allow
        .split('\n')
        .map((s) => s.trim())
        .filter(Boolean),
    });

  const importRules = async () => {
    const sel = await open({ multiple: false, filters: [{ name: t.rulesFile, extensions: ['json'] }] });
    if (typeof sel !== 'string') return;
    api
      .importRules(sel)
      .then((r) => {
        onChange(r);
        setAllow(r.allowlist.join('\n'));
        onToast(t.rulesImported);
      })
      .catch(onFail);
  };
  const exportRules = async () => {
    const sel = await save({ defaultPath: `${rules.name || t.rulesDefaultFile}.anonym-rules.json`, filters: [{ name: t.rulesFile, extensions: ['json'] }] });
    if (!sel) return;
    api
      .exportRules(rules, sel)
      .then(() => onToast(t.rulesExported))
      .catch(onFail);
  };
  const reset = async () => {
    if (!(await onConfirm(t.rulesResetConfirm, t.rulesReset))) return;
    api
      .defaultRules()
      .then((r) => {
        onChange(r);
        setAllow('');
      })
      .catch(onFail);
  };
  // Neue Saat = neue Pseudonyme; ein gefüllter Pseudonym-Speicher passt danach nicht mehr
  const newSeed = async () => {
    const info = await api.storeInfo().catch(() => null);
    if (info && info.entries > 0 && !(await onConfirm(t.rulesNewSeedConfirm(info.entries), t.rulesNewSeed))) return;
    setSeedDraft(null);
    api
      .newSeed()
      .then((seed) => patch({ seed }))
      .catch(onFail);
  };
  // Saat von Hand: erst beim Verlassen des Felds (oder Enter) übernehmen und — wie „Neu“ —
  // bei gefülltem Speicher nachfragen; leer oder Abbrechen stellt die bisherige Saat wieder her
  const [seedDraft, setSeedDraft] = useState<string | null>(null);
  const commitSeed = async () => {
    if (seedDraft == null) return;
    const next = seedDraft.trim();
    if (!next || next === rules.seed) {
      setSeedDraft(null);
      return;
    }
    const info = await api.storeInfo().catch(() => null);
    const ok = !(info && info.entries > 0) || (await onConfirm(t.rulesSeedChangeConfirm(info.entries), t.rulesSeedChange));
    setSeedDraft(null);
    if (ok) patch({ seed: next });
  };
  const removePattern = async (i: number) => {
    const p = rules.customPatterns[i];
    if ((p.name || p.pattern) && !(await onConfirm(t.removeEntryConfirm(p.name || p.pattern), t.remove))) return;
    patch({ customPatterns: rules.customPatterns.filter((_, j) => j !== i) });
  };
  const removeWords = async (i: number) => {
    const w = rules.customWords[i];
    if ((w.name || w.words.length) && !(await onConfirm(t.removeEntryConfirm(w.name || w.words[0]), t.remove))) return;
    patch({ customWords: rules.customWords.filter((_, j) => j !== i) });
  };

  return (
    <div className="rules">
      <div className="row3">
        <label className="field grow1">
          <span>{t.rulesName}</span>
          <input type="text" value={rules.name} onChange={(e) => patch({ name: e.target.value })} />
        </label>
        <label className="field grow1">
          <span>{t.rulesSeed}</span>
          <div className="pickrow" style={{ margin: 0 }}>
            <input
              type="text"
              value={seedDraft ?? rules.seed}
              spellCheck={false}
              onChange={(e) => setSeedDraft(e.target.value)}
              onBlur={commitSeed}
              onKeyDown={(e) => {
                if (e.key === 'Enter') e.currentTarget.blur();
                if (e.key === 'Escape') setSeedDraft(null);
              }}
            />
            <button title={t.rulesNewSeedHint} onClick={newSeed}>
              {t.rulesNewSeed}
            </button>
          </div>
        </label>
        <label className="field grow1">
          <span>{t.rulesWorld}</span>
          <select value={rules.world} onChange={(e) => patch({ world: e.target.value })}>
            {WORLDS.map((w) => (
              <option key={w} value={w}>
                {t.world[w]}
              </option>
            ))}
          </select>
        </label>
      </div>
      <div className="note">{t.rulesSeedHint}</div>
      {rules.seed === 'anonym' && <div className="note">{t.rulesSeedWeak}</div>}
      <div className="row3">
        <label className="field grow1">
          <span>{t.rulesDateShift}</span>
          <div className="pickrow" style={{ margin: 0 }}>
            <input type="number" value={rules.dateShiftDays} min={-3650} max={3650} onChange={(e) => patch({ dateShiftDays: Number(e.target.value) || 0 })} />
            <span className="dim">{rules.dateShiftDays === 0 ? t.auto : t.days}</span>
          </div>
        </label>
        <label className="check grow1" style={{ alignSelf: 'end', marginBottom: 14 }}>
          <input type="checkbox" checked={rules.columnTyping} onChange={(e) => patch({ columnTyping: e.target.checked })} />
          {t.rulesColumnTyping}
        </label>
      </div>
      <div className="note">{t.rulesDateShiftHint}</div>

      <h3 className="rh">{t.rulesCategories}</h3>
      <div className="note" style={{ marginTop: 0 }}>
        {t.rulesCategoriesHint}
      </div>
      <table className="rtable cattable">
        <thead>
          <tr>
            <th>{t.colEnabled}</th>
            <th>{t.colCategory}</th>
            <th>{t.colStrategy}</th>
            <th>{t.colTuning}</th>
          </tr>
        </thead>
        <tbody>
          {CATEGORIES.map((c) => {
            const r = rules.categories[c];
            return (
              <tr key={c} className={r.enabled ? '' : 'rejected'}>
                <td>
                  <input type="checkbox" checked={r.enabled} onChange={(e) => patchCat(c, { enabled: e.target.checked })} />
                </td>
                <td>
                  <i className="dot" style={{ background: `var(--cat-${c})` }} /> {categoryLabel(t, c)}
                </td>
                <td>
                  <select value={r.strategy} title={t.stratHint[r.strategy]} onChange={(e) => patchCat(c, { strategy: e.target.value as Strategy })}>
                    {STRATEGIES.map((s) => (
                      <option key={s} value={s}>
                        {strategyLabel(t, s)}
                      </option>
                    ))}
                  </select>
                </td>
                <td className="tuning">
                  {/* Flex nur innen: ein flex-<td> fällt aus dem Tabellenlayout (Zeilenlinien versetzt) */}
                  <div className="tun">
                    {r.strategy === 'redact' && (
                      <>
                        <input type="text" className="ch" maxLength={1} value={r.redactChar} disabled={!masked} title={t.redactChar} onChange={(e) => patchCat(c, { redactChar: e.target.value || '█' })} />
                        <label className="check" style={{ margin: 0 }}>
                          <input type="checkbox" checked={r.keepLength} disabled={!masked} onChange={(e) => patchCat(c, { keepLength: e.target.checked })} />
                          {t.keepLength}
                        </label>
                      </>
                    )}
                    {r.strategy === 'mask' && (
                      <>
                        <input type="text" className="ch" maxLength={1} value={r.maskChar} disabled={!masked} title={t.maskChar} onChange={(e) => patchCat(c, { maskChar: e.target.value || 'X' })} />
                        <span className="dim">{t.keepLast}</span>
                        <input type="number" className="n" min={0} max={20} value={r.keepLast} disabled={!masked} onChange={(e) => patchCat(c, { keepLast: Math.max(0, Number(e.target.value) || 0) })} />
                      </>
                    )}
                    {r.strategy === 'placeholder' && (
                      <input type="text" className="tpl" value={r.placeholder} disabled={!masked} title={t.placeholderTplHint} onChange={(e) => patchCat(c, { placeholder: e.target.value })} />
                    )}
                  </div>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
      {!masked && <div className="note">{t.tuningLocked}</div>}

      <h3 className="rh">{t.rulesAllowlist}</h3>
      <div className="note" style={{ marginTop: 0 }}>
        {t.rulesAllowlistHint}
      </div>
      <textarea className="mono" rows={4} value={allow} onChange={(e) => setAllow(e.target.value)} onBlur={commitAllow} />

      <h3 className="rh">
        {t.rulesCustomPatterns} {!masked && <span className="badge dimmed">masked</span>}
      </h3>
      <div className="note" style={{ marginTop: 0 }}>
        {t.rulesCustomPatternsHint}
      </div>
      {rules.customPatterns.map((p, i) => (
        <div key={i} className="cprow">
          <input type="text" placeholder={t.patternName} value={p.name} disabled={!masked} onChange={(e) => patch({ customPatterns: rules.customPatterns.map((x, j) => (j === i ? { ...x, name: e.target.value } : x)) })} />
          <input type="text" className="mono grow1" placeholder={t.pattern} value={p.pattern} disabled={!masked} onChange={(e) => patch({ customPatterns: rules.customPatterns.map((x, j) => (j === i ? { ...x, pattern: e.target.value } : x)) })} />
          <select value={p.category} disabled={!masked} onChange={(e) => patch({ customPatterns: rules.customPatterns.map((x, j) => (j === i ? { ...x, category: e.target.value as Category } : x)) })}>
            {CATEGORIES.map((c) => (
              <option key={c} value={c}>
                {categoryLabel(t, c)}
              </option>
            ))}
          </select>
          <button className="icon ghost" disabled={!masked} onClick={() => removePattern(i)}>
            <IconTrash size={11} />
          </button>
        </div>
      ))}
      <button disabled={!masked} onClick={() => patch({ customPatterns: [...rules.customPatterns, { name: '', pattern: '', category: 'customer-id' }] })}>
        {t.addPattern}
      </button>

      <h3 className="rh">
        {t.rulesCustomWords} {!masked && <span className="badge dimmed">masked</span>}
      </h3>
      <div className="note" style={{ marginTop: 0 }}>
        {t.rulesCustomWordsHint}
      </div>
      {rules.customWords.map((w, i) => (
        <div key={i} className="cwrow">
          <div className="cprow">
            <input type="text" placeholder={t.listName} value={w.name} disabled={!masked} onChange={(e) => patch({ customWords: rules.customWords.map((x, j) => (j === i ? { ...x, name: e.target.value } : x)) })} />
            <select value={w.category} disabled={!masked} onChange={(e) => patch({ customWords: rules.customWords.map((x, j) => (j === i ? { ...x, category: e.target.value as Category } : x)) })}>
              {CATEGORIES.map((c) => (
                <option key={c} value={c}>
                  {categoryLabel(t, c)}
                </option>
              ))}
            </select>
            <button className="icon ghost" disabled={!masked} onClick={() => removeWords(i)}>
              <IconTrash size={11} />
            </button>
          </div>
          <textarea
            className="mono"
            rows={3}
            disabled={!masked}
            value={wordDrafts[i] ?? w.words.join('\n')}
            onChange={(e) => setWordDrafts({ ...wordDrafts, [i]: e.target.value })}
            onBlur={() => {
              const d = wordDrafts[i];
              if (d == null) return;
              patch({ customWords: rules.customWords.map((x, j) => (j === i ? { ...x, words: d.split('\n').map((s) => s.trim()).filter(Boolean) } : x)) });
              const next = { ...wordDrafts };
              delete next[i];
              setWordDrafts(next);
            }}
          />
        </div>
      ))}
      <button disabled={!masked} onClick={() => patch({ customWords: [...rules.customWords, { name: '', category: 'person', words: [] }] })}>
        {t.addWordList}
      </button>
      {!masked && <div className="note">{t.customLocked}</div>}

      <div className="sep" />
      <div className="btnrow" style={{ justifyContent: 'flex-start' }}>
        <button onClick={importRules}>{t.rulesImport}</button>
        <button onClick={exportRules}>{t.rulesExport}</button>
        <button className="ghost" onClick={reset}>
          {t.rulesReset}
        </button>
      </div>
    </div>
  );
}
