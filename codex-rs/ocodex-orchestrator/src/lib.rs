//! ocodex-orchestrator: Library-first orchestrator core for ocodex/Codex.
//!
//! Phase 1 exposes the core contracts and minimal implementations for events,
//! memory, and orchestration so other modules can be ported onto a stable base.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::sync::Arc;

pub mod builder;
pub mod config;
pub mod events;
pub mod cancel;
pub mod metrics;
pub mod memory;
pub mod policy;
pub mod queue;
pub mod scheduler;
pub mod workspace;
pub mod qc;
pub mod ab;
pub mod workers;
pub mod planner_simple;
#[cfg(feature = "sqlite-memory")]
pub mod memory_sqlite;
pub mod planner_llm;
pub mod router;
pub mod blackboard;

pub use builder::OrchestratorBuilder;
pub use config::OrchestrationConfig;
pub use events::{Event, EventBus, EventKind, InProcEventBus};
pub use cancel::{CancelSource, CancelToken};
pub use metrics::{Metrics, NoopMetrics};
pub use memory::{InMemoryMemoryService, MemoryDelta, MemoryService, MemorySnapshot, TodoItem};
pub use policy::{ExecutionPolicy, NoopExecutionPolicy};
pub use queue::{InMemoryTaskQueue, TaskQueue};
pub use scheduler::{InProcessScheduler, Scheduler, TaskRunner};
pub use workspace::{NoopWorkspaceManager, WorkspaceManager};
pub use qc::{QuantumOptimizer, ClassicalOptimizer, QuboInstance, QuboTask, QuboHorizon, QuboWeights, ScheduleDelta};
pub use ab::{compare as qc_ab_compare, AbResult};
pub use workers::{EnvWorker, PatchWorker, ReviewerWorker};
pub use planner_simple::SimplePlanner;
pub use planner_llm::{LlmPlanner, AutoPlanner};
pub use router::{BanditRouter, RouteDecision};
pub use blackboard::{BlackboardService, InMemoryBlackboard};

// Temporary compatibility layer to access the labs orchestrator API via a feature
// flag so teams can import everything from ocodex_orchestrator and target a single
// location while we complete the port.
#[cfg(feature = "labs-compat")]
pub mod labs_compat {
    pub use orchestrator::*;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playbook {
    pub name: String,
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub description: String,
    pub payload: JsonValue,
}

pub trait TaskWorker {
    fn name(&self) -> &'static str;
    fn can_handle(&self, task: &Task) -> bool;
    fn run(&mut self, task: Task) -> Result<JsonValue, OrchestrationError>;
}

pub trait Planner {
    fn plan_from_prompt(&mut self, prompt: &str) -> Result<Playbook, OrchestrationError>;
}

#[derive(Debug)]
pub enum OrchestrationError {
    Unsupported(&'static str),
    PlanningFailed(String),
    ExecutionFailed(String),
    Internal(String),
}

impl core::fmt::Display for OrchestrationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            OrchestrationError::Unsupported(s) => write!(f, "unsupported: {s}"),
            OrchestrationError::PlanningFailed(s) => write!(f, "planning failed: {s}"),
            OrchestrationError::ExecutionFailed(s) => write!(f, "execution failed: {s}"),
            OrchestrationError::Internal(s) => write!(f, "internal error: {s}"),
        }
    }
}

impl std::error::Error for OrchestrationError {}

pub struct MultiAgentOrchestrator<W: TaskWorker, P: Planner> {
    planner: P,
    workers: std::sync::Arc<std::sync::Mutex<Vec<W>>>,
    max_concurrency: usize,
    memory: Arc<dyn MemoryService + Send + Sync>,
    events: Arc<dyn EventBus + Send + Sync>,
    policy: Arc<dyn crate::policy::ExecutionPolicy + Send + Sync>,
    config: OrchestrationConfig,
    cancel: CancelToken,
    metrics: Arc<dyn Metrics>,
    scheduler: Arc<dyn Scheduler>,
    optimizer: Arc<dyn QuantumOptimizer>,
    workspace: Arc<dyn WorkspaceManager + Send + Sync>,
}

impl<W: TaskWorker + Send, P: Planner> MultiAgentOrchestrator<W, P> {
    pub fn new(planner: P, workers: Vec<W>, max_concurrency: usize) -> Self {
        Self::new_with(
            planner,
            workers,
            max_concurrency,
            Arc::new(InMemoryMemoryService::default()),
            Arc::new(InProcEventBus::default()),
        )
    }

    pub fn new_with(
        planner: P,
        workers: Vec<W>,
        max_concurrency: usize,
        memory: Arc<dyn MemoryService + Send + Sync>,
        events: Arc<dyn EventBus + Send + Sync>,
    ) -> Self {
        Self {
            planner,
            workers: std::sync::Arc::new(std::sync::Mutex::new(workers)),
            max_concurrency,
            memory,
            events,
            policy: Arc::new(crate::policy::NoopExecutionPolicy::default()),
            config: OrchestrationConfig::default(),
            cancel: CancelSource::default().token(),
            metrics: Arc::new(NoopMetrics::default()),
            scheduler: Arc::new(InProcessScheduler::default()),
            optimizer: Arc::new(crate::qc::ClassicalOptimizer),
            workspace: Arc::new(NoopWorkspaceManager::default()),
        }
    }

    pub fn with_policy(mut self, policy: Arc<dyn crate::policy::ExecutionPolicy + Send + Sync>) -> Self {
        self.policy = policy; self
    }

    pub fn with_config(mut self, cfg: OrchestrationConfig) -> Self {
        self.config = cfg; self
    }

    pub fn with_cancel(mut self, token: CancelToken) -> Self { self.cancel = token; self }
    pub fn with_metrics(mut self, m: Arc<dyn Metrics>) -> Self { self.metrics = m; self }
    pub fn with_scheduler(mut self, s: Arc<dyn Scheduler>) -> Self { self.scheduler = s; self }
    pub fn with_optimizer(mut self, q: Arc<dyn QuantumOptimizer>) -> Self { self.optimizer = q; self }
    pub fn with_workspace(mut self, w: Arc<dyn WorkspaceManager + Send + Sync>) -> Self { self.workspace = w; self }

    pub fn orchestrate_prompt(&mut self, prompt: &str) -> Result<(), OrchestrationError> {
        // Ensure project scaffold and load any prior memory snapshot
        let project_root = ensure_project_scaffold(prompt);
        if let Some(snapshot) = load_memory_snapshot_from_disk(&project_root) {
            // Best-effort merge into memory service state
            let delta = MemoryDelta { state_patch: snapshot.state, todo_add: snapshot.todo, todo_update: vec![] };
            let _ = self.memory.merge(&delta);
        }
        // Write AGENTS.md guidance if any memory present
        let _ = write_agents_md(&project_root, &self.memory.load());
        let playbook = self.planner.plan_from_prompt(prompt)?;
        self.execute_with_delegation(playbook)
    }

    pub fn execute_with_delegation(&mut self, playbook: Playbook) -> Result<(), OrchestrationError> {
        self.events.publish(Event { kind: EventKind::Info, message: format!("starting playbook: {}", playbook.name), ..Default::default() });

        // Build a reduced instance for the optimizer
        let tasks = playbook.tasks.clone();
        let qubo_tasks: Vec<crate::qc::QuboTask> = tasks
            .iter()
            .map(|t| crate::qc::QuboTask {
                id: t.id.clone(),
                priority: t.payload.get("priority").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                write: t.payload.get("needs_write_lock").and_then(|v| v.as_bool()).unwrap_or(false),
                depends_on: vec![],
                resources: vec![],
                deadline_ms: None,
                duration_ms: None,
            })
            .collect();
        let inst = crate::qc::QuboInstance {
            tasks: qubo_tasks,
            horizon: crate::qc::QuboHorizon { buckets: (tasks.len().min(8)) as u32, capacity: self.max_concurrency as u32, write_cap: 1 },
            weights: crate::qc::QuboWeights { lateness: 1.0, priority: 1.0, fairness: 0.5, reorder_cost: 0.1 },
            seed: None,
            max_iter: None,
            timeout_ms: Some(50),
        };
        let delta = self.optimizer.optimize(&inst).unwrap_or_else(|_| crate::qc::ScheduleDelta { order: tasks.iter().map(|t| t.id.clone()).collect(), priority_bumps: vec![], deferrals: vec![], cancellations: vec![], confidence: 0.0, metadata: None });
        // A/B compare against classical baseline and publish a summary event
        let classical = crate::qc::ClassicalOptimizer;
        let ab = crate::ab::compare(&classical, self.optimizer.as_ref(), &inst);
        self.events.publish(Event { kind: EventKind::Info, message: format!("qc_ab:winner={} classical_cost={} qc_cost={} confidence={}", ab.winner, ab.classical_cost, ab.qc_cost, delta.confidence), ..Default::default() });
        // Reorder tasks per delta.order while keeping any extras
        let mut by_id: std::collections::HashMap<String, Task> = tasks.into_iter().map(|t| (t.id.clone(), t)).collect();
        let mut ordered: Vec<Task> = Vec::new();
        for id in delta.order.iter() { if let Some(t) = by_id.remove(id) { ordered.push(t); } }
        for (_id, t) in by_id.into_iter() { ordered.push(t); }

        struct LocalRunner<W: TaskWorker + Send> {
            events: Arc<dyn EventBus + Send + Sync>,
            policy: Arc<dyn crate::policy::ExecutionPolicy + Send + Sync>,
            mem: Arc<dyn MemoryService + Send + Sync>,
            metrics: Arc<dyn Metrics>,
            cancel: CancelToken,
            workspace: Arc<dyn WorkspaceManager + Send + Sync>,
            project_root: std::path::PathBuf,
            workers: std::sync::Arc<std::sync::Mutex<Vec<W>>>,
            router: BanditRouter,
        }
        impl<W: TaskWorker + Send> TaskRunner for LocalRunner<W> {
            fn run_one(&self, task: Task) -> Result<(), OrchestrationError> {
                if self.cancel.is_canceled() {
                    self.events.publish(Event { kind: EventKind::Warn, message: "canceled".into(), ..Default::default() });
                    return Ok(());
                }
                self.events.publish(Event { kind: EventKind::Progress, message: format!("task:start:{}", task.id), task_id: Some(task.id.clone()), ..Default::default() });
                self.policy.before_task(&task, &self.events).map_err(OrchestrationError::Internal)?;
                // Choose and run worker
                let mut chosen_idx = 0usize;
                let result = {
                    let mut guard = self.workers.lock().map_err(|_| OrchestrationError::Internal("workers mutex".into()))?;
                    let decision = self.router.choose(task.clone(), &*guard);
                    chosen_idx = decision.index;
                    let w = guard.get_mut(decision.index).ok_or(OrchestrationError::Internal("no worker".into()))?;
                    w.run(task.clone())?
                };
                // Merge memory update if provided
                let mut state_patch = serde_json::json!({"last_result": result});
                if let Some(mu) = result.get("memory_update") { state_patch = merge_json_values(state_patch, mu.clone()); }
                let delta = MemoryDelta { state_patch, todo_add: vec![], todo_update: vec![] };
                let snapshot = self.mem.merge(&delta);
                // Persist memory and sync workspace hints (best-effort)
                let _ = persist_memory_snapshot(&self.project_root, &snapshot);
                let _ = write_todo_md(&self.project_root, &snapshot);
                let _ = write_agents_md(&self.project_root, &snapshot);
                self.policy.after_task(&task, &self.events).map_err(OrchestrationError::Internal)?;
                self.events.publish(Event { kind: EventKind::Progress, message: "task:done".into(), task_id: Some(task.id.clone()), ..Default::default() });
                self.metrics.inc("tasks_completed");
                // Bandit observation
                let success = result.get("success").and_then(|v| v.as_bool()).unwrap_or(true);
                self.router.observe(chosen_idx, success);
                Ok(())
            }
        }

        let project_root = current_project_root();
        let runner = LocalRunner::<W> {
            events: self.events.clone(),
            policy: self.policy.clone(),
            mem: self.memory.clone(),
            metrics: self.metrics.clone(),
            cancel: self.cancel.clone(),
            workspace: self.workspace.clone(),
            project_root,
            workers: self.workers.clone(),
            router: BanditRouter::from_env(),
        };

        self.metrics.inc("playbook_started");
        let ordered = topo_order_with_hint(ordered);
        let target = crate::scheduler::compute_concurrency(self.max_concurrency, ordered.len());
        self.events.publish(Event { kind: EventKind::Info, message: format!("scheduler_target_concurrency={}", target), ..Default::default() });
        self.scheduler.run(ordered, target, &runner)?;
        // Final persistence and summary
        let snapshot = self.memory.load();
        let root = current_project_root();
        let _ = persist_memory_snapshot(&root, &snapshot);
        let _ = write_todo_md(&root, &snapshot);
        self.events.publish(Event { kind: EventKind::Info, message: "playbook:done".into(), ..Default::default() });
        Ok(())
    }
}

// --- Project scaffold and memory persistence (port from labs orchestrator) ---
fn infer_project_name_from_prompt_scaffold(prompt: &str) -> String {
    let p = prompt.to_lowercase();
    let mut r#type = None;
    let type_keywords: &[(&str, &str)] = &[
        ("website", "website"), ("web site", "website"), ("web app", "webapp"), ("webapp", "webapp"),
        ("api", "api"), ("cli", "cli"), ("service", "service"), ("library", "lib"), ("package", "pkg"),
        ("bot", "bot"), ("mobile", "mobile"), ("server", "server"), ("app", "app"),
    ];
    for (needle, label) in type_keywords { if p.contains(needle) { r#type = Some(*label); break; } }
    let mut words: Vec<String> = prompt
        .split(|c: char| c.is_whitespace() || ",.;:!? ' \"()[]{}".contains(c))
        .filter(|w| !w.is_empty())
        .filter(|w| !matches!(w.to_lowercase().as_str(), "a"|"an"|"the"|"and"|"or"|"with"|"for"|"of"|"to"|"then"|"that"))
        .take(6)
        .map(|w| w.chars().filter(|c| c.is_ascii_alphanumeric() || *c=='-' || *c=='_').collect::<String>().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if words.is_empty() { words.push("project".into()); }
    let base = words.join("-");
    match r#type { Some(t) => format!("{}-{}", t, base), None => base }
}

fn copy_dir_recursive_scaffold(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    if !dst.exists() { std::fs::create_dir_all(dst)?; }
    for entry in std::fs::read_dir(src)? { let entry = entry?; let p = entry.path(); let t = dst.join(entry.file_name());
        if p.is_dir() { copy_dir_recursive_scaffold(&p, &t)?; }
        else if p.is_file() {
            let mut copy = true;
            if let (Ok(ms), Ok(md)) = (std::fs::metadata(&p), std::fs::metadata(&t)) { if ms.len() == md.len() { copy = false; } }
            if copy { let _ = std::fs::copy(&p, &t); }
        }
    }
    Ok(())
}

fn ensure_project_scaffold(prompt: &str) -> std::path::PathBuf {
    let repo_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let shared = std::env::var("ORCH_SHARED_DIR").unwrap_or_else(|_| "shared".into());
    let project = std::env::var("ORCH_PROJECT_NAME").unwrap_or_else(|_| infer_project_name_from_prompt_scaffold(prompt));
    let rel = format!("{}/{}", shared.trim_matches('/'), project.trim_matches('/'));
    let abs = repo_root.join(&rel);
    let _ = std::fs::create_dir_all(&abs);
    unsafe {
        std::env::set_var("ORCH_WORKDIR_REL", &rel);
        std::env::set_var("ORCH_WORKDIR_ABS", abs.to_string_lossy().to_string());
        std::env::set_var("ORCH_SHARED_DIR", &shared);
        std::env::set_var("ORCH_PROJECT_NAME", &project);
    }
    let codex = abs.join(".codex");
    let _ = std::fs::create_dir_all(&codex);
    let cfg = codex.join("config.toml");
    if !cfg.exists() {
        let _ = std::fs::write(&cfg, "approval_policy = \"never\"\nsandbox_mode    = \"workspace-write\"\n\n[sandbox_workspace_write]\nnetwork_access = true\n");
    }
    let repo_toolpack = repo_root.join("ocodex/.codex");
    if repo_toolpack.is_dir() { let _ = copy_dir_recursive_scaffold(&repo_toolpack, &codex); }
    let _ = std::fs::create_dir_all(abs.join(".orch"));
    unsafe { std::env::set_var("CODEX_HOME", codex.to_string_lossy().to_string()); }
    abs
}

fn current_project_root() -> std::path::PathBuf {
    std::env::var("ORCH_WORKDIR_ABS").map(std::path::PathBuf::from).unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")))
}

fn persist_memory_snapshot(root: &std::path::Path, mem: &crate::memory::MemorySnapshot) -> std::io::Result<()> {
    let base = root.join(".orch");
    let _ = std::fs::create_dir_all(&base);
    let path = base.join("memory.json");
    let s = serde_json::to_string_pretty(mem).unwrap_or_else(|_| "{}".into());
    std::fs::write(path, s)
}

fn load_memory_snapshot_from_disk(root: &std::path::Path) -> Option<crate::memory::MemorySnapshot> {
    let path = root.join(".orch").join("memory.json");
    let s = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<crate::memory::MemorySnapshot>(&s).ok()
}

fn write_todo_md(root: &std::path::Path, mem: &crate::memory::MemorySnapshot) -> std::io::Result<()> {
    // Controlled by env; default on
    let enabled = std::env::var("ORCH_TODO_SYNC").map(|v| v != "0" && v.to_lowercase() != "false").unwrap_or(true);
    if !enabled { return Ok(()); }
    let codex_home = root.join(".codex");
    let _ = std::fs::create_dir_all(&codex_home);
    let target = std::env::var("ORCH_TODO_TARGET").unwrap_or_else(|_| "root".into()); // root|codex|both
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    match target.as_str() {
        "codex" => files.push(codex_home.join("TODO.md")),
        "both" => { files.push(root.join("TODO.md")); files.push(codex_home.join("TODO.md")); },
        _ => files.push(root.join("TODO.md")),
    }
    let mut todo = String::new();
    todo.push_str("# TODO\n\n");
    for (i, t) in mem.todo.iter().enumerate() {
        let mark = match t.status.as_str() { "done" | "complete" => "x", _ => " " };
        todo.push_str(&format!("- [{}] {}", mark, t.title));
        if let Some(a) = &t.assignee { todo.push_str(&format!(" ({} )", a)); }
        if let Some(p) = &t.priority { todo.push_str(&format!(" [prio:{}]", p)); }
        todo.push('\n');
        for n in t.notes.iter() { todo.push_str("  - "); todo.push_str(n); todo.push('\n'); }
        if i + 1 == mem.todo.len() { todo.push('\n'); }
    }
    todo.push_str("\nGuidance: Update this list as tasks complete.\n");
    for f in files.iter() { let _ = std::fs::write(f, &todo); }
    Ok(())
}

fn write_agents_md(root: &std::path::Path, mem: &crate::memory::MemorySnapshot) -> std::io::Result<()> {
    // Controlled by env; default on
    let enabled = std::env::var("ORCH_AGENTS_MD").map(|v| v != "0" && v.to_lowercase() != "false").unwrap_or(true);
    if !enabled { return Ok(()); }
    let codex_home = root.join(".codex");
    let _ = std::fs::create_dir_all(&codex_home);
    let target = std::env::var("ORCH_AGENTS_MD_TARGET").unwrap_or_else(|_| "codex".into()); // codex|root|both
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    match target.as_str() {
        "root" => files.push(root.join("AGENTS.md")),
        "both" => { files.push(root.join("AGENTS.md")); files.push(codex_home.join("AGENTS.md")); },
        _ => files.push(codex_home.join("AGENTS.md")),
    }
    let mut md = String::new();
    md.push_str("# Orchestrator Guidance\n\n");
    if !mem.state.is_null() {
        if let Some(goal) = mem.state.get("goal").and_then(|v| v.as_str()) { md.push_str(&format!("- Goal: {}\n", goal)); }
        if let Some(pattern) = mem.state.get("pattern").and_then(|v| v.as_str()) { md.push_str(&format!("- Pattern: {}\n", pattern)); }
        if let Some(roles) = mem.state.get("roles").and_then(|v| v.as_array()) {
            md.push_str("- Roles:\n");
            for r in roles.iter() {
                let n = r.get("name").and_then(|x| x.as_str()).unwrap_or("role");
                let t = r.get("title").and_then(|x| x.as_str()).unwrap_or("");
                let p = r.get("purpose").and_then(|x| x.as_str()).unwrap_or("");
                md.push_str(&format!("  - {} {} — {}\n", n, t, p));
            }
        }
    }
    md.push_str("\nGuidelines\n- Prefer small, safe diffs.\n- Verify build/test before marking TODOs done.\n");
    for f in files.iter() { let _ = std::fs::write(f, &md); }
    Ok(())
}

fn merge_json_values(mut base: serde_json::Value, patch: serde_json::Value) -> serde_json::Value {
    use serde_json::Value::{Object, Array};
    match (&mut base, patch) {
        (Object(b), Object(p)) => {
            for (k, v) in p.into_iter() {
                let nv = if let Some(ev) = b.remove(&k) { merge_json_values(ev, v) } else { v };
                b.insert(k, nv);
            }
            serde_json::Value::Object(b.clone())
        }
        (Array(a), Array(mut p)) => { a.append(&mut p); serde_json::Value::Array(a.clone()) }
        (_, p) => p,
    }
}

fn topo_order_with_hint(tasks: Vec<Task>) -> Vec<Task> {
    // Build dependency map from payload.depends_on: [id]
    let mut by_id: std::collections::HashMap<String, Task> = tasks.into_iter().map(|t| (t.id.clone(), t)).collect();
    let mut indeg: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut edges: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for (id, t) in by_id.iter() {
        let deps: Vec<String> = t.payload.get("depends_on").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
        indeg.entry(id.clone()).or_insert(0);
        for d in deps.into_iter() {
            edges.entry(d.clone()).or_default().push(id.clone());
            *indeg.entry(id.clone()).or_insert(0) += 1;
        }
    }
    // Kahn's algorithm; stable ordering by lexical id
    let mut ready: Vec<String> = indeg.iter().filter(|&(_, &v)| v == 0).map(|(k, _)| k.clone()).collect();
    ready.sort();
    let mut out: Vec<Task> = Vec::new();
    while let Some(id) = ready.pop() {
        if let Some(t) = by_id.remove(&id) { out.push(t); }
        if let Some(children) = edges.remove(&id) {
            for c in children.into_iter() {
                if let Some(e) = indeg.get_mut(&c) { *e = e.saturating_sub(1); if *e == 0 { ready.push(c); ready.sort(); } }
            }
        }
    }
    // Append any remaining tasks in insertion order
    for (_id, t) in by_id.into_iter() { out.push(t); }
    out
}
