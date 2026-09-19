# Design: Modernize Main UI

## Variables Missing
Currently, `index.css` lacks mapping for `--color-bg1`, meaning the classes `bg-bg1` and `bg-bg2` inside `App.jsx`, `Toolbar.jsx`, `Sidebar.jsx`, and `DownloadsTable.jsx` fail to resolve to any color. As a result, the user does not see the changes in the application.

## Solution
Expose the `--color-bg1` variables through Tailwind `@theme` properly.

## Options State Saving
Check and ensure `OptionsDialog.jsx` fully saves configuration back to the API and persists any frontend-only state (logins, dialup) safely inside LocalStorage.
