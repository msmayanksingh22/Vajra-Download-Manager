import React, { useRef, useEffect } from 'react';
import { X, Keyboard, FolderDown, Compass, Wand2 } from 'lucide-react';
import { useFocusTrap } from '../../hooks/useFocusTrap';
import { useDialogEscape } from '../../hooks/useDialogEscape';

interface ShortcutGroup {
  title: string;
  icon: React.ElementType;
  shortcuts: Array<{ keys: string[]; description: string }>;
}

const SHORTCUT_GROUPS: ShortcutGroup[] = [
  {
    title: 'Downloads',
    icon: FolderDown,
    shortcuts: [
      { keys: ['Ctrl', 'N'], description: 'Add new download' },
      { keys: ['Del'], description: 'Delete selected' },
      { keys: ['Ctrl', 'P'], description: 'Pause selected' },
      { keys: ['Ctrl', 'R'], description: 'Resume selected' },
      { keys: ['Ctrl', 'A'], description: 'Select all' },
      { keys: ['Ctrl', 'I'], description: 'Open properties' },
      { keys: ['Enter'], description: 'Open progress window' },
    ],
  },
  {
    title: 'Navigation',
    icon: Compass,
    shortcuts: [
      { keys: ['F5'], description: 'Refresh download list' },
      { keys: ['Ctrl', 'F'], description: 'Focus search' },
      { keys: ['Esc'], description: 'Clear search / close dialog' },
      { keys: ['↑', '↓'], description: 'Navigate list' },
      { keys: ['Shift', 'Click'], description: 'Range select' },
      { keys: ['Ctrl', 'Click'], description: 'Multi-select' },
    ],
  },
  {
    title: 'App',
    icon: Wand2,
    shortcuts: [
      { keys: ['Ctrl', 'O'], description: 'Open options' },
      { keys: ['Ctrl', ','], description: 'Open options (alt)' },
      { keys: ['Ctrl', 'G'], description: 'Open grabber' },
      { keys: ['F1'], description: 'Open help' },
      { keys: ['Ctrl', 'Q'], description: 'Quit application' },
    ],
  },
];

const Kbd = ({ children }: { children: string }) => (
  <kbd
    style={{
      display: 'inline-flex',
      alignItems: 'center',
      justifyContent: 'center',
      padding: '2px 6px',
      borderRadius: 'var(--radius-sm)',
      backgroundColor: 'var(--color-surface-elevated)',
      border: '1px solid var(--color-border)',
      borderBottom: '2px solid var(--color-border)',
      fontSize: 'var(--text-xs-size)',
      fontFamily: 'var(--font-mono)',
      fontWeight: 600,
      color: 'var(--color-text-2)',
      lineHeight: 1,
      whiteSpace: 'nowrap',
      userSelect: 'none',
    }}
  >
    {children}
  </kbd>
);

interface HelpDialogProps {
  onClose: () => void;
}

export default function HelpDialog({ onClose }: HelpDialogProps) {
  const trapRef = useFocusTrap();
  const closeBtnRef = useRef<HTMLButtonElement>(null);
  useDialogEscape(onClose);

  const handleOverlayClick = (e: React.MouseEvent) => {
    if (e.target === e.currentTarget) onClose();
  };

  useEffect(() => {
    // Auto-focus close button on mount for immediate Escape dismissal
    setTimeout(() => closeBtnRef.current?.focus(), 50);
  }, []);

  return (
    <div
      className="dialog-overlay"
      role="dialog"
      aria-modal="true"
      aria-labelledby="help-dialog-title"
      onClick={handleOverlayClick}
    >
      <div ref={trapRef} className="dialog-panel" style={{ maxWidth: 620, width: '90vw' }}>
        {/* Header */}
        <div className="dialog-header">
          <div className="dialog-header-title" id="help-dialog-title">
            <Keyboard size={16} style={{ color: 'var(--color-brand)' }} />
            Keyboard Shortcuts
          </div>
          <button
            ref={closeBtnRef}
            className="btn-icon"
            onClick={onClose}
            title="Close"
            aria-label="Close help dialog"
          >
            <X size={15} />
          </button>
        </div>

        {/* Body — two-column grid */}
        <div
          className="dialog-body"
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(260px, 1fr))',
            gap: 'var(--sp-4)',
            maxHeight: '70vh',
            overflowY: 'auto',
          }}
        >
          {SHORTCUT_GROUPS.map(({ title, icon: Icon, shortcuts }) => (
            <div key={title} className="card-subtle" style={{ padding: 'var(--sp-4)' }}>
              <div
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                  marginBottom: 'var(--sp-3)',
                  paddingBottom: 'var(--sp-2)',
                  borderBottom: '1px solid var(--color-border-subtle)',
                }}
              >
                <Icon size={14} style={{ color: 'var(--color-brand)', flexShrink: 0 }} />
                <span className="section-title" style={{ margin: 0 }}>
                  {title}
                </span>
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                {shortcuts.map(({ keys, description }) => (
                  <div
                    key={description}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'space-between',
                      gap: 8,
                    }}
                  >
                    <span
                      style={{
                        fontSize: 'var(--text-sm-size)',
                        color: 'var(--color-text-2)',
                        flex: 1,
                      }}
                    >
                      {description}
                    </span>
                    <div style={{ display: 'flex', gap: 3, flexShrink: 0, alignItems: 'center' }}>
                      {keys.map((k, i) => (
                        <React.Fragment key={k}>
                          {i > 0 && (
                            <span
                              style={{
                                fontSize: 'var(--text-xs-size)',
                                color: 'var(--color-text-4)',
                              }}
                            >
                              +
                            </span>
                          )}
                          <Kbd>{k}</Kbd>
                        </React.Fragment>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>

        {/* Footer */}
        <div className="dialog-footer">
          <span style={{ fontSize: 'var(--text-xs-size)', color: 'var(--color-text-4)' }}>
            Press <Kbd>Esc</Kbd> or click outside to close
          </span>
          <button className="btn-secondary" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
