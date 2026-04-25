use crate::error::TranslateError;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::engine::{TranslateProgress, TranslateRequest, TranslateResult};

#[derive(Debug, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionResponse {
    pub choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
pub struct ChatChoice {
    pub message: ChatMessage,
}

pub fn build_translation_prompt(request: &TranslateRequest) -> String {
    let context = request
        .context_hint
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .map(|v| format!("Context: {}\n\n", v))
        .unwrap_or_default();

    let lines = request
        .texts
        .iter()
        .enumerate()
        .map(|(idx, text)| format!("[{}] {}", idx + 1, text))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "You are a subtitle translation engine.\n\
Translate from {src} to {dst}.\n\
Keep the number of lines exactly the same.\n\
Return only one translated line per input line, in order, with no extra notes.\n\n\
{context}{lines}",
        src = request.source_lang,
        dst = request.target_lang
    )
}

pub fn normalize_base_url(base_url: &str) -> String {
    base_url.trim().trim_end_matches('/').to_string()
}

pub fn validate_http_url(field_name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{}不能为空", field_name));
    }
    if !(value.starts_with("http://") || value.starts_with("https://")) {
        return Err(format!("{}必须以 http:// 或 https:// 开头", field_name));
    }
    Ok(())
}

pub fn parse_multiline_translation(content: &str, expected_count: usize) -> Vec<String> {
    let mut lines: Vec<String> = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect();

    if lines.len() < expected_count {
        lines.resize(expected_count, "[Translation Error]".to_string());
    } else if lines.len() > expected_count {
        lines.truncate(expected_count);
    }

    lines
}

pub async fn send_progress(
    progress_tx: &mpsc::Sender<TranslateProgress>,
    translated_count: usize,
    total_count: usize,
) {
    let percent = if total_count == 0 {
        100.0
    } else {
        translated_count as f32 / total_count as f32 * 100.0
    };
    let _ = progress_tx
        .send(TranslateProgress {
            percent,
            translated_count,
            total_count,
        })
        .await;
}

pub async fn translate_openai_compatible(
    client: &Client,
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
    request: &TranslateRequest,
    progress_tx: mpsc::Sender<TranslateProgress>,
) -> Result<TranslateResult, TranslateError> {
    let prompt = build_translation_prompt(request);
    let request_body = ChatCompletionRequest {
        model: model.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: "Translate subtitle lines faithfully and concisely.".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: prompt,
            },
        ],
        temperature: 0.2,
    };

    let endpoint = format!("{}/chat/completions", normalize_base_url(base_url));
    let mut req = client
        .post(&endpoint)
        .header("Content-Type", "application/json")
        .json(&request_body);

    if let Some(key) = api_key.filter(|v| !v.trim().is_empty()) {
        req = req.header("Authorization", format!("Bearer {}", key));
    }

    let response = req
        .send()
        .await
        .map_err(|e| TranslateError::Network(e.to_string()))?;

    let status = response.status().as_u16();
    if !response.status().is_success() {
        let message = response.text().await.unwrap_or_default();
        if status == 401 || status == 403 {
            return Err(TranslateError::InvalidApiKey);
        }
        return Err(TranslateError::Api { status, message });
    }

    let body: ChatCompletionResponse = response
        .json()
        .await
        .map_err(|e| TranslateError::Network(format!("Failed to parse response: {}", e)))?;

    let content = body
        .choices
        .first()
        .map(|c| c.message.content.as_str())
        .unwrap_or("");
    let texts = parse_multiline_translation(content, request.texts.len());
    send_progress(&progress_tx, texts.len(), request.texts.len()).await;

    Ok(TranslateResult {
        texts,
        engine: model.to_string(),
    })
}
