use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemorySnapshot {
    pub state: JsonValue,
    pub todo: Vec<TodoItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub title: String,
    pub status: String, // open|in_progress|done
    pub assignee: Option<String>,
    pub priority: Option<String>, // P1|P2|P3
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryDelta {
    pub state_patch: JsonValue,
    pub todo_add: Vec<TodoItem>,
    pub todo_update: Vec<(String, String)>, // (title, new_status)
}

pub trait MemoryService {
    fn load(&self) -> MemorySnapshot;
    fn save(&self, snapshot: &MemorySnapshot);
    fn merge(&self, delta: &MemoryDelta) -> MemorySnapshot;
}

#[derive(Default)]
pub struct InMemoryMemoryService {
    inner: Arc<Mutex<MemorySnapshot>>,
}

impl InMemoryMemoryService {
    pub fn new() -> Self { Self::default() }
}

fn merge_json(base: &mut JsonValue, patch: &JsonValue) {
    match (base, patch) {
        (JsonValue::Object(b), JsonValue::Object(p)) => {
            for (k, v) in p.iter() {
                if let Some(bv) = b.get_mut(k) {
                    merge_json(bv, v);
                } else {
                    b.insert(k.clone(), v.clone());
                }
            }
        }
        (b, p) => {
            *b = p.clone();
        }
    }
}

impl MemoryService for InMemoryMemoryService {
    fn load(&self) -> MemorySnapshot {
        self.inner.lock().expect("mem poisoned").clone()
    }

    fn save(&self, snapshot: &MemorySnapshot) {
        let mut guard = self.inner.lock().expect("mem poisoned");
        *guard = snapshot.clone();
    }

    fn merge(&self, delta: &MemoryDelta) -> MemorySnapshot {
        let mut guard = self.inner.lock().expect("mem poisoned");
        if !delta.state_patch.is_null() {
            if guard.state.is_null() {
                guard.state = json!({});
            }
            merge_json(&mut guard.state, &delta.state_patch);
        }
        guard.todo.extend(delta.todo_add.clone());
        for (title, new_status) in delta.todo_update.iter() {
            if let Some(item) = guard.todo.iter_mut().find(|t| &t.title == title) {
                item.status = new_status.clone();
            }
        }
        guard.clone()
    }
}

