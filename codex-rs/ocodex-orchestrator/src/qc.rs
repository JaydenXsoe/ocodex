use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuboTask {
    pub id: String,
    pub priority: i32,
    pub write: bool,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub resources: Vec<String>,
    #[serde(default)]
    pub deadline_ms: Option<u64>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuboHorizon { pub buckets: u32, pub capacity: u32, pub write_cap: u32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuboWeights { pub lateness: f64, pub priority: f64, pub fairness: f64, pub reorder_cost: f64 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuboInstance {
    pub tasks: Vec<QuboTask>,
    pub horizon: QuboHorizon,
    pub weights: QuboWeights,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub max_iter: Option<u32>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityBump { pub id: String, pub new_priority: i32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleDelta {
    pub order: Vec<String>,
    #[serde(default)]
    pub priority_bumps: Vec<PriorityBump>,
    #[serde(default)]
    pub deferrals: Vec<String>,
    #[serde(default)]
    pub cancellations: Vec<String>,
    pub confidence: f64,
    #[serde(default)]
    pub metadata: Option<JsonValue>,
}

pub trait QuantumOptimizer: Send + Sync {
    fn optimize(&self, inst: &QuboInstance) -> Result<ScheduleDelta, String>;
}

pub struct ClassicalOptimizer;

impl ClassicalOptimizer {
    fn topo_sort_with_priority(inst: &QuboInstance) -> Vec<String> {
        use std::collections::{HashMap, HashSet};
        let mut deps: HashMap<String, HashSet<String>> = HashMap::new();
        let mut prio: HashMap<String, i32> = HashMap::new();
        for t in inst.tasks.iter() {
            deps.insert(t.id.clone(), t.depends_on.iter().cloned().collect());
            prio.insert(t.id.clone(), t.priority);
        }
        let mut ready: Vec<String> = deps
            .iter()
            .filter(|(_, v)| v.is_empty())
            .map(|(k, _)| k.clone())
            .collect();
        let mut order = Vec::new();
        let mut remaining: HashSet<String> = deps.keys().cloned().collect();
        while !ready.is_empty() {
            ready.sort_by_key(|i| -(prio.get(i).cloned().unwrap_or_default()));
            let n = ready.remove(0);
            order.push(n.clone());
            remaining.remove(&n);
            deps.remove(&n);
            for (k, v) in deps.iter_mut() {
                v.remove(&n);
                if v.is_empty() && !order.contains(k) && !ready.contains(k) {
                    ready.push(k.clone());
                }
            }
        }
        // append any remaining (cycle fallback)
        order.extend(remaining.into_iter());
        order
    }
}

impl QuantumOptimizer for ClassicalOptimizer {
    fn optimize(&self, inst: &QuboInstance) -> Result<ScheduleDelta, String> {
        let order = Self::topo_sort_with_priority(inst);
        Ok(ScheduleDelta { order, priority_bumps: vec![], deferrals: vec![], cancellations: vec![], confidence: 0.5, metadata: None })
    }
}

#[cfg(feature = "qc-http")]
pub struct HttpQuantumOptimizer { pub base_url: String }

#[cfg(feature = "qc-http")]
impl QuantumOptimizer for HttpQuantumOptimizer {
    fn optimize(&self, inst: &QuboInstance) -> Result<ScheduleDelta, String> {
        let url = format!("{}/optimize", self.base_url.trim_end_matches('/'));
        let res = ureq::post(&url).set("Content-Type", "application/json").send_json(serde_json::to_value(inst).map_err(|e| e.to_string())?);
        if res.ok() {
            let delta: ScheduleDelta = res.into_json().map_err(|e| e.to_string())?;
            Ok(delta)
        } else {
            Err(format!("qc http error: {}", res.status()))
        }
    }
}

