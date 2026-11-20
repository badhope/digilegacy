# Connection setup handoff

Use this page when a mailbox, webhook, Feishu or WeCom account belongs to
someone else. The platform owner does not collect the account password; the
account owner fills it in and tests it in the console.

**Entry:** `https://<your-digilegacy-host>/settings/connections`

The same page contains a short "Account setup guide" panel with one-click
buttons for SMTP, Webhook, Feishu and WeCom. It can also copy the note below so
it can be sent to the person filling it in.

## What to send

Send only:

1. the console URL;
2. the platform write token, through a separate trusted channel;
3. the account fields listed below.

Do not send an email password, app password, webhook secret or token in a group
chat, issue tracker, screenshot or repository.

## Fields

| Kind | Required | Notes |
|---|---|---|
| SMTP mailbox | Endpoint `host:port` | Port 465 normally uses **TLS**; port 587 normally uses **STARTTLS**. Confirm with the mail provider. |
| SMTP mailbox | Username | Usually the full mailbox address. |
| SMTP mailbox | Credential | An app password / authorization code is preferred. A normal login password may be rejected by the provider. |
| SMTP mailbox | From | The address recipients will see. It must be allowed by the provider. |
| SMTP mailbox | Security | `TLS`, `STARTTLS` or `None`. Never choose `None` on a network you do not control. |
| Webhook | Full URL | `https://...`; no spaces or line breaks. |
| Webhook | Credential | Optional bearer token or signing secret, if the provider requires it. |
| Feishu | Full bot webhook URL | Create the bot in the target group; do not use a personal chat webhook. |
| WeCom | Full bot webhook URL | Same rule: use a group bot, not a personal one. |

## Steps for the person filling it in

1. Open the console URL and go to **Settings → Connections**.
2. Enter the **platform write token** at the top. It stays in that browser
   session; it is not written to local storage.
3. Pick the matching button in the guide, or press **Add connection** and choose
   the kind.
4. Fill the label, endpoint, credential and, for SMTP, username, From and
   security.
5. Press **Save**, then **Test**. A green **OK** with a latency value means the
   platform reached the provider. It does not prove final delivery.
6. Send back the connection **label** and a screenshot of the status only.
   Never send the credential.

## What the platform stores

- The connection config is encrypted with the server's connection key
  (`DIGILEGACY_CONFIG_KEY_FILE`, or a domain key derived from
  `DIGILEGACY_DB_KEY_FILE`).
- The Web UI never echoes a saved credential. Editing requires re-entering the
  endpoint and credential.
- If the server has no connection key, saving fails with `CONFIG_KEY_MISSING`;
  the platform owner must set the key first.
- The connection test only checks reachability/authentication. Provider
  acceptance and final inbox delivery remain a separate drill.

## Handoff checklist

- [ ] console URL and write token sent separately
- [ ] account owner confirmed which SMTP ports and security mode the provider allows
- [ ] connection saved with a clear label, e.g. `work-mail` or `ops-webhook`
- [ ] test returned OK
- [ ] label and status screenshot received; no credential in the ticket
- [ ] `security status --json` and `/api/v1/metrics` checked after the change

If the test fails, record the error code and provider response. Do not disable
TLS verification or paste the credential into the ticket to "make it work".
