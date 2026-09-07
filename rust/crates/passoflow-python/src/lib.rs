//! Python access to PassoFlow's Rust scenario contracts.

use std::collections::BTreeMap;

use passoflow_core::{CONTRACT_VERSION, RetryPolicy, Scenario, action_schema, step_meta_keys};
use passoflow_engine::{
    NoopSleeper, RuntimeActionResult, RuntimeState, RuntimeStepExecutor,
    run_with_runtime_state_and_tables,
};
use passoflow_web::{BrowserBackend, CdpBrowser, JsonCdpTransport, WaitState, WebSocketCdpWire};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use serde_json::{from_str, json, to_string};

type RustDomBrowser = CdpBrowser<JsonCdpTransport<WebSocketCdpWire>>;

/// Direct Rust DOM browser access for a local Chromium CDP endpoint.
#[pyclass]
struct DomBrowser {
    inner: RustDomBrowser,
}

#[pymethods]
impl DomBrowser {
    /// Connect to a local Chromium `ws://` DevTools endpoint.
    #[new]
    fn new(endpoint: &str) -> PyResult<Self> {
        RustDomBrowser::connect(endpoint)
            .map(|inner| Self { inner })
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))
    }

    /// Navigate the connected browser page.
    fn navigate(&mut self, url: &str) -> PyResult<()> {
        self.inner
            .navigate(url)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))
    }

    /// Click the first element matching a CSS selector.
    fn click(&mut self, selector: &str) -> PyResult<()> {
        self.inner
            .click(selector)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))
    }

    /// Fill the first element matching a CSS selector.
    fn fill(&mut self, selector: &str, text: &str) -> PyResult<()> {
        self.inner
            .fill(selector, text)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))
    }

    /// Wait for a selector to reach `attached`, `detached`, `hidden`, or `visible`.
    #[pyo3(signature = (selector, state="visible", timeout_ms=10_000))]
    fn wait_for(&mut self, selector: &str, state: &str, timeout_ms: u64) -> PyResult<()> {
        let state = match state {
            "attached" => WaitState::Attached,
            "detached" => WaitState::Detached,
            "hidden" => WaitState::Hidden,
            "visible" => WaitState::Visible,
            other => {
                return Err(PyValueError::new_err(format!(
                    "invalid wait state: {other}"
                )));
            }
        };
        self.inner
            .wait_for(selector, state, timeout_ms)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))
    }

    /// Return selector match counts, samples, and conservative repair suggestions as JSON.
    fn preview_selector(&mut self, selector: &str) -> PyResult<String> {
        let preview = self
            .inner
            .preview_selector(selector)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        to_string(&preview).map_err(|error| PyRuntimeError::new_err(error.to_string()))
    }
}

/// Validate YAML and return the versioned Rust result contract as JSON.
#[pyfunction]
fn validate_yaml(yaml: &str) -> PyResult<String> {
    let scenario = Scenario::from_yaml(yaml)
        .map_err(|error| PyValueError::new_err(format!("invalid scenario YAML: {error}")))?;
    let diagnostics = scenario.validate();
    let normalized_yaml = scenario
        .to_yaml()
        .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    let plan = scenario.execution_plan();
    serde_json::to_string(&json!({
        "contract": CONTRACT_VERSION,
        "valid": diagnostics.iter().all(|diagnostic| {
            diagnostic.severity == passoflow_core::Severity::Warning
        }),
        "diagnostics": diagnostics,
        "normalized_yaml": normalized_yaml,
        "execution_plan": plan,
    }))
    .map_err(|error| PyRuntimeError::new_err(error.to_string()))
}

/// Return the deterministic normalized YAML accepted by the Rust model.
#[pyfunction]
fn normalize_yaml(yaml: &str) -> PyResult<String> {
    let scenario = Scenario::from_yaml(yaml)
        .map_err(|error| PyValueError::new_err(format!("invalid scenario YAML: {error}")))?;
    scenario
        .to_yaml()
        .map_err(|error| PyRuntimeError::new_err(error.to_string()))
}

/// Expand nested scenario steps using YAML documents supplied by the caller.
#[pyfunction]
fn expand_nested_steps(yaml: &str, scenarios_json: &str) -> PyResult<String> {
    let scenario = Scenario::from_yaml(yaml)
        .map_err(|error| PyValueError::new_err(format!("invalid scenario YAML: {error}")))?;
    let sources: BTreeMap<String, String> = from_str(scenarios_json)
        .map_err(|error| PyValueError::new_err(format!("invalid scenarios JSON: {error}")))?;
    let mut scenarios = BTreeMap::new();
    for (path, source) in sources {
        let nested = Scenario::from_yaml(&source).map_err(|error| {
            PyValueError::new_err(format!("invalid nested scenario {path:?}: {error}"))
        })?;
        scenarios.insert(path, nested);
    }
    let steps = scenario
        .expand_nested_steps(&scenarios)
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
    to_string(&steps).map_err(|error| PyRuntimeError::new_err(error.to_string()))
}

/// Return the stable scenario/event contract version.
#[pyfunction]
fn contract_version() -> &'static str {
    CONTRACT_VERSION
}

/// Return the stable action and step-metadata schema as JSON.
#[pyfunction]
fn action_schema_json() -> PyResult<String> {
    to_string(&json!({
        "contract": CONTRACT_VERSION,
        "actions": action_schema(),
        "step_meta_keys": step_meta_keys(),
    }))
    .map_err(|error| PyRuntimeError::new_err(error.to_string()))
}

struct PythonRuntimeExecutor {
    callback: Py<PyAny>,
}

impl RuntimeStepExecutor for PythonRuntimeExecutor {
    fn execute_runtime(
        &mut self,
        step: &passoflow_core::PlannedStep,
        state: &RuntimeState,
    ) -> Result<RuntimeActionResult, passoflow_engine::EngineError> {
        Python::attach(|py| {
            let step_json = to_string(step).map_err(|error| {
                passoflow_engine::EngineError::Adapter(format!("could not serialize step: {error}"))
            })?;
            let state_json = to_string(state).map_err(|error| {
                passoflow_engine::EngineError::Adapter(format!(
                    "could not serialize state: {error}"
                ))
            })?;
            let result = self
                .callback
                .bind(py)
                .call1((step_json, state_json))
                .map_err(|error| passoflow_engine::EngineError::Adapter(error.to_string()))?;
            let result_json = result
                .extract::<String>()
                .map_err(|error| passoflow_engine::EngineError::Adapter(error.to_string()))?;
            from_str(&result_json).map_err(|error| {
                passoflow_engine::EngineError::Adapter(format!(
                    "runtime callback returned invalid JSON: {error}"
                ))
            })
        })
    }
}

/// Run a Rust runtime plan while delegating platform actions to a Python callback.
///
/// The callback receives `(step_json, state_json)` and must return a JSON string
/// matching `RuntimeActionResult`. This keeps OS-specific action adapters in
/// Python while control flow, retries, variables, and events stay in Rust.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
fn run_runtime_state(
    yaml: &str,
    variables_json: &str,
    tables_json: &str,
    callback: Py<PyAny>,
    stop_callback: Option<Py<PyAny>>,
    run_id: &str,
    attempts: u32,
    interval_ms: u64,
) -> PyResult<String> {
    let scenario = Scenario::from_yaml(yaml)
        .map_err(|error| PyValueError::new_err(format!("invalid scenario YAML: {error}")))?;
    let diagnostics = scenario.validate();
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == passoflow_core::Severity::Error)
    {
        return Err(PyValueError::new_err("scenario validation failed"));
    }
    let variables: BTreeMap<String, String> = from_str(variables_json)
        .map_err(|error| PyValueError::new_err(format!("invalid variables JSON: {error}")))?;
    let tables: BTreeMap<String, Vec<BTreeMap<String, String>>> = from_str(tables_json)
        .map_err(|error| PyValueError::new_err(format!("invalid tables JSON: {error}")))?;
    let mut executor = PythonRuntimeExecutor { callback };
    let mut sleeper = NoopSleeper;
    let stop_callback = stop_callback.as_ref();
    let report = run_with_runtime_state_and_tables(
        &scenario.execution_plan(),
        variables,
        &tables,
        &mut executor,
        &mut sleeper,
        run_id,
        RetryPolicy {
            attempts,
            interval_ms,
        },
        || {
            stop_callback.is_some_and(|callback| {
                Python::attach(|py| {
                    callback
                        .bind(py)
                        .call0()
                        .and_then(|value| value.extract::<bool>())
                        .unwrap_or(false)
                })
            })
        },
    )
    .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    to_string(&report).map_err(|error| PyRuntimeError::new_err(error.to_string()))
}

/// PassoFlow's Rust contract binding module.
#[pymodule]
fn passoflow_python(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<DomBrowser>()?;
    module.add_function(wrap_pyfunction!(validate_yaml, module)?)?;
    module.add_function(wrap_pyfunction!(normalize_yaml, module)?)?;
    module.add_function(wrap_pyfunction!(expand_nested_steps, module)?)?;
    module.add_function(wrap_pyfunction!(contract_version, module)?)?;
    module.add_function(wrap_pyfunction!(action_schema_json, module)?)?;
    module.add_function(wrap_pyfunction!(run_runtime_state, module)?)?;
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{expand_nested_steps, validate_yaml};
    use serde_json::Value;

    #[test]
    fn emits_a_python_consumable_contract_result() {
        let result = validate_yaml("title: Demo\nsteps: []\n").expect("valid YAML");
        let result: Value = serde_json::from_str(&result).expect("JSON result");
        assert_eq!(result["contract"], "0.1");
        assert_eq!(result["valid"], true);
        assert!(result["execution_plan"]["steps"].is_array());
    }

    #[test]
    fn expands_nested_steps_for_the_python_bridge() {
        let expanded = expand_nested_steps(
            "steps:\n  - action: call_scenario\n    path: child.yaml\n",
            r#"{"child.yaml":"steps:\n  - action: wait\n    ms: 1\n"}"#,
        )
        .expect("nested steps should expand");
        let steps: Vec<serde_json::Value> =
            serde_json::from_str(&expanded).expect("steps are JSON");
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0]["action"], "wait");
    }
}
