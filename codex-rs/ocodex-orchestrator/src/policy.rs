use crate::{Event, EventBus, EventKind, Task};
use std::sync::Arc;

pub trait ExecutionPolicy {
    fn before_task(&self, task: &Task, events: &Arc<dyn EventBus + Send + Sync>) -> Result<(), String>;
    fn after_task(&self, task: &Task, events: &Arc<dyn EventBus + Send + Sync>) -> Result<(), String>;
}

#[derive(Default)]
pub struct NoopExecutionPolicy;

impl ExecutionPolicy for NoopExecutionPolicy {
    fn before_task(&self, task: &Task, events: &Arc<dyn EventBus + Send + Sync>) -> Result<(), String> {
        events.publish(Event { kind: EventKind::Info, message: format!("policy:before:{}", task.id) });
        Ok(())
    }
    fn after_task(&self, task: &Task, events: &Arc<dyn EventBus + Send + Sync>) -> Result<(), String> {
        events.publish(Event { kind: EventKind::Info, message: format!("policy:after:{}", task.id) });
        Ok(())
    }
}

