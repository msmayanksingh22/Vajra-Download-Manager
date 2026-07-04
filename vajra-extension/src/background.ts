// Vajra Download Manager — Background Service Worker
// Talks directly to vajrad REST API on 127.0.0.1:6277.
// No native messaging required — the daemon IS the bridge.

'use strict';

const DAEMON  = 'http://127.0.0.1:6277';
const API     = `${DAEMON}/api/v1`;
const POLL_MS = 4000; // daemon health-check interval

const INTERCEPT_EXTENSIONS = new Set([
  'zip','rar','7z','tar','gz','bz2','xz','zst',
  'exe','msi','msix','dmg','pkg','deb','rpm','apk',
  'iso','img','bin','dat',
  'mp4','mkv','avi','mov','wmv','flv','webm','m4v',
  'mp3','flac','wav','aac','ogg','m4a',
  'pdf','doc','docx','xls','xlsx','ppt','pptx',
]);

const MEDIA_STREAM_EXTENSIONS = new Set(['m3u8', 'mpd']);

// ── State ─────────────────────────────────────────────────────────────────────

let daemonAlive = false;
let settings    = { interceptAll: true, minSizeMB: 0, vajra_enabled: true, defaultSavePath: '' };
const requestCache = new Map();
// `interceptedDownloads` is persisted to chrome.storage.session so it survives
// MV3 service-worker restarts (BUG-10). Without this, after a SW restart
// Chrome re-fires onCreated for in-flight downloads, causing duplicate intercepts.
const interceptedDownloads = new Set<number>();
let currentHoldingKey = null;

// Restore session-persisted intercept IDs on SW startup.
chrome.storage.session.get({ vajra_intercepted: [] }, (data) => {
  const ids: number[] = (data.vajra_intercepted as number[]) || [];
  for (const id of ids) interceptedDownloads.add(id);
});

function persistInterceptedDownloads() {
  chrome.storage.session.set({ vajra_intercepted: [...interceptedDownloads] });
}

chrome.storage.local.get({
  interceptAll: true,
  minSizeMB: 0,
  vajra_enabled: true,
  vajra_save_path: ''
}, (s) => {
  settings = {
    interceptAll:    s.interceptAll !== false,
    minSizeMB:       (s.minSizeMB as number) ?? 0,
    vajra_enabled:   s.vajra_enabled !== false,
    defaultSavePath: (s.vajra_save_path as string) || ''
  };
  console.log('[Vajra] Loaded settings:', settings);
});
chrome.storage.onChanged.addListener((changes, area) => {
  if (area !== 'local' && area !== 'sync') return;
  // Explicit key-by-key assignment — never allow unknown keys to corrupt settings
  if ('interceptAll'  in changes) settings.interceptAll    = changes.interceptAll.newValue !== false;
  if ('minSizeMB'     in changes) settings.minSizeMB       = (changes.minSizeMB.newValue as number) ?? 0;
  if ('vajra_enabled' in changes) settings.vajra_enabled   = changes.vajra_enabled.newValue !== false;
  if ('vajra_save_path' in changes) settings.defaultSavePath = (changes.vajra_save_path.newValue as string) || '';
});

// ── Daemon health-check ───────────────────────────────────────────────────────

async function pingDaemon() {
  try {
    const r = await fetch(`${DAEMON}/health`, { signal: AbortSignal.timeout(2000) });
    daemonAlive = r.ok;
  } catch {
    daemonAlive = false;
  }
  updateBadge();
}

function updateBadge() {
  if (daemonAlive) {
    chrome.action.setBadgeText({ text: '' });
  } else {
    chrome.action.setBadgeText({ text: '!' });
    chrome.action.setBadgeBackgroundColor({ color: '#F85149' });
  }
}

// ── Auto-start Vajra via OS protocol handler ─────────────────────────────────
// Opens vajra://start which triggers the registered Windows URL protocol handler.
// Returns true if daemon comes up within 10 seconds.
async function tryAutoStart() {
  // Silent start via Native Messaging Host
  chrome.runtime.sendNativeMessage('com.vajra.manager', { cmd: 'start' }, () => {
    // Ignore errors. If the host isn't registered, the fallback polling will time out.
    if (chrome.runtime.lastError) {
       console.warn('[Vajra] Native Messaging start failed:', chrome.runtime.lastError.message);
    }
  });
  // Poll up to 10 seconds for daemon to respond
  for (let i = 0; i < 20; i++) {
    await new Promise(r => setTimeout(r, 500));
    try {
      const r = await fetch(`${DAEMON}/health`, { signal: AbortSignal.timeout(1000) });
      if (r.ok) { daemonAlive = true; updateBadge(); return true; }
    } catch {}
  }
  return false;
}

// Poll every 4s
pingDaemon();
setInterval(pingDaemon, POLL_MS);

// ── Add download to daemon ────────────────────────────────────────────────────

async function addToDaemon(url, filename, referrer, cookieHeader, useYtdlp = false) {
  const headers = {};
  if (cookieHeader) headers['Cookie']  = cookieHeader;
  if (referrer)    headers['Referer'] = referrer;

  const body = {
    url,
    filename:        filename || null,
    headers,
    output_dir:      settings.defaultSavePath || null,
    expected_hash:   null,
    max_connections: useYtdlp ? 1 : null,
    speed_limit_bps: null,
    priority:        'normal',
    schedule_at:     null,
    use_ytdlp:       useYtdlp,
  };

  const r = await fetch(`${API}/intercept`, {
    method:  'POST',
    headers: { 'Content-Type': 'application/json' },
    body:    JSON.stringify(body),
    signal:  AbortSignal.timeout(5000),
  });
  return r.ok;
}

// Helper to parse filename from Content-Disposition header
function getFilenameFromContentDisposition(disposition) {
  if (!disposition) return null;
  
  // Try filename*=UTF-8''filename.ext
  const utf8Match = disposition.match(/filename\*=\s*UTF-8''([^;'\n]*)/i);
  if (utf8Match && utf8Match[1]) {
    try {
      return decodeURIComponent(utf8Match[1]);
    } catch {
      // ignore decoding error and try fallback
    }
  }
  
  // Try filename="filename.ext" or filename=filename.ext
  const normalMatch = disposition.match(/filename\s*=\s*["']?([^;"'\n]+)["']?/i);
  if (normalMatch && normalMatch[1]) {
    return normalMatch[1].trim();
  }
  
  return null;
}

// Keep track of recent network requests to capture their headers
chrome.webRequest.onHeadersReceived.addListener(
  (details: any) => {
    if (!settings.vajra_enabled) return {};
    if (details.method !== 'GET' && details.method !== 'POST') return {};
    
    const headers = details.responseHeaders || [];
    const responseHeadersMap = {};
    for (const h of headers) {
      if (h.name && h.value) {
        responseHeadersMap[h.name.toLowerCase()] = h.value;
      }
    }
    
    const contentType = responseHeadersMap['content-type'] || '';
    const contentDisposition = responseHeadersMap['content-disposition'] || '';
    const contentLength = responseHeadersMap['content-length'] ? parseInt(responseHeadersMap['content-length'], 10) : null;
    
    const isAttachment = contentDisposition.toLowerCase().includes('attachment');
    
    const url = details.url;
    const urlFilename = getFilenameFromUrl(url);
    const urlExt = urlFilename.split('.').pop()?.toLowerCase() ?? '';
    
    const hasDownloadMime = contentType.startsWith('application/') && 
      !['application/x-javascript', 'application/javascript', 'application/json', 'application/xml', 'application/pdf', 'application/x-shockwave-flash'].some(mime => contentType.includes(mime));
      
    const isInterceptable = isAttachment || INTERCEPT_EXTENSIONS.has(urlExt) || hasDownloadMime;
    const isMediaStream = MEDIA_STREAM_EXTENSIONS.has(urlExt) || contentType.includes('application/x-mpegurl') || contentType.includes('application/dash+xml');
    
    if (isMediaStream) {
      // BUG-22: Use details.tabId to target the EXACT tab that triggered the
      // request, not the currently-active tab. chrome.tabs.query({active:true})
      // returns the wrong tab for background tabs, prefetched pages, and iframes.
      if (details.tabId && details.tabId !== -1) {
        chrome.tabs.sendMessage(details.tabId, {
          type: 'media_stream_detected',
          url: details.url,
          contentType,
          title: '', // content script will use document.title
        }).catch(() => {}); // ignore errors if content script not loaded
      }
    }

    if (isInterceptable) {
      const filename = getFilenameFromContentDisposition(contentDisposition) || urlFilename;
      requestCache.set(url, {
        url,
        filename,
        contentType,
        contentLength,
        contentDisposition,
        referrer: details.initiator || '',
        timestamp: Date.now()
      });
      
      // Clean up old entries to prevent memory leaks
      if (requestCache.size > 200) {
        const now = Date.now();
        for (const [k, v] of requestCache.entries()) {
          if (now - v.timestamp > 60000) { // 1 minute expiry
            requestCache.delete(k);
          }
        }
      }
    }
  },
  { urls: ["<all_urls>"] },
  ["responseHeaders"]
);

function shouldInterceptDownload(item) {
  if (currentHoldingKey === 'Alt') {
    console.log('[Vajra] ALT key held: bypassing interception.');
    return false;
  }
  if (currentHoldingKey === 'Control') {
    console.log('[Vajra] CTRL key held: forcing interception.');
    return true;
  }

  if (!settings.vajra_enabled) return false;
  if (item.state === 'complete' || item.state === 'interrupted' || item.error) return false;
  if (!isInterceptableUrl(item.url)) return false;
  
  // Look up in requestCache
  const cached = requestCache.get(item.url) || requestCache.get(item.finalUrl);
  
  if (cached) {
    const filename = cached.filename || item.filename || getFilenameFromUrl(item.url);
    const ext = filename.split('.').pop()?.toLowerCase() ?? '';
    
    const size = cached.contentLength !== null ? cached.contentLength : item.fileSize;
    
    return settings.interceptAll ||
      INTERCEPT_EXTENSIONS.has(ext) ||
      (size <= 0) || // unknown size
      (size > settings.minSizeMB * 1024 * 1024);
  }
  
  // Fallback to standard check
  const filename = item.filename || getFilenameFromUrl(item.url);
  const ext = filename.split('.').pop()?.toLowerCase() ?? '';
  return settings.interceptAll ||
    INTERCEPT_EXTENSIONS.has(ext) ||
    (item.fileSize <= 0) ||
    (item.fileSize > settings.minSizeMB * 1024 * 1024);
}

async function handleIntercept(item) {
  if (interceptedDownloads.has(item.id)) return;
  interceptedDownloads.add(item.id);
  persistInterceptedDownloads();

  // Clean up set if it grows too large (LRU-style: remove oldest by insertion order)
  if (interceptedDownloads.size > 100) {
    const it = interceptedDownloads.values();
    for (let i = 0; i < 50; i++) {
      interceptedDownloads.delete(it.next().value);
    }
    persistInterceptedDownloads();
  }

  console.log(`[Vajra] Intercepting download ID ${item.id}: ${item.url}`);
  
  // Cancel and erase immediately
  try {
    await chrome.downloads.cancel(item.id);
    await chrome.downloads.erase({ id: item.id });
    console.log(`[Vajra] Successfully canceled/erased browser download.`);
  } catch (err) {
    console.error(`[Vajra] Failed to cancel browser download:`, err);
  }
  
  const url = item.url;
  const cached = requestCache.get(url) || requestCache.get(item.finalUrl);
  const filename = (cached?.filename) || item.filename || getFilenameFromUrl(url);
  
  // Auto-start daemon if down
  if (!daemonAlive) {
    await tryAutoStart();
  }
  
  const cookieHeader = await collectCookies(url);
  const ok = await addToDaemon(url, filename, item.referrer || cached?.referrer || '', cookieHeader)
    .catch(() => false);
    
  if (!ok) {
    chrome.notifications.create(`vajra-err-${Date.now()}`, {
      type: 'basic', iconUrl: 'logo.png',
      title: 'Vajra Download Manager',
      message: `❌ Failed to send to Vajra: ${filename}`,
    });
  }
}

// ── Download Interception ─────────────────────────────────────────────────────

chrome.downloads.onCreated.addListener(async (item) => {
  console.log('[Vajra] onCreated fired:', item);
  if (shouldInterceptDownload(item)) {
    await handleIntercept(item);
  }
});

// We also use onDeterminingFilename to reliably cancel the "Save As" dialog
chrome.downloads.onDeterminingFilename.addListener((item, suggest) => {
  if (shouldInterceptDownload(item)) {
    console.log(`[Vajra] onDeterminingFilename: cleanly canceling ${item.filename}`);
    
    // Trigger interception
    handleIntercept(item);
    
    // Call suggest immediately to resolve the callback and prevent the native Save As dialog
    suggest({ filename: 'vajra_dummy_filename' });
    return;
  }
  suggest();
});

// ── Context Menu ──────────────────────────────────────────────────────────────

chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.create({
    id: 'vajra-link', title: '⚡ Download with Vajra',
    contexts: ['link', 'image', 'video', 'audio'],
  });
  pingDaemon();
});

chrome.contextMenus.onClicked.addListener(async (info) => {
  const url = info.linkUrl || info.srcUrl;
  if (!url || !isInterceptableUrl(url)) return;

  // If daemon is down, attempt to auto-start it via vajra:// protocol
  if (!daemonAlive) {
    chrome.notifications.create(`vajra-starting-${Date.now()}`, {
      type: 'basic', iconUrl: 'logo.png',
      title: 'Vajra — Starting…',
      message: 'Launching Vajra automatically, please wait…',
    });
    const started = await tryAutoStart();
    if (!started) {
      chrome.notifications.create(`vajra-fail-${Date.now()}`, {
        type: 'basic', iconUrl: 'logo.png',
        title: 'Vajra — Could Not Start',
        message: 'Please launch Vajra manually and try again.',
      });
      return;
    }
  }

  const ok = await addToDaemon(url, getFilenameFromUrl(url), info.pageUrl ?? '', '')
    .catch(() => false);

  chrome.notifications.create(`vajra-ctx-${Date.now()}`, {
    type: 'basic', iconUrl: 'logo.png',
    title: ok ? 'Vajra Download Manager' : 'Vajra — Failed',
    message: ok ? `⚡ Downloading: ${getFilenameFromUrl(url)}` : 'Could not reach Vajra daemon.',
  });
});

// ── Extension Messaging (from popup) ─────────────────────────────────────────

chrome.runtime.onMessage.addListener((msg, _sender, reply) => {
  switch (msg.type) {
    case 'update_holding_key':
      currentHoldingKey = msg.key;
      reply({ ok: true });
      return true;

    case 'get_status':
      reply({ connected: daemonAlive, settings, daemonUrl: DAEMON });
      return true;

    case 'save_settings': {
      // Explicit merge — never allow unknown keys to corrupt settings
      const s = msg.settings || {};
      if ('interceptAll'    in s) settings.interceptAll    = s.interceptAll !== false;
      if ('minSizeMB'       in s) settings.minSizeMB       = Number(s.minSizeMB) || 0;
      if ('vajra_enabled'   in s) settings.vajra_enabled   = s.vajra_enabled !== false;
      if ('vajra_save_path' in s) settings.defaultSavePath = String(s.vajra_save_path || '');
      chrome.storage.local.set(msg.settings);
      reply({ ok: true });
      return true;
    }

    case 'open_vajra':
      // Launch silently via Native Messaging Host
      chrome.runtime.sendNativeMessage('com.vajra.manager', { cmd: 'open' }, (response) => {
        if (chrome.runtime.lastError) {
          console.warn('[Vajra] Native Messaging open failed:', chrome.runtime.lastError.message);
          // Fallback to vajra:// link if Native Messaging isn't configured yet (e.g. old installer)
          chrome.tabs.create({ url: 'vajra://open' });
        }
      });
      // Also start the daemon if it isn't running
      if (!daemonAlive) {
        tryAutoStart();
      }
      return true;

    case 'add_download':
      if (msg.url) addToDaemon(
        msg.url,
        msg.filename ?? '',
        msg.referrer ?? '',
        '',
        msg.use_ytdlp ?? false,
      ).then(ok => reply({ ok })).catch(() => reply({ ok: false }));
      return true;

    case 'batch_add_download':
      if (msg.items && Array.isArray(msg.items)) {
        Promise.all(msg.items.map(item => 
          addToDaemon(
            item.url,
            item.filename ?? '',
            item.referrer ?? '',
            '',
            item.use_ytdlp ?? false
          )
        )).then(results => {
          reply({ ok: results.every(r => r) });
        }).catch(() => reply({ ok: false }));
      } else {
        reply({ ok: false });
      }
      return true;

    case 'ping_daemon':
      pingDaemon().then(() => reply({ alive: daemonAlive }));
      return true;
  }
  return false;
});

// ── Helpers ───────────────────────────────────────────────────────────────────

function isInterceptableUrl(url) {
  if (!url) return false;
  return !['blob:','data:','about:','chrome:','chrome-extension:','moz-extension:','edge:']
    .some(p => url.startsWith(p));
}

function getFilenameFromUrl(url) {
  try {
    const parts = new URL(url).pathname.split('/').filter(Boolean);
    return decodeURIComponent(parts.at(-1) ?? 'download').split('?')[0] || 'download';
  } catch { return 'download'; }
}

async function collectCookies(url) {
  try {
    const cookies = await chrome.cookies.getAll({ url });
    return cookies.map(c => `${c.name}=${c.value}`).join('; ');
  } catch { return ''; }
}
