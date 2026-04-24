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

pub struct DeepSeekService {
    descriptor: ServiceDescriptor,
    client: Client,
}

impl DeepSeekService {
    pub fn new() -> Self {
        Self {
            descriptor: ServiceDescriptor {
                key: "deepseek".to_string(),
                name: "DeepSeek".to_string(),
                description: "使用 DeepSeek 云端模型执行字幕翻译。".to_string(),
                mode: TranslateMode::OnlineLlm,
                requires_network: true,
                config_schema: vec![
                    ConfigField {
                        key: "api_key".to_string(),
                        label: "API Key".to_string(),
                        field_type: ConfigFieldType::Password,
                        default: String::new(),
                        required: true,
                        placeholder: Some("sk-...".to_string()),
                        description: Some("DeepSeek 控制台生成的 API Key。".to_string()),
                    },
                    ConfigField {
                        key: "base_url".to_string(),
                        label: "基础地址".to_string(),
                        field_type: ConfigFieldType::Url,
                        default: "https://api.deepseek.com".to_string(),
                        required: true,
                        placeholder: None,
                        description: Some("默认使用官方 OpenAI 兼容接口地址。".to_string()),
                    },
                    ConfigField {
                        key: "model".to_string(),
                        label: "翻译模型".to_string(),
                        field_type: ConfigFieldType::Text,
                        default: "deepseek-chat".to_string(),
                        required: true,
                        placeholder: Some("deepseek-chat".to_string()),
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
impl TranslationService for DeepSeekService {
    fn descriptor(&self) -> &ServiceDescriptor {
        &self.descriptor
    }

    fn validate_config(&self, config: &ServiceConfig) -> Result<(), Vec<ValidationIssue>> {
        let mut issues = Vec::new();
        if config.get("api_key").trim().is_empty() {
            issues.push(ValidationIssue {
                field: "api_key".to_string(),
                message: "API Key不能为空".to_string(),
            });
        }
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
        tracing::info!("DeepSeek 初始化: 校验配置");
        self.validate_config(config)
            .map_err(|issues| TranslateError::Config(issues[0].message.clone()))?;
        Ok(())
    }

    async fn health_check(&self, config: &ServiceConfig) -> HealthStatus {
        let endpoint = format!("{}/models", normalize_base_url(config.get("base_url")));
        match self.client.get(&endpoint).send().await {
            Ok(resp) if resp.status().is_success() => HealthStatus::Healthy,
            Ok(resp) if resp.status().as_u16() == 401 => {
                HealthStatus::Unhealthy("API Key 无效".to_string())
            }
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
            Some(config.get("api_key")),
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
        let service = DeepSeekService::new();
        let config = ServiceConfig::new("deepseek");
        let issues = service.validate_config(&config).unwrap_err();
        assert!(issues.iter().any(|issue| issue.field == "api_key"));
        assert!(issues.iter().any(|issue| issue.field == "base_url"));
        assert!(issues.iter().any(|issue| issue.field == "model"));
    }
}
