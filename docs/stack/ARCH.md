# Architecture — apprise-mcp

## Layer diagram

```
┌─────────────────────────────────────────────────────┐
│  Clients                                            │
│  ┌──────────────────┐  ┌───────────────────────┐   │
│  │  Claude / MCP    │  │  CLI (apprise notify) │   │
│  └────────┬─────────┘  └───────────┬───────────┘   │
└───────────┼────────────────────────┼───────────────┘
            │ MCP protocol           │ stdin args
┌───────────▼────────────────────────▼───────────────┐
│  Transport / Entry (src/main.rs)                    │
│  ┌──────────────┐  ┌──────────────┐                │
│  │ HTTP /mcp    │  │ stdio mcp    │                │
│  │ (axum+rmcp)  │  │ (rmcp stdio) │                │
│  └──────┬───────┘  └──────┬───────┘                │
└─────────┼─────────────────┼──────────────────────── ┘
          │                 │
┌─────────▼─────────────────▼───────────────────────┐
│  mcp/tools.rs — thin dispatch shim                 │
│  Parses action + args, calls AppriseService        │
└─────────────────────┬─────────────────────────────┘
                      │
┌─────────────────────▼─────────────────────────────┐
│  app.rs — AppriseService (business logic)          │
│  notify / notify_all / notify_url / health         │
└─────────────────────┬─────────────────────────────┘
                      │
┌─────────────────────▼─────────────────────────────┐
│  apprise.rs — AppriseClient (HTTP REST)            │
│  POST /notify/{tag}                                │
│  POST /notify                                      │
│  POST /notify/  (stateless, with urls field)       │
│  GET  /health                                      │
└─────────────────────┬─────────────────────────────┘
                      │ HTTP
┌─────────────────────▼─────────────────────────────┐
│  Apprise API server (external)                     │
│  http://APPRISE_URL                                │
└────────────────────────────────────────────────────┘
```

## Module responsibilities

| File | Responsibility |
|------|---------------|
| `src/apprise.rs` | HTTP REST client. All network I/O. `NotifyType` enum. |
| `src/app.rs` | `AppriseService` — business logic, thin delegation to client. |
| `src/config.rs` | `AppriseConfig`, `McpConfig`. TOML + env loading. |
| `src/mcp.rs` | `AppState`. Declares `mcp/*` submodules. |
| `src/mcp/tools.rs` | `execute_tool()` — parses action/args, calls service. |
| `src/mcp/schemas.rs` | JSON input schema for the `apprise` tool. |
| `src/mcp/prompts.rs` | `send_alert` prompt. |
| `src/mcp/rmcp_server.rs` | `AppriseRmcpServer` implementing `rmcp::ServerHandler`. |
| `src/mcp/routes.rs` | Axum router — `/mcp`, `/health`, CORS. |
| `src/lib.rs` | Public module declarations + `testing` helpers. |
| `src/main.rs` | Binary entry — arg dispatch, `cli` module. |

## Design decisions

**Single tool, action-based dispatch**: One `apprise` tool with an `action` field is simpler for LLMs than many narrow tools. The schema enumerates valid actions so clients can validate upfront.

**No auth on MCP transport by default**: Apprise notifications are low-sensitivity write operations. Auth can be added via `APPRISE_MCP_TOKEN` for network-exposed deployments.

**reqwest with rustls**: No OpenSSL dependency, better for musl/Alpine builds.

**Stateless notify support**: The Apprise API's `/notify/` endpoint lets agents send one-off notifications without pre-configuring the server, useful for ephemeral or dynamic targets.
