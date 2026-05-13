---
name: apprise
description: >
  Send push notifications through the apprise-mcp server — a standalone Rust MCP bridge to the
  Apprise universal notification library (Slack, Discord, email, Telegram, and 100+ more).
  Use this skill whenever the user wants to send a notification, push an alert, notify a service,
  fire off a push message, or use Apprise in any way — even if they just say "send me an alert"
  or "let me know when X is done". Covers three tiers: MCP tool (preferred), CLI binary, and
  direct REST API curl calls. Trigger phrases include: send notification, push alert, Apprise
  notify, send push notification, alert via Apprise, notify service, send alert, notify via
  apprise, ping me, fire an alert.
---

# Apprise — Universal Push Notifications

Send notifications through 100+ services (Slack, Discord, email, Telegram, PagerDuty, Pushover,
and more) via the standalone `apprise-mcp` server. Choose the tier that matches what's available.

---

## Tier 1 (Preferred): MCP Tool

**Tool name:** `apprise`  
**Dispatch:** set `action` to one of the actions below.

### notify — send to a tag group or all services

```
apprise(action="notify", body="<message>")
apprise(action="notify", body="<message>", tag="<tag>", title="<title>", type="<type>")
```

| Parameter | Required | Notes |
|-----------|----------|-------|
| `body` | yes | Notification message body |
| `tag` | no | Named group (e.g. `servers`, `ops`). Omit to notify **all** configured services |
| `title` | no | Notification title |
| `type` | no | `info` (default) · `success` · `warning` · `failure` |

**Examples:**
```
# Alert the "servers" tag group
apprise(action="notify", body="Disk at 95%", tag="servers", title="Disk Warning", type="warning")

# Notify every configured service
apprise(action="notify", body="Deploy to production complete", title="CI/CD", type="success")

# Simple informational — type defaults to info
apprise(action="notify", body="Backup finished successfully")
```

### notify_url — stateless one-off (no server config needed)

```
apprise(action="notify_url", urls="<apprise-url-schema>", body="<message>")
apprise(action="notify_url", urls="<url>", body="<message>", title="<title>", type="<type>")
```

| Parameter | Required | Notes |
|-----------|----------|-------|
| `urls` | yes | Apprise URL schema string |
| `body` | yes | Notification message body |
| `title` | no | Notification title |
| `type` | no | `info` · `success` · `warning` · `failure` |

**Common URL schemas:**
```
slack://tokenA/tokenB/tokenC
discord://webhook_id/webhook_token
mailto://user:pass@gmail.com
telegram://bottoken/chatid
pushover://userkey/apptoken
```

**Examples:**
```
apprise(action="notify_url", urls="slack://T0/B0/C0", body="Build failed", title="CI", type="failure")
apprise(action="notify_url", urls="discord://webhook_id/token", body="All tests passed", type="success")
```

### health — server liveness check

```
apprise(action="health")
```

Returns `{"status": "OK"}` when the Apprise API server is reachable.

### help — built-in documentation

```
apprise(action="help")
```

---

## Tier 2 (Fallback): CLI Binary

Binary: `/home/jmagar/workspace/apprise-mcp/target/release/apprise`

```bash
# Notify a tag group
apprise notify "Disk at 95%" --tag servers --title "Disk Warning" --type warning

# Notify all configured services
apprise notify "Deploy complete" --title "CI/CD" --type success

# Simple notification (type defaults to info)
apprise notify "Backup done"

# Stateless one-off via URL schema
apprise notify-url "slack://tokenA/tokenB/tokenC" "Build failed" --title "CI" --type failure
apprise notify-url "discord://webhook_id/token" "Deploy done" --type success

# Health check
apprise health
```

**Type aliases accepted:** `warn` → warning, `fail`/`error` → failure

---

## Tier 3 (Last Resort): Direct REST API

Uses `$APPRISE_URL`. Add `-H "Authorization: Bearer $APPRISE_TOKEN"` and
`-H "X-Apprise-API-Key: $APPRISE_TOKEN"` if auth is required.

```bash
# Notify a tag group
curl -X POST "$APPRISE_URL/notify/servers" \
  -H "Content-Type: application/json" \
  -d '{"title":"Disk Warning","body":"Disk at 95%","type":"warning"}'

# Notify all configured services
curl -X POST "$APPRISE_URL/notify" \
  -H "Content-Type: application/json" \
  -d '{"title":"CI/CD","body":"Deploy complete","type":"success"}'

# Stateless — include "urls" field, trailing slash required
curl -X POST "$APPRISE_URL/notify/" \
  -H "Content-Type: application/json" \
  -d '{"urls":"slack://tokenA/tokenB/tokenC","title":"CI","body":"Build failed","type":"failure"}'

# Health check
curl "$APPRISE_URL/health"
```

---

## Notification types

| Type | When to use |
|------|-------------|
| `info` | Default — routine information |
| `success` | Completed successfully |
| `warning` | Non-critical issue needing attention |
| `failure` | Critical failure or error |

---

## Environment variables

| Variable | Purpose |
|----------|---------|
| `APPRISE_URL` | Apprise API server base URL (required) |
| `APPRISE_TOKEN` | Bearer token for Apprise API auth (optional) |
| `APPRISE_MCP_HOST` | MCP HTTP bind host |
| `APPRISE_MCP_PORT` | MCP HTTP bind port (default 8765) |
| `APPRISE_MCP_TOKEN` | Static token for MCP HTTP auth |

---

## Key notes

- **Tag routing:** `tag` targets a pre-configured group of services on the Apprise server. Use when your Apprise config defines named keys like `servers`, `alerts`, `ops`.
- **Stateless `notify_url`:** bypasses all server config — ideal for one-off or dynamic routing where the destination isn't pre-configured.
- **Response normalization:** the server returns plain `"OK"` text or JSON; the MCP client normalizes both to `{"ok": true, "response": "OK"}`.
- **Auth dual-header:** if `APPRISE_TOKEN` is set, both `Authorization: Bearer` and `X-Apprise-API-Key` are sent for compatibility with older Apprise versions.
