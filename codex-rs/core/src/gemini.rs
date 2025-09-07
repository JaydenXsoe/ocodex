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
struct GeminiPart<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a str>,
}

#[derive(Debug, Serialize)]
struct GeminiContent<'a> {
    role: &'static str,
    parts: Vec<GeminiPart<'a>>,
}

#[derive(Debug, Serialize)]
struct GeminiReq<'a> {
    contents: Vec<GeminiContent<'a>>,
}

#[derive(Debug, Deserialize)]
struct GeminiRespCandidate {
    content: GeminiRespContent,
}

#[derive(Debug, Deserialize)]
struct GeminiRespContent {
    parts: Vec<GeminiRespPart>,
}

#[derive(Debug, Deserialize)]
struct GeminiRespPart {
    #[serde(default)]
    text: String,
}

#[derive(Debug, Deserialize)]
struct GeminiResp {
    candidates: Option<Vec<GeminiRespCandidate>>,
}

pub async fn call_gemini(
    prompt: &Prompt,
    model_family: &ModelFamily,
    client: &reqwest::Client,
    provider: &ModelProviderInfo,
    model: &str,
) -> Result<ResponseStream> {
    let system = prompt.get_full_instructions(model_family);
    let mut user_text = String::new();
    for item in prompt.get_formatted_input() {
        if let ResponseItem::Message { content, .. } = item {
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
    let combined = format!("{}\n\n{}", system, user_text);
    let contents = vec![GeminiContent {
        role: "user",
        parts: vec![GeminiPart {
            text: Some(&combined),
        }],
    }];
    let body = GeminiReq { contents };

    // Build URL: {base}/v1beta/models/{model}:generateContent?key=API_KEY
    let base = provider
        .base_url
        .clone()
        .unwrap_or_else(|| "https://generativelanguage.googleapis.com".to_string());
    let key = provider.api_key()?;
    let key_qs = key.as_deref().unwrap_or("");
    let url = format!("{base}/v1beta/models/{model}:generateContent?key={key_qs}");

    let res = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(CodexErr::Reqwest)?;
    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(CodexErr::UnexpectedStatus(status, body));
    }
    let data: GeminiResp = res.json().await.map_err(CodexErr::Reqwest)?;
    let text = data
        .candidates
        .unwrap_or_default()
        .into_iter()
        .flat_map(|c| c.content.parts)
        .map(|p| p.text)
        .collect::<Vec<_>>()
        .join("");

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
