# nginx reverse proxy

`digilegacy.conf.example` is a same-origin server block for the Web console and
the API.

1. Copy it into your nginx `http {}` configuration.
2. Set `root` to the installed `web/` directory (for example
   `/opt/digilegacy/web`).
3. Set `server_name`, listen address and TLS certificates.
4. Keep `/api/` on the same origin as the SPA; the frontend uses relative API
   paths.
5. Run `nginx -t` and reload only after it passes.

The sample does not terminate TLS itself; add `listen 443 ssl`,
`ssl_certificate` and `ssl_certificate_key` in front of it, or terminate TLS at
an upstream load balancer. If a proxy is placed in front of the outbound
connection probes, see the runbook: those probes deliberately bypass
`HTTP_PROXY` and `HTTPS_PROXY`.
