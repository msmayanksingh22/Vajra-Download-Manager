import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { Minus, Square, X as XIcon } from 'lucide-react';
import { api } from './api';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { listen } from '@tauri-apps/api/event';

import MenuBar from './components/MenuBar';
import Toolbar from './components/Toolbar';
import Sidebar from './components/Sidebar';
import DownloadsTable from './components/DownloadsTable';
import Dashboard from './components/Dashboard';
import BatchRenameDialog from './components/BatchRenameDialog';

import OptionsDialog from './components/Dialogs/OptionsDialog';
import SchedulerDialog from './components/Dialogs/SchedulerDialog';
import DeleteDialog from './components/Dialogs/DeleteDialog';
import RefreshUrlDialog from './components/Dialogs/RefreshUrlDialog';
import { GrabberDialog } from './components/Dialogs/GrabberDialog';
import { SpiderDialog } from './components/Dialogs/SpiderDialog';
import PropertiesDialog from './components/Dialogs/PropertiesDialog';
import ImportContainerDialog from './components/Dialogs/ImportContainerDialog';
import AboutDialog from './components/Dialogs/AboutDialog';
import HelpDialog from './components/Dialogs/HelpDialog';
import OnboardingModal from './components/Dialogs/OnboardingModal';

import { playSound } from './audio';
import { Toaster, toast } from 'sonner';
import { readText } from '@tauri-apps/plugin-clipboard-manager';

import { useDownloadStore, initDownloadStoreTauriEvents } from './stores/downloadStore';
import { useConfigStore } from './stores/configStore';
import { useUiStore } from './stores/uiStore';
import { useKeyboardShortcuts } from './hooks/useKeyboardShortcuts';
import { useShallow } from 'zustand/react/shallow';

export default function App() {
  const [isConnected, setIsConnected] = useState(true);
  const config = useConfigStore(useShallow((state) => state.config));
  const fetchConfig = useConfigStore((state) => state.fetchConfig);
  const downloads = useDownloadStore(useShallow((s) => s.getSortedAndFilteredDownloads()));
  const downloadsMap = useDownloadStore((state) => state.downloads);
  const selectedIds = useDownloadStore((state) => state.selectedIds);
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const selectDownload = useDownloadStore((state) => state.selectDownload);

  const [activeCategory, setActiveCategory] = useState('All Downloads');
  const [searchQuery, setSearchQuery] = useState('');
  const searchInputRef = useRef<HTMLInputElement>(null);

  const [isBatchRenameModalOpen, setBatchRenameModalOpen] = useState(false);
  const [showOnboarding, setShowOnboarding] = useState(false);

  const ui = useUiStore();

  useEffect(() => {
    document.documentElement.dir = ui.dir;
  }, [ui.dir]);

  // Show first-run onboarding once after the app has loaded.
  useEffect(() => {
    const dismissed = localStorage.getItem('vajra-onboarding-dismissed');
    if (!dismissed) {
      setShowOnboarding(true);
    }
  }, []);

  // Empty-state CTA in DownloadsTable dispatches this event to avoid prop-drilling
  useEffect(() => {
    const handler = () => spawnAddUrlWindow('');
    document.addEventListener('vajra:open-add-url', handler);
    return () => document.removeEventListener('vajra:open-add-url', handler);
    // spawnAddUrlWindow is stable (defined above and captured in closure)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const isSpawningAddUrl = useRef<boolean>(false);

  const spawnAddUrlWindow = async (url = '', filename = '') => {
    if (isSpawningAddUrl.current) return;
    isSpawningAddUrl.current = true;
    try {
      const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      const existing = await WebviewWindow.getByLabel('add-url');
      if (existing) {
        if (url) {
          const { emit } = await import('@tauri-apps/api/event');
          await emit('update-add-url', { url, filename });
        }
        await existing.unminimize().catch(console.error);
        await existing.setAlwaysOnTop(true).catch(console.error);
        await existing.setAlwaysOnTop(false).catch(console.error);
        await existing.setFocus().catch(console.error);
        isSpawningAddUrl.current = false;
        return;
      }
      const webview = new WebviewWindow('add-url', {
        url: `/?window=addUrl&url=${encodeURIComponent(url)}&filename=${encodeURIComponent(filename)}`,
        title: 'Add New Download',
        width: 560,
        height: 620,
        minWidth: 480,
        minHeight: 520,
        resizable: true,
        maximizable: false,
        decorations: false,
        center: true,
        focus: true,
        alwaysOnTop: false,
        visible: false,
      });

      webview.once('tauri://created', async () => {
        await webview.show().catch(console.error);
        await webview.setAlwaysOnTop(true).catch(console.error);
        await webview.setAlwaysOnTop(false).catch(console.error);
        await webview.setFocus().catch(console.error);
      });

      webview.once('tauri://destroyed', () => {
        // cleanup
      });
    } catch (e) {
      console.error(e);
    } finally {
      isSpawningAddUrl.current = false;
    }
  };

  useKeyboardShortcuts(searchInputRef as React.RefObject<HTMLInputElement>, spawnAddUrlWindow);

  useEffect(() => {
    let unlistenDrop: () => void;
    let unlistenIntercept: () => void;

    const setup = async () => {
      unlistenDrop = await listen<{ paths: string[] }>('tauri://drop', (event) => {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const paths = event.payload.paths || (event.payload as any);
        if (Array.isArray(paths) && paths.length > 0) {
          // Open Add URL window with the first dropped file/url
          spawnAddUrlWindow(paths[0], '');
        }
      });

      unlistenIntercept = await listen<{ url: string; filename: string }>(
        'vajra-intercepted',
        (event) => {
          spawnAddUrlWindow(event.payload.url, event.payload.filename);
        },
      );
    };

    setup();
    return () => {
      if (unlistenDrop) unlistenDrop();
      if (unlistenIntercept) unlistenIntercept();
    };
  }, []);

  const completedWindowsShown = useRef<Set<string>>(new Set());
  const failedWindowsShown = useRef<Set<string>>(new Set());
  const activeProgressWindows = useRef<Set<string>>(new Set());
  const lastSSEUpdate = useRef<number>(Date.now());
  const retryCounts = useRef<Record<string, number>>({});

  const spawnDownloadCompleteWindow = useCallback(async (id: string) => {
    try {
      const dl = await api.get(id);
      if (dl && dl.tags && dl.tags.includes('cli')) return;
    } catch {
      /* ignore */
    }

    if (completedWindowsShown.current.has(id)) return;
    completedWindowsShown.current.add(id);

    const item = useDownloadStore.getState().downloads[id];
    if (item) {
      localStorage.setItem(`vajra_complete_init_${id}`, JSON.stringify(item));
    }

    try {
      const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      const existing = await WebviewWindow.getByLabel(`complete-${id}`);
      if (existing) {
        await existing.setFocus();
        return;
      }
      const webview = new WebviewWindow(`complete-${id}`, {
        url: `/?window=downloadComplete&id=${encodeURIComponent(id)}`,
        title: 'Download Complete',
        width: 480,
        height: 420,
        resizable: false,
        maximizable: false,
        center: true,
        decorations: false,
        alwaysOnTop: false,
        visible: false,
      });

      webview.once('tauri://created', async () => {
        await webview.show().catch(console.error);
        await webview.setAlwaysOnTop(true).catch(console.error);
        await webview.setAlwaysOnTop(false).catch(console.error);
        await webview.setFocus().catch(console.error);
        playSound('success');
      });

      webview.once('tauri://destroyed', () => {
        // window closed
      });
    } catch (e) {
      console.error('Failed to spawn download complete window:', e);
      toast.error(`Failed to show complete window: ${String(e)}`);
      completedWindowsShown.current.delete(id);
    }
  }, []);

  const spawnDownloadFailedWindow = useCallback(async (id: string) => {
    try {
      const dl = await api.get(id);
      if (dl && dl.tags && dl.tags.includes('cli')) return;
    } catch {
      /* ignore */
    }

    console.log('[spawnDownloadFailedWindow] Spawning failed window for:', id);
    if (failedWindowsShown.current.has(id)) return;
    failedWindowsShown.current.add(id);

    const item = useDownloadStore.getState().downloads[id];
    if (item) {
      localStorage.setItem(`vajra_failed_init_${id}`, JSON.stringify(item));
    }

    try {
      const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      const existing = await WebviewWindow.getByLabel(`failed-${id}`);
      if (existing) {
        await existing.setFocus();
        return;
      }
      const webview = new WebviewWindow(`failed-${id}`, {
        url: `/?window=downloadFailed&id=${encodeURIComponent(id)}`,
        title: 'Download Failed',
        width: 480,
        height: 420,
        resizable: false,
        maximizable: false,
        center: true,
        decorations: false,
        alwaysOnTop: false,
        visible: false,
      });

      webview.once('tauri://created', async () => {
        await webview.show().catch(console.error);
        await webview.setAlwaysOnTop(true).catch(console.error);
        await webview.setAlwaysOnTop(false).catch(console.error);
        await webview.setFocus().catch(console.error);
        playSound('fail');
      });

      webview.once('tauri://destroyed', () => {
        failedWindowsShown.current.delete(id);
      });
    } catch (e) {
      console.error('Failed to spawn download failed window:', e);
      toast.error(`Failed to show failed window: ${String(e)}`);
      failedWindowsShown.current.delete(id);
    }
  }, []);

  const spawnProgressWindow = useCallback(async (id: string) => {
    if (activeProgressWindows.current.has(id)) return;
    activeProgressWindows.current.add(id);

    const item = useDownloadStore.getState().downloads[id];
    if (item) {
      localStorage.setItem(`vajra_progress_init_${id}`, JSON.stringify(item));
    }

    try {
      const existing = await WebviewWindow.getByLabel(`progress-${id}`);
      if (existing) {
        await existing.setFocus();
        return;
      }
      const webview = new WebviewWindow(`progress-${id}`, {
        url: `/?window=progress&id=${encodeURIComponent(id)}`,
        title: 'Download Progress',
        width: 540,
        height: 580,
        resizable: true,
        maximizable: false,
        center: true,
        decorations: false,
        alwaysOnTop: false,
        visible: false,
      });

      webview.once('tauri://created', async () => {
        await webview.show().catch(console.error);
        await webview.setAlwaysOnTop(true).catch(console.error);
        await webview.setAlwaysOnTop(false).catch(console.error);
        await webview.setFocus().catch(console.error);
      });

      webview.once('tauri://destroyed', () => {
        activeProgressWindows.current.delete(id);
      });
    } catch (e) {
      console.error(e);
      toast.error(`Failed to show progress window: ${String(e)}`);
      activeProgressWindows.current.delete(id);
    }
  }, []);

  const hasInitialFetchCompleted = useRef(false);

  const fetchDownloads = useCallback(() => {
    api
      .list()
      .then((res) => {
        useDownloadStore.getState().setDownloads(res.items || []);
        hasInitialFetchCompleted.current = true;
        lastSSEUpdate.current = Date.now();
      })
      .catch(console.error);
  }, []);

  useEffect(() => {
    fetchDownloads();
    fetchConfig();

    let unmounted = false;
    let unlistenProgress: (() => void) | null = null;
    let unlistenAdd: (() => void) | null = null;

    let timerId: any = null;
    const runPoll = () => {
      if (unmounted) return;

      const active = Object.values(useDownloadStore.getState().downloads).some(
        (dl) =>
          dl.status === 'downloading' || dl.status === 'connecting' || dl.status === 'verifying',
      );
      const sseIdleTime = Date.now() - lastSSEUpdate.current;
      const threshold = active ? 2500 : 10000;

      if (sseIdleTime > threshold) {
        api
          .list()
          .then((res) => {
            if (!unmounted) {
              useDownloadStore.getState().setDownloads(res.items || []);
              hasInitialFetchCompleted.current = true;
              lastSSEUpdate.current = Date.now();
            }
          })
          .catch((err) => {
            console.error('[vajra-poll] Failed to poll downloads:', err);
          });
      }

      const delay = active ? 2000 : 10000;
      timerId = setTimeout(runPoll, delay);
    };

    // Start dynamic polling timeout loop
    timerId = setTimeout(runPoll, 2000);

    const healthInterval = setInterval(() => {
      api
        .health()
        .then((res) => {
          if (res?.status === 'ok') {
            setIsConnected(true);
          }
        })
        .catch(() => {
          setIsConnected(false);
        });
    }, 5000);

    listen('open-progress-window', async (event) => {
      const id = event.payload as string;
      const item = useDownloadStore.getState().downloads[id];
      if (item) {
        localStorage.setItem(`vajra_progress_init_${id}`, JSON.stringify(item));
      }
      try {
        const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
        const existing = await WebviewWindow.getByLabel(`progress-${id}`);
        if (existing) {
          await existing.setFocus();
          return;
        }
        const webview = new WebviewWindow(`progress-${id}`, {
          url: `/?window=progress&id=${encodeURIComponent(id)}`,
          title: 'Vajra Progress',
          width: 540,
          height: 580,
          resizable: true,
          maximizable: false,
          center: true,
          decorations: false,
          transparent: true,
          shadow: false,
          alwaysOnTop: false,
          visible: false,
        });
        webview.once('tauri://created', async () => {
          await webview.show().catch(console.error);
          await webview.setAlwaysOnTop(true).catch(console.error);
          await webview.setAlwaysOnTop(false).catch(console.error);
          await webview.setFocus().catch(console.error);
        });
        webview.once('tauri://error', (e) => console.error(e));
      } catch (e) {
        console.error('Spawn progress window error:', e);
      }
    })
      .then((unsub) => {
        if (unmounted) unsub();
        else unlistenProgress = unsub;
      })
      .catch(console.error);

    listen('open-addurl-window', async (event) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const p = (event.payload as any) || {};
      const q = new URLSearchParams();
      if (p.url) q.set('url', p.url);
      if (p.filename) q.set('filename', p.filename);
      try {
        const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
        const existing = await WebviewWindow.getByLabel('addurl');
        if (existing) {
          await existing.setFocus();
          if (p.url) existing.emit('update-add-url', { url: p.url, filename: p.filename });
          return;
        }
        const webview = new WebviewWindow('addurl', {
          url: `/?window=addUrl&${q.toString()}`,
          title: 'Add Download',
          width: 600,
          height: 520,
          resizable: false,
          maximizable: false,
          decorations: false,
          transparent: true,
          shadow: false,
          alwaysOnTop: false,
          visible: false,
        });
        webview.once('tauri://created', async () => {
          await webview.show().catch(console.error);
          await webview.setAlwaysOnTop(true).catch(console.error);
          await webview.setAlwaysOnTop(false).catch(console.error);
          await webview.setFocus().catch(console.error);
        });
        webview.once('tauri://error', (e) => console.error(e));
      } catch (e) {
        console.error('Spawn addurl window error:', e);
      }
    })
      .then((unsub) => {
        if (unmounted) unsub();
        else unlistenAdd = unsub;
      })
      .catch(console.error);

    let unsubSSE: (() => void) | null = null;
    initDownloadStoreTauriEvents()
      .then((unsub) => {
        if (unmounted) unsub();
        else unsubSSE = unsub;
      })
      .catch(console.error);

    let unlistenVajraApp: (() => void) | null = null;
    listen('vajra-event', (event: any) => {
      lastSSEUpdate.current = Date.now();
      console.log('RECEIVED EVENT (App):', event);
      const e = event.payload;
      if (
        (e.event === 'state_change' && e.status === 'completed') ||
        e.event === 'completed' ||
        e.status === 'completed'
      ) {
        const id = e.id || e.download_id;
        if (id) spawnDownloadCompleteWindow(id);
      }
    })
      .then((unsub) => {
        if (unmounted) unsub();
        else unlistenVajraApp = unsub;
      })
      .catch(console.error);

    // Primary trigger: ProgressWindow emits this just before closing on completion.
    // More reliable than the Zustand subscriber because it has no timing race.
    let unlistenDownloadComplete: (() => void) | null = null;
    listen('vajra-download-complete', (event: any) => {
      const id = event.payload as string;
      if (id) spawnDownloadCompleteWindow(id);
    })
      .then((unsub) => {
        if (unmounted) unsub();
        else unlistenDownloadComplete = unsub;
      })
      .catch(console.error);

    let unlistenAddDialog: (() => void) | null = null;
    listen('open-add-url-dialog', () => {
      spawnAddUrlWindow();
    })
      .then((unsub) => {
        if (unmounted) unsub();
        else unlistenAddDialog = unsub;
      })
      .catch(console.error);

    let unlistenSpiderNl: (() => void) | null = null;
    listen('open-spider-with-nl', (event: any) => {
      const p = event.payload || {};
      ui.setSpiderInitial(p.url, p.extensions);
      ui.setSpiderModalOpen(true);
    })
      .then((unsub) => {
        if (unmounted) unsub();
        else unlistenSpiderNl = unsub;
      })
      .catch(console.error);

    let unlistenPauseAll: (() => void) | null = null;
    listen('tray-pause-all', () => {
      handleStopAll();
    })
      .then((unsub) => {
        if (unmounted) unsub();
        else unlistenPauseAll = unsub;
      })
      .catch(console.error);

    let unlistenResumeAll: (() => void) | null = null;
    listen('tray-resume-all', () => {
      handleResumeAll();
    })
      .then((unsub) => {
        if (unmounted) unsub();
        else unlistenResumeAll = unsub;
      })
      .catch(console.error);

    const unsub = useDownloadStore.subscribe((state, prevState) => {
      Object.values(state.downloads).forEach((dl) => {
        const prev = prevState.downloads[dl.id];

        // Completion Check
        if (
          hasInitialFetchCompleted.current &&
          dl.status === 'completed' &&
          prev &&
          prev.status !== 'completed'
        ) {
          spawnDownloadCompleteWindow(dl.id);
        }

        // Failure Check with Auto-Retry and Failed Window Trigger
        if (
          hasInitialFetchCompleted.current &&
          dl.status === 'failed' &&
          prev &&
          prev.status !== 'failed'
        ) {
          const id = dl.id;
          const currentRetries = retryCounts.current[id] || 0;
          const maxRetries = config?.max_retries !== undefined ? config.max_retries : 2;
          console.log(
            `[Zustand-Failed] Transition to failed detected for ${id}. Attempts: ${currentRetries}, Max allowed: ${maxRetries}`,
          );

          if (currentRetries < maxRetries) {
            retryCounts.current[id] = currentRetries + 1;
            localStorage.setItem(`vajra_retry_count_${id}`, String(currentRetries + 1));
            console.log(
              `[Auto-Retry] Triggering attempt ${currentRetries + 1}/${maxRetries} for ${id} in 2s...`,
            );
            toast.warning(
              `Download failed. Retrying... (Attempt ${currentRetries + 1}/${maxRetries})`,
            );

            setTimeout(async () => {
              try {
                await api.patch(id, { action: 'resume' });
                fetchDownloads();
              } catch (err) {
                console.error(`[Auto-Retry] Failed to trigger retry for ${id}:`, err);
              }
            }, 2000);
          } else {
            // Keep retry count at maxRetries + 1 to stop future retries on state updates.
            retryCounts.current[id] = maxRetries + 1;
            localStorage.setItem(`vajra_retry_count_${id}`, String(maxRetries));
            console.log(`[Auto-Retry] Max retries reached for ${id}. Showing failed window.`);

            // Close progress window if open
            (async () => {
              try {
                const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
                const progressWin = await WebviewWindow.getByLabel(`progress-${id}`);
                if (progressWin) {
                  await progressWin.close().catch(console.error);
                }
              } catch (err) {
                console.error('Failed to close progress window:', err);
              }
              spawnDownloadFailedWindow(id);
            })();
          }
        }
      });
    });

    return () => {
      unmounted = true;
      if (unlistenProgress) unlistenProgress();
      if (unlistenAdd) unlistenAdd();
      if (unlistenVajraApp) unlistenVajraApp();
      if (unlistenDownloadComplete) unlistenDownloadComplete();
      if (unlistenAddDialog) unlistenAddDialog();
      if (unlistenPauseAll) unlistenPauseAll();
      if (unlistenResumeAll) unlistenResumeAll();
      if (unlistenSpiderNl) unlistenSpiderNl();
      if (unsubSSE) unsubSSE();
      if (timerId) clearTimeout(timerId);
      clearInterval(healthInterval);
      unsub();
    };
  }, [
    spawnProgressWindow,
    fetchDownloads,
    fetchConfig,
    spawnDownloadCompleteWindow,
    spawnDownloadFailedWindow,
    config,
  ]);

  // Drag and Drop
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWindow()
      .onDragDropEvent((event) => {
        if (event.payload.type === 'drop') {
          const files = event.payload.paths;
          if (files && files.length > 0) {
            const file = files[0];
            if (file.toLowerCase().endsWith('.torrent')) {
              spawnAddUrlWindow(file);
            }
          }
        }
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(console.error);

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // Clipboard monitor
  const lastClipboardText = useRef<string | null>(null);
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const setupClipboardListener = async () => {
      try {
        unlisten = await listen('plugin:clipboard://clipboard-monitor/update', async () => {
          try {
            const text = await readText();
            if (text && text !== lastClipboardText.current) {
              lastClipboardText.current = text;
              const trimmed = text.trim();
              const isUrl = /^https?:\/\//i.test(trimmed) || /^magnet:\?/i.test(trimmed);

              if (isUrl) {
                toast.info('Download Link Detected - Click to Add', {
                  description: trimmed.length > 50 ? trimmed.substring(0, 47) + '...' : trimmed,
                  action: {
                    label: 'Add Download',
                    onClick: () => spawnAddUrlWindow(trimmed),
                  },
                  duration: 5000,
                });
              }
            }
          } catch (e) {
            /* ignore */
          }
        });
      } catch (e) {
        console.error('Failed to setup clipboard listener', e);
      }
    };
    setupClipboardListener();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const handleMinimize = () => getCurrentWindow().minimize().catch(console.error);
  const handleMaximize = () => getCurrentWindow().toggleMaximize().catch(console.error);
  const handleClose = () => getCurrentWindow().close().catch(console.error);

  const dismissOnboarding = () => {
    localStorage.setItem('vajra-onboarding-dismissed', '1');
    setShowOnboarding(false);
  };

  const handleSelect = useCallback((id: string, shift: boolean, ctrl: boolean) => {
    if (ctrl) {
      useDownloadStore.getState().selectDownload(id, true);
    } else {
      useDownloadStore.getState().clearSelection();
      useDownloadStore.getState().selectDownload(id, false);
    }
  }, []);

  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const handleSelectAll = useCallback((ids: string[]) => {
    useDownloadStore.getState().selectAll();
  }, []);

  const handlePause = async (id: string) => {
    if (!id) return;
    try {
      await api.patch(id, { action: 'pause' });
      fetchDownloads();
    } catch (e) {
      console.error(e);
    }
  };

  const handleResume = async (id: string) => {
    if (!id) return;
    try {
      retryCounts.current[id] = 0;
      localStorage.removeItem(`vajra_retry_count_${id}`);
      await api.patch(id, { action: 'resume' });
      fetchDownloads();
      spawnProgressWindow(id);
    } catch (e) {
      console.error(e);
    }
  };

  const executeDeleteSelected = async (deleteFromDisk: boolean, remember: boolean = false) => {
    ui.setDeleteModalOpen(false);
    if (remember) {
      localStorage.setItem('vajra_delete_preference', deleteFromDisk ? 'disk' : 'list_only');
    }
    const ids = [...selectedIds];
    useDownloadStore.getState().clearSelection();
    for (const id of ids) {
      try {
        await api.remove(id, deleteFromDisk);
      } catch (e) {
        console.error(e);
      }
    }
    fetchDownloads();
  };

  const handleDeleteCompleted = async () => {
    const completed = downloads.filter((d) => d.status === 'completed');
    if (completed.length === 0) return;
    for (const d of completed) {
      try {
        await api.remove(d.id, false);
      } catch (e) {
        /* ignore */
      }
    }
    fetchDownloads();
    useDownloadStore.getState().clearSelection();
    toast.success(
      `Cleared ${completed.length} completed download${completed.length !== 1 ? 's' : ''}`,
    );
  };

  const handlePauseSelected = async () => {
    for (const id of selectedIds) {
      const d = downloadsMap[id];
      if (d && (d.status === 'downloading' || d.status === 'connecting')) await handlePause(id);
    }
  };

  const handleResumeSelected = async () => {
    let count = 0;
    for (const id of selectedIds) {
      const d = downloadsMap[id];
      if (d && (d.status === 'paused' || d.status === 'failed')) {
        await handleResume(id);
        count++;
      }
    }
    if (count > 0) toast.success(`Resumed ${count} download${count !== 1 ? 's' : ''}`);
  };

  const handleResumeAll = async () => {
    const pausedOrError = downloads.filter((d) => d.status === 'paused' || d.status === 'failed');
    for (const d of pausedOrError) {
      await handleResume(d.id);
    }
  };

  const handleStopAll = async () => {
    const active = downloads.filter((d) => d.status === 'downloading' || d.status === 'connecting');
    for (const d of active) {
      await handlePause(d.id);
    }
    if (active.length > 0)
      toast.info(`Stopped ${active.length} active download${active.length !== 1 ? 's' : ''}`);
  };

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const handleGridAction = useCallback(
    async (item: any, action: string) => {
      if (action === 'pause') handlePause(item.id);
      else if (action === 'resume') handleResume(item.id);
      else if (action === 'delete') {
        useDownloadStore.getState().clearSelection();
        useDownloadStore.getState().selectDownload(item.id, false);
        ui.setDeleteModalOpen(true);
      } else if (action === 'open') {
        try {
          const { invoke } = await import('@tauri-apps/api/core');
          await invoke('open_file_path', { path: item.output_path });
        } catch (e) {
          console.error('Failed to open file', e);
        }
      } else if (action === 'properties') {
        ui.setPropertiesModalItem(item);
        ui.setPropertiesModalOpen(true);
      } else if (action === 'refresh_url') {
        ui.setRefreshModalItem(item);
        ui.setRefreshModalOpen(true);
      } else if (action === 'show_progress') {
        spawnProgressWindow(item.id);
      }
    },
    [ui, spawnProgressWindow],
  );

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const handleDoubleClick = useCallback(
    (item: any) => {
      if (item.status === 'completed') {
        handleGridAction(item, 'open');
      } else if (item.status === 'downloading' || item.status === 'connecting') {
        spawnProgressWindow(item.id);
      } else {
        handleGridAction(item, 'properties');
      }
      // eslint-disable-next-line react-hooks/exhaustive-deps
    },
    [spawnProgressWindow, handleGridAction],
  );

  const canResume = useMemo(
    () =>
      downloads.some(
        (d) => selectedIds.has(d.id) && (d.status === 'paused' || d.status === 'failed'),
      ),
    [downloads, selectedIds],
  );
  const isSelectedFailed = useMemo(
    () => downloads.some((d) => selectedIds.has(d.id) && d.status === 'failed'),
    [downloads, selectedIds],
  );
  const canPause = useMemo(
    () =>
      downloads.some(
        (d) => selectedIds.has(d.id) && (d.status === 'downloading' || d.status === 'connecting'),
      ),
    [downloads, selectedIds],
  );
  const canStopAll = useMemo(
    () => downloads.some((d) => d.status === 'downloading' || d.status === 'connecting'),
    [downloads],
  );
  const canDelete = selectedIds.size > 0;

  const filteredDownloads = useMemo(() => {
    if (!searchQuery.trim()) return downloads;
    const q = searchQuery.toLowerCase();
    return downloads.filter(
      (d) =>
        (d.filename && d.filename.toLowerCase().includes(q)) ||
        (d.url && d.url.toLowerCase().includes(q)),
    );
  }, [downloads, searchQuery]);

  return (
    <div className="app-root flex flex-col h-screen overflow-hidden">
      <Toaster
        position="bottom-right"
        visibleToasts={3}
        duration={3000}
        theme={document.documentElement.classList.contains('dark') ? 'dark' : 'light'}
        toastOptions={{
          style: {
            background: 'var(--color-surface-elevated)',
            border: '1px solid var(--color-border)',
            color: 'var(--color-text-1)',
            fontSize: 'var(--text-sm-size)',
            fontFamily: 'var(--font-sans)',
          },
        }}
      />

      {/* OS window controls — top-right overlay */}
      <div
        className="absolute no-drag"
        style={{ top: 0, right: 0, height: 32, display: 'flex', zIndex: 50 }}
      >
        {(
          [
            { icon: <Minus size={12} />, title: 'Minimize', action: handleMinimize, danger: false },
            {
              icon: <Square size={11} />,
              title: 'Maximize',
              action: handleMaximize,
              danger: false,
            },
            { icon: <XIcon size={13} />, title: 'Close', action: handleClose, danger: true },
          ] as { icon: React.ReactNode; title: string; action: () => void; danger: boolean }[]
        ).map(({ icon, title, action, danger }) => (
          <button
            key={title}
            title={title}
            onClick={action}
            className={`window-chrome-btn${danger ? ' danger' : ''}`}
          >
            {icon}
          </button>
        ))}
      </div>

      {/* Menu bar + drag region */}
      <div className="menu-bar-region">
        <MenuBar
          onAdd={() => spawnAddUrlWindow('')}
          onGrabber={() => ui.setGrabberModalOpen(true)}
          onOptions={() => ui.setSettingsModalOpen(true)}
          onPauseAll={handleStopAll}
          onResumeAll={handleResumeAll}
          onHelp={() => ui.setHelpModalOpen(true)}
          onAbout={() => ui.setAboutModalOpen(true)}
          onSpider={() => ui.setSpiderModalOpen(true)}
          onScheduler={() => ui.setSchedulerModalOpen(true)}
          onBatchRename={() => setBatchRenameModalOpen(true)}
          onClearCompleted={handleDeleteCompleted}
        />
      </div>

      {/* Toolbar */}
      <div className="no-drag">
        <Toolbar
          selectedIds={selectedIds}
          onAdd={() => spawnAddUrlWindow('')}
          onResumeSelected={handleResumeSelected}
          onPauseSelected={handlePauseSelected}
          onStopAll={handleStopAll}
          onDeleteSelected={() => {
            const pref = localStorage.getItem('vajra_delete_preference');
            if (pref) executeDeleteSelected(pref === 'disk', false);
            else ui.setDeleteModalOpen(true);
          }}
          onDeleteCompleted={handleDeleteCompleted}
          onOptions={() => ui.setSettingsModalOpen(true)}
          onScheduler={() => ui.setSchedulerModalOpen(true)}
          onGrabber={() => ui.setGrabberModalOpen(true)}
          onSpider={() => ui.setSpiderModalOpen(true)}
          onBatchRename={() => setBatchRenameModalOpen(true)}
          onHelp={() => ui.setHelpModalOpen(true)}
          canResume={canResume}
          resumeLabel={isSelectedFailed ? 'Retry' : 'Resume'}
          canPause={canPause}
          canStopAll={canStopAll}
          canDelete={canDelete}
          canDeleteCompleted={downloads.some((d) => d.status === 'completed')}
          onShowProgress={() => {
            if (selectedIds.size === 1) {
              const d = downloadsMap[Array.from(selectedIds)[0]];
              if (d?.status === 'completed') {
                ui.setPropertiesModalItem(d);
                ui.setPropertiesModalOpen(true);
              } else spawnProgressWindow(Array.from(selectedIds)[0]);
            }
          }}
          canShowProgress={selectedIds.size === 1}
          isSelectedCompleted={(() => {
            if (selectedIds.size !== 1) return false;
            const d = downloadsMap[Array.from(selectedIds)[0]];
            return d?.status === 'completed';
          })()}
        >
          <div className="flex-1 min-w-[12px]" />
          <div className="relative flex items-center pr-2">
            <input
              ref={searchInputRef}
              type="text"
              placeholder="Search (Ctrl+F)"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="search-input no-drag"
            />
          </div>
        </Toolbar>
      </div>

      {/* Main content: sidebar + table */}
      <div className="no-drag flex flex-1 overflow-hidden">
        <Sidebar
          downloads={downloads}
          activeCategory={activeCategory}
          onSelectCategory={setActiveCategory}
          categoryRules={config?.category_rules || []}
        />
        <div className="main-content-area">
          {activeCategory === 'Dashboard' ? (
            <Dashboard onNavigate={setActiveCategory} />
          ) : (
            <DownloadsTable
              items={filteredDownloads}
              selectedIds={selectedIds}
              activeCategory={activeCategory}
              categoryRules={config?.category_rules || []}
              onSelect={handleSelect}
              onSelectAll={handleSelectAll}
              onDoubleClick={handleDoubleClick}
              onAction={handleGridAction}
            />
          )}
        </div>
      </div>

      {/* Status bar */}
      <div className="status-bar no-drag" aria-live="polite" aria-label="Status bar">
        <div className="flex items-center" style={{ gap: 12 }}>
          <span>{downloads.length} downloads</span>
          <span className="status-bar-sep" />
          <span>
            {
              downloads.filter((d) => d.status === 'downloading' || d.status === 'connecting')
                .length
            }{' '}
            active
          </span>
        </div>
        <div className="flex items-center" style={{ gap: 12 }}>
          <div className="flex items-center" style={{ gap: 4 }}>
            <span
              style={{
                fontWeight: 600,
                textTransform: 'uppercase',
                letterSpacing: '0.08em',
                fontSize: 9,
              }}
              className="text-4"
            >
              Speed:
            </span>
            <span className="text-1" style={{ fontWeight: 500 }}>
              {(() => {
                const totalSpeed = downloads.reduce(
                  (sum, d) =>
                    sum +
                    (d.status === 'downloading' || d.status === 'connecting'
                      ? d.speed_bps || 0
                      : 0),
                  0,
                );
                return `${(totalSpeed / (1024 * 1024)).toFixed(1)} MB/s`;
              })()}
            </span>
          </div>
          <span className="status-bar-sep" />
          <div className="flex items-center" style={{ gap: 6 }}>
            <div className={`status-dot ${isConnected ? 'connected' : 'disconnected'}`} />
            <span>{isConnected ? 'Connected' : 'Disconnected'}</span>
          </div>
        </div>
      </div>

      {/* Dialogs */}

      {ui.isSettingsModalOpen && (
        <OptionsDialog
          onClose={() => {
            ui.setSettingsModalOpen(false);
            fetchConfig();
          }}
        />
      )}

      {ui.isSchedulerModalOpen && (
        <SchedulerDialog downloads={downloads} onClose={() => ui.setSchedulerModalOpen(false)} />
      )}

      {ui.isDeleteModalOpen && (
        <DeleteDialog
          count={selectedIds.size}
          onClose={() => ui.setDeleteModalOpen(false)}
          onConfirm={executeDeleteSelected}
        />
      )}

      {ui.isRefreshModalOpen && (
        <RefreshUrlDialog
          item={ui.refreshModalItem}
          onClose={() => ui.setRefreshModalOpen(false)}
          onOk={() => {
            ui.setRefreshModalOpen(false);
            fetchDownloads();
          }}
        />
      )}

      {ui.isGrabberModalOpen && (
        <div className="dialog-overlay">
          <GrabberDialog
            onClose={() => {
              ui.setGrabberModalOpen(false);
              fetchDownloads();
            }}
          />
        </div>
      )}

      {isBatchRenameModalOpen && (
        <BatchRenameDialog
          items={downloads.filter((d) => selectedIds.has(d.id))}
          onClose={() => {
            setBatchRenameModalOpen(false);
            fetchDownloads();
          }}
        />
      )}

      <SpiderDialog
        open={ui.isSpiderModalOpen}
        onOpenChange={ui.setSpiderModalOpen}
        onBatchAdd={async (urls) => {
          for (const url of urls) {
            try {
              await api.add({ url });
            } catch (e) {
              console.error('Failed to batch add URL', url, e);
            }
          }
          fetchDownloads();
        }}
      />

      {ui.isPropertiesModalOpen && (
        <PropertiesDialog
          item={ui.propertiesModalItem}
          onClose={() => ui.setPropertiesModalOpen(false)}
          onSave={fetchDownloads}
        />
      )}

      {ui.isImportContainerModalOpen && (
        <ImportContainerDialog
          onClose={() => ui.setImportContainerModalOpen(false)}
          onImport={async (urls, outputDir) => {
            ui.setImportContainerModalOpen(false);
            for (const url of urls) {
              try {
                await api.add({ url, output_dir: outputDir });
              } catch (e) {
                console.error('Failed to add decrypted URL:', url, e);
              }
            }
            fetchDownloads();
          }}
        />
      )}
      {ui.isAboutModalOpen && <AboutDialog onClose={() => ui.setAboutModalOpen(false)} />}
      {ui.isHelpModalOpen && <HelpDialog onClose={() => ui.setHelpModalOpen(false)} />}

      {showOnboarding && <OnboardingModal onClose={dismissOnboarding} />}
    </div>
  );
}
