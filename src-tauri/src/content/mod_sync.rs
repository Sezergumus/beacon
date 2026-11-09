use crate::content::modrinth::{compute_file_sha512, download_file_sha512};
use crate::error::{AppError, AppResult};
use iroh::NodeAddr;
use iroh_blobs::ticket::BlobTicket;
use iroh_blobs::{BlobFormat, Hash as BlobHash};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModManifestEntry {
    pub filename: String,
    pub relative_path: String, // e.g., "mods/lithium.jar" or "config/lithium.properties"
    pub size_bytes: u64,
    pub sha512: String,
    pub blake3_hash: String,
    pub modrinth_download_url: Option<String>,
    pub iroh_blob_ticket: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerModManifest {
    pub mc_version: String,
    pub loader: String, // "fabric" | "neoforge" | "vanilla"
    pub mods: Vec<ModManifestEntry>,
    pub generated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModSyncStatusEvent {
    pub total_mods: usize,
    pub synced_mods: usize,
    pub current_mod: String,
    pub source: String, // "Modrinth CDN" | "Iroh P2P Blob" | "Local Cache"
    pub completed: bool,
}

/// Build the Host's `ServerModManifest` by scanning `server/mods`, resolving Modrinth CDN URLs via SHA-512,
/// and generating BLAKE3 `iroh_blobs::ticket::BlobTicket`s for P2P fallback.
pub async fn build_server_mod_manifest(
    server_dir: &Path,
    mc_version: &str,
    loader: &str,
    host_node_addr: Option<&NodeAddr>,
) -> AppResult<ServerModManifest> {
    let mods_dir = server_dir.join("mods");
    let _ = tokio::fs::create_dir_all(&mods_dir).await;

    let mods_dir_clone = mods_dir.clone();
    let host_addr_clone = host_node_addr.cloned();

    let mut entries: Vec<ModManifestEntry> =
        tokio::task::spawn_blocking(move || -> AppResult<Vec<ModManifestEntry>> {
            let mut list = Vec::new();
            if let Ok(read_dir) = fs::read_dir(&mods_dir_clone) {
                for entry in read_dir.flatten() {
                    let path = entry.path();
                    if path.is_file()
                        && path.extension().and_then(|e| e.to_str()) == Some("jar")
                    {
                        let filename = entry.file_name().to_string_lossy().to_string();
                        let bytes = fs::read(&path)?;
                        let size_bytes = bytes.len() as u64;

                        let mut sha = Sha512::new();
                        sha.update(&bytes);
                        let sha512 = hex::encode(sha.finalize());

                        let b3 = blake3::hash(&bytes);
                        let blake3_hash = b3.to_hex().to_string();

                        let ticket_str = host_addr_clone.as_ref().and_then(|addr| {
                            let blob_hash = BlobHash::from_bytes(*b3.as_bytes());
                            BlobTicket::new(addr.clone(), blob_hash, BlobFormat::Raw)
                                .ok()
                                .map(|t| t.to_string())
                        });

                        list.push(ModManifestEntry {
                            relative_path: format!("mods/{}", filename),
                            filename,
                            size_bytes,
                            sha512,
                            blake3_hash,
                            modrinth_download_url: None,
                            iroh_blob_ticket: ticket_str,
                        });
                    }
                }
            }
            Ok(list)
        })
        .await
        .map_err(|e| AppError::Content(e.to_string()))??;

    // Resolve Modrinth CDN URLs by SHA-512 hash where available
    if !entries.is_empty() {
        let client = reqwest::Client::builder()
            .user_agent("AetherMC/0.1.0 (https://github.com/aethermc)")
            .build()?;

        for item in &mut entries {
            let lookup_url = format!(
                "https://api.modrinth.com/v2/version_file/{}?algorithm=sha512",
                item.sha512
            );
            if let Ok(resp) = client.get(&lookup_url).send().await {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if let Some(files) = json["files"].as_array() {
                            if let Some(matched) = files
                                .iter()
                                .find(|f| f["hashes"]["sha512"].as_str() == Some(&item.sha512))
                                .or_else(|| files.first())
                            {
                                item.modrinth_download_url =
                                    matched["url"].as_str().map(|s| s.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    Ok(ServerModManifest {
        mc_version: mc_version.to_string(),
        loader: loader.to_string(),
        mods: entries,
        generated_at_ms: now_ms,
    })
}

/// Diff the Host's `ServerModManifest` against the local Client instance folder and return missing/mismatched mods.
pub async fn diff_client_mods(
    manifest: &ServerModManifest,
    client_instance_dir: &Path,
) -> AppResult<Vec<ModManifestEntry>> {
    let mut missing = Vec::new();
    for entry in &manifest.mods {
        let local_path = client_instance_dir.join(&entry.relative_path);
        if !local_path.exists() {
            missing.push(entry.clone());
            continue;
        }
        match compute_file_sha512(&local_path).await {
            Ok(local_sha512) if local_sha512.eq_ignore_ascii_case(&entry.sha512) => {}
            _ => missing.push(entry.clone()),
        }
    }
    Ok(missing)
}

/// Pre-Join Hook: Sync all required mods into the Peer's isolated profile directory.
/// - Downloads standard mods from Modrinth CDN in parallel (12 streams) with SHA-512 verification.
/// - Pulls custom non-CDN mods or overrides directly from the Host via Iroh P2P stream with BLAKE3 verification.
pub async fn sync_client_mods_with_host(
    manifest: &ServerModManifest,
    client_instance_dir: &Path,
    endpoint: Option<&iroh::Endpoint>,
    host_addr: Option<&NodeAddr>,
    app_handle: Option<&AppHandle>,
) -> AppResult<PathBuf> {
    tokio::fs::create_dir_all(client_instance_dir.join("mods")).await?;

    let missing = diff_client_mods(manifest, client_instance_dir).await?;
    let total = missing.len();

    if total == 0 {
        if let Some(app) = app_handle {
            let _ = app.emit(
                "network:mod-sync",
                ModSyncStatusEvent {
                    total_mods: manifest.mods.len(),
                    synced_mods: manifest.mods.len(),
                    current_mod: "All mods verified up-to-date".to_string(),
                    source: "Local Cache".to_string(),
                    completed: true,
                },
            );
        }
        return Ok(client_instance_dir.to_path_buf());
    }

    let semaphore = Arc::new(Semaphore::new(12));
    let client = Arc::new(
        reqwest::Client::builder()
            .user_agent("AetherMC/0.1.0 (https://github.com/aethermc)")
            .build()?,
    );
    let synced_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let mut cdn_tasks = Vec::new();
    let mut p2p_entries = Vec::new();

    for entry in missing {
        if let Some(ref url) = entry.modrinth_download_url {
            let sem = semaphore.clone();
            let http = client.clone();
            let dest = client_instance_dir.join(&entry.relative_path);
            let url_clone = url.clone();
            let sha = entry.sha512.clone();
            let fname = entry.filename.clone();
            let counter = synced_counter.clone();
            let app_opt = app_handle.cloned();

            cdn_tasks.push(tokio::spawn(async move {
                let _permit = sem
                    .acquire()
                    .await
                    .map_err(|e| AppError::Content(e.to_string()))?;
                download_file_sha512(&http, &url_clone, &dest, Some(&sha)).await?;
                let done = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                if let Some(app) = app_opt {
                    let _ = app.emit(
                        "network:mod-sync",
                        ModSyncStatusEvent {
                            total_mods: total,
                            synced_mods: done,
                            current_mod: fname,
                            source: "Modrinth CDN".to_string(),
                            completed: false,
                        },
                    );
                }
                Ok::<(), AppError>(())
            }));
        } else {
            p2p_entries.push(entry);
        }
    }

    for task in cdn_tasks {
        task.await
            .map_err(|e| AppError::Content(format!("Mod sync task join error: {}", e)))??;
    }

    // Pull remaining custom/non-CDN mods directly from Host over Iroh P2P with BLAKE3 verification
    if !p2p_entries.is_empty() {
        if let (Some(ep), Some(addr)) = (endpoint, host_addr) {
            for entry in p2p_entries {
                let dest = client_instance_dir.join(&entry.relative_path);
                crate::network::iroh_node::fetch_blob_from_host(
                    ep,
                    addr,
                    &entry.relative_path,
                    &entry.blake3_hash,
                    &dest,
                )
                .await?;

                let done =
                    synced_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "network:mod-sync",
                        ModSyncStatusEvent {
                            total_mods: total,
                            synced_mods: done,
                            current_mod: entry.filename,
                            source: "Iroh P2P Blob".to_string(),
                            completed: false,
                        },
                    );
                }
            }
        }
    }

    if let Some(app) = app_handle {
        let _ = app.emit(
            "network:mod-sync",
            ModSyncStatusEvent {
                total_mods: total,
                synced_mods: total,
                current_mod: "Mod synchronization complete".to_string(),
                source: "Verified".to_string(),
                completed: true,
            },
        );
    }

    Ok(client_instance_dir.to_path_buf())
}
