## 1. Backend Implementation

- [x] 1.1 Add `scraper` dependency to `vajra-daemon/Cargo.toml`
- [x] 1.2 Create `vajra-daemon/src/spider.rs` module with core parsing logic
- [x] 1.3 Add `POST /api/v1/spider` to `vajra-daemon/src/api/router.rs`
- [x] 1.4 Link `spider` module in `vajra-daemon/src/main.rs` (if needed)

## 2. Frontend Implementation

- [x] 2.1 Create `Spider.jsx` component in `src/windows/` or similar views folder
- [x] 2.2 Add Spider button to trigger the new UI (maybe on the main dashboard sidebar)
- [x] 2.3 Implement API calls in `Spider.jsx` to `/api/v1/spider`
- [x] 2.4 Implement regex and extension filtering UI within `Spider.jsx`
- [x] 2.5 Implement "Download Selected" button which pushes batch tasks to `/api/v1/downloads`

## 3. Integration & Testing

- [x] 3.1 Test spider parsing on a mock HTML page
- [x] 3.2 Ensure frontend tree/list selection state works correctly
- [x] 3.3 Confirm batch downloads enqueue correctly without blocking
