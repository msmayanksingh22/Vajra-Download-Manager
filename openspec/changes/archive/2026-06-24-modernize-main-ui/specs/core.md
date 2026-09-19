# Spec: UI Modernization

## CSS Resolution
- Update Tailwind `@theme` in `index.css`
- Ensure mapping between Tailwind variable standard and the actual `--color-bg1` variables

## Options Save Functionality
- `OptionsDialog.jsx` must call `api.setConfig(config)` on save and close.
