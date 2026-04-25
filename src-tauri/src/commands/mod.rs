use crate::asr::engine::LanguageInfo;
use crate::asr::models::{DownloadProgress, ModelInfo, ModelManager};
use crate::asr::whisper::WhisperEngine;
use crate::audio::extractor::MediaInfo;
use crate::config::settings::AppConfig;
use crate::pipeline::orchestrator::{PipelineConfig, PipelineOrchestrator, PipelineStage};
use crate::translate::{
    build_factory, default_settings, HealthStatus, ServiceInfo, SharedFactory, TranslateEngine,
    TranslateMode, TranslateModeInfo, TranslateProgress, TranslateRequest, TranslateResult,
    TranslationSettings,
};
use crate::translate::services::embedded_llm::models::{
    EmbeddedDownloadProgress, EmbeddedModelInfo, EmbeddedModelManager,
};
use chrono::Utc;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{mpsc, watch, Mutex};

pub struct AppState {
    pub models_dir: PathBuf,
    pub tmp_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub config_path: PathBuf,
    pub cancel_tx: Arc<Mutex<Option<watch::Sender<bool>>>>,
    pub is_running: Arc<std::sync::atomic::AtomicBool>,
    pub app_config: Arc<Mutex<AppConfig>>,
    pub translation_settings: Arc<Mutex<TranslationSettings>>,
    pub translate_factory: SharedFactory,
}

impl AppState {
    pub fn new(models_dir: PathBuf, tmp_dir: PathBuf, cache_dir: PathBuf, config_path: PathBuf) -> Self {
        let factory_impl = build_factory(models_dir.clone());
        let settings = default_settings(&factory_impl);
        let factory = Arc::new(tokio::sync::RwLock::new(factory_impl));
        let app_config = AppConfig::load_or_default(&config_path);

        Self {
            models_dir,
            tmp_dir,
            cache_dir,
            config_path,
            cancel_tx: Arc::new(Mutex::new(None)),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_config: Arc::new(Mutex::new(app_config)),
            translation_settings: Arc::new(Mutex::new(settings)),
            translate_factory: factory,
        }
    }
}

struct ServiceTranslateAdapter {
    factory: SharedFactory,
    settings: TranslationSettings,
}

#[async_trait::async_trait]
impl TranslateEngine for ServiceTranslateAdapter {
    async fn translate(
        &self,
        request: &TranslateRequest,
        progress_tx: mpsc::Sender<TranslateProgress>,
    ) -> Result<TranslateResult, crate::error::TranslateError> {
        let factory = self.factory.read().await;
        factory
            .translate_with_fallback(&self.settings, request, progress_tx)
            .await
    }

    fn name(&self) -> &str {
        &self.settings.active_service
    }

    fn requires_network(&self) -> bool {
        matches!(self.settings.active_mode, TranslateMode::OnlineLlm)
    }

    fn supported_pairs(&self) -> Vec<(String, String)> {
        vec![]
    }
}

#[tauri::command]
pub async fn start_pipeline(
    app: AppHandle,
    state: State<'_, AppState>,
    mut config: PipelineConfig,
) -> Result<String, String> {
    if state
        .is_running
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        return Err("A pipeline is already running".into());
    }

    let manager = ModelManager::new(state.models_dir.clone());
    let model_path = manager
        .check_model(&config.asr_model)
        .map_err(|e| e.to_string())?;

    state
        .is_running
        .store(true, std::sync::atomic::Ordering::Relaxed);

    let (cancel_tx, cancel_rx) = watch::channel(false);
    {
        let mut tx = state.cancel_tx.lock().await;
        *tx = Some(cancel_tx);
    }

    let (stage_tx, mut stage_rx) = mpsc::channel::<PipelineStage>(64);
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(stage) = stage_rx.recv().await {
            let _ = app_clone.emit("pipeline-progress", &stage);
        }
    });

    let tmp_dir = state.tmp_dir.clone();
    let cache_dir = state.cache_dir.clone();
    let is_running = state.is_running.clone();
    let app_config = state.app_config.lock().await.clone();
    let translate_factory = state.translate_factory.clone();
    let mut translation_settings = state.translation_settings.lock().await.clone();
    translation_settings.active_service = config.translate_engine.clone();
    if let Some(service) = translate_factory.read().await.get(&translation_settings.active_service) {
        translation_settings.active_mode = service.descriptor().mode;
    }

    if config.n_threads.unwrap_or(0) == 0 {
        config.n_threads = Some(app_config.resolved_asr_threads());
    }

    tokio::spawn(async move {
        tracing::info!("Pipeline started for: {}", config.input_path);
        tracing::info!(
            "Source language: {:?}, Target language: {}",
            config.source_language,
            config.target_language
        );
        tracing::info!(
            "Translation selection: mode={}, service={}",
            translation_settings.active_mode.key(),
            translation_settings.active_service
        );
        tracing::info!(
            "ASR runtime config: gpu={}, threads={}",
            config.use_gpu,
            config.n_threads.unwrap_or(0)
        );

        let asr_engine = WhisperEngine::new(model_path, config.use_gpu);

        let translate_engine = Box::new(ServiceTranslateAdapter {
            factory: translate_factory,
            settings: translation_settings,
        });

        let orchestrator = PipelineOrchestrator::new(
            Box::new(asr_engine),
            translate_engine,
            tmp_dir,
            cache_dir,
        );

        let result = orchestrator.run(config, stage_tx, cancel_rx).await;
        if let Err(err) = result {
            tracing::error!("Pipeline failed: {}", err);
        }

        is_running.store(false, std::sync::atomic::Ordering::Relaxed);
    });

    Ok("Pipeline started".into())
}

#[tauri::command]
pub async fn cancel_pipeline(state: State<'_, AppState>) -> Result<(), String> {
    let tx = state.cancel_tx.lock().await;
    if let Some(ref cancel_tx) = *tx {
        cancel_tx.send(true).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn list_models(state: State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    let manager = ModelManager::new(state.models_dir.clone());
    Ok(manager.list_models())
}

#[tauri::command]
pub async fn download_model(
    app: AppHandle,
    state: State<'_, AppState>,
    model_key: String,
) -> Result<String, String> {
    let manager = ModelManager::new(state.models_dir.clone());
    let (progress_tx, mut progress_rx) = mpsc::channel::<DownloadProgress>(32);
    let app_clone = app.clone();

    tokio::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            let _ = app_clone.emit("model-download-progress", &progress);
        }
    });

    manager.download_model(&model_key, progress_tx).await
}

#[tauri::command]
pub async fn open_model_directory(state: State<'_, AppState>) -> Result<(), String> {
    let models_dir = state.models_dir.clone();
    std::fs::create_dir_all(&models_dir).map_err(|e| format!("Failed to create dir: {}", e))?;

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&models_dir)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&models_dir)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&models_dir)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn list_embedded_models(state: State<'_, AppState>) -> Result<Vec<EmbeddedModelInfo>, String> {
    let manager = EmbeddedModelManager::new(state.models_dir.clone());
    Ok(manager.list_models())
}

#[tauri::command]
pub async fn download_embedded_model(
    app: AppHandle,
    state: State<'_, AppState>,
    model_key: String,
) -> Result<String, String> {
    let manager = EmbeddedModelManager::new(state.models_dir.clone());
    let (progress_tx, mut progress_rx) = mpsc::channel::<EmbeddedDownloadProgress>(32);
    let app_clone = app.clone();

    tokio::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            let _ = app_clone.emit("embedded-model-download-progress", &progress);
        }
    });

    manager.download_model(&model_key, progress_tx).await
}

#[tauri::command]
pub async fn open_embedded_model_directory(state: State<'_, AppState>) -> Result<(), String> {
    let models_dir = state.models_dir.join("embedded_llm");
    std::fs::create_dir_all(&models_dir).map_err(|e| format!("Failed to create dir: {}", e))?;

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&models_dir)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&models_dir)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&models_dir)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn check_model_exists(
    state: State<'_, AppState>,
    model_key: String,
) -> Result<bool, String> {
    let manager = ModelManager::new(state.models_dir.clone());
    Ok(manager.check_model(&model_key).is_ok())
}

#[tauri::command]
pub async fn list_translate_modes() -> Result<Vec<TranslateModeInfo>, String> {
    Ok(TranslateMode::all()
        .into_iter()
        .map(|mode| TranslateModeInfo {
            key: mode.key().to_string(),
            name: mode.label_zh().to_string(),
            description: mode.description_zh().to_string(),
        })
        .collect())
}

#[tauri::command]
pub async fn list_translate_services(
    state: State<'_, AppState>,
    mode_key: Option<String>,
) -> Result<Vec<ServiceInfo>, String> {
    let factory = state.translate_factory.read().await;
    if let Some(mode_key) = mode_key {
        let mode = TranslateMode::from_key(&mode_key)
            .ok_or_else(|| format!("Unsupported mode: {}", mode_key))?;
        Ok(factory.services_by_mode(mode))
    } else {
        let mut all = Vec::new();
        for mode in TranslateMode::all() {
            all.extend(factory.services_by_mode(mode));
        }
        Ok(all)
    }
}

#[tauri::command]
pub async fn get_translate_settings(
    state: State<'_, AppState>,
) -> Result<TranslationSettings, String> {
    let mut settings = state.translation_settings.lock().await.clone();
    let factory = state.translate_factory.read().await;
    factory.ensure_default_settings(&mut settings);
    Ok(settings)
}

#[tauri::command]
pub async fn save_translate_settings(
    state: State<'_, AppState>,
    settings: TranslationSettings,
) -> Result<(), String> {
    let timestamp = Utc::now().to_rfc3339();
    tracing::info!(
        "翻译服务选择事件: time={}, mode={}, service={}",
        timestamp,
        settings.active_mode.key(),
        settings.active_service
    );
    let mut guard = state.translation_settings.lock().await;
    *guard = settings;
    Ok(())
}

#[tauri::command]
pub async fn debug_select_translate_service(
    state: State<'_, AppState>,
    mode_key: String,
    service_key: String,
    settings: TranslationSettings,
) -> Result<String, String> {
    let timestamp = Utc::now().to_rfc3339();
    tracing::info!(
        "翻译服务选择事件: time={}, mode={}, service={}",
        timestamp,
        mode_key,
        service_key
    );

    let factory = state.translate_factory.read().await;
    let service = factory
        .get(&service_key)
        .ok_or_else(|| format!("Service not found: {}", service_key))?;
    tracing::info!("翻译初始化流程: step=lookup, service_name={}", service.descriptor().name);

    let config = settings
        .service_configs
        .get(&service_key)
        .cloned()
        .unwrap_or_else(|| service.create_default_config());

    tracing::info!("翻译初始化流程: step=validate");
    if let Err(issues) = service.validate_config(&config) {
        let message = issues
            .iter()
            .map(|issue| format!("{}: {}", issue.field, issue.message))
            .collect::<Vec<_>>()
            .join("; ");
        tracing::error!("翻译初始化异常: step=validate, error={}", message);
        return Ok(format!("validate_failed: {}", message));
    }

    tracing::info!("翻译初始化流程: step=initialize");
    if let Err(err) = service.initialize(&config).await {
        tracing::error!("翻译初始化异常: step=initialize, error={}", err);
        return Ok(format!("initialize_failed: {}", err));
    }

    tracing::info!("翻译初始化流程: step=health_check");
    let health = service.health_check(&config).await;
    tracing::info!("翻译初始化流程: step=health_check_done, status={:?}", health);

    Ok("ok".to_string())
}

#[tauri::command]
pub async fn health_check_translate_service(
    state: State<'_, AppState>,
    service_key: String,
) -> Result<HealthStatus, String> {
    let settings = state.translation_settings.lock().await.clone();
    let factory = state.translate_factory.read().await;
    let service = factory
        .get(&service_key)
        .ok_or_else(|| format!("Service not found: {}", service_key))?;
    let config = settings
        .service_configs
        .get(&service_key)
        .cloned()
        .unwrap_or_else(|| service.create_default_config());
    Ok(service.health_check(&config).await)
}

#[tauri::command]
pub async fn probe_video(path: String) -> Result<MediaInfo, String> {
    let path = std::path::Path::new(&path);
    crate::audio::extractor::AudioExtractor::probe(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_supported_languages() -> Vec<LanguageInfo> {
    vec![
        LanguageInfo { code: "auto".into(), name: "Auto Detect".into() },
        LanguageInfo { code: "en".into(), name: "English".into() },
        LanguageInfo { code: "zh".into(), name: "中文".into() },
        LanguageInfo { code: "ja".into(), name: "日本語".into() },
        LanguageInfo { code: "ko".into(), name: "한국어".into() },
        LanguageInfo { code: "fr".into(), name: "Français".into() },
        LanguageInfo { code: "de".into(), name: "Deutsch".into() },
        LanguageInfo { code: "es".into(), name: "Español".into() },
        LanguageInfo { code: "ru".into(), name: "Русский".into() },
        LanguageInfo { code: "pt".into(), name: "Português".into() },
        LanguageInfo { code: "it".into(), name: "Italiano".into() },
        LanguageInfo { code: "ar".into(), name: "العربية".into() },
        LanguageInfo { code: "hi".into(), name: "हिन्दी".into() },
        LanguageInfo { code: "th".into(), name: "ไทย".into() },
        LanguageInfo { code: "vi".into(), name: "Tiếng Việt".into() },
    ]
}

#[tauri::command]
pub async fn get_app_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    Ok(state.app_config.lock().await.clone())
}

#[tauri::command]
pub async fn save_app_config(
    state: State<'_, AppState>,
    config: AppConfig,
) -> Result<(), String> {
    config.save_to_path(&state.config_path)?;
    let mut guard = state.app_config.lock().await;
    *guard = config.clone();
    tracing::info!("Saving config: {:?}", config);
    Ok(())
}
