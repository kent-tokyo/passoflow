"""Always-on-top screen overlays shown while a scenario runs: a border around the window
being automated, and a HUD showing the current step name.

Both run in a single persistent background thread with its own Tk root, so they don't block
the runner's execution loop. Commands are pushed from any thread via a queue and applied on
the overlay thread's own event loop. Both overlay windows are made click-through via the
Win32 WS_EX_TRANSPARENT extended style, since the border's geometry spans the entire target
window — without it, every click meant for the automated app would hit our overlay instead.
"""

import ctypes
import ctypes.wintypes
import logging
import queue
import threading
import tkinter as tk
import tkinter.font as tkfont

import pyautogui

logger = logging.getLogger(__name__)

_BORDER_COLOR = "#00e5ff"
_BORDER_WIDTH = 4
_COLORKEY = "black"  # arbitrary color-key used only for the border window's transparent center

_HUD_MARGIN = 16
_HUD_AVOID_RADIUS = 160  # if the cursor is within this many px of the HUD's default corner, move it
_HUD_BG = "#202020"
_HUD_FG = "white"
_HUD_FONT = ("Yu Gothic UI", 11)
_HUD_PAD_X = 10
_HUD_PAD_Y = 6

_GWL_EXSTYLE = -20
_WS_EX_LAYERED = 0x80000
_WS_EX_TRANSPARENT = 0x20
_LWA_COLORKEY = 0x1
_LWA_ALPHA = 0x2

# A private WinDLL handle, not ctypes.windll.user32: that object is cached and shared
# process-wide, so setting .argtypes/.restype on its functions (below) would also rewrite
# them for any other library in the same process calling the same user32 functions — as
# pygetwindow does for GetWindowRect, which then rejects its own RECT struct as the wrong
# pointer type. Empirically confirmed: it broke gw.getWindowsWithTitle() during a real run.
_user32 = ctypes.WinDLL("user32")
# HWNDs are pointer-sized; without these, ctypes' default 32-bit int return/arg type would
# truncate them on 64-bit Windows and break the hwnd comparisons in _foreground_window_rect.
_user32.GetForegroundWindow.restype = ctypes.wintypes.HWND
_user32.GetWindowRect.argtypes = [ctypes.wintypes.HWND, ctypes.POINTER(ctypes.wintypes.RECT)]


def _make_click_through(win: tk.Toplevel) -> None:
    """Make an overrideredirect Tk window pass mouse input through to whatever is beneath it."""
    try:
        hwnd = win.winfo_id()
        style = _user32.GetWindowLongW(hwnd, _GWL_EXSTYLE)
        _user32.SetWindowLongW(hwnd, _GWL_EXSTYLE, style | _WS_EX_LAYERED | _WS_EX_TRANSPARENT)
    except OSError as e:
        logger.warning("Could not make overlay window click-through: %s", e)


def _refresh_layered(win: tk.Toplevel) -> None:
    """Force Windows to recomposite a click-through window's cached surface from its current
    content. A window (like the HUD) that's only layered via our own manual WS_EX_LAYERED flag
    never gets that automatically, so without this its on-screen bitmap freezes at whatever it
    looked like the moment click-through was applied — later content/size changes update the
    widget correctly but never reach the screen. Empirically confirmed on this Windows build."""
    try:
        hwnd = win.winfo_id()
        _user32.SetLayeredWindowAttributes(hwnd, 0, 255, _LWA_ALPHA)
    except OSError as e:
        logger.warning("Could not refresh overlay window: %s", e)


def _refresh_colorkey(win: tk.Toplevel) -> None:
    """Re-apply the border window's colorkey transparency after _make_click_through runs.
    Tk's -transparentcolor attribute keeps the colorkey in sync on its own across ordinary
    geometry/content redraws, but the raw SetWindowLongW call in _make_click_through resets
    whatever layered-window state Tk had established for it — without this, the window renders
    fully opaque (the canvas's colorkey background shows up as a solid fill) from that point on,
    including after later moves. Colorref 0 is RGB(0, 0, 0), i.e. _COLORKEY = "black".
    Empirically confirmed on this Windows build."""
    try:
        hwnd = win.winfo_id()
        _user32.SetLayeredWindowAttributes(hwnd, 0, 0, _LWA_COLORKEY)
    except OSError as e:
        logger.warning("Could not refresh overlay window colorkey: %s", e)


def _foreground_window_rect() -> tuple[int | None, tuple[int, int, int, int] | None]:
    """Return (hwnd, (left, top, width, height)) for whatever window currently has OS focus,
    or (hwnd, None) if it has no usable rect (e.g. minimized)."""
    hwnd = _user32.GetForegroundWindow()
    if not hwnd:
        return None, None
    rect = ctypes.wintypes.RECT()
    if not _user32.GetWindowRect(hwnd, ctypes.byref(rect)):
        return hwnd, None
    width, height = rect.right - rect.left, rect.bottom - rect.top
    if width <= 0 or height <= 0:
        return hwnd, None
    return hwnd, (rect.left, rect.top, width, height)


class Overlay:
    """Manages the border and HUD overlay windows on a dedicated background thread."""

    def __init__(self):
        self._queue: queue.Queue = queue.Queue()
        self._thread: threading.Thread | None = None
        self._root: tk.Tk | None = None
        self._border_win: tk.Toplevel | None = None
        self._border_canvas: tk.Canvas | None = None
        self._hud_win: tk.Toplevel | None = None
        self._hud_canvas: tk.Canvas | None = None
        self._hud_font: tkfont.Font | None = None
        self._hud_size: tuple[int, int] = (0, 0)
        self._last_fg_hwnd: int | None = None

    def _ensure_started(self) -> None:
        if self._thread and self._thread.is_alive():
            return
        self._thread = threading.Thread(target=self._run, daemon=True)
        self._thread.start()

    def _run(self) -> None:
        self._root = tk.Tk()
        self._root.withdraw()  # the root itself is never shown, only its Toplevel children
        self._root.after(30, self._poll)
        self._root.mainloop()

    def _poll(self) -> None:
        try:
            while True:
                command, payload = self._queue.get_nowait()
                try:
                    self._handle(command, payload)
                except Exception as e:
                    # Never let a bad command (e.g. a stale window handle) kill the poll loop —
                    # that would permanently freeze the overlay for the rest of the run, since
                    # the reschedule below would never run again.
                    logger.warning("Overlay command %r failed: %s", command, e)
        except queue.Empty:
            pass
        self._sync_border_to_foreground()
        if self._root:
            self._root.after(30, self._poll)

    def _sync_border_to_foreground(self) -> None:
        """Snap the border to whatever window the OS just gave focus to, e.g. a dialog the
        automated app popped up on its own between scenario steps — not just windows our own
        activate_window calls explicitly targeted. Runs every poll tick (~30ms) so the border
        never visibly lags behind a focus change."""
        if self._border_win is None:
            return
        hwnd, rect = _foreground_window_rect()
        if hwnd is None or hwnd == self._last_fg_hwnd:
            return
        self._last_fg_hwnd = hwnd
        try:
            if hwnd == self._border_win.winfo_id() or (self._hud_win and hwnd == self._hud_win.winfo_id()):
                return
        except tk.TclError:
            return
        if rect is not None:
            self._set_outline(rect)

    def _handle(self, command: str, payload) -> None:
        if command == "outline":
            self._set_outline(payload)
        elif command == "step":
            self._set_step_label(payload)
        elif command == "stop":
            if self._border_win:
                self._border_win.destroy()
            if self._hud_win:
                self._hud_win.destroy()
            self._root.quit()

    def _hud_text_size(self, text: str) -> tuple[int, int]:
        width = self._hud_font.measure(text) + _HUD_PAD_X * 2
        height = self._hud_font.metrics("linespace") + _HUD_PAD_Y * 2
        return width, height

    def _set_outline(self, rect: tuple[int, int, int, int] | None) -> None:
        if rect is None:
            if self._border_win:
                self._border_win.destroy()
                self._border_win = None
                self._border_canvas = None
                self._last_fg_hwnd = None
            return
        left, top, width, height = rect
        if width <= 0 or height <= 0:
            return
        is_new = self._border_win is None
        if is_new:
            win = tk.Toplevel(self._root)
            win.overrideredirect(True)
            win.attributes("-topmost", True)
            # No -alpha here: combining it with -transparentcolor on this Windows build lets the
            # colorkey blend through as a faint tint over the whole window instead of true
            # transparency, which corrupts the pixels click_image's screenshot-based matching
            # relies on for anything under the border's area (i.e. the whole target window).
            win.attributes("-transparentcolor", _COLORKEY)
            canvas = tk.Canvas(win, bg=_COLORKEY, highlightthickness=0)
            canvas.pack(fill="both", expand=True)
            self._border_win = win
            self._border_canvas = canvas
        self._border_win.geometry(f"{width}x{height}+{left}+{top}")
        self._border_canvas.configure(width=width, height=height)
        self._border_canvas.delete("all")
        half = _BORDER_WIDTH / 2
        self._border_canvas.create_rectangle(
            half, half, width - half, height - half, outline=_BORDER_COLOR, width=_BORDER_WIDTH
        )
        if is_new:
            # Applying the click-through style before the window's first real geometry/paint
            # leaves it permanently blank (content never composites in) — empirically confirmed
            # on this Windows build. Applying it once, after the first draw, works and survives
            # every later geometry/redraw (e.g. moving the border to a newly activated window).
            self._root.update()
            _make_click_through(self._border_win)
            # _make_click_through's raw SetWindowLongW call resets the colorkey transparency
            # that -transparentcolor just established, leaving the interior fully opaque black
            # until it's reasserted here — see _refresh_colorkey. Doing it once here (like
            # click-through itself) survives every later move/redraw of this window.
            _refresh_colorkey(self._border_win)

    def _set_step_label(self, text: str | None) -> None:
        if text is None:
            if self._hud_win:
                self._hud_win.destroy()
                self._hud_win = None
                self._hud_canvas = None
            return
        is_new = self._hud_win is None
        if is_new:
            win = tk.Toplevel(self._root)
            win.overrideredirect(True)
            win.attributes("-topmost", True)
            self._hud_font = tkfont.Font(family=_HUD_FONT[0], size=_HUD_FONT[1])
            canvas = tk.Canvas(win, bg=_HUD_BG, highlightthickness=0)
            canvas.pack()
            self._hud_win = win
            self._hud_canvas = canvas
        # Rendered on a Canvas (create_text), not a Label: a Label's native text painting goes
        # stale after the window is made click-through (see _make_click_through) — the widget's
        # own `text` option updates correctly (confirmed via cget), but nothing new reaches the
        # screen. A Canvas's self-painted bitmap, like the border's, keeps redrawing correctly
        # through the same click-through style — empirically confirmed on this Windows build.
        width, height = self._hud_text_size(text)
        self._hud_size = (width, height)
        canvas = self._hud_canvas
        canvas.configure(width=width, height=height)
        canvas.delete("all")
        canvas.create_text(_HUD_PAD_X, height / 2, text=text, fill=_HUD_FG, font=self._hud_font, anchor="w")
        self._position_hud()
        if is_new:
            # See the matching comment in _set_outline: click-through must be applied after
            # the window's first real content/geometry, not before.
            self._root.update()
            _make_click_through(self._hud_win)
        else:
            self._root.update_idletasks()
            _refresh_layered(self._hud_win)

    def _position_hud(self) -> None:
        win = self._hud_win
        width, height = self._hud_size
        try:
            mouse_x, mouse_y = pyautogui.position()
        except Exception:
            mouse_x, mouse_y = -1, -1
        near_top_left = mouse_x < _HUD_MARGIN + width + _HUD_AVOID_RADIUS and mouse_y < _HUD_MARGIN + height + _HUD_AVOID_RADIUS
        if near_top_left:
            x = win.winfo_screenwidth() - width - _HUD_MARGIN
        else:
            x = _HUD_MARGIN
        win.geometry(f"{width}x{height}+{x}+{_HUD_MARGIN}")

    def set_window_outline(self, rect: tuple[int, int, int, int]) -> None:
        """Draw (or move) a semi-transparent border around rect = (left, top, width, height)."""
        self._ensure_started()
        self._queue.put(("outline", rect))

    def set_step_label(self, text: str) -> None:
        """Show (or update) the step-name HUD in a screen corner."""
        self._ensure_started()
        self._queue.put(("step", text))

    def clear(self) -> None:
        """Hide both overlays, e.g. when a run ends, without stopping the background thread."""
        if not (self._thread and self._thread.is_alive()):
            return
        self._queue.put(("outline", None))
        self._queue.put(("step", None))

    def stop(self) -> None:
        """Tear down both overlays and stop the background thread."""
        if not (self._thread and self._thread.is_alive()):
            return
        self._queue.put(("stop", None))
        self._thread.join(timeout=2)


overlay = Overlay()
