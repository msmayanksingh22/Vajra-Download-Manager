import { create } from 'zustand';
import { DownloadInfo, DownloadStatus } from '../types';
import { api } from '../api';
import { listen } from '@tauri-apps/api/event';
interface DownloadStore {
  downloads: Record<string, DownloadInfo>;
  selectedIds: Set<string>;
  sortBy: keyof DownloadInfo | '';
  sortDirection: 'asc' | 'desc';
  filters: {
    status?: DownloadStatus[];
    category?: string;
    search?: string;
  };

  // Actions
  addOrUpdateDownload: (download: DownloadInfo) => void;
  batchUpdateDownloads: (updates: Record<string, Partial<DownloadInfo>>) => void;
  removeDownload: (id: string) => void;
  setDownloads: (downloads: DownloadInfo[]) => void;

  // Selection
  selectDownload: (id: string, multiSelect?: boolean) => void;
  clearSelection: () => void;
  selectAll: () => void;

  // Filtering and Sorting
  setSortBy: (field: keyof DownloadInfo) => void;
  setFilters: (filters: Partial<DownloadStore['filters']>) => void;

  // Computed (getters)
  getSortedAndFilteredDownloads: () => DownloadInfo[];
}

export const useDownloadStore = create<DownloadStore>((set, get) => ({
  downloads: {},
  selectedIds: new Set(),
  sortBy: 'created_at',
  sortDirection: 'desc',
  filters: {},

  addOrUpdateDownload: (download) =>
    set((state) => {
      const existing = state.downloads[download.id];
      if (existing) {
        const existingBytes = existing.bytes_done || 0;
        const incomingBytes = download.bytes_done || 0;

        const merged = { ...download };
        // Protect status: do not let an 'idle' status override a 'paused' or 'failed' status
        if (
          download.status === 'idle' &&
          (existing.status === 'paused' || existing.status === 'failed')
        ) {
          merged.status = existing.status;
        }

        if (existingBytes > incomingBytes || existing.status === 'completed') {
          return {
            downloads: {
              ...state.downloads,
              [download.id]: {
                ...merged,
                ...existing,
                status: merged.status,
              },
            },
          };
        }

        return {
          downloads: {
            ...state.downloads,
            [download.id]: merged,
          },
        };
      }
      return {
        downloads: {
          ...state.downloads,
          [download.id]: download,
        },
      };
    }),

  batchUpdateDownloads: (updates) =>
    set((state) => {
      const newDownloads = { ...state.downloads };
      let changed = false;
      for (const [id, partial] of Object.entries(updates)) {
        if (newDownloads[id]) {
          const merged = { ...newDownloads[id], ...partial };
          if (merged.status === 'completed') {
            merged.progress_pct = 100;
          } else if (merged.total_bytes && merged.total_bytes > 0) {
            merged.progress_pct = (merged.bytes_done / merged.total_bytes) * 100;
          }
          newDownloads[id] = merged;
          changed = true;
        } else {
          const merged = {
            id,
            status: 'queued',
            url: '',
            filename: 'Unknown',
            output_path: '',
            total_bytes: null,
            downloaded_bytes: 0,
            bytes_done: 0,
            speed_bps: 0,
            eta_seconds: null,
            error: null,
            created_at: Date.now(),
            segments: [],
            resume_supported: false,
            ...partial,
          } as DownloadInfo;
          if (merged.status === 'completed') {
            merged.progress_pct = 100;
          } else if (merged.total_bytes && merged.total_bytes > 0) {
            merged.progress_pct = (merged.bytes_done / merged.total_bytes) * 100;
          } else {
            merged.progress_pct = 0;
          }
          newDownloads[id] = merged;
          changed = true;
        }
      }
      return changed ? { downloads: newDownloads } : state;
    }),

  removeDownload: (id) =>
    set((state) => {
      const newDownloads = { ...state.downloads };
      delete newDownloads[id];

      const newSelected = new Set(state.selectedIds);
      newSelected.delete(id);

      return {
        downloads: newDownloads,
        selectedIds: newSelected,
      };
    }),

  setDownloads: (downloadsList) =>
    set((state) => {
      const newDownloads: Record<string, DownloadInfo> = {};
      for (const d of downloadsList) {
        const existing = state.downloads[d.id];
        let merged = { ...d };
        if (existing) {
          // Protect status: do not let an 'idle' status override a 'paused' or 'failed' status
          if (
            d.status === 'idle' &&
            (existing.status === 'paused' || existing.status === 'failed')
          ) {
            merged.status = existing.status;
          }

          // Protect progress
          const existingBytes = existing.bytes_done || 0;
          const incomingBytes = d.bytes_done || 0;
          if (existingBytes > incomingBytes || existing.status === 'completed') {
            merged = {
              ...merged,
              ...existing,
              status: merged.status,
            };
          }
        }
        newDownloads[d.id] = merged;
      }
      return { downloads: newDownloads };
    }),

  selectDownload: (id, multiSelect = false) =>
    set((state) => {
      const newSelected = multiSelect ? new Set(state.selectedIds) : new Set<string>();

      if (newSelected.has(id)) {
        newSelected.delete(id);
      } else {
        newSelected.add(id);
      }

      return { selectedIds: newSelected };
    }),

  clearSelection: () => set({ selectedIds: new Set() }),

  selectAll: () =>
    set((state) => ({
      selectedIds: new Set(Object.keys(state.downloads)),
    })),

  setSortBy: (field) =>
    set((state) => ({
      sortBy: field,
      sortDirection: state.sortBy === field && state.sortDirection === 'desc' ? 'asc' : 'desc',
    })),

  setFilters: (filters) =>
    set((state) => ({
      filters: { ...state.filters, ...filters },
    })),

  getSortedAndFilteredDownloads: () => {
    const { downloads, sortBy, sortDirection, filters } = get();
    let arr = Object.values(downloads);

    // Apply filters
    if (filters.status && filters.status.length > 0) {
      arr = arr.filter((d) => filters.status!.includes(d.status));
    }
    if (filters.search) {
      const s = filters.search.toLowerCase();
      arr = arr.filter(
        (d) => d.filename.toLowerCase().includes(s) || d.url.toLowerCase().includes(s),
      );
    }

    // Apply sort
    if (sortBy) {
      arr.sort((a, b) => {
        const valA = a[sortBy];
        const valB = b[sortBy];

        if (typeof valA === 'string' && typeof valB === 'string') {
          return sortDirection === 'asc' ? valA.localeCompare(valB) : valB.localeCompare(valA);
        }

        if (typeof valA === 'number' && typeof valB === 'number') {
          return sortDirection === 'asc' ? valA - valB : valB - valA;
        }

        // Handle nulls/undefined gracefully
        if (valA === valB) return 0;
        if (valA == null) return sortDirection === 'asc' ? -1 : 1;
        if (valB == null) return sortDirection === 'asc' ? 1 : -1;

        return 0;
      });
    }

    return arr;
  },
}));

// Setup Tauri event listener for vajra-event SSE bridge
export async function initDownloadStoreTauriEvents() {
  const pendingGets = new Set<string>();

  const unsubVajra = await listen('vajra-event', (event) => {
    const store = useDownloadStore.getState();
    const e = event.payload as any;
    console.log('[Tauri Event] Received vajra-event in store:', e);
    switch (e.event) {
      case 'progress': {
        const id = e.download_id;
        if (!id) break;
        // If we don't know about this download yet, fetch full info from API
        if (!store.downloads[id] && !pendingGets.has(id)) {
          pendingGets.add(id);
          api
            .get(id)
            .then((d) => {
              pendingGets.delete(id);
              if (d) store.addOrUpdateDownload(d);
            })
            .catch(() => {
              pendingGets.delete(id);
            });
        }
        store.batchUpdateDownloads({
          [id]: {
            bytes_done: e.downloaded_bytes,
            speed_bps: e.speed_bps,
            eta_seconds: e.eta_seconds,
            total_bytes: e.total_bytes,
            status: e.status,
            segments: e.segments,
            error: e.error,
          } as Partial<DownloadInfo>,
        });
        break;
      }
      case 'batch_progress': {
        const downloads = e.downloads;
        if (!downloads || !Array.isArray(downloads)) break;
        const updates: Record<string, Partial<DownloadInfo>> = {};
        for (const item of downloads) {
          const id = item.download_id;
          if (!id) continue;
          if (!store.downloads[id] && !pendingGets.has(id)) {
            pendingGets.add(id);
            api
              .get(id)
              .then((d) => {
                pendingGets.delete(id);
                if (d) store.addOrUpdateDownload(d);
              })
              .catch(() => {
                pendingGets.delete(id);
              });
          }
          updates[id] = {
            bytes_done: item.downloaded_bytes,
            speed_bps: item.speed_bps,
            eta_seconds: item.eta_seconds,
            total_bytes: item.total_bytes,
            status: item.status,
            segments: item.segments,
            error: item.error,
          } as Partial<DownloadInfo>;
        }
        store.batchUpdateDownloads(updates);
        break;
      }
      case 'state_change': {
        const id = e.id;
        if (!id) break;
        if (!store.downloads[id] && !pendingGets.has(id)) {
          pendingGets.add(id);
          api
            .get(id)
            .then((d) => {
              pendingGets.delete(id);
              if (d) store.addOrUpdateDownload(d);
            })
            .catch(() => {
              pendingGets.delete(id);
            });
        }
        store.batchUpdateDownloads({
          [id]: {
            status: e.status,
            ...(e.output_path ? { output_path: e.output_path } : {}),
            error: e.error,
          } as Partial<DownloadInfo>,
        });
        break;
      }
      case 'added': {
        const id = e.id;
        if (!id) break;
        if (!pendingGets.has(id)) {
          pendingGets.add(id);
          // Fetch full download info from the API so we have all fields
          api
            .get(id)
            .then((d) => {
              pendingGets.delete(id);
              if (d) store.addOrUpdateDownload(d);
              else {
                // Minimal stub so the row appears immediately
                store.batchUpdateDownloads({
                  [id]: { url: e.url, filename: e.filename } as Partial<DownloadInfo>,
                });
              }
            })
            .catch(() => {
              pendingGets.delete(id);
            });
        }
        break;
      }
      case 'removed': {
        store.removeDownload(e.id);
        break;
      }
      case 'intercepted': {
        import('@tauri-apps/api/event')
          .then(({ emit }) => {
            emit('vajra-intercepted', { url: e.url, filename: e.filename });
          })
          .catch(console.error);
        break;
      }
    }
  });

  const unsubFailed = await listen('DownloadFailed', (event) => {
    const store = useDownloadStore.getState();
    const e = event.payload as any;
    console.log('[Tauri Event] Received DownloadFailed in store:', e);
    const id = e.id || e.download_id;
    if (id) {
      store.batchUpdateDownloads({
        [id]: {
          status: 'failed',
          error: e.error || 'Download failed',
          speed_bps: 0,
        } as Partial<DownloadInfo>,
      });
    }
  });

  return () => {
    unsubVajra();
    unsubFailed();
  };
}
