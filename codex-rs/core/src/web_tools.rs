use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use std::collections::HashSet;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct WebSearchArgs {
    pub query: String,
    #[serde(default)]
    pub max_results: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct HttpGetArgs {
    pub url: String,
    #[serde(default)]
    pub max_bytes: Option<usize>,
}

#[derive(Debug, Serialize)]
struct SearchResultItem {
    title: String,
    url: String,
    snippet: String,
}

#[derive(Debug, Serialize)]
struct WebSearchResult {
    query: String,
    results: Vec<SearchResultItem>,
}

#[derive(Debug, Serialize)]
struct HttpGetResult {
    url: String,
    status: u16,
    content_type: Option<String>,
    content_bytes_len: usize,
    content_text: Option<String>,
}

/// Perform a lightweight web search using DuckDuckGo's Instant Answer API as a best-effort fallback.
/// This API does not return full SERP data; we extract title/url/snippet from RelatedTopics where available.
pub async fn web_search(args: WebSearchArgs) -> anyhow::Result<String> {
    // Best-effort load of .env from current working directory.
    let _ = dotenvy::dotenv();
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build()?;

    // Choose provider based on env vars. We never log key values.
    enum Provider {
        SerpApi { key: String },
        GoogleCse { key: String, cx: String },
        DuckDuckGo,
    }
    fn choose_provider() -> Provider {
        if let Ok(key) = std::env::var("SERPAPI_KEY")
            && !key.trim().is_empty() {
                return Provider::SerpApi { key };
            }
        if let (Ok(key), Ok(cx)) = (
            std::env::var("GOOGLE_CSE_KEY"),
            std::env::var("GOOGLE_CSE_CX"),
        )
            && !key.trim().is_empty() && !cx.trim().is_empty() {
                return Provider::GoogleCse { key, cx };
            }
        Provider::DuckDuckGo
    }

    async fn serpapi_search(
        client: &reqwest::Client,
        key: &str,
        query: &str,
        cap: usize,
    ) -> Vec<SearchResultItem> {
        // SerpAPI Google search endpoint
        let url = format!(
            "https://serpapi.com/search.json?engine=google&q={}&num={}&api_key={}",
            urlencoding::encode(query),
            cap,
            urlencoding::encode(key)
        );
        let mut out = Vec::new();
        let Ok(resp) = client.get(url).send().await else {
            return out;
        };
        let Ok(text) = resp.text().await else {
            return out;
        };
        let v: serde_json::Value = serde_json::from_str(&text).unwrap_or(json!({}));
        if let Some(items) = v.get("organic_results").and_then(|x| x.as_array()) {
            for item in items.iter().take(cap) {
                let title = item
                    .get("title")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let url = item
                    .get("link")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let snippet = item
                    .get("snippet")
                    .and_then(|x| x.as_str())
                    .unwrap_or(title.as_str())
                    .to_string();
                if !title.is_empty() && !url.is_empty() {
                    out.push(SearchResultItem {
                        title,
                        url,
                        snippet,
                    });
                }
            }
        }
        out
    }

    async fn google_cse_search(
        client: &reqwest::Client,
        key: &str,
        cx: &str,
        query: &str,
        cap: usize,
    ) -> Vec<SearchResultItem> {
        let url = format!(
            "https://www.googleapis.com/customsearch/v1?key={}&cx={}&q={}&num={}",
            urlencoding::encode(key),
            urlencoding::encode(cx),
            urlencoding::encode(query),
            cap
        );
        let mut out = Vec::new();
        let Ok(resp) = client.get(url).send().await else {
            return out;
        };
        let Ok(text) = resp.text().await else {
            return out;
        };
        let v: serde_json::Value = serde_json::from_str(&text).unwrap_or(json!({}));
        if let Some(items) = v.get("items").and_then(|x| x.as_array()) {
            for item in items.iter().take(cap) {
                let title = item
                    .get("title")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let url = item
                    .get("link")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let snippet = item
                    .get("snippet")
                    .and_then(|x| x.as_str())
                    .unwrap_or(title.as_str())
                    .to_string();
                if !title.is_empty() && !url.is_empty() {
                    out.push(SearchResultItem {
                        title,
                        url,
                        snippet,
                    });
                }
            }
        }
        out
    }

    // Helper to query DDG Instant Answers and extract various fields.
    async fn ddg_fetch(
        client: &reqwest::Client,
        query: &str,
        cap: usize,
        seen: &mut HashSet<String>,
    ) -> Vec<SearchResultItem> {
        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
            urlencoding::encode(query)
        );
        let mut out = Vec::new();
        let Ok(resp) = client.get(url).send().await else {
            return out;
        };
        let Ok(body) = resp.text().await else {
            return out;
        };
        let v: serde_json::Value = serde_json::from_str(&body).unwrap_or(json!({}));

        // 1) Results array
        if let Some(items) = v.get("Results").and_then(|x| x.as_array()) {
            for item in items {
                if let (Some(text), Some(first_url)) = (item.get("Text"), item.get("FirstURL")) {
                    let title = text.as_str().unwrap_or("").to_string();
                    let url = first_url.as_str().unwrap_or("").to_string();
                    if title.is_empty() || url.is_empty() {
                        continue;
                    }
                    if seen.insert(url.clone()) {
                        let snippet = title.clone();
                        out.push(SearchResultItem {
                            title,
                            url,
                            snippet,
                        });
                        if out.len() >= cap {
                            return out;
                        }
                    }
                }
            }
        }

        // 2) RelatedTopics (including nested Topics)
        if let Some(items) = v.get("RelatedTopics").and_then(|x| x.as_array()) {
            for item in items {
                if let (Some(text), Some(first_url)) = (item.get("Text"), item.get("FirstURL")) {
                    let title = text.as_str().unwrap_or("").to_string();
                    let url = first_url.as_str().unwrap_or("").to_string();
                    if title.is_empty() || url.is_empty() {
                        continue;
                    }
                    if seen.insert(url.clone()) {
                        let snippet = title.clone();
                        out.push(SearchResultItem {
                            title,
                            url,
                            snippet,
                        });
                        if out.len() >= cap {
                            return out;
                        }
                    }
                } else if let Some(topics) = item.get("Topics").and_then(|x| x.as_array()) {
                    for t in topics {
                        if let (Some(text), Some(first_url)) = (t.get("Text"), t.get("FirstURL")) {
                            let title = text.as_str().unwrap_or("").to_string();
                            let url = first_url.as_str().unwrap_or("").to_string();
                            if title.is_empty() || url.is_empty() {
                                continue;
                            }
                            if seen.insert(url.clone()) {
                                let snippet = title.clone();
                                out.push(SearchResultItem {
                                    title,
                                    url,
                                    snippet,
                                });
                                if out.len() >= cap {
                                    return out;
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3) AbstractURL + AbstractText
        if let (Some(u), Some(t)) = (
            v.get("AbstractURL").and_then(|x| x.as_str()),
            v.get("AbstractText").and_then(|x| x.as_str()),
        ) {
            let url = u.to_string();
            let title = v
                .get("Heading")
                .and_then(|x| x.as_str())
                .unwrap_or(t)
                .to_string();
            if !url.is_empty() && seen.insert(url.clone()) {
                let snippet = t.to_string();
                out.push(SearchResultItem {
                    title,
                    url,
                    snippet,
                });
            }
        }

        out
    }

    let q = args.query;
    let cap = args.max_results.unwrap_or(5) as usize;
    let mut seen: HashSet<String> = HashSet::new();
    let mut results: Vec<SearchResultItem> = Vec::new();

    // Primary query via chosen provider; fallback to DDG
    match choose_provider() {
        Provider::SerpApi { key } => {
            let r = serpapi_search(&client, &key, &q, cap).await;
            if r.is_empty() {
                results.extend(ddg_fetch(&client, &q, cap, &mut seen).await);
            } else {
                results.extend(r);
            }
        }
        Provider::GoogleCse { key, cx } => {
            let r = google_cse_search(&client, &key, &cx, &q, cap).await;
            if r.is_empty() {
                results.extend(ddg_fetch(&client, &q, cap, &mut seen).await);
            } else {
                results.extend(r);
            }
        }
        Provider::DuckDuckGo => {
            results.extend(ddg_fetch(&client, &q, cap, &mut seen).await);
        }
    }

    // If results are scarce, try a few helpful variants
    if results.len() < cap {
        let q_lower = q.to_lowercase();
        let mut variants: Vec<String> = Vec::new();
        if !q_lower.contains("google") {
            variants.push(format!("google {q}"));
        }
        if q_lower.contains("worlds") && !q_lower.contains("world model") {
            variants.push(q_lower.replace("worlds", "world model"));
        }
        variants.push(format!("{q} site:blog.google"));
        variants.push(format!("{q} site:deepmind.com"));

        for vq in variants {
            if results.len() >= cap {
                break;
            }
            let more = ddg_fetch(&client, &vq, cap - results.len(), &mut seen).await;
            results.extend(more);
        }
    }

    results.truncate(cap);
    let payload = WebSearchResult { query: q, results };
    Ok(serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string()))
}

/// Fetch a URL and return text content (truncated) and basic metadata.
pub async fn http_get(args: HttpGetArgs) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .build()?;

    let max_bytes = args.max_bytes.unwrap_or(200_000);
    let resp = client.get(&args.url).send().await?;
    let status = resp.status();
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Stream and cap size to avoid OOMs
    let mut bytes = Vec::with_capacity(1024);
    let mut stream = resp.bytes_stream();
    use futures::StreamExt as _;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if bytes.len() + chunk.len() > max_bytes {
            let remaining = max_bytes.saturating_sub(bytes.len());
            bytes.extend_from_slice(&chunk[..remaining.min(chunk.len())]);
            break;
        } else {
            bytes.extend_from_slice(&chunk);
        }
    }

    let content_bytes_len = bytes.len();
    let content_text = if let Some(ct) = &content_type {
        let is_text = ct.starts_with("text/")
            || ct.contains("json")
            || ct.contains("xml")
            || ct.contains("javascript");
        if is_text {
            Some(String::from_utf8_lossy(&bytes).to_string())
        } else {
            None
        }
    } else {
        Some(String::from_utf8_lossy(&bytes).to_string())
    };

    let res = HttpGetResult {
        url: args.url,
        status: status.as_u16(),
        content_type,
        content_bytes_len,
        content_text,
    };
    Ok(serde_json::to_string(&res).unwrap_or_else(|_| "{}".to_string()))
}
