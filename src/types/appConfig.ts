export interface GeneralConfig {
  ui_language: string;
  theme: string;
  output_dir: string | null;
  use_gpu: boolean;
}

export interface AsrSettings {
  model_size: string;
  default_language: string;
  n_threads: number;
}

export interface TranslationAppSettings {
  engine: string;
  default_target_language: string;
}

export interface SubtitleSettings {
  max_line_length?: number;
  max_lines_per_subtitle?: number;
}

export interface AppConfig {
  general: GeneralConfig;
  asr: AsrSettings;
  translation: TranslationAppSettings;
  subtitle: SubtitleSettings;
}
