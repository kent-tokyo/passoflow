"""Manual test: click whichever お気に入り state is on screen."""

from pathlib import Path

from logging_config import setup_logging
from screen_actions import click_image

if __name__ == "__main__":
    setup_logging()

    IMAGE_DIR = Path(__file__).resolve().parent.parent / "scenarios" / "images" / "obic_sales"
    IMAGE_PATHS = [IMAGE_DIR / f"お気に入り_{n}.png" for n in ("01", "02", "03")]
    click_image(IMAGE_PATHS)
