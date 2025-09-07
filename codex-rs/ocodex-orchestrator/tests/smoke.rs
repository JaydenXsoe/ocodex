use ocodex_orchestrator as orch;
use ocodex_orchestrator::{EventBus, MemoryService, TaskQueue};
use serde_json::json;

struct DummyPlanner;
impl orch::Planner for DummyPlanner {
    fn plan_from_prompt(&mut self, prompt: &str) -> Result<orch::Playbook, orch::OrchestrationError> {
        Ok(orch::Playbook {
            name: prompt.to_string(),
            tasks: vec![orch::Task { id: "t1".into(), description: "noop".into(), payload: json!({}) }],
        })
    }
}

struct NoopWorker;
impl orch::TaskWorker for NoopWorker {
    fn name(&self) -> &'static str { "noop" }
    fn can_handle(&self, _task: &orch::Task) -> bool { true }
    fn run(&mut self, _task: orch::Task) -> Result<serde_json::Value, orch::OrchestrationError> { Ok(json!({"ok":true})) }
}

#[test]
fn orchestrates_single_task() {
    let mut orch = orch::OrchestratorBuilder::new()
        .build(DummyPlanner, vec![NoopWorker]);
    let res = orch.orchestrate_prompt("demo");
    assert!(res.is_ok());
}

#[test]
fn event_bus_publish_subscribe() {
    let bus = std::sync::Arc::new(orch::InProcEventBus::default());
    let rx = bus.subscribe();
    bus.publish(orch::Event { kind: orch::EventKind::Info, message: "hello".into(), ..Default::default() });
    let got = rx.recv().expect("recv");
    assert!(matches!(got.kind, orch::EventKind::Info));
    assert_eq!(got.message, "hello");
}

#[test]
fn memory_merge_and_load() {
    let mem = orch::InMemoryMemoryService::default();
    let snap1 = mem.load();
    assert!(snap1.state.is_null());
    mem.merge(&orch::MemoryDelta {
        state_patch: json!({"a": {"b": 1}}),
        todo_add: vec![orch::TodoItem { title: "t".into(), status: "open".into(), assignee: None, priority: None, notes: vec![] }],
        todo_update: vec![],
    });
    let snap2 = mem.load();
    assert_eq!(snap2.state["a"]["b"], 1);
    assert_eq!(snap2.todo.len(), 1);
}

#[test]
fn queue_fifo_order() {
    let q = orch::InMemoryTaskQueue::default();
    q.push_all(vec![
        orch::Task { id: "1".into(), description: "a".into(), payload: json!({"priority": 0}) },
        orch::Task { id: "2".into(), description: "b".into(), payload: json!({"priority": 0}) },
    ]);
    assert_eq!(q.len(), 2);
    assert_eq!(q.pop().unwrap().id, "1");
    assert_eq!(q.pop().unwrap().id, "2");
    assert!(q.pop().is_none());
}

#[test]
fn classical_optimizer_respects_priority_and_deps() {
    let inst = orch::QuboInstance {
        tasks: vec![
            orch::QuboTask { id: "A".into(), priority: 0, write: false, depends_on: vec![], resources: vec![], deadline_ms: None, duration_ms: None },
            orch::QuboTask { id: "B".into(), priority: 5, write: false, depends_on: vec!["A".into()], resources: vec![], deadline_ms: None, duration_ms: None },
            orch::QuboTask { id: "C".into(), priority: 10, write: false, depends_on: vec![], resources: vec![], deadline_ms: None, duration_ms: None },
        ],
        horizon: orch::QuboHorizon { buckets: 3, capacity: 1, write_cap: 1 },
        weights: orch::QuboWeights { lateness: 1.0, priority: 1.0, fairness: 0.5, reorder_cost: 0.1 },
        seed: None,
        max_iter: None,
        timeout_ms: None,
    };
    let opt = orch::ClassicalOptimizer;
    let delta = opt.optimize(&inst).unwrap();
    // C (prio 10) should come before A (0), and B depends on A
    let pos = |id: &str| delta.order.iter().position(|x| x == id).unwrap();
    assert!(pos("C") < pos("A"));
    assert!(pos("A") < pos("B"));
}

#[test]
fn write_lock_pop_eligible_behaves() {
    let q = orch::InMemoryTaskQueue::default();
    q.push_all(vec![
        orch::Task { id: "W".into(), description: "write".into(), payload: json!({"needs_write_lock": true}) },
        orch::Task { id: "R".into(), description: "read".into(), payload: json!({}) },
    ]);
    // With a write in flight, we should skip the first (write) and pick the read
    let pick = q.pop_eligible(1);
    assert!(pick.is_some());
    let (t, needs) = pick.unwrap();
    assert_eq!(t.id, "R");
    assert!(!needs);
}
