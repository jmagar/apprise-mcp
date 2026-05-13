dev:
    cargo run -- serve mcp

build:
    cargo build

release:
    cargo build --release

check:
    cargo check

lint:
    cargo clippy -- -D warnings

fmt:
    cargo fmt

test:
    cargo test

install: release
    install -m 755 target/release/apprise ~/.local/bin/apprise
    @echo "Installed to ~/.local/bin/apprise"

docker-up:
    docker compose up -d

docker-down:
    docker compose down

docker-build:
    docker build -t apprise-mcp -f config/Dockerfile .

up:
    docker compose up -d

down:
    docker compose down

restart:
    docker compose restart

logs:
    docker compose logs -f

health:
    curl -sf http://localhost:8765/health | jq .

repair:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "apprise-mcp: checking and repairing service..."
    if docker compose ps --quiet apprise-mcp 2>/dev/null | grep -q .; then
      docker compose down
      docker compose up -d
      echo "apprise-mcp: docker container repaired"
    elif systemctl --user is-active --quiet apprise-mcp.service 2>/dev/null; then
      systemctl --user restart apprise-mcp.service
      echo "apprise-mcp: systemd service restarted"
    else
      echo "apprise-mcp: no running service found — run 'just docker-up' or 'just dev'"
    fi

setup:
    cp -n .env.example .env || true

gen-token:
    openssl rand -hex 32


test-mcporter:
    #!/usr/bin/env bash
    set -euo pipefail
    bash tests/mcporter/test-tools.sh

validate-skills:
    #!/usr/bin/env bash
    set -euo pipefail
    found=0
    for dir in plugins/skills/*; do
      [[ -d "$dir" ]] || continue
      found=1
      test -f "$dir/SKILL.md" || { echo "MISSING: $dir/SKILL.md"; exit 1; }
      grep -q '^name:' "$dir/SKILL.md" || { echo "MISSING name: $dir/SKILL.md"; exit 1; }
      grep -q '^description:' "$dir/SKILL.md" || { echo "MISSING description: $dir/SKILL.md"; exit 1; }
    done
    [[ "$found" -eq 1 ]] || { echo "MISSING: plugins/skills/*"; exit 1; }
    echo "OK"

# Generate a standalone CLI for this server (requires running server; HTTP-only transport)
generate-cli:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Server must be running on port 8765 (run 'just dev' first)"
    echo "Generated CLI embeds your OAuth token — do not commit or share"
    mkdir -p dist dist/.cache
    current_hash=$(timeout 10 curl -sf \
      -H "Authorization: Bearer $MCP_TOKEN" \
      -H "Accept: application/json, text/event-stream" \
      http://localhost:8765/mcp/tools/list 2>/dev/null | sha256sum | cut -d' ' -f1 || echo "nohash")
    cache_file="dist/.cache/apprise-mcp-cli.schema_hash"
    if [[ -f "$cache_file" ]] && [[ "$(cat "$cache_file")" == "$current_hash" ]] && [[ -f "dist/apprise-mcp-cli" ]]; then
      echo "SKIP: apprise-mcp tool schema unchanged — use existing dist/apprise-mcp-cli"
      exit 0
    fi
    timeout 30 mcporter generate-cli \
      --command http://localhost:8765/mcp \
      --header "Authorization: Bearer $MCP_TOKEN" \
      --name apprise-mcp-cli \
      --output dist/apprise-mcp-cli
    printf '%s' "$current_hash" > "$cache_file"
    echo "Generated dist/apprise-mcp-cli (requires bun at runtime)"

clean:
    cargo clean
    rm -rf .cache/

# Linux only — Windows would need .exe binaries; requires git lfs install
build-plugin: release
    #!/bin/sh
    set -eu
    target_dir="${CARGO_TARGET_DIR:-target}"
    if [ ! -x "$target_dir/release/apprise" ] && [ -x ".cache/cargo/release/apprise" ]; then
      target_dir=".cache/cargo"
    fi
    mkdir -p bin
    install -m 755 "$target_dir/release/apprise" bin/apprise

# Publish: bump version, tag, push (triggers crates.io + Docker publish)
publish bump="patch":
    #!/usr/bin/env bash
    set -euo pipefail
    [ "$(git branch --show-current)" = "main" ] || { echo "Switch to main first"; exit 1; }
    [ -z "$(git status --porcelain)" ] || { echo "Commit or stash changes first"; exit 1; }
    git pull origin main
    CURRENT=$(grep -m1 "^version" Cargo.toml | sed "s/.*\"\(.*\)\".*/\1/")
    IFS="." read -r major minor patch <<< "$CURRENT"
    case "{{bump}}" in
      major) major=$((major+1)); minor=0; patch=0 ;;
      minor) minor=$((minor+1)); patch=0 ;;
      patch) patch=$((patch+1)) ;;
      *) echo "Usage: just publish [major|minor|patch]"; exit 1 ;;
    esac
    NEW="${major}.${minor}.${patch}"
    echo "Version: ${CURRENT} → ${NEW}"
    sed -i "s/^version = \"${CURRENT}\"/version = \"${NEW}\"/" Cargo.toml
    cargo check 2>/dev/null || true
    for f in .claude-plugin/plugin.json .codex-plugin/plugin.json gemini-extension.json; do
      [ -f "$f" ] && python3 -c 'import json,sys; p=sys.argv[1]; v=sys.argv[2]; d=json.load(open(p)); d["version"]=v; json.dump(d,open(p,"w"),indent=2); open(p,"a").write("\n")' "$f" "${NEW}"
    done
    git add -A && git commit -m "release: v${NEW}" && git tag "v${NEW}" && git push origin main --tags
    echo "Tagged v${NEW} — publish workflow will run automatically"

# Refresh local reference documentation (crawls + repomix)
refresh-docs:
    bash scripts/refresh-docs.sh

# Refresh docs — repomix packs only (no crawl)
refresh-docs-repomix:
    bash scripts/refresh-docs.sh --skip-crawl

# Refresh docs — crawl only (no repomix)
refresh-docs-crawl:
    bash scripts/refresh-docs.sh --skip-repomix

# Dry-run: print what would be refreshed
refresh-docs-dry:
    bash scripts/refresh-docs.sh --dry-run
