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
