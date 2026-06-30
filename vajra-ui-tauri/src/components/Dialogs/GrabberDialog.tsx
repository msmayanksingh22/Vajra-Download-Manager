import { useState, useRef, useEffect } from 'react';
import { X, Globe, Link, FileDown, FileText, FileImage, FileAudio, FileVideo, Download, AppWindow, AlertCircle } from 'lucide-react';
import { api } from '../../api';
// eslint-disable-next-line @typescript-eslint/no-unused-vars
import { cn } from '../../utils';
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

export function GrabberDialog({ onClose }: { onClose: () => void }) {
  const [url, setUrl]           = useState('');
  const [running, setRunning]   = useState(false);
  const [results, setResults]   = useState<SpiderResult[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [filter, setFilter]     = useState('all');
  const [grabError, setGrabError] = useState<string | null>(null);
  const esRef = useRef<EventSource | null>(null);
  useDialogEscape(onClose);
  const trapRef = useFocusTrap();

  useEffect(() => () => { if (esRef.current) esRef.current.close(); }, []);

  function handleStart(e: React.FormEvent) {
    e.preventDefault();
    if (!url.trim()) return;
    if (running) { esRef.current?.close(); setRunning(false); return; }
    setResults([]); setSelected(new Set()); setRunning(true); setGrabError(null);
    const es = new EventSource(`http://127.0.0.1:6277/api/v1/spider?url=${encodeURIComponent(url)}`);
    esRef.current = es;
    es.onmessage = (ev) => { try { const d = JSON.parse(ev.data); setResults(p => p.some(r => r.url===d.url) ? p : [...p, d]); } catch { /* ignore */ } };
    es.onerror = () => {
      const hadResults = esRef.current !== null;
      es.close();
      setRunning(false);
      setResults(prev => {
        if (prev.length === 0 && hadResults) {
          setGrabError('Could not connect to daemon or no resources found. Make sure Vajra is running.');
        }
        return prev;
      });
    };
  }

  const filtered = results.filter(r => filter === 'all' || r.resource_type === filter);
  const toggleSelect = (u: string) => setSelected(p => { const n = new Set(p); n.has(u) ? n.delete(u) : n.add(u); return n; });
  const selectAll    = () => setSelected(selected.size === filtered.length ? new Set() : new Set(filtered.map(r => r.url)));

  async function downloadSelected() {
    for (const r of results.filter(r => selected.has(r.url))) { try { await api.add({ url: r.url }); } catch { /* ignore */ } }
    onClose();
  }

  const tabRow = (items: string[], activeKey: string, setKey: (k: string) => void) => (
    <div className="flex gap-5 px-4 pt-2 shrink-0 drag-region" style={{ borderBottom: '1px solid var(--color-border)', backgroundColor: 'var(--color-surface-raised)' }}>
      {items.map(f => (
        <div key={f} onClick={() => setKey(f)} className="no-drag relative pb-2" style={{
          fontSize: 'var(--text-xs-size)', fontWeight: 700, textTransform: 'capitalize', cursor: 'default',
          color: activeKey === f ? 'var(--color-brand)' : 'var(--color-text-3)',
          transition: 'color var(--transition-fast)',
        }}>
          {f}
          {activeKey === f && <div style={{ position: 'absolute', bottom: 0, left: 0, right: 0, height: 2, backgroundColor: 'var(--color-brand)', borderRadius: 2 }} />}
        </div>
      ))}
    </div>
  );

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div ref={trapRef} className="dialog-panel" style={{ width: 780, height: '78vh', maxHeight: 620 }} onClick={e => e.stopPropagation()}
        role="dialog" aria-modal="true" aria-labelledby="grabber-dialog-title"
      >
        {/* Header */}
        <div className="dialog-header">
          <div className="dialog-header-title" id="grabber-dialog-title"><Globe size={16} /> Site Grabber</div>
          <button className="btn-icon" onClick={onClose} title="Close"><X size={15} /></button>
        </div>

        {/* Crawl form */}
        <div style={{ padding: 'var(--sp-3)', borderBottom: '1px solid var(--color-border)', flexShrink: 0 }}>
          <form onSubmit={handleStart} className="flex gap-2">
            <input className="input-field flex-1" placeholder="Root URL to grab resources fromâ€¦" value={url} onChange={e => setUrl(e.target.value)} disabled={running} />
            <button type="submit" className={running ? 'btn-danger' : 'btn-primary'} style={{ minWidth: 80 }}>
              {running ? 'Stop' : 'Grab'}
            </button>
          </form>
        </div>

        {/* Filter tabs */}
        {results.length > 0 && tabRow(['all','video','audio','image','document','page'], filter, setFilter)}

        {/* Results list */}
        <div className="flex-1 overflow-y-auto" style={{ backgroundColor: 'var(--color-surface)' }}>
          {filtered.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full gap-2" style={{ color: 'var(--color-text-4)' }}>
              {grabError ? (
                <>
                  <AlertCircle size={28} style={{ color: 'var(--color-error)', opacity: 0.7 }} />
                  <span style={{ fontSize: 'var(--text-sm-size)', fontWeight: 600, color: 'var(--color-error)', textAlign: 'center', maxWidth: 320, padding: '0 16px' }}>
                    {grabError}
                  </span>
                  <button className="btn-secondary" style={{ fontSize: 'var(--text-xs-size)', marginTop: 4 }} onClick={() => setGrabError(null)}>Dismiss</button>
                </>
              ) : running ? (
                <>
                  <div className="w-6 h-6 rounded-full border-2 animate-spin" style={{ borderColor: 'var(--color-brand)', borderTopColor: 'transparent' }} />
                  <span style={{ fontSize: 'var(--text-sm-size)', fontWeight: 600 }}>Grabbing…</span>
                </>
              ) : (
                <span style={{ fontSize: 'var(--text-sm-size)' }}>No results. Enter a URL and start the grabber.</span>
              )}
            </div>
          ) : (
            <div className="flex flex-col">
              {filtered.map(r => {
                const def = TYPE_ICONS[r.resource_type] || { icon: Link, color: 'var(--color-text-3)' };
                const Icon = def.icon;
                const isSel = selected.has(r.url);
                return (
                  <label key={r.url} className="flex items-center gap-3 px-3 py-2" style={{
                    borderBottom: '1px solid var(--color-border-subtle)', cursor: 'default',
                    backgroundColor: isSel ? 'var(--color-brand-dim)' : 'transparent',
                    transition: 'background-color var(--transition-fast)',
                  }}>
                    <input type="checkbox" checked={isSel} onChange={() => toggleSelect(r.url)} style={{ accentColor: 'var(--color-brand)', width: 13, height: 13, cursor: 'default' }} />
                    <Icon size={14} style={{ color: def.color, flexShrink: 0 }} />
                    <div className="flex-1 min-w-0 flex items-center justify-between gap-4">
                      <span className="truncate" style={{ fontSize: 'var(--text-sm-size)', fontWeight: 600, color: isSel ? 'var(--color-brand)' : 'var(--color-text-1)' }} title={r.name}>{r.name}</span>
                      <span className="truncate" style={{ fontSize: 'var(--text-xs-size)', color: 'var(--color-text-4)', maxWidth: '50%' }} title={r.url}>{r.url}</span>
                    </div>
                  </label>
                );
              })}
            </div>
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