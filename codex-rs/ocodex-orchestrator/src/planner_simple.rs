use serde_json::json;
use crate::{Playbook, Task, Planner, OrchestrationError};

pub struct SimplePlanner;

impl Planner for SimplePlanner {
    fn plan_from_prompt(&mut self, prompt: &str) -> Result<Playbook, OrchestrationError> {
        let mut id = 1usize;
        let mut tasks: Vec<Task> = Vec::new();
        // Detect environment
        tasks.push(Task { id: format!("{}", id), description: "detect environment".into(), payload: json!({"worker":"env","action":"detect","needs_write_lock": false}) }); id += 1;
        // Placeholder: delegate prompt to an external worker by convention (ocodex) without coupling here
        tasks.push(Task { id: format!("{}", id), description: "execute prompt".into(), payload: json!({"worker":"ocodex","prompt": prompt, "needs_write_lock": true}) });
        Ok(Playbook { name: "simple-plan".into(), tasks })
    }
}

