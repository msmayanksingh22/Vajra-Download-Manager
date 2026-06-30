import { useState, useEffect, useRef } from 'react';
import { ArrowUpRight, CirclePower, CircleCheck, CircleX, PackageOpen, Link2, Loader2, AlertTriangle } from 'lucide-react';
import './App.css';

declare var chrome: any;

const DAEMON = 'http://127.0.0.1:6277';

function App() {
  // 4.1: Unified key — vajra_enabled (was interceptEnabled)
  const [interceptEnabled, setInterceptEnabled] = useState(true);
  const [daemonStatus, setDaemonStatus] = useState<'checking' | 'online' | 'offline'>('checking');

  // 4.3: Active download count
  const [activeCount, setActiveCount] = useState<number | null>(null);

  // 4.4: Quick Add URL
  const [quickUrl, setQuickUrl] = useState('');
  const [addStatus, setAddStatus] = useState<'idle' | 'sending' | 'success' | 'error'>('idle');
  const addStatusTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    // Load setting with new unified key; fall back to interceptEnabled for migration
    if (chrome && chrome.storage) {
      chrome.storage.local.get(['vajra_enabled', 'interceptEnabled'], (result: any) => {
        const val = result.vajra_enabled !== undefined
          ? result.vajra_enabled
          : result.interceptEnabled !== undefined
            ? result.interceptEnabled
            : true;
        setInterceptEnabled(val);
      });
    }

    // Check Vajra daemon + fetch active count
    const checkDaemon = async () => {
      try {
        const res = await fetch(`${DAEMON}/health`);
        if (res.ok) {
          setDaemonStatus('online');
          // 4.3: Fetch active download count
          try {
            const dlRes = await fetch(`${DAEMON}/api/v1/downloads`);
            if (dlRes.ok) {
              const data = await dlRes.json();
              const list: any[] = Array.isArray(data) ? data : (data.downloads ?? []);
              const active = list.filter(d =>
                d.status === 'downloading' || d.status === 'connecting'
              ).length;
              setActiveCount(active);
            }
          } catch {
            setActiveCount(null);
          }
        } else {
          setDaemonStatus('offline');
          setActiveCount(null);
        }
      } catch {
        setDaemonStatus('offline');
        setActiveCount(null);
      }
    };

    checkDaemon();
    const interval = setInterval(checkDaemon, 3000);
    return () => clearInterval(interval);
  }, []);

  const toggleIntercept = () => {
    const newVal = !interceptEnabled;
    setInterceptEnabled(newVal);
    if (chrome && chrome.storage) {
      // 4.1: Write with unified key
      chrome.storage.local.set({ vajra_enabled: newVal });
    }
  };

  const handleLaunchVajra = () => {
    const url = daemonStatus === 'online' ? 'vajra://open' : 'vajra://start';
    if (chrome && chrome.tabs) {
      chrome.tabs.create({ url, active: false }, (tab: any) => {
        setTimeout(() => {
          if (tab && tab.id) chrome.tabs.remove(tab.id).catch(() => { });
        }, 500);
      });
    }
  };

  // 4.4: Quick Add URL
  const handleQuickAdd = async () => {
    const url = quickUrl.trim();
    if (!url) return;
    setAddStatus('sending');
    try {
      const r = await fetch(`${DAEMON}/api/v1/intercept`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ url, filename: null, headers: {}, output_dir: null, priority: 'normal' }),
      });
      setAddStatus(r.ok ? 'success' : 'error');
      if (r.ok) setQuickUrl('');
    } catch {
      setAddStatus('error');
    }
    if (addStatusTimerRef.current) clearTimeout(addStatusTimerRef.current);
    addStatusTimerRef.current = setTimeout(() => setAddStatus('idle'), 2500);
  };

  const handleQuickAddKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') handleQuickAdd();
    if (e.key === 'Escape') { setQuickUrl(''); setAddStatus('idle'); }
  };

  return (
    <div className="popup-container">
      <header className="popup-header">
        <div className="logo-section">
          <img src="/logo.png" className="logo-img" alt="Vajra Logo" />
          <h1>Vajra Manager</h1>
        </div>
        <div className={`status-badge ${daemonStatus}`}>
          {daemonStatus === 'online' ? <CircleCheck size={14} /> : <CircleX size={14} />}
          <span>{daemonStatus === 'online' ? 'Online' : daemonStatus === 'offline' ? 'Offline' : 'Checking'}</span>
        </div>
      </header>

      <main className="popup-main">
        {/* Active download count — 4.3 */}
        {daemonStatus === 'online' && activeCount !== null && (
          <div className="count-row">
            <PackageOpen size={14} className="count-icon" />
            <span>
              {activeCount === 0
                ? 'No active downloads'
                : `${activeCount} active download${activeCount !== 1 ? 's' : ''}`}
            </span>
          </div>
        )}

        {/* Intercept toggle */}
        <div className="card">
          <div className="setting-row">
            <div className="setting-info">
              <h3>Intercept Downloads</h3>
              <p>Send browser downloads directly to Vajra</p>
            </div>
            <button
              className={`toggle-btn ${interceptEnabled ? 'active' : ''}`}
              onClick={toggleIntercept}
              role="switch"
              aria-checked={interceptEnabled}
              aria-label="Toggle download interception"
            >
              <div className="toggle-thumb" />
            </button>
          </div>
        </div>

        {/* Quick Add URL — 4.4 */}
        <div className="card quick-add-card">
          <div className="quick-add-label">
            <Link2 size={13} />
            <span>Quick Add URL</span>
          </div>
          <div className="quick-add-row">
            <input
              className="quick-add-input"
              type="url"
              placeholder="Paste URL and press Enter"
              value={quickUrl}
              onChange={e => setQuickUrl(e.target.value)}
              onKeyDown={handleQuickAddKeyDown}
              disabled={daemonStatus !== 'online' || addStatus === 'sending'}
              aria-label="URL to add to Vajra"
            />
            <button
              className={`quick-add-btn ${addStatus === 'success' ? 'success' : addStatus === 'error' ? 'error' : ''}`}
              onClick={handleQuickAdd}
              disabled={!quickUrl.trim() || daemonStatus !== 'online' || addStatus === 'sending'}
              title="Add URL to Vajra"
              aria-label="Add URL"
            >
              {addStatus === 'sending'
                ? <Loader2 size={14} className="spin" />
                : addStatus === 'success'
                  ? <CircleCheck size={14} />
                  : addStatus === 'error'
                    ? <AlertTriangle size={14} />
                    : <Link2 size={14} />}
            </button>
          </div>
          {addStatus === 'success' && (
            <p className="quick-add-feedback success">Added to Vajra successfully</p>
          )}
          {addStatus === 'error' && (
            <p className="quick-add-feedback error">Failed — check daemon is running</p>
          )}
          {daemonStatus !== 'online' && (
            <p className="quick-add-feedback muted">Launch Vajra to use Quick Add</p>
          )}
        </div>

        {/* Daemon status info */}
        <div className="card info-card">
          <div className="info-icon">
            <CirclePower size={20} />
          </div>
          <div className="info-text">
            {daemonStatus === 'online'
              ? "Vajra is actively running and ready to accelerate downloads."
              : "Vajra daemon is not running. Launch the Vajra Desktop App to enable interception."}
          </div>
        </div>
      </main>

      <footer className="popup-footer">
        {/* 4.5: ExternalLink icon instead of Settings */}
        <button className="icon-btn" title={daemonStatus === 'online' ? "Open Vajra" : "Launch Vajra"} onClick={handleLaunchVajra}>
          <ArrowUpRight size={18} />
          <span>{daemonStatus === 'online' ? "Open Vajra App" : "Launch Vajra App"}</span>
        </button>
      </footer>
    </div>
  );
}

export default App;
