#!/usr/bin/env python3
"""Machine-assisted pre-check for the policy source library.

This does not verify that a page states the policy in the `data_source` field.
It only reports:

- whether a URL is present and HTTPS;
- whether it is a platform root or a deep policy link;
- duplicate URLs shared by several platforms;
- HTTP reachability (2xx/3xx, blocked by 401/403/429, or broken).

The output is a worklist for a human reviewer. The product data stays marked
`unverified`; this tool must never be treated as a verification write.
"""

from __future__ import annotations

import argparse
import csv
import json
import time
from collections import Counter
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import urlparse

import requests

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_INPUT = ROOT / "content" / "policies" / "platforms.json"
DEFAULT_JSON = ROOT / "docs" / "operations" / "measurements" / "policy-sources-2026-11-20.json"
DEFAULT_CSV = ROOT / "docs" / "operations" / "policy-source-worklist.csv"
USER_AGENT = "DigiLegacyPolicyAudit/0.0.19"


def now_utc() -> str:
    return datetime.now(timezone.utc).isoformat()


def check_url(url: str, timeout: float) -> dict:
    if not url:
        return {"httpStatus": None, "finalUrl": "", "reachability": "missing", "error": "URL 为空"}
    try:
        response = requests.get(
            url,
            timeout=timeout,
            allow_redirects=True,
            stream=True,
            headers={"User-Agent": USER_AGENT, "Accept": "text/html,application/xhtml+xml"},
        )
        status = response.status_code
        final_url = response.url
        response.close()
        if 200 <= status < 400:
            reachability = "reachable"
        elif status in (401, 403, 429):
            reachability = "blocked"
        else:
            reachability = "broken"
        return {
            "httpStatus": status,
            "finalUrl": final_url,
            "reachability": reachability,
            "error": "",
        }
    except requests.Timeout:
        return {"httpStatus": None, "finalUrl": "", "reachability": "timeout", "error": "请求超时"}
    except requests.RequestException as error:
        return {
            "httpStatus": None,
            "finalUrl": "",
            "reachability": "error",
            "error": str(error)[:200],
        }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, default=DEFAULT_INPUT)
    parser.add_argument("--json-out", type=Path, default=DEFAULT_JSON)
    parser.add_argument("--csv-out", type=Path, default=DEFAULT_CSV)
    parser.add_argument("--timeout", type=float, default=8.0)
    parser.add_argument("--workers", type=int, default=8)
    parser.add_argument("--no-network", action="store_true", help="skip HTTP probes and only classify URLs")
    args = parser.parse_args()

    items = json.loads(args.input.read_text())
    if not isinstance(items, list):
        raise SystemExit("platforms.json must be a list")

    urls = [str(item.get("official_policy_url", "")).strip() for item in items]
    duplicate_counts = Counter(url for url in urls if url)
    entries = []
    started = time.perf_counter()

    for item, url in zip(items, urls):
        parsed = urlparse(url)
        path = parsed.path.rstrip("/")
        url_kind = "root" if path == "" else "deep-link"
        if not url:
            url_kind = "missing"
        entries.append(
            {
                "id": item.get("id"),
                "name": item.get("name", ""),
                "nameEn": item.get("name_en", ""),
                "url": url,
                "scheme": parsed.scheme,
                "host": parsed.netloc,
                "urlKind": url_kind,
                "duplicateCount": duplicate_counts.get(url, 0) if url else 0,
                "hasDataSource": bool(str(item.get("data_source", "")).strip()),
                "httpStatus": None,
                "finalUrl": "",
                "reachability": "not-checked",
                "error": "",
            }
        )

    if args.no_network:
        for entry in entries:
            entry["reachability"] = "not-checked" if entry["url"] else "missing"
    else:
        unique_urls = sorted({entry["url"] for entry in entries if entry["url"]})
        results: dict[str, dict] = {}
        with ThreadPoolExecutor(max_workers=max(1, args.workers)) as pool:
            futures = {pool.submit(check_url, url, args.timeout): url for url in unique_urls}
            for future in as_completed(futures):
                url = futures[future]
                results[url] = future.result()
        for entry in entries:
            entry.update(results.get(entry["url"], {"httpStatus": None, "finalUrl": "", "reachability": "missing", "error": "URL 为空"}))

    counts = Counter(entry["reachability"] for entry in entries)
    url_kinds = Counter(entry["urlKind"] for entry in entries)
    duplicate_groups = [
        {"url": url, "count": count, "platforms": [e["name"] for e in entries if e["url"] == url]}
        for url, count in duplicate_counts.items()
        if count > 1
    ]
    duplicate_groups.sort(key=lambda group: (-group["count"], group["url"]))
    review_count = sum(
        1
        for entry in entries
        if entry["urlKind"] != "deep-link"
        or entry["duplicateCount"] > 1
        or entry["reachability"] != "reachable"
    )

    evidence = {
        "generatedAt": now_utc(),
        "result": "PRECHECK-ONLY",
        "sourceStatus": "unverified",
        "input": str(args.input),
        "policyCount": len(entries),
        "urlPresence": {
            "present": sum(1 for entry in entries if entry["url"]),
            "missing": sum(1 for entry in entries if not entry["url"]),
            "https": sum(1 for entry in entries if entry["scheme"] == "https"),
        },
        "urlKind": {
            "root": url_kinds.get("root", 0),
            "deepLink": url_kinds.get("deep-link", 0),
            "missing": url_kinds.get("missing", 0),
        },
        "duplicates": {
            "groups": len(duplicate_groups),
            "entries": sum(1 for entry in entries if entry["duplicateCount"] > 1),
            "groupsDetail": duplicate_groups,
        },
        "reachability": {
            "reachable": counts.get("reachable", 0),
            "blocked": counts.get("blocked", 0),
            "broken": counts.get("broken", 0),
            "timeout": counts.get("timeout", 0),
            "error": counts.get("error", 0),
            "notChecked": counts.get("not-checked", 0),
            "missing": counts.get("missing", 0),
        },
        "needsHumanReview": review_count,
        "durationMs": int((time.perf_counter() - started) * 1000),
        "notes": [
            "PRECHECK-ONLY: reachability is not verification and does not confirm that a page contains the cited rule.",
            "Root URLs are platform home pages, not policy deep links; they need a specific official page before review.",
            "A blocked status (401/403/429) may be anti-bot behavior, not a dead source; a reviewer should open it manually.",
            "The product policy library remains unverified; no content/policies data is changed by this tool.",
        ],
        "entries": entries,
    }

    args.json_out.parent.mkdir(parents=True, exist_ok=True)
    args.json_out.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + "\n")

    args.csv_out.parent.mkdir(parents=True, exist_ok=True)
    with args.csv_out.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(
            ["id", "name", "url", "url_kind", "duplicate_count", "http_status", "reachability", "note"]
        )
        for entry in entries:
            notes = []
            if entry["urlKind"] == "root":
                notes.append("平台首页，需补具体政策深链")
            elif entry["urlKind"] == "missing":
                notes.append("缺 URL")
            if entry["duplicateCount"] > 1:
                notes.append("与其他平台重复")
            if entry["error"]:
                notes.append(entry["error"])
            note = "; ".join(notes)
            writer.writerow(
                [
                    entry["id"],
                    entry["name"],
                    entry["url"],
                    entry["urlKind"],
                    entry["duplicateCount"],
                    entry["httpStatus"] or "",
                    entry["reachability"],
                    note,
                ]
            )

    print(
        json.dumps(
            {
                "result": evidence["result"],
                "policies": evidence["policyCount"],
                "root": evidence["urlKind"]["root"],
                "deepLink": evidence["urlKind"]["deepLink"],
                "duplicateGroups": evidence["duplicates"]["groups"],
                "reachable": evidence["reachability"]["reachable"],
                "review": evidence["needsHumanReview"],
                "json": str(args.json_out),
                "csv": str(args.csv_out),
            },
            ensure_ascii=False,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
