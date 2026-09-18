import { useMemo, useState } from 'react';
import { Decision, Finding } from '../api';
import { Dict } from '../i18n';
import { buildOutput, effective } from '../util';

interface Mark {
  start: number;
  end: number;
  category: string;
  id: number;
  accept: boolean;
}

/** Text mit Markierungen zeilenweise rendern. */
function renderLines(text: string, marks: Mark[], onToggle: (id: number) => void, onlyChanged: boolean, changedLines: Set<number> | null) {
  const lines = text.split('\n');
  const out: JSX.Element[] = [];
  let pos = 0;
  let mi = 0;
  for (let li = 0; li < lines.length; li++) {
    const line = lines[li];
    const lineStart = pos;
    const lineEnd = pos + line.length;
    const segs: JSX.Element[] = [];
    let cur = lineStart;
    while (mi < marks.length && marks[mi].start < lineEnd) {
      const m = marks[mi];
      if (m.end <= cur) {
        mi++;
        continue;
      }
      const s = Math.max(m.start, cur);
      const e = Math.min(m.end, lineEnd);
      if (s > cur) segs.push(<span key={`t${cur}`}>{text.slice(cur, s)}</span>);
      segs.push(
        <mark key={`m${m.id}-${s}`} className={m.accept ? '' : 'rejected'} style={{ background: `var(--cat-${m.category}-bg)`, borderColor: `var(--cat-${m.category})` }} onClick={() => onToggle(m.id)}>
          {text.slice(s, e) || '∅'}
        </mark>
      );
      cur = e;
      if (m.end <= lineEnd) mi++;
      else break;
    }
    if (cur < lineEnd) segs.push(<span key={`t${cur}`}>{text.slice(cur, lineEnd)}</span>);
    const changed = changedLines ? changedLines.has(li) : segs.some((s) => s.type === 'mark');
    if (!onlyChanged || changed) {
      out.push(
        <div key={li} className={`pl ${changed ? 'changed' : ''}`}>
          <span className="ln">{li + 1}</span>
          <span className="lt">{segs.length ? segs : ' '}</span>
        </div>
      );
    }
    pos = lineEnd + 1;
  }
  return out;
}

export function Preview({
  text,
  findings,
  decisions,
  t,
  onToggle,
}: {
  text: string;
  findings: Finding[];
  decisions: Record<number, Decision>;
  t: Dict;
  onToggle: (id: number) => void;
}) {
  const [onlyChanged, setOnlyChanged] = useState(false);
  const origMarks: Mark[] = useMemo(() => findings.map((f) => ({ start: f.start, end: f.end, category: effective(f, decisions[f.id]).category, id: f.id, accept: effective(f, decisions[f.id]).accept })), [findings, decisions]);
  const output = useMemo(() => buildOutput(text, findings, decisions), [text, findings, decisions]);
  const outMarks: Mark[] = useMemo(() => output.marks.map((m) => ({ ...m, accept: true })), [output]);

  // Geänderte Zeilen (Original) — für den Filter „nur geänderte Zeilen“
  const changedOrig = useMemo(() => {
    const set = new Set<number>();
    let line = 0;
    let pos = 0;
    for (const f of findings) {
      if (!effective(f, decisions[f.id]).accept) continue;
      while (pos < f.start) {
        if (text.charCodeAt(pos) === 10) line++;
        pos++;
      }
      set.add(line);
    }
    return set;
  }, [text, findings, decisions]);

  return (
    <div className="preview">
      <div className="pv-bar">
        <span className="dim">{t.previewHint}</span>
        <span className="grow" />
        <label className="check" style={{ margin: 0 }}>
          <input type="checkbox" checked={onlyChanged} onChange={(e) => setOnlyChanged(e.target.checked)} />
          {t.previewOnlyChanged}
        </label>
      </div>
      <div className="pv-panes">
        <div className="pv-pane">
          <div className="pv-head">{t.previewOriginal}</div>
          <div className="pv-body">{renderLines(text, origMarks, onToggle, onlyChanged, changedOrig)}</div>
        </div>
        <div className="pv-pane">
          <div className="pv-head">{t.previewOutput}</div>
          <div className="pv-body">{renderLines(output.text, outMarks, onToggle, onlyChanged, null)}</div>
        </div>
      </div>
    </div>
  );
}
