use ocodex_orchestrator as orch;

struct DemoPlanner;
impl orch::Planner for DemoPlanner {
    fn plan_from_prompt(&mut self, _prompt: &str) -> Result<orch::Playbook, orch::OrchestrationError> {
        Ok(orch::Playbook { name: "demo".into(), tasks: vec![] })
    }
}

struct NoopWorker;
impl orch::TaskWorker for NoopWorker {
    fn name(&self) -> &'static str { "noop" }
    fn can_handle(&self, _task: &orch::Task) -> bool { true }
    fn run(&mut self, _task: orch::Task) -> Result<serde_json::Value, orch::OrchestrationError> { Ok(serde_json::json!({})) }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let qc_endpoint = std::env::var("OCODEX_QC_ENDPOINT").ok();
    let mut builder = orch::OrchestratorBuilder::new();
    if let Some(url) = qc_endpoint {
        #[cfg(feature = "qc-http")]
        {
            builder = builder.optimizer(std::sync::Arc::new(orch::qc::HttpQuantumOptimizer { base_url: url }));
        }
    }
    let mut orch = builder.build(DemoPlanner, vec![NoopWorker]);
    let playbook = orch::Playbook {
        name: "qc-demo".into(),
        tasks: vec![
            orch::Task { id: "A".into(), description: "task A".into(), payload: serde_json::json!({"priority": 1}) },
            orch::Task { id: "B".into(), description: "task B".into(), payload: serde_json::json!({"priority": 5, "needs_write_lock": false}) },
            orch::Task { id: "C".into(), description: "task C".into(), payload: serde_json::json!({"priority": 3, "needs_write_lock": true}) },
        ],
    };
    orch.execute_with_delegation(playbook)?;
    println!("qc-demo complete. Set OCODEX_QC_ENDPOINT to use the sidecar.");
    Ok(())
}

