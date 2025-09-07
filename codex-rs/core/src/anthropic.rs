use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::client_common::ResponseStream;
use crate::error::CodexErr;
use crate::error::Result;
use crate::model_family::ModelFamily;
use crate::model_provider_info::ModelProviderInfo;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::mpsc;

#[derive(Debug, Serialize)]
struct AnthropicMessageReq<'a> {
    model: &'a str,
    max_tokens: u64,
    system: &'a str,
    messages: Vec<AnthropicUserMessage<'a>>,
}

#[derive(Debug, Serialize)]
struct AnthropicUserMessage<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct AnthropicMessageResp {
    content: Vec<AnthropicTextBlock>,
}

#[derive(Debug, Deserialize)]
struct AnthropicTextBlock {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

pub async fn call_anthropic(
    prompt: &Prompt,
    model_family: &ModelFamily,
    client: &reqwest::Client,
    provider: &ModelProviderInfo,
    model: &str,
    max_output_tokens: Option<u64>,
) -> Result<ResponseStream> {
    let system = prompt.get_full_instructions(model_family);

    // Flatten text content from the prompt into a single user string.
    let mut user_text = String::new();
    for item in prompt.get_formatted_input() {
        if let ResponseItem::Message { role, content, .. } = item
            && (role == "user" || role == "system")
        {
            for c in content {
                match c {
                    ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                        user_text.push_str(&text);
                    }
                    _ => {}
                }
            }
        }
    }
    if user_text.trim().is_empty() {
        // Avoid empty content per API expectations.
        user_text = "".to_string();
    }

    let body = AnthropicMessageReq {
        model,
        max_tokens: max_output_tokens.unwrap_or(1024),
        system: &system,
        messages: vec![AnthropicUserMessage {
            role: "user",
            content: &user_text,
        }],
    };

    let key = provider.api_key()?;
    let url = format!(
        "{}/v1/messages",
        provider
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.anthropic.com".to_string())
    );
    let mut req = client.post(url);
    if let Some(k) = key {
        req = req.header("x-api-key", k);
    }
    req = req.header("anthropic-version", "2023-06-01");

    let res = req.json(&body).send().await.map_err(CodexErr::Reqwest)?;
    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(CodexErr::UnexpectedStatus(status, body));
    }
    let data: AnthropicMessageResp = res.json().await.map_err(CodexErr::Reqwest)?;

    let mut text = String::new();
    for block in data.content {
        if block.kind == "text"
            && let Some(t) = block.text
        {
            text.push_str(&t);
        }
    }

    // Convert to a single ResponseItem and return a stream that yields it then completes.
    let (tx, rx) = mpsc::channel(4);
    tokio::spawn(async move {
        let _ = tx
            .send(Ok(ResponseEvent::OutputItemDone(ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText { text }],
            })))
            .await;
        let _ = tx
            .send(Ok(ResponseEvent::Completed {
                response_id: String::new(),
                token_usage: None,
            }))
            .await;
    });

    Ok(ResponseStream { rx_event: rx })
}
