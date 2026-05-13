# Changelog

All notable changes to this project will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [0.1.0] — 2026-05-13

### Added

- Initial release of `apprise-mcp`
- `AppriseClient` HTTP REST client for the Apprise API
  - `notify(tag, title, body, type)` — POST /notify/{tag}
  - `notify_all(title, body, type)` — POST /notify
  - `notify_url(urls, title, body, type)` — stateless POST /notify/
  - `health()` — GET /health
- `AppriseService` business logic layer wrapping the client
- MCP tool `apprise` with actions: `notify`, `notify_url`, `health`, `help`
- MCP prompt `send_alert` for guided critical alert sending
- CLI: `notify`, `notify-url`, `health` subcommands
- HTTP MCP server (axum + rmcp streamable HTTP transport)
- stdio MCP transport
- `NotifyType` enum: `info`, `success`, `warning`, `failure`
- Config loading from `config.toml` + environment variables
  - `APPRISE_URL`, `APPRISE_TOKEN`
  - `APPRISE_MCP_HOST`, `APPRISE_MCP_PORT`, `APPRISE_MCP_TOKEN`
- Integration tests: stub-based graceful failure tests for all service methods
- Unit tests: `NotifyType` parsing, config defaults, bind address
