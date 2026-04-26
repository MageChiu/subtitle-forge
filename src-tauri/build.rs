use std::{
    env, fs,
    io,
    path::{Path, PathBuf},
};

fn main() {
    #[cfg(target_os = "windows")]
    copy_ffmpeg_runtime_dlls().unwrap_or_else(|err| {
        panic!("Failed to prepare FFmpeg runtime DLLs: {err}");
    });

    #[cfg(target_os = "windows")]
    prepare_optional_windows_runtimes().unwrap_or_else(|err| {
        panic!("Failed to prepare optional Windows runtimes: {err}");
    });

    tauri_build::build()
}

#[cfg(target_os = "windows")]
fn copy_ffmpeg_runtime_dlls() -> io::Result<()> {
    println!("cargo:rerun-if-env-changed=VCPKG_ROOT");
    println!("cargo:rerun-if-env-changed=VCPKG_DEFAULT_TRIPLET");
    println!("cargo:rerun-if-env-changed=FFMPEG_DIR");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap_or_default());
    let target_dir = find_target_profile_dir(&out_dir).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("Unable to resolve target dir from OUT_DIR: {}", out_dir.display()),
        )
    })?;

    let mut candidates = ffmpeg_bin_candidates();
    candidates.retain(|path| path.exists());

    if candidates.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "No FFmpeg DLL directory found. Checked VCPKG_ROOT/VCPKG_DEFAULT_TRIPLET, FFMPEG_DIR, and common Windows install paths.",
        ));
    }

    for candidate in &candidates {
        println!("cargo:warning=Checking FFmpeg runtime DLL directory: {}", candidate.display());
    }

    let mut copied = Vec::new();
    for dll_dir in candidates {
        copy_dlls_from_dir(&dll_dir, &target_dir, &mut copied)?;
    }

    let missing = missing_required_dlls(&copied);
    if !missing.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Missing required FFmpeg runtime DLLs in {}: {}",
                target_dir.display(),
                missing.join(", ")
            ),
        ));
    }

    println!(
        "cargo:warning=Prepared required FFmpeg runtime DLLs: {}",
        REQUIRED_FFMPEG_DLLS.join(", ")
    );

    Ok(())
}

#[cfg(target_os = "windows")]
fn prepare_optional_windows_runtimes() -> io::Result<()> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap_or_default());
    let target_dir = find_target_profile_dir(&out_dir).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("Unable to resolve target dir from OUT_DIR: {}", out_dir.display()),
        )
    })?;

    if cargo_feature_enabled("OFFLINE_TRANSLATE") {
        copy_required_runtime_from_candidates(
            &target_dir,
            &onnxruntime_candidates(),
            "onnxruntime.dll",
            "ONNX Runtime",
        )?;
    }

    if cargo_feature_enabled("CUDA") {
        println!(
            "cargo:warning=Feature `cuda` is enabled. Verify NVIDIA CUDA runtime DLLs are available on the target Windows machine."
        );
    }

    if cargo_feature_enabled("OPENBLAS") {
        println!(
            "cargo:warning=Feature `openblas` is enabled. Verify OpenBLAS runtime DLLs are available on the target Windows machine."
        );
    }

    Ok(())
}

#[cfg(target_os = "windows")]
const REQUIRED_FFMPEG_DLLS: &[&str] = &[
    "avcodec-62.dll",
    "avutil-60.dll",
    "avformat-62.dll",
    "swresample-6.dll",
];

#[cfg(target_os = "windows")]
fn missing_required_dlls(copied: &[String]) -> Vec<String> {
    REQUIRED_FFMPEG_DLLS
        .iter()
        .filter(|required| !copied.iter().any(|name| name.eq_ignore_ascii_case(required)))
        .map(|dll| (*dll).to_string())
        .collect()
}

#[cfg(target_os = "windows")]
fn copy_dlls_from_dir(
    source_dir: &Path,
    target_dir: &Path,
    copied: &mut Vec<String>,
) -> io::Result<()> {
    let required_in_dir = REQUIRED_FFMPEG_DLLS
        .iter()
        .map(|dll| source_dir.join(dll))
        .filter(|path| path.exists())
        .collect::<Vec<_>>();

    for path in required_in_dir {
        let file_name = path.file_name().and_then(|name| name.to_str()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid DLL file name: {}", path.display()),
            )
        })?;
        let destination = target_dir.join(file_name);
        fs::copy(&path, &destination)?;
        println!(
            "cargo:warning=Copied required runtime DLL {} -> {}",
            path.display(),
            destination.display()
        );
        copied.push(file_name.to_string());
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn copy_required_runtime_from_candidates(
    target_dir: &Path,
    candidates: &[PathBuf],
    dll_name: &str,
    runtime_label: &str,
) -> io::Result<()> {
    let target_path = target_dir.join(dll_name);
    if target_path.exists() {
        println!(
            "cargo:warning={} already available at {}",
            runtime_label,
            target_path.display()
        );
        return Ok(());
    }

    for candidate in candidates {
        let candidate_file = candidate.join(dll_name);
        if !candidate_file.exists() {
            continue;
        }

        fs::copy(&candidate_file, &target_path)?;
        println!(
            "cargo:warning=Copied {} DLL {} -> {}",
            runtime_label,
            candidate_file.display(),
            target_path.display()
        );
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "{} DLL `{}` not found. Checked: {}",
            runtime_label,
            dll_name,
            candidates
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    ))
}

#[cfg(target_os = "windows")]
fn ffmpeg_bin_candidates() -> Vec<PathBuf> {
    let triplet = env::var("VCPKG_DEFAULT_TRIPLET").unwrap_or_else(|_| "x64-windows".into());
    let mut candidates = Vec::new();

    if let Ok(vcpkg_root) = env::var("VCPKG_ROOT") {
        candidates.push(Path::new(&vcpkg_root).join("installed").join(&triplet).join("bin"));
    }

    if let Ok(ffmpeg_dir) = env::var("FFMPEG_DIR") {
        let ffmpeg_path = PathBuf::from(&ffmpeg_dir);
        candidates.push(ffmpeg_path.join("bin"));
        candidates.push(ffmpeg_path);
    }

    candidates.push(PathBuf::from(r"C:\Program Files\FFmpeg\bin"));
    candidates.push(PathBuf::from(
        r"C:\ProgramData\chocolatey\lib\ffmpeg\tools\ffmpeg\bin",
    ));

    candidates
}

#[cfg(target_os = "windows")]
fn onnxruntime_candidates() -> Vec<PathBuf> {
    println!("cargo:rerun-if-env-changed=ORT_DYLIB_PATH");
    println!("cargo:rerun-if-env-changed=ORT_LIB_LOCATION");
    println!("cargo:rerun-if-env-changed=ORT_RUST_LIB_LOCATION");

    let mut candidates = Vec::new();

    for var_name in ["ORT_DYLIB_PATH", "ORT_LIB_LOCATION", "ORT_RUST_LIB_LOCATION"] {
        if let Ok(value) = env::var(var_name) {
            let path = PathBuf::from(value);
            if path.is_file() {
                if let Some(parent) = path.parent() {
                    candidates.push(parent.to_path_buf());
                }
            } else {
                candidates.push(path.clone());
                candidates.push(path.join("lib"));
                candidates.push(path.join("bin"));
            }
        }
    }

    candidates
}

#[cfg(target_os = "windows")]
fn cargo_feature_enabled(feature_name: &str) -> bool {
    env::var_os(format!("CARGO_FEATURE_{feature_name}")).is_some()
}

#[cfg(target_os = "windows")]
fn find_target_profile_dir(out_dir: &Path) -> Option<PathBuf> {
    out_dir.ancestors().find_map(|ancestor| {
        let profile = ancestor.file_name()?.to_str()?;
        if matches!(profile, "debug" | "release") {
            Some(ancestor.to_path_buf())
        } else {
            None
        }
    })
}
