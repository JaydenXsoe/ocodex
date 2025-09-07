use crate::{OrchestrationError, Task};

pub trait TaskRunner: Send + Sync {
    fn run_one(&self, task: Task) -> Result<(), OrchestrationError>;
}

pub trait Scheduler: Send + Sync {
    fn run(&self, tasks: Vec<Task>, max_concurrency: usize, runner: &dyn TaskRunner) -> Result<(), OrchestrationError>;
}

#[derive(Default)]
pub struct InProcessScheduler;

impl Scheduler for InProcessScheduler {
    fn run(&self, tasks: Vec<Task>, _max_concurrency: usize, runner: &dyn TaskRunner) -> Result<(), OrchestrationError> {
        for t in tasks.into_iter() {
            runner.run_one(t)?;
        }
        Ok(())
    }
}

pub struct EnvProbe;

impl EnvProbe {
    pub fn cpus() -> usize {
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
    }
}

/// Compute a safe concurrency based on caps, CPU availability, and workload size.
pub fn compute_concurrency(cap: usize, total_tasks: usize) -> usize {
    let env_cap = std::env::var("ORCH_MAX_CONCURRENCY").ok().and_then(|s| s.parse::<usize>().ok());
    let cpus = EnvProbe::cpus();
    let cpu_limit = cpus.saturating_sub(1).max(1); // leave one core for OS
    let hard_cap = env_cap.unwrap_or(cap.max(1));
    hard_cap.min(cpu_limit).min(total_tasks.max(1))
}
