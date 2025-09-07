use crate::config::OrchestrationConfig;
use crate::events::{EventBus, InProcEventBus};
use crate::memory::{InMemoryMemoryService, MemoryService};
use crate::policy::{ExecutionPolicy, NoopExecutionPolicy};
use crate::queue::{InMemoryTaskQueue, TaskQueue};
use crate::qc::{ClassicalOptimizer, QuantumOptimizer};
#[cfg(feature = "qc-http")]
use crate::qc::HttpQuantumOptimizer;
use crate::{MultiAgentOrchestrator, Planner, TaskWorker};
use crate::planner_llm::AutoPlanner;
use std::sync::Arc;

pub struct OrchestratorBuilder {
    pub config: OrchestrationConfig,
    pub memory: Option<Arc<dyn MemoryService + Send + Sync>>,
    pub events: Option<Arc<dyn EventBus + Send + Sync>>,
    pub queue: Option<Arc<dyn TaskQueue + Send + Sync>>,
    pub policy: Option<Arc<dyn ExecutionPolicy + Send + Sync>>,
    pub optimizer: Option<Arc<dyn QuantumOptimizer>>, 
}

impl Default for OrchestratorBuilder {
    fn default() -> Self {
        Self {
            config: OrchestrationConfig::default(),
            memory: None,
            events: None,
            queue: None,
            policy: None,
            optimizer: None,
        }
    }
}

impl OrchestratorBuilder {
    pub fn new() -> Self { Self::default() }
    pub fn config(mut self, cfg: OrchestrationConfig) -> Self { self.config = cfg; self }
    pub fn memory(mut self, m: Arc<dyn MemoryService + Send + Sync>) -> Self { self.memory = Some(m); self }
    pub fn events(mut self, e: Arc<dyn EventBus + Send + Sync>) -> Self { self.events = Some(e); self }
    pub fn queue(mut self, q: Arc<dyn TaskQueue + Send + Sync>) -> Self { self.queue = Some(q); self }
    pub fn policy(mut self, p: Arc<dyn ExecutionPolicy + Send + Sync>) -> Self { self.policy = Some(p); self }
    pub fn optimizer(mut self, q: Arc<dyn QuantumOptimizer>) -> Self { self.optimizer = Some(q); self }

    pub fn build<W: TaskWorker + Send, P: Planner>(self, planner: P, workers: Vec<W>) -> MultiAgentOrchestrator<W, P> {
        let memory = self.memory.unwrap_or_else(|| Arc::new(InMemoryMemoryService::default()));
        let events = self.events.unwrap_or_else(|| Arc::new(InProcEventBus::default()));
        let _queue = self.queue.unwrap_or_else(|| Arc::new(InMemoryTaskQueue::default()));
        let policy = self.policy.unwrap_or_else(|| Arc::new(NoopExecutionPolicy::default()));

        let optimizer: Arc<dyn QuantumOptimizer> = if let Some(q) = self.optimizer {
            q
        } else if let Some(url) = self.config.qc_endpoint.clone() {
            #[cfg(feature = "qc-http")]
            { Arc::new(HttpQuantumOptimizer { base_url: url }) }
            #[cfg(not(feature = "qc-http"))]
            { Arc::new(ClassicalOptimizer) }
        } else {
            Arc::new(ClassicalOptimizer)
        };
        MultiAgentOrchestrator::new_with(planner, workers, self.config.max_concurrency, memory, events)
            .with_policy(policy)
            .with_config(self.config)
            .with_optimizer(optimizer)
    }

    pub fn build_auto<W: TaskWorker + Send>(self, workers: Vec<W>) -> MultiAgentOrchestrator<W, AutoPlanner> {
        let memory = self.memory.unwrap_or_else(|| Arc::new(InMemoryMemoryService::default()));
        let events = self.events.unwrap_or_else(|| Arc::new(InProcEventBus::default()));
        let _queue = self.queue.unwrap_or_else(|| Arc::new(InMemoryTaskQueue::default()));
        let policy = self.policy.unwrap_or_else(|| Arc::new(NoopExecutionPolicy::default()));

        let optimizer: Arc<dyn QuantumOptimizer> = if let Some(q) = self.optimizer {
            q
        } else if let Some(url) = self.config.qc_endpoint.clone() {
            #[cfg(feature = "qc-http")]
            { Arc::new(HttpQuantumOptimizer { base_url: url }) }
            #[cfg(not(feature = "qc-http"))]
            { Arc::new(ClassicalOptimizer) }
        } else {
            Arc::new(ClassicalOptimizer)
        };
        let planner = AutoPlanner::new();
        MultiAgentOrchestrator::new_with(planner, workers, self.config.max_concurrency, memory, events)
            .with_policy(policy)
            .with_config(self.config)
            .with_optimizer(optimizer)
    }
}
