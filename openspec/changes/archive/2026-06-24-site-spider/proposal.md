## Why

Users often need to download multiple files from a single website (e.g. all images, PDFs, or software releases on an index page). Currently, they have to manually add each link or rely on external tools. The Site Spider (Phase 7) solves this by providing an integrated crawler to fetch and filter links recursively from a base URL, allowing mass-selection and batch downloading directly within Vajra.

## What Changes

- Add a new `spider.rs` module in `vajra-daemon` to recursively crawl pages, extract links, map relative paths to absolute URLs, and filter by extension or regex.
- Expose a new API endpoint (e.g., `POST /api/v1/spider`) that yields links either as an SSE stream or JSON list.
- Add a new `scraper` or `nipper` dependency to `vajra-daemon/Cargo.toml` for fast HTML parsing.
- Implement a `Spider.jsx` window component in `vajra-ui-tauri` with URL input, regex/extension filters, and a tree/list view for mass selection.
- Implement UI integration to batch-add selected files from the Spider to the download queue.

## Capabilities

### New Capabilities
- `site-spider`: Crawler to fetch, parse, and filter links from web pages, providing batch download capabilities.

### Modified Capabilities

## Impact

- `vajra-daemon/Cargo.toml` (new dependencies)
- `vajra-daemon/src/spider.rs` (new module)
- `vajra-daemon/src/api/router.rs` (new routes)
- `vajra-ui-tauri` (new window `Spider.tsx`, integration in existing components like Toolbar/ContextMenu)
