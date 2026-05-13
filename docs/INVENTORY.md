# Inventory — apprise-mcp

## MCP tool: `apprise`

All actions share the same tool. Set `action` to select the operation.

| Action | Required args | Optional args | Description |
|--------|---------------|---------------|-------------|
| `notify` | `body` | `tag`, `title`, `type` | Send to tag (or all if tag omitted) |
| `notify_url` | `urls`, `body` | `title`, `type` | Stateless one-off to Apprise URL schema |
| `health` | — | — | Check Apprise server health |
| `help` | — | — | Return inline documentation |

### Argument reference

| Arg | Type | Description |
|-----|------|-------------|
| `action` | string (enum) | Required. One of: `notify`, `notify_url`, `health`, `help` |
| `body` | string | Notification message text |
| `tag` | string | Apprise tag; omit to send to all services |
| `title` | string | Notification title (optional) |
| `type` | string (enum) | `info` (default), `success`, `warning`, `failure` |
| `urls` | string | Apprise URL schema (e.g. `slack://token`) — for `notify_url` only |

## MCP prompt: `send_alert`

Guides the agent to send a critical failure-type notification with clear title and body.

## CLI commands

| Command | Description |
|---------|-------------|
| `apprise notify <body> [--tag T] [--title T] [--type ...]` | Send notification |
| `apprise notify-url <urls> <body> [--title T] [--type ...]` | Stateless notify |
| `apprise health [--json]` | Health check |
| `apprise serve` | Start MCP HTTP server |
| `apprise mcp` | Start MCP stdio transport |
| `apprise --help` | Usage |
| `apprise --version` | Version |

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `APPRISE_URL` | `http://localhost:8000` | Apprise API server base URL |
| `APPRISE_TOKEN` | _(empty)_ | Bearer token for Apprise API (optional) |
| `APPRISE_MCP_HOST` | `0.0.0.0` | MCP HTTP server bind host |
| `APPRISE_MCP_PORT` | `8765` | MCP HTTP server bind port |
| `APPRISE_MCP_TOKEN` | _(none)_ | Static bearer token for MCP HTTP auth |
| `APPRISE_MCP_ALLOWED_ORIGINS` | _(none)_ | Comma-separated additional CORS origins |
| `RUST_LOG` | `info` | Tracing log filter |

## HTTP endpoints (MCP server mode)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/mcp` | POST | MCP streamable HTTP transport |
| `/health` | GET | MCP server liveness (always returns `{"status":"ok"}`) |

## Known live instance

- URL: `http://100.120.242.29:8766`
- Token: _(none required)_
