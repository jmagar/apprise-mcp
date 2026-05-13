# apprise-mcp — CLAUDE.md

## Module map

```
src/
  apprise.rs     — AppriseClient: HTTP REST client for Apprise API
  app.rs         — AppriseService: business logic layer (wraps client)
  config.rs      — AppriseConfig + McpConfig; env var loading
  mcp.rs         — AppState; declares mcp submodules
  mcp/
    tools.rs     — execute_tool() thin shim; action dispatch
    schemas.rs   — tool_definitions() JSON schema for the apprise tool
    prompts.rs   — list_prompts() / get_prompt() — send_alert prompt
    rmcp_server.rs — AppriseRmcpServer implementing rmcp ServerHandler
    routes.rs    — Axum router; /mcp + /health
  lib.rs         — pub module declarations; testing helpers
  main.rs        — binary entry; CLI/serve/stdio dispatch; cli module
```

## Key patterns

### Layering rule
All business logic lives in `app.rs`. `mcp/tools.rs` and `cli.rs` are thin shims — they parse arguments and call `AppriseService`, nothing else.

### Adding a new action
1. Add method to `AppriseClient` in `apprise.rs`
2. Add delegating method to `AppriseService` in `app.rs`
3. Add arm to `dispatch()` in `mcp/tools.rs`
4. Add arm to `run()` in `cli.rs`
5. Update `APPRISE_ACTIONS` in `mcp/schemas.rs` and the schema `properties`
6. Update `HELP_TEXT` in `mcp/tools.rs`

### Apprise API conventions
- `POST /notify/{tag}` — send to tag
- `POST /notify` — send to all
- `POST /notify/` (trailing slash, with `urls` field) — stateless
- Response is plain `"OK"` or JSON; client handles both

### Auth
No auth middleware on the MCP transport in the current implementation. Add `APPRISE_MCP_TOKEN` for static bearer auth if exposing over a network.

### Config loading order
1. `config.toml` (if present, at CWD)
2. Environment variables override (env wins)

### Testing
Tests in `tests/` use `apprise_mcp::testing::stub_state()` which points the client at `localhost:1` (unreachable). All network tests assert `is_err()`.

## CLI ↔ MCP Action Parity

| Service Method | MCP Action | CLI Command |
|---|---|---|
| `service.notify(tag, ...)` | `apprise(action="notify", body=..., tag=..., title=..., type=...)` | `apprise notify <body> --tag TAG [--title T] [--type ...]` |
| `service.notify_all(...)` | `apprise(action="notify", body=..., title=..., type=...)` | `apprise notify <body> [--title T] [--type ...]` |
| `service.notify_url(urls, ...)` | `apprise(action="notify_url", urls=..., body=..., title=..., type=...)` | `apprise notify-url <urls> <body> [--title T] [--type ...]` |
| `service.health()` | `apprise(action="health")` | `apprise health` |
| *(built-in)* | `apprise(action="help")` | `apprise help` |

## Environment variables

| Variable | Purpose |
|----------|---------|
| `APPRISE_URL` | Apprise API server base URL |
| `APPRISE_TOKEN` | Bearer token for Apprise API auth |
| `APPRISE_MCP_HOST` | MCP HTTP bind host |
| `APPRISE_MCP_PORT` | MCP HTTP bind port (default 8765) |
| `APPRISE_MCP_TOKEN` | Static token for MCP HTTP server auth |
| `RUST_LOG` | Tracing log filter |
