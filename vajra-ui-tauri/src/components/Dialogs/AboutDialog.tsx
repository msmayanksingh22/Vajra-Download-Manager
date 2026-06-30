import React, { useEffect, useState } from 'react';
import { X, CircleAlert } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { getVersion } from '@tauri-apps/api/app';
import { useDialogEscape } from '../../hooks/useDialogEscape';
import { useFocusTrap } from '../../hooks/useFocusTrap';
import { api } from '../../api';

export default function AboutDialog({
  onClose,
}: {
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [version, setVersion] = useState<string>('...');
  const [engineVersion, setEngineVersion] = useState<string>('...');
  useDialogEscape(onClose);
  const trapRef = useFocusTrap();

  useEffect(() => {
    getVersion().then(v => setVersion(v)).catch(() => setVersion('0.2.0'));
    api.health().then((res: any) => {
      setEngineVersion(res?.version || res?.daemon_version || 'N/A');
    }).catch(() => setEngineVersion('N/A'));
  }, []);

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div
        ref={trapRef}
        className="dialog-panel"
        style={{ width: 400 }}
        onClick={e => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="about-dialog-title"
      >
        {/* Header */}
        <div className="dialog-header">
          <div className="dialog-header-title" id="about-dialog-title">
            <CircleAlert size={16} style={{ color: 'var(--color-brand)' }} />
            {t('About Vajra', 'About Vajra')}
          </div>
          <button className="btn-icon" onClick={onClose} title="Close">
            <X size={15} />
          </button>
        </div>

        {/* Body */}
        <div className="dialog-body flex flex-col items-center" style={{ gap: 'var(--sp-4)', padding: 'var(--sp-5)' }}>
          {/* Logo container */}
          <div 
            className="flex items-center justify-center"
            style={{
              width: 100,
              height: 100,
              borderRadius: 'var(--radius-2xl)',
              backgroundColor: 'var(--color-surface-raised)',
              border: '1px solid var(--color-border)',
              boxShadow: 'var(--shadow-md)',
              padding: '16px',
            }}
          >
            <img 
              src="/logo.png" 
              alt="Vajra Logo" 
              style={{ 
                width: '100%', 
                height: '100%', 
                objectFit: 'contain',
              }} 
            />
          </div>

          <div className="text-center">
            <h3 style={{ fontSize: 'var(--text-lg-size)', fontWeight: 700, color: 'var(--color-text-1)', marginBottom: 2 }}>
              Vajra Download Manager
            </h3>
            <p style={{ fontSize: 'var(--text-xs-size)', color: 'var(--color-brand)', fontWeight: 600, letterSpacing: '0.05em', marginBottom: 12 }}>
              v{version} (Beta)
            </p>
            <p style={{ fontSize: 'var(--text-sm-size)', color: 'var(--color-text-2)', lineHeight: 1.5, maxWidth: '280px', margin: '0 auto' }}>
              A high-performance, concurrent download accelerator powered by a native Rust core.
            </p>
          </div>

          <div 
            className="card-subtle w-full text-center" 
            style={{ 
              padding: 'var(--sp-3)', 
              fontSize: 'var(--text-xs-size)', 
              color: 'var(--color-text-4)',
              backgroundColor: 'rgba(255, 255, 255, 0.01)',
              borderColor: 'var(--color-border-subtle)',
            }}
          >
            <p>© 2026 Vajra Project. Open Source under MIT License.</p>
            <p style={{ marginTop: 2 }}>Rust Engine v{engineVersion}</p>
          </div>
        </div>

        {/* Footer */}
        <div className="dialog-footer" style={{ justifyContent: 'center' }}>
          <button className="btn-secondary" style={{ minWidth: 100 }} onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}