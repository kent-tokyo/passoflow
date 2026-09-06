# PassoFlow Rust engine boundary

This document records the Phase 0 migration boundary. It is deliberately
short: Rust becomes the source of truth gradually, while the current Python
runner and UI remain the compatibility baseline until each phase has evidence.

## Rules

- Rust owns scenario semantics and, eventually, input, capture, and image
  recognition. It must not wrap PyAutoGUI, Enigo, or another general-purpose
  GUI automation library.
- Python exposes the Rust implementation through PyO3. It is an adapter, not a
  second engine implementation.
- The current React UI, interaction flow, visual hierarchy, light/dark
  readability, and keyboard access are compatibility requirements.
- The first Rust crate has no GUI, OS, browser, network, or Python dependency.

## Current PyAutoGUI capability inventory

| Current capability | PassoFlow-owned Rust boundary | Phase |
| --- | --- | --- |
| Mouse position and movement | `InputBackend::move_pointer` | 3 |
| Single/double click | `InputBackend::click` | 3 |
| Scroll | `InputBackend::scroll` | 3 |
| Key press and hotkey | `InputBackend::press_key` / `hotkey` | 3 |
| Clipboard-backed text input | `InputBackend::type_text` | 3 |
| Foreground window lookup/activation | `WindowBackend` | 3 |
| Screen/window screenshot | `CaptureBackend` | 4 |
| Region crop and coordinate conversion | `CaptureBackend::crop` | 4 |
| Template image matching | `VisionBackend::find_candidates` | 4 |
| Confidence and candidate ordering | `VisionBackend` + engine policy | 4/5 |
| Red click indicator | PassoFlow-owned UI/overlay adapter | 3 |
| Failure screenshots | Capture backend + engine artifact policy | 4/5 |

The Python implementation remains in place during migration. Removing
`pyautogui` from runtime dependencies is a Phase 3/4 completion criterion,
not a Phase 0 change.

## Event contract

The engine emits versioned JSON events. Names and fields are intentionally
stable before the implementation moves between processes or language bindings.

```json
{
  "protocol": "passoflow.run.v1",
  "type": "action_finished",
  "run_id": "local-run-id",
  "step": 3,
  "action": "click_image",
  "outcome": "success",
  "message": "Matched image candidate 1",
  "artifacts": []
}
```

Allowed outcomes are `success`, `warning_continue`, and `failure_stop`.
Artifact entries are local paths only; the engine does not upload screen
content. New fields must be optional, and a breaking contract change requires
a new protocol version. `passoflow-core` now exposes these outcomes as typed
`ActionOutcome`/`ActionResult` values, together with `RetryPolicy`,
`FailureArtifact`, and `RunEvent` for binding and UI adapters.

## Phase 0 verification

The workspace must compile and the contract document must agree with the
current action reference, runner outcome contract, and UI behavior before the
first Rust runtime adapter is introduced. `cargo run --manifest-path
rust/Cargo.toml --bin passoflow-validate -- <scenario.yaml>` is the initial
machine-readable validation entry point; it exits non-zero when errors exist.
Repository checks are defined in `.github/workflows/quality.yml`: Rust is
formatted and tested against the MSRV, Python sources are syntax-checked, and
the Web UI build, lint, and smoke checks run on every push and pull request.
An opt-in Python adapter in `src/rust_validator.py` can consume the CLI report
when `PASSOFLOW_VALIDATE_BIN` points to a built validator. It is not enabled by
default until nested references and Python warning behavior are covered. The
API exposes this explicitly as `/api/scenarios/{filename}/validate-rust`; the
existing `/validate` endpoint remains Python-backed. The CLI now follows
relative `call_scenario` and `repeat` references and reports missing, invalid,
or circular child scenarios with stable diagnostic codes.
For an opt-in runner trial, set both `PASSOFLOW_USE_RUST_VALIDATOR=1` and
`PASSOFLOW_VALIDATE_BIN` to the built CLI. With both settings, the runner uses
Rust validation and deterministic normalized YAML for loading root and nested
steps. Without the flag or binary, the existing Python validation and loading
path remains active. A Rust CLI failure is surfaced as an actionable runner
error rather than silently running unvalidated YAML.
The same binary accepts `--normalized <scenario.yaml>` to emit deterministic
normalized YAML for compatibility comparisons.
It also accepts `--plan <scenario.yaml>` to emit the typed structural
`ExecutionPlan` as JSON for future engine and binding adapters.
The `/validate-compare` endpoint runs the Python baseline and Rust validator
side by side and reports whether their blocking validity decisions match. It
also reports error-count and warning-count matches; it is the gate for a
future default-path migration. `ready_for_default` is true only when all
three comparisons match.

The core also exposes a deterministic `ExecutionPlan` containing step order,
branch depth, loop metadata, and variable definitions/references. It is a
planning boundary only: the existing Python runner still executes actions, and
the plan does not grant any OS or browser capability. During an opt-in runner
trial, the plan is checked against the normalized steps before execution so a
stale or mismatched plan stops safely.

Phase 3 now has a portable `passoflow-input` crate. Its `InputController`
validates screen bounds, the fail-safe point, key names, hotkey contents, text
length, and single/double-click counts before dispatching typed events to an
`InputBackend`. Click events carry the configured indicator duration. The crate
ships with a recording backend for deterministic tests; OS adapters are not yet
implemented.

Windows also has an initial GDI-backed `WindowsCapture` adapter. It captures
the virtual desktop or a validated screen-coordinate region, converts BGRA
pixels to tightly packed RGBA, and preserves the actual screen origin. It also
supports foreground/native window capture, per-monitor enumeration, best-effort
DPI metadata, and a non-invasive GDI availability diagnostic; the current host
can only cross-compile this path, not perform Windows runtime capture.

An initial Windows adapter calls the native `user32` API directly for screen and
active-window coordinates, clicks, scrolling, named keys, hotkeys, and Unicode
text through Win32 `SendInput` UTF-16 keyboard events. It does not wrap PyAutoGUI
or Enigo. The adapter reports the Windows virtual-desktop bounds and keyboard
layout; other targets use the explicit fail-closed fallback until their adapters
exist. Runtime Windows permission verification remains a device-level gate.
`WindowsInput::controller()` applies the detected virtual-desktop bounds by
default and reports the active Windows keyboard-layout name for setup diagnostics.

`passoflow-input-check` runs a deterministic dry-run through the recording
backend and prints the versioned event sequence as JSON. It never sends input
to the operating system, so it is suitable for CI and setup diagnostics:
`cargo run --manifest-path rust/Cargo.toml -p passoflow-input --bin
passoflow-input-check`.

Phase 4 now has portable `passoflow-capture` and `passoflow-vision` crates.
Capture validates tightly packed RGBA frames, crops frame-relative regions, and
preserves the screen-space origin of cropped frames. Vision searches each named
template inside an optional region, scores RGB differences deterministically,
orders candidates by confidence and author order, and returns `Ambiguous`
instead of authorizing a click when the top candidates are too close. The
candidate also resolves the existing center/edge anchors or an explicit pixel
offset to a checked screen-space click point.
Windows GDI supplies the initial native frame source, including screen-space
regions and native window bounds. Native DPI and capture-availability APIs are
exposed, while end-to-end capture/search/click latency measurements and
device-level permission verification remain follow-up work.

The initial matching specification is exact pixel dimensions, RGB difference,
and no implicit scaling or color-space conversion; alpha is ignored. The crate
contains fixed-image accuracy tests and a standard-library-only benchmark at
`rust/crates/passoflow-vision/benches/search.rs`. A local 64x64 frame and 8x8
template baseline is reported by the benchmark, but is not a cross-machine
performance claim.

`passoflow-vision` also provides a capture-backed retry boundary. It recaptures
only after `NotFound`, while returning a unique match, an ambiguous match, or a
capture error immediately. The retry wait is injected through
`passoflow-vision::RetrySleeper`, so the behavior remains deterministic in
tests and can be coordinated with the engine scheduler.

The same crate defines `RegionOrigin` and `ActiveWindow` contracts. It resolves
window-relative regions into screen coordinates and verifies optional target
titles case-insensitively with substring matching. Missing window context or a
title mismatch fails closed before capture or clicking.

`click_match` connects a unique candidate to `passoflow-input` and preserves
the controller's click-indicator duration, coordinate validation, and fail-safe
checks. `click_match_guarded` performs the target-title check first and never
dispatches input for missing or ambiguous decisions.

`capture_and_click` combines screen-coordinate capture, `NotFound` retry, target
guard, matching, anchor/offset resolution, and input dispatch. `SearchOptions`
and `ClickOptions` keep the call site explicit without a long positional
argument list. Native backends can optimize the screen-region boundary at the
source; generic backends use a deterministic crop relative to the captured
frame origin. The active-window resolver is used before this call when the
scenario region is window-relative.

On Windows, `WindowsInput::active_window` reads the foreground title and
screen-space bounds directly from `user32`; `active_window_from_windows_input`
converts that snapshot into the vision contract. The current host can
cross-compile this adapter, while runtime verification still belongs in the
Windows CI/device gate.

`WindowsCapture` also exposes foreground/native window capture and
`GetDpiForWindow` metadata. Its `diagnostics` method performs a non-invasive
one-pixel GDI probe and reports `granted`, `denied`, or `unknown`; this is a
capture availability signal, not a substitute for device-level permission
verification. The same adapter enumerates Windows monitors and returns
best-effort effective DPI metadata per monitor; geometry remains available when
an individual DPI lookup is unavailable. Platform-specific setup guidance and
runtime verification remain open work.

The initial `passoflow-engine` runner now owns the control boundary around
these adapters. It accepts an `ExecutionPlan` and a `StepExecutor`, retries
only `FailureStop` results, continues after `WarningContinue`, honors stop
requests before and between attempts, and emits versioned `step_attempt`
events. Sleeping is injected through `RetrySleeper`, keeping tests and future
UI integrations deterministic.

Each `PlannedStep` also carries the normalized YAML parameter map. This keeps
the structural plan deterministic while giving a future native executor the
actual image paths, confidence, region, selector, and timeout values required
to perform the action.

`FileTemplateLoader` is the first concrete loader. It decodes PNG, JPEG, and
BMP files into tightly packed RGBA frames and canonicalizes paths under the
configured scenario root. Missing files, decode errors, and paths outside that
root are explicit failures; callers may still supply another `TemplateLoader`
for packaged assets or test fixtures.

The initial `passoflow` package (`passoflow-python` Rust crate) exposes
`validate_yaml`,
`normalize_yaml`, and `contract_version`. `validate_yaml` returns a JSON object
containing the contract version, validity, diagnostics, normalized YAML, and
execution plan. It is an opt-in compatibility surface; the existing Python
runner remains the default and desktop actions are not silently redirected.

`passoflow-vision` provides the first concrete `StepExecutor` for
`click_image` and `move_mouse_to_image`. It parses the plan parameters, loads
templates through the injected `TemplateLoader`, obtains optional window
context, applies region, confidence, search-retry, and target guards, then
maps unique/not-found/ambiguous results to the shared success/warning/failure
outcomes. For click steps, `click_indicator_duration` is forwarded to the
input controller; move steps use the same match point without dispatching a
click. Native image decoding and OS adapters remain injected boundaries, so
tests do not require a desktop.

The input contract also models exact display scale factors, multi-monitor
display bounds, keyboard layout metadata, and accessibility permission states.
Native adapters can report these facts before dispatching events so coordinate
and permission failures remain explicit.

Until a native adapter is compiled for a target, `UnavailableInput` is the
explicit fallback. It never performs a no-op success: every event returns an
unsupported-platform error, while the platform and permission information stay
available for setup guidance.
Missing image files are reported as non-blocking `missing_image` warnings at
the CLI boundary, matching the current Python runner's safety behavior. An
unloaded `loop_table` is likewise reported as a non-blocking
`missing_loop_table` warning.
Undefined variables are reported as non-blocking `undefined_variable` warnings
when the scenario has no loaded table definitions, matching the current
static-reference behavior.
