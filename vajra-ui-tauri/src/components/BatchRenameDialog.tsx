import React, { useState } from 'react';
import { api } from '../api';
import { DownloadInfo } from '../types';

interface BatchRenameDialogProps {
  items: DownloadInfo[];
  onClose: () => void;
}

export default function BatchRenameDialog({ items, onClose }: BatchRenameDialogProps) {
  const [pattern, setPattern] = useState('{name}_{index}.{ext}');

  const handleApply = async () => {
    // Send a patch request for each item to update filename
    for (let i = 0; i < items.length; i++) {
      const item = items[i];
      const dotIndex = item.filename.lastIndexOf('.');
      const name = dotIndex >= 0 ? item.filename.substring(0, dotIndex) : item.filename;
      const ext = dotIndex >= 0 ? item.filename.substring(dotIndex + 1) : '';

      const newName = pattern
        .replace('{name}', name)
        .replace('{ext}', ext)
        .replace('{index}', (i + 1).toString());

      try {
        await api.patch(item.id, { filename: newName });
      } catch (err) {
        console.error(`Failed to rename ${item.filename} to ${newName}:`, err);
      }
    }
    onClose();
  };

  return (
    <div
      className="dialog-overlay"
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        background: 'rgba(0,0,0,0.5)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1000,
      }}
    >
      <div style={{ background: 'var(--color-surface)', padding: 24, borderRadius: 8, width: 400 }}>
        <h2 style={{ margin: '0 0 16px 0', fontSize: 18 }}>Batch Rename</h2>
        <p style={{ color: 'var(--color-text-3)', fontSize: 13, marginBottom: 16 }}>
          Use {'{name}'}, {'{ext}'}, and {'{index}'} variables.
        </p>
        <input
          value={pattern}
          onChange={(e) => setPattern(e.target.value)}
          style={{
            width: '100%',
            padding: '8px',
            background: 'var(--color-surface-raised)',
            border: '1px solid var(--color-border)',
            color: '#fff',
            borderRadius: 4,
            marginBottom: 16,
          }}
        />
        <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8 }}>
          <button
            onClick={onClose}
            style={{
              padding: '6px 12px',
              background: 'transparent',
              border: '1px solid var(--color-border)',
              color: '#fff',
              borderRadius: 4,
              cursor: 'pointer',
            }}
          >
            Cancel
          </button>
          <button
            onClick={handleApply}
            style={{
              padding: '6px 12px',
              background: 'var(--color-brand)',
              border: 'none',
              color: '#000',
              borderRadius: 4,
              cursor: 'pointer',
              fontWeight: 600,
            }}
          >
            Rename {items.length} items
          </button>
        </div>
      </div>
    </div>
  );
}
