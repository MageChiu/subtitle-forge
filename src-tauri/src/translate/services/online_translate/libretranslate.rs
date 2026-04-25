use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::error::TranslateError;
use crate::translate::core::{
    ConfigField, ConfigFieldType, HealthStatus, ServiceConfig, ServiceDescriptor, TranslateMode,
    TranslationService, ValidationIssue,
};
use crate::translate::engine::{TranslateProgress, TranslateRequest, TranslateResult};
use crate::translate::shared::{send_progress, validate_http_url};

pub struct LibreTranslateService {
    descriptor: ServiceDescriptor,
    client: Client,
}

impl LibreTranslateService {
    pub fn new() -> Self {
        Self {
            descriptor: ServiceDescriptor {
                key: "libretranslate".to_string(),
                name: "LibreTranslate".to_string(),
                description: "使用 LibreTranslate 公共实例或自部署实例执行翻译。".to_string(),
                mode: TranslateMode::OnlineTranslate,
                requires_network: true,
                config_schema: vec![
                    ConfigField {
                        key: "base_url".to_string(),
                        label: "基础地址".to_string(),
                        field_type: ConfigFieldType::Url,
                        default: "https://libretranslate.de".to_string(),
                        required: true,
                        placeholder: Some("https://libretranslate.de".to_string()),
                        description: Some("支持自部署实例地址。".to_string()),
                    },
                    ConfigField {
                        key: "api_key".to_string(),
                        label: "API Key".to_string(),
                        field_type: ConfigFieldType::Password,
                        default: String::new(),
                        required: false,
                        placeholder: Some("可选".to_string()),
                        description: Some("部分实例需要 API Key。".to_string()),
                    },
                ],
            },
            client: Client::new(),
        }
    }
}

#[derive(Serialize)]
struct LibreRequest {
    q: String,
    source: String,
    target: String,
    format: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    api_key: String,
}

#[derive(Deserialize)]
struct LibreResponse {
    translated_text: String,
}

#[async_trait]
impl TranslationService for LibreTranslateService {
    fn descriptor(&self) -> &ServiceDescriptor {
        &self.descriptor
    }

    fn validate_config(&self, config: &ServiceConfig) -> Result<(), Vec<ValidationIssue>> {
        match validate_http_url("基础地址", config.get("base_url")) {
            Ok(_) => Ok(()),
            Err(message) => Err(vec![ValidationIssue {
                field: "base_url".to_string(),
                message,
            }]),
        }
    }

    async fn initialize(&self, config: &ServiceConfig) -> Result<(), TranslateError> {
        self.validate_config(config)
            .map_err(|issues| TranslateError::Config(issues[0].message.clone()))?;
        Ok(())
    }

    async fn health_check(&self, config: &ServiceConfig) -> HealthStatus {
        let url = format!("{}/languages", config.get("base_url").trim_end_matches('/'));
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => HealthStatus::Healthy,
            Ok(resp) => HealthStatus::Degraded(format!("HTTP {}", resp.status())),
            Err(err) => HealthStatus::Unhealthy(err.to_string()),
        }
    }

    async fn translate(
        &self,
        request: &TranslateRequest,
        config: &ServiceConfig,
        progress_tx: mpsc::Sender<TranslateProgress>,
    ) -> Result<TranslateResult, TranslateError> {
        let url = format!("{}/translate", config.get("base_url").trim_end_matches('/'));
        let total = request.texts.len();
        let mut results = Vec::with_capacity(total);

        for (idx, text) in request.texts.iter().enumerate() {
            if text.trim().is_empty() {
                results.push(String::new());
                continue;
            }

            let body = LibreRequest {
                q: text.clone(),
                source: request.source_lang.clone(),
                target: request.target_lang.clone(),
                format: "text".to_string(),
                api_key: config.get("api_key").to_string(),
            };

            let response = self
                .client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| TranslateError::Network(e.to_string()))?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let message = response.text().await.unwrap_or_default();
                if status == 401 || status == 403 {
                    return Err(TranslateError::InvalidApiKey);
                }
                return Err(TranslateError::Api { status, message });
            }

            let payload: LibreResponse = response
                .json()
                .await
                .map_err(|e| TranslateError::Network(format!("Failed to parse response: {}", e)))?;
            results.push(payload.translated_text);
            send_progress(&progress_tx, idx + 1, total).await;
        }

        Ok(TranslateResult {
            texts: results,
            engine: self.descriptor.key.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_base_url() {
        let service = LibreTranslateService::new();
        let mut config = ServiceConfig::new("libretranslate");
        config.set("base_url", "invalid-url");
        assert!(service.validate_config(&config).is_err());
    }
}
