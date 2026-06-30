import React, { useState, useEffect, useRef } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { api, fmtBytes, fmtSpeed, fmtEta } from '../api';
import { listen, emit } from '@tauri-apps/api/event';
import { X, Activity, ShieldCheck, Folder, FileIcon, Pause, Play, Square, Settings2, AlertCircle, RefreshCw, Gauge, CheckSquare2, Copy, Check } from 'lucide-react';
// eslint-disable-next-line @typescript-eslint/no-unused-vars
import { cn } from '../utils';
import { isPermissionGranted, requestPermission, sendNotification } from '@tauri-apps/plugin-notification';

export default function ProgressWindow({ downloadId }: { downloadId: string }) {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [item, setItem] = useState<any>(() => {
    try {
      const init = localStorage.getItem(`vajra_progress_init_${downloadId}`);
      if (init) return JSON.parse(init);
    } catch(e) { /* ignore */ }
    return null;
  });
  const [speedHistory, setSpeedHistory] = useState<number[]>([]);
  const [copiedError, setCopiedError] = useState(false);
  
  const [enableLimit, setEnableLimit] = useState(false);
  const [limitKbps, setLimitKbps] = useState('');
  const isInputFocused = useRef(false);

  useEffect(() => {
    if (item && item.speed_limit_bps !== undefined) {
      const bps = item.speed_limit_bps || 0;
      if (bps > 0) {
        setEnableLimit(true);
        if (!isInputFocused.current) {
          setLimitKbps(String(Math.floor(bps / 1024)));
        }
      } else {
        setEnableLimit(false);
        if (!isInputFocused.current) {
          setLimitKbps('');
        }
      }
    }
  }, [item?.speed_limit_bps]);
  
  const [showCompleteDialog, setShowCompleteDialog] = useState(false);
  const [shutdownAfterComplete, setShutdownAfterComplete] = useState(false);

  const [connectionMode, setConnectionMode] = useState('sse'); // sse or polling
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const [sseRetries, setSseRetries] = useState(0);

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const lastOverride = useRef<any>(null);
  
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const itemRef = useRef<any>(null);
  useEffect(() => {
    itemRef.current = item;
  }, [item]);

  useEffect(() => {
    getCurrentWindow().show().catch(console.error);

    const storedComplete = localStorage.getItem(`vajra_show_complete_${downloadId}`);
    if (storedComplete !== null) setShowCompleteDialog(storedComplete === 'true');

    const storedShutdown = localStorage.getItem(`vajra_shutdown_${downloadId}`);
    if (storedShutdown !== null) setShutdownAfterComplete(storedShutdown === 'true');
  }, [downloadId]);

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const updateItemFromPayload = (payload: any) => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    setItem((prev: any) => {
      let nextStatus = payload.status;
      const now = Date.now();
      const override = lastOverride.current;
      if (override && now - override.timestamp < 1200) {
        nextStatus = override.status;
        if (payload.status === override.status || (override.status === 'connecting' && payload.status === 'downloading')) {
          lastOverride.current = null;
        }
      }

      const prevStatus = prev?.status;
      if ((nextStatus === 'completed') &&
          (prevStatus && prevStatus !== 'completed')) {
        
        const storedComplete = localStorage.getItem(`vajra_show_complete_${payload.download_id || payload.id}`);
        if (storedComplete === 'true') {
          (async () => {
            try {
              let permissionGranted = await isPermissionGranted();
              if (!permissionGranted) {
                const permission = await requestPermission();
                permissionGranted = permission === 'granted';
              }
              if (permissionGranted) {
                sendNotification({ 
                  title: 'Download Complete', 
                  body: `${payload.filename || payload.file_name || prev?.filename || 'File'} has finished downloading.` 
                });
              }
            } catch (err) {
              console.error("Failed to send notification:", err);
            }
          })();
        }

        const completedId = payload.download_id || payload.id;
        if (completedId) emit('vajra-download-complete', completedId).catch(console.error);
        setTimeout(() => {
          try { getCurrentWindow().close().catch(console.error); } catch(e) { /* ignore */ }
        }, 100);
      }

      return {
        ...prev,
        id: payload.download_id || payload.id,
        filename: payload.filename || payload.file_name || (prev?.filename),
        total_bytes: payload.total_bytes ?? prev?.total_bytes ?? null,
        downloaded_bytes: payload.downloaded_bytes ?? payload.bytes_done ?? prev?.downloaded_bytes ?? 0,
        bytes_done: payload.downloaded_bytes ?? payload.bytes_done ?? prev?.bytes_done ?? 0,
        speed_bps: nextStatus === 'paused' ? 0 : (payload.speed_bps ?? 0),
        eta_seconds: nextStatus === 'paused' ? null : (payload.eta_seconds ?? prev?.eta_seconds ?? null),
        status: nextStatus || prev?.status,
        resume_supported: payload.resume_supported ?? prev?.resume_supported,
        segments: payload.segments || prev?.segments || [],
        url: payload.url || prev?.url,
        output_path: payload.output_path || prev?.output_path,
        error: payload.error,
        hash_result: payload.hash_result || prev?.hash_result,
        expected_hash: payload.expected_hash || prev?.expected_hash
      };
    });

    const currentStatus = payload.status || (itemRef.current?.status);
    if (payload.speed_bps !== undefined) {
      if (currentStatus === 'downloading') {
        setSpeedHistory(prev => {
          const next = [...prev, payload.speed_bps];
          if (next.length > 50) next.shift();
          return next;
        });
      } else {
        setSpeedHistory(prev => prev.length > 0 && prev[prev.length - 1] === 0 ? prev : [...prev, 0].slice(-50));
      }
    }
  };

  useEffect(() => {
    if (!downloadId) return;
    let active = true;
    api.get(downloadId).then(found => {
      if (found && active) {
        setItem((prev: any) => {
          if (!prev) return found;
          // If we already received a newer update (higher bytes_done or completed status),
          // merge the static metadata fields (filename, url, output_path, expected_hash, resume_supported)
          // into the current state without overwriting newer progress metrics.
          const prevBytes = prev.bytes_done || 0;
          const foundBytes = found.bytes_done || 0;
          if (prevBytes > foundBytes || prev.status === 'completed') {
            return {
              ...found,
              ...prev,
              filename: prev.filename || found.filename,
              url: prev.url || found.url,
              output_path: prev.output_path || found.output_path,
            };
          }
          return found;
        });
        
        if (found.status === 'completed') {
          emit('vajra-download-complete', downloadId).catch(console.error);
          setTimeout(() => {
            try { getCurrentWindow().close().catch(console.error); } catch(e) { /* ignore */ }
          }, 100);
        }

        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        if (found.speed_bps) setSpeedHistory([found.speed_bps] as any);
      }
    }).catch(console.error);
    return () => { active = false; };
  }, [downloadId]);

  useEffect(() => {
    if (!downloadId) return;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    let sseCleanup: any = null;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    let pollIntervalId: any = null;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    let sseTimeoutId: any = null;

    const startPolling = (intervalMs: number) => {
      if (pollIntervalId) clearInterval(pollIntervalId);
      const poll = () => {
        api.get(downloadId).then(data => { if (data) updateItemFromPayload(data); }).catch(() => {});
      };
      poll();
      pollIntervalId = setInterval(poll, intervalMs);
    };

    let isLocalUnmounted = false;

    const startSSE = () => {
      if (sseCleanup) { sseCleanup(); sseCleanup = null; }
      
      let unsubVajra: (() => void) | null = null;
      let unsubFailed: (() => void) | null = null;
      let isSetupActive = true;
      
      listen('vajra-event', (event: any) => {
        console.log("RECEIVED EVENT (ProgressWindow):", event);
        if (isLocalUnmounted) return;
        const ev = event.payload;

        if (ev.event === 'batch_progress') {
          const downloads = ev.downloads;
          if (downloads && Array.isArray(downloads)) {
            const myDownload = downloads.find((d: any) => (d.download_id || d.id)?.toLowerCase() === downloadId.toLowerCase());
            if (myDownload) {
              setSseRetries(0);
              setConnectionMode('sse');
              updateItemFromPayload({
                ...myDownload,
                status: myDownload.status
              });
            }
          }
          return;
        }

        const evId = ev.id || ev.download_id;
        if (!evId || evId.toLowerCase() !== downloadId.toLowerCase()) return;
        setSseRetries(0);
        setConnectionMode('sse');
        if (ev.event === 'progress') {
          updateItemFromPayload(ev);
        } else if (ev.event === 'state_change') {
          let status = ev.status;
          const now = Date.now();
          const override = lastOverride.current as any;
          if (override && now - override.timestamp < 1200) {
            status = override.status;
            if (ev.status === override.status || (override.status === 'connecting' && ev.status === 'downloading')) {
              lastOverride.current = null;
            }
          }
          setItem((prev: any) => {
            const base = prev || {
              id: ev.id || downloadId,
              filename: '—', total_bytes: null, downloaded_bytes: 0, speed_bps: 0,
              eta_seconds: null, resume_supported: false, segments: [], url: '—', output_path: ''
            };
            if (status === 'completed' && base.status !== 'completed') {
              const storedComplete = localStorage.getItem(`vajra_show_complete_${ev.id || downloadId}`);
              if (storedComplete === 'true') {
                (async () => {
                  try {
                    let permissionGranted = await isPermissionGranted();
                    if (!permissionGranted) {
                      const permission = await requestPermission();
                      permissionGranted = permission === 'granted';
                    }
                    if (permissionGranted) {
                      sendNotification({ 
                        title: 'Download Complete', 
                        body: `${base.filename || 'File'} has finished downloading.` 
                      });
                    }
                  } catch (err) {
                    console.error("Failed to send notification:", err);
                  }
                })();
              }
              const completedId2 = ev.id || downloadId;
              if (completedId2) emit('vajra-download-complete', completedId2).catch(console.error);
              setTimeout(() => { try { getCurrentWindow().close().catch(console.error); } catch(e) { /* ignore */ } }, 100);
            }
            return { 
              ...base, 
              status: status, 
              output_path: ev.output_path || base.output_path, 
              error: ev.error,
              hash_result: ev.hash_result || base.hash_result,
              expected_hash: ev.expected_hash || base.expected_hash
            };
          });
        }
      }).then((unlisten) => {
        if (!isSetupActive || isLocalUnmounted) {
          unlisten();
        } else {
          unsubVajra = unlisten;
        }
      }).catch((err) => {
        console.error("Failed to register vajra-event listener in ProgressWindow:", err);
        setSseRetries(prev => {
          const next = prev + 1;
          if (next >= 3) {
            setConnectionMode('polling');
          } else {
            sseTimeoutId = setTimeout(() => startSSE(), 2000);
          }
          return next;
        });
      });

      listen('DownloadFailed', (event: any) => {
        console.log("RECEIVED DownloadFailed EVENT (ProgressWindow):", event);
        if (isLocalUnmounted) return;
        const ev = event.payload;
        const evId = ev.id || ev.download_id;
        if (!evId || evId.toLowerCase() !== downloadId.toLowerCase()) return;
        setItem((prev: any) => ({
          ...prev,
          status: 'failed',
          error: ev.error || 'Download failed',
          speed_bps: 0,
        }));
      }).then((unlisten) => {
        if (!isSetupActive || isLocalUnmounted) {
          unlisten();
        } else {
          unsubFailed = unlisten;
        }
      }).catch((err) => {
        console.error("Failed to register DownloadFailed listener in ProgressWindow:", err);
      });

      sseCleanup = () => {
        isSetupActive = false;
        if (unsubVajra) unsubVajra();
        if (unsubFailed) unsubFailed();
      };
    };

    let safetyPollIntervalId: any = null;

    if (connectionMode === 'sse') {
      startSSE();
      // Safety poll fallback to prevent freezes from missed SSE events or registration race conditions
      safetyPollIntervalId = setInterval(() => {
        api.get(downloadId).then(data => {
          if (data && !isLocalUnmounted) {
            updateItemFromPayload(data);
          }
        }).catch(() => {});
      }, 3000);
    } else {
      startPolling(item?.status === 'paused' ? 5000 : 1500);
      const sseCheckIntervalId = setInterval(() => {
        api.health().then(() => {
          setSseRetries(0);
          setConnectionMode('sse');
        }).catch(() => {});
      }, 30000);
      return () => { clearInterval(pollIntervalId); clearInterval(sseCheckIntervalId); };
    }

    return () => {
      isLocalUnmounted = true;
      if (sseCleanup) sseCleanup();
      if (pollIntervalId) clearInterval(pollIntervalId);
      if (safetyPollIntervalId) clearInterval(safetyPollIntervalId);
      if (sseTimeoutId) clearTimeout(sseTimeoutId);
    };
   
  }, [downloadId, connectionMode, item?.status]);

  const handleClose = () => { getCurrentWindow().close().catch(console.error); };

  // eslint-disable-next-line @typescript-eslint/no-unused-vars, @typescript-eslint/no-explicit-any
  const handleApplyLimit = async (e: any) => {
    e.preventDefault();
    try {
      const kbps = parseInt(limitKbps as string, 10);
      const bps = !isNaN(kbps) && kbps > 0 ? kbps * 1024 : 0;
      await api.patch(downloadId, { speed_limit_bps: bps });
      if (bps > 0) setEnableLimit(true);
      else setEnableLimit(false);
    } catch (err) { console.error(err); }
  };

  const handleToggleLimit = async () => {
    const newState = !enableLimit;
    setEnableLimit(newState);
    try {
      if (!newState) {
        await api.patch(downloadId, { speed_limit_bps: 0 });
      } else {
        const kbps = parseInt(limitKbps as string, 10);
        const bps = !isNaN(kbps) && kbps > 0 ? kbps * 1024 : 0;
        await api.patch(downloadId, { speed_limit_bps: bps });
      }
    } catch (e) { console.error(e); }
  };

  const handlePause = async () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    lastOverride.current = { status: 'paused', timestamp: Date.now() } as any;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    setItem((prev: any) => prev ? { ...prev, status: 'paused', speed_bps: 0 } as any : null);
    try { await api.patch(downloadId, { action: 'pause' }); } catch(e) { console.error(e); }
  };

  const handleResume = async () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    lastOverride.current = { status: 'connecting', timestamp: Date.now() } as any;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    setItem((prev: any) => prev ? { ...prev, status: 'connecting' } as any : null);
    try { await api.patch(downloadId, { action: 'resume' }); } catch(e) { console.error(e); }
  };

  const handleCancel = async () => {
    try { await api.patch(downloadId, { action: 'pause' }); handleClose(); } catch(e) { console.error(e); }
  };

  const handleOpenFolder = async () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    try { if (item && (item as any).output_path) { const { invoke } = await import('@tauri-apps/api/core'); await invoke('show_in_explorer', { path: (item as any).output_path }); } } catch (e) { console.error("Failed to open folder", e); }
  };

  const handleOpenFile = async () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    try { if (item && (item as any).output_path) { const { invoke } = await import('@tauri-apps/api/core'); await invoke('open_file_path', { path: (item as any).output_path }); } } catch (e) { console.error("Failed to open file", e); }
  };

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const currentItem: any = item || { filename: '—', status: 'connecting', total_bytes: null, downloaded_bytes: 0, speed_bps: 0, eta_seconds: null, resume_supported: false, segments: [], url: '—', output_path: '' };
  
  const dlBytes = currentItem.downloaded_bytes || 0;
  const totBytes = currentItem.total_bytes;
  let pct = 0;
  if (currentItem.status === 'completed') pct = 100;
  else if (totBytes && totBytes > 0) {
    pct = (dlBytes / totBytes) * 100;
    if (pct > 100) pct = 100; // fix overshoot
  }

  const retryCountStr = localStorage.getItem(`vajra_retry_count_${downloadId}`);
  const retryAttempt = retryCountStr ? parseInt(retryCountStr, 10) : 0;

  const segments = currentItem.segments || [];
  const fileTotal = currentItem.total_bytes || 0;

  const getSegmentColors = (status: string): { track: string; fill: string } => {
    switch (status) {
      case 'completed':  return { track: 'var(--color-success-dim)',  fill: 'var(--color-success)' };
      case 'downloading': return { track: 'var(--color-brand-dim)',   fill: 'var(--color-brand)' };
      case 'paused':     return { track: 'var(--color-warning-dim)',  fill: 'var(--color-warning)' };
      case 'failed':     return { track: 'var(--color-error-dim)',    fill: 'var(--color-error)' };
      default:           return { track: 'var(--color-surface-elevated)', fill: 'var(--color-text-4)' };
    }
  };

  /* Shared inline style values */
  const S = {
    surface:   'var(--color-surface)',
    raised:    'var(--color-surface-raised)',
    elevated:  'var(--color-surface-elevated)',
    border:    'var(--color-border)',
    borderSub: 'var(--color-border-subtle)',
    t1: 'var(--color-text-1)',
    t2: 'var(--color-text-2)',
    t3: 'var(--color-text-3)',
    t4: 'var(--color-text-4)',
    brand: 'var(--color-brand)',
    success: 'var(--color-success)',
    successDim: 'var(--color-success-dim)',
    error: 'var(--color-error)',
  } as const;

  const segStatusColor = (s: string) => {
    if (s === 'downloading') return S.brand;
    if (s === 'completed')   return S.success;
    if (s === 'error' || s === 'failed') return S.error;
    return S.t4;
  };

  return (
    <div
      className="window-mount"
      role="dialog"
      aria-modal="true"
      aria-label="Download Progress"
      style={{ display: 'flex', flexDirection: 'column', height: '100vh', overflow: 'hidden', fontFamily: 'var(--font-sans)', backgroundColor: S.surface, color: S.t1, userSelect: 'none' }}
    >

      {/* Title bar */}
      <div className="drag-region window-titlebar">
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 'var(--text-xs-size)', fontWeight: 600, color: S.t2 }}>
          <Activity size={14} style={{ color: (currentItem.status === 'downloading' || retryAttempt > 0) ? S.brand : S.t4 }} />
          {retryAttempt > 0
            ? `Retrying… (Attempt ${retryAttempt}/2)`
            : ({
                connecting:  'Connecting…',
                downloading: 'Downloading…',
                paused:      'Paused',
                completed:   'Complete',
                failed:      'Failed',
              } as Record<string, string>)[currentItem.status] ?? currentItem.status
          }
        </div>
        <button className="btn-icon no-drag" onClick={handleClose} style={{ width: 28, height: 28 }} title="Close">
          <X size={14} />
        </button>
      </div>

      {/* Body */}
      <div style={{ flex: 1, overflow: 'hidden', display: 'flex', flexDirection: 'column', gap: 6, padding: 8, cursor: 'default' }}>

        {/* File / URL info */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 4, flexShrink: 0, backgroundColor: S.raised, padding: 8, borderRadius: 'var(--radius-lg)', border: `1px solid ${S.border}` }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
            <span className="truncate" style={{ fontWeight: 600, fontSize: 'var(--text-sm-size)', color: S.t1, maxWidth: 400, userSelect: 'text' }} title={currentItem.filename}>
              {currentItem.filename}
            </span>
            <span style={{ fontSize: 'var(--text-xs-size)', fontWeight: 600, color: S.brand, backgroundColor: 'var(--color-brand-dim)', padding: '2px 8px', borderRadius: 'var(--radius-full)', flexShrink: 0 }}>
              {currentItem.filename?.split('.').pop()?.toUpperCase() || 'FILE'}
            </span>
          </div>
          <div style={{ fontSize: 'var(--text-xs-size)', color: S.t4, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', userSelect: 'text' }}>
            <span style={{ color: S.brand, fontWeight: 600 }}>URL: </span>{currentItem.url}
          </div>
          {currentItem.output_path && (
            <div style={{ fontSize: 'var(--text-xs-size)', color: S.t4, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', userSelect: 'text' }}>
              <span style={{ color: S.t3, fontWeight: 600 }}>DIR: </span>{currentItem.output_path}
            </div>
          )}
          {currentItem.expected_hash && (
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginTop: 4, paddingTop: 6, borderTop: `1px solid ${S.borderSub}` }}>
              <ShieldCheck size={12} style={{ color: currentItem.hash_result === 'Matched' ? S.success : currentItem.hash_result === 'Mismatched' ? S.error : S.t4 }} />
              <span style={{ fontSize: 'var(--text-xs-size)', fontWeight: 600, color: currentItem.hash_result === 'Matched' ? S.success : currentItem.hash_result === 'Mismatched' ? S.error : S.t4 }}>
                {currentItem.hash_result === 'Matched' ? 'Hash Matched' : currentItem.hash_result === 'Mismatched' ? 'Hash Mismatched' : 'Hash Pending'}
              </span>
              <span style={{ fontSize: 'var(--text-xs-size)', color: S.t4, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{currentItem.expected_hash}</span>
            </div>
          )}
        </div>

        {/* Stats grid */}
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 6, flexShrink: 0, backgroundColor: S.raised, padding: 8, borderRadius: 'var(--radius-lg)', border: `1px solid ${S.border}` }}>
          {[{
            label: 'Downloaded',
            value: <>{fmtBytes(Math.min(dlBytes, totBytes || dlBytes))} <span style={{ color: S.t4, fontSize: 'var(--text-xs-size)' }}>/ {totBytes ? fmtBytes(totBytes) : '?'}</span></>
          }, {
            label: 'Transfer Rate',
            value: <span style={{ color: S.brand }}>{fmtSpeed(currentItem.speed_bps)}</span>
          }, {
            label: 'Time Left',
            value: fmtEta(currentItem.eta_seconds)
          }, {
            label: 'Threads',
            value: currentItem.status === 'downloading' && segments.length > 0 ? `${segments.length} Active` : '—'
          }].map(({ label, value }) => (
            <div key={label} style={{ display: 'flex', flexDirection: 'column' }}>
              <span style={{ fontSize: 'var(--text-xs-size)', color: S.t3, fontWeight: 500 }}>{label}</span>
              <span style={{ fontWeight: 600, color: S.t1, fontSize: 'var(--text-sm-size)', marginTop: 2 }}>{value}</span>
            </div>
          ))}
        </div>

        {/* Controls */}
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 6, flexShrink: 0 }}>
          {/* Speed limit */}
          <div className="no-drag" style={{ backgroundColor: S.raised, border: `1px solid ${S.border}`, borderRadius: 'var(--radius-lg)', padding: 8, display: 'flex', flexDirection: 'column', gap: 6 }}>
            <button
              type="button"
              style={{ display: 'flex', width: '100%', justifyContent: 'space-between', alignItems: 'center', background: 'none', border: 'none', cursor: 'default', padding: 0 }}
              onClick={handleToggleLimit}
            >
              <span style={{ fontSize: 'var(--text-xs-size)', fontWeight: 600, color: S.t2, display: 'flex', alignItems: 'center', gap: 4 }}>
                <Gauge size={13} /> Speed Limit
              </span>
              {/* Toggle pill rendered via CSS class + aria-checked */}
              <span
                className="toggle-pill"
                role="switch"
                aria-checked={enableLimit}
                style={{ pointerEvents: 'none' }}
              />
            </button>
            <div style={{ display: 'flex', alignItems: 'center', gap: 4, backgroundColor: S.elevated, borderRadius: 'var(--radius-md)', border: `1px solid ${S.border}`, padding: '0 8px' }}>
              <input
                type="number"
                style={{ background: 'transparent', border: 'none', outline: 'none', color: S.t1, fontSize: 'var(--text-xs-size)', fontWeight: 500, padding: '6px 0', width: '100%' }}
                placeholder="Unlimited"
                value={limitKbps}
                onFocus={() => { isInputFocused.current = true; }}
                onBlur={() => { isInputFocused.current = false; }}
                onChange={e => {
                  const val = e.target.value;
                  setLimitKbps(val);
                  const kbps = parseInt(val, 10);
                  if (!isNaN(kbps) && kbps > 0) { setEnableLimit(true); api.patch(downloadId, { speed_limit_bps: kbps * 1024 }).catch(console.error); }
                  else api.patch(downloadId, { speed_limit_bps: 0 }).catch(console.error);
                }}
              />
              <span style={{ fontSize: 'var(--text-xs-size)', color: S.t4, whiteSpace: 'nowrap' }}>KB/s</span>
            </div>
          </div>

          {/* Completion actions */}
          <div className="no-drag" style={{ backgroundColor: S.raised, border: `1px solid ${S.border}`, borderRadius: 'var(--radius-lg)', padding: 8, display: 'flex', flexDirection: 'column', gap: 6 }}>
            <span style={{ fontSize: 'var(--text-xs-size)', fontWeight: 600, color: S.t2, display: 'flex', alignItems: 'center', gap: 4 }}>
              <CheckSquare2 size={13} /> On Completion
            </span>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
              {[{
                label: 'Show Notification', checked: showCompleteDialog,
                onChange: (v: boolean) => { setShowCompleteDialog(v); localStorage.setItem(`vajra_show_complete_${downloadId}`, String(v)); }
              }, {
                label: 'Shutdown PC', checked: shutdownAfterComplete,
                onChange: (v: boolean) => { setShutdownAfterComplete(v); localStorage.setItem(`vajra_shutdown_${downloadId}`, String(v)); }
              }].map(({ label, checked, onChange }) => (
                <label key={label} style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 'var(--text-xs-size)', color: S.t1, cursor: 'default' }}>
                  <input type="checkbox" checked={checked} onChange={e => onChange(e.target.checked)} style={{ accentColor: S.brand, width: 13, height: 13, cursor: 'default' }} />
                  {label}
                </label>
              ))}
            </div>
          </div>
        </div>

        {/* Progress + Speed chart */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 4, flexShrink: 0 }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-end' }}>
            <span style={{ fontSize: 'var(--text-xs-size)', fontWeight: 600, color: S.t3 }}>Progress</span>
            <span style={{ fontWeight: 700, fontSize: 'var(--text-sm-size)', color: currentItem.status === 'failed' ? S.error : S.brand }}>{pct.toFixed(2)}%</span>
          </div>
          <div style={{ width: '100%', height: 6, backgroundColor: S.elevated, borderRadius: 'var(--radius-full)', overflow: 'hidden', border: `1px solid ${S.border}` }}>
            <div style={{ height: '100%', width: `${pct}%`, backgroundColor: currentItem.status === 'completed' ? S.success : currentItem.status === 'failed' ? S.error : S.brand, transition: 'width 0.3s ease', borderRadius: 'var(--radius-full)' }} />
          </div>
        </div>

        {currentItem.status === 'failed' ? (
          <div className="no-drag" style={{
            flex: 1,
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            gap: 12,
            backgroundColor: S.raised,
            border: `1px solid ${S.border}`,
            borderRadius: 'var(--radius-lg)',
            padding: '24px 16px',
            textAlign: 'center',
            marginTop: 4,
            overflow: 'hidden'
          }}>
            <div style={{
              width: 48, height: 48,
              backgroundColor: 'var(--color-error-dim)',
              border: `1px solid ${S.error}`,
              borderRadius: '50%',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              color: S.error,
              marginBottom: 2
            }}>
              <AlertCircle size={24} />
            </div>
            <div>
              <h4 style={{ margin: 0, fontSize: 'var(--text-md-size)', fontWeight: 600, color: S.t1 }}>Download Interrupted</h4>
              {retryAttempt > 0 ? (
                <p style={{ margin: '4px 0 0 0', fontSize: 'var(--text-xs-size)', color: S.brand, fontWeight: 600 }}>
                  Retrying connection automatically... (Attempt {retryAttempt}/2)
                </p>
              ) : (
                <p style={{ margin: '4px 0 0 0', fontSize: 'var(--text-xs-size)', color: S.t3 }}>
                  An error occurred during the transfer process.
                </p>
              )}
            </div>
            
            {currentItem.error && (
              <div style={{
                backgroundColor: S.elevated,
                border: `1px solid ${S.borderSub}`,
                borderRadius: 'var(--radius-md)',
                padding: '10px 12px',
                width: '100%',
                maxHeight: 110,
                overflowY: 'auto',
                fontSize: 'var(--text-xs-size)',
                color: S.t2,
                fontFamily: 'var(--font-mono)',
                textAlign: 'left',
                whiteSpace: 'pre-wrap',
                wordBreak: 'break-all',
                userSelect: 'text',
                position: 'relative'
              }}>
              <div style={{ display: 'flex', alignItems: 'flex-start', gap: 6 }}>
                  <span style={{ flex: 1 }}>{currentItem.error}</span>
                  <button
                    className="btn-icon"
                    style={{ width: 22, height: 22, flexShrink: 0 }}
                    onClick={() => {
                      navigator.clipboard.writeText(currentItem.error || '');
                      setCopiedError(true);
                      setTimeout(() => setCopiedError(false), 1500);
                    }}
                    title="Copy Error"
                  >
                    {copiedError
                      ? <Check size={11} style={{ color: 'var(--color-success)' }} />
                      : <Copy size={11} />}
                  </button>
                </div>
              </div>
            )}

            <button
              className="btn-primary flex items-center gap-2"
              style={{
                backgroundColor: S.error,
                borderColor: S.error,
                padding: '6px 20px',
                height: 'auto',
                fontSize: 'var(--text-xs-size)',
                fontWeight: 600,
                marginTop: 6
              }}
              onClick={handleResume}
            >
              <RefreshCw size={12} /> Retry Download
            </button>
          </div>
        ) : (
          <>
            {currentItem.status !== 'completed' && speedHistory.length >= 2 && (
              <div style={{ position: 'relative', height: 30, width: '100%', backgroundColor: S.raised, borderRadius: 'var(--radius-md)', border: `1px solid ${S.border}`, overflow: 'hidden', flexShrink: 0 }}>
                <svg viewBox="0 0 500 30" style={{ position: 'absolute', inset: 0, width: '100%', height: '100%' }} preserveAspectRatio="none">
                  <defs><linearGradient id="chartGrad" x1="0" y1="0" x2="0" y2="1"><stop offset="0%" stopColor={S.brand} stopOpacity={0.3} /><stop offset="100%" stopColor={S.brand} stopOpacity={0} /></linearGradient></defs>
                  {/* eslint-disable-next-line @typescript-eslint/no-explicit-any */}
                  <path d={`M 0,30 L ${speedHistory.map((s, i) => { const x = (i/(speedHistory.length-1))*500; const mx = Math.max(...speedHistory as any, 1024*1024); const y = 30 - ((s as any)/mx)*30*0.8 - 1; return `${x},${y}`; }).join(' ')} L 500,30 Z`} fill="url(#chartGrad)" />
                  {/* eslint-disable-next-line @typescript-eslint/no-explicit-any */}
                  <polyline fill="none" stroke={S.brand} strokeWidth="1.5" points={speedHistory.map((s, i) => { const x = (i/(speedHistory.length-1))*500; const mx = Math.max(...speedHistory as any, 1024*1024); const y = 30-((s as any)/mx)*30*0.8-1; return `${x},${y}`; }).join(' ')} />
                </svg>
              </div>
            )}

            {/* Segment bar + thread table */}
            {currentItem.status !== 'completed' && segments.length > 0 && (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 4, flex: 1, minHeight: 0 }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 'var(--text-xs-size)', color: S.t3 }}>
                  <span style={{ fontWeight: 600 }}>Thread Distribution</span>
                  <span style={{ fontWeight: 600 }}>{segments.length} Connections</span>
                </div>
                {/* Segment bar */}
                <div style={{ display: 'flex', width: '100%', height: 8, gap: 1, backgroundColor: S.elevated, borderRadius: 'var(--radius-md)', overflow: 'hidden', border: `1px solid ${S.border}` }}>
                  {/* eslint-disable-next-line @typescript-eslint/no-explicit-any */}
                  {segments.map((seg: any, idx: number) => {
                    const wp  = fileTotal > 0 ? (seg.allocated_bytes / fileTotal) * 100 : (100 / segments.length);
                    const fp  = seg.allocated_bytes > 0 ? Math.min(100, Math.max(0, (seg.bytes_done / seg.allocated_bytes) * 100)) : 0;
                    const col = getSegmentColors(seg.status);
                    return (
                      <div key={seg.id ?? idx} style={{ position: 'relative', height: '100%', width: `${wp}%`, minWidth: 1, backgroundColor: col.track }}>
                        <div style={{ position: 'absolute', top: 0, bottom: 0, left: 0, width: `${fp}%`, backgroundColor: col.fill, transition: 'width 0.3s ease' }} />
                      </div>
                    );
                  })}
                </div>
                {/* Thread table */}
                <div style={{ flex: 1, minHeight: 60, border: `1px solid ${S.border}`, borderRadius: 'var(--radius-md)', backgroundColor: S.raised, overflow: 'hidden' }}>
                  <div style={{ overflowY: 'auto', height: '100%' }}>
                    <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 'var(--text-xs-size)' }}>
                      <thead style={{ position: 'sticky', top: 0, backgroundColor: S.raised, zIndex: 1, borderBottom: `1px solid ${S.border}` }}>
                        <tr>
                          {['Connection', 'Progress', 'Status'].map((h, i) => (
                            <th key={h} style={{ padding: '6px 10px', fontWeight: 600, color: S.t3, textAlign: i === 2 ? 'right' : 'left', whiteSpace: 'nowrap' }}>{h}</th>
                          ))}
                        </tr>
                      </thead>
                      <tbody>
                        {/* eslint-disable-next-line @typescript-eslint/no-explicit-any */}
                        {segments.map((seg: any, idx: number) => (
                          <tr key={seg.id ?? idx} style={{ borderBottom: `1px solid ${S.borderSub}` }}
                            onMouseEnter={e => (e.currentTarget.style.backgroundColor = S.elevated)}
                            onMouseLeave={e => (e.currentTarget.style.backgroundColor = 'transparent')}>
                            <td style={{ padding: '5px 10px', fontWeight: 500, color: S.t1, whiteSpace: 'nowrap' }}>Thread {idx + 1}</td>
                            <td style={{ padding: '5px 10px', color: S.t3, whiteSpace: 'nowrap' }}>{fmtBytes(seg.bytes_done)} / {seg.allocated_bytes > 0 ? fmtBytes(seg.allocated_bytes) : '???'}</td>
                            <td style={{ padding: '5px 10px', fontWeight: 600, color: segStatusColor(seg.status), textAlign: 'right', whiteSpace: 'nowrap' }}>{(seg.status || 'unknown').toUpperCase()}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </div>
              </div>
            )}
          </>
        )}

        {/* Completion banner */}
        {currentItem.status === 'completed' && (
          <div className="no-drag" style={{ marginTop: 4, backgroundColor: 'var(--color-success-dim)', border: `1px solid ${S.success}`, borderRadius: 'var(--radius-lg)', display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '10px 12px' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, color: S.success, fontWeight: 600, fontSize: 'var(--text-sm-size)' }}>
              <ShieldCheck size={16} /> Download Complete
            </div>
            <div style={{ display: 'flex', gap: 6 }}>
              <button className="btn-secondary flex items-center gap-2" onClick={handleOpenFolder}><Folder size={13} /> Folder</button>
              <button className="btn-primary flex items-center gap-2" onClick={handleOpenFile}><FileIcon size={13} /> Open</button>
            </div>
          </div>
        )}
      </div>

      {/* Footer */}
      <div style={{ height: 48, padding: '0 16px', borderTop: `1px solid ${S.border}`, backgroundColor: S.raised, display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexShrink: 0, userSelect: 'none' }}>
        <button className="btn-secondary" onClick={handleClose}>Hide Window</button>
        <div className="flex gap-2 no-drag">
          {(currentItem.status === 'downloading' || currentItem.status === 'connecting') && (
            <button className="btn-primary flex items-center gap-2" onClick={handlePause} aria-label="Pause download"><Pause size={13} /> Pause</button>
          )}
          {(currentItem.status === 'paused' || currentItem.status === 'failed') && (
            <button className="btn-primary flex items-center gap-2" onClick={handleResume} aria-label={currentItem.status === 'failed' ? 'Retry failed download' : 'Resume download'}>
              {currentItem.status === 'failed' ? <RefreshCw size={13} /> : <Play size={13} />}
              {currentItem.status === 'failed' ? 'Retry' : 'Resume'}
            </button>
          )}
          {currentItem.status !== 'completed' && (
            <button className="btn-danger flex items-center gap-2" onClick={handleCancel}><Square size={13} /> Stop</button>
          )}
        </div>
      </div>
    </div>
  );
}
