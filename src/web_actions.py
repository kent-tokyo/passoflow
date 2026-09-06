"""Browser actions for visual and DOM-based web automation."""

import logging
import webbrowser

logger = logging.getLogger(__name__)

_playwright = None
_browser = None
_page = None


def open_url(url: str) -> None:
    """Open a URL in the user's default browser for visual/image-based actions."""
    if not webbrowser.open(url, new=2):
        raise RuntimeError(f"Could not open URL in the default browser: {url}")
    logger.info("Opened URL in the default browser: %s", url)


def _get_page():
    global _playwright, _browser, _page
    if _page is not None:
        return _page
    try:
        from playwright.sync_api import sync_playwright
    except ImportError as exc:
        raise RuntimeError("DOM browser actions require the Playwright Python package") from exc
    _playwright = sync_playwright().start()
    _browser = _playwright.chromium.launch(headless=False)
    _page = _browser.new_page()
    return _page


def browser_navigate(url: str, timeout_ms: int = 30_000) -> None:
    page = _get_page()
    page.goto(url, wait_until="domcontentloaded", timeout=timeout_ms)
    logger.info("Navigated DOM browser to: %s", url)


def browser_click(selector: str, timeout_ms: int = 10_000) -> None:
    page = _get_page()
    page.locator(selector).click(timeout=timeout_ms)
    logger.info("Clicked DOM selector: %s", selector)


def browser_fill(selector: str, text: str, timeout_ms: int = 10_000) -> None:
    page = _get_page()
    page.locator(selector).fill(text, timeout=timeout_ms)
    logger.info("Filled DOM selector: %s", selector)


def browser_wait_for(selector: str, state: str = "visible", timeout_ms: int = 10_000) -> None:
    page = _get_page()
    page.locator(selector).wait_for(state=state, timeout=timeout_ms)
    logger.info("DOM selector is %s: %s", state, selector)


def close_browser() -> None:
    global _playwright, _browser, _page
    if _browser is not None:
        _browser.close()
    if _playwright is not None:
        _playwright.stop()
    _playwright = None
    _browser = None
    _page = None
