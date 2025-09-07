use crate::Task;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub trait TaskQueue {
    fn push_all(&self, tasks: Vec<Task>);
    fn pop(&self) -> Option<Task>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool { self.len() == 0 }
    /// Pop the next eligible task respecting write-lock semantics.
    /// Returns (task, needs_write_lock). Default falls back to simple pop.
    fn pop_eligible(&self, writes_in_flight: usize) -> Option<(Task, bool)> {
        let t = self.pop()?;
        let needs = t
            .payload
            .get("needs_write_lock")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if needs && writes_in_flight > 0 {
            None
        } else {
            Some((t, needs))
        }
    }
}

#[derive(Default)]
pub struct InMemoryTaskQueue {
    inner: Arc<Mutex<VecDeque<Task>>>,
}

impl InMemoryTaskQueue {
    pub fn new() -> Self { Self::default() }
}

impl TaskQueue for InMemoryTaskQueue {
    fn push_all(&self, tasks: Vec<Task>) {
        let mut guard = self.inner.lock().expect("queue poisoned");
        for t in tasks { guard.push_back(t); }
    }
    fn pop(&self) -> Option<Task> {
        let mut guard = self.inner.lock().expect("queue poisoned");
        guard.pop_front()
    }
    fn len(&self) -> usize {
        let guard = self.inner.lock().expect("queue poisoned");
        guard.len()
    }
    fn pop_eligible(&self, writes_in_flight: usize) -> Option<(Task, bool)> {
        let mut guard = self.inner.lock().expect("queue poisoned");
        // Find first eligible task considering write lock
        let mut pick: Option<usize> = None;
        let mut needs_flag = false;
        for (i, t) in guard.iter().enumerate() {
            let needs = t
                .payload
                .get("needs_write_lock")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if needs && writes_in_flight > 0 { continue; }
            pick = Some(i);
            needs_flag = needs;
            break;
        }
        match pick {
            Some(i) => Some((guard.remove(i).expect("index valid"), needs_flag)),
            None => None,
        }
    }
}
