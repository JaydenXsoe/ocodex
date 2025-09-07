use serde::{Deserialize, Serialize};
use std::sync::{mpsc, Arc, Mutex};

/// Event schema version for consumers to validate compatibility.
pub const EVENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventKind {
    Info,
    Warn,
    Error,
    Progress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub kind: EventKind,
    pub message: String,
}

pub trait EventBus {
    fn subscribe(&self) -> mpsc::Receiver<Event>;
    fn publish(&self, event: Event);
}

#[derive(Default)]
pub struct InProcEventBus {
    inner: Arc<Mutex<Vec<mpsc::Sender<Event>>>>,
}

impl InProcEventBus {
    pub fn new() -> Self { Self::default() }
}

impl EventBus for InProcEventBus {
    fn subscribe(&self) -> mpsc::Receiver<Event> {
        let (tx, rx) = mpsc::channel();
        let mut guard = self.inner.lock().expect("event bus poisoned");
        guard.push(tx);
        drop(guard);
        rx
    }

    fn publish(&self, event: Event) {
        let guard = self.inner.lock().expect("event bus poisoned");
        for sub in guard.iter() {
            let _ = sub.send(event.clone());
        }
    }
}

