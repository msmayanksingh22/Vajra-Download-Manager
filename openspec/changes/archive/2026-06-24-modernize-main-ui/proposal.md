# Proposal: Modernize Main UI

## Description
Apply Fission-AI driven design standardisation to the primary components (`App.jsx`, `MainWindow.jsx` equivalent, `Sidebar.jsx`, `Toolbar.jsx`, `DownloadsTable.jsx`). Replace generic legacy components with new `sys-` token based glassmorphism UI.

## Motivation
The secondary dialogs (Options, Properties, Scheduler) were already modernized. However, the core application shell remained untouched visually due to a missing mapping in the CSS variable definitions. This change unifies the whole application.

## Goals
- Bind all `bg-bg1` and `bg-bg2` Tailwind references correctly in `index.css`.
- Ensure main structural UI files employ the `.sys-btn` and `.sys-input` definitions.
- Deliver an aesthetically flawless Vajra UI experience.
