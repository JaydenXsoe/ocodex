use serde_json::Value as JsonValue;

pub trait BlackboardService: Send + Sync {
    fn get(&self, key: &str) -> Option<JsonValue>;
    fn set(&self, key: &str, val: JsonValue);
    fn merge(&self, key: &str, patch: &JsonValue);
    fn snapshot(&self) -> std::collections::HashMap<String, JsonValue>;
}

#[derive(Default)]
pub struct InMemoryBlackboard { inner: std::sync::Mutex<std::collections::HashMap<String, JsonValue>> }

impl BlackboardService for InMemoryBlackboard {
    fn get(&self, key: &str) -> Option<JsonValue> { self.inner.lock().ok().and_then(|m| m.get(key).cloned()) }
    fn set(&self, key: &str, val: JsonValue) { if let Ok(mut m) = self.inner.lock() { m.insert(key.to_string(), val); } }
    fn merge(&self, key: &str, patch: &JsonValue) {
        if let Ok(mut m) = self.inner.lock() {
            let v = m.remove(key).unwrap_or_else(|| JsonValue::Null);
            m.insert(key.to_string(), merge_json(v, patch.clone()));
        }
    }
    fn snapshot(&self) -> std::collections::HashMap<String, JsonValue> { self.inner.lock().map(|m| m.clone()).unwrap_or_default() }
}

fn merge_json(mut base: JsonValue, patch: JsonValue) -> JsonValue {
    use serde_json::Value::{Array, Object};
    match (&mut base, patch) {
        (Object(b), Object(p)) => { for (k, v) in p.into_iter() { let nv = if let Some(ev) = b.remove(&k) { merge_json(ev, v) } else { v }; b.insert(k, nv); } JsonValue::Object(b.clone()) }
        (Array(a), Array(mut p)) => { a.append(&mut p); JsonValue::Array(a.clone()) }
        (_, p) => p,
    }
}

