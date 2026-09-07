# Changelog

All notable changes to PassoFlow are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Added a Windows one-click launcher that installs missing local dependencies, starts the API/UI, waits for readiness, and opens the browser.
- Added a packaged-build launcher path that serves the built UI through FastAPI without requiring Node.js.
- Added Python binding type stubs, a minimal usage example, and clean-wheel installation coverage.
- Added the platform-independent Rust DOM action contract and deterministic recording backend for dry runs and previews.
- Tightened Rust DOM navigation validation to require an HTTP(S) host and reject whitespace or control characters.
- Added non-mutating recursive variable resolution for Rust execution-plan parameters.
- Added deterministic branch-boundary and contiguous-loop metadata to Rust execution plans.
- Added a fail-closed Rust selector for chosen branches and fixed-count loops.
- Connected selected Rust control-flow steps to the engine's retry, stop, and event path.
- Added Rust-compatible runtime branch-condition evaluation for variables and `last_step`.
- Added isolated runtime table-row expansion for Rust execution-plan loops.
- Added the engine API for executing resolved table-loop steps with shared outcomes.
- Added a runtime-snapshot facade for deriving Rust engine branch decisions.
- Runtime-snapshot execution now resolves action parameters inside the Rust engine before dispatch.
- Added a stateful Rust branch runner that applies adapter variable updates before later conditions.
- Added runtime-aware fixed-count and table-row loop execution with scoped row variables.
- Exposed the stateful Rust runner through a Python callback binding for staged runner migration.
- Preserved step-level retry and retry-interval settings in the Rust runtime path.
- Added an opt-in Python runner path that dispatches compatible root actions through the Rust engine callback bridge.
- Added typed DOM action conversion from normalized Rust execution-plan steps.
- Added a testable Rust CDP adapter prototype for navigation and DOM operations.
- Added correlated JSON CDP transport handling behind an injectable text-wire boundary.
- Added an optional synchronous `tungstenite` WebSocket wire for local Chromium CDP endpoints.
- Preserved live progress markers and step failure screenshot artifacts in the Rust-backed runner path.
- Completed the handoff of Rust failure artifact paths into the existing runner/Web UI log stream.
- Added step-boundary cooperative stop propagation for Rust-backed runs, with the existing kill fallback retained.
- Changed API stop handling to wait for graceful completion before the delayed kill fallback.
- The opt-in Python bridge now dispatches normalized action parameters from the Rust plan after a strict step-alignment check.
- Updated the runtime dependency on `elixcee` to `1.0.3`.

### Changed

- Adopted Rust Edition 2024 with Rust 1.88 as the minimum supported toolchain.
- Updated CI Python dependency setup and Rust Clippy coverage, including the Windows native adapter.

## [0.1.2] - 2026-09-06

### Added

- Image-candidate priority management in the parameter panel (edit, remove, and reorder).
- Image-candidate guidance now explains top-to-bottom priority and matched-candidate logging.
- Optional failure screenshots saved under `logs/` for failed or stopped runs.
- Configurable Webhook failure handling with `on_error: continue` or `stop`.
- Optional `launch_app.wait_for_window` startup readiness checks and timeout handling.
- First-use guidance now explains the visual/DOM browser choice and Chromium setup.
- First-use guidance now detects missing Playwright or Chromium setup and explains the local fix.
- Selected DOM actions now show the same setup status beside their parameters.
- Missing setup commands can be copied directly from the selected DOM action panel.
- Image search regions can now be relative to the current foreground window with `region_origin: active_window`.
- Image actions can optionally fail closed unless the foreground window title contains `target_window_title`.
- Image action parameters now offer a current-screen region preview, with a clear note for `active_window` coordinates.
- The action API now exposes a consistent success/warning-continue/failure-stop outcome contract.
- DOM action selector fields now provide a safe local CSS syntax check before saving or running.
- DOM selector fields now support a temporary-page match preview with element samples and a conservative suggested selector.
- DOM preview results now let users choose a reviewed element candidate and copy its generated selector into the step.
- DOM selector preview failures now provide localized recovery guidance for timeout, navigation, and invalid-selector cases.
- Zero-match DOM previews now show only page-confirmed, conservative repair candidates and require explicit selection before applying one.
- The editor now records first-use, setup, run, and recovery timing locally without sending telemetry.
- Scalar parameter fields now have keyboard-accessible copy buttons with localized confirmation.
- The roadmap now targets a PassoFlow-owned Rust input, capture, and vision stack with PyO3 Python bindings; PyAutoGUI is explicitly not a runtime dependency target.
- The Rust migration plan explicitly preserves the current UI design, interaction flow, accessibility, and visual clarity.
- Added the initial `passoflow-core` Rust workspace crate with typed scenario contracts and deterministic basic validation tests.
- Added the `passoflow-validate` Rust CLI for machine-readable scenario diagnostics.
- Added shared Rust/Python validator fixtures for control-flow and basic schema compatibility.
- Added a shared expected-diagnostics contract consumed by Rust and Python compatibility tests.
- Added an opt-in Python adapter for consuming Rust validator reports without changing the default runner.
- Rust bridge reports now require the expected contract version and internally consistent counts.
- Rust bridge reports now validate every diagnostic's severity and required fields.
- Rust bridge timeouts now return explicit actionable errors instead of leaking subprocess exceptions.
- Added a real-binary bridge regression test and CI execution after building the Rust validator.
- Added an explicit `/validate-rust` API endpoint for selecting Rust validation per request.
- Added an opt-in runner flag to delegate root scenario validation to the Rust CLI.
- Added deterministic normalized-YAML output to the Rust CLI and Python bridge.
- Added a `/validate-compare` compatibility gate for side-by-side Python/Rust validation.
- Compatibility gate responses now include error-count and warning-count matches.
- Compatibility gate responses now expose a consolidated `ready_for_default` decision.
- Python validation now distinguishes missing, empty, and non-string `action` values like the Rust validator.
- The opt-in Python runner path now loads Rust-normalized YAML after Rust validation, while the default path remains unchanged.
- Rust core now exposes an initial typed `ExecutionPlan` for branches, loops, and variable references.
- The opt-in runner now stops before execution when a Rust plan and loaded steps differ.
- Added the portable `passoflow-input` Rust crate with typed input events and fail-safe validation.
- Added typed display scaling, multi-monitor, keyboard-layout, and permission-state contracts.
- Added an explicit unsupported-platform input fallback that fails closed instead of silently doing nothing.
- Added the initial Windows `user32` input adapter for screen-coordinate pointer and keyboard events.
- Windows input now maps active-window coordinates and reports the virtual-desktop bounds, including negative monitor origins.
- Windows Unicode text input now uses Win32 `SendInput` UTF-16 events for non-ASCII accuracy.
- Windows adapter setup now provides an auto-bounded controller and keyboard-layout reporting.
- Added Windows CI coverage for compiling, testing, and linting the native input adapter.
- Added `passoflow-input-check`, a non-invasive JSON dry-run tool for input setup and CI checks.
- Added the portable `passoflow-capture` crate with RGBA frame validation and deterministic region cropping.
- Added the initial Windows GDI screen capture adapter with virtual-desktop and region support.
- Added the portable `passoflow-vision` Rust crate with region-bounded deterministic template matching, confidence scoring, stable candidate ordering, and ambiguity-safe results.
- Rust image candidates now resolve the existing named anchors and explicit offsets to checked screen-space click points.
- Added the initial platform-independent `passoflow-engine` runner with retry, warning-continue, failure-stop, stop-request, and versioned step-attempt event handling.
- Rust vision now retries only `NotFound` results through the capture backend; ambiguous matches and capture errors return immediately.
- Rust vision now resolves active-window-relative regions and rejects missing or mismatched target-window context before image actions proceed.
- Rust vision now dispatches unique image matches through the PassoFlow input controller while refusing missing, ambiguous, or target-window-unsafe clicks.
- Windows input now exposes the foreground window title and screen-space bounds, with a vision conversion helper for native guarded image actions.
- Rust vision now documents exact-pixel RGB matching (alpha ignored, no implicit scaling), adds fixed-image accuracy coverage, and includes a dependency-light benchmark.
- Rust vision now exposes one configurable capture-search-guard-click pipeline, preserving retry, target-window, ambiguity, indicator, and fail-safe behavior.
- Rust capture now provides a screen-coordinate region boundary, using direct Windows GDI capture when available and deterministic cropping for generic backends.
- Windows capture now exposes foreground/native window capture, per-window DPI lookup, and a non-invasive GDI availability diagnostic.
- Windows capture now enumerates monitors with best-effort effective DPI metadata while retaining monitor geometry when DPI lookup is unavailable.
- Rust `ExecutionPlan` now carries normalized step parameters for native image and browser executors.
- Added the initial Rust `click_image` `StepExecutor` with injectable template loading and safe capture/search/click result mapping.
- Added Rust `move_mouse_to_image` execution with the same retry, region, confidence, and target-window safety policy.
- Rust image execution now forwards per-step `click_indicator_duration` to the input controller while retaining the visible safety cue.
- Added a scenario-rooted Rust `FileTemplateLoader` for PNG/JPEG/BMP templates with path-traversal protection.
- Added a Python YAML data-model round-trip check for Rust normalization.
- Python runner regression tests now skip cleanly when desktop automation dependencies are absent.
- CI now runs the dependency-light Python compatibility and runner test suite.
- README英日版にRust validatorの安全なopt-in手順とPython fallbackを追記。
- Rust CLI validation now follows relative nested scenario references and detects missing, invalid, and circular children.
- Added nested-reference regression fixtures for missing and circular scenario targets.
- Rust CLI validation now reports missing image files as non-blocking warnings.
- Rust CLI validation now reports unloaded loop tables as non-blocking warnings.
- Rust CLI validation now reports undefined variables as non-blocking warnings.
- Rust scenario parsing now preserves Python-compatible empty scenarios and reports missing step actions as diagnostics.
- Rust scenario parsing now rejects a missing root `steps` key as `invalid_scenario`.
- Rust CLI now returns malformed YAML as a machine-readable `invalid_scenario` diagnostic.
- Rust scenario parsing now reports non-mapping steps as `invalid_step` diagnostics.
- Rust scenario validation now reports non-string action values as `invalid_action` diagnostics.
- Added a static-reference regression ensuring defined variables and loaded tables stay warning-free.
- Rust CLI diagnostics are now deterministically ordered after nested checks and warning enrichment.
- Added the official repository URL to Rust package metadata for future crates.io publication.
- Added an environment-aware Python compatibility test for the shared validator fixtures.
- Added a repository quality workflow covering Rust MSRV checks, Python syntax, and Web UI build/lint/smoke checks.
- Rust CI now treats workspace clippy warnings as errors for the core candidate.
- Rust CI now verifies locked crate packaging for the core candidate.
- Roadmap Phase 1 now records the normalized-scenario and diagnostic compatibility gate as complete.
- Added typed Rust action outcomes, retry policy, failure artifacts, and versioned run events for the future engine boundary.
- Added a compact shared scenario contract covering YAML shape, normalization, and action outcomes.
- Extended Rust validation coverage for image safety, coordinates, confidence, and timing constraints.
- Aligned Rust validation for DOM timeouts, Webhook payloads, and conditional values with the Python contract.
- Added a deterministic normalized-YAML round-trip regression for future Python/Rust compatibility checks.
- Failed or stopped runs now retain failed-step before/after screenshots when screen capture is available.
- Failure artifact log lines now link to the affected step and offer a guarded one-step rerun.

## [0.1.1] - 2026-09-06

### Changed

- Updated the minimum supported `elixcee` version to `1.0.2`.
- Updated Python and web dependencies to the latest stable versions checked on 2026-09-06.
- Reduced the default `click_image` indicator pause to 0.25 seconds while keeping the visible safety cue.
- Added separate visual-browser and DOM-browser actions using the default browser and Playwright.

## [0.1.0] - 2026-09-06

### Added

- Initial PassoFlow release.
- Windows-oriented RPA runtime with screen-image actions, keyboard and clipboard operations, window activation, application launching, file operations, and Excel/CSV support.
- Visual scenario editor with loops, conditional branches, execution logs, screenshot capture, and optional AI assistance.
