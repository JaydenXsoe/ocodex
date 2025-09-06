mod client;
mod parser;
mod pull;
mod url;

pub use client::OllamaClient;
use codex_core::config::Config;
pub use pull::CliProgressReporter;
pub use pull::PullEvent;
pub use pull::PullProgressReporter;
pub use pull::TuiProgressReporter;

/// Default OSS model to use when `--oss` is passed without an explicit `-m`.
pub const DEFAULT_OSS_MODEL: &str = "gpt-oss:20b";
/// Heavier OSS model used when the environment appears capable.
pub const HEAVY_OSS_MODEL: &str = "gpt-oss:120b";

/// Choose a default OSS model based on host resources.
///
/// - If the system appears sufficiently provisioned, prefer `gpt-oss:120b`.
/// - Otherwise, fall back to `gpt-oss:20b`.
///
/// Heuristic: total system memory >= 64 GiB selects the heavier model.
/// This intentionally avoids probing Ollama or triggering network pulls.
pub fn dynamic_default_oss_model() -> &'static str {
    // Use a lightweight, cross‑platform heuristic based on total memory.
    // Avoids network calls and keeps selection deterministic.
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();

    // sysinfo reports memory in kibibytes. Convert to GiB.
    let total_mem_gib = sys.total_memory() as f64 / (1024.0 * 1024.0);
    // Prefer heavy model when the host has ample memory.
    if total_mem_gib >= 64.0 {
        HEAVY_OSS_MODEL
    } else {
        DEFAULT_OSS_MODEL
    }
}

/// Prepare the local OSS environment when `--oss` is selected.
///
/// - Ensures a local Ollama server is reachable.
/// - Checks if the model exists locally and pulls it if missing.
pub async fn ensure_oss_ready(config: &Config) -> std::io::Result<()> {
    // Only download when the requested model is the default OSS model (or when -m is not provided).
    let model = config.model.as_ref();

    // Verify local Ollama is reachable.
    let ollama_client = crate::OllamaClient::try_from_oss_provider(config).await?;

    // If the model is not present locally, pull it.
    match ollama_client.fetch_models().await {
        Ok(models) => {
            if !models.iter().any(|m| m == model) {
                let mut reporter = crate::CliProgressReporter::new();
                ollama_client
                    .pull_with_reporter(model, &mut reporter)
                    .await?;
            }
        }
        Err(err) => {
            // Not fatal; higher layers may still proceed and surface errors later.
            tracing::warn!("Failed to query local models from Ollama: {}.", err);
        }
    }

    Ok(())
}
