## Context

Vajra currently processes individual downloads or sniffed media streams. A highly requested feature (similar to IDM's Site Grabber) is the ability to point the application to a base URL, parse the HTML for links (media, documents, binaries), filter them, and queue them for batch downloading. This requires an HTML parser in the backend and a new UI view in the frontend to manage the spidered links.

## Goals / Non-Goals

**Goals:**
- Implement an HTML parsing spider in `vajra-daemon` using `reqwest` and `scraper` (or similar HTML parsing library).
- Create a `POST /api/v1/spider` endpoint to trigger a site scan and return a list of discovered URLs.
- Build a React component (`Spider.jsx`) that lets the user input a URL, view discovered links in a list/tree, filter them via Regex/Extensions, and select specific items to download.
- Ensure batch enqueuing does not block the main engine loop.

**Non-Goals:**
- Full recursive crawling of entire domains (depth > 1 or 2 is out of scope for the initial version to prevent infinite loops and memory exhaustion).
- Javascript rendering for dynamically loaded content (initial version will rely on static HTML parsing).

## Decisions

- **HTML Parsing**: Use the `scraper` crate in Rust for parsing HTML and extracting `href` and `src` attributes. It is robust and uses CSS selectors.
- **Frontend State**: The `Spider.jsx` component will keep the state of discovered links. The backend will return a flat or hierarchical list of links, and the frontend will handle filtering (e.g., regex filtering for `.mp4`, `.pdf`, etc.).
- **Batch Enqueuing**: When the user clicks "Download Selected", the frontend will loop over selected items and `POST /api/v1/downloads` for each, or we introduce a `POST /api/v1/downloads/batch` endpoint. To keep things simple initially, we can just call the single download endpoint concurrently or sequentially.

## Risks / Trade-offs

- **Risk: Spidering dynamic SPAs**: Sites that require JavaScript to render links will not be spiderable with a simple `reqwest` + `scraper` setup.
  - *Mitigation*: Clearly document that this is a static site grabber. (Future versions could integrate a headless browser, but that's out of scope for Phase 7).
- **Risk: Getting blocked/rate-limited**:
  - *Mitigation*: Ensure `reqwest` sets a standard User-Agent, and maybe introduce a small delay between requests if crawling multiple pages.
