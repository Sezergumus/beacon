use crate::engine::runner::{ServerCore, ServerLaunchConfig, ServerRunner};
use crate::error::{AppError, AppResult};
use crate::network::room_code::publish_room_to_dht;
use iroh::NodeAddr;
use iroh_blobs::ticket::BlobTicket;
use iroh_blobs::{BlobFormat, Hash as BlobHash};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotInfo {
    pub snapshot_id: String,
    pub filename: String,
    pub size_bytes: u64,
    pub blake3_hash: String,
    pub blob_ticket: Option<String>,
    pub created_at_ms: u64,
    pub mc_version: String,
    pub loader: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationEvent {
    pub stage: String,
    pub room_code: String,
    pub new_host_node_id: String,
    pub message: String,
}

/// Create a `.tar.zst` world snapshot (`save-off` -> `save-all flush` -> zstd archive -> `save-on`)
/// packaging `world/`, `mods/`, and `server.properties` inside `tokio::task::spawn_blocking`.
pub async fn create_world_snapshot(
    runner: &ServerRunner,
    host_node_addr: Option<&NodeAddr>,
    app_handle: Option<&AppHandle>,
) -> AppResult<SnapshotInfo> {
    // 1. If server is running, flush chunks safely before snapshotting
    let is_running = *runner.status.lock().await == crate::engine::runner::ServerStatus::Running;
    if is_running {
        let _ = runner.send_command("save-off").await;
        let _ = runner.send_command("save-all flush").await;
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    }

    let server_dir = runner.server_dir().clone();
    let snapshots_dir = runner.app_data_dir.join("snapshots");
    tokio::fs::create_dir_all(&snapshots_dir).await?;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let snapshot_id = format!("snapshot-{}", now_ms);
    let filename = format!("{}.tar.zst", snapshot_id);
    let archive_path = snapshots_dir.join(&filename);
    let archive_path_clone = archive_path.clone();
    let server_dir_clone = server_dir.clone();

    let (size_bytes, blake3_bytes) =
        tokio::task::spawn_blocking(move || -> AppResult<(u64, [u8; 32])> {
            let out_file = fs::File::create(&archive_path_clone)?;
            // Compress with zstd level 3 (ultra-fast real-time snapshot speed)
            let encoder = zstd::Encoder::new(out_file, 3)?;
            let mut tar_builder = tar::Builder::new(encoder);

            let prop_path = server_dir_clone.join("server.properties");
            if prop_path.exists() {
                tar_builder.append_path_with_name(&prop_path, "server.properties")?;
            }

            // The world in use goes into the archive as `world/`, whatever its folder is called.
            let world_dir = crate::engine::worlds::level_dir(&server_dir_clone);
            if world_dir.exists() {
                append_dir_filtered(&mut tar_builder, &world_dir, Path::new("world"))?;
            }

            let mods_dir = server_dir_clone.join("mods");
            if mods_dir.exists() {
                append_dir_filtered(&mut tar_builder, &mods_dir, Path::new("mods"))?;
            }

            let encoder = tar_builder.into_inner()?;
            let mut out_file = encoder.finish()?;
            std::io::Write::flush(&mut out_file)?;
            drop(out_file);

            let bytes = fs::read(&archive_path_clone)?;
            let size = bytes.len() as u64;
            let hash = *blake3::hash(&bytes).as_bytes();
            Ok((size, hash))
        })
        .await
        .map_err(|e| AppError::Migration(format!("Snapshot thread error: {}", e)))??;

    // Re-enable chunk saving
    if is_running {
        let _ = runner.send_command("save-on").await;
    }

    let blake3_hash = hex::encode(blake3_bytes);
    let blob_ticket = host_node_addr.and_then(|addr| {
        let bh = BlobHash::from_bytes(blake3_bytes);
        BlobTicket::new(addr.clone(), bh, BlobFormat::Raw)
            .ok()
            .map(|t| t.to_string())
    });

    let mc_version = runner.mc_version.lock().await.clone();
    let core = *runner.core.lock().await;
    let loader = match core {
        ServerCore::Paper => "paper",
        ServerCore::Fabric => "fabric",
        ServerCore::Neoforge => "neoforge",
    }
    .to_string();

    let info = SnapshotInfo {
        snapshot_id,
        filename,
        size_bytes,
        blake3_hash,
        blob_ticket,
        created_at_ms: now_ms,
        mc_version,
        loader,
    };

    if let Some(app) = app_handle {
        let _ = app.emit("migration:snapshot-created", info.clone());
    }

    Ok(info)
}

fn append_dir_filtered<W: std::io::Write>(
    builder: &mut tar::Builder<W>,
    src_dir: &Path,
    tar_prefix: &Path,
) -> AppResult<()> {
    if let Ok(entries) = fs::read_dir(src_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            // Skip session.lock so Windows/Linux file locks never fail snapshot creation
            if name == "session.lock" {
                continue;
            }
            let dest_rel = tar_prefix.join(&name);
            if path.is_dir() {
                append_dir_filtered(builder, &path, &dest_rel)?;
            } else if path.is_file() {
                let _ = builder.append_path_with_name(&path, &dest_rel);
            }
        }
    }
    Ok(())
}

/// Extract a replicated `.tar.zst` snapshot into `server_dir` inside `tokio::task::spawn_blocking`.
pub async fn extract_world_snapshot(
    snapshot_archive_path: &Path,
    server_dir: &Path,
) -> AppResult<()> {
    let archive_buf = snapshot_archive_path.to_path_buf();
    let server_buf = server_dir.to_path_buf();

    tokio::task::spawn_blocking(move || -> AppResult<()> {
        fs::create_dir_all(&server_buf)?;
        let file = fs::File::open(&archive_buf)?;
        let decoder = zstd::Decoder::new(file)?;
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(&server_buf)?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::Migration(format!("Snapshot extraction join error: {}", e)))??;

    Ok(())
}

/// Promote a Trusted Peer to the new Host from the latest synced `.tar.zst` snapshot:
/// 1. Extracts the latest `.tar.zst` snapshot into `server_dir`.
/// 2. Updates the `pkarr` Mainline DHT record for `room_code` with this Peer's own `NodeAddr`.
/// 3. Boots the local Paper/Fabric engine with the replicated world.
pub async fn promote_peer_to_host(
    runner: &ServerRunner,
    snapshot_path: &Path,
    room_code: &str,
    local_node_addr: &NodeAddr,
    mc_version: &str,
    loader: &str,
    app_handle: AppHandle,
) -> AppResult<()> {
    let _ = app_handle.emit(
        "migration:event",
        MigrationEvent {
            stage: "extracting".to_string(),
            room_code: room_code.to_string(),
            new_host_node_id: local_node_addr.node_id.to_string(),
            message: "Extracting replicated .tar.zst world snapshot...".to_string(),
        },
    );

    if snapshot_path.exists() {
        extract_world_snapshot(snapshot_path, &runner.server_dir()).await?;
        // Snapshots always hold the world as `world/`, so run that one (the archive's
        // server.properties just overwrote our own `level-name`).
        crate::engine::worlds::set_level_name(
            &runner.server_dir(),
            crate::engine::worlds::LEGACY_WORLD_ID,
        )?;
    }

    let _ = app_handle.emit(
        "migration:event",
        MigrationEvent {
            stage: "publishing_dht".to_string(),
            room_code: room_code.to_string(),
            new_host_node_id: local_node_addr.node_id.to_string(),
            message: format!(
                "Updating Mainline DHT record for '{}' with new Host NodeAddr...",
                room_code
            ),
        },
    );

    publish_room_to_dht(
        &runner.app_data_dir,
        room_code,
        local_node_addr,
        "Migrated Co-Host",
        mc_version,
        loader,
        None,
    )
    .await?;

    let core = match loader {
        "fabric" => ServerCore::Fabric,
        "neoforge" => ServerCore::Neoforge,
        _ => ServerCore::Paper,
    };

    let _ = app_handle.emit(
        "migration:event",
        MigrationEvent {
            stage: "booting_server".to_string(),
            room_code: room_code.to_string(),
            new_host_node_id: local_node_addr.node_id.to_string(),
            message: "Booting local Minecraft Server Engine with migrated world...".to_string(),
        },
    );

    runner
        .start_server(
            ServerLaunchConfig {
                core,
                mc_version: mc_version.to_string(),
                allocated_ram_mb: None,
                java_major: None,
                world_id: None,
            },
            app_handle.clone(),
        )
        .await?;

    let _ = app_handle.emit(
        "migration:event",
        MigrationEvent {
            stage: "completed".to_string(),
            room_code: room_code.to_string(),
            new_host_node_id: local_node_addr.node_id.to_string(),
            message: "Host Migration complete! You are now hosting the session.".to_string(),
        },
    );

    Ok(())
}
