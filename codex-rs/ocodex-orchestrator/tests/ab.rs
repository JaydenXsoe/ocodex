use ocodex_orchestrator as orch;
use serde_json::json;

struct ReverseOptimizer;

impl orch::QuantumOptimizer for ReverseOptimizer {
    fn optimize(&self, inst: &orch::QuboInstance) -> Result<orch::ScheduleDelta, String> {
        let mut ids: Vec<String> = inst.tasks.iter().map(|t| t.id.clone()).collect();
        ids.reverse();
        Ok(orch::ScheduleDelta { order: ids, priority_bumps: vec![], deferrals: vec![], cancellations: vec![], confidence: 0.4, metadata: None })
    }
}

#[test]
fn ab_compare_reports_winner() {
    let inst = orch::QuboInstance {
        tasks: vec![
            orch::QuboTask { id: "A".into(), priority: 0, write: false, depends_on: vec![], resources: vec![], deadline_ms: None, duration_ms: None },
            orch::QuboTask { id: "B".into(), priority: 10, write: false, depends_on: vec!["A".into()], resources: vec![], deadline_ms: None, duration_ms: None },
            orch::QuboTask { id: "C".into(), priority: 5, write: true, depends_on: vec![], resources: vec![], deadline_ms: None, duration_ms: None },
        ],
        horizon: orch::QuboHorizon { buckets: 3, capacity: 1, write_cap: 1 },
        weights: orch::QuboWeights { lateness: 1.0, priority: 1.0, fairness: 0.5, reorder_cost: 0.1 },
        seed: None,
        max_iter: None,
        timeout_ms: None,
    };
    let classical = orch::ClassicalOptimizer;
    let rev = ReverseOptimizer;
    let res = orch::qc_ab_compare(&classical, &rev, &inst);
    // Classical should win because reverse breaks precedence of B->A
    assert_eq!(res.winner, "classical");
}

#[test]
fn orchestrator_uses_optimizer_order() {
    // Build playbook with tasks 1..=3; optimizer will reverse order and we should see events in reversed start order
    struct DummyPlanner;
    impl orch::Planner for DummyPlanner {
        fn plan_from_prompt(&mut self, _prompt: &str) -> Result<orch::Playbook, orch::OrchestrationError> {
            Ok(orch::Playbook { name: "x".into(), tasks: vec![] })
        }
    }
    struct NoopWorker;
    impl orch::TaskWorker for NoopWorker {
        fn name(&self) -> &'static str { "noop" }
        fn can_handle(&self, _task: &orch::Task) -> bool { true }
        fn run(&mut self, _task: orch::Task) -> Result<serde_json::Value, orch::OrchestrationError> { Ok(json!({})) }
    }
    let tasks = vec![
        orch::Task { id: "1".into(), description: "t1".into(), payload: serde_json::json!({}) },
        orch::Task { id: "2".into(), description: "t2".into(), payload: serde_json::json!({}) },
        orch::Task { id: "3".into(), description: "t3".into(), payload: serde_json::json!({}) },
    ];
    let bus = std::sync::Arc::new(orch::InProcEventBus::default());
    let rx = bus.subscribe();
    let mut orchr = orch::OrchestratorBuilder::new()
        .events(bus)
        .optimizer(std::sync::Arc::new(ReverseOptimizer))
        .build(DummyPlanner, vec![NoopWorker])
        .with_config(orch::OrchestrationConfig { max_concurrency: 1, planner: None, model: None, backend: None, container_mode: None, project_name: None, qc_endpoint: None });
    // Execute
    orchr.execute_with_delegation(orch::Playbook { name: "ab".into(), tasks: tasks.clone() }).unwrap();
    // Drain events quickly
    let mut starts = Vec::new();
    let mut saw_ab = false;
    while let Ok(ev) = rx.try_recv() {
        if ev.message.starts_with("task:start:") { starts.push(ev.message.replace("task:start:", "")); }
        if ev.message.starts_with("qc_ab:winner=") { saw_ab = true; }
    }
    // We expect reverse start order: 3,2,1 (sequential scheduler)
    assert!(starts.len() >= 1);
    assert_eq!(starts[0], "3");
    assert!(saw_ab);
}
