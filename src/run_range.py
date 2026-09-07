"""Validation for guarded, 1-indexed scenario rerun ranges."""

from __future__ import annotations


def resolve_run_range(total: int, start: int | None, end: int | None) -> tuple[int, int]:
    """Return a zero-based half-open range after validating CLI-style bounds."""
    if total < 0:
        raise ValueError("scenario step count must not be negative")
    if start is not None and start < 1:
        raise ValueError("--start must be a positive 1-indexed step")
    if end is not None and end < 1:
        raise ValueError("--end must be a positive 1-indexed step")
    if start is None and end is None:
        return 0, total
    first = (start - 1) if start is not None else 0
    last = end if end is not None else total
    if first > total or last > total:
        raise ValueError(f"rerun range {first + 1}-{last} is outside the {total}-step scenario")
    if first >= last:
        raise ValueError(f"rerun range must be ordered and non-empty: {first + 1}-{last}")
    return first, last
