// Vajra — Content Script (Phase 6D enhanced)
// Detects <video> and <audio> elements and overlays a "Download with Vajra" button.
// Supports direct HTTP downloads and yt-dlp streaming URLs.

(function () {
  'use strict';

  // Bail out on non-interceptable pages
  const { href } = location;
  if (!href || href.startsWith('about:') || href.startsWith('chrome:') ||
      href.startsWith('chrome-extension:') || href.startsWith('moz-extension:') ||
      href.startsWith('data:') || href.startsWith('blob:')) {
    return;
  }

  // Tell the setup page that the extension is active
  if (href.startsWith('http://127.0.0.1:6277/setup') || href.startsWith('http://localhost:6277/setup')) {
    document.documentElement.setAttribute('data-vajra-ext', '1');
  }

  const BUTTON_CLASS = 'vajra-dl-btn';
  const INJECTED_ATTR = 'data-vajra-injected';

  // ── Streaming URL heuristic ──────────────────────────────────────────────────
  // Returns true if the URL looks like a streaming manifest or known streaming site
  function isStreamingSrc(url) {
    try {
      const u = new URL(url);
      const p = u.pathname.toLowerCase();
      if (p.endsWith('.m3u8') || p.endsWith('.mpd') || p.endsWith('.ts')) return true;
      const h = u.hostname;
      return h.includes('youtube.com') || h.includes('youtu.be') ||
             h.includes('vimeo.com')   || h.includes('twitch.tv') ||
             h.includes('dailymotion') || h.includes('bilibili');
    } catch { return false; }
  }

  // ── Styles ──────────────────────────────────────────────────────────────────

  const style = document.createElement('style');
  style.textContent = `
    .${BUTTON_CLASS} {
      position: absolute;
      top: 8px;
      right: 8px;
      z-index: 2147483647;
      background: linear-gradient(135deg, #F0B429, #FF6C35);
      color: #fff;
      border: none;
      border-radius: 8px;
      padding: 6px 12px;
      font-size: 12px;
      font-weight: 600;
      cursor: pointer;
      font-family: 'Segoe UI', system-ui, sans-serif;
      box-shadow: 0 2px 8px rgba(0,0,0,0.4);
      transition: opacity .15s, transform .1s;
      pointer-events: auto;
    }
    .${BUTTON_CLASS}:hover  { opacity: 0.9; transform: scale(1.03); }
    .${BUTTON_CLASS}.sent   { background: #22c55e; }
    .${BUTTON_CLASS}.failed { background: #ef4444; }
    .${BUTTON_CLASS}.ytdlp { background: linear-gradient(135deg, #9333ea, #6366f1); }
    .vajra-batch-cb {
      margin-right: 6px;
      vertical-align: middle;
      width: 16px;
      height: 16px;
      cursor: pointer;
      z-index: 2147483646;
      flex-shrink: 0;
    }
  `;
  (document.head || document.documentElement).appendChild(style);

  // ── Button injection ─────────────────────────────────────────────────────────

  function addVajraButton(mediaEl) {
    // Check if src changed since last injection — reset guard if so
    const prevSrc = mediaEl.getAttribute(INJECTED_ATTR);
    const currentSrc = mediaEl.currentSrc || mediaEl.src || '';
    if (!currentSrc || currentSrc.startsWith('blob:') || currentSrc.startsWith('data:')) return;
    if (prevSrc === currentSrc) return; // already injected for this src

    mediaEl.setAttribute(INJECTED_ATTR, currentSrc);

    const parent = mediaEl.parentElement;
    if (!parent) return;
    const parentPos = getComputedStyle(parent).position;
    if (parentPos === 'static') parent.style.position = 'relative';

    // Remove old button if src changed
    parent.querySelector(`.${BUTTON_CLASS}`)?.remove();

    const streaming = isStreamingSrc(currentSrc);
    const btn = document.createElement('button');
    btn.className   = `${BUTTON_CLASS}${streaming ? ' ytdlp' : ''}`;
    btn.textContent = streaming ? '⚡ Stream Grab' : '⚡ Download';
    btn.title       = streaming
      ? `Grab stream with yt-dlp: ${currentSrc}`
      : `Download with Vajra: ${decodeURIComponent(currentSrc.split('/').pop().split('?')[0])}`;

    btn.addEventListener('click', async (e) => {
      e.preventDefault();
      e.stopPropagation();

      const src = mediaEl.currentSrc || mediaEl.src;
      btn.textContent = '…';

      // Send to background (which has access to cookies etc.)
      chrome.runtime.sendMessage({
        type:      'add_download',
        url:       src,
        filename:  src.split('/').pop().split('?')[0] || 'media',
        referrer:  location.href,
        use_ytdlp: isStreamingSrc(src),
      }).then((resp) => {
        if (resp && resp.ok) {
          btn.textContent = '✓ Added!';
          btn.classList.add('sent');
        } else {
          btn.textContent = '✗ Failed';
          btn.classList.add('failed');
        }
        setTimeout(() => {
          btn.textContent = streaming ? '⚡ Stream Grab' : '⚡ Download';
          btn.classList.remove('sent', 'failed');
        }, 2500);
      }).catch(() => {
        btn.textContent = '✗ Failed';
        btn.classList.add('failed');
        setTimeout(() => {
          btn.textContent = streaming ? '⚡ Stream Grab' : '⚡ Download';
          btn.classList.remove('sent', 'failed');
        }, 2500);
      });
    });

    parent.appendChild(btn);
  }

  // ── Scan ──────────────────────────────────────────────────────────────────────

  function scanMedia() {
    document.querySelectorAll('video, audio').forEach(addVajraButton);
  }

  // ── Listen for background-triggered scan ──────────────────────────────────────

  chrome.runtime.onMessage.addListener((msg) => {
    if (msg.type === 'scan_media') scanMedia();
    if (msg.type === 'media_stream_detected') {
      injectGlobalStreamButton(msg.url, msg.title);
    }
  });

  function injectGlobalStreamButton(url, title) {
    if (document.getElementById('vajra-global-stream-btn')) return;
    
    const btn = document.createElement('button');
    btn.id = 'vajra-global-stream-btn';
    btn.className = `${BUTTON_CLASS} ytdlp`;
    btn.textContent = '⚡ Sniffed Stream: Grab';
    btn.title = `Grab sniffed stream: ${url}`;
    
    // Position it fixed at bottom right
    Object.assign(btn.style, {
      position: 'fixed',
      bottom: '20px',
      right: '20px',
      top: 'auto',
      zIndex: '2147483647',
      padding: '10px 16px',
      fontSize: '14px',
      boxShadow: '0 4px 12px rgba(0,0,0,0.5)'
    });

    btn.addEventListener('click', async (e) => {
      e.preventDefault();
      e.stopPropagation();
      btn.textContent = '…';
      chrome.runtime.sendMessage({
        type: 'add_download',
        url: url,
        filename: title + '.mp4',
        referrer: location.href,
        use_ytdlp: true,
      }).then((resp) => {
        if (resp && resp.ok) {
          btn.textContent = '✓ Added!';
          btn.classList.add('sent');
          setTimeout(() => btn.remove(), 2000);
        } else {
          btn.textContent = '✗ Failed';
          btn.classList.add('failed');
          setTimeout(() => { btn.textContent = '⚡ Sniffed Stream: Grab'; btn.classList.remove('failed'); }, 2500);
        }
      }).catch(() => {
        btn.textContent = '✗ Failed';
        btn.classList.add('failed');
        setTimeout(() => { btn.textContent = '⚡ Sniffed Stream: Grab'; btn.classList.remove('failed'); }, 2500);
      });
    });
    
    // Add close button
    const closeBtn = document.createElement('span');
    closeBtn.textContent = ' ×';
    closeBtn.style.marginLeft = '8px';
    closeBtn.style.opacity = '0.7';
    closeBtn.onclick = (e) => { e.stopPropagation(); btn.remove(); };
    btn.appendChild(closeBtn);
    
    (document.body || document.documentElement).appendChild(btn);
  }

  // ── Observe DOM mutations ─────────────────────────────────────────────────────

  const observer = new MutationObserver((mutations) => {
    let needScan = false;
    for (const m of mutations) {
      if (m.type === 'childList') { needScan = true; break; }
      const target = m.target as Element;
      if (m.type === 'attributes' &&
          (target.tagName === 'VIDEO' || target.tagName === 'AUDIO')) {
        addVajraButton(target);
      }
    }
    if (needScan) {
      if (typeof requestIdleCallback === 'function') {
        requestIdleCallback(scanMedia, { timeout: 1000 });
      } else {
        scanMedia();
      }
    }
  });

  function startObserving() {
    if (!document.body) { setTimeout(startObserving, 100); return; }
    scanMedia();
    observer.observe(document.body, {
      childList: true, subtree: true,
      attributes: true, attributeFilter: ['src', 'currentSrc'],
    });
  }

  // ── Keyboard Modifier Listeners ──────────────────────────────────────────────
  let lastHoldingKey = null;
  let batchSelectionActive = false;
  let selectedUrls = new Set();
  
  function getFilenameFromUrl(url) {
    try {
      const parts = new URL(url).pathname.split('/').filter(Boolean);
      return decodeURIComponent(parts.at(-1) || 'download').split('?')[0] || 'download';
    } catch { return 'download'; }
  }

  function updateBatchUI() {
    let container = document.getElementById('vajra-batch-container');
    if (selectedUrls.size === 0) {
      if (container) container.remove();
      return;
    }
    
    if (!container) {
      container = document.createElement('div');
      container.id = 'vajra-batch-container';
      Object.assign(container.style, {
        position: 'fixed',
        bottom: '20px',
        left: '50%',
        transform: 'translateX(-50%)',
        zIndex: '2147483647',
        background: '#1f2937',
        color: '#fff',
        padding: '12px 20px',
        borderRadius: '12px',
        boxShadow: '0 4px 16px rgba(0,0,0,0.5)',
        display: 'flex',
        alignItems: 'center',
        gap: '16px',
        fontFamily: 'system-ui, sans-serif'
      });
      
      const countLabel = document.createElement('span');
      countLabel.id = 'vajra-batch-count';
      countLabel.style.fontWeight = 'bold';
      
      const btn = document.createElement('button');
      btn.className = BUTTON_CLASS;
      btn.style.position = 'static';
      btn.textContent = '⚡ Batch Download';
      btn.onclick = () => {
        const urls = Array.from(selectedUrls);
        btn.textContent = '…';
        chrome.runtime.sendMessage({
          type: 'batch_add_download',
          items: urls.map(url => ({
            url,
            filename: getFilenameFromUrl(url),
            referrer: location.href,
            use_ytdlp: false
          }))
        }).then((resp) => {
          if (resp && resp.ok) {
            btn.textContent = '✓ Added!';
            btn.classList.add('sent');
            setTimeout(() => {
              selectedUrls.clear();
              removeBatchCheckboxes();
              updateBatchUI();
            }, 1000);
          } else {
            btn.textContent = '✗ Failed';
            btn.classList.add('failed');
            setTimeout(() => { btn.textContent = '⚡ Batch Download'; btn.classList.remove('failed'); }, 2000);
          }
        }).catch(() => {
          btn.textContent = '✗ Failed';
          btn.classList.add('failed');
          setTimeout(() => { btn.textContent = '⚡ Batch Download'; btn.classList.remove('failed'); }, 2000);
        });
      };
      
      const closeBtn = document.createElement('button');
      closeBtn.textContent = '×';
      Object.assign(closeBtn.style, {
        background: 'transparent', border: 'none', color: '#9ca3af', fontSize: '20px', cursor: 'pointer', padding: '0 4px'
      });
      closeBtn.onclick = () => {
        selectedUrls.clear();
        removeBatchCheckboxes();
        updateBatchUI();
      };
      
      container.appendChild(countLabel);
      container.appendChild(btn);
      container.appendChild(closeBtn);
      (document.body || document.documentElement).appendChild(container);
    }
    
    document.getElementById('vajra-batch-count').textContent = `${selectedUrls.size} selected`;
  }

  function injectBatchCheckboxes() {
    if (batchSelectionActive) return;
    batchSelectionActive = true;
    
    document.querySelectorAll('a[href]').forEach(a => {
      const href = (a as HTMLAnchorElement).href;
      if (!href || href.startsWith('javascript:') || href.startsWith('#')) return;
      
      // Don't inject if already has one
      if (a.previousElementSibling?.classList.contains('vajra-batch-cb')) return;
      
      const cb = document.createElement('input');
      cb.type = 'checkbox';
      cb.className = 'vajra-batch-cb';
      cb.dataset.url = href;
      // Styles applied via .vajra-batch-cb CSS class (injected at content script init)
      
      if (selectedUrls.has(href)) cb.checked = true;
      
      cb.onchange = (e) => {
        e.stopPropagation();
        if (cb.checked) selectedUrls.add(href);
        else selectedUrls.delete(href);
        updateBatchUI();
      };
      
      cb.onclick = (e) => e.stopPropagation();
      
      a.parentNode.insertBefore(cb, a);
    });
  }

  function removeBatchCheckboxes() {
    batchSelectionActive = false;
    document.querySelectorAll('.vajra-batch-cb').forEach(cb => cb.remove());
  }

  function updateHoldingKey(key) {
    if (lastHoldingKey === key) return;
    lastHoldingKey = key;
    chrome.runtime.sendMessage({ type: 'update_holding_key', key }).catch(() => {});
    
    if (key === 'Alt') {
      injectBatchCheckboxes();
    } else if (selectedUrls.size === 0) {
      removeBatchCheckboxes();
    }
  }

  window.addEventListener('keydown', (e) => {
    if (e.key === 'Alt') {
      updateHoldingKey('Alt');
    } else if (e.key === 'Control') {
      updateHoldingKey('Control');
    }
  }, true);

  window.addEventListener('keyup', (e) => {
    if (e.key === 'Alt' || e.key === 'Control') {
      if (e.altKey) {
        updateHoldingKey('Alt');
      } else if (e.ctrlKey) {
        updateHoldingKey('Control');
      } else {
        updateHoldingKey(null);
      }
    }
  }, true);

  window.addEventListener('blur', () => {
    updateHoldingKey(null);
  });

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', startObserving);
  } else {
    startObserving();
  }

})();
