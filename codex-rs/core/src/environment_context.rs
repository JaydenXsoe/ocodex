use serde::Deserialize;
use serde::Serialize;
use strum_macros::Display as DeriveDisplay;

use crate::protocol::AskForApproval;
use crate::protocol::SandboxPolicy;
use crate::shell::Shell;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use std::path::PathBuf;

/// wraps environment context message in a tag for the model to parse more easily.
pub(crate) const ENVIRONMENT_CONTEXT_START: &str = "<environment_context>";
pub(crate) const ENVIRONMENT_CONTEXT_END: &str = "</environment_context>";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, DeriveDisplay)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum NetworkAccess {
    Restricted,
    Enabled,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename = "environment_context", rename_all = "snake_case")]
pub(crate) struct EnvironmentContext {
    pub cwd: Option<PathBuf>,
    pub approval_policy: Option<AskForApproval>,
    pub sandbox_mode: Option<SandboxMode>,
    pub network_access: Option<NetworkAccess>,
    pub shell: Option<Shell>,
}

impl EnvironmentContext {
    pub fn new(
        cwd: Option<PathBuf>,
        approval_policy: Option<AskForApproval>,
        sandbox_policy: Option<SandboxPolicy>,
        shell: Option<Shell>,
    ) -> Self {
        Self {
            cwd,
            approval_policy,
            sandbox_mode: match sandbox_policy {
                Some(SandboxPolicy::DangerFullAccess) => Some(SandboxMode::DangerFullAccess),
                Some(SandboxPolicy::ReadOnly) => Some(SandboxMode::ReadOnly),
                Some(SandboxPolicy::WorkspaceWrite { .. }) => Some(SandboxMode::WorkspaceWrite),
                None => None,
            },
            network_access: match sandbox_policy {
                Some(SandboxPolicy::DangerFullAccess) => Some(NetworkAccess::Enabled),
                Some(SandboxPolicy::ReadOnly) => Some(NetworkAccess::Restricted),
                Some(SandboxPolicy::WorkspaceWrite { network_access, .. }) => {
                    if network_access {
                        Some(NetworkAccess::Enabled)
                    } else {
                        Some(NetworkAccess::Restricted)
                    }
                }
                None => None,
            },
            shell,
        }
    }
}

impl EnvironmentContext {
    /// Serializes the environment context to XML. Libraries like `quick-xml`
    /// require custom macros to handle Enums with newtypes, so we just do it
    /// manually, to keep things simple. Output looks like:
    ///
    /// ```xml
    /// <environment_context>
    ///   <cwd>...</cwd>
    ///   <approval_policy>...</approval_policy>
    ///   <sandbox_mode>...</sandbox_mode>
    ///   <network_access>...</network_access>
    ///   <shell>...</shell>
    /// </environment_context>
    /// ```
    pub fn serialize_to_xml(self) -> String {
        let mut lines = vec![ENVIRONMENT_CONTEXT_START.to_string()];
        if let Some(cwd) = self.cwd {
            lines.push(format!("  <cwd>{}</cwd>", cwd.to_string_lossy()));
        }
        if let Some(approval_policy) = self.approval_policy {
            lines.push(format!(
                "  <approval_policy>{}</approval_policy>",
                approval_policy
            ));
        }
        if let Some(sandbox_mode) = self.sandbox_mode {
            lines.push(format!("  <sandbox_mode>{}</sandbox_mode>", sandbox_mode));
        }
        if let Some(network_access) = self.network_access {
            lines.push(format!(
                "  <network_access>{}</network_access>",
                network_access
            ));
        }
        if let Some(shell) = self.shell
            && let Some(shell_name) = shell.name()
        {
            lines.push(format!("  <shell>{}</shell>", shell_name));
        }

        // Surface web search configurations to the agent when credentials are
        // present in the environment so the model can format queries correctly
        // without guesswork.
        let has_google_key = std::env::var("GOOGLE_API_KEY").is_ok();
        let has_google_cse = std::env::var("GOOGLE_CSE_ID").is_ok();
        if has_google_key && has_google_cse {
            // Recommend a curl template and a portable URI-encoding snippet.
            // The model should replace `{QUERY}` with a URL-encoded query.
            let curl_template = "curl -s \"https://www.googleapis.com/customsearch/v1?key=$GOOGLE_API_KEY&cx=$GOOGLE_CSE_ID&q={QUERY}&num=5\"";
            let encode_hint = "ENC=$(printf '%s' \"$QUERY\" | jq -sRr @uri)";
            lines.push("  <google_search>".to_string());
            lines.push("    <api>customsearch</api>".to_string());
            lines.push(format!(
                "    <curl_template>{}</curl_template>",
                curl_template
            ));
            lines.push(format!(
                "    <note>URL-encode the query first, e.g., {}</note>",
                encode_hint
            ));
            lines.push("  </google_search>".to_string());
        }

        // Also surface SerpAPI configuration, if available, as an alternative
        // simple web-search path.
        if std::env::var("SERPAPI_KEY").is_ok() {
            let curl_template = "curl -s \"https://serpapi.com/search.json?engine=google&q={QUERY}&api_key=$SERPAPI_KEY&num=5\"";
            let encode_hint = "ENC=$(printf '%s' \"$QUERY\" | jq -sRr @uri)";
            lines.push("  <serpapi_search>".to_string());
            lines.push("    <api>serpapi</api>".to_string());
            lines.push(format!(
                "    <curl_template>{}</curl_template>",
                curl_template
            ));
            lines.push(format!(
                "    <note>URL-encode the query first, e.g., {}</note>",
                encode_hint
            ));
            lines.push("  </serpapi_search>".to_string());
        }
        lines.push(ENVIRONMENT_CONTEXT_END.to_string());
        lines.join("\n")
    }
}

impl From<EnvironmentContext> for ResponseItem {
    fn from(ec: EnvironmentContext) -> Self {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: ec.serialize_to_xml(),
            }],
        }
    }
}
