// ============================================================
// translate/llm_api.rs — LLM API translation (OpenAI-compatible)
// ============================================================

use super::engine::*;
use crate::error::TranslateError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// LLM API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub api_key: String,
    pub api_base: String,
    pub model: String,
    /// Maximum segments per batch (to avoid token limits)
    pub batch_size: usize,
    /// Maximum retries per batch
    pub max_retries: u32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_base: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o-mini".to_string(),
            batch_size: 20,
            max_retries: 3,
        }
    }
}

/// LLM-based translation engine
pub struct LlmTranslateEngine {
    client: reqwest::Client,
    config: LlmConfig,
}

impl LlmTranslateEngine {
    pub fn new(config: LlmConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            config,
        }
    }

    /// Build translation prompt
    fn build_prompt(
        &self,
        texts: &[String],
        source_lang: &str,
        target_lang: &str,
        context_hint: Option<&str>,
    ) -> String {
        let context = context_hint
            .map(|c| format!("\nContext: {}\n", c))
            .unwrap_or_default();

        let segments: String = texts
            .iter()
            .enumerate()
            .map(|(i, t)| format!("[{}] {}", i + 1, t))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"You are a professional subtitle translator specializing in {source_lang} to {target_lang} translation.
{context}
Rules:
1. Maintain the original meaning, tone, and register
2. Keep translations concise and natural for subtitle display
3. Preserve proper nouns, brand names, and technical terms appropriately
4. Return ONLY the translated text, one line per segment, in the SAME order
5. Do NOT include segment numbers, brackets, or any extra formatting
6. Do NOT add explanations or notes

Translate the following {count} subtitle segments:

{segments}"#,
            count = texts.len()
        )
    }

    /// Call LLM API for a single batch
    async fn translate_batch(
        &self,
        texts: &[String],
        source_lang: &str,
        target_lang: &str,
        context_hint: Option<&str>,
    ) -> Result<Vec<String>, TranslateError> {
        let prompt = self.build_prompt(texts, source_lang, target_lang, context_hint);

        #[derive(Serialize)]
        struct ChatRequest {
            model: String,
            messages: Vec<Message>,
            temperature: f32,
        }

        #[derive(Serialize)]
        struct Message {
            role: String,
            content: String,
        }

        #[derive(Deserialize)]
        struct ChatResponse {
            choices: Vec<Choice>,
        }

        #[derive(Deserialize)]
        struct Choice {
            message: ResponseMessage,
        }

        #[derive(Deserialize)]
        struct ResponseMessage {
            content: String,
        }

        let request_body = ChatRequest {
            model: self.config.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt,
            }],
            temperature: 0.3,
        };

        let mut retries = 0;
        loop {
            let response = self
                .client
                .post(format!("{}/chat/completions", self.config.api_base))
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await
                .map_err(|e| TranslateError::Network(e.to_string()))?;

            let status = response.status().as_u16();

            if status == 429 {
                retries += 1;
                if retries > self.config.max_retries {
                    return Err(TranslateError::RateLimited {
                        retry_after_secs: 60,
                    });
                }
                tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(retries))).await;
                continue;
            }

            if !response.status().is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(TranslateError::Api {
                    status,
                    message: body,
                });
            }

            let chat_response: ChatResponse = response
                .json()
                .await
                .map_err(|e| TranslateError::Network(e.to_string()))?;

            let content = chat_response
                .choices
                .first()
                .map(|c| c.message.content.clone())
                .unwrap_or_default();

            // Parse response: one translation per line
            let translations: Vec<String> = content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| line.trim().to_string())
                .collect();

            // Verify count matches
            if translations.len() != texts.len() {
                tracing::warn!(
                    "Translation count mismatch: expected {}, got {}. Padding/truncating.",
                    texts.len(),
                    translations.len()
                );
                let mut result = translations;
                result.resize(texts.len(), "[Translation Error]".to_string());
                return Ok(result);
            }

            return Ok(translations);
        }
    }
}

#[async_trait]
impl TranslateEngine for LlmTranslateEngine {
    async fn translate(
        &self,
        request: &TranslateRequest,
        progress_tx: mpsc::Sender<TranslateProgress>,
    ) -> Result<TranslateResult, TranslateError> {
        let total = request.texts.len();
        let mut all_translations = Vec::with_capacity(total);

        // Process in batches
        for (batch_idx, chunk) in request.texts.chunks(self.config.batch_size).enumerate() {
            tracing::info!(
                "Translating batch {}/{} ({} segments)",
                batch_idx + 1,
                (total + self.config.batch_size - 1) / self.config.batch_size,
                chunk.len()
            );

            let batch_result = self
                .translate_batch(
                    chunk,
                    &request.source_lang,
                    &request.target_lang,
                    request.context_hint.as_deref(),
                )
                .await?;

            all_translations.extend(batch_result);

            let _ = progress_tx
                .send(TranslateProgress {
                    percent: (all_translations.len() as f32 / total as f32) * 100.0,
                    translated_count: all_translations.len(),
                    total_count: total,
                })
                .await;
        }

        Ok(TranslateResult {
            texts: all_translations,
            engine: self.name().to_string(),
        })
    }

    fn name(&self) -> &str {
        "LLM API"
    }

    fn requires_network(&self) -> bool {
        true
    }

    fn supported_pairs(&self) -> Vec<(String, String)> {
        vec![] // LLM supports any pair
    }
}
