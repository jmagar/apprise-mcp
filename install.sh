#!/usr/bin/env bash
# install.sh — one-line install for apprise-mcp binary
# Usage: curl -fsSL https://raw.githubusercontent.com/jmagar/apprise-mcp/main/install.sh | bash
set -euo pipefail

INSTALL_DIR="${HOME}/.local/bin"
BINARY_NAME="apprise"

# ── Detect platform ───────────────────────────────────────────────────────────
OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}-${ARCH}" in
  Linux-x86_64)  ASSET="apprise-x86_64-linux" ;;
  Linux-aarch64) ASSET="apprise-aarch64-linux" ;;
  Darwin-x86_64) ASSET="apprise-x86_64-macos" ;;
  Darwin-arm64)  ASSET="apprise-aarch64-macos" ;;
  *)
    echo "ERROR: unsupported platform ${OS}-${ARCH}" >&2
    echo "       Build from source: cargo install --path ." >&2
    exit 1
    ;;
esac

REPO="jmagar/apprise-mcp"
DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${ASSET}"

# ── Install binary ────────────────────────────────────────────────────────────
mkdir -p "${INSTALL_DIR}"

echo "Downloading apprise-mcp from ${DOWNLOAD_URL} ..."
curl -fsSL --progress-bar "${DOWNLOAD_URL}" -o "${INSTALL_DIR}/${BINARY_NAME}"
chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

echo "Installed: ${INSTALL_DIR}/${BINARY_NAME}"

# Verify
if "${INSTALL_DIR}/${BINARY_NAME}" --version >/dev/null 2>&1; then
  echo "Version: $("${INSTALL_DIR}/${BINARY_NAME}" --version)"
fi

# ── Starter .env ──────────────────────────────────────────────────────────────
if [[ ! -f ".env" ]]; then
  if [[ -f ".env.example" ]]; then
    cp .env.example .env
    echo "Created .env from .env.example — edit it to set APPRISE_URL and APPRISE_MCP_TOKEN"
  else
    cat > .env << 'EOF'
# Upstream Apprise API server URL (required)
APPRISE_URL=http://localhost:8000

# Static bearer token for the MCP HTTP server (generate: openssl rand -hex 32)
APPRISE_MCP_TOKEN=

# Optional Apprise API token
APPRISE_TOKEN=

RUST_LOG=info
EOF
    echo "Created starter .env — edit it to set APPRISE_URL and APPRISE_MCP_TOKEN"
  fi
fi

# ── PATH hint ─────────────────────────────────────────────────────────────────
if ! command -v "${BINARY_NAME}" >/dev/null 2>&1; then
  echo ""
  echo "NOTE: ${INSTALL_DIR} is not in your PATH."
  echo "      Add this to your shell profile:"
  echo "        export PATH=\"\${HOME}/.local/bin:\${PATH}\""
fi

echo ""
echo "Next steps:"
echo "  1. Edit .env — set APPRISE_URL (your Apprise server) and APPRISE_MCP_TOKEN (shared secret)"
echo "  2. Edit config.toml for non-secret settings (host, port, auth_mode)"
echo "  3. Run: apprise serve mcp"
echo "     Or:  docker compose up -d"
