//! OS-independent DOM browser contracts for `PassoFlow`.
//!
//! This crate intentionally does not launch a browser or make network calls.
//! A future CDP/Chromium adapter can implement [`BrowserBackend`] without
//! changing the scenario-facing operation contract.

#![forbid(unsafe_code)]

use passoflow_core::PlannedStep;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The state a selector must reach before a wait operation completes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitState {
    Attached,
    Detached,
    Hidden,
    Visible,
}

/// A validated operation sent to a browser backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BrowserAction {
    Navigate {
        url: String,
    },
    Click {
        selector: String,
    },
    Fill {
        selector: String,
        text: String,
    },
    WaitFor {
        selector: String,
        state: WaitState,
        timeout_ms: u64,
    },
}

impl BrowserAction {
    /// Validate user-provided browser parameters before backend dispatch.
    ///
    /// # Errors
    ///
    /// Returns an explicit error for empty selectors, unsupported URLs, or a
    /// zero wait timeout.
    pub fn validate(&self) -> Result<(), BrowserError> {
        match self {
            Self::Navigate { url } => {
                let supported = url.split_once("://").is_some_and(|(scheme, authority)| {
                    let host = authority.split(['/', '?', '#']).next().unwrap_or_default();
                    matches!(scheme, "http" | "https")
                        && !host.trim().is_empty()
                        && !url.chars().any(char::is_whitespace)
                        && !url.chars().any(char::is_control)
                });
                if !supported {
                    return Err(BrowserError::InvalidUrl { url: url.clone() });
                }
            }
            Self::Click { selector }
            | Self::Fill { selector, .. }
            | Self::WaitFor { selector, .. } => {
                if selector.trim().is_empty() {
                    return Err(BrowserError::EmptySelector);
                }
            }
        }
        if let Self::WaitFor { timeout_ms, .. } = self {
            if *timeout_ms == 0 {
                return Err(BrowserError::InvalidTimeout);
            }
        }
        Ok(())
    }
}

/// Errors that must be reported before a browser backend is called.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BrowserError {
    #[error("unsupported DOM action: {0}")]
    UnsupportedAction(String),
    #[error("missing DOM action parameter: {0}")]
    MissingParameter(String),
    #[error("DOM selector must not be empty")]
    EmptySelector,
    #[error("browser URL must use http or https: {url}")]
    InvalidUrl { url: String },
    #[error("browser wait timeout must be greater than zero")]
    InvalidTimeout,
    #[error("unsupported browser wait state: {0}")]
    InvalidWaitState(String),
    #[error("browser backend is unavailable: {0}")]
    BackendUnavailable(String),
}

/// Backend boundary for a real DOM browser implementation.
pub trait BrowserBackend {
    /// Navigate the managed browser page.
    ///
    /// # Errors
    ///
    /// Returns a backend error when navigation cannot be completed.
    fn navigate(&mut self, url: &str) -> Result<(), BrowserError>;
    /// Click the first element matching `selector`.
    ///
    /// # Errors
    ///
    /// Returns a backend error when the element cannot be clicked.
    fn click(&mut self, selector: &str) -> Result<(), BrowserError>;
    /// Fill the first element matching `selector`.
    ///
    /// # Errors
    ///
    /// Returns a backend error when the element cannot be filled.
    fn fill(&mut self, selector: &str, text: &str) -> Result<(), BrowserError>;
    /// Wait for a selector to reach the requested state.
    ///
    /// # Errors
    ///
    /// Returns a backend error when the requested state is not reached.
    fn wait_for(
        &mut self,
        selector: &str,
        state: WaitState,
        timeout_ms: u64,
    ) -> Result<(), BrowserError>;
}

/// Deterministic backend for tests, dry runs, and future UI previews.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RecordingBrowser {
    events: Vec<BrowserAction>,
}

impl RecordingBrowser {
    /// Return the operations recorded so far.
    #[must_use]
    pub fn events(&self) -> &[BrowserAction] {
        &self.events
    }
}

impl BrowserBackend for RecordingBrowser {
    fn navigate(&mut self, url: &str) -> Result<(), BrowserError> {
        let action = BrowserAction::Navigate {
            url: url.to_owned(),
        };
        action.validate()?;
        self.events.push(action);
        Ok(())
    }

    fn click(&mut self, selector: &str) -> Result<(), BrowserError> {
        let action = BrowserAction::Click {
            selector: selector.to_owned(),
        };
        action.validate()?;
        self.events.push(action);
        Ok(())
    }

    fn fill(&mut self, selector: &str, text: &str) -> Result<(), BrowserError> {
        let action = BrowserAction::Fill {
            selector: selector.to_owned(),
            text: text.to_owned(),
        };
        action.validate()?;
        self.events.push(action);
        Ok(())
    }

    fn wait_for(
        &mut self,
        selector: &str,
        state: WaitState,
        timeout_ms: u64,
    ) -> Result<(), BrowserError> {
        let action = BrowserAction::WaitFor {
            selector: selector.to_owned(),
            state,
            timeout_ms,
        };
        action.validate()?;
        self.events.push(action);
        Ok(())
    }
}

impl BrowserAction {
    /// Convert a normalized Rust plan step into a validated DOM operation.
    ///
    /// This is deliberately separate from [`BrowserBackend`] so a future CDP
    /// adapter receives only supported, typed operations.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported actions, missing parameters, or an
    /// invalid wait state/timeout.
    pub fn from_planned_step(step: &PlannedStep) -> Result<Self, BrowserError> {
        let text = |name: &str| {
            step.params
                .get(name)
                .and_then(serde_yaml::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| BrowserError::MissingParameter(name.to_owned()))
        };
        let timeout = || {
            step.params
                .get("timeout_ms")
                .and_then(serde_yaml::Value::as_u64)
                .ok_or(BrowserError::InvalidTimeout)
        };
        let action = match step.action.as_str() {
            "browser_navigate" => Self::Navigate { url: text("url")? },
            "browser_click" => Self::Click {
                selector: text("selector")?,
            },
            "browser_fill" => Self::Fill {
                selector: text("selector")?,
                text: text("text")?,
            },
            "browser_wait_for" => {
                let state = match step
                    .params
                    .get("state")
                    .and_then(serde_yaml::Value::as_str)
                    .unwrap_or("visible")
                {
                    "attached" => WaitState::Attached,
                    "detached" => WaitState::Detached,
                    "hidden" => WaitState::Hidden,
                    "visible" => WaitState::Visible,
                    other => return Err(BrowserError::InvalidWaitState(other.to_owned())),
                };
                Self::WaitFor {
                    selector: text("selector")?,
                    state,
                    timeout_ms: timeout()?,
                }
            }
            other => return Err(BrowserError::UnsupportedAction(other.to_owned())),
        };
        action.validate()?;
        Ok(action)
    }
}

/// Dispatch one validated action to a backend.
///
/// # Errors
///
/// Returns a validation error before dispatch for unsafe parameters, or a
/// backend error when the operation cannot be completed.
pub fn execute<B: BrowserBackend>(
    backend: &mut B,
    action: &BrowserAction,
) -> Result<(), BrowserError> {
    action.validate()?;
    match action {
        BrowserAction::Navigate { url } => backend.navigate(url),
        BrowserAction::Click { selector } => backend.click(selector),
        BrowserAction::Fill { selector, text } => backend.fill(selector, text),
        BrowserAction::WaitFor {
            selector,
            state,
            timeout_ms,
        } => backend.wait_for(selector, *state, *timeout_ms),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BrowserAction, BrowserBackend, BrowserError, RecordingBrowser, WaitState, execute,
    };

    #[test]
    fn records_valid_dom_operations_in_order() {
        let mut browser = RecordingBrowser::default();
        execute(
            &mut browser,
            &BrowserAction::Navigate {
                url: "https://example.test".to_owned(),
            },
        )
        .expect("navigation is valid");
        execute(
            &mut browser,
            &BrowserAction::Fill {
                selector: "#name".to_owned(),
                text: "PassoFlow".to_owned(),
            },
        )
        .expect("fill is valid");
        browser
            .wait_for("#result", WaitState::Visible, 1_000)
            .expect("wait is valid");
        assert_eq!(browser.events().len(), 3);
    }

    #[test]
    fn rejects_unsafe_or_empty_parameters_before_dispatch() {
        let mut browser = RecordingBrowser::default();
        assert_eq!(
            execute(
                &mut browser,
                &BrowserAction::Navigate {
                    url: "file:///secret".to_owned(),
                },
            ),
            Err(BrowserError::InvalidUrl {
                url: "file:///secret".to_owned()
            })
        );
        assert_eq!(
            execute(
                &mut browser,
                &BrowserAction::Navigate {
                    url: "https://".to_owned(),
                },
            ),
            Err(BrowserError::InvalidUrl {
                url: "https://".to_owned()
            })
        );
        assert_eq!(
            execute(
                &mut browser,
                &BrowserAction::Click {
                    selector: "  ".to_owned(),
                },
            ),
            Err(BrowserError::EmptySelector)
        );
        assert!(browser.events().is_empty());
    }

    #[test]
    fn maps_normalized_plan_steps_to_typed_dom_actions() {
        let scenario = passoflow_core::Scenario::from_yaml(
            "steps:\n  - action: browser_navigate\n    url: https://example.test\n  - action: browser_click\n    selector: '#submit'\n  - action: browser_fill\n    selector: '#name'\n    text: Ada\n  - action: browser_wait_for\n    selector: '#result'\n    state: attached\n    timeout_ms: 2000\n",
        )
        .expect("scenario should parse");
        let plan = scenario.execution_plan();

        assert_eq!(
            BrowserAction::from_planned_step(&plan.steps[0]),
            Ok(BrowserAction::Navigate {
                url: "https://example.test".to_owned()
            })
        );
        assert_eq!(
            BrowserAction::from_planned_step(&plan.steps[1]),
            Ok(BrowserAction::Click {
                selector: "#submit".to_owned()
            })
        );
        assert_eq!(
            BrowserAction::from_planned_step(&plan.steps[2]),
            Ok(BrowserAction::Fill {
                selector: "#name".to_owned(),
                text: "Ada".to_owned()
            })
        );
        assert_eq!(
            BrowserAction::from_planned_step(&plan.steps[3]),
            Ok(BrowserAction::WaitFor {
                selector: "#result".to_owned(),
                state: WaitState::Attached,
                timeout_ms: 2_000
            })
        );
    }

    #[test]
    fn rejects_unsupported_plan_dom_steps_before_backend_dispatch() {
        let scenario = passoflow_core::Scenario::from_yaml(
            "steps:\n  - action: browser_wait_for\n    selector: '#result'\n    state: eventually\n    timeout_ms: 1000\n",
        )
        .expect("scenario should parse");
        assert_eq!(
            BrowserAction::from_planned_step(&scenario.execution_plan().steps[0]),
            Err(BrowserError::InvalidWaitState("eventually".to_owned()))
        );
    }
}
