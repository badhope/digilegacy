# Asset depth entry acceptance (0.0.34, 2026-11-22)

The six gaps listed in the 0.0.33 UX report are closed: deep fields in the single-entry form, kind filtering, save-and-continue, draft protection, a valuation basis, and an explicit hard-delete path for archived assets.

The path was exercised in real Chromium against the isolated database `/tmp/dg-ui-demo.db`, not in jsdom alone. A six-record demo set covered property, a bank card, a photo archive, a domain, a bracelet and an archived mailbox. The screenshots are in `screenshots-0.0.34-ui/`; the machine-readable record is `docs/operations/measurements/ux-asset-depth-0.0.34-2026-11-22.json`.

Observed: the kind filter returned exactly one financial asset; save-and-continue kept `financial` and `insured`; a text draft restored after a route change while a type-only form did not; the archived record was deleted after a second confirmation, the list moved from six to five, GET returned 404 and the audit contained `asset.delete`.

The detailed Chinese report also records a test-environment incident: the first demo server was started with the wrong environment variable and briefly opened the real database. The six inserted demo assets and their audit rows were removed immediately; the real database is back to zero assets and its 18 pre-existing audit rows, although its schema is now migrated to v19. Every later check used the isolated database.

**Done.**
