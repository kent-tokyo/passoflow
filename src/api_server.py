"""FastAPI backend for the scenario editor Web UI.

Serves the action schema, reads/writes YAML scenario files under scenarios/,
and runs scenarios as a subprocess while streaming their log output over a
WebSocket. Run with: python src/api_server.py
"""

import asyncio
import io
import json
import logging
import os
import re
import sys
import threading
import uuid
import zipfile
from pathlib import Path, PurePosixPath
from typing import Any
from urllib.parse import quote

import anthropic
import markdown
import pyautogui
import uvicorn
import yaml
from anthropic import Anthropic
from dotenv import load_dotenv
from fastapi import FastAPI, File, HTTPException, Response, UploadFile, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import FileResponse, HTMLResponse
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel

from app_paths import app_root
from input_actions import read_table_rows
from run_scenario import _validate_scenario, action_outcome_contract
from rust_validator import compare_with_python, validate_with_rust
from screen_actions import POSITIONS, REGION_ORIGINS
from web_actions import dom_browser_setup_status, preview_dom_selector

load_dotenv(app_root() / ".env")  # see .env.example

logger = logging.getLogger(__name__)

SCENARIOS_DIR = app_root() / "scenarios"
RUN_SCENARIO_SCRIPT = Path(__file__).resolve().parent / "run_scenario.py"  # dev (non-frozen) only
VERSION_FILE = app_root() / "VERSION"
WEB_UI_DIST = app_root() / "web-ui" / "dist"
DOCS_DIR = app_root() / "docs"

# Model is app-level config (not an SDK-recognized env var like ANTHROPIC_API_KEY),
# so it's read directly here rather than relying on SDK auto-detection.
ANTHROPIC_MODEL = os.environ.get("ANTHROPIC_MODEL", "claude-opus-5")
CHAT_MAX_TOKENS = 4096
CHAT_SYSTEM_PROMPT = (
    "You are a helpful assistant embedded in passoflow, an RPA scenario editor. "
    "passoflow scenarios are YAML files describing steps (image search, mouse clicks, "
    "keyboard input, variables) that a Python runner executes to automate desktop apps. "
    "Help the user understand actions, debug scenarios, and write YAML. Be concise. "
    "When the request explicitly asks to add, create, or automate steps and a selected "
    "step is provided, call propose_flow_actions with the smallest complete sequence of "
    "ordinary executable actions. For a request to paste a variable into the active input, "
    "use set_clipboard with text like {{variable_name}}, followed by paste; use "
    "paste_variable only when no clipboard step is requested. Do not propose start, end, "
    "if, else, or endif as inserted actions."
)
ACTION_SUGGEST_SYSTEM_PROMPT = (
    "You are an assistant embedded in passoflow's scenario editor, helping configure a single step. "
    "You are given the full catalog of available actions and their fields, the step's current "
    "action and parameters, and an optional instruction from the user. Propose the single best "
    "action for this step (usually the same one, but switch to a clearly more suitable action from "
    "the catalog if one fits better) and a complete params object for it, using only field names "
    "listed for the chosen action in the catalog (the catalog lists every field the runner actually "
    "accepts, including ones not shown as a dedicated field in this editor, e.g. retry/position on "
    "image-search actions — do not drop an existing valid field just because it seems uncommon). "
    "Keep every existing field whose value is still applicable; only drop or change a field if it's "
    "invalid for the chosen action, contradicts the instruction, or the instruction asks to remove "
    "it. Preserve the 'note' and 'title' meta fields from the current params if present, unless the "
    "instruction asks to change them. Never invent or guess image "
    "file paths for 'image'/'images' fields (kind=image) — only the user can supply screenshots, so "
    "keep any existing ones as-is unless switching to an action that doesn't need one; the same "
    "goes for 'path' fields referencing another scenario file (kind=scenario). Keep string values "
    "(notes, variable names) in whatever language they're already in. Reply only by calling "
    "propose_action, with a one or two sentence explanation in the same language as the instruction "
    "or the existing notes."
)

RUN_FEEDBACK_SYSTEM_PROMPT = (
    "You are an assistant embedded in passoflow, an RPA scenario editor. A scenario run just "
    "finished and produced one or more WARNING-level log lines — each means something didn't "
    "go as expected but the run continued anyway (e.g. an image wasn't found on screen, no "
    "window matched, a window couldn't be brought to the foreground). Analyze the warnings "
    "given below and suggest concrete, specific improvements to the scenario that would prevent "
    "them next time (e.g. raise retry/retry_interval_ms on a specific step, add a wait beforehand, "
    "recheck a candidate image, broaden/narrow a title_contains match). Be concise — a short list "
    "of points, not an essay. Reply in Japanese, unless the warning text itself is clearly in a "
    "different language."
)
RUN_FEEDBACK_MAX_TOKENS = 700

RUN_FAILURE_ANALYSIS_SYSTEM_PROMPT = (
    "You are an assistant embedded in passoflow, an RPA scenario editor. A scenario run just ended "
    "with a non-zero exit code, shown below as its tail of log output (this may be because a step "
    "raised an error, or because the user pressed Stop partway through). Analyze the log tail and "
    "explain the likely cause: what the last step being attempted was, whether it looks like a real "
    "failure (a Python traceback, repeated retries on the same step, no progress for a while) versus "
    "a clean stop with nothing wrong, and suggest concrete next steps or scenario fixes if a real "
    "problem is evident (e.g. raise retry/retry_interval_ms on a specific step, add a wait beforehand, "
    "recheck a candidate image, broaden/narrow a title_contains match). Be concise — a short list of "
    "points, not an essay. Reply in Japanese, unless the log text itself is clearly in a different "
    "language."
)
RUN_FAILURE_ANALYSIS_MAX_LINES = 50

FOLDER_PATTERN = re.compile(r"^[\w\-]+(/[\w\-]+)*$")
PROGRESS_PATTERN = re.compile(r"^@@PROGRESS@@(\d+)/(\d+)$")
COMPLETED_PATTERN = re.compile(r"^@@COMPLETED@@(\d+)/(\d+)$")
RESERVED_ROOT_FOLDER = "images"  # scenario images live under scenarios/images/, so it can't also be a user folder

app = FastAPI(title="passoflow scenario editor API")

app.add_middleware(
    CORSMiddleware,
    # Vite may choose another port when the default is occupied (for example 5177 during
    # local preview). Keep the API local-only while allowing those development origins.
    allow_origin_regex=r"^https?://(localhost|127\.0\.0\.1)(:\d+)?$",
    allow_methods=["*"],
    allow_headers=["*"],
)


class Scenario(BaseModel):
    title: str = ""
    steps: list[dict[str, Any]]


class ScenarioCreate(Scenario):
    filename: str


class ScenarioSummary(BaseModel):
    filename: str
    title: str


class FolderCreate(BaseModel):
    path: str


class MoveRequest(BaseModel):
    destination: str


class ChatMessage(BaseModel):
    role: str
    content: str


class ChatRequest(BaseModel):
    messages: list[ChatMessage]
    selected_action: str | None = None
    selected_params: dict[str, Any] | None = None


class ChatResponse(BaseModel):
    reply: str
    actions: list[dict[str, Any]] = []


class ActionSuggestRequest(BaseModel):
    action: str
    params: dict[str, Any]
    instruction: str = ""


class ActionSuggestResponse(BaseModel):
    action: str
    params: dict[str, Any]
    explanation: str


class TableImportRequest(BaseModel):
    path: str
    sheet: str | None = None


class TableImportResponse(BaseModel):
    columns: list[str]
    rows: list[dict[str, str]]


class DomPreviewRequest(BaseModel):
    url: str
    selector: str
    timeout_ms: int = 10_000


# Valid key names for press_key/hotkey, sourced from pyautogui itself so the UI's
# picker can never drift out of sync with what run_scenario.py actually accepts.
KEY_OPTIONS = list(pyautogui.KEYBOARD_KEYS)

# Named anchor points on a matched image, sourced from screen_actions so this can't drift
# out of sync with what run_scenario.py actually accepts for the "position" option.
POSITION_OPTIONS = list(POSITIONS)

# Hand-authored schema describing each action's parameters, since ACTIONS in
# run_scenario.py maps names to functions that read from a raw dict rather
# than a typed signature. Drives the palette and parameter panel in the UI.
#
# "note" and "group" are reserved on every step for the web UI (free-text memo
# and visual grouping label, see ParameterPanel.tsx / ScenarioEditor.tsx) and are
# never read by run_scenario.py — don't reuse them as action-specific field names.
ACTION_SCHEMA = [
    {"action": "start", "category": "flow", "fields": []},
    {"action": "end", "category": "flow", "fields": []},
    {"action": "wait", "category": "control", "fields": [{"name": "ms", "type": "number", "required": True}]},
    {
        "action": "call_scenario",
        "category": "control",
        "fields": [{"name": "path", "type": "string", "required": True, "kind": "scenario"}],
    },
    {
        "action": "repeat",
        "category": "control",
        "fields": [
            {"name": "path", "type": "string", "required": True, "kind": "scenario"},
            {"name": "count", "type": "number", "required": True, "default": 1},
        ],
    },
    {
        "action": "if",
        "category": "control",
        "fields": [
            {"name": "variable", "type": "string", "required": False, "kind": "variable"},
            {"name": "equals", "type": "string", "required": False},
            {
                "name": "last_step",
                "type": "string",
                "required": False,
                "kind": "select",
                "options": ["ok", "warned"],
            },
        ],
    },
    {"action": "else", "category": "control", "fields": []},
    {"action": "endif", "category": "control", "fields": []},
    {
        "action": "activate_window",
        "category": "app",
        "fields": [
            {"name": "title_contains", "type": "string", "required": True},
            {"name": "retry", "type": "number", "required": False, "default": 0},
            {"name": "retry_interval_ms", "type": "number", "required": False, "default": 500},
        ],
    },
    {
        "action": "open_url",
        "category": "app",
        "fields": [{"name": "url", "type": "string", "required": True}],
    },
    {
        "action": "browser_navigate",
        "category": "app",
        "fields": [
            {"name": "url", "type": "string", "required": True},
            {"name": "timeout_ms", "type": "number", "required": False, "default": 30000},
        ],
    },
    {
        "action": "browser_click",
        "category": "app",
        "fields": [
            {"name": "selector", "type": "string", "required": True},
            {"name": "timeout_ms", "type": "number", "required": False, "default": 10000},
        ],
    },
    {
        "action": "browser_fill",
        "category": "app",
        "fields": [
            {"name": "selector", "type": "string", "required": True},
            {"name": "text", "type": "string", "required": True},
            {"name": "timeout_ms", "type": "number", "required": False, "default": 10000},
        ],
    },
    {
        "action": "browser_wait_for",
        "category": "app",
        "fields": [
            {"name": "selector", "type": "string", "required": True},
            {"name": "state", "type": "string", "required": False, "default": "visible", "kind": "select", "options": ["attached", "detached", "hidden", "visible"]},
            {"name": "timeout_ms", "type": "number", "required": False, "default": 10000},
        ],
    },
    {
        "action": "launch_app",
        "category": "app",
        "fields": [
            {"name": "path", "type": "string", "required": True},
            {"name": "args", "type": "string[]", "required": False},
            {"name": "wait_for_window", "type": "string", "required": False},
            {"name": "startup_timeout_ms", "type": "number", "required": False, "default": 10000},
        ],
    },
    {
        "action": "rename_file",
        "category": "file",
        "fields": [
            {"name": "path", "type": "string", "required": True},
            {"name": "new_name", "type": "string", "required": True},
        ],
    },
    {
        "action": "move_file",
        "category": "file",
        "fields": [
            {"name": "path", "type": "string", "required": True},
            {"name": "destination", "type": "string", "required": True},
        ],
    },
    {
        "action": "copy_file",
        "category": "file",
        "fields": [
            {"name": "path", "type": "string", "required": True},
            {"name": "destination", "type": "string", "required": True},
            {
                "name": "if_destination_newer",
                "type": "string",
                "required": False,
                "default": "overwrite",
                "kind": "select",
                "options": ["overwrite", "skip"],
            },
        ],
    },
    {
        "action": "map_network_drive",
        "category": "app",
        "fields": [
            {"name": "drive", "type": "string", "required": True},
            {"name": "path", "type": "string", "required": True},
        ],
    },
    {
        "action": "open_excel_file",
        "category": "excel",
        "fields": [{"name": "path", "type": "string", "required": True}],
    },
    {"action": "open_new_excel", "category": "excel", "fields": []},
    {
        "action": "get_excel_value",
        "category": "excel",
        "fields": [
            {"name": "path", "type": "string", "required": True},
            {"name": "sheet", "type": "string", "required": False},
            {"name": "cell", "type": "string", "required": True},
            {"name": "name", "type": "string", "required": True},
        ],
    },
    {
        "action": "set_excel_value",
        "category": "excel",
        "fields": [
            {"name": "path", "type": "string", "required": True},
            {"name": "sheet", "type": "string", "required": False},
            {"name": "cell", "type": "string", "required": True},
            {"name": "value", "type": "string", "required": True},
        ],
    },
    {
        "action": "save_excel_file",
        "category": "excel",
        "fields": [{"name": "path", "type": "string", "required": True}],
    },
    {
        "action": "create_excel_sheet",
        "category": "excel",
        "fields": [
            {"name": "path", "type": "string", "required": True},
            {"name": "sheet", "type": "string", "required": True},
        ],
    },
    {
        "action": "delete_excel_sheet",
        "category": "excel",
        "fields": [
            {"name": "path", "type": "string", "required": True},
            {"name": "sheet", "type": "string", "required": True},
        ],
    },
    {
        "action": "delete_excel_row",
        "category": "excel",
        "fields": [
            {"name": "path", "type": "string", "required": True},
            {"name": "sheet", "type": "string", "required": False},
            {"name": "row", "type": "number", "required": True},
        ],
    },
    {
        "action": "sort_excel_range",
        "category": "excel",
        "fields": [
            {"name": "path", "type": "string", "required": True},
            {"name": "sheet", "type": "string", "required": False},
            {"name": "range", "type": "string", "required": True},
            {"name": "key_cell", "type": "string", "required": True},
            {
                "name": "order",
                "type": "string",
                "required": False,
                "default": "asc",
                "kind": "select",
                "options": ["asc", "desc"],
            },
        ],
    },
    {
        "action": "run_excel_macro",
        "category": "excel",
        "fields": [
            {"name": "path", "type": "string", "required": True},
            {"name": "macro", "type": "string", "required": True},
            {"name": "args", "type": "string[]", "required": False},
        ],
    },
    {
        "action": "load_table",
        "category": "variable",
        # "columns" and "selected_rows" are populated by the import panel and are retained on
        # the node so the preview metadata is available when the scenario runs. They are not
        # exposed as ordinary parameter fields in this schema.
        "fields": [
            {"name": "path", "type": "string", "required": True},
            {"name": "sheet", "type": "string", "required": False},
            {"name": "name", "type": "string", "required": True},
        ],
    },
    {
        "action": "send_webhook",
        "category": "app",
        "fields": [
            {"name": "url", "type": "string", "required": True},
            {
                "name": "method",
                "type": "string",
                "required": False,
                "default": "POST",
                "kind": "select",
                "options": ["GET", "POST", "PUT", "PATCH", "DELETE"],
            },
            {"name": "payload", "type": "string", "required": False},
            {"name": "on_error", "type": "string", "required": False, "default": "continue", "kind": "select", "options": ["continue", "stop"]},
        ],
    },
    {
        "action": "click_image",
        "category": "screen",
        "fields": [
            {"name": "images", "type": "string[]", "required": True, "kind": "image"},
            {
                "name": "click_type",
                "type": "string",
                "required": False,
                "default": "single",
                "kind": "select",
                "options": ["single", "double"],
            },
            {"name": "confidence", "type": "number", "required": False, "default": 0.8},
            {"name": "offset", "type": "number[]", "required": False},
            {"name": "region", "type": "number[]", "required": False},
            {"name": "region_origin", "type": "string", "required": False, "default": "screen", "kind": "select", "options": sorted(REGION_ORIGINS)},
            {"name": "target_window_title", "type": "string", "required": False},
            {
                "name": "position",
                "type": "string",
                "required": False,
                "default": "center",
                "kind": "select",
                "options": POSITION_OPTIONS,
            },
            {"name": "retry", "type": "number", "required": False, "default": 0},
            {"name": "retry_interval_ms", "type": "number", "required": False, "default": 500},
            {"name": "click_indicator_duration", "type": "number", "required": False, "default": 0.25},
        ],
    },
    {
        "action": "move_mouse_to_image",
        "category": "screen",
        "fields": [
            {"name": "images", "type": "string[]", "required": True, "kind": "image"},
            {"name": "confidence", "type": "number", "required": False, "default": 0.8},
            {"name": "offset", "type": "number[]", "required": False},
            {"name": "region", "type": "number[]", "required": False},
            {"name": "region_origin", "type": "string", "required": False, "default": "screen", "kind": "select", "options": sorted(REGION_ORIGINS)},
            {"name": "target_window_title", "type": "string", "required": False},
            {
                "name": "position",
                "type": "string",
                "required": False,
                "default": "center",
                "kind": "select",
                "options": POSITION_OPTIONS,
            },
            {"name": "retry", "type": "number", "required": False, "default": 0},
            {"name": "retry_interval_ms", "type": "number", "required": False, "default": 500},
        ],
    },
    {
        "action": "set_variable",
        "category": "variable",
        "fields": [
            {"name": "name", "type": "string", "required": True},
            {"name": "value", "type": "string", "required": True},
        ],
    },
    {
        "action": "concat_variable",
        "category": "variable",
        "fields": [
            {"name": "name", "type": "string", "required": True},
            {"name": "value", "type": "string", "required": True},
        ],
    },
    {
        "action": "set_year_month_variable",
        "category": "variable",
        "fields": [
            {"name": "name", "type": "string", "required": True},
            {"name": "days_offset", "type": "number", "required": False, "default": 0},
            {"name": "months_offset", "type": "number", "required": False, "default": 0},
        ],
    },
    {
        "action": "set_year_month_day_variable",
        "category": "variable",
        "fields": [
            {"name": "name", "type": "string", "required": True},
            {"name": "days_offset", "type": "number", "required": False, "default": 0},
            {"name": "months_offset", "type": "number", "required": False, "default": 0},
        ],
    },
    {
        "action": "set_month_start_variable",
        "category": "variable",
        "fields": [
            {"name": "name", "type": "string", "required": True},
            {"name": "days_offset", "type": "number", "required": False, "default": 0},
            {"name": "months_offset", "type": "number", "required": False, "default": 0},
        ],
    },
    {
        "action": "set_month_end_variable",
        "category": "variable",
        "fields": [
            {"name": "name", "type": "string", "required": True},
            {"name": "days_offset", "type": "number", "required": False, "default": 0},
            {"name": "months_offset", "type": "number", "required": False, "default": 0},
        ],
    },
    {"action": "type_text", "category": "input", "fields": [{"name": "text", "type": "string", "required": True}]},
    {
        "action": "set_clipboard",
        "category": "input",
        "fields": [{"name": "text", "type": "string", "required": True}],
    },
    {"action": "paste", "category": "input", "fields": []},
    {
        "action": "paste_variable",
        "category": "input",
        "fields": [{"name": "name", "type": "string", "required": True, "kind": "variable"}],
    },
    {"action": "clear_input", "category": "input", "fields": []},
    {
        "action": "press_key",
        "category": "input",
        "fields": [
            {"name": "key", "type": "string", "required": True, "kind": "key", "options": KEY_OPTIONS},
            {"name": "wait", "type": "number", "required": False, "default": 100},
        ],
    },
    {
        "action": "hotkey",
        "category": "input",
        "fields": [{"name": "keys", "type": "string[]", "required": True, "kind": "key", "options": KEY_OPTIONS}],
    },
]

# start/end are visual flow markers, not something the AI should suggest switching to/from.
_SUGGESTABLE_ACTIONS = {schema["action"] for schema in ACTION_SCHEMA if schema["action"] not in ("start", "end")}
_FLOW_INSERTABLE_ACTIONS = _SUGGESTABLE_ACTIONS - {"if", "else", "endif"}

def _field_desc(field: dict) -> str:
    name = field["name"] + ("*" if field.get("required") else "")
    if field.get("kind") == "key":
        return f"{name} (must be a valid pyautogui key name, e.g. enter, tab, esc, ctrl, shift, alt, a-z, 0-9, f1-f24)"
    if field.get("options"):
        return f"{name} (must be one of: {', '.join(field['options'])})"
    return name


def _action_catalog_text() -> str:
    """Render ACTION_SCHEMA as plain text for the AI-suggestion prompt, one action per line."""
    lines = []
    for schema in ACTION_SCHEMA:
        if schema["action"] not in _SUGGESTABLE_ACTIONS:
            continue
        parts = [_field_desc(f) for f in schema["fields"]]
        lines.append(f"- {schema['action']} [{schema['category']}]: {', '.join(parts) or '(no parameters)'}")
    return "\n".join(lines)


# Human-friendly display names per action, in each UI language. Keyed separately from
# ACTION_SCHEMA so the schema stays readable; merged into the /api/actions response below.
ACTION_LABELS: dict[str, dict[str, str]] = {
    "start": {"en": "Start", "ja": "開始", "zh": "开始"},
    "end": {"en": "End", "ja": "終了", "zh": "结束"},
    "wait": {"en": "Wait", "ja": "待機", "zh": "等待"},
    "call_scenario": {"en": "Call Scenario", "ja": "シナリオ呼び出し", "zh": "调用场景"},
    "repeat": {"en": "Repeat", "ja": "繰り返し", "zh": "重复"},
    "if": {"en": "If", "ja": "もし〜なら", "zh": "如果"},
    "else": {"en": "Else", "ja": "そうでなければ", "zh": "否则"},
    "endif": {"en": "End If", "ja": "もし〜終わり", "zh": "结束如果"},
    "activate_window": {"en": "Activate Window", "ja": "ウィンドウをアクティブ化", "zh": "激活窗口"},
    "open_url": {"en": "Open URL (Visual)", "ja": "URLを開く（画面操作）", "zh": "打开URL（视觉操作）"},
    "browser_navigate": {"en": "Browser Navigate (DOM)", "ja": "ブラウザ遷移（DOM）", "zh": "浏览器导航（DOM）"},
    "browser_click": {"en": "Browser Click (DOM)", "ja": "ブラウザクリック（DOM）", "zh": "浏览器点击（DOM）"},
    "browser_fill": {"en": "Browser Fill (DOM)", "ja": "ブラウザ入力（DOM）", "zh": "浏览器填写（DOM）"},
    "browser_wait_for": {"en": "Wait for Browser Element (DOM)", "ja": "ブラウザ要素を待つ（DOM）", "zh": "等待浏览器元素（DOM）"},
    "launch_app": {"en": "Launch App", "ja": "アプリを起動", "zh": "启动应用"},
    "rename_file": {"en": "Rename File", "ja": "ファイルの名前を変更", "zh": "重命名文件"},
    "move_file": {"en": "Move File", "ja": "ファイルを移動", "zh": "移动文件"},
    "copy_file": {"en": "Copy File", "ja": "ファイルをコピー", "zh": "复制文件"},
    "map_network_drive": {"en": "Map Network Drive", "ja": "ネットワークドライブを設定", "zh": "映射网络驱动器"},
    "open_excel_file": {"en": "Open Excel File", "ja": "Excelファイルを開く", "zh": "打开Excel文件"},
    "open_new_excel": {"en": "Open New Excel", "ja": "新規でExcelを開く", "zh": "新建Excel"},
    "get_excel_value": {"en": "Get Excel Value", "ja": "Excel 値の取得", "zh": "获取Excel值"},
    "set_excel_value": {"en": "Set Excel Value", "ja": "Excel 値を設定", "zh": "设置Excel值"},
    "save_excel_file": {"en": "Save Excel File", "ja": "Excelファイルを保存", "zh": "保存Excel文件"},
    "create_excel_sheet": {"en": "Create Excel Sheet", "ja": "Excelシートを作成", "zh": "创建Excel工作表"},
    "delete_excel_sheet": {"en": "Delete Excel Sheet", "ja": "Excelシートを削除", "zh": "删除Excel工作表"},
    "delete_excel_row": {"en": "Delete Excel Row", "ja": "Excel行を削除", "zh": "删除Excel行"},
    "sort_excel_range": {"en": "Sort Excel Range", "ja": "Excel範囲を並べ替え", "zh": "排序Excel区域"},
    "run_excel_macro": {"en": "Run Excel Macro", "ja": "Excelマクロを実行", "zh": "运行Excel宏"},
    "load_table": {"en": "Load Table", "ja": "表データを読み込み", "zh": "加载表格数据"},
    "send_webhook": {"en": "Send Webhook", "ja": "Webhook送信", "zh": "发送Webhook"},
    "click_image": {"en": "Click Image", "ja": "画像をクリック", "zh": "点击图像"},
    "move_mouse_to_image": {"en": "Move Mouse to Image", "ja": "画像にマウスを移動", "zh": "将鼠标移到图像"},
    "set_variable": {"en": "Set Variable", "ja": "変数を設定", "zh": "设置变量"},
    "concat_variable": {"en": "Concatenate Variable", "ja": "文字列変数を連結", "zh": "拼接字符串变量"},
    "set_year_month_variable": {"en": "Set Year-Month Variable (YYYYMM)", "ja": "年月(YYYYMM)を変数に設定", "zh": "设置年月变量(YYYYMM)"},
    "set_year_month_day_variable": {
        "en": "Set Date Variable (YYYY/MM/DD)",
        "ja": "年月日(YYYY/MM/DD)を変数に設定",
        "zh": "设置年月日变量(YYYY/MM/DD)",
    },
    "set_month_start_variable": {
        "en": "Set Month-Start Variable (YYYY/MM/01)",
        "ja": "月初日(YYYY/MM/01)を変数に設定",
        "zh": "设置月初变量(YYYY/MM/01)",
    },
    "set_month_end_variable": {
        "en": "Set Month-End Variable (YYYY/MM/DD)",
        "ja": "月末日(YYYY/MM/DD)を変数に設定",
        "zh": "设置月末变量(YYYY/MM/DD)",
    },
    "type_text": {"en": "Type Text", "ja": "テキストを入力", "zh": "输入文本"},
    "set_clipboard": {"en": "Set Clipboard", "ja": "クリップボードに設定", "zh": "设置剪贴板"},
    "paste": {"en": "Paste", "ja": "クリップボードから貼り付け", "zh": "粘贴"},
    "paste_variable": {"en": "Paste from Variable", "ja": "変数から貼り付け", "zh": "从变量粘贴"},
    "clear_input": {"en": "Clear Input", "ja": "入力内容を消去", "zh": "清空输入内容"},
    "press_key": {"en": "Press Key", "ja": "キーを押す", "zh": "按键"},
    "hotkey": {"en": "Hotkey", "ja": "ショートカットキー", "zh": "快捷键"},
}

# One-line purpose descriptions shown below each action in the palette. Keep these short and
# task-oriented so users can search by what they want to accomplish, not only by the action name.
ACTION_PURPOSES: dict[str, dict[str, str]] = {
    "start": {"en": "Mark the beginning of the scenario.", "ja": "シナリオの開始位置を示します。", "zh": "标记场景的开始位置。"},
    "end": {"en": "Mark the end of the scenario.", "ja": "シナリオの終了位置を示します。", "zh": "标记场景的结束位置。"},
    "wait": {"en": "Pause before the next step.", "ja": "次の処理まで一時停止します。", "zh": "在下一步之前暂停。"},
    "call_scenario": {"en": "Run another scenario once.", "ja": "別のシナリオを1回呼び出します。", "zh": "运行另一个场景一次。"},
    "repeat": {"en": "Run another scenario repeatedly.", "ja": "別のシナリオを指定回数繰り返します。", "zh": "重复运行另一个场景。"},
    "if": {"en": "Run a branch only when a condition is true.", "ja": "条件に応じて処理を分岐します。", "zh": "根据条件选择分支运行。"},
    "else": {"en": "Start the alternative branch of an if.", "ja": "条件が偽の場合の分岐を開始します。", "zh": "开始if条件为假时的分支。"},
    "endif": {"en": "End an if branch block.", "ja": "条件分岐ブロックを終了します。", "zh": "结束条件分支块。"},
    "activate_window": {"en": "Bring a matching application window to the front.", "ja": "指定したウィンドウを前面に表示します。", "zh": "将匹配的应用窗口置于前台。"},
    "open_url": {"en": "Open a URL in the default browser, then use image actions on it.", "ja": "既定ブラウザでURLを開き、その画面を画像認識操作で操作します。", "zh": "在默认浏览器中打开URL，然后使用视觉动作操作。"},
    "browser_navigate": {"en": "Navigate a Playwright-controlled browser page by URL.", "ja": "Playwright管理下のブラウザページをURLで遷移します。", "zh": "使用URL导航Playwright控制的浏览器页面。"},
    "browser_click": {"en": "Click an element with a CSS selector in the DOM browser.", "ja": "DOMブラウザでCSSセレクタの要素をクリックします。", "zh": "在DOM浏览器中点击CSS选择器指定的元素。"},
    "browser_fill": {"en": "Fill an input or textarea selected in the DOM browser.", "ja": "DOMブラウザでCSSセレクタの入力欄に文字を設定します。", "zh": "在DOM浏览器中填写CSS选择器指定的输入框或文本区。"},
    "browser_wait_for": {"en": "Wait until a DOM element reaches the selected state.", "ja": "DOM要素が指定した状態になるまで待ちます。", "zh": "等待DOM元素达到所选状态。"},
    "launch_app": {"en": "Start an application from its executable path.", "ja": "実行ファイルからアプリを起動します。", "zh": "从可执行文件路径启动应用。"},
    "rename_file": {"en": "Change a file's name.", "ja": "ファイル名を変更します。", "zh": "更改文件名。"},
    "move_file": {"en": "Move a file to another folder.", "ja": "ファイルを別のフォルダへ移動します。", "zh": "将文件移动到其他文件夹。"},
    "copy_file": {"en": "Copy a file to another location.", "ja": "ファイルを別の場所へコピーします。", "zh": "将文件复制到其他位置。"},
    "map_network_drive": {"en": "Map a network folder to a drive letter.", "ja": "ネットワークフォルダをドライブに割り当てます。", "zh": "将网络文件夹映射为驱动器。"},
    "open_excel_file": {"en": "Open an existing Excel workbook.", "ja": "既存のExcelブックを開きます。", "zh": "打开现有Excel工作簿。"},
    "open_new_excel": {"en": "Create and open a new Excel workbook.", "ja": "新しいExcelブックを作成して開きます。", "zh": "创建并打开新的Excel工作簿。"},
    "get_excel_value": {"en": "Read a cell value into a variable.", "ja": "Excelのセル値を変数に読み込みます。", "zh": "将Excel单元格值读入变量。"},
    "set_excel_value": {"en": "Write a value into an Excel cell.", "ja": "Excelのセルに値を書き込みます。", "zh": "将值写入Excel单元格。"},
    "save_excel_file": {"en": "Save changes to an Excel workbook.", "ja": "Excelブックの変更を保存します。", "zh": "保存Excel工作簿的更改。"},
    "create_excel_sheet": {"en": "Add a sheet to an Excel workbook.", "ja": "Excelブックにシートを追加します。", "zh": "向Excel工作簿添加工作表。"},
    "delete_excel_sheet": {"en": "Remove a sheet from an Excel workbook.", "ja": "Excelブックからシートを削除します。", "zh": "从Excel工作簿删除工作表。"},
    "delete_excel_row": {"en": "Delete a row from an Excel sheet.", "ja": "Excelシートの行を削除します。", "zh": "删除Excel工作表中的行。"},
    "sort_excel_range": {"en": "Sort rows in an Excel range.", "ja": "Excelの範囲内の行を並べ替えます。", "zh": "对Excel区域中的行排序。"},
    "run_excel_macro": {"en": "Run a macro in an Excel workbook.", "ja": "Excelブックのマクロを実行します。", "zh": "运行Excel工作簿中的宏。"},
    "load_table": {"en": "Import Excel or CSV rows for use in a loop.", "ja": "ExcelやCSVの表データを読み込みます。", "zh": "导入Excel或CSV行数据供循环使用。"},
    "send_webhook": {"en": "Send data to a web service.", "ja": "Webサービスへデータを送信します。", "zh": "向Web服务发送数据。"},
    "click_image": {"en": "Find an image on screen and click it.", "ja": "画面上の画像を探してクリックします。", "zh": "在屏幕上查找图像并点击。"},
    "move_mouse_to_image": {"en": "Find an image on screen and move the mouse to it.", "ja": "画面上の画像を探してマウスを移動します。", "zh": "在屏幕上查找图像并移动鼠标。"},
    "set_variable": {"en": "Store a fixed value in a variable.", "ja": "固定値を変数に保存します。", "zh": "将固定值保存到变量。"},
    "concat_variable": {"en": "Build a value by combining text and variables.", "ja": "文字や変数を連結して値を作ります。", "zh": "组合文本和变量生成值。"},
    "set_year_month_variable": {"en": "Store a year and month such as 202608.", "ja": "年月をYYYYMM形式で変数に保存します。", "zh": "将年月以YYYYMM格式保存到变量。"},
    "set_year_month_day_variable": {"en": "Store a date in YYYY/MM/DD format.", "ja": "年月日をYYYY/MM/DD形式で変数に保存します。", "zh": "将日期以YYYY/MM/DD格式保存到变量。"},
    "set_month_start_variable": {"en": "Store the first day of a month.", "ja": "月初日を変数に保存します。", "zh": "将月份第一天保存到变量。"},
    "set_month_end_variable": {"en": "Store the last day of a month.", "ja": "月末日を変数に保存します。", "zh": "将月份最后一天保存到变量。"},
    "type_text": {"en": "Type text into the active input.", "ja": "アクティブな入力欄に文字を入力します。", "zh": "在当前输入框中输入文本。"},
    "set_clipboard": {"en": "Put text on the clipboard.", "ja": "文字をクリップボードに設定します。", "zh": "将文本放入剪贴板。"},
    "paste": {"en": "Paste clipboard contents into the active input.", "ja": "クリップボードの内容を貼り付けます。", "zh": "将剪贴板内容粘贴到当前输入框。"},
    "paste_variable": {"en": "Paste a variable value into the active input.", "ja": "変数の値をアクティブな入力欄に貼り付けます。", "zh": "将变量值粘贴到当前输入框。"},
    "clear_input": {"en": "Clear the active input.", "ja": "アクティブな入力欄を消去します。", "zh": "清空当前输入框。"},
    "press_key": {"en": "Press one keyboard key.", "ja": "キーボードのキーを1つ押します。", "zh": "按下一个键盘按键。"},
    "hotkey": {"en": "Press several keys together.", "ja": "複数のキーを同時に押します。", "zh": "同时按下多个按键。"},
}

# Inline help for parameters that aren't self-explanatory from their name alone, shown as a
# tooltip in the parameter panel. Keyed "action.field"; only fields that need it are listed.
FIELD_HINTS: dict[str, dict[str, str]] = {
    "click_image.images": {
        "en": "Candidate images are tried from top to bottom. Put the most reliable or common appearance first; the run log records which candidate matched.",
        "ja": "候補画像は上から順に試します。最も確実または一般的な見た目を上に置くと、実行ログにマッチした候補番号も記録されます。",
        "zh": "候选图像按从上到下的顺序尝试。将最可靠或最常见的外观放在前面；运行日志会记录匹配的候选编号。",
    },
    "move_mouse_to_image.images": {
        "en": "Candidate images are tried from top to bottom. Put the most reliable or common appearance first; the run log records which candidate matched.",
        "ja": "候補画像は上から順に試します。最も確実または一般的な見た目を上に置くと、実行ログにマッチした候補番号も記録されます。",
        "zh": "候选图像按从上到下的顺序尝试。将最可靠或最常见的外观放在前面；运行日志会记录匹配的候选编号。",
    },
    "open_url.url": {
        "en": "URL to open in the default browser. Use this before click_image when you want visual browser automation.",
        "ja": "既定ブラウザで開くURL。画面を画像認識で操作する場合は、先にこのアクションを使う。",
        "zh": "要在默认浏览器中打开的URL。使用视觉浏览器自动化时，请先使用此动作。",
    },
    "browser_navigate.url": {
        "en": "URL for the Playwright-controlled browser. This is separate from visual/image-based browser automation.",
        "ja": "Playwright管理ブラウザで開くURL。画像認識方式のブラウザ操作とは別の方式。",
        "zh": "Playwright控制的浏览器URL。与基于视觉/图像的浏览器自动化分开。",
    },
    "browser_click.selector": {
        "en": "CSS selector for the element to click, e.g. button[type=submit] or #login.",
        "ja": "クリックする要素のCSSセレクタ。例: button[type=submit]、#login。",
        "zh": "要点击元素的CSS选择器，例如 button[type=submit] 或 #login。",
    },
    "browser_fill.selector": {
        "en": "CSS selector for an input or textarea, e.g. input[name=email].",
        "ja": "入力するinputまたはtextareaのCSSセレクタ。例: input[name=email]。",
        "zh": "输入框或文本区的CSS选择器，例如 input[name=email]。",
    },
    "browser_wait_for.selector": {
        "en": "CSS selector for the element whose DOM state should be awaited.",
        "ja": "DOM状態を待つ要素のCSSセレクタ。",
        "zh": "要等待其DOM状态的元素CSS选择器。",
    },
    "browser_wait_for.state": {
        "en": "State to wait for: visible, attached, hidden, or detached.",
        "ja": "待つ状態: visible（表示）、attached（存在）、hidden（非表示）、detached（DOMから消滅）。",
        "zh": "等待状态：visible（可见）、attached（已附加）、hidden（隐藏）或 detached（已分离）。",
    },
    "set_variable.name": {
        "en": "A name you choose for this variable (Japanese is fine). Reference its value later as {{name}}.",
        "ja": "この変数に付ける名前（自由入力、日本語可）。後で {{name}} という形で値を参照する。",
        "zh": "为该变量指定的名称（可自由输入，支持中文）。之后以 {{名称}} 的形式引用其值。",
    },
    "set_variable.value": {
        "en": "The value to store under that name. Plain text — it is not itself resolved for {{...}} placeholders.",
        "ja": "その名前に保存する値。プレーンテキストでよい。値自体は {{...}} プレースホルダの解決対象にはならない。",
        "zh": "要保存到该名称下的值。纯文本即可；该值本身不会被解析 {{...}} 占位符。",
    },
    "concat_variable.name": {
        "en": "A name you choose for the resulting variable (Japanese is fine). Reference its value later as {{name}}.",
        "ja": "結果を保存する変数に付ける名前（自由入力、日本語可）。後で {{name}} という形で値を参照する。",
        "zh": "为结果变量指定的名称（可自由输入，支持中文）。之后以 {{名称}} 的形式引用其值。",
    },
    "concat_variable.value": {
        "en": "A template resolved for {{...}} placeholders before storing — unlike set_variable's value, this one is resolved, so you can concatenate other variables with each other or with literal text, e.g. {{first_name}}{{last_name}} or {{name}}様.",
        "ja": "保存する前に {{...}} プレースホルダが解決されるテンプレート — set_variableのvalueと違いこちらは解決されるため、他の変数同士や変数と固定文字を連結できる。例: {{first_name}}{{last_name}} や {{name}}様。",
        "zh": "保存前会解析 {{...}} 占位符的模板——与set_variable的value不同，此处的值会被解析，因此可以将多个变量彼此拼接，或将变量与固定文本拼接，例如 {{first_name}}{{last_name}} 或 {{name}}様。",
    },
    "if.variable": {
        "en": "The variable name to check (as set by set_variable, without {{ }}). Steps until 'else'/'end if' run only if the condition is true.",
        "ja": "チェックする変数名（set_variableで設定したもの、{{ }}は付けない）。条件が真の場合のみ「そうでなければ」/「もし〜終わり」までのステップが実行される。",
        "zh": "要检查的变量名（由set_variable设置，不带{{ }}）。仅当条件为真时才会执行到\"否则\"/\"结束如果\"之间的步骤。",
    },
    "paste_variable.name": {
        "en": "The variable name whose value to paste (as set by set_variable or one of the set_*_variable date actions, without {{ }}) — pastes the raw value, not a template, so unlike type_text's text field it can't mix in surrounding literal text.",
        "ja": "貼り付ける値を持つ変数名（set_variableまたはset_*_variable系の日付アクションで設定したもの、{{ }}は付けない）— テンプレートではなく値そのものを貼り付けるため、type_textのtextと違い前後に固定文字を混ぜることはできない。",
        "zh": "要粘贴其值的变量名（由set_variable或set_*_variable系列日期动作设置，不带{{ }}）——粘贴的是原始值本身而非模板，因此与type_text的text不同，无法在前后混入固定文本。",
    },
    "if.equals": {
        "en": "Compared as text against the variable's value. Leave empty to just check the variable is set to something non-empty.",
        "ja": "変数の値と文字列として比較される。空欄のままにすると、変数が空でない値に設定されているかどうかだけをチェックする。",
        "zh": "作为文本与变量的值进行比较。留空则仅检查该变量是否被设置为非空值。",
    },
    "if.last_step": {
        "en": "Check the immediately previous action: ok (no warning) or warned (the action continued after a warning). Use this instead of variable.",
        "ja": "直前のアクションの結果をチェックする。ok（警告なし）またはwarned（警告後も継続）から選ぶ。variableとはどちらか一方だけを使う。",
        "zh": "检查紧前动作的结果：ok（无警告）或warned（出现警告但继续）。请与variable二选一。",
    },
    "set_year_month_variable.name": {
        "en": "A name you choose for this variable (Japanese is fine). Reference its value later as {{name}}.",
        "ja": "この変数に付ける名前（自由入力、日本語可）。後で {{name}} という形で値を参照する。",
        "zh": "为该变量指定的名称（可自由输入，支持中文）。之后以 {{名称}} 的形式引用其值。",
    },
    "set_year_month_variable.days_offset": {
        "en": "Days to add before computing the year-month (negative for the past). Rarely needed — only matters if it crosses a month boundary, e.g. -1 near the 1st of the month.",
        "ja": "年月を求める前に加算する日数（マイナスで過去）。月をまたぐ場合のみ意味を持つ（例: 月初め付近で-1を指定した場合）。通常は不要。",
        "zh": "计算年月之前加上的天数（负数表示过去）。只有跨越月份边界时才有意义（例如在月初附近指定-1）。通常不需要。",
    },
    "set_year_month_variable.months_offset": {
        "en": "Months to add/subtract, e.g. -1 for last month, 1 for next month.",
        "ja": "加算・減算する月数。例: -1で先月、1で来月。",
        "zh": "要加减的月数。例如-1表示上个月，1表示下个月。",
    },
    "set_year_month_day_variable.name": {
        "en": "A name you choose for this variable (Japanese is fine). Reference its value later as {{name}}.",
        "ja": "この変数に付ける名前（自由入力、日本語可）。後で {{name}} という形で値を参照する。",
        "zh": "为该变量指定的名称（可自由输入，支持中文）。之后以 {{名称}} 的形式引用其值。",
    },
    "set_year_month_day_variable.days_offset": {
        "en": "Days to add to today's date (negative for the past, e.g. -1 for yesterday). Applied before months_offset.",
        "ja": "今日の日付に加算する日数（マイナスで過去、例: -1で昨日）。months_offsetより先に適用される。",
        "zh": "加到今天日期上的天数（负数表示过去，例如-1表示昨天）。在months_offset之前应用。",
    },
    "set_year_month_day_variable.months_offset": {
        "en": "Months to add after days_offset (negative for the past, e.g. -1 for last month). The day is clamped to the target month's length (e.g. Jan 31 with -1 becomes Feb 28/29, not Mar 3).",
        "ja": "days_offsetの後に加算する月数（マイナスで過去、例: -1で先月）。日は対象月の末日にクランプされる（例: 1月31日に-1を指定すると3月3日ではなく2月28日/29日になる）。",
        "zh": "在days_offset之后加上的月数（负数表示过去，例如-1表示上个月）。日期会被限制在目标月份的末日内（例如1月31日指定-1会得到2月28/29日，而不是3月3日）。",
    },
    "set_month_start_variable.name": {
        "en": "A name you choose for this variable (Japanese is fine). Reference its value later as {{name}}.",
        "ja": "この変数に付ける名前（自由入力、日本語可）。後で {{name}} という形で値を参照する。",
        "zh": "为该变量指定的名称（可自由输入，支持中文）。之后以 {{名称}} 的形式引用其值。",
    },
    "set_month_start_variable.days_offset": {
        "en": "Days to add before computing the target month (negative for the past). Rarely needed — the day is always fixed to 01 regardless.",
        "ja": "対象月を求める前に加算する日数（マイナスで過去）。日は常に01に固定されるため、通常は不要。",
        "zh": "计算目标月份之前加上的天数（负数表示过去）。日期总是固定为01，通常不需要。",
    },
    "set_month_start_variable.months_offset": {
        "en": "Months to add/subtract from the target month, e.g. -1 for the 1st of last month, 1 for the 1st of next month.",
        "ja": "対象月に加算・減算する月数。例: -1で先月1日、1で来月1日。",
        "zh": "对目标月份加减的月数。例如-1表示上月1日，1表示下月1日。",
    },
    "set_month_end_variable.name": {
        "en": "A name you choose for this variable (Japanese is fine). Reference its value later as {{name}}.",
        "ja": "この変数に付ける名前（自由入力、日本語可）。後で {{name}} という形で値を参照する。",
        "zh": "为该变量指定的名称（可自由输入，支持中文）。之后以 {{名称}} 的形式引用其值。",
    },
    "set_month_end_variable.days_offset": {
        "en": "Days to add before computing the target month (negative for the past). Rarely needed — the day is always computed as that month's actual last day regardless.",
        "ja": "対象月を求める前に加算する日数（マイナスで過去）。日は常に対象月の実際の末日になるため、通常は不要。",
        "zh": "计算目标月份之前加上的天数（负数表示过去）。日期总是计算为该月实际的最后一天，通常不需要。",
    },
    "set_month_end_variable.months_offset": {
        "en": "Months to add/subtract from the target month, e.g. -1 for the last day of last month, 1 for the last day of next month. The last day (28-31) is always computed correctly via calendar.monthrange, regardless of today's day-of-month.",
        "ja": "対象月に加算・減算する月数。例: -1で先月末、1で来月末。末日（28〜31日）は今日が何日でもcalendar.monthrangeで正しく計算される。",
        "zh": "对目标月份加减的月数。例如-1表示上月末，1表示下月末。末日（28～31日）始终通过calendar.monthrange正确计算，与今天是几号无关。",
    },
    "click_image.position": {
        "en": "Named point on the matched image to click, instead of the default center — e.g. \"right\" for the middle of the right edge. Ignored if offset is set (offset always wins).",
        "ja": "クリックするマッチ画像上の位置を、デフォルトの中央の代わりに名前で指定する — 例: 右端中央にしたい場合は「right」。offsetを指定した場合はそちらが優先され、positionは無視される。",
        "zh": "指定要点击的匹配图像上的位置（用名称表示），代替默认的中央 — 例如「right」表示右边缘中点。如果设置了offset，则以offset为准，position会被忽略。",
    },
    "click_image.region": {
        "en": "Optional screen rectangle [left, top, width, height] to search. Use it around the target window or panel to speed up matching and reduce false positives.",
        "ja": "検索する画面範囲を [left, top, width, height] で指定（任意）。対象ウィンドウやパネルの周辺に絞ると、検索が速くなり誤検出も減る。",
        "zh": "可选的屏幕搜索区域 [left, top, width, height]。限制在目标窗口或面板附近可加快匹配并减少误匹配。",
    },
    "click_image.region_origin": {
        "en": "Coordinate origin: screen uses absolute screen pixels; active_window uses pixels from the current foreground window. Leave region empty to search the whole foreground window.",
        "ja": "座標の基準: screenは画面全体の絶対ピクセル、active_windowは現在の前面ウィンドウ左上からのピクセル。regionを空にすると前面ウィンドウ全体を検索する。",
        "zh": "坐标基准：screen使用屏幕绝对像素；active_window使用当前前台窗口左上角的像素。留空region可搜索整个前台窗口。",
    },
    "click_image.target_window_title": {
        "en": "Optional safety check: the current foreground window title must contain this text before searching or clicking. The action fails closed if it does not.",
        "ja": "安全確認（任意）: 画像検索・クリック前に、現在の前面ウィンドウのタイトルにこの文字列が含まれることを確認する。不一致なら安全側に停止する。",
        "zh": "可选安全检查：搜索或点击前，当前前台窗口标题必须包含此文本。不匹配时动作会安全停止。",
    },
    "move_mouse_to_image.position": {
        "en": "Named point on the matched image to move the mouse to, instead of the default center. Ignored if offset is set (offset always wins).",
        "ja": "マウスを移動させるマッチ画像上の位置を、デフォルトの中央の代わりに名前で指定する。offsetを指定した場合はそちらが優先され、positionは無視される。",
        "zh": "指定要将鼠标移动到的匹配图像上的位置（用名称表示），代替默认的中央。如果设置了offset，则以offset为准，position会被忽略。",
    },
    "move_mouse_to_image.region": {
        "en": "Optional screen rectangle [left, top, width, height] to search. Use it around the target window or panel to speed up matching and reduce false positives.",
        "ja": "検索する画面範囲を [left, top, width, height] で指定（任意）。対象ウィンドウやパネルの周辺に絞ると、検索が速くなり誤検出も減る。",
        "zh": "可选的屏幕搜索区域 [left, top, width, height]。限制在目标窗口或面板附近可加快匹配并减少误匹配。",
    },
    "move_mouse_to_image.region_origin": {
        "en": "Coordinate origin: screen uses absolute screen pixels; active_window uses pixels from the current foreground window. Leave region empty to search the whole foreground window.",
        "ja": "座標の基準: screenは画面全体の絶対ピクセル、active_windowは現在の前面ウィンドウ左上からのピクセル。regionを空にすると前面ウィンドウ全体を検索する。",
        "zh": "坐标基准：screen使用屏幕绝对像素；active_window使用当前前台窗口左上角的像素。留空region可搜索整个前台窗口。",
    },
    "move_mouse_to_image.target_window_title": {
        "en": "Optional safety check: the current foreground window title must contain this text before searching or moving the mouse. The action fails closed if it does not.",
        "ja": "安全確認（任意）: 画像検索・マウス移動前に、現在の前面ウィンドウのタイトルにこの文字列が含まれることを確認する。不一致なら安全側に停止する。",
        "zh": "可选安全检查：搜索或移动鼠标前，当前前台窗口标题必须包含此文本。不匹配时动作会安全停止。",
    },
    "click_image.retry": {
        "en": "Number of additional attempts if the image isn't found right away. 0 (default) means try once and give up.",
        "ja": "画像がすぐに見つからない場合の追加試行回数。デフォルトの0は1回試して諦める。",
        "zh": "图像未能立即找到时的额外重试次数。默认值0表示只尝试一次就放弃。",
    },
    "click_image.retry_interval_ms": {
        "en": "Milliseconds to wait between attempts. Only relevant when retry is greater than 0.",
        "ja": "試行間の待機時間（ミリ秒）。retryが1以上の場合のみ意味を持つ。",
        "zh": "重试之间的等待时间（毫秒）。仅当retry大于0时才有意义。",
    },
    "click_image.click_indicator_duration": {
        "en": "Seconds to show the red click indicator before clicking. Set 0 to disable it; keeping it enabled makes the target easier to verify in recordings.",
        "ja": "クリック前に赤いインジケータを表示する秒数。0にすると非表示にできる。表示したままにすると録画や実行確認でクリック先を確認しやすい。",
        "zh": "点击前显示红色指示器的秒数。设为0可禁用；保留显示有助于在录制或运行时确认点击目标。",
    },
    "move_mouse_to_image.retry": {
        "en": "Number of additional attempts if the image isn't found right away. 0 (default) means try once and give up.",
        "ja": "画像がすぐに見つからない場合の追加試行回数。デフォルトの0は1回試して諦める。",
        "zh": "图像未能立即找到时的额外重试次数。默认值0表示只尝试一次就放弃。",
    },
    "move_mouse_to_image.retry_interval_ms": {
        "en": "Milliseconds to wait between attempts. Only relevant when retry is greater than 0.",
        "ja": "試行間の待機時間（ミリ秒）。retryが1以上の場合のみ意味を持つ。",
        "zh": "重试之间的等待时间（毫秒）。仅当retry大于0时才有意义。",
    },
    "activate_window.retry": {
        "en": "Number of additional attempts if no matching window is found right away — useful right after launch_app, since the target window may not exist yet. 0 (default) means try once and give up.",
        "ja": "マッチするウィンドウがすぐに見つからない場合の追加試行回数 — launch_appの直後など、対象ウィンドウがまだ存在しないタイミングで有用。デフォルトの0は1回試して諦める。",
        "zh": "未能立即找到匹配窗口时的额外重试次数——在launch_app之后很有用，因为目标窗口可能还不存在。默认值0表示只尝试一次就放弃。",
    },
    "activate_window.retry_interval_ms": {
        "en": "Milliseconds to wait between attempts. Only relevant when retry is greater than 0.",
        "ja": "試行間の待機時間（ミリ秒）。retryが1以上の場合のみ意味を持つ。",
        "zh": "重试之间的等待时间（毫秒）。仅当retry大于0时才有意义。",
    },
    "launch_app.wait_for_window": {
        "en": "Optional window-title fragment to wait for after launch. When set, startup failure or timeout stops the scenario instead of silently continuing.",
        "ja": "起動後に待つウィンドウタイトルの一部（任意）。指定すると、起動失敗やタイムアウトでシナリオを停止します。",
        "zh": "启动后等待的可选窗口标题片段。设置后，启动失败或超时会停止场景，而不是静默继续。",
    },
    "launch_app.startup_timeout_ms": {
        "en": "Maximum time to wait for wait_for_window, in milliseconds. Used only when a window title is set.",
        "ja": "wait_for_windowを待つ最大時間（ミリ秒）。ウィンドウタイトルを指定した場合だけ使います。",
        "zh": "等待wait_for_window的最长时间（毫秒）。仅在设置窗口标题时使用。",
    },
    "send_webhook.on_error": {
        "en": "Choose whether a failed request only warns and continues, or stops the scenario.",
        "ja": "リクエスト失敗時に警告して継続するか、シナリオを停止するかを選びます。",
        "zh": "选择请求失败时仅警告并继续，还是停止场景。",
    },
    "get_excel_value.path": {
        "en": "Full path to the .xlsx file to read from, e.g. C:\\data\\report.xlsx. Resolved for {{...}} placeholders.",
        "ja": "読み込む.xlsxファイルのフルパス。例: C:\\data\\report.xlsx。{{...}}プレースホルダは解決される。",
        "zh": "要读取的.xlsx文件的完整路径，例如 C:\\data\\report.xlsx。会解析 {{...}} 占位符。",
    },
    "get_excel_value.sheet": {
        "en": "Sheet name to read from. Leave empty to use the workbook's active sheet.",
        "ja": "読み込むシート名。空欄にするとブックのアクティブシートを使う。",
        "zh": "要读取的工作表名称。留空则使用工作簿的活动工作表。",
    },
    "get_excel_value.cell": {
        "en": "Cell reference to read, e.g. B3.",
        "ja": "読み込むセル番地。例: B3。",
        "zh": "要读取的单元格地址，例如 B3。",
    },
    "get_excel_value.name": {
        "en": "A name you choose for the variable that receives the cell's value (Japanese is fine). Reference it later as {{name}}.",
        "ja": "セルの値を受け取る変数に付ける名前（自由入力、日本語可）。後で {{name}} という形で値を参照する。",
        "zh": "为接收该单元格值的变量指定的名称（可自由输入，支持中文）。之后以 {{名称}} 的形式引用其值。",
    },
    "set_excel_value.path": {
        "en": "Full path to the .xlsx file to write to, e.g. C:\\data\\report.xlsx. Resolved for {{...}} placeholders. The file is saved after the cell is set.",
        "ja": "書き込む.xlsxファイルのフルパス。例: C:\\data\\report.xlsx。{{...}}プレースホルダは解決される。セル設定後にファイルは保存される。",
        "zh": "要写入的.xlsx文件的完整路径，例如 C:\\data\\report.xlsx。会解析 {{...}} 占位符。设置单元格后会保存文件。",
    },
    "set_excel_value.sheet": {
        "en": "Sheet name to write to. Leave empty to use the workbook's active sheet.",
        "ja": "書き込むシート名。空欄にするとブックのアクティブシートを使う。",
        "zh": "要写入的工作表名称。留空则使用工作簿的活动工作表。",
    },
    "set_excel_value.cell": {
        "en": "Cell reference to write, e.g. B3.",
        "ja": "書き込むセル番地。例: B3。",
        "zh": "要写入的单元格地址，例如 B3。",
    },
    "set_excel_value.value": {
        "en": "Value to write into the cell, resolved for {{...}} placeholders first — e.g. {{customer_name}}. Numeric-looking text is stored as a real number so formulas elsewhere keep working.",
        "ja": "セルに書き込む値。先に {{...}} プレースホルダが解決される — 例: {{customer_name}}。数値に見える文字列は実際の数値として保存されるため、他のセルの数式が引き続き機能する。",
        "zh": "要写入单元格的值，会先解析 {{...}} 占位符——例如 {{customer_name}}。看起来像数字的文本会作为真正的数字保存，因此其他单元格中的公式仍可正常工作。",
    },
    "load_table.path": {
        "en": "Full path to the .xlsx or .csv file to load, e.g. C:\\data\\sales.xlsx. Resolved for {{...}} placeholders. The first row is treated as column names.",
        "ja": "読み込む.xlsxまたは.csvファイルのフルパス。例: C:\\data\\sales.xlsx。{{...}}プレースホルダは解決される。1行目は列名として扱われる。",
        "zh": "要加载的.xlsx或.csv文件的完整路径，例如 C:\\data\\sales.xlsx。会解析 {{...}} 占位符。第一行会被视为列名。",
    },
    "load_table.sheet": {
        "en": "Sheet name to read from (.xlsx only, ignored for .csv). Leave empty to use the workbook's active sheet.",
        "ja": "読み込むシート名（.xlsxのみ、.csvでは無視される）。空欄にするとブックのアクティブシートを使う。",
        "zh": "要读取的工作表名称（仅限.xlsx，.csv会忽略）。留空则使用工作簿的活动工作表。",
    },
    "load_table.name": {
        "en": "A name you choose for this table (Japanese is fine) — reference it later from a loop's \"table\" setting to run that loop once per row, with each column's value available as a variable of the same name.",
        "ja": "このテーブルに付ける名前（自由入力、日本語可）— 後でループの「テーブル」設定からこの名前を参照すると、1行ごとに1回ループが実行され、各列の値が同名の変数として使えるようになる。",
        "zh": "为该表指定的名称（可自由输入，支持中文）——之后可在循环的\"表\"设置中引用它，使该循环按每一行执行一次，每列的值会作为同名变量可用。",
    },
    "press_key.wait": {
        "en": "Milliseconds to wait after pressing the key, for whatever it triggered to take effect. Default 100.",
        "ja": "キーを押した後、その結果が反映されるまで待機する時間（ミリ秒）。デフォルト100。",
        "zh": "按键后等待其效果生效的时间（毫秒）。默认100。",
    },
    "click_image.confidence": {
        "en": "How closely the image must match, from 0 to 1 (default 0.8). Lower is more lenient but risks matching the wrong thing; raise it toward 0.9-0.99 when a similar-looking UI element nearby gets clicked by mistake instead of the intended one.",
        "ja": "画像がどれだけ一致していれば良いかを0〜1で指定（デフォルト0.8）。低いほど緩く一致するが誤検出しやすくなる。近くにある見た目の似た別の要素が間違ってクリックされる場合は0.9〜0.99程度まで上げる。",
        "zh": "图像匹配所需的相似度，取值0到1（默认0.8）。数值越低越宽松，但更容易误匹配；如果附近外观相似的元素被误点击，可将其调高到0.9-0.99左右。",
    },
    "move_mouse_to_image.confidence": {
        "en": "How closely the image must match, from 0 to 1 (default 0.8). Lower is more lenient but risks matching the wrong thing; raise it toward 0.9-0.99 when a similar-looking UI element nearby gets matched by mistake instead of the intended one.",
        "ja": "画像がどれだけ一致していれば良いかを0〜1で指定（デフォルト0.8）。低いほど緩く一致するが誤検出しやすくなる。近くにある見た目の似た別の要素が間違って一致してしまう場合は0.9〜0.99程度まで上げる。",
        "zh": "图像匹配所需的相似度，取值0到1（默认0.8）。数值越低越宽松，但更容易误匹配；如果附近外观相似的元素被误匹配，可将其调高到0.9-0.99左右。",
    },
    "rename_file.new_name": {
        "en": "The new file name only (not a full path) — the file stays in the same folder.",
        "ja": "新しいファイル名のみを指定する（フルパスではない）。ファイルは同じフォルダ内に留まる。",
        "zh": "仅填写新的文件名（不是完整路径）——文件仍保留在同一文件夹中。",
    },
    "move_file.destination": {
        "en": "Full path of the file after moving, including the file name, e.g. C:\\out\\report.xlsx. Parent folders are created automatically if missing.",
        "ja": "移動後のファイルのフルパス（ファイル名まで含む）。例: C:\\out\\report.xlsx。存在しない親フォルダは自動作成される。",
        "zh": "移动后文件的完整路径（包含文件名），例如 C:\\out\\report.xlsx。缺失的父文件夹会自动创建。",
    },
    "copy_file.path": {
        "en": "Source file path. May contain wildcards (*, ?, [...]) to copy multiple files at once — when it does, destination is treated as a folder instead of a full file path.",
        "ja": "コピー元のパス。ワイルドカード（*, ?, [...]）を使って複数ファイルを一括コピーできる。ワイルドカードを使う場合、コピー先はフルパスではなくフォルダとして扱われる。",
        "zh": "源文件路径。可使用通配符（*, ?, [...]）一次复制多个文件——使用通配符时，目标将被视为文件夹而非完整文件路径。",
    },
    "copy_file.destination": {
        "en": "Without wildcards in the source path: full path of the copy, including the file name, e.g. C:\\out\\report.xlsx. With wildcards: the destination folder each matched file is copied into (keeping its original name). Missing folders are created automatically.",
        "ja": "コピー元にワイルドカードを使わない場合: コピー先のフルパス（ファイル名まで含む）。例: C:\\out\\report.xlsx。ワイルドカードを使う場合: マッチした各ファイルが元のファイル名のままコピーされるコピー先フォルダ。存在しないフォルダは自動作成される。",
        "zh": "源路径不含通配符时：副本的完整路径（包含文件名），例如 C:\\out\\report.xlsx。含通配符时：每个匹配文件将以原文件名复制到的目标文件夹。缺失的文件夹会自动创建。",
    },
    "copy_file.if_destination_newer": {
        "en": "What to do when the destination file already exists and is newer than the source: \"overwrite\" (default) copies over it anyway; \"skip\" leaves the newer destination file untouched.",
        "ja": "コピー先に既にファイルがあり、それがコピー元より新しい場合の挙動: \"overwrite\"（デフォルト）は上書きする。\"skip\"は新しいコピー先ファイルをそのままにしてコピーしない。",
        "zh": "当目标文件已存在且比源文件更新时的处理方式：\"overwrite\"（默认）仍然覆盖；\"skip\" 保留较新的目标文件，不进行复制。",
    },
    "map_network_drive.drive": {
        "en": "Drive letter to map, e.g. Z:",
        "ja": "割り当てるドライブ文字。例: Z:",
        "zh": "要映射的驱动器号，例如 Z:",
    },
    "map_network_drive.path": {
        "en": "UNC path to map the drive to, e.g. \\\\server\\share. Re-running this step first disconnects any existing mapping on the same drive letter, so it's safe to run again.",
        "ja": "割り当て先のUNCパス。例: \\\\server\\share。このステップは実行前に同じドライブ文字の既存の割り当てを解除するため、再実行しても安全。",
        "zh": "要映射到的UNC路径，例如 \\\\server\\share。重新执行此步骤会先断开同一驱动器号的现有映射，因此可以安全地重复运行。",
    },
    "save_excel_file.path": {
        "en": "Full path to the .xlsx to re-save, e.g. C:\\data\\report.xlsx. Uses elixcee (no Microsoft Excel install needed) to open and save the file as-is.",
        "ja": "保存し直す.xlsxのフルパス。例: C:\\data\\report.xlsx。elixcee（Microsoft Excel不要のライブラリ）でファイルを開いてそのまま保存する。",
        "zh": "要重新保存的.xlsx文件的完整路径，例如 C:\\data\\report.xlsx。使用elixcee（无需安装Microsoft Excel）打开并按原样保存文件。",
    },
}


def _reject_reserved_path(relative_path: str) -> None:
    """Block writes into scenarios/images/, which is reserved for uploaded action images."""
    normalized = relative_path.strip().replace("\\", "/")
    if normalized == RESERVED_ROOT_FOLDER or normalized.startswith(f"{RESERVED_ROOT_FOLDER}/"):
        raise HTTPException(status_code=400, detail=f"'{RESERVED_ROOT_FOLDER}' is a reserved folder name")


def _resolve_scenario_path(filename: str) -> Path:
    if not isinstance(filename, str) or "\x00" in filename:
        raise HTTPException(status_code=400, detail="Invalid filename")

    normalized = filename.strip().replace("\\", "/")
    scenarios_dir = SCENARIOS_DIR.resolve()
    candidate = Path(normalized)
    if candidate.is_absolute():
        path = candidate.resolve()
        if not path.is_relative_to(scenarios_dir):
            raise HTTPException(status_code=400, detail="Invalid filename")
        normalized = path.relative_to(scenarios_dir).as_posix()
    else:
        if normalized == "scenarios":
            raise HTTPException(status_code=400, detail="Invalid filename")
        if normalized.casefold().startswith("scenarios/"):
            normalized = normalized[len("scenarios/") :]
        path = (scenarios_dir / normalized).resolve()

    if not path.is_relative_to(SCENARIOS_DIR.resolve()):
        raise HTTPException(status_code=400, detail="Invalid filename")
    if path.suffix.lower() not in {".yaml", ".yml"} or path.name in {"", ".", ".."}:
        raise HTTPException(status_code=400, detail="Invalid filename")
    return path


def _resolve_folder_path(folder_path: str) -> Path:
    if not FOLDER_PATTERN.match(folder_path):
        raise HTTPException(status_code=400, detail="Invalid folder path")
    _reject_reserved_path(folder_path)
    path = (SCENARIOS_DIR / folder_path).resolve()
    if not path.is_relative_to(SCENARIOS_DIR.resolve()):
        raise HTTPException(status_code=400, detail="Invalid folder path")
    return path


def _move_scenario_file(old_path: Path, new_path: Path) -> None:
    """Move a scenario YAML and its images folder together, rewriting image path references to match."""
    old_rel = old_path.relative_to(SCENARIOS_DIR).with_suffix("").as_posix()
    new_rel = new_path.relative_to(SCENARIOS_DIR).with_suffix("").as_posix()
    old_prefix = f"images/{old_rel}/"
    new_prefix = f"images/{new_rel}/"

    def _rewrite(value: Any) -> Any:
        return new_prefix + value[len(old_prefix) :] if isinstance(value, str) and value.startswith(old_prefix) else value

    with open(old_path, encoding="utf-8") as f:
        data = yaml.safe_load(f) or {}
    for step in data.get("steps", []):
        if isinstance(step.get("images"), list):
            step["images"] = [_rewrite(v) for v in step["images"]]

    new_path.parent.mkdir(parents=True, exist_ok=True)
    _write_scenario(new_path, data.get("title") or "", data.get("steps", []))
    old_path.unlink()

    old_images_dir = SCENARIOS_DIR / "images" / old_rel
    new_images_dir = SCENARIOS_DIR / "images" / new_rel
    if old_images_dir.is_dir():
        new_images_dir.parent.mkdir(parents=True, exist_ok=True)
        old_images_dir.rename(new_images_dir)


def _collect_scenario_package(filename: str, _seen: set[str] | None = None) -> tuple[set[str], set[str]]:
    """Recursively collect every scenario YAML (as a SCENARIOS_DIR-relative posix path string)
    and every image it references, following call_scenario/repeat targets, so an export is a
    fully self-contained, standalone package rather than one that silently depends on files
    left behind."""
    seen = _seen if _seen is not None else set()
    if filename in seen:
        return set(), set()
    seen.add(filename)

    path = (SCENARIOS_DIR / filename).resolve()
    if not path.is_relative_to(SCENARIOS_DIR.resolve()) or not path.is_file():
        return set(), set()

    scenario_paths = {filename}
    image_paths: set[str] = set()
    with open(path, encoding="utf-8") as f:
        data = yaml.safe_load(f) or {}
    for step in data.get("steps", []):
        if isinstance(step.get("images"), list):
            image_paths.update(v for v in step["images"] if isinstance(v, str))
        if step.get("action") in ("call_scenario", "repeat") and isinstance(step.get("path"), str):
            nested_scenarios, nested_images = _collect_scenario_package(step["path"], seen)
            scenario_paths |= nested_scenarios
            image_paths |= nested_images

    return scenario_paths, image_paths


# Only these extensions may be written by scenario import, so an uploaded zip can't be used
# to drop arbitrary files (e.g. executables) into the scenarios/ tree.
_IMPORT_YAML_EXTENSIONS = {".yaml", ".yml"}
_IMPORT_IMAGE_EXTENSIONS = {".png", ".jpg", ".jpeg", ".bmp"}


def _validate_import_entry(arcname: str) -> Path:
    """Check one zip entry's path is safe to extract, returning its resolved destination path.

    Rejects path traversal / absolute paths / backslashes outright, then enforces the same
    images/-folder convention as the rest of the app: scenario YAMLs may not live under it,
    and images may only live under it.
    """
    if "\\" in arcname or arcname.startswith("/"):
        raise HTTPException(status_code=400, detail=f"Invalid entry in zip: {arcname}")
    arc_path = PurePosixPath(arcname)
    if ".." in arc_path.parts or not arc_path.parts:
        raise HTTPException(status_code=400, detail=f"Invalid entry in zip: {arcname}")

    ext = arc_path.suffix.lower()
    under_images = arc_path.parts[0] == RESERVED_ROOT_FOLDER
    if ext in _IMPORT_YAML_EXTENSIONS and under_images:
        raise HTTPException(status_code=400, detail=f"Scenario file cannot live under '{RESERVED_ROOT_FOLDER}/': {arcname}")
    if ext in _IMPORT_IMAGE_EXTENSIONS and not under_images:
        raise HTTPException(status_code=400, detail=f"Image file must live under '{RESERVED_ROOT_FOLDER}/': {arcname}")
    if ext not in _IMPORT_YAML_EXTENSIONS and ext not in _IMPORT_IMAGE_EXTENSIONS:
        raise HTTPException(status_code=400, detail=f"Unsupported file type in zip: {arcname}")

    dest = (SCENARIOS_DIR / arc_path).resolve()
    if not dest.is_relative_to(SCENARIOS_DIR.resolve()):
        raise HTTPException(status_code=400, detail=f"Invalid entry in zip: {arcname}")
    return dest


def _resolve_image_path(rel_path: str) -> Path:
    resolved = (SCENARIOS_DIR / rel_path).resolve()
    if not resolved.is_relative_to(SCENARIOS_DIR.resolve()) or not resolved.is_file():
        raise HTTPException(status_code=404, detail="Image not found")
    return resolved


@app.get("/api/actions")
def list_actions() -> list[dict[str, Any]]:
    result = []
    for schema in ACTION_SCHEMA:
        fields = [
            {**field, "hint": hint} if (hint := FIELD_HINTS.get(f"{schema['action']}.{field['name']}")) else field
            for field in schema["fields"]
        ]
        result.append(
            {
                **schema,
                "fields": fields,
                "labels": ACTION_LABELS[schema["action"]],
                "purpose": ACTION_PURPOSES[schema["action"]],
                "outcomes": action_outcome_contract(schema["action"]),
            }
        )
    return result


@app.get("/api/environment")
def environment_status() -> dict[str, object]:
    """Report local setup needed by browser and desktop actions without sending input."""
    desktop = {"supported": os.name == "nt", "capture": "unknown", "message": ""}
    if os.name != "nt":
        desktop["message"] = "Desktop input and screen capture are supported on Windows only."
    else:
        try:
            # A one-pixel read is a non-invasive capture probe; it does not move the
            # pointer or synthesize input. A real action still performs its own checks.
            pyautogui.screenshot(region=(0, 0, 1, 1))
            desktop["capture"] = "ready"
            desktop["message"] = "Screen capture is available. Input permission is checked when an action runs."
        except Exception as exc:
            desktop["capture"] = "blocked"
            desktop["message"] = f"Screen capture is unavailable: {exc}"
    return {"dom_browser": dom_browser_setup_status(), "desktop": desktop}


@app.post("/api/dom/preview")
def dom_preview(req: DomPreviewRequest) -> dict[str, object]:
    """Preview CSS selector matches without touching the scenario's browser session."""
    try:
        return preview_dom_selector(req.url, req.selector, req.timeout_ms)
    except (ValueError, RuntimeError) as exc:
        raise HTTPException(status_code=422, detail=str(exc)) from exc
    except Exception as exc:
        logger.info("DOM selector preview failed: %s", exc)
        raise HTTPException(status_code=400, detail=f"DOM preview failed: {exc}") from exc


_anthropic_client: Anthropic | None = None


def _get_anthropic_client() -> Anthropic:
    global _anthropic_client
    if _anthropic_client is None:
        if not os.environ.get("ANTHROPIC_API_KEY"):
            raise HTTPException(status_code=500, detail="ANTHROPIC_API_KEY is not set (see .env.example)")
        _anthropic_client = Anthropic()
    return _anthropic_client


def _create_message(**kwargs: Any):
    """client.messages.create, mapping Anthropic SDK errors to HTTPExceptions. Shared by every /api endpoint that calls the model."""
    client = _get_anthropic_client()
    try:
        return client.messages.create(model=ANTHROPIC_MODEL, **kwargs)
    except anthropic.AuthenticationError as e:
        raise HTTPException(status_code=401, detail="Invalid Anthropic API key") from e
    except anthropic.RateLimitError as e:
        raise HTTPException(status_code=429, detail="Rate limited by Anthropic API, try again shortly") from e
    except anthropic.APIConnectionError as e:
        raise HTTPException(status_code=503, detail="Could not reach the Anthropic API") from e
    except anthropic.APIStatusError as e:
        raise HTTPException(status_code=e.status_code, detail=e.message) from e


@app.post("/api/chat")
def chat(req: ChatRequest) -> ChatResponse:
    messages = [{"role": m.role, "content": m.content} for m in req.messages]
    tools: list[dict[str, Any]] | None = None
    if req.selected_action:
        selected_context = (
            "The user currently has this step selected in the flow. "
            "Only call propose_flow_actions when the user's latest request explicitly asks "
            "to create/add/insert or automate steps; otherwise answer normally. "
            f"Available actions:\n{_action_catalog_text()}\n\n"
            f"Selected step:\naction: {req.selected_action}\n"
            f"params: {json.dumps(req.selected_params or {}, ensure_ascii=False)}"
        )
        messages = [{"role": "user", "content": selected_context}] + messages
        tools = [_PROPOSE_FLOW_ACTIONS_TOOL]
    kwargs: dict[str, Any] = {
        "max_tokens": CHAT_MAX_TOKENS,
        "system": CHAT_SYSTEM_PROMPT,
        "messages": messages,
    }
    if tools is not None:
        kwargs["tools"] = tools
    response = _create_message(**kwargs)
    if response.stop_reason == "refusal":
        raise HTTPException(status_code=422, detail="The assistant declined to respond to this request")

    reply = "".join(block.text for block in response.content if block.type == "text")
    tool_use = next((block for block in response.content if block.type == "tool_use" and block.name == "propose_flow_actions"), None)
    actions: list[dict[str, Any]] = []
    if tool_use is not None:
        proposed = tool_use.input.get("actions")
        if not isinstance(proposed, list) or not proposed:
            raise HTTPException(status_code=422, detail="The assistant returned no flow actions")
        for item in proposed:
            if not isinstance(item, dict) or item.get("action") not in _FLOW_INSERTABLE_ACTIONS:
                raise HTTPException(status_code=422, detail="The assistant returned an invalid flow action")
            params = item.get("params")
            if not isinstance(params, dict):
                raise HTTPException(status_code=422, detail="The assistant returned invalid flow parameters")
            actions.append({"action": item["action"], "params": params})
        if not reply:
            reply = tool_use.input.get("explanation", "")
    return ChatResponse(reply=reply, actions=actions)


_PROPOSE_FLOW_ACTIONS_TOOL = {
    "name": "propose_flow_actions",
    "description": "Create a short sequence of executable actions to insert immediately after the selected step.",
    "input_schema": {
        "type": "object",
        "properties": {
            "actions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "action": {"type": "string", "description": "Action name from the catalog"},
                        "params": {"type": "object", "description": "Parameters accepted by that action"},
                    },
                    "required": ["action", "params"],
                },
            },
            "explanation": {"type": "string", "description": "Brief explanation in the user's language"},
        },
        "required": ["actions", "explanation"],
    },
}


_PROPOSE_ACTION_TOOL = {
    "name": "propose_action",
    "description": "Propose the best action and parameters for this scenario step.",
    "input_schema": {
        "type": "object",
        "properties": {
            "action": {"type": "string", "description": "Action name, must be one of the actions in the catalog"},
            "params": {"type": "object", "description": "Complete parameter object for the chosen action"},
            "explanation": {"type": "string", "description": "One or two sentence explanation of the suggestion"},
        },
        "required": ["action", "params", "explanation"],
    },
}


@app.post("/api/actions/suggest")
def suggest_action(req: ActionSuggestRequest) -> ActionSuggestResponse:
    prompt = (
        f"Available actions:\n{_action_catalog_text()}\n\n"
        f"Current step:\naction: {req.action}\nparams: {json.dumps(req.params, ensure_ascii=False)}\n\n"
        + (f"User instruction: {req.instruction}\n\n" if req.instruction.strip() else "")
        + "Call propose_action with the best action and complete params for this step."
    )
    response = _create_message(
        max_tokens=CHAT_MAX_TOKENS,
        system=ACTION_SUGGEST_SYSTEM_PROMPT,
        tools=[_PROPOSE_ACTION_TOOL],
        tool_choice={"type": "tool", "name": "propose_action"},
        messages=[{"role": "user", "content": prompt}],
    )

    tool_use = next((block for block in response.content if block.type == "tool_use"), None)
    if tool_use is None:
        raise HTTPException(status_code=422, detail="The assistant didn't return a suggestion")

    suggested_action = tool_use.input.get("action")
    if suggested_action not in _SUGGESTABLE_ACTIONS:
        raise HTTPException(status_code=422, detail=f"Assistant suggested an unknown action: {suggested_action}")

    return ActionSuggestResponse(
        action=suggested_action,
        params=tool_use.input.get("params") or {},
        explanation=tool_use.input.get("explanation", ""),
    )


@app.post("/api/tables/import")
def import_table(req: TableImportRequest) -> TableImportResponse:
    """Read an .xlsx/.csv file's column names and rows for the editor's "取り込み" (import)
    button — a preview only, distinct from the load_table action which reads the same kind
    of file again at run time. Errors surface immediately as a 400, unlike load_table's own
    run-time behavior of logging a warning and continuing with an empty table."""
    try:
        rows = read_table_rows(req.path, req.sheet)
    except Exception as e:
        raise HTTPException(status_code=400, detail=str(e)) from e
    columns = list(rows[0].keys()) if rows else []
    return TableImportResponse(columns=columns, rows=rows)


class TablePickFileResponse(BaseModel):
    path: str | None


def _pick_table_file() -> str | None:
    """Show a native Windows "Open File" dialog and return the chosen absolute path, or None
    if the user cancelled. Runs its own throwaway Tk root (like screen_actions.py's click
    flash), since this is a one-off blocking modal, not a persistent overlay.

    Calls the Tcl `tk_getOpenFile` command directly rather than going through
    `tkinter.filedialog.askopenfilename` — empirically, that wrapper silently returns ""
    without ever showing a dialog on this deployment (verified via a real automated
    click-through: the raw Tcl call reliably shows the dialog and returns the chosen path,
    while the Python wrapper around the identical underlying call does not).
    """
    import tkinter as tk

    root = tk.Tk()
    root.withdraw()
    root.attributes("-topmost", True)
    try:
        # Same Windows focus-stealing prevention as activate_window in input_actions.py: this
        # API server is a background process with no "recent input" standing, so Windows
        # silently refuses to give the dialog the foreground — it opens behind whatever window
        # the user was actually looking at, with no error, looking like it never opened at all.
        # Tapping a key first counts as recent input and lifts that restriction.
        pyautogui.press("alt")
        path = root.tk.call(
            "tk_getOpenFile",
            "-title",
            "Excel/CSVファイルを選択",
            "-filetypes",
            [("Excel/CSV", ".xlsx .csv"), ("Excel", ".xlsx"), ("CSV", ".csv"), ("すべてのファイル", "*")],
        )
    finally:
        root.destroy()
    return str(path) if path else None


@app.post("/api/tables/pick-file")
async def pick_table_file() -> TablePickFileResponse:
    """Runs the (blocking, potentially long-lived while the user browses) native file dialog
    on a worker thread so it doesn't stall the event loop for other requests meanwhile."""
    path = await asyncio.to_thread(_pick_table_file)
    return TablePickFileResponse(path=path)


class ScenarioPickFileResponse(BaseModel):
    path: str | None


def _pick_scenario_file() -> str | None:
    """Same native-dialog approach as _pick_table_file, but rooted at SCENARIOS_DIR and
    filtered to .yaml/.yml, for the "open scenario" file dialog."""
    import tkinter as tk

    root = tk.Tk()
    root.withdraw()
    root.attributes("-topmost", True)
    try:
        pyautogui.press("alt")
        path = root.tk.call(
            "tk_getOpenFile",
            "-title",
            "シナリオファイルを選択",
            "-initialdir",
            str(SCENARIOS_DIR),
            "-filetypes",
            [("YAML", ".yaml .yml"), ("すべてのファイル", "*")],
        )
    finally:
        root.destroy()
    return str(path) if path else None


@app.post("/api/scenarios/pick-file")
async def pick_scenario_file() -> ScenarioPickFileResponse:
    """Shows a native "Open File" dialog rooted at the scenarios folder and returns the
    chosen file as a SCENARIOS_DIR-relative posix path (what the rest of the scenario API
    expects), or None if cancelled."""
    absolute = await asyncio.to_thread(_pick_scenario_file)
    if absolute is None:
        return ScenarioPickFileResponse(path=None)
    resolved = Path(absolute).resolve()
    if not resolved.is_relative_to(SCENARIOS_DIR.resolve()):
        raise HTTPException(status_code=400, detail="Please choose a file inside the scenarios folder")
    return ScenarioPickFileResponse(path=resolved.relative_to(SCENARIOS_DIR.resolve()).as_posix())


class ScenarioSaveFileRequest(BaseModel):
    default_name: str = ""


def _pick_scenario_save_file(default_name: str) -> str | None:
    """Same native-dialog approach as _pick_scenario_file, but a "Save As" dialog: the OS
    itself prompts to confirm overwriting an existing file, so the caller can just write the
    result unconditionally (see update_scenario, which upserts)."""
    import tkinter as tk

    root = tk.Tk()
    root.withdraw()
    root.attributes("-topmost", True)
    try:
        pyautogui.press("alt")
        path = root.tk.call(
            "tk_getSaveFile",
            "-title",
            "名前を付けて保存",
            "-initialdir",
            str(SCENARIOS_DIR),
            "-initialfile",
            default_name,
            "-defaultextension",
            ".yaml",
            "-filetypes",
            [("YAML", ".yaml .yml"), ("すべてのファイル", "*")],
        )
    finally:
        root.destroy()
    return str(path) if path else None


@app.post("/api/scenarios/pick-save-file")
async def pick_scenario_save_file(req: ScenarioSaveFileRequest) -> ScenarioPickFileResponse:
    """Shows a native "Save As" dialog rooted at the scenarios folder and returns the chosen
    destination as a SCENARIOS_DIR-relative posix path, or None if cancelled."""
    absolute = await asyncio.to_thread(_pick_scenario_save_file, req.default_name)
    if absolute is None:
        return ScenarioPickFileResponse(path=None)
    resolved = Path(absolute).resolve()
    if not resolved.is_relative_to(SCENARIOS_DIR.resolve()):
        raise HTTPException(status_code=400, detail="Please choose a location inside the scenarios folder")
    return ScenarioPickFileResponse(path=resolved.relative_to(SCENARIOS_DIR.resolve()).as_posix())


@app.get("/api/scenario-images/{rel_path:path}")
def get_scenario_image(rel_path: str) -> FileResponse:
    return FileResponse(_resolve_image_path(rel_path))


@app.get("/api/screenshot")
def get_screenshot() -> Response:
    """Captures the current screen, in the same coordinate space click_image's own
    pyautogui.locateOnScreen matching uses, so a crop taken here lines up at run time."""
    img = pyautogui.screenshot()
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    return Response(content=buf.getvalue(), media_type="image/png")


IMAGE_EXTENSIONS = {".png", ".jpg", ".jpeg", ".bmp"}


class ImageUploadResponse(BaseModel):
    path: str


@app.post("/api/scenarios/{filename:path}/images")
async def upload_scenario_image(filename: str, file: UploadFile = File(...)) -> ImageUploadResponse:
    """Save an uploaded image under scenarios/images/<scenario's relative path>/ and return its relative path."""
    scenario_path = _resolve_scenario_path(filename)
    ext = Path(file.filename or "").suffix.lower()
    if ext not in IMAGE_EXTENSIONS:
        raise HTTPException(status_code=400, detail="Unsupported image type")

    # Mirror the scenario's own folder structure so scenarios with the same base name in
    # different folders (e.g. OBIC/report.yaml and daily/report.yaml) don't collide.
    scenario_rel = scenario_path.relative_to(SCENARIOS_DIR).with_suffix("").as_posix()
    target_dir = SCENARIOS_DIR / "images" / scenario_rel
    target_dir.mkdir(parents=True, exist_ok=True)
    safe_name = Path(file.filename).name
    target_path = target_dir / safe_name
    if target_path.resolve().parent != target_dir.resolve():
        raise HTTPException(status_code=400, detail="Invalid filename")

    with open(target_path, "wb") as f:
        f.write(await file.read())

    return ImageUploadResponse(path=f"images/{scenario_rel}/{safe_name}")


@app.get("/api/scenarios")
def list_scenarios() -> list[ScenarioSummary]:
    summaries = []
    for path in sorted(SCENARIOS_DIR.rglob("*.yaml")):
        if path.relative_to(SCENARIOS_DIR).parts[0] == RESERVED_ROOT_FOLDER:
            continue
        with open(path, encoding="utf-8") as f:
            data = yaml.safe_load(f) or {}
        filename = path.relative_to(SCENARIOS_DIR).as_posix()
        summaries.append(ScenarioSummary(filename=filename, title=data.get("title") or ""))
    return summaries


@app.get("/api/version")
def get_version() -> dict[str, str]:
    return {"version": VERSION_FILE.read_text(encoding="utf-8").strip()}


@app.get("/docs/assets/{filename}")
def get_manual_asset(filename: str) -> FileResponse:
    """Serve the small, explicitly allowlisted asset set used by the user manual."""
    assets = {"screenshot_passoflow_01.png": app_root() / "images" / "screenshot_passoflow_01.png"}
    path = assets.get(filename)
    if path is None or not path.is_file():
        raise HTTPException(status_code=404, detail="Manual asset not found")
    return FileResponse(path, media_type="image/png")


@app.get("/docs/manual")
def get_manual(lang: str = "ja", audience: str = "user") -> HTMLResponse:
    """Renders docs/actions[_ja].md as a standalone HTML page for the Help menu's Manual link.

    Served by the app itself (rather than linking to GitHub) so it still works on a packaged
    exe with no internet/repo access.
    """
    if audience not in {"user", "advanced"}:
        raise HTTPException(status_code=400, detail="Invalid manual audience")
    if audience == "advanced":
        path = DOCS_DIR / ("actions_ja.md" if lang == "ja" else "actions.md")
    else:
        path = DOCS_DIR / ("guide_ja.md" if lang == "ja" else "guide.md")
    if not path.exists():
        raise HTTPException(status_code=404, detail="Manual not found")
    body = markdown.markdown(
        path.read_text(encoding="utf-8"), extensions=["tables", "fenced_code"]
    )
    # Give fenced examples a stable hook for the manual's code-block presentation.
    # The language token is emitted by Python-Markdown from the fenced-code info string.
    body = re.sub(
        r'<pre><code class="language-([\w+-]+)">',
        lambda match: (
            f'<pre class="manual-code-block" data-language="{match.group(1).upper()}">'
            f'<span class="manual-code-label" aria-hidden="true">{match.group(1).upper()}</span>'
            '<code class="language-'
            f'{match.group(1)}">'
        ),
        body,
    )
    body = body.replace(
        "docs-assets/screenshot_passoflow_01.png", "/docs/assets/screenshot_passoflow_01.png"
    )
    headings: list[tuple[str, str]] = []
    used_ids: dict[str, int] = {}
    heading_counters: list[int] = []

    def add_heading_id(match: re.Match[str]) -> str:
        level, title = match.group(1), match.group(2)
        plain_title = re.sub(r"<[^>]+>", "", title)
        id_title = plain_title
        heading_number = ""
        if int(level) >= 2:
            counter_index = int(level) - 2
            while len(heading_counters) <= counter_index:
                heading_counters.append(0)
            heading_counters[counter_index] += 1
            del heading_counters[counter_index + 1 :]
            heading_number = ".".join(str(value) for value in heading_counters)
            title = f"{heading_number} {title}"
            plain_title = f"{heading_number} {plain_title}"
        base_id = re.sub(r"[^\w\-ぁ-んァ-ヶ一-龠]+", "-", id_title, flags=re.UNICODE).strip("-").lower() or "section"
        used_ids[base_id] = used_ids.get(base_id, 0) + 1
        heading_id = base_id if used_ids[base_id] == 1 else f"{base_id}-{used_ids[base_id]}"
        if int(level) >= 2:
            headings.append((heading_id, plain_title))
        return f'<h{level} id="{heading_id}">{title}</h{level}>'

    body = re.sub(r"<h([1-6])>(.*?)</h\1>", add_heading_id, body, flags=re.DOTALL)
    # The Markdown files are also browsed directly in the repository, but this
    # endpoint renders them under /docs/manual, so their language switch links
    # must stay inside the application route instead of resolving to a 404.
    body = body.replace(
        'href="actions.md"', f'href="/docs/manual?lang=en&audience={audience}"'
    )
    body = body.replace(
        'href="actions_ja.md"', f'href="/docs/manual?lang=ja&audience={audience}"'
    )
    toc_label = "このマニュアルの目次" if lang == "ja" else "Manual index"
    intro = (
        "<p class=\"manual-intro\"><strong>目的から選んでください。</strong> "
        "まずアクションを選び、必要な値を設定して保存・実行します。"
        "分からない項目は、下の目的別一覧から始めると読みやすくなります。</p>"
        if lang == "ja"
        else "<p class=\"manual-intro\"><strong>Start with your goal.</strong> Choose an action, fill in its parameters, then save and run the scenario. The goal-based guide below is the quickest way to find the relevant section.</p>"
    )
    audience_label = (
        "一般ユーザー向けマニュアル" if audience == "user" else "上級者向け YAML / アクションリファレンス"
    ) if lang == "ja" else (
        "General user guide" if audience == "user" else "Advanced YAML / action reference"
    )
    alternate_label = (
        "上級者向けを見る" if audience == "user" else "一般ユーザー向けを見る"
    ) if lang == "ja" else (
        "Open advanced reference" if audience == "user" else "Open general user guide"
    )
    alternate_audience = "advanced" if audience == "user" else "user"
    audience_nav = (
        f'<nav class="manual-audience" aria-label="{audience_label}">'
        f'<strong>{audience_label}</strong> · '
        f'<a href="/docs/manual?lang={lang}&audience={alternate_audience}">{alternate_label}</a></nav>'
    )
    toc = "".join(f'<li><a href="#{heading_id}">{title}</a></li>' for heading_id, title in headings)
    index = f'<nav class="manual-index" aria-label="{toc_label}"><h2>{toc_label}</h2><ol>{toc}</ol></nav>'
    html = f"""<!doctype html>
<html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>passoflow Manual</title>
<style>
body {{ font-family: sans-serif; max-width: 980px; margin: 2rem auto; padding: 0 1rem 4rem; line-height: 1.6; color: #242331; }}
h1 {{ line-height: 1.25; margin-bottom: .5rem; }}
h2, h3 {{ scroll-margin-top: 1rem; }}
.manual-intro {{ padding: 1rem 1.2rem; border-left: 4px solid #8b3dff; background: #f4efff; }}
.manual-audience {{ display: flex; gap: .5rem; align-items: baseline; padding: .7rem 1rem; border: 1px solid #d8cbea; border-radius: 8px; background: #fff; }}
.manual-index {{ padding: 1rem 1.2rem; border: 1px solid #d8cbea; border-radius: 8px; background: #fbfaff; margin: 1.5rem 0 2rem; }}
.manual-index h2 {{ margin-top: 0; font-size: 1.15rem; }}
.manual-index ol {{ columns: 2; padding-left: 1.5rem; }}
.manual-index li {{ margin: .25rem 0; }}
a {{ color: #6330b5; }}
table {{ border-collapse: collapse; width: 100%; margin: 1rem 0; }}
th, td {{ border: 1px solid #ccc; padding: 6px 10px; text-align: left; vertical-align: top; }}
th {{ background: #f0f0f0; }}
code {{ background: #f0f0f0; padding: 2px 4px; border-radius: 3px; }}
pre.manual-code-block {{ position: relative; background: #1f2430; color: #f4f7fb; padding: 2.6rem 1rem 1rem; overflow-x: auto; border: 1px solid #454d61; border-radius: 8px; box-shadow: 0 2px 5px rgb(0 0 0 / 12%); }}
pre code[class*="language-"] {{ display: block; background: transparent; color: inherit; padding: 0; font: 0.92rem/1.55 ui-monospace, SFMono-Regular, Consolas, "Liberation Mono", monospace; white-space: pre; }}
.manual-code-label {{ position: absolute; top: .55rem; left: .8rem; color: #b9c5ff; font: 700 .72rem/1 ui-monospace, SFMono-Regular, Consolas, monospace; letter-spacing: .08em; }}
@media (max-width: 650px) {{ .manual-index ol {{ columns: 1; }} body {{ margin: 1rem auto; }} }}
</style></head><body>{audience_nav}{intro}{index}{body}</body></html>"""
    return HTMLResponse(content=html)


@app.get("/api/folders")
def list_folders() -> list[str]:
    folders = []
    for path in sorted(SCENARIOS_DIR.rglob("*")):
        if not path.is_dir():
            continue
        rel_parts = path.relative_to(SCENARIOS_DIR).parts
        if rel_parts[0] == RESERVED_ROOT_FOLDER:
            continue
        folders.append("/".join(rel_parts))
    return folders


@app.post("/api/folders", status_code=201)
def create_folder(folder: FolderCreate) -> dict[str, str]:
    path = _resolve_folder_path(folder.path)
    if path.exists():
        raise HTTPException(status_code=409, detail="Folder already exists")
    path.mkdir(parents=True)
    return {"path": folder.path}


@app.delete("/api/folders/{folder_path:path}", status_code=204)
def delete_folder(folder_path: str) -> None:
    path = _resolve_folder_path(folder_path)
    if not path.is_dir():
        raise HTTPException(status_code=404, detail="Folder not found")
    if any(path.iterdir()):
        raise HTTPException(status_code=409, detail="Folder is not empty")
    path.rmdir()


@app.post("/api/folders/{folder_path:path}/move")
def move_folder(folder_path: str, req: MoveRequest) -> dict[str, str]:
    old_folder = _resolve_folder_path(folder_path)
    if not old_folder.is_dir():
        raise HTTPException(status_code=404, detail="Folder not found")
    new_folder = _resolve_folder_path(req.destination)
    if new_folder.exists():
        raise HTTPException(status_code=409, detail="Destination already exists")

    for old_file in sorted(old_folder.rglob("*.yaml")):
        _move_scenario_file(old_file, new_folder / old_file.relative_to(old_folder))

    # Clean up now-empty leftover directories, deepest first.
    for d in sorted(old_folder.rglob("*"), key=lambda p: len(p.parts), reverse=True):
        if d.is_dir() and not any(d.iterdir()):
            d.rmdir()
    if old_folder.is_dir() and not any(old_folder.iterdir()):
        old_folder.rmdir()

    return {"path": req.destination}


@app.get("/api/scenarios/{filename:path}/export")
def export_scenario(filename: str) -> Response:
    """Package a scenario as a downloadable zip: its YAML, every call_scenario/repeat target
    reachable from it, and every image any of those reference — so re-importing it elsewhere
    reproduces a fully working scenario rather than one missing its screenshots or sub-scenarios.
    Registered before the bare {filename:path} route below so "/export" isn't swallowed by it."""
    path = _resolve_scenario_path(filename)
    if not path.exists():
        raise HTTPException(status_code=404, detail="Scenario not found")

    scenario_paths, image_paths = _collect_scenario_package(filename)

    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as zf:
        for rel in scenario_paths:
            zf.write(SCENARIOS_DIR / rel, arcname=rel)
        for rel in image_paths:
            image_path = (SCENARIOS_DIR / rel).resolve()
            if image_path.is_relative_to(SCENARIOS_DIR.resolve()) and image_path.is_file():
                zf.write(image_path, arcname=rel)

    zip_filename = f"{PurePosixPath(filename).stem}.zip"
    return Response(
        content=buffer.getvalue(),
        media_type="application/zip",
        headers={"Content-Disposition": f"attachment; filename*=UTF-8''{quote(zip_filename)}"},
    )


@app.get("/api/scenarios/{filename:path}")
def get_scenario(filename: str) -> Scenario:
    path = _resolve_scenario_path(filename)
    if not path.exists():
        raise HTTPException(status_code=404, detail="Scenario not found")
    with open(path, encoding="utf-8") as f:
        data = yaml.safe_load(f) or {}
    return Scenario(title=data.get("title") or "", steps=data.get("steps", []))


@app.post("/api/scenarios", status_code=201)
def create_scenario(scenario: ScenarioCreate) -> Scenario:
    _reject_reserved_path(scenario.filename)
    path = _resolve_scenario_path(scenario.filename)
    if path.exists():
        raise HTTPException(status_code=409, detail="Scenario already exists")
    _write_scenario(path, scenario.title, scenario.steps)
    return Scenario(title=scenario.title, steps=scenario.steps)


@app.post("/api/scenarios/import", status_code=201)
async def import_scenario(file: UploadFile = File(...)) -> dict[str, list[str]]:
    """Extract a scenario package (as produced by /export) into scenarios/. All-or-nothing:
    if any entry would overwrite an existing file, nothing is written and every conflict is
    listed, so the caller can rename/remove first rather than ending up with a partial import."""
    content = await file.read()
    try:
        zf = zipfile.ZipFile(io.BytesIO(content))
    except zipfile.BadZipFile as e:
        raise HTTPException(status_code=400, detail="Not a valid zip file") from e

    entries = [(info.filename, _validate_import_entry(info.filename)) for info in zf.infolist() if not info.is_dir()]
    conflicts = [arcname for arcname, dest in entries if dest.exists()]
    if conflicts:
        raise HTTPException(status_code=409, detail=f"Would overwrite existing file(s): {', '.join(conflicts)}")

    imported_scenarios = []
    for arcname, dest in entries:
        dest.parent.mkdir(parents=True, exist_ok=True)
        with zf.open(arcname) as src, open(dest, "wb") as out:
            out.write(src.read())
        if PurePosixPath(arcname).suffix.lower() in _IMPORT_YAML_EXTENSIONS:
            imported_scenarios.append(arcname)

    return {"scenarios": imported_scenarios}


@app.put("/api/scenarios/{filename:path}")
def update_scenario(filename: str, scenario: Scenario) -> Scenario:
    _reject_reserved_path(filename)
    path = _resolve_scenario_path(filename)
    _write_scenario(path, scenario.title, scenario.steps)
    return scenario


@app.post("/api/scenarios/{filename:path}/move")
def move_scenario(filename: str, req: MoveRequest) -> dict[str, str]:
    _reject_reserved_path(req.destination)
    old_path = _resolve_scenario_path(filename)
    if not old_path.exists():
        raise HTTPException(status_code=404, detail="Scenario not found")
    new_path = _resolve_scenario_path(req.destination)
    if new_path.exists():
        raise HTTPException(status_code=409, detail="Destination already exists")
    _move_scenario_file(old_path, new_path)
    return {"filename": req.destination}


@app.delete("/api/scenarios/{filename:path}", status_code=204)
def delete_scenario(filename: str) -> None:
    path = _resolve_scenario_path(filename)
    if not path.exists():
        raise HTTPException(status_code=404, detail="Scenario not found")
    path.unlink()


def _write_scenario(path: Path, title: str, steps: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        yaml.dump({"title": title, "steps": steps}, f, allow_unicode=True, sort_keys=False)


# --- Scenario execution, streamed to the browser over a WebSocket ---

_active_runs: dict[str, dict[str, Any]] = {}


def _force_kill_process(process: asyncio.subprocess.Process) -> None:
    """Terminate a process that did not honor a cooperative stop in time."""
    if process.returncode is not None:
        return
    try:
        process.kill()
    except ProcessLookupError:
        pass


def _classify_run_outcome(returncode: int, stop_requested: bool, warning_lines: list[str]) -> str:
    if stop_requested:
        return "stopped"
    if returncode != 0:
        return "failed"
    return "warning" if warning_lines else "success"


def _capture_failure_screenshot(run_id: str) -> Path | None:
    """Capture the current screen for a failed/stopped run without masking its outcome."""
    target = app_root() / "logs" / f"run_{run_id}_failure.png"
    try:
        target.parent.mkdir(parents=True, exist_ok=True)
        pyautogui.screenshot().save(target)
        return target
    except Exception as error:
        logger.warning("Could not capture failure screenshot for run %s: %s", run_id, error)
        return None


@app.get("/api/scenarios/{filename:path}/validate")
def validate_scenario(filename: str) -> dict[str, list[str]]:
    path = _resolve_scenario_path(filename)
    if not path.exists():
        raise HTTPException(status_code=404, detail="Scenario not found")
    errors, warnings = _validate_scenario(path)
    return {"errors": errors, "warnings": warnings}


@app.get("/api/scenarios/{filename:path}/validate-rust")
def validate_scenario_with_rust(filename: str) -> dict[str, Any]:
    """Run the opt-in Rust validator without changing the Python baseline."""
    path = _resolve_scenario_path(filename)
    if not path.exists():
        raise HTTPException(status_code=404, detail="Scenario not found")
    try:
        report = validate_with_rust(path)
    except (OSError, RuntimeError, ValueError) as error:
        raise HTTPException(status_code=502, detail=str(error)) from error
    if report is None:
        raise HTTPException(status_code=503, detail="PASSOFLOW_VALIDATE_BIN is not configured")
    return {
        "valid": report.valid,
        "errors": report.messages("error"),
        "warnings": report.messages("warning"),
        "diagnostics": list(report.diagnostics),
    }


@app.get("/api/scenarios/{filename:path}/validate-compare")
def compare_scenario_validation(filename: str) -> dict[str, Any]:
    """Compare Python and opt-in Rust validation before changing the default."""
    path = _resolve_scenario_path(filename)
    if not path.exists():
        raise HTTPException(status_code=404, detail="Scenario not found")
    try:
        report = validate_with_rust(path)
    except (OSError, RuntimeError, ValueError) as error:
        raise HTTPException(status_code=502, detail=str(error)) from error
    if report is None:
        raise HTTPException(status_code=503, detail="PASSOFLOW_VALIDATE_BIN is not configured")
    python_errors, python_warnings = _validate_scenario(path)
    return compare_with_python(python_errors, python_warnings, report)


@app.post("/api/scenarios/{filename:path}/run")
def start_run(filename: str, start: int | None = None, end: int | None = None) -> dict[str, str]:
    path = _resolve_scenario_path(filename)
    if not path.exists():
        raise HTTPException(status_code=404, detail="Scenario not found")
    run_id = uuid.uuid4().hex
    stop_file = app_root() / "logs" / f"run_{run_id}.stop"
    # reserved until the WebSocket starts the process
    _active_runs[run_id] = {
        "process": None,
        "start": start,
        "end": end,
        "stop_requested": False,
        "stop_file": stop_file,
    }
    return {"run_id": run_id, "filename": filename}


@app.post("/api/runs/{run_id}/stop")
def stop_run(run_id: str) -> dict[str, str]:
    """Request graceful stop, with a delayed kill fallback for an unresponsive process."""
    run_info = _active_runs.get(run_id)
    if run_info is None:
        raise HTTPException(status_code=404, detail="Run not found (already finished or invalid run_id)")
    if run_info["process"] is None:
        raise HTTPException(status_code=409, detail="Run hasn't started yet")
    run_info["stop_requested"] = True
    stop_file = run_info.get("stop_file")
    if stop_file is not None:
        stop_file.parent.mkdir(parents=True, exist_ok=True)
        stop_file.write_text("stop\n", encoding="utf-8")
    timer = threading.Timer(2.0, _force_kill_process, args=(run_info["process"],))
    timer.daemon = True
    run_info["stop_timer"] = timer
    timer.start()
    return {"status": "stopping"}


def _build_local_run_feedback(log_lines: list[str]) -> str:
    """Provide actionable fallback advice when model feedback cannot be generated."""
    joined = "\n".join(log_lines)
    suggestions: list[str] = []
    if "images were found" in joined or "image" in joined.lower() and "found" in joined.lower():
        suggestions.append(
            "画像が見つからない場合：直前にwaitを追加し、retry/retry_interval_msを増やし、候補画像とconfidenceを見直してください。"
        )
    if "No window found" in joined:
        suggestions.append(
            "ウィンドウが見つからない場合：アプリ起動後にwaitを追加し、activate_windowのretry/retry_interval_msを増やしてください。title_containsも確認してください。"
        )
    if "Webhook" in joined or "webhook" in joined:
        suggestions.append("Webhook失敗の場合：URL、接続先、payloadを確認し、必要なら処理前にwaitや再試行を追加してください。")
    if not suggestions:
        suggestions.append("警告が発生したステップを確認し、前段のwait、retry、入力値、対象ファイルを見直してください。")
    return "\n".join(f"- {suggestion}" for suggestion in suggestions)


def _generate_run_feedback(log_lines: list[str], system_prompt: str) -> str | None:
    """Ask the model to analyze a run's log lines (warnings, or a failure/stop tail) and give feedback.

    Best-effort: any failure (missing API key, rate limit, network) is logged and swallowed
    rather than raised, since losing the feedback is not a reason to report the run as failed.
    """
    try:
        response = _create_message(
            max_tokens=RUN_FEEDBACK_MAX_TOKENS,
            system=system_prompt,
            messages=[{"role": "user", "content": "\n".join(log_lines)}],
        )
    except HTTPException as e:
        logger.warning("Could not generate run feedback: %s", e.detail)
        return None
    text = "".join(block.text for block in response.content if block.type == "text")
    return text or None


@app.websocket("/ws/runs/{run_id}")
async def stream_run(websocket: WebSocket, run_id: str) -> None:
    await websocket.accept()
    filename = websocket.query_params.get("filename")
    run_info = _active_runs.get(run_id)
    if run_info is None or not filename:
        await websocket.close(code=4404)
        return

    try:
        path = _resolve_scenario_path(filename)
    except HTTPException:
        await websocket.close(code=4400)
        return

    if getattr(sys, "frozen", False):
        cmd = [sys.executable, "--run-scenario", str(path)]
    else:
        cmd = [sys.executable, str(RUN_SCENARIO_SCRIPT), str(path)]
    if run_info["start"] is not None:
        cmd += ["--start", str(run_info["start"])]
    if run_info["end"] is not None:
        cmd += ["--end", str(run_info["end"])]
    cmd += ["--run-id", run_id]

    child_env = os.environ.copy()
    stop_file = run_info.get("stop_file")
    if stop_file is not None:
        child_env["PASSOFLOW_STOP_FILE"] = str(stop_file)
    process = await asyncio.create_subprocess_exec(
        *cmd,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.STDOUT,
        env=child_env,
    )
    run_info["process"] = process

    warning_lines: list[str] = []
    all_lines: list[str] = []
    try:
        assert process.stdout is not None
        async for raw_line in process.stdout:
            line = raw_line.decode("utf-8", errors="replace").rstrip()
            progress_match = PROGRESS_PATTERN.match(line)
            completed_match = COMPLETED_PATTERN.match(line)
            if progress_match:
                run_info["active_step"] = int(progress_match.group(1))
                await websocket.send_json(
                    {"type": "progress", "step": int(progress_match.group(1)), "total": int(progress_match.group(2))}
                )
            elif completed_match:
                await websocket.send_json(
                    {"type": "completed", "step": int(completed_match.group(1)), "total": int(completed_match.group(2))}
                )
            else:
                await websocket.send_json({"type": "log", "line": line})
                all_lines.append(line)
                if "[WARNING]" in line:
                    warning_lines.append(line)
        returncode = await process.wait()
        if returncode != 0:
            failure_screenshot = await asyncio.to_thread(_capture_failure_screenshot, run_id)
            if failure_screenshot:
                screenshot_line = f"Failure screenshot saved: {failure_screenshot}"
                all_lines.append(screenshot_line)
                await websocket.send_json({"type": "log", "line": screenshot_line})
        # A non-zero exit means the run either errored out or was stopped (POST .../stop kills the
        # process) partway through -- either way there's no clean set of WARNING lines to point at,
        # so fall back to analyzing the tail of the whole log instead.
        if returncode == 0:
            analysis_lines, system_prompt = warning_lines, RUN_FEEDBACK_SYSTEM_PROMPT
        else:
            analysis_lines, system_prompt = all_lines[-RUN_FAILURE_ANALYSIS_MAX_LINES:], RUN_FAILURE_ANALYSIS_SYSTEM_PROMPT
        if analysis_lines:
            feedback = await asyncio.to_thread(_generate_run_feedback, analysis_lines, system_prompt)
            if not feedback:
                feedback = _build_local_run_feedback(analysis_lines)
            if feedback:
                await websocket.send_json({"type": "feedback", "text": feedback})
        outcome = _classify_run_outcome(returncode, bool(run_info["stop_requested"]), warning_lines)
        await websocket.send_json({"type": "done", "returncode": returncode, "outcome": outcome})
    except WebSocketDisconnect:
        pass
    finally:
        stop_timer = run_info.get("stop_timer")
        if stop_timer is not None:
            stop_timer.cancel()
        if process.returncode is None:
            process.kill()
        stop_file = run_info.get("stop_file")
        if stop_file is not None:
            try:
                stop_file.unlink()
            except FileNotFoundError:
                pass
        _active_runs.pop(run_id, None)
        try:
            await websocket.close()
        except RuntimeError:
            pass  # already closed


# Serves the built web UI (web-ui/dist, produced by `npm run build`) so a packaged build can
# run as a single process. In dev this directory doesn't exist -- the UI is served separately
# by the Vite dev server instead -- so the mount is skipped rather than erroring.
if WEB_UI_DIST.is_dir():
    app.mount("/", StaticFiles(directory=str(WEB_UI_DIST), html=True), name="web-ui")


if __name__ == "__main__":
    uvicorn.run(app, host="127.0.0.1", port=8000)
