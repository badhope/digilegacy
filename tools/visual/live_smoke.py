#!/usr/bin/env python3
"""Live Web console smoke through a real Chromium browser.

Unlike `regress.py`, this runs against the Vue dev server and a real API, not
the mock build. It checks that every route renders a `main` element, does not
overflow horizontally, and produces no page exceptions or API 5xx responses.
It writes an evidence JSON file; screenshots are kept in a temporary directory.

Usage:

    DIGILEGACY_DB=/tmp/live.db DIGILEGACY_API_TOKEN=test \
      target/release/digilegacy-server
    cd web && npm run dev -- --port 5173 --strictPort
    python3 tools/visual/live_smoke.py
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from urllib.request import urlopen

try:
    from playwright.sync_api import sync_playwright
except ImportError as exc:  # pragma: no cover - operator feedback
    raise SystemExit("live smoke needs Playwright: python3 -m pip install playwright") from exc

ROOT = Path(__file__).resolve().parents[2]
AXE_SOURCE = ROOT / "web" / "node_modules" / "axe-core" / "axe.min.js"
FROZEN_CLOCK = "2026-11-20T10:00:00+08:00"


def now_utc() -> str:
    return datetime.now(timezone.utc).isoformat()


def chromium_path() -> str | None:
    configured = os.environ.get("CHROMIUM_PATH")
    if configured:
        return configured
    for name in ("chromium", "chromium-browser", "google-chrome"):
        found = shutil.which(name)
        if found:
            return found
    return None


def init_script(theme: str) -> str:
    return f"""
const RealDate = Date;
const fixed = new RealDate('{FROZEN_CLOCK}').getTime();
class FakeDate extends RealDate {{
  constructor(...args) {{ if (args.length === 0) super(fixed); else super(...args); }}
  static now() {{ return fixed; }}
}}
window.Date = FakeDate;
try {{
  localStorage.setItem('digilegacy.locale', 'zh-CN');
  localStorage.setItem('digilegacy.theme', '{theme}');
}} catch (error) {{}}
"""


def api_health(base: str, timeout: float = 5.0) -> dict:
    with urlopen(f"{base.rstrip('/')}/api/v1/health", timeout=timeout) as response:
        return json.loads(response.read().decode("utf-8"))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--web-url", default=os.environ.get("LIVE_WEB_URL", "http://127.0.0.1:5173"))
    parser.add_argument("--api-url", default=os.environ.get("LIVE_API_URL", "http://127.0.0.1:8787"))
    parser.add_argument(
        "--json-out",
        type=Path,
        default=ROOT / "docs" / "operations" / "measurements" / "live-browser-2026-11-20.json",
    )
    parser.add_argument("--keep-screenshots", type=Path, default=None)
    args = parser.parse_args()

    routes = [
        ("overview-light", "/overview", "light", 1280, 900),
        ("assets-light", "/assets", "light", 1280, 900),
        ("assets-discover-light", "/assets/discover", "light", 1280, 900),
        ("policies-light", "/policies", "light", 1280, 900),
        ("planning-light", "/planning", "light", 1280, 900),
        ("deliverables-light", "/deliverables", "light", 1280, 900),
        ("automation-light", "/automation", "light", 1280, 900),
        ("automation-tools-light", "/automation?tab=tools", "light", 1280, 900),
        ("automation-approvals-light", "/automation?tab=approvals", "light", 1280, 900),
        ("run-new-light", "/automation/runs/new", "light", 1280, 900),
        ("settings-connections-light", "/settings/connections", "light", 1280, 900),
        ("settings-security-light", "/settings/security", "light", 1280, 900),
        ("overview-dark", "/overview", "dark", 1280, 900),
        ("overview-mobile", "/overview", "dark", 390, 844),
    ]

    if not AXE_SOURCE.is_file():
        raise SystemExit(f"axe-core is not installed: {AXE_SOURCE}")
    axe_script = AXE_SOURCE.read_text()
    health = api_health(args.api_url)
    work = Path(tempfile.mkdtemp(prefix="digilegacy-live-smoke-"))
    if args.keep_screenshots is not None:
        args.keep_screenshots.mkdir(parents=True, exist_ok=True)
    results: list[dict] = []
    started = time.perf_counter()

    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(
            headless=True,
            executable_path=chromium_path(),
        )
        try:
            for name, path, theme, width, height in routes:
                page = browser.new_page(viewport={"width": width, "height": height}, device_scale_factor=1)
                page.add_init_script(init_script(theme))
                page.emulate_media(reduced_motion="reduce")
                route_started = time.perf_counter()
                console_errors: list[str] = []
                page_errors: list[str] = []
                api_errors: list[str] = []

                def on_console(message):
                    if message.type == "error":
                        # The dev server has no favicon; that is not a page failure.
                        if "Failed to load resource: the server responded with a status of 404" not in message.text:
                            console_errors.append(message.text[:300])

                def on_response(response):
                    if "/api/" in response.url and response.status >= 500:
                        api_errors.append(f"{response.status} {response.url.split('://', 1)[-1].split('/', 3)[-1]}")

                page.on("console", on_console)
                page.on("pageerror", lambda error: page_errors.append(str(error)[:300]))
                page.on("response", on_response)

                page.goto(f"{args.web_url.rstrip('/')}{path}", wait_until="domcontentloaded")
                page.wait_for_timeout(900)
                page.add_script_tag(content=axe_script)
                a11y = page.evaluate(
                    """async () => {
                        const result = await axe.run(document.body, {
                            runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'] }
                        });
                        return result.violations.map((violation) => ({
                            id: violation.id,
                            impact: violation.impact,
                            help: violation.help,
                            nodes: violation.nodes.length,
                            targets: violation.nodes.slice(0, 3).map((node) => node.target.join(' '))
                        }));
                    }"""
                )
                has_main = page.locator("main").count() == 1
                overflow = page.evaluate("document.documentElement.scrollWidth > window.innerWidth + 1")
                body_text = page.locator("body").inner_text().strip()
                screenshot = work / f"{name}.png"
                page.screenshot(path=str(screenshot), animations="disabled")
                if args.keep_screenshots is not None:
                    shutil.copyfile(screenshot, args.keep_screenshots / screenshot.name)
                page.close()

                ok = (
                    has_main
                    and not overflow
                    and not page_errors
                    and not api_errors
                    and not console_errors
                    and not a11y
                    and bool(body_text)
                )
                results.append(
                    {
                        "name": name,
                        "path": path,
                        "theme": theme,
                        "viewport": f"{width}x{height}",
                        "status": "PASS" if ok else "FAIL",
                        "durationMs": int((time.perf_counter() - route_started) * 1000),
                        "hasMain": has_main,
                        "horizontalOverflow": overflow,
                        "nonEmptyBody": bool(body_text),
                        "pageErrors": page_errors,
                        "apiErrors": api_errors,
                        "consoleErrors": console_errors,
                        "a11yViolations": a11y,
                    }
                )
        finally:
            browser.close()

    failures = [result for result in results if result["status"] != "PASS"]
    a11y_count = sum(len(result["a11yViolations"]) for result in results)
    evidence = {
        "generatedAt": now_utc(),
        "result": "PASS" if not failures else "FAIL",
        "webUrl": args.web_url,
        "apiUrl": args.api_url,
        "apiHealth": {
            "code": health.get("code"),
            "version": health.get("data", {}).get("version"),
            "schemaVersion": health.get("data", {}).get("schemaVersion"),
            "tables": health.get("data", {}).get("tables"),
        },
        "browser": {
            "engine": "chromium",
            "version": browser.version if "browser" in locals() else "unknown",
            "headless": True,
        },
        "routeCount": len(results),
        "totalDurationMs": int((time.perf_counter() - started) * 1000),
        "failures": len(failures),
        "a11yViolationCount": a11y_count,
        "routes": results,
        "notes": [
            "This is a real Chromium render against the Vue dev server and a real API, not the mock build.",
            "It runs axe WCAG A/AA/2.2 AA in a real browser, but does not replace a human screen-reader review; it catches page exceptions, API 5xx, horizontal overflow and DOM-level axe violations.",
            "Screenshots stay in a temporary directory by default and are not release artifacts.",
        ],
    }
    args.json_out.parent.mkdir(parents=True, exist_ok=True)
    args.json_out.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps({"result": evidence["result"], "routes": len(results), "failures": len(failures), "evidence": str(args.json_out)}, ensure_ascii=False))
    shutil.rmtree(work, ignore_errors=True)
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
