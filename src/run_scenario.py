"""Run a sequence of mouse actions defined in a YAML scenario file."""

import argparse
import json
import logging
import os
import re
import time
import urllib.error
import urllib.request
from pathlib import Path

import pyautogui
import yaml

from app_paths import app_root
from input_actions import (
    activate_window,
    clear_input,
    copy_file,
    create_excel_sheet,
    delete_excel_row,
    delete_excel_sheet,
    get_date,
    get_excel_cell_value,
    hotkey,
    launch_app,
    map_network_drive,
    move_file,
    open_excel_file,
    open_new_excel,
    paste,
    press_key,
    read_table_rows,
    rename_file,
    run_excel_macro,
    save_excel_file,
    set_clipboard,
    set_excel_cell_value,
    sort_excel_range,
    type_text,
)
from logging_config import setup_logging
from overlay import overlay
from web_actions import (
    browser_click,
    browser_fill,
    browser_navigate,
    browser_wait_for,
    close_browser,
    open_url,
)
from rust_validator import (
    assert_plan_matches_steps,
    normalize_with_rust,
    plan_with_rust,
    steps_from_rust_plan,
    validate_with_rust,
)
from screen_actions import (
    CONFIDENCE,
    POSITION,
    POSITIONS,
    REGION_ORIGINS,
    RETRIES,
    RETRY_INTERVAL_MS,
    click_image,
    move_mouse_to_image,
)

SCENARIOS_DIR = app_root() / "scenarios"

STEP_DELAY = 0.05  # Seconds to wait after every step, so scenarios don't need an explicit wait between each action.

VARIABLE_PATTERN = re.compile(r"\{\{(.+?)\}\}")

logger = logging.getLogger(__name__)

RUN_ID: str | None = None
RUST_ENGINE_TOTAL: int | None = None

# Outcome contract: actions normally succeed or stop on an unexpected exception. These
# actions have a documented, recoverable warning path that lets the scenario continue.
_WARNING_CONTINUE_ACTIONS = {
    "activate_window",
    "copy_file",
    "create_excel_sheet",
    "delete_excel_row",
    "delete_excel_sheet",
    "get_excel_value",
    "load_table",
    "map_network_drive",
    "move_file",
    "move_mouse_to_image",
    "click_image",
    "rename_file",
    "run_excel_macro",
    "save_excel_file",
    "set_excel_value",
    "sort_excel_range",
}


def action_outcome_contract(action: str, params: dict | None = None) -> dict[str, bool]:
    """Describe the outcomes an action may produce without executing it."""
    if action == "send_webhook" and (params or {}).get("on_error", "continue") == "continue":
        warning_continue = True
    else:
        warning_continue = action in _WARNING_CONTINUE_ACTIONS
    return {"success": True, "warning_continue": warning_continue, "failure_stop": True}


class _StepWarningHandler(logging.Handler):
    """Record whether one action emitted a warning without changing log output."""

    def __init__(self) -> None:
        super().__init__(level=logging.WARNING)
        self.warned = False

    def emit(self, record: logging.LogRecord) -> None:
        if record.levelno >= logging.WARNING:
            self.warned = True


def _rust_warning_outcome(action: str, step: dict, warned: bool) -> str:
    """Map callback warnings to the same action outcome contract as Python runs."""
    if not warned:
        return "success"
    contract = action_outcome_contract(action, step)
    return "warning_continue" if contract["warning_continue"] else "failure_stop"

# Tables loaded by load_table steps, keyed by their `name`, for loop_table blocks to iterate
# over. Module-level rather than threaded through every function alongside `variables`, since
# each run is a fresh, single-threaded subprocess (see api_server.py's stream_run) — nothing
# else needs to see or reset this between runs.
_LOADED_TABLES: dict[str, list[dict[str, str]]] = {}


def _resolve(text: str, variables: dict[str, str]) -> str:
    """Replace {{変数名}} placeholders in text with their stored values."""
    return VARIABLE_PATTERN.sub(lambda m: variables.get(m.group(1).strip(), ""), text)


def _capture_step_screenshot() -> object | None:
    """Capture a best-effort in-memory image before a step."""
    try:
        return pyautogui.screenshot()
    except Exception as error:
        logger.debug("Could not capture pre-step screenshot: %s", error)
        return None


def _save_failure_context(before: object | None, step_number: int, action: str) -> list[str]:
    """Save before/after images for one failed step without masking the failure."""
    if before is None or not RUN_ID:
        return []
    try:
        after = pyautogui.screenshot()
        prefix = app_root() / "logs" / f"run_{RUN_ID}_step_{step_number}_{action}"
        prefix.parent.mkdir(parents=True, exist_ok=True)
        before.save(f"{prefix}_before.png")
        after.save(f"{prefix}_after.png")
        logger.error("Failure screenshots saved for step %d: %s_before.png and %s_after.png", step_number, prefix, prefix)
        return [f"{prefix}_before.png", f"{prefix}_after.png"]
    except Exception as error:
        logger.warning("Could not save before/after screenshots for step %d: %s", step_number, error)
        return []


def _run_set_variable(step: dict, variables: dict[str, str]) -> None:
    variables[step["name"]] = step["value"]
    logger.info("Set variable %s = %s", step["name"], step["value"])


def _run_concat_variable(step: dict, variables: dict[str, str]) -> None:
    value = _resolve(step["value"], variables)
    variables[step["name"]] = value
    logger.info("Set variable %s = %s", step["name"], value)


def _date_offsets(step: dict) -> dict:
    return {"days_offset": step.get("days_offset", 0), "months_offset": step.get("months_offset", 0)}


def _run_set_year_month_variable(step: dict, variables: dict[str, str]) -> None:
    value = get_date("%Y%m", **_date_offsets(step))
    variables[step["name"]] = value
    logger.info("Set variable %s = %s", step["name"], value)


def _run_set_year_month_day_variable(step: dict, variables: dict[str, str]) -> None:
    value = get_date("%Y/%m/%d", **_date_offsets(step))
    variables[step["name"]] = value
    logger.info("Set variable %s = %s", step["name"], value)


def _run_set_month_start_variable(step: dict, variables: dict[str, str]) -> None:
    value = get_date("%Y/%m/01", **_date_offsets(step))
    variables[step["name"]] = value
    logger.info("Set variable %s = %s", step["name"], value)


def _run_set_month_end_variable(step: dict, variables: dict[str, str]) -> None:
    value = get_date("%Y/%m/%d", end_of_month=True, **_date_offsets(step))
    variables[step["name"]] = value
    logger.info("Set variable %s = %s", step["name"], value)


def _run_type_text(step: dict, variables: dict[str, str]) -> None:
    type_text(_resolve(step["text"], variables))


def _run_paste_variable(step: dict, variables: dict[str, str]) -> None:
    type_text(variables.get(step["name"], ""))


def _run_set_clipboard(step: dict, variables: dict[str, str]) -> None:
    set_clipboard(_resolve(step["text"], variables))


def _run_paste(step: dict, variables: dict[str, str]) -> None:
    paste()


def _run_clear_input(step: dict, variables: dict[str, str]) -> None:
    clear_input()


def _run_press_key(step: dict, variables: dict[str, str]) -> None:
    press_key(step["key"], wait_ms=step.get("wait", 100))


def _run_hotkey(step: dict, variables: dict[str, str]) -> None:
    hotkey(*step["keys"])


def _run_launch_app(step: dict, variables: dict[str, str]) -> None:
    launch_app(
        _resolve(step["path"], variables),
        args=[_resolve(arg, variables) for arg in (step.get("args") or [])],
        wait_for_window=_resolve(step["wait_for_window"], variables) if step.get("wait_for_window") else None,
        startup_timeout_ms=step.get("startup_timeout_ms", 10_000),
    )


def _run_rename_file(step: dict, variables: dict[str, str]) -> None:
    rename_file(_resolve(step["path"], variables), _resolve(step["new_name"], variables))


def _run_move_file(step: dict, variables: dict[str, str]) -> None:
    move_file(_resolve(step["path"], variables), _resolve(step["destination"], variables))


def _run_copy_file(step: dict, variables: dict[str, str]) -> None:
    copy_file(
        _resolve(step["path"], variables),
        _resolve(step["destination"], variables),
        step.get("if_destination_newer", "overwrite"),
    )


def _run_map_network_drive(step: dict, variables: dict[str, str]) -> None:
    map_network_drive(_resolve(step["drive"], variables), _resolve(step["path"], variables))


def _run_open_excel_file(step: dict, variables: dict[str, str]) -> None:
    open_excel_file(_resolve(step["path"], variables))


def _run_open_new_excel(step: dict, variables: dict[str, str]) -> None:
    open_new_excel()


def _resolve_excel_sheet(step: dict, variables: dict[str, str]) -> str | None:
    return _resolve(step["sheet"], variables) if step.get("sheet") else None


def _run_get_excel_value(step: dict, variables: dict[str, str]) -> None:
    sheet = _resolve_excel_sheet(step, variables)
    value = get_excel_cell_value(_resolve(step["path"], variables), _resolve(step["cell"], variables), sheet=sheet)
    variables[step["name"]] = value
    logger.info("Set variable %s = %s", step["name"], value)


def _run_set_excel_value(step: dict, variables: dict[str, str]) -> None:
    sheet = _resolve_excel_sheet(step, variables)
    set_excel_cell_value(
        _resolve(step["path"], variables),
        _resolve(step["cell"], variables),
        _resolve(step["value"], variables),
        sheet=sheet,
    )


def _run_save_excel_file(step: dict, variables: dict[str, str]) -> None:
    save_excel_file(_resolve(step["path"], variables))


def _run_create_excel_sheet(step: dict, variables: dict[str, str]) -> None:
    create_excel_sheet(_resolve(step["path"], variables), _resolve(step["sheet"], variables))


def _run_delete_excel_sheet(step: dict, variables: dict[str, str]) -> None:
    delete_excel_sheet(_resolve(step["path"], variables), _resolve(step["sheet"], variables))


def _run_delete_excel_row(step: dict, variables: dict[str, str]) -> None:
    sheet = _resolve_excel_sheet(step, variables)
    delete_excel_row(_resolve(step["path"], variables), int(step["row"]), sheet=sheet)


def _run_sort_excel_range(step: dict, variables: dict[str, str]) -> None:
    sheet = _resolve_excel_sheet(step, variables)
    sort_excel_range(
        _resolve(step["path"], variables),
        _resolve(step["range"], variables),
        _resolve(step["key_cell"], variables),
        order=step.get("order", "asc"),
        sheet=sheet,
    )


def _run_run_excel_macro(step: dict, variables: dict[str, str]) -> None:
    args = [_resolve(a, variables) for a in step.get("args") or []]
    run_excel_macro(_resolve(step["path"], variables), _resolve(step["macro"], variables), args=args)


def _run_load_table(step: dict, variables: dict[str, str]) -> None:
    path = _resolve(step["path"], variables)
    sheet = _resolve_excel_sheet(step, variables)
    try:
        rows = read_table_rows(path, sheet)
    except Exception as e:
        # Loud on purpose: a loop_table block whose table failed to load would otherwise just
        # silently run zero iterations, which looks exactly like a successful empty run.
        logger.warning("Could not load table %r from %s: %s — loop_table blocks using it will run 0 iterations", step["name"], path, e)
        rows = []
    selected_rows = step.get("selected_rows")
    if isinstance(selected_rows, list):
        selected = {index for index in selected_rows if isinstance(index, int) and not isinstance(index, bool) and index >= 0}
        rows = [row for index, row in enumerate(rows) if index in selected]
    _LOADED_TABLES[step["name"]] = rows
    logger.info("Loaded table %r: %d row(s) from %s", step["name"], len(rows), path)


def _run_activate_window(step: dict, variables: dict[str, str]) -> None:
    activate_window(
        step["title_contains"],
        retries=step.get("retry", RETRIES),
        retry_interval_ms=step.get("retry_interval_ms", RETRY_INTERVAL_MS),
    )


def _run_open_url(step: dict, variables: dict[str, str]) -> None:
    open_url(_resolve(step["url"], variables))


def _run_browser_navigate(step: dict, variables: dict[str, str]) -> None:
    browser_navigate(_resolve(step["url"], variables), timeout_ms=step.get("timeout_ms", 30_000))


def _run_browser_click(step: dict, variables: dict[str, str]) -> None:
    browser_click(_resolve(step["selector"], variables), timeout_ms=step.get("timeout_ms", 10_000))


def _run_browser_fill(step: dict, variables: dict[str, str]) -> None:
    browser_fill(
        _resolve(step["selector"], variables),
        _resolve(step["text"], variables),
        timeout_ms=step.get("timeout_ms", 10_000),
    )


def _run_browser_wait_for(step: dict, variables: dict[str, str]) -> None:
    browser_wait_for(
        _resolve(step["selector"], variables),
        state=step.get("state", "visible"),
        timeout_ms=step.get("timeout_ms", 10_000),
    )


def _offset(step: dict) -> tuple[int, int] | None:
    """Read an [x, y] offset from a step, or None to use the match's center."""
    offset = step.get("offset")
    return tuple(offset) if offset is not None else None


def _region(step: dict) -> tuple[int, int, int, int] | None:
    """Read an optional screen search region [left, top, width, height]."""
    region = step.get("region")
    return tuple(region) if region is not None else None


def _image_search_kwargs(step: dict) -> dict:
    """Read the options common to every image-search action: confidence, offset, position, retry, retry_interval_ms."""
    return {
        "confidence": step.get("confidence", CONFIDENCE),
        "offset": _offset(step),
        "region": _region(step),
        "region_origin": step.get("region_origin", "screen"),
        "target_window_title": step.get("target_window_title"),
        "position": step.get("position", POSITION),
        "retries": step.get("retry", RETRIES),
        "retry_interval_ms": step.get("retry_interval_ms", RETRY_INTERVAL_MS),
    }


def _run_click_image(step: dict, variables: dict[str, str]) -> None:
    images = [SCENARIOS_DIR / image for image in step["images"]]
    click_image(
        images,
        double_click=step.get("click_type") == "double",
        click_indicator_duration=step.get("click_indicator_duration", 0.25),
        **_image_search_kwargs(step),
    )


def _run_move_mouse_to_image(step: dict, variables: dict[str, str]) -> None:
    images = [SCENARIOS_DIR / image for image in step["images"]]
    move_mouse_to_image(images, **_image_search_kwargs(step))


def _run_wait(step: dict, variables: dict[str, str]) -> None:
    time.sleep(step["ms"] / 1000)


def _run_noop(step: dict, variables: dict[str, str]) -> None:
    """Start/end/if/else/endif markers carry no behavior of their own.

    if/else/endif are intercepted by _run_steps before ever reaching ACTIONS (see there for the
    branch logic) — this entry only exists so the schema/label lookups elsewhere have something
    to dispatch to if one is ever encountered outside that scan (e.g. a malformed scenario that
    somehow got past validation).
    """


def _rust_runtime_callback(step_json: str, state_json: str) -> str:
    """Adapt one Rust runtime callback invocation to the existing Python actions."""
    step = json.loads(step_json)
    state = json.loads(state_json)
    if not isinstance(step, dict) or not isinstance(state, dict):
        raise ValueError("Rust runtime callback received malformed JSON")
    variables = dict(state.get("variables", {}))
    action = step.get("action")
    if not isinstance(action, str) or action in {"if", "else", "endif"}:
        raise ValueError(f"Rust runtime callback received structural action: {action!r}")
    action_handler = ACTIONS.get(action)
    if action_handler is None:
        raise ValueError(f"Rust runtime callback received unsupported action: {action!r}")
    step_number = int(step.get("index", 0))
    if RUST_ENGINE_TOTAL:
        print(f"@@PROGRESS@@{step_number}/{RUST_ENGINE_TOTAL}", flush=True)
        overlay.set_step_label(f"Step {step_number}/{RUST_ENGINE_TOTAL}: {step.get('title') or step.get('note') or action}")
    warning_handler = _StepWarningHandler()
    logging.getLogger().addHandler(warning_handler)
    before_screenshot = _capture_step_screenshot() if RUN_ID else None
    try:
        action_handler(step, variables)
    except Exception as error:
        artifact_paths = _save_failure_context(before_screenshot, step_number, action)
        return json.dumps(
            {
                "result": {
                    "outcome": "failure_stop",
                    "message": str(error),
                    "artifacts": [
                        {"kind": "step_screenshot", "path": path} for path in artifact_paths
                    ],
                },
                "variables": variables,
            }
        )
    finally:
        logging.getLogger().removeHandler(warning_handler)
    outcome = _rust_warning_outcome(action, step, warning_handler.warned)
    if outcome == "failure_stop":
        artifact_paths = _save_failure_context(before_screenshot, step_number, action)
        return json.dumps(
            {
                "result": {
                    "outcome": outcome,
                    "message": f"{action} emitted a non-recoverable warning",
                    "artifacts": [
                        {"kind": "step_screenshot", "path": path} for path in artifact_paths
                    ],
                },
                "variables": variables,
            }
        )
    if RUST_ENGINE_TOTAL:
        print(f"@@COMPLETED@@{step_number}/{RUST_ENGINE_TOTAL}", flush=True)
    return json.dumps(
        {
            "result": {"outcome": outcome, "message": action, "artifacts": []},
            "variables": variables,
        }
    )


def _rust_stop_requested() -> bool:
    """Return whether the API requested a cooperative stop for this process."""
    stop_file = os.environ.get("PASSOFLOW_STOP_FILE")
    return bool(stop_file and Path(stop_file).is_file())


def _run_with_rust_engine(yaml_path: str | Path, steps: list[dict], run_id: str | None) -> None:
    """Run a root scenario through Rust, including Rust-owned nested expansion."""
    try:
        import passoflow_python
    except ImportError as error:
        raise RuntimeError(
            "PASSOFLOW_USE_RUST_ENGINE=1 requires the passoflow Python binding; install the wheel or build it with maturin"
        ) from error
    sources = _collect_rust_nested_sources(yaml_path, steps)
    expanded_steps = json.loads(
        passoflow_python.expand_nested_steps(
            yaml.safe_dump({"steps": steps}, sort_keys=False, allow_unicode=True),
            json.dumps(sources, ensure_ascii=False),
        )
    )
    global RUST_ENGINE_TOTAL
    RUST_ENGINE_TOTAL = len(expanded_steps)
    _LOADED_TABLES.clear()
    # Rust selects table-loop iterations before invoking the Python action callback. Preload
    # declared tables with the same loader and selected-row policy used by the compatibility
    # runner. The load_table callback still runs at its original step for logging and parity.
    for step in expanded_steps:
        if step.get("action") == "load_table":
            _run_load_table(step, {})
    try:
        report_json = passoflow_python.run_runtime_state(
            yaml.safe_dump({"steps": expanded_steps}, sort_keys=False, allow_unicode=True),
            "{}",
            json.dumps(_LOADED_TABLES, ensure_ascii=False),
            _rust_runtime_callback,
            _rust_stop_requested,
            run_id or "run",
            1,
            0,
        )
    finally:
        RUST_ENGINE_TOTAL = None
    report = json.loads(report_json)
    for event in report.get("events", []):
        logger.info("Rust engine step %s: %s", event.get("step"), event.get("message"))
        for artifact in event.get("artifacts", []):
            if isinstance(artifact, dict) and artifact.get("path"):
                logger.error("Failure artifact (%s): %s", artifact.get("kind", "artifact"), artifact["path"])
    status = report.get("status")
    if status == "failure_stop":
        raise RuntimeError("Rust engine stopped after an action failure")
    if status == "stopped":
        raise RuntimeError("Rust engine run was stopped")


def _run_call_scenario(step: dict, variables: dict[str, str], runtime_state: dict[str, str] | None = None) -> None:
    """Run another scenario file's steps inline, sharing the current variables."""
    steps = _load_steps(SCENARIOS_DIR / step["path"])
    # Don't emit progress markers for the nested run: its step numbers are relative to the
    # sub-scenario, not the caller's, so the web UI keeps highlighting this call_scenario step.
    _run_steps(steps, variables, emit_progress=False, runtime_state=runtime_state)


def _run_repeat(step: dict, variables: dict[str, str], runtime_state: dict[str, str] | None = None) -> None:
    """Run another scenario file's steps inline, `count` times, sharing the current variables."""
    steps = _load_steps(SCENARIOS_DIR / step["path"])
    count = int(step["count"])
    for i in range(count):
        logger.info("Repeat %d/%d: %s", i + 1, count, step["path"])
        _run_steps(steps, variables, emit_progress=False, runtime_state=runtime_state)


def _resolve_deep(value, variables: dict[str, str]):
    """Recursively replace {{変数名}} placeholders in every string found inside value."""
    if isinstance(value, str):
        return _resolve(value, variables)
    if isinstance(value, dict):
        return {k: _resolve_deep(v, variables) for k, v in value.items()}
    if isinstance(value, list):
        return [_resolve_deep(v, variables) for v in value]
    return value


def _evaluate_condition(step: dict, variables: dict[str, str], runtime_state: dict[str, str]) -> bool:
    """Evaluate a variable condition or the outcome of the immediately previous action.

    An unset variable reads as "" (same default _resolve uses), so a bare `if: variable: foo`
    with no `equals` is a "was foo ever set to something non-empty" check.
    """
    if "last_step" in step:
        return runtime_state.get("last_step") == step["last_step"]

    value = variables.get(step["variable"], "")
    if "equals" in step:
        return str(value) == str(step["equals"])
    return bool(value)


def _find_if_boundaries(steps: list[dict], if_index: int) -> tuple[int | None, int]:
    """Given steps[if_index] is an 'if' step, return (else_index_or_None, endif_index).

    Tracks nesting depth so a nested if's own else/endif isn't mistaken for this one's.
    """
    depth = 0
    else_idx = None
    j = if_index + 1
    while j < len(steps):
        action = steps[j].get("action") if isinstance(steps[j], dict) else None
        if action == "if":
            depth += 1
        elif action == "endif":
            if depth == 0:
                return else_idx, j
            depth -= 1
        elif action == "else" and depth == 0 and else_idx is None:
            else_idx = j
        j += 1
    raise ValueError(f"'if' at step {if_index + 1} has no matching 'endif'")


def _run_send_webhook(step: dict, variables: dict[str, str]) -> None:
    """POST/GET a URL. `payload` may be a mapping (resolved recursively) or a raw JSON string."""
    url = _resolve(step["url"], variables)
    method = step.get("method", "POST").upper()

    payload = step.get("payload")
    if isinstance(payload, str):
        payload = json.loads(_resolve(payload, variables)) if payload.strip() else None
    elif payload is not None:
        payload = _resolve_deep(payload, variables)

    body = json.dumps(payload).encode("utf-8") if payload is not None else None
    headers = {"Content-Type": "application/json"} if body is not None else {}
    request = urllib.request.Request(url, data=body, headers=headers, method=method)
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            logger.info("Webhook %s %s -> %d", method, url, response.status)
    except urllib.error.URLError as e:
        if step.get("on_error", "continue") == "stop":
            raise RuntimeError(f"Webhook {method} {url} failed: {e}") from e
        logger.warning("Webhook %s %s failed: %s", method, url, e)


ACTIONS = {
    "start": _run_noop,
    "end": _run_noop,
    "if": _run_noop,
    "else": _run_noop,
    "endif": _run_noop,
    "call_scenario": _run_call_scenario,
    "repeat": _run_repeat,
    "send_webhook": _run_send_webhook,
    "set_variable": _run_set_variable,
    "concat_variable": _run_concat_variable,
    "set_year_month_variable": _run_set_year_month_variable,
    "set_year_month_day_variable": _run_set_year_month_day_variable,
    "set_month_start_variable": _run_set_month_start_variable,
    "set_month_end_variable": _run_set_month_end_variable,
    "activate_window": _run_activate_window,
    "open_url": _run_open_url,
    "browser_navigate": _run_browser_navigate,
    "browser_click": _run_browser_click,
    "browser_fill": _run_browser_fill,
    "browser_wait_for": _run_browser_wait_for,
    "launch_app": _run_launch_app,
    "rename_file": _run_rename_file,
    "move_file": _run_move_file,
    "copy_file": _run_copy_file,
    "map_network_drive": _run_map_network_drive,
    "open_excel_file": _run_open_excel_file,
    "open_new_excel": _run_open_new_excel,
    "get_excel_value": _run_get_excel_value,
    "set_excel_value": _run_set_excel_value,
    "save_excel_file": _run_save_excel_file,
    "create_excel_sheet": _run_create_excel_sheet,
    "delete_excel_sheet": _run_delete_excel_sheet,
    "delete_excel_row": _run_delete_excel_row,
    "sort_excel_range": _run_sort_excel_range,
    "run_excel_macro": _run_run_excel_macro,
    "load_table": _run_load_table,
    "type_text": _run_type_text,
    "set_clipboard": _run_set_clipboard,
    "paste": _run_paste,
    "paste_variable": _run_paste_variable,
    "clear_input": _run_clear_input,
    "press_key": _run_press_key,
    "hotkey": _run_hotkey,
    "click_image": _run_click_image,
    "move_mouse_to_image": _run_move_mouse_to_image,
    "wait": _run_wait,
}


def _load_steps(yaml_path: str | Path) -> list[dict]:
    if os.environ.get("PASSOFLOW_USE_RUST_VALIDATOR") == "1":
        normalized = normalize_with_rust(yaml_path)
        if normalized is not None:
            scenario = yaml.safe_load(normalized)
            return scenario["steps"]
    with open(yaml_path, encoding="utf-8") as f:
        scenario = yaml.safe_load(f)
    return scenario["steps"]


def _collect_rust_nested_sources(yaml_path: str | Path, steps: list[dict]) -> dict[str, str]:
    """Load nested YAML documents into the bundle consumed by the Rust expander."""
    del yaml_path  # Nested paths are scenario-rooted for compatibility with the existing runner.
    sources: dict[str, str] = {}
    pending = list(steps)
    while pending:
        step = pending.pop()
        if not isinstance(step, dict) or step.get("action") not in {"call_scenario", "repeat"}:
            continue
        target = step.get("path")
        if not isinstance(target, str):
            continue
        nested_path = (SCENARIOS_DIR / target).resolve()
        try:
            key = nested_path.relative_to(SCENARIOS_DIR.resolve()).as_posix()
        except ValueError as error:
            raise ValueError(f"nested scenario path escapes the scenarios directory: {target}") from error
        if key in sources:
            continue
        source = nested_path.read_text(encoding="utf-8")
        sources[key] = source
        nested = yaml.safe_load(source) or {}
        pending.extend(nested.get("steps", []))
    return sources


# Keys allowed on every step regardless of action, set by the web UI rather than by hand.
# "note" is a documentation-only memo; "group" is a visual grouping label; "title" is a
# user-entered display label — the runner ignores all three. "loop"/"loop_count"/"loop_table"
# mark membership in an inline loop block (see _run_steps) and DO affect execution, unlike the
# rest — a loop block has exactly one of loop_count (fixed repeat count) or loop_table (once per
# row of a table registered by an earlier load_table step).
_STEP_META_KEYS = {"action", "note", "group", "title", "loop", "loop_count", "loop_table"}

_RETRY_OPTIONS = {"retry", "retry_interval_ms"}
_IMAGE_SEARCH_OPTIONS = _RETRY_OPTIONS | {"confidence", "offset", "position", "region", "region_origin", "target_window_title"}

# action -> (required parameter names, optional parameter names). Keep in sync with ACTIONS above.
_ACTION_SCHEMA = {
    "start": (set(), set()),
    "end": (set(), set()),
    "if": (set(), {"variable", "equals", "last_step"}),
    "else": (set(), set()),
    "endif": (set(), set()),
    "call_scenario": ({"path"}, set()),
    "repeat": ({"path", "count"}, set()),
    "send_webhook": ({"url"}, {"method", "payload", "on_error"}),
    "set_variable": ({"name", "value"}, set()),
    "concat_variable": ({"name", "value"}, set()),
    "set_year_month_variable": ({"name"}, {"days_offset", "months_offset"}),
    "set_year_month_day_variable": ({"name"}, {"days_offset", "months_offset"}),
    "set_month_start_variable": ({"name"}, {"days_offset", "months_offset"}),
    "set_month_end_variable": ({"name"}, {"days_offset", "months_offset"}),
    "type_text": ({"text"}, set()),
    "set_clipboard": ({"text"}, set()),
    "paste": (set(), set()),
    "paste_variable": ({"name"}, set()),
    "clear_input": (set(), set()),
    "press_key": ({"key"}, {"wait"}),
    "hotkey": ({"keys"}, set()),
    "launch_app": ({"path"}, {"args", "wait_for_window", "startup_timeout_ms"}),
    "rename_file": ({"path", "new_name"}, set()),
    "move_file": ({"path", "destination"}, set()),
    "copy_file": ({"path", "destination"}, {"if_destination_newer"}),
    "map_network_drive": ({"drive", "path"}, set()),
    "open_excel_file": ({"path"}, set()),
    "open_new_excel": (set(), set()),
    "get_excel_value": ({"path", "cell", "name"}, {"sheet"}),
    "set_excel_value": ({"path", "cell", "value"}, {"sheet"}),
    "save_excel_file": ({"path"}, set()),
    "create_excel_sheet": ({"path", "sheet"}, set()),
    "delete_excel_sheet": ({"path", "sheet"}, set()),
    "delete_excel_row": ({"path", "row"}, {"sheet"}),
    "sort_excel_range": ({"path", "range", "key_cell"}, {"sheet", "order"}),
    "run_excel_macro": ({"path", "macro"}, {"args"}),
    # "columns" is UI-only metadata (the web UI's editor-time preview of the file's column
    # names). "selected_rows" is persisted by the import panel so unchecked rows can be skipped.
    "load_table": ({"path", "name"}, {"sheet", "columns", "selected_rows"}),
    "activate_window": ({"title_contains"}, _RETRY_OPTIONS),
    "open_url": ({"url"}, set()),
    "browser_navigate": ({"url"}, {"timeout_ms"}),
    "browser_click": ({"selector"}, {"timeout_ms"}),
    "browser_fill": ({"selector", "text"}, {"timeout_ms"}),
    "browser_wait_for": ({"selector"}, {"state", "timeout_ms"}),
    "move_mouse_to_image": ({"images"}, _IMAGE_SEARCH_OPTIONS),
    "click_image": ({"images"}, _IMAGE_SEARCH_OPTIONS | {"click_type", "click_indicator_duration"}),
    "wait": ({"ms"}, set()),
}

_IMAGE_ACTIONS = {"move_mouse_to_image", "click_image"}
_CLICK_TYPES = {"single", "double"}
_COPY_NEWER_MODES = {"overwrite", "skip"}
_BROWSER_WAIT_STATES = {"attached", "detached", "hidden", "visible"}


def _validate_step(step: dict, label: str, errors: list[str], warnings: list[str]) -> None:
    """Check one step against _ACTION_SCHEMA, appending problem descriptions to errors/warnings."""
    if not isinstance(step, dict):
        errors.append(f"{label}: step must be a mapping, got {type(step).__name__}")
        return

    if "action" not in step:
        errors.append(f"{label}: missing required key 'action'")
        return
    action = step["action"]
    if not isinstance(action, str):
        errors.append(f"{label}: 'action' must be a string")
        return
    if not action:
        errors.append(f"{label}: missing required key 'action'")
        return

    schema = _ACTION_SCHEMA.get(action)
    if schema is None:
        errors.append(f"{label}: unknown action '{action}'")
        return

    required, optional = schema
    keys = set(step.keys())

    missing = required - keys
    if missing:
        errors.append(f"{label} ({action}): missing required parameter(s): {', '.join(sorted(missing))}")

    unknown = keys - required - optional - _STEP_META_KEYS
    if unknown:
        errors.append(f"{label} ({action}): unknown parameter(s): {', '.join(sorted(unknown))}")

    if "images" in step and not isinstance(step["images"], list):
        errors.append(f"{label} ({action}): 'images' must be a list")
    if "keys" in step and not isinstance(step["keys"], list):
        errors.append(f"{label} ({action}): 'keys' must be a list")
    if "offset" in step and not (isinstance(step["offset"], list) and len(step["offset"]) == 2):
        errors.append(f"{label} ({action}): 'offset' must be a 2-element list [x, y]")
    elif "offset" in step and not all(isinstance(value, int) and not isinstance(value, bool) for value in step["offset"]):
        errors.append(f"{label} ({action}): 'offset' values must be integers [x, y]")
    if "region" in step and not (isinstance(step["region"], list) and len(step["region"]) == 4):
        errors.append(f"{label} ({action}): 'region' must be a 4-element list [left, top, width, height]")
    elif "region" in step and not all(isinstance(value, int) and not isinstance(value, bool) for value in step["region"]):
        errors.append(f"{label} ({action}): 'region' values must be integers")
    elif "region" in step and (step["region"][2] <= 0 or step["region"][3] <= 0):
        errors.append(f"{label} ({action}): 'region' width and height must be positive")
    if "position" in step and step["position"] not in POSITIONS:
        errors.append(
            f"{label} ({action}): unknown position '{step['position']}' (must be one of: {', '.join(sorted(POSITIONS))})"
        )
    if "region_origin" in step and step["region_origin"] not in REGION_ORIGINS:
        errors.append(f"{label} ({action}): unknown region_origin '{step['region_origin']}' (must be screen or active_window)")
    if "confidence" in step and (
        isinstance(step["confidence"], bool)
        or not isinstance(step["confidence"], (int, float))
        or not 0 <= step["confidence"] <= 1
    ):
        errors.append(f"{label} ({action}): 'confidence' must be a number from 0 to 1")
    for option in ("retry", "retry_interval_ms", "click_indicator_duration"):
        if option in step and (
            isinstance(step[option], bool)
            or not isinstance(step[option], (int, float))
            or step[option] < 0
        ):
            errors.append(f"{label} ({action}): '{option}' must be a non-negative number")
    if "startup_timeout_ms" in step and (
        isinstance(step["startup_timeout_ms"], bool)
        or not isinstance(step["startup_timeout_ms"], (int, float))
        or step["startup_timeout_ms"] < 0
    ):
        errors.append(f"{label} ({action}): 'startup_timeout_ms' must be a non-negative number")

    if action in _IMAGE_ACTIONS and isinstance(step.get("images"), list):
        for image in step["images"]:
            if not (SCENARIOS_DIR / image).is_file():
                warnings.append(f"{label} ({action}): image file not found: {image}")
    if action == "click_image" and "click_type" in step and step["click_type"] not in _CLICK_TYPES:
        errors.append(
            f"{label} ({action}): unknown click_type '{step['click_type']}' (must be one of: {', '.join(sorted(_CLICK_TYPES))})"
        )
    if action == "browser_wait_for" and "state" in step and step["state"] not in _BROWSER_WAIT_STATES:
        errors.append(
            f"{label} ({action}): unknown state '{step['state']}' "
            f"(must be one of: {', '.join(sorted(_BROWSER_WAIT_STATES))})"
        )
    if action in {"browser_navigate", "browser_click", "browser_fill", "browser_wait_for"} and "timeout_ms" in step:
        if not isinstance(step["timeout_ms"], int) or isinstance(step["timeout_ms"], bool) or step["timeout_ms"] <= 0:
            errors.append(f"{label} ({action}): 'timeout_ms' must be a positive integer")
    if (
        action == "copy_file"
        and "if_destination_newer" in step
        and step["if_destination_newer"] not in _COPY_NEWER_MODES
    ):
        errors.append(
            f"{label} ({action}): unknown if_destination_newer '{step['if_destination_newer']}' "
            f"(must be one of: {', '.join(sorted(_COPY_NEWER_MODES))})"
        )
    if action == "repeat" and "count" in step and not (isinstance(step["count"], int) and step["count"] > 0):
        errors.append(f"{label} ({action}): 'count' must be a positive integer")
    if action == "send_webhook" and "payload" in step and not isinstance(step["payload"], (dict, str)):
        errors.append(f"{label} ({action}): 'payload' must be a mapping or a JSON string")
    if action == "send_webhook" and step.get("on_error", "continue") not in {"continue", "stop"}:
        errors.append(f"{label} ({action}): 'on_error' must be 'continue' or 'stop'")
    if action == "if":
        condition_keys = [key for key in ("variable", "last_step") if key in step]
        if len(condition_keys) != 1:
            errors.append(f"{label} (if): exactly one of 'variable' or 'last_step' is required")
        if "last_step" in step and step["last_step"] not in {"ok", "warned"}:
            errors.append(f"{label} (if): 'last_step' must be one of: ok, warned")


def _validate_loops(steps: list[dict], filename: str, errors: list[str]) -> None:
    """Check that same-labeled loop steps are contiguous and agree on loop_count/loop_table."""
    seen_labels: set[str] = set()
    current_label: str | None = None
    current_count = None
    current_table = None
    for i, step in enumerate(steps, start=1):
        if not isinstance(step, dict):
            continue
        label = step.get("loop")
        step_label = f"{filename} step {i}"
        if label != current_label:
            if label is not None:
                if label in seen_labels:
                    errors.append(f"{step_label}: loop '{label}' steps must be contiguous")
                seen_labels.add(label)
                current_count = step.get("loop_count")
                current_table = step.get("loop_table")
            current_label = label
        elif label is not None and (step.get("loop_count") != current_count or step.get("loop_table") != current_table):
            errors.append(f"{step_label}: loop '{label}' steps must all have the same loop_count/loop_table")

        if label is not None:
            has_count = isinstance(step.get("loop_count"), int) and step["loop_count"] > 0
            has_table = isinstance(step.get("loop_table"), str) and bool(step["loop_table"])
            if has_count and has_table:
                errors.append(f"{step_label}: loop '{label}' cannot have both loop_count and loop_table")
            elif not has_count and not has_table:
                errors.append(f"{step_label}: loop '{label}' requires a positive integer loop_count or a loop_table name")


def _validate_if_blocks(steps: list[dict], filename: str, errors: list[str]) -> None:
    """Check that if/else/endif are properly nested: every if has one endif, at most one else."""
    stack: list[tuple[int, bool]] = []  # (1-indexed step number of the 'if', already-seen-an-else)
    for i, step in enumerate(steps, start=1):
        if not isinstance(step, dict):
            continue
        action = step.get("action")
        if action == "if":
            stack.append((i, False))
        elif action == "else":
            if not stack:
                errors.append(f"{filename} step {i}: 'else' without a matching 'if'")
            elif stack[-1][1]:
                errors.append(f"{filename} step {i}: duplicate 'else' for the same 'if'")
            else:
                stack[-1] = (stack[-1][0], True)
        elif action == "endif":
            if not stack:
                errors.append(f"{filename} step {i}: 'endif' without a matching 'if'")
            else:
                stack.pop()
    for if_step, _ in stack:
        errors.append(f"{filename} step {if_step}: 'if' has no matching 'endif'")


def _find_static_reference_warnings(steps: list[dict], filename: str) -> list[str]:
    """Find references that cannot be satisfied by the scenario's declared runtime inputs."""
    variable_definitions = {
        step.get("name")
        for step in steps
        if isinstance(step, dict)
        and step.get("action") in {
            "set_variable",
            "concat_variable",
            "set_year_month_variable",
            "set_year_month_day_variable",
            "set_month_start_variable",
            "set_month_end_variable",
            "get_excel_value",
        }
        and isinstance(step.get("name"), str)
    }
    table_definitions = {
        step.get("name")
        for step in steps
        if isinstance(step, dict) and step.get("action") == "load_table" and isinstance(step.get("name"), str)
    }
    warnings: list[str] = []
    for index, step in enumerate(steps, start=1):
        if not isinstance(step, dict):
            continue
        label = f"{filename} step {index}"
        table_name = step.get("loop_table")
        if isinstance(table_name, str) and table_name not in table_definitions:
            warnings.append(f"{label}: loop table not loaded in this scenario: {table_name}")
        if table_definitions:
            continue
        references: set[str] = set()
        if step.get("action") == "paste_variable" and isinstance(step.get("name"), str):
            references.add(step["name"])
        if step.get("action") == "if" and isinstance(step.get("variable"), str):
            references.add(step["variable"])
        for value in step.values():
            if isinstance(value, str):
                references.update(match.group(1).strip() for match in VARIABLE_PATTERN.finditer(value))
        for name in sorted(references - variable_definitions):
            warnings.append(f"{label} ({step.get('action', 'step')}): variable not defined in this scenario: {name}")
    return warnings


def _validate_scenario(yaml_path: str | Path, _ancestors: set[Path] | None = None) -> tuple[list[str], list[str]]:
    """Validate a scenario file and everything it reaches via call_scenario/repeat.

    Returns (errors, warnings). Errors mean the scenario cannot run as written
    (unknown action, missing/unknown parameters, malformed lists, a circular or
    missing call_scenario/repeat target) and should stop the run before anything
    executes. Warnings flag likely mistakes that won't necessarily crash the run,
    such as a referenced image file that doesn't exist.
    """
    resolved = Path(yaml_path).resolve()
    if _ancestors is None and os.environ.get("PASSOFLOW_USE_RUST_VALIDATOR") == "1":
        report = validate_with_rust(resolved)
        if report is not None:
            return report.messages("error"), report.messages("warning")
    # Tracks files on the current call chain (ancestors), not every file visited anywhere in
    # the tree, so the same sub-scenario can legitimately be called from two sibling steps
    # without being mistaken for a cycle — only an ancestor calling back into itself is one.
    is_root = _ancestors is None
    ancestors = _ancestors if _ancestors is not None else set()
    if resolved in ancestors:
        return [f"{resolved.name}: circular call_scenario reference"], []
    ancestors.add(resolved)

    try:
        steps = _load_steps(resolved)
    except Exception as e:
        ancestors.discard(resolved)
        return [f"{resolved.name}: failed to load: {e}"], []

    errors: list[str] = []
    warnings: list[str] = []
    _validate_loops(steps, resolved.name, errors)
    _validate_if_blocks(steps, resolved.name, errors)
    for i, step in enumerate(steps, start=1):
        label = f"{resolved.name} step {i}"
        _validate_step(step, label, errors, warnings)

        if isinstance(step, dict) and step.get("action") in ("call_scenario", "repeat") and "path" in step:
            nested_path = SCENARIOS_DIR / step["path"]
            if not nested_path.is_file():
                errors.append(f"{label}: {step['action']} path not found: {step['path']}")
            else:
                nested_errors, nested_warnings = _validate_scenario(nested_path, ancestors)
                errors.extend(nested_errors)
                warnings.extend(nested_warnings)

    if is_root:
        warnings.extend(_find_static_reference_warnings(steps, resolved.name))

    ancestors.discard(resolved)
    return errors, warnings


def _run_block(
    block: list[dict],
    variables: dict[str, str],
    offset: int,
    total: int,
    emit_progress: bool,
    runtime_state: dict[str, str],
) -> None:
    """Run a slice of steps once, in order. step numbers are offset + this slice's own index."""
    for k, step in enumerate(block):
        step_number = offset + k + 1
        logger.info("Step %d/%d: %s", step_number, total, step)
        if emit_progress:
            # Machine-readable marker (separate from the human log line above) so the web UI
            # can highlight the currently running step. Parsed by api_server.py's stream_run.
            print(f"@@PROGRESS@@{step_number}/{total}", flush=True)
            # Same reasoning as the marker above: a nested call_scenario/repeat run's step
            # numbers are relative to the sub-scenario, not the caller's, so showing them on
            # the HUD here would contradict the (deliberately unmoving) web UI progress bar.
            step_display = step.get("title") or step.get("note") or step["action"]
            overlay.set_step_label(f"Step {step_number}/{total}: {step_display}")
        before_screenshot = _capture_step_screenshot() if RUN_ID else None
        warning_handler = _StepWarningHandler()
        logging.getLogger().addHandler(warning_handler)
        try:
            if step["action"] in {"call_scenario", "repeat"}:
                ACTIONS[step["action"]](step, variables, runtime_state)
            else:
                ACTIONS[step["action"]](step, variables)
        except Exception:
            runtime_state["last_step"] = "failed"
            _save_failure_context(before_screenshot, step_number, step.get("action", "step"))
            raise
        finally:
            logging.getLogger().removeHandler(warning_handler)
        if runtime_state.get("last_step") != "failed":
            runtime_state["last_step"] = "warned" if warning_handler.warned else "ok"
        if emit_progress:
            # This marker is separate from the running-step marker so the UI can retain the
            # last action that actually completed after a stop or fatal failure.
            print(f"@@COMPLETED@@{step_number}/{total}", flush=True)
        time.sleep(STEP_DELAY)


def _run_steps(
    steps: list[dict],
    variables: dict[str, str],
    offset: int = 0,
    total: int | None = None,
    emit_progress: bool = True,
    runtime_state: dict[str, str] | None = None,
) -> None:
    total = len(steps) if total is None else total
    runtime_state = runtime_state if runtime_state is not None else {"last_step": "none"}
    i = 0
    while i < len(steps):
        step = steps[i]

        if isinstance(step, dict) and step.get("action") == "if":
            else_idx, endif_idx = _find_if_boundaries(steps, i)
            true_end = else_idx if else_idx is not None else endif_idx
            if _evaluate_condition(step, variables, runtime_state):
                logger.info("If condition true, running true branch")
                _run_steps(steps[i + 1 : true_end], variables, offset=offset + i + 1, total=total, emit_progress=emit_progress, runtime_state=runtime_state)
            elif else_idx is not None:
                logger.info("If condition false, running else branch")
                _run_steps(
                    steps[else_idx + 1 : endif_idx], variables, offset=offset + else_idx + 1, total=total, emit_progress=emit_progress, runtime_state=runtime_state
                )
            else:
                logger.info("If condition false, no else branch")
            i = endif_idx + 1
            continue

        label = step.get("loop") if isinstance(step, dict) else None
        if label is None:
            _run_block([step], variables, offset + i, total, emit_progress, runtime_state)
            i += 1
            continue

        # A loop block is the contiguous run of steps sharing the same `loop` label — set by the
        # web UI's node-selection loop feature, like `group` but with real execution semantics.
        j = i
        while j < len(steps) and steps[j].get("loop") == label:
            j += 1
        block = steps[i:j]
        table_name = block[0].get("loop_table")
        if table_name:
            rows = _LOADED_TABLES.get(table_name)
            if rows is None:
                # Loud on purpose (see _run_load_table): a missing/never-loaded table would
                # otherwise silently run 0 iterations, indistinguishable from a successful run.
                logger.warning(
                    "Loop %s: table %r was never loaded (missing/failed load_table step before this loop?) — running 0 iterations",
                    label,
                    table_name,
                )
                rows = []
            row_keys = {key for row in rows for key in row}
            saved_values = {key: variables[key] for key in row_keys if key in variables}
            for iteration, row in enumerate(rows):
                logger.info("Loop %s: iteration %d/%d (table %s, row %s)", label, iteration + 1, len(rows), table_name, row)
                variables.update(row)
                _run_block(block, variables, offset + i, total, emit_progress, runtime_state)
            for key in row_keys:
                if key in saved_values:
                    variables[key] = saved_values[key]
                else:
                    variables.pop(key, None)
        else:
            count = int(block[0].get("loop_count", 1))
            for iteration in range(count):
                logger.info("Loop %s: iteration %d/%d", label, iteration + 1, count)
                _run_block(block, variables, offset + i, total, emit_progress, runtime_state)
        i = j


def run_scenario(yaml_path: str | Path, start: int | None = None, end: int | None = None, run_id: str | None = None) -> None:
    """Load a YAML scenario file and run its steps in order.

    start/end are 1-indexed and inclusive, letting the caller re-run a slice of
    the scenario (e.g. to debug one failing step) without editing the file.
    """
    global RUN_ID
    RUN_ID = run_id
    errors, warnings = _validate_scenario(yaml_path)
    for warning in warnings:
        logger.warning("Scenario validation warning: %s", warning)
    if errors:
        message = "Scenario validation failed:\n" + "\n".join(f"- {error}" for error in errors)
        logger.error(message)
        raise ValueError(message)

    all_steps = _load_steps(yaml_path)
    if os.environ.get("PASSOFLOW_USE_RUST_VALIDATOR") == "1":
        plan = plan_with_rust(yaml_path)
        if plan is not None:
            assert_plan_matches_steps(plan, all_steps)
            all_steps = steps_from_rust_plan(plan)
    start_idx = (start - 1) if start else 0
    end_idx = end if end else len(all_steps)
    steps = all_steps[start_idx:end_idx]
    if start or end:
        logger.info("Partial run: steps %d-%d of %d", start_idx + 1, end_idx, len(all_steps))
        for local_idx, step in enumerate(steps):
            if isinstance(step, dict) and step.get("action") == "if":
                try:
                    _find_if_boundaries(steps, local_idx)
                except ValueError as e:
                    raise ValueError(
                        f"'if' at step {start_idx + local_idx + 1} has no matching 'endif' within "
                        f"the selected step range ({start_idx + 1}-{end_idx}) — its 'endif' is likely "
                        "outside the --start/--end range"
                    ) from e
    variables: dict[str, str] = {}
    try:
        if os.environ.get("PASSOFLOW_USE_RUST_ENGINE") == "1":
            if start or end:
                raise ValueError("PASSOFLOW_USE_RUST_ENGINE does not support partial runs; unset it for --start/--end")
            _run_with_rust_engine(yaml_path, all_steps, run_id)
        else:
            _run_steps(steps, variables, offset=start_idx, total=len(all_steps))
    finally:
        overlay.clear()
        close_browser()
        RUN_ID = None


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Run a YAML scenario file.")
    parser.add_argument("yaml_path", help="Path to the scenario YAML file")
    parser.add_argument("--start", type=int, default=None, help="1-indexed first step to run (inclusive)")
    parser.add_argument("--end", type=int, default=None, help="1-indexed last step to run (inclusive)")
    parser.add_argument("--run-id", default=None, help="Internal run id used to name failure artifacts")
    args = parser.parse_args()

    setup_logging()
    run_scenario(args.yaml_path, start=args.start, end=args.end, run_id=args.run_id)
