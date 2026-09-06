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
| `passoflow-core` | Scenario model, YAML validation, normalization, diagnostics, execution plan | Implemented |
| `passoflow-engine` | Retry, warning/failure outcomes, stop requests, step-attempt events | Initial runner implemented |
| `passoflow-input` | Typed input events, safety checks, recording backend, Windows `user32` adapter | Windows initial adapter implemented |
| `passoflow-capture` | RGBA frames, regions, cropping, Windows GDI capture and diagnostics | Windows initial adapter implemented |
| `passoflow-vision` | Region-bounded RGB matching, candidate ordering, ambiguity guard, image executors | Initial image path implemented |
| `passoflow-python` | PyO3 binding for validation, normalization, and plans | 0.1.2 crate published; PyPI wheel publication remains open |
| `passoflow-web` | DOM operation contract and recording backend | Initial boundary implemented |
| `passoflow-server` | Future Rust local API compatible with the React editor | Planned |

## Replaced capability boundary

| Existing operation | Rust boundary |
| --- | --- |
| Mouse, click, scroll, keys, hotkeys, text | `InputBackend` and Windows `user32` adapter |
| Screen/window capture and crop | `CaptureBackend` and Windows GDI adapter |
| Template search, confidence, regions, anchors | `passoflow-vision` |
| Failure screenshots | Capture backend plus local failure-artifact policy |
| Red click indicator and fail-safe checks | PassoFlow input/overlay policy |

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
- Windows input reports virtual-desktop bounds, active-window information,
  keyboard layout, monitor geometry, best-effort DPI, and capture availability.
  Other platforms use an explicit unsupported adapter rather than a no-op.
- Image actions use exact-size RGB matching with no implicit scaling. Search
  supports candidate priority, confidence, retry, regions, anchors/offsets,
  active-window regions, target-window guards, and ambiguity-safe clicks.
- The initial Rust image executor covers `click_image` and
  `move_mouse_to_image`; the existing Python runner still executes the full
  action set.
- `ExecutionPlan::resolve_variables` provides non-mutating, recursive
  `{{variable}}` expansion for native executors. Unset variables resolve to an
  empty string for compatibility with the Python runner.
- `ExecutionPlan::control_flow` exposes deterministic nested branch boundaries
  and contiguous loop ranges without evaluating conditions or invoking an OS
  adapter.
- `ExecutionPlan::select_steps` applies caller-supplied branch decisions and
  expands fixed-count loops. Table-backed loops remain an explicit runtime-data
  boundary, and missing branch decisions fail closed.
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

The remaining migration work is selecting and prototyping the Rust DOM/Chromium
adapter on top of `passoflow-web`, moving variable and control-flow execution
behind the Rust engine, a local Rust-compatible API, platform wheels, runtime
permission guidance, and PyPI installation verification. `passoflow-core` and
the `passoflow-python` crate are published; the Python wheel remains open. See
the [roadmap](../ROADMAP.md) for the phase order and completion criteria.
