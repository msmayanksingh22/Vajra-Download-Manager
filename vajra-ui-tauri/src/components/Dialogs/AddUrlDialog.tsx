import React, { useState, useEffect } from 'react';
// eslint-disable-next-line @typescript-eslint/no-unused-vars
import {
  X,
  Lock,
  Link as LinkIcon,
  Folder,
  HardDriveDownload,
  Search,
  Activity,
  Box,
  Download,
  FileCode2,
} from 'lucide-react';
import { api, fmtBytes } from '../../api';
import { open } from '@tauri-apps/plugin-dialog';
import { useUiStore } from '../../stores/uiStore';
// eslint-disable-next-line @typescript-eslint/no-unused-vars
import { cn } from '../../utils';
import { useDialogEscape } from '../../hooks/useDialogEscape';
import { useFocusTrap } from '../../hooks/useFocusTrap';

export default function AddUrlDialog({
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  downloads = [],
  initialUrl = '',
  onClose,
  onOk,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
}: any) {
  const [url, setUrl] = useState(initialUrl);
  useDialogEscape(onClose);
  const trapRef = useFocusTrap();
  const [useAuth, setUseAuth] = useState(false);
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [maxConnections, setMaxConnections] = useState(8);
  const [speedLimitKb, setSpeedLimitKb] = useState('');
  const [outputDir, setOutputDir] = useState('');
  const [expectedHash, setExpectedHash] = useState('');
  const [autoExtract, setAutoExtract] = useState(false);
  const [postProcessingScript, setPostProcessingScript] = useState('');
  const [startLater, setStartLater] = useState(false);
  const [scheduleAt, setScheduleAt] = useState('');
  const [queueType, setQueueType] = useState('Standard');
  const [useYtdlp, setUseYtdlp] = useState(false);
  const [ytdlpFormat, setYtdlpFormat] = useState('bestvideo+bestaudio/best');
  const [ytdlpSubtitles, setYtdlpSubtitles] = useState(false);
  const [ytdlpPlaylist, setYtdlpPlaylist] = useState(false);
  const [syncIntervalMins, setSyncIntervalMins] = useState(60);
  const [inspectState, setInspectState] = useState<'idle' | 'loading' | 'success' | 'error'>(
    'idle',
  );
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [inspectData, setInspectData] = useState<any>(null);
  const [filename, setFilename] = useState('');

  useEffect(() => {
    api
      .config()
      .then((cfg) => {
        if (cfg?.default_max_connections) setMaxConnections(cfg.default_max_connections);
      })
      .catch(console.error);
  }, []);

  // Auto-fill credentials
  useEffect(() => {
    if (!url.trim()) return;
    try {
      const host = new URL(url.trim()).hostname.toLowerCase();
      const saved = localStorage.getItem('vajra_site_logins');
      if (saved) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const match = JSON.parse(saved).find(
          (l: any) => host === l.host.toLowerCase() || host.endsWith('.' + l.host.toLowerCase()),
        );
        if (match) {
          setUseAuth(true);
          setUsername(match.user);
          setPassword(match.pass);
        }
      }
    } catch {
      /* ignore */
    }
  }, [url]);

  // Debounced inspect
  useEffect(() => {
    if (!url.trim() || !url.startsWith('http')) {
      setInspectState('idle');
      setInspectData(null);
      return;
    }
    const t = setTimeout(async () => {
      setInspectState('loading');
      try {
        const res = await api.inspect(url.trim());
        setInspectData(res);
        if (res.filename) setFilename(res.filename);
        if (res.ytdlp_supported) setUseYtdlp(true);
        setInspectState('success');
      } catch {
        setInspectState('error');
      }
    }, 800);
    return () => clearTimeout(t);
  }, [url]);

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

  const handleOk = () => {
    if (!url.trim()) return;

    const parsedNl = parseNaturalLanguageDownload(url.trim());
    if (parsedNl) {
      const uiState = useUiStore.getState();
      uiState.setSpiderInitial(parsedNl.url, parsedNl.extensions);
      uiState.setSpiderModalOpen(true);
      onClose();
      return;
    }
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const payload: any = {
      url: url.trim(),
      filename: filename.trim() || undefined,
      output_dir: outputDir.trim() || undefined,
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      max_connections: parseInt(maxConnections as any, 10) || 8,
      expected_hash: expectedHash.trim() || undefined,
      auto_extract: autoExtract,
      post_processing_script: postProcessingScript.trim() || undefined,
      speed_limit_bps: speedLimitKb ? parseInt(speedLimitKb, 10) * 1024 : 0,
      schedule_at: scheduleAt ? Math.floor(new Date(scheduleAt).getTime() / 1000) : undefined,
      _startLater: startLater,
      queue_type: queueType,
      sync_interval_secs: queueType === 'Synchronization' ? syncIntervalMins * 60 : 0,
      use_ytdlp: useYtdlp,
      ytdlp_format: ytdlpFormat,
      ytdlp_subtitles: ytdlpSubtitles,
      ytdlp_playlist: ytdlpPlaylist,
    };
    if (useAuth && username && password) {
      payload.headers = { Authorization: `Basic ${btoa(`${username}:${password}`)}` };
    }
    onOk(payload);
  };

  const handleBrowse = async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (selected) setOutputDir(selected as string);
    } catch (err) {
      console.error('Folder picker not available', err);
    }
  };

  /* ---- Field helpers ---- */
  const Label = ({ children }: { children: React.ReactNode }) => (
    <span className="form-label">{children}</span>
  );
  const CheckRow = ({
    checked,
    onChange,
    label,
  }: {
    checked: boolean;
    onChange: (v: boolean) => void;
    label: string;
  }) => (
    <label
      className="flex items-center gap-2"
      style={{ cursor: 'default', fontSize: 'var(--text-sm-size)', color: 'var(--color-text-2)' }}
    >
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        style={{ accentColor: 'var(--color-brand)', width: 13, height: 13, cursor: 'default' }}
      />
      {label}
    </label>
  );

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div
        ref={trapRef}
        className="dialog-panel"
        style={{ width: 580 }}
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="addurl-dialog-title"
      >
        {/* Header */}
        <div className="dialog-header">
          <div className="dialog-header-title" id="addurl-dialog-title">
            <LinkIcon size={16} />
            New Download Task
          </div>
          <button className="btn-icon" onClick={onClose} title="Close">
            <X size={15} />
          </button>
        </div>

        {/* Body */}
        <div className="dialog-body" style={{ gap: 'var(--sp-4)' }}>
          {/* URL */}
          <div className="card-subtle" style={{ padding: 'var(--sp-3) var(--sp-4)' }}>
            <div className="flex items-center justify-between mb-2">
              <Label>Download URL</Label>
              <span style={{ fontSize: 'var(--text-xs-size)' }}>
                {inspectState === 'loading' && (
                  <span
                    style={{
                      color: 'var(--color-info)',
                      display: 'flex',
                      alignItems: 'center',
                      gap: 4,
                    }}
                  >
                    <Search size={10} className="animate-pulse" /> Inspectingâ€¦
                  </span>
                )}
                {inspectState === 'error' && (
                  <span style={{ color: 'var(--color-error)' }}>Inspect failed</span>
                )}
                {inspectState === 'success' && (
                  <span
                    style={{
                      color: 'var(--color-success)',
                      display: 'flex',
                      alignItems: 'center',
                      gap: 4,
                    }}
                  >
                    <Activity size={10} /> Verified
                  </span>
                )}
              </span>
            </div>
            <input
              type="text"
              className="input-field"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://"
              autoFocus
            />
            {inspectState === 'success' && inspectData && (
              <div
                className="flex gap-4 mt-2 pt-2"
                style={{
                  borderTop: '1px solid var(--color-border-subtle)',
                  fontSize: 'var(--text-xs-size)',
                  color: 'var(--color-text-3)',
                }}
              >
                <span style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                  <HardDriveDownload size={10} style={{ color: 'var(--color-brand)' }} />
                  {inspectData.total_bytes ? fmtBytes(inspectData.total_bytes) : 'Unknown'}
                </span>
                <span style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                  <Box size={10} style={{ color: 'var(--color-brand)' }} />
                  {inspectData.content_type || 'Unknown'}
                </span>
                <span
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 4,
                    color: inspectData.accepts_ranges
                      ? 'var(--color-success)'
                      : 'var(--color-warning)',
                    fontWeight: 600,
                  }}
                >
                  <Activity size={10} />
                  {inspectData.accepts_ranges ? 'Resume Supported' : 'No Resume'}
                </span>
              </div>
            )}
          </div>

          {/* File Name + Save Dir */}
          <div className="grid grid-cols-2 gap-3">
            <div className="form-group">
              <Label>File Name</Label>
              <input
                type="text"
                className="input-field"
                value={filename}
                onChange={(e) => setFilename(e.target.value)}
                placeholder="Auto-detect"
              />
            </div>
            <div className="form-group">
              <Label>Save Directory</Label>
              <div className="flex gap-1.5">
                <input
                  type="text"
                  className="input-field"
                  value={outputDir}
                  onChange={(e) => setOutputDir(e.target.value)}
                  placeholder="System default"
                />
                <button
                  className="btn-secondary flex-shrink-0 gap-1.5"
                  style={{ padding: '0 10px' }}
                  onClick={handleBrowse}
                  title="Browse"
                >
                  <Folder size={14} />
                </button>
              </div>
            </div>
          </div>

          {/* yt-dlp Configuration */}
          {useYtdlp && (
            <div
              className="card-subtle animate-fade-in"
              style={{
                padding: 'var(--sp-3) var(--sp-4)',
                borderColor: 'var(--color-brand)',
                background: 'var(--color-brand-muted)',
              }}
            >
              <div className="section-title mb-2" style={{ color: 'var(--color-brand)' }}>
                yt-dlp Extraction
              </div>
              <div className="grid grid-cols-2 gap-3 mb-2">
                <div className="form-group">
                  <Label>Format / Quality</Label>
                  <input
                    type="text"
                    className="input-field"
                    value={ytdlpFormat}
                    onChange={(e) => setYtdlpFormat(e.target.value)}
                    placeholder="bestvideo+bestaudio/best"
                  />
                </div>
                <div className="flex flex-col gap-2 justify-center mt-4">
                  <CheckRow
                    checked={ytdlpSubtitles}
                    onChange={setYtdlpSubtitles}
                    label="Embed Subtitles"
                  />
                  <CheckRow
                    checked={ytdlpPlaylist}
                    onChange={setYtdlpPlaylist}
                    label="Download entire playlist"
                  />
                </div>
              </div>
            </div>
          )}

          {/* Engine Parameters */}
          <div className="card-subtle" style={{ padding: 'var(--sp-3) var(--sp-4)' }}>
            <div className="section-title mb-2">Engine Parameters</div>
            <div className="grid grid-cols-3 gap-3 mb-3">
              <div className="form-group">
                <Label>Threads</Label>
                {/* eslint-disable-next-line @typescript-eslint/no-explicit-any */}
                <select
                  className="select-field"
                  value={maxConnections}
                  onChange={(e) => setMaxConnections(e.target.value as any)}
                >
                  {[1, 2, 4, 8, 16, 32, 64].map((n) => (
                    <option key={n} value={n}>
                      {n}
                      {n === 64 ? ' (Unsafe)' : n === 32 ? ' (Extreme)' : ''}
                    </option>
                  ))}
                </select>
              </div>
              <div className="form-group">
                <Label>Speed Limit (KB/s)</Label>
                <input
                  type="number"
                  className="input-field"
                  value={speedLimitKb}
                  onChange={(e) => setSpeedLimitKb(e.target.value)}
                  placeholder="0 = Unlimited"
                />
              </div>
              <div className="form-group">
                <Label>Expected Hash</Label>
                <input
                  type="text"
                  className="input-field"
                  value={expectedHash}
                  onChange={(e) => setExpectedHash(e.target.value)}
                  placeholder="sha256:â€¦"
                />
              </div>
            </div>
            <div
              className="grid grid-cols-2 gap-3 pt-3"
              style={{ borderTop: '1px solid var(--color-border-subtle)' }}
            >
              <div className="flex flex-col gap-2">
                <CheckRow checked={useAuth} onChange={setUseAuth} label="Basic Authentication" />
                {useAuth && (
                  <div className="flex gap-1.5 animate-fade-in">
                    <input
                      type="text"
                      className="input-field"
                      value={username}
                      onChange={(e) => setUsername(e.target.value)}
                      placeholder="Username"
                    />
                    <input
                      type="password"
                      className="input-field"
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      placeholder="Password"
                    />
                  </div>
                )}
              </div>
              <div className="form-group">
                <Label>Post-Process Script</Label>
                <input
                  type="text"
                  className="input-field"
                  value={postProcessingScript}
                  onChange={(e) => setPostProcessingScript(e.target.value)}
                  placeholder="C:\scripts\run.bat"
                />
              </div>
            </div>
            <div
              className="grid grid-cols-2 gap-3 pt-3 mt-1"
              style={{ borderTop: '1px solid var(--color-border-subtle)' }}
            >
              <div className="form-group">
                <Label>Queue Type</Label>
                <select
                  className="select-field"
                  value={queueType}
                  onChange={(e) => setQueueType(e.target.value)}
                >
                  <option value="Standard">Standard</option>
                  <option value="Synchronization">Synchronization (Auto-Sync)</option>
                </select>
              </div>
              {queueType === 'Synchronization' && (
                <div className="form-group animate-fade-in">
                  <Label>Sync Interval (Minutes)</Label>
                  <input
                    type="number"
                    className="input-field"
                    value={syncIntervalMins}
                    onChange={(e) => setSyncIntervalMins(parseInt(e.target.value) || 60)}
                  />
                </div>
              )}
            </div>
          </div>

          {/* Quick Toggles + Schedule */}
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-4">
              <CheckRow checked={startLater} onChange={setStartLater} label="Queue Paused" />
              <CheckRow checked={autoExtract} onChange={setAutoExtract} label="Auto-Extract" />
              <CheckRow checked={useYtdlp} onChange={setUseYtdlp} label="Use yt-dlp" />
            </div>
            <div className="flex items-center gap-2">
              <span className="form-label">Schedule</span>
              <input
                type="datetime-local"
                className="input-field"
                style={{ width: 'auto' }}
                value={scheduleAt}
                onChange={(e) => setScheduleAt(e.target.value)}
              />
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="dialog-footer">
          <button className="btn-secondary" onClick={onClose}>
            Cancel
          </button>
          <button
            className="btn-primary flex items-center gap-2"
            onClick={handleOk}
            disabled={!url.trim()}
          >
            <Download size={14} />
            Add Download
          </button>
        </div>
      </div>
    </div>
  );
}
