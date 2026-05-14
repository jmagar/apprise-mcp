#!/usr/bin/env bash
# SessionStart / ConfigChange hook for apprise-mcp.
# Keep setup policy in the binary; this script only adapts plugin settings to env.
set -euo pipefail

: "${CLAUDE_PLUGIN_ROOT:=$(cd "$(dirname "$0")/.." && pwd)}"
: "${CLAUDE_PLUGIN_DATA:=${HOME}/.claude/plugins/data/apprise-jmagar-lab}"
: "${APPRISE_HOME:=${CLAUDE_PLUGIN_DATA}}"

reject_unsafe_value() {
  local name="$1" value="${2:-}"
  if [[ "${value}" == *$'\n'* || "${value}" == *$'\r'* ]]; then
    printf 'apprise plugin setup: %s must not contain newlines\n' "${name}" >&2
    exit 2
  fi
}

export_if_set() {
  local env_name="$1" option_name="$2" value
  value="$(printenv "${option_name}" || true)"
  reject_unsafe_value "${option_name}" "${value}"
  [[ -n "${value}" ]] || return 0
  export "${env_name}=${value}"
}

ensure_apprise_binary() {
  if command -v apprise >/dev/null 2>&1; then
    return 0
  fi

  local bundled="${CLAUDE_PLUGIN_ROOT}/bin/apprise"
  if [[ -x "${bundled}" ]]; then
    mkdir -p "${HOME}/.local/bin"
    ln -sf "${bundled}" "${HOME}/.local/bin/apprise"
    export PATH="${HOME}/.local/bin:${PATH}"
  fi

  command -v apprise >/dev/null 2>&1 || {
    printf 'apprise plugin setup: apprise binary not found on PATH or at %s\n' "${bundled}" >&2
    exit 1
  }
}

main() {
  export_if_set APPRISE_MCP_TOKEN CLAUDE_PLUGIN_OPTION_API_TOKEN
  export_if_set APPRISE_MCP_PORT CLAUDE_PLUGIN_OPTION_MCP_PORT
  export_if_set APPRISE_URL CLAUDE_PLUGIN_OPTION_APPRISE_URL
  export_if_set APPRISE_TOKEN CLAUDE_PLUGIN_OPTION_APPRISE_TOKEN
  export_if_set APPRISE_MCP_NO_AUTH CLAUDE_PLUGIN_OPTION_NO_AUTH
  export_if_set APPRISE_MCP_AUTH_MODE CLAUDE_PLUGIN_OPTION_AUTH_MODE
  export_if_set APPRISE_MCP_PUBLIC_URL CLAUDE_PLUGIN_OPTION_PUBLIC_URL
  export_if_set APPRISE_MCP_GOOGLE_CLIENT_ID CLAUDE_PLUGIN_OPTION_GOOGLE_CLIENT_ID
  export_if_set APPRISE_MCP_GOOGLE_CLIENT_SECRET CLAUDE_PLUGIN_OPTION_GOOGLE_CLIENT_SECRET
  export_if_set APPRISE_MCP_AUTH_ADMIN_EMAIL CLAUDE_PLUGIN_OPTION_AUTH_ADMIN_EMAIL

  mkdir -p "${APPRISE_HOME}"
  chmod 700 "${APPRISE_HOME}" 2>/dev/null || true
  export CLAUDE_PLUGIN_DATA APPRISE_HOME

  ensure_apprise_binary
  apprise setup plugin-hook "$@"
}

main "$@"
