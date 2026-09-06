"""Resolves where the app's user-visible data (scenarios/, logs/, VERSION, .env) lives.

In a PyInstaller --onedir build this must be the folder the .exe was launched from, not
PyInstaller's bundled-code location, so scenarios/logs/config live next to the distributed
app rather than inside it. In a normal `python src/...py` run it's the repo root.
"""

import sys
from pathlib import Path


def app_root() -> Path:
    if getattr(sys, "frozen", False):
        return Path(sys.executable).resolve().parent
    return Path(__file__).resolve().parent.parent
