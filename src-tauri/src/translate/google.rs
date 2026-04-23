use super::engine::*;
use super::plugin::*;
use crate::error::TranslateError;
use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::mpsc;

pub struct GoogleTranslatePlugin {
    metadata: PluginMetadata,
    client: reqwest::Client,
}

impl GoogleTranslatePlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                namespace: "google/v1".to_string(),
                display_name: "Google Translate".to_string(),
                description: "Free Google Translate API — no API key required".to_string(),
                version: "1.0.0".to_string(),
                category: PluginCategory::RemoteApi,
                requires_network: true,
                config_schema: vec![],
            },
            client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
                .build()
                .unwrap_or_default(),
        }
    }

    fn to_google_lang(lang: &str) -> &str {
        match lang {
            "zh" => "zh-CN",
            "pt" => "pt-BR",
            other => other,
        }
    }
}

#[derive(Deserialize)]
struct GoogleFreeResponse(Vec<serde_json::Value>);

#[async_trait]
impl TranslationPlugin for GoogleTranslatePlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn translate(
        &self,
        request: &TranslateRequest,
        _config: &PluginConfig,
        progress_tx: mpsc::Sender<TranslateProgress>,
    ) -> Result<TranslateResult, TranslateError> {
        let total = request.texts.len();
        let mut all_translations = Vec::with_capacity(total);
        let sl = Self::to_google_lang(&request.source_lang);
        let tl = Self::to_google_lang(&request.target_lang);

        for (idx, text) in request.texts.iter().enumerate() {
            if text.trim().is_empty() {
                all_translations.push(String::new());
                continue;
            }

            let url = "https://translate.googleapis.com/translate_a/single";

            let response = self
                .client
                .get(url)
                .query(&[
                    ("client", "gtx"),
                    ("sl", sl),
                    ("tl", tl),
                    ("dt", "t"),
                    ("ie", "UTF-8"),
                    ("oe", "UTF-8"),
                    ("q", text),
                ])
                .send()
                .await
                .map_err(|e| TranslateError::Network(e.to_string()))?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                return Err(TranslateError::Api { status, message: body });
            }

            let body: GoogleFreeResponse = response
                .json()
                .await
                .map_err(|e| TranslateError::Network(format!("Failed to parse response: {}", e)))?;

            let translated = body.0.first()
                .and_then(|arr| arr.as_array())
                .map(|sentences| {
                    sentences.iter()
                        .filter_map(|s| s.as_array())
                        .filter_map(|s| s.first())
                        .filter_map(|s| s.as_str())
                        .collect::<String>()
                })
                .unwrap_or_default();

            all_translations.push(if translated.is_empty() {
                text.clone()
            } else {
                translated
            });

            if idx % 5 == 0 || idx == total - 1 {
                let _ = progress_tx.send(TranslateProgress {
                    percent: ((idx + 1) as f32 / total as f32) * 100.0,
                    translated_count: idx + 1,
                    total_count: total,
                }).await;
            }

            if idx % 10 == 9 {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }

        Ok(TranslateResult {
            texts: all_translations,
            engine: self.metadata.namespace.clone(),
        })
    }

    async fn health_check(&self, _config: &PluginConfig) -> HealthStatus {
        let url = "https://translate.googleapis.com/translate_a/single";
        match self
            .client
            .get(url)
            .query(&[("client", "gtx"), ("sl", "en"), ("tl", "zh"), ("dt", "t"), ("q", "hello")])
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => HealthStatus::Healthy,
            Ok(resp) => HealthStatus::Degraded(format!("HTTP {}", resp.status())),
            Err(e) => HealthStatus::Unhealthy(e.to_string()),
        }
    }
}
