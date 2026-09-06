//! Python access to PassoFlow's Rust scenario contracts.

use passoflow_core::{CONTRACT_VERSION, Scenario};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use serde_json::json;

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

/// PassoFlow's Rust contract binding module.
#[pymodule]
fn passoflow_python(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(validate_yaml, module)?)?;
    module.add_function(wrap_pyfunction!(normalize_yaml, module)?)?;
    module.add_function(wrap_pyfunction!(contract_version, module)?)?;
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
