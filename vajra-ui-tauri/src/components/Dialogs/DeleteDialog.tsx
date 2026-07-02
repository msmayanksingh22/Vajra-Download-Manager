import React, { useState } from 'react';
import { X, Trash2, AlertTriangle, ListX } from 'lucide-react';
import { useDialogEscape } from '../../hooks/useDialogEscape';
import { useFocusTrap } from '../../hooks/useFocusTrap';

export default function DeleteDialog({
  count,
  onClose,
  onConfirm,
}: {
  count: number;
  onClose: () => void;
  onConfirm: (deleteFromDisk: boolean, remember: boolean) => void;
}) {
  const [rememberChoice, setRememberChoice] = useState(false);
  useDialogEscape(onClose);
  const trapRef = useFocusTrap();

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div
        ref={trapRef}
        className="dialog-panel"
        style={{ width: 380 }}
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="delete-dialog-title"
      >
        {/* Header */}
        <div className="dialog-header">
          <div
            className="dialog-header-title"
            id="delete-dialog-title"
            style={{ color: 'var(--color-error)' }}
          >
            <AlertTriangle size={16} style={{ color: 'var(--color-error)' }} />
            Confirm Deletion
          </div>
          <button className="btn-icon" onClick={onClose} title="Close">
            <X size={15} />
          </button>
        </div>

        {/* Body */}
        <div className="dialog-body" style={{ gap: 'var(--sp-3)' }}>
          {/* Message */}
          <div
            className="card-subtle text-center"
            style={{ padding: 'var(--sp-4)', borderColor: 'var(--color-error-dim)' }}
          >
            <p
              style={{
                fontWeight: 600,
                fontSize: 'var(--text-base-size)',
                color: 'var(--color-text-1)',
              }}
            >
              Delete {count === 1 ? 'this download' : `these ${count} downloads`}?
            </p>
            <p
              style={{
                fontSize: 'var(--text-sm-size)',
                color: 'var(--color-text-3)',
                marginTop: 4,
              }}
            >
              Choose whether to remove from the list only, or also delete the file from disk.
            </p>
          </div>

          {/* Remember checkbox */}
          <label
            className="card-subtle flex items-center gap-3"
            style={{ padding: 'var(--sp-3) var(--sp-4)', cursor: 'default' }}
          >
            <input
              type="checkbox"
              checked={rememberChoice}
              onChange={(e) => setRememberChoice(e.target.checked)}
              style={{
                accentColor: 'var(--color-brand)',
                width: 14,
                height: 14,
                cursor: 'default',
              }}
            />
            <span style={{ fontSize: 'var(--text-sm-size)', color: 'var(--color-text-2)' }}>
              Remember my choice
            </span>
          </label>
        </div>

        {/* Footer — three action buttons */}
        <div className="dialog-footer" style={{ gap: 'var(--sp-2)' }}>
          <button className="btn-secondary" onClick={onClose}>
            Cancel
          </button>
          <button
            className="btn-secondary flex items-center gap-1.5"
            onClick={() => onConfirm(false, rememberChoice)}
            title="Remove from list but keep the file on disk"
          >
            <ListX size={14} />
            Remove from List
          </button>
          <button
            className="btn-danger flex items-center gap-1.5"
            onClick={() => onConfirm(true, rememberChoice)}
            title="Delete the file from disk permanently"
          >
            <Trash2 size={14} />
            Delete from Disk
          </button>
        </div>
      </div>
    </div>
  );
}
