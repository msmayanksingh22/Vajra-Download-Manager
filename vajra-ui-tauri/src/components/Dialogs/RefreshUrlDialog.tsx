import React, { useState, useEffect } from 'react';
import { X, Link as LinkIcon, AlertTriangle, ShieldCheck } from 'lucide-react';
import { api, fmtBytes } from '../../api';
import { DownloadInfo } from '../../types';
import { useDialogEscape } from '../../hooks/useDialogEscape';
import { useFocusTrap } from '../../hooks/useFocusTrap';

interface InspectData {
  total_bytes?: number | null;
  accepts_ranges?: boolean;
  content_type?: string | null;
}

export default function RefreshUrlDialog({
  item,
  onClose,
  onOk,
}: {
  item: DownloadInfo;
  onClose: () => void;
  onOk: () => void;
}) {
  const [url, setUrl] = useState(item?.url || '');
  const [inspectState, setIS] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');
  const [inspectData, setID] = useState<InspectData | null>(null);
  const [errorMsg, setErrorMsg] = useState('');
  useDialogEscape(onClose);
  const trapRef = useFocusTrap();

  useEffect(() => {
    if (!url.trim() || !url.startsWith('http') || url.trim() === item?.url) {
      setIS('idle');
      setID(null);
      return;
    }
    const t = setTimeout(async () => {
      setIS('loading');
      setErrorMsg('');
      try {
        const r = await api.inspect(url.trim());
        setID(r);
        setIS('success');
      } catch (e: any) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        setIS('error');
        setErrorMsg(e.message || 'Could not inspect URL');
      }
    }, 800);
    return () => clearTimeout(t);
  }, [url, item]);

  const handleApply = async () => {
    if (!url.trim()) return;
    try {
      await api.patch(item.id, { url: url.trim() });
      await api.patch(item.id, { action: 'resume' });
      onOk();
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
    } catch (e: any) {
      alert('Failed to refresh address: ' + e.message);
    }
  };

  const sizeMismatch =
    inspectData?.total_bytes && item?.total_bytes && item.total_bytes !== inspectData.total_bytes;

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div
        ref={trapRef}
        className="dialog-panel"
        style={{ width: 480 }}
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="refresh-dialog-title"
      >
        {/* Header */}
        <div className="dialog-header">
          <div className="dialog-header-title" id="refresh-dialog-title">
            <LinkIcon size={16} />
            Refresh Download Address
          </div>
          <button className="btn-icon" onClick={onClose} title="Close">
            <X size={15} />
          </button>
        </div>

        {/* Body */}
        <div className="dialog-body">
          <p
            style={{
              fontSize: 'var(--text-sm-size)',
              color: 'var(--color-text-3)',
              lineHeight: 1.6,
            }}
          >
            Paste a new download URL to update an expired link. Vajra will resume from where it left
            off.
          </p>

          {/* File name info */}
          <div className="card-subtle" style={{ padding: 'var(--sp-2) var(--sp-3)' }}>
            <span className="section-title">File</span>
            <p
              style={{
                fontWeight: 600,
                fontSize: 'var(--text-base-size)',
                color: 'var(--color-text-1)',
                marginTop: 2,
              }}
            >
              {item?.filename || 'Unknown File'}
            </p>
          </div>

          {/* New URL */}
          <div className="form-group">
            <div className="flex items-center justify-between">
              <span className="form-label">New URL</span>
              {inspectState === 'loading' && (
                <span style={{ fontSize: 'var(--text-xs-size)', color: 'var(--color-info)' }}>
                  Inspectingâ€¦
                </span>
              )}
              {inspectState === 'error' && (
                <span style={{ fontSize: 'var(--text-xs-size)', color: 'var(--color-error)' }}>
                  Inspect failed
                </span>
              )}
            </div>
            <input
              type="text"
              className="input-field"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="Paste new https:// link"
              autoFocus
            />
          </div>

          {/* Inspect results */}
          {inspectState === 'success' && inspectData && (
            <div className="card-subtle" style={{ padding: 'var(--sp-3)' }}>
              <div
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  fontSize: 'var(--text-sm-size)',
                }}
              >
                <span style={{ color: 'var(--color-text-3)' }}>Remote Size</span>
                <span style={{ fontWeight: 600, color: 'var(--color-text-1)' }}>
                  {inspectData.total_bytes ? fmtBytes(inspectData.total_bytes) : 'â€”'}
                  {sizeMismatch && (
                    <span style={{ color: 'var(--color-error)', marginLeft: 6 }}>(Mismatch!)</span>
                  )}
                </span>
              </div>
              <div
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  marginTop: 6,
                  fontSize: 'var(--text-sm-size)',
                }}
              >
                <span style={{ color: 'var(--color-text-3)' }}>Resume Support</span>
                <span
                  style={{
                    fontWeight: 600,
                    color: inspectData.accepts_ranges
                      ? 'var(--color-success)'
                      : 'var(--color-warning)',
                  }}
                >
                  {inspectData.accepts_ranges ? 'Yes' : 'No'}
                </span>
              </div>
              {sizeMismatch && (
                <div
                  style={{
                    display: 'flex',
                    alignItems: 'flex-start',
                    gap: 6,
                    marginTop: 8,
                    paddingTop: 8,
                    borderTop: '1px solid var(--color-border-subtle)',
                    fontSize: 'var(--text-xs-size)',
                    color: 'var(--color-warning)',
                    fontWeight: 600,
                  }}
                >
                  <AlertTriangle size={12} style={{ flexShrink: 0, marginTop: 1 }} />
                  File size mismatch â€” resuming may fail or corrupt the file.
                </div>
              )}
            </div>
          )}

          {inspectState === 'error' && (
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 6,
                fontSize: 'var(--text-xs-size)',
                color: 'var(--color-error)',
                fontWeight: 600,
              }}
            >
              <AlertTriangle size={12} />
              {errorMsg}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="dialog-footer">
          <button className="btn-secondary" onClick={onClose}>
            Cancel
          </button>
          <button
            className="btn-primary flex items-center gap-2"
            onClick={handleApply}
            disabled={!url.trim() || url.trim() === item?.url}
          >
            <ShieldCheck size={14} />
            Apply & Resume
          </button>
        </div>
      </div>
    </div>
  );
}
