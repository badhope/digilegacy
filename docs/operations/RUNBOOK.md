# Runbook

This runbook assumes a single-node deployment with the database, key files and
CLI on the same host. Commands use `digilegacy-cli`; from a source checkout the
equivalent is `cargo run -q -p digilegacy-cli --`.

## Environment

Set these variables in a root-owned `0600` environment file. Do not put secrets
in a repository, shell history or the unit file.

| Variable | Required | Meaning |
|---|---|---|
| `DIGILEGACY_DB` | Recommended | Absolute database path, e.g. `/var/lib/digilegacy/digilegacy.db`. Defaults to `data/digilegacy.db` relative to the working directory. |
| `DIGILEGACY_DB_KEY_FILE` | For encrypted deployments | 32-byte SQLCipher key file. Losing it makes the database unreadable; back it up separately from the database. |
| `DIGILEGACY_CONFIG_KEY_FILE` | Optional | Separate key for encrypted connection config. If absent, a domain key is derived from the database key. |
| `DIGILEGACY_BIND` | Yes | API bind address, default `127.0.0.1:8787`. Use a reverse proxy for remote access. |
| `DIGILEGACY_API_TOKEN` | For writes | Bearer token required by write routes. Without it, write routes stay closed. |
| `DIGILEGACY_ALLOW_PRIVATE_NETWORK` | Only for LAN integrations | `1`, `true` or `yes` allows private-network outbound URLs after DNS resolution. Cloud metadata remains blocked. |
| `DIGILEGACY_OUTBOUND_ALLOWLIST` | Optional | Comma-separated host allowlist, e.g. `smtp.example.com,*.feishu.cn`. When set, all other outbound hosts are refused before DNS resolution. |
| `DIGILEGACY_POLICY_URL` / `TOKEN` | Optional | Fallback remote policy feed. Prefer a saved `policy` connection. |
| `DIGILEGACY_SMTP_*`, `DIGILEGACY_WEBHOOK_*` | Optional | Fallback notification endpoints. Prefer saved connections. |

### Outbound network

Connection tests, policy sync and SMTP delivery resolve the destination once,
check every returned address, then connect to that pinned set. This closes the
DNS-rebinding window between validation and connect. Private addresses require
`DIGILEGACY_ALLOW_PRIVATE_NETWORK`; cloud metadata addresses are always refused.
The outbound clients ignore `HTTP_PROXY` / `HTTPS_PROXY` and `NO_PROXY` on
purpose, because a proxy would resolve the target again. If the deployment
requires an egress proxy, terminate the connection at a controlled proxy and
allowlist the proxy separately; do not point these clients at one.

Set `DIGILEGACY_OUTBOUND_ALLOWLIST` in production to the exact mail, webhook,
Feishu and WeCom hosts in use. Wildcards match subdomains only
(`*.example.com` does not match `example.com` itself).

### DNS-rebinding drill

The resolver is covered by unit tests, but the end-to-end behaviour can be
reproduced in an isolated user/network/mount namespace:

```bash
cargo build -p digilegacy-security --example dns_rebind_probe
unshare -Urnm python3 tools/security/dns_rebind_drill.py
```

The drill starts a fake DNS server on `127.0.0.1:53` and a TCP service on
`8.8.8.8`. The probe resolves `rebind.test` once, the DNS answer is changed to
`10.0.0.1`, and the probe must still connect to the originally validated
`8.8.8.8` with exactly one A query. A second process resolving after the change
must be rejected. It is a test-only drill from a source checkout; it needs Linux user
namespaces and must not run against a production resolver.

Start the API:

```bash
set -a
. /etc/digilegacy.env
set +a
exec /opt/digilegacy/bin/digilegacy-server
```

Health check:

```bash
curl -fsS http://127.0.0.1:8787/api/v1/health
```

Expected fields include `success: true`, `schemaVersion: 17` and `tables: 32`.
A health response proves the process can open the store; it does not prove the
write token or outbound integrations work.

## Daily checks

```bash
# Process and API
curl -fsS http://127.0.0.1:8787/api/v1/health

# HTTP counters and latency buckets since process start
curl -fsS -H "Authorization: Bearer $DIGILEGACY_API_TOKEN" \
  http://127.0.0.1:8787/api/v1/metrics

# Backup freshness and recovery state
digilegacy-cli security status --json

# Schema, integrity and audit chain (writes one audit row)
digilegacy-cli diagnostics export --allow-write --json
```

Acceptance:

- health succeeds
- `audit.valid` is `true`
- `lastBackupAt` is no older than 24 hours
- no unexpected 5xx burst in the proxy log

## Checkpoints and backup

A checkpoint is an authenticated copy of the database plus a manifest. The
manifest stores SHA-256 and, when the database key is available, an HMAC over
the metadata. Checkpoints stay beside the live database under `checkpoints/`;
at most 12 are retained.

```bash
# Create before an upgrade, migration or risky repair
digilegacy-cli security checkpoint --label before-upgrade --json

# List checkpoints and their absolute paths
digilegacy-cli security checkpoints --json

# Verify size, hash and HMAC without writing
digilegacy-cli security verify <checkpoint-id> --json
```

The command uses a WAL checkpoint before copying, so it is safe while the API
is running. It writes a `backup.create` audit record and updates
`security status`'s `lastBackupAt`.

The checkpoint command is a **local CLI command**. It is not exposed over HTTP
or MCP, because it needs the database key and writes files beside the store.

Daily schedule example (adjust paths and user):

```cron
0 3 * * * cd /opt/digilegacy && set -a && . /etc/digilegacy.env && set +a && /opt/digilegacy/bin/digilegacy-cli security checkpoint --label scheduled >> /var/log/digilegacy/checkpoint.log 2>&1
```

Keep the key file backed up separately. A checkpoint without the matching key is
cryptographically useless.

## Restore

Restore is deliberately manual and offline. There is no `security restore`
command because overwriting a live database is the highest-risk operation in the
product.

1. Declare a maintenance window and stop the API. Confirm the port is closed.
2. Take a final checkpoint if the database is still readable.
3. Load the same environment file the service uses.
4. List checkpoints and verify the chosen one:
   ```bash
   digilegacy-cli security checkpoints --json
   digilegacy-cli security verify <checkpoint-id> --json
   ```
5. Copy the verified `.db` file to a new target, for example
   `/var/lib/digilegacy/restore/digilegacy.db`. Do not copy over the live file
   yet.
6. Start a temporary API on a spare port with `DIGILEGACY_DB` pointing at the
   restored copy and the same key file. Check:
   ```bash
   DIGILEGACY_BIND=127.0.0.1:8788 DIGILEGACY_DB=/var/lib/digilegacy/restore/digilegacy.db \
     /opt/digilegacy/bin/digilegacy-server
   curl -fsS http://127.0.0.1:8788/api/v1/health
   ```
7. Run a read smoke: open the overview, read one asset, confirm the audit count,
   then run `digilegacy-cli diagnostics export --allow-write --json` against the
   restored copy and require `audit.valid: true`.
8. Stop the temporary API. Move the old live database, `-wal` and `-shm` files
   to an incident directory. Move the restored database into the live path.
9. Start the real service and repeat health and read smoke.
10. Create a fresh checkpoint labelled `after-restore` and record the incident,
    restore duration and chosen checkpoint ID.

If step 6 or 9 fails, stop and put the old database back. Do not run two
processes against the same database file.

## Common failures

| Symptom | Check | Action |
|---|---|---|
| `401` / `403` on a write | `DIGILEGACY_API_TOKEN` and the Web write token | Set the same token on the server and in Settings → Connections. Do not disable the gate. |
| `unable to open database file` | Path, ownership, free space, key file | Fix permissions or restore from a verified checkpoint. Never delete the key file to "make it open". |
| `file is not a database` | Wrong key file or corrupted copy | Use the matching key. Verify the checkpoint before restoring. |
| Policy sync fails | URL allowlist, HMAC key, `If-None-Match`, remote status | Check the feed with `curl`; keep the library unchanged until a candidate is approved. |
| SMTP or webhook test fails | DNS, TLS, token, provider status | Use the connection test row; do not bypass certificate checks. |
| Approval queue grows | Worker process, `jobs.get`, `approvals.list` | Restart the server after a checkpoint; do not approve in bulk to clear a backlog. |
| Audit chain invalid | `diagnostics.export` → `audit.valid=false` | Stop writes, preserve the database, open an incident, restore the last verified checkpoint. |
| Health is green but reads fail | Key/config mismatch, store lock | Check logs and `diagnostics.export`; health alone is not a read proof. |

## Drills

Run these on a throwaway database, never on the live one:

- **Capacity:** seed 1,000 assets, 250 heirs and 10,000 audit rows with
  `cargo run -q -p digilegacy-server --example seed_reference`, then run
  `tools/bench/capacity.py` with `--strict`. The latest result is in
  `docs/operations/measurements/`.
- **Restore:** create a checkpoint, copy its `.db`, start a temporary instance,
  probe health and counts, and verify the audit chain. The 2026-11-20 drill
  reached healthy in 158 ms and verified 10,120 audit rows.
- **Notifications:** use a real SMTP or webhook account and record acceptance
  and final delivery separately. Local TLS tests prove the handshake only.

Record the dataset size, build version, host, result file and elapsed time. A
drill without those fields is not evidence.

## Upgrade and rollback

1. Create a `before-upgrade` checkpoint and verify it.
2. Run `./tools/verify.sh` on the new build.
3. Stop the API.
4. Keep the old binary and database until the new instance passes health and
   read smoke.
5. Run migrations against the live copy only after the checkpoint is verified.
6. If schema or read smoke fails, stop, restore the pre-upgrade checkpoint and
   start the old binary. Record the failed version and schema in the incident.

Never run a migration on a database that has no verified checkpoint.

## Incident handoff

Record:

- start time, detection source and affected entry points
- last successful health check
- chosen checkpoint ID and `security verify` result
- restore start/end time and RTO
- data loss window (time between last checkpoint and incident)
- follow-up action and owner

Do not paste tokens, key bytes, personal asset values or connection passwords
into the incident ticket.
