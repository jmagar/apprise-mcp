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

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:ca08a54f -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

## Session Completion

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd dolt push
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
<!-- END BEADS INTEGRATION -->

## Plugin setup hooks

Plugin setup is owned by the binary. Keep `plugins/apprise/hooks/plugin-setup.sh` as a thin adapter that maps `CLAUDE_PLUGIN_OPTION_*` values to environment variables, prepares appdata, ensures `apprise` is on `PATH`, and then calls `apprise setup plugin-hook "$@"`.

`apprise setup check` is read-only, `apprise setup repair` is idempotent, and `apprise setup plugin-hook --no-repair` is audit mode. Do not add Docker Compose, systemd, or service bootstrap logic back into the hook script.
