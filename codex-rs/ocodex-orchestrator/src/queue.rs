use crate::Task;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub trait TaskQueue {
    fn push_all(&self, tasks: Vec<Task>);
    fn pop(&self) -> Option<Task>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool { self.len() == 0 }
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
}

