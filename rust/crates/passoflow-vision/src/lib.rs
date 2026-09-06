//! Deterministic template matching for `PassoFlow`.
//!
//! The matcher is platform-independent and consumes frames from
//! `passoflow-capture`. It deliberately returns an ambiguous result instead of
//! guessing when two candidates are too close to one another.

#![forbid(unsafe_code)]

use passoflow_capture::{CaptureBackend, CaptureError, CaptureRegion, CapturedFrame, ImageFrame};
use passoflow_core::ActionOutcome;
use passoflow_engine::{EngineError, StepExecutor};
use passoflow_input::{
    CoordinateSpace, InputBackend, InputController, InputError, MouseButton, Point,
};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::path::PathBuf;
use thiserror::Error;

/// A named image candidate in the order chosen by the scenario author.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    pub name: String,
    pub image: ImageFrame,
}

/// Coordinate origin for an image-search region.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegionOrigin {
    #[default]
    Screen,
    ActiveWindow,
}

/// Native window facts needed by the portable image-search safety policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveWindow {
    pub title: String,
    pub bounds: CaptureRegion,
}

/// Read the native Windows foreground window into the vision safety contract.
#[cfg(windows)]
/// # Errors
///
/// Returns the native input adapter error when the foreground window cannot be
/// inspected.
pub fn active_window_from_windows_input(
    input: &passoflow_input::WindowsInput,
) -> Result<ActiveWindow, VisionError> {
    let window = input.active_window()?;
    Ok(ActiveWindow {
        title: window.title,
        bounds: CaptureRegion {
            left: window.bounds.left,
            top: window.bounds.top,
            width: window.bounds.width,
            height: window.bounds.height,
        },
    })
}

/// Resolve a screen- or active-window-relative search region.
///
/// The returned region is in screen coordinates. An active-window region
/// requires a current window snapshot; a missing snapshot fails closed.
/// # Errors
///
/// Returns an error for a missing active window or coordinate overflow.
pub fn resolve_search_region(
    region: Option<CaptureRegion>,
    origin: RegionOrigin,
    active_window: Option<&ActiveWindow>,
) -> Result<Option<CaptureRegion>, VisionError> {
    match origin {
        RegionOrigin::Screen => Ok(region),
        RegionOrigin::ActiveWindow => {
            let window = active_window.ok_or(VisionError::ActiveWindowUnavailable)?;
            let region = region.unwrap_or(CaptureRegion {
                left: 0,
                top: 0,
                width: window.bounds.width,
                height: window.bounds.height,
            });
            let left = window
                .bounds
                .left
                .checked_add(region.left)
                .ok_or(VisionError::CoordinateOverflow)?;
            let top = window
                .bounds
                .top
                .checked_add(region.top)
                .ok_or(VisionError::CoordinateOverflow)?;
            Ok(Some(CaptureRegion {
                left,
                top,
                width: region.width,
                height: region.height,
            }))
        }
    }
}

/// Verify that the current foreground window is the requested target.
///
/// Matching is case-insensitive and uses substring semantics, preserving the
/// existing `target_window_title` behavior. An absent or empty target passes.
/// # Errors
///
/// Returns an error when the active window is unavailable or its title does not
/// contain the requested text.
pub fn ensure_target_window(
    expected_title: Option<&str>,
    active_window: Option<&ActiveWindow>,
) -> Result<(), VisionError> {
    let Some(expected) = expected_title.filter(|title| !title.is_empty()) else {
        return Ok(());
    };
    let window = active_window.ok_or(VisionError::ActiveWindowUnavailable)?;
    if window
        .title
        .to_lowercase()
        .contains(&expected.to_lowercase())
    {
        Ok(())
    } else {
        Err(VisionError::TargetWindowMismatch {
            expected: expected.to_owned(),
            actual: window.title.clone(),
        })
    }
}

/// Search controls shared by image actions.
///
/// Matching currently uses exact pixel dimensions and compares RGB channels
/// only; alpha is intentionally ignored. No implicit scaling or color-space
/// conversion is performed, so callers must capture templates at the same
/// display scale as the target frame.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SearchConfig {
    pub confidence_threshold: f64,
    pub ambiguity_margin: f64,
}

/// Retry controls for screen-search operations. `attempts` is the total
/// number of captures, with zero normalized to one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchRetryPolicy {
    pub attempts: u32,
    pub interval_ms: u64,
}

/// Matching and retry settings for the complete visual pipeline.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SearchOptions {
    pub config: SearchConfig,
    pub retry: SearchRetryPolicy,
}

impl Default for SearchRetryPolicy {
    fn default() -> Self {
        Self {
            attempts: 1,
            interval_ms: 500,
        }
    }
}

/// Injectable wait boundary for deterministic retry tests and UI integration.
pub trait RetrySleeper {
    /// Wait before the next screen capture.
    fn sleep(&mut self, interval_ms: u64);
}

/// A no-op sleeper for callers that manage timing externally.
#[derive(Debug, Default)]
pub struct NoopSleeper;

impl RetrySleeper for NoopSleeper {
    fn sleep(&mut self, _interval_ms: u64) {}
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            confidence_threshold: 0.8,
            ambiguity_margin: 0.02,
        }
    }
}

/// Screen-space location of a template match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchLocation {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

/// Named anchor on a matched image, equivalent to the current image-action
/// position choices.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MatchPosition {
    #[default]
    Center,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// A safe screen-space point derived from a match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchPoint {
    pub x: i32,
    pub y: i32,
}

/// One scored candidate, ordered by confidence then author order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchCandidate {
    pub template_index: usize,
    pub name: String,
    pub confidence: f64,
    pub location: MatchLocation,
}

impl MatchCandidate {
    /// Resolve an anchor or explicit pixel offset to a screen-space point.
    ///
    /// An offset takes precedence over the named position, matching the
    /// existing Python image-action behavior.
    /// # Errors
    ///
    /// Returns [`VisionError::CoordinateOverflow`] when the derived point
    /// cannot be represented as an `i32` screen coordinate.
    pub fn point(
        &self,
        position: MatchPosition,
        offset: Option<(i32, i32)>,
    ) -> Result<MatchPoint, VisionError> {
        let width =
            i32::try_from(self.location.width).map_err(|_| VisionError::CoordinateOverflow)?;
        let height =
            i32::try_from(self.location.height).map_err(|_| VisionError::CoordinateOverflow)?;
        let (relative_x, relative_y) = if let Some(offset) = offset {
            offset
        } else {
            match position {
                MatchPosition::Center => (
                    round_half_even(self.location.width)?,
                    round_half_even(self.location.height)?,
                ),
                MatchPosition::Top => (round_half_even(self.location.width)?, 0),
                MatchPosition::Bottom => (round_half_even(self.location.width)?, height),
                MatchPosition::Left => (0, round_half_even(self.location.height)?),
                MatchPosition::Right => (width, round_half_even(self.location.height)?),
                MatchPosition::TopLeft => (0, 0),
                MatchPosition::TopRight => (width, 0),
                MatchPosition::BottomLeft => (0, height),
                MatchPosition::BottomRight => (width, height),
            }
        };
        let x = relative_x
            .checked_add(self.location.left)
            .ok_or(VisionError::CoordinateOverflow)?;
        let y = relative_y
            .checked_add(self.location.top)
            .ok_or(VisionError::CoordinateOverflow)?;
        Ok(MatchPoint { x, y })
    }
}

fn round_half_even(value: u32) -> Result<i32, VisionError> {
    let lower = value / 2;
    let rounded = if value.is_multiple_of(2) || lower.is_multiple_of(2) {
        lower
    } else {
        lower + 1
    };
    i32::try_from(rounded).map_err(|_| VisionError::CoordinateOverflow)
}

/// Safe result of a template search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MatchDecision {
    NotFound,
    Unique(MatchCandidate),
    Ambiguous(Vec<MatchCandidate>),
}

/// Outcome of a guarded click request.
#[derive(Debug, Clone, PartialEq)]
pub enum ClickDecision {
    NotFound,
    Ambiguous(Vec<MatchCandidate>),
    Clicked(MatchCandidate),
}

/// Window-safety and click-point options for the complete visual pipeline.
#[derive(Debug, Clone, Copy)]
pub struct ClickOptions<'a> {
    pub expected_title: Option<&'a str>,
    pub active_window: Option<&'a ActiveWindow>,
    pub position: MatchPosition,
    pub offset: Option<(i32, i32)>,
    pub double_click: bool,
}

/// Loads named scenario images into portable templates.
pub trait TemplateLoader {
    /// Load one image path selected by the scenario.
    /// # Errors
    ///
    /// Returns a vision error when the path is unavailable or cannot be decoded.
    fn load(&mut self, path: &str) -> Result<Template, VisionError>;
}

/// File-backed template loader rooted at the scenario directory.
#[derive(Debug, Clone)]
pub struct FileTemplateLoader {
    root: PathBuf,
}

impl FileTemplateLoader {
    /// Create a loader rooted at a trusted scenario directory.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn resolve(&self, relative: &str) -> Result<PathBuf, VisionError> {
        let root = self
            .root
            .canonicalize()
            .map_err(|error| VisionError::ImageLoad(error.to_string()))?;
        let path = root.join(relative);
        let canonical = path
            .canonicalize()
            .map_err(|error| VisionError::ImageLoad(error.to_string()))?;
        if !canonical.starts_with(&root) {
            return Err(VisionError::ImagePathOutsideRoot {
                path: relative.to_owned(),
            });
        }
        Ok(canonical)
    }
}

impl TemplateLoader for FileTemplateLoader {
    fn load(&mut self, path: &str) -> Result<Template, VisionError> {
        let path = self.resolve(path)?;
        let image = image::ImageReader::open(&path)
            .map_err(|error| VisionError::ImageLoad(error.to_string()))?
            .decode()
            .map_err(|error| VisionError::ImageLoad(error.to_string()))?
            .to_rgba8();
        let width = image.width();
        let height = image.height();
        let image = ImageFrame::new(width, height, image.into_raw())?;
        Ok(Template {
            name: path.display().to_string(),
            image,
        })
    }
}

/// Supplies the current foreground window to guarded visual actions.
pub trait WindowContextProvider {
    /// Return a current window snapshot, or None when unavailable.
    /// # Errors
    ///
    /// Returns a native window-inspection error.
    fn active_window(&mut self) -> Result<Option<ActiveWindow>, VisionError>;
}

/// Explicit provider used by callers that do not have native window access.
#[derive(Debug, Default)]
pub struct UnavailableWindowContext;

impl WindowContextProvider for UnavailableWindowContext {
    fn active_window(&mut self) -> Result<Option<ActiveWindow>, VisionError> {
        Ok(None)
    }
}

/// Rust engine executor for the current `click_image` action.
pub struct ImageStepExecutor<C, B, L, W> {
    pub capture: C,
    pub controller: InputController<B>,
    pub loader: L,
    pub windows: W,
}

impl<C, B, L, W> ImageStepExecutor<C, B, L, W> {
    /// Create an image executor around native or recording adapters.
    #[must_use]
    pub fn new(capture: C, controller: InputController<B>, loader: L, windows: W) -> Self {
        Self {
            capture,
            controller,
            loader,
            windows,
        }
    }
}

impl<C, B, L, W> StepExecutor for ImageStepExecutor<C, B, L, W>
where
    C: CaptureBackend,
    B: InputBackend,
    L: TemplateLoader,
    W: WindowContextProvider,
{
    fn execute(
        &mut self,
        step: &passoflow_core::PlannedStep,
    ) -> Result<passoflow_core::ActionResult, EngineError> {
        if !matches!(step.action.as_str(), "click_image" | "move_mouse_to_image") {
            return Err(EngineError::Adapter(format!(
                "image executor does not support action {:?}",
                step.action
            )));
        }
        let templates = load_templates(&mut self.loader, &step.params)?;
        let region = parse_region(&step.params)?;
        let origin = parse_region_origin(&step.params)?;
        let active_window = self
            .windows
            .active_window()
            .map_err(|error| EngineError::Adapter(error.to_string()))?;
        let resolved_region = resolve_search_region(region, origin, active_window.as_ref())
            .map_err(|error| EngineError::Adapter(error.to_string()))?;
        let click = ClickOptions {
            expected_title: step
                .params
                .get("target_window_title")
                .and_then(Value::as_str),
            active_window: active_window.as_ref(),
            position: parse_position(&step.params)?,
            offset: parse_offset(&step.params)?,
            double_click: step
                .params
                .get("click_type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind == "double"),
        };
        if step.action == "click_image" {
            if let Some(value) = step.params.get("click_indicator_duration") {
                self.controller
                    .set_click_indicator_duration_ms(parse_duration_ms(value)?);
            }
        }
        let options = SearchOptions {
            config: SearchConfig {
                confidence_threshold: step
                    .params
                    .get("confidence")
                    .and_then(Value::as_f64)
                    .unwrap_or(SearchConfig::default().confidence_threshold),
                ..SearchConfig::default()
            },
            retry: SearchRetryPolicy {
                attempts: step
                    .params
                    .get("retry")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
                    .saturating_add(1)
                    .try_into()
                    .unwrap_or(u32::MAX),
                interval_ms: step
                    .params
                    .get("retry_interval_ms")
                    .and_then(Value::as_u64)
                    .unwrap_or(SearchRetryPolicy::default().interval_ms),
            },
        };
        let mut sleeper = NoopSleeper;
        if step.action == "click_image" {
            let decision = capture_and_click(
                &mut self.capture,
                &mut self.controller,
                &templates,
                resolved_region,
                options,
                &mut sleeper,
                click,
            )
            .map_err(|error| EngineError::Adapter(error.to_string()))?;
            Ok(action_result(decision, "clicked"))
        } else {
            let decision = capture_and_move(
                &mut self.capture,
                &mut self.controller,
                &templates,
                resolved_region,
                options,
                &mut sleeper,
                click,
            )
            .map_err(|error| EngineError::Adapter(error.to_string()))?;
            Ok(action_result(decision, "moved to"))
        }
    }
}

fn load_templates<L: TemplateLoader>(
    loader: &mut L,
    params: &std::collections::BTreeMap<String, Value>,
) -> Result<Vec<Template>, EngineError> {
    let images = params
        .get("images")
        .and_then(Value::as_sequence)
        .ok_or_else(|| EngineError::Adapter("click_image requires an images list".to_owned()))?;
    if images.is_empty() {
        return Err(EngineError::Adapter(
            "click_image images list is empty".to_owned(),
        ));
    }
    images
        .iter()
        .map(|image| {
            let path = image
                .as_str()
                .ok_or_else(|| EngineError::Adapter("image path must be a string".to_owned()))?;
            loader
                .load(path)
                .map_err(|error| EngineError::Adapter(error.to_string()))
        })
        .collect()
}

fn parse_region(
    params: &std::collections::BTreeMap<String, Value>,
) -> Result<Option<CaptureRegion>, EngineError> {
    let Some(values) = params.get("region").and_then(Value::as_sequence) else {
        return Ok(None);
    };
    if values.len() != 4 {
        return Err(EngineError::Adapter(
            "region must contain four values".to_owned(),
        ));
    }
    let number = |value: &Value| {
        value
            .as_i64()
            .ok_or_else(|| EngineError::Adapter("region values must be integers".to_owned()))
    };
    let left = number(&values[0])?;
    let top = number(&values[1])?;
    let width = u32::try_from(number(&values[2])?)
        .map_err(|_| EngineError::Adapter("region width must be positive".to_owned()))?;
    let height = u32::try_from(number(&values[3])?)
        .map_err(|_| EngineError::Adapter("region height must be positive".to_owned()))?;
    Ok(Some(CaptureRegion {
        left: i32::try_from(left)
            .map_err(|_| EngineError::Adapter("region left is out of range".to_owned()))?,
        top: i32::try_from(top)
            .map_err(|_| EngineError::Adapter("region top is out of range".to_owned()))?,
        width,
        height,
    }))
}

fn parse_region_origin(
    params: &std::collections::BTreeMap<String, Value>,
) -> Result<RegionOrigin, EngineError> {
    match params
        .get("region_origin")
        .and_then(Value::as_str)
        .unwrap_or("screen")
    {
        "screen" => Ok(RegionOrigin::Screen),
        "active_window" => Ok(RegionOrigin::ActiveWindow),
        value => Err(EngineError::Adapter(format!(
            "unknown region_origin {value:?}"
        ))),
    }
}

fn parse_position(
    params: &std::collections::BTreeMap<String, Value>,
) -> Result<MatchPosition, EngineError> {
    match params
        .get("position")
        .and_then(Value::as_str)
        .unwrap_or("center")
    {
        "center" => Ok(MatchPosition::Center),
        "top" => Ok(MatchPosition::Top),
        "bottom" => Ok(MatchPosition::Bottom),
        "left" => Ok(MatchPosition::Left),
        "right" => Ok(MatchPosition::Right),
        "top-left" => Ok(MatchPosition::TopLeft),
        "top-right" => Ok(MatchPosition::TopRight),
        "bottom-left" => Ok(MatchPosition::BottomLeft),
        "bottom-right" => Ok(MatchPosition::BottomRight),
        value => Err(EngineError::Adapter(format!(
            "unknown image position {value:?}"
        ))),
    }
}

fn parse_offset(
    params: &std::collections::BTreeMap<String, Value>,
) -> Result<Option<(i32, i32)>, EngineError> {
    let Some(values) = params.get("offset").and_then(Value::as_sequence) else {
        return Ok(None);
    };
    if values.len() != 2 {
        return Err(EngineError::Adapter(
            "offset must contain two values".to_owned(),
        ));
    }
    let parse = |value: &Value| {
        value
            .as_i64()
            .and_then(|number| i32::try_from(number).ok())
            .ok_or_else(|| EngineError::Adapter("offset values must be integers".to_owned()))
    };
    Ok(Some((parse(&values[0])?, parse(&values[1])?)))
}

fn parse_duration_ms(value: &Value) -> Result<u64, EngineError> {
    let seconds = value.as_f64().ok_or_else(|| {
        EngineError::Adapter("click_indicator_duration must be a number".to_owned())
    })?;
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(EngineError::Adapter(
            "click_indicator_duration must be finite and non-negative".to_owned(),
        ));
    }
    let duration = std::time::Duration::try_from_secs_f64(seconds)
        .map_err(|_| EngineError::Adapter("click_indicator_duration is too large".to_owned()))?;
    duration
        .as_millis()
        .try_into()
        .map_err(|_| EngineError::Adapter("click_indicator_duration is too large".to_owned()))
}

fn action_result(decision: ClickDecision, completed_verb: &str) -> passoflow_core::ActionResult {
    match decision {
        ClickDecision::Clicked(candidate) => passoflow_core::ActionResult {
            outcome: ActionOutcome::Success,
            message: format!(
                "{completed_verb} image candidate {} ({:.3})",
                candidate.name, candidate.confidence
            ),
            artifacts: Vec::new(),
        },
        ClickDecision::NotFound => passoflow_core::ActionResult {
            outcome: ActionOutcome::WarningContinue,
            message: "no image candidate met the confidence threshold".to_owned(),
            artifacts: Vec::new(),
        },
        ClickDecision::Ambiguous(candidates) => passoflow_core::ActionResult {
            outcome: ActionOutcome::FailureStop,
            message: format!("ambiguous image match ({} candidates)", candidates.len()),
            artifacts: Vec::new(),
        },
    }
}

/// Errors raised before a matcher can make a decision.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum VisionError {
    #[error("template list must not be empty")]
    EmptyTemplates,
    #[error("confidence threshold must be between 0 and 1")]
    InvalidConfidence,
    #[error("ambiguity margin must be between 0 and 1")]
    InvalidAmbiguityMargin,
    #[error("template {name:?} is larger than the capture frame")]
    TemplateLargerThanFrame { name: String },
    #[error("search region is invalid or outside the capture frame")]
    InvalidRegion,
    #[error("screen coordinate is outside the supported range")]
    CoordinateOverflow,
    #[error("screen capture failed: {0}")]
    Capture(#[from] CaptureError),
    #[error("input dispatch failed: {0}")]
    Input(#[from] InputError),
    #[error("image load failed: {0}")]
    ImageLoad(String),
    #[error("image path {path:?} is outside the template root")]
    ImagePathOutsideRoot { path: String },
    #[error("active window information is unavailable")]
    ActiveWindowUnavailable,
    #[error("active window title {actual:?} does not contain target {expected:?}")]
    TargetWindowMismatch { expected: String, actual: String },
}

/// Click a unique match through the `PassoFlow` input controller.
///
/// `NotFound` and `Ambiguous` are returned without dispatching any input. The
/// controller retains the configured visible click indicator and fail-safe
/// checks, so this helper does not create a second unsafe input path.
/// # Errors
///
/// Returns an anchor/offset or input-dispatch error. Ambiguous and missing
/// matches are normal non-clicking outcomes, not errors.
pub fn click_match<B: InputBackend>(
    controller: &mut InputController<B>,
    decision: MatchDecision,
    position: MatchPosition,
    offset: Option<(i32, i32)>,
    double_click: bool,
) -> Result<ClickDecision, VisionError> {
    match decision {
        MatchDecision::NotFound => Ok(ClickDecision::NotFound),
        MatchDecision::Ambiguous(candidates) => Ok(ClickDecision::Ambiguous(candidates)),
        MatchDecision::Unique(candidate) => {
            let point = candidate.point(position, offset)?;
            controller.click(
                Point {
                    x: point.x,
                    y: point.y,
                },
                CoordinateSpace::Screen,
                MouseButton::Left,
                if double_click { 2 } else { 1 },
            )?;
            Ok(ClickDecision::Clicked(candidate))
        }
    }
}

/// Move to a unique match without clicking it.
/// # Errors
///
/// Returns an anchor/offset or input-dispatch error. Missing and ambiguous
/// matches return non-moving decisions.
pub fn move_match<B: InputBackend>(
    controller: &mut InputController<B>,
    decision: MatchDecision,
    position: MatchPosition,
    offset: Option<(i32, i32)>,
) -> Result<ClickDecision, VisionError> {
    match decision {
        MatchDecision::NotFound => Ok(ClickDecision::NotFound),
        MatchDecision::Ambiguous(candidates) => Ok(ClickDecision::Ambiguous(candidates)),
        MatchDecision::Unique(candidate) => {
            let point = candidate.point(position, offset)?;
            controller.move_to(
                Point {
                    x: point.x,
                    y: point.y,
                },
                CoordinateSpace::Screen,
            )?;
            Ok(ClickDecision::Clicked(candidate))
        }
    }
}

/// Verify the optional target window and click a unique match.
/// # Errors
///
/// Returns a target-window, anchor/offset, or input-dispatch error. The
/// controller is untouched when the target guard fails.
pub fn click_match_guarded<B: InputBackend>(
    controller: &mut InputController<B>,
    decision: MatchDecision,
    expected_title: Option<&str>,
    active_window: Option<&ActiveWindow>,
    position: MatchPosition,
    offset: Option<(i32, i32)>,
    double_click: bool,
) -> Result<ClickDecision, VisionError> {
    if matches!(&decision, MatchDecision::Unique(_)) {
        ensure_target_window(expected_title, active_window)?;
    }
    click_match(controller, decision, position, offset, double_click)
}

/// Run the complete capture, search, guard, and click pipeline.
///
/// `region` is expressed in screen coordinates. The active-window snapshot is
/// used for the target-title guard; callers should resolve an
/// active-window-relative region with [`resolve_search_region`] before passing
/// it to this function.
/// # Errors
///
/// Returns a capture, search, target-window, anchor/offset, or input-dispatch
/// error. Missing and ambiguous matches return non-clicking decisions.
pub fn capture_and_click<C, B, S>(
    capture: &mut C,
    controller: &mut InputController<B>,
    templates: &[Template],
    region: Option<CaptureRegion>,
    options: SearchOptions,
    sleeper: &mut S,
    click: ClickOptions<'_>,
) -> Result<ClickDecision, VisionError>
where
    C: CaptureBackend,
    B: InputBackend,
    S: RetrySleeper,
{
    ensure_target_window(click.expected_title, click.active_window)?;
    let decision = search_with_retry(
        capture,
        templates,
        region,
        options.config,
        options.retry,
        sleeper,
    )?;
    click_match(
        controller,
        decision,
        click.position,
        click.offset,
        click.double_click,
    )
}

/// Run the complete capture, search, guard, and move pipeline.
/// # Errors
///
/// Returns a capture, search, target-window, anchor/offset, or input-dispatch
/// error. Missing and ambiguous matches return non-moving decisions.
pub fn capture_and_move<C, B, S>(
    capture: &mut C,
    controller: &mut InputController<B>,
    templates: &[Template],
    region: Option<CaptureRegion>,
    options: SearchOptions,
    sleeper: &mut S,
    click: ClickOptions<'_>,
) -> Result<ClickDecision, VisionError>
where
    C: CaptureBackend,
    B: InputBackend,
    S: RetrySleeper,
{
    ensure_target_window(click.expected_title, click.active_window)?;
    let decision = search_with_retry(
        capture,
        templates,
        region,
        options.config,
        options.retry,
        sleeper,
    )?;
    move_match(controller, decision, click.position, click.offset)
}

/// Search all templates and return a unique, missing, or ambiguous decision.
///
/// Each template contributes its best top-left position. Candidates are sorted
/// by descending confidence, then by the original template order, then by
/// screen position. A result below the threshold is `NotFound`; a second
/// candidate within the ambiguity margin makes the result `Ambiguous`.
/// # Errors
///
/// Returns an error for invalid configuration, an empty candidate list, or an
/// invalid/out-of-bounds region.
pub fn search(
    frame: &CapturedFrame,
    templates: &[Template],
    region: Option<CaptureRegion>,
    config: SearchConfig,
) -> Result<MatchDecision, VisionError> {
    validate_config(config)?;
    if templates.is_empty() {
        return Err(VisionError::EmptyTemplates);
    }
    let search_region = resolve_region(frame.image.width, frame.image.height, region)?;
    let mut candidates = Vec::with_capacity(templates.len());
    for (template_index, template) in templates.iter().enumerate() {
        if template.image.width > search_region.width
            || template.image.height > search_region.height
        {
            return Err(VisionError::TemplateLargerThanFrame {
                name: template.name.clone(),
            });
        }
        let (confidence, left, top) = best_position(&frame.image, &template.image, search_region)?;
        let screen_left = frame
            .origin
            .x
            .checked_add(left)
            .ok_or(VisionError::CoordinateOverflow)?;
        let screen_top = frame
            .origin
            .y
            .checked_add(top)
            .ok_or(VisionError::CoordinateOverflow)?;
        candidates.push(MatchCandidate {
            template_index,
            name: template.name.clone(),
            confidence,
            location: MatchLocation {
                left: screen_left,
                top: screen_top,
                width: template.image.width,
                height: template.image.height,
            },
        });
    }
    candidates.sort_by(|left, right| {
        right
            .confidence
            .total_cmp(&left.confidence)
            .then(left.template_index.cmp(&right.template_index))
            .then(left.location.top.cmp(&right.location.top))
            .then(left.location.left.cmp(&right.location.left))
    });
    let Some(best) = candidates.first() else {
        return Err(VisionError::EmptyTemplates);
    };
    if best.confidence < config.confidence_threshold {
        return Ok(MatchDecision::NotFound);
    }
    if candidates
        .get(1)
        .is_some_and(|next| next.confidence >= best.confidence - config.ambiguity_margin)
    {
        return Ok(MatchDecision::Ambiguous(candidates));
    }
    Ok(MatchDecision::Unique(best.clone()))
}

/// Capture and search repeatedly until a candidate is found or attempts end.
///
/// Only `NotFound` is retried. Unique and ambiguous matches, invalid regions,
/// and capture failures are returned immediately so safety decisions are not
/// hidden behind a retry loop.
/// # Errors
///
/// Returns the first validation or capture error raised by the backend.
pub fn search_with_retry<C, S>(
    capture: &mut C,
    templates: &[Template],
    region: Option<CaptureRegion>,
    config: SearchConfig,
    retry: SearchRetryPolicy,
    sleeper: &mut S,
) -> Result<MatchDecision, VisionError>
where
    C: CaptureBackend,
    S: RetrySleeper,
{
    let attempts = retry.attempts.max(1);
    for attempt in 1..=attempts {
        let frame = capture.capture_screen_region(region)?;
        // The capture backend has already restricted the frame to `region`;
        // matching the cropped frame with the same region would apply the
        // bounds twice.
        let decision = search(&frame, templates, None, config)?;
        if !matches!(decision, MatchDecision::NotFound) || attempt == attempts {
            return Ok(decision);
        }
        sleeper.sleep(retry.interval_ms);
    }
    unreachable!("attempts is normalized to at least one")
}

fn validate_config(config: SearchConfig) -> Result<(), VisionError> {
    if !(0.0..=1.0).contains(&config.confidence_threshold) {
        return Err(VisionError::InvalidConfidence);
    }
    if !(0.0..=1.0).contains(&config.ambiguity_margin) {
        return Err(VisionError::InvalidAmbiguityMargin);
    }
    Ok(())
}

fn resolve_region(
    width: u32,
    height: u32,
    region: Option<CaptureRegion>,
) -> Result<CaptureRegion, VisionError> {
    let region = region.unwrap_or(CaptureRegion {
        left: 0,
        top: 0,
        width,
        height,
    });
    if region.left < 0 || region.top < 0 || region.width == 0 || region.height == 0 {
        return Err(VisionError::InvalidRegion);
    }
    let (right, bottom) = region.edges();
    if right > i64::from(width) || bottom > i64::from(height) {
        return Err(VisionError::InvalidRegion);
    }
    Ok(region)
}

fn best_position(
    frame: &ImageFrame,
    template: &ImageFrame,
    region: CaptureRegion,
) -> Result<(f64, i32, i32), VisionError> {
    let region_left = usize::try_from(region.left).map_err(|_| VisionError::InvalidRegion)?;
    let region_top = usize::try_from(region.top).map_err(|_| VisionError::InvalidRegion)?;
    let max_left = region_left
        .checked_add(
            usize::try_from(region.width - template.width)
                .map_err(|_| VisionError::CoordinateOverflow)?,
        )
        .ok_or(VisionError::CoordinateOverflow)?;
    let max_top = region_top
        .checked_add(
            usize::try_from(region.height - template.height)
                .map_err(|_| VisionError::CoordinateOverflow)?,
        )
        .ok_or(VisionError::CoordinateOverflow)?;
    let mut best = (f64::NEG_INFINITY, region_left, region_top);
    for top in region_top..=max_top {
        for left in region_left..=max_left {
            let confidence = confidence_at(frame, template, left, top);
            if confidence > best.0 {
                best = (confidence, left, top);
            }
        }
    }
    Ok((
        best.0,
        i32::try_from(best.1).map_err(|_| VisionError::CoordinateOverflow)?,
        i32::try_from(best.2).map_err(|_| VisionError::CoordinateOverflow)?,
    ))
}

#[allow(clippy::cast_precision_loss)]
fn confidence_at(frame: &ImageFrame, template: &ImageFrame, left: usize, top: usize) -> f64 {
    let frame_stride = frame.width as usize * 4;
    let template_stride = template.width as usize * 4;
    let mut difference = 0u64;
    for row in 0..template.height as usize {
        let frame_start = (top + row) * frame_stride + left * 4;
        let template_start = row * template_stride;
        for column in 0..template.width as usize {
            let frame_pixel = frame_start + column * 4;
            let template_pixel = template_start + column * 4;
            for channel in 0..3 {
                difference += u64::from(
                    frame.pixels[frame_pixel + channel]
                        .abs_diff(template.pixels[template_pixel + channel]),
                );
            }
        }
    }
    let max_difference = u64::from(template.width) * u64::from(template.height) * 3 * 255;
    1.0 - difference as f64 / max_difference as f64
}

#[cfg(test)]
mod tests {
    use super::{
        ActiveWindow, ClickDecision, ClickOptions, FileTemplateLoader, ImageStepExecutor,
        MatchCandidate, MatchDecision, MatchLocation, MatchPoint, MatchPosition, RegionOrigin,
        SearchConfig, SearchOptions, Template, TemplateLoader, VisionError, WindowContextProvider,
        capture_and_click, capture_and_move, click_match_guarded, ensure_target_window,
        resolve_search_region, round_half_even, search,
    };
    use passoflow_capture::{
        CaptureBackend, CaptureError, CaptureRegion, CapturedFrame, ImageFrame, Origin,
    };
    use passoflow_core::{ActionOutcome, PlannedStep};
    use passoflow_engine::StepExecutor;
    use passoflow_input::{InputConfig, InputController, InputEvent, RecordingInput, Rect};
    use serde_yaml::Value;

    fn frame() -> CapturedFrame {
        CapturedFrame {
            origin: Origin { x: 100, y: 50 },
            image: ImageFrame::new(
                4,
                3,
                vec![
                    0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 0,
                    0, 255, 255, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255,
                    0, 0, 0, 255,
                ],
            )
            .expect("fixture dimensions should match"),
        }
    }

    fn red_pixel_template(name: &str) -> Template {
        Template {
            name: name.to_owned(),
            image: ImageFrame::new(1, 1, vec![255, 0, 0, 255]).expect("template should parse"),
        }
    }

    struct SequenceCapture {
        frames: Vec<CapturedFrame>,
    }

    impl CaptureBackend for SequenceCapture {
        fn capture(
            &mut self,
            _region: Option<CaptureRegion>,
        ) -> Result<CapturedFrame, CaptureError> {
            if self.frames.len() > 1 {
                Ok(self.frames.remove(0))
            } else {
                self.frames
                    .pop()
                    .ok_or_else(|| CaptureError::Unsupported("no frame".to_owned()))
            }
        }
    }

    #[derive(Default)]
    struct CountingSleeper(usize);

    impl super::RetrySleeper for CountingSleeper {
        fn sleep(&mut self, _interval_ms: u64) {
            self.0 += 1;
        }
    }

    #[test]
    fn returns_the_best_candidate_in_screen_coordinates() {
        let decision = search(
            &frame(),
            &[red_pixel_template("red")],
            Some(passoflow_capture::CaptureRegion {
                left: 1,
                top: 1,
                width: 1,
                height: 1,
            }),
            SearchConfig::default(),
        )
        .expect("search should succeed");
        let MatchDecision::Unique(candidate) = decision else {
            panic!("expected unique match")
        };
        assert_eq!(candidate.name, "red");
        assert_eq!(candidate.location.left, 101);
        assert_eq!(candidate.location.top, 51);
        assert!((candidate.confidence - 1.0).abs() < f64::EPSILON);
        assert_eq!(
            candidate.point(MatchPosition::Center, None),
            Ok(MatchPoint { x: 101, y: 51 })
        );
        assert_eq!(
            candidate.point(MatchPosition::Center, Some((3, -2))),
            Ok(MatchPoint { x: 104, y: 49 })
        );
    }

    #[test]
    fn exact_rgb_match_beats_alpha_only_difference() {
        let frame = CapturedFrame {
            origin: Origin::default(),
            image: ImageFrame::new(2, 1, vec![10, 20, 30, 0, 200, 100, 50, 255])
                .expect("fixture dimensions should match"),
        };
        let template = Template {
            name: "rgb".to_owned(),
            image: ImageFrame::new(1, 1, vec![10, 20, 30, 255])
                .expect("template dimensions should match"),
        };
        let decision = search(&frame, &[template], None, SearchConfig::default())
            .expect("search should succeed");
        let MatchDecision::Unique(candidate) = decision else {
            panic!("expected unique RGB match")
        };
        assert_eq!(candidate.location.left, 0);
        assert!((candidate.confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn refuses_ambiguous_candidates_and_invalid_regions() {
        let decision = search(
            &frame(),
            &[red_pixel_template("first"), red_pixel_template("second")],
            None,
            SearchConfig::default(),
        )
        .expect("search should succeed");
        assert!(matches!(decision, MatchDecision::Ambiguous(_)));
        assert_eq!(
            search(
                &frame(),
                &[red_pixel_template("red")],
                Some(passoflow_capture::CaptureRegion {
                    left: 4,
                    top: 0,
                    width: 1,
                    height: 1
                }),
                SearchConfig::default(),
            ),
            Err(VisionError::InvalidRegion)
        );
    }

    #[test]
    fn rounds_anchor_centers_like_python() {
        assert_eq!(round_half_even(1), Ok(0));
        assert_eq!(round_half_even(3), Ok(2));
        assert_eq!(round_half_even(5), Ok(2));
    }

    #[test]
    fn retries_not_found_after_capture() {
        let blank = CapturedFrame {
            origin: Origin { x: 100, y: 50 },
            image: ImageFrame::new(1, 1, vec![0, 0, 0, 255]).expect("fixture should parse"),
        };
        let found = CapturedFrame {
            origin: Origin { x: 120, y: 80 },
            image: ImageFrame::new(1, 1, vec![255, 0, 0, 255]).expect("fixture should parse"),
        };
        let mut capture = SequenceCapture {
            frames: vec![blank, found],
        };
        let mut sleeper = CountingSleeper::default();
        let decision = super::search_with_retry(
            &mut capture,
            &[red_pixel_template("red")],
            None,
            SearchConfig::default(),
            super::SearchRetryPolicy {
                attempts: 2,
                interval_ms: 30,
            },
            &mut sleeper,
        )
        .expect("retry search should succeed");
        assert!(matches!(decision, MatchDecision::Unique(_)));
        assert_eq!(sleeper.0, 1);
    }

    #[test]
    fn resolves_active_window_regions_and_fails_closed_on_title_mismatch() {
        let window = ActiveWindow {
            title: "Invoice - Excel".to_owned(),
            bounds: CaptureRegion {
                left: 100,
                top: 200,
                width: 800,
                height: 600,
            },
        };
        assert_eq!(
            resolve_search_region(
                Some(CaptureRegion {
                    left: 10,
                    top: 20,
                    width: 300,
                    height: 200,
                }),
                RegionOrigin::ActiveWindow,
                Some(&window),
            ),
            Ok(Some(CaptureRegion {
                left: 110,
                top: 220,
                width: 300,
                height: 200,
            }))
        );
        assert!(ensure_target_window(Some("excel"), Some(&window)).is_ok());
        assert_eq!(
            ensure_target_window(Some("browser"), Some(&window)),
            Err(VisionError::TargetWindowMismatch {
                expected: "browser".to_owned(),
                actual: "Invoice - Excel".to_owned(),
            })
        );
        assert_eq!(
            resolve_search_region(None, RegionOrigin::ActiveWindow, None),
            Err(VisionError::ActiveWindowUnavailable)
        );
    }

    #[test]
    fn guarded_click_dispatches_only_unique_matches() {
        let mut controller = InputController::new(
            RecordingInput::default(),
            InputConfig {
                screen_bounds: Some(Rect {
                    left: 0,
                    top: 0,
                    width: 800,
                    height: 600,
                }),
                ..InputConfig::default()
            },
        );
        let candidate = MatchCandidate {
            template_index: 0,
            name: "button".to_owned(),
            confidence: 0.99,
            location: MatchLocation {
                left: 100,
                top: 200,
                width: 20,
                height: 10,
            },
        };
        let window = ActiveWindow {
            title: "Editor".to_owned(),
            bounds: CaptureRegion {
                left: 0,
                top: 0,
                width: 800,
                height: 600,
            },
        };
        let clicked = click_match_guarded(
            &mut controller,
            MatchDecision::Unique(candidate),
            Some("editor"),
            Some(&window),
            MatchPosition::Center,
            None,
            true,
        )
        .expect("guarded click should succeed");
        assert!(matches!(clicked, ClickDecision::Clicked(_)));
        let backend = controller.into_backend();
        assert!(matches!(
            backend.events.as_slice(),
            [InputEvent::Click { count: 2, .. }]
        ));
    }

    #[test]
    fn complete_pipeline_captures_searches_and_clicks() {
        let mut capture = SequenceCapture {
            frames: vec![CapturedFrame {
                origin: Origin { x: 20, y: 30 },
                image: ImageFrame::new(1, 1, vec![255, 0, 0, 255])
                    .expect("fixture dimensions should match"),
            }],
        };
        let mut controller = InputController::new(
            RecordingInput::default(),
            InputConfig {
                screen_bounds: Some(Rect {
                    left: 0,
                    top: 0,
                    width: 800,
                    height: 600,
                }),
                ..InputConfig::default()
            },
        );
        let mut sleeper = super::NoopSleeper;
        let result = capture_and_click(
            &mut capture,
            &mut controller,
            &[red_pixel_template("red")],
            None,
            SearchOptions::default(),
            &mut sleeper,
            ClickOptions {
                expected_title: Some("editor"),
                active_window: Some(&ActiveWindow {
                    title: "Editor".to_owned(),
                    bounds: CaptureRegion {
                        left: 0,
                        top: 0,
                        width: 800,
                        height: 600,
                    },
                }),
                position: MatchPosition::Center,
                offset: None,
                double_click: false,
            },
        )
        .expect("complete pipeline should succeed");
        assert!(matches!(result, ClickDecision::Clicked(_)));
        let backend = controller.into_backend();
        assert!(matches!(
            backend.events.as_slice(),
            [InputEvent::Click { count: 1, .. }]
        ));
    }

    struct FixedLoader;

    impl TemplateLoader for FixedLoader {
        fn load(&mut self, path: &str) -> Result<Template, VisionError> {
            Ok(Template {
                name: path.to_owned(),
                image: ImageFrame::new(1, 1, vec![255, 0, 0, 255])
                    .expect("template dimensions should match"),
            })
        }
    }

    struct FixedWindow;

    impl WindowContextProvider for FixedWindow {
        fn active_window(&mut self) -> Result<Option<ActiveWindow>, VisionError> {
            Ok(Some(ActiveWindow {
                title: "Editor".to_owned(),
                bounds: CaptureRegion {
                    left: 0,
                    top: 0,
                    width: 800,
                    height: 600,
                },
            }))
        }
    }

    #[test]
    fn image_step_executor_maps_plan_parameters_to_success() {
        let mut params = std::collections::BTreeMap::new();
        params.insert(
            "images".to_owned(),
            Value::Sequence(vec![Value::String("button.png".to_owned())]),
        );
        params.insert("click_type".to_owned(), Value::String("double".to_owned()));
        params.insert(
            "click_indicator_duration".to_owned(),
            Value::Number(serde_yaml::Number::from(0.05)),
        );
        params.insert(
            "target_window_title".to_owned(),
            Value::String("editor".to_owned()),
        );
        let step = PlannedStep {
            index: 1,
            action: "click_image".to_owned(),
            params,
            branch_depth: 0,
            loop_label: None,
            loop_count: None,
            loop_table: None,
            variables_defined: Vec::new(),
            variables_referenced: Vec::new(),
        };
        let capture = SequenceCapture {
            frames: vec![CapturedFrame {
                origin: Origin { x: 20, y: 30 },
                image: ImageFrame::new(1, 1, vec![255, 0, 0, 255])
                    .expect("fixture dimensions should match"),
            }],
        };
        let controller = InputController::new(
            RecordingInput::default(),
            InputConfig {
                screen_bounds: Some(Rect {
                    left: 0,
                    top: 0,
                    width: 800,
                    height: 600,
                }),
                ..InputConfig::default()
            },
        );
        let mut executor = ImageStepExecutor::new(capture, controller, FixedLoader, FixedWindow);
        let result = executor.execute(&step).expect("executor should succeed");
        assert_eq!(result.outcome, ActionOutcome::Success);
        let backend = executor.controller.into_backend();
        assert!(matches!(
            backend.events.as_slice(),
            [InputEvent::Click {
                count: 2,
                indicator_duration_ms: 50,
                ..
            }]
        ));
    }

    #[test]
    fn image_step_move_pipeline_dispatches_only_pointer_movement() {
        let mut capture = SequenceCapture {
            frames: vec![CapturedFrame {
                origin: Origin { x: 20, y: 30 },
                image: ImageFrame::new(1, 1, vec![255, 0, 0, 255])
                    .expect("fixture dimensions should match"),
            }],
        };
        let mut controller = InputController::new(
            RecordingInput::default(),
            InputConfig {
                screen_bounds: Some(Rect {
                    left: 0,
                    top: 0,
                    width: 800,
                    height: 600,
                }),
                ..InputConfig::default()
            },
        );
        let mut sleeper = super::NoopSleeper;
        let result = capture_and_move(
            &mut capture,
            &mut controller,
            &[red_pixel_template("red")],
            None,
            SearchOptions::default(),
            &mut sleeper,
            ClickOptions {
                expected_title: None,
                active_window: None,
                position: MatchPosition::Center,
                offset: None,
                double_click: false,
            },
        )
        .expect("complete move pipeline should succeed");
        assert!(matches!(result, ClickDecision::Clicked(_)));
        let backend = controller.into_backend();
        assert!(matches!(
            backend.events.as_slice(),
            [InputEvent::MoveTo { .. }]
        ));
    }

    #[test]
    fn file_loader_rejects_paths_outside_root() {
        let loader = FileTemplateLoader::new(".");
        assert!(matches!(
            loader.resolve("../"),
            Err(VisionError::ImagePathOutsideRoot { .. })
        ));
    }
}
