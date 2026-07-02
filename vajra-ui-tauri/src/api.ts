// Vajra REST API client — connects to vajrad on 127.0.0.1:6277
import { invoke } from '@tauri-apps/api/core';
import {
  AddDownloadRequest,
  DaemonConfig,
  DownloadInfo,
  DownloadList,
  StatsResponse,
  InspectResponse,
  VaultCredentialResponse,
  AddVaultCredentialRequest,
  PatchDownloadRequest,
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  InspectRequest,
} from './types';

const PORT = 6277;
const BASE = `http://127.0.0.1:${PORT}/api/v1`;
const HEALTH = `http://127.0.0.1:${PORT}/health`;

// eslint-disable-next-line @typescript-eslint/no-explicit-any
async function req<T>(method: string, path: string, body?: any): Promise<T> {
  const opts: RequestInit = { method, headers: {} };
  if (body) {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (opts.headers as any)['Content-Type'] = 'application/json';
    opts.body = JSON.stringify(body);
  }
  const url = path.startsWith('http') ? path : `${BASE}${path}`;
  const r = await fetch(url, opts);
  if (!r.ok) {
    const text = await r.text().catch(() => r.statusText);
    throw new Error(text || `HTTP ${r.status}`);
  }
  const ct = r.headers.get('content-type') || '';
  if (ct.includes('application/json')) return r.json();
  const text = await r.text();
  try {
    return JSON.parse(text);
  } catch {
    return text as unknown as T;
  }
}

const isMock = window.location.search.includes('mock=true');

export const api = {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  health: (): Promise<any> => (isMock ? Promise.resolve({ status: 'ok' }) : req('GET', HEALTH)),
  stats: (): Promise<StatsResponse> =>
    isMock
      ? Promise.resolve({
          active_count: 1,
          queued_count: 0,
          paused_count: 0,
          complete_today: 0,
          failed_today: 0,
          aggregate_speed_bps: 0,
          aggregate_limit_bps: null,
          total_downloaded_bytes: 0,
          daemon_uptime_seconds: 0,
        })
      : req('GET', '/stats'),
  list: (params: string = ''): Promise<DownloadList> =>
    isMock
      ? Promise.resolve({ items: [], total: 0, limit: 100, offset: 0 })
      : req('GET', `/downloads${params}`),
  get: (id: string): Promise<DownloadInfo> =>
    isMock ? Promise.resolve({} as DownloadInfo) : req('GET', `/downloads/${id}`),
  add: (
    body: AddDownloadRequest,
  ): Promise<{ id: string; status: string; url: string; filename?: string; created_at: number }> =>
    isMock
      ? Promise.resolve({
          id: 'mock-2',
          status: 'downloading',
          url: body.url,
          filename: body.filename,
          created_at: Date.now(),
        })
      : req('POST', '/downloads', body),
  patch: (id: string, body: PatchDownloadRequest): Promise<void> =>
    isMock ? Promise.resolve() : req('PATCH', `/downloads/${id}`, body),
  remove: (id: string, del?: boolean): Promise<void> =>
    isMock ? Promise.resolve() : req('DELETE', `/downloads/${id}${del ? '?delete_file=true' : ''}`),
  inspect: (url: string): Promise<InspectResponse> =>
    isMock
      ? Promise.resolve({
          effective_url: url,
          filename: 'mock_file.zip',
          total_bytes: 1024 * 1024 * 500,
          content_type: 'application/zip',
          accepts_ranges: true,
          ytdlp_supported: false,
        })
      : req('POST', '/inspect', { url }),
  config: (): Promise<DaemonConfig> =>
    isMock ? Promise.resolve({} as DaemonConfig) : req('GET', '/config'),
  setConfig: (body: Partial<DaemonConfig>): Promise<void> =>
    isMock ? Promise.resolve() : req('PATCH', '/config', body),
  getVault: (): Promise<VaultCredentialResponse[]> =>
    isMock ? Promise.resolve([]) : req('GET', '/vault'),
  addVault: (body: AddVaultCredentialRequest): Promise<VaultCredentialResponse> =>
    isMock
      ? Promise.resolve({
          id: 'mock',
          domain: body.domain,
          username: body.username,
          created_at: Date.now(),
        })
      : req('POST', '/vault', body),
  deleteVault: (id: string): Promise<void> =>
    isMock ? Promise.resolve() : req('DELETE', `/vault/${id}`),
  getAuditLogs: (): Promise<any[]> => (isMock ? Promise.resolve([]) : req('GET', '/audit')),
  getSharedQueue: (): Promise<any[]> =>
    isMock ? Promise.resolve([]) : req('GET', '/shared/queue'),
  decrypt: async (file: File): Promise<string[]> => {
    if (isMock) {
      return ['https://example.com/file1.zip', 'https://example.com/file2.zip'];
    }
    const formData = new FormData();
    formData.append('file', file);
    const r = await fetch(`${BASE}/decrypt`, {
      method: 'POST',
      body: formData,
    });
    if (!r.ok) {
      const text = await r.text().catch(() => r.statusText);
      throw new Error(text || `HTTP ${r.status}`);
    }
    return r.json();
  },
  openBrowserSetup: async (): Promise<void> => {
    try {
      await invoke('open_browser_setup');
    } catch (err) {
      window.open('http://127.0.0.1:6277/setup', '_blank');
    }
  },
};

// Removed connectSSE per new Tauri event architecture

// Formatting helpers
export function fmtBytes(b: number | null | undefined): string {
  if (b == null) return '?';
  const u = ['B', 'KB', 'MB', 'GB', 'TB'];
  let v = b,
    i = 0;
  while (v >= 1024 && i < u.length - 1) {
    v /= 1024;
    i++;
  }
  return i === 0 ? `${v.toFixed(0)} ${u[i]}` : `${v.toFixed(1)} ${u[i]}`;
}

export function fmtSpeed(bps: number | null | undefined): string {
  if (bps == null) return '';
  return `${fmtBytes(bps)}/s`;
}

export function fmtEta(secs: number | null | undefined): string {
  if (!secs) return '';
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
  return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`;
}

export function fileExt(name: string | null | undefined): string {
  const m = name?.match(/\.([a-z0-9]{1,8})$/i);
  return m ? m[1].toUpperCase() : 'FILE';
}
