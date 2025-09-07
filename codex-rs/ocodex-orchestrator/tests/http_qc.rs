#[cfg(feature = "qc-http")]
#[test]
fn http_qc_sidecar_integration_smoke() {
    let endpoint = match std::env::var("OCODEX_QC_ENDPOINT") { Ok(v) => v, Err(_) => return /* skip when not set */ };
    let opt = ocodex_orchestrator::qc::HttpQuantumOptimizer { base_url: endpoint };
    let inst = ocodex_orchestrator::QuboInstance {
        tasks: vec![
            ocodex_orchestrator::QuboTask { id: "A".into(), priority: 0, write: false, depends_on: vec![], resources: vec![], deadline_ms: None, duration_ms: None },
            ocodex_orchestrator::QuboTask { id: "B".into(), priority: 10, write: false, depends_on: vec!["A".into()], resources: vec![], deadline_ms: None, duration_ms: None },
        ],
        horizon: ocodex_orchestrator::QuboHorizon { buckets: 2, capacity: 1, write_cap: 1 },
        weights: ocodex_orchestrator::QuboWeights { lateness: 1.0, priority: 1.0, fairness: 0.5, reorder_cost: 0.1 },
        seed: None,
        max_iter: None,
        timeout_ms: Some(200),
    };
    let delta = opt.optimize(&inst).expect("qc sidecar");
    assert_eq!(delta.order.len(), 2);
}

