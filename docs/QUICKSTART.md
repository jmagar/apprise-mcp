# Quickstart — apprise-mcp

Get up and running in 5 minutes.

## Prerequisites

- Rust 1.86+ (`rustup update stable`)
- A running [Apprise API server](https://github.com/caronc/apprise-api)

### Start Apprise API server (Docker)

```bash
docker run -d --name apprise -p 8766:8000 caronc/apprise:latest
```

The live homelab instance is already at `http://100.120.242.29:8766`.

## Step 1 — Build the binary

```bash
cd /home/jmagar/workspace/apprise-mcp
cargo build --release
# Binary at: target/release/apprise
```

## Step 2 — Set environment

```bash
export APPRISE_URL=http://100.120.242.29:8766
# APPRISE_TOKEN=  (leave empty for open installs)
```

Or copy `.env.example` to `.env` and fill in values.

## Step 3 — Health check

```bash
cargo run -- health
# or after install:
apprise health
```

Expected output:
```
status: OK
```

## Step 4 — Send a test notification

```bash
cargo run -- notify "Hello from apprise-mcp" --title "Test" --type info
```

## Step 5 — Use with Claude

### stdio transport (simplest)

Add to Claude Desktop config (`~/.config/claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "apprise": {
      "command": "/path/to/apprise",
      "args": ["mcp"],
      "env": {
        "APPRISE_URL": "http://100.120.242.29:8766"
      }
    }
  }
}
```

### HTTP transport

```bash
APPRISE_URL=http://100.120.242.29:8766 apprise serve
# Listening on 0.0.0.0:8765
```

Then point Claude at `http://localhost:8765/mcp`.

## Configuring notification services in Apprise

Visit the Apprise web UI at your Apprise URL, or use the REST API:

```bash
# Add a Slack webhook under the "ops" tag
curl -X POST http://100.120.242.29:8766/add/ops \
  -H "Content-Type: application/json" \
  -d '{"urls": "slack://your/webhook/url"}'
```

Then from Claude:
```
Use the apprise tool with action=notify, tag=ops, body="Deploy complete", type=success
```
