import React from 'react';
import { X, Blocks, CircleCheckBig } from 'lucide-react';
import { useFocusTrap } from '../../hooks/useFocusTrap';
import { useDialogEscape } from '../../hooks/useDialogEscape';

interface OnboardingModalProps {
  onClose: () => void;
}

const Step = ({
  number,
  title,
  children,
}: {
  number: number;
  title: string;
  children: React.ReactNode;
}) => (
  <div style={{ display: 'flex', gap: 'var(--sp-3)', alignItems: 'flex-start' }}>
    <div
      style={{
        width: 28,
        height: 28,
        borderRadius: '50%',
        background: 'var(--color-brand)',
        color: '#fff',
        display: 'grid',
        placeItems: 'center',
        fontSize: 'var(--text-sm-size)',
        fontWeight: 700,
        flexShrink: 0,
      }}
    >
      {number}
    </div>
    <div style={{ flex: 1 }}>
      <h3
        style={{
          margin: 0,
          fontSize: 'var(--text-sm-size)',
          fontWeight: 600,
          color: 'var(--color-text-1)',
        }}
      >
        {title}
      </h3>
      <div
        style={{
          marginTop: 'var(--sp-2)',
          color: 'var(--color-text-2)',
          fontSize: 'var(--text-sm-size)',
          lineHeight: 1.5,
        }}
      >
        {children}
      </div>
    </div>
  </div>
);

export default function OnboardingModal({ onClose }: OnboardingModalProps) {
  const trapRef = useFocusTrap();
  useDialogEscape(onClose);

  return (
    <div
      className="dialog-overlay"
      role="dialog"
      aria-modal="true"
      aria-labelledby="onboarding-title"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={trapRef}
        className="dialog-panel"
        style={{ maxWidth: 520, width: '90vw' }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="dialog-header">
          <div className="dialog-header-title" id="onboarding-title">
            <Blocks size={18} style={{ color: 'var(--color-brand)' }} />
            Install the Browser Extension
          </div>
          <button
            className="btn-icon"
            onClick={onClose}
            title="Close"
            aria-label="Close onboarding dialog"
          >
            <X size={15} />
          </button>
        </div>

        <div
          className="dialog-body"
          style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-4)' }}
        >
          <p
            style={{
              margin: 0,
              color: 'var(--color-text-2)',
              fontSize: 'var(--text-sm-size)',
              lineHeight: 1.5,
            }}
          >
            Vajra can intercept downloads right from your browser. Finish setup by loading the
            unpacked extension once.
          </p>

          <Step number={1} title="Open Chrome/Edge Extensions">
            Go to{' '}
            <code
              style={{
                background: 'var(--color-surface-elevated)',
                padding: '2px 4px',
                borderRadius: 'var(--radius-sm)',
              }}
            >
              chrome://extensions
            </code>{' '}
            or{' '}
            <code
              style={{
                background: 'var(--color-surface-elevated)',
                padding: '2px 4px',
                borderRadius: 'var(--radius-sm)',
              }}
            >
              edge://extensions
            </code>
            .
          </Step>

          <Step number={2} title="Enable Developer Mode">
            Toggle <strong>Developer mode</strong> in the top-right corner.
          </Step>

          <Step number={3} title="Load Unpacked">
            Click <strong>Load unpacked</strong>, then select the extension folder inside your Vajra
            installation:
            <code
              style={{
                display: 'block',
                marginTop: 'var(--sp-2)',
                background: 'var(--color-surface-elevated)',
                padding: 'var(--sp-2) var(--sp-3)',
                borderRadius: 'var(--radius-md)',
                fontSize: 'var(--text-xs-size)',
                wordBreak: 'break-all',
              }}
            >
              resources/extension
            </code>
          </Step>

          <Step number={4} title="Start Browsing">
            The extension will auto-connect to Vajra and intercept downloads. Hold <kbd>Alt</kbd>{' '}
            while clicking a link to bypass interception.
          </Step>
        </div>

        <div className="dialog-footer">
          <button
            className="btn-primary"
            onClick={onClose}
            style={{ display: 'flex', alignItems: 'center', gap: 6 }}
          >
            <CircleCheckBig size={15} /> Got it
          </button>
        </div>
      </div>
    </div>
  );
}
