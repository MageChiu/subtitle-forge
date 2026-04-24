use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::error::TranslateError;
use crate::translate::core::{
    ConfigField, ConfigFieldType, HealthStatus, SelectOption, ServiceConfig, ServiceDescriptor,
    TranslateMode, TranslationService, ValidationIssue,
};
use crate::translate::engine::{TranslateProgress, TranslateRequest, TranslateResult};
use crate::translate::shared::{normalize_base_url, translate_openai_compatible};
use super::models::{EmbeddedModelManager, EmbeddedModelPreset};

const INTERNAL_HOST: &str = "127.0.0.1";
const INTERNAL_PORT: u16 = 18080;
const INTERNAL_BASE_URL: &str = "http://127.0.0.1:18080/v1";

fn launched_servers() -> &'static Mutex<HashSet<String>> {
    static LAUNCHED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    LAUNCHED.get_or_init(|| Mutex::new(HashSet::new()))
}

pub struct LlamaCppService {
    descriptor: ServiceDescriptor,
    client: Client,
    model_manager: EmbeddedModelManager,
}

impl LlamaCppService {
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            descriptor: ServiceDescriptor {
                key: "llama_cpp".to_string(),
                name: "llama.cpp GGUF".to_string(),
                description: "通过 llama.cpp 直接加载本地 GGUF 模型。".to_string(),
                mode: TranslateMode::EmbeddedLlm,
                requires_network: false,
                config_schema: vec![
                    ConfigField {
                        key: "model_key".to_string(),
                        label: "内嵌模型".to_string(),
                        field_type: ConfigFieldType::Select {
                            options: EmbeddedModelPreset::all()
                                .iter()
                                .map(|preset| SelectOption {
                                    value: preset.key().to_string(),
                                    label: preset.display_name().to_string(),
                                })
                                .collect(),
                        },
                        default: EmbeddedModelPreset::Qwen25_15B.key().to_string(),
                        required: true,
                        placeholder: None,
                        description: Some("选择已下载的 GGUF 模型，由应用内部自动加载。".to_string()),
                    },
                    ConfigField {
                        key: "ctx_size".to_string(),
                        label: "上下文长度".to_string(),
                        field_type: ConfigFieldType::Number,
                        default: "4096".to_string(),
                        required: false,
                        placeholder: None,
                        description: Some("启动 llama-server 时传入 --ctx-size。".to_string()),
                    },
                ],
            },
            client: Client::new(),
            model_manager: EmbeddedModelManager::new(models_dir),
        }
    }

    fn internal_base_url(&self) -> &'static str {
        INTERNAL_BASE_URL
    }

    async fn wait_until_ready(&self, base_url: &str) -> Result<(), TranslateError> {
        let health_url = if base_url.ends_with("/v1") {
            format!("{}{}", base_url.trim_end_matches("/v1"), "/health")
        } else {
            format!("{}/health", base_url)
        };

        for _ in 0..60 {
            match self.client.get(&health_url).send().await {
                Ok(resp) if resp.status().is_success() => return Ok(()),
                _ => tokio::time::sleep(Duration::from_millis(500)).await,
            }
        }

        Err(TranslateError::Initialization(
            "llama.cpp 服务启动超时".to_string(),
        ))
    }

    async fn ensure_server_started(&self, config: &ServiceConfig) -> Result<(), TranslateError> {
        let base_url = normalize_base_url(self.internal_base_url());
        let health = self.health_check(config).await;
        if matches!(health, HealthStatus::Healthy) {
            tracing::info!("llama.cpp 初始化: 已检测到运行中的服务");
            return Ok(());
        }

        let model_key = config.get("model_key");
        let model_preset = EmbeddedModelPreset::from_key(model_key)
            .ok_or_else(|| TranslateError::Config("未选择有效的内嵌模型".to_string()))?;
        let model_path = self
            .model_manager
            .check_model(model_key)
            .map_err(TranslateError::Initialization)?;
        let launch_key = format!("llama-server|{}", model_preset.key());
        let should_launch = {
            let mut launched = launched_servers().lock().unwrap();
            if launched.contains(&launch_key) {
                false
            } else {
                launched.insert(launch_key.clone());
                true
            }
        };

        if should_launch {
            tracing::info!(
                "llama.cpp 初始化: 启动应用内部托管服务 host={}, port={}",
                INTERNAL_HOST,
                INTERNAL_PORT
            );
            let ctx_size = config.get("ctx_size");

            let mut cmd = Command::new("llama-server");
            cmd.arg("-m")
                .arg(&model_path)
                .arg("--host")
                .arg(INTERNAL_HOST)
                .arg("--port")
                .arg(INTERNAL_PORT.to_string())
                .arg("--ctx-size")
                .arg(if ctx_size.is_empty() { "4096" } else { ctx_size })
                .stdout(Stdio::null())
                .stderr(Stdio::null());

            cmd.spawn()
                .map_err(|e| TranslateError::Initialization(format!("启动 llama-server 失败: {}", e)))?;
        } else {
            tracing::info!("llama.cpp 初始化: 服务已在启动流程中，等待就绪");
        }

        self.wait_until_ready(&base_url).await
    }
}

#[async_trait]
impl TranslationService for LlamaCppService {
    fn descriptor(&self) -> &ServiceDescriptor {
        &self.descriptor
    }

    fn validate_config(&self, config: &ServiceConfig) -> Result<(), Vec<ValidationIssue>> {
        let mut issues = Vec::new();
        let model_key = config.get("model_key");
        if model_key.trim().is_empty() {
            issues.push(ValidationIssue {
                field: "model_key".to_string(),
                message: "请选择一个内嵌模型".to_string(),
            });
        } else if EmbeddedModelPreset::from_key(model_key).is_none() {
            issues.push(ValidationIssue {
                field: "model_key".to_string(),
                message: "内嵌模型无效".to_string(),
            });
        }
        if issues.is_empty() {
            Ok(())
        } else {
            Err(issues)
        }
    }

    async fn initialize(&self, config: &ServiceConfig) -> Result<(), TranslateError> {
        tracing::info!("llama.cpp 初始化: 校验配置");
        self.validate_config(config)
            .map_err(|issues| TranslateError::Config(issues[0].message.clone()))?;
        tracing::info!("llama.cpp 初始化: 准备加载 GGUF 模型");
        self.ensure_server_started(config).await
    }

    async fn health_check(&self, _config: &ServiceConfig) -> HealthStatus {
        let base_url = normalize_base_url(self.internal_base_url());
        let health_url = if base_url.ends_with("/v1") {
            format!("{}{}", base_url.trim_end_matches("/v1"), "/health")
        } else {
            format!("{}/health", base_url)
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
        self.ensure_server_started(config).await?;
        let model_key = config.get("model_key");
        let model_preset = EmbeddedModelPreset::from_key(model_key)
            .ok_or_else(|| TranslateError::Config("未选择有效的内嵌模型".to_string()))?;
        translate_openai_compatible(
            &self.client,
            &normalize_base_url(self.internal_base_url()),
            None,
            model_preset.default_model_id(),
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
        let service = LlamaCppService::new(PathBuf::from("/tmp/subtitle-forge-test"));
        let config = ServiceConfig::new("llama_cpp");
        let issues = service.validate_config(&config).unwrap_err();
        assert!(issues.iter().any(|issue| issue.field == "model_key"));
    }
}
