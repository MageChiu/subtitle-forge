use super::engine::*;
use super::plugin::*;
use crate::error::TranslateError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

pub struct OllamaTranslatePlugin {
    metadata: PluginMetadata,
    client: reqwest::Client,
}

impl OllamaTranslatePlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                namespace: "ollama/v1".to_string(),
                display_name: "Ollama (Local LLM)".to_string(),
                description: "Privacy-first via local Ollama, no network required".to_string(),
                version: "1.0.0".to_string(),
                category: PluginCategory::LocalLlm,
                requires_network: false,
                config_schema: vec![
                    ConfigField {
                        key: "base_url".to_string(),
                        label: "Ollama URL".to_string(),
                        field_type: ConfigFieldType::Url,
                        default: "http://localhost:11434".to_string(),
                        required: true,
                        placeholder: None,
                        description: None,
                    },
                    ConfigField {
                        key: "model".to_string(),
                        label: "Model".to_string(),
                        field_type: ConfigFieldType::Text,
                        default: "qwen2.5:7b".to_string(),
                        required: true,
                        placeholder: Some("qwen2.5:7b".to_string()),
                        description: None,
                    },
                    ConfigField {
                        key: "batch_size".to_string(),
                        label: "Batch Size".to_string(),
                        field_type: ConfigFieldType::Number,
                        default: "20".to_string(),
                        required: false,
                        placeholder: None,
                        description: Some("Segments per generation call".to_string()),
                    },
                ],
            },
            client: reqwest::Client::new(),
        }
    }

    fn build_prompt(texts: &[String], source_lang: &str, target_lang: &str, context_hint: Option<&str>) -> String {
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
4. Return ONLY the translated text, one line per segment, in the SAME ORDER
5. Do NOT include segment numbers, brackets, or any extra formatting
6. Do NOT add explanations or notes

Translate the following {count} subtitle segments:

{segments}"#,
            count = texts.len()
        )
    }

    async fn translate_batch(
        &self,
        texts: &[String],
        source_lang: &str,
        target_lang: &str,
        context_hint: Option<&str>,
        config: &PluginConfig,
    ) -> Result<Vec<String>, TranslateError> {
        let base_url = config.get("base_url");
        let model = config.get("model");
        let prompt = Self::build_prompt(texts, source_lang, target_lang, context_hint);

        #[derive(Serialize)]
        struct OllamaRequest {
            model: String,
            prompt: String,
            stream: bool,
        }

        #[derive(Deserialize)]
        struct OllamaResponse {
            response: String,
        }

        let request_body = OllamaRequest {
            model: model.to_string(),
            prompt,
            stream: false,
        };

        let url = format!("{}/api/generate", base_url);

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| TranslateError::Network(format!("Ollama connection failed: {}. Is Ollama running?", e)))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(TranslateError::Api { status, message: body });
        }

        let ollama_resp: OllamaResponse = response
            .json()
            .await
            .map_err(|e| TranslateError::Network(e.to_string()))?;

        let translations: Vec<String> = ollama_resp
            .response
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .collect();

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

        Ok(translations)
    }
}

#[async_trait]
impl TranslationPlugin for OllamaTranslatePlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn translate(
        &self,
        request: &TranslateRequest,
        config: &PluginConfig,
        progress_tx: mpsc::Sender<TranslateProgress>,
    ) -> Result<TranslateResult, TranslateError> {
        let batch_size: usize = config.get("batch_size").parse().unwrap_or(20);
        let total = request.texts.len();
        let mut all_translations = Vec::with_capacity(total);

        for (batch_idx, chunk) in request.texts.chunks(batch_size).enumerate() {
            tracing::info!(
                "Ollama translating batch {}/{} ({} segments)",
                batch_idx + 1,
                (total + batch_size - 1) / batch_size,
                chunk.len()
            );

            let batch_result = self
                .translate_batch(
                    chunk,
                    &request.source_lang,
                    &request.target_lang,
                    request.context_hint.as_deref(),
                    config,
                )
                .await?;

            all_translations.extend(batch_result);

            let _ = progress_tx.send(TranslateProgress {
                percent: (all_translations.len() as f32 / total as f32) * 100.0,
                translated_count: all_translations.len(),
                total_count: total,
            }).await;
        }

        Ok(TranslateResult {
            texts: all_translations,
            engine: self.metadata.namespace.clone(),
        })
    }

    async fn health_check(&self, config: &PluginConfig) -> HealthStatus {
        let base_url = config.get("base_url");
        match self
            .client
            .get(format!("{}/api/tags", base_url))
            .timeout(std::time::Duration::from_secs(3))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => HealthStatus::Healthy,
            Ok(resp) => HealthStatus::Degraded(format!("HTTP {}", resp.status())),
            Err(e) => HealthStatus::Unhealthy(format!("Cannot connect: {}. Is Ollama running?", e)),
        }
    }
}
