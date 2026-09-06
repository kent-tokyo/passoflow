# Scenario contract

This is the compact compatibility contract for PassoFlow `0.1.x`. The Rust
core is the destination source of truth; the Python validator remains the
runtime baseline until the compatibility bridge is complete.

## YAML shape

The root mapping accepts `title` and `steps`. `steps` is an ordered list of
step mappings. Every step has an `action` and may contain action parameters
plus these reserved editor/control keys:

`note`, `group`, `title`, `loop`, `loop_count`, and `loop_table`.

An empty `steps` list is valid for compatibility with the current Python
runner. A step without `action` is rejected during validation with the stable
`missing_action` diagnostic rather than failing YAML parsing.
Malformed YAML is returned by the CLI as an `invalid_scenario` diagnostic in
the same JSON report shape used for semantic validation.
Non-mapping entries in `steps` are reported as `invalid_step` diagnostics.
An `action` value that is not a string is reported as `invalid_action`.

Action names and required/optional parameters are maintained in the action
reference and mirrored in `passoflow-core`. Unknown actions or parameters are
validation errors. Validation diagnostics use stable Rust codes and step paths
such as `steps[3]`; diagnostic ordering is deterministic by path, code, and
message.

## Normalization

Parsing preserves YAML values. Serialization emits a deterministic YAML form:
mapping keys are ordered by the Rust model, and parsing the normalized output
must produce the same `Scenario`. This representation is intended for the
Python/UI compatibility comparison; it is not a user-facing formatting rule.

Execution plans can resolve `{{variable}}` placeholders without changing the
original plan. Resolution applies recursively to strings, lists, and mappings;
an unset variable becomes an empty string, matching the current Python runner.
The plan also exposes nested `if`/`else`/`endif` boundaries and contiguous loop
ranges as a separate control-flow view; it does not evaluate conditions or
invoke platform adapters.
Given caller-supplied decisions, the Rust core can select one branch and expand
fixed-count loops into an executable step sequence. Table-backed loops return a
runtime-data error until their rows are supplied by the engine.
`passoflow-engine::run_selected` executes that selected sequence with the same
retry, stop, and event contract as the existing linear runner.
Branch conditions are evaluated by `evaluate_branch_condition`: unset
variables are empty/false, scalar `equals` comparisons use their text form,
and `last_step` accepts `ok`, `warned`, and `failed` runtime states.

## Action outcomes

Every future engine action uses one of these outcomes:

- `success`: the action completed.
- `warning_continue`: the action reported a recoverable issue and the scenario may continue.
- `failure_stop`: the action failed and execution stops.

Results may include local-only `FailureArtifact` paths. The engine never uploads
screen content as part of this contract. Breaking changes require a new event
protocol version.

See [the Rust engine boundary](rust-engine.md) and the [action reference](actions.md)
for migration boundaries and per-action parameters.
