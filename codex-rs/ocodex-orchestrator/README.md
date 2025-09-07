# ocodex-orchestrator (library-first)

This is the single place to target when integrating orchestrator functionality into Codex.

Status
- Core library in-place with events, memory, queue, scheduler, QC stubs.
- Temporary compatibility layer `labs-compat` exposes the full labs orchestrator API under this crate while we complete the port.

Usage (temporary)
- Enable the feature and import from one place:

  - Cargo.toml: `ocodex-orchestrator = { path = "./ocodex-orchestrator", features = ["labs-compat"] }`
  - Code: `use ocodex_orchestrator::labs_compat as orch;` then `orch::MultiAgentOrchestrator`, etc.

Port Plan
- See `docs/CORE/PORT_PLAN_TO_CODEX.md` in the root for a detailed checklist.
- We will move workers (env/patch/reviewer/container/ocodex), planners/patterns, memory/workspace, and HTTP/SSE into this crate behind feature flags.

Notes
- The goal is to deprecate the labs orchestrator soon after parity is reached. Until then, labs-compat keeps everything reachable under this crate to remove confusion about where to edit.

