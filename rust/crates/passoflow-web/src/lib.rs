//! OS-independent DOM browser contracts for `PassoFlow`.
//!
//! This crate intentionally does not launch a browser or make network calls.
//! A CDP/Chromium adapter can implement [`BrowserBackend`] without changing
//! the scenario-facing operation contract.

#![forbid(unsafe_code)]

use passoflow_core::PlannedStep;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
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
    #[error("invalid DOM selector: {0}")]
    InvalidSelector(String),
    #[error("browser URL must use http or https: {url}")]
    InvalidUrl { url: String },
    #[error("browser wait timeout must be greater than zero")]
    InvalidTimeout,
    #[error("unsupported browser wait state: {0}")]
    InvalidWaitState(String),
    #[error("browser backend is unavailable: {0}")]
    BackendUnavailable(String),
}

/// A concise element sample returned by selector preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectorSample {
    pub tag: String,
    pub text: String,
    pub id: String,
    pub testid: String,
    pub visible: bool,
    pub selector: String,
}

/// Selector candidates that may repair a selector after a page change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectorRepair {
    pub selector: String,
    pub count: u64,
}

/// Read-only selector inspection result from the current browser page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectorPreview {
    pub selector: String,
    pub count: u64,
    pub samples: Vec<SelectorSample>,
    pub suggested_selector: Option<String>,
    pub repair_suggestions: Vec<SelectorRepair>,
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

/// Transport boundary for a Chromium `DevTools` Protocol connection.
pub trait CdpTransport {
    /// Send one CDP command and return its decoded result object.
    /// # Errors
    ///
    /// Returns a backend error when the transport or CDP command fails.
    fn command(&mut self, method: &str, params: Value) -> Result<Value, BrowserError>;
}

/// Minimal text wire needed by a production WebSocket implementation.
pub trait CdpWire {
    /// Send one CDP text frame.
    ///
    /// # Errors
    ///
    /// Returns a backend error when the frame cannot be sent.
    fn send_text(&mut self, payload: &str) -> Result<(), BrowserError>;
    /// Receive one CDP text frame.
    ///
    /// # Errors
    ///
    /// Returns a backend error when the frame cannot be received.
    fn receive_text(&mut self) -> Result<String, BrowserError>;
}

#[cfg(feature = "websocket")]
/// Synchronous local Chromium CDP wire backed by a WebSocket.
pub struct WebSocketCdpWire {
    socket: tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
}

#[cfg(feature = "websocket")]
impl WebSocketCdpWire {
    /// Connect to a local Chromium `DevTools` WebSocket endpoint.
    ///
    /// # Errors
    ///
    /// Returns a backend error when the endpoint cannot be parsed or connected.
    pub fn connect(endpoint: &str) -> Result<Self, BrowserError> {
        let url = endpoint
            .parse::<tungstenite::http::Uri>()
            .map_err(|error| {
                BrowserError::BackendUnavailable(format!("invalid CDP WebSocket endpoint: {error}"))
            })?;
        if url.scheme_str() != Some("ws") {
            return Err(BrowserError::BackendUnavailable(
                "local CDP WebSocket endpoint must use ws://".to_owned(),
            ));
        }
        let (socket, _) = tungstenite::connect(url).map_err(|error| {
            BrowserError::BackendUnavailable(format!("connect to CDP WebSocket: {error}"))
        })?;
        Ok(Self { socket })
    }
}

#[cfg(feature = "websocket")]
impl CdpWire for WebSocketCdpWire {
    fn send_text(&mut self, payload: &str) -> Result<(), BrowserError> {
        self.socket
            .send(tungstenite::Message::Text(payload.to_owned().into()))
            .map_err(|error| BrowserError::BackendUnavailable(format!("send CDP frame: {error}")))
    }

    fn receive_text(&mut self) -> Result<String, BrowserError> {
        loop {
            let message = self.socket.read().map_err(|error| {
                BrowserError::BackendUnavailable(format!("receive CDP frame: {error}"))
            })?;
            match message {
                tungstenite::Message::Text(text) => return Ok(text.to_string()),
                tungstenite::Message::Binary(bytes) => {
                    return String::from_utf8(bytes.to_vec()).map_err(|error| {
                        BrowserError::BackendUnavailable(format!(
                            "decode CDP binary frame: {error}"
                        ))
                    });
                }
                tungstenite::Message::Ping(payload) => self
                    .socket
                    .send(tungstenite::Message::Pong(payload))
                    .map_err(|error| {
                        BrowserError::BackendUnavailable(format!("reply to CDP ping: {error}"))
                    })?,
                tungstenite::Message::Pong(_) | tungstenite::Message::Frame(_) => {}
                tungstenite::Message::Close(frame) => {
                    return Err(BrowserError::BackendUnavailable(format!(
                        "CDP WebSocket closed: {frame:?}"
                    )));
                }
            }
        }
    }
}

/// JSON CDP transport with request/response correlation.
///
/// The wire is injected so the protocol can be tested without a browser. A
/// later WebSocket adapter only needs to implement [`CdpWire`].
#[derive(Debug)]
pub struct JsonCdpTransport<W> {
    wire: W,
    next_id: u64,
}

impl<W> JsonCdpTransport<W> {
    /// Create a transport whose first command has CDP id 1.
    #[must_use]
    pub fn new(wire: W) -> Self {
        Self { wire, next_id: 1 }
    }

    /// Return the underlying wire after execution or inspection.
    #[must_use]
    pub fn into_wire(self) -> W {
        self.wire
    }
}

impl<W: CdpWire> CdpTransport for JsonCdpTransport<W> {
    fn command(&mut self, method: &str, params: Value) -> Result<Value, BrowserError> {
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).ok_or_else(|| {
            BrowserError::BackendUnavailable("CDP command id exhausted".to_owned())
        })?;
        let payload = serde_json::to_string(&json!({
            "id": id,
            "method": method,
            "params": params,
        }))
        .map_err(|error| {
            BrowserError::BackendUnavailable(format!("encode CDP command: {error}"))
        })?;
        self.wire.send_text(&payload)?;

        loop {
            let raw = self.wire.receive_text()?;
            let response: Value = serde_json::from_str(&raw).map_err(|error| {
                BrowserError::BackendUnavailable(format!("decode CDP response: {error}"))
            })?;
            let Some(response_id) = response.get("id").and_then(Value::as_u64) else {
                continue;
            };
            if response_id != id {
                return Err(BrowserError::BackendUnavailable(format!(
                    "unexpected CDP response id {response_id}, expected {id}"
                )));
            }
            if let Some(error) = response.get("error") {
                return Err(BrowserError::BackendUnavailable(format!(
                    "CDP command {method} failed: {error}"
                )));
            }
            return Ok(response);
        }
    }
}

/// Rust-owned DOM backend that emits CDP commands through an injected transport.
#[derive(Debug)]
pub struct CdpBrowser<T> {
    transport: T,
    poll_interval_ms: u64,
}

impl<T> CdpBrowser<T> {
    /// Create a CDP browser backend with a short, deterministic wait poll interval.
    #[must_use]
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            poll_interval_ms: 25,
        }
    }

    /// Return the injected transport after execution or inspection.
    #[must_use]
    pub fn into_transport(self) -> T {
        self.transport
    }
}

#[cfg(feature = "websocket")]
impl CdpBrowser<JsonCdpTransport<WebSocketCdpWire>> {
    /// Connect a Rust DOM backend to a local Chromium `DevTools` endpoint.
    ///
    /// # Errors
    ///
    /// Returns a backend error when the WebSocket endpoint cannot be opened.
    pub fn connect(endpoint: &str) -> Result<Self, BrowserError> {
        Ok(Self::new(JsonCdpTransport::new(WebSocketCdpWire::connect(
            endpoint,
        )?)))
    }
}

impl<T: CdpTransport> BrowserBackend for CdpBrowser<T> {
    fn navigate(&mut self, url: &str) -> Result<(), BrowserError> {
        self.transport
            .command("Page.navigate", json!({"url": url}))?;
        Ok(())
    }

    fn click(&mut self, selector: &str) -> Result<(), BrowserError> {
        let selector = json_string(selector);
        self.evaluate(&format!(
            "(() => {{ const e = document.querySelector({selector}); if (!e) return false; e.click(); return true; }})()"
        ))?;
        Ok(())
    }

    fn fill(&mut self, selector: &str, text: &str) -> Result<(), BrowserError> {
        let selector = json_string(selector);
        let text = json_string(text);
        self.evaluate(&format!(
            "(() => {{ const e = document.querySelector({selector}); if (!e) return false; e.focus(); e.value = {text}; e.dispatchEvent(new Event('input', {{bubbles:true}})); e.dispatchEvent(new Event('change', {{bubbles:true}})); return true; }})()"
        ))?;
        Ok(())
    }

    fn wait_for(
        &mut self,
        selector: &str,
        state: WaitState,
        timeout_ms: u64,
    ) -> Result<(), BrowserError> {
        let deadline = std::time::Instant::now()
            .checked_add(std::time::Duration::from_millis(timeout_ms))
            .ok_or(BrowserError::InvalidTimeout)?;
        let selector = json_string(selector);
        let expression = match state {
            WaitState::Attached => format!("!!document.querySelector({selector})"),
            WaitState::Detached => format!("!document.querySelector({selector})"),
            WaitState::Visible => format!(
                "(() => {{ const e = document.querySelector({selector}); return !!e && !!(e.offsetWidth || e.offsetHeight || e.getClientRects().length); }})()"
            ),
            WaitState::Hidden => format!(
                "(() => {{ const e = document.querySelector({selector}); return !e || !(e.offsetWidth || e.offsetHeight || e.getClientRects().length); }})()"
            ),
        };
        loop {
            if self.evaluate(&expression)? {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(BrowserError::BackendUnavailable(format!(
                    "selector did not reach state {state:?} before timeout"
                )));
            }
            std::thread::sleep(std::time::Duration::from_millis(self.poll_interval_ms));
        }
    }
}

impl<T: CdpTransport> CdpBrowser<T> {
    /// Preview selector matches without mutating the page or scenario state.
    ///
    /// # Errors
    ///
    /// Returns a validation error for an empty or invalid selector and a
    /// backend error when the browser cannot evaluate the inspection script.
    pub fn preview_selector(&mut self, selector: &str) -> Result<SelectorPreview, BrowserError> {
        if selector.trim().is_empty() {
            return Err(BrowserError::EmptySelector);
        }
        let selector = json_string(selector.trim());
        let expression = format!(
            "(() => {{ try {{ const s = {selector}; const elements = [...document.querySelectorAll(s)]; const esc = value => CSS.escape(value); const stable = element => element.id ? '#' + esc(element.id) : element.getAttribute('data-testid') ? '[data-testid=\"' + element.getAttribute('data-testid').replaceAll('\\\\', '\\\\\\\\').replaceAll('\"', '\\\"') + '\"]' : element.tagName.toLowerCase() + [...element.classList].filter(Boolean).slice(0, 2).map(name => '.' + esc(name)).join(''); const samples = elements.slice(0, 5).map(element => ({{ tag: element.tagName.toLowerCase(), text: (element.innerText || element.getAttribute('aria-label') || '').trim().slice(0, 120), id: element.id || '', testid: element.getAttribute('data-testid') || '', visible: !!(element.offsetWidth || element.offsetHeight || element.getClientRects().length), selector: stable(element) }})); const repairs = []; if (!elements.length) {{ const id = s.match(/^(?:[a-zA-Z][\\w-]*)?#([\\w-]+)$/); const testid = s.match(/^\\[data-testid=[\\\"']([^\\\"']+)[\\\"']\\]$/); const className = s.match(/^(?:[a-zA-Z][\\w-]*)?\\.([\\w-]+)$/); const candidates = id ? ['[data-testid=\"' + id[1] + '\"]', '[name=\"' + id[1] + '\"]'] : testid ? ['#' + testid[1], '[aria-label=\"' + testid[1] + '\"]'] : className ? ['[class~=\"' + className[1] + '\"]'] : []; for (const candidate of candidates) {{ const count = document.querySelectorAll(candidate).length; if (count) repairs.push({{ selector: candidate, count }}); }} }} return {{ selector: s, count: elements.length, samples, suggested_selector: elements.length ? stable(elements[0]) : null, repair_suggestions: repairs }}; }} catch (error) {{ return {{ invalid_selector: String(error) }}; }} }})()"
        );
        let value = self.evaluate_value(&expression)?;
        if let Some(error) = value.get("invalid_selector").and_then(Value::as_str) {
            return Err(BrowserError::InvalidSelector(error.to_owned()));
        }
        serde_json::from_value(value).map_err(|error| {
            BrowserError::BackendUnavailable(format!("decode selector preview: {error}"))
        })
    }

    fn evaluate(&mut self, expression: &str) -> Result<bool, BrowserError> {
        let value = self.evaluate_value(expression)?;
        value.as_bool().ok_or_else(|| {
            BrowserError::BackendUnavailable("CDP evaluation returned no boolean value".to_owned())
        })
    }

    fn evaluate_value(&mut self, expression: &str) -> Result<Value, BrowserError> {
        let result = self.transport.command(
            "Runtime.evaluate",
            json!({"expression": expression, "returnByValue": true}),
        )?;
        result
            .pointer("/result/result/value")
            .cloned()
            .ok_or_else(|| {
                BrowserError::BackendUnavailable("CDP evaluation returned no value".to_owned())
            })
    }
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a string cannot fail")
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
        BrowserAction, BrowserBackend, BrowserError, CdpBrowser, CdpTransport, CdpWire,
        JsonCdpTransport, RecordingBrowser, SelectorPreview, SelectorRepair, SelectorSample,
        WaitState, execute,
    };
    use serde_json::{Value, json};

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

    #[derive(Debug, Default)]
    struct RecordingCdp {
        commands: Vec<(String, Value)>,
        preview: bool,
    }

    impl CdpTransport for RecordingCdp {
        fn command(&mut self, method: &str, params: Value) -> Result<Value, BrowserError> {
            self.commands.push((method.to_owned(), params));
            if self.preview {
                return Ok(json!({"result": {"result": {"value": {
                    "selector": "#submit",
                    "count": 1,
                    "samples": [{"tag": "button", "text": "Submit", "id": "submit", "testid": "", "visible": true, "selector": "#submit"}],
                    "suggested_selector": "#submit",
                    "repair_suggestions": [{"selector": "[data-testid=\"submit\"]", "count": 1}]
                }}}}));
            }
            if method == "Runtime.evaluate" {
                Ok(json!({"result": {"result": {"value": true}}}))
            } else {
                Ok(json!({}))
            }
        }
    }

    #[test]
    fn emits_safe_cdp_commands_for_dom_operations() {
        let mut browser = CdpBrowser::new(RecordingCdp::default());
        browser.navigate("https://example.test").expect("navigate");
        browser.click("#submit").expect("click");
        browser.fill("#name", "Ada").expect("fill");
        browser
            .wait_for("#result", WaitState::Visible, 100)
            .expect("wait");
        let transport = browser.into_transport();

        assert_eq!(
            transport
                .commands
                .iter()
                .map(|(method, _)| method.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Page.navigate",
                "Runtime.evaluate",
                "Runtime.evaluate",
                "Runtime.evaluate"
            ]
        );
        assert!(transport.commands[1].1["expression"].as_str().is_some());
        assert_eq!(transport.commands[0].1["url"], "https://example.test");
    }

    #[derive(Debug, Default)]
    struct RecordingWire {
        sent: Vec<String>,
        responses: Vec<String>,
    }

    impl CdpWire for RecordingWire {
        fn send_text(&mut self, payload: &str) -> Result<(), BrowserError> {
            self.sent.push(payload.to_owned());
            Ok(())
        }

        fn receive_text(&mut self) -> Result<String, BrowserError> {
            if self.responses.is_empty() {
                return Err(BrowserError::BackendUnavailable(
                    "test wire ran out of responses".to_owned(),
                ));
            }
            Ok(self.responses.remove(0))
        }
    }

    #[test]
    fn correlates_json_cdp_responses_and_ignores_events() {
        let wire = RecordingWire {
            responses: vec![
                r#"{"method":"Page.loadEventFired"}"#.to_owned(),
                r#"{"id":1,"result":{"ok":true}}"#.to_owned(),
            ],
            ..RecordingWire::default()
        };
        let mut transport = JsonCdpTransport::new(wire);
        let response = transport
            .command("Page.enable", json!({}))
            .expect("matching response");
        assert_eq!(response["result"]["ok"], true);
        let wire = transport.into_wire();
        assert_eq!(wire.sent.len(), 1);
        let command: Value = serde_json::from_str(&wire.sent[0]).expect("valid command");
        assert_eq!(command["id"], 1);
        assert_eq!(command["method"], "Page.enable");
    }

    #[test]
    fn fails_closed_on_cdp_error_or_mismatched_response() {
        let mut transport = JsonCdpTransport::new(RecordingWire {
            responses: vec![r#"{"id":1,"error":{"code":-1,"message":"nope"}}"#.to_owned()],
            ..RecordingWire::default()
        });
        assert!(matches!(
            transport.command("Page.enable", json!({})),
            Err(BrowserError::BackendUnavailable(message)) if message.contains("failed")
        ));

        let mut transport = JsonCdpTransport::new(RecordingWire {
            responses: vec![r#"{"id":99,"result":{}}"#.to_owned()],
            ..RecordingWire::default()
        });
        assert!(matches!(
            transport.command("Page.enable", json!({})),
            Err(BrowserError::BackendUnavailable(message)) if message.contains("expected 1")
        ));
    }

    #[test]
    fn previews_selector_matches_and_rejects_empty_selector() {
        let mut browser = CdpBrowser::new(RecordingCdp {
            commands: Vec::new(),
            preview: true,
        });
        let preview = browser.preview_selector("#submit").expect("preview");
        assert_eq!(
            preview,
            SelectorPreview {
                selector: "#submit".to_owned(),
                count: 1,
                samples: vec![SelectorSample {
                    tag: "button".to_owned(),
                    text: "Submit".to_owned(),
                    id: "submit".to_owned(),
                    testid: String::new(),
                    visible: true,
                    selector: "#submit".to_owned(),
                }],
                suggested_selector: Some("#submit".to_owned()),
                repair_suggestions: vec![SelectorRepair {
                    selector: "[data-testid=\"submit\"]".to_owned(),
                    count: 1,
                }],
            }
        );
        assert_eq!(
            browser.preview_selector(" "),
            Err(BrowserError::EmptySelector)
        );
    }
}
