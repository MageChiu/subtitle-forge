# SubtitleForge — 跨平台双语字幕生成器

> 基于 Tauri 2.0 + Rust + React/TypeScript 的桌面应用，实现视频音频提取、语音识别、翻译、双语字幕生成全链路。

---

## 一、项目概览

### 1.1 功能需求

| 功能 | 描述 | 输入 | 输出 |
|---|---|---|---|
| 音频提取 | 从视频文件中分离音轨 | MP4/MKV/AVI/MOV 等 | WAV/PCM (16kHz, mono) |
| 语音识别 | 音频转带时间戳文本 | WAV 音频 | `Vec<Segment>` (text + timestamps) |
| 翻译 | 源语言文本翻译为目标语言 | 源文本 + 目标语言 | 译文 |
| 字幕生成 | 合并双语文本，输出标准字幕文件 | 源文本 + 译文 + 时间戳 | SRT / ASS / VTT |
| GUI | 跨平台桌面界面 | 用户交互 | Windows/macOS/Linux |

### 1.2 技术栈总览

| 层级 | 技术 | 版本 |
|---|---|---|
| 应用框架 | Tauri | 2.x |
| 后端语言 | Rust | 1.75+ (edition 2021) |
| 前端框架 | React | 18.x |
| 前端语言 | TypeScript | 5.x |
| 构建工具 | Vite | 5.x |
| 音频提取 | FFmpeg (via `ffmpeg-next`) | 7.x |
| 语音识别 | whisper.cpp (via `whisper-rs`) | latest |
| 翻译(在线) | LLM API / DeepL API | - |
| 翻译(离线) | ONNX Runtime (via `ort`) + Opus-MT | - |
| 字幕处理 | 自实现 SRT/ASS/VTT 解析器 | - |
| 状态管理 | Zustand | 4.x |
| UI 组件 | Shadcn/ui + Tailwind CSS | - |
| 包管理 | pnpm (前端) + Cargo (后端) | - |

### 1.3 系统架构图

```
┌──────────────────────────────────────────────────────────────┐
│                      SubtitleForge App                       │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │              Frontend (React + TypeScript)              │  │
│  │                                                        │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │  │
│  │  │ 文件选择 │ │ 进度面板 │ │ 字幕预览 │ │ 设置页面 │  │  │
│  │  │ /拖拽区  │ │ /日志    │ │ /编辑器  │ │ /模型管理│  │  │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘  │  │
│  └───────────────────────┬────────────────────────────────┘  │
│                          │ Tauri IPC (invoke / event)        │
│  ┌───────────────────────▼────────────────────────────────┐  │
│  │                Backend (Rust Core)                      │  │
│  │                                                        │  │
│  │  ┌──────────────────────────────────────────────────┐  │  │
│  │  │              Pipeline Orchestrator               │  │  │
│  │  │  (tokio async runtime + channel-based progress)  │  │  │
│  │  └──────┬──────────┬──────────┬──────────┬─────────┘  │  │
│  │         │          │          │          │             │  │
│  │  ┌──────▼───┐ ┌────▼─────┐ ┌─▼────────┐ ┌▼─────────┐ │  │
│  │  │ Audio    │ │ ASR      │ │Translate │ │ Subtitle │ │  │
│  │  │ Extractor│ │ Engine   │ │ Engine   │ │ Writer   │ │  │
│  │  │          │ │          │ │          │ │          │ │  │
│  │  │ ffmpeg-  │ │whisper-rs│ │reqwest + │ │SRT/ASS/  │ │  │
│  │  │ next     │ │          │ │ort+opus  │ │VTT       │ │  │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘ │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │                   Platform Layer                       │  │
│  │  FFmpeg libs │ CUDA/Metal/Vulkan │ File System │ TLS   │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

---

## 二、项目目录结构

```
subtitle-forge/
├── src-tauri/                          # Rust 后端
│   ├── Cargo.toml                      # Rust 依赖管理
│   ├── tauri.conf.json                 # Tauri 应用配置
│   ├── build.rs                        # 构建脚本（FFmpeg linking）
│   ├── icons/                          # 应用图标
│   └── src/
│       ├── main.rs                     # 入口 + Tauri bootstrap
│       ├── lib.rs                      # 模块注册
│       ├── commands/                   # Tauri IPC 命令层
│       │   ├── mod.rs
│       │   ├── extract.rs              # 音频提取命令
│       │   ├── transcribe.rs           # 语音识别命令
│       │   ├── translate.rs            # 翻译命令
│       │   └── subtitle.rs            # 字幕生成命令
│       ├── pipeline/                   # 核心处理管线
│       │   ├── mod.rs
│       │   ├── orchestrator.rs         # 流水线编排器
│       │   └── progress.rs            # 进度追踪
│       ├── audio/                      # 音频提取模块
│       │   ├── mod.rs
│       │   └── extractor.rs
│       ├── asr/                        # 语音识别模块
│       │   ├── mod.rs
│       │   ├── engine.rs              # ASR 引擎 trait
│       │   ├── whisper.rs             # whisper.cpp 实现
│       │   └── models.rs             # 模型管理
│       ├── translate/                  # 翻译模块
│       │   ├── mod.rs
│       │   ├── engine.rs             # 翻译引擎 trait
│       │   ├── llm_api.rs            # LLM API 翻译
│       │   ├── deepl.rs              # DeepL API 翻译
│       │   └── offline.rs            # 离线翻译 (Opus-MT)
│       ├── subtitle/                   # 字幕处理模块
│       │   ├── mod.rs
│       │   ├── types.rs              # 字幕数据结构
│       │   ├── srt.rs                # SRT 格式读写
│       │   ├── ass.rs                # ASS 格式读写
│       │   ├── vtt.rs                # VTT 格式读写
│       │   └── merger.rs             # 双语合并
│       ├── config/                     # 配置管理
│       │   ├── mod.rs
│       │   └── settings.rs
│       └── error.rs                    # 统一错误处理
│
├── src/                                # React 前端
│   ├── main.tsx                        # React 入口
│   ├── App.tsx                         # 主路由
│   ├── components/                     # UI 组件
│   │   ├── FileDropZone.tsx           # 文件拖拽区
│   │   ├── ProgressPanel.tsx          # 进度面板
│   │   ├── SubtitlePreview.tsx        # 字幕预览
│   │   ├── SettingsDialog.tsx         # 设置对话框
│   │   └── LanguageSelector.tsx       # 语言选择器
│   ├── hooks/                          # 自定义 Hooks
│   │   ├── usePipeline.ts            # 管线调用 hook
│   │   └── useProgress.ts            # 进度监听 hook
│   ├── stores/                         # 状态管理
│   │   ├── appStore.ts
│   │   └── settingsStore.ts
│   ├── lib/                            # 工具函数
│   │   ├── tauri.ts                   # Tauri API 封装
│   │   └── formats.ts                # 格式化工具
│   └── styles/
│       └── globals.css                # Tailwind 入口
│
├── models/                             # 模型文件目录（gitignore）
│   ├── whisper/                        # Whisper GGML 模型
│   └── opus-mt/                        # Opus-MT ONNX 模型
│
├── package.json                        # 前端依赖
├── pnpm-lock.yaml
├── tsconfig.json
├── vite.config.ts
├── tailwind.config.js
├── postcss.config.js
├── index.html
├── .gitignore
├── LICENSE
└── README.md
```

---

## 三、核心模块设计

### 3.1 音频提取模块 (`audio/`)

**职责**：从视频文件中提取音频，转换为 whisper.cpp 所需的 16kHz 单声道 WAV/f32 PCM 格式。

**关键设计**：
- 使用 `ffmpeg-next` crate 直接链接 FFmpeg 库，避免依赖系统 FFmpeg CLI
- 支持流式提取，大文件不需要完整加载到内存
- 输出临时 WAV 文件到 `app_data_dir/tmp/`

```rust
// audio/extractor.rs — 核心接口设计

use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// 音频提取配置
pub struct ExtractConfig {
    /// 采样率（默认 16000）
    pub sample_rate: u32,
    /// 声道数（默认 1 = mono）
    pub channels: u16,
    /// 输出格式
    pub format: AudioFormat,
}

impl Default for ExtractConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
            format: AudioFormat::Wav,
        }
    }
}

pub enum AudioFormat {
    Wav,
    RawPcm,
}

/// 提取进度回调
pub struct ExtractProgress {
    pub percent: f32,
    pub duration_ms: u64,
    pub processed_ms: u64,
}

/// 音频提取器
pub struct AudioExtractor;

impl AudioExtractor {
    /// 从视频文件提取音频
    ///
    /// - `input`: 视频文件路径
    /// - `output_dir`: 输出目录
    /// - `config`: 提取配置
    /// - `progress_tx`: 进度发送通道
    pub async fn extract(
        input: &Path,
        output_dir: &Path,
        config: &ExtractConfig,
        progress_tx: mpsc::Sender<ExtractProgress>,
    ) -> Result<PathBuf, AudioError> {
        // 1. 使用 ffmpeg-next 打开输入文件
        // 2. 找到最佳音频流
        // 3. 设置重采样器 (resample to 16kHz mono)
        // 4. 逐帧解码 + 重采样 + 写入输出文件
        // 5. 通过 progress_tx 发送进度
        todo!()
    }

    /// 获取视频文件的媒体信息
    pub fn probe(input: &Path) -> Result<MediaInfo, AudioError> {
        todo!()
    }
}

pub struct MediaInfo {
    pub duration_ms: u64,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub audio_sample_rate: u32,
    pub audio_channels: u16,
    pub file_size: u64,
}
```

### 3.2 语音识别模块 (`asr/`)

**职责**：将音频转换为带时间戳的文本片段（segments）。

**关键设计**：
- 基于 `whisper-rs`（whisper.cpp 的 Rust binding）
- 支持多种模型大小（tiny/base/small/medium/large）
- 自动检测语言，也可手动指定
- 支持 GPU 加速（CUDA / Metal / Vulkan，编译时特性门控）
- 长音频按 30s chunk 处理，流式返回结果

```rust
// asr/engine.rs — ASR 引擎 trait

use async_trait::async_trait;
use tokio::sync::mpsc;

/// 识别出的单个片段
#[derive(Debug, Clone, serde::Serialize)]
pub struct Segment {
    /// 序号（从 0 开始）
    pub index: usize,
    /// 开始时间（毫秒）
    pub start_ms: u64,
    /// 结束时间（毫秒）
    pub end_ms: u64,
    /// 识别文本
    pub text: String,
    /// 检测到的语言
    pub language: String,
    /// 置信度 (0.0 - 1.0)
    pub confidence: f32,
}

/// ASR 配置
#[derive(Debug, Clone)]
pub struct AsrConfig {
    /// 模型路径
    pub model_path: std::path::PathBuf,
    /// 指定语言（None = 自动检测）
    pub language: Option<String>,
    /// 是否启用翻译到英文（whisper 内置功能）
    pub translate_to_english: bool,
    /// 线程数
    pub n_threads: u32,
    /// 是否使用 GPU
    pub use_gpu: bool,
}

/// ASR 进度
pub struct AsrProgress {
    pub percent: f32,
    pub current_segment: Option<Segment>,
}

/// ASR 引擎 trait — 方便后续扩展其他引擎
#[async_trait]
pub trait AsrEngine: Send + Sync {
    /// 执行语音识别
    async fn transcribe(
        &self,
        audio_path: &std::path::Path,
        config: &AsrConfig,
        progress_tx: mpsc::Sender<AsrProgress>,
    ) -> Result<Vec<Segment>, AsrError>;

    /// 获取支持的语言列表
    fn supported_languages(&self) -> Vec<(&str, &str)>;
}
```

```rust
// asr/whisper.rs — whisper.cpp 实现

use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};

pub struct WhisperEngine {
    ctx: WhisperContext,
}

impl WhisperEngine {
    pub fn new(model_path: &std::path::Path, use_gpu: bool) -> Result<Self, AsrError> {
        let mut params = WhisperContextParameters::default();
        params.use_gpu(use_gpu);
        let ctx = WhisperContext::new_with_params(
            model_path.to_str().unwrap(),
            params,
        ).map_err(|e| AsrError::ModelLoad(e.to_string()))?;
        Ok(Self { ctx })
    }
}

#[async_trait::async_trait]
impl AsrEngine for WhisperEngine {
    async fn transcribe(
        &self,
        audio_path: &std::path::Path,
        config: &AsrConfig,
        progress_tx: tokio::sync::mpsc::Sender<AsrProgress>,
    ) -> Result<Vec<Segment>, AsrError> {
        // 1. 读取 WAV 文件为 f32 samples
        // 2. 配置 FullParams (language, n_threads, etc.)
        // 3. 调用 ctx.full() 执行识别
        // 4. 遍历 segments，构建 Vec<Segment>
        // 5. 通过 progress_tx 发送进度
        todo!()
    }

    fn supported_languages(&self) -> Vec<(&str, &str)> {
        vec![
            ("auto", "自动检测"),
            ("en", "English"),
            ("zh", "中文"),
            ("ja", "日本語"),
            ("ko", "한국어"),
            ("fr", "Français"),
            ("de", "Deutsch"),
            ("es", "Español"),
            ("ru", "Русский"),
            // ... 99 languages supported by whisper
        ]
    }
}
```

### 3.3 翻译模块 (`translate/`)

**职责**：将源语言文本翻译为目标语言。

**关键设计**：
- 抽象 `TranslateEngine` trait，支持多后端切换
- 在线模式：LLM API (OpenAI / DeepSeek / Claude) 或 DeepL
- 离线模式：Opus-MT (Helsinki-NLP) via ONNX Runtime
- 批量翻译：按 segment 批次发送，保持上下文连贯
- 翻译缓存：相同文本不重复翻译

```rust
// translate/engine.rs — 翻译引擎 trait

use async_trait::async_trait;

/// 翻译请求
#[derive(Debug, Clone)]
pub struct TranslateRequest {
    /// 源文本列表（按 segment 分组）
    pub texts: Vec<String>,
    /// 源语言 (ISO 639-1, e.g., "en", "zh")
    pub source_lang: String,
    /// 目标语言
    pub target_lang: String,
    /// 上下文提示（帮助 LLM 理解领域）
    pub context_hint: Option<String>,
}

/// 翻译结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct TranslateResult {
    /// 翻译后的文本列表（与输入一一对应）
    pub texts: Vec<String>,
    /// 使用的引擎名称
    pub engine: String,
}

/// 翻译进度
pub struct TranslateProgress {
    pub percent: f32,
    pub translated_count: usize,
    pub total_count: usize,
}

#[async_trait]
pub trait TranslateEngine: Send + Sync {
    /// 批量翻译
    async fn translate(
        &self,
        request: &TranslateRequest,
        progress_tx: tokio::sync::mpsc::Sender<TranslateProgress>,
    ) -> Result<TranslateResult, TranslateError>;

    /// 引擎名称
    fn name(&self) -> &str;

    /// 是否需要网络
    fn requires_network(&self) -> bool;

    /// 支持的语言对
    fn supported_pairs(&self) -> Vec<(String, String)>;
}
```

```rust
// translate/llm_api.rs — LLM API 翻译实现

pub struct LlmTranslateEngine {
    client: reqwest::Client,
    api_key: String,
    api_base: String,
    model: String,
}

impl LlmTranslateEngine {
    pub fn new(api_key: String, api_base: String, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            api_base,
            model,
        }
    }

    /// 构建翻译 prompt
    fn build_prompt(&self, texts: &[String], source_lang: &str, target_lang: &str) -> String {
        format!(
            r#"You are a professional subtitle translator. Translate the following subtitle segments from {source_lang} to {target_lang}.

Rules:
1. Maintain the original meaning and tone
2. Keep translations concise (suitable for subtitles)
3. Return ONLY the translations, one per line, in the same order
4. Do NOT add numbering or extra formatting

Segments:
{}
"#,
            texts.iter()
                .enumerate()
                .map(|(i, t)| format!("[{}] {}", i + 1, t))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

#[async_trait::async_trait]
impl TranslateEngine for LlmTranslateEngine {
    async fn translate(
        &self,
        request: &TranslateRequest,
        progress_tx: tokio::sync::mpsc::Sender<TranslateProgress>,
    ) -> Result<TranslateResult, TranslateError> {
        // 1. 按批次（每批 20 个 segment）分组
        // 2. 每批构建 prompt，调用 LLM API
        // 3. 解析返回的翻译文本
        // 4. 合并所有批次结果
        // 5. 发送进度
        todo!()
    }

    fn name(&self) -> &str { "LLM API" }
    fn requires_network(&self) -> bool { true }
    fn supported_pairs(&self) -> Vec<(String, String)> {
        // LLM 支持任意语言对
        vec![]
    }
}
```

### 3.4 字幕处理模块 (`subtitle/`)

**职责**：管理字幕数据结构，支持 SRT/ASS/VTT 格式的读写，实现双语合并。

```rust
// subtitle/types.rs — 核心数据结构

use serde::{Deserialize, Serialize};

/// 时间码
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Timecode {
    pub hours: u32,
    pub minutes: u32,
    pub seconds: u32,
    pub milliseconds: u32,
}

impl Timecode {
    pub fn from_ms(total_ms: u64) -> Self {
        Self {
            hours: (total_ms / 3_600_000) as u32,
            minutes: ((total_ms % 3_600_000) / 60_000) as u32,
            seconds: ((total_ms % 60_000) / 1_000) as u32,
            milliseconds: (total_ms % 1_000) as u32,
        }
    }

    pub fn to_ms(&self) -> u64 {
        self.hours as u64 * 3_600_000
            + self.minutes as u64 * 60_000
            + self.seconds as u64 * 1_000
            + self.milliseconds as u64
    }

    /// SRT 格式: "00:01:23,456"
    pub fn to_srt_string(&self) -> String {
        format!(
            "{:02}:{:02}:{:02},{:03}",
            self.hours, self.minutes, self.seconds, self.milliseconds
        )
    }

    /// VTT 格式: "00:01:23.456"
    pub fn to_vtt_string(&self) -> String {
        format!(
            "{:02}:{:02}:{:02}.{:03}",
            self.hours, self.minutes, self.seconds, self.milliseconds
        )
    }

    /// ASS 格式: "0:01:23.46" (centiseconds)
    pub fn to_ass_string(&self) -> String {
        format!(
            "{}:{:02}:{:02}.{:02}",
            self.hours, self.minutes, self.seconds, self.milliseconds / 10
        )
    }
}

/// 单条字幕
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleEntry {
    pub index: usize,
    pub start: Timecode,
    pub end: Timecode,
    /// 主语言文本
    pub primary_text: String,
    /// 副语言文本（双语模式）
    pub secondary_text: Option<String>,
}

/// 字幕文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleFile {
    pub entries: Vec<SubtitleEntry>,
    pub source_language: String,
    pub target_language: Option<String>,
    pub format: SubtitleFormat,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SubtitleFormat {
    Srt,
    Ass,
    Vtt,
}
```

```rust
// subtitle/srt.rs — SRT 格式读写

use super::types::*;

pub struct SrtWriter;

impl SrtWriter {
    /// 生成 SRT 字幕内容
    pub fn write(subtitle: &SubtitleFile) -> String {
        let mut output = String::new();
        for entry in &subtitle.entries {
            // 序号
            output.push_str(&format!("{}\n", entry.index));
            // 时间轴
            output.push_str(&format!(
                "{} --> {}\n",
                entry.start.to_srt_string(),
                entry.end.to_srt_string()
            ));
            // 文本（双语则上下两行）
            output.push_str(&entry.primary_text);
            output.push('\n');
            if let Some(ref secondary) = entry.secondary_text {
                output.push_str(secondary);
                output.push('\n');
            }
            output.push('\n');
        }
        output
    }

    /// 解析 SRT 字幕内容
    pub fn parse(content: &str) -> Result<Vec<SubtitleEntry>, SubtitleError> {
        // 按空行分割 blocks
        // 每个 block: 序号 + 时间轴 + 文本
        todo!()
    }
}
```

```rust
// subtitle/ass.rs — ASS 格式（支持丰富样式的双语字幕）

pub struct AssWriter;

impl AssWriter {
    /// 生成 ASS 字幕内容（双语样式）
    pub fn write(subtitle: &SubtitleFile, style: &AssStyle) -> String {
        let mut output = String::new();

        // [Script Info]
        output.push_str("[Script Info]\n");
        output.push_str("Title: SubtitleForge Generated\n");
        output.push_str("ScriptType: v4.00+\n");
        output.push_str("PlayResX: 1920\n");
        output.push_str("PlayResY: 1080\n");
        output.push_str("WrapStyle: 0\n\n");

        // [V4+ Styles] — 主语言和副语言分别定义样式
        output.push_str("[V4+ Styles]\n");
        output.push_str("Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n");
        // 主语言样式（下方，白色）
        output.push_str(&format!(
            "Style: Primary,{},{},&H00FFFFFF,&H000000FF,&H00000000,&H80000000,-1,0,0,0,100,100,0,0,1,2,1,2,10,10,30,1\n",
            style.primary_font, style.primary_size
        ));
        // 副语言样式（上方，淡黄色）
        output.push_str(&format!(
            "Style: Secondary,{},{},&H0000FFFF,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,2,1,8,10,10,30,1\n",
            style.secondary_font, style.secondary_size
        ));
        output.push('\n');

        // [Events]
        output.push_str("[Events]\n");
        output.push_str("Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n");

        for entry in &subtitle.entries {
            let start = entry.start.to_ass_string();
            let end = entry.end.to_ass_string();

            // 主语言字幕行
            output.push_str(&format!(
                "Dialogue: 0,{},{},Primary,,0,0,0,,{}\n",
                start, end, entry.primary_text
            ));
            // 副语言字幕行
            if let Some(ref secondary) = entry.secondary_text {
                output.push_str(&format!(
                    "Dialogue: 0,{},{},Secondary,,0,0,0,,{}\n",
                    start, end, secondary
                ));
            }
        }
        output
    }
}

/// ASS 样式配置
pub struct AssStyle {
    pub primary_font: String,
    pub primary_size: u32,
    pub secondary_font: String,
    pub secondary_size: u32,
}

impl Default for AssStyle {
    fn default() -> Self {
        Self {
            primary_font: "Arial".to_string(),
            primary_size: 48,
            secondary_font: "Arial".to_string(),
            secondary_size: 36,
        }
    }
}
```

```rust
// subtitle/merger.rs — 双语字幕合并

use super::types::*;
use crate::asr::engine::Segment;
use crate::translate::engine::TranslateResult;

pub struct SubtitleMerger;

impl SubtitleMerger {
    /// 合并 ASR 结果和翻译结果为双语字幕
    pub fn merge(
        segments: &[Segment],
        translation: &TranslateResult,
        source_lang: &str,
        target_lang: &str,
        format: SubtitleFormat,
    ) -> Result<SubtitleFile, SubtitleError> {
        assert_eq!(
            segments.len(),
            translation.texts.len(),
            "Segment count mismatch"
        );

        let entries: Vec<SubtitleEntry> = segments
            .iter()
            .zip(translation.texts.iter())
            .enumerate()
            .map(|(i, (seg, translated))| SubtitleEntry {
                index: i + 1,
                start: Timecode::from_ms(seg.start_ms),
                end: Timecode::from_ms(seg.end_ms),
                primary_text: seg.text.trim().to_string(),
                secondary_text: Some(translated.trim().to_string()),
            })
            .collect();

        Ok(SubtitleFile {
            entries,
            source_language: source_lang.to_string(),
            target_language: Some(target_lang.to_string()),
            format,
        })
    }
}
```

### 3.5 管线编排器 (`pipeline/`)

**职责**：串联所有模块，管理异步执行流程和进度通知。

```rust
// pipeline/orchestrator.rs

use crate::audio::extractor::AudioExtractor;
use crate::asr::engine::{AsrEngine, AsrConfig, Segment};
use crate::translate::engine::{TranslateEngine, TranslateRequest};
use crate::subtitle::merger::SubtitleMerger;
use crate::subtitle::types::*;
use crate::subtitle::srt::SrtWriter;
use crate::subtitle::ass::AssWriter;
use tokio::sync::mpsc;
use serde::Serialize;

/// 管线任务配置
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PipelineConfig {
    pub input_path: String,
    pub output_dir: String,
    pub source_language: Option<String>,
    pub target_language: String,
    pub output_format: SubtitleFormat,
    pub asr_model: String,
    pub translate_engine: String,
    pub use_gpu: bool,
}

/// 管线阶段
#[derive(Debug, Clone, Serialize)]
pub enum PipelineStage {
    Idle,
    ExtractingAudio { percent: f32 },
    Transcribing { percent: f32, current_text: Option<String> },
    Translating { percent: f32, translated_count: usize, total_count: usize },
    GeneratingSubtitle,
    Completed { output_path: String },
    Failed { error: String },
}

/// 管线编排器
pub struct PipelineOrchestrator {
    asr_engine: Box<dyn AsrEngine>,
    translate_engine: Box<dyn TranslateEngine>,
}

impl PipelineOrchestrator {
    pub fn new(
        asr_engine: Box<dyn AsrEngine>,
        translate_engine: Box<dyn TranslateEngine>,
    ) -> Self {
        Self { asr_engine, translate_engine }
    }

    /// 执行完整管线
    pub async fn run(
        &self,
        config: PipelineConfig,
        stage_tx: mpsc::Sender<PipelineStage>,
    ) -> Result<String, PipelineError> {
        // ============ Stage 1: 音频提取 ============
        let (progress_tx, mut progress_rx) = mpsc::channel(32);
        let audio_path = {
            let input = std::path::Path::new(&config.input_path);
            let output_dir = std::path::Path::new(&config.output_dir);

            // 转发进度到 stage_tx
            let stage_tx_clone = stage_tx.clone();
            tokio::spawn(async move {
                while let Some(p) = progress_rx.recv().await {
                    let _ = stage_tx_clone
                        .send(PipelineStage::ExtractingAudio { percent: p.percent })
                        .await;
                }
            });

            AudioExtractor::extract(
                input,
                output_dir,
                &Default::default(),
                progress_tx,
            ).await?
        };

        // ============ Stage 2: 语音识别 ============
        let (asr_progress_tx, mut asr_progress_rx) = mpsc::channel(32);
        let asr_config = AsrConfig {
            model_path: std::path::PathBuf::from(&config.asr_model),
            language: config.source_language.clone(),
            translate_to_english: false,
            n_threads: num_cpus::get() as u32,
            use_gpu: config.use_gpu,
        };

        let stage_tx_clone = stage_tx.clone();
        tokio::spawn(async move {
            while let Some(p) = asr_progress_rx.recv().await {
                let _ = stage_tx_clone
                    .send(PipelineStage::Transcribing {
                        percent: p.percent,
                        current_text: p.current_segment.map(|s| s.text),
                    })
                    .await;
            }
        });

        let segments = self.asr_engine
            .transcribe(&audio_path, &asr_config, asr_progress_tx)
            .await?;

        // ============ Stage 3: 翻译 ============
        let (trans_progress_tx, mut trans_progress_rx) = mpsc::channel(32);
        let source_lang = segments.first()
            .map(|s| s.language.clone())
            .unwrap_or_else(|| "auto".to_string());

        let translate_request = TranslateRequest {
            texts: segments.iter().map(|s| s.text.clone()).collect(),
            source_lang: source_lang.clone(),
            target_lang: config.target_language.clone(),
            context_hint: None,
        };

        let stage_tx_clone = stage_tx.clone();
        tokio::spawn(async move {
            while let Some(p) = trans_progress_rx.recv().await {
                let _ = stage_tx_clone
                    .send(PipelineStage::Translating {
                        percent: p.percent,
                        translated_count: p.translated_count,
                        total_count: p.total_count,
                    })
                    .await;
            }
        });

        let translation = self.translate_engine
            .translate(&translate_request, trans_progress_tx)
            .await?;

        // ============ Stage 4: 生成字幕文件 ============
        let _ = stage_tx.send(PipelineStage::GeneratingSubtitle).await;

        let subtitle = SubtitleMerger::merge(
            &segments,
            &translation,
            &source_lang,
            &config.target_language,
            config.output_format,
        )?;

        let output_content = match config.output_format {
            SubtitleFormat::Srt => SrtWriter::write(&subtitle),
            SubtitleFormat::Ass => AssWriter::write(&subtitle, &Default::default()),
            SubtitleFormat::Vtt => todo!("VTT writer"),
        };

        let ext = match config.output_format {
            SubtitleFormat::Srt => "srt",
            SubtitleFormat::Ass => "ass",
            SubtitleFormat::Vtt => "vtt",
        };
        let input_stem = std::path::Path::new(&config.input_path)
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap();
        let output_filename = format!(
            "{}.{}-{}.{}",
            input_stem, source_lang, config.target_language, ext
        );
        let output_path = std::path::Path::new(&config.output_dir).join(&output_filename);
        tokio::fs::write(&output_path, &output_content).await?;

        let output_str = output_path.to_string_lossy().to_string();
        let _ = stage_tx
            .send(PipelineStage::Completed { output_path: output_str.clone() })
            .await;

        Ok(output_str)
    }
}
```

### 3.6 Tauri IPC 命令层 (`commands/`)

**职责**：桥接前端调用和后端逻辑，管理 Tauri 状态。

```rust
// commands/mod.rs

use tauri::{AppHandle, State, Manager};
use tokio::sync::mpsc;
use crate::pipeline::orchestrator::{PipelineConfig, PipelineOrchestrator, PipelineStage};

/// Tauri 管理的全局状态
pub struct AppState {
    pub orchestrator: std::sync::Arc<tokio::sync::Mutex<Option<PipelineOrchestrator>>>,
    pub models_dir: std::path::PathBuf,
}

/// 启动处理管线
#[tauri::command]
pub async fn start_pipeline(
    app: AppHandle,
    state: State<'_, AppState>,
    config: PipelineConfig,
) -> Result<String, String> {
    let orchestrator = state.orchestrator.lock().await;
    let orchestrator = orchestrator.as_ref().ok_or("Engine not initialized")?;

    let (stage_tx, mut stage_rx) = mpsc::channel::<PipelineStage>(64);

    // 将管线阶段转发为 Tauri 事件
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(stage) = stage_rx.recv().await {
            let _ = app_clone.emit("pipeline-progress", &stage);
        }
    });

    orchestrator
        .run(config, stage_tx)
        .await
        .map_err(|e| e.to_string())
}

/// 获取可用模型列表
#[tauri::command]
pub async fn list_models(
    state: State<'_, AppState>,
) -> Result<Vec<ModelInfo>, String> {
    let models_dir = &state.models_dir;
    // 扫描 models/ 目录，返回可用模型信息
    todo!()
}

/// 获取视频文件信息
#[tauri::command]
pub async fn probe_video(path: String) -> Result<crate::audio::extractor::MediaInfo, String> {
    crate::audio::extractor::AudioExtractor::probe(std::path::Path::new(&path))
        .map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct ModelInfo {
    pub name: String,
    pub size_mb: u64,
    pub model_type: String,
    pub path: String,
}
```

```rust
// main.rs — Tauri 入口

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod asr;
mod commands;
mod config;
mod error;
mod pipeline;
mod subtitle;
mod translate;

use commands::AppState;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().expect("Failed to get app data dir");
            let models_dir = app_data_dir.join("models");
            std::fs::create_dir_all(&models_dir).ok();

            app.manage(AppState {
                orchestrator: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
                models_dir,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_pipeline,
            commands::list_models,
            commands::probe_video,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

---

## 四、前端设计

### 4.1 核心组件

```typescript
// src/hooks/usePipeline.ts

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useState, useCallback, useEffect } from 'react';

export interface PipelineConfig {
  input_path: string;
  output_dir: string;
  source_language: string | null;
  target_language: string;
  output_format: 'Srt' | 'Ass' | 'Vtt';
  asr_model: string;
  translate_engine: string;
  use_gpu: boolean;
}

export interface PipelineStage {
  type: 'Idle' | 'ExtractingAudio' | 'Transcribing' | 'Translating' | 'GeneratingSubtitle' | 'Completed' | 'Failed';
  percent?: number;
  current_text?: string;
  translated_count?: number;
  total_count?: number;
  output_path?: string;
  error?: string;
}

export function usePipeline() {
  const [stage, setStage] = useState<PipelineStage>({ type: 'Idle' });
  const [isRunning, setIsRunning] = useState(false);

  useEffect(() => {
    const unlisten = listen<PipelineStage>('pipeline-progress', (event) => {
      setStage(event.payload);
      if (event.payload.type === 'Completed' || event.payload.type === 'Failed') {
        setIsRunning(false);
      }
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  const start = useCallback(async (config: PipelineConfig) => {
    setIsRunning(true);
    setStage({ type: 'ExtractingAudio', percent: 0 });
    try {
      const result = await invoke<string>('start_pipeline', { config });
      return result;
    } catch (err) {
      setStage({ type: 'Failed', error: String(err) });
      setIsRunning(false);
      throw err;
    }
  }, []);

  return { stage, isRunning, start };
}
```

```tsx
// src/components/FileDropZone.tsx

import { open } from '@tauri-apps/plugin-dialog';
import { useState, DragEvent } from 'react';

interface FileDropZoneProps {
  onFileSelect: (path: string) => void;
}

const VIDEO_EXTENSIONS = ['mp4', 'mkv', 'avi', 'mov', 'wmv', 'flv', 'webm', 'ts'];

export function FileDropZone({ onFileSelect }: FileDropZoneProps) {
  const [isDragging, setIsDragging] = useState(false);

  const handleClick = async () => {
    const file = await open({
      multiple: false,
      filters: [{ name: 'Video', extensions: VIDEO_EXTENSIONS }],
    });
    if (file) onFileSelect(file);
  };

  return (
    <div
      onClick={handleClick}
      onDragOver={(e: DragEvent) => { e.preventDefault(); setIsDragging(true); }}
      onDragLeave={() => setIsDragging(false)}
      onDrop={(e: DragEvent) => {
        e.preventDefault();
        setIsDragging(false);
        // Tauri 2.0 handles file drop via plugin
      }}
      className={`
        border-2 border-dashed rounded-xl p-12 text-center cursor-pointer
        transition-colors duration-200
        ${isDragging
          ? 'border-blue-500 bg-blue-50 dark:bg-blue-950'
          : 'border-gray-300 hover:border-gray-400 dark:border-gray-600'}
      `}
    >
      <div className="text-4xl mb-4">🎬</div>
      <p className="text-lg font-medium">拖拽视频文件到这里</p>
      <p className="text-sm text-gray-500 mt-2">
        或点击选择文件 · 支持 MP4, MKV, AVI, MOV 等格式
      </p>
    </div>
  );
}
```

```tsx
// src/components/ProgressPanel.tsx

import { PipelineStage } from '../hooks/usePipeline';

interface ProgressPanelProps {
  stage: PipelineStage;
}

const STAGE_LABELS: Record<string, string> = {
  Idle: '等待开始',
  ExtractingAudio: '🎵 提取音频',
  Transcribing: '🎙️ 语音识别',
  Translating: '🌐 翻译中',
  GeneratingSubtitle: '📝 生成字幕',
  Completed: '✅ 完成',
  Failed: '❌ 失败',
};

export function ProgressPanel({ stage }: ProgressPanelProps) {
  const label = STAGE_LABELS[stage.type] || stage.type;
  const percent = stage.percent ?? 0;

  return (
    <div className="bg-white dark:bg-gray-800 rounded-xl p-6 shadow-sm">
      <div className="flex items-center justify-between mb-3">
        <span className="font-medium">{label}</span>
        {stage.percent !== undefined && (
          <span className="text-sm text-gray-500">{Math.round(percent)}%</span>
        )}
      </div>

      {/* 进度条 */}
      <div className="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2.5">
        <div
          className="bg-blue-600 h-2.5 rounded-full transition-all duration-300"
          style={{ width: `${percent}%` }}
        />
      </div>

      {/* 当前识别文本预览 */}
      {stage.current_text && (
        <p className="mt-3 text-sm text-gray-600 dark:text-gray-400 italic truncate">
          "{stage.current_text}"
        </p>
      )}

      {/* 翻译进度 */}
      {stage.translated_count !== undefined && (
        <p className="mt-2 text-xs text-gray-500">
          已翻译 {stage.translated_count} / {stage.total_count} 段
        </p>
      )}

      {/* 完成路径 */}
      {stage.output_path && (
        <p className="mt-3 text-sm text-green-600">
          📁 输出：{stage.output_path}
        </p>
      )}

      {/* 错误信息 */}
      {stage.error && (
        <p className="mt-3 text-sm text-red-600">
          {stage.error}
        </p>
      )}
    </div>
  );
}
```

---

## 五、Cargo.toml 依赖配置

```toml
[package]
name = "subtitle-forge"
version = "0.1.0"
edition = "2021"
description = "Cross-platform bilingual subtitle generator"

[dependencies]
# Tauri 核心
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-dialog = "2"
tauri-plugin-fs = "2"
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# 异步运行时
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"

# 音频处理
ffmpeg-next = "7"             # FFmpeg Rust binding

# 语音识别
whisper-rs = "0.12"           # whisper.cpp Rust binding

# 翻译（在线）
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }

# 翻译（离线 — 可选）
ort = { version = "2", optional = true }     # ONNX Runtime
tokenizers = { version = "0.19", optional = true }

# 工具
num_cpus = "1"
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
chrono = "0.4"
uuid = { version = "1", features = ["v4"] }
dirs = "5"

[features]
default = ["online-translate"]
online-translate = []
offline-translate = ["ort", "tokenizers"]
cuda = ["whisper-rs/cuda"]          # NVIDIA GPU
metal = ["whisper-rs/metal"]        # Apple GPU
vulkan = ["whisper-rs/vulkan"]      # Vulkan GPU

[build-dependencies]
tauri-build = { version = "2", features = [] }
```

---

## 六、构建与分发

### 6.1 开发环境搭建

```bash
# 1. 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. 安装 Node.js + pnpm
npm install -g pnpm

# 3. 安装 Tauri CLI
cargo install tauri-cli --version "^2"

# 4. 安装系统依赖
# macOS:
brew install ffmpeg pkg-config

# Ubuntu/Debian:
sudo apt install libffmpeg-dev pkg-config libssl-dev libgtk-3-dev \
  libwebkit2gtk-4.1-dev librsvg2-dev

# Windows:
# 下载 FFmpeg shared + dev 包，设置 FFMPEG_DIR 环境变量
# vcpkg install ffmpeg

# 5. 创建项目
pnpm create tauri-app subtitle-forge --template react-ts

# 6. 下载 Whisper 模型
mkdir -p models/whisper
curl -L -o models/whisper/ggml-base.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin
```

### 6.2 开发运行

```bash
# 启动开发模式（前端热重载 + Rust 增量编译）
cargo tauri dev

# 仅编译后端
cd src-tauri && cargo build

# 运行测试
cd src-tauri && cargo test
```

### 6.3 生产构建

```bash
# 构建安装包（自动检测当前平台）
cargo tauri build

# 跨平台产物：
# Windows: src-tauri/target/release/bundle/msi/*.msi
#          src-tauri/target/release/bundle/nsis/*.exe
# macOS:   src-tauri/target/release/bundle/dmg/*.dmg
#          src-tauri/target/release/bundle/macos/*.app
# Linux:   src-tauri/target/release/bundle/deb/*.deb
#          src-tauri/target/release/bundle/appimage/*.AppImage
```

### 6.4 FFmpeg 分发策略

| 策略 | 优点 | 缺点 | 推荐 |
|---|---|---|---|
| **Bundled (内置)** | 开箱即用，无需用户配置 | 包体积增大 ~30-50MB | ✅ 推荐 |
| System (依赖系统) | 包体积小 | 用户需自行安装，支持成本高 | ❌ |
| 首次下载 | 初始包小 | 需要网络，用户体验差 | ❌ |

**推荐方案**：通过 `build.rs` 在编译时静态链接 FFmpeg 库，确保开箱即用。

### 6.5 Whisper 模型分发策略

```
首次启动 → 检测模型是否存在
  ├── 存在 → 直接使用
  └── 不存在 → 弹出"模型下载"对话框
                ├── 选择模型大小 (tiny 75MB / base 142MB / small 466MB / medium 1.5GB)
                └── 后台下载 + 进度条 → 下载完成后可用
```

---

## 七、关键优化策略

### 7.1 性能优化

| 优化项 | 方案 |
|---|---|
| 长视频内存 | 音频按 30s chunk 流式处理，不全量加载 |
| GPU 加速 | whisper.cpp 支持 CUDA/Metal/Vulkan，编译时特性门控 |
| 并行翻译 | 翻译按批次并发请求（tokio::spawn） |
| 缓存 | ASR 结果缓存（基于文件 hash），避免重复识别 |
| 增量进度 | 全链路 channel-based 进度推送，不阻塞 UI |

### 7.2 用户体验优化

| 优化项 | 方案 |
|---|---|
| 拖拽支持 | Tauri 原生 file drop 事件 |
| 实时预览 | ASR 阶段就逐行展示识别结果 |
| 字幕编辑 | 内置简易编辑器，可调整时间轴和文本 |
| 批量处理 | 支持队列，多视频依次处理 |
| 暗色模式 | 跟随系统主题 |
| 国际化 | i18next，支持中英日韩界面 |

### 7.3 错误处理策略

```rust
// error.rs — 统一错误类型

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Audio extraction failed: {0}")]
    AudioExtract(#[from] AudioError),

    #[error("ASR failed: {0}")]
    Asr(#[from] AsrError),

    #[error("Translation failed: {0}")]
    Translate(#[from] TranslateError),

    #[error("Subtitle generation failed: {0}")]
    Subtitle(#[from] SubtitleError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Pipeline error: {0}")]
    Pipeline(String),
}

// 实现 serde::Serialize 以便通过 Tauri IPC 返回
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_str(&self.to_string())
    }
}
```

---

## 八、测试策略

| 测试类型 | 工具 | 覆盖范围 |
|---|---|---|
| 单元测试 | `cargo test` | 各模块独立逻辑 |
| 集成测试 | `cargo test --test integration` | 管线端到端 |
| 前端测试 | Vitest + React Testing Library | 组件 + Hook |
| E2E 测试 | Tauri WebDriver | 完整用户流程 |
| 性能基准 | `criterion` | ASR/翻译吞吐量 |

```rust
// tests/subtitle_test.rs — 字幕模块单元测试示例

#[cfg(test)]
mod tests {
    use subtitle_forge::subtitle::types::*;
    use subtitle_forge::subtitle::srt::SrtWriter;

    #[test]
    fn test_timecode_from_ms() {
        let tc = Timecode::from_ms(3_723_456);
        assert_eq!(tc.hours, 1);
        assert_eq!(tc.minutes, 2);
        assert_eq!(tc.seconds, 3);
        assert_eq!(tc.milliseconds, 456);
        assert_eq!(tc.to_srt_string(), "01:02:03,456");
    }

    #[test]
    fn test_srt_generation() {
        let subtitle = SubtitleFile {
            entries: vec![
                SubtitleEntry {
                    index: 1,
                    start: Timecode::from_ms(1000),
                    end: Timecode::from_ms(4000),
                    primary_text: "Hello, world!".to_string(),
                    secondary_text: Some("你好，世界！".to_string()),
                },
            ],
            source_language: "en".to_string(),
            target_language: Some("zh".to_string()),
            format: SubtitleFormat::Srt,
        };

        let srt = SrtWriter::write(&subtitle);
        assert!(srt.contains("Hello, world!"));
        assert!(srt.contains("你好，世界！"));
        assert!(srt.contains("00:00:01,000 --> 00:00:04,000"));
    }
}
```

---

## 九、后续扩展路线

| 阶段 | 功能 | 优先级 |
|---|---|---|
| v0.1 | 核心链路：提取 → ASR → 翻译 → SRT 输出 | P0 |
| v0.2 | GPU 加速、ASS 格式、字幕编辑器 | P1 |
| v0.3 | 离线翻译（Opus-MT）、批量处理队列 | P1 |
| v0.4 | 字幕样式定制、实时视频预览 | P2 |
| v0.5 | 插件系统（自定义翻译/ASR 引擎） | P2 |
| v1.0 | 自动更新、国际化、性能调优、稳定发布 | P0 |

---

## 十、许可证与兼容性

| 依赖 | License | 注意事项 |
|---|---|---|
| Tauri | MIT/Apache-2.0 | 商用友好 |
| whisper.cpp | MIT | 商用友好 |
| FFmpeg | LGPL 2.1+ / GPL | **动态链接可 LGPL，静态链接需 GPL** |
| ONNX Runtime | MIT | 商用友好 |
| Opus-MT Models | CC-BY-4.0 | 需注明来源 |

> **重要**：如果选择静态链接 FFmpeg（含 x264 等 GPL 组件），整个应用需以 GPL 发布。
> 推荐方案：**动态链接 FFmpeg**（LGPL），或仅使用 LGPL 兼容的编解码器。
