use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::{mpsc, RwLock};

use crate::error::TranslateError;

use super::core::{
    HealthStatus, ServiceDescriptor, ServiceInfo, TranslateMode, TranslateModeInfo,
    TranslationService, TranslationSettings,
};
use super::engine::{TranslateProgress, TranslateRequest, TranslateResult};
use super::services::embedded_llm::llama_cpp::LlamaCppService;
use super::services::local_llm::ollama::OllamaService;
use super::services::online_translate::deepl::DeepLService;
use super::services::online_translate::google::GoogleTranslateService;
use super::services::online_translate::libretranslate::LibreTranslateService;
use super::services::online_llm::ark::ArkService;
use super::services::online_llm::deepseek::DeepSeekService;

pub type SharedFactory = Arc<RwLock<TranslationServiceFactory>>;

pub struct TranslationServiceFactory {
    services: HashMap<String, Box<dyn TranslationService>>,
}

impl TranslationServiceFactory {
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
        }
    }

    pub fn register<T: TranslationService + 'static>(&mut self, service: T) {
        let key = service.descriptor().key.clone();
        self.services.insert(key, Box::new(service));
    }

    pub fn get(&self, service_key: &str) -> Option<&dyn TranslationService> {
        self.services.get(service_key).map(|service| service.as_ref())
    }

    pub fn descriptors(&self) -> Vec<ServiceDescriptor> {
        self.services
            .values()
            .map(|service| service.descriptor().clone())
            .collect()
    }

    pub fn mode_infos(&self) -> Vec<TranslateModeInfo> {
        TranslateMode::all()
            .into_iter()
            .map(|mode| TranslateModeInfo {
                key: mode.key().to_string(),
                name: mode.label_zh().to_string(),
                description: mode.description_zh().to_string(),
            })
            .collect()
    }

    pub fn services_by_mode(&self, mode: TranslateMode) -> Vec<ServiceInfo> {
        self.services
            .values()
            .filter(|service| service.descriptor().mode == mode)
            .map(|service| ServiceInfo {
                descriptor: service.descriptor().clone(),
                health_status: Some(HealthStatus::Unknown),
            })
            .collect()
    }

    pub fn ensure_default_settings(&self, settings: &mut TranslationSettings) {
        for service in self.services.values() {
            let key = service.descriptor().key.clone();
            settings
                .service_configs
                .entry(key)
                .or_insert_with(|| service.create_default_config());
        }
    }

    fn fallback_candidates(&self, mode: TranslateMode, active_service: &str) -> Vec<String> {
        let mut result = vec![active_service.to_string()];
        for service in self.services.values() {
            if service.descriptor().mode == mode && service.descriptor().key != active_service {
                result.push(service.descriptor().key.clone());
            }
        }
        result
    }

    pub async fn translate_with_fallback(
        &self,
        settings: &TranslationSettings,
        request: &TranslateRequest,
        progress_tx: mpsc::Sender<TranslateProgress>,
    ) -> Result<TranslateResult, TranslateError> {
        let timestamp = Utc::now().to_rfc3339();
        tracing::info!(
            "翻译服务选择事件: time={}, mode={}, service={}",
            timestamp,
            settings.active_mode.key(),
            settings.active_service
        );

        let candidates = self.fallback_candidates(settings.active_mode, &settings.active_service);
        tracing::info!("翻译初始化流程: 候选服务={:?}", candidates);

        let mut last_error: Option<TranslateError> = None;

        for service_key in candidates {
            let Some(service) = self.get(&service_key) else {
                tracing::warn!("翻译初始化流程: 未找到服务 {}", service_key);
                continue;
            };

            let config = settings
                .service_configs
                .get(&service_key)
                .cloned()
                .unwrap_or_else(|| service.create_default_config());

            tracing::info!("翻译初始化流程: step=validate, service={}", service_key);
            if let Err(issues) = service.validate_config(&config) {
                let message = issues
                    .iter()
                    .map(|issue| format!("{}: {}", issue.field, issue.message))
                    .collect::<Vec<_>>()
                    .join("; ");
                tracing::error!(
                    "翻译初始化异常: step=validate, service={}, error={}",
                    service_key,
                    message
                );
                last_error = Some(TranslateError::Config(message));
                continue;
            }

            tracing::info!("翻译初始化流程: step=initialize, service={}", service_key);
            if let Err(err) = service.initialize(&config).await {
                tracing::error!(
                    "翻译初始化异常: step=initialize, service={}, error={}",
                    service_key,
                    err
                );
                last_error = Some(err);
                continue;
            }

            tracing::info!("翻译初始化流程: step=health_check, service={}", service_key);
            match service.health_check(&config).await {
                HealthStatus::Unhealthy(message) => {
                    tracing::error!(
                        "翻译初始化异常: step=health_check, service={}, error={}",
                        service_key,
                        message
                    );
                    last_error = Some(TranslateError::Initialization(message));
                    continue;
                }
                HealthStatus::Degraded(message) => {
                    tracing::warn!(
                        "翻译服务健康状态降级: service={}, message={}",
                        service_key,
                        message
                    );
                }
                HealthStatus::Healthy | HealthStatus::Unknown => {}
            }

            tracing::info!("翻译初始化流程: step=translate, service={}", service_key);
            match service.translate(request, &config, progress_tx.clone()).await {
                Ok(mut result) => {
                    result.engine = service_key.clone();
                    return Ok(result);
                }
                Err(err) => {
                    tracing::error!(
                        "翻译执行异常: service={}, error={}",
                        service_key,
                        err
                    );
                    last_error = Some(err);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| TranslateError::Initialization("没有可用的翻译服务".to_string())))
    }
}

pub fn build_factory(models_dir: PathBuf) -> TranslationServiceFactory {
    let mut factory = TranslationServiceFactory::new();
    factory.register(GoogleTranslateService::new());
    factory.register(LibreTranslateService::new());
    factory.register(DeepLService::new());
    factory.register(DeepSeekService::new());
    factory.register(ArkService::new());
    factory.register(OllamaService::new());
    factory.register(LlamaCppService::new(models_dir));
    factory
}

pub fn create_factory(models_dir: PathBuf) -> SharedFactory {
    Arc::new(RwLock::new(build_factory(models_dir)))
}

pub fn default_settings(factory: &TranslationServiceFactory) -> TranslationSettings {
    let mut settings = TranslationSettings::new(TranslateMode::OnlineTranslate, "google");
    factory.ensure_default_settings(&mut settings);
    settings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_services_by_mode() {
        let factory = build_factory(PathBuf::from("/tmp/subtitle-forge-test"));
        assert_eq!(factory.services_by_mode(TranslateMode::OnlineTranslate).len(), 3);
        assert_eq!(factory.services_by_mode(TranslateMode::OnlineLlm).len(), 2);
        assert_eq!(factory.services_by_mode(TranslateMode::LocalLlm).len(), 1);
        assert_eq!(factory.services_by_mode(TranslateMode::EmbeddedLlm).len(), 1);
    }

    #[test]
    fn creates_default_settings_for_all_services() {
        let factory = build_factory(PathBuf::from("/tmp/subtitle-forge-test"));
        let settings = default_settings(&factory);
        assert_eq!(settings.active_mode, TranslateMode::OnlineTranslate);
        assert_eq!(settings.active_service, "google");
        assert!(settings.service_configs.contains_key("google"));
        assert!(settings.service_configs.contains_key("libretranslate"));
        assert!(settings.service_configs.contains_key("deepl"));
        assert!(settings.service_configs.contains_key("deepseek"));
        assert!(settings.service_configs.contains_key("ark"));
        assert!(settings.service_configs.contains_key("ollama"));
        assert!(settings.service_configs.contains_key("llama_cpp"));
    }
}
