pub trait Metrics: Send + Sync {
    fn inc(&self, _key: &str) {}
    fn observe_ms(&self, _key: &str, _ms: u64) {}
}

#[derive(Default)]
pub struct NoopMetrics;

impl Metrics for NoopMetrics {}

