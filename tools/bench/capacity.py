#!/usr/bin/env python3
"""Reference capacity drill for the SLO read/write latency targets.

The server is expected to be a throwaway instance seeded by
`apps/server/examples/seed_reference.rs`. Reads hit the real HTTP surface;
writes create scratch assets through the same invoke endpoint used by the
console. No production database is touched.
"""
from __future__ import annotations

import argparse
import concurrent.futures
import json
import math
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

READ_PATHS = [
    "/api/v1/health",
    "/api/v1/overview",
    "/api/v1/assets?limit=50",
    "/api/v1/plan",
    "/api/v1/audit?limit=50",
    "/api/v1/security",
    "/api/v1/metrics",
    "/api/v1/assets?limit=50",
    "/api/v1/audit?limit=50",
]


def request(base: str, path: str, token: str | None = None, body: dict | None = None):
    data = None if body is None else json.dumps(body).encode("utf-8")
    req = urllib.request.Request(base + path, data=data, method="POST" if body is not None else "GET")
    if body is not None:
        req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    started = time.perf_counter()
    try:
        with urllib.request.urlopen(req, timeout=15) as response:
            payload = response.read()
            elapsed = (time.perf_counter() - started) * 1000
            return response.status, payload, elapsed
    except urllib.error.HTTPError as error:
        elapsed = (time.perf_counter() - started) * 1000
        return error.code, error.read(), elapsed


def percentile(values: list[float], percent: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    index = max(0, math.ceil(percent / 100 * len(ordered)) - 1)
    return round(ordered[index], 2)


def summarize(values: list[float], errors: int):
    return {
        "count": len(values) + errors,
        "ok": len(values),
        "errors": errors,
        "p50Ms": percentile(values, 50),
        "p95Ms": percentile(values, 95),
        "p99Ms": percentile(values, 99),
        "maxMs": round(max(values), 2) if values else 0,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", default="http://127.0.0.1:8799")
    parser.add_argument("--token", default="capacity-token")
    parser.add_argument("--reads", type=int, default=1200)
    parser.add_argument("--writes", type=int, default=120)
    parser.add_argument("--concurrency", type=int, default=8)
    parser.add_argument("--out", default="docs/operations/measurements/capacity-latest.json")
    parser.add_argument("--strict", action="store_true")
    args = parser.parse_args()
    base = args.base_url.rstrip("/")
    read_budget = {"p95Ms": 250, "p99Ms": 800}
    write_budget = {"p95Ms": 500}

    # Record the dataset actually under test before warming anything up.
    overview = json.loads(request(base, "/api/v1/overview", args.token)[1])["data"]
    audit = json.loads(request(base, "/api/v1/audit?limit=1", args.token)[1])["data"]
    dataset = {"assets": overview["assetsTotal"], "audit": audit["total"]}

    # Warm up every read path once; fail fast if the temporary server is not ready.
    for path in READ_PATHS:
        status, _, _ = request(base, path, args.token)
        if status != 200:
            raise SystemExit(f"warmup failed: {path} -> {status}")

    def read_job(index: int):
        path = READ_PATHS[index % len(READ_PATHS)]
        status, payload, elapsed = request(base, path, args.token)
        return status, elapsed, payload

    read_latencies: list[float] = []
    read_errors = 0
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.concurrency) as pool:
        for status, elapsed, _ in pool.map(read_job, range(args.reads)):
            if status == 200:
                read_latencies.append(elapsed)
            else:
                read_errors += 1

    def write_job(index: int):
        body = {
            "name": "assets.create",
            "args": {"name": f"容量写入 {time.time_ns()}-{index}", "kind": "account"},
        }
        status, payload, elapsed = request(base, "/api/v1/invoke", args.token, body)
        ok = status == 200 and json.loads(payload).get("success") is True
        return ok, elapsed

    write_latencies: list[float] = []
    write_errors = 0
    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, args.concurrency // 2)) as pool:
        for ok, elapsed in pool.map(write_job, range(args.writes)):
            if ok:
                write_latencies.append(elapsed)
            else:
                write_errors += 1

    metrics_before_status, metrics_before_body, _ = request(base, "/api/v1/metrics", args.token)
    metrics_before = json.loads(metrics_before_body)["data"] if metrics_before_status == 200 else None

    reads = summarize(read_latencies, read_errors)
    writes = summarize(write_latencies, write_errors)
    passed = (
        reads["errors"] == 0
        and writes["errors"] == 0
        and reads["p95Ms"] <= read_budget["p95Ms"]
        and reads["p99Ms"] <= read_budget["p99Ms"]
        and writes["p95Ms"] <= write_budget["p95Ms"]
    )
    result = {
        "generatedAt": datetime.now(timezone.utc).isoformat(),
        "baseUrl": base,
        "concurrency": args.concurrency,
        "dataset": dataset,
        "reads": reads,
        "writes": writes,
        "budgets": {"reads": read_budget, "writes": write_budget},
        "passed": passed,
        "serverMetrics": metrics_before,
    }
    print(json.dumps(result, ensure_ascii=False, indent=2))
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if args.strict and not passed:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
