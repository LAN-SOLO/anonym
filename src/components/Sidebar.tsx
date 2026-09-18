import { Analysis, ApplyResult } from '../api';
import { Dict } from '../i18n';
import { IconFile, IconTable, IconX } from '../icons';
import { baseName } from '../util';

export type FileStatus = 'idle' | 'analyzing' | 'ready' | 'done' | 'error';

export interface FileEntry {
  path: string;
  status: FileStatus;
  analysis?: Analysis;
  error?: string;
  result?: ApplyResult;
  /** Alle bisher geschriebenen Ausgaben (Ursprungsformat und Exporte). */
  outputs?: ApplyResult[];
}

function isTable(path: string): boolean {
  const ext = path.split('.').pop()?.toLowerCase() ?? '';
  return ['csv', 'tsv', 'tab', 'xlsx', 'xlsm', 'xltx', 'ods', 'ots'].includes(ext);
}

export function Sidebar({
  files,
  selected,
  masked,
  busy,
  t,
  onSelect,
  onAdd,
  onRemove,
  onClear,
  onApplyAll,
}: {
  files: FileEntry[];
  selected: string | null;
  masked: boolean;
  busy: boolean;
  t: Dict;
  onSelect: (path: string) => void;
  onAdd: () => void;
  onRemove: (path: string) => void;
  onClear: () => void;
  onApplyAll: () => void;
}) {
  const ready = files.filter((f) => f.status === 'ready' || f.status === 'done').length;
  return (
    <aside className="sidebar">
      <div className="sb-title">
        <span>{t.files}</span>
        <span className="grow" />
        {files.length > 0 && (
          <button className="icon" title={t.clearList} onClick={onClear}>
            <IconX size={11} />
          </button>
        )}
      </div>
      {files.length === 0 && <div className="sb-empty">{t.dropHint}</div>}
      {files.map((f) => {
        const n = f.analysis?.findings.length ?? 0;
        return (
          <button key={f.path} className={`sb-item ${selected === f.path ? 'active' : ''} st-${f.status}`} onClick={() => onSelect(f.path)} title={f.path}>
            {isTable(f.path) ? <IconTable size={12} /> : <IconFile size={12} />}
            <span className="fname">{baseName(f.path)}</span>
            {f.status === 'analyzing' && <span className="cnt">…</span>}
            {f.status === 'ready' && <span className="cnt">{n}</span>}
            {f.status === 'done' && <span className="cnt ok">✓</span>}
            {f.status === 'error' && <span className="cnt bad">!</span>}
            <span
              className="icon-x"
              role="button"
              title={t.removeFile}
              onClick={(e) => {
                e.stopPropagation();
                onRemove(f.path);
              }}
            >
              <IconX size={10} />
            </span>
          </button>
        );
      })}
      <div className="sb-foot">
        <button onClick={onAdd}>{t.addFiles}</button>
        {masked && ready > 1 && (
          <button className="primary" style={{ marginTop: 6 }} disabled={busy} title={t.applyAllHint} onClick={onApplyAll}>
            {t.applyAll} ({ready})
          </button>
        )}
      </div>
    </aside>
  );
}
