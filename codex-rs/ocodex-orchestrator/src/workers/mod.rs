use serde_json::json;
use serde_json::Value as JsonValue;

use crate::{OrchestrationError, Task, TaskWorker};

pub struct EnvWorker;
impl TaskWorker for EnvWorker {
    fn name(&self) -> &'static str { "env" }
    fn can_handle(&self, task: &Task) -> bool {
        task.payload.get("worker").and_then(|v| v.as_str()) == Some("env") ||
        task.payload.get("action").and_then(|v| v.as_str()) == Some("detect")
    }
    fn run(&mut self, task: Task) -> Result<JsonValue, OrchestrationError> {
        let exists = |p: &str| std::path::Path::new(p).exists();
        let mut langs: Vec<&str> = Vec::new();
        let mut tools: Vec<&str> = Vec::new();
        let mut reasons = serde_json::Map::new();
        if exists("package.json") { langs.push("node"); reasons.insert("node".into(), json!("package.json")); }
        if exists("requirements.txt") || exists("pyproject.toml") { langs.push("python"); reasons.insert("python".into(), json!("requirements/pyproject")); }
        if exists("Cargo.toml") { langs.push("rust"); reasons.insert("rust".into(), json!("Cargo.toml")); }
        if exists("go.mod") { langs.push("go"); reasons.insert("go".into(), json!("go.mod")); }
        if exists("pom.xml") || exists("build.gradle") { langs.push("java"); reasons.insert("java".into(), json!("pom.xml/gradle")); }
        if exists("Gemfile") { langs.push("ruby"); reasons.insert("ruby".into(), json!("Gemfile")); }
        if exists("composer.json") { langs.push("php"); reasons.insert("php".into(), json!("composer.json")); }
        if exists("Makefile") { tools.push("make"); }
        if exists("Dockerfile") { tools.push("dockerfile"); }
        if exists("Justfile") || exists("justfile") { tools.push("just"); }
        let mut container = serde_json::Map::new();
        container.insert("languages".into(), json!(langs));
        container.insert("tools".into(), json!(tools));
        container.insert("reasons".into(), json!(reasons));
        Ok(json!({"memory_update": { "container": JsonValue::Object(container) }, "note": "environment detection complete", "task_id": task.id }))
    }
}

pub struct PatchWorker;
impl TaskWorker for PatchWorker {
    fn name(&self) -> &'static str { "patch" }
    fn can_handle(&self, task: &Task) -> bool {
        task.payload.get("worker").and_then(|v| v.as_str()) == Some("patch") ||
        task.payload.get("action").and_then(|v| v.as_str()) == Some("apply_patch")
    }
    fn run(&mut self, task: Task) -> Result<JsonValue, OrchestrationError> {
        let action = task.payload.get("action").and_then(|v| v.as_str()).unwrap_or("");
        if action != "apply_patch" { return Err(OrchestrationError::Unsupported("patch action")); }
        let patch = task.payload.get("patch").and_then(|v| v.as_str()).unwrap_or("");
        let tool = task.payload.get("tool").and_then(|v| v.as_str()).unwrap_or("git");
        let cwd = task.payload.get("cwd").and_then(|v| v.as_str());
        let mut cmd = std::process::Command::new(match tool { "patch" => "patch", _ => "git" });
        if tool == "patch" {
            cmd.arg("-p0").arg("-t");
        } else {
            cmd.arg("apply").arg("-p0").arg("--whitespace=nowarn");
        }
        if let Some(dir) = cwd { cmd.current_dir(dir); }
        cmd.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| OrchestrationError::ExecutionFailed(e.to_string()))?;
        use std::io::Write;
        if let Some(mut s) = child.stdin.take() { let _ = s.write_all(patch.as_bytes()); }
        let out = child.wait_with_output().map_err(|e| OrchestrationError::ExecutionFailed(e.to_string()))?;
        let success = out.status.success();
        Ok(json!({
            "success": success,
            "tool": tool,
            "status": out.status.code(),
            "stdout": String::from_utf8_lossy(&out.stdout),
            "stderr": String::from_utf8_lossy(&out.stderr),
        }))
    }
}

pub struct ReviewerWorker;
impl TaskWorker for ReviewerWorker {
    fn name(&self) -> &'static str { "reviewer" }
    fn can_handle(&self, task: &Task) -> bool {
        task.payload.get("worker").and_then(|v| v.as_str()) == Some("reviewer")
    }
    fn run(&mut self, task: Task) -> Result<JsonValue, OrchestrationError> {
        let summary = task.payload.get("summary").cloned().unwrap_or_else(|| json!([]));
        Ok(json!({"review": {"summary": summary}, "success": true, "task_id": task.id }))
    }
}

