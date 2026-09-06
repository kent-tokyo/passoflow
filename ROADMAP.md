# PassoFlow roadmap

Updated 2026-09-06. Current version: **0.1.1**.

## Product direction

PassoFlow is a personal, local automation tool. Evaluation focuses on GUI operation, image recognition, easy scenario authoring, repeatability, safety, and ease of use.

The following are deliberately not priorities: orchestration, deep data workflows, distributed execution, cloud operation, team management, and audit platforms.

## Current baseline

- [x] Browser-based flow editor with palette, canvas, parameter panel, validation, undo/redo, loops, branches, table preview, and run logs.
- [x] Image actions with confidence, retry, offset, candidate logging, and optional `region` search bounds.
- [x] Faster visible click indicator: 0.25 seconds by default and configurable for safer review.
- [x] Visual browser mode: `open_url`, `activate_window`, `click_image`, and `type_text`.
- [x] DOM browser mode: `browser_navigate`, `browser_click`, `browser_fill`, and `browser_wait_for` using Playwright and CSS selectors.
- [x] Local validation, safety checks, scenario persistence, and explicit run/stop controls.
- [x] English, Japanese, and Chinese UI strings for user-facing changes.

## Next priorities

### 1. Make the first flow easy

- [ ] Add a short first-run path from “add action” to “validate and run”.
- [ ] Improve setup guidance and show missing dependency/browser feedback in context.
- [ ] Measure time-to-first-success and recovery from a failed step.

### 2. Improve image and GUI reliability

- [ ] Add image-candidate add/remove/reorder controls.
- [ ] Support active-window-relative regions and clearer coordinate units.
- [ ] Improve DPI/scaling and multi-scale matching guidance.
- [ ] Add optional before/after screenshots and safer click confirmation.

### 3. Improve browser workflows

- [ ] Add selector capture and a small selector preview/test flow.
- [ ] Expose browser-session settings without hiding the visual/DOM distinction.
- [ ] Make selector timeouts, detached elements, and browser startup errors actionable.

### 4. Improve reuse and accessibility

- [ ] Make common parameter patterns easier to copy and edit.
- [ ] Keep keyboard navigation, focus, localization, and non-color status cues under regression coverage.

## Done criteria

A feature is complete when the implementation, validation behavior, user-facing docs, and all three UI locales agree; the affected flow passes build/lint/smoke checks and a live browser check; and safety-sensitive behavior is explained in the UI.
