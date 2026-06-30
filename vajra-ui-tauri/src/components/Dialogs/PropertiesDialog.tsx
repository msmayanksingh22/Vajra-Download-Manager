import React, { useState, useEffect, useRef } from 'react';
// eslint-disable-next-line @typescript-eslint/no-unused-vars
import { X, FileText, Globe, ShieldCheck, Activity, Calendar, Clock, FolderDown, Bolt, FileBadge, Hash, Save, CircleCheck, Download, CheckCircle2 } from 'lucide-react';
import { api, fmtBytes } from '../../api';
import { cn } from '../../utils';
import { useDialogEscape } from '../../hooks/useDialogEscape';
import { useFocusTrap } from '../../hooks/useFocusTrap';

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export default function PropertiesDialog({ item, onClose }: any) {
  const [url, setUrl]                 = useState(item.url || '');
  const [maxConnections, setMaxConnections] = useState(item.max_connections || 8);
  const [speedLimit, setSpeedLimit]   = useState(item.speed_limit_bps ? Math.floor(item.speed_limit_bps / 1024) : '');
  const [priority, setPriority]       = useState(item.priority || 'normal');
  const [filename, setFilename]       = useState(item.filename || item.file_name || '');
  const [tagsInput, setTagsInput]     = useState((item.tags || []).join(', '));
  const [showSaved, setShowSaved]     = useState(false);
  const savedTimerRef                 = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isDirty                       = useRef(false);
  useDialogEscape(onClose);
  const trapRef = useFocusTrap();

  // Auto-save debounce — only fires after the user makes a change
  useEffect(() => {
    if (!isDirty.current) {
        isDirty.current = true;
        return;
    }
    const t = setTimeout(async () => {
      try {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const payload: any = {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          max_connections: parseInt(maxConnections as any, 10),
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          speed_limit_bps: speedLimit ? parseInt(speedLimit as any, 10) * 1024 : 0,
          priority,
        };
        if (url !== item.url) payload.url = url;
        if (filename !== (item.filename || item.file_name)) payload.filename = filename;
        const currentTags = (item.tags || []).join(', ');
        if (tagsInput !== currentTags) {
          payload.tags = tagsInput.split(',').map((t: string) => t.trim()).filter(Boolean);
        }
        await api.patch(item.id, payload);
        // Flash "Saved ✓" indicator
        setShowSaved(true);
        if (savedTimerRef.current) clearTimeout(savedTimerRef.current);
        savedTimerRef.current = setTimeout(() => setShowSaved(false), 1500);
      } catch (e) { console.error(e); }
    }, 500);
    return () => clearTimeout(t);
  }, [url, filename, maxConnections, speedLimit, priority, tagsInput, item.id, item.url, item.filename, item.file_name, item.tags]);

  const STATUS_COLORS: Record<string, string> = {
    completed:   'var(--color-success)',
    downloading: 'var(--color-brand)',
    paused:      'var(--color-warning)',
    error:       'var(--color-error)',
  };
  const statusColor = STATUS_COLORS[item.status] || 'var(--color-text-4)';

  const SectionHead = ({ icon: Icon, label }: { icon: React.ElementType; label: string }) => (
    <div className="flex items-center gap-2 pb-2 mb-1" style={{ borderBottom: '1px solid var(--color-border-subtle)' }}>
      <Icon size={14} style={{ color: 'var(--color-brand)' }} />
      <span style={{ fontSize: 'var(--text-sm-size)', fontWeight: 700, color: 'var(--color-text-1)' }}>{label}</span>
    </div>
  );
  const FieldLabel = ({ children }: { children: React.ReactNode }) => (
    <span className="form-label">{children}</span>
  );
  const StatCard = ({ icon: Icon, label, value }: { icon: React.ElementType; label: string; value: React.ReactNode }) => (
    <div className="card-subtle flex flex-col gap-1" style={{ padding: 'var(--sp-3)' }}>
      <span style={{ fontSize: 'var(--text-xs-size)', color: 'var(--color-text-3)', display: 'flex', alignItems: 'center', gap: 4 }}>
        <Icon size={10} /> {label}
      </span>
      <span style={{ fontSize: 'var(--text-sm-size)', fontWeight: 700, color: 'var(--color-text-1)' }}>{value}</span>
    </div>
  );

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div ref={trapRef} className="dialog-panel" style={{ width: 500, maxHeight: '85vh' }} onClick={e => e.stopPropagation()}
        role="dialog" aria-modal="true" aria-labelledby="properties-dialog-title"
      >
        {/* Header */}
        <div className="dialog-header">
          <div className="dialog-header-title" id="properties-dialog-title" style={{ gap: 10, minWidth: 0 }}>
            <div style={{ backgroundColor: 'var(--color-brand-dim)', padding: 7, borderRadius: 'var(--radius-md)', display: 'flex', flexShrink: 0 }}>
              <FileBadge size={15} style={{ color: 'var(--color-brand)' }} />
            </div>
            <div className="min-w-0">
              <div className="truncate" style={{ fontWeight: 700, fontSize: 'var(--text-sm-size)', color: 'var(--color-text-1)' }}>{filename || 'Unknown File'}</div>
              <div style={{ fontSize: 'var(--text-xs-size)', color: 'var(--color-text-4)' }}>ID: {item.id}</div>
            </div>
          </div>
          <div className="flex items-center gap-2 flex-shrink-0">
            {showSaved && (
              <span
                className="flex items-center gap-1"
                style={{
                  fontSize: 'var(--text-xs-size)', fontWeight: 600,
                  color: 'var(--color-success)',
                  animation: 'fadeIn 0.15s ease',
                }}
              >
                <CheckCircle2 size={12} />
                Saved
              </span>
            )}
            <button className="btn-icon" onClick={onClose} title="Close"><X size={15} /></button>
          </div>
        </div>

        {/* Body */}
        <div className="dialog-body" style={{ overflowY: 'auto' }}>

          {/* General */}
          <div className="flex flex-col gap-3">
            <SectionHead icon={FileText} label="General" />
            <div className="form-group">
              <FieldLabel>File Name</FieldLabel>
              <input type="text" className="input-field" value={filename} onChange={e => setFilename(e.target.value)} />
            </div>
            <div className="form-group">
              <FieldLabel>Source URL</FieldLabel>
              <div className="relative">
                <input type="text" className="input-field" value={url} onChange={e => setUrl(e.target.value)} style={{ paddingRight: 32 }} />
                <Globe size={13} style={{ position: 'absolute', right: 10, top: '50%', transform: 'translateY(-50%)', color: 'var(--color-text-4)' }} />
              </div>
            </div>
            <div className="form-group">
              <FieldLabel>Save Location</FieldLabel>
              <div className="input-field" style={{ opacity: 0.7, cursor: 'not-allowed', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                {item.output_path || 'Default folder'}
              </div>
            </div>
            <div className="form-group">
              <FieldLabel>Tags (comma separated)</FieldLabel>
              <input type="text" className="input-field" placeholder="e.g. video, work, urgent" value={tagsInput} onChange={e => setTagsInput(e.target.value)} />
            </div>
            <div className="form-group">
              <FieldLabel>Queue Priority</FieldLabel>
              <div className="flex gap-2">
                {['high','normal','low'].map(p => (
                  <button key={p} type="button" onClick={() => setPriority(p)}
                    className={cn('flex-1 py-1.5 rounded border text-xs font-bold uppercase tracking-wider transition-colors', priority === p ? 'btn-primary' : 'btn-secondary')}
                    style={{ padding: '6px 0' }}>
                    {p}
                  </button>
                ))}
              </div>
            </div>
          </div>

          {/* Connection */}
          <div className="flex flex-col gap-3 mt-1">
            <SectionHead icon={Bolt} label="Connection Tuning" />
            <div className="grid grid-cols-2 gap-3">
              <div className="form-group">
                <FieldLabel>Max Connections</FieldLabel>
                <select className="select-field" value={maxConnections} onChange={e => setMaxConnections(e.target.value)}>
                  <option value="1">1 Thread (Safest)</option>
                  <option value="4">4 Threads</option>
                  <option value="8">8 Threads (Recommended)</option>
                  <option value="16">16 Threads (Aggressive)</option>
                  <option value="32">32 Threads (Extreme)</option>
                </select>
              </div>
              <div className="form-group">
                <FieldLabel>Speed Limit (KB/s)</FieldLabel>
                <input type="number" min="0" className="input-field" placeholder="0 = Unlimited" value={speedLimit} onChange={e => setSpeedLimit(e.target.value)} />
              </div>
            </div>
          </div>

          {/* Status Details */}
          <div className="flex flex-col gap-3 mt-1">
            <SectionHead icon={Activity} label="Status Details" />

            {/* Status banner */}
            <div className="card-subtle flex items-center justify-between" style={{ padding: 'var(--sp-3) var(--sp-4)' }}>
              <div className="flex items-center gap-2">
                <div className="w-2 h-2 rounded-full" style={{ backgroundColor: statusColor, animation: item.status === 'downloading' ? 'pulse 2s infinite' : 'none' }} />
                <span style={{ fontSize: 'var(--text-xs-size)', color: 'var(--color-text-3)', fontWeight: 700 }}>Current Status</span>
              </div>
              <span style={{ fontWeight: 700, fontSize: 'var(--text-sm-size)', color: statusColor, textTransform: 'uppercase', letterSpacing: '0.1em' }}>
                {item.status}
              </span>
            </div>

            <div className="grid grid-cols-2 gap-2">
              <StatCard icon={Download} label="Total Size"       value={item.total_bytes ? fmtBytes(item.total_bytes) : 'â€”'} />
              <StatCard icon={Save}     label="Downloaded"       value={fmtBytes(item.bytes_done || 0)} />
              <StatCard icon={Calendar} label="Date Added"       value={item.created_at ? new Date(item.created_at * 1000).toLocaleDateString() : 'â€”'} />
              <StatCard icon={Clock}    label="Date Completed"   value={item.completed_at ? new Date(item.completed_at * 1000).toLocaleDateString() : 'â€”'} />
            </div>

            <div className="card-subtle flex items-center justify-between" style={{ padding: 'var(--sp-3) var(--sp-4)' }}>
              <span style={{ fontSize: 'var(--text-sm-size)', color: 'var(--color-text-3)', fontWeight: 500 }}>Server Resume Support</span>
              {item.resume_supported !== false ? (
                <span className="tag tag-success">Supported</span>
              ) : (
                <span className="tag tag-error">Not Supported</span>
              )}
            </div>

            {item.hash_result && (
              <div className="card-subtle" style={{ padding: 'var(--sp-3) var(--sp-4)', borderColor: 'var(--color-brand-dim)', backgroundColor: 'var(--color-brand-dim)' }}>
                <div className="flex items-center gap-2 mb-2" style={{ fontSize: 'var(--text-xs-size)', fontWeight: 700, color: 'var(--color-brand)' }}>
                  <ShieldCheck size={12} /> Integrity Verification
                </div>
                <div className="flex items-center justify-between">
                  <span style={{ fontSize: 'var(--text-xs-size)', color: 'var(--color-text-3)' }}>
                    {item.hash_result.algorithm}
                  </span>
                  <span className={item.hash_result.matched ? 'tag tag-success' : 'tag tag-error'}>
                    {item.hash_result.matched ? 'Matched' : 'Mismatch'}
                  </span>
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="dialog-footer">
          <button className="btn-primary" onClick={onClose}>Done</button>
        </div>
      </div>
    </div>
  );
}