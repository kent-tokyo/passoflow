"""Safe scenario-rooted path resolution shared by validation and Rust expansion."""

from __future__ import annotations

from pathlib import Path


def resolve_scenario_path(root: str | Path, target: object) -> Path:
    """Resolve a nested scenario while preventing traversal and absolute paths."""
    if not isinstance(target, str) or not target.strip():
        raise ValueError("nested scenario path must be a non-empty string")
    root_path = Path(root).resolve()
    candidate = (root_path / target).resolve()
    try:
        relative = candidate.relative_to(root_path)
    except ValueError as error:
        raise ValueError(f"nested scenario path escapes the scenarios directory: {target}") from error
    return root_path / relative
