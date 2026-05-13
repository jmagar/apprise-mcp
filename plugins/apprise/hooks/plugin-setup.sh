#!/usr/bin/env bash
# SessionStart / ConfigChange hook — deploys or validates apprise-mcp
set -euo pipefail

# Derive CLAUDE_PLUGIN_ROOT from script location when invoked directly.
: "${CLAUDE_PLUGIN_ROOT:=$(cd "$(dirname "$0")/.." && pwd)}"
: "${CLAUDE_PLUGIN_DATA:=${HOME}/.claude/plugins/data/apprise-jmagar-lab}"

# ── Helpers ───────────────────────────────────────────────────────────────────

existing_env_value() {
  local key="$1"
  local file value
  for file in "${CLAUDE_PLUGIN_DATA}/.env" "${CLAUDE_PLUGIN_DATA}/apprise-mcp.env"; do
    [[ -f "${file}" ]] || continue
    value="$(awk -F= -v key="${key}" '$1 == key {print substr($0, index($0, "=") + 1); exit}' "${file}")"
    if [[ -n "${value}" ]]; then
      printf '%s\n' "${value}"
      return 0
    fi
  done
  return 0
}

validate_port_value() {
  local name="$1" value="$2"
  if ! [[ "${value}" =~ ^[0-9]+$ ]] || (( value < 1 || value > 65535 )); then
    echo "ERROR: ${name} must be a port number (1-65535), got: ${value}" >&2
    exit 1
  fi
}

mcp_host_is_loopback() {
  case "$1" in
    127.*|::1) return 0 ;;
    *) return 1 ;;
  esac
}

strip_trailing_mcp_path() {
  local url="${1%/}"
  [[ "${url}" == */mcp ]] && url="${url%/mcp}"
  printf '%s\n' "${url}"
}

derive_public_url() {
  if [[ -n "${PUBLIC_URL}" ]]; then
    strip_trailing_mcp_path "${PUBLIC_URL}"
    return
  fi
  if [[ "${SERVER_URL}" == https://* ]]; then
    strip_trailing_mcp_path "${SERVER_URL}"
  fi
}

codex_oauth_callback_url() {
  local config="${HOME}/.codex/config.toml"
  [[ -f "${config}" ]] || return 0
  awk -F= '
    $1 ~ /^[[:space:]]*mcp_oauth_callback_url[[:space:]]*$/ {
      value = $2
      sub(/^[[:space:]]*"/, "", value)
      sub(/"[[:space:]]*$/, "", value)
      print value
      exit
    }
  ' "${config}"
}

append_csv_unique() {
  local csv="$1" value="$2"
  [[ -n "${value}" ]] || { printf '%s\n' "${csv}"; return; }
  local existing
  IFS=',' read -r -a existing <<< "${csv}"
  for item in "${existing[@]}"; do
    item="${item#"${item%%[![:space:]]*}"}"
    item="${item%"${item##*[![:space:]]}"}"
    [[ "${item}" == "${value}" ]] && { printf '%s\n' "${csv}"; return; }
  done
  if [[ -n "${csv}" ]]; then
    printf '%s,%s\n' "${csv}" "${value}"
  else
    printf '%s\n' "${value}"
  fi
}

# ── Seed token from existing env so redeploy doesn't fail ─────────────────────

NO_AUTH="${CLAUDE_PLUGIN_OPTION_NO_AUTH:-$(existing_env_value NO_AUTH)}"
NO_AUTH="${NO_AUTH:-false}"
NO_AUTH="$(printf '%s' "${NO_AUTH}" | tr '[:upper:]' '[:lower:]')"

AUTH_MODE="${CLAUDE_PLUGIN_OPTION_AUTH_MODE:-$(existing_env_value APPRISE_MCP_AUTH_MODE)}"
AUTH_MODE="${AUTH_MODE:-bearer}"
AUTH_MODE="$(printf '%s' "${AUTH_MODE}" | tr '[:upper:]' '[:lower:]')"

if [[ "${NO_AUTH}" != "true" && -z "${CLAUDE_PLUGIN_OPTION_API_TOKEN:-}" ]]; then
  _tok="$(existing_env_value APPRISE_MCP_TOKEN)"
  [[ -n "${_tok}" ]] && CLAUDE_PLUGIN_OPTION_API_TOKEN="${_tok}"
  unset _tok
fi

# ── Config from userConfig ────────────────────────────────────────────────────

USE_DOCKER="${CLAUDE_PLUGIN_OPTION_USE_DOCKER:-false}"
API_TOKEN="${CLAUDE_PLUGIN_OPTION_API_TOKEN:-}"
SERVER_URL="${CLAUDE_PLUGIN_OPTION_SERVER_URL:-http://localhost:8765}"
MCP_HOST="${APPRISE_MCP_HOST:-0.0.0.0}"
MCP_PORT="${APPRISE_MCP_PORT:-8765}"
validate_port_value APPRISE_MCP_PORT "${MCP_PORT}"

PUBLIC_URL="${CLAUDE_PLUGIN_OPTION_PUBLIC_URL:-$(existing_env_value APPRISE_MCP_PUBLIC_URL)}"
GOOGLE_CLIENT_ID="${CLAUDE_PLUGIN_OPTION_GOOGLE_CLIENT_ID:-$(existing_env_value APPRISE_MCP_GOOGLE_CLIENT_ID)}"
GOOGLE_CLIENT_SECRET="${CLAUDE_PLUGIN_OPTION_GOOGLE_CLIENT_SECRET:-$(existing_env_value APPRISE_MCP_GOOGLE_CLIENT_SECRET)}"
AUTH_ADMIN_EMAIL="${CLAUDE_PLUGIN_OPTION_AUTH_ADMIN_EMAIL:-$(existing_env_value APPRISE_MCP_AUTH_ADMIN_EMAIL)}"
APPRISE_URL="${CLAUDE_PLUGIN_OPTION_APPRISE_URL:-$(existing_env_value APPRISE_URL)}"
APPRISE_URL="${APPRISE_URL:-http://localhost:8000}"
APPRISE_TOKEN="${CLAUDE_PLUGIN_OPTION_APPRISE_TOKEN:-$(existing_env_value APPRISE_TOKEN)}"

# ── Auth validation ───────────────────────────────────────────────────────────

if [[ "${NO_AUTH}" != "true" && -z "${API_TOKEN}" ]]; then
  if ! [[ "${AUTH_MODE}" == "oauth" ]] || ! mcp_host_is_loopback "${MCP_HOST}"; then
    echo "ERROR: api_token is required unless no_auth=true or OAuth mode binds MCP to loopback" >&2
    exit 1
  fi
fi

# ── Paths ─────────────────────────────────────────────────────────────────────

ENV_FILE="${CLAUDE_PLUGIN_DATA}/.env"
UNIT_FILE="${HOME}/.config/systemd/user/apprise-mcp.service"
COMPOSE_DIR="${CLAUDE_PLUGIN_DATA}"
COMPOSE_FILE="${COMPOSE_DIR}/docker-compose.yml"

# ── OAuth env block ───────────────────────────────────────────────────────────

oauth_env_block() {
  [[ "${NO_AUTH}" == "true" ]] && return 0
  [[ "${AUTH_MODE}" == "oauth" ]] || return 0

  local public_url
  public_url="$(derive_public_url)"
  if [[ -z "${public_url}" ]]; then
    echo "ERROR: OAuth mode requires public_url or an https server_url" >&2
    return 1
  fi
  if [[ -z "${GOOGLE_CLIENT_ID}" || -z "${GOOGLE_CLIENT_SECRET}" || -z "${AUTH_ADMIN_EMAIL}" ]]; then
    echo "ERROR: OAuth mode requires google_client_id, google_client_secret, and auth_admin_email" >&2
    return 1
  fi

  local redirects=""
  redirects="$(append_csv_unique "${redirects}" "https://claude.ai/api/mcp/auth_callback")"
  redirects="$(append_csv_unique "${redirects}" "https://claudeai.ai/api/mcp/auth_callback")"

  local codex_callback
  codex_callback="$(codex_oauth_callback_url)"
  [[ -n "${codex_callback}" ]] && redirects="$(append_csv_unique "${redirects}" "${codex_callback}")"

  cat << EOF
APPRISE_MCP_AUTH_MODE=oauth
APPRISE_MCP_PUBLIC_URL=${public_url}
APPRISE_MCP_GOOGLE_CLIENT_ID=${GOOGLE_CLIENT_ID}
APPRISE_MCP_GOOGLE_CLIENT_SECRET=${GOOGLE_CLIENT_SECRET}
APPRISE_MCP_AUTH_ADMIN_EMAIL=${AUTH_ADMIN_EMAIL}
APPRISE_MCP_AUTH_ALLOWED_REDIRECT_URIS=${redirects}
APPRISE_MCP_AUTH_DISABLE_STATIC_TOKEN_WITH_OAUTH=false
EOF
}

# ── Env file writer ───────────────────────────────────────────────────────────

# Returns 0 if written/changed, 1 if unchanged, 2 on error
write_env() {
  mkdir -p "${CLAUDE_PLUGIN_DATA}"

  local new_env
  new_env=$(cat << EOF
APPRISE_URL=${APPRISE_URL}
APPRISE_MCP_HOST=${MCP_HOST}
APPRISE_MCP_PORT=${MCP_PORT}
NO_AUTH=${NO_AUTH}
EOF
)

  [[ -n "${APPRISE_TOKEN}" ]] && new_env="${new_env}
APPRISE_TOKEN=${APPRISE_TOKEN}"

  [[ "${NO_AUTH}" != "true" && -n "${API_TOKEN}" ]] && new_env="${new_env}
APPRISE_MCP_TOKEN=${API_TOKEN}"

  if [[ "${USE_DOCKER}" == "true" ]]; then
    new_env="${new_env}
APPRISE_UID=$(id -u)
APPRISE_GID=$(id -g)"
  fi

  local auth_block
  if ! auth_block="$(oauth_env_block)"; then
    return 2
  fi
  [[ -n "${auth_block}" ]] && new_env="${new_env}
${auth_block}"

  if [[ -f "${ENV_FILE}" ]] && diff -q <(printf '%s\n' "${new_env}") "${ENV_FILE}" >/dev/null 2>&1; then
    return 1  # unchanged
  fi

  printf '%s\n' "${new_env}" > "${ENV_FILE}"
  chmod 600 "${ENV_FILE}"
  return 0  # changed
}

ensure_env_written() {
  local rc=0
  write_env || rc=$?
  [[ "${rc}" -le 1 ]] || return "${rc}"
  return 0
}

# ── Systemd deployment ────────────────────────────────────────────────────────

setup_systemd() {
  mkdir -p "${HOME}/.config/systemd/user"

  if [[ ! -x "${CLAUDE_PLUGIN_ROOT}/bin/apprise" ]]; then
    echo "ERROR: apprise binary not found at ${CLAUDE_PLUGIN_ROOT}/bin/apprise" >&2
    return 1
  fi

  # Port conflict check (skip if service already owns the port)
  if ! systemctl --user is-active --quiet apprise-mcp.service 2>/dev/null; then
    if ss -tlnp "sport = :${MCP_PORT}" 2>/dev/null | awk 'NR>1 && NF>0' | grep -q .; then
      echo "ERROR: port ${MCP_PORT}/tcp is already in use — cannot start apprise-mcp" >&2
      return 1
    fi
  fi

  # Stop Docker container if one exists
  if [[ -f "${COMPOSE_FILE}" ]] && command -v docker >/dev/null 2>&1; then
    if (cd "${COMPOSE_DIR}" && docker compose ps --quiet apprise-mcp 2>/dev/null | grep -q .); then
      echo "apprise-mcp: stopping existing docker container before systemd cutover"
      (cd "${COMPOSE_DIR}" && docker compose down)
    fi
  fi

  local new_unit
  new_unit=$(cat << EOF
[Unit]
Description=apprise-mcp server
After=network.target

[Service]
ExecStart=${CLAUDE_PLUGIN_ROOT}/bin/apprise serve mcp
EnvironmentFile=${ENV_FILE}
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF
)

  local unit_changed=false
  if ! diff -q <(printf '%s\n' "${new_unit}") "${UNIT_FILE}" >/dev/null 2>&1; then
    printf '%s\n' "${new_unit}" > "${UNIT_FILE}"
    unit_changed=true
  fi

  ensure_env_written

  if [[ "${unit_changed}" == "true" ]]; then
    systemctl --user daemon-reload
    systemctl --user enable --now apprise-mcp.service
  else
    systemctl --user restart apprise-mcp.service
  fi

  echo "apprise-mcp: systemd service running on ${MCP_HOST}:${MCP_PORT}"
}

# ── Docker deployment ─────────────────────────────────────────────────────────

setup_docker() {
  mkdir -p "${COMPOSE_DIR}"

  if ! docker info >/dev/null 2>&1; then
    echo "ERROR: docker daemon is not reachable — is dockerd running?" >&2
    return 1
  fi

  # Port conflict check (skip if container already running)
  local container_running=false
  if [[ -f "${COMPOSE_FILE}" ]] && \
     docker compose -f "${COMPOSE_FILE}" ps --quiet apprise-mcp 2>/dev/null | grep -q .; then
    container_running=true
  fi

  if [[ "${container_running}" == "false" ]]; then
    if ss -tlnp "sport = :${MCP_PORT}" 2>/dev/null | awk 'NR>1 && NF>0' | grep -q .; then
      echo "ERROR: port ${MCP_PORT}/tcp is already in use — cannot start apprise-mcp" >&2
      return 1
    fi
  fi

  # Remove systemd unit if switching from binary to docker
  if systemctl --user list-unit-files apprise-mcp.service >/dev/null 2>&1; then
    systemctl --user is-active --quiet apprise-mcp.service && \
      systemctl --user stop apprise-mcp.service || true
    systemctl --user is-enabled --quiet apprise-mcp.service 2>/dev/null && \
      systemctl --user disable apprise-mcp.service >/dev/null 2>&1 || true
    [[ -f "${UNIT_FILE}" ]] && rm -f "${UNIT_FILE}" && systemctl --user daemon-reload || true
  fi

  # Refresh compose file from plugin root
  if ! diff -q "${CLAUDE_PLUGIN_ROOT}/docker-compose.yml" "${COMPOSE_FILE}" >/dev/null 2>&1; then
    cp "${CLAUDE_PLUGIN_ROOT}/docker-compose.yml" "${COMPOSE_FILE}"
  fi

  ensure_env_written
  cd "${COMPOSE_DIR}"

  # Ensure external network exists
  local network_name="${DOCKER_NETWORK:-jakenet}"
  if ! docker network inspect "${network_name}" >/dev/null 2>&1; then
    echo "apprise-mcp: creating docker network ${network_name}"
    docker network create "${network_name}"
  fi

  # Build locally if source is available, otherwise pull
  if [[ "${CLAUDE_PLUGIN_OPTION_BUILD_LOCAL:-false}" == "true" && \
        -f "${CLAUDE_PLUGIN_ROOT}/Cargo.toml" && \
        -f "${CLAUDE_PLUGIN_ROOT}/config/Dockerfile" ]]; then
    (cd "${CLAUDE_PLUGIN_ROOT}" && docker compose build --no-cache apprise-mcp)
  else
    docker compose pull --quiet apprise-mcp 2>&1 || \
      echo "apprise-mcp: pull failed; will try cached image" >&2
  fi

  if docker compose ps --quiet apprise-mcp 2>/dev/null | grep -q .; then
    docker compose up -d --force-recreate --no-build
  else
    docker compose up -d --no-build
  fi

  echo "apprise-mcp: docker container running on ${MCP_HOST}:${MCP_PORT}"
}

# ── Client-only validation ────────────────────────────────────────────────────

validate_client() {
  if curl -sf "${SERVER_URL}/health" >/dev/null 2>&1; then
    echo "apprise-mcp: connected to ${SERVER_URL}"
  else
    echo "WARNING: apprise-mcp server at ${SERVER_URL} is not reachable" >&2
  fi
}

# ── Binary symlink ────────────────────────────────────────────────────────────

link_binary() {
  mkdir -p "${HOME}/.local/bin"
  if [[ -f "${CLAUDE_PLUGIN_ROOT}/bin/apprise" ]]; then
    ln -sf "${CLAUDE_PLUGIN_ROOT}/bin/apprise" "${HOME}/.local/bin/apprise"
  fi
}

# ── Main ──────────────────────────────────────────────────────────────────────

link_binary

if [[ "${USE_DOCKER}" == "true" ]]; then
  setup_docker
elif [[ -f "${CLAUDE_PLUGIN_ROOT}/bin/apprise" ]]; then
  setup_systemd
else
  validate_client
fi
