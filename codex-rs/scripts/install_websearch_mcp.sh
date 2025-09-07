#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Install the simple websearch MCP script to a preferred location and update Codex config.

Usage:
  scripts/install_websearch_mcp.sh [--local|--global] [--force]

Options:
  --local   Install to ./ocodex/.codex (project-local). Default.
  --global  Install to /usr/local/share/ocodex/.codex (system-wide).
  --force   Overwrite existing file.

Notes:
  - This copies codex-rs/scripts/mcp_websearch.js to the destination and updates
    the MCP server config via ../scripts/setup_mcp_from_env.sh.
  - Expects a project .env at repo root containing GOOGLE_API_KEY / GOOGLE_CSE_ID (and optional SERPAPI_KEY).
USAGE
}

scope="auto"
force=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --local) scope="local"; shift ;;
    --global) scope="global"; shift ;;
    --force) force=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Detect Docker/OCI container heuristically
is_docker=0
if [[ -f "/.dockerenv" ]]; then is_docker=1; fi
if [[ ${is_docker} -eq 0 && -r "/proc/1/cgroup" ]] && grep -qiE '(docker|containerd|kubepods|podman)' /proc/1/cgroup; then is_docker=1; fi
if [[ ${is_docker} -eq 0 && -n "${container:-}" ]]; then is_docker=1; fi

# If scope is auto and we are in a container, prefer global share install
if [[ "$scope" == "auto" ]]; then
  if [[ $is_docker -eq 1 ]]; then scope="global"; else scope="local"; fi
fi
SRC="$REPO_ROOT/scripts/mcp_websearch.js"
if [[ ! -f "$SRC" ]]; then
  echo "missing source: $SRC" >&2
  exit 1
fi

if [[ "$scope" == "local" ]]; then
  DEST_DIR="$REPO_ROOT/ocodex/.codex"
else
  DEST_DIR="/usr/local/share/ocodex/.codex"
fi
DEST="$DEST_DIR/mcp_websearch.js"

mkdir -p "$DEST_DIR"
if [[ -f "$DEST" && $force -ne 1 ]]; then
  echo "exists: $DEST (use --force to overwrite)" >&2
else
  cp "$SRC" "$DEST"
  echo "Installed: $DEST"
fi

# Update MCP server config to use destination path
if [[ "$scope" == "local" ]]; then
  bash "$REPO_ROOT/../scripts/setup_mcp_from_env.sh" \
    --local --server search --command node --arg "$DEST" \
    --env-key GOOGLE_API_KEY --env-key GOOGLE_CSE_ID --env-key SERPAPI_KEY
  echo "Launch with: CODEX_HOME=\"$REPO_ROOT/ocodex/.codex\" ocodex"
else
  if [[ $EUID -ne 0 && ! -w "$DEST_DIR" ]]; then
    echo "warning: destination may require sudo for install; rerun with sudo if copy failed." >&2
  fi
  # Use global-share config dir for container-friendly default
  bash "$REPO_ROOT/../scripts/setup_mcp_from_env.sh" \
    --global-share --server search --command node --arg "$DEST" \
    --env-key GOOGLE_API_KEY --env-key GOOGLE_CSE_ID --env-key SERPAPI_KEY
  echo "Global-share config updated at: /usr/local/share/ocodex/.codex/config.toml"
  echo "Run ocodex with: CODEX_HOME=\"/usr/local/share/ocodex/.codex\" ocodex"
fi

exit 0
