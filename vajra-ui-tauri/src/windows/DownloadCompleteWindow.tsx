import React, { useState, useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import { CheckCircle2, FolderOpen, Play, X, Copy, Check, File, FileArchive, FileAudio, FileVideo, Settings as SettingsIcon } from 'lucide-react';
import { api, fmtBytes } from '../api';

const getFileIcon = (filename = '') => {
  const ext = filename.split('.').pop()?.toLowerCase();
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  if (['zip','rar','7z','tar','gz'].includes(ext as any)) return FileArchive;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  if (['mp3','wav','flac','aac','ogg','m4a'].includes(ext as any)) return FileAudio;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  if (['mp4','mkv','avi','mov','webm','wmv'].includes(ext as any)) return FileVideo;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  if (['exe','msi','apk','dmg','bat'].includes(ext as any)) return SettingsIcon;
  return File;
};

const InfoCard = ({ label, children }: { label: string; children: React.ReactNode }) => (
  <div className="card-subtle" style={{ padding: 'var(--sp-3)' }}>
    <span className="section-title" style={{ marginBottom: 4, display: 'block' }}>{label}</span>
    {children}
  </div>
);

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export default function DownloadCompleteWindow({ downloadId }: any) {
  const [download, setDownload] = useState<any>(() => {
    try {
      const cached = localStorage.getItem(`vajra_complete_init_${downloadId}`);
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
          localStorage.setItem(`vajra_complete_init_${downloadId}`, JSON.stringify(found));
        }
      }).catch(console.error);
    }
  }, [downloadId]);

  const close      = () => getCurrentWindow().close().catch(console.error);
  const openFolder = async () => { try { await invoke('show_in_explorer', { path: download.output_path }); } catch { /* ignore */ } };
  const openFile   = async () => { try { await invoke('open_file_path',    { path: download.output_path }); } catch { /* ignore */ } close(); };
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
      aria-label="Download Complete"
      style={{ display: 'flex', flexDirection: 'column', height: '100vh', overflow: 'hidden', fontFamily: 'var(--font-sans)', backgroundColor: 'var(--color-surface)', color: 'var(--color-text-1)', userSelect: 'none' }}
    >
      {/* Title bar */}
      <div className="drag-region window-titlebar">
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 'var(--text-xs-size)', fontWeight: 700, color: 'var(--color-success)' }}>
          <CheckCircle2 size={14} /> Download Complete
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
            onClick={openFile}
            title="Open File"
            className="file-icon-hover"
            style={{
              width: 48, height: 48, flexShrink: 0, cursor: 'pointer',
              backgroundColor: 'var(--color-brand-dim)', border: '1px solid var(--color-brand)',
              borderRadius: 'var(--radius-lg)', display: 'flex', alignItems: 'center', justifyContent: 'center',
              color: 'var(--color-brand)', transition: 'background-color var(--transition-fast)',
            }}
          >
            <Icon size={24} strokeWidth={1.5} />
          </div>
          <div style={{ minWidth: 0, flex: 1 }}>
            <span className="truncate" style={{ display: 'block', fontWeight: 700, fontSize: 'var(--text-md-size)', color: 'var(--color-text-1)' }} title={download.filename}>
              {download.filename}
            </span>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 'var(--text-xs-size)', color: 'var(--color-text-4)', marginTop: 2 }}>
              <span>{fmtBytes(download.total_bytes || download.bytes_done || 0)}</span>
              <span>•</span>
              {download.hash_result ? (
                <span className={download.hash_result.matched ? 'text-success' : 'text-error'} style={{ fontWeight: 600 }}>
                  {download.hash_result.matched ? 'Verified ✓' : 'Hash Mismatch ✕'}
                </span>
              ) : (
                <span>Unchecked</span>
              )}
            </div>
          </div>
        </div>

        {/* Metadata Details */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-2)' }}>
          <InfoCard label="Save Path">
            <div className="flex items-center gap-2 no-drag">
              <span className="truncate flex-1 font-semibold" style={{ fontSize: 'var(--text-xs-size)', color: 'var(--color-text-2)', userSelect: 'text' }} title={download.output_path}>
                {download.output_path}
              </span>
              <button className="btn-icon" style={{ width: 24, height: 24 }} onClick={() => copy(download.output_path, 'path')} title="Copy path">
                {copiedField === 'path' ? <Check size={12} style={{ color: 'var(--color-success)' }} /> : <Copy size={12} />}
              </button>
            </div>
          </InfoCard>

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
        <button className="btn-secondary" onClick={close}>Close</button>
        <button className="btn-secondary flex items-center gap-2" onClick={openFolder}><FolderOpen size={14} /> Show in Folder</button>
        <button className="btn-primary flex items-center gap-2" onClick={openFile}><Play size={14} /> Open File</button>
      </div>
    </div>
  );
}
