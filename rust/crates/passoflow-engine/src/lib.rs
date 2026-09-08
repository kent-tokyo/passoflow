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

/// Result of a runtime-aware action, including variable updates for later steps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeActionResult {
    pub result: ActionResult,
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
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

/// Runtime-aware adapter used when later conditions depend on earlier actions.
pub trait RuntimeStepExecutor {
    /// Execute one planned step using the current immutable state snapshot.
    /// # Errors
    ///
    /// Returns an error when the adapter cannot execute the action.
    fn execute_runtime(
        &mut self,
        step: &PlannedStep,
        state: &RuntimeState,
    ) -> Result<RuntimeActionResult, EngineError>;
}

/// Mutable state owned by the engine between runtime-aware steps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeState {
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    pub last_step: LastStepState,
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
    let resolved_plan = plan.resolve_variables(variables);
    let decisions = resolved_plan.branch_decisions(variables, last_step)?;
    run_selected_with_tables(
        &resolved_plan,
        &decisions,
        tables,
        executor,
        sleeper,
        run_id,
        retry_policy,
        stop_requested,
    )
}

/// Execute branch conditions against state updated after every action.
///
/// This is the stateful bridge for scenarios whose `if` conditions depend on
/// earlier actions. Structural markers are handled by the engine and are not
/// dispatched to the adapter. Use [`run_with_runtime_state_and_tables`] when
/// the plan contains loops.
///
/// # Errors
///
/// Returns an error when the plan contract is incompatible, a branch condition
/// is invalid, or an adapter operation fails.
pub fn run_with_runtime_state<E, S, F>(
    plan: &ExecutionPlan,
    variables: BTreeMap<String, String>,
    executor: &mut E,
    sleeper: &mut S,
    run_id: impl Into<String>,
    retry_policy: RetryPolicy,
    stop_requested: F,
) -> Result<RunReport, EngineError>
where
    E: RuntimeStepExecutor,
    S: RetrySleeper,
    F: FnMut() -> bool,
{
    run_with_runtime_state_and_tables(
        plan,
        variables,
        &BTreeMap::new(),
        executor,
        sleeper,
        run_id,
        retry_policy,
        stop_requested,
    )
}

/// Execute runtime-aware branches and loops with caller-provided table rows.
///
/// Table-row variables are scoped to one iteration and restored afterward.
/// Fixed-count and table loops share the same retry, stop, event, and branch
/// behavior as the non-loop runtime path.
///
/// # Errors
///
/// Returns an error when the plan contract is incompatible, control-flow
/// selection is invalid, required table rows are missing, or an adapter fails.
#[allow(clippy::too_many_arguments)]
pub fn run_with_runtime_state_and_tables<E, S, F>(
    plan: &ExecutionPlan,
    variables: BTreeMap<String, String>,
    tables: &BTreeMap<String, Vec<BTreeMap<String, String>>>,
    executor: &mut E,
    sleeper: &mut S,
    run_id: impl Into<String>,
    retry_policy: RetryPolicy,
    stop_requested: F,
) -> Result<RunReport, EngineError>
where
    E: RuntimeStepExecutor,
    S: RetrySleeper,
    F: FnMut() -> bool,
{
    if plan.contract != CONTRACT_VERSION {
        return Err(EngineError::UnsupportedContract {
            actual: plan.contract.clone(),
            expected: CONTRACT_VERSION.to_owned(),
        });
    }
    let mut report = RunReport {
        run_id: run_id.into(),
        status: ExecutionStatus::Success,
        completed_steps: 0,
        events: Vec::new(),
    };
    let mut state = RuntimeState {
        variables,
        last_step: LastStepState::None,
    };
    let mut stop_requested = stop_requested;
    run_dynamic_range(
        plan,
        0,
        plan.steps.len(),
        &mut state,
        executor,
        sleeper,
        &mut report,
        retry_policy,
        &mut stop_requested,
        tables,
        None,
    )?;
    Ok(report)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn run_dynamic_range<E, S, F>(
    plan: &ExecutionPlan,
    start: usize,
    end: usize,
    state: &mut RuntimeState,
    executor: &mut E,
    sleeper: &mut S,
    report: &mut RunReport,
    retry_policy: RetryPolicy,
    stop_requested: &mut F,
    tables: &BTreeMap<String, Vec<BTreeMap<String, String>>>,
    active_loop: Option<&str>,
) -> Result<(), EngineError>
where
    E: RuntimeStepExecutor,
    S: RetrySleeper,
    F: FnMut() -> bool,
{
    let boundaries = plan.control_flow();
    let branches: BTreeMap<_, _> = boundaries
        .branches
        .into_iter()
        .map(|boundary| (boundary.if_step, boundary))
        .collect();
    let loops: BTreeMap<_, _> = boundaries
        .loops
        .into_iter()
        .map(|boundary| (boundary.start_step, boundary))
        .collect();
    let mut index = start;
    while index < end {
        let step = &plan.steps[index];
        if step.action == "if" {
            let boundary = branches
                .get(&step.index)
                .expect("validated plan must contain an if boundary");
            let take_true =
                passoflow_core::evaluate_branch_condition(step, &state.variables, state.last_step)?;
            let branch_start = if take_true {
                index + 1
            } else {
                boundary.else_step.unwrap_or(boundary.endif_step) as usize
            };
            let branch_end = if take_true {
                boundary.else_step.unwrap_or(boundary.endif_step) as usize
            } else {
                boundary.endif_step as usize
            };
            run_dynamic_range(
                plan,
                branch_start,
                branch_end,
                state,
                executor,
                sleeper,
                report,
                retry_policy,
                stop_requested,
                tables,
                active_loop,
            )?;
            if report.status == ExecutionStatus::Stopped
                || report.status == ExecutionStatus::FailureStop
            {
                return Ok(());
            }
            index = boundary.endif_step as usize;
            continue;
        }
        if let Some(label) = step.loop_label.as_deref()
            && active_loop != Some(label)
        {
            let loop_boundary = loops
                .get(&step.index)
                .expect("validated plan must contain a loop boundary");
            let loop_end = loop_boundary.end_step as usize;
            if let Some(table) = loop_boundary.table.as_deref() {
                let rows = tables
                    .get(table)
                    .ok_or_else(|| ControlFlowError::RuntimeTableLoop {
                        label: label.to_owned(),
                        start_step: loop_boundary.start_step,
                        end_step: loop_boundary.end_step,
                    })?;
                let row_keys: std::collections::BTreeSet<_> =
                    rows.iter().flat_map(|row| row.keys().cloned()).collect();
                let saved_values: BTreeMap<_, _> = row_keys
                    .iter()
                    .filter_map(|key| {
                        state
                            .variables
                            .get(key)
                            .map(|value| (key.clone(), value.clone()))
                    })
                    .collect();
                for row in rows {
                    state.variables.extend(row.clone());
                    run_dynamic_range(
                        plan,
                        index,
                        loop_end,
                        state,
                        executor,
                        sleeper,
                        report,
                        retry_policy,
                        stop_requested,
                        tables,
                        Some(label),
                    )?;
                    if report.status == ExecutionStatus::Stopped
                        || report.status == ExecutionStatus::FailureStop
                    {
                        return Ok(());
                    }
                }
                for key in row_keys {
                    if let Some(value) = saved_values.get(&key) {
                        state.variables.insert(key, value.clone());
                    } else {
                        state.variables.remove(&key);
                    }
                }
            } else {
                let count =
                    loop_boundary
                        .count
                        .ok_or_else(|| ControlFlowError::InvalidLoopCount {
                            label: label.to_owned(),
                            count: loop_boundary.count,
                        })?;
                let count =
                    u32::try_from(count).map_err(|_| ControlFlowError::InvalidLoopCount {
                        label: label.to_owned(),
                        count: loop_boundary.count,
                    })?;
                if count == 0 {
                    return Err(ControlFlowError::InvalidLoopCount {
                        label: label.to_owned(),
                        count: loop_boundary.count,
                    }
                    .into());
                }
                for _ in 0..count {
                    run_dynamic_range(
                        plan,
                        index,
                        loop_end,
                        state,
                        executor,
                        sleeper,
                        report,
                        retry_policy,
                        stop_requested,
                        tables,
                        Some(label),
                    )?;
                    if report.status == ExecutionStatus::Stopped
                        || report.status == ExecutionStatus::FailureStop
                    {
                        return Ok(());
                    }
                }
            }
            index = loop_end;
            continue;
        }
        if step.action == "else" || step.action == "endif" {
            index += 1;
            continue;
        }
        if stop_requested() {
            report.status = ExecutionStatus::Stopped;
            return Ok(());
        }
        let step_retry_policy = retry_policy_for_step(step, retry_policy);
        let attempts = step_retry_policy.attempts.max(1);
        let mut final_result = None;
        for attempt in 1..=attempts {
            let resolved_step = ExecutionPlan {
                contract: plan.contract.clone(),
                steps: vec![step.clone()],
            }
            .resolve_variables(&state.variables)
            .steps
            .into_iter()
            .next()
            .expect("single-step resolution must retain the step");
            let runtime_result = executor.execute_runtime(&resolved_step, state)?;
            let should_retry =
                runtime_result.result.outcome == ActionOutcome::FailureStop && attempt < attempts;
            report
                .events
                .push(event(&report.run_id, step, &runtime_result.result, attempt));
            if !should_retry {
                final_result = Some(runtime_result);
                break;
            }
            sleeper.sleep(step_retry_policy.interval_ms);
        }
        let Some(runtime_result) = final_result else {
            report.status = ExecutionStatus::Stopped;
            return Ok(());
        };
        state.variables.extend(runtime_result.variables);
        state.last_step = match runtime_result.result.outcome {
            ActionOutcome::Success => LastStepState::Ok,
            ActionOutcome::WarningContinue => LastStepState::Warned,
            ActionOutcome::FailureStop => LastStepState::Failed,
        };
        report.completed_steps = step.index;
        match runtime_result.result.outcome {
            ActionOutcome::Success => {}
            ActionOutcome::WarningContinue => report.status = ExecutionStatus::WarningContinue,
            ActionOutcome::FailureStop => {
                report.status = ExecutionStatus::FailureStop;
                return Ok(());
            }
        }
        index += 1;
    }
    Ok(())
}

#[allow(clippy::redundant_closure_for_method_calls)]
fn retry_policy_for_step(step: &PlannedStep, default: RetryPolicy) -> RetryPolicy {
    let extra_attempts = step.params.get("retry").and_then(|value| value.as_u64());
    let interval_ms = step
        .params
        .get("retry_interval_ms")
        .and_then(|value| value.as_u64())
        .unwrap_or(default.interval_ms);
    RetryPolicy {
        attempts: extra_attempts
            .and_then(|extra| u32::try_from(extra.saturating_add(1)).ok())
            .unwrap_or(default.attempts),
        interval_ms,
    }
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
        ExecutionStatus, NoopSleeper, RetrySleeper, RuntimeActionResult, RuntimeState,
        RuntimeStepExecutor, StepExecutor, run, run_selected, run_selected_with_tables,
        run_with_runtime_snapshot, run_with_runtime_state, run_with_runtime_state_and_tables,
    };
    use passoflow_core::{
        ActionOutcome, ActionResult, CONTRACT_VERSION, ExecutionPlan, PlannedStep, RetryPolicy,
    };

    struct FakeExecutor {
        results: Vec<ActionResult>,
        calls: usize,
    }

    struct RuntimeFakeExecutor {
        calls: Vec<u32>,
        action_state: Option<RuntimeState>,
        seen_names: Vec<String>,
    }

    impl RuntimeStepExecutor for RuntimeFakeExecutor {
        fn execute_runtime(
            &mut self,
            step: &PlannedStep,
            state: &RuntimeState,
        ) -> Result<RuntimeActionResult, super::EngineError> {
            self.calls.push(step.index);
            self.action_state = Some(state.clone());
            if let Some(name) = state.variables.get("name") {
                self.seen_names.push(name.clone());
            }
            let variables = if step.action == "set_variable" {
                BTreeMap::from([(
                    step.params
                        .get("name")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_owned(),
                    step.params
                        .get("value")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default()
                        .to_owned(),
                )])
            } else {
                BTreeMap::new()
            };
            Ok(RuntimeActionResult {
                result: result(ActionOutcome::Success, "ok"),
                variables,
            })
        }
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
    fn preserves_warning_status_and_event_order_until_later_success() {
        let mut executor = FakeExecutor {
            results: vec![
                result(ActionOutcome::WarningContinue, "window not confirmed"),
                result(ActionOutcome::Success, "clicked"),
            ],
            calls: 0,
        };
        let mut sleeper = NoopSleeper;
        let report = run(
            &ExecutionPlan {
                contract: CONTRACT_VERSION.to_owned(),
                steps: vec![
                    plan().steps[0].clone(),
                    PlannedStep {
                        index: 2,
                        ..plan().steps[0].clone()
                    },
                ],
            },
            &mut executor,
            &mut sleeper,
            "run-warning-then-success",
            RetryPolicy {
                attempts: 1,
                interval_ms: 0,
            },
            || false,
        )
        .expect("run should succeed");
        assert_eq!(report.status, ExecutionStatus::WarningContinue);
        assert_eq!(report.completed_steps, 2);
        assert_eq!(report.events.len(), 2);
        assert_eq!(report.events[0].outcome, ActionOutcome::WarningContinue);
        assert_eq!(report.events[1].outcome, ActionOutcome::Success);
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

    #[test]
    fn runtime_snapshot_facade_resolves_action_parameters_inside_engine() {
        struct InspectingExecutor {
            text: Option<String>,
        }
        impl StepExecutor for InspectingExecutor {
            fn execute(&mut self, step: &PlannedStep) -> Result<ActionResult, super::EngineError> {
                self.text = step
                    .params
                    .get("text")
                    .and_then(|value| value.as_str())
                    .map(str::to_owned);
                Ok(result(ActionOutcome::Success, "typed"))
            }
        }
        let scenario = passoflow_core::Scenario::from_yaml(
            "steps:\n  - action: type_text\n    text: 'Hello {{ name }}'\n",
        )
        .expect("scenario should parse");
        let mut executor = InspectingExecutor { text: None };
        let mut sleeper = NoopSleeper;
        let report = run_with_runtime_snapshot(
            &scenario.execution_plan(),
            &BTreeMap::from([(String::from("name"), String::from("Ada"))]),
            passoflow_core::LastStepState::None,
            &BTreeMap::new(),
            &mut executor,
            &mut sleeper,
            "run-resolved",
            RetryPolicy {
                attempts: 1,
                interval_ms: 0,
            },
            || false,
        )
        .expect("resolved run should succeed");

        assert_eq!(report.status, ExecutionStatus::Success);
        assert_eq!(executor.text.as_deref(), Some("Hello Ada"));
    }

    #[test]
    fn runtime_state_rechecks_branch_after_variable_update() {
        let scenario = passoflow_core::Scenario::from_yaml(
            "steps:\n  - action: set_variable\n    name: ready\n    value: yes\n  - action: if\n    variable: ready\n  - action: type_text\n    text: success\n  - action: else\n  - action: type_text\n    text: fallback\n  - action: endif\n",
        )
        .expect("scenario should parse");
        let mut executor = RuntimeFakeExecutor {
            calls: Vec::new(),
            action_state: None,
            seen_names: Vec::new(),
        };
        let mut sleeper = NoopSleeper;
        let report = run_with_runtime_state(
            &scenario.execution_plan(),
            BTreeMap::new(),
            &mut executor,
            &mut sleeper,
            "run-dynamic",
            RetryPolicy {
                attempts: 1,
                interval_ms: 0,
            },
            || false,
        )
        .expect("dynamic run should succeed");

        assert_eq!(report.status, ExecutionStatus::Success);
        assert_eq!(executor.calls, vec![1, 3]);
        assert_eq!(
            executor
                .action_state
                .expect("if should receive state")
                .variables
                .get("ready")
                .map(String::as_str),
            Some("yes")
        );
    }

    #[test]
    fn runtime_state_executes_fixed_loops() {
        let scenario = passoflow_core::Scenario::from_yaml(
            "steps:\n  - action: noop\n    loop: repeat\n    loop_count: 2\n",
        )
        .expect("scenario should parse");
        let mut executor = RuntimeFakeExecutor {
            calls: Vec::new(),
            action_state: None,
            seen_names: Vec::new(),
        };
        let mut sleeper = NoopSleeper;
        let report = run_with_runtime_state(
            &scenario.execution_plan(),
            BTreeMap::new(),
            &mut executor,
            &mut sleeper,
            "run-fixed-loop",
            RetryPolicy {
                attempts: 1,
                interval_ms: 0,
            },
            || false,
        )
        .expect("fixed loop run should succeed");

        assert_eq!(report.status, ExecutionStatus::Success);
        assert_eq!(executor.calls, vec![1, 1]);
    }

    #[test]
    fn runtime_state_scopes_table_rows_and_resolves_parameters() {
        let scenario = passoflow_core::Scenario::from_yaml(
            "steps:\n  - action: type_text\n    text: '{{ name }}'\n    loop: people\n    loop_table: rows\n",
        )
        .expect("scenario should parse");
        let tables = BTreeMap::from([(
            String::from("rows"),
            vec![
                BTreeMap::from([(String::from("name"), String::from("Ada"))]),
                BTreeMap::from([(String::from("name"), String::from("Grace"))]),
            ],
        )]);
        let mut executor = RuntimeFakeExecutor {
            calls: Vec::new(),
            action_state: None,
            seen_names: Vec::new(),
        };
        let mut sleeper = NoopSleeper;
        let report = run_with_runtime_state_and_tables(
            &scenario.execution_plan(),
            BTreeMap::new(),
            &tables,
            &mut executor,
            &mut sleeper,
            "run-table-loop",
            RetryPolicy {
                attempts: 1,
                interval_ms: 0,
            },
            || false,
        )
        .expect("table loop run should succeed");

        assert_eq!(report.status, ExecutionStatus::Success);
        assert_eq!(executor.calls, vec![1, 1]);
        assert_eq!(executor.seen_names, vec!["Ada", "Grace"]);
    }
}
