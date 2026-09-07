"""Locate an image on screen and act on it (move, click, double-click)."""

import logging
import time
import tkinter as tk
from pathlib import Path

import pyautogui
import pygetwindow as gw
from PIL import Image

CONFIDENCE = 0.8
OVERLAY_DURATION = 0.25
OVERLAY_SIZE = 30
RETRIES = 0
RETRY_INTERVAL_MS = 500
POSITION = "center"

# Named anchor points on the matched image, as (fraction of width, fraction of height)
# from the top-left corner. Used when offset isn't given.
POSITIONS = {
    "center": (0.5, 0.5),
    "top": (0.5, 0.0),
    "bottom": (0.5, 1.0),
    "left": (0.0, 0.5),
    "right": (1.0, 0.5),
    "top-left": (0.0, 0.0),
    "top-right": (1.0, 0.0),
    "bottom-left": (0.0, 1.0),
    "bottom-right": (1.0, 1.0),
}
REGION_ORIGINS = {"screen", "active_window"}

logger = logging.getLogger(__name__)


def resolve_search_region(
    region: tuple[int, int, int, int] | None,
    region_origin: str = "screen",
) -> tuple[int, int, int, int] | None:
    """Resolve a region from screen coordinates or the current foreground window."""
    if region_origin == "screen":
        return region
    if region_origin != "active_window":
        raise ValueError(f"Unknown region origin: {region_origin}")
    try:
        window = gw.getActiveWindow()
    except Exception as exc:
        raise RuntimeError(f"Could not inspect the active window: {exc}") from exc
    if window is None or window.width <= 0 or window.height <= 0:
        raise RuntimeError("Could not determine the active window bounds for image search")
    if region is None:
        return (window.left, window.top, window.width, window.height)
    return (window.left + region[0], window.top + region[1], region[2], region[3])


def ensure_target_window(title_contains: str | None) -> None:
    """Fail closed if the requested foreground-window title is not active."""
    if not title_contains:
        return
    try:
        window = gw.getActiveWindow()
        title = getattr(window, "title", "") if window is not None else ""
    except Exception as exc:
        raise RuntimeError(f"Could not inspect the active window: {exc}") from exc
    if title_contains.casefold() not in title.casefold():
        raise RuntimeError(
            f"Target window check failed: active window title {title!r} does not contain {title_contains!r}"
        )


def locate_image(
    image_path: str | Path,
    confidence: float = CONFIDENCE,
    offset: tuple[int, int] | None = None,
    position: str = POSITION,
    region: tuple[int, int, int, int] | None = None,
) -> pyautogui.Point | None:
    """Find image_path on screen and return a point on the match, or None if not found.

    By default searches the whole screen and returns the center of the match. Pass
    region=(left, top, width, height) to limit the search to a screen rectangle.
    Pass position (one of POSITIONS,
    e.g. "right" for the middle of the right edge) to target a different named
    point on the match. Pass offset=(x, y) to instead return the point x, y
    pixels from the match's top-left corner; offset takes precedence over
    position when both are given.
    """
    # Loaded via PIL rather than passed as a path string, since pyscreeze's
    # cv2.imread cannot read non-ASCII (e.g. Japanese) file paths on Windows.
    with Image.open(image_path) as needle:
        try:
            search_kwargs = {"confidence": confidence}
            if region is not None:
                search_kwargs["region"] = region
            box = pyautogui.locateOnScreen(needle, **search_kwargs)
        except pyautogui.ImageNotFoundException:
            return None

    if offset is not None:
        return pyautogui.Point(box.left + offset[0], box.top + offset[1])
    fraction_x, fraction_y = POSITIONS[position]
    return pyautogui.Point(round(box.left + box.width * fraction_x), round(box.top + box.height * fraction_y))


def _locate_any(
    image_paths: list[str | Path],
    confidence: float,
    offset: tuple[int, int] | None,
    position: str,
    region: tuple[int, int, int, int] | None,
    region_origin: str,
    target_window_title: str | None,
) -> tuple[pyautogui.Point, str | Path] | None:
    """Try each image_path once, in order, and return the first match plus which path matched."""
    ensure_target_window(target_window_title)
    resolved_region = resolve_search_region(region, region_origin)
    for index, image_path in enumerate(image_paths, start=1):
        logger.debug("Trying image candidate %d/%d: %s", index, len(image_paths), image_path)
        location = locate_image(image_path, confidence=confidence, offset=offset, position=position, region=resolved_region)
        if location is not None:
            logger.info("Matched image candidate %d/%d: %s", index, len(image_paths), image_path)
            return location, image_path
    return None


def _search_with_retry(search, retries: int, retry_interval_ms: int):
    """Call search() up to retries+1 times, waiting retry_interval_ms between attempts, until it returns non-None.

    search is retried as a whole, so for the *_any_image variants a retry re-tries
    every candidate image again rather than exhausting retries on the first one.
    """
    for attempt in range(retries + 1):
        result = search()
        if result is not None:
            return result
        if attempt < retries:
            time.sleep(retry_interval_ms / 1000)
    return None


def move_mouse_to_image(
    image_paths: list[str | Path],
    confidence: float = CONFIDENCE,
    offset: tuple[int, int] | None = None,
    position: str = POSITION,
    retries: int = RETRIES,
    retry_interval_ms: int = RETRY_INTERVAL_MS,
    region: tuple[int, int, int, int] | None = None,
    region_origin: str = "screen",
    target_window_title: str | None = None,
) -> bool:
    """Try each path in image_paths in order and move the mouse to the first match.

    Passing a single-element list behaves like matching one fixed image. Multiple
    entries are useful when the same UI element has multiple visual states (e.g.
    different background/text colors) and any one of them can appear on screen.
    Retries the whole set of candidates up to `retries` additional times, waiting
    retry_interval_ms between attempts, if none match right away.
    Returns True if any image was found, False otherwise.
    """
    result = _search_with_retry(
        lambda: _locate_any(image_paths, confidence, offset, position, region, region_origin, target_window_title), retries, retry_interval_ms
    )
    if result is None:
        logger.warning("None of the images were found on screen after %d attempt(s): %s", retries + 1, image_paths)
        return False

    location, image_path = result
    logger.info("Found %s at: %s", image_path, location)
    pyautogui.moveTo(location)
    return True


def _show_click_overlay(x: int, y: int, size: int = OVERLAY_SIZE, duration: float = OVERLAY_DURATION) -> None:
    """Briefly flash a red circle without rebuilding Tk for every click."""
    # The persistent overlay owns the Tk event loop used during normal scenario runs.
    # Keeping this import local preserves the standalone screen-actions module contract.
    try:
        from overlay import overlay

        overlay.show_click_indicator(x, y, duration)
    except (ImportError, RuntimeError, tk.TclError):
        # Keep direct callers usable when the persistent overlay cannot be started.
        root = tk.Tk()
        root.overrideredirect(True)
        root.attributes("-topmost", True)
        root.attributes("-transparentcolor", "white")
        root.geometry(f"{size}x{size}+{x - size // 2}+{y - size // 2}")
        canvas = tk.Canvas(root, width=size, height=size, bg="white", highlightthickness=0)
        canvas.pack()
        canvas.create_oval(2, 2, size - 2, size - 2, outline="red", width=3)
        root.after(int(duration * 1000), root.destroy)
        root.mainloop()


def click_image(
    image_paths: list[str | Path],
    confidence: float = CONFIDENCE,
    offset: tuple[int, int] | None = None,
    position: str = POSITION,
    retries: int = RETRIES,
    retry_interval_ms: int = RETRY_INTERVAL_MS,
    double_click: bool = False,
    click_indicator_duration: float = OVERLAY_DURATION,
    region: tuple[int, int, int, int] | None = None,
    region_origin: str = "screen",
    target_window_title: str | None = None,
) -> bool:
    """Try each path in image_paths in order, flash the first match, then click (or double-click) there.

    Passing a single-element list behaves like matching one fixed image. Multiple
    entries are useful when the same UI element has multiple visual states (e.g.
    different background/text colors) and any one of them can appear on screen.
    Retries the whole set of candidates up to `retries` additional times, waiting
    retry_interval_ms between attempts, if none match right away.
    Returns True if any image was found and clicked, False otherwise.
    """
    result = _search_with_retry(
        lambda: _locate_any(image_paths, confidence, offset, position, region, region_origin, target_window_title), retries, retry_interval_ms
    )
    if result is None:
        logger.warning("None of the images were found on screen after %d attempt(s): %s", retries + 1, image_paths)
        return False

    location, image_path = result
    action = "Double-clicking" if double_click else "Clicking"
    logger.info("%s %s at: %s", action, image_path, location)
    if click_indicator_duration > 0:
        _show_click_overlay(location.x, location.y, duration=click_indicator_duration)
    if double_click:
        pyautogui.doubleClick(location.x, location.y)
    else:
        pyautogui.click(location.x, location.y)
    return True
