#!/bin/sh
# entrypoint.sh — Docker entrypoint for apprise-mcp
# Runs as root, validates config, then exec's the service as 1000:1000
set -e

DATA_DIR="${DATA_DIR:-/data}"

# Validate required env vars (fail fast before dropping privileges)
if [ -z "${APPRISE_URL:-}" ]; then
    echo "ERROR: APPRISE_URL is not set — set it to your Apprise API server URL" >&2
    exit 1
fi

# Ensure data directory exists and is owned by the service user
mkdir -p "${DATA_DIR}"
chown -R 1000:1000 "${DATA_DIR}"
chmod 750 "${DATA_DIR}"

# Lock down config files if present
if [ -f "${DATA_DIR}/config.toml" ]; then
    chmod 640 "${DATA_DIR}/config.toml"
fi
if [ -f "${DATA_DIR}/.env" ]; then
    chmod 600 "${DATA_DIR}/.env"
fi

# Drop to service user and exec the binary
exec su-exec 1000:1000 "$@"
