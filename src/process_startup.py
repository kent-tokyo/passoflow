"""Dependency-free process readiness policy for application launch actions."""

from __future__ import annotations

from collections.abc import Callable
from typing import Protocol


class ProcessLike(Protocol):
    returncode: int | None

    def poll(self) -> int | None: ...


def wait_for_window(
    process: ProcessLike,
    title: str,
    startup_timeout_ms: int,
    window_lookup: Callable[[str], object],
    sleep: Callable[[float], None],
    monotonic: Callable[[], float],
) -> None:
    """Wait for a matching window, rejecting early exit and timeout."""
    deadline = monotonic() + max(startup_timeout_ms, 0) / 1000
    while monotonic() <= deadline:
        if process.poll() is not None:
            raise RuntimeError(f"Application exited during startup with code {process.returncode}")
        if window_lookup(title):
            return
        sleep(0.1)
    raise TimeoutError(
        f"Application did not open a matching window within {startup_timeout_ms} ms"
    )
