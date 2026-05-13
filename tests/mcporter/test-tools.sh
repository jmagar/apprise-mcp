#!/usr/bin/env bash
# =============================================================================
# test-tools.sh — Integration smoke-test for apprise-mcp MCP server tools
#
# Tests every non-destructive action against a running apprise-mcp server:
#   apprise health   — verifies the Apprise API server is reachable
#   apprise help     — verifies the help documentation is returned
#   apprise notify   — sends to APPRISE_TEST_TAG (skipped if not configured)
#   apprise notify_url — (skipped unless APPRISE_TEST_URL_SCHEMA is set)
#
# Credentials are sourced from ~/.claude-homelab/.env:
#   APPRISE_MCP_HOST   (default: localhost)
#   APPRISE_MCP_PORT   (default: 8765)
#   APPRISE_MCP_TOKEN  (optional)
#
# Notify test config (optional):
#   APPRISE_TEST_TAG        Tag to notify (e.g. "test"). Skipped if unset.
#   APPRISE_TEST_URL_SCHEMA Apprise URL schema for notify_url (e.g. "slack://..."). Skipped if unset.
#
# Usage:
#   ./tests/mcporter/test-tools.sh [--timeout-ms N] [--verbose]
#
# Exit codes:
#   0 — all tests passed or skipped
#   1 — one or more tests failed
#   2 — prerequisite check failed (mcporter not found, server unreachable)
# =============================================================================

set -uo pipefail

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------
readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
readonly PROJECT_DIR="$(cd -- "${SCRIPT_DIR}/../.." && pwd -P)"
readonly SCRIPT_NAME="$(basename -- "${BASH_SOURCE[0]}")"
readonly TS_START="$(date +%s%N)"
readonly LOG_FILE="${TMPDIR:-/tmp}/${SCRIPT_NAME%.sh}.$(date +%Y%m%d-%H%M%S).log"
readonly ENV_FILE="${HOME}/.claude-homelab/.env"

# Colours (disabled when stdout is not a terminal)
if [[ -t 1 ]]; then
  C_RESET='\033[0m'
  C_BOLD='\033[1m'
  C_GREEN='\033[0;32m'
  C_RED='\033[0;31m'
  C_YELLOW='\033[0;33m'
  C_CYAN='\033[0;36m'
  C_DIM='\033[2m'
else
  C_RESET='' C_BOLD='' C_GREEN='' C_RED='' C_YELLOW='' C_CYAN='' C_DIM=''
fi

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------
CALL_TIMEOUT_MS=25000
VERBOSE=false

# ---------------------------------------------------------------------------
# Counters
# ---------------------------------------------------------------------------
PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0
declare -a FAIL_NAMES=()

# Runtime globals — populated after ENV load
MCP_URL=''
MCPORTER_HEADER_ARGS=()

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --timeout-ms)
        CALL_TIMEOUT_MS="${2:?--timeout-ms requires a value}"
        shift 2
        ;;
      --verbose)
        VERBOSE=true
        shift
        ;;
      -h|--help)
        printf 'Usage: %s [--timeout-ms N] [--verbose]\n' "${SCRIPT_NAME}"
        printf '\nEnvironment:\n'
        printf '  APPRISE_TEST_TAG        Send a test notification to this tag (skipped if unset)\n'
        printf '  APPRISE_TEST_URL_SCHEMA Send a notify_url test using this schema (skipped if unset)\n'
        exit 0
        ;;
      *)
        printf '[ERROR] Unknown argument: %s\n' "$1" >&2
        exit 2
        ;;
    esac
  done
}

# ---------------------------------------------------------------------------
# Logging
# ---------------------------------------------------------------------------
log_info()  { printf "${C_CYAN}[INFO]${C_RESET}  %s\n" "$*" | tee -a "${LOG_FILE}"; }
log_warn()  { printf "${C_YELLOW}[WARN]${C_RESET}  %s\n" "$*" | tee -a "${LOG_FILE}"; }
log_error() { printf "${C_RED}[ERROR]${C_RESET} %s\n" "$*" | tee -a "${LOG_FILE}" >&2; }

cleanup() {
  local rc=$?
  [[ $rc -ne 0 ]] && log_warn "Script exited with rc=${rc}. Log: ${LOG_FILE}"
}
trap cleanup EXIT

# ---------------------------------------------------------------------------
# Load environment
# ---------------------------------------------------------------------------
load_env() {
  if [[ -f "${ENV_FILE}" ]]; then
    set -a
    # shellcheck disable=SC1090
    source "${ENV_FILE}"
    set +a
    log_info "Loaded credentials from ${ENV_FILE}"
  else
    log_warn "${ENV_FILE} not found — using defaults / environment"
  fi

  local host="${APPRISE_MCP_HOST:-localhost}"
  # Remap 0.0.0.0 (bind address) to localhost for outbound connections.
  [[ "${host}" == "0.0.0.0" ]] && host="localhost"
  local port="${APPRISE_MCP_PORT:-8765}"
  MCP_URL="http://${host}:${port}/mcp"

  local token="${APPRISE_MCP_TOKEN:-}"
  MCPORTER_HEADER_ARGS=()
  if [[ -n "${token}" ]]; then
    MCPORTER_HEADER_ARGS+=(--header "Authorization: Bearer ${token}")
  fi

  log_info "MCP URL: ${MCP_URL}"
  if [[ ${#MCPORTER_HEADER_ARGS[@]} -gt 0 ]]; then
    log_info "Auth: Bearer token configured"
  else
    log_info "Auth: none (APPRISE_MCP_TOKEN unset)"
  fi
}

# ---------------------------------------------------------------------------
# Prerequisites
# ---------------------------------------------------------------------------
check_prerequisites() {
  local missing=false

  if ! command -v mcporter &>/dev/null; then
    log_error "mcporter not found in PATH. Install it and re-run."
    missing=true
  fi

  if ! command -v python3 &>/dev/null; then
    log_error "python3 not found in PATH."
    missing=true
  fi

  if ! command -v curl &>/dev/null; then
    log_error "curl not found in PATH."
    missing=true
  fi

  [[ "${missing}" == false ]]
}

# ---------------------------------------------------------------------------
# Server connectivity smoke-test
# ---------------------------------------------------------------------------
smoke_test_server() {
  log_info "Smoke-testing server connectivity..."

  local base_url="${MCP_URL%/mcp}"

  # 1. Health endpoint (unauthenticated)
  local health_out
  health_out="$(curl -sf --max-time 10 "${base_url}/health" 2>/dev/null)" || health_out=''

  # Apprise-mcp returns {"status":"ok"} or {"health":"ok"} depending on version
  local health_ok
  health_ok="$(printf '%s' "${health_out}" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    # Accept status=ok or health=ok
    val = str(d.get('status', d.get('health', ''))).lower()
    print('ok' if val in ('ok', 'healthy', 'up') else 'bad:' + val)
except Exception as e:
    print('parse_error: ' + str(e))
" 2>/dev/null)" || health_ok="error"

  if [[ "${health_ok}" != "ok" ]]; then
    log_error "Health endpoint at ${base_url}/health did not return a healthy status (got: '${health_ok}')"
    log_error "Is apprise-mcp running?  docker ps | grep apprise-mcp"
    return 2
  fi
  log_info "Health endpoint OK"

  # 2. tools/list — confirm MCP layer is up
  local tool_count
  tool_count="$(
    curl -sf --max-time 10 \
      -X POST "${MCP_URL}" \
      -H "Content-Type: application/json" \
      -H "Accept: application/json, text/event-stream" \
      ${MCPORTER_HEADER_ARGS[@]+"${MCPORTER_HEADER_ARGS[@]}"} \
      -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' 2>/dev/null | \
    python3 -c "
import sys, json
d = json.load(sys.stdin)
tools = d.get('result', {}).get('tools', [])
print(len(tools))
" 2>/dev/null
  )" || tool_count=0

  if [[ "${tool_count:-0}" -lt 1 ]]; then
    log_error "tools/list returned ${tool_count:-0} tools — expected at least 1"
    return 2
  fi

  log_info "Server OK — ${tool_count} tool(s) available"
  return 0
}

# ---------------------------------------------------------------------------
# mcporter call wrapper
#   Usage: mcporter_call <tool> <args_json>
# ---------------------------------------------------------------------------
mcporter_call() {
  local tool="${1:?tool required}"
  local args_json="${2:?args_json required}"

  mcporter call \
    --http-url "${MCP_URL}" \
    --allow-http \
    ${MCPORTER_HEADER_ARGS[@]+"${MCPORTER_HEADER_ARGS[@]}"} \
    --tool "${tool}" \
    --args "${args_json}" \
    --timeout "${CALL_TIMEOUT_MS}" \
    --output json \
    2>>"${LOG_FILE}"
}

# ---------------------------------------------------------------------------
# Test runner
#   Usage: run_test <label> <tool> <args_json> [expected_key] [expected_value]
#
#   expected_key: dot-notation path into the JSON response, e.g. "help" or "ok"
#   expected_value: if set, the key's value must equal this string (case-insensitive)
# ---------------------------------------------------------------------------
run_test() {
  local label="${1:?label required}"
  local tool="${2:?tool required}"
  local args_json="${3:?args_json required}"
  local expected_key="${4:-}"
  local expected_value="${5:-}"

  local t0
  t0="$(date +%s%N)"

  local output
  output="$(mcporter_call "${tool}" "${args_json}")" || true

  local elapsed_ms
  elapsed_ms="$(( ( $(date +%s%N) - t0 ) / 1000000 ))"

  if [[ "${VERBOSE}" == true ]]; then
    printf '%s\n' "${output}" | tee -a "${LOG_FILE}"
  else
    printf '%s\n' "${output}" >> "${LOG_FILE}"
  fi

  # Check JSON parses and is not an error payload
  local json_check
  json_check="$(
    printf '%s' "${output}" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    if isinstance(d, dict) and ('error' in d or d.get('kind') == 'error'):
        print('error: ' + str(d.get('error', d.get('message', 'unknown'))))
    else:
        print('ok')
except Exception as e:
    print('invalid_json: ' + str(e))
" 2>/dev/null
  )" || json_check="parse_error"

  if [[ "${json_check}" != "ok" ]]; then
    printf "${C_RED}[FAIL]${C_RESET} %-60s ${C_DIM}%dms${C_RESET}\n" \
      "${label}" "${elapsed_ms}" | tee -a "${LOG_FILE}"
    printf '       response validation failed: %s\n' "${json_check}" | tee -a "${LOG_FILE}"
    FAIL_COUNT=$(( FAIL_COUNT + 1 ))
    FAIL_NAMES+=("${label}")
    return 1
  fi

  # Check key presence
  if [[ -n "${expected_key}" ]]; then
    local key_result
    key_result="$(
      printf '%s' "${output}" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    keys = '${expected_key}'.split('.')
    node = d
    for k in keys:
        if k:
            node = node[int(k)] if (isinstance(node, list) and k.isdigit()) else node[k]
    # Return the value for optional equality check
    print('ok|' + str(node))
except Exception as e:
    print('missing: ' + str(e))
" 2>/dev/null
    )" || key_result="parse_error"

    if [[ "${key_result}" != ok* ]]; then
      printf "${C_RED}[FAIL]${C_RESET} %-60s ${C_DIM}%dms${C_RESET}\n" \
        "${label}" "${elapsed_ms}" | tee -a "${LOG_FILE}"
      printf '       expected key .%s not found: %s\n' "${expected_key}" "${key_result}" | tee -a "${LOG_FILE}"
      FAIL_COUNT=$(( FAIL_COUNT + 1 ))
      FAIL_NAMES+=("${label}")
      return 1
    fi

    # Optional value check
    if [[ -n "${expected_value}" ]]; then
      local actual_value="${key_result#ok|}"
      if [[ "${actual_value,,}" != "${expected_value,,}" ]]; then
        printf "${C_RED}[FAIL]${C_RESET} %-60s ${C_DIM}%dms${C_RESET}\n" \
          "${label}" "${elapsed_ms}" | tee -a "${LOG_FILE}"
        printf '       .%s expected %q got %q\n' "${expected_key}" "${expected_value}" "${actual_value}" | tee -a "${LOG_FILE}"
        FAIL_COUNT=$(( FAIL_COUNT + 1 ))
        FAIL_NAMES+=("${label}")
        return 1
      fi
    fi
  fi

  printf "${C_GREEN}[PASS]${C_RESET} %-60s ${C_DIM}%dms${C_RESET}\n" \
    "${label}" "${elapsed_ms}" | tee -a "${LOG_FILE}"
  PASS_COUNT=$(( PASS_COUNT + 1 ))
  return 0
}

# ---------------------------------------------------------------------------
# Skip helper
# ---------------------------------------------------------------------------
skip_test() {
  local label="${1:?label required}"
  local reason="${2:-prerequisite not met}"
  printf "${C_YELLOW}[SKIP]${C_RESET} %-60s %s\n" "${label}" "${reason}" | tee -a "${LOG_FILE}"
  SKIP_COUNT=$(( SKIP_COUNT + 1 ))
}

# ---------------------------------------------------------------------------
# Auth enforcement tests
# ---------------------------------------------------------------------------
suite_auth() {
  if [[ -z "${APPRISE_MCP_TOKEN:-}" ]]; then
    printf '\n%b== auth (skipped — APPRISE_MCP_TOKEN unset) ==%b\n' "${C_BOLD}" "${C_RESET}" | tee -a "${LOG_FILE}"
    skip_test "auth: unauthenticated /mcp returns 401" "APPRISE_MCP_TOKEN unset"
    skip_test "auth: bad token returns 401"             "APPRISE_MCP_TOKEN unset"
    return
  fi

  printf '\n%b== auth enforcement ==%b\n' "${C_BOLD}" "${C_RESET}" | tee -a "${LOG_FILE}"

  local label status

  label="auth: unauthenticated /mcp returns 401"
  status="$(curl -s --max-time 10 -o /dev/null -w "%{http_code}" \
    -X POST "${MCP_URL}" \
    -H "Content-Type: application/json" \
    -H "Accept: application/json, text/event-stream" \
    -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' 2>/dev/null)" || status=0
  if [[ "${status}" == "401" ]]; then
    printf "${C_GREEN}[PASS]${C_RESET} %-60s\n" "${label}" | tee -a "${LOG_FILE}"
    PASS_COUNT=$(( PASS_COUNT + 1 ))
  else
    printf "${C_RED}[FAIL]${C_RESET} %-60s (got HTTP %s)\n" "${label}" "${status}" | tee -a "${LOG_FILE}"
    FAIL_COUNT=$(( FAIL_COUNT + 1 ))
    FAIL_NAMES+=("${label}")
  fi

  label="auth: bad token returns 401"
  status="$(curl -s --max-time 10 -o /dev/null -w "%{http_code}" \
    -X POST "${MCP_URL}" \
    -H "Authorization: Bearer intentionally-invalid-token" \
    -H "Content-Type: application/json" \
    -H "Accept: application/json, text/event-stream" \
    -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' 2>/dev/null)" || status=0
  if [[ "${status}" == "401" ]]; then
    printf "${C_GREEN}[PASS]${C_RESET} %-60s\n" "${label}" | tee -a "${LOG_FILE}"
    PASS_COUNT=$(( PASS_COUNT + 1 ))
  else
    printf "${C_RED}[FAIL]${C_RESET} %-60s (got HTTP %s)\n" "${label}" "${status}" | tee -a "${LOG_FILE}"
    FAIL_COUNT=$(( FAIL_COUNT + 1 ))
    FAIL_NAMES+=("${label}")
  fi
}

# ---------------------------------------------------------------------------
# Meta suite: health + help
# ---------------------------------------------------------------------------
suite_meta() {
  printf '\n%b== meta (health + help) ==%b\n' "${C_BOLD}" "${C_RESET}" | tee -a "${LOG_FILE}"

  # health: the MCP tool calls the upstream Apprise API /health — this means
  # a PASS here confirms real Apprise data is flowing.
  run_test "apprise health: returns ok status" \
    "apprise" '{"action":"health"}' \
    "status"

  run_test "apprise help: returns help documentation" \
    "apprise" '{"action":"help"}' \
    "help"

  # Verify help is non-empty
  local help_out
  help_out="$(mcporter_call apprise '{"action":"help"}' 2>/dev/null)" || help_out=''
  local help_len
  help_len="$(printf '%s' "${help_out}" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    h = d.get('help','')
    print(len(h))
except Exception:
    print(0)
" 2>/dev/null)" || help_len=0

  if [[ "${help_len:-0}" -gt 50 ]]; then
    printf "${C_GREEN}[PASS]${C_RESET} %-60s\n" \
      "apprise help: documentation is non-empty (${help_len} chars)" | tee -a "${LOG_FILE}"
    PASS_COUNT=$(( PASS_COUNT + 1 ))
  else
    printf "${C_RED}[FAIL]${C_RESET} %-60s\n" \
      "apprise help: documentation is non-empty (${help_len} chars)" | tee -a "${LOG_FILE}"
    FAIL_COUNT=$(( FAIL_COUNT + 1 ))
    FAIL_NAMES+=("apprise help: documentation is non-empty")
  fi
}

# ---------------------------------------------------------------------------
# Notify suite: only runs when APPRISE_TEST_TAG is configured
# ---------------------------------------------------------------------------
suite_notify() {
  printf '\n%b== notify ==%b\n' "${C_BOLD}" "${C_RESET}" | tee -a "${LOG_FILE}"

  local test_tag="${APPRISE_TEST_TAG:-}"

  if [[ -z "${test_tag}" ]]; then
    skip_test "apprise notify: send to test tag" \
      "set APPRISE_TEST_TAG=<tag> to enable (e.g. APPRISE_TEST_TAG=test)"
    skip_test "apprise notify: response indicates success" \
      "set APPRISE_TEST_TAG=<tag> to enable"
  else
    local args
    args="$(python3 -c "
import json, sys
print(json.dumps({
  'action': 'notify',
  'tag': '${test_tag}',
  'title': 'apprise-mcp smoke test',
  'body': 'Test notification from test-tools.sh (automated)',
  'type': 'info'
}))
" 2>/dev/null)"

    run_test "apprise notify: send to tag '${test_tag}'" \
      "apprise" "${args}" \
      "ok"

    # Validate ok=true
    run_test "apprise notify: ok field is true" \
      "apprise" "${args}" \
      "ok" "true"
  fi

  # notify_url: only when APPRISE_TEST_URL_SCHEMA is set
  local test_schema="${APPRISE_TEST_URL_SCHEMA:-}"

  if [[ -z "${test_schema}" ]]; then
    skip_test "apprise notify_url: stateless one-off send" \
      "set APPRISE_TEST_URL_SCHEMA=<schema> to enable (e.g. slack://...)"
  else
    local url_args
    url_args="$(python3 -c "
import json, sys
print(json.dumps({
  'action': 'notify_url',
  'urls': '${test_schema}',
  'body': 'Test one-off notification from test-tools.sh',
  'title': 'apprise-mcp smoke test',
  'type': 'info'
}))
" 2>/dev/null)"
    run_test "apprise notify_url: stateless one-off send" \
      "apprise" "${url_args}" \
      "ok"
  fi
}

# ---------------------------------------------------------------------------
# Schema resource test
# ---------------------------------------------------------------------------
suite_schema_resource() {
  printf '\n%b== schema resource ==%b\n' "${C_BOLD}" "${C_RESET}" | tee -a "${LOG_FILE}"

  # Test resources/list
  local resources_out
  resources_out="$(
    curl -sf --max-time 10 \
      -X POST "${MCP_URL}" \
      -H "Content-Type: application/json" \
      -H "Accept: application/json, text/event-stream" \
      ${MCPORTER_HEADER_ARGS[@]+"${MCPORTER_HEADER_ARGS[@]}"} \
      -d '{"jsonrpc":"2.0","id":1,"method":"resources/list","params":{}}' 2>/dev/null
  )" || resources_out=''

  local resource_count
  resource_count="$(printf '%s' "${resources_out}" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    resources = d.get('result', {}).get('resources', [])
    print(len(resources))
except Exception:
    print(0)
" 2>/dev/null)" || resource_count=0

  if [[ "${resource_count:-0}" -gt 0 ]]; then
    printf "${C_GREEN}[PASS]${C_RESET} %-60s\n" \
      "apprise: resources/list returns ${resource_count} resource(s)" | tee -a "${LOG_FILE}"
    PASS_COUNT=$(( PASS_COUNT + 1 ))
  else
    # Not all servers expose resources — treat as skip not fail
    skip_test "apprise: schema resource (apprise://schema/mcp-tool)" \
      "no resources exposed by this server version"
    return
  fi

  # Check for the apprise://schema/mcp-tool resource specifically
  local has_schema
  has_schema="$(printf '%s' "${resources_out}" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    resources = d.get('result', {}).get('resources', [])
    uris = [r.get('uri','') for r in resources]
    found = any('schema' in u.lower() or 'mcp-tool' in u.lower() for u in uris)
    print('found' if found else 'not_found:' + str(uris))
except Exception as e:
    print('error: ' + str(e))
" 2>/dev/null)" || has_schema="error"

  if [[ "${has_schema}" == "found" ]]; then
    printf "${C_GREEN}[PASS]${C_RESET} %-60s\n" \
      "apprise: schema resource present" | tee -a "${LOG_FILE}"
    PASS_COUNT=$(( PASS_COUNT + 1 ))
  else
    skip_test "apprise: schema resource (apprise://schema/mcp-tool)" \
      "not present in resources list: ${has_schema}"
  fi
}

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
print_summary() {
  local total_ms
  total_ms="$(( ( $(date +%s%N) - TS_START ) / 1000000 ))"
  local total=$(( PASS_COUNT + FAIL_COUNT + SKIP_COUNT ))

  printf '\n%b%s%b\n' "${C_BOLD}" "$(printf '=%.0s' {1..65})" "${C_RESET}"
  printf '%b%-20s%b  %b%d%b\n' "${C_BOLD}" "PASS" "${C_RESET}" "${C_GREEN}"  "${PASS_COUNT}" "${C_RESET}"
  printf '%b%-20s%b  %b%d%b\n' "${C_BOLD}" "FAIL" "${C_RESET}" "${C_RED}"   "${FAIL_COUNT}" "${C_RESET}"
  printf '%b%-20s%b  %b%d%b\n' "${C_BOLD}" "SKIP" "${C_RESET}" "${C_YELLOW}" "${SKIP_COUNT}" "${C_RESET}"
  printf '%b%-20s%b  %d\n'     "${C_BOLD}" "TOTAL" "${C_RESET}" "${total}"
  printf '%b%-20s%b  %ds (%dms)\n' "${C_BOLD}" "ELAPSED" "${C_RESET}" \
    "$(( total_ms / 1000 ))" "${total_ms}"
  printf '%b%s%b\n' "${C_BOLD}" "$(printf '=%.0s' {1..65})" "${C_RESET}"

  if [[ "${FAIL_COUNT}" -gt 0 ]]; then
    printf '\n%bFailed tests:%b\n' "${C_RED}" "${C_RESET}"
    local name
    for name in "${FAIL_NAMES[@]}"; do
      printf '  * %s\n' "${name}"
    done
    printf '\nFull log: %s\n' "${LOG_FILE}"
  fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
main() {
  parse_args "$@"
  load_env

  printf '%b%s%b\n' "${C_BOLD}" "$(printf '=%.0s' {1..65})" "${C_RESET}"
  printf '%b  apprise-mcp integration smoke-test%b\n' "${C_BOLD}" "${C_RESET}"
  printf '%b  Project:   %s%b\n' "${C_BOLD}" "${PROJECT_DIR}" "${C_RESET}"
  printf '%b  MCP URL:   %s%b\n' "${C_BOLD}" "${MCP_URL}" "${C_RESET}"
  printf '%b  Timeout:   %dms/call%b\n' "${C_BOLD}" "${CALL_TIMEOUT_MS}" "${C_RESET}"
  printf '%b  Test tag:  %s%b\n' "${C_BOLD}" "${APPRISE_TEST_TAG:-(not set — notify skipped)}" "${C_RESET}"
  printf '%b  Log:       %s%b\n' "${C_BOLD}" "${LOG_FILE}" "${C_RESET}"
  printf '%b%s%b\n\n' "${C_BOLD}" "$(printf '=%.0s' {1..65})" "${C_RESET}"

  check_prerequisites || exit 2

  smoke_test_server || {
    log_error ""
    log_error "Server connectivity check failed. Aborting — no tests will run."
    log_error ""
    log_error "To diagnose:"
    log_error "  docker ps | grep apprise-mcp"
    log_error "  curl http://localhost:8765/health"
    log_error "  apprise serve mcp  (or just dev)"
    exit 2
  }

  suite_auth
  suite_meta
  suite_notify
  suite_schema_resource

  print_summary

  if [[ "${FAIL_COUNT}" -gt 0 ]]; then
    exit 1
  fi
  exit 0
}

main "$@"
