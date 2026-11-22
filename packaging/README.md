# DigiLegacy release package

This archive contains the single-node DigiLegacy API, CLI and MCP entry points,
the built Web console, policy content and operations docs.

## Requirements

- Linux x86_64 (or the architecture named in the archive) with glibc.
- A writable install prefix and data directory.
- `curl` for the readiness check in `digilegacy-start`.

The Web console is a static SPA. Serve `web/` from a reverse proxy and proxy
`/api/` to the API bind address; do not open `web/index.html` from disk, because
the console uses same-origin `/api` calls. A same-origin nginx server block is
included at `share/nginx/digilegacy.conf.example`.

## Install

```bash
tar -xzf digilegacy-<version>-linux-x86_64.tar.gz
cd digilegacy-<version>
./install.sh --prefix "$HOME/digilegacy" --data-dir "$HOME/digilegacy-data"
```

The installer:

- copies binaries, Web assets, content, docs and the optional operations tools;
- creates `etc/digilegacy.env` with mode `0600` and a random write token;
- creates the data directory and initializes schema v19 plus key files;
- never prints the write token.

Read the token when you need it:

```bash
sed -n 's/^DIGILEGACY_API_TOKEN=//p' "$HOME/digilegacy/etc/digilegacy.env"
```

## Start, check and stop

```bash
"$HOME/digilegacy/bin/digilegacy-start"
"$HOME/digilegacy/bin/digilegacy-cli" health --json
curl -fsS http://127.0.0.1:8787/api/v1/metrics
"$HOME/digilegacy/bin/digilegacy-stop"
```

For an upgrade, keep the same prefix and data directory. The installer keeps an
existing `etc/digilegacy.env`; stop the server before replacing binaries.

## Export and checkpoint

```bash
set -a; . "$HOME/digilegacy/etc/digilegacy.env"; set +a
"$HOME/digilegacy/bin/digilegacy-cli" diagnostics export --allow-write --json
"$HOME/digilegacy/bin/digilegacy-cli" security checkpoint --label pre-upgrade --json
```

`diagnostics export` is structural and sanitized. A checkpoint is written under
the database directory’s `checkpoints/` folder; verify it with
`security verify <id>`.

## Policy source worklist

The archive includes `tools/policies/source_audit.py`. It needs Python 3 and the
`requests` package, and writes a precheck JSON plus a CSV worklist under
`docs/operations/`. It does not verify policy text or change provenance; see
`docs/operations/POLICY-SOURCES.md`.

## Uninstall

```bash
"$HOME/digilegacy/bin/digilegacy-uninstall"
```

The default keeps the database and checkpoints. Add `--purge-data` only after a
verified backup exists.
