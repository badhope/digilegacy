#!/usr/bin/env python3
"""End-to-end DNS-rebinding drill for the pinned-address resolver.

Run under an isolated user/network/mount namespace:

    unshare -Urnm python3 tools/security/dns_rebind_drill.py

The drill starts a fake DNS server on 127.0.0.1:53 and a TCP service on
8.8.8.8. The Rust probe resolves `rebind.test` once, the DNS answer is changed
to a private address, and the probe must still connect to the originally
validated public address without a second DNS query. A second process then
resolves after the change and must be rejected.
"""

from __future__ import annotations

import argparse
import json
import os
import socket
import struct
import subprocess
import threading
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_PROBE = ROOT / "target" / "debug" / "examples" / "dns_rebind_probe"
DEFAULT_EVIDENCE = ROOT / "docs" / "operations" / "measurements" / "dns-rebind-2026-11-20.json"
PUBLIC_IP = "8.8.8.8"
PRIVATE_IP = "10.0.0.1"


class FakeDns:
    def __init__(self) -> None:
        self.mode = "public"
        self.a_queries = 0
        self.aaaa_queries = 0
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.sock.bind(("127.0.0.1", 53))
        self.thread = threading.Thread(target=self._serve, daemon=True)

    def start(self) -> None:
        self.thread.start()

    def _serve(self) -> None:
        while True:
            try:
                data, peer = self.sock.recvfrom(4096)
            except OSError:
                return
            try:
                response = self._answer(data)
            except Exception:
                continue
            if response:
                self.sock.sendto(response, peer)

    def _answer(self, data: bytes) -> bytes:
        if len(data) < 17:
            return b""
        tid = data[:2]
        q = data[12:]
        i = 0
        while i < len(q) and q[i] != 0:
            i += 1 + q[i]
        if i + 1 > len(q):
            return b""
        i += 1
        qtype, qclass = struct.unpack("!HH", q[i : i + 4])
        question = q[: i + 4]
        if qtype == 1:
            self.a_queries += 1
            address = PUBLIC_IP if self.mode == "public" else PRIVATE_IP
            answer = b"\xc0\x0c" + struct.pack("!HHIH", 1, 1, 0, 4) + socket.inet_aton(address)
            header = tid + b"\x81\x80" + struct.pack("!HHHH", 1, 1, 0, 0)
            return header + question + answer
        if qtype == 28:
            self.aaaa_queries += 1
            header = tid + b"\x81\x80" + struct.pack("!HHHH", 1, 0, 0, 0)
            return header + question
        return tid + b"\x81\x80" + struct.pack("!HHHH", 1, 0, 0, 0) + question


class TcpService:
    def __init__(self, port: int) -> None:
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.sock.bind((PUBLIC_IP, port))
        self.sock.listen(2)
        self.thread = threading.Thread(target=self._serve, daemon=True)

    def start(self) -> None:
        self.thread.start()

    def _serve(self) -> None:
        while True:
            try:
                conn, _ = self.sock.accept()
            except OSError:
                return
            with conn:
                conn.settimeout(2)
                try:
                    data = conn.recv(16)
                    if data == b"ping":
                        conn.sendall(b"pong")
                except OSError:
                    pass


def run(cmd: list[str], timeout: float = 10) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, text=True, capture_output=True, timeout=timeout)


def free_port() -> int:
    sock = socket.socket()
    sock.bind(("127.0.0.1", 0))
    port = sock.getsockname()[1]
    sock.close()
    return port


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--probe", type=Path, default=DEFAULT_PROBE)
    parser.add_argument("--evidence", type=Path, default=DEFAULT_EVIDENCE)
    parser.add_argument("--host", default="rebind.test")
    args = parser.parse_args()

    evidence: dict = {
        "generatedAt": datetime.now(timezone.utc).isoformat(),
        "result": "FAIL",
        "host": args.host,
        "steps": [],
        "network": {"publicAnswer": PUBLIC_IP, "privateAnswer": PRIVATE_IP},
    }

    def step(name: str, status: str, detail: str) -> None:
        evidence["steps"].append({"name": name, "status": status, "detail": detail})

    if not os.path.exists("/etc/resolv.conf"):
        print("missing /etc/resolv.conf")
        return 2

    # The caller must already be in `unshare -Urnm`; fail closed if not.
    try:
        run(["ip", "link", "set", "lo", "up"], timeout=2)
        run(["ip", "addr", "add", f"{PUBLIC_IP}/32", "dev", "lo"], timeout=2)
    except Exception as error:
        step("namespace", "fail", f"cannot configure loopback: {error}")
        args.evidence.write_text(json.dumps(evidence, indent=2) + "\n")
        print(json.dumps(evidence, indent=2))
        return 2

    resolv_path = Path("/tmp") / f"digilegacy-resolv-{os.getpid()}.conf"
    resolv_path.write_text("nameserver 127.0.0.1\noptions timeout:1 attempts:1\n")
    try:
        run(["mount", "--bind", str(resolv_path), "/etc/resolv.conf"], timeout=2)
    except Exception as error:
        step("resolver", "fail", f"cannot bind resolver config: {error}")
        args.evidence.write_text(json.dumps(evidence, indent=2) + "\n")
        return 2
    step("setup", "done", f"isolated resolver points at 127.0.0.1; {PUBLIC_IP} bound on loopback")

    if not args.probe.exists():
        step("probe", "fail", f"missing probe binary: {args.probe}")
        args.evidence.write_text(json.dumps(evidence, indent=2) + "\n")
        return 2

    dns = FakeDns()
    dns.start()
    port = free_port()
    service = TcpService(port)
    service.start()
    time.sleep(0.2)

    marker = Path("/tmp") / f"digilegacy-dns-marker-{os.getpid()}"
    if marker.exists():
        marker.unlink()

    started = time.perf_counter()
    proc = subprocess.Popen(
        [str(args.probe), "pin", args.host, str(port), str(marker), "300"],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    switched = False
    deadline = time.time() + 5
    while time.time() < deadline:
        if marker.exists():
            dns.mode = "private"
            switched = True
            break
        if proc.poll() is not None:
            break
        time.sleep(0.01)

    try:
        out, err = proc.communicate(timeout=8)
    except subprocess.TimeoutExpired:
        proc.kill()
        out, err = proc.communicate()

    elapsed = round((time.perf_counter() - started) * 1000, 1)
    if not switched:
        step("switch", "fail", "probe never signalled after its first resolution")
    elif proc.returncode != 0:
        step("pinned-connect", "fail", f"probe failed: {out.strip()} {err.strip()}")
    elif f"addrs=[{PUBLIC_IP}:{port}]" not in out or "connected=true" not in out or "reply=pong" not in out:
        step("pinned-connect", "fail", f"unexpected probe output: {out.strip()}")
    elif dns.a_queries != 1:
        step("pinned-connect", "fail", f"expected one A query, saw {dns.a_queries}")
    else:
        step(
            "pinned-connect",
            "done",
            f"resolved once to {PUBLIC_IP}:{port}; DNS changed to {PRIVATE_IP}; probe still connected to pinned address; A queries={dns.a_queries}; {elapsed}ms",
        )

    block = run([str(args.probe), "block", args.host, str(port)], timeout=5)
    if block.returncode == 0 and "blocked=true" in block.stdout:
        step("rebound-reject", "done", block.stdout.strip())
    else:
        step("rebound-reject", "fail", f"expected rejection: {block.stdout.strip()} {block.stderr.strip()}")

    evidence["result"] = "PASS" if all(s["status"] == "done" for s in evidence["steps"]) else "FAIL"
    evidence["probe"] = str(args.probe)
    evidence["tcpPort"] = port
    evidence["dnsQueries"] = {"a": dns.a_queries, "aaaa": dns.aaaa_queries}
    evidence["durationMs"] = round((time.perf_counter() - started) * 1000, 1)
    args.evidence.parent.mkdir(parents=True, exist_ok=True)
    args.evidence.write_text(json.dumps(evidence, indent=2) + "\n")
    print(json.dumps({"result": evidence["result"], "evidence": str(args.evidence), "steps": evidence["steps"]}, ensure_ascii=False))
    return 0 if evidence["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
