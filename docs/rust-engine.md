# PassoFlow Rust engine boundary

This document records the current migration boundary. Rust is becoming the
source of truth in phases; the Python runner and React editor remain the
compatibility baseline until each phase passes its gate.

## Rules

- Rust owns scenario semantics and the PassoFlow-specific input, capture, and
  image-recognition implementations. It does not wrap PyAutoGUI, Enigo, or
  another general-purpose automation library.
- Python exposes Rust through PyO3. It is a binding and compatibility surface,
  not a second engine implementation.
- The existing UI design, operation flow, accessibility, keyboard access, and
  light/dark readability are compatibility requirements.
- Rust workspace crates use Edition 2024 and require Rust 1.88 or later.

## Crate responsibilities

| Crate | Responsibility | Status |
| --- | --- | --- |
| `passoflow` | Stable umbrella crate re-exporting the published platform-independent contract | 0.1.2 published to crates.io |
| `passoflow-core` | Scenario model, YAML validation, normalization, diagnostics, execution plan | Implemented |
| `passoflow-engine` | Retry, warning/failure outcomes, stop requests, step-attempt events | Initial runner implemented |
| `passoflow-input` | Typed input events, safety checks, recording backend, Windows `user32` adapter | Windows initial adapter implemented |
| `passoflow-capture` | RGBA frames, regions, cropping, Windows GDI capture and diagnostics | Windows initial adapter implemented |
| `passoflow-vision` | Region-bounded RGB matching, candidate ordering, ambiguity guard, image executors | Initial image path implemented |
| `passoflow-python` | PyO3 binding for validation, normalization, plans, and callback-backed runtime execution | 0.1.2 crate published; PyPI wheel publication remains open |
| `passoflow-web` | DOM operation contract and recording backend | Initial boundary implemented |
| `passoflow-server` | Future Rust local API compatible with the React editor | Planned |

## Replaced capability boundary

| Existing operation | Rust boundary |
| --- | --- |
| Mouse, click, scroll, keys, hotkeys, text | `InputBackend` and Windows `user32` adapter |
| Screen/window capture and crop | `CaptureBackend` and Windows GDI adapter |
| Template search, confidence, regions, anchors | `passoflow-vision` |
| Failure screenshots | Capture backend plus local failure-artifact policy |
| Red click indicator and fail-safe checks | PassoFlow input/overlay policy; the persistent overlay reuses one marker window |

The Python implementation remains active while equivalent Rust behavior is
verified. Removing `pyautogui` from runtime dependencies is a later Phase 3/4
gate, not an automatic consequence of adding the Rust crates.

## Shared event contract

Rust emits versioned JSON events. The current protocol is
`passoflow.run.v1`; action outcomes are `success`, `warning_continue`, and
`failure_stop`.

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

Failure artifacts are local paths only; screen content is not uploaded. New
fields must be optional. A breaking change requires a new protocol version.
The same concepts are exposed by `passoflow-core` as
`ActionOutcome`, `ActionResult`, `RetryPolicy`, `FailureArtifact`, and
`RunEvent`.

## Current behavior

- Rust validation supports nested scenarios, deterministic diagnostics,
  normalized YAML, missing-image/loop-table/variable warnings, and a typed
  `ExecutionPlan`.
- The opt-in Python bridge uses `PASSOFLOW_USE_RUST_VALIDATOR=1` and
  `PASSOFLOW_VALIDATE_BIN`. Without both settings, the Python validator remains
  the default.
- When both settings are enabled, the runner also dispatches normalized action
  parameters from the Rust plan after strict step alignment. Python action
  functions remain the runtime adapter; variable mutation and dynamic control
  flow are evaluated by the stateful Rust engine.
- The callback bridge rejects unknown actions explicitly before dispatch, even
  if a caller bypasses the normal validator.
- Windows input reports virtual-desktop bounds, active-window information,
  keyboard layout, monitor geometry, best-effort DPI, and capture availability.
  Other platforms use an explicit unsupported adapter rather than a no-op.
- Input safety checks use the adapter-resolved screen coordinate, so an
  active-window-relative point cannot bypass the configured bounds or fail-safe
  point.
- A native adapter reporting denied accessibility permission is rejected before
  any input event reaches the OS.
- Screen-only adapters reject active-window-relative coordinates unless they
  explicitly provide a resolver.
- Unsupported-platform adapters report the unsupported platform consistently,
  regardless of the requested coordinate space.
- Image actions use exact-size sRGB matching with no implicit scaling or color
  conversion; alpha is ignored. Search supports candidate priority, confidence, retry, regions, anchors/offsets,
  active-window regions, target-window guards, and ambiguity-safe clicks.
- The initial Rust image executor covers `click_image` and
  `move_mouse_to_image`; the existing Python runner still executes the full
  action set.
- `passoflow-web::BrowserAction::from_planned_step` converts normalized
  `browser_navigate`, `browser_click`, `browser_fill`, and `browser_wait_for`
  steps into typed, validated DOM operations before a backend is called.
- `CdpBrowser` implements the DOM backend over an injected `CdpTransport`.
  It emits `Page.navigate` and `Runtime.evaluate` commands for navigation,
  click, fill, and selector-state waits. The optional WebSocket wire connects
  this contract to local Chromium without changing the browser action API.
- `JsonCdpTransport` now owns CDP command IDs, ignores protocol events, and
  fails closed on malformed, failed, or mismatched responses. The optional
  `websocket` feature provides `WebSocketCdpWire` for local `ws://` Chromium
  endpoints; it is kept optional so the default core remains dependency-light.
- With the `websocket` feature, `CdpBrowser::connect(endpoint)` composes the
  wire and JSON transport into one local DOM backend constructor.
- `CdpBrowser::preview_selector` provides read-only match counts, representative
  element metadata, a stable selector suggestion, and conservative repair
  candidates for common id, `data-testid`, and class selectors. Invalid CSS is
  reported as an error rather than guessed.
- `ExecutionPlan::resolve_variables` provides non-mutating, recursive
  `{{variable}}` expansion for native executors. Unset variables resolve to an
  empty string for compatibility with the Python runner.
- `run_with_runtime_snapshot` applies that resolution inside the engine before
  branch selection and action dispatch, so one snapshot governs both decisions
  and parameters.
- `run_with_runtime_state` is the stateful execution boundary: a runtime-aware
  adapter can return variable updates, and the next `if` is evaluated against
  the updated state. Fixed-count and table-row loops are supported; table-row
  variables are scoped to each iteration and restored afterward.
- The Python binding exposes `run_runtime_state(yaml, variables_json,
  tables_json, callback, stop_callback, run_id, attempts, interval_ms)`. The
  callback receives one serialized step and state, then returns a serialized
  `RuntimeActionResult`; the stop callback is polled at step boundaries.
  This moves control flow, retries, variables, and events into Rust while
  allowing the existing Python OS adapters to remain the callback implementation.
- `passoflow-core::Scenario::expand_nested_steps` expands a caller-supplied
  nested-scenario bundle without filesystem access and fails closed on missing
  or circular targets. The Python bridge now loads the bundle and delegates
  expansion to this API; table preloading remains in Python for compatibility.
- Runtime execution honors each step's `retry` and `retry_interval_ms` when
  present; the function arguments provide the fallback policy for steps without
  those fields.
- The existing Python runner can opt into this bridge with
  `PASSOFLOW_USE_RUST_ENGINE=1`. The current migration slice supports root
  actions, fixed loops, table-backed loops, and validated nested scenarios.
  `call_scenario` and `repeat` are flattened before Rust builds its plan; table
  rows are preloaded with the compatibility loader before Rust selects
  iterations. The default runner is unchanged.
- The bridge emits the existing `@@PROGRESS@@` and `@@COMPLETED@@` markers and
  returns step screenshot paths as failure artifacts when a run id is present.
- The Python runner logs those returned artifact paths, so the existing Web UI
  stream receives them alongside the Rust event message. Cooperative stop
  propagation is available at step boundaries; the API waits two seconds for
  graceful completion before using kill as a fallback.
- `web_actions` can opt into direct Rust DOM dispatch with
  `PASSOFLOW_USE_RUST_DOM=1` and `PASSOFLOW_RUST_CDP_ENDPOINT=ws://...`. The
  default Playwright path remains unchanged, and switching modes does not alter
  the scenario action names.
- A macOS arm64 0.1.2 wheel was installed into a clean venv and used against a
  live Chromium page for navigate, visible wait, selector preview, and click.
  A reusable HTML fixture also verifies fill, click, result waiting, and preview
  together; the observed result was `Hello, Rust E2E`.
- `ExecutionPlan::control_flow` exposes deterministic nested branch boundaries
  and contiguous loop ranges without evaluating conditions or invoking an OS
  adapter.
- `ExecutionPlan::select_steps` applies caller-supplied branch decisions and
  expands fixed-count loops. Table-backed loops remain an explicit runtime-data
  boundary, and missing branch decisions fail closed.
- `passoflow-engine::run_selected` now executes the selected sequence while
  preserving the existing retry, stop, and versioned event behavior.
- `evaluate_branch_condition` provides the Rust-side compatibility rule for
  variable truthiness, scalar equality, and `last_step` state checks.
- `ExecutionPlan::select_steps_with_tables` accepts runtime table rows and
  expands loop bodies with isolated row variables; missing rows fail closed.
- `passoflow-engine::run_selected_with_tables` sends those resolved steps
  through the shared retry, stop, and versioned-event path.
- `passoflow-engine::run_with_runtime_snapshot` derives branch decisions from a
  single variables/`last_step` snapshot before execution; mutable conditions
  between actions remain an open integration gate.
- `passoflow-web` validates `http`/`https` navigation, non-empty CSS
  selectors, and positive wait timeouts before dispatch. Its recording backend
  provides deterministic dry-run evidence; it does not open a browser yet.

## Verification

Run the portable Rust checks with:

```text
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets --locked -- -D warnings
cargo test --manifest-path rust/Cargo.toml --workspace --locked
```

The repository workflow also checks the Windows adapter, Python compatibility
tests, and Web UI build/lint/smoke tests. `passoflow-input-check` is a
non-invasive recording-backend tool suitable for setup checks; it never sends
input to the operating system.

## Open gates

The remaining migration work is validating the WebSocket adapter against a live
Chromium endpoint, replacing the remaining Python orchestration around nested
scenario expansion and table preloading, a local Rust-compatible API, platform
wheels, and PyPI installation verification. `passoflow`, `passoflow-core`,
and the `passoflow-python` crate are published; the Python wheel remains open.
See the
[roadmap](../ROADMAP.md) for the phase order and completion criteria.
