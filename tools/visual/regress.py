#!/usr/bin/env python3
"""Lightweight visual regression for the Web console.

Captures a small set of fixed-viewport WebP snapshots against a mock build and
compares them with the committed baseline. This is a manual release gate, not
part of `tools/verify.sh`, so the normal build stays free of Playwright.

Requirements: Python 3.10+, Pillow, Playwright, and a Chromium binary.
Start the app first:

    cd web
    VITE_USE_MOCK=true npm run build
    npm run preview -- --port 4173 --strictPort

Then run from the repository root:

    python3 tools/visual/regress.py            # check
    python3 tools/visual/regress.py --update   # refresh baseline
"""

from __future__ import annotations

import argparse
import os
import shutil
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

try:
    from PIL import Image, ImageChops
    from playwright.sync_api import sync_playwright
except ImportError as exc:  # pragma: no cover - operator feedback
    raise SystemExit(
        "visual regression needs Pillow and Playwright: "
        "python3 -m pip install pillow playwright"
    ) from exc

ROOT = Path(__file__).resolve().parents[2]
BASELINE_DIR = Path(__file__).resolve().parent / "baseline"
ARTIFACT_DIR = Path(__file__).resolve().parent / "artifacts"
FROZEN_CLOCK = "2026-11-20T10:00:00+08:00"
MAX_DIFF_RATIO = 0.004
CHANNEL_TOLERANCE = 10


@dataclass(frozen=True)
class Shot:
    name: str
    path: str
    width: int = 1200
    height: int = 800
    theme: str = "light"


SHOTS: tuple[Shot, ...] = (
    Shot("overview-light", "/overview"),
    Shot("assets-light", "/assets"),
    Shot("policies-light", "/policies"),
    Shot("planning-light", "/planning"),
    Shot("deliverables-light", "/deliverables"),
    Shot("automation-light", "/automation"),
    Shot("settings-light", "/settings/workspace"),
    Shot("overview-dark", "/overview", theme="dark"),
    Shot("overview-mobile", "/overview", width=390, height=844),
)

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
}} catch (error) {{
  // about:blank has no storage; the real page gets the same script again.
}}
"""


def chromium_path() -> str | None:
    configured = os.environ.get("CHROMIUM_PATH")
    if configured:
        return configured
    for name in ("chromium", "chromium-browser", "google-chrome"):
        found = shutil.which(name)
        if found:
            return found
    return None


def capture(base_url: str, output: Path) -> None:
    executable = chromium_path()
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(
            headless=True,
            executable_path=executable,
        )
        try:
            for shot in SHOTS:
                page = browser.new_page(
                    viewport={"width": shot.width, "height": shot.height},
                    device_scale_factor=1,
                )
                page.add_init_script(init_script(shot.theme))
                page.emulate_media(reduced_motion="reduce")
                page.goto(f"{base_url.rstrip('/')}{shot.path}", wait_until="networkidle")
                page.wait_for_timeout(1100)
                page.add_style_tag(
                    content=".rail-meta span:first-child { visibility: hidden !important; }"
                )
                target = output / f"{shot.name}.webp"
                page.screenshot(
                    path=str(target),
                    full_page=False,
                    animations="disabled",
                )
                image = Image.open(target).convert("RGB")
                image.save(target, "WEBP", quality=82, method=6)
                print(f"captured {shot.name}: {target.stat().st_size} bytes")
                page.close()
        finally:
            browser.close()


def compare(expected_path: Path, actual_path: Path, artifact_path: Path | None) -> tuple[bool, float]:
    expected = Image.open(expected_path).convert("RGB")
    actual = Image.open(actual_path).convert("RGB")
    if expected.size != actual.size:
        return False, 1.0
    diff = ImageChops.difference(expected, actual)
    histogram = diff.convert("L").histogram()
    changed = sum(histogram[CHANNEL_TOLERANCE + 1 :])
    total = expected.width * expected.height
    ratio = changed / total
    if artifact_path is not None and ratio > MAX_DIFF_RATIO:
        artifact_path.parent.mkdir(parents=True, exist_ok=True)
        diff.save(artifact_path)
    return ratio <= MAX_DIFF_RATIO, ratio


def main() -> int:
    parser = argparse.ArgumentParser(description="DigiLegacy visual regression")
    parser.add_argument("--update", action="store_true", help="replace the baseline")
    parser.add_argument("--url", default=os.environ.get("VISUAL_BASE_URL", "http://127.0.0.1:4173"))
    args = parser.parse_args()

    if not args.update and not BASELINE_DIR.exists():
        print("no baseline; run with --update first", file=sys.stderr)
        return 2

    with tempfile.TemporaryDirectory(prefix="digilegacy-visual-") as temp:
        current = Path(temp)
        capture(args.url, current)

        if args.update:
            BASELINE_DIR.mkdir(parents=True, exist_ok=True)
            for shot in SHOTS:
                source = current / f"{shot.name}.webp"
                shutil.copyfile(source, BASELINE_DIR / source.name)
            print(f"baseline updated: {len(SHOTS)} shots in {BASELINE_DIR}")
            return 0

        failures = 0
        for shot in SHOTS:
            actual = current / f"{shot.name}.webp"
            expected = BASELINE_DIR / f"{shot.name}.webp"
            if not expected.exists():
                print(f"FAIL {shot.name}: baseline missing")
                failures += 1
                continue
            ok, ratio = compare(expected, actual, ARTIFACT_DIR / f"{shot.name}-diff.png")
            print(f"{'OK  ' if ok else 'FAIL'} {shot.name}: {ratio:.3%} changed")
            if not ok:
                failures += 1
        if failures:
            print(f"visual regression: {failures} shot(s) changed", file=sys.stderr)
            return 1
        print("visual regression: OK")
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
