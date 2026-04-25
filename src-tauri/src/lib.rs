pub mod audio;
pub mod asr;
pub mod commands;
pub mod config;
pub mod error;
pub mod log_layer;
pub mod pipeline;
pub mod subtitle;
pub mod translate;

use commands::AppState;
use log_layer::TauriLogLayer;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tracing_subscriber::{fmt, prelude::*, EnvFilter, Layer};

pub fn run() {
    let app_handle_holder: Arc<Mutex<Option<tauri::AppHandle>>> = Arc::new(Mutex::new(None));

    let log_layer = TauriLogLayer::new(app_handle_holder.clone());

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("subtitle_forge=info"));

    let tauri_layer = log_layer.with_filter(
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("subtitle_forge=info")),
    );
    let fmt_layer = fmt::layer().with_filter(env_filter);

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(tauri_layer)
        .init();

    tracing::info!("Logging initialized");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .setup(move |app| {
            {
                let handle = app.handle().clone();
                let mut guard = app_handle_holder.lock().unwrap();
                *guard = Some(handle);
            }

            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to resolve app data dir");

            let models_dir = app_data_dir.join("models");
            let config_path = app_data_dir.join("app-config.json");
            let (tmp_dir, cache_dir) = if cfg!(debug_assertions) {
                let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
                let debug_root = project_root.join(".cache");
                (debug_root.join("tmp"), debug_root.join("debug"))
            } else {
                (app_data_dir.join("tmp"), app_data_dir.join("cache"))
            };

            for dir in [&models_dir, &tmp_dir, &cache_dir] {
                std::fs::create_dir_all(dir).ok();
            }

            tracing::info!("App data dir: {:?}", app_data_dir);
            tracing::info!("Models dir: {:?}", models_dir);
            tracing::info!("Temp dir: {:?}", tmp_dir);
            tracing::info!("Cache dir: {:?}", cache_dir);
            tracing::info!("Config path: {:?}", config_path);

            app.manage(AppState::new(models_dir, tmp_dir, cache_dir, config_path));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_pipeline,
            commands::cancel_pipeline,
            commands::list_models,
            commands::download_model,
            commands::open_model_directory,
            commands::list_embedded_models,
            commands::download_embedded_model,
            commands::open_embedded_model_directory,
            commands::check_model_exists,
            commands::list_translate_modes,
            commands::list_translate_services,
            commands::get_translate_settings,
            commands::save_translate_settings,
            commands::debug_select_translate_service,
            commands::health_check_translate_service,
            commands::probe_video,
            commands::get_supported_languages,
            commands::get_app_config,
            commands::save_app_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SubtitleForge");
}
