"""Browser actions for visual and DOM-based web automation."""

import importlib.util
import logging
import os
import re
from pathlib import Path
import webbrowser

logger = logging.getLogger(__name__)

_playwright = None
_browser = None
_page = None
_rust_dom_browser = None


def _use_rust_dom() -> bool:
    return os.environ.get("PASSOFLOW_USE_RUST_DOM") == "1"


def _get_rust_dom_browser():
    global _rust_dom_browser
    if _rust_dom_browser is not None:
        return _rust_dom_browser
    endpoint = os.environ.get("PASSOFLOW_RUST_CDP_ENDPOINT", "").strip()
    if not endpoint:
        raise RuntimeError("PASSOFLOW_RUST_CDP_ENDPOINT is required when PASSOFLOW_USE_RUST_DOM=1")
    try:
        from passoflow_python import DomBrowser
    except ImportError as exc:
        raise RuntimeError("Rust DOM mode requires the passoflow Python binding") from exc
    _rust_dom_browser = DomBrowser(endpoint)
    return _rust_dom_browser


def dom_browser_setup_status() -> dict[str, object]:
    """Return local, non-invasive readiness information for DOM browser actions."""
    if _use_rust_dom():
        endpoint = os.environ.get("PASSOFLOW_RUST_CDP_ENDPOINT", "").strip()
        binding_available = importlib.util.find_spec("passoflow_python") is not None
        if not binding_available:
            return {
                "mode": "rust",
                "rust_binding": False,
                "cdp_endpoint": bool(endpoint),
                "message": "Install the passoflow Python binding for Rust DOM mode.",
            }
        if not endpoint:
            return {
                "mode": "rust",
                "rust_binding": True,
                "cdp_endpoint": False,
                "message": "Set PASSOFLOW_RUST_CDP_ENDPOINT to a local Chromium ws:// endpoint.",
            }
        return {
            "mode": "rust",
            "rust_binding": True,
            "cdp_endpoint": True,
            "message": "Rust DOM browser is configured.",
        }
    if importlib.util.find_spec("playwright") is None:
        return {
            "playwright": False,
            "chromium": False,
            "message": "Install the Playwright Python package with pip install playwright.",
        }
    try:
        from playwright.sync_api import sync_playwright

        with sync_playwright() as playwright:
            executable = Path(playwright.chromium.executable_path)
    except Exception as exc:
        logger.debug("Could not inspect Playwright Chromium: %s", exc)
        return {
            "playwright": True,
            "chromium": False,
            "message": "Install the Playwright Chromium browser with python -m playwright install chromium.",
        }
    if not executable.is_file():
        return {
            "playwright": True,
            "chromium": False,
            "message": "Install the Playwright Chromium browser with python -m playwright install chromium.",
        }
    return {"playwright": True, "chromium": True, "message": "DOM browser actions are ready."}


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
    if _use_rust_dom():
        _get_rust_dom_browser().navigate(url)
        logger.info("Navigated Rust DOM browser to: %s", url)
        return
    page = _get_page()
    page.goto(url, wait_until="domcontentloaded", timeout=timeout_ms)
    logger.info("Navigated DOM browser to: %s", url)


def browser_click(selector: str, timeout_ms: int = 10_000) -> None:
    if _use_rust_dom():
        _get_rust_dom_browser().click(selector)
        logger.info("Clicked Rust DOM selector: %s", selector)
        return
    page = _get_page()
    page.locator(selector).click(timeout=timeout_ms)
    logger.info("Clicked DOM selector: %s", selector)


def browser_fill(selector: str, text: str, timeout_ms: int = 10_000) -> None:
    if _use_rust_dom():
        _get_rust_dom_browser().fill(selector, text)
        logger.info("Filled Rust DOM selector: %s", selector)
        return
    page = _get_page()
    page.locator(selector).fill(text, timeout=timeout_ms)
    logger.info("Filled DOM selector: %s", selector)


def browser_wait_for(selector: str, state: str = "visible", timeout_ms: int = 10_000) -> None:
    if _use_rust_dom():
        _get_rust_dom_browser().wait_for(selector, state, timeout_ms)
        logger.info("Rust DOM selector is %s: %s", state, selector)
        return
    page = _get_page()
    page.locator(selector).wait_for(state=state, timeout=timeout_ms)
    logger.info("DOM selector is %s: %s", state, selector)


def preview_dom_selector(url: str, selector: str, timeout_ms: int = 10_000) -> dict[str, object]:
    """Inspect a selector in a throwaway local Playwright page for the editor preview.

    This deliberately does not reuse the scenario browser session: previewing a selector
    must not change the page, cookies, or execution state of a running scenario.
    """
    if not url.strip().lower().startswith(("http://", "https://")):
        raise ValueError("DOM preview accepts only http:// or https:// URLs")
    if not selector.strip():
        raise ValueError("A CSS selector is required")
    if timeout_ms < 100 or timeout_ms > 30_000:
        raise ValueError("Preview timeout must be between 100 and 30000 milliseconds")
    try:
        from playwright.sync_api import sync_playwright
    except ImportError as exc:
        raise RuntimeError("DOM browser actions require the Playwright Python package") from exc

    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True)
        try:
            page = browser.new_page()
            page.goto(url.strip(), wait_until="domcontentloaded", timeout=timeout_ms)
            locator = page.locator(selector.strip())
            count = locator.count()
            samples = locator.evaluate_all(
                """elements => elements.slice(0, 5).map(element => ({
                    tag: element.tagName.toLowerCase(),
                    text: (element.innerText || element.getAttribute('aria-label') || '').trim().slice(0, 120),
                    id: element.id || '',
                    testid: element.getAttribute('data-testid') || '',
                    visible: !!(element.offsetWidth || element.offsetHeight || element.getClientRects().length),
                    selector: element.id
                        ? `#${CSS.escape(element.id)}`
                        : element.getAttribute('data-testid')
                            ? `[data-testid="${element.getAttribute('data-testid').replaceAll('\\\\', '\\\\\\\\').replaceAll('"', '\\\\"')}"]`
                            : element.tagName.toLowerCase() + [...element.classList].filter(Boolean).slice(0, 2).map(name => `.${CSS.escape(name)}`).join('')
                }))"""
            )
            suggested_selector = locator.first.evaluate(
                """element => {
                    if (element.id) return `#${CSS.escape(element.id)}`;
                    const testid = element.getAttribute('data-testid');
                    if (testid) return `[data-testid="${testid.replaceAll('\\\\', '\\\\\\\\').replaceAll('"', '\\\\"')}"]`;
                    const classes = [...element.classList].filter(Boolean).slice(0, 2);
                    return element.tagName.toLowerCase() + classes.map(name => `.${CSS.escape(name)}`).join('');
                }"""
            ) if count else None
            repair_suggestions: list[dict[str, object]] = []
            if count == 0:
                candidates: list[str] = []
                id_match = re.fullmatch(r"(?:[a-zA-Z][\\w-]*)?#([\\w-]+)", selector.strip())
                testid_match = re.fullmatch(r"\\[data-testid=[\\\"']([^\\\"']+)[\\\"']\\]", selector.strip())
                class_match = re.fullmatch(r"(?:[a-zA-Z][\\w-]*)?\\.([\\w-]+)", selector.strip())
                if id_match:
                    value = id_match.group(1)
                    candidates = [f'[data-testid="{value}"]', f'[name="{value}"]']
                elif testid_match:
                    value = testid_match.group(1)
                    candidates = [f"#{value}", f'[aria-label="{value}"]']
                elif class_match:
                    value = class_match.group(1)
                    candidates = [f'[class~="{value}"]']
                for candidate in candidates:
                    candidate_count = page.locator(candidate).count()
                    if candidate_count:
                        repair_suggestions.append({"selector": candidate, "count": candidate_count})
            return {
                "url": page.url,
                "selector": selector.strip(),
                "count": count,
                "samples": samples,
                "suggested_selector": suggested_selector,
                "repair_suggestions": repair_suggestions,
            }
        finally:
            browser.close()


def close_browser() -> None:
    global _playwright, _browser, _page, _rust_dom_browser
    if _browser is not None:
        _browser.close()
    if _playwright is not None:
        _playwright.stop()
    _playwright = None
    _browser = None
    _page = None
    _rust_dom_browser = None
