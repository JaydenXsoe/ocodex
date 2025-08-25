#!/usr/bin/env bash
# ----------------------------------------------------------
#   Verify that the `upstream` remote is set to the
#   OpenAI “codex” repository; if it isn’t, add it.
# ----------------------------------------------------------

URL="https://github.com/openai/codex"

# Grab the URL of the `upstream` remote, silently discarding
# the error that appears when the remote is missing.
actual_upstream="$(git remote get-url upstream 2>/dev/null)"

if [[ "$actual_upstream" != "$URL" ]]; then
    echo -e "\n### Adding Upstream $URL ###"
    ./scripts/add_upstream.sh
fi

# ----------------------------------------------------------
#   Run git fetch and determine merge with fork.
# ----------------------------------------------------------

echo -e "\n### Running git fetch upstream ###"
git fetch upstream
echo -e "\n### git log HEAD..upstream/main --oneline ###"
git log HEAD..upstream/main --oneline
echo -e "\n### git diff HEAD..upstream/main ###"
git diff HEAD..upstream/main
