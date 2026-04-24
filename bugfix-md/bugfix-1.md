# Bugfix-1：工程 ASR 流水线比脚本直调 ffmpeg + whisper 慢的原因分析

## 背景

通过脚本直接调用 `ffmpeg` 抽音频 + `whisper-cli`（whisper.cpp）解析同一段视频，得到原始语言字幕所需的时间，**明显快于当前 Tauri 工程里相同输入、相同模型的处理耗时**。

本文定位"为什么工程版本慢"，给出分阶段的原因、影响估算和验证方法。

---

## 核心结论

一句话：脚本直调的 `ffmpeg + whisper-cli` 通常是 Homebrew / 官方预编译版本，**默认启用 Metal + Accelerate(BLAS) + SIMD swresample + 多线程解码**；而当前工程 `cargo build --release` **默认没开启任何 GPU/BLAS 加速 feature**，再加上 VAD 切片、每块重建 state、ffmpeg-next 单线程循环、二次重采样等冗余开销，叠加起来就显著变慢。

---

## 关键代码位置

- Whisper 推理入口：[src-tauri/src/asr/whisper.rs](src-tauri/src/asr/whisper.rs)
- ASR 默认配置：[src-tauri/src/asr/engine.rs](src-tauri/src/asr/engine.rs)
- VAD 规划：[src-tauri/src/asr/vad.rs](src-tauri/src/asr/vad.rs)
- 音频抽取：[src-tauri/src/audio/extractor.rs](src-tauri/src/audio/extractor.rs)
- Pipeline 编排：[src-tauri/src/pipeline/orchestrator.rs](src-tauri/src/pipeline/orchestrator.rs)
- 构建特性：[src-tauri/Cargo.toml](src-tauri/Cargo.toml)

---

## 问题清单（按影响大小排序）

### 1. 编译特性：推理走"纯 CPU 标量"路径（最大嫌疑）

`src-tauri/Cargo.toml`：

```toml
[features]
default  = ["online-translate"]
cuda     = ["whisper-rs/cuda"]
metal    = ["whisper-rs/metal"]
coreml   = ["whisper-rs/coreml"]
hipblas  = ["whisper-rs/hipblas"]
opencl   = ["whisper-rs/opencl"]
openblas = ["whisper-rs/openblas"]
```

`default` **没有带任何 GPU/BLAS feature**。意味着 `cargo build --release` 出来的 binary：

- 不链 Metal 内核（macOS 推理最关键路径）
- 不链 CoreML
- 不链 OpenBLAS / Accelerate

而 `src-tauri/src/asr/whisper.rs` 里虽然调用了 `ctx_params.use_gpu(use_gpu)`：

```rust
let mut ctx_params = whisper_rs::WhisperContextParameters::new();
ctx_params.use_gpu(use_gpu);
```

但没编 metal feature 时 ggml 根本没有 metal backend 可用，实际等同于关闭。

对比脚本路径：Homebrew 的 `whisper-cpp` / 官方预编译版本默认都带 Metal。同样模型、同样音频，**仅这一条差距常见 2–5 倍**。

### 2. 线程数给得太激进

`src-tauri/src/pipeline/orchestrator.rs`：

```rust
let asr_config = AsrConfig {
    ...
    n_threads: num_cpus::get() as u32,
    ...
};
```

在 Apple Silicon 上 `num_cpus::get()` 返回**所有核（含 E-core）**。例如 M2 Pro 10/12 核全部给到 whisper，会因为 P/E 核频率不同 + 内存带宽争抢反而变慢。

`whisper.cpp` 官方默认 `-t 4`，建议不超过 P-core 数。脚本路径通常只开 4 线程，反而更快。

### 3. VAD 切片 + 每块重建 state + 双层 overlap

`src-tauri/src/asr/whisper.rs` 中的主循环：

```rust
for (chunk_idx, chunk) in planned_chunks.iter().enumerate() {
    ...
    let mut state = ctx.create_state()?;        // 每块都重建 KV cache
    let mut params = FullParams::new(...);
    ...
    params.set_no_context(true);                // 不复用前文
    state.full(params, chunk_samples)?;         // 包含两侧 overlap 的整段再 encode
}
```

默认 VAD 配置 `src-tauri/src/asr/engine.rs`：

```rust
vad_max_chunk_ms: 120_000,   // 2 分钟
vad_overlap_ms:   5_000,     // 两侧各 5s
```

等价于：

- 每个 chunk 送给 whisper 的 window 是 `main + 左 5s + 右 5s`，**每段重复编码 10 秒**
- 每块都 `create_state()` 重建 KV cache
- `set_no_context(true)` 丢掉跨段上下文，encoder 每次都冷启动

`whisper-cli` 对完整音频默认使用内部 30s 滑窗 + prompt 延续，**累计需要 encode 的帧数显著少**于本工程方案。

此外 `transcribe_chunks` 还有 fallback 路径：VAD 方案产出 0 条字幕时，会**再完整跑一次 fixed chunk 的 transcribe**，最坏情况推理量翻倍。

### 4. VAD 本身有重复扫描

`src-tauri/src/asr/whisper.rs` 中的 VAD 流水线：

```
detect_regions (fast-vad 全量扫)
 → detect_energy_regions (再全量扫一次 + 排序)
 → intersect_regions
 → split_long_regions_by_energy   ← 外层调用了一次
 → plan_chunks                    ← 内部又调用了一次 split_long_regions_by_energy
```

`src-tauri/src/asr/vad.rs` 里 `plan_chunks` 内部又跑了一次 `split_long_regions_by_energy`，对 1h 音频会多扫一轮能量帧。相对推理算不上大头，但属于"白给"的开销。

### 5. 音频抽取：ffmpeg-next 单线程 + 每帧 Rust 层拷贝

`src-tauri/src/audio/extractor.rs`：

```rust
for (stream, packet) in ictx.packets() {
    if stream.index() == audio_idx {
        decoder.send_packet(&packet)?;
        while decoder.receive_frame(&mut decoded_frame).is_ok() {
            resampler.run(&decoded_frame, &mut resampled_frame)?;
            let data = resampled_frame.data(0);
            let samples = data.chunks_exact(2).filter_map(|chunk| {
                let bytes = [chunk[0], chunk[1]];
                Some(i16::from_le_bytes(bytes))
            });
            pcm_data.extend(samples);        // 每帧一次 Rust 闭包 + extend
            ...
        }
    }
}
```

对比 CLI `ffmpeg -i in.mp4 -vn -ac 1 -ar 16000 out.wav`：

1. **没有给 decoder 设置 `threads = auto`**。CLI 默认开多线程解码 H.264/AAC，工程里是单线程。
2. `ictx.packets()` 逐 packet 读取，**视频 packet 也被迭代**（只是不解码），依然走 demux。CLI 的 `-vn` 能省掉这部分。
3. resample 后对每个 i16 都过 Rust 闭包 + 边界检查，比 swresample 的 SIMD 批量拷贝慢。
4. 末尾 `write_wav_file` 里 `samples.iter().flat_map(|s| s.to_le_bytes()).collect()` 把 `Vec<i16>` 又展平成 `Vec<u8>` 再 `write_all`，多一次分配 + 拷贝。

### 6. Whisper 阶段又读 WAV 再做一次格式转换

`src-tauri/src/asr/whisper.rs` 的 `read_wav_samples`：

- 整个 WAV 一次性 `read_exact` 进 `Vec<u8>`
- 再遍历成 `Vec<f32>`（两份内存）
- `simple_resample` 是**纯标量线性插值**，没有 SIMD、没有抗混叠滤波；正常路径下 16kHz == 16kHz 不会走到，但若 extractor 意外落了非 16k 采样，就成为瓶颈

相比之下，脚本路径是 `ffmpeg -ar 16000 -ac 1 out.wav` → `whisper-cli -f out.wav`，whisper 内部直接 mmap，没有 Rust 侧的二次拷贝。

### 7. 每个 chunk 都 `dup2 stderr → /dev/null`

`src-tauri/src/asr/whisper.rs` 的 `StderrGuard`：每段构造 + 销毁 = 4 个 syscall。量不大，但在短音频、几十个 chunk 时仍是可见噪声。不是主因。

---

## 影响估算

| # | 问题 | 预期加速比（修掉后） |
|---|-------|---|
| 1 | 未开 metal/coreml/openblas feature | **2–5×** |
| 2 | `n_threads = num_cpus::get()` | 10–30% |
| 3 | VAD overlap 10s/块 + 每块重建 state + `no_context` | 20–40% |
| 4 | `plan_chunks` 重复 `split` + fallback 重算 | <10% 常态，最坏 2× |
| 5 | ffmpeg-next 单线程 decode，无 `threads=auto` | 抽音频阶段 1.5–3× |
| 6 | 落盘 WAV → 再 read → 再转 f32 | 秒级 |
| 7 | `StderrGuard` 每块 dup2 | 可忽略 |

---

## 建议验证 / 修复步骤（由低风险到高风险）

### 步骤 1：开启加速特性（最该先做）

macOS：

```bash
cd src-tauri
cargo build --release --features metal,coreml
```

如果不想依赖 CoreML，只开 Metal 也行：

```bash
cargo build --release --features metal
```

或退而求其次用 BLAS：

```bash
cargo build --release --features openblas
```

只改这一条，用工程已有的日志 `speed={:.2}x realtime (RTF {:.2})`（`whisper.rs` 的 `Chunk X/Y done` 行）对比修改前后即可确认是否接近脚本速度。

### 步骤 2：把线程数限制为 P-core 数

`src-tauri/src/pipeline/orchestrator.rs`：

```rust
n_threads: num_cpus::get_physical().min(8) as u32,
```

或者干脆硬编码 `n_threads: 4`，与 CLI 默认一致后再对比。

### 步骤 3：验证 VAD 是不是拖累

临时把 `src-tauri/src/asr/engine.rs` 的默认 `enable_vad` 改成 `false`，或把 `vad_max_chunk_ms` 调到 `30 * 60 * 1000`（等价于单块），再跑一次同一视频，看 ASR 总耗时是否大幅缩短。如果缩短明显，说明 VAD 规划本身是主要拖慢点，再考虑：

- 减小 `vad_overlap_ms`（例如 2000）
- 不要在 `plan_chunks` 内部再调一次 `split_long_regions_by_energy`
- 移除或精简 fallback 二次推理

### 步骤 4：音频抽取优化（视情况）

- 给解码器设置 `threads = auto`（`ffmpeg-next` 支持 `codec_ctx.set_threading(...)`）
- resample 后用 `resampled_frame.plane::<i16>(0)` 或 `bytemuck` 直接一次性 `extend_from_slice`，避免 per-sample 闭包
- `write_wav_file` 用 `bytemuck::cast_slice(samples)` 零拷贝写

### 步骤 5：分阶段计时对齐

- 在 `AudioExtractor::extract` 入口/出口、`transcribe` 入口/出口分别打印 `Instant::now() - start`
- 和脚本 `time ffmpeg ...` / `time whisper-cli ...` 分别对齐
- 最终定位差距最大的阶段，再针对性优化

---

## 预期结论

`#1 + #2` 修完后，工程耗时应当就能接近脚本水准；`#3 / #5` 是后续进一步优化空间。

