"""Shared logging setup: writes to console and a timestamped file under logs/."""

import logging
import sys
from datetime import datetime

from app_paths import app_root

LOG_DIR = app_root() / "logs"


def setup_logging(name: str = "passoflow") -> logging.Logger:
    """Configure the root logger to write to console and logs/<name>_<timestamp>.log."""
    # logging.StreamHandler() defaults to sys.stderr, which on Windows falls back to the
    # system ANSI codepage (e.g. cp932) rather than UTF-8 when the process isn't attached to
    # a real console (as when api_server.py runs this script as a piped subprocess). Without
    # this, Japanese/Chinese log text is written as cp932 bytes but read back as UTF-8 by the
    # web UI's WebSocket relay, garbling it. print() (used for the @@PROGRESS@@ marker) goes
    # through sys.stdout, so reconfigure both.
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8")

    LOG_DIR.mkdir(exist_ok=True)
    log_file = LOG_DIR / f"{name}_{datetime.now():%Y%m%d_%H%M%S}.log"

    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
        handlers=[
            logging.FileHandler(log_file, encoding="utf-8"),
            logging.StreamHandler(),
        ],
    )
    return logging.getLogger(name)
