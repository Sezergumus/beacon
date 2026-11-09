use crate::error::{AppError, AppResult};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use sysinfo::System;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub total_ram_mb: u64,
    pub free_ram_mb: u64,
    pub recommended_ram_mb: u64,
    pub cpu_cores: usize,
    pub cpu_brand: String,
    pub os_name: String,
    pub arch: String,
    pub gc_profile: String,
    pub jvm_flags: Vec<String>,
    pub java_installed: bool,
    pub java_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JvmProgressEvent {
    pub stage: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percent: f32,
}

/// Detect system hardware and compute optimal RAM & JVM GC flags.
pub fn detect_hardware_profile(app_data_dir: &Path, custom_ram_mb: Option<u64>, java_major: u8) -> HardwareProfile {
    let mut sys = System::new_all();
    sys.refresh_memory();
    sys.refresh_cpu_all();

    let total_ram_mb = sys.total_memory() / (1024 * 1024);
    let available_mem = sys.available_memory();
    let free_ram_mb = if available_mem > 0 {
        available_mem / (1024 * 1024)
    } else {
        (sys.total_memory().saturating_sub(sys.used_memory())) / (1024 * 1024)
    };

    let recommended_ram_mb = calculate_optimal_ram_mb(free_ram_mb);
    let effective_ram_mb = custom_ram_mb.unwrap_or(recommended_ram_mb);

    let cpu_cores = sys.cpus().len().max(1);
    let cpu_brand = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Multi-Core Processor".to_string());

    let (os_name, arch) = adoptium_os_arch();
    let jvm_flags = generate_jvm_flags(effective_ram_mb, java_major);
    let gc_profile = if effective_ram_mb >= 6144 && java_major >= 21 {
        "Generational ZGC (Ultra-Low Pause >= 6GB)".to_string()
    } else {
        "Aikar's G1GC Optimized Flags".to_string()
    };

    let runtime_dir = app_data_dir.join("runtimes").join(format!("java-{}", java_major));
    let java_bin = find_java_binary_in_dir(&runtime_dir);

    HardwareProfile {
        total_ram_mb,
        free_ram_mb,
        recommended_ram_mb,
        cpu_cores,
        cpu_brand,
        os_name: os_name.to_string(),
        arch: arch.to_string(),
        gc_profile,
        jvm_flags,
        java_installed: java_bin.is_some(),
        java_path: java_bin.map(|p| p.to_string_lossy().to_string()),
    }
}

/// Allocate `clamp(free_ram * 0.65, 2048MB, 10240MB)` per architectural specification.
pub fn calculate_optimal_ram_mb(free_ram_mb: u64) -> u64 {
    let raw = (free_ram_mb as f64 * 0.65).round() as u64;
    raw.clamp(2048, 10240)
}

/// Generate hardware-tuned JVM arguments:
/// - If allocated RAM >= 6GB (6144MB) and Java 21+, inject Generational ZGC.
/// - Otherwise, inject standard Aikar's Flags.
pub fn generate_jvm_flags(allocated_ram_mb: u64, java_major: u8) -> Vec<String> {
    let mut flags = vec![
        format!("-Xms{}M", allocated_ram_mb),
        format!("-Xmx{}M", allocated_ram_mb),
    ];

    if allocated_ram_mb >= 6144 && java_major >= 21 {
        // Generational ZGC for 6GB+ RAM on Java 21+
        flags.push("-XX:+UseZGC".to_string());
        // Java 21-23 need to be told to use the generational mode; 24+ only has it.
        if java_major < 24 {
            flags.push("-XX:+ZGenerational".to_string());
        }
        flags.extend([
            "-XX:+AlwaysPreTouch".to_string(),
            "-XX:+DisableExplicitGC".to_string(),
            "-XX:+ParallelRefProcEnabled".to_string(),
            "-XX:+PerfDisableSharedMem".to_string(),
        ]);
    } else {
        // Standard Aikar's G1GC Flags
        flags.extend([
            "-XX:+UseG1GC".to_string(),
            "-XX:+ParallelRefProcEnabled".to_string(),
            "-XX:MaxGCPauseMillis=200".to_string(),
            "-XX:+UnlockExperimentalVMOptions".to_string(),
            "-XX:+DisableExplicitGC".to_string(),
            "-XX:+AlwaysPreTouch".to_string(),
            "-XX:G1NewSizePercent=30".to_string(),
            "-XX:G1MaxNewSizePercent=40".to_string(),
            "-XX:G1HeapRegionSize=8M".to_string(),
            "-XX:G1ReservePercent=20".to_string(),
            "-XX:G1HeapWastePercent=5".to_string(),
            "-XX:G1MixedGCCountTarget=4".to_string(),
            "-XX:InitiatingHeapOccupancyPercent=15".to_string(),
            "-XX:G1MixedGCLiveThresholdPercent=90".to_string(),
            "-XX:G1RSetUpdatingPauseTimePercent=5".to_string(),
            "-XX:SurvivorRatio=32".to_string(),
            "-XX:+PerfDisableSharedMem".to_string(),
            "-XX:MaxTenuringThreshold=1".to_string(),
            "-Dusing.aikars.flags=https://mcflags.emc.gs".to_string(),
            "-Daikars.new.flags=true".to_string(),
        ]);
    }

    flags
}

pub fn adoptium_os_arch() -> (&'static str, &'static str) {
    let os = match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "mac",
        _ => "linux",
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "aarch64",
        _ => "x64",
    };
    (os, arch)
}

/// Recursively locate `bin/java.exe` or `bin/java` inside the portable runtime directory.
pub fn find_java_binary_in_dir(dir: &Path) -> Option<PathBuf> {
    if !dir.exists() {
        return None;
    }
    let bin_name = if cfg!(windows) { "java.exe" } else { "java" };

    // Direct check
    let direct = dir.join("bin").join(bin_name);
    if direct.is_file() {
        return Some(direct);
    }

    // One or two levels nested (e.g., jdk-21.0.5+11-jre/bin/java.exe or Contents/Home/bin/java)
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let candidate = path.join("bin").join(bin_name);
                if candidate.is_file() {
                    return Some(candidate);
                }
                let mac_candidate = path.join("Contents").join("Home").join("bin").join(bin_name);
                if mac_candidate.is_file() {
                    return Some(mac_candidate);
                }
            }
        }
    }
    None
}

/// Ensure portable Java runtime (Java 21 or Java 17) exists in `app_data_dir/runtimes/java-{version}`.
/// If missing, silently downloads from Adoptium v3 API and extracts it inside `spawn_blocking`.
pub async fn ensure_portable_jre(
    app_data_dir: &Path,
    java_major: u8,
    app_handle: Option<&AppHandle>,
) -> AppResult<PathBuf> {
    let runtime_dir = app_data_dir
        .join("runtimes")
        .join(format!("java-{}", java_major));

    if let Some(existing) = find_java_binary_in_dir(&runtime_dir) {
        return Ok(existing);
    }

    tokio::fs::create_dir_all(&runtime_dir).await?;

    let (os, arch) = adoptium_os_arch();
    let url = format!(
        "https://api.adoptium.net/v3/binary/latest/{}/ga/{}/{}/jre/hotspot/normal/eclipse",
        java_major, os, arch
    );

    if let Some(app) = app_handle {
        let _ = app.emit(
            "jvm:download-progress",
            JvmProgressEvent {
                stage: format!("Fetching Portable Java {} from Adoptium...", java_major),
                downloaded_bytes: 0,
                total_bytes: 0,
                percent: 0.0,
            },
        );
    }

    let client = reqwest::Client::builder()
        .user_agent("Beacon/0.1.0 (https://github.com/Sezergumus/beacon)")
        .build()?;

    let response = client.get(&url).send().await?;
    if !response.status().is_success() {
        return Err(AppError::Jvm(format!(
            "Adoptium API returned HTTP {} when provisioning Java {}",
            response.status(),
            java_major
        )));
    }

    let total_bytes = response.content_length().unwrap_or(50_000_000);
    let archive_ext = if os == "windows" { "zip" } else { "tar.gz" };
    let archive_path = runtime_dir.join(format!("adoptium-jre-{}.{}", java_major, archive_ext));

    let mut file = fs::File::create(&archive_path)?;
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();
    let mut last_emit = std::time::Instant::now();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;

        if last_emit.elapsed().as_millis() >= 120 {
            last_emit = std::time::Instant::now();
            if let Some(app) = app_handle {
                let percent = ((downloaded as f64 / total_bytes.max(1) as f64) * 100.0) as f32;
                let _ = app.emit(
                    "jvm:download-progress",
                    JvmProgressEvent {
                        stage: format!("Downloading Portable Java {}...", java_major),
                        downloaded_bytes: downloaded,
                        total_bytes,
                        percent: percent.clamp(0.0, 99.0),
                    },
                );
            }
        }
    }
    file.flush()?;
    drop(file);

    if let Some(app) = app_handle {
        let _ = app.emit(
            "jvm:download-progress",
            JvmProgressEvent {
                stage: format!("Extracting Portable Java {} runtime...", java_major),
                downloaded_bytes: downloaded,
                total_bytes,
                percent: 99.0,
            },
        );
    }

    let archive_clone = archive_path.clone();
    let runtime_dir_clone = runtime_dir.clone();
    let is_zip = archive_ext == "zip";

    tokio::task::spawn_blocking(move || -> AppResult<()> {
        if is_zip {
            let reader = fs::File::open(&archive_clone)?;
            let mut zip = zip::ZipArchive::new(reader)?;
            for i in 0..zip.len() {
                let mut entry = zip.by_index(i)?;
                let Some(enclosed) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
                    continue;
                };
                let out_path = runtime_dir_clone.join(enclosed);
                if entry.is_dir() {
                    fs::create_dir_all(&out_path)?;
                } else {
                    if let Some(parent) = out_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    let mut outfile = fs::File::create(&out_path)?;
                    std::io::copy(&mut entry, &mut outfile)?;
                }
            }
        } else {
            let tar_gz = fs::File::open(&archive_clone)?;
            let tar = flate2::read::GzDecoder::new(tar_gz);
            let mut archive = tar::Archive::new(tar);
            archive.unpack(&runtime_dir_clone)?;
        }
        let _ = fs::remove_file(&archive_clone);
        Ok(())
    })
    .await
    .map_err(|e| AppError::Jvm(format!("Join error during JRE extraction: {}", e)))??;

    let java_bin = find_java_binary_in_dir(&runtime_dir).ok_or_else(|| {
        AppError::Jvm(format!(
            "Extracted Adoptium Java {} archive, but could not locate bin/java executable",
            java_major
        ))
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(&java_bin) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&java_bin, perms);
        }
    }

    if let Some(app) = app_handle {
        let _ = app.emit(
            "jvm:download-progress",
            JvmProgressEvent {
                stage: format!("Portable Java {} Ready", java_major),
                downloaded_bytes: total_bytes,
                total_bytes,
                percent: 100.0,
            },
        );
    }

    Ok(java_bin)
}
