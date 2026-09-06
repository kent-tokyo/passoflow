# PassoFlow UI design

PassoFlow is a local browser-based editor with a desktop-like shell. It is not an Electron app and does not aim to become a cloud orchestration console.

## Layout

- `TitleBar`: product name, scenario name, and window-level actions.
- `MenuBar`: File, Edit, View, and Help menus for secondary commands.
- `ActionPalette`: searchable actions grouped by purpose.
- `FlowCanvas`: the scenario graph; nodes have a light tint and clear selected/error states.
- `ParameterPanel`: schema-driven fields, hints, units, image capture, and selector inputs.
- `ExecutionPanel`: run, stop, validation, and compact logs.
- `StatusBar`: save state, locale, and execution status.

## Interaction principles

- A first-time user should be able to add an action, configure it, validate it, and run it without reading the full reference.
- Visual browser actions and DOM browser actions are separate so the user chooses between screen-image control and Playwright/CSS-selector control explicitly.
- Required fields, safety-sensitive options, coordinate units, and file/variable fields explain themselves in the UI.
- Keyboard focus, visible focus indicators, accessible names, and color-independent status cues are required.
- Desktop actions remain Windows-oriented; the editor itself remains usable in a normal browser tab.

## Verification

UI changes require `npm run build`, `npm run lint`, `npm run test:smoke`, and a live browser check of the affected flow. User-facing strings must be updated in English, Japanese, and Chinese.

