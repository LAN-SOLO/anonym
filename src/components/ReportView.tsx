import { ApplyResult, CATEGORIES, Category } from '../api';
import { Dict, Lang } from '../i18n';
import { IconExternalLink } from '../icons';
import { baseName, categoryLabel, dirName, fmtDateTime } from '../util';

export function ReportView({ result, outputs, t, lang, onOpen }: { result: ApplyResult | undefined; outputs: ApplyResult[]; t: Dict; lang: Lang; onOpen: (path: string) => void }) {
  if (!result) return <div className="empty">{t.reportNone}</div>;
  const r = result.report;
  return (
    <div className="report">
      <div className="kpis">
        <div className="kpi">
          <div className="k">{t.reportReplaced}</div>
          <div className="v">{r.replaced}</div>
          <div className="s">
            {r.rejected} {t.reportRejected}
          </div>
        </div>
        <div className="kpi">
          <div className="k">{t.reportDistinct}</div>
          <div className="v">{r.entries.length}</div>
          <div className="s">{r.format}</div>
        </div>
        <div className="kpi">
          <div className="k">{t.reportDateShift}</div>
          <div className="v">
            {r.dateShiftDays > 0 ? '+' : ''}
            {r.dateShiftDays}
          </div>
          <div className="s">{t.days}</div>
        </div>
        <div className="kpi">
          <div className="k">{t.reportWorld}</div>
          <div className="v" style={{ fontSize: 16 }}>
            {r.world}
          </div>
          <div className="s">{r.rules}</div>
        </div>
      </div>
      <div className="panel">
        <h3>{t.reportOutput}</h3>
        <div className="pathrow">
          <span className="mono">{result.output}</span>
          <button onClick={() => onOpen(dirName(result.output))}>
            <IconExternalLink size={11} /> {t.reportOpenFolder}
          </button>
        </div>
        {result.reportPath && (
          <div className="pathrow">
            <span className="mono dim">
              {t.reportFile}: {baseName(result.reportPath)}
            </span>
          </div>
        )}
        {result.recoverPath && (
          <div className="pathrow">
            <span className="mono dim">
              {t.recoverFile}: {baseName(result.recoverPath)}
            </span>
          </div>
        )}
        <div className="note">
          {t.reportCreated}: {fmtDateTime(r.created, lang)} · anonym {r.version}
        </div>
        {outputs.length > 1 && (
          <>
            <h3 style={{ marginTop: 12 }}>{t.outputs}</h3>
            {outputs.map((o) => (
              <div key={o.output} className="pathrow">
                <span className="chip mini dim">{o.report.format}</span>
                <span className="mono">{baseName(o.output)}</span>
                <button onClick={() => onOpen(dirName(o.output))}>
                  <IconExternalLink size={11} /> {t.reportOpenFolder}
                </button>
              </div>
            ))}
          </>
        )}
      </div>
      <div className="panel" style={{ marginTop: 12 }}>
        <h3>{t.reportEntries}</h3>
        <div className="catbars">
          {CATEGORIES.filter((c) => r.byCategory[c]).map((c) => (
            <span key={c} className="chip mini" style={{ borderColor: `var(--cat-${c})` }}>
              <i className="dot" style={{ background: `var(--cat-${c})` }} />
              {categoryLabel(t, c)} {r.byCategory[c]}
            </span>
          ))}
        </div>
        <table className="rtable detail">
          <thead>
            <tr>
              <th>{t.colCategory}</th>
              <th>{t.colOriginal}</th>
              <th>{t.colReplacement}</th>
              <th className="num">{t.colCount}</th>
            </tr>
          </thead>
          <tbody>
            {r.entries.map((e, i) => (
              <tr key={i}>
                <td>
                  <i className="dot" style={{ background: `var(--cat-${e.category as Category})` }} /> {categoryLabel(t, e.category)}
                </td>
                <td className="mono">{e.original}</td>
                <td className="mono">{e.replacement === '' ? '∅' : e.replacement}</td>
                <td className="num">{e.count}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
