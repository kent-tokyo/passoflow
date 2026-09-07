//! PassoFlow-owned input contracts and safety checks.
//!
//! This crate intentionally contains no platform automation dependency. OS-specific
//! adapters implement [`InputBackend`] and are kept outside the portable contract.

#![cfg_attr(not(windows), forbid(unsafe_code))]
#![cfg_attr(windows, allow(unsafe_code))]

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A point in a declared screen coordinate space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

/// A screen or window rectangle using top-left origin and positive dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    /// Whether the point is inside the half-open rectangle.
    #[must_use]
    pub fn contains(self, point: Point) -> bool {
        let right = i64::from(self.left) + i64::from(self.width);
        let bottom = i64::from(self.top) + i64::from(self.height);
        i64::from(point.x) >= i64::from(self.left)
            && i64::from(point.y) >= i64::from(self.top)
            && i64::from(point.x) < right
            && i64::from(point.y) < bottom
    }
}

/// Coordinate origin used by an input event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinateSpace {
    Screen,
    ActiveWindow,
}

/// A display scale represented exactly as a positive rational number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleFactor {
    pub numerator: u32,
    pub denominator: u32,
}

impl ScaleFactor {
    /// Create a scale factor without allowing a zero denominator.
    /// # Errors
    ///
    /// Returns [`InputError::InvalidScaleFactor`] when the denominator is zero
    /// or the numerator is zero.
    pub fn new(numerator: u32, denominator: u32) -> Result<Self, InputError> {
        if numerator == 0 || denominator == 0 {
            return Err(InputError::InvalidScaleFactor);
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }

    /// Scale a logical length up to a physical pixel count, rounding upward.
    #[must_use]
    pub fn to_physical(self, logical: u32) -> u32 {
        logical
            .saturating_mul(self.numerator)
            .saturating_add(self.denominator - 1)
            / self.denominator
    }
}

/// Permission state required by a native input adapter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionState {
    Granted,
    Denied,
    #[default]
    Unknown,
}

/// Stable description of one display in a multi-monitor layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayInfo {
    pub id: String,
    pub bounds: Rect,
    pub scale_factor: ScaleFactor,
    pub primary: bool,
}

/// Keyboard layout reported by a platform adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyboardLayout {
    pub name: String,
    pub locale: Option<String>,
}

/// Platform facts and permission diagnostics exposed before execution.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformInfo {
    #[serde(default)]
    pub displays: Vec<DisplayInfo>,
    pub keyboard_layout: Option<KeyboardLayout>,
    pub accessibility_permission: PermissionState,
}

/// Snapshot of the current foreground window for guarded visual actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveWindowInfo {
    pub title: String,
    pub bounds: Rect,
}

/// Mouse button supported by the portable input contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// A platform-neutral event that an adapter may execute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputEvent {
    MoveTo {
        point: Point,
        space: CoordinateSpace,
    },
    Click {
        point: Point,
        space: CoordinateSpace,
        button: MouseButton,
        count: u8,
        indicator_duration_ms: u64,
    },
    Scroll {
        delta_x: i32,
        delta_y: i32,
    },
    PressKey {
        key: String,
    },
    Hotkey {
        keys: Vec<String>,
    },
    TypeText {
        text: String,
    },
}

/// Safety and usability controls shared by platform adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputConfig {
    pub screen_bounds: Option<Rect>,
    pub fail_safe: bool,
    pub fail_safe_point: Point,
    pub click_indicator_duration_ms: u64,
    pub max_text_length: usize,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            screen_bounds: None,
            fail_safe: true,
            fail_safe_point: Point { x: 0, y: 0 },
            click_indicator_duration_ms: 250,
            max_text_length: 1_000_000,
        }
    }
}

/// Errors raised before an event reaches an OS adapter.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum InputError {
    #[error("point ({x}, {y}) is outside the configured screen bounds")]
    OutsideScreen { x: i32, y: i32 },
    #[error("fail-safe point ({x}, {y}) was selected")]
    FailSafeTriggered { x: i32, y: i32 },
    #[error("coordinate space {space:?} is unavailable from this input adapter")]
    CoordinateSpaceUnavailable { space: CoordinateSpace },
    #[error("key name must not be empty")]
    EmptyKey,
    #[error("hotkey must contain at least one key")]
    EmptyHotkey,
    #[error("text length {actual} exceeds the configured limit {limit}")]
    TextTooLong { actual: usize, limit: usize },
    #[error("click count must be 1 or 2")]
    InvalidClickCount,
    #[error("scale factor numerator and denominator must be positive")]
    InvalidScaleFactor,
    #[error("accessibility permission is {0:?}")]
    PermissionDenied(PermissionState),
    #[error("native input is unavailable on {platform}: {reason}")]
    UnsupportedPlatform { platform: String, reason: String },
    #[error("input backend failed: {0}")]
    Backend(String),
}

/// OS-specific implementations execute validated events through this trait.
pub trait InputBackend {
    /// Execute one event. The event has already passed portable safety checks.
    /// # Errors
    ///
    /// Returns an adapter-specific [`InputError::Backend`] when the OS refuses
    /// or cannot complete the event.
    fn execute(&mut self, event: &InputEvent) -> Result<(), InputError>;

    /// Resolve an event point into screen coordinates before safety checks.
    ///
    /// Adapters with active-window support should override this method. The
    /// screen-space default keeps recording and other screen-only adapters useful,
    /// while unsupported relative coordinates fail closed.
    /// # Errors
    ///
    /// Returns an adapter error when the active window cannot be resolved.
    fn resolve_point(&self, point: Point, space: CoordinateSpace) -> Result<Point, InputError> {
        match space {
            CoordinateSpace::Screen => Ok(point),
            CoordinateSpace::ActiveWindow => Err(InputError::CoordinateSpaceUnavailable { space }),
        }
    }

    /// Report displays, keyboard layout, and permission state before execution.
    fn platform_info(&self) -> PlatformInfo {
        PlatformInfo::default()
    }
}

/// Explicit safe fallback used until a platform-native adapter is available.
///
/// It never synthesizes input. Keeping this as a real backend makes unsupported
/// platform and permission states observable to callers instead of accidentally
/// treating a no-op as a successful action.
#[derive(Debug, Clone)]
pub struct UnavailableInput {
    pub platform: String,
    pub reason: String,
    pub info: PlatformInfo,
}

impl UnavailableInput {
    /// Describe the current target when no native adapter is compiled yet.
    #[must_use]
    pub fn current() -> Self {
        Self {
            platform: std::env::consts::OS.to_owned(),
            reason: "native input adapter is not implemented for this target".to_owned(),
            info: PlatformInfo::default(),
        }
    }
}

impl InputBackend for UnavailableInput {
    fn execute(&mut self, _event: &InputEvent) -> Result<(), InputError> {
        Err(InputError::UnsupportedPlatform {
            platform: self.platform.clone(),
            reason: self.reason.clone(),
        })
    }

    fn platform_info(&self) -> PlatformInfo {
        self.info.clone()
    }
}

/// Windows-native input adapter using the platform `user32` API directly.
#[cfg(windows)]
#[derive(Debug)]
pub struct WindowsInput {
    info: PlatformInfo,
}

#[cfg(windows)]
impl WindowsInput {
    /// Inspect the primary display and create a Win32 input adapter.
    #[must_use]
    pub fn new() -> Self {
        let left = unsafe { get_system_metrics(SM_XVIRTUALSCREEN) };
        let top = unsafe { get_system_metrics(SM_YVIRTUALSCREEN) };
        let width = unsafe { get_system_metrics(SM_CXVIRTUALSCREEN) };
        let height = unsafe { get_system_metrics(SM_CYVIRTUALSCREEN) };
        let displays = if width > 0 && height > 0 {
            vec![DisplayInfo {
                id: "virtual-desktop".to_owned(),
                bounds: Rect {
                    left,
                    top,
                    width: u32::try_from(width).unwrap_or(0),
                    height: u32::try_from(height).unwrap_or(0),
                },
                scale_factor: ScaleFactor {
                    numerator: 1,
                    denominator: 1,
                },
                primary: true,
            }]
        } else {
            Vec::new()
        };
        let keyboard_layout = current_keyboard_layout();
        Self {
            info: PlatformInfo {
                displays,
                keyboard_layout,
                accessibility_permission: PermissionState::Unknown,
            },
        }
    }

    /// Create a controller bounded to the detected Windows virtual desktop.
    #[must_use]
    pub fn controller() -> InputController<Self> {
        let backend = Self::new();
        let screen_bounds = backend.info.displays.first().map(|display| display.bounds);
        InputController::new(
            backend,
            InputConfig {
                screen_bounds,
                ..InputConfig::default()
            },
        )
    }

    /// Read the current foreground window title and screen-space bounds.
    /// # Errors
    ///
    /// Returns an error when there is no foreground window or its bounds are
    /// invalid. An empty title is preserved because some native windows have
    /// no caption.
    pub fn active_window(&self) -> Result<ActiveWindowInfo, InputError> {
        let window = unsafe { get_foreground_window() };
        if window == 0 {
            return Err(Self::native_error("GetForegroundWindow"));
        }
        let mut bounds = WindowRect::default();
        if unsafe { get_window_rect(window, &mut bounds) } == 0
            || bounds.right <= bounds.left
            || bounds.bottom <= bounds.top
        {
            return Err(Self::native_error("GetWindowRect"));
        }
        let mut title = [0u16; 512];
        let title_capacity = i32::try_from(title.len()).unwrap_or(i32::MAX);
        let length = unsafe { get_window_text(window, title.as_mut_ptr(), title_capacity) };
        let title = String::from_utf16_lossy(&title[..usize::try_from(length).unwrap_or(0)]);
        Ok(ActiveWindowInfo {
            title,
            bounds: Rect {
                left: bounds.left,
                top: bounds.top,
                width: u32::try_from(bounds.right - bounds.left)
                    .map_err(|_| Self::native_error("GetWindowRect width"))?,
                height: u32::try_from(bounds.bottom - bounds.top)
                    .map_err(|_| Self::native_error("GetWindowRect height"))?,
            },
        })
    }

    fn native_error(operation: &str) -> InputError {
        InputError::Backend(format!("Win32 user32 {operation} failed"))
    }
}

#[cfg(windows)]
impl Default for WindowsInput {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
impl InputBackend for WindowsInput {
    fn execute(&mut self, event: &InputEvent) -> Result<(), InputError> {
        match event {
            InputEvent::MoveTo { point, space } => {
                let point = screen_point(*point, *space)?;
                if unsafe { set_cursor_pos(point.x, point.y) } == 0 {
                    return Err(Self::native_error("SetCursorPos"));
                }
            }
            InputEvent::Click {
                point,
                space,
                button,
                count,
                ..
            } => {
                let point = screen_point(*point, *space)?;
                if unsafe { set_cursor_pos(point.x, point.y) } == 0 {
                    return Err(Self::native_error("SetCursorPos"));
                }
                let (down, up) = mouse_flags(*button);
                for _ in 0..*count {
                    unsafe {
                        mouse_event(down, 0, 0, 0, 0);
                        mouse_event(up, 0, 0, 0, 0);
                    }
                }
            }
            InputEvent::Scroll { delta_y, .. } => unsafe {
                mouse_event(
                    MOUSEEVENTF_WHEEL,
                    0,
                    0,
                    u32::from_ne_bytes(delta_y.to_ne_bytes()),
                    0,
                );
            },
            InputEvent::PressKey { key } => press_named_key(key)?,
            InputEvent::Hotkey { keys } => {
                let virtual_keys: Vec<_> = keys
                    .iter()
                    .map(|key| virtual_key(key))
                    .collect::<Result<_, _>>()?;
                unsafe {
                    for key in &virtual_keys {
                        keybd_event(*key, 0, 0, 0);
                    }
                    for key in virtual_keys.iter().rev() {
                        keybd_event(*key, 0, KEYEVENTF_KEYUP, 0);
                    }
                }
            }
            InputEvent::TypeText { text } => {
                for unit in text.encode_utf16() {
                    send_unicode_unit(unit)?;
                }
            }
        }
        Ok(())
    }

    fn platform_info(&self) -> PlatformInfo {
        self.info.clone()
    }

    fn resolve_point(&self, point: Point, space: CoordinateSpace) -> Result<Point, InputError> {
        screen_point(point, space)
    }
}

#[cfg(windows)]
fn screen_point(point: Point, space: CoordinateSpace) -> Result<Point, InputError> {
    if space == CoordinateSpace::Screen {
        return Ok(point);
    }
    let window = unsafe { get_foreground_window() };
    if window == 0 {
        return Err(InputError::Backend(
            "Win32 user32 GetForegroundWindow returned no active window".to_owned(),
        ));
    }
    let mut bounds = WindowRect::default();
    if unsafe { get_window_rect(window, &mut bounds) } == 0
        || bounds.right <= bounds.left
        || bounds.bottom <= bounds.top
    {
        return Err(InputError::Backend(
            "Win32 user32 GetWindowRect returned invalid active-window bounds".to_owned(),
        ));
    }
    Ok(Point {
        x: bounds.left.saturating_add(point.x),
        y: bounds.top.saturating_add(point.y),
    })
}

#[cfg(windows)]
fn current_keyboard_layout() -> Option<KeyboardLayout> {
    let mut buffer = [0u16; KL_NAMELENGTH as usize];
    if unsafe { get_keyboard_layout_name(buffer.as_mut_ptr()) } == 0 {
        return None;
    }
    let end = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
    Some(KeyboardLayout {
        name: String::from_utf16_lossy(&buffer[..end]),
        locale: None,
    })
}

#[cfg(windows)]
fn press_named_key(key: &str) -> Result<(), InputError> {
    let virtual_key = virtual_key(key)?;
    unsafe {
        keybd_event(virtual_key, 0, 0, 0);
        keybd_event(virtual_key, 0, KEYEVENTF_KEYUP, 0);
    }
    Ok(())
}

#[cfg(windows)]
fn send_unicode_unit(unit: u16) -> Result<(), InputError> {
    let mut inputs = [
        NativeInput {
            kind: INPUT_KEYBOARD,
            data: NativeInputData {
                keyboard: NativeKeyboardInput {
                    virtual_key: 0,
                    scan_code: unit,
                    flags: KEYEVENTF_UNICODE,
                    time: 0,
                    extra_info: 0,
                },
            },
        },
        NativeInput {
            kind: INPUT_KEYBOARD,
            data: NativeInputData {
                keyboard: NativeKeyboardInput {
                    virtual_key: 0,
                    scan_code: unit,
                    flags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                    time: 0,
                    extra_info: 0,
                },
            },
        },
    ];
    let sent = unsafe {
        send_input(
            u32::try_from(inputs.len()).expect("Unicode input count fits Win32 UINT"),
            inputs.as_mut_ptr(),
            i32::try_from(std::mem::size_of::<NativeInput>())
                .expect("NativeInput size fits Win32 INT"),
        )
    };
    if sent != u32::try_from(inputs.len()).expect("Unicode input count fits Win32 UINT") {
        return Err(WindowsInput::native_error("SendInput (Unicode)"));
    }
    Ok(())
}

#[cfg(windows)]
fn virtual_key(key: &str) -> Result<u8, InputError> {
    let normalized = key.trim().to_ascii_lowercase();
    let code = match normalized.as_str() {
        "backspace" => 0x08,
        "tab" => 0x09,
        "enter" | "return" => 0x0D,
        "shift" => 0x10,
        "ctrl" | "control" => 0x11,
        "alt" => 0x12,
        "esc" | "escape" => 0x1B,
        "space" => 0x20,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        "delete" => 0x2E,
        value if value.len() == 1 && value.as_bytes()[0].is_ascii_alphanumeric() => {
            value.as_bytes()[0].to_ascii_uppercase()
        }
        value
            if value
                .strip_prefix('f')
                .and_then(|number| number.parse::<u8>().ok())
                .is_some_and(|number| (1..=12).contains(&number)) =>
        {
            0x70 + value[1..].parse::<u8>().expect("validated function key") - 1
        }
        _ => {
            return Err(InputError::Backend(format!(
                "unsupported Windows key: {key}"
            )));
        }
    };
    Ok(code)
}

#[cfg(windows)]
fn mouse_flags(button: MouseButton) -> (u32, u32) {
    match button {
        MouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
        MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        MouseButton::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
    }
}

#[cfg(windows)]
const SM_XVIRTUALSCREEN: i32 = 76;
#[cfg(windows)]
const SM_YVIRTUALSCREEN: i32 = 77;
#[cfg(windows)]
const SM_CXVIRTUALSCREEN: i32 = 78;
#[cfg(windows)]
const SM_CYVIRTUALSCREEN: i32 = 79;
#[cfg(windows)]
const KL_NAMELENGTH: u32 = 9;
#[cfg(windows)]
const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
#[cfg(windows)]
const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
#[cfg(windows)]
const MOUSEEVENTF_RIGHTDOWN: u32 = 0x0008;
#[cfg(windows)]
const MOUSEEVENTF_RIGHTUP: u32 = 0x0010;
#[cfg(windows)]
const MOUSEEVENTF_MIDDLEDOWN: u32 = 0x0020;
#[cfg(windows)]
const MOUSEEVENTF_MIDDLEUP: u32 = 0x0040;
#[cfg(windows)]
const MOUSEEVENTF_WHEEL: u32 = 0x0800;
#[cfg(windows)]
const KEYEVENTF_KEYUP: u32 = 0x0002;
#[cfg(windows)]
const KEYEVENTF_UNICODE: u32 = 0x0004;
#[cfg(windows)]
const INPUT_KEYBOARD: u32 = 1;

#[cfg(windows)]
#[link(name = "user32")]
unsafe extern "system" {
    fn GetSystemMetrics(index: i32) -> i32;
    fn GetForegroundWindow() -> isize;
    fn GetWindowRect(window: isize, rect: *mut WindowRect) -> i32;
    fn GetWindowTextW(window: isize, text: *mut u16, max_count: i32) -> i32;
    fn GetKeyboardLayoutNameW(buffer: *mut u16) -> i32;
    fn SendInput(count: u32, inputs: *mut NativeInput, size: i32) -> u32;
    fn SetCursorPos(x: i32, y: i32) -> i32;
    fn mouse_event(flags: u32, dx: u32, dy: u32, data: u32, extra_info: usize);
    fn keybd_event(virtual_key: u8, scan_code: u8, flags: u32, extra_info: usize);
}

#[cfg(windows)]
use self::{
    GetForegroundWindow as get_foreground_window,
    GetKeyboardLayoutNameW as get_keyboard_layout_name, GetSystemMetrics as get_system_metrics,
    GetWindowRect as get_window_rect, GetWindowTextW as get_window_text,
    SetCursorPos as set_cursor_pos,
};

#[cfg(windows)]
#[repr(C)]
#[derive(Debug, Default)]
struct WindowRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct NativeKeyboardInput {
    virtual_key: u16,
    scan_code: u16,
    flags: u32,
    time: u32,
    extra_info: usize,
}

#[cfg(windows)]
#[repr(C)]
union NativeInputData {
    keyboard: NativeKeyboardInput,
}

#[cfg(windows)]
#[repr(C)]
struct NativeInput {
    kind: u32,
    data: NativeInputData,
}

#[cfg(windows)]
use self::SendInput as send_input;

/// Portable controller that validates and dispatches input events.
pub struct InputController<B> {
    backend: B,
    config: InputConfig,
}

impl<B: InputBackend> InputController<B> {
    /// Create a controller around a platform adapter.
    #[must_use]
    pub fn new(backend: B, config: InputConfig) -> Self {
        Self { backend, config }
    }

    /// Consume the controller and return its backend.
    #[must_use]
    pub fn into_backend(self) -> B {
        self.backend
    }

    /// Return platform facts reported by the adapter before execution.
    #[must_use]
    pub fn platform_info(&self) -> PlatformInfo {
        self.backend.platform_info()
    }

    /// Change the visible red click-indicator duration for subsequent clicks.
    pub fn set_click_indicator_duration_ms(&mut self, duration_ms: u64) {
        self.config.click_indicator_duration_ms = duration_ms;
    }

    /// Move the pointer to a point after applying coordinate safety checks.
    /// # Errors
    ///
    /// Returns an error when the point is outside configured bounds or triggers
    /// the fail-safe point, or when the backend rejects the event.
    pub fn move_to(&mut self, point: Point, space: CoordinateSpace) -> Result<(), InputError> {
        self.dispatch(&InputEvent::MoveTo { point, space })
    }

    /// Click once or twice while retaining the red-indicator duration in the event.
    /// # Errors
    ///
    /// Returns an error for unsafe coordinates, an invalid click count, or a
    /// backend failure.
    pub fn click(
        &mut self,
        point: Point,
        space: CoordinateSpace,
        button: MouseButton,
        count: u8,
    ) -> Result<(), InputError> {
        self.dispatch(&InputEvent::Click {
            point,
            space,
            button,
            count,
            indicator_duration_ms: self.config.click_indicator_duration_ms,
        })
    }

    /// Scroll by a signed horizontal/vertical delta.
    /// # Errors
    ///
    /// Returns a backend failure.
    pub fn scroll(&mut self, delta_x: i32, delta_y: i32) -> Result<(), InputError> {
        self.dispatch(&InputEvent::Scroll { delta_x, delta_y })
    }

    /// Press one named key.
    /// # Errors
    ///
    /// Returns an error when the key is empty or the backend fails.
    pub fn press_key(&mut self, key: impl Into<String>) -> Result<(), InputError> {
        self.dispatch(&InputEvent::PressKey { key: key.into() })
    }

    /// Press a simultaneous key combination.
    /// # Errors
    ///
    /// Returns an error when no keys or an empty key is supplied, or when the
    /// backend fails.
    pub fn hotkey<I, S>(&mut self, keys: I) -> Result<(), InputError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.dispatch(&InputEvent::Hotkey {
            keys: keys.into_iter().map(Into::into).collect(),
        })
    }

    /// Type text through the platform adapter, subject to the configured limit.
    /// # Errors
    ///
    /// Returns an error when the text exceeds the configured limit or the
    /// backend fails.
    pub fn type_text(&mut self, text: impl Into<String>) -> Result<(), InputError> {
        self.dispatch(&InputEvent::TypeText { text: text.into() })
    }

    fn dispatch(&mut self, event: &InputEvent) -> Result<(), InputError> {
        self.validate(event)?;
        self.backend.execute(event)
    }

    fn validate(&self, event: &InputEvent) -> Result<(), InputError> {
        let permission = self.backend.platform_info().accessibility_permission;
        if permission == PermissionState::Denied {
            return Err(InputError::PermissionDenied(permission));
        }
        match event {
            InputEvent::MoveTo { point, space } | InputEvent::Click { point, space, .. } => {
                let screen_point = self.backend.resolve_point(*point, *space)?;
                if let Some(bounds) = self.config.screen_bounds {
                    if !bounds.contains(screen_point) {
                        return Err(InputError::OutsideScreen {
                            x: screen_point.x,
                            y: screen_point.y,
                        });
                    }
                }
                if self.config.fail_safe && screen_point == self.config.fail_safe_point {
                    return Err(InputError::FailSafeTriggered {
                        x: screen_point.x,
                        y: screen_point.y,
                    });
                }
                if let InputEvent::Click { count, .. } = event {
                    if !matches!(count, 1 | 2) {
                        return Err(InputError::InvalidClickCount);
                    }
                }
            }
            InputEvent::PressKey { key } => {
                if key.trim().is_empty() {
                    return Err(InputError::EmptyKey);
                }
            }
            InputEvent::Hotkey { keys } => {
                if keys.is_empty() {
                    return Err(InputError::EmptyHotkey);
                }
                if keys.iter().any(|key| key.trim().is_empty()) {
                    return Err(InputError::EmptyKey);
                }
            }
            InputEvent::TypeText { text } => {
                if text.chars().count() > self.config.max_text_length {
                    return Err(InputError::TextTooLong {
                        actual: text.chars().count(),
                        limit: self.config.max_text_length,
                    });
                }
            }
            InputEvent::Scroll { .. } => {}
        }
        Ok(())
    }
}

/// In-memory adapter used by contract tests and future dry-run tooling.
#[derive(Debug, Default)]
pub struct RecordingInput {
    pub events: Vec<InputEvent>,
}

impl InputBackend for RecordingInput {
    fn execute(&mut self, event: &InputEvent) -> Result<(), InputError> {
        self.events.push(event.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CoordinateSpace, InputBackend, InputConfig, InputController, InputError, InputEvent,
        MouseButton, PermissionState, PlatformInfo, Point, RecordingInput, Rect, ScaleFactor,
    };

    fn controller() -> InputController<RecordingInput> {
        InputController::new(
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
        )
    }

    #[derive(Default)]
    struct OffsetInput {
        events: Vec<InputEvent>,
    }

    impl InputBackend for OffsetInput {
        fn execute(&mut self, event: &InputEvent) -> Result<(), InputError> {
            self.events.push(event.clone());
            Ok(())
        }

        fn resolve_point(&self, point: Point, space: CoordinateSpace) -> Result<Point, InputError> {
            Ok(match space {
                CoordinateSpace::Screen => point,
                CoordinateSpace::ActiveWindow => Point {
                    x: point.x + 100,
                    y: point.y + 100,
                },
            })
        }
    }

    #[derive(Default)]
    struct DeniedInput;

    impl InputBackend for DeniedInput {
        fn execute(&mut self, _event: &InputEvent) -> Result<(), InputError> {
            panic!("permission-denied input must not reach the backend")
        }

        fn platform_info(&self) -> PlatformInfo {
            PlatformInfo {
                accessibility_permission: PermissionState::Denied,
                ..PlatformInfo::default()
            }
        }
    }

    #[test]
    fn records_supported_input_with_indicator_duration() {
        let mut controller = controller();
        controller
            .click(
                Point { x: 20, y: 30 },
                CoordinateSpace::Screen,
                MouseButton::Left,
                2,
            )
            .expect("click should be accepted");
        controller.scroll(0, -3).expect("scroll should be accepted");
        controller
            .hotkey(["ctrl", "s"])
            .expect("hotkey should be accepted");
        let backend = controller.into_backend();
        assert_eq!(backend.events.len(), 3);
        assert!(matches!(
            backend.events[0],
            super::InputEvent::Click {
                count: 2,
                indicator_duration_ms: 250,
                ..
            }
        ));
    }

    #[test]
    fn blocks_unsafe_coordinates_and_invalid_text() {
        let mut controller = controller();
        assert_eq!(
            controller.move_to(Point { x: 800, y: 10 }, CoordinateSpace::Screen),
            Err(InputError::OutsideScreen { x: 800, y: 10 })
        );
        assert_eq!(
            controller.move_to(Point { x: 0, y: 0 }, CoordinateSpace::Screen),
            Err(InputError::FailSafeTriggered { x: 0, y: 0 })
        );
        assert_eq!(controller.press_key(" "), Err(InputError::EmptyKey));
    }

    #[test]
    fn rejects_active_window_coordinates_on_screen_only_adapters() {
        let mut controller = controller();
        assert_eq!(
            controller.move_to(Point { x: 10, y: 10 }, CoordinateSpace::ActiveWindow),
            Err(InputError::CoordinateSpaceUnavailable {
                space: CoordinateSpace::ActiveWindow
            })
        );
    }

    #[test]
    fn applies_fail_safe_after_active_window_resolution() {
        let mut controller = InputController::new(
            OffsetInput::default(),
            InputConfig {
                screen_bounds: Some(Rect {
                    left: 0,
                    top: 0,
                    width: 800,
                    height: 600,
                }),
                fail_safe_point: Point { x: 100, y: 100 },
                ..InputConfig::default()
            },
        );
        assert_eq!(
            controller.move_to(Point { x: 0, y: 0 }, CoordinateSpace::ActiveWindow),
            Err(InputError::FailSafeTriggered { x: 100, y: 100 })
        );
    }

    #[test]
    fn blocks_events_when_the_adapter_reports_denied_permission() {
        let mut controller = InputController::new(DeniedInput, InputConfig::default());
        assert_eq!(
            controller.press_key("enter"),
            Err(InputError::PermissionDenied(PermissionState::Denied))
        );
    }

    #[test]
    fn unsupported_platform_never_reports_a_successful_noop() {
        let mut controller = InputController::new(
            super::UnavailableInput::current(),
            InputConfig {
                fail_safe: false,
                ..InputConfig::default()
            },
        );
        assert!(matches!(
            controller.move_to(Point { x: 10, y: 10 }, CoordinateSpace::Screen),
            Err(InputError::UnsupportedPlatform { .. })
        ));
    }

    #[test]
    fn represents_display_scaling_without_float_rounding() {
        let scale = ScaleFactor::new(3, 2).expect("scale should be valid");
        assert_eq!(scale.to_physical(100), 150);
        assert_eq!(scale.to_physical(101), 152);
        assert_eq!(ScaleFactor::new(1, 0), Err(InputError::InvalidScaleFactor));
    }
}
