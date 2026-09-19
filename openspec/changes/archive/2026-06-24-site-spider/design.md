## Context

Phase 7 of the Vajra Roadmap specifies implementing a Site Spider / Grabber that fetches and parses HTML to extract links, enabling users to mass-select and download files from an index page or website.

## Goals / Non-Goals

**Goals:**
- Provide a robust HTML parsing mechanism in `vajra-daemon` to recursively fetch pages and extract all links.
- Create a performant backend endpoint to stream links back to the frontend.
- Implement a user-friendly UI in `vajra-ui-tauri` for selecting links and adding them in batch to the download queue.

**Non-Goals:**
- Deep, exhaustive web crawling (like a search engine spider) - depth should be configurable and limited to prevent infinite loops.
- Executing JavaScript to render SPAs (we will only parse static HTML for simplicity and performance).

## Decisions

- **HTML Parsing Library:** We will use `scraper` (or similar Rust library like `nipper`) to parse HTML documents and use CSS selectors (e.g. `a[href], img[src]`) to extract URLs.
- **Link Resolution:** `reqwest::Url` will be used to resolve relative URLs against the base URL.
- **API Response Format:** The `/api/v1/spider` endpoint will return a JSON list of discovered links. If crawling takes too long, we may consider Server-Sent Events (SSE), but for simple depth 1 or 2 crawls, a JSON response is sufficient and simpler to implement.
- **UI Component:** We will build `SpiderDialog.tsx` as a modal or secondary window, containing a URL input, depth configuration, and a table/tree of discovered files.
- **Batch Add:** Once the user selects the files, the UI will iterate and call `/api/v1/downloads` (the add endpoint) for each URL.

## Risks / Trade-offs

- **Risk:** Infinite loops or crawling massive websites can OOM or block the daemon.
  - *Mitigation:* Implement a strict recursion depth limit (default 1) and restrict crawling to the same domain.
- **Risk:** Parsing large HTML documents on the main async thread.
  - *Mitigation:* Spawn parsing tasks on a blocking threadpool using `tokio::task::spawn_blocking` to avoid stalling the HTTP server.
