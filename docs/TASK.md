# Vajra Implementation Tasks
# Source of truth: docs/PLAN.md
---

## Phase 6: Advanced Features & Browser Extension Integration (Completed)

### 1. High-Performance Core Engine Upgrades
- [x] **Memory-Mapped I/O (mmap)**: Introduced zero-copy writing bypassing thread pools with `MmapHandle` mapped pre-allocated files directly into virtual memory.
- [x] **Linux `io_uring`**: Added asynchronous kernel ring-buffer integration compile checks.
- [x] **Stabilized Rolling ETA**: EMA-smoothed historical rolling ETA calculation.

### 2. UI & UX Integrations
- [x] **Dynamic Smart Lists**: Implemented dynamic custom query-based folders in `Sidebar.tsx` and `DownloadsTable.tsx`.
- [x] **Natural Language Parsing**: Added input sentence parsing ("download all pdfs from http...") that redirects to the Site Spider.
- [x] **Right-to-Left (RTL) Layout**: Dynamic direction shift in Options and document wrapper.
- [x] **Tray Menu Controls & Speed Limits**: Enhanced system tray context menu in `lib.rs` with global resume/pause/add actions, and added inline pill-based speed selector controls inside the downloads table right-click context menu.
- [x] **Brand Sizing & Styling**: Polished header brand icon, extension popup, and About dialog modal.

### 3. Bug Fixes & Code Health
- [x] **Type Compatibility**: Added `speed_limit_bps` to the frontend `DownloadInfo` type interface.
- [x] **Vitest Test Selection**: Excluded `tests/e2e/**` from the Vitest test runner.

### 4. Hardening and Bugfixes (Session 7 / Phase 6 Hardening)
- [x] **Production Spawn Permission**: Allowed `"bin/vajrad"` sidecar spawn permission in Tauri's production capabilities JSON.
- [x] **UI/UX Menu Handlers**: Wired missing "Batch Download" and "Help Documentation" actions in MenuBar and App.
- [x] **Zustand Subscriber Race Condition**: Stopped unwanted complete window triggers during completed downloads removal.
- [x] **Sidecar Diagnostics**: Implemented file-based `tauri-shell.log` logging for sidecar processes.
- [x] **Design System CSS Integration**: Converted raw inline layouts and colors to utility classes (`window-chrome-btn`, `search-input`, `menu-bar-btn`, `smart-list-dialog`) in `index.css`.

---

## Phase 7: UI/UX Audit & Systematic Refactor (Completed — 2026-06-28)

### Phase 0 — Audit & Blueprint
- [x] Full codebase audit across all components, dialogs, and CSS.
- [x] Created `UI_UX_SUPREME_PLAN.md` — 5-phase refactor blueprint.

### Phase 1 — Hardcoded Style Scrub
- [x] Removed hardcoded hex colors and pixel values from `App.tsx` and `DownloadsTable.tsx`.
- [x] Replaced all inline JS style hacks with CSS custom property references (`var(--color-*)`, `var(--sp-*)`, etc.).

### Phase 2 — Navigation Chrome Refactor
- [x] Refactored `MenuBar.tsx`: replaced `window.alert/confirm` with proper Tauri event dispatching.
- [x] Refactored `Sidebar.tsx`: unified spacing, hover states, active indicators using design tokens.
- [x] Refactored `Toolbar.tsx`: standardized button classes, removed ad-hoc inline styles.
- [x] All legacy `window.*` dialog calls replaced with proper component-level state.

### Phase 3 — Downloads Table & Empty States
- [x] **Rich empty state**: Redesigned with icon, hierarchy, descriptive copy, and "Add Your First Download" CTA button wired via `vajra:open-add-url` custom DOM event.
- [x] **Resume column logic**: Robust `resume_supported` field added to `DownloadInfo` type; column shows only for eligible statuses.
- [x] **Error indicators**: Replaced raw emoji with `<AlertCircle>` SVG icon from `lucide-react`.
- [x] **Reset Columns**: Added "Reset Columns" option to the column visibility context menu.
- [x] `App.tsx` wired `vajra:open-add-url` listener to open `AddUrlDialog`.

### Phase 4 — Dialogs Audit & UX Patterns
- [x] **`useDialogEscape` hook** (`src/hooks/useDialogEscape.ts`): Shared hook — Escape key closes any dialog via its `onClose` callback.
- [x] **`aria-modal`, `role="dialog"`, `aria-labelledby`**: Applied to all 10 dialog panels for full accessibility compliance.
- [x] **`AboutDialog`**: Fetches live app version via Tauri `getVersion()` API.
- [x] **`DeleteDialog`**: Split single "Delete" into two distinct actions — **Remove from List** (keeps file on disk) and **Delete from Disk** (permanent). Added "Remember my choice" checkbox.
- [x] **`PropertiesDialog`**: Added animated **Saved ✓** badge in header that appears for 1.5s after each auto-save debounce fires.
- [x] **`AboutDialog`**: Fetches live app version via Tauri `getVersion()` API.
- [x] **`DeleteDialog`**: Split single "Delete" into two distinct actions — **Remove from List** (keeps file on disk) and **Delete from Disk** (permanent). Added "Remember my choice" checkbox.
- [x] **`PropertiesDialog`**: Added animated **Saved ✓** badge in header that appears for 1.5s after each auto-save debounce fires.
- [x] Escape key + aria applied to: `AddUrlDialog`, `RefreshUrlDialog`, `SchedulerDialog`, `ImportContainerDialog`, `GrabberDialog`, `SpiderDialog`, `OptionsDialog`.

### Phase 5 — Window Chrome Consistency
- [x] Standardized title bars across all popup windows (`AddUrlDialog`, `PropertiesDialog`, `SchedulerDialog`, `RefreshUrlDialog`).
- [x] Replaced remaining native `alert` / `confirm` calls with inline error/confirmation UI.
- [x] Applied `dialog-header`, `dialog-body`, `dialog-footer` CSS classes uniformly across all 10 dialogs.
- [x] Consistent close button (`btn-icon` + `X` lucide icon) on every dialog panel.

### Phase 6 — Dashboard Analytics Overhaul
- [x] Full rewrite of `Dashboard.tsx` — replaced placeholder with live analytics view.
- [x] **KPI Cards**: Active, Completed, Failed, and Total Bytes counters with trend indicators.
- [x] **Speed History Chart**: Recharts-based real-time sparkline for download throughput (last 60 data points).
- [x] **Recent Activity Feed**: Last 5 completed/active downloads with status pills.
- [x] `App.tsx`: Added `onNavigate` prop to `<Dashboard />` to wire CTA actions into sidebar navigation.

### Phase 7 — Accessibility, Focus Trapping & Final Polish
- [x] **`useFocusTrap` hook** (`src/hooks/useFocusTrap.ts`): New lightweight hook — traps Tab/Shift+Tab within dialog panel; moves focus to first focusable element on open. Zero dependencies.
- [x] Applied `useFocusTrap` to all 10 dialogs: `AboutDialog`, `AddUrlDialog`, `DeleteDialog`, `GrabberDialog`, `ImportContainerDialog`, `OptionsDialog`, `PropertiesDialog`, `RefreshUrlDialog`, `SchedulerDialog`, `SpiderDialog`.
- [x] **`aria-sort`** + `scope="col"` added to all sortable `<th>` headers in `DownloadsTable.tsx` via `ResizableHeader`.
- [x] **`aria-label` + `aria-disabled`** added to `ActionButton` in `Toolbar.tsx`.
- [x] **Sidebar navigation semantics**: Root `<div>` → `<nav aria-label="Application navigation">`; all items get `role="button"`, `tabIndex`, `aria-current="page"`, and keyboard `Enter`/`Space` handler.
- [x] Verified `@media (prefers-reduced-motion: reduce)` and `*:focus-visible` global styles already present.
- [x] Verified `aria-live="polite"` on status bar already present.

### Hotfix — Toolbar Group Layout
- [x] Added `[role="toolbar"] > [role="group"] { display: flex; align-items: center; }` to `@layer components` in `index.css` — the `role="group"` divs were defaulting to `display: block`, collapsing the toolbar into a vertical stack.
