use std::process::Command;

use passoflow_core::Scenario;

const VALID: &str = include_str!("../../../../tests/fixtures/validator_compat/valid_control.yaml");
const UNKNOWN: &str =
    include_str!("../../../../tests/fixtures/validator_compat/unknown_action.yaml");
const MISSING: &str =
    include_str!("../../../../tests/fixtures/validator_compat/missing_parameter.yaml");
const INVALID: &str =
    include_str!("../../../../tests/fixtures/validator_compat/invalid_control.yaml");
const INVALID_VALUES: &str =
    include_str!("../../../../tests/fixtures/validator_compat/invalid_values.yaml");
const INVALID_WEB_VALUES: &str =
    include_str!("../../../../tests/fixtures/validator_compat/invalid_web_values.yaml");
const EXPECTED: &str =
    include_str!("../../../../tests/fixtures/validator_compat/expected_diagnostics.json");

#[test]
fn shared_fixture_matches_expected_validity() {
    assert!(Scenario::from_yaml(VALID)
        .expect("valid fixture parses")
        .is_valid());
    assert!(!Scenario::from_yaml(UNKNOWN)
        .expect("unknown fixture parses")
        .is_valid());
    assert!(!Scenario::from_yaml(MISSING)
        .expect("missing fixture parses")
        .is_valid());
    assert!(!Scenario::from_yaml(INVALID)
        .expect("invalid fixture parses")
        .is_valid());
    assert!(!Scenario::from_yaml(INVALID_VALUES)
        .expect("invalid values fixture parses")
        .is_valid());
    assert!(!Scenario::from_yaml(INVALID_WEB_VALUES)
        .expect("invalid web values fixture parses")
        .is_valid());
}

#[test]
fn shared_fixture_keeps_stable_diagnostic_codes() {
    let codes: Vec<_> = Scenario::from_yaml(UNKNOWN)
        .expect("fixture parses")
        .validate()
        .into_iter()
        .map(|item| item.code)
        .collect();
    assert_eq!(codes, vec!["unknown_action"]);

    let codes: Vec<_> = Scenario::from_yaml(MISSING)
        .expect("fixture parses")
        .validate()
        .into_iter()
        .map(|item| item.code)
        .collect();
    assert_eq!(codes, vec!["missing_parameter"]);
}

#[test]
fn shared_value_fixture_reports_image_safety_constraints() {
    let codes: Vec<_> = Scenario::from_yaml(INVALID_VALUES)
        .expect("fixture parses")
        .validate()
        .into_iter()
        .map(|item| item.code)
        .collect();
    for expected in [
        "invalid_images",
        "invalid_offset",
        "invalid_region_size",
        "invalid_position",
        "invalid_region_origin",
        "invalid_confidence",
        "invalid_non_negative_number",
        "invalid_click_type",
    ] {
        assert!(
            codes.contains(&expected.to_owned()),
            "missing {expected}: {codes:?}"
        );
    }
}

#[test]
fn shared_web_fixture_reports_timeout_and_payload_constraints() {
    let codes: Vec<_> = Scenario::from_yaml(INVALID_WEB_VALUES)
        .expect("fixture parses")
        .validate()
        .into_iter()
        .map(|item| item.code)
        .collect();
    for expected in ["invalid_timeout", "invalid_payload", "invalid_last_step"] {
        assert!(
            codes.contains(&expected.to_owned()),
            "missing {expected}: {codes:?}"
        );
    }
}

#[test]
fn shared_expected_diagnostics_are_exhaustive() {
    let expected: serde_json::Value =
        serde_json::from_str(EXPECTED).expect("expected diagnostics should be valid JSON");
    let fixtures = [
        ("valid_control.yaml", VALID),
        ("unknown_action.yaml", UNKNOWN),
        ("missing_parameter.yaml", MISSING),
        ("invalid_control.yaml", INVALID),
        ("invalid_values.yaml", INVALID_VALUES),
        ("invalid_web_values.yaml", INVALID_WEB_VALUES),
    ];
    for (name, yaml) in fixtures {
        let scenario = Scenario::from_yaml(yaml).expect("fixture parses");
        let diagnostics = scenario.validate();
        let codes: Vec<_> = diagnostics.iter().map(|item| item.code.as_str()).collect();
        let report = &expected[name];
        assert_eq!(
            scenario.is_valid(),
            report["valid"].as_bool().expect("valid should be boolean"),
            "validity changed for {name}"
        );
        let expected_codes: Vec<_> = report["codes"]
            .as_array()
            .expect("codes should be an array")
            .iter()
            .map(|code| code.as_str().expect("diagnostic code should be a string"))
            .collect();
        assert_eq!(codes, expected_codes, "codes changed for {name}");
    }
}

#[test]
fn cli_validates_relative_nested_scenarios() {
    let root = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/fixtures/validator_compat/nested/root.yaml"
    );
    let output = Command::new(env!("CARGO_BIN_EXE_passoflow-validate"))
        .arg(root)
        .output()
        .expect("validator binary should run");
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("validator report should be JSON");
    assert_eq!(report["errors"], 1);
    assert_eq!(report["diagnostics"][0]["code"], "missing_parameter");
    assert_eq!(report["diagnostics"][0]["path"], "child.yaml:steps[1]");
}

#[test]
fn cli_emits_the_versioned_execution_plan() {
    let root = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/fixtures/validator_compat/valid_control.yaml"
    );
    let output = Command::new(env!("CARGO_BIN_EXE_passoflow-validate"))
        .args(["--plan", root])
        .output()
        .expect("planner binary should run");
    assert!(output.status.success());
    let plan: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("execution plan should be JSON");
    assert_eq!(plan["contract"], "0.1");
    assert_eq!(plan["steps"][0]["index"], 1);
    assert_eq!(plan["steps"][0]["action"], "if");
}

#[test]
fn cli_rejects_missing_and_circular_nested_scenarios() {
    let cases = [
        ("missing-root.yaml", "missing_scenario", "steps[1]"),
        (
            "cycle-a.yaml",
            "circular_scenario",
            "cycle-b.yaml:cycle-a.yaml:scenario",
        ),
    ];
    for (name, code, path) in cases {
        let root = format!(
            "{}/../../../tests/fixtures/validator_compat/nested/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_passoflow-validate"))
            .arg(root)
            .output()
            .expect("validator binary should run");
        assert_eq!(
            output.status.code(),
            Some(1),
            "unexpected status for {name}"
        );
        let report: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("validator report should be JSON");
        assert_eq!(
            report["diagnostics"][0]["code"], code,
            "code changed for {name}"
        );
        assert_eq!(
            report["diagnostics"][0]["path"], path,
            "path changed for {name}"
        );
    }
}

#[test]
fn cli_matches_shared_expected_diagnostics_for_nested_cases() {
    let expected: serde_json::Value =
        serde_json::from_str(EXPECTED).expect("expected diagnostics should be valid JSON");
    for name in [
        "nested/root.yaml",
        "nested/missing-root.yaml",
        "nested/cycle-a.yaml",
        "nested/warning-root.yaml",
        "nested/table-warning-root.yaml",
        "nested/variable-warning-root.yaml",
        "nested/static-valid-root.yaml",
        "nested/missing-steps.yaml",
        "nested/nonmapping-step.yaml",
        "nested/non-string-action.yaml",
    ] {
        let root = format!(
            "{}/../../../tests/fixtures/validator_compat/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_passoflow-validate"))
            .arg(root)
            .output()
            .expect("validator binary should run");
        let report: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("validator report should be JSON");
        let expected_case = &expected[name];
        assert_eq!(
            report["valid"].as_bool().expect("valid should be boolean"),
            expected_case["valid"]
                .as_bool()
                .expect("expected valid should be boolean"),
            "validity changed for {name}"
        );
        let codes: Vec<_> = report["diagnostics"]
            .as_array()
            .expect("diagnostics should be an array")
            .iter()
            .map(|item| item["code"].as_str().expect("code should be a string"))
            .collect();
        let expected_codes: Vec<_> = expected_case["codes"]
            .as_array()
            .expect("codes should be an array")
            .iter()
            .map(|code| code.as_str().expect("expected code should be a string"))
            .collect();
        assert_eq!(codes, expected_codes, "codes changed for {name}");
    }
}

#[test]
fn cli_does_not_warn_for_defined_variables_and_loaded_tables() {
    let root = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/fixtures/validator_compat/nested/static-valid-root.yaml"
    );
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_passoflow-validate"))
        .arg(root)
        .output()
        .expect("validator binary should run");
    assert_eq!(output.status.code(), Some(0));
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("validator report should be JSON");
    assert_eq!(report["valid"], true);
    assert_eq!(report["warnings"], 0);
    assert!(report["diagnostics"].as_array().unwrap().is_empty());
}

#[test]
fn cli_returns_json_diagnostic_for_invalid_yaml() {
    let root = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/fixtures/validator_compat/nested/invalid-yaml.yaml"
    );
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_passoflow-validate"))
        .arg(root)
        .output()
        .expect("validator binary should run");
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("validator report should be JSON");
    assert_eq!(report["errors"], 1);
    assert_eq!(report["diagnostics"][0]["code"], "invalid_scenario");
    assert!(output.stderr.is_empty());
}

#[test]
fn cli_reports_missing_images_as_non_blocking_warnings() {
    let root = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/fixtures/validator_compat/nested/warning-root.yaml"
    );
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_passoflow-validate"))
        .arg(root)
        .output()
        .expect("validator binary should run");
    assert_eq!(output.status.code(), Some(0));
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("validator report should be JSON");
    assert_eq!(report["valid"], true);
    assert_eq!(report["errors"], 0);
    assert_eq!(report["warnings"], 1);
    assert_eq!(report["diagnostics"][0]["code"], "missing_image");
    assert_eq!(report["diagnostics"][0]["severity"], "warning");
}

#[test]
fn cli_reports_unloaded_loop_tables_as_non_blocking_warnings() {
    let root = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/fixtures/validator_compat/nested/table-warning-root.yaml"
    );
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_passoflow-validate"))
        .arg(root)
        .output()
        .expect("validator binary should run");
    assert_eq!(output.status.code(), Some(0));
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("validator report should be JSON");
    assert_eq!(report["warnings"], 1);
    assert_eq!(report["diagnostics"][0]["code"], "missing_loop_table");
    assert_eq!(report["diagnostics"][0]["severity"], "warning");
}

#[test]
fn cli_reports_undefined_variables_as_non_blocking_warnings() {
    let root = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/fixtures/validator_compat/nested/variable-warning-root.yaml"
    );
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_passoflow-validate"))
        .arg(root)
        .output()
        .expect("validator binary should run");
    assert_eq!(output.status.code(), Some(0));
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("validator report should be JSON");
    assert_eq!(report["warnings"], 3);
    assert_eq!(report["diagnostics"][0]["code"], "undefined_variable");
    assert_eq!(report["diagnostics"][1]["code"], "undefined_variable");
    assert_eq!(report["diagnostics"][2]["code"], "undefined_variable");
}
