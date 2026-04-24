use async_trait::async_trait;
use reqwest::Client;
use tokio::sync::mpsc;

use crate::error::TranslateError;
use crate::translate::core::{
    ConfigField, ConfigFieldType, HealthStatus, ServiceConfig, ServiceDescriptor, TranslateMode,
    TranslationService, ValidationIssue,
};
use crate::translate::engine::{TranslateProgress, TranslateRequest, TranslateResult};
use crate::translate::shared::{normalize_base_url, translate_openai_compatible, validate_http_url};

pub struct OllamaService {
    descriptor: ServiceDescriptor,
    client: Client,
}

impl OllamaService {
    pub fn new() -> Self {
        Self {
            descriptor: ServiceDescriptor {
                key: "ollama".to_string(),
                name: "Ollama".to_string(),
                description: "通过本地 Ollama 服务执行字幕翻译。".to_string(),
                mode: TranslateMode::LocalLlm,
                requires_network: false,
                config_schema: vec![
                    ConfigField {
                        key: "base_url".to_string(),
                        label: "基础地址".to_string(),
                        field_type: ConfigFieldType::Url,
                        default: "http://127.0.0.1:11434/v1".to_string(),
                        required: true,
                        placeholder: None,
                        description: Some("本地 Ollama 的 OpenAI 兼容入口。".to_string()),
                    },
                    ConfigField {
                        key: "model".to_string(),
                        label: "翻译模型".to_string(),
                        field_type: ConfigFieldType::Text,
                        default: "qwen2.5:7b".to_string(),
                        required: true,
                        placeholder: Some("qwen2.5:7b".to_string()),
                        description: None,
                    },
                    ConfigField {
                        key: "batch_size".to_string(),
                        label: "批大小".to_string(),
                        field_type: ConfigFieldType::Number,
                        default: "20".to_string(),
                        required: false,
                        placeholder: None,
                        description: Some("每次请求处理的字幕段数。".to_string()),
                    },
                ],
            },
            client: Client::new(),
        }
    }
}

#[async_trait]
impl TranslationService for OllamaService {
    fn descriptor(&self) -> &ServiceDescriptor {
        &self.descriptor
    }

    fn validate_config(&self, config: &ServiceConfig) -> Result<(), Vec<ValidationIssue>> {
        let mut issues = Vec::new();
        if let Err(message) = validate_http_url("基础地址", config.get("base_url")) {
            issues.push(ValidationIssue {
                field: "base_url".to_string(),
                message,
            });
        }
        if config.get("model").trim().is_empty() {
            issues.push(ValidationIssue {
                field: "model".to_string(),
                message: "翻译模型不能为空".to_string(),
            });
        }
        if issues.is_empty() {
            Ok(())
        } else {
            Err(issues)
        }
    }

    async fn initialize(&self, config: &ServiceConfig) -> Result<(), TranslateError> {
        tracing::info!("Ollama 初始化: 校验配置");
        self.validate_config(config)
            .map_err(|issues| TranslateError::Config(issues[0].message.clone()))?;
        Ok(())
    }

    async fn health_check(&self, config: &ServiceConfig) -> HealthStatus {
        let base_url = normalize_base_url(config.get("base_url"));
        let health_url = if base_url.ends_with("/v1") {
            format!("{}{}", base_url.trim_end_matches("/v1"), "/api/tags")
        } else {
            format!("{}/api/tags", base_url)
        };
        match self.client.get(&health_url).send().await {
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
        translate_openai_compatible(
            &self.client,
            &normalize_base_url(config.get("base_url")),
            None,
            config.get("model"),
            request,
            progress_tx,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_required_fields() {
        let service = OllamaService::new();
        let config = ServiceConfig::new("ollama");
        let issues = service.validate_config(&config).unwrap_err();
        assert!(issues.iter().any(|issue| issue.field == "base_url"));
        assert!(issues.iter().any(|issue| issue.field == "model"));
    }
}
