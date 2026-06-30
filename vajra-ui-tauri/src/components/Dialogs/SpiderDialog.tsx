import { useState, useRef, useEffect } from 'react';
import { X, Activity, Link, FileText, FileImage, FileAudio, FileVideo, Download, AppWindow } from 'lucide-react';
// eslint-disable-next-line @typescript-eslint/no-unused-vars
import { api } from '../../api';
import { useUiStore } from '../../stores/uiStore';
import { useDialogEscape } from '../../hooks/useDialogEscape';
import { useFocusTrap } from '../../hooks/useFocusTrap';

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const TYPE_ICONS: Record<string, { icon: any; color: string }> = {
  video:    { icon: FileVideo,  color: 'var(--color-brand)' },
  audio:    { icon: FileAudio,  color: 'var(--color-success)' },
  image:    { icon: FileImage,  color: 'var(--color-error)' },
  document: { icon: FileText,   color: 'var(--color-info)' },
  page:     { icon: AppWindow,  color: 'var(--color-warning)' },
};

interface SpiderResult { url: string; name: string; resource_type: string; }

export interface SpiderDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onBatchAdd: (urls: string[]) => void;
}

export function SpiderDialog({ open, onOpenChange, onBatchAdd }: SpiderDialogProps) {
  const [url, setUrl]           = useState('');
  const [depth, setDepth]       = useState(1);
  const [running, setRunning]   = useState(false);
  const [results, setResults]   = useState<SpiderResult[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [filter, setFilter]     = useState('all');
  const [regexStr, setRegexStr] = useState('');
  const [extStr, setExtStr]     = useState('');
  const esRef = useRef<EventSource | null>(null);
  const { spiderInitialUrl, spiderInitialExtensions, setSpiderInitial } = useUiStore();
  useDialogEscape(() => onOpenChange(false));
  const trapRef = useFocusTrap();

  useEffect(() => {
    if (open) {
      if (spiderInitialUrl) {
        setUrl(spiderInitialUrl);
      }
      if (spiderInitialExtensions) {
        setExtStr(spiderInitialExtensions);
      }
      if (spiderInitialUrl || spiderInitialExtensions) {
        setSpiderInitial('', '');
      }
    } else {
      esRef.current?.close();
      setRunning(false);
      setUrl('');
      setDepth(1);
      setResults([]);
      setSelected(new Set());
      setExtStr('');
    }
  }, [open, spiderInitialUrl, spiderInitialExtensions, setSpiderInitial]);

  useEffect(() => () => { esRef.current?.close(); }, []);

  function handleStart(e: React.FormEvent) {
    e.preventDefault();
    if (!url.trim()) return;
    if (running) { esRef.current?.close(); setRunning(false); return; }
    setResults([]); setSelected(new Set()); setRunning(true);
    const qs = `url=${encodeURIComponent(url)}&depth=${depth}&regex=${encodeURIComponent(regexStr)}&extensions=${encodeURIComponent(extStr)}`;
    const es = new EventSource(`http://127.0.0.1:6277/api/v1/spider?${qs}`);
    esRef.current = es;
    es.onmessage = (ev) => { try { const d = JSON.parse(ev.data); setResults(p => p.some(r => r.url===d.url) ? p : [...p, d]); } catch { /* ignore */ } };
    es.onerror = () => { es.close(); setRunning(false); };
  }

  const filtered     = results.filter(r => filter === 'all' || r.resource_type === filter);
  const toggleSelect = (u: string) => setSelected(p => { const n = new Set(p); n.has(u) ? n.delete(u) : n.add(u); return n; });
  const selectAll    = () => setSelected(selected.size === filtered.length ? new Set() : new Set(filtered.map(r => r.url)));

  function downloadSelected() {
    onBatchAdd(results.filter(r => selected.has(r.url)).map(r => r.url));
    onOpenChange(false);
  }

  if (!open) return null;

  return (
    <div className="dialog-overlay" onClick={() => onOpenChange(false)}>
      <div ref={trapRef} className="dialog-panel" style={{ width: 780, height: '78vh', maxHeight: 620 }} onClick={e => e.stopPropagation()}
        role="dialog" aria-modal="true" aria-labelledby="spider-dialog-title"
      >
        {/* Header */}
        <div className="dialog-header">
          <div className="dialog-header-title" id="spider-dialog-title"><Activity size={16} /> Site Spider</div>
          <button className="btn-icon" onClick={() => onOpenChange(false)} title="Close"><X size={15} /></button>
        </div>

        {/* Form */}
        <div style={{ padding: 'var(--sp-3)', borderBottom: '1px solid var(--color-border)', flexShrink: 0 }}>
          <form onSubmit={handleStart} className="flex flex-col gap-2">
            <div className="flex gap-2">
              <input className="input-field flex-1" placeholder="Root URL to crawlâ€¦" value={url} onChange={e => setUrl(e.target.value)} disabled={running} />
              <div className="flex items-center gap-2 px-3" style={{ border: '1px solid var(--color-border)', borderRadius: 'var(--radius-md)', backgroundColor: 'var(--color-surface-raised)' }}>
                <span className="form-label" style={{ whiteSpace: 'nowrap' }}>Depth</span>
                <input type="number" min="1" max="5" value={depth} onChange={e => setDepth(parseInt(e.target.value) || 1)} disabled={running}
                  className="input-field text-center" style={{ width: 40, padding: '2px 4px' }} />
              </div>
              <button type="submit" className={running ? 'btn-danger' : 'btn-primary'} style={{ minWidth: 80 }}>
                {running ? 'Stop' : 'Spider'}
              </button>
            </div>
            <div className="flex gap-2">
              <input className="input-field flex-1" placeholder="Regex filter (optional)" value={regexStr} onChange={e => setRegexStr(e.target.value)} disabled={running} />
              <input className="input-field flex-1" placeholder="Extensions e.g. pdf, mp4, zip" value={extStr} onChange={e => setExtStr(e.target.value)} disabled={running} />
            </div>
          </form>
        </div>

        {/* Filter tabs */}
        {results.length > 0 && (
          <div className="flex gap-5 px-4 pt-2 shrink-0 drag-region" style={{ borderBottom: '1px solid var(--color-border)', backgroundColor: 'var(--color-surface-raised)' }}>
            {['all','video','audio','image','document','page'].map(f => (
              <div key={f} onClick={() => setFilter(f)} className="no-drag relative pb-2" style={{
                fontSize: 'var(--text-xs-size)', fontWeight: 700, textTransform: 'capitalize', cursor: 'default',
                color: filter === f ? 'var(--color-brand)' : 'var(--color-text-3)',
              }}>
                {f}
                {filter === f && <div style={{ position: 'absolute', bottom: 0, left: 0, right: 0, height: 2, backgroundColor: 'var(--color-brand)', borderRadius: 2 }} />}
              </div>
            ))}
          </div>
        )}

        {/* Results */}
        <div className="flex-1 overflow-y-auto" style={{ backgroundColor: 'var(--color-surface)' }}>
          {filtered.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full gap-2" style={{ color: 'var(--color-text-4)' }}>
              {running ? (
                <>
                  <div className="w-6 h-6 rounded-full border-2 animate-spin" style={{ borderColor: 'var(--color-brand)', borderTopColor: 'transparent' }} />
                  <span style={{ fontSize: 'var(--text-sm-size)', fontWeight: 600 }}>Spideringâ€¦</span>
                </>
              ) : (
                <span style={{ fontSize: 'var(--text-sm-size)' }}>No results. Enter a URL and start the spider.</span>
              )}
            </div>
          ) : (
            filtered.map(r => {
              const def  = TYPE_ICONS[r.resource_type] || { icon: Link, color: 'var(--color-text-3)' };
              const Icon = def.icon;
              const isSel = selected.has(r.url);
              return (
                <label key={r.url} className="flex items-center gap-3 px-3 py-2" style={{
                  borderBottom: '1px solid var(--color-border-subtle)', cursor: 'default',
                  backgroundColor: isSel ? 'var(--color-brand-dim)' : 'transparent',
                }}>
                  <input type="checkbox" checked={isSel} onChange={() => toggleSelect(r.url)} style={{ accentColor: 'var(--color-brand)', width: 13, height: 13, cursor: 'default' }} />
                  <Icon size={14} style={{ color: def.color, flexShrink: 0 }} />
                  <div className="flex-1 min-w-0 flex items-center justify-between gap-4">
                    <span className="truncate" style={{ fontSize: 'var(--text-sm-size)', fontWeight: 600, color: isSel ? 'var(--color-brand)' : 'var(--color-text-1)' }} title={r.name}>{r.name}</span>
                    <span className="truncate" style={{ fontSize: 'var(--text-xs-size)', color: 'var(--color-text-4)', maxWidth: '50%' }} title={r.url}>{r.url}</span>
                  </div>
                </label>
              );
            })
          )}
        </div>

        {/* Footer */}
        <div className="dialog-footer justify-between">
          <span style={{ fontSize: 'var(--text-sm-size)', color: 'var(--color-text-3)', fontWeight: 600 }}>
            {selected.size} / {filtered.length} selected
          </span>
          <div className="flex gap-2">
            <button className="btn-secondary" onClick={selectAll} disabled={filtered.length === 0}>Select All</button>
            <button className="btn-primary flex items-center gap-2" onClick={downloadSelected} disabled={selected.size === 0}>
              <Download size={14} /> Batch Add
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}