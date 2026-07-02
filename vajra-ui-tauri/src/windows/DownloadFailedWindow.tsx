import React, { useState, useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { emit } from '@tauri-apps/api/event';
import { AlertCircle, RefreshCw, X, Copy, Check, File, FileArchive, FileAudio, FileVideo, Settings as SettingsIcon } from 'lucide-react';
import { api, fmtBytes } from '../api';

const getFileIcon = (filename = '') => {
  const ext = filename.split('.').pop()?.toLowerCase();
  if (['zip','rar','7z','tar','gz'].includes(ext as any)) return FileArchive;
  if (['mp3','wav','flac','aac','ogg','m4a'].includes(ext as any)) return FileAudio;
  if (['mp4','mkv','avi','mov','webm','wmv'].includes(ext as any)) return FileVideo;
  if (['exe','msi','apk','dmg','bat'].includes(ext as any)) return SettingsIcon;
  return File;
};

const InfoCard = ({ label, children }: { label: string; children: React.ReactNode }) => (
  <div className="card-subtle" style={{ padding: 'var(--sp-3)' }}>
    <span className="section-title" style={{ marginBottom: 4, display: 'block' }}>{label}</span>
    {children}
  </div>
);

export default function DownloadFailedWindow({ downloadId }: { downloadId: string }) {
  const [download, setDownload] = useState<any>(() => {
    try {
      const cached = localStorage.getItem(`vajra_failed_init_${downloadId}`);
      if (cached) return JSON.parse(cached);
    } catch (e) {
      // ignore
    }
    return null;
  });
  const [copiedField, setCopiedField] = useState<any>(null);

  useEffect(() => {
    getCurrentWindow().show().catch(console.error);
    if (downloadId) {
      api.get(downloadId).then(found => {
        if (found) {
          setDownload(found);
          localStorage.setItem(`vajra_failed_init_${downloadId}`, JSON.stringify(found));
        }
      }).catch(console.error);
    }
  }, [downloadId]);

  const close = () => getCurrentWindow().close().catch(console.error);

  const retry = async () => {
    try {
      await api.patch(downloadId, { action: 'resume' });
      try {
        await emit('open-progress-window', downloadId);
      } catch (e) {
        console.error("Failed to emit open-progress-window:", e);
      }
    } catch (e) {
      console.error("Failed to retry download:", e);
    }
    close();
  };

  const copy = (text: string, field: string) => {
    navigator.clipboard.writeText(text);
    setCopiedField(field);
    setTimeout(() => setCopiedField(null), 1500);
  };

  if (!download) return (
    <div style={{ backgroundColor: 'var(--color-surface)', height: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--color-text-4)', fontFamily: 'var(--font-sans)', fontSize: 'var(--text-sm-size)' }}>
      Loading…
    </div>
  );

  const Icon = getFileIcon(download.filename || download.file_name);

  return (
    <div
      className="window-mount"
      role="dialog"
      aria-modal="true"
      aria-label="Download Failed"
      style={{ display: 'flex', flexDirection: 'column', height: '100vh', overflow: 'hidden', fontFamily: 'var(--font-sans)', backgroundColor: 'var(--color-surface)', color: 'var(--color-text-1)', userSelect: 'none' }}
    >
      {/* Title bar */}
      <div className="drag-region window-titlebar">
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 'var(--text-xs-size)', fontWeight: 700, color: 'var(--color-error)' }}>
          <AlertCircle size={14} /> Download Failed
        </div>
        <button className="btn-icon no-drag" onClick={close} style={{ width: 28, height: 28 }} title="Close">
          <X size={14} />
        </button>
      </div>

      {/* Body */}
      <div style={{ flex: 1, padding: 'var(--sp-4)', overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: 'var(--sp-3)' }}>
        {/* Header Section: Icon + Filename */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--sp-3)', backgroundColor: 'var(--color-surface-raised)', padding: 'var(--sp-3)', borderRadius: 'var(--radius-lg)', border: '1px solid var(--color-border-subtle)' }}>
          <div
            style={{
              width: 48, height: 48, flexShrink: 0, position: 'relative',
              backgroundColor: 'var(--color-error-dim)', border: '1px solid var(--color-error)',
              borderRadius: 'var(--radius-lg)', display: 'flex', alignItems: 'center', justifyContent: 'center',
              color: 'var(--color-error)',
            }}
          >
            <Icon size={24} strokeWidth={1.5} />
            <div style={{
              position: 'absolute', bottom: -2, right: -2,
              width: 14, height: 14, borderRadius: '50%',
              backgroundColor: 'var(--color-error)',
              border: '1.5px solid var(--color-surface)',
              display: 'flex', alignItems: 'center', justifyContent: 'center',
            }}>
              <AlertCircle size={8} style={{ color: '#fff' }} />
            </div>
          </div>
          <div style={{ minWidth: 0, flex: 1 }}>
            <span className="truncate" style={{ display: 'block', fontWeight: 700, fontSize: 'var(--text-md-size)', color: 'var(--color-text-1)' }} title={download.filename}>
              {download.filename}
            </span>
            <span style={{ fontSize: 'var(--text-xs-size)', color: 'var(--color-text-4)', marginTop: 2, display: 'block' }}>
              {fmtBytes(download.total_bytes || download.bytes_done || 0)}
            </span>
          </div>
        </div>

        {/* Error Alert Box */}
        <div className="card-subtle" style={{ padding: 'var(--sp-3)', backgroundColor: 'var(--color-error-dim)', borderColor: 'var(--color-error)', borderRadius: 'var(--radius-lg)', display: 'flex', flexDirection: 'column', gap: 4 }}>
          <span className="section-title" style={{ fontSize: 'var(--text-xs-size)', fontWeight: 700, color: 'var(--color-error)', textTransform: 'uppercase', letterSpacing: '0.05em' }}>Error Message</span>
          <div style={{ display: 'flex', alignItems: 'flex-start', gap: 'var(--sp-2)', minWidth: 0 }}>
            <span style={{ flex: 1, fontSize: 'var(--text-sm-size)', color: 'var(--color-error)', wordBreak: 'break-word', userSelect: 'text', lineHeight: '1.4' }}>
              {download.error || 'Unknown failure'}
            </span>
            <button className="btn-icon" style={{ width: 24, height: 24, flexShrink: 0, color: 'var(--color-error)' }} onClick={() => copy(download.error || 'Unknown failure', 'error')} title="Copy Error">
              {copiedField === 'error' ? <Check size={12} style={{ color: 'var(--color-success)' }} /> : <Copy size={12} />}
            </button>
          </div>
        </div>

        {/* Metadata Details */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-2)' }}>
          <InfoCard label="Source URL">
            <div className="flex items-center gap-2 no-drag">
              <span className="truncate flex-1 font-semibold" style={{ fontSize: 'var(--text-xs-size)', color: 'var(--color-text-3)', userSelect: 'text' }} title={download.url}>
                {download.url}
              </span>
              <button className="btn-icon" style={{ width: 24, height: 24 }} onClick={() => copy(download.url, 'url')} title="Copy URL">
                {copiedField === 'url' ? <Check size={12} style={{ color: 'var(--color-success)' }} /> : <Copy size={12} />}
              </button>
            </div>
          </InfoCard>
        </div>
      </div>

      {/* Footer */}
      <div className="no-drag" style={{ display: 'flex', alignItems: 'center', justifyContent: 'flex-end', gap: 'var(--sp-2)', padding: '0 var(--sp-4)', height: 48, borderTop: '1px solid var(--color-border)', backgroundColor: 'var(--color-surface-raised)', flexShrink: 0 }}>
        <button className="btn-secondary" onClick={close} aria-label="Close window">Close</button>
        <button className="btn-primary flex items-center gap-2" style={{ backgroundColor: 'var(--color-error)', borderColor: 'var(--color-error)' }} onClick={retry}><RefreshCw size={14} /> Retry Download</button>
      </div>
    </div>
  );
}
