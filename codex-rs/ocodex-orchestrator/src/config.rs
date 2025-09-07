use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationConfig {
    pub max_concurrency: usize,
    pub planner: Option<String>,
    pub model: Option<String>,
    pub backend: Option<String>, // http|ollama|custom
    pub container_mode: Option<String>, // devcontainer|docker|k8s
    pub project_name: Option<String>,
    pub qc_endpoint: Option<String>,
}

impl Default for OrchestrationConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 1,
            planner: None,
            model: None,
            backend: None,
            container_mode: None,
            project_name: None,
            qc_endpoint: None,
        }
    }
}
