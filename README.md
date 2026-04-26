# SubtitleForge

SubtitleForge 是一个基于 `Tauri v2 + React + Rust` 的桌面应用，用于从视频中生成字幕，并在需要时生成双语字幕。

## 开发环境初始化

首次在本地开发前，请先执行与当前平台对应的初始化脚本：

```bash
# macOS
./scripts/init-macos.sh

# Linux
./scripts/init-linux.sh

# Windows PowerShell / cmd
scripts\init-windows.bat
```

初始化脚本会负责安装依赖、设置环境变量，并在需要时生成当前平台专用的本地配置。

## 本地配置约定

- `src-tauri/.cargo` 是本地编译环境目录，已被 Git 忽略，不应提交到仓库
- 不同平台使用不同配置来源：
  - Windows 通过 `init-windows.bat` 写入环境变量
  - macOS / Linux 通过各自的 `init` 脚本生成 `src-tauri/.cargo/config.toml`
- 若切换开发平台或遇到构建异常，请重新执行对应平台的 `init` 脚本

## 启动前检查

- `pnpm tauri dev`
- `pnpm tauri build`

在执行前会先运行预检查：

- Windows 若检测到遗留的 `src-tauri/.cargo/config.toml`，会直接报错并提示重新执行 `init-windows.bat`
- macOS / Linux 若缺少 `src-tauri/.cargo/config.toml`，会提示先执行对应平台的 `init` 脚本
- Windows 若缺少 `FFMPEG_DIR`、`LIBCLANG_PATH`、`BINDGEN_EXTRA_CLANG_ARGS` 等关键环境变量，也会直接提示

这样可以尽量把“环境没初始化”与“代码本身有问题”区分开。

## 安装版本

发布页现在会按平台和能力区分安装包名称，便于用户直接选择合适版本：

- `macos-intel-metal`
  - 适用于 Intel Mac
  - 默认包含 Metal 后端
- `macos-apple-silicon-metal`
  - 适用于 Apple Silicon Mac
  - 默认包含 Metal 后端
- `windows-x64-standard`
  - 适用于常规 64 位 Windows
  - 不额外要求 CUDA 运行时
- `windows-x64-openblas`
  - 适用于常规 64 位 Windows
  - 启用 `openblas`，偏向 CPU 推理性能优化
- `windows-arm64-standard`
  - 适用于 Windows ARM64
  - 不额外要求 CUDA 运行时
- `linux-x64-standard`
  - 适用于通用 64 位 Linux
  - 追求兼容性优先
- `linux-x64-openblas`
  - 适用于 64 位 Linux
  - 启用 `openblas`，偏向 CPU 性能优化

当前会在 CI 中自动发布 `windows-x64-openblas` 版本，但还没有自动发布 `cuda` 版本。

原因是 `cuda` 版本通常要求构建机和目标机都具备额外的 CUDA 工具链或运行时，GitHub Hosted Windows runner 难以稳定提供这类环境。后续如果引入专门的 Windows CUDA 构建环境，可以再把它加入官方发布矩阵。

## 本地构建不同版本

如果需要在本地手动构建特性版安装包，可以使用这些命令：

```bash
# 通用构建
pnpm tauri:build

# Linux / Windows，需启用 CPU BLAS 加速时
pnpm tauri:build:openblas

# 具备 CUDA 构建环境时
pnpm tauri:build:cuda

# Apple Core ML 版本
pnpm tauri:build:coreml

# macOS Metal 版本
pnpm tauri:build:metal
```

建议优先选择与当前操作系统和硬件匹配的版本，不要盲目选择带更多 feature 的安装包。

## 当前工作流程


1. 选择输入视频文件
2. 提取音频
3. 执行语音识别（ASR）
4. 基于识别结果决定是否翻译
5. 生成字幕文件

对应到当前后端实现，流水线在 [orchestrator.rs](file:///Users/zhaopeng.charles/code/magechiu/subtitle-forge/src-tauri/src/pipeline/orchestrator.rs) 中分为 4 个阶段：

1. `ExtractingAudio`
   - 从视频中提取单声道、16kHz 的 WAV 音频
2. `Transcribing`
   - 使用 Whisper 模型将音频切分并识别为一组带时间戳的原语言片段
3. `Translating`
   - 当未开启 `skip_translation` 时，将第 2 步得到的原语言片段送入翻译服务
4. `GeneratingSubtitle`
   - 将原语言片段和翻译结果合并，输出 `SRT / ASS / VTT`

## 当前数据流

从数据结构上看，当前已经是“先原文、再翻译”的模型：

1. ASR 阶段先生成 `segments`
   - 每个 segment 都包含：
     - 起止时间
     - 原语言文本
     - 语言信息
2. 翻译阶段并不是重新对音频做处理
   - 而是直接对 `segments[].text` 进行翻译
3. 字幕合并阶段：
   - 单语模式：直接把 `segments` 写成字幕
   - 双语模式：把 `segments` 作为主文本，再把翻译结果写入副文本

这部分逻辑对应：

- 单语字幕生成：[from_segments](file:///Users/zhaopeng.charles/code/magechiu/subtitle-forge/src-tauri/src/subtitle/merger.rs#L49-L73)
- 双语字幕生成：[merge](file:///Users/zhaopeng.charles/code/magechiu/subtitle-forge/src-tauri/src/subtitle/merger.rs#L13-L47)

## 是否应该先生成原语言字幕，再叠加目标语言翻译？

结论：**是，这样更合理，而且当前实现本质上已经在这么做。**

更合理的原因有：

1. 原语言字幕是整个系统的基准层
   - 时间戳、断句、分段都应该由 ASR 决定
   - 翻译只应该作用在已经稳定的原语言字幕文本上

2. 更容易调试
   - 如果最终双语字幕有问题，可以快速判断问题来自：
     - ASR 识别错误
     - 分段不合理
     - 翻译质量不佳
   - 这也是 `skip_translation` 模式存在的核心价值

3. 更利于复用
   - 一份原语言字幕可以对应多个目标语言
   - 不需要每次都重新跑 ASR

4. 更适合后续能力扩展
   - 原文校对
   - 术语替换
   - 多语言批量翻译
   - 人工修订后再二次翻译

## 当前实现与理想流程的差异

当前实现虽然逻辑上是“先 ASR、后翻译”，但输出层面仍然偏向“一次性直接生成最终字幕”。

也就是说：

- `skip_translation = true`
  - 输出单语字幕
- `skip_translation = false`
  - 直接输出双语字幕

当前没有显式把“原语言字幕文件”作为一个稳定的中间产物落盘再继续后续翻译。

## 推荐的更合理流程

建议将流程明确为两层：

### 第一层：先生成原语言字幕

1. 提取音频
2. 执行 ASR
3. 生成原语言字幕文件
   - 例如：`video.en.srt`
   - 或：`video.zh.srt`

这一层的输出应视为“基准字幕”。

### 第二层：在原语言字幕基础上扩展目标语言

1. 读取原语言字幕条目
2. 逐条翻译原文文本
3. 将翻译结果写入副字幕行
4. 输出双语字幕文件
   - 例如：`video.en-zh.srt`

这样做的直接收益是：

- 原文字幕与双语字幕同时可得
- 单语 / 双语流程边界更清晰
- 翻译失败时不影响原文字幕产出
- 可以把翻译阶段做成独立任务

## 当前支持的翻译模式

当前项目中的翻译模式包括：

1. 在线翻译服务
   - Google Translate
   - LibreTranslate
   - DeepL

2. 在线 LLM 服务
   - DeepSeek
   - 方舟

3. 本地 LLM 服务
   - Ollama

4. 内嵌 LLM 服务
   - llama.cpp GGUF

## 模型管理

### Whisper 模型

Whisper 模型用于 ASR，当前支持下载和管理：

- Tiny
- Base
- Small
- Medium
- Large V3

### 内嵌 LLM 模型

内嵌 LLM 模型用于本地翻译，当前支持下载和管理：

- Qwen2.5 1.5B Instruct Q4_K_M
- Qwen2.5 3B Instruct Q4_K_M
- Llama 3.2 3B Instruct Q4_K_M

## 当前建议的使用方式

如果目标是稳定生成双语字幕，建议按下面顺序使用：

1. 先开启 `skip translation`
   - 验证 ASR 是否正常
   - 检查原语言字幕断句、时间轴、识别质量
2. 确认原语言字幕正确后，再开启翻译
3. 根据场景选择翻译模式
   - 追求稳定和速度：在线翻译服务
   - 追求质量和可控性：在线 LLM
   - 追求离线和隐私：本地 / 内嵌 LLM

## 后续推荐优化

从产品流程上，后续最值得做的优化是：

1. 显式输出原语言字幕中间文件
2. 允许“从已有原语言字幕继续翻译”
3. 支持一个原语言字幕对应多个目标语言批量生成
4. 将翻译阶段从“整条流水线的一部分”提升为“可独立执行的二阶段任务”

这样整体架构会更清晰，也更符合字幕生产的实际工作流。
