use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AsrFeatureStatus {
    pub key: String,
    pub label: String,
    pub enabled: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AsrRuntimeCapabilities {
    pub app_version: String,
    pub whisper_backend: String,
    pub gpu_backend_available: bool,
    pub enabled_gpu_backends: Vec<String>,
    pub features: Vec<AsrFeatureStatus>,
}

pub fn metal_enabled() -> bool {
    cfg!(target_os = "macos") || cfg!(feature = "metal")
}

pub fn coreml_enabled() -> bool {
    cfg!(feature = "coreml")
}

pub fn cuda_enabled() -> bool {
    cfg!(feature = "cuda")
}

pub fn hipblas_enabled() -> bool {
    cfg!(feature = "hipblas")
}

pub fn opencl_enabled() -> bool {
    cfg!(feature = "opencl")
}

pub fn openblas_enabled() -> bool {
    cfg!(feature = "openblas")
}

pub fn gpu_backend_available() -> bool {
    metal_enabled()
        || coreml_enabled()
        || cuda_enabled()
        || hipblas_enabled()
        || opencl_enabled()
}

pub fn enabled_gpu_backends() -> Vec<String> {
    let mut backends = Vec::new();

    if metal_enabled() {
        backends.push("metal".to_string());
    }
    if coreml_enabled() {
        backends.push("coreml".to_string());
    }
    if cuda_enabled() {
        backends.push("cuda".to_string());
    }
    if hipblas_enabled() {
        backends.push("hipblas".to_string());
    }
    if opencl_enabled() {
        backends.push("opencl".to_string());
    }

    backends
}

pub fn runtime_capabilities() -> AsrRuntimeCapabilities {
    let gpu_backends = enabled_gpu_backends();
    let mut features = Vec::new();

    features.push(AsrFeatureStatus {
        key: "metal".into(),
        label: "Metal".into(),
        enabled: metal_enabled(),
        detail: if cfg!(target_os = "macos") {
            "macOS 构建默认包含 Metal Whisper 后端".into()
        } else {
            "Whisper GPU 后端（Apple Metal）".into()
        },
    });
    features.push(AsrFeatureStatus {
        key: "coreml".into(),
        label: "Core ML".into(),
        enabled: coreml_enabled(),
        detail: "Apple Neural Engine / Core ML 推理加速".into(),
    });
    features.push(AsrFeatureStatus {
        key: "cuda".into(),
        label: "CUDA".into(),
        enabled: cuda_enabled(),
        detail: "NVIDIA GPU 推理加速".into(),
    });
    features.push(AsrFeatureStatus {
        key: "hipblas".into(),
        label: "hipBLAS".into(),
        enabled: hipblas_enabled(),
        detail: "AMD GPU 推理加速".into(),
    });
    features.push(AsrFeatureStatus {
        key: "opencl".into(),
        label: "OpenCL".into(),
        enabled: opencl_enabled(),
        detail: "通用 OpenCL GPU 推理加速".into(),
    });
    features.push(AsrFeatureStatus {
        key: "openblas".into(),
        label: "OpenBLAS".into(),
        enabled: openblas_enabled(),
        detail: "CPU BLAS 加速，提升部分矩阵计算性能".into(),
    });
    features.push(AsrFeatureStatus {
        key: "thread_control".into(),
        label: "线程控制".into(),
        enabled: true,
        detail: "支持在设置中调整 ASR 推理线程数".into(),
    });

    AsrRuntimeCapabilities {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        whisper_backend: "whisper.cpp".into(),
        gpu_backend_available: !gpu_backends.is_empty(),
        enabled_gpu_backends: gpu_backends,
        features,
    }
}
