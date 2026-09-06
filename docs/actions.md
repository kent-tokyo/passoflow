# Action reference

[日本語版はこちら](actions_ja.md)

`confidence`, `offset`, `position`, `retry`, and `retry_interval_ms` are common options for actions that involve image search. `activate_window` also supports `retry`/`retry_interval_ms` (it has no `confidence`/`offset`/`position`, since it doesn't search for an image).

## Quick starts by goal

Use these paths when you are not sure which action to choose. The action
catalog below remains the detailed reference for every parameter.

| Goal | Start with | Typical next steps |
| --- | --- | --- |
| Open an application and work in it | `launch_app` | Set `wait_for_window` when readiness matters, then use `activate_window` → screen or keyboard actions |
| Click a button or menu | `click_image` | Capture the target image, then set `retry` if the screen may load slowly |
| Enter text | `type_text` | Use `{{variable}}` when text comes from a variable |
| Read or write one Excel cell | `get_excel_value` / `set_excel_value` | Set the file, sheet, and cell; use a variable for values that change |
| Process selected rows in Excel or CSV | `load_table` | Use the web UI import flow; it creates a table loop over the scenario's `Start`–`End` range |
| Do something only in one case | `if` | Set the variable and optional `equals`, then place the steps inside the block |
| Reuse an existing scenario | `call_scenario` | Use `repeat` when the referenced scenario must run more than once |

### The basic editor workflow

1. Search the action palette by the goal you want to accomplish.
2. Drag the action onto the canvas, or click it to add it after the selected step; the new action is selected automatically.
   When a node has focus, use the arrow keys to move focus between nearby nodes. Use Alt+Arrow to nudge its position (Shift uses a larger step).
3. Select the new node and complete the required fields in the parameter panel.
4. Add a short title or note when the purpose is not obvious from the action name.
5. Save the scenario, then run it and read the execution log.

If execution produces a warning, start with the step named in the log. Image
and window matching problems are often fixed by checking the captured image,
adding a suitable `wait`, or increasing `retry`.

The web action palette groups actions by purpose: Flow, Control, App, File
Operations, Excel, Screen, Variable, and Keyboard Input. File Operations
contains `rename_file`, `move_file`, and `copy_file`; application launching and
window activation remain in App.

| action | required parameters | behavior |
| --- | --- | --- |
| `call_scenario` | `path` | Runs another YAML scenario file in place. Variables are shared with the caller |
| `repeat` | `path`, `count` | Runs another YAML scenario file in place, `count` times in a row. Variables are shared with the caller, across iterations |
| `send_webhook` | `url` | Sends an HTTP request. `method` defaults to `POST`; `payload` is sent as JSON. Set `on_error: stop` when delivery is required; the default `continue` only warns |
| `if` | `variable` or `last_step` | Starts a conditional block. Use `variable` with optional text comparison `equals`, or use `last_step: ok`/`warned` to inspect the immediately previous action. Must be paired with an `endif`, with an optional `else` in between — see Branching below |
| `else` | none | Marks the start of the false branch of the nearest open `if`. Optional — an `if` without an `else` just does nothing when its condition is false |
| `endif` | none | Marks the end of the nearest open `if`/`else` block |
| `set_variable` | `name`, `value` | Stores a variable. `name` can be any string (Japanese allowed) |
| `concat_variable` | `name`, `value` | Like `set_variable`, but `value` is resolved for `{{...}}` placeholders first — concatenate other variables with each other or with literal text |
| `set_year_month_variable` | `name` | Stores today's (or an offset) year-month as `YYYYMM`. Adjustable with `days_offset`, `months_offset` |
| `set_year_month_day_variable` | `name` | Stores today's (or an offset) date as `YYYY/MM/DD`. Adjustable with `days_offset`, `months_offset` |
| `set_month_start_variable` | `name` | Stores the 1st of the target month as `YYYY/MM/01`. Adjustable with `days_offset`, `months_offset` |
| `set_month_end_variable` | `name` | Stores the actual last day of the target month as `YYYY/MM/DD` (28-31, computed correctly regardless of today's day-of-month). Adjustable with `days_offset`, `months_offset` |
| `type_text` | `text` | Types text into the focused element (via clipboard; supports Unicode including Japanese) |
| `set_clipboard` | `text` | Copies a string to the clipboard |
| `paste` | none | Pastes the clipboard contents with Ctrl+V |
| `paste_variable` | `name` | Pastes a variable's value directly (via clipboard, like `type_text`), without needing `{{...}}` template syntax |
| `clear_input` | none | Clears the focused element: selects all (Ctrl+A) then deletes |
| `press_key` | `key` | Presses a single key (e.g. `enter`, `tab`, `esc`), then waits `wait` milliseconds (default 100) |
| `hotkey` | `keys` | Presses multiple keys simultaneously (e.g. `[ctrl, v]` for Ctrl+V) |
| `launch_app` | `path` | Launches the specified executable. `wait_for_window` optionally waits for a matching window title and detects an early process exit; `startup_timeout_ms` limits that wait. Pass a list of arguments with `args` |
| `rename_file` | `path`, `new_name` | Renames a file within its current folder |
| `move_file` | `path`, `destination` | Moves a file to `destination` (a full target path). Missing parent folders are created |
| `copy_file` | `path`, `destination`, `if_destination_newer` | Copies a file (or, if `path` has wildcards, every matching file) to `destination`. Missing folders are created. `if_destination_newer` (`overwrite`/`skip`, default `overwrite`) controls whether an existing, newer destination file is left alone |
| `map_network_drive` | `drive`, `path` | Maps `drive` (e.g. `Z:`) to a UNC `path` (e.g. `\\server\share`) via `net use` |
| `open_excel_file` | `path` | Opens an existing file with its associated application (typically Excel), same as double-clicking it |
| `open_new_excel` | none | Opens Excel with a new blank workbook |
| `get_excel_value` | `path`, `cell`, `name` | Reads one cell (e.g. `B3`) from an `.xlsx` file into a variable. `sheet` selects a sheet by name (defaults to the active sheet) |
| `set_excel_value` | `path`, `cell`, `value` | Writes `value` into one cell (e.g. `B3`) of an `.xlsx` file and saves it. `sheet` selects a sheet by name (defaults to the active sheet) |
| `save_excel_file` | `path` | Re-saves an `.xlsx` file in place (no Microsoft Excel install required) |
| `create_excel_sheet` | `path`, `sheet` | Adds a new empty sheet named `sheet` to an `.xlsx` file (appended at the end — position isn't configurable) and saves it |
| `delete_excel_sheet` | `path`, `sheet` | Removes the sheet named `sheet` from an `.xlsx` file and saves it |
| `delete_excel_row` | `path`, `row` | Deletes row number `row` (1-based, matching Excel's own row numbers) from an `.xlsx` file, shifting the rows below it up, and saves it. `sheet` selects a sheet by name (defaults to the active sheet) |
| `sort_excel_range` | `path`, `range`, `key_cell` | Sorts `range` (e.g. `A2:C20`) by the column containing `key_cell` (e.g. `B2`), then saves. `range` must be the data only, **excluding any header row** — a header row included in `range` would be sorted along with the data. `order` is `asc` (default) or `desc`. `sheet` selects a sheet by name (defaults to the active sheet) |
| `run_excel_macro` | `path`, `macro` | Runs a VBA macro (Sub) in the workbook via COM automation — requires a real Excel install. Reuses the workbook if it's already open; otherwise opens it. Pass extra positional arguments to the macro with `args` |
| `load_table` | `path`, `name` | Reads an `.xlsx`/`.csv` file's header row as column names and data rows, registering them under `name` for a loop to iterate over (see Looping over a table below). `selected_rows` can restrict execution to the checked zero-based row indexes from the web UI import preview. `sheet` selects a sheet by name, `.xlsx` only (defaults to the active sheet) |
| `activate_window` | `title_contains` | Finds a window whose title contains the given string and brings it to the foreground |
| `open_url` | `url` | Opens a URL in the default browser; use the existing image actions to operate the page visually |
| `browser_navigate` | `url` | Navigates a separate visible Playwright browser page by URL |
| `browser_click` | `selector` | Clicks a DOM element selected by CSS selector in the Playwright browser |
| `browser_fill` | `selector`, `text` | Fills an input or textarea selected by CSS selector in the Playwright browser |
| `browser_wait_for` | `selector` | Waits for a DOM element to become visible, attached, hidden, or detached |
| `move_mouse_to_image` | `images` | Tries multiple candidate images in order and moves the mouse to the first match |
| `click_image` | `images` | Tries multiple candidate images in order, shows the position with a red circle, then clicks the first match. Set `click_type: double` to double-click instead; set `click_indicator_duration: 0` to hide the indicator or a smaller positive value to speed up runs |
| `wait` | `ms` | Waits for the specified number of milliseconds |

### Choosing a browser operation mode

Use `open_url` followed by `activate_window` and `click_image`/`type_text` when the browser should be operated like any other visible desktop application. This mode follows what is actually rendered on screen and works with canvas-heavy pages, but is sensitive to resolution, scaling, and visual changes.

Use `browser_navigate`, `browser_click`, `browser_fill`, and `browser_wait_for` when the page exposes stable DOM selectors. This mode is usually more precise and avoids screen-coordinate issues, but requires Playwright and Chromium (`python -m playwright install chromium`). The DOM browser session is shared by these actions within one scenario and closed when the scenario ends.

In the editor, `Preview matches` opens a separate headless local Playwright page for a URL and reports how many elements the selector matches. It does not change the scenario browser session; use the suggested selector only after checking that the result is appropriate.

## Detailed behavior per action

| action | detailed behavior |
| --- | --- |
| `call_scenario` | Loads the target YAML and runs its `steps` inline, in the same process. The `variables` dict is the *same object* as the caller's (not a copy), so any `set_variable`/`set_*_variable` step inside the called scenario is visible to the caller after it returns, and vice versa. Nested steps do not emit the `@@PROGRESS@@n/total` marker that the web UI uses to highlight the running step — the UI keeps highlighting the `call_scenario` step itself for the whole nested run, since the nested step numbers are relative to the sub-scenario, not the caller's. |
| `repeat` | Same loading/variable-sharing/progress-marker behavior as `call_scenario`, but runs the target's `steps` `count` times in a row before moving on to the next step. `count` must be a positive integer. |
| `send_webhook` | Resolves `{{...}}` placeholders in `url`, and recursively in `payload`. Sends the request with Python's standard `urllib.request`; if `payload` is present, `Content-Type: application/json` is set automatically. A failed request logs a warning and continues by default; set `on_error: stop` to make delivery failure stop the scenario. |
| `set_variable` | Stores `value` as-is; `value` itself is not resolved for `{{...}}` placeholders (only `type_text`'s, `set_clipboard`'s, and `concat_variable`'s `text`/`value` are). `name` and `value` are always treated as-is (both may contain Japanese). |
| `concat_variable` | Same as `set_variable`, except `value` *is* resolved for `{{...}}` placeholders first (same `_resolve()` used by `type_text`/`set_clipboard`) — reference other variables to concatenate them with each other or with literal text, e.g. `{{last_name}}{{first_name}}` or `{{name}}様`. |
| `set_year_month_variable` / `set_year_month_day_variable` / `set_month_start_variable` / `set_month_end_variable` | All four compute `datetime.now()`, apply `days_offset` first (`timedelta(days=...)`), then `months_offset` — adding months clamps the day to the target month's length, so e.g. Jan 31 with `months_offset: -1` becomes Feb 28 (or 29 in a leap year), not Mar 3. Each then formats/overrides the result differently: `set_year_month_variable` formats as `%Y%m`; `set_year_month_day_variable` formats as `%Y/%m/%d`; `set_month_start_variable` formats as `%Y/%m/01` (the `01` is fixed text, not a real strftime code, so it's always day 01 regardless of the actual computed day); `set_month_end_variable` overrides the day to the target month's actual last day (28-31) via `calendar.monthrange` before formatting as `%Y/%m/%d` — this is what actually computes "the last day of the month" correctly regardless of today's day-of-month, unlike `months_offset`'s own clamping (which only happens to produce the last day when today's day-of-month already exceeds it). |
| `type_text` | Reads the current clipboard, overwrites it with `text`, sends Ctrl+V, then restores the original clipboard contents. Uses the clipboard (rather than simulated keystrokes) because `pyautogui.write()` can only type characters present on the physical keyboard layout and garbles non-ASCII text such as Japanese. |
| `set_clipboard` | Copies `text` to the clipboard and leaves it there (not restored afterward, unlike `type_text`). |
| `paste` | Just sends Ctrl+V; does not touch clipboard contents. |
| `paste_variable` | Looks up `name` directly in the variables dict (`name` itself is *not* resolved for `{{...}}` — it's the bare variable name, like `if`'s `variable`) and pastes that value the same way `type_text` does (clipboard swap, Ctrl+V, restore). An unset variable pastes an empty string rather than erroring. |
| `clear_input` | Sends Ctrl+A then the `delete` key to whichever element has focus; does not touch the clipboard. |
| `press_key` | Passes `key` straight to `pyautogui.press`; must be one of pyautogui's `KEYBOARD_KEYS` names (see below). Then sleeps `wait` milliseconds (default 100), separate from the automatic `STEP_DELAY` (50ms) applied after every step. |
| `hotkey` | Passes `keys` straight to `pyautogui.hotkey(*keys)`, pressing them together as a chord, not sequentially. |
| `launch_app` | Runs `subprocess.Popen([path, *args])`. A spawn error stops the scenario. Set `wait_for_window` to a title fragment to wait for the application window; an early process exit or `startup_timeout_ms` timeout stops the scenario. Without `wait_for_window`, the action returns after the process is spawned. |
| `rename_file` | Resolves `path`/`new_name` for `{{...}}` placeholders and calls `Path(path).rename(Path(path).parent / new_name)` — `new_name` is a bare file name, not a path; the file stays in its original folder. A failure (missing source file, name collision) logs a warning rather than crashing the run. |
| `move_file` | Resolves `path`/`destination` for `{{...}}` placeholders, creates `destination`'s parent folder if missing (`Path(destination).parent.mkdir(parents=True, exist_ok=True)`), then calls `shutil.move`. `destination` is always a full file path, not a folder — to move into a folder while keeping the same name, build the path with `{{folder}}/original_name.ext`. A failure logs a warning rather than crashing the run. |
| `copy_file` | Resolves `path`/`destination` for `{{...}}` placeholders. If `path` contains no wildcard characters (`*`, `?`, `[...]`), behaves like `move_file` but calls `shutil.copy2` (which preserves metadata like the modification time) — `destination` is a full file path, and its parent folder is created if missing. If `path` does contain a wildcard, it's expanded with `glob.glob(path)` and every matched file is copied into `destination`, which is then treated as a folder (created if missing) — each file keeps its original name; no matches logs a warning and copies nothing. In both cases, `if_destination_newer` (`overwrite`/`skip`, default `overwrite`) controls what happens when the destination file already exists and its modification time is newer than the source's: `overwrite` copies over it anyway, `skip` leaves it untouched. A failure logs a warning rather than crashing the run. |
| `map_network_drive` | Resolves `drive`/`path` for `{{...}}` placeholders. First runs `net use <drive> /delete /y` and ignores its result (there may be nothing mapped yet), then runs `net use <drive> <path>` and logs a warning if that second command's exit code is non-zero. The upfront disconnect makes the step idempotent — re-running the same scenario doesn't fail with "already in use". No username/password support; mapping relies on the current Windows session's existing authentication to the target share. |
| `open_excel_file` | `path` resolves `{{...}}` placeholders (so a variable can pick a date-stamped filename, for example), then calls `os.startfile(path)` — the same mechanism as double-clicking the file in Explorer, so it opens with whatever's actually associated with the file's extension (typically Excel for `.xlsx`, but not guaranteed). Returns immediately; does not wait for the app to finish opening or the file to load. |
| `open_new_excel` | Runs `start excel` via `cmd`, which resolves `excel` through Windows' App Paths registry (the same lookup as typing `excel` into the Run dialog) rather than a hardcoded install path, since that differs by Office version/architecture and installation. If Excel is already running, this typically opens an additional blank workbook window in the existing instance rather than a wholly separate process. |
| `get_excel_value` | Resolves `path`/`sheet`/`cell` for `{{...}}` placeholders, opens the workbook, switches to `sheet` if given (an unknown sheet name is treated as an error rather than silently reading nothing), converts the A1-style `cell` reference (e.g. `B3`) to a row/column position, and reads its value — for a formula cell, that's whatever cached value the file carries, same as reading it without recalculating in Excel; a file whose formulas were never computed reads as empty. A whole-number value is stringified without a trailing `.0`; an empty cell becomes `""`. A date-formatted cell is **not** converted to a date string — it comes back as the raw Excel serial number instead (a fix for this is planned). Does not modify or save the file. |
| `set_excel_value` | Resolves `path`/`sheet`/`cell`/`value` for `{{...}}` placeholders, opens the workbook, switches to `sheet` if given (same existence check as `get_excel_value`), and writes `value` into the cell — converted to a number first if it looks numeric, so formulas elsewhere that reference the cell keep working, otherwise stored as text — then saves. |
| `save_excel_file` | Resolves `path` for `{{...}}` placeholders, then opens and re-saves the file in place, without needing Microsoft Excel installed. |
| `create_excel_sheet` | Resolves `path`/`sheet` for `{{...}}` placeholders, opens the workbook, adds a sheet named `sheet` (if a sheet with that name already exists, nothing changes), and saves. The new sheet is always appended at the end — its position can't be controlled — and a plain-ASCII sheet name is currently saved in lowercase (a Japanese/non-ASCII name is unaffected; a fix for the ASCII case is planned). A failure (bad path, file open elsewhere) is logged as a warning rather than crashing the run. |
| `delete_excel_sheet` | Resolves `path`/`sheet` for `{{...}}` placeholders, opens the workbook, checks `sheet` exists, removes it, and saves. A failure (bad path, unknown sheet name, file open elsewhere) is logged as a warning rather than crashing the run. |
| `delete_excel_row` | Resolves `path`, opens the workbook, validates/selects `sheet` if given, deletes row `row` (1-based) via elixcee's workbook API, and saves. The operation shifts cell values below the row; merge/style metadata is not rewritten. A failure (bad path, `row` less than 1, unknown sheet name, file open elsewhere) is logged as a warning rather than crashing the run. |
| `sort_excel_range` | Resolves `path`/`range`/`key_cell`, opens the workbook, validates/selects `sheet` if given, and sorts `range` by the column containing `key_cell` via elixcee's workbook API. `range` must exclude any header row. `order` is `asc` (default) or `desc`. A failure (bad path, invalid `range`/`key_cell`, unknown sheet name, file open elsewhere) is logged as a warning rather than crashing the run. |
| `run_excel_macro` | Resolves `path`/`macro`/each entry of `args` for `{{...}}` placeholders. Unlike the other Excel actions, this drives a real, visible Excel via COM automation, since it runs a macro that's already stored in the workbook's own VBA project — something the other, lighter-weight Excel actions here can't do. First tries to attach to the workbook if it's already open in a running Excel instance (e.g. from a prior `open_excel_file`/`open_new_excel` step); if that fails, launches a new visible Excel instance and opens the file itself. Then calls `Application.Run` with `macro` (the Sub name, optionally `Module.Sub`) and `args` passed through as the macro's own parameters. A failure (macro not found, VBA error, no Excel installed) logs a warning rather than crashing the run. |
| `load_table` | Resolves `path`/`sheet` for `{{...}}` placeholders. For a `.csv` path, first tries UTF-8 (`utf-8-sig`, so a leading BOM is stripped) and falls back to CP932 for Japanese spreadsheet exports; otherwise reads the given sheet of an `.xlsx` file (`sheet` selects a sheet by name, `.xlsx` only). The first row is used as column names, and every other non-blank row becomes a `{column: value}` dict. When `selected_rows` is present, only those zero-based row indexes are registered; when it is absent, all rows are registered. The result is stored under `name` in an in-memory table registry for a `loop_table` block to iterate over (see Loops above) — this step does not itself set any variables. A failure (bad path/sheet, corrupt file) is logged as a warning and registers an empty table (0 rows) rather than crashing the run. |
| `activate_window` | Looks up windows via `pygetwindow.getWindowsWithTitle(title_contains)` (substring match, case-sensitive). If multiple windows match, only the first one returned by the OS is used. If not found immediately, retries up to `retry` additional times, waiting `retry_interval_ms` between attempts — useful right after `launch_app`, since the target window may not exist yet. If a window is found, it is restored first if minimized, then a key tap (Alt) is sent immediately before activating — Windows' focus-stealing prevention can otherwise silently downgrade a background process's `SetForegroundWindow` call (e.g. this runner, started from the web UI while the browser itself has focus) to just flashing the taskbar icon rather than truly switching focus, with no error raised, so every subsequent image search then quietly fails against whatever *was* actually on screen; a simulated key press counts as recent input and lifts that restriction. If no window matches after all attempts, logs a warning and does nothing. If Windows still refuses the foreground-focus switch outright (pygetwindow raises regardless, even without a meaningful error code), that's logged as a warning too rather than aborting the run. |
| `move_mouse_to_image` / `click_image` | Tries each entry in `images` in order with `pyautogui.locateOnScreen` (loaded via PIL rather than a path string, since pyscreeze's cv2-based loader can't read non-ASCII/Japanese file paths on Windows), and stops at the first one found on screen; later candidates are never checked once a match is found. The target point defaults to `position: center`; set `position` to one of the 9 named anchors to target a different point on the match (e.g. `right` for the middle of the right edge), or set `offset` for an exact `[x, y]` pixel offset from the top-left corner instead (offset wins if both are set). If none match and `retry` is set, the *entire candidate list* is retried from the top (not just the last candidate tried), waiting `retry_interval_ms` between rounds (both default to 0 retries / 500ms, so nothing changes unless a step sets `retry`). If still none match after all attempts, logs a warning and the step is skipped — the scenario continues to the next step regardless (a missing image is never treated as a fatal error). `click_image` first flashes a red circle at the target point for the default 0.25 seconds (a small always-on-top Tkinter window) before actually clicking, so the click location remains visible in screenshots/recordings; set `click_indicator_duration` to a larger value for debugging, a smaller positive number to shorten the pause, or `0` to disable it. Set `click_type: double` (default `single`) to double-click instead of single-clicking. |
| `wait` | Sleeps for `ms` milliseconds, in addition to the automatic `STEP_DELAY` (50ms) applied after every step. |
| `start` / `end` | No-ops. They only exist as visual start/end markers for the web-based flow editor and have no runtime effect. |

## On-screen run overlays

When a run fails or is stopped, PassoFlow records a final screenshot and, when screen capture is available, before/after screenshots for the failed step under `logs/`. The paths are included in the execution log; these files may contain sensitive screen content.

While a scenario runs, two always-on-top, click-through overlays (implemented in `src/overlay.py`) help show what's being automated, without ever intercepting a click meant for the real application:

- A semi-transparent (50% alpha) cyan border is drawn around whatever window `activate_window` last successfully found, redrawn/moved on each subsequent `activate_window` call. It does not track a window that moves or resizes on its own between activations.
- A small HUD box in a screen corner shows the current step (`title`/`note` if set, otherwise the raw action name), updated on every step. It defaults to the top-left corner, moving to the top-right instead if the mouse cursor is currently near the top-left.

Both are cleared automatically when the run ends (including on error) and have no effect on a scenario's YAML — there's nothing to configure.

## AI-assisted flow insertion

Open **Ask AI** while an action is selected and describe a concrete operation to add, such as “paste the 入荷番号 variable into the input”. When the request requires new steps, the assistant creates the smallest valid action sequence and inserts it immediately after the selected action. For the clipboard example this is typically `set_clipboard` with `text: "{{入荷番号}}"`, followed by `paste`. Downstream nodes are shifted down automatically to make room while their horizontal layout is preserved. The insertion is one undoable edit; review the generated parameters before saving.

## Run feedback

When a run finishes with one or more WARNING-level log lines (e.g. an image or window wasn't found), the web UI's run log sends those warnings to the model and appends its analysis as a "--- フィードバック ---" block at the end of the log, with concrete suggestions for preventing the same warnings next time (e.g. raise a step's `retry`, add a `wait`, re-check a candidate image). A run with no warnings gets no feedback block. Generation failures (no API key configured, rate limited, network error) are logged server-side and silently skipped — they never fail the run itself, since the run already finished by that point.

## Keys available for press_key / hotkey

Key names follow pyautogui's `KEYBOARD_KEYS` (194 total). Commonly used ones:

`a`-`z`, `0`-`9`, `enter`, `esc`, `tab`, `space`, `backspace`, `delete`, `up`, `down`, `left`, `right`, `home`, `end`, `pageup`, `pagedown`, `ctrl`, `ctrlleft`, `ctrlright`, `alt`, `altleft`, `altright`, `shift`, `shiftleft`, `shiftright`, `win`, `f1`-`f24`

`hotkey` presses multiple keys at the same time.

```yaml
- action: hotkey
  keys: [ctrl, v]

- action: hotkey
  keys: [ctrl, shift, esc]
```

You can list all key names with:

```
python -c "import pyautogui; print(pyautogui.KEYBOARD_KEYS)"
```

In the web UI, `key`/`keys` fields can also be filled by pressing the actual key(s) instead of picking from the dropdown — click the keyboard-icon button next to the field, then press the key (or key combination, for `hotkey`) you want recorded.

## Common options

| option | default | description |
| --- | --- | --- |
| `confidence` | `0.8` | Template matching threshold (0-1). Lower values are more prone to false matches |
| `offset` | none | `[x, y]` pixel position relative to the matched image's top-left corner. Takes precedence over `position` when both are set |
| `position` | `center` | Named point on the matched image to target: `center`, `top`, `bottom`, `left`, `right`, `top-left`, `top-right`, `bottom-left`, `bottom-right` |
| `region` | none | Optional search rectangle `[left, top, width, height]` in screen pixels. Restricting the search to the target window or panel improves speed and reduces false matches |
| `region_origin` | `screen` | Set to `active_window` to interpret `region` from the current foreground window's top-left. If `region` is omitted, the whole foreground window is searched. This is useful when the window moves; activate the intended window immediately before the image action |
| `target_window_title` | none | Optional fail-closed safety check. Before searching, the foreground window title must contain this text; use it when a wrong-window click would be costly |
| `retry` | `0` | Number of additional attempts if the image isn't found right away. `0` (default) means try once and give up, matching the previous behavior |
| `retry_interval_ms` | `500` | Milliseconds to wait between attempts. Only relevant when `retry` is greater than 0; you don't need to set this just to set `retry` |
| `click_indicator_duration` | `0.25` | Seconds to show the red click indicator before a `click_image` action. Set to `0` to disable it; increase it when recording or debugging |

Actions that take `images` try each candidate in order, useful when the same UI element looks different depending on state (background color, text color, etc.). When `retry` is set on one of these actions, a retry re-tries the whole candidate list from the top, not just the last candidate that failed.

```yaml
# retry up to 3 extra times (4 attempts total), 2 seconds apart
- action: click_image
  images:
    - images/example/ok.png
  retry: 3
  retry_interval_ms: 2000

# click the middle of the right edge of the match, e.g. a scrollbar handle or slider,
# instead of computing a manual [x, y] offset
- action: click_image
  images:
    - images/example/slider.png
  position: right

# double-click instead of a single click
- action: click_image
  images:
    - images/example/folder.png
  click_type: double
```

## Variables

Store a name (Japanese allowed) and value with `set_variable`, then reference it as `{{name}}` inside the `text` of `type_text` or `set_clipboard` to substitute the value. Variables persist only for the duration of the current scenario run.

```yaml
- action: set_variable
  name: 名前
  value: 太郎

- action: type_text
  text: "こんにちは{{名前}}さん"
```

Use `concat_variable` to build a new variable out of other variables and/or literal text — unlike `set_variable`, its `value` *is* resolved for `{{...}}` placeholders:

```yaml
- action: set_variable
  name: 姓
  value: 山田

- action: set_variable
  name: 名
  value: 太郎

- action: concat_variable
  name: 氏名
  value: "{{姓}}{{名}}様"
# 氏名 is now "山田太郎様"
```

Four actions store a date-derived value as a variable, differing only in what they format/compute — pick whichever matches what you need rather than juggling a `format` string. `days_offset` and `months_offset` (both optional, default 0) shift the date on all four and also accept negative values.

```yaml
# today (yyyy/mm/dd)
- action: set_year_month_day_variable
  name: today

# yesterday (yyyy/mm/dd)
- action: set_year_month_day_variable
  name: yesterday
  days_offset: -1

# this month (yyyymm)
- action: set_year_month_variable
  name: this_month

# last month (yyyymm)
- action: set_year_month_variable
  name: last_month
  months_offset: -1

# 1st of this month (yyyy/mm/01)
- action: set_month_start_variable
  name: month_start

# last day of this month (yyyy/mm/dd) — computes the real last day (28-31)
# via calendar.monthrange, unlike a plain months_offset's own clamping
- action: set_month_end_variable
  name: month_end

# last day of last month (yyyy/mm/dd)
- action: set_month_end_variable
  name: last_month_end
  months_offset: -1
```

## Writing YAML scenarios

A scenario file has a top-level `steps` key holding an ordered list of steps to execute. Each step has an `action` plus whatever parameters that action requires.

```yaml
steps:
  # wait for the screen transition right after launch
  - action: wait
    ms: 3000

  # the favorites button looks different depending on state, so list candidates
  - action: click_image
    images:
      - images/example/favorites-1.png
      - images/example/favorites-2.png
      - images/example/favorites-3.png

  - action: wait
    ms: 1000

  # click at (10, 5) relative to the matched image's top-left corner
  - action: click_image
    images:
      - images/example/list.png
    confidence: 0.9
    offset: [10, 5]
```

Notes:

- Both image paths and `call_scenario`'s `path` are relative to the `scenarios/` directory
- `confidence` and `offset` are optional; defaults are `confidence=0.8` and clicking the image center
- Steps run in order from top to bottom. If an image isn't found, execution does not stop with an error — it logs a warning and moves on to the next step
- After each step, `run_scenario.py` automatically waits 50ms (`STEP_DELAY`). Add a `wait` step only if you need to wait longer

## Loops

Two different ways to repeat something, for two different situations:

- **`repeat`** (an action, see the table above): references *another* scenario file and runs its steps `count` times. Use this for a reusable sub-scenario you also call standalone or from multiple places (`call_scenario`).
- **Inline loop** (`loop`/`loop_count` step fields, set by the web UI): wraps a consecutive run of steps *in the same file* to repeat `loop_count` times, without needing a separate file. In the web UI, select a consecutive run of nodes and choose "ループ化" (Make loop) — same gesture as grouping, but with real execution effect. Double-click the loop's label to rename it, change its count, or switch it to a table-driven loop (see below).

```yaml
- action: set_variable
  name: i
  value: "1"

- action: wait
  ms: 500
  loop: myLoop
  loop_count: 5

- action: click_image
  images:
    - images/foo/button.png
  loop: myLoop
  loop_count: 5
```

All steps sharing the same `loop` label must be contiguous and agree on the same `loop_count`/`loop_table` — the validator rejects a non-contiguous or inconsistent loop, or one with both/neither set.

### Looping over a table

Instead of a fixed `loop_count`, a loop can run once per row of a table loaded by an earlier `load_table` step, via `loop_table: <name>` (the loop's steps set exactly one of `loop_count` or `loop_table`, never both). Each iteration sets every column's value as a variable of that column's name (e.g. a column "顧客コード" becomes `{{顧客コード}}`) before running the loop body — same mechanism as `set_variable`, just driven by the row instead of a fixed value:

```yaml
- action: load_table
  path: C:\data\customers.xlsx
  name: customers

- action: type_text
  text: "{{顧客コード}}"
  loop: sendToEach
  loop_table: customers

- action: click_image
  images:
    - images/send_button.png
  loop: sendToEach
  loop_table: customers
```

In the web UI, use the "取り込み" (import) button in the execution panel's "取り込んだデータ" tab to pick a file: it creates or updates an internal `load_table` step that is hidden on the canvas, previews the table with all rows checked, and wraps the scenario's `Start`–`End` range in a table loop. Uncheck rows that should be skipped; only checked zero-based row indexes are saved as `selected_rows` and used when the scenario runs. The imported column names show up in the variable list and variable-name dropdowns, and each selected row's values become variables for that iteration (their values are only set once the table loop runs). Existing manual loops can still be edited independently.

For accessibility, click an action in the palette to add it after the selected step (or before `end` when no step is selected); dragging remains available. Palette items, action nodes, and terminal nodes are keyboard-focusable, and Enter or Space activates the focused item. When a node has focus, the arrow keys move focus to the nearest node in that direction; `Alt+Arrow` nudges its position (`Shift+Alt+Arrow` uses a larger step). Connections end in arrows so the flow direction is clear. Drag a left or right panel boundary to change its width, or focus the boundary and use the Left/Right arrow keys. The canvas legend explains the regular, running, and conditional-branch colors.

Deleting an action asks for confirmation and reconnects its single predecessor and successor when possible. The `Start` and `End` boundary nodes cannot be deleted.

If the referenced table was never loaded (a missing/failed `load_table` step, or a typo in the name), the loop logs a warning and runs 0 iterations rather than silently looking like a successful empty run.

## Branching

`if`/`else`/`endif` are three plain sequential steps, connected in order like any other action. The web UI also has a dedicated gesture for this: select a consecutive run of nodes and choose "Make if" (same gesture as grouping/looping) to wrap them in a new `if`/`else`/`endif` block — an orange frame is drawn around the block with separate "true" and "false" regions. Drop an action into the upper region to add it to the true branch, or into the lower region to add it to the false branch. You can set `variable`/`equals` on the `if` node itself via the parameter panel. Existing scenarios without an `else` remain supported; right-click the frame's label to add the missing `else` branch or to remove the wrapper again (right-click → "Remove if"; keeps the body steps, just drops the branching):

```yaml
- action: set_variable
  name: status
  value: ok

- action: if
  variable: status
  equals: ok

- action: click_image           # runs only if status == "ok"
  images:
    - images/foo/ok_button.png

- action: else

- action: click_image           # runs only if status != "ok"
  images:
    - images/foo/retry_button.png

- action: endif

- action: wait                  # always runs, after the if/else block
  ms: 500
```

- The condition compares as **text**: `equals` is compared against the variable's value with `str(...) == str(...)`, so `equals: "5"` and a variable holding the number `5` still match. An unset variable reads as `""`.
- Omit `equals` to just check whether the variable is set to something non-empty (a truthy check), e.g. `if: variable: result` after a step that only sets `result` on success.
- Instead of `variable`, use `last_step: warned` to run the branch when the immediately previous action logged a warning but continued (for example, an image was not found or a webhook failed). Use `last_step: ok` to run it only when the previous action emitted no warning. `variable` and `last_step` are mutually exclusive.
- `else` is optional — an `if` with no `else` simply does nothing when its condition is false.
- `if`/`else`/`endif` blocks can nest; the validator matches each `if` to its own `else`/`endif` by nesting depth, and rejects an orphaned `else`/`endif`, a duplicate `else` for the same `if`, or an `if` with no `endif`.
- `last_step` only describes an action that completed and allowed the scenario to continue. A fatal exception aborts the run before a following `if` can execute.

## Validation

## Action outcomes

Every action has the same three-state contract: a successful action continues normally; a known recoverable problem may emit a warning and continue; an unexpected exception or validation error stops the run. Image search, window activation, file/network operations, table loading, and Excel read/write actions use the warning/continue path for expected operational misses. `send_webhook` warns and continues by default, or stops when `on_error: stop` is selected. The `/api/actions` response exposes this contract as `outcomes` for editor integrations.

Before running any step, `run_scenario.py` validates the whole scenario — including every file reachable through `call_scenario`, recursively. This catches authoring mistakes before the RPA touches the screen, rather than partway through a run.

- **Errors** (abort the run before anything executes, listing every problem found, not just the first one): unknown `action`, a required parameter missing, an unrecognized parameter (e.g. a typo like `retries` instead of `retry`), `images`/`keys` not written as a list, `offset` not a 2-element `[x, y]` list, an unrecognized `position` value, a `call_scenario`/`repeat` target that doesn't exist, a circular `call_scenario`/`repeat` reference, `repeat`'s `count` not a positive integer, `send_webhook`'s `payload` not a mapping or a JSON string, `loop` steps that aren't contiguous or don't agree on `loop_count`, a `loop_count` that isn't a positive integer, an orphaned `else`/`endif`, a duplicate `else` for one `if`, an `if` with neither or both of `variable`/`last_step`, an invalid `last_step` value, or an `if` with no matching `endif`
- **Warnings** (logged, but the run proceeds): an `images` entry that doesn't exist under `scenarios/`

`note`, `group`, and `title` are recognized meta keys you can add to any step that the runner otherwise ignores (`note` is a free-text memo; `group`/`title` are set by the web UI's grouping and per-step title features). `loop`/`loop_count`/`loop_table` are also recognized, but unlike the other three, they *do* affect execution — see Loops above.
