#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: google_search.sh [-n NUM] [-S SITE] [-r DATE_RESTRICT] [--env FILE] QUERY...

Options:
  -n NUM            Number of results (default: 5)
  -S SITE           Restrict to site (e.g., example.com)
  -r DATE_RESTRICT  Google dateRestrict (e.g., d7 for last 7 days, m1, y1)
  --env FILE        Path to .env with GOOGLE_API_KEY and GOOGLE_CSE_ID

Examples:
  ./scripts/google_search.sh 'googles latest world model'
  ./scripts/google_search.sh -S ai.google 'gemini 2.0 world model'
  ./scripts/google_search.sh -n 10 -r d7 'google world model update'
USAGE
}

NUM=5
SITE=""
DATE_RESTRICT=""
ENV_FILE=""

ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    -n)
      NUM="${2:-}"
      shift 2;;
    -S)
      SITE="${2:-}"
      shift 2;;
    -r)
      DATE_RESTRICT="${2:-}"
      shift 2;;
    --env)
      ENV_FILE="${2:-}"
      shift 2;;
    -h|--help)
      usage; exit 0;;
    --)
      shift; break;;
    -*)
      echo "Unknown option: $1" >&2; usage; exit 2;;
    *)
      ARGS+=("$1"); shift;;
  esac
done

if [[ ${#ARGS[@]} -eq 0 ]]; then
  echo "error: missing QUERY" >&2
  usage
  exit 2
fi

# Load env vars. Preference: explicit --env, then ../.env (repo-level), then ./.env, then ~/.codex/.env
if [[ -n "$ENV_FILE" && -f "$ENV_FILE" ]]; then
  set -a; source "$ENV_FILE"; set +a
else
  if [[ -f "../.env" ]]; then set -a; source ../.env; set +a; fi
  if [[ -f ".env" ]]; then set -a; source ./.env; set +a; fi
  if [[ -f "$HOME/.codex/.env" ]]; then set -a; source "$HOME/.codex/.env"; set +a; fi
fi

if [[ -z "${GOOGLE_API_KEY:-}" || -z "${GOOGLE_CSE_ID:-}" ]]; then
  echo "error: GOOGLE_API_KEY and GOOGLE_CSE_ID must be set (use --env FILE or .env)" >&2
  exit 2
fi

QUERY="${ARGS[*]}"

# URL-encode query using jq (portable and reliable)
if ! command -v jq >/dev/null 2>&1; then
  echo "error: jq is required (brew install jq or apt-get install jq)" >&2
  exit 2
fi

ENC=$(printf '%s' "$QUERY" | jq -sRr @uri)
if [[ -n "$SITE" ]]; then
  SITE_ENC=$(printf '%s' "$SITE" | jq -sRr @uri)
  ENC="$ENC+site%3A$SITE_ENC"
fi

URL="https://www.googleapis.com/customsearch/v1?key=${GOOGLE_API_KEY}&cx=${GOOGLE_CSE_ID}&q=${ENC}&num=${NUM}"
if [[ -n "$DATE_RESTRICT" ]]; then
  URL+="&dateRestrict=$(printf '%s' "$DATE_RESTRICT" | jq -sRr @uri)"
fi

curl -sS "$URL" \
  | jq -r '.items[]? | "- " + (.title // "") + " — " + (.link // "")' || {
    echo "warning: no results or API error" >&2
    exit 1
  }

exit 0

