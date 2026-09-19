# Change: UI Design System Overhaul
ID: ui-design-overhaul
Status: proposed
Priority: high
Area: vajra-ui-tauri

## Problem
See docs/implementation_plan.md (full analysis).
Key issues: token conflict between tailwind.config.js (hardcoded dark HSL) and index.css
(CSS variables for both themes); raw Tailwind color classes in Toolbar bypassing design system;
missing semantic surface tokens; inconsistent dialog structure across 8 dialogs; broken light mode.

## Scope
22 files in vajra-ui-tauri/src/ — no backend/Rust changes.

## Implementation Order
1. index.html - Inter font link
2. index.css - full design system rewrite
3. tailwind.config.js - remove hardcoded palette
4. ThemeContext.tsx - OS theme sync
5. Toolbar.tsx - semantic icon colors
6. Sidebar.tsx - sidebar-item class
7. MenuBar.tsx - header unification
8. DownloadsTable.tsx - table-th/table-row/tag
9. All 8 Dialogs - dialog-panel pattern
10. All 3 Windows - window pattern
11. App.tsx - cleanup
