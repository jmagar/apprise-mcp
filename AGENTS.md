# AGENTS.md — apprise-mcp

## Purpose
This repository is an MCP server for sending push notifications via Apprise. It exposes one MCP tool (`apprise`) with actions: `notify`, `notify_url`, `health`, `help`.

## Key facts for agents
- Binary name: `apprise`
- Default MCP HTTP port: **8765**
- Default Apprise API URL: `http://localhost:8000` (override with `APPRISE_URL`)
- Known live instance: `http://100.120.242.29:8766` (no token required)

## Common tasks

### Build
```bash
cargo build --release
```

### Run tests
```bash
cargo test
```

### Check (fast compile check)
```bash
cargo check
```

### Run MCP server
```bash
APPRISE_URL=http://100.120.242.29:8766 cargo run -- serve
```

### Run stdio MCP transport
```bash
APPRISE_URL=http://100.120.242.29:8766 cargo run -- mcp
```

### Send a test notification
```bash
APPRISE_URL=http://100.120.242.29:8766 cargo run -- notify "hello from agent" --type info
```

## Architecture rules
1. `apprise.rs` — HTTP only, no business logic
2. `app.rs` — business logic only, no HTTP parsing
3. `mcp/tools.rs` and `cli.rs` — argument parsing only, delegate to `AppriseService`
4. Never add auth logic to `apprise.rs`; tokens are set at client construction

## Adding a notification service
You do not need to modify this MCP server to add new notification services. Configure them in the Apprise API server's web UI or via `POST /add/{tag}`. Then call `notify` with the relevant tag.
