# Visual regression

Nine fixed-viewport snapshots of the Web console, stored as lossy WebP. The
baseline is about 270 KB, not the multi-megabyte screenshot set that was removed
from this repository earlier.

## Why this is a manual gate

The normal `tools/verify.sh` stays free of browser downloads and native image
dependencies. This check runs when a release candidate needs a visual review:

- Chromium 146 (or a compatible version)
- Python 3.10+
- `pillow` and `playwright`

It is intentionally not wired into CI. A browser-version change can shift
anti-aliasing, so a failure is evidence for a human to inspect, not an automatic
blocker on every pull request.

## Run

```bash
cd web
VITE_USE_MOCK=true npm run build
npm run preview -- --port 4173 --strictPort
```

In a second shell, from the repository root:

```bash
python3 tools/visual/regress.py
python3 tools/visual/regress.py --update
```

`--update` replaces the baseline. Only do that after opening the current and
baseline images and confirming the change is intended. The script writes
per-shot differences to `tools/visual/artifacts/` on failure; that directory is
ignored by Git.

## What is covered

- Desktop light: overview, assets, policies, planning, deliverables, automation,
  settings workspace
- Desktop dark: overview
- Mobile light: overview at 390 x 844

The clock is frozen at `2026-11-20T10:00:00+08:00`, the locale is `zh-CN`, and
the theme is set before the page loads. Those choices keep relative timestamps
and locale-specific copy stable. The rail version label is hidden before each
shot so a patch release does not invalidate the baseline.

## Difference policy

Each shot may differ on at most 0.4% of pixels, with a per-channel tolerance of
10. The first capture compares lossy WebP against lossy WebP, so a small
re-encode drift (well under 0.1%) is expected. A real layout or copy change
moves far more pixels and should be reviewed.

Do not refresh the baseline to hide a regression. If a change is intentional,
update the baseline in the same commit as the UI change and say which shots
changed in the commit message.
