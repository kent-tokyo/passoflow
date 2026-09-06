# PassoFlow roadmap

Updated 2026-09-06. Current version: **0.1.2**.

## Product direction

PassoFlow is a personal, local automation tool. The main evaluation axes are GUI operation, image recognition, easy scenario authoring, repeatability, safety, and ease of setup.

Orchestration, deep data workflows, distributed execution, cloud operation, team management, and audit platforms are deliberately out of scope.

## Target architecture

Rust becomes the single source of truth for the scenario model, execution engine, input control, screen capture, and image recognition. PassoFlow will not depend on PyAutoGUI or another general-purpose GUI automation library. Python is an official PyO3 binding and compatibility surface, not a second implementation. The current UI design, operation flow, accessibility, and visual clarity are product requirements and must be preserved throughout the migration; the existing React editor remains the UI while the Rust backend stabilizes.

```text
passoflow-core       Scenario model, validation, variables, diagnostics
passoflow-engine     Execution state, retries, safety, recovery, events
passoflow-input      PassoFlow-owned mouse, keyboard, window adapters
passoflow-capture    PassoFlow-owned screen and window capture
passoflow-vision     PassoFlow-owned region search and image matching
passoflow-web        Rust DOM browser adapter
passoflow-python     PyO3 bindings, published to PyPI
passoflow-server     Local API used by the existing React editor
```

All public layers share typed concepts: `Scenario`, `Step`, `Action`, `Diagnostic`, `ActionResult`, `RetryPolicy`, `FailureArtifact`, and `RunEvent`.

## Current baseline

- [x] Browser-based flow editor with validation, undo/redo, loops, branches, image capture, table preview, logs, and guarded reruns.
- [x] Separate visual-browser and DOM-browser actions.
- [x] Image candidates, confidence, retry, offsets, regions, target-window safety, selector preview, and failure evidence.
- [x] Local-only first-success, setup, run, and recovery timing instrumentation.
- [x] Provide a Windows one-click launcher that starts the local services and opens the browser when ready.
- [x] English, Japanese, and Chinese UI strings.

## Phased plan

### Phase 0 — contracts and dependency boundary

- [ ] Freeze the YAML schema, reserved keys, action names, and three-state action outcome contract.
- [x] Define the versioned Rust/JSON event protocol for the engine, Python binding, and local UI.
- [x] Add MIT OR Apache-2.0 license metadata and preserve the copyright holder name.
- [x] Add a Rust workspace, MSRV policy, and CI matrix without changing the current runner.
- [x] List every PyAutoGUI capability currently used and define its PassoFlow-owned replacement interface.

Gate: the contract and replacement scope are documented, and existing action fixtures cover the boundary.

### Phase 1 — `passoflow-core`

- [x] Implement typed scenario, step, action, variable, diagnostic, and retry-policy model foundations in Rust.
- [x] Implement initial YAML parsing, reserved-key validation, and deterministic normalization.
- [x] Add initial Rust tests for existing-style scenarios, unknown actions/parameters, and deterministic diagnostics.
- [x] Add shared Rust/Python compatibility fixtures for control-flow and basic schema failures.
- [x] Add a Python-side compatibility test that runs when the current runner dependencies are installed.
- [x] Extend shared fixtures to image safety, coordinate, confidence, and timeout constraints.
- [x] Align DOM timeout, Webhook payload, and conditional-value constraints in shared fixtures.
- [x] Add a Rust CLI that validates scenarios and emits machine-readable diagnostics.

Gate: Rust validation agrees with the Python validator on the fixture corpus and has no GUI/OS dependency.

### Phase 2 — compatibility bridge

- [x] Make the Python runner call `passoflow-core` for parsing and validation while preserving existing commands (opt-in bridge; Python remains the default).
- [x] Compare Python and Rust normalized scenarios and diagnostics in compatibility tests.
- [x] Define the initial Rust `ExecutionPlan` boundary for step order, branches, loops, and variable references.
- [x] Carry normalized step parameters in the plan so native image and browser executors receive action values.
- [x] Consume the plan in the opt-in runner as a pre-execution step-alignment safety gate.
- [ ] Move variable and control-flow planning behind the Rust engine interface.
- [ ] Preserve YAML compatibility, logs, screenshots, guarded reruns, and the existing Web UI.

Gate: existing scenarios validate and run without observable schema regression.

### Phase 3 — PassoFlow-owned input library

- [x] Implement a Rust input abstraction for pointer movement, click/double-click, scrolling, key press, hotkeys, and text entry.
- [x] Add portable coordinate bounds, fail-safe point checks, click-indicator timing, and an in-memory recording backend.
- [x] Define typed display scaling, multi-monitor display metadata, keyboard layout metadata, and accessibility permission states.
- [x] Add an initial Windows `user32` adapter for screen-coordinate pointer, click, scroll, key, hotkey, and Unicode text events.
- [x] Send Unicode text through Win32 `SendInput` UTF-16 events instead of an 8-bit keybd-event path.
- [x] Add Windows controller construction with automatic virtual-desktop bounds and keyboard-layout reporting.
- [x] Map Windows active-window coordinates and expose the virtual-desktop bounds, including negative monitor origins.
- [x] Add a Windows foreground-window snapshot API with title and screen-space bounds for guarded visual actions.
- [x] Add a Windows CI job that compiles and checks the native adapter on `windows-latest`.
- [ ] Implement platform adapters directly against supported OS APIs; do not wrap PyAutoGUI, Enigo, or another automation library.
- [ ] Define coordinate spaces, display scaling, multi-monitor behavior, keyboard layouts, and permission diagnostics.
- [x] Add a small standalone dry-run input test tool and portable safety tests.
- [ ] Keep the red click indicator and fail-safe behavior as PassoFlow-owned features.

Gate: basic input actions work on the stated platform matrix with explicit permission and coordinate failures.

### Phase 4 — PassoFlow-owned capture and vision

- [x] Define a platform-independent RGBA frame, screen origin, capture region, and deterministic crop contract without PyAutoGUI/Pillow.
- [x] Implement an initial Windows GDI screen capture adapter without PyAutoGUI/Pillow capture paths.
- [x] Add deterministic template matching with region bounds, confidence scoring, stable candidate ordering, ambiguity-safe results, and anchor/offset click-point resolution.
- [x] Add a screen-coordinate capture boundary with a native Windows fast path and a deterministic crop fallback.
- [x] Add Windows foreground/native window capture and per-window DPI lookup primitives.
- [x] Add a non-invasive Windows GDI capture availability diagnostic with explicit granted/denied/unknown states.
- [x] Add Windows per-monitor enumeration with best-effort effective DPI metadata.
- [ ] Add runtime permission guidance on every supported platform.
- [x] Port retry-on-`NotFound` behavior into the Rust capture/vision boundary.
- [x] Add platform-independent active-window region resolution and fail-closed target-title guards.
- [x] Connect image-match anchor/offset results to the PassoFlow input controller with guarded single/double-click dispatch.
- [x] Reuse the guarded capture/search pipeline for `move_mouse_to_image` without dispatching a click.
- [x] Pass per-step `click_indicator_duration` from the Rust image executor to the input controller.
- [x] Expose one capture-search-guard-click pipeline with injectable search and click options.
- [x] Add the Windows adapter-to-vision window-context conversion boundary.
- [ ] Define scaling and color-space behavior for deterministic template matching.
- [x] Define the initial exact-pixel/RGB-only/no-implicit-scaling matching behavior.
- [x] Add fixed-image accuracy tests and a dependency-light search benchmark.
- [x] Preserve ambiguous-match safety: no automatic click when the result is below the configured confidence or cannot be uniquely selected.

Gate: Rust capture and image actions meet the current accuracy target and have reproducible local measurements.

### Phase 5 — Rust DOM and unified engine

- [x] Implement the initial platform-independent `passoflow-engine` runner with retries, warning continuation, failure stop, stop requests, and versioned step-attempt events.
- [x] Define a platform-independent DOM action boundary with selector/URL/timeout safety checks and a recording backend.
- [ ] Select and prototype a Rust Chromium adapter using CDP or a maintained Playwright-compatible client.
- [ ] Port navigation, click, fill, wait, selector preview, and selector recovery diagnostics.
- [x] Implement the initial `passoflow-engine` execution boundary with retries, safety stop semantics, and structured events.
- [x] Add the initial Rust `click_image` `StepExecutor` with injected template loading and capture/search/click policy.
- [x] Add `move_mouse_to_image` execution with the same region, retry, confidence, and target-window safety policy.
- [x] Add a scenario-rooted PNG/JPEG/BMP `FileTemplateLoader` with path-traversal protection.
- [ ] Replace Python orchestration with the Rust engine while retaining only explicitly scoped platform adapters.
- [ ] Add `passoflow-server` as a local-only API compatible with the current React editor.
- [ ] Keep the current palette, canvas, parameter panel, dialogs, logs, focus behavior, and light/dark visual treatment behaviorally and visually compatible.

Gate: successful, warned, failed, stopped, and recovered scenarios behave consistently through the Rust engine.

### Phase 6 — Python binding and public crates

- [x] Build the initial `passoflow-python` PyO3/maturin binding, exposing normalized YAML, diagnostics, execution plans, and contract errors.
- [x] Generate Python type stubs and provide a Rust/Python example with identical contract behavior.
- [x] Publish the reusable `passoflow-core` interface to crates.io; publish the
  engine only after its interface is stable.
- [x] Add the guarded GitHub Actions publish path for `passoflow-core` followed by `passoflow-python` using `CARGO_REGISTRY_TOKEN`.
- [ ] Publish platform wheels to PyPI and test clean virtual-environment installation.
- [x] Register the `passoflow` GitHub Actions Trusted Publisher configuration for the `release.yml` / `pypi` environment.
- [x] Document desktop permissions, Chromium requirements, supported platforms, and unsupported adapters.

Gate: Rust users can use the core from crates.io, and Python users can use the same Rust core from a wheel without local Rust compilation.

### Phase 7 — candidate release and deprecation

- [ ] Run compatibility, security, packaging, UI smoke, and platform checks.
- [ ] Measure image accuracy, input latency, first success, setup completion, and recovery time with fixed local protocols.
- [ ] Deprecate Python-only implementations only after equivalent Rust behavior is verified.
- [ ] Prepare migration notes, CHANGELOG, crates.io release, PyPI release, and the next version candidate.

Gate: published artifacts, docs, version metadata, and the UI describe the same supported behavior.

## Non-goals

- Distributed execution, cloud orchestration, team administration, audit platforms, and deep data-pipeline features.
- A broad PyAutoGUI-compatible API unrelated to PassoFlow's scenario model.
- Rewriting the React editor solely to make the backend Rust-based.
- Replacing the current UI design with a new paradigm during the engine migration.

## Done criteria

A phase is complete only when implementation, compatibility fixtures, user-facing docs, security behavior, packaging evidence, and UI regression evidence agree. Performance claims require fixed local measurements; platform claims require runtime evidence on the stated platforms. Rust migration work must not regress the current UI's ease of use, visual clarity, keyboard access, or light/dark readability.
