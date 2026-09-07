use std::{collections::BTreeSet, env, fs, path::Path, process::ExitCode};

use passoflow_core::{
    CONTRACT_VERSION, Diagnostic, Scenario, Severity, action_schema, step_meta_keys,
};
use serde::Serialize;

#[derive(Serialize)]
struct ValidationReport {
    contract: &'static str,
    valid: bool,
    errors: usize,
    warnings: usize,
    diagnostics: Vec<passoflow_core::Diagnostic>,
}

fn usage() {
    eprintln!("Usage: passoflow-validate [--normalized|--plan] <scenario.yaml> | --schema");
}

fn collect_variable_references(value: &serde_yaml::Value, references: &mut BTreeSet<String>) {
    match value {
        serde_yaml::Value::String(text) => {
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
        serde_yaml::Value::Sequence(items) => {
            for item in items {
                collect_variable_references(item, references);
            }
        }
        serde_yaml::Value::Mapping(items) => {
            for (key, value) in items {
                collect_variable_references(key, references);
                collect_variable_references(value, references);
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_lines)]
fn validate_file(path: &Path, prefix: &str, ancestors: &mut BTreeSet<String>) -> Vec<Diagnostic> {
    let display = path.display().to_string();
    let identity = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string();
    if !ancestors.insert(identity.clone()) {
        return vec![Diagnostic {
            severity: Severity::Error,
            code: "circular_scenario".to_owned(),
            path: format!("{prefix}scenario"),
            message: format!("circular scenario reference: {display}"),
        }];
    }
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) => {
            ancestors.remove(&identity);
            return vec![Diagnostic {
                severity: Severity::Error,
                code: "missing_scenario".to_owned(),
                path: format!("{prefix}scenario"),
                message: format!("could not read scenario {display}: {error}"),
            }];
        }
    };
    let scenario = match Scenario::from_yaml(&contents) {
        Ok(scenario) => scenario,
        Err(error) => {
            ancestors.remove(&identity);
            return vec![Diagnostic {
                severity: Severity::Error,
                code: "invalid_scenario".to_owned(),
                path: format!("{prefix}scenario"),
                message: error.to_string(),
            }];
        }
    };
    let mut diagnostics: Vec<_> = scenario
        .validate()
        .into_iter()
        .map(|mut diagnostic| {
            if !prefix.is_empty() {
                diagnostic.path = format!("{prefix}{}", diagnostic.path);
            }
            diagnostic
        })
        .collect();
    let loaded_tables: BTreeSet<_> = scenario
        .steps
        .iter()
        .filter(|step| step.action == "load_table")
        .filter_map(|step| step.params.get("name").and_then(serde_yaml::Value::as_str))
        .collect();
    let variable_definitions: BTreeSet<_> = scenario
        .steps
        .iter()
        .filter(|step| {
            matches!(
                step.action.as_str(),
                "set_variable"
                    | "concat_variable"
                    | "set_year_month_variable"
                    | "set_year_month_day_variable"
                    | "set_month_start_variable"
                    | "set_month_end_variable"
                    | "get_excel_value"
            )
        })
        .filter_map(|step| step.params.get("name").and_then(serde_yaml::Value::as_str))
        .map(str::to_owned)
        .collect();
    for (index, step) in scenario.steps.iter().enumerate() {
        if let Some(table) = step
            .params
            .get("loop_table")
            .and_then(serde_yaml::Value::as_str)
        {
            if !loaded_tables.contains(table) {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "missing_loop_table".to_owned(),
                    path: format!("{prefix}steps[{}]", index + 1),
                    message: format!("loop table not loaded in this scenario: {table}"),
                });
            }
        }
        if loaded_tables.is_empty() {
            let mut references = BTreeSet::new();
            if step.action == "paste_variable" || step.action == "if" {
                if let Some(name) = step
                    .params
                    .get("name")
                    .or_else(|| step.params.get("variable"))
                    .and_then(serde_yaml::Value::as_str)
                {
                    references.insert(name.to_owned());
                }
            }
            for value in step.params.values() {
                collect_variable_references(value, &mut references);
            }
            for name in references.difference(&variable_definitions) {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "undefined_variable".to_owned(),
                    path: format!("{prefix}steps[{}]", index + 1),
                    message: format!("variable not defined in this scenario: {name}"),
                });
            }
        }
        if matches!(step.action.as_str(), "move_mouse_to_image" | "click_image") {
            if let Some(images) = step
                .params
                .get("images")
                .and_then(serde_yaml::Value::as_sequence)
            {
                for image in images.iter().filter_map(serde_yaml::Value::as_str) {
                    let image_path = path.parent().unwrap_or_else(|| Path::new(".")).join(image);
                    if !image_path.is_file() {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Warning,
                            code: "missing_image".to_owned(),
                            path: format!("{prefix}steps[{}]", index + 1),
                            message: format!("image file not found: {image}"),
                        });
                    }
                }
            }
        }
        if !matches!(step.action.as_str(), "call_scenario" | "repeat") {
            continue;
        }
        let Some(target) = step.params.get("path").and_then(serde_yaml::Value::as_str) else {
            continue;
        };
        let target_path = path.parent().unwrap_or_else(|| Path::new(".")).join(target);
        if !target_path.is_file() {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "missing_scenario".to_owned(),
                path: format!("{prefix}steps[{}]", index + 1),
                message: format!("{} path not found: {target}", step.action),
            });
            continue;
        }
        diagnostics.extend(validate_file(
            &target_path,
            &format!("{prefix}{target}:"),
            ancestors,
        ));
    }
    ancestors.remove(&identity);
    diagnostics
}

fn main() -> ExitCode {
    let args: Vec<_> = env::args().skip(1).collect();
    let normalized = args.first().is_some_and(|arg| arg == "--normalized");
    let plan = args.first().is_some_and(|arg| arg == "--plan");
    let schema = args.first().is_some_and(|arg| arg == "--schema");
    if schema {
        if args.len() != 1 {
            usage();
            return ExitCode::from(2);
        }
        let payload = serde_json::json!({
            "contract": CONTRACT_VERSION,
            "actions": action_schema(),
            "step_meta_keys": step_meta_keys(),
        });
        println!("{payload}");
        return ExitCode::SUCCESS;
    }
    let path_arg = if normalized || plan {
        args.get(1)
    } else {
        args.first()
    };
    if ((normalized || plan) && args.len() != 2) || (!normalized && !plan && args.len() != 1) {
        usage();
        return ExitCode::from(2);
    }
    let Some(path) = path_arg else {
        usage();
        return ExitCode::from(2);
    };
    let root = Path::new(&path);
    if !root.is_file() {
        eprintln!("could not read {path}");
        return ExitCode::from(2);
    }
    if normalized || plan {
        let contents = match fs::read_to_string(root) {
            Ok(contents) => contents,
            Err(error) => {
                eprintln!("could not read {path}: {error}");
                return ExitCode::from(2);
            }
        };
        let scenario = match Scenario::from_yaml(&contents) {
            Ok(scenario) => scenario,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::from(1);
            }
        };
        if plan {
            match serde_json::to_string_pretty(&scenario.execution_plan()) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("could not encode execution plan: {error}");
                    return ExitCode::from(2);
                }
            }
        } else {
            match scenario.to_yaml() {
                Ok(yaml) => print!("{yaml}"),
                Err(error) => {
                    eprintln!("could not encode normalized scenario: {error}");
                    return ExitCode::from(2);
                }
            }
        }
        return ExitCode::SUCCESS;
    }
    let mut ancestors = BTreeSet::new();
    let mut diagnostics = validate_file(root, "", &mut ancestors);
    diagnostics.sort_by(|left, right| {
        (&left.path, &left.code, &left.message).cmp(&(&right.path, &right.code, &right.message))
    });
    let errors = diagnostics
        .iter()
        .filter(|item| item.severity == Severity::Error)
        .count();
    let warnings = diagnostics.len() - errors;
    let report = ValidationReport {
        contract: CONTRACT_VERSION,
        valid: errors == 0,
        errors,
        warnings,
        diagnostics,
    };
    match serde_json::to_string_pretty(&report) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("could not encode validation report: {error}");
            return ExitCode::from(2);
        }
    }
    if report.valid {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
