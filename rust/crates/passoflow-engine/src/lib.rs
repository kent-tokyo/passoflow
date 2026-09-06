//! Deterministic execution policy for `PassoFlow`.
//!
//! The engine owns control flow around an action adapter. It does not know how
//! to operate a desktop, browser, or file system; those capabilities implement
//! [`StepExecutor`] and return the shared [`ActionResult`] contract.

#![forbid(unsafe_code)]

use passoflow_core::{
    ActionOutcome, ActionResult, ExecutionPlan, PlannedStep, RetryPolicy, RunEvent,
    CONTRACT_VERSION,
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
    mut stop_requested: F,
) -> Result<RunReport, EngineError>
where
    E: StepExecutor,
    S: RetrySleeper,
    F: FnMut() -> bool,
{
    if plan.contract != CONTRACT_VERSION {
        return Err(EngineError::UnsupportedContract {
            actual: plan.contract.clone(),
            expected: CONTRACT_VERSION.to_owned(),
        });
    }
    let run_id = run_id.into();
    let mut events = Vec::new();
    let mut completed_steps = 0;
    let mut status = ExecutionStatus::Success;
    let attempts = retry_policy.attempts.max(1);

    for step in &plan.steps {
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
    use super::{run, ExecutionStatus, NoopSleeper, RetrySleeper, StepExecutor};
    use passoflow_core::{
        ActionOutcome, ActionResult, ExecutionPlan, PlannedStep, RetryPolicy, CONTRACT_VERSION,
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
}
