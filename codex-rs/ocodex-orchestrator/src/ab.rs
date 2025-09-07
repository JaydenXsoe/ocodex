use crate::qc::{QuboInstance, ScheduleDelta};

fn cost_order(inst: &QuboInstance, order: &[String]) -> i64 {
    // Cost: weighted lateness proxy + write-cap violations by bucket
    // Priority reward: higher priority earlier reduces cost
    let mut prio = std::collections::HashMap::new();
    let mut write = std::collections::HashMap::new();
    for t in inst.tasks.iter() {
        prio.insert(t.id.clone(), t.priority);
        write.insert(t.id.clone(), t.write);
    }
    let mut cost: i64 = 0;
    for (pos, id) in order.iter().enumerate() {
        let p = *prio.get(id).unwrap_or(&0) as i64;
        cost += (pos as i64) * inst.weights.lateness as i64; // lateness proxy
        cost -= p * inst.weights.priority as i64; // reward priority earlier
    }
    // write-cap violations per bucket
    let cap = inst.horizon.capacity.max(1) as usize;
    let write_cap = inst.horizon.write_cap as usize;
    for chunk in order.chunks(cap) {
        let writes = chunk.iter().filter(|id| *write.get(*id).unwrap_or(&false)).count();
        if writes > write_cap { cost += ((writes - write_cap) as f64 * 10.0) as i64; }
    }
    cost
}

pub struct AbResult {
    pub classical_cost: i64,
    pub qc_cost: i64,
    pub winner: &'static str, // "qc" | "classical" | "tie"
    pub qc_delta: ScheduleDelta,
}

pub fn compare(classical: &dyn crate::qc::QuantumOptimizer, qc: &dyn crate::qc::QuantumOptimizer, inst: &QuboInstance) -> AbResult {
    let base = classical.optimize(inst).unwrap_or_else(|_| ScheduleDelta { order: inst.tasks.iter().map(|t| t.id.clone()).collect(), priority_bumps: vec![], deferrals: vec![], cancellations: vec![], confidence: 0.0, metadata: None });
    let qd = qc.optimize(inst).unwrap_or_else(|_| base.clone());
    let cc = cost_order(inst, &base.order);
    let qc = cost_order(inst, &qd.order);
    let winner = if qc < cc { "qc" } else if cc < qc { "classical" } else { "tie" };
    AbResult { classical_cost: cc, qc_cost: qc, winner, qc_delta: qd }
}

