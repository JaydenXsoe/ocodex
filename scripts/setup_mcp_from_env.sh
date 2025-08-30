#!/usr/bin/env bash
# Synchronize MCP server configuration from a project .env into Codex config.toml.
#
# Usage:
#   scripts/setup_mcp_from_env.sh [--local|--global] \
#       --server <name> \
#       --command <program> \
#       --arg <arg> [--arg <arg> ...] \
#       [--env-key <NAME> ...]
#
# Examples:
#   # Configure the bundled websearch MCP server using project-local CODEX_HOME
#   scripts/setup_mcp_from_env.sh \
#     --local \
#     --server search \
#     --command node \
#     --arg "$(pwd)/codex-rs/scripts/mcp_websearch.js" \
#     --env-key GOOGLE_API_KEY --env-key GOOGLE_CSE_ID --env-key SERPAPI_KEY
#
#   # Configure a generic npm MCP server globally
#   scripts/setup_mcp_from_env.sh \
#     --global \
#     --server my-server \
#     --command npx \
#     --arg -y --arg @vendor/mcp-server \
#     --env-key API_KEY --env-key OTHER_TOKEN
#
# Notes:
# - --local writes to "$PROJECT_ROOT/ocodex/.codex/config.toml" and exports
#   CODEX_HOME you can use when launching ocodex: `CODEX_HOME=ocodex/.codex ocodex`.
# - --global writes to "$HOME/.codex/config.toml".
# - The script updates (replaces) only the [mcp_servers.<name>] block.
# - Values are read from the project .env ("$PROJECT_ROOT/.env").

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DOT_ENV="$PROJECT_ROOT/.env"

scope="local"
server=""
command_prog=""
args=()
env_keys=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --local) scope="local"; shift ;;
    --global) scope="global"; shift ;;
    --server) server="${2:-}"; shift 2 ;;
    --command) command_prog="${2:-}"; shift 2 ;;
    --arg) args+=("${2:-}"); shift 2 ;;
    --env-key) env_keys+=("${2:-}"); shift 2 ;;
    -h|--help)
      sed -n '1,80p' "$0" | sed -n '1,80p'; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; exit 2 ;;
  esac
done

if [[ -z "$server" || -z "$command_prog" ]]; then
  echo "Missing --server or --command" >&2
  exit 2
fi

if [[ ! -f "$DOT_ENV" ]]; then
  echo "Project .env not found: $DOT_ENV" >&2
  exit 1
fi

# Resolve CODEX_HOME and CONFIG path per scope
if [[ "$scope" == "local" ]]; then
  CODEX_HOME_DIR="$PROJECT_ROOT/ocodex/.codex"
else
  CODEX_HOME_DIR="$HOME/.codex"
fi
CONFIG_PATH="$CODEX_HOME_DIR/config.toml"
mkdir -p "$CODEX_HOME_DIR"

# Build TOML for the MCP server block in a temp file
tmp_toml="$(mktemp)"
{
  echo "[mcp_servers.$server]"
  printf 'command = "%s"\n' "$command_prog"
  printf 'args = ['
  for i in "${!args[@]}"; do
    if [[ $i -gt 0 ]]; then printf ', '; fi
    printf '"%s"' "${args[$i]}"
  done
  printf ']\n'
  echo "[mcp_servers.$server.env]"

  # Load .env safely without exporting to our process; read key/value pairs
  while IFS= read -r line || [[ -n "$line" ]]; do
    # Skip comments and empty lines
    [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]] && continue
    # Only consider simple KEY=VALUE lines
    if [[ "$line" =~ ^([A-Za-z_][A-Za-z0-9_]*)=(.*)$ ]]; then
      k="${BASH_REMATCH[1]}"
      v="${BASH_REMATCH[2]}"
      # strip optional quotes
      v="${v%\r}"
      v="${v%\n}"
      v="${v#\"}"; v="${v%\"}"
      v="${v#\'}"; v="${v%\'}"
      # Include only if listed via --env-key (if provided)
      if [[ ${#env_keys[@]} -eq 0 ]]; then
        include=1
      else
        include=0
        for want in "${env_keys[@]}"; do
          if [[ "$k" == "$want" ]]; then include=1; break; fi
        done
      fi
      if [[ $include -eq 1 ]]; then
        printf '%s = "%s"\n' "$k" "$v"
      fi
    fi
  done < "$DOT_ENV"
} > "$tmp_toml"

# Function to replace or append the [mcp_servers.<server>] block in config.toml
update_config() {
  local cfg="$1" block_server="$2" new_block_file="$3"
  local tmp_cfg
  tmp_cfg="$(mktemp)"

  if [[ -f "$cfg" ]]; then
    # Use awk to copy everything except the existing block, then append the new block
    awk -v sec="mcp_servers.""$block_server" \
      'BEGIN{skip=0} {
         if ($0 ~ "^\\["sec"\\]") { skip=1; next }
         if (skip && $0 ~ /^\[/) { skip=0 }
         if (!skip) print $0
       } END{}' "$cfg" > "$tmp_cfg"
  else
    : > "$tmp_cfg"
  fi

  echo >> "$tmp_cfg"
  cat "$new_block_file" >> "$tmp_cfg"
  mv "$tmp_cfg" "$cfg"
}

update_config "$CONFIG_PATH" "$server" "$tmp_toml"
rm -f "$tmp_toml"

echo "Updated: $CONFIG_PATH"
if [[ "$scope" == "local" ]]; then
  echo "Launch ocodex with: CODEX_HOME=\"$CODEX_HOME_DIR\" ocodex"
fi

