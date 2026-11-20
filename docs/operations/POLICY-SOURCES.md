# Policy Source Precheck

The policy library ships with `policy_provenance.status = unverified` for every
row. The source precheck is a **worklist generator**, not a verification step:
a reachable platform home page does not prove that the page contains the cited
inheritance rule.

## Run

```bash
python3 tools/policies/source_audit.py
```

The script is an operations tool and needs Python `requests`; the release
archive ships it under `tools/policies/source_audit.py`. It writes:

- `docs/operations/measurements/policy-sources-2026-11-20.json` — machine
  evidence and per-entry status;
- `docs/operations/policy-source-worklist.csv` — human worklist for Excel or a
  spreadsheet.

Useful options:

```bash
python3 tools/policies/source_audit.py --no-network
python3 tools/policies/source_audit.py --timeout 12 --workers 6
```

## 2026-11-20 precheck

| Check | Result |
| --- | --- |
| Policies | 152 |
| URL present / HTTPS | 152 / 152 |
| Platform root URLs | 150 |
| Specific deep links | 2 (`iCloud`, `Apple ID`, same Apple support page) |
| Duplicate URL groups | 17, covering 38 entries |
| Reachable (2xx/3xx) | 134 |
| Blocked (401/403/429) | 2 |
| Broken (4xx/5xx) | 2 |
| Timeout | 11 |
| Transport error | 3 |
| Entries needing human review | 152 |

The 152 review count is deliberate: every entry is either a root URL, a
duplicate, or did not return a clean 2xx/3xx. `timeout`, `blocked`, and
`transport error` are **not** proof that a source is dead; cross-border access,
anti-bot behavior, and site maintenance can all produce them.

## Human review procedure

1. Start with the top 30 platforms by usage/risk, not all 152 at once.
2. Replace a platform root with the specific official help/rule page. Do not
   use a search engine result, a blog, or an aggregator as the source.
3. Confirm the page itself states the rule cited in `data_source`; record
   reviewer, date, and evidence hash in `policy_provenance`.
4. Update one row through `PATCH /api/v1/policies/{id}/provenance` or the
   corresponding CLI/UI path. The product starts at `unverified`; only a named
   human reviewer may set `verified`.
5. Re-run the precheck after URL edits. A green precheck is still not a
   verification: it only says the URL is present, specific enough to inspect,
   not duplicated, and reachable at that moment.

## Boundaries

- The tool never changes `content/policies/platforms.json`, the database, or
  provenance status.
- A machine precheck must not be used to satisfy G-16 on its own.
- The two deep links are not “done” either; they share one Apple page and still
  need a human to confirm the cited iCloud/Apple ID rules.
- Policy content remains a user-owned content decision. The code can expose
  provenance and evidence, but it cannot invent a reviewer or a legal
  conclusion.
