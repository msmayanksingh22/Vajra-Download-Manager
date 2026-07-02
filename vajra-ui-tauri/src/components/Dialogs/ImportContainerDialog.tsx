import React, { useState, useEffect } from 'react';
import {
  X,
  FileCode2,
  Download,
  CheckSquare,
  Square,
  Folder,
  AlertTriangle,
  Loader2,
} from 'lucide-react';
import { api } from '../../api';
import { open } from '@tauri-apps/plugin-dialog';
import { useDialogEscape } from '../../hooks/useDialogEscape';
import { useFocusTrap } from '../../hooks/useFocusTrap';

export default function ImportContainerDialog({
  onClose,
  onImport,
}: {
  onClose: () => void;
  onImport: (links: string[], outputDir?: string) => void;
}) {
  const [file, setFile] = useState<File | null>(null);
  const [links, setLinks] = useState<string[]>([]);
  const [selectedLinks, setSelectedLinks] = useState<Record<string, boolean>>({});
  const [outputDir, setOutputDir] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useDialogEscape(onClose);
  const trapRef = useFocusTrap();

  const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const selectedFile = e.target.files?.[0];
    if (!selectedFile) return;

    setFile(selectedFile);
    setIsLoading(true);
    setError(null);

    try {
      const decrypted = await api.decrypt(selectedFile);
      setLinks(decrypted);
      const initialSelected: Record<string, boolean> = {};
      decrypted.forEach((link) => {
        initialSelected[link] = true;
      });
      setSelectedLinks(initialSelected);
    } catch (err: any) {
      console.error(err);
      setError(err.message || 'Failed to decrypt container file');
      setFile(null);
    } finally {
      setIsLoading(false);
    }
  };

  const handleBrowseFolder = async () => {
    try {
      const selected = await open({ directory: true });
      if (selected) {
        setOutputDir(selected as string);
      }
    } catch (err) {
      console.error('Folder picker not available', err);
    }
  };

  const toggleLink = (link: string) => {
    setSelectedLinks((prev) => ({
      ...prev,
      [link]: !prev[link],
    }));
  };

  const toggleAll = (checked: boolean) => {
    const updated: Record<string, boolean> = {};
    links.forEach((link) => {
      updated[link] = checked;
    });
    setSelectedLinks(updated);
  };

  const handleAdd = () => {
    const urlsToAdd = links.filter((link) => selectedLinks[link]);
    if (urlsToAdd.length === 0) return;
    onImport(urlsToAdd, outputDir.trim() || undefined);
  };

  const hasSelected = Object.values(selectedLinks).some(Boolean);

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div
        ref={trapRef}
        className="dialog-panel"
        style={{ width: 560, maxHeight: '90vh', display: 'flex', flexDirection: 'column' }}
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="import-dialog-title"
      >
        {/* Header */}
        <div className="dialog-header" style={{ flexShrink: 0 }}>
          <div className="dialog-header-title" id="import-dialog-title">
            <FileCode2 size={16} style={{ color: 'var(--color-brand)' }} />
            Import DLC / RSDF Container
          </div>
          <button className="btn-icon" onClick={onClose} title="Close">
            <X size={15} />
          </button>
        </div>

        {/* Body */}
        <div
          className="dialog-body"
          style={{ flex: 1, overflowY: 'auto', gap: 'var(--sp-4)', padding: 'var(--sp-4)' }}
        >
          {error && (
            <div
              className="card-subtle flex items-start gap-2.5"
              style={{
                borderColor: 'var(--color-error-dim)',
                backgroundColor: 'var(--color-error-muted)',
                padding: 'var(--sp-3)',
              }}
            >
              <AlertTriangle
                size={16}
                style={{ color: 'var(--color-error)', flexShrink: 0, marginTop: 2 }}
              />
              <div style={{ fontSize: 'var(--text-sm-size)', color: 'var(--color-text-1)' }}>
                {error}
              </div>
            </div>
          )}

          {!file && !isLoading && (
            <div
              className="card-subtle flex flex-col items-center justify-center border-dashed"
              style={{
                minHeight: 160,
                gap: 'var(--sp-3)',
                cursor: 'pointer',
                padding: 'var(--sp-6)',
              }}
              onClick={() => document.getElementById('file-upload-input')?.click()}
            >
              <FileCode2 size={36} style={{ color: 'var(--color-text-4)' }} />
              <div style={{ textAlign: 'center' }}>
                <p
                  style={{
                    fontWeight: 600,
                    color: 'var(--color-text-1)',
                    fontSize: 'var(--text-sm-size)',
                  }}
                >
                  Click to upload a container file
                </p>
                <p
                  style={{
                    fontSize: 'var(--text-xs-size)',
                    color: 'var(--color-text-3)',
                    marginTop: 4,
                  }}
                >
                  Supports DLC and RSDF files
                </p>
              </div>
              <input
                id="file-upload-input"
                type="file"
                accept=".dlc,.rsdf"
                onChange={handleFileChange}
                style={{ display: 'none' }}
              />
            </div>
          )}

          {isLoading && (
            <div
              className="flex flex-col items-center justify-center"
              style={{ minHeight: 160, gap: 'var(--sp-3)' }}
            >
              <Loader2 size={32} className="animate-spin" style={{ color: 'var(--color-brand)' }} />
              <span style={{ fontSize: 'var(--text-sm-size)', color: 'var(--color-text-2)' }}>
                Decrypting container links...
              </span>
            </div>
          )}

          {file && links.length > 0 && (
            <div className="flex flex-col gap-4 animate-fade-in" style={{ flex: 1 }}>
              {/* Output Directory */}
              <div className="form-group">
                <label className="form-label" style={{ fontWeight: 600 }}>
                  Save Location
                </label>
                <div className="flex gap-2">
                  <input
                    type="text"
                    className="input-field"
                    style={{ flex: 1 }}
                    value={outputDir}
                    onChange={(e) => setOutputDir(e.target.value)}
                    placeholder="Default Directory"
                  />
                  <button className="btn-secondary" onClick={handleBrowseFolder} title="Browse">
                    <Folder size={14} />
                  </button>
                </div>
              </div>

              {/* Checklist toolbar */}
              <div
                className="flex justify-between items-center"
                style={{
                  borderBottom: '1px solid var(--color-border-subtle)',
                  paddingBottom: 'var(--sp-2)',
                }}
              >
                <div
                  style={{
                    fontSize: 'var(--text-xs-size)',
                    color: 'var(--color-text-3)',
                    fontWeight: 600,
                  }}
                >
                  {links.length} Links Decrypted
                </div>
                <div className="flex gap-3">
                  <button
                    className="btn-ghost text-xs px-2.5 py-1 rounded-md"
                    onClick={() => toggleAll(true)}
                  >
                    Select All
                  </button>
                  <button
                    className="btn-ghost text-xs px-2.5 py-1 rounded-md"
                    onClick={() => toggleAll(false)}
                  >
                    Deselect All
                  </button>
                </div>
              </div>

              {/* Links list */}
              <div
                className="card-subtle flex flex-col overflow-y-auto"
                style={{ maxHeight: 200, padding: 0, overflowX: 'hidden' }}
              >
                {links.map((link, idx) => (
                  <div
                    key={idx}
                    className="flex items-center gap-3 px-4 py-2.5 hover:bg-[var(--color-surface-raised)] transition-colors"
                    style={{
                      borderBottom:
                        idx < links.length - 1 ? '1px solid var(--color-border-subtle)' : 'none',
                      cursor: 'default',
                    }}
                    onClick={() => toggleLink(link)}
                  >
                    <button
                      className="btn-icon"
                      style={{
                        width: 16,
                        height: 16,
                        flexShrink: 0,
                        padding: 0,
                        color: selectedLinks[link] ? 'var(--color-brand)' : 'var(--color-text-4)',
                      }}
                    >
                      {selectedLinks[link] ? <CheckSquare size={14} /> : <Square size={14} />}
                    </button>
                    <span
                      style={{
                        fontSize: 'var(--text-xs-size)',
                        color: selectedLinks[link] ? 'var(--color-text-1)' : 'var(--color-text-3)',
                        whiteSpace: 'nowrap',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        flex: 1,
                      }}
                    >
                      {link}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="dialog-footer" style={{ flexShrink: 0 }}>
          <button className="btn-secondary" onClick={onClose}>
            Cancel
          </button>
          <button
            className="btn-primary flex items-center gap-2"
            onClick={handleAdd}
            disabled={!file || links.length === 0 || !hasSelected}
          >
            <Download size={14} />
            Add {Object.values(selectedLinks).filter(Boolean).length} Downloads
          </button>
        </div>
      </div>
    </div>
  );
}
