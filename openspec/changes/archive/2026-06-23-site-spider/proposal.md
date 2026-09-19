## Why

Users need a way to batch download entire websites, extract links, filter by regex, and select multiple items at once (similar to IDM's Site Grabber). Currently, Vajra only handles single URLs or sniffed media.

## What Changes

- Add `Spider.jsx` component for the frontend to manage a tree/list of discovered links and regex filters.
- Add `spider.rs` module in `vajra-daemon` to scrape websites, fetch HTML, and parse media, document, and binary links.
- Expose `POST /api/v1/spider` in `vajra-daemon` to trigger Spider processes.
- Implement link selection, filtering (by filetype/regex), and batch task spawning.

## Capabilities

### New Capabilities
- `site-spider`: Site Spider / Grabber functionality to scan a base URL, extract downloadable links, filter by user rules, and push them to the core engine.

### Modified Capabilities
- `vajra-daemon`: Adding `spider.rs` and the `POST /api/v1/spider` router handler.

## Impact

- Adds `Spider.jsx` window/modal in the frontend.
- `vajra-daemon/src/api/router.rs` receives a new endpoint.
- `vajra-daemon/src/spider.rs` becomes a new module requiring HTML parsing capabilities (e.g., `scraper` or regex based).
