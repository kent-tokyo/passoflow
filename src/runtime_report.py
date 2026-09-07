"""Validation for the Rust runtime report consumed by the Python runner."""

from __future__ import annotations


VALID_STATUSES = {"success", "warning_continue", "failure_stop", "stopped"}
VALID_EVENT_OUTCOMES = {"success", "warning_continue", "failure_stop"}


def validate_runtime_report(report: object) -> dict:
    """Validate the minimal report shape before it reaches UI/log handling."""
    if not isinstance(report, dict):
        raise RuntimeError("Rust engine returned a malformed report")
    if report.get("status") not in VALID_STATUSES:
        raise RuntimeError("Rust engine returned an invalid execution status")
    events = report.get("events")
    if not isinstance(events, list):
        raise RuntimeError("Rust engine returned a malformed event list")
    for event in events:
        if not isinstance(event, dict) or not isinstance(event.get("step"), int) or isinstance(event["step"], bool):
            raise RuntimeError("Rust engine returned a malformed event")
        if event.get("outcome") not in VALID_EVENT_OUTCOMES:
            raise RuntimeError("Rust engine returned an invalid event outcome")
    return report
