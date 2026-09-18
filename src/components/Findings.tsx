import { useMemo, useState } from 'react';
import { CATEGORIES, Category, Decision, Finding } from '../api';
import { Dict } from '../i18n';
import { IconCheck, IconRefresh, IconShield, IconX } from '../icons';
import { categoryLabel, countBy, effective, sourceLabel } from '../util';

export function Findings({
  findings,
  decisions,
  t,
  onDecide,
  onAllow,
}: {
  findings: Finding[];
  decisions: Record<number, Decision>;
  t: Dict;
  onDecide: (ids: number[], patch: Partial<Decision> | null) => void;
  onAllow: (text: string) => void;
}) {
  const [filter, setFilter] = useState<Category | 'all'>('all');
  const [editing, setEditing] = useState<number | null>(null);
  const [draft, setDraft] = useState('');

  const counts = useMemo(() => countBy(findings.map((f) => f.category)), [findings]);
  const list = useMemo(() => (filter === 'all' ? findings : findings.filter((f) => effective(f, decisions[f.id]).category === filter)), [findings, filter, decisions]);
  const occurrences = useMemo(() => countBy(findings.map((f) => `${f.category}|${f.text.toLowerCase()}`)), [findings]);

  const startEdit = (f: Finding) => {
    setEditing(f.id);
    setDraft(effective(f, decisions[f.id]).replacement);
  };
  const commitEdit = (f: Finding) => {
    if (editing !== f.id) return;
    const value = draft;
    setEditing(null);
    if (value === f.replacement) onDecide([f.id], { replacement: null });
    else onDecide([f.id], { replacement: value, accept: true });
  };

  const visibleIds = list.map((f) => f.id);
  const accepted = findings.filter((f) => effective(f, decisions[f.id]).accept).length;

  return (
    <div className="findings">
      <div className="filters">
        <button className={`chip ${filter === 'all' ? 'active' : ''}`} onClick={() => setFilter('all')}>
          {t.filterAll} <span className="n">{findings.length}</span>
        </button>
        {CATEGORIES.filter((c) => counts[c]).map((c) => (
          <button key={c} className={`chip ${filter === c ? 'active' : ''}`} onClick={() => setFilter(c)}>
            <i className="dot" style={{ background: `var(--cat-${c})` }} />
            {categoryLabel(t, c)} <span className="n">{counts[c]}</span>
          </button>
        ))}
        <span className="count">
          {accepted} {t.accepted} · {findings.length - accepted} {t.rejected}
        </span>
      </div>
      <div className="bulk">
        <button onClick={() => onDecide(visibleIds, { accept: true })}>
          <IconCheck size={11} /> {filter === 'all' ? t.acceptAll : t.acceptCategory}
        </button>
        <button onClick={() => onDecide(visibleIds, { accept: false })}>
          <IconX size={11} /> {filter === 'all' ? t.rejectAll : t.rejectCategory}
        </button>
      </div>
      <table className="rtable ftable">
        <thead>
          <tr>
            <th className="num">{t.colLine}</th>
            <th>{t.colCategory}</th>
            <th>{t.colOriginal}</th>
            <th>{t.colReplacement}</th>
            <th>{t.colSource}</th>
            <th>{t.colDecision}</th>
          </tr>
        </thead>
        <tbody>
          {list.map((f) => {
            const e = effective(f, decisions[f.id]);
            const occ = occurrences[`${f.category}|${f.text.toLowerCase()}`] ?? 1;
            const custom = decisions[f.id]?.replacement != null;
            return (
              <tr key={f.id} className={e.accept ? '' : 'rejected'}>
                <td className="num dim">{f.line}</td>
                <td>
                  <select
                    className="catsel"
                    style={{ borderColor: `var(--cat-${e.category})` }}
                    value={e.category}
                    onChange={(ev) => onDecide([f.id], { category: ev.target.value === f.category ? null : (ev.target.value as Category), replacement: null })}
                  >
                    {CATEGORIES.map((c) => (
                      <option key={c} value={c}>
                        {categoryLabel(t, c)}
                      </option>
                    ))}
                  </select>
                </td>
                <td className="orig">
                  <span className="mono">{f.text}</span>
                  {occ > 1 && <span className="chip mini dim">{t.occurrences(occ)}</span>}
                </td>
                <td className="repl">
                  {editing === f.id ? (
                    <input
                      type="text"
                      autoFocus
                      value={draft}
                      onChange={(ev) => setDraft(ev.target.value)}
                      onBlur={() => commitEdit(f)}
                      onKeyDown={(ev) => {
                        if (ev.key === 'Enter') commitEdit(f);
                        if (ev.key === 'Escape') setEditing(null);
                      }}
                    />
                  ) : (
                    <span className={`mono editable ${custom ? 'custom' : ''}`} title={t.editReplacement} onClick={() => startEdit(f)}>
                      {e.replacement === '' ? '∅' : e.replacement}
                    </span>
                  )}
                  {custom && editing !== f.id && (
                    <button className="icon ghost" title={t.resetReplacement} onClick={() => onDecide([f.id], { replacement: null })}>
                      <IconRefresh size={10} />
                    </button>
                  )}
                </td>
                <td className="dim">
                  {sourceLabel(t, f.source)} <span className="conf">{f.confidence}%</span>
                </td>
                <td className="acts">
                  <button className={`icon ${e.accept ? 'on' : ''}`} title={t.accept} onClick={() => onDecide([f.id], { accept: true })}>
                    <IconCheck size={11} />
                  </button>
                  <button className={`icon ${e.accept ? '' : 'off'}`} title={t.reject} onClick={() => onDecide([f.id], { accept: false })}>
                    <IconX size={11} />
                  </button>
                  <button className="icon" title={t.toAllowlistHint} onClick={() => onAllow(f.text)}>
                    <IconShield size={11} />
                  </button>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
