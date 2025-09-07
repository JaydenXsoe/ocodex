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
    workers: Vec<W>,
    max_concurrency: usize,
    memory: Arc<dyn MemoryService + Send + Sync>,
    events: Arc<dyn EventBus + Send + Sync>,
    policy: Arc<dyn crate::policy::ExecutionPolicy + Send + Sync>,
    config: OrchestrationConfig,
    cancel: CancelToken,
    metrics: Arc<dyn Metrics>,
    scheduler: Arc<dyn Scheduler>,
    optimizer: Arc<dyn QuantumOptimizer>,
}

impl<W: TaskWorker, P: Planner> MultiAgentOrchestrator<W, P> {
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
            workers,
            max_concurrency,
            memory,
            events,
            policy: Arc::new(crate::policy::NoopExecutionPolicy::default()),
            config: OrchestrationConfig::default(),
            cancel: CancelSource::default().token(),
            metrics: Arc::new(NoopMetrics::default()),
            scheduler: Arc::new(InProcessScheduler::default()),
            optimizer: Arc::new(crate::qc::ClassicalOptimizer),
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

    pub fn orchestrate_prompt(&mut self, prompt: &str) -> Result<(), OrchestrationError> {
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

        struct LocalRunner {
            events: Arc<dyn EventBus + Send + Sync>,
            policy: Arc<dyn crate::policy::ExecutionPolicy + Send + Sync>,
            mem: Arc<dyn MemoryService + Send + Sync>,
            metrics: Arc<dyn Metrics>,
            cancel: CancelToken,
        }
        impl TaskRunner for LocalRunner {
            fn run_one(&self, task: Task) -> Result<(), OrchestrationError> {
                if self.cancel.is_canceled() {
                    self.events.publish(Event { kind: EventKind::Warn, message: "canceled".into(), ..Default::default() });
                    return Ok(());
                }
                self.events.publish(Event { kind: EventKind::Progress, message: format!("task:start:{}", task.id), task_id: Some(task.id.clone()), ..Default::default() });
                self.policy.before_task(&task, &self.events).map_err(OrchestrationError::Internal)?;
                let result = serde_json::json!({"ok": true, "task": task.id});
                let delta = MemoryDelta {
                    state_patch: serde_json::json!({"last_result": result}),
                    todo_add: vec![TodoItem { title: format!("Reviewed {}", task.id), status: "done".into(), assignee: None, priority: None, notes: vec![] }],
                    todo_update: vec![],
                };
                let _ = self.mem.merge(&delta);
                self.policy.after_task(&task, &self.events).map_err(OrchestrationError::Internal)?;
                self.events.publish(Event { kind: EventKind::Progress, message: "task:done".into(), task_id: Some(task.id.clone()), ..Default::default() });
                self.metrics.inc("tasks_completed");
                Ok(())
            }
        }

        let runner = LocalRunner {
            events: self.events.clone(),
            policy: self.policy.clone(),
            mem: self.memory.clone(),
            metrics: self.metrics.clone(),
            cancel: self.cancel.clone(),
        };

        self.metrics.inc("playbook_started");
        let target = crate::scheduler::compute_concurrency(self.max_concurrency, ordered.len());
        self.events.publish(Event { kind: EventKind::Info, message: format!("scheduler_target_concurrency={}", target), ..Default::default() });
        self.scheduler.run(ordered, target, &runner)?;
        self.events.publish(Event { kind: EventKind::Info, message: "playbook:done".into(), ..Default::default() });
        Ok(())
    }
}
