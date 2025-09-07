use crate::{Planner, Playbook, Task, OrchestrationError};
use serde_json::json;

pub struct LlmPlanner {
    pub model: String,
    pub base_url: Option<String>, // OpenAI-compatible /v1
    pub api_key: Option<String>,
    pub use_ollama_cli: bool,
}

impl LlmPlanner {
    pub fn from_env() -> Option<Self> {
        let model = std::env::var("ORCH_LLM_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
        let base = std::env::var("OPENAI_BASE_URL").ok().or_else(|| std::env::var("CODEX_OSS_BASE_URL").ok());
        let key = std::env::var("OPENAI_API_KEY").ok();
        let use_ollama_cli = std::env::var("ORCH_LLM_OLLAMA_CLI").map(|v| v == "1" || v.to_lowercase() == "true").unwrap_or(false);
        if base.is_some() || key.is_some() || use_ollama_cli { Some(Self { model, base_url: base, api_key: key, use_ollama_cli }) } else { None }
    }

    fn call_http(&mut self, system: &str, user: &str) -> Result<String, OrchestrationError> {
        #[cfg(feature = "llm-http")]
        {
            use ureq::json as ujson;
            let url = format!("{}/chat/completions", self.base_url.as_ref().unwrap().trim_end_matches('/'));
            let req_body = ujson!({
                "model": self.model,
                "messages": [
                    {"role":"system","content":system},
                    {"role":"user","content":user}
                ],
                "temperature": 0.1
            });
            let mut r = ureq::post(&url).set("content-type","application/json");
            if let Some(k) = &self.api_key { r = r.set("authorization", &format!("Bearer {}", k)); }
            let resp = r.send_json(req_body).map_err(|e| OrchestrationError::PlanningFailed(e.to_string()))?;
            let v: serde_json::Value = resp.into_json().map_err(|e| OrchestrationError::PlanningFailed(e.to_string()))?;
            let content = v["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
            Ok(content)
        }
        #[cfg(not(feature = "llm-http"))]
        { let _ = (system, user); Err(OrchestrationError::PlanningFailed("llm-http feature disabled".into())) }
    }

    fn call_ollama_cli(&mut self, system: &str, user: &str) -> Result<String, OrchestrationError> {
        let prompt = format!("System:{}\n\nUser:{}\n\nAssistant:", system, user);
        let out = std::process::Command::new("ollama")
            .arg("run").arg(&self.model)
            .arg(prompt)
            .output()
            .map_err(|e| OrchestrationError::PlanningFailed(e.to_string()))?;
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }
}

impl Planner for LlmPlanner {
    fn plan_from_prompt(&mut self, prompt: &str) -> Result<Playbook, OrchestrationError> {
        let system = "You are an orchestrator architect. Plan a sequence of tasks for a worker pool. Output ONLY minified JSON matching this schema: {\"name\": string, \"tasks\": [{\"id\": string, \"description\": string, \"worker\": string, \"payload\": object}] }. The \"worker\" field routes tasks (e.g., 'ocodex', 'env', 'patch', 'reviewer'). For each task, include \"payload\" to pass to that worker. Do not include comments, markdown, or extra text.";
        let user = format!("Goal: {}", prompt);
        let raw = if self.use_ollama_cli || self.base_url.is_none() { self.call_ollama_cli(system, &user) } else { self.call_http(system, &user) }?;
        let json_text = extract_json(&raw).ok_or_else(|| OrchestrationError::PlanningFailed("LLM did not return JSON".into()))?;
        let v: serde_json::Value = serde_json::from_str(&json_text).map_err(|e| OrchestrationError::PlanningFailed(e.to_string()))?;
        // Convert
        let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("plan");
        let mut tasks_out: Vec<Task> = Vec::new();
        if let Some(tasks) = v.get("tasks").and_then(|x| x.as_array()) {
            for t in tasks {
                let id = t.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let description = t.get("description").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let worker = t.get("worker").and_then(|x| x.as_str()).unwrap_or("ocodex");
                let mut payload = t.get("payload").cloned().unwrap_or_else(|| json!({}));
                if let Some(obj) = payload.as_object_mut() { obj.insert("worker".into(), json!(worker)); }
                tasks_out.push(Task { id, description, payload });
            }
        }
        if tasks_out.is_empty() {
            tasks_out.push(Task { id: "1".into(), description: "delegate prompt to ocodex".into(), payload: json!({"worker":"ocodex","prompt":prompt}) });
        }
        Ok(Playbook { name: name.to_string(), tasks: tasks_out })
    }
}

pub struct AutoPlanner {
    llm: Option<LlmPlanner>,
    simple: crate::planner_simple::SimplePlanner,
}

impl AutoPlanner {
    pub fn new() -> Self { Self { llm: LlmPlanner::from_env(), simple: crate::planner_simple::SimplePlanner } }
}

impl Planner for AutoPlanner {
    fn plan_from_prompt(&mut self, prompt: &str) -> Result<Playbook, OrchestrationError> {
        if let Some(llm) = &mut self.llm {
            if let Ok(p) = llm.plan_from_prompt(prompt) { return Ok(p); }
        }
        self.simple.plan_from_prompt(prompt)
    }
}

fn extract_json(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut start: Option<usize> = None;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'{' { if depth == 0 { start = Some(i); } depth += 1; }
        else if b == b'}' { depth -= 1; if depth == 0 { if let Some(st) = start { return Some(s[st..=i].to_string()); } } }
    }
    None
}

