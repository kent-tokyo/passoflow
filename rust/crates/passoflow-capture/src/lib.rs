//! PassoFlow-owned screen capture and region contracts.
//!
//! This crate is platform-independent. Native capture adapters provide frames;
//! region validation and cropping remain deterministic and testable here.

#![cfg_attr(not(windows), forbid(unsafe_code))]
#![cfg_attr(windows, allow(unsafe_code))]

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Top-left origin of a captured frame in screen coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    pub x: i32,
    pub y: i32,
}

/// A positive region in screen or frame-relative coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureRegion {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

impl CaptureRegion {
    /// Return the region's exclusive right and bottom edges without overflow.
    #[must_use]
    pub fn edges(self) -> (i64, i64) {
        (
            i64::from(self.left) + i64::from(self.width),
            i64::from(self.top) + i64::from(self.height),
        )
    }
}

/// A tightly packed 8-bit RGBA image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl ImageFrame {
    /// Create a frame and verify its RGBA buffer length.
    /// # Errors
    ///
    /// Returns an error when dimensions overflow or the pixel buffer is not
    /// exactly four bytes per pixel.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, CaptureError> {
        let expected = usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(CaptureError::InvalidDimensions)?;
        if pixels.len() != expected {
            return Err(CaptureError::InvalidPixelBuffer {
                expected,
                actual: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Crop a frame-relative region while preserving deterministic row order.
    /// # Errors
    ///
    /// Returns an error when the region is empty or outside the frame.
    pub fn crop(&self, region: CaptureRegion) -> Result<Self, CaptureError> {
        if region.left < 0 || region.top < 0 || region.width == 0 || region.height == 0 {
            return Err(CaptureError::InvalidRegion);
        }
        let right = u64::try_from(region.left).unwrap_or_default() + u64::from(region.width);
        let bottom = u64::try_from(region.top).unwrap_or_default() + u64::from(region.height);
        if right > u64::from(self.width) || bottom > u64::from(self.height) {
            return Err(CaptureError::RegionOutside {
                width: self.width,
                height: self.height,
            });
        }
        let row_bytes = usize::try_from(region.width)
            .ok()
            .and_then(|width| width.checked_mul(4))
            .ok_or(CaptureError::InvalidDimensions)?;
        let mut pixels = Vec::with_capacity(
            row_bytes
                .checked_mul(
                    usize::try_from(region.height).map_err(|_| CaptureError::InvalidDimensions)?,
                )
                .ok_or(CaptureError::InvalidDimensions)?,
        );
        let source_stride = usize::try_from(self.width)
            .map_err(|_| CaptureError::InvalidDimensions)?
            .checked_mul(4)
            .ok_or(CaptureError::InvalidDimensions)?;
        let left = usize::try_from(region.left).map_err(|_| CaptureError::InvalidRegion)?;
        let top = usize::try_from(region.top).map_err(|_| CaptureError::InvalidRegion)?;
        for row in 0..usize::try_from(region.height).map_err(|_| CaptureError::InvalidDimensions)? {
            let start = (top + row)
                .checked_mul(source_stride)
                .and_then(|offset| offset.checked_add(left.checked_mul(4)?))
                .ok_or(CaptureError::InvalidDimensions)?;
            pixels.extend_from_slice(&self.pixels[start..start + row_bytes]);
        }
        Self::new(region.width, region.height, pixels)
    }
}

/// A frame plus its screen-space origin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedFrame {
    pub origin: Origin,
    pub image: ImageFrame,
}

/// Errors raised before a frame reaches image matching.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CaptureError {
    #[error("capture dimensions overflow")]
    InvalidDimensions,
    #[error("pixel buffer has {actual} bytes; expected {expected}")]
    InvalidPixelBuffer { expected: usize, actual: usize },
    #[error("capture region must have non-negative origin and positive dimensions")]
    InvalidRegion,
    #[error("capture region is outside the {width}x{height} frame")]
    RegionOutside { width: u32, height: u32 },
    #[error("native screen capture is unavailable: {0}")]
    Unsupported(String),
    #[error("Windows screen capture failed: {0}")]
    Native(String),
}

/// Result of a best-effort native capture permission probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapturePermissionState {
    Granted,
    Denied,
    Unknown,
}

/// Per-window DPI metadata reported by a native capture adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DpiInfo {
    pub horizontal: u32,
    pub vertical: u32,
}

/// Native capture setup facts for first-run diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureDiagnostics {
    pub permission: CapturePermissionState,
    pub source: String,
    pub dpi: Option<DpiInfo>,
}

/// One Windows monitor and its best-effort physical DPI metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayCaptureInfo {
    pub id: String,
    pub bounds: CaptureRegion,
    pub dpi: Option<DpiInfo>,
    pub primary: bool,
}

/// Native screen/window adapters implement this interface.
pub trait CaptureBackend {
    /// Capture the full source or a source-relative region.
    /// # Errors
    ///
    /// Returns a capture or region error when the source cannot provide the
    /// requested frame.
    fn capture(&mut self, region: Option<CaptureRegion>) -> Result<CapturedFrame, CaptureError>;

    /// Capture an optional region expressed in screen coordinates.
    ///
    /// Native adapters should override this when they can restrict capture at
    /// the source. The default implementation captures the source once and
    /// crops it relative to the returned frame origin, preserving a safe
    /// fallback for recording and test backends.
    /// # Errors
    ///
    /// Returns a capture or region error when the requested screen region is
    /// unavailable.
    fn capture_screen_region(
        &mut self,
        region: Option<CaptureRegion>,
    ) -> Result<CapturedFrame, CaptureError> {
        let Some(region) = region else {
            return self.capture(None);
        };
        let frame = self.capture(None)?;
        let left = i64::from(region.left) - i64::from(frame.origin.x);
        let top = i64::from(region.top) - i64::from(frame.origin.y);
        let relative = CaptureRegion {
            left: i32::try_from(left).map_err(|_| CaptureError::InvalidRegion)?,
            top: i32::try_from(top).map_err(|_| CaptureError::InvalidRegion)?,
            width: region.width,
            height: region.height,
        };
        let image = frame.image.crop(relative)?;
        Ok(CapturedFrame {
            origin: Origin {
                x: region.left,
                y: region.top,
            },
            image,
        })
    }
}

/// Fixed-frame backend used for deterministic tests and future benchmarks.
#[derive(Debug, Clone)]
pub struct RecordingCapture {
    pub frame: CapturedFrame,
}

impl RecordingCapture {
    /// Create a recording backend from one fixed frame.
    #[must_use]
    pub fn new(frame: CapturedFrame) -> Self {
        Self { frame }
    }
}

impl CaptureBackend for RecordingCapture {
    fn capture(&mut self, region: Option<CaptureRegion>) -> Result<CapturedFrame, CaptureError> {
        let Some(region) = region else {
            return Ok(self.frame.clone());
        };
        let image = self.frame.image.crop(region)?;
        Ok(CapturedFrame {
            origin: Origin {
                x: self.frame.origin.x.saturating_add(region.left),
                y: self.frame.origin.y.saturating_add(region.top),
            },
            image,
        })
    }
}

/// Windows GDI screen capture adapter using the virtual desktop coordinates.
#[cfg(windows)]
#[derive(Debug, Clone, Copy)]
pub struct WindowsCapture {
    bounds: CaptureRegion,
}

#[cfg(windows)]
impl WindowsCapture {
    /// Inspect the virtual desktop and create a native capture adapter.
    /// # Errors
    ///
    /// Returns an error when Windows reports an invalid virtual desktop size.
    pub fn new() -> Result<Self, CaptureError> {
        let left = unsafe { get_system_metrics(SM_XVIRTUALSCREEN) };
        let top = unsafe { get_system_metrics(SM_YVIRTUALSCREEN) };
        let width = unsafe { get_system_metrics(SM_CXVIRTUALSCREEN) };
        let height = unsafe { get_system_metrics(SM_CYVIRTUALSCREEN) };
        if width <= 0 || height <= 0 {
            return Err(CaptureError::Native(
                "GetSystemMetrics returned an invalid virtual desktop".to_owned(),
            ));
        }
        Ok(Self {
            bounds: CaptureRegion {
                left,
                top,
                width: u32::try_from(width).map_err(|_| {
                    CaptureError::Native(
                        "GetSystemMetrics returned an invalid virtual desktop width".to_owned(),
                    )
                })?,
                height: u32::try_from(height).map_err(|_| {
                    CaptureError::Native(
                        "GetSystemMetrics returned an invalid virtual desktop height".to_owned(),
                    )
                })?,
            },
        })
    }

    /// Capture the current foreground window using its native HWND bounds.
    /// # Errors
    ///
    /// Returns an error when Windows has no foreground window or its bounds
    /// cannot be captured.
    pub fn capture_foreground_window(&mut self) -> Result<CapturedFrame, CaptureError> {
        let window = unsafe { get_foreground_window() };
        if window == 0 {
            return Err(CaptureError::Native(
                "GetForegroundWindow returned no window".to_owned(),
            ));
        }
        self.capture_window(window)
    }

    /// Capture a native window using its screen-space rectangle.
    /// # Errors
    ///
    /// Returns an error when the HWND is invalid or its rectangle cannot be
    /// captured.
    pub fn capture_window(&mut self, window: isize) -> Result<CapturedFrame, CaptureError> {
        let mut bounds = WindowRect::default();
        if unsafe { get_window_rect(window, &raw mut bounds) } == 0
            || bounds.right <= bounds.left
            || bounds.bottom <= bounds.top
        {
            return Err(CaptureError::Native(
                "GetWindowRect returned invalid bounds".to_owned(),
            ));
        }
        self.capture_screen_region(Some(CaptureRegion {
            left: bounds.left,
            top: bounds.top,
            width: u32::try_from(bounds.right - bounds.left)
                .map_err(|_| CaptureError::InvalidDimensions)?,
            height: u32::try_from(bounds.bottom - bounds.top)
                .map_err(|_| CaptureError::InvalidDimensions)?,
        }))
    }

    /// Read the DPI associated with a native window.
    /// # Errors
    ///
    /// Returns an error when Windows cannot provide a non-zero DPI value.
    pub fn window_dpi(&self, window: isize) -> Result<DpiInfo, CaptureError> {
        let dpi = unsafe { get_dpi_for_window(window) };
        if dpi == 0 {
            return Err(CaptureError::Native(
                "GetDpiForWindow returned zero".to_owned(),
            ));
        }
        Ok(DpiInfo {
            horizontal: dpi,
            vertical: dpi,
        })
    }

    /// Probe basic screen-capture availability without changing input state.
    #[must_use]
    pub fn diagnostics(&mut self) -> CaptureDiagnostics {
        let permission = match self.capture_screen_region(Some(CaptureRegion {
            left: self.bounds.left,
            top: self.bounds.top,
            width: 1,
            height: 1,
        })) {
            Ok(_) => CapturePermissionState::Granted,
            Err(CaptureError::Native(_)) => CapturePermissionState::Denied,
            Err(_) => CapturePermissionState::Unknown,
        };
        CaptureDiagnostics {
            permission,
            source: "windows_gdi".to_owned(),
            dpi: None,
        }
    }

    /// Enumerate Windows monitors and best-effort per-monitor DPI metadata.
    /// # Errors
    ///
    /// Returns an error when monitor enumeration itself fails. A monitor whose
    /// DPI API is unavailable remains in the result with `dpi: None`.
    pub fn displays(&self) -> Result<Vec<DisplayCaptureInfo>, CaptureError> {
        let mut displays = Vec::new();
        let result = unsafe {
            enum_display_monitors(
                0,
                std::ptr::null(),
                Some(enumerate_monitor),
                (&raw mut displays) as *mut Vec<DisplayCaptureInfo> as isize,
            )
        };
        if result == 0 {
            return Err(CaptureError::Native(
                "EnumDisplayMonitors failed".to_owned(),
            ));
        }
        Ok(displays)
    }
}

#[cfg(windows)]
impl CaptureBackend for WindowsCapture {
    fn capture(&mut self, region: Option<CaptureRegion>) -> Result<CapturedFrame, CaptureError> {
        let region = region.unwrap_or(CaptureRegion {
            left: 0,
            top: 0,
            width: self.bounds.width,
            height: self.bounds.height,
        });
        if region.left < 0 || region.top < 0 || region.width == 0 || region.height == 0 {
            return Err(CaptureError::InvalidRegion);
        }
        let right = u64::try_from(region.left).unwrap_or_default() + u64::from(region.width);
        let bottom = u64::try_from(region.top).unwrap_or_default() + u64::from(region.height);
        if right > u64::from(self.bounds.width) || bottom > u64::from(self.bounds.height) {
            return Err(CaptureError::RegionOutside {
                width: self.bounds.width,
                height: self.bounds.height,
            });
        }
        let left = self.bounds.left.saturating_add(region.left);
        let top = self.bounds.top.saturating_add(region.top);
        let image = capture_gdi(left, top, region.width, region.height)?;
        Ok(CapturedFrame {
            origin: Origin { x: left, y: top },
            image,
        })
    }

    fn capture_screen_region(
        &mut self,
        region: Option<CaptureRegion>,
    ) -> Result<CapturedFrame, CaptureError> {
        let region = region.unwrap_or(self.bounds);
        if region.width == 0 || region.height == 0 {
            return Err(CaptureError::InvalidRegion);
        }
        let right = i64::from(region.left) + i64::from(region.width);
        let bottom = i64::from(region.top) + i64::from(region.height);
        let bounds_right = i64::from(self.bounds.left) + i64::from(self.bounds.width);
        let bounds_bottom = i64::from(self.bounds.top) + i64::from(self.bounds.height);
        if i64::from(region.left) < i64::from(self.bounds.left)
            || i64::from(region.top) < i64::from(self.bounds.top)
            || right > bounds_right
            || bottom > bounds_bottom
        {
            return Err(CaptureError::RegionOutside {
                width: self.bounds.width,
                height: self.bounds.height,
            });
        }
        let image = capture_gdi(region.left, region.top, region.width, region.height)?;
        Ok(CapturedFrame {
            origin: Origin {
                x: region.left,
                y: region.top,
            },
            image,
        })
    }
}

#[cfg(windows)]
fn capture_gdi(left: i32, top: i32, width: u32, height: u32) -> Result<ImageFrame, CaptureError> {
    let width_i32 = i32::try_from(width)
        .map_err(|_| CaptureError::Native("Capture width exceeds Win32 limits".to_owned()))?;
    let height_i32 = i32::try_from(height)
        .map_err(|_| CaptureError::Native("Capture height exceeds Win32 limits".to_owned()))?;
    let screen = unsafe { get_dc(0) };
    if screen == 0 {
        return Err(CaptureError::Native("GetDC failed".to_owned()));
    }
    let memory = unsafe { create_compatible_dc(screen) };
    if memory == 0 {
        unsafe { release_dc(0, screen) };
        return Err(CaptureError::Native("CreateCompatibleDC failed".to_owned()));
    }
    let bitmap = unsafe { create_compatible_bitmap(screen, width_i32, height_i32) };
    if bitmap == 0 {
        unsafe {
            delete_dc(memory);
            release_dc(0, screen);
        }
        return Err(CaptureError::Native(
            "CreateCompatibleBitmap failed".to_owned(),
        ));
    }
    let previous = unsafe { select_object(memory, bitmap) };
    let result = capture_gdi_bitmap(
        screen, memory, bitmap, left, top, width_i32, height_i32, width, height,
    );
    unsafe {
        select_object(memory, previous);
        delete_object(bitmap);
        delete_dc(memory);
        release_dc(0, screen);
    }
    result
}

#[cfg(windows)]
#[allow(clippy::too_many_arguments)]
fn capture_gdi_bitmap(
    screen: Hdc,
    memory: Hdc,
    bitmap: Hbitmap,
    left: i32,
    top: i32,
    width_i32: i32,
    height_i32: i32,
    width: u32,
    height: u32,
) -> Result<ImageFrame, CaptureError> {
    if unsafe {
        bit_blt(
            memory, 0, 0, width_i32, height_i32, screen, left, top, SRCCOPY,
        )
    } == 0
    {
        return Err(CaptureError::Native("BitBlt failed".to_owned()));
    }
    let mut info = BitmapInfo {
        header: BitmapInfoHeader {
            size: u32::try_from(std::mem::size_of::<BitmapInfoHeader>())
                .expect("BitmapInfoHeader size fits Win32 DWORD"),
            width: width_i32,
            height: -height_i32,
            planes: 1,
            bit_count: 32,
            compression: BI_RGB,
            ..BitmapInfoHeader::default()
        },
    };
    let mut pixels = vec![
        0u8;
        usize::try_from(width)
            .unwrap_or(0)
            .saturating_mul(usize::try_from(height).unwrap_or(0))
            .saturating_mul(4)
    ];
    if unsafe {
        get_dibits(
            memory,
            bitmap,
            0,
            height,
            pixels.as_mut_ptr(),
            &raw mut info,
            DIB_RGB_COLORS,
        )
    } == 0
    {
        return Err(CaptureError::Native("GetDIBits failed".to_owned()));
    }
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    ImageFrame::new(width, height, pixels)
}

#[cfg(windows)]
type Hdc = isize;
#[cfg(windows)]
type Hbitmap = isize;
#[cfg(windows)]
type Hmonitor = isize;

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct MonitorRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(windows)]
#[repr(C)]
struct MonitorInfoEx {
    size: u32,
    monitor: MonitorRect,
    work: MonitorRect,
    flags: u32,
    device: [u16; 32],
}

#[cfg(windows)]
type MonitorEnumProc =
    Option<unsafe extern "system" fn(Hmonitor, Hdc, *const MonitorRect, isize) -> i32>;

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct BitmapInfoHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    image_size: u32,
    x_pels_per_meter: i32,
    y_pels_per_meter: i32,
    colors_used: u32,
    colors_important: u32,
}

#[cfg(windows)]
#[repr(C)]
struct BitmapInfo {
    header: BitmapInfoHeader,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct WindowRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
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
const SRCCOPY: u32 = 0x00CC_0020;
#[cfg(windows)]
const BI_RGB: u32 = 0;
#[cfg(windows)]
const DIB_RGB_COLORS: u32 = 0;

#[cfg(windows)]
#[link(name = "user32")]
unsafe extern "system" {
    fn GetSystemMetrics(index: i32) -> i32;
    fn GetDC(window: isize) -> Hdc;
    fn ReleaseDC(window: isize, dc: Hdc) -> i32;
    fn GetForegroundWindow() -> isize;
    fn GetWindowRect(window: isize, rect: *mut WindowRect) -> i32;
    fn GetDpiForWindow(window: isize) -> u32;
    fn EnumDisplayMonitors(
        dc: Hdc,
        clip: *const MonitorRect,
        callback: MonitorEnumProc,
        data: isize,
    ) -> i32;
    fn GetMonitorInfoW(monitor: Hmonitor, info: *mut MonitorInfoEx) -> i32;
}

#[cfg(windows)]
#[link(name = "shcore")]
unsafe extern "system" {
    fn GetDpiForMonitor(monitor: Hmonitor, dpi_type: u32, dpi_x: *mut u32, dpi_y: *mut u32) -> i32;
}

#[cfg(windows)]
#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateCompatibleDC(dc: Hdc) -> Hdc;
    fn CreateCompatibleBitmap(dc: Hdc, width: i32, height: i32) -> Hbitmap;
    fn SelectObject(dc: Hdc, object: isize) -> isize;
    fn DeleteObject(object: Hbitmap) -> i32;
    fn DeleteDC(dc: Hdc) -> i32;
    fn BitBlt(
        dest: Hdc,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        source: Hdc,
        source_x: i32,
        source_y: i32,
        raster: u32,
    ) -> i32;
    fn GetDIBits(
        dc: Hdc,
        bitmap: Hbitmap,
        start: u32,
        lines: u32,
        pixels: *mut u8,
        info: *mut BitmapInfo,
        usage: u32,
    ) -> i32;
}

#[cfg(windows)]
use self::{
    BitBlt as bit_blt, CreateCompatibleBitmap as create_compatible_bitmap,
    CreateCompatibleDC as create_compatible_dc, DeleteDC as delete_dc,
    DeleteObject as delete_object, EnumDisplayMonitors as enum_display_monitors, GetDC as get_dc,
    GetDIBits as get_dibits, GetDpiForWindow as get_dpi_for_window,
    GetForegroundWindow as get_foreground_window, GetMonitorInfoW as get_monitor_info,
    GetSystemMetrics as get_system_metrics, GetWindowRect as get_window_rect,
    ReleaseDC as release_dc, SelectObject as select_object,
};

#[cfg(windows)]
use self::GetDpiForMonitor as get_dpi_for_monitor;

#[cfg(windows)]
const MDT_EFFECTIVE_DPI: u32 = 0;

#[cfg(windows)]
unsafe extern "system" fn enumerate_monitor(
    monitor: Hmonitor,
    _dc: Hdc,
    _clip: *const MonitorRect,
    data: isize,
) -> i32 {
    let mut info = MonitorInfoEx {
        size: u32::try_from(std::mem::size_of::<MonitorInfoEx>())
            .expect("MonitorInfoEx size fits Win32 DWORD"),
        monitor: MonitorRect::default(),
        work: MonitorRect::default(),
        flags: 0,
        device: [0; 32],
    };
    if unsafe { get_monitor_info(monitor, &raw mut info) } == 0 {
        return 1;
    }
    let mut dpi_x = 0;
    let mut dpi_y = 0;
    let dpi = if unsafe {
        get_dpi_for_monitor(monitor, MDT_EFFECTIVE_DPI, &raw mut dpi_x, &raw mut dpi_y)
    } >= 0
        && dpi_x > 0
        && dpi_y > 0
    {
        Some(DpiInfo {
            horizontal: dpi_x,
            vertical: dpi_y,
        })
    } else {
        None
    };
    let length = info
        .device
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(info.device.len());
    let id = String::from_utf16_lossy(&info.device[..length]);
    let list = unsafe { &mut *(data as *mut Vec<DisplayCaptureInfo>) };
    list.push(DisplayCaptureInfo {
        id,
        bounds: CaptureRegion {
            left: info.monitor.left,
            top: info.monitor.top,
            width: u32::try_from(info.monitor.right - info.monitor.left).unwrap_or(0),
            height: u32::try_from(info.monitor.bottom - info.monitor.top).unwrap_or(0),
        },
        dpi,
        primary: info.flags & 1 != 0,
    });
    1
}

#[cfg(test)]
mod tests {
    use super::{
        CaptureBackend, CaptureError, CaptureRegion, CapturedFrame, ImageFrame, Origin,
        RecordingCapture,
    };

    fn frame() -> CapturedFrame {
        CapturedFrame {
            origin: Origin { x: -10, y: 20 },
            image: ImageFrame::new(
                3,
                2,
                vec![
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
            )
            .expect("fixture dimensions should match"),
        }
    }

    #[test]
    fn crops_region_and_updates_screen_origin() {
        let mut capture = RecordingCapture::new(frame());
        let cropped = capture
            .capture(Some(CaptureRegion {
                left: 1,
                top: 0,
                width: 2,
                height: 1,
            }))
            .expect("region should be inside frame");
        assert_eq!(cropped.origin, Origin { x: -9, y: 20 });
        assert_eq!(cropped.image.pixels, vec![5, 6, 7, 8, 9, 10, 11, 12]);
    }

    #[test]
    fn captures_screen_region_relative_to_frame_origin() {
        let mut capture = RecordingCapture::new(frame());
        let captured = capture
            .capture_screen_region(Some(CaptureRegion {
                left: -9,
                top: 21,
                width: 1,
                height: 1,
            }))
            .expect("screen-space crop should succeed");
        assert_eq!(captured.origin, Origin { x: -9, y: 21 });
        assert_eq!(captured.image.pixels, vec![17, 18, 19, 20]);
    }

    #[test]
    fn rejects_invalid_regions_and_buffers() {
        let image = ImageFrame::new(2, 2, vec![0; 3]);
        assert!(matches!(
            image,
            Err(CaptureError::InvalidPixelBuffer { .. })
        ));
        let mut capture = RecordingCapture::new(frame());
        let error = capture.capture(Some(CaptureRegion {
            left: 2,
            top: 1,
            width: 2,
            height: 1,
        }));
        assert_eq!(
            error,
            Err(CaptureError::RegionOutside {
                width: 3,
                height: 2,
            })
        );
    }
}
