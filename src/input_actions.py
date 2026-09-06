"""Keyboard, clipboard, window, and process actions (no image search involved)."""

import calendar
import csv
import glob
import logging
import os
import re
import shutil
import subprocess
import time
from datetime import datetime, timedelta
from pathlib import Path

import elixcee
import pygetwindow as gw
import pyautogui
import pyperclip
import pythoncom
import win32com.client

from overlay import overlay
from screen_actions import RETRIES, RETRY_INTERVAL_MS

logger = logging.getLogger(__name__)


def _add_months(date: datetime, months: int) -> datetime:
    """Shift date by a number of months, clamping the day to the target month's length."""
    month_index = date.month - 1 + months
    year = date.year + month_index // 12
    month = month_index % 12 + 1
    day = min(date.day, calendar.monthrange(year, month)[1])
    return date.replace(year=year, month=month, day=day)


def get_date(format: str = "%Y%m%d", days_offset: int = 0, months_offset: int = 0, end_of_month: bool = False) -> str:
    """Return a date relative to today, formatted with a strftime format string.

    days_offset/months_offset shift the date, e.g. days_offset=-1 for yesterday
    or months_offset=-1 for last month. Both default to 0 (today). If end_of_month
    is true, the day is then overridden to the target month's actual last day
    (28-31) — unlike months_offset's own day-clamping, this doesn't depend on
    today's day-of-month, so e.g. months_offset=0, end_of_month=True reliably
    gives the last day of the current month regardless of what day it is today.
    """
    date = datetime.now() + timedelta(days=days_offset)
    if months_offset:
        date = _add_months(date, months_offset)
    if end_of_month:
        date = date.replace(day=calendar.monthrange(date.year, date.month)[1])
    return date.strftime(format)


def type_text(text: str) -> None:
    """Type text into whichever element currently has focus.

    Goes through the clipboard (copy + Ctrl+V) rather than simulating key
    presses, since pyautogui.write() can only type characters present on the
    physical keyboard layout and garbles non-ASCII text such as Japanese.
    The previous clipboard contents are restored afterwards.
    """
    logger.info("Typing text: %s", text)
    previous_clipboard = pyperclip.paste()
    pyperclip.copy(text)
    hotkey("ctrl", "v")
    pyperclip.copy(previous_clipboard)


def press_key(key: str, wait_ms: int = 100) -> None:
    """Press a single key, e.g. 'enter', 'tab', 'esc', then wait wait_ms milliseconds
    (default 100) for whatever the key triggered to take effect."""
    logger.info("Pressing key: %s", key)
    pyautogui.press(key)
    time.sleep(wait_ms / 1000)


def hotkey(*keys: str) -> None:
    """Press multiple keys simultaneously, e.g. hotkey('ctrl', 'v')."""
    logger.info("Pressing hotkey: %s", "+".join(keys))
    pyautogui.hotkey(*keys)


def set_clipboard(text: str) -> None:
    """Copy text to the system clipboard."""
    logger.info("Setting clipboard: %s", text)
    pyperclip.copy(text)


def paste() -> None:
    """Paste the clipboard contents via Ctrl+V into whichever element has focus."""
    logger.info("Pasting clipboard contents")
    hotkey("ctrl", "v")


def clear_input() -> None:
    """Clear whichever element has focus: select all (Ctrl+A), then delete."""
    logger.info("Clearing input")
    hotkey("ctrl", "a")
    press_key("delete")


def launch_app(path: str, args: list[str] | None = None) -> None:
    """Launch an application, e.g. notepad.exe, without waiting for it to exit."""
    logger.info("Launching: %s %s", path, args or "")
    subprocess.Popen([path, *(args or [])])


def rename_file(path: str, new_name: str) -> None:
    """Renames a file within its current folder (new_name is a filename, not a full path)."""
    logger.info("Renaming: %s -> %s", path, new_name)
    try:
        Path(path).rename(Path(path).parent / new_name)
    except Exception as e:
        logger.warning("Could not rename %s to %s: %s", path, new_name, e)


def move_file(path: str, destination: str) -> None:
    """Moves a file to destination, a full target file path. Creates missing parent folders."""
    logger.info("Moving: %s -> %s", path, destination)
    try:
        Path(destination).parent.mkdir(parents=True, exist_ok=True)
        shutil.move(path, destination)
    except Exception as e:
        logger.warning("Could not move %s to %s: %s", path, destination, e)


_WILDCARD_CHARS = set("*?[")


def copy_file(path: str, destination: str, if_destination_newer: str = "overwrite") -> None:
    """Copies path to destination.

    If path contains wildcard characters (*, ?, [), it's expanded via glob and every
    matched file is copied into the destination folder (created if missing), keeping
    each file's original name. Otherwise path is a single file and destination is the
    full target file path, as before.

    if_destination_newer controls what happens when a destination file already exists
    and is newer than its source: "overwrite" (default) copies anyway; "skip" leaves
    the existing (newer) file alone.
    """
    logger.info("Copying: %s -> %s (if_destination_newer=%s)", path, destination, if_destination_newer)
    try:
        if any(c in path for c in _WILDCARD_CHARS):
            matches = sorted(glob.glob(path))
            if not matches:
                logger.warning("No files matched pattern: %s", path)
                return
            dest_dir = Path(destination)
            dest_dir.mkdir(parents=True, exist_ok=True)
            for src in matches:
                _copy_one(Path(src), dest_dir / Path(src).name, if_destination_newer)
        else:
            dst = Path(destination)
            dst.parent.mkdir(parents=True, exist_ok=True)
            _copy_one(Path(path), dst, if_destination_newer)
    except Exception as e:
        logger.warning("Could not copy %s to %s: %s", path, destination, e)


def _copy_one(src: Path, dst: Path, if_destination_newer: str) -> None:
    if if_destination_newer == "skip" and dst.exists() and dst.stat().st_mtime > src.stat().st_mtime:
        logger.info("Skipping copy, destination is newer: %s", dst)
        return
    shutil.copy2(src, dst)


def map_network_drive(drive: str, path: str) -> None:
    """Maps drive (e.g. "Z:") to a UNC path (e.g. "\\\\server\\share") via `net use`.

    Always disconnects any existing mapping on drive first (ignoring failure -- there may be
    nothing to disconnect), so re-running a scenario doesn't fail with "already in use".
    """
    logger.info("Mapping network drive: %s -> %s", drive, path)
    try:
        subprocess.run(["net", "use", drive, "/delete", "/y"], capture_output=True, text=True)
        result = subprocess.run(["net", "use", drive, path], capture_output=True, text=True)
        if result.returncode != 0:
            logger.warning("Could not map %s to %s: %s", drive, path, result.stderr.strip())
    except Exception as e:
        logger.warning("Could not map %s to %s: %s", drive, path, e)


def open_excel_file(path: str) -> None:
    """Open an existing file (e.g. .xlsx) with its associated application — normally Excel,
    but whatever Windows has registered for that extension, same as double-clicking it."""
    logger.info("Opening file: %s", path)
    os.startfile(path)


def _a1_to_row_col(cell: str) -> tuple[int, int]:
    """Convert an A1-style cell reference (e.g. "B3") to elixcee's 1-based (row, col)."""
    match = re.match(r"^([A-Za-z]+)(\d+)$", cell.strip())
    if not match:
        raise ValueError(f"Invalid cell reference: {cell!r}")
    letters, row_text = match.groups()
    col = 0
    for ch in letters.upper():
        col = col * 26 + (ord(ch) - ord("A") + 1)
    return int(row_text), col


_A1_REF_RE = re.compile(r"^[A-Za-z]+\d+(:[A-Za-z]+\d+)?$")


def _require_valid_a1_ref(value: str) -> None:
    """Raise if `value` isn't a plain A1-style cell or range reference (e.g. "B2" or "A2:C20").
    """
    if not _A1_REF_RE.match(value.strip()):
        raise ValueError(f"Invalid cell/range reference: {value!r}")


def _require_sheet(vm, sheet: str) -> None:
    """Raise if `sheet` doesn't already exist in `vm` — elixcee's own set_sheet()/get_sheet()
    silently create/no-op on an unknown name instead of erroring, which would otherwise turn a
    typo'd sheet name into a silent wrong-sheet read or (for a step that saves) an unwanted new
    empty sheet."""
    if sheet.lower() not in vm.sheet_names():
        raise ValueError(f"no sheet named {sheet!r}")


def _excel_cell_to_str(value) -> str:
    """Stringify a cell value read back from elixcee.

    Whole-number floats (e.g. a code column reading back as 1234.0) are rendered without the
    trailing .0, so they don't corrupt the value if pasted verbatim. None (empty cell) becomes
    "". Unlike openpyxl, elixcee does not convert date-formatted cells to datetime objects — it
    returns the raw Excel serial number instead, since it doesn't expose cell number-format info
    (see elixcee issue for a request to change this); such cells intentionally pass through here
    as plain numbers rather than being guessed at.
    """
    if value is None:
        return ""
    if isinstance(value, float) and value.is_integer():
        return str(int(value))
    return str(value)


def _coerce_excel_value(text: str):
    """Convert a string back to int/float when it looks numeric, so formulas
    referencing the cell (e.g. SUM) keep working instead of seeing text.

    Only coerces when the conversion round-trips exactly back to the original
    text — otherwise a zero-padded code (e.g. a postal/product code "0123")
    would silently lose its leading zero (int("0123") == 123).
    """
    try:
        as_int = int(text)
        if str(as_int) == text:
            return as_int
    except ValueError:
        pass
    try:
        as_float = float(text)
        if str(as_float) == text:
            return as_float
    except ValueError:
        pass
    return text


def get_excel_cell_value(path: str, cell: str, sheet: str | None = None) -> str:
    """Read a single cell's value from an Excel file, e.g. cell="B3", via elixcee.

    Reads whatever elixcee has cached for the cell — for a formula cell, that's the
    last-computed value if the file carries one (e.g. it was saved by real Excel, or by a prior
    run_excel_macro/set_excel_value step in this same run), same as openpyxl's data_only=True.
    Like every other action here, a failure (bad path/sheet name, corrupt file, etc.) is logged
    as a warning and doesn't crash the run — returns "" as if the cell were empty.
    """
    logger.info("Reading Excel value: %s!%s (%s)", sheet or "<active>", cell, path)
    try:
        vm = elixcee.load_workbook(path)
        if sheet:
            _require_sheet(vm, sheet)
            vm.set_sheet(sheet)
        row, col = _a1_to_row_col(cell)
        return _excel_cell_to_str(vm.get_cell(row, col))
    except Exception as e:
        logger.warning("Could not read Excel value %s!%s (%s): %s", sheet or "<active>", cell, path, e)
        return ""


def set_excel_cell_value(path: str, cell: str, value: str, sheet: str | None = None) -> None:
    """Write a single cell's value into an Excel file, e.g. cell="B3", then save, via elixcee.

    Numeric-looking text is stored as an actual number rather than a string,
    so formulas elsewhere in the workbook that reference the cell still work.
    A failure (bad path/sheet name, or the file being open elsewhere — e.g. in
    Excel itself, via a sibling open_excel_file/open_new_excel step — raises
    PermissionError on save) is logged as a warning rather than crashing the run.
    """
    logger.info("Setting Excel value: %s!%s = %s (%s)", sheet or "<active>", cell, value, path)
    try:
        vm = elixcee.load_workbook(path)
        if sheet:
            _require_sheet(vm, sheet)
            vm.set_sheet(sheet)
        row, col = _a1_to_row_col(cell)
        vm.set_cell(row, col, _coerce_excel_value(value))
        vm.save_workbook(path)
    except Exception as e:
        logger.warning("Could not set Excel value %s!%s (%s): %s", sheet or "<active>", cell, path, e)


def save_excel_file(path: str) -> None:
    """Re-saves an existing .xlsx in place via elixcee, which needs no Excel install.

    See https://github.com/kent-tokyo/elixcee. Requires elixcee>=1.0.2.
    """
    try:
        vm = elixcee.load_workbook(path)
        vm.save_workbook(path)
    except Exception as e:
        logger.warning("Could not save Excel file %s: %s", path, e)


def create_excel_sheet(path: str, sheet: str) -> None:
    """Add a new, empty sheet named `sheet` to an existing .xlsx file, then save, via elixcee's
    set_sheet() (which creates the sheet if it doesn't already exist).

    Unlike the old openpyxl-based version, there's no way to control where the new sheet is
    inserted — elixcee always appends it, with no positional-insert option (see elixcee issue).
    Also note: elixcee currently lowercases a newly created sheet's name on save if it's plain
    ASCII (a Japanese/non-ASCII name round-trips fine) — filed upstream, see elixcee issue.
    A failure (bad path, or the file being open elsewhere — e.g. in Excel itself — raises
    PermissionError on save) is logged as a warning rather than crashing the run.
    """
    logger.info("Creating Excel sheet: %s (%s)", sheet, path)
    try:
        vm = elixcee.load_workbook(path)
        vm.set_sheet(sheet)
        vm.save_workbook(path)
    except Exception as e:
        logger.warning("Could not create Excel sheet %s (%s): %s", sheet, path, e)


def delete_excel_sheet(path: str, sheet: str) -> None:
    """Remove the sheet named `sheet` from an existing .xlsx file, then save.

    elixcee has no direct "delete sheet" API, so this runs a one-line VBA macro through its own
    VBA emulator (vm.run) — the same "Sheets(...).Delete" statement real Excel VBA would use.
    A failure (bad path, unknown sheet name, or the file being open elsewhere) is logged as a
    warning rather than crashing the run, same as set_excel_value.
    """
    logger.info("Deleting Excel sheet: %s (%s)", sheet, path)
    try:
        vm = elixcee.load_workbook(path)
        _require_sheet(vm, sheet)
        escaped = sheet.replace('"', '""')
        vba = (
            "Sub DeleteSheet()\n"
            "    Application.DisplayAlerts = False\n"
            f'    Sheets("{escaped}").Delete\n'
            "End Sub\n"
        )
        vm.run(vba, "DeleteSheet")
        vm.save_workbook(path)
    except Exception as e:
        logger.warning("Could not delete Excel sheet %s (%s): %s", sheet, path, e)


def sort_excel_range(path: str, cell_range: str, key_cell: str, order: str = "asc", sheet: str | None = None) -> None:
    """Sort a range of cells in an Excel file by one column, then save.

    `cell_range` must be the data range only, e.g. "A2:C20"; callers should exclude any header
    row because the action always sorts every row in the supplied range.
    `key_cell` is the top cell of the column to sort by within `cell_range`, e.g. "B2" to sort
    by column B starting at row 2. `order` is "asc" or "desc".
    """
    logger.info("Sorting Excel range: %s (key %s, %s) (%s)", cell_range, key_cell, order, path)
    try:
        _require_valid_a1_ref(cell_range)
        _require_valid_a1_ref(key_cell)
        vm = elixcee.load_workbook(path)
        if sheet:
            _require_sheet(vm, sheet)
            vm.set_sheet(sheet)
        _, key_col = _a1_to_row_col(key_cell)
        vm.sort_range(
            cell_range,
            key_col,
            descending=order == "desc",
            header=False,
            sheet=sheet,
        )
        vm.save_workbook(path)
    except Exception as e:
        logger.warning("Could not sort Excel range %s (%s): %s", cell_range, path, e)


def delete_excel_row(path: str, row: int, sheet: str | None = None) -> None:
    """Delete a single row (shifting the rows below it up) from an Excel file, then save.

    `row` is 1-based, matching the row numbers shown in Excel itself. The direct workbook API
    shifts cell values below the row but does not rewrite merge/style metadata.
    """
    logger.info("Deleting Excel row: %d (%s)", row, path)
    try:
        if row < 1:
            raise ValueError(f"row must be >= 1, got {row}")
        vm = elixcee.load_workbook(path)
        if sheet:
            _require_sheet(vm, sheet)
            vm.set_sheet(sheet)
        vm.delete_rows(row, sheet=sheet)
        vm.save_workbook(path)
    except Exception as e:
        logger.warning("Could not delete Excel row %d (%s): %s", row, path, e)


def run_excel_macro(path: str, macro: str, args: list[str] | None = None) -> None:
    """Run a VBA macro (Sub) in an Excel workbook via COM automation.

    Unlike the other Excel actions here, this needs a real Excel install rather than elixcee:
    elixcee's own VBA emulator (used by delete_excel_sheet above) only runs VBA source text
    handed to it directly — it doesn't read/execute a macro already stored in the workbook's own
    VBA project (.xlsm's vbaProject.bin), which is exactly what this action targets. If the
    workbook is already open (e.g. via a prior open_excel_file/open_new_excel step), reuses that
    instance instead of opening a second copy. `macro` is the Sub name (optionally
    "Module.Sub"), resolved against the workbook per Application.Run's own
    `'workbook.xlsm'!Macro` convention.
    """
    logger.info("Running Excel macro: %s (%s)", macro, path)
    resolved_path = str(Path(path).resolve())
    # A fresh run is its own single-threaded subprocess (see run_scenario.py), so this thread's
    # COM apartment has never been initialized — win32com's calls below fail without this.
    pythoncom.CoInitialize()
    try:
        try:
            workbook = win32com.client.GetObject(resolved_path)
            excel = workbook.Application
        except Exception:
            excel = win32com.client.Dispatch("Excel.Application")
            excel.Visible = True
            workbook = excel.Workbooks.Open(resolved_path)
        excel.Run(f"'{workbook.Name}'!{macro}", *(args or []))
    except Exception as e:
        logger.warning("Could not run Excel macro %s (%s): %s", macro, path, e)
    finally:
        pythoncom.CoUninitialize()


def _read_excel_rows(path: str, sheet: str | None) -> list[dict[str, str]]:
    vm = elixcee.load_workbook(path)
    if sheet:
        _require_sheet(vm, sheet)
    target_sheet = sheet or vm.active_sheet()
    # get_sheet returns only non-empty cells as a sparse {(row, col): value} dict (1-based, like
    # VBA), so the row/column grid has to be reconstructed from the max row/col seen — same dense
    # shape openpyxl's iter_rows(values_only=True) gave us, including blank trailing cells within
    # the used range.
    cells = vm.get_sheet(target_sheet)
    if not cells:
        return []
    max_row = max(r for r, _ in cells)
    max_col = max(c for _, c in cells)
    columns = [_excel_cell_to_str(cells.get((1, c))) for c in range(1, max_col + 1)]
    blank_count = sum(1 for c in columns if not c)
    if blank_count:
        raise ValueError(f"{blank_count} blank column header(s) in {path!r}")
    duplicates = {c for c in columns if columns.count(c) > 1}
    if duplicates:
        raise ValueError(f"duplicate column header(s) in {path!r}: {', '.join(sorted(duplicates))}")
    result = []
    for r in range(2, max_row + 1):
        row = [cells.get((r, c)) for c in range(1, max_col + 1)]
        if all(value is None for value in row):
            continue
        result.append({columns[i]: _excel_cell_to_str(row[i]) for i in range(len(columns))})
    return result


def _read_csv_rows(path: str) -> list[dict[str, str]]:
    # Prefer utf-8-sig because Excel's "CSV UTF-8" export writes a BOM. Older Japanese Excel
    # exports and OBIC/RPA files commonly use CP932 instead, so retry that encoding only when
    # UTF-8 decoding fails. This keeps UTF-8 files strict while accepting both common formats.
    try:
        with open(path, encoding="utf-8-sig", newline="") as f:
            return [dict(row) for row in csv.DictReader(f)]
    except UnicodeDecodeError:
        with open(path, encoding="cp932", newline="") as f:
            return [dict(row) for row in csv.DictReader(f)]


def read_table_rows(path: str, sheet: str | None = None) -> list[dict[str, str]]:
    """Read an Excel (.xlsx) or CSV file's header row as column names and every data row as a
    dict of {column_name: value}, in file order. `sheet` is ignored for CSV files."""
    if path.lower().endswith(".csv"):
        logger.info("Reading table: %s (csv)", path)
        return _read_csv_rows(path)
    logger.info("Reading table: %s (%s)", path, sheet or "<active>")
    return _read_excel_rows(path, sheet)


def open_new_excel() -> None:
    """Launch Excel with a new blank workbook.

    Resolved through Windows' "start" command, which looks the name up in the App Paths
    registry (the same mechanism as typing "excel" into the Run dialog) rather than a
    hardcoded install path, since that differs by Office version/architecture.
    """
    logger.info("Opening a new Excel workbook")
    subprocess.Popen(["cmd", "/c", "start", "", "excel"])


def activate_window(
    title_substring: str,
    retries: int = RETRIES,
    retry_interval_ms: int = RETRY_INTERVAL_MS,
) -> bool:
    """Find a window whose title contains title_substring and bring it to the front.

    Retries up to `retries` additional times, waiting retry_interval_ms between
    attempts, if no matching window is found right away (e.g. it hasn't finished
    launching yet).
    Returns True if a matching window was found and activation was attempted (even if
    Windows refused to bring it to the foreground), False if no window was found.
    """
    for attempt in range(retries + 1):
        windows = gw.getWindowsWithTitle(title_substring)
        if windows:
            break
        if attempt < retries:
            time.sleep(retry_interval_ms / 1000)
    else:
        logger.warning("No window found with title containing after %d attempt(s): %s", retries + 1, title_substring)
        return False

    window = windows[0]
    logger.info("Activating window: %s", window.title)
    try:
        if window.isMinimized:
            window.restore()
        # Windows' focus-stealing prevention can silently downgrade a background process's
        # SetForegroundWindow call to just flashing the taskbar icon, leaving whatever the
        # user was actually looking at (e.g. the browser tab that started this run) in front
        # instead — with no error raised, so every subsequent image search then quietly fails
        # against the wrong screen. Tapping a key first counts as recent input and lifts that
        # restriction, letting the activation that follows actually take effect.
        pyautogui.press("alt")
        window.activate()
    except gw.PyGetWindowException as e:
        # Windows can also refuse SetForegroundWindow outright, and pygetwindow raises
        # unconditionally on that refusal — even reporting Windows' own "the operation
        # completed successfully" as the error, since SetForegroundWindow doesn't reliably
        # set a real error code on this kind of failure. Not fatal: the window may already be
        # in front, or a later step's image search will simply miss.
        logger.warning("Could not activate window %r: %s", window.title, e)
    overlay.set_window_outline((window.left, window.top, window.width, window.height))
    return True
