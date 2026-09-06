//! Deterministic execution policy for `PassoFlow`.
//!
//! The engine owns control flow around an action adapter. It does not know how
//! to operate a desktop, browser, or file system; those capabilities implement
//! [`StepExecutor`] and return the shared [`ActionResult`] contract.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use passoflow_core::{
    ActionOutcome, ActionResult, CONTRACT_VERSION, ControlFlowError, ExecutionPlan, LastStepState,
    PlannedStep, RetryPolicy, RunEvent,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result of an entire plan execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Success,
    WarningContinue,
    FailureStop,
    Stopped,
}

/// Structured report retained by bindings and the local UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunReport {
    pub run_id: String,
    pub status: ExecutionStatus,
    pub completed_steps: u32,
    pub events: Vec<RunEvent>,
}

/// Action adapter called by the engine after plan validation.
pub trait StepExecutor {
    /// Execute one planned step.
    /// # Errors
    ///
    /// Adapter errors are represented as [`ActionOutcome::FailureStop`] in the
    /// shared result contract; this method only fails for an unavailable
    /// adapter or an internal adapter error.
    fn execute(&mut self, step: &PlannedStep) -> Result<ActionResult, EngineError>;
}

/// Injectable wait boundary so retry tests never need real sleeps.
pub trait RetrySleeper {
    /// Wait between two attempts.
    fn sleep(&mut self, interval_ms: u64);
}

/// No-op sleeper for callers that provide their own timing or tests.
#[derive(Debug, Default)]
pub struct NoopSleeper;

impl RetrySleeper for NoopSleeper {
    fn sleep(&mut self, _interval_ms: u64) {}
}

/// Errors that prevent the engine from producing a trustworthy report.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EngineError {
    #[error("execution plan contract {actual:?} is not supported; expected {expected:?}")]
    UnsupportedContract { actual: String, expected: String },
    #[error("step adapter failed: {0}")]
    Adapter(String),
    #[error("control-flow selection failed: {0}")]
    ControlFlow(#[from] ControlFlowError),
}

/// Execute a validated plan with deterministic retry and stop semantics.
///
/// `retry_policy.attempts` is the total number of attempts per step; zero is
/// normalized to one attempt. Only `FailureStop` is retried. A warning is
/// recorded and execution continues, while a stop request prevents the next
/// action and returns `Stopped`.
/// # Errors
///
/// Returns an error when the plan contract is incompatible or an adapter cannot
/// return an action result.
pub fn run<E, S, F>(
    plan: &ExecutionPlan,
    executor: &mut E,
    sleeper: &mut S,
    run_id: impl Into<String>,
    retry_policy: RetryPolicy,
    stop_requested: F,
) -> Result<RunReport, EngineError>
where
    E: StepExecutor,
    S: RetrySleeper,
    F: FnMut() -> bool,
{
    run_steps(
        &plan.steps,
        plan.contract.as_str(),
        executor,
        sleeper,
        run_id,
        retry_policy,
        stop_requested,
    )
}

/// Select branches and fixed-count loops, then execute the resulting steps.
///
/// The caller supplies decisions for each `if` step because condition values
/// can depend on runtime variables and prior action outcomes. Table-backed
/// loops are intentionally rejected until a runtime data provider is added.
/// Retry, stop, and event behavior is identical to [`run`].
///
/// # Errors
///
/// Returns an error when the plan contract is incompatible, control-flow
/// selection cannot be completed, or an adapter cannot return a result.
pub fn run_selected<E, S, F>(
    plan: &ExecutionPlan,
    branch_decisions: &BTreeMap<u32, bool>,
    executor: &mut E,
    sleeper: &mut S,
    run_id: impl Into<String>,
    retry_policy: RetryPolicy,
    stop_requested: F,
) -> Result<RunReport, EngineError>
where
    E: StepExecutor,
    S: RetrySleeper,
    F: FnMut() -> bool,
{
    run_selected_with_tables(
        plan,
        branch_decisions,
        &BTreeMap::new(),
        executor,
        sleeper,
        run_id,
        retry_policy,
        stop_requested,
    )
}

/// Select branches and loops using runtime table rows, then execute the result.
///
/// Each table row is resolved into its loop body before the shared engine path
/// runs it. Missing branch decisions and table data fail closed.
///
/// # Errors
///
/// Returns an error when selection cannot be completed, the plan contract is
/// incompatible, or an adapter cannot return a result.
#[allow(clippy::too_many_arguments)]
pub fn run_selected_with_tables<E, S, F>(
    plan: &ExecutionPlan,
    branch_decisions: &BTreeMap<u32, bool>,
    tables: &BTreeMap<String, Vec<BTreeMap<String, String>>>,
    executor: &mut E,
    sleeper: &mut S,
    run_id: impl Into<String>,
    retry_policy: RetryPolicy,
    stop_requested: F,
) -> Result<RunReport, EngineError>
where
    E: StepExecutor,
    S: RetrySleeper,
    F: FnMut() -> bool,
{
    let selected = plan.select_steps_with_tables(branch_decisions, tables)?;
    let steps: Vec<PlannedStep> = selected
        .into_iter()
        .map(|selected_step| selected_step.step)
        .collect();
    run_steps(
        &steps,
        plan.contract.as_str(),
        executor,
        sleeper,
        run_id,
        retry_policy,
        stop_requested,
    )
}

/// Evaluate branch conditions from one runtime snapshot and execute the plan.
///
/// This facade combines [`ExecutionPlan::branch_decisions`] with
/// [`run_selected_with_tables`]. The snapshot is intentionally immutable for
/// the duration of the run; conditions that depend on intermediate action
/// results belong to the future step-by-step runtime integration.
///
/// # Errors
///
/// Returns an error when condition evaluation, loop selection, contract
/// validation, or an adapter operation fails.
#[allow(clippy::too_many_arguments)]
pub fn run_with_runtime_snapshot<E, S, F>(
    plan: &ExecutionPlan,
    variables: &BTreeMap<String, String>,
    last_step: LastStepState,
    tables: &BTreeMap<String, Vec<BTreeMap<String, String>>>,
    executor: &mut E,
    sleeper: &mut S,
    run_id: impl Into<String>,
    retry_policy: RetryPolicy,
    stop_requested: F,
) -> Result<RunReport, EngineError>
where
    E: StepExecutor,
    S: RetrySleeper,
    F: FnMut() -> bool,
{
    let decisions = plan.branch_decisions(variables, last_step)?;
    run_selected_with_tables(
        plan,
        &decisions,
        tables,
        executor,
        sleeper,
        run_id,
        retry_policy,
        stop_requested,
    )
}

fn run_steps<E, S, F>(
    steps: &[PlannedStep],
    contract: &str,
    executor: &mut E,
    sleeper: &mut S,
    run_id: impl Into<String>,
    retry_policy: RetryPolicy,
    mut stop_requested: F,
) -> Result<RunReport, EngineError>
where
    E: StepExecutor,
    S: RetrySleeper,
    F: FnMut() -> bool,
{
    if contract != CONTRACT_VERSION {
        return Err(EngineError::UnsupportedContract {
            actual: contract.to_owned(),
            expected: CONTRACT_VERSION.to_owned(),
        });
    }
    let run_id = run_id.into();
    let mut events = Vec::new();
    let mut completed_steps = 0;
    let mut status = ExecutionStatus::Success;
    let attempts = retry_policy.attempts.max(1);

    for step in steps {
        if stop_requested() {
            status = ExecutionStatus::Stopped;
            break;
        }
        let mut final_result = None;
        for attempt in 1..=attempts {
            if stop_requested() {
                status = ExecutionStatus::Stopped;
                break;
            }
            let result = executor.execute(step)?;
            let should_retry = result.outcome == ActionOutcome::FailureStop && attempt < attempts;
            events.push(event(&run_id, step, &result, attempt));
            if !should_retry {
                final_result = Some(result);
                break;
            }
            sleeper.sleep(retry_policy.interval_ms);
        }
        if status == ExecutionStatus::Stopped {
            break;
        }
        let Some(result) = final_result else {
            status = ExecutionStatus::Stopped;
            break;
        };
        completed_steps = step.index;
        match result.outcome {
            ActionOutcome::Success => {}
            ActionOutcome::WarningContinue => status = ExecutionStatus::WarningContinue,
            ActionOutcome::FailureStop => {
                status = ExecutionStatus::FailureStop;
                break;
            }
        }
    }
    Ok(RunReport {
        run_id,
        status,
        completed_steps,
        events,
    })
}

fn event(run_id: &str, step: &PlannedStep, result: &ActionResult, attempt: u32) -> RunEvent {
    RunEvent {
        protocol: CONTRACT_VERSION.to_owned(),
        event_type: "step_attempt".to_owned(),
        run_id: run_id.to_owned(),
        step: step.index,
        action: step.action.clone(),
        outcome: result.outcome,
        message: format!("attempt {attempt}: {}", result.message),
        artifacts: result.artifacts.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        ExecutionStatus, NoopSleeper, RetrySleeper, StepExecutor, run, run_selected,
        run_selected_with_tables, run_with_runtime_snapshot,
    };
    use passoflow_core::{
        ActionOutcome, ActionResult, CONTRACT_VERSION, ExecutionPlan, PlannedStep, RetryPolicy,
    };

    struct FakeExecutor {
        results: Vec<ActionResult>,
        calls: usize,
    }

    impl StepExecutor for FakeExecutor {
        fn execute(&mut self, _step: &PlannedStep) -> Result<ActionResult, super::EngineError> {
            let result = self.results[self.calls].clone();
            self.calls += 1;
            Ok(result)
        }
    }

    #[derive(Default)]
    struct CountingSleeper(usize);

    impl RetrySleeper for CountingSleeper {
        fn sleep(&mut self, _interval_ms: u64) {
            self.0 += 1;
        }
    }

    fn plan() -> ExecutionPlan {
        ExecutionPlan {
            contract: CONTRACT_VERSION.to_owned(),
            steps: vec![PlannedStep {
                index: 1,
                action: "click_image".to_owned(),
                params: std::collections::BTreeMap::new(),
                branch_depth: 0,
                loop_label: None,
                loop_count: None,
                loop_table: None,
                variables_defined: Vec::new(),
                variables_referenced: Vec::new(),
            }],
        }
    }

    fn result(outcome: ActionOutcome, message: &str) -> ActionResult {
        ActionResult {
            outcome,
            message: message.to_owned(),
            artifacts: Vec::new(),
        }
    }

    #[test]
    fn retries_failure_then_completes() {
        let mut executor = FakeExecutor {
            results: vec![
                result(ActionOutcome::FailureStop, "not found"),
                result(ActionOutcome::Success, "clicked"),
            ],
            calls: 0,
        };
        let mut sleeper = CountingSleeper::default();
        let report = run(
            &plan(),
            &mut executor,
            &mut sleeper,
            "run-1",
            RetryPolicy {
                attempts: 2,
                interval_ms: 25,
            },
            || false,
        )
        .expect("run should succeed");
        assert_eq!(report.status, ExecutionStatus::Success);
        assert_eq!(report.completed_steps, 1);
        assert_eq!(report.events.len(), 2);
        assert_eq!(sleeper.0, 1);
    }

    #[test]
    fn warning_continues_without_retry() {
        let mut executor = FakeExecutor {
            results: vec![result(ActionOutcome::WarningContinue, "webhook warning")],
            calls: 0,
        };
        let mut sleeper = NoopSleeper;
        let report = run(
            &plan(),
            &mut executor,
            &mut sleeper,
            "run-2",
            RetryPolicy {
                attempts: 3,
                interval_ms: 10,
            },
            || false,
        )
        .expect("run should succeed");
        assert_eq!(report.status, ExecutionStatus::WarningContinue);
        assert_eq!(report.events.len(), 1);
    }

    #[test]
    fn stop_request_prevents_execution() {
        let mut executor = FakeExecutor {
            results: vec![result(ActionOutcome::Success, "unused")],
            calls: 0,
        };
        let mut sleeper = NoopSleeper;
        let report = run(
            &plan(),
            &mut executor,
            &mut sleeper,
            "run-3",
            RetryPolicy {
                attempts: 1,
                interval_ms: 0,
            },
            || true,
        )
        .expect("run should succeed");
        assert_eq!(report.status, ExecutionStatus::Stopped);
        assert_eq!(executor.calls, 0);
    }

    #[test]
    fn run_selected_executes_only_the_chosen_branch_and_fixed_loop() {
        let scenario = passoflow_core::Scenario::from_yaml(
            "steps:\n  - action: if\n    variable: ready\n  - action: noop\n  - action: else\n  - action: wait\n    ms: 2\n  - action: endif\n  - action: noop\n    loop: repeat\n    loop_count: 2\n",
        )
        .expect("scenario should parse");
        let plan = scenario.execution_plan();
        let mut executor = FakeExecutor {
            results: vec![
                result(ActionOutcome::Success, "waited"),
                result(ActionOutcome::Success, "repeated"),
                result(ActionOutcome::Success, "repeated"),
            ],
            calls: 0,
        };
        let mut sleeper = NoopSleeper;
        let report = run_selected(
            &plan,
            &BTreeMap::from([(1, false)]),
            &mut executor,
            &mut sleeper,
            "run-selected",
            RetryPolicy {
                attempts: 1,
                interval_ms: 0,
            },
            || false,
        )
        .expect("selected run should succeed");

        assert_eq!(report.status, ExecutionStatus::Success);
        assert_eq!(report.events.len(), 3);
        assert_eq!(executor.calls, 3);
        assert_eq!(report.events[0].step, 4);
        assert_eq!(report.events[1].step, 6);
        assert_eq!(report.events[2].step, 6);
    }

    #[test]
    fn run_selected_with_tables_executes_each_resolved_row() {
        let scenario = passoflow_core::Scenario::from_yaml(
            "steps:\n  - action: type_text\n    text: '{{ name }}'\n    loop: people\n    loop_table: rows\n",
        )
        .expect("scenario should parse");
        let plan = scenario.execution_plan();
        let tables = BTreeMap::from([(
            "rows".to_owned(),
            vec![
                BTreeMap::from([("name".to_owned(), "Ada".to_owned())]),
                BTreeMap::from([("name".to_owned(), "Grace".to_owned())]),
            ],
        )]);
        let mut executor = FakeExecutor {
            results: vec![
                result(ActionOutcome::Success, "typed Ada"),
                result(ActionOutcome::Success, "typed Grace"),
            ],
            calls: 0,
        };
        let mut sleeper = NoopSleeper;
        let report = run_selected_with_tables(
            &plan,
            &BTreeMap::new(),
            &tables,
            &mut executor,
            &mut sleeper,
            "run-table",
            RetryPolicy {
                attempts: 1,
                interval_ms: 0,
            },
            || false,
        )
        .expect("table run should succeed");

        assert_eq!(report.status, ExecutionStatus::Success);
        assert_eq!(report.events.len(), 2);
        assert_eq!(executor.calls, 2);
    }

    #[test]
    fn runtime_snapshot_facade_evaluates_conditions_before_execution() {
        let scenario = passoflow_core::Scenario::from_yaml(
            "steps:\n  - action: if\n    variable: ready\n  - action: noop\n  - action: else\n  - action: wait\n    ms: 2\n  - action: endif\n",
        )
        .expect("scenario should parse");
        let mut executor = FakeExecutor {
            results: vec![result(ActionOutcome::Success, "waited")],
            calls: 0,
        };
        let mut sleeper = NoopSleeper;
        let report = run_with_runtime_snapshot(
            &scenario.execution_plan(),
            &BTreeMap::new(),
            passoflow_core::LastStepState::None,
            &BTreeMap::new(),
            &mut executor,
            &mut sleeper,
            "run-snapshot",
            RetryPolicy {
                attempts: 1,
                interval_ms: 0,
            },
            || false,
        )
        .expect("snapshot run should succeed");

        assert_eq!(report.status, ExecutionStatus::Success);
        assert_eq!(report.events.len(), 1);
        assert_eq!(report.events[0].step, 4);
        assert_eq!(executor.calls, 1);
    }
}
