## 1. Backend Implementation (`vajra-daemon`)

- [ ] 1.1 Add `scraper` and `reqwest` (if not already present with appropriate features) to `vajra-daemon/Cargo.toml`.
- [ ] 1.2 Create `vajra-daemon/src/spider.rs` module. Implement a function to fetch a URL, parse HTML using `scraper`, and return a list of extracted absolute URLs.
- [ ] 1.3 Add filtering logic (by extension and regex) to the spider extraction.
- [ ] 1.4 Add `POST /api/v1/spider` to `vajra-daemon/src/api/router.rs` to expose the spider functionality.

## 2. Frontend Implementation (`vajra-ui-tauri`)

- [ ] 2.1 Create `SpiderDialog.tsx` modal or window component.
- [ ] 2.2 Add an entry point to open the Spider modal (e.g., Toolbar button or Context Menu).
- [ ] 2.3 Implement the URL input bar and advanced filter options (Regex, Extension checkboxes).
- [ ] 2.4 Build the parsed links tree/list view with checkboxes for mass-selection.
- [ ] 2.5 Implement the "Download Selected" button that pushes batch tasks to `/api/v1/downloads`.

## 3. Integration & Polish

- [ ] 3.1 Verify batch downloads are enqueued correctly without locking up the daemon.
- [ ] 3.2 Ensure the `SpiderDialog.tsx` matches the dark glassmorphism aesthetic.
