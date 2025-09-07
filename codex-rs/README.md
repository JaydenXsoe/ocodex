# Codex CLI (Rust Implementation)

We provide Codex CLI as a standalone, native executable to ensure a zero-dependency install.

## Installing Codex

Today, the easiest way to install Codex is via `npm`, though we plan to publish Codex to other package managers soon.

```shell
npm i -g @openai/codex@native
codex
```

You can also download a platform-specific release directly from our [GitHub Releases](https://github.com/openai/codex/releases).

## What's new in the Rust CLI

While we are [working to close the gap between the TypeScript and Rust implementations of Codex CLI](https://github.com/openai/codex/issues/1262), note that the Rust CLI has a number of features that the TypeScript CLI does not!

### Config

Codex supports a rich set of configuration options. Note that the Rust CLI uses `config.toml` instead of `config.json`. See [`config.md`](./config.md) for details.

### Model Context Protocol Support

Codex CLI functions as an MCP client that can connect to MCP servers on startup. See the [`mcp_servers`](./config.md#mcp_servers) section in the configuration documentation for details.

It is still experimental, but you can also launch Codex as an MCP _server_ by running `codex mcp`. Use the [`@modelcontextprotocol/inspector`](https://github.com/modelcontextprotocol/inspector) to try it out:

```shell
npx @modelcontextprotocol/inspector codex mcp
```

### Notifications

You can enable notifications by configuring a script that is run whenever the agent finishes a turn. The [notify documentation](./config.md#notify) includes a detailed example that explains how to get desktop notifications via [terminal-notifier](https://github.com/julienXX/terminal-notifier) on macOS.

### `codex exec` to run Codex programmatially/non-interactively

To run Codex non-interactively, run `codex exec PROMPT` (you can also pass the prompt via `stdin`) and Codex will work on your task until it decides that it is done and exits. Output is printed to the terminal directly. You can set the `RUST_LOG` environment variable to see more about what's going on.

### Use `@` for file search

Typing `@` triggers a fuzzy-filename search over the workspace root. Use up/down to select among the results and Tab or Enter to replace the `@` with the selected path. You can use Esc to cancel the search.

### `--cd`/`-C` flag

Sometimes it is not convenient to `cd` to the directory you want Codex to use as the "working root" before running Codex. Fortunately, `codex` supports a `--cd` option so you can specify whatever folder you want. You can confirm that Codex is honoring `--cd` by double-checking the **workdir** it reports in the TUI at the start of a new session.

### Shell completions

Generate shell completion scripts via:

```shell
codex completion bash
codex completion zsh
codex completion fish
```

### Experimenting with the Codex Sandbox

To test to see what happens when a command is run under the sandbox provided by Codex, we provide the following subcommands in Codex CLI:

```
# macOS
codex debug seatbelt [--full-auto] [COMMAND]...

# Linux
codex debug landlock [--full-auto] [COMMAND]...
```

### Selecting a sandbox policy via `--sandbox`

The Rust CLI exposes a dedicated `--sandbox` (`-s`) flag that lets you pick the sandbox policy **without** having to reach for the generic `-c/--config` option:

```shell
# Run Codex with the default, read-only sandbox
codex --sandbox read-only

# Allow the agent to write within the current workspace while still blocking network access
codex --sandbox workspace-write

# Danger! Disable sandboxing entirely (only do this if you are already running in a container or other isolated env)
codex --sandbox danger-full-access
```

The same setting can be persisted in `~/.codex/config.toml` via the top-level `sandbox_mode = "MODE"` key, e.g. `sandbox_mode = "workspace-write"`.

## Code Organization

This folder is the root of a Cargo workspace. It contains quite a bit of experimental code, but here are the key crates:

- [`core/`](./core) contains the business logic for Codex. Ultimately, we hope this to be a library crate that is generally useful for building other Rust/native applications that use Codex.
- [`exec/`](./exec) "headless" CLI for use in automation.
- [`tui/`](./tui) CLI that launches a fullscreen TUI built with [Ratatui](https://ratatui.rs/).
- [`cli/`](./cli) CLI multitool that provides the aforementioned CLIs via subcommands.

## `ocodex` binary and local defaults

The workspace produces an `ocodex` binary alongside `codex` with local-first defaults:

- Provider: `oss` (Ollama-compatible)
- Model: dynamic default for `--oss` (>= 64 GiB RAM prefers `gpt-oss:120b`, otherwise `gpt-oss:20b`). Override with `-m`.
- OpenAI usage: disabled by default; enable with `--openai`
  (For OpenAI providers, you can disable client-side rate-limit waits with `CODEX_DISABLE_RATE_LIMITS=1`.)

### Install

Install binaries to your PATH:

```
cargo install --path cli
```

This installs both `codex` and `ocodex`.

### Usage

- Local, interactive TUI:

```
ocodex
```

- Local, non-interactive:

```
ocodex exec -m gpt-oss:20b -- json "Your prompt here"
```

- Opt-in OpenAI:

```
ocodex --openai
ocodex exec --openai -m gpt-5 -- json "Your prompt here"
```

If a selected provider requires OpenAI auth and `--openai` is not passed,
`ocodex` exits with a hint to re-run with `--openai`.

### Running from any directory

`ocodex exec` allows running outside a Git repo by default. Use `--no-skip-git-repo-check` to enforce the check.

### Web Search (Simple and MCP)

`ocodex` supports two approaches to web search without relying on OpenAI:

- Simple (shell + HTTP API): Provide keys via env and let the agent call `curl`.
  - Google Programmable Search: set `GOOGLE_API_KEY` and `GOOGLE_CSE_ID` in either `ocodex/.codex/.env` or your project `.env`.
    The environment context includes a curl template like:
    `curl -s "https://www.googleapis.com/customsearch/v1?key=$GOOGLE_API_KEY&cx=$GOOGLE_CSE_ID&q={QUERY}&num=5"`
  - SerpAPI: set `SERPAPI_KEY`. The environment context includes:
    `curl -s "https://serpapi.com/search.json?engine=google&q={QUERY}&api_key=$SERPAPI_KEY&num=5"`
  - Tip: URL-encode queries: `ENC=$(printf '%s' "$QUERY" | jq -sRr @uri)`.

- MCP (Model Context Protocol): Run a search MCP server and add it to `~/.codex/config.toml`:

```
[mcp_servers.search]
command = "node"
args = ["path/to/search-mcp-server.js"]
[mcp_servers.search.env]
API_KEY = "..."
```

Codex will advertise MCP tools to the model at startup. This yields structured tool calls (e.g., `search.query`) instead of raw shell.

Default CODEX_HOME resolution (when `CODEX_HOME` is unset):
- In containers (Docker/OCI): `/usr/local/share/ocodex/.codex`
- In a checkout of this repo: `REPO_ROOT/ocodex/.codex`
- Otherwise, if present: `/usr/local/share/ocodex/.codex`
- Else: `~/.codex`

Note on network: `ocodex` defaults to `workspace-write` and enables outbound network by default unless you explicitly disable it via config. If your environment still blocks network, check your sandbox settings or CI runner.

#### Example: Built-in simple MCP search server

This repo includes a minimal MCP server script exposing a `search.query` tool:

- Script: `codex-rs/scripts/mcp_websearch.js`
- Implements: `initialize`, `tools/list` (tool call not required for detection)

Arguments accepted by `search.query`:
- `q` or `query`: query string (aliases; either works)
- `num`: max results (Google allows 1–10; values are clamped)
- `site`: optional site filter (e.g., `ai.google`)
- `engine`: `google_cse`, `serpapi`, or `google` (alias of `google_cse`)
- `dateRestrict`: Google `dateRestrict` param (e.g., `d7`, `m1`, `y1`)

To configure ocodex to run it, add to `ocodex/.codex/config.toml`:

```
[mcp_servers.search]
command = "node"
args = ["/absolute/path/to/ocodex/codex-rs/scripts/mcp_websearch.js"]

[mcp_servers.search.env]
# Pass API keys explicitly; the MCP client runs with a clean env.
GOOGLE_API_KEY = "..."
GOOGLE_CSE_ID = "..."
SERPAPI_KEY = "..." # optional
```

You can validate the server independently:

```
cd codex-rs
cargo run -p codex-mcp-client -- node scripts/mcp_websearch.js
```

This should print a tools/list response containing `search.query`.

Quick local dry-run without network (verifies argument parsing):

```
cd codex-rs
(
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}'
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
  # Use engine="none" to avoid network; send only `query` to validate alias acceptance
  echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"search.query","arguments":{"engine":"none","num":11,"query":"alias acceptance test","site":"example.com","dateRestrict":"d7"}}}'
) | node scripts/mcp_websearch.js
```

You should see a tools/list with both `q` and `query` in the schema and a `No results.` response for the tools/call (since no engine is selected). The `num` argument will be clamped to `10` internally for Google.

### Models and Ollama deployment

For detailed guidance on choosing models, resource tradeoffs, and multiple Ollama deployment patterns (local, Docker, Kubernetes, network-mounted storage, SSH tunnels), see:

- ../../docs/models-and-ollama-deployment.md
