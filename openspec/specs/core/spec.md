# core Specification

## Purpose
Core UI capabilities and modernization features.

## Requirements

### Requirement: CSS Resolution
The system SHALL define modern theme variables using Tailwind `@theme` in `index.css`.
The system SHALL map standard Tailwind variables to actual `--color-bg1` variables used in the app.

### Requirement: Options Save Functionality
The system SHALL provide an options dialog (`OptionsDialog.jsx`) that correctly calls `api.setConfig(config)` when the user saves and closes the dialog.
