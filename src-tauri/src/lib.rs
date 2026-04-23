// ============================================================
// SubtitleForge — lib.rs
// Module declarations and Tauri app builder
// ============================================================

pub mod audio;
pub mod asr;
pub mod commands;
pub mod config;
pub mod error;
pub mod pipeline;
pub mod subtitle;
pub mod translate;

use commands::AppState;
use tracing_subscriber::EnvFilter;

pub fn run() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("subtitle_forge=info")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to resolve app data dir");

            let models_dir = app_data_dir.join("models");
            let tmp_dir = app_data_dir.join("tmp");
            let cache_dir = app_data_dir.join("cache");

            // Ensure directories exist
            for dir in [&models_dir, &tmp_dir, &cache_dir] {
                std::fs::create_dir_all(dir).ok();
            }

            tracing::info!("App data dir: {:?}", app_data_dir);
            tracing::info!("Models dir: {:?}", models_dir);

            app.manage(AppState::new(models_dir, tmp_dir, cache_dir));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_pipeline,
            commands::cancel_pipeline,
            commands::list_models,
            commands::probe_video,
            commands::get_supported_languages,
            commands::get_app_config,
            commands::save_app_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SubtitleForge");
}
