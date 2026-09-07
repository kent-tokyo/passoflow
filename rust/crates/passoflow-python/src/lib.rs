//! Python access to PassoFlow's Rust scenario contracts.

use std::collections::BTreeMap;

use passoflow_core::{CONTRACT_VERSION, RetryPolicy, Scenario};
use passoflow_engine::{
    NoopSleeper, RuntimeActionResult, RuntimeState, RuntimeStepExecutor,
    run_with_runtime_state_and_tables,
};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use serde_json::{from_str, json, to_string};

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

/// Return the stable scenario/event contract version.
#[pyfunction]
fn contract_version() -> &'static str {
    CONTRACT_VERSION
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
    module.add_function(wrap_pyfunction!(validate_yaml, module)?)?;
    module.add_function(wrap_pyfunction!(normalize_yaml, module)?)?;
    module.add_function(wrap_pyfunction!(contract_version, module)?)?;
    module.add_function(wrap_pyfunction!(run_runtime_state, module)?)?;
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_yaml;
    use serde_json::Value;

    #[test]
    fn emits_a_python_consumable_contract_result() {
        let result = validate_yaml("title: Demo\nsteps: []\n").expect("valid YAML");
        let result: Value = serde_json::from_str(&result).expect("JSON result");
        assert_eq!(result["contract"], "0.1");
        assert_eq!(result["valid"], true);
        assert!(result["execution_plan"]["steps"].is_array());
    }
}
