#!/usr/bin/env python3
"""Clean-host installation drill for a DigiLegacy release archive.

The drill uses a fresh temporary HOME, prefix and data directory, an explicit
minimal environment and a free loopback port. It installs, starts, probes
health/metrics, exports diagnostics, creates and verifies a checkpoint,
uninstalls while preserving data, reinstalls to prove the preserved database is
readable, then purges the data directory.

No credential is printed or written to the evidence file.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import socket
import stat
import subprocess
import tarfile
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from urllib.request import urlopen

ROOT = Path(__file__).resolve().parents[2]


def now_utc() -> str:
    return datetime.now(timezone.utc).isoformat()


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def parse_env(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    for raw in path.read_text().splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        values[key.strip()] = value.strip()
    return values


def run(cmd: list[str], *, env: dict[str, str], cwd: Path | None = None) -> tuple[str, int]:
    started = time.perf_counter()
    completed = subprocess.run(
        cmd,
        cwd=str(cwd) if cwd else None,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=120,
    )
    elapsed = int((time.perf_counter() - started) * 1000)
    if completed.returncode != 0:
        raise RuntimeError(
            f"command failed ({completed.returncode}): {' '.join(cmd)}\n{completed.stdout[-2000:]}"
        )
    return completed.stdout, elapsed


def http_json(url: str, timeout: float = 5.0) -> dict:
    with urlopen(url, timeout=timeout) as response:
        return json.loads(response.read().decode("utf-8"))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--archive",
        type=Path,
        default=ROOT / "packages" / "digilegacy-0.0.19-linux-x86_64.tar.gz",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        default=ROOT / "docs" / "operations" / "measurements" / "install-2026-11-20.json",
    )
    parser.add_argument("--work-dir", type=Path, default=None)
    args = parser.parse_args()

    archive = args.archive.resolve()
    if not archive.is_file():
        raise SystemExit(f"archive not found: {archive}")

    work = Path(tempfile.mkdtemp(prefix="digilegacy-clean-drill-")) if args.work_dir is None else args.work_dir.resolve()
    if work.exists() and args.work_dir is not None:
        shutil.rmtree(work)
    work.mkdir(parents=True, exist_ok=True)
    home = work / "home"
    prefix = work / "prefix"
    data = work / "data"
    home.mkdir(mode=0o700)

    env = {
        "PATH": os.environ.get("PATH", "/usr/local/bin:/usr/bin:/bin"),
        "HOME": str(home),
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
        "TZ": "UTC",
        "TMPDIR": str(work / "tmp"),
    }
    (work / "tmp").mkdir(parents=True, exist_ok=True)

    steps: list[dict] = []
    checks: dict = {}
    started = time.perf_counter()

    try:
        # Extract separately so the package root is available to the next steps.
        extract_started = time.perf_counter()
        with tarfile.open(archive, "r:gz") as tar:
            for member in tar.getmembers():
                target = (work / member.name).resolve()
                if not str(target).startswith(str(work.resolve()) + os.sep) and target != work.resolve():
                    raise RuntimeError(f"archive member escapes the work directory: {member.name}")
            tar.extractall(work, filter="data")
        package_dirs = [path for path in work.iterdir() if path.is_dir() and (path / "install.sh").is_file()]
        if len(package_dirs) != 1:
            raise RuntimeError(f"expected one unpacked package, found {len(package_dirs)}")
        package = package_dirs[0]
        steps.append(
            {
                "name": "extract",
                "status": "done",
                "durationMs": int((time.perf_counter() - extract_started) * 1000),
                "detail": f"unpacked {archive.name}",
            }
        )

        port = free_port()
        bind = f"127.0.0.1:{port}"
        install_output, install_ms = run(
            [
                str(package / "install.sh"),
                "--prefix",
                str(prefix),
                "--data-dir",
                str(data),
                "--bind",
                bind,
            ],
            env=env,
            cwd=package,
        )
        if "DIGILEGACY_API_TOKEN" in install_output:
            raise RuntimeError("installer exposed the write token in its output")
        expected_modes = {
            "web/index.html": 0o644,
            "share/nginx/digilegacy.conf.example": 0o644,
            "docs/operations/INSTALL.md": 0o644,
            "tools/policies/source_audit.py": 0o755,
            "etc/digilegacy.env": 0o600,
        }
        actual_modes = {
            relative: stat.S_IMODE((prefix / relative).stat().st_mode)
            for relative in expected_modes
        }
        checks["permissions"] = {
            "expected": {key: oct(value) for key, value in expected_modes.items()},
            "actual": {key: oct(value) for key, value in actual_modes.items()},
        }
        if actual_modes != expected_modes:
            raise RuntimeError(f"unexpected installed modes: {checks['permissions']}")
        steps.append({"name": "install", "status": "done", "durationMs": install_ms, "detail": "fresh prefix and data"})

        start_output, start_ms = run([str(prefix / "bin" / "digilegacy-start")], env=env)
        steps.append({"name": "start", "status": "done", "durationMs": start_ms, "detail": "health probe inside start script"})

        base = f"http://127.0.0.1:{port}"
        health = http_json(f"{base}/api/v1/health")
        metrics = http_json(f"{base}/api/v1/metrics")
        checks["health"] = {
            "code": health.get("code"),
            "version": health.get("data", {}).get("version"),
            "schemaVersion": metrics.get("data", {}).get("store", {}).get("schemaVersion"),
            "tables": metrics.get("data", {}).get("store", {}).get("tables"),
        }
        if health.get("code") != "OK" or not checks["health"]["version"]:
            raise RuntimeError(f"health returned {health.get('code')}")
        if checks["health"]["schemaVersion"] != 19 or checks["health"]["tables"] != 32:
            raise RuntimeError(f"unexpected store shape: {checks['health']}")
        steps.append({"name": "health", "status": "done", "durationMs": 0, "detail": "HTTP /api/v1/health and /api/v1/metrics"})

        env_file = prefix / "etc" / "digilegacy.env"
        runtime_env = dict(env)
        runtime_env.update(parse_env(env_file))
        cli = str(prefix / "bin" / "digilegacy-cli")
        diag_output, diag_ms = run([cli, "diagnostics", "export", "--allow-write", "--json"], env=runtime_env)
        diag = json.loads(diag_output)
        audit = diag.get("data", {}).get("audit", {})
        checks["diagnostics"] = {
            "code": diag.get("code"),
            "auditValid": audit.get("valid"),
            "schemaVersion": diag.get("data", {}).get("store", {}).get("schema_version"),
        }
        if diag.get("code") != "OK" or audit.get("valid") is not True:
            raise RuntimeError(f"diagnostics failed: {checks['diagnostics']}")
        steps.append({"name": "diagnostics-export", "status": "done", "durationMs": diag_ms, "detail": "sanitized export with valid audit chain"})

        checkpoint_output, checkpoint_ms = run(
            [cli, "security", "checkpoint", "--label", "clean-drill", "--json"], env=runtime_env
        )
        checkpoint = json.loads(checkpoint_output)
        checkpoint_id = checkpoint.get("data", {}).get("id")
        if checkpoint.get("code") != "OK" or not checkpoint_id:
            raise RuntimeError(f"checkpoint failed: {checkpoint}")
        verify_output, verify_ms = run([cli, "security", "verify", str(checkpoint_id), "--json"], env=runtime_env)
        verify = json.loads(verify_output)
        checks["checkpoint"] = {"id": checkpoint_id, "verified": verify.get("code") == "OK"}
        if not checks["checkpoint"]["verified"]:
            raise RuntimeError(f"checkpoint verification failed: {verify}")
        steps.append({"name": "checkpoint", "status": "done", "durationMs": checkpoint_ms + verify_ms, "detail": "created and verified"})

        stop_output, stop_ms = run([str(prefix / "bin" / "digilegacy-stop")], env=env)
        steps.append({"name": "stop", "status": "done", "durationMs": stop_ms, "detail": "SIGTERM"})

        uninstall_output, uninstall_ms = run([str(prefix / "bin" / "digilegacy-uninstall")], env=env)
        if prefix.exists():
            raise RuntimeError("prefix still exists after uninstall")
        if not data.exists() or not (data / "keys" / "db.key").is_file():
            raise RuntimeError("data or database keys were not preserved")
        checks["uninstall"] = {
            "prefixRemoved": True,
            "dataPreserved": True,
            "keysPreserved": True,
        }
        steps.append({"name": "uninstall-preserve-data", "status": "done", "durationMs": uninstall_ms, "detail": "prefix removed; database and keys preserved"})

        reinstall_output, reinstall_ms = run(
            [
                str(package / "install.sh"),
                "--prefix",
                str(prefix),
                "--data-dir",
                str(data),
                "--bind",
                bind,
            ],
            env=env,
            cwd=package,
        )
        if "DIGILEGACY_API_TOKEN" in reinstall_output:
            raise RuntimeError("installer exposed the write token on reinstall")
        steps.append({"name": "reinstall", "status": "done", "durationMs": reinstall_ms, "detail": "same data directory and preserved keys"})

        start_output, start_ms = run([str(prefix / "bin" / "digilegacy-start")], env=env)
        health_again = http_json(f"{base}/api/v1/health")
        runtime_env = dict(env)
        runtime_env.update(parse_env(prefix / "etc" / "digilegacy.env"))
        diag_again_output, diag_again_ms = run(
            [str(prefix / "bin" / "digilegacy-cli"), "diagnostics", "export", "--allow-write", "--json"],
            env=runtime_env,
        )
        diag_again = json.loads(diag_again_output)
        checks["reinstall"] = {
            "healthCode": health_again.get("code"),
            "auditValid": diag_again.get("data", {}).get("audit", {}).get("valid"),
        }
        if health_again.get("code") != "OK" or checks["reinstall"]["auditValid"] is not True:
            raise RuntimeError(f"preserved database did not reopen: {checks['reinstall']}")
        steps.append({"name": "reinstall-health", "status": "done", "durationMs": start_ms + diag_again_ms, "detail": "preserved database reopened and audit chain valid"})

        stop_output, stop_ms = run([str(prefix / "bin" / "digilegacy-stop")], env=env)
        steps.append({"name": "stop-again", "status": "done", "durationMs": stop_ms, "detail": "SIGTERM"})

        uninstall_purge_output, purge_ms = run(
            [str(prefix / "bin" / "digilegacy-uninstall"), "--purge-data"], env=env
        )
        if prefix.exists() or data.exists():
            raise RuntimeError("prefix or data still exists after purge")
        checks["purge"] = {"prefixRemoved": True, "dataRemoved": True}
        steps.append({"name": "uninstall-purge", "status": "done", "durationMs": purge_ms, "detail": "prefix and data removed"})

        archive_digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        evidence = {
            "generatedAt": now_utc(),
            "result": "PASS",
            "cleanEnvironment": {
                "freshHome": True,
                "freshPrefix": True,
                "freshDataDirectory": True,
                "minimalEnvironment": ["PATH", "HOME", "LANG", "LC_ALL", "TZ", "TMPDIR"],
                "loopbackBind": bind,
            },
            "archive": {
                "name": archive.name,
                "bytes": archive.stat().st_size,
                "sha256": archive_digest,
            },
            "host": {
                "platform": platform.platform(),
                "machine": platform.machine(),
            },
            "totalDurationMs": int((time.perf_counter() - started) * 1000),
            "steps": steps,
            "checks": checks,
            "notes": [
                "The Web SPA is included but not served by the API; a reverse proxy is required for the console.",
                "This drill proves install, start, diagnostics export, checkpoint verification, uninstall, key preservation and reinstall.",
                "Real SMTP/Webhook delivery latency and human screen-reader review remain external acceptance items.",
            ],
        }
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + "\n")
        print(json.dumps({"result": "PASS", "evidence": str(args.json_out), "steps": len(steps)}, ensure_ascii=False))
        return 0
    except Exception as error:  # noqa: BLE001 - write failure evidence before exiting
        evidence = {
            "generatedAt": now_utc(),
            "result": "FAIL",
            "error": str(error),
            "steps": steps,
            "checks": checks,
        }
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + "\n")
        print(json.dumps({"result": "FAIL", "error": str(error), "evidence": str(args.json_out)}, ensure_ascii=False))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
