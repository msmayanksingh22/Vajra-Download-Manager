import React, { useState, useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { emit, listen } from '@tauri-apps/api/event';
// eslint-disable-next-line @typescript-eslint/no-unused-vars
import {
  X,
  Lock,
  Link as LinkIcon,
  Folder,
  Download,
  Search,
  List,
  AlertTriangle,
  AlertCircle,
} from 'lucide-react';
// eslint-disable-next-line @typescript-eslint/no-unused-vars
import { cn } from '../utils';
import { api, fmtBytes } from '../api';
import { useTranslation } from 'react-i18next';

export default function AddUrlWindow({
  initialUrl = '',
  initialFilename = '',
}: {
  initialUrl?: string;
  initialFilename?: string;
}) {
  const { t } = useTranslation();
  const [url, setUrl] = useState(initialUrl);
  const [useAuth, setUseAuth] = useState(false);
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');

  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [speedLimitKb, setSpeedLimitKb] = useState('');
  const [outputDir, setOutputDir] = useState('');
  const [expectedHash, setExpectedHash] = useState('');
  const [autoExtract, setAutoExtract] = useState(false);
  const [postProcessingScript, setPostProcessingScript] = useState('');
  const [startLater, setStartLater] = useState(false);
  const [useHttp3, setUseHttp3] = useState(false);

  const [inspectState, setInspectState] = useState('idle');
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [inspectData, setInspectData] = useState<any>(null);
  const [filename, setFilename] = useState(initialFilename);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [downloads, setDownloads] = useState<any[]>([]);
  const [showDuplicatePrompt, setShowDuplicatePrompt] = useState(false);
  const [inlineError, setInlineError] = useState<string | null>(null);

  const cleanUrl = (rawUrl: string): string => {
    let u = rawUrl.trim();
    // Remove trailing commas, periods, quotes, or whitespace
    u = u.replace(/[,."'\s]+$/, '');
    return u;
  };

  useEffect(() => {
    getCurrentWindow().show().catch(console.error);

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const unlistenPromise = listen('update-add-url', (e: any) => {
      if (e.payload?.url) setUrl(e.payload.url);
      if (e.payload?.filename) setFilename(e.payload.filename);
    });

    return () => {
      unlistenPromise.then((fn) => fn()).catch(console.error);
    };
  }, []);

  useEffect(() => {
    // Fetch downloads to check for duplicates
    api
      .list()
      .then((res) => setDownloads(res.items || []))
      .catch(console.error);
    api.config().catch(console.error);
    // Focus the window on launch so user can interact immediately
    getCurrentWindow().setFocus().catch(console.error);
  }, []);

  // Load vault credentials on mount for autofill
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [vaultLogins, setVaultLogins] = useState<any[]>([]);
  useEffect(() => {
    api
      .getVault()
      .then(setVaultLogins)
      .catch(() => {
        // Fallback to localStorage for offline/daemon-down scenarios
        try {
          const saved = localStorage.getItem('vajra_site_logins');
          if (saved) setVaultLogins(JSON.parse(saved));
        } catch {
          /* ignore */
        }
      });
  }, []);

  // Auto-fill credentials from vault matching url domain
  useEffect(() => {
    if (!url.trim()) return;
    try {
      const parsedUrl = new URL(cleanUrl(url));
      const host = parsedUrl.hostname.toLowerCase();

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const match = vaultLogins.find((item: any) => {
        const itemHost = (item.domain || item.host || '').toLowerCase();
        return host === itemHost || host.endsWith('.' + itemHost);
      });

      if (match) {
        setUseAuth(true);
        setUsername(match.username || match.user || '');
        setPassword(match.password || match.pass || '');
        setShowAdvanced(true);
      }
    } catch (e) {
      // Ignore URL parsing errors while typing
    }
  }, [url, vaultLogins]);

  useEffect(() => {
    const cleaned = cleanUrl(url);
    if (!cleaned || !cleaned.startsWith('http')) {
      setInspectState('idle');
      setInspectData(null);
      setFilename('');
      return;
    }

    try {
      let urlFile = cleaned.split('/').pop() || 'download';
      urlFile = urlFile.split('?')[0].split('#')[0];
      if (urlFile) setFilename(decodeURIComponent(urlFile));
    } catch (e) {
      /* ignore */
    }

    const timer = setTimeout(async () => {
      setInspectState('loading');
      try {
        const res = await api.inspect(cleaned);
        setInspectData(res);
        if (res.filename) setFilename(res.filename);
        setInspectState('success');
      } catch (e) {
        setInspectState('error');
      }
    }, 800);

    return () => clearTimeout(timer);
  }, [url]);

  const handleClose = () => {
    getCurrentWindow().close().catch(console.error);
  };

  const urlLines = url
    .split('\n')
    .map((s) => cleanUrl(s))
    .filter(Boolean);
  const isBatch = urlLines.length > 1;

  const submitDownload = async (forceDuplicate: boolean = false) => {
    for (const singleUrl of urlLines) {
      let currentFilename = filename.trim();
      if (!currentFilename && !isBatch) {
        let urlFile = singleUrl.split('/').pop() || 'download';
        urlFile = urlFile.split('?')[0].split('#')[0];
        try {
          currentFilename = decodeURIComponent(urlFile);
        } catch (e) {
          currentFilename = urlFile;
        }
        if (!currentFilename) currentFilename = 'download';
      }

      // Automatically resolve duplicate filenames in the list
      if (currentFilename && !isBatch) {
        const baseFilename = currentFilename;
        let suffix = 1;
        let targetName = currentFilename;

        // If forceDuplicate is requested (from the duplicate URL modal), start with suffix 1 immediately
        if (forceDuplicate) {
          const parts = baseFilename.split('.');
          const ext = parts.length > 1 ? parts.pop() : '';
          const baseWithoutExt = parts.join('.');
          if (ext) {
            targetName = `${baseWithoutExt} (${suffix}).${ext}`;
          } else {
            targetName = `${baseWithoutExt} (${suffix})`;
          }
          suffix++;
        }

        const parts = baseFilename.split('.');
        const ext = parts.length > 1 ? parts.pop() : '';
        const baseWithoutExt = parts.join('.');

        // Loop to find an unused filename in the current download list
        while (
          downloads.some((d) => d.filename && d.filename.toLowerCase() === targetName.toLowerCase())
        ) {
          if (ext) {
            targetName = `${baseWithoutExt} (${suffix}).${ext}`;
          } else {
            targetName = `${baseWithoutExt} (${suffix})`;
          }
          suffix++;
        }
        currentFilename = targetName;
      }

      const payload = {
        url: singleUrl,
        filename: currentFilename || undefined,
        output_dir: outputDir.trim() || undefined,
        expected_hash: isBatch ? undefined : expectedHash.trim() || undefined,
        auto_extract: autoExtract,
        post_processing_script: postProcessingScript.trim() || undefined,
        speed_limit_bps: speedLimitKb ? parseInt(speedLimitKb, 10) * 1024 : 0,
        use_http3: useHttp3,
      };

      if (useAuth && username && password) {
        const token = btoa(`${username}:${password}`);
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (payload as any).headers = { Authorization: `Basic ${token}` };
      }

      try {
        const added = await api.add(payload);
        if (startLater) {
          await api.patch(added.id, { action: 'pause' });
        } else if (!isBatch) {
          const id = added.id;
          try {
            await emit('open-progress-window', id);
          } catch (e) {
            console.error('Emit error:', e);
          }
        }
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
      } catch (err: any) {
        setInlineError(`Failed to add download for ${singleUrl}: ${err.message || err}`);
        return;
      }
    }
    handleClose();
  };

  const parseNaturalLanguageDownload = (
    input: string,
  ): { url: string; extensions: string } | null => {
    const match = input.match(
      /(?:download|grab|get|spider)\s+all\s+(?:of\s+)?(?:the\s+)?([\w\s,.-]+?)\s+from\s+(https?:\/\/[^\s]+)/i,
    );
    if (match) {
      const rawExts = match[1]
        .toLowerCase()
        .replace(/\band\b/g, '')
        .replace(/[\s,]+/g, ' ')
        .trim();
      const exts = rawExts
        .split(' ')
        .map((ext) => {
          const e = ext.trim();
          if (e === 'images' || e === 'image') return 'png, jpg, jpeg, gif, webp';
          if (e === 'videos' || e === 'video') return 'mp4, mkv, avi, mov';
          if (e === 'documents' || e === 'document') return 'pdf, docx, doc, xlsx, txt';
          if (e.endsWith('s') && e.length > 2) return e.slice(0, -1);
          return e;
        })
        .filter(Boolean)
        .join(', ');

      return {
        url: match[2].trim(),
        extensions: exts,
      };
    }
    return null;
  };

  const handleOk = async () => {
    if (url.trim()) {
      const parsedNl = parseNaturalLanguageDownload(url.trim());
      if (parsedNl) {
        await emit('open-spider-with-nl', { url: parsedNl.url, extensions: parsedNl.extensions });
        handleClose();
        return;
      }

      if (isDuplicate) {
        setShowDuplicatePrompt(true);
        return;
      }
      submitDownload(false);
    }
  };

  const handleBrowse = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({ directory: true });
      if (selected) {
        setOutputDir(selected as string);
      }
    } catch (err) {
      console.error('Folder picker not available', err);
      setInlineError('Folder picker unavailable — please type the path manually.');
    }
  };

  const isDuplicate = url && downloads.some((d) => d.url === cleanUrl(url));

  /* Shared inline style shortcuts */
  const S = {
    surface: 'var(--color-surface)',
    raised: 'var(--color-surface-raised)',
    elevated: 'var(--color-surface-elevated)',
    border: 'var(--color-border)',
    borderSub: 'var(--color-border-subtle)',
    t1: 'var(--color-text-1)',
    t2: 'var(--color-text-2)',
    t3: 'var(--color-text-3)',
    t4: 'var(--color-text-4)',
    brand: 'var(--color-brand)',
    brandDim: 'var(--color-brand-dim)',
    warning: 'var(--color-warning)',
    warningDim: 'var(--color-warning-dim)',
    error: 'var(--color-error)',
    success: 'var(--color-success)',
  } as const;

  const card = (extra?: React.CSSProperties): React.CSSProperties => ({
    backgroundColor: S.raised,
    border: `1px solid ${S.border}`,
    borderRadius: 'var(--radius-lg)',
    padding: 10,
    ...extra,
  });

  return (
    <div
      className="window-mount"
      role="dialog"
      aria-modal="true"
      aria-label={t('add_url.title', 'Add New Download')}
      style={{
        display: 'flex',
        flexDirection: 'column',
        height: '100vh',
        overflow: 'hidden',
        fontFamily: 'var(--font-sans)',
        backgroundColor: S.surface,
        color: S.t1,
        userSelect: 'none',
      }}
    >
      {/* Title bar */}
      <div className="drag-region window-titlebar">
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 6,
            fontSize: 'var(--text-xs-size)',
            fontWeight: 600,
            color: S.t2,
          }}
        >
          <Download size={14} style={{ color: S.brand }} />
          {t('add_url.title', 'Add New Download')}
        </div>
        <button
          className="btn-icon no-drag"
          onClick={handleClose}
          style={{ width: 28, height: 28 }}
          title="Close"
        >
          <X size={14} />
        </button>
      </div>

      {/* Scrollable body */}
      <div
        style={{
          flex: 1,
          overflowY: 'auto',
          display: 'flex',
          flexDirection: 'column',
          gap: 8,
          padding: 8,
        }}
      >
        {/* Inline error banner (replaces window.alert) */}
        {inlineError && (
          <div className="error-banner" role="alert">
            <AlertCircle size={15} />
            <span>{inlineError}</span>
            <button
              onClick={() => setInlineError(null)}
              style={{
                marginLeft: 'auto',
                background: 'none',
                border: 'none',
                cursor: 'default',
                color: 'inherit',
                padding: 0,
                lineHeight: 1,
              }}
              title="Dismiss"
            >
              <X size={13} />
            </button>
          </div>
        )}

        {/* URL input */}
        <div style={card()}>
          <label
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              fontSize: 'var(--text-xs-size)',
              fontWeight: 600,
              color: S.t2,
              marginBottom: 6,
            }}
          >
            <span style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
              <LinkIcon size={12} /> Target URL
            </span>
            {inspectState === 'loading' && (
              <span
                style={{
                  color: S.brand,
                  display: 'flex',
                  alignItems: 'center',
                  gap: 4,
                  fontSize: 'var(--text-xs-size)',
                }}
              >
                <Search size={11} /> Inspecting…
              </span>
            )}
            {inspectState === 'error' && (
              <span style={{ color: S.error, fontSize: 'var(--text-xs-size)' }}>
                HEAD request failed
              </span>
            )}
          </label>
          <textarea
            className="textarea-field"
            style={{ minHeight: 48, resize: 'vertical', fontSize: 'var(--text-sm-size)' }}
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder={t(
              'add_url.url_placeholder',
              'https://…\nPaste multiple URLs on separate lines for batch downloading.',
            )}
            autoFocus
          />
          {isBatch ? (
            <div
              style={{
                marginTop: 6,
                display: 'flex',
                gap: 12,
                fontSize: 'var(--text-xs-size)',
                color: S.brand,
                backgroundColor: S.brandDim,
                padding: '6px 10px',
                borderRadius: 'var(--radius-md)',
                border: `1px solid ${S.brand}`,
              }}
            >
              <span style={{ fontWeight: 600 }}>Batch Download Detected</span>
              <span>{urlLines.length} URLs</span>
            </div>
          ) : (
            inspectState === 'success' &&
            inspectData && (
              <div
                style={{
                  marginTop: 6,
                  display: 'flex',
                  gap: 12,
                  fontSize: 'var(--text-xs-size)',
                  color: S.t3,
                  backgroundColor: S.elevated,
                  padding: '6px 10px',
                  borderRadius: 'var(--radius-md)',
                  border: `1px solid ${S.borderSub}`,
                }}
              >
                <span>
                  <b style={{ color: S.t1 }}>SIZE:</b>{' '}
                  {inspectData.total_bytes ? fmtBytes(inspectData.total_bytes) : 'N/A'}
                </span>
                <span>
                  <b style={{ color: S.t1 }}>TYPE:</b> {inspectData.content_type || 'Unknown'}
                </span>
                <span style={{ color: inspectData.accepts_ranges ? S.success : S.error }}>
                  <b style={{ color: S.t1 }}>RESUME:</b> {inspectData.accepts_ranges ? 'YES' : 'NO'}
                </span>
              </div>
            )
          )}
          {isDuplicate && (
            <div
              style={{
                marginTop: 6,
                fontSize: 'var(--text-xs-size)',
                fontWeight: 600,
                color: S.warning,
                backgroundColor: S.warningDim,
                padding: '6px 10px',
                borderRadius: 'var(--radius-md)',
                border: `1px solid ${S.warning}`,
              }}
            >
              ⚠️ Warning: This URL is already in your download list.
            </div>
          )}
        </div>

        {/* Save As / Directory */}
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8, flexShrink: 0 }}>
          <div style={card()}>
            <label
              style={{
                display: 'block',
                fontSize: 'var(--text-xs-size)',
                fontWeight: 600,
                color: S.t2,
                marginBottom: 5,
              }}
            >
              Save As
            </label>
            <input
              type="text"
              className="input-field"
              style={isBatch ? { opacity: 0.5, cursor: 'not-allowed' } : {}}
              value={filename}
              onChange={(e) => setFilename(e.target.value)}
              placeholder={isBatch ? 'Auto-detect (Batch)' : 'Auto-detect'}
              disabled={isBatch}
              title={
                isBatch
                  ? 'File name is auto-detected in batch mode — add a single URL to set it manually'
                  : 'Optional: override the auto-detected file name'
              }
            />
          </div>
          <div style={card()}>
            <label
              style={{
                display: 'block',
                fontSize: 'var(--text-xs-size)',
                fontWeight: 600,
                color: S.t2,
                marginBottom: 5,
              }}
            >
              Save Location
            </label>
            <div style={{ display: 'flex', gap: 4 }}>
              <input
                type="text"
                className="input-field"
                style={{ flex: 1 }}
                value={outputDir}
                onChange={(e) => setOutputDir(e.target.value)}
                placeholder="Default Directory"
              />
              <button
                className="btn-secondary"
                style={{ padding: '0 10px', flexShrink: 0 }}
                onClick={handleBrowse}
                title="Browse"
              >
                <Folder size={14} />
              </button>
            </div>
          </div>
        </div>

        {/* Speed / Hash */}
        <div style={{ ...card(), display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
          <div className="form-group">
            <label className="form-label">Speed Limit (KB/s)</label>
            <input
              type="number"
              className="input-field"
              value={speedLimitKb}
              onChange={(e) => setSpeedLimitKb(e.target.value)}
              placeholder="0 = Max"
            />
          </div>
          <div className="form-group">
            <label className="form-label">Verify Hash</label>
            <input
              type="text"
              className="input-field"
              style={isBatch ? { opacity: 0.5, cursor: 'not-allowed' } : {}}
              value={expectedHash}
              onChange={(e) => setExpectedHash(e.target.value)}
              placeholder={isBatch ? 'Disabled in batch' : 'sha256:abc123…'}
              disabled={isBatch}
            />
          </div>
        </div>

        {/* Toggles */}
        <div
          style={{ ...card(), display: 'flex', flexWrap: 'wrap', gap: 16, alignItems: 'center' }}
        >
          {[
            { label: 'HTTP Auth', state: useAuth, set: setUseAuth },
            { label: 'Start Paused', state: startLater, set: setStartLater },
            { label: 'Auto-Extract Archive', state: autoExtract, set: setAutoExtract },
            { label: 'HTTP/3 (QUIC)', state: useHttp3, set: setUseHttp3 },
          ].map(({ label, state, set }) => (
            <label
              key={label}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 6,
                fontSize: 'var(--text-xs-size)',
                color: S.t1,
                cursor: 'default',
              }}
            >
              <input
                type="checkbox"
                checked={state}
                onChange={(e) => set(e.target.checked)}
                style={{ accentColor: S.brand, width: 13, height: 13, cursor: 'default' }}
              />
              {label}
            </label>
          ))}
        </div>

        {/* Auth credentials */}
        {useAuth && (
          <div style={{ ...card(), display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
            <div className="form-group">
              <label
                className="form-label"
                style={{ display: 'flex', alignItems: 'center', gap: 4 }}
              >
                <Lock size={11} /> Username
              </label>
              <input
                type="text"
                className="input-field"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="Username"
                autoComplete="username"
              />
            </div>
            <div className="form-group">
              <label
                className="form-label"
                style={{ display: 'flex', alignItems: 'center', gap: 4 }}
              >
                <Lock size={11} /> Password
              </label>
              <input
                type="password"
                className="input-field"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="Password"
                autoComplete="current-password"
              />
            </div>
          </div>
        )}

        {/* Post-processing */}
        <div style={card()}>
          <label
            className="form-label"
            style={{ display: 'flex', alignItems: 'center', gap: 4, marginBottom: 5 }}
          >
            <List size={11} style={{ color: S.brand }} /> Post-Processing Script (.bat, .ps1)
          </label>
          <input
            type="text"
            className="input-field"
            value={postProcessingScript}
            onChange={(e) => setPostProcessingScript(e.target.value)}
            placeholder="C:\Scripts\on_complete.ps1"
          />
        </div>
      </div>

      {/* Footer */}
      <div
        style={{
          padding: '8px 12px',
          borderTop: `1px solid ${S.border}`,
          backgroundColor: S.raised,
          display: 'flex',
          justifyContent: 'flex-end',
          gap: 6,
          flexShrink: 0,
        }}
      >
        <button className="btn-secondary" onClick={handleClose}>
          Cancel
        </button>
        <button
          className="btn-primary flex items-center gap-2"
          onClick={handleOk}
          disabled={urlLines.length === 0}
        >
          <Download size={14} />{' '}
          {isBatch ? `Add ${urlLines.length} Tasks` : t('add_url.add_button', 'Add Download')}
        </button>
      </div>

      {/* Duplicate modal */}
      {showDuplicatePrompt && (
        <div
          style={{
            position: 'absolute',
            inset: 0,
            backgroundColor: 'rgba(0,0,0,0.72)',
            zIndex: 50,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            padding: 16,
            backdropFilter: 'blur(4px)',
          }}
        >
          <div
            style={{
              backgroundColor: S.surface,
              border: `1px solid ${S.warning}`,
              borderRadius: 'var(--radius-xl)',
              padding: 20,
              maxWidth: 340,
              width: '100%',
              display: 'flex',
              flexDirection: 'column',
              gap: 12,
              boxShadow: 'var(--shadow-xl)',
            }}
          >
            <h3
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 8,
                fontWeight: 600,
                fontSize: 'var(--text-md-size)',
                color: S.warning,
                margin: 0,
              }}
            >
              <AlertTriangle size={16} /> Duplicate Download
            </h3>
            <p style={{ fontSize: 'var(--text-sm-size)', color: S.t1, lineHeight: 1.6, margin: 0 }}>
              This URL is already in your download list. Download it again as a new file?
            </p>
            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8 }}>
              <button className="btn-secondary" onClick={() => setShowDuplicatePrompt(false)}>
                Cancel
              </button>
              <button
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: 6,
                  fontFamily: 'var(--font-sans)',
                  fontSize: 'var(--text-base-size)',
                  fontWeight: 500,
                  padding: '0 12px',
                  height: 30,
                  borderRadius: 'var(--radius-md)',
                  cursor: 'default',
                  border: `1px solid ${S.warning}`,
                  backgroundColor: S.warningDim,
                  color: S.warning,
                }}
                onClick={() => {
                  setShowDuplicatePrompt(false);
                  submitDownload(true);
                }}
              >
                Download Anyway
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
