//! Stable, platform-independent scenario contracts and validation for `PassoFlow`.
//!
//! Platform input, screen capture, image recognition, and browser adapters belong
//! in separate crates and must not leak into this scenario model.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use thiserror::Error;

/// Version of the stable scenario/event contract, independent of package version.
pub const CONTRACT_VERSION: &str = "0.1";

const META_KEYS: &[&str] = &[
    "action",
    "note",
    "group",
    "title",
    "loop",
    "loop_count",
    "loop_table",
];

/// A YAML scenario. Unknown root keys are rejected during validation rather than
/// silently discarded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    #[serde(default)]
    pub title: String,
    pub steps: Vec<Step>,
}

/// One executable or structural scenario step. Parameters are preserved as YAML
/// values so existing scenarios retain their types and formatting semantics.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Step {
    #[serde(default)]
    pub action: String,
    #[serde(flatten)]
    pub params: BTreeMap<String, Value>,
    #[serde(skip)]
    malformed_shape: bool,
    #[serde(skip)]
    malformed_action: bool,
}

impl<'de> Deserialize<'de> for Step {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let Value::Mapping(mapping) = value else {
            return Ok(Self {
                action: String::new(),
                params: BTreeMap::new(),
                malformed_shape: true,
                malformed_action: false,
            });
        };
        let mut action = String::new();
        let mut params = BTreeMap::new();
        let mut malformed_action = false;
        for (key, value) in mapping {
            let Some(name) = key.as_str() else {
                continue;
            };
            if name == "action" {
                if let Some(value) = value.as_str() {
                    value.clone_into(&mut action);
                } else {
                    malformed_action = true;
                }
            } else {
                params.insert(name.to_owned(), value);
            }
        }
        Ok(Self {
            action,
            params,
            malformed_shape: false,
            malformed_action,
        })
    }
}

/// Severity used by the compatibility diagnostic stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

/// A stable, machine-readable validation result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub path: String,
    pub message: String,
}

/// Outcome contract shared by actions, the Python binding, and the local UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionOutcome {
    Success,
    WarningContinue,
    FailureStop,
}

/// A local artifact retained for inspection or guarded recovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureArtifact {
    pub kind: String,
    pub path: String,
}

/// Result returned by one action without coupling the core to an OS backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionResult {
    pub outcome: ActionOutcome,
    pub message: String,
    #[serde(default)]
    pub artifacts: Vec<FailureArtifact>,
}

/// Retry policy carried by the future Rust engine boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub attempts: u32,
    pub interval_ms: u64,
}

/// One deterministic, platform-independent entry in an execution plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedStep {
    pub index: u32,
    pub action: String,
    /// Normalized action parameters needed by native executors.
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
    pub branch_depth: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_table: Option<String>,
    #[serde(default)]
    pub variables_defined: Vec<String>,
    #[serde(default)]
    pub variables_referenced: Vec<String>,
}

/// Stable planning output consumed by future engine and binding adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub contract: String,
    pub steps: Vec<PlannedStep>,
}

/// The resolved boundaries of structural control-flow blocks in a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlFlowPlan {
    pub branches: Vec<BranchBoundary>,
    pub loops: Vec<LoopBoundary>,
}

/// One `if`/`else`/`endif` range using one-based plan step numbers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchBoundary {
    pub if_step: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub else_step: Option<u32>,
    pub endif_step: u32,
}

/// One contiguous loop range using one-based plan step numbers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopBoundary {
    pub label: String,
    pub start_step: u32,
    pub end_step: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
}

impl ExecutionPlan {
    /// Extract deterministic branch and loop ranges without evaluating them.
    #[must_use]
    pub fn control_flow(&self) -> ControlFlowPlan {
        let mut branches = Vec::new();
        let mut branch_stack: Vec<(u32, Option<u32>)> = Vec::new();
        let mut loops = Vec::new();
        let mut index = 0;
        while index < self.steps.len() {
            let step = &self.steps[index];
            match step.action.as_str() {
                "if" => branch_stack.push((step.index, None)),
                "else" => {
                    if let Some((_, else_step)) = branch_stack.last_mut() {
                        *else_step = Some(step.index);
                    }
                }
                "endif" => {
                    if let Some((if_step, else_step)) = branch_stack.pop() {
                        branches.push(BranchBoundary {
                            if_step,
                            else_step,
                            endif_step: step.index,
                        });
                    }
                }
                _ => {}
            }

            if let Some(label) = step.loop_label.as_deref() {
                let start_step = step.index;
                let mut end_index = index;
                while self
                    .steps
                    .get(end_index + 1)
                    .and_then(|next| next.loop_label.as_deref())
                    == Some(label)
                {
                    end_index += 1;
                }
                loops.push(LoopBoundary {
                    label: label.to_owned(),
                    start_step,
                    end_step: self.steps[end_index].index,
                    count: step.loop_count,
                    table: step.loop_table.clone(),
                });
                index = end_index;
            }
            index += 1;
        }
        branches.sort_by_key(|branch| branch.if_step);
        ControlFlowPlan { branches, loops }
    }

    /// Resolve `{{variable}}` placeholders in action parameters.
    ///
    /// Resolution is non-mutating: the structural plan remains reusable for
    /// retries and dry runs. Missing variables resolve to an empty string,
    /// matching the current Python runner.
    #[must_use]
    pub fn resolve_variables(&self, variables: &BTreeMap<String, String>) -> Self {
        Self {
            contract: self.contract.clone(),
            steps: self
                .steps
                .iter()
                .map(|step| PlannedStep {
                    params: step
                        .params
                        .iter()
                        .map(|(key, value)| (key.clone(), resolve_value(value, variables)))
                        .collect(),
                    ..step.clone()
                })
                .collect(),
        }
    }
}

/// Versioned local event emitted by the future engine and binding adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunEvent {
    pub protocol: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub run_id: String,
    pub step: u32,
    pub action: String,
    pub outcome: ActionOutcome,
    pub message: String,
    #[serde(default)]
    pub artifacts: Vec<FailureArtifact>,
}

/// Parsing or serialization failure.
#[derive(Debug, Error)]
pub enum ScenarioError {
    #[error("invalid scenario YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

impl Scenario {
    /// Parse a scenario from YAML without performing semantic validation.
    /// # Errors
    ///
    /// Returns an error when the input is not valid YAML for a scenario.
    pub fn from_yaml(yaml: &str) -> Result<Self, ScenarioError> {
        Ok(serde_yaml::from_str(yaml)?)
    }

    /// Validate action names, parameters, and the value constraints shared by
    /// the current Python runner. Diagnostics are deterministic by path/code.
    #[must_use]
    pub fn validate(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for (index, step) in self.steps.iter().enumerate() {
            validate_step(step, index + 1, &mut diagnostics);
        }
        validate_loops(&self.steps, &mut diagnostics);
        validate_if_blocks(&self.steps, &mut diagnostics);
        diagnostics
            .sort_by(|a, b| (&a.path, &a.code, &a.message).cmp(&(&b.path, &b.code, &b.message)));
        diagnostics
    }

    /// True when semantic validation found no errors. Warnings do not block a run.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.validate()
            .iter()
            .all(|item| item.severity != Severity::Error)
    }

    /// Serialize a normalized scenario representation for Python/UI comparison.
    /// # Errors
    ///
    /// Returns an error if the scenario cannot be serialized as YAML.
    pub fn to_yaml(&self) -> Result<String, ScenarioError> {
        Ok(serde_yaml::to_string(self)?)
    }

    /// Build a deterministic structural plan without invoking any OS adapter.
    /// # Panics
    ///
    /// Panics only when a scenario contains more than `u32::MAX` steps, which
    /// cannot be represented by the versioned plan contract.
    #[must_use]
    pub fn execution_plan(&self) -> ExecutionPlan {
        let mut branch_depth: u32 = 0;
        let steps = self
            .steps
            .iter()
            .enumerate()
            .map(|(index, step)| {
                let current_depth = branch_depth;
                if step.action == "endif" {
                    branch_depth = branch_depth.saturating_sub(1);
                }
                let plan = PlannedStep {
                    index: u32::try_from(index + 1)
                        .expect("scenario cannot contain more than u32::MAX steps"),
                    action: step.action.clone(),
                    params: step.params.clone(),
                    branch_depth: current_depth,
                    loop_label: step
                        .params
                        .get("loop")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    loop_count: step.params.get("loop_count").and_then(Value::as_i64),
                    loop_table: step
                        .params
                        .get("loop_table")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    variables_defined: variable_definitions(step),
                    variables_referenced: variable_references(step),
                };
                if step.action == "if" {
                    branch_depth = branch_depth.saturating_add(1);
                }
                plan
            })
            .collect();
        ExecutionPlan {
            contract: CONTRACT_VERSION.to_owned(),
            steps,
        }
    }
}

fn variable_definitions(step: &Step) -> Vec<String> {
    if matches!(
        step.action.as_str(),
        "set_variable"
            | "concat_variable"
            | "set_year_month_variable"
            | "set_year_month_day_variable"
            | "set_month_start_variable"
            | "set_month_end_variable"
            | "get_excel_value"
    ) {
        step.params
            .get("name")
            .and_then(Value::as_str)
            .map(|name| vec![name.to_owned()])
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn variable_references(step: &Step) -> Vec<String> {
    let mut references = BTreeSet::new();
    if step.action == "paste_variable" || step.action == "if" {
        if let Some(name) = step
            .params
            .get("name")
            .or_else(|| step.params.get("variable"))
            .and_then(Value::as_str)
        {
            references.insert(name.to_owned());
        }
    }
    for value in step.params.values() {
        collect_variable_references(value, &mut references);
    }
    references.into_iter().collect()
}

fn collect_variable_references(value: &Value, references: &mut BTreeSet<String>) {
    match value {
        Value::String(text) => {
            let mut remainder = text.as_str();
            while let Some(start) = remainder.find("{{") {
                let after_start = &remainder[start + 2..];
                let Some(end) = after_start.find("}}") else {
                    break;
                };
                let name = after_start[..end].trim();
                if !name.is_empty() {
                    references.insert(name.to_owned());
                }
                remainder = &after_start[end + 2..];
            }
        }
        Value::Sequence(items) => {
            for item in items {
                collect_variable_references(item, references);
            }
        }
        Value::Mapping(items) => {
            for (key, value) in items {
                collect_variable_references(key, references);
                collect_variable_references(value, references);
            }
        }
        _ => {}
    }
}

fn resolve_value(value: &Value, variables: &BTreeMap<String, String>) -> Value {
    match value {
        Value::String(text) => Value::String(resolve_text(text, variables)),
        Value::Sequence(items) => Value::Sequence(
            items
                .iter()
                .map(|item| resolve_value(item, variables))
                .collect(),
        ),
        Value::Mapping(items) => Value::Mapping(
            items
                .iter()
                .map(|(key, value)| {
                    (
                        resolve_value(key, variables),
                        resolve_value(value, variables),
                    )
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

fn resolve_text(text: &str, variables: &BTreeMap<String, String>) -> String {
    let mut output = String::with_capacity(text.len());
    let mut remainder = text;
    while let Some(start) = remainder.find("{{") {
        output.push_str(&remainder[..start]);
        let after_start = &remainder[start + 2..];
        let Some(end) = after_start.find("}}") else {
            output.push_str(&remainder[start..]);
            break;
        };
        let name = after_start[..end].trim();
        output.push_str(variables.get(name).map(String::as_str).unwrap_or_default());
        remainder = &after_start[end + 2..];
    }
    if !remainder.is_empty() && !remainder.contains("{{") {
        output.push_str(remainder);
    }
    output
}

fn error(code: &str, path: &str, message: &str) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        code: code.to_owned(),
        path: path.to_owned(),
        message: message.to_owned(),
    }
}

fn warning(code: &str, path: &str, message: &str) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        code: code.to_owned(),
        path: path.to_owned(),
        message: message.to_owned(),
    }
}

#[allow(clippy::match_same_arms)]
fn schema(action: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    Some(match action {
        "start" | "end" | "else" | "endif" | "paste" | "clear_input" | "open_new_excel" => {
            (&[], &[])
        }
        "wait" => (&["ms"], &[]),
        "if" => (&[], &["variable", "equals", "last_step"]),
        "call_scenario" => (&["path"], &[]),
        "repeat" => (&["path", "count"], &[]),
        "send_webhook" => (&["url"], &["method", "payload", "on_error"]),
        "set_variable" | "concat_variable" => (&["name", "value"], &[]),
        "set_year_month_variable"
        | "set_year_month_day_variable"
        | "set_month_start_variable"
        | "set_month_end_variable" => (&["name"], &["days_offset", "months_offset"]),
        "type_text" | "set_clipboard" => (&["text"], &[]),
        "paste_variable" => (&["name"], &[]),
        "press_key" => (&["key"], &["wait"]),
        "hotkey" => (&["keys"], &[]),
        "launch_app" => (
            &["path"],
            &["args", "wait_for_window", "startup_timeout_ms"],
        ),
        "rename_file" => (&["path", "new_name"], &[]),
        "move_file" => (&["path", "destination"], &[]),
        "copy_file" => (&["path", "destination"], &["if_destination_newer"]),
        "map_network_drive" => (&["drive", "path"], &[]),
        "open_excel_file" => (&["path"], &[]),
        "get_excel_value" => (&["path", "cell", "name"], &["sheet"]),
        "set_excel_value" => (&["path", "cell", "value"], &["sheet"]),
        "save_excel_file" => (&["path"], &[]),
        "create_excel_sheet" | "delete_excel_sheet" => (&["path", "sheet"], &[]),
        "delete_excel_row" => (&["path", "row"], &["sheet"]),
        "sort_excel_range" => (&["path", "range", "key_cell"], &["sheet", "order"]),
        "run_excel_macro" => (&["path", "macro"], &["args"]),
        "load_table" => (&["path", "name"], &["sheet", "columns", "selected_rows"]),
        "activate_window" => (&["title_contains"], &["retry", "retry_interval_ms"]),
        "open_url" => (&["url"], &[]),
        "browser_navigate" => (&["url"], &["timeout_ms"]),
        "browser_click" => (&["selector"], &["timeout_ms"]),
        "browser_fill" => (&["selector", "text"], &["timeout_ms"]),
        "browser_wait_for" => (&["selector"], &["state", "timeout_ms"]),
        "move_mouse_to_image" => (&["images"], IMAGE_OPTIONS),
        "click_image" => (&["images"], CLICK_IMAGE_OPTIONS),
        _ => return None,
    })
}

const IMAGE_OPTIONS: &[&str] = &[
    "retry",
    "retry_interval_ms",
    "confidence",
    "offset",
    "position",
    "region",
    "region_origin",
    "target_window_title",
];
const CLICK_IMAGE_OPTIONS: &[&str] = &[
    "retry",
    "retry_interval_ms",
    "confidence",
    "offset",
    "position",
    "region",
    "region_origin",
    "target_window_title",
    "click_type",
    "click_indicator_duration",
];
const POSITIONS: &[&str] = &[
    "center",
    "top",
    "bottom",
    "left",
    "right",
    "top-left",
    "top-right",
    "bottom-left",
    "bottom-right",
];
const REGION_ORIGINS: &[&str] = &["screen", "active_window"];
const COPY_NEWER_MODES: &[&str] = &["overwrite", "skip"];

#[allow(clippy::too_many_lines)]
fn validate_step(step: &Step, number: usize, diagnostics: &mut Vec<Diagnostic>) {
    let path = format!("steps[{number}]");
    if step.malformed_shape {
        diagnostics.push(error("invalid_step", &path, "step must be a mapping"));
        return;
    }
    if step.malformed_action {
        diagnostics.push(error("invalid_action", &path, "'action' must be a string"));
        return;
    }
    if step.action.is_empty() {
        diagnostics.push(error(
            "missing_action",
            &path,
            "missing required key 'action'",
        ));
        return;
    }
    let Some((required, optional)) = schema(&step.action) else {
        diagnostics.push(error(
            "unknown_action",
            &path,
            &format!("unknown action '{}'", step.action),
        ));
        return;
    };
    let required_set: BTreeSet<&str> = required.iter().copied().collect();
    let optional_set: BTreeSet<&str> = optional.iter().copied().collect();
    for name in required_set.difference(&step.params.keys().map(String::as_str).collect()) {
        diagnostics.push(error(
            "missing_parameter",
            &path,
            &format!("missing required parameter '{name}'"),
        ));
    }
    for name in step.params.keys() {
        if !optional_set.contains(name.as_str())
            && !META_KEYS.contains(&name.as_str())
            && !required_set.contains(name.as_str())
        {
            diagnostics.push(error(
                "unknown_parameter",
                &path,
                &format!("unknown parameter '{name}'"),
            ));
        }
    }
    if let Some(value) = step.params.get("keys") {
        if !value.is_sequence() {
            diagnostics.push(error("invalid_keys", &path, "'keys' must be a list"));
        }
    }
    if let Some(value) = step.params.get("offset") {
        if !is_integer_sequence(value, 2) {
            diagnostics.push(error(
                "invalid_offset",
                &path,
                "'offset' must be a 2-element integer list [x, y]",
            ));
        }
    }
    if let Some(value) = step.params.get("region") {
        if !is_integer_sequence(value, 4) {
            diagnostics.push(error(
                "invalid_region",
                &path,
                "'region' must be a 4-element integer list",
            ));
        } else if let Some(items) = value.as_sequence() {
            if items[2].as_i64().is_none_or(|width| width <= 0)
                || items[3].as_i64().is_none_or(|height| height <= 0)
            {
                diagnostics.push(error(
                    "invalid_region_size",
                    &path,
                    "'region' width and height must be positive",
                ));
            }
        }
    }
    if let Some(value) = step.params.get("position") {
        if !value
            .as_str()
            .is_some_and(|position| POSITIONS.contains(&position))
        {
            diagnostics.push(error(
                "invalid_position",
                &path,
                "'position' is not a supported image anchor",
            ));
        }
    }
    if let Some(value) = step.params.get("region_origin") {
        if !value
            .as_str()
            .is_some_and(|origin| REGION_ORIGINS.contains(&origin))
        {
            diagnostics.push(error(
                "invalid_region_origin",
                &path,
                "'region_origin' must be screen or active_window",
            ));
        }
    }
    if let Some(value) = step.params.get("confidence") {
        if value
            .as_f64()
            .is_none_or(|confidence| !(0.0..=1.0).contains(&confidence))
        {
            diagnostics.push(error(
                "invalid_confidence",
                &path,
                "'confidence' must be a number from 0 to 1",
            ));
        }
    }
    for name in [
        "retry",
        "retry_interval_ms",
        "click_indicator_duration",
        "startup_timeout_ms",
    ] {
        if let Some(value) = step.params.get(name) {
            if value.as_f64().is_none_or(|number| number < 0.0) {
                diagnostics.push(error(
                    "invalid_non_negative_number",
                    &path,
                    &format!("'{name}' must be non-negative"),
                ));
            }
        }
    }
    if step.action == "repeat" {
        if let Some(value) = step.params.get("count") {
            if value.as_i64().is_none_or(|count| count <= 0) {
                diagnostics.push(error(
                    "invalid_repeat_count",
                    &path,
                    "'count' must be a positive integer",
                ));
            }
        }
    }
    if step.action == "browser_wait_for" {
        if let Some(value) = step.params.get("state") {
            if !matches!(
                value.as_str(),
                Some("attached" | "detached" | "hidden" | "visible")
            ) {
                diagnostics.push(error(
                    "invalid_wait_state",
                    &path,
                    "'state' must be attached, detached, hidden, or visible",
                ));
            }
        }
    }
    if matches!(
        step.action.as_str(),
        "browser_navigate" | "browser_click" | "browser_fill" | "browser_wait_for"
    ) {
        if let Some(value) = step.params.get("timeout_ms") {
            if value.as_i64().is_none_or(|timeout| timeout <= 0) {
                diagnostics.push(error(
                    "invalid_timeout",
                    &path,
                    "'timeout_ms' must be a positive integer",
                ));
            }
        }
    }
    if step.action == "send_webhook" {
        if let Some(value) = step.params.get("payload") {
            if !value.is_mapping() && value.as_str().is_none() {
                diagnostics.push(error(
                    "invalid_payload",
                    &path,
                    "'payload' must be a mapping or a JSON string",
                ));
            }
        }
        if let Some(value) = step.params.get("on_error") {
            if !matches!(value.as_str(), Some("continue" | "stop")) {
                diagnostics.push(error(
                    "invalid_on_error",
                    &path,
                    "'on_error' must be continue or stop",
                ));
            }
        }
    }
    if step.action == "click_image" {
        if let Some(value) = step.params.get("click_type") {
            if !matches!(value.as_str(), Some("single" | "double")) {
                diagnostics.push(error(
                    "invalid_click_type",
                    &path,
                    "'click_type' must be single or double",
                ));
            }
        }
    }
    if step.action == "copy_file" {
        if let Some(value) = step.params.get("if_destination_newer") {
            if !value
                .as_str()
                .is_some_and(|mode| COPY_NEWER_MODES.contains(&mode))
            {
                diagnostics.push(error(
                    "invalid_copy_mode",
                    &path,
                    "'if_destination_newer' must be overwrite or skip",
                ));
            }
        }
    }
    if step.action == "if" {
        let has_variable = step.params.contains_key("variable");
        let has_last_step = step.params.contains_key("last_step");
        if has_variable == has_last_step {
            diagnostics.push(error(
                "invalid_condition",
                &path,
                "exactly one of variable or last_step is required",
            ));
        }
        if let Some(value) = step.params.get("last_step") {
            if !matches!(value.as_str(), Some("ok" | "warned")) {
                diagnostics.push(error(
                    "invalid_last_step",
                    &path,
                    "'last_step' must be ok or warned",
                ));
            }
        }
    }
    if step.action == "move_mouse_to_image" || step.action == "click_image" {
        if let Some(value) = step.params.get("images") {
            if !value.is_sequence() {
                diagnostics.push(error("invalid_images", &path, "'images' must be a list"));
            }
        }
    }
    if step.action == "click_image" && step.params.contains_key("click_indicator_duration") {
        let valid = step.params["click_indicator_duration"]
            .as_f64()
            .is_some_and(|value| value >= 0.0);
        if !valid {
            diagnostics.push(error(
                "invalid_duration",
                &path,
                "'click_indicator_duration' must be non-negative",
            ));
        }
    }
    if step.action == "activate_window" && !step.params.contains_key("title_contains") {
        diagnostics.push(warning(
            "missing_window_title",
            &path,
            "activate_window needs a window title to be reliable",
        ));
    }
}

fn is_integer_sequence(value: &Value, expected_len: usize) -> bool {
    value.as_sequence().is_some_and(|items| {
        items.len() == expected_len && items.iter().all(|item| item.as_i64().is_some())
    })
}

fn validate_loops(steps: &[Step], diagnostics: &mut Vec<Diagnostic>) {
    let mut seen = BTreeSet::new();
    let mut current: Option<String> = None;
    let mut current_count: Option<Value> = None;
    let mut current_table: Option<Value> = None;
    for (index, step) in steps.iter().enumerate() {
        let label = step
            .params
            .get("loop")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if label != current {
            if let Some(label) = label.as_ref() {
                if !seen.insert(label.clone()) {
                    diagnostics.push(error(
                        "non_contiguous_loop",
                        &format!("steps[{}]", index + 1),
                        &format!("loop '{label}' steps must be contiguous"),
                    ));
                }
                current_count = step.params.get("loop_count").cloned();
                current_table = step.params.get("loop_table").cloned();
            } else {
                current_count = None;
                current_table = None;
            }
            current = label;
        } else if current.is_some()
            && (step.params.get("loop_count") != current_count.as_ref()
                || step.params.get("loop_table") != current_table.as_ref())
        {
            diagnostics.push(error(
                "inconsistent_loop",
                &format!("steps[{}]", index + 1),
                "steps in one loop must use the same loop_count or loop_table",
            ));
        }

        if let Some(label) = current.as_ref() {
            let has_count = step
                .params
                .get("loop_count")
                .and_then(Value::as_i64)
                .is_some_and(|count| count > 0);
            let has_table = step
                .params
                .get("loop_table")
                .and_then(Value::as_str)
                .is_some_and(|table| !table.is_empty());
            if has_count && has_table {
                diagnostics.push(error(
                    "ambiguous_loop",
                    &format!("steps[{}]", index + 1),
                    &format!("loop '{label}' cannot have both loop_count and loop_table"),
                ));
            } else if !has_count && !has_table {
                diagnostics.push(error(
                    "missing_loop_control",
                    &format!("steps[{}]", index + 1),
                    &format!("loop '{label}' requires a positive loop_count or a loop_table name"),
                ));
            }
        }
    }
}

fn validate_if_blocks(steps: &[Step], diagnostics: &mut Vec<Diagnostic>) {
    let mut stack: Vec<(usize, bool)> = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        match step.action.as_str() {
            "if" => stack.push((index + 1, false)),
            "else" => match stack.last_mut() {
                None => diagnostics.push(error(
                    "orphan_else",
                    &format!("steps[{}]", index + 1),
                    "'else' has no matching 'if'",
                )),
                Some((_, has_else)) if *has_else => diagnostics.push(error(
                    "duplicate_else",
                    &format!("steps[{}]", index + 1),
                    "only one 'else' is allowed for each 'if'",
                )),
                Some((_, has_else)) => *has_else = true,
            },
            "endif" if stack.pop().is_none() => diagnostics.push(error(
                "orphan_endif",
                &format!("steps[{}]", index + 1),
                "'endif' has no matching 'if'",
            )),
            _ => {}
        }
    }
    for (index, _) in stack {
        diagnostics.push(error(
            "missing_endif",
            &format!("steps[{index}]"),
            "'if' has no matching 'endif'",
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        ActionOutcome, ActionResult, BranchBoundary, CONTRACT_VERSION, LoopBoundary, RunEvent,
        Scenario, Severity,
    };
    use serde_yaml::Value;

    #[test]
    fn validates_existing_style_scenario() {
        let scenario = Scenario::from_yaml("title: Login\nsteps:\n  - action: browser_click\n    selector: '#login'\n  - action: click_image\n    images: [button.png]\n    confidence: 0.9\n").expect("YAML should parse");
        assert!(scenario.is_valid());
    }

    #[test]
    fn reports_unknown_and_missing_parameters_deterministically() {
        let scenario = Scenario::from_yaml("steps:\n  - action: browser_click\n    wrong: true\n")
            .expect("YAML should parse");
        let diagnostics = scenario.validate();
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].severity, Severity::Error);
        assert_eq!(diagnostics[0].code, "missing_parameter");
        assert_eq!(diagnostics[1].code, "unknown_parameter");
    }

    #[test]
    fn accepts_empty_scenarios_and_reports_missing_actions() {
        let empty = Scenario::from_yaml("steps: []\n").expect("YAML should parse");
        assert!(empty.is_valid());

        let missing_action = Scenario::from_yaml("steps:\n  - title: unnamed\n")
            .expect("missing action should be validated, not rejected by parsing");
        let diagnostics = missing_action.validate();
        assert_eq!(diagnostics[0].code, "missing_action");

        let invalid_action = Scenario::from_yaml("steps:\n  - action: 42\n")
            .expect("invalid action type should be validated, not rejected by parsing");
        assert_eq!(invalid_action.validate()[0].code, "invalid_action");
    }

    #[test]
    fn validates_loop_contiguity_and_if_nesting() {
        let scenario = Scenario::from_yaml(
            "steps:\n  - action: if\n    last_step: ok\n  - action: else\n  - action: endif\n  - action: wait\n    ms: 1\n    loop: work\n    loop_count: 2\n  - action: wait\n    ms: 1\n    loop: work\n    loop_count: 2\n",
        )
        .expect("YAML should parse");
        assert!(scenario.is_valid());

        let invalid = Scenario::from_yaml(
            "steps:\n  - action: if\n    last_step: ok\n  - action: else\n  - action: else\n  - action: wait\n    ms: 1\n    loop: work\n",
        )
        .expect("YAML should parse");
        let codes: Vec<_> = invalid
            .validate()
            .into_iter()
            .map(|item| item.code)
            .collect();
        assert!(codes.contains(&"duplicate_else".to_owned()));
        assert!(codes.contains(&"missing_loop_control".to_owned()));
        assert!(codes.contains(&"missing_endif".to_owned()));
    }

    #[test]
    fn emits_deterministic_normalized_yaml_for_compatibility() {
        let scenario = Scenario::from_yaml(
            "steps:\n  - action: browser_click\n    timeout_ms: 1500\n    selector: '#login'\ntitle: Login\n",
        )
        .expect("YAML should parse");
        let normalized = scenario.to_yaml().expect("scenario should serialize");
        assert_eq!(
            normalized,
            "title: Login\nsteps:\n- action: browser_click\n  selector: '#login'\n  timeout_ms: 1500\n"
        );
        assert_eq!(
            Scenario::from_yaml(&normalized).expect("normalized YAML should parse"),
            scenario
        );
    }

    #[test]
    fn serializes_the_versioned_action_event_contract() {
        let event = RunEvent {
            protocol: "passoflow.run.v1".to_owned(),
            event_type: "action_finished".to_owned(),
            run_id: "local-run".to_owned(),
            step: 3,
            action: "click_image".to_owned(),
            outcome: ActionOutcome::Success,
            message: "Matched image candidate 1".to_owned(),
            artifacts: Vec::new(),
        };
        let json = serde_json::to_string(&event).expect("event should serialize");
        assert!(json.contains("\"type\":\"action_finished\""));
        assert!(json.contains("\"outcome\":\"success\""));
        assert_eq!(CONTRACT_VERSION, "0.1");

        let result = ActionResult {
            outcome: ActionOutcome::WarningContinue,
            message: "window title was not confirmed".to_owned(),
            artifacts: Vec::new(),
        };
        assert_eq!(
            serde_json::to_value(result).expect("result should serialize")["outcome"],
            "warning_continue"
        );
    }

    #[test]
    fn builds_a_deterministic_plan_for_branches_loops_and_variables() {
        let scenario = Scenario::from_yaml(
            "steps:\n  - action: set_variable\n    name: token\n    value: abc\n  - action: if\n    variable: token\n  - action: wait\n    ms: 1\n    loop: retry\n    loop_count: 2\n    note: '{{token}}'\n  - action: endif\n",
        )
        .expect("scenario should parse");
        let plan = scenario.execution_plan();
        assert_eq!(plan.contract, CONTRACT_VERSION);
        assert_eq!(plan.steps[0].variables_defined, vec!["token"]);
        assert_eq!(plan.steps[1].branch_depth, 0);
        assert_eq!(plan.steps[2].branch_depth, 1);
        assert_eq!(plan.steps[2].loop_label.as_deref(), Some("retry"));
        assert_eq!(plan.steps[2].loop_count, Some(2));
        assert_eq!(plan.steps[2].variables_referenced, vec!["token"]);
        assert_eq!(plan.steps[3].branch_depth, 1);
        assert_eq!(
            plan.steps[0].params["name"],
            Value::String("token".to_owned())
        );
    }

    #[test]
    fn resolves_nested_plan_parameters_without_mutating_the_source_plan() {
        let scenario = Scenario::from_yaml(
            "steps:\n  - action: send_webhook\n    url: 'https://example.test/{{ token }}'\n    payload:\n      message: 'Hello {{ name }}'\n      tags: ['{{ missing }}', fixed]\n",
        )
        .expect("scenario should parse");
        let plan = scenario.execution_plan();
        let variables = BTreeMap::from([
            ("token".to_owned(), "abc".to_owned()),
            ("name".to_owned(), "PassoFlow".to_owned()),
        ]);

        let resolved = plan.resolve_variables(&variables);
        assert_eq!(
            resolved.steps[0].params["url"],
            Value::String("https://example.test/abc".to_owned())
        );
        assert_eq!(
            resolved.steps[0].params["payload"]["message"],
            Value::String("Hello PassoFlow".to_owned())
        );
        assert_eq!(
            resolved.steps[0].params["payload"]["tags"][0],
            Value::String(String::new())
        );
        assert_eq!(
            plan.steps[0].params["url"],
            Value::String("https://example.test/{{ token }}".to_owned())
        );
    }

    #[test]
    fn extracts_nested_branch_and_contiguous_loop_boundaries() {
        let scenario = Scenario::from_yaml(
            "steps:\n  - action: if\n    variable: ready\n  - action: wait\n    ms: 1\n  - action: if\n    variable: nested\n  - action: noop\n  - action: endif\n  - action: else\n  - action: wait\n    ms: 2\n  - action: endif\n  - action: click_image\n    images: [one.png]\n    loop: retry\n    loop_count: 3\n  - action: wait\n    ms: 1\n    loop: retry\n    loop_count: 3\n",
        )
        .expect("scenario should parse");
        let control_flow = scenario.execution_plan().control_flow();
        assert_eq!(
            control_flow.branches,
            vec![
                BranchBoundary {
                    if_step: 1,
                    else_step: Some(6),
                    endif_step: 8,
                },
                BranchBoundary {
                    if_step: 3,
                    else_step: None,
                    endif_step: 5,
                },
            ]
        );
        assert_eq!(
            control_flow.loops,
            vec![LoopBoundary {
                label: "retry".to_owned(),
                start_step: 9,
                end_step: 10,
                count: Some(3),
                table: None,
            }]
        );
    }
}
