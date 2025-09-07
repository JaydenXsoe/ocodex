use crate::{Task, TaskWorker};

#[derive(Clone, Debug)]
pub struct RouteDecision { pub index: usize, pub reason: &'static str }

#[derive(Default)]
pub struct BanditRouter {
    eps: f32,
    stats: std::sync::Mutex<std::collections::HashMap<usize, (u32, u32)>>, // (tries, success)
}

impl BanditRouter {
    pub fn from_env() -> Self {
        let eps = std::env::var("ORCH_ROUTER_EPS").ok().and_then(|s| s.parse::<f32>().ok()).unwrap_or(0.1);
        Self { eps, ..Default::default() }
    }

    pub fn choose<W: TaskWorker>(&self, task: Task, workers: &[W]) -> RouteDecision {
        // 1) Explicit route
        if let Some(name) = task.payload.get("worker").and_then(|v| v.as_str()) {
            if let Some((idx, _)) = workers.iter().enumerate().find(|(_, w)| w.name() == name) {
                return RouteDecision { index: idx, reason: "explicit" };
            }
        }
        // 2) Capability
        if let Some((idx, _)) = workers.iter().enumerate().find(|(_, w)| w.can_handle(&task)) {
            return RouteDecision { index: idx, reason: "capability" };
        }
        // 3) Bandit epsilon-greedy over historical success
        let mut rng = rand_seed();
        let roll = next_f32(&mut rng);
        if roll < self.eps {
            return RouteDecision { index: (rand_u32(&mut rng) as usize) % workers.len().max(1), reason: "explore" };
        }
        // Exploit: pick max success rate
        let stats = self.stats.lock().expect("router stats");
        let mut best = 0usize; let mut best_rate = -1.0f32;
        for i in 0..workers.len() {
            let (t, s) = stats.get(&i).cloned().unwrap_or((0, 0));
            let rate = if t == 0 { 0.5 } else { (s as f32) / (t as f32) };
            if rate > best_rate { best_rate = rate; best = i; }
        }
        RouteDecision { index: best, reason: "exploit" }
    }

    pub fn observe(&self, index: usize, success: bool) {
        let mut stats = self.stats.lock().expect("router stats");
        let entry = stats.entry(index).or_insert((0, 0));
        entry.0 += 1; if success { entry.1 += 1; }
    }
}

// Simple xorshift RNG to avoid pulling in heavy deps
fn rand_seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0x12345678u64)
}
fn xorshift64(x: &mut u64) -> u64 { let mut n = *x; n ^= n << 13; n ^= n >> 7; n ^= n << 17; *x = n; n }
fn next_f32(state: &mut u64) -> f32 { ((xorshift64(state) >> 40) as f32) / ((1u64 << 24) as f32) }
fn rand_u32(state: &mut u64) -> u32 { (xorshift64(state) >> 32) as u32 }

