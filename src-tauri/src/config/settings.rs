// ============================================================
// config/settings.rs — Application settings
// ============================================================

use serde::{Deserialize, Serialize};

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// General settings
    pub general: GeneralConfig,
    /// ASR settings
    pub asr: AsrSettings,
    /// Translation settings
    pub translation: TranslationSettings,
    /// Subtitle output settings
    pub subtitle: SubtitleSettings,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            asr: AsrSettings::default(),
            translation: TranslationSettings::default(),
            subtitle: SubtitleSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// UI language
    pub ui_language: String,
    /// Theme: "system", "light", "dark"
    pub theme: String,
    /// Default output directory
    pub output_dir: Option<String>,
    /// Enable GPU acceleration
    pub use_gpu: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            ui_language: "en".to_string(),
            theme: "system".to_string(),
            output_dir: None,
            use_gpu: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrSettings {
    /// Selected Whisper model size
    pub model_size: String,
    /// Source language preference
    pub default_language: String,
    /// Number of threads (0 = auto)
    pub n_threads: u32,
}

impl Default for AsrSettings {
    fn default() -> Self {
        Self {
            model_size: "base".to_string(),
            default_language: "auto".to_string(),
            n_threads: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationSettings {
    /// Selected engine: "llm", "deepl", "offline"
    pub engine: String,
    /// Default target language
    pub default_target_language: String,
    /// LLM API settings
    pub llm: LlmSettings,
    /// DeepL API settings
    pub deepl: DeepLSettings,
}

impl Default for TranslationSettings {
    fn default() -> Self {
        Self {
            engine: "llm".to_string(),
            default_target_language: "zh".to_string(),
            llm: LlmSettings::default(),
            deepl: DeepLSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSettings {
    pub api_key: String,
    pub api_base: String,
    pub model: String,
    pub batch_size: usize,
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_base: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o-mini".to_string(),
            batch_size: 20,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepLSettings {
    pub api_key: String,
}

impl Default for DeepLSettings {
    fn default() -> Self {
        Self {
            api_key: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleSettings {
    /// Default output format: "srt", "ass", "vtt"
    pub default_format: String,
    /// ASS style settings
    pub ass_style: AssStyleSettings,
    /// Add BOM for better compatibility
    pub add_bom: bool,
}

impl Default for SubtitleSettings {
    fn default() -> Self {
        Self {
            default_format: "srt".to_string(),
            ass_style: AssStyleSettings::default(),
            add_bom: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssStyleSettings {
    pub primary_font: String,
    pub primary_size: u32,
    pub secondary_font: String,
    pub secondary_size: u32,
}

impl Default for AssStyleSettings {
    fn default() -> Self {
        Self {
            primary_font: "Arial".to_string(),
            primary_size: 48,
            secondary_font: "Arial".to_string(),
            secondary_size: 36,
        }
    }
}
