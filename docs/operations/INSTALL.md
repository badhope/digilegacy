# Install, upgrade and uninstall

The release archive is the single-node deployment shape: one API process, the
CLI and MCP binaries, static Web assets, policy content and the operations docs.

## Build the archive

```bash
./tools/package/build.sh
```

Output goes to `packages/digilegacy-<version>-linux-<arch>.tar.gz` plus a
`.sha256` file. The build uses the checked-in `web/package-lock.json` when
`web/node_modules` is absent, then runs the same release build used by the
verification gate.

## Fresh install

```bash
tar -xzf packages/digilegacy-0.0.18-linux-x86_64.tar.gz -C /tmp
cd /tmp/digilegacy-0.0.18
./install.sh --prefix /opt/digilegacy --data-dir /var/lib/digilegacy
/opt/digilegacy/bin/digilegacy-start
```

The installer is idempotent for an upgrade: it replaces binaries, Web assets,
content and docs, but keeps `etc/digilegacy.env` and the data directory. It
creates the write token once and never prints it.

`DIGILEGACY_ALLOW_PRIVATE_NETWORK` and `DIGILEGACY_OUTBOUND_ALLOWLIST` are not
set by the installer. Add them to the environment file when the deployment uses
LAN integrations or an explicit outbound host list.

## Web console

The API does not serve the SPA. Put a reverse proxy in front:

A ready-to-edit same-origin server block ships at
`<prefix>/share/nginx/digilegacy.conf.example` (source:
`packaging/nginx/digilegacy.conf.example`). Set `root` to the installed `web/`
directory and keep `/api/` on the same origin. The abbreviated example is:

```nginx
server {
  listen 443 ssl;
  server_name digilegacy.example.com;

  root /opt/digilegacy/web;
  index index.html;

  location / {
    try_files $uri $uri/ /index.html;
  }

  location /api/ {
    proxy_pass http://127.0.0.1:8787;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-Proto $scheme;
  }
}
```

The console calls same-origin `/api`, so do not set `VITE_API_BASE` to a
different origin unless CORS is added deliberately. The sample was checked with
nginx 1.18.0: `/`, `/settings`, `/api/v1/health` and a hashed asset all returned
200, and the security/cache headers matched the sample. Evidence:
`docs/operations/measurements/proxy-2026-11-20.json`.

## Export and checkpoint

```bash
set -a; . /opt/digilegacy/etc/digilegacy.env; set +a
/opt/digilegacy/bin/digilegacy-cli diagnostics export --allow-write --json
/opt/digilegacy/bin/digilegacy-cli security checkpoint --label pre-upgrade --json
/opt/digilegacy/bin/digilegacy-cli security verify <id> --json
```

`diagnostics export` is structural and sanitized; it is not a full data export.
For migration or disaster recovery, copy the database and both key files while
the server is stopped, or use the offline checkpoint procedure in the runbook.

## Upgrade

1. Take a verified checkpoint and record its id.
2. Stop the server: `digilegacy-stop`.
3. Extract the new archive and run `install.sh` with the same `--prefix` and
   `--data-dir`.
4. Start and check `/api/v1/health`, `/api/v1/metrics` and a read route.
5. If the new version fails, stop it, restore the old binaries or the checkpoint
   following the offline runbook.

## Uninstall

```bash
/opt/digilegacy/bin/digilegacy-uninstall
```

The default removes the installation but preserves the data directory and copies
`db.key` / `config.key` into `<data>/keys/`. A later reinstall with the same
`--data-dir` restores those keys before the health check, so the preserved
database remains readable. `--purge-data` removes the data directory too; only
use it after a verified backup.
