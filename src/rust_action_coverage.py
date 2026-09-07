"""Preflight checks for actions before Rust-backed execution starts."""

from __future__ import annotations


STRUCTURAL_ACTIONS = frozenset({"if", "else", "endif"})


def unsupported_actions(steps: list[dict], supported: set[str]) -> list[str]:
    """Return sorted non-structural actions without a Python runtime adapter."""
    actions = {
        step.get("action")
        for step in steps
        if isinstance(step, dict) and step.get("action") not in STRUCTURAL_ACTIONS
    }
    missing = {
        action if isinstance(action, str) else "<missing action>"
        for action in actions
        if not isinstance(action, str) or action not in supported
    }
    return sorted(missing)
