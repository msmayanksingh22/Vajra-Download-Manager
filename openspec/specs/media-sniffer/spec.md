# Media Sniffer

## Summary

The Media Sniffer acts as the advanced capture layer that detects streaming media on web pages (HLS, DASH, direct audio/video). It bridges the gap between conventional downloads and embedded streams.

## Architecture

1. **Content Script Injection (`content.ts`)**:
   - Injected automatically into web pages by the Vajra Extension.
   - Constantly observes the DOM for `<video>` and `<audio>` elements.
   - Listens to XHR and Fetch requests using `chrome.webRequest` (handled in `background.ts` which forwards the info to `content.ts` when applicable, or directly detected on the page).

2. **Overlay UI**:
   - Upon detecting media, injects a customizable overlay directly over the media element.
   - Example overlays: "⚡ Download" for direct URLs or "⚡ Stream Grab" for complex streams (like `m3u8`).
   - Ensures the button doesn't block standard player controls.

3. **Stream Handling**:
   - For simple media, it intercepts the `.src` attribute and sends it to the Vajra daemon.
   - For `m3u8`, `mpd`, or obfuscated streaming sites, it flags the request with `use_ytdlp: true` when sending to the Vajra daemon. The daemon then offloads stream fetching to `yt-dlp`.

4. **Integration with Site Spider**:
   - The sniffer provides the initial trigger, but the `vajra-daemon` can also run a Site Spider parser (`spider.rs`) to aggressively extract all linked media assets from a given page, broadcasting them back to the UI via Server-Sent Events (SSE).
