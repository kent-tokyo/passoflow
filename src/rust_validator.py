"""Optional bridge for consuming the Rust scenario validator from Python.

The bridge is deliberately opt-in. The Python runner remains the default until
Rust validation also covers nested scenario references and runtime warnings.
"""

from __future__ import annotations

import json
import os
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any

CONTRACT_VERSION = "0.1"


@dataclass(frozen=True)
class RustValidationReport:
    """Machine-readable result emitted by ``passoflow-validate``."""

    valid: bool
    errors: int
    warnings: int
    diagnostics: tuple[dict[str, Any], ...]

    @classmethod
    def from_json(cls, payload: str) -> "RustValidationReport":
        """Parse and minimally validate a Rust validator JSON report."""
        data = json.loads(payload)
        if (
            not isinstance(data, dict)
            or data.get("contract") != CONTRACT_VERSION
            or not isinstance(data.get("valid"), bool)
            or not isinstance(data.get("errors"), int)
            or not isinstance(data.get("warnings"), int)
            or not isinstance(data.get("diagnostics"), list)
        ):
            raise ValueError("Rust validator returned an invalid report")
        diagnostics = tuple(item for item in data["diagnostics"] if isinstance(item, dict))
        if len(diagnostics) != len(data["diagnostics"]):
            raise ValueError("Rust validator returned a malformed diagnostic")
        for item in diagnostics:
            if (
                item.get("severity") not in {"error", "warning"}
                or not isinstance(item.get("code"), str)
                or not isinstance(item.get("path"), str)
                or not isinstance(item.get("message"), str)
            ):
                raise ValueError("Rust validator returned a malformed diagnostic")
        errors = sum(item.get("severity") == "error" for item in diagnostics)
        warnings = sum(item.get("severity") == "warning" for item in diagnostics)
        if (
            data["errors"] < 0
            or data["warnings"] < 0
            or errors != data["errors"]
            or warnings != data["warnings"]
            or data["valid"] != (errors == 0)
        ):
            raise ValueError("Rust validator returned inconsistent counts")
        return cls(
            valid=data["valid"],
            errors=data["errors"],
            warnings=data["warnings"],
            diagnostics=diagnostics,
        )

    def messages(self, severity: str) -> list[str]:
        """Render diagnostics in the Python API's existing message shape."""
        return [
            f"{item.get('path', 'scenario')}: {item.get('message', item.get('code', 'validation error'))}"
            for item in self.diagnostics
            if item.get("severity") == severity
        ]


def validate_with_rust(path: str | Path, binary: str | Path | None = None) -> RustValidationReport | None:
    """Validate one scenario with an explicitly configured Rust binary.

    Returns ``None`` when ``PASSOFLOW_VALIDATE_BIN`` is not configured. This
    makes the migration safe for packaged installations that do not yet ship
    the Rust executable.
    """
    executable = binary or os.environ.get("PASSOFLOW_VALIDATE_BIN")
    if not executable:
        return None
    try:
        completed = subprocess.run(
            [str(executable), str(path)],
            check=False,
            capture_output=True,
            text=True,
            timeout=10,
        )
    except subprocess.TimeoutExpired as error:
        raise RuntimeError("Rust validator timed out after 10 seconds") from error
    if not completed.stdout.strip():
        detail = completed.stderr.strip() or "Rust validator produced no report"
        raise RuntimeError(detail)
    return RustValidationReport.from_json(completed.stdout)


def normalize_with_rust(path: str | Path, binary: str | Path | None = None) -> str | None:
    """Return deterministic normalized YAML from an explicitly configured CLI."""
    executable = binary or os.environ.get("PASSOFLOW_VALIDATE_BIN")
    if not executable:
        return None
    try:
        completed = subprocess.run(
            [str(executable), "--normalized", str(path)],
            check=False,
            capture_output=True,
            text=True,
            timeout=10,
        )
    except subprocess.TimeoutExpired as error:
        raise RuntimeError("Rust validator timed out after 10 seconds") from error
    if completed.returncode != 0 or not completed.stdout.strip():
        detail = completed.stderr.strip() or "Rust validator produced no normalized scenario"
        raise RuntimeError(detail)
    return completed.stdout


def plan_with_rust(path: str | Path, binary: str | Path | None = None) -> dict[str, Any] | None:
    """Return a validated structural execution plan from the Rust CLI."""
    executable = binary or os.environ.get("PASSOFLOW_VALIDATE_BIN")
    if not executable:
        return None
    try:
        completed = subprocess.run(
            [str(executable), "--plan", str(path)],
            check=False,
            capture_output=True,
            text=True,
            timeout=10,
        )
    except subprocess.TimeoutExpired as error:
        raise RuntimeError("Rust planner timed out after 10 seconds") from error
    if completed.returncode != 0 or not completed.stdout.strip():
        detail = completed.stderr.strip() or "Rust planner produced no execution plan"
        raise RuntimeError(detail)
    data = json.loads(completed.stdout)
    if not isinstance(data, dict) or data.get("contract") != CONTRACT_VERSION or not isinstance(data.get("steps"), list):
        raise ValueError("Rust planner returned an invalid execution plan")
    for step in data["steps"]:
        if not isinstance(step, dict) or not isinstance(step.get("index"), int) or not isinstance(step.get("action"), str):
            raise ValueError("Rust planner returned a malformed step")
    return data


def assert_plan_matches_steps(plan: dict[str, Any], steps: list[dict[str, Any]]) -> None:
    """Reject a plan when it no longer describes the steps about to execute."""
    planned_steps = plan["steps"]
    if len(planned_steps) != len(steps):
        raise RuntimeError(
            "Rust execution plan is stale: "
            f"planned {len(planned_steps)} steps, loaded {len(steps)}"
        )
    for index, (planned, step) in enumerate(zip(planned_steps, steps, strict=True), start=1):
        if planned["index"] != index or planned["action"] != step.get("action"):
            raise RuntimeError(f"Rust execution plan does not match loaded step {index}")


def compare_with_python(
    python_errors: list[str], python_warnings: list[str], rust_report: RustValidationReport
) -> dict[str, Any]:
    """Compare the blocking result of Python and Rust validation."""
    python_valid = not python_errors
    valid_match = python_valid == rust_report.valid
    error_count_match = len(python_errors) == rust_report.errors
    warning_count_match = len(python_warnings) == rust_report.warnings
    return {
        "ready_for_default": valid_match and error_count_match and warning_count_match,
        "valid_match": valid_match,
        "error_count_match": error_count_match,
        "warning_count_match": warning_count_match,
        "python_valid": python_valid,
        "rust_valid": rust_report.valid,
        "python_errors": python_errors,
        "python_warnings": python_warnings,
        "rust_errors": rust_report.errors,
        "rust_warnings": rust_report.warnings,
        "rust_diagnostics": list(rust_report.diagnostics),
    }
