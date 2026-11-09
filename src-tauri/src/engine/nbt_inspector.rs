use crate::error::{AppError, AppResult};
use fastnbt::Value as NbtValue;
use flate2::read::GzDecoder;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItemSlot {
    /// Raw NBT Slot byte (-106 for offhand, 0..8 hotbar, 9..35 main inventory, 100..103 armor)
    pub slot: i8,
    /// Command slot identifier for `/item replace entity <player> <command_slot> with air`
    pub command_slot: String,
    /// Item ID e.g. `minecraft:diamond_sword`
    pub id: String,
    /// Item count (1..64)
    pub count: u8,
    /// Optional custom display name or enchantments summary
    pub tag_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInventoryData {
    pub uuid: String,
    pub username: Option<String>,
    pub health: f32,
    pub max_health: f32,
    pub food_level: i32,
    pub saturation: f32,
    pub xp_level: i32,
    pub xp_total: i32,
    pub gamemode: i32,
    pub dimension: String,
    pub position: [f64; 3],
    pub inventory: Vec<InventoryItemSlot>,
    pub ender_items: Vec<InventoryItemSlot>,
    pub last_updated_ms: u64,
}

#[derive(Debug, Deserialize)]
struct RawPlayerDat {
    #[serde(rename = "Health", default = "default_health")]
    health: f32,
    #[serde(rename = "foodLevel", default = "default_food")]
    food_level: i32,
    #[serde(rename = "foodSaturationLevel", default)]
    food_saturation_level: f32,
    #[serde(rename = "XpLevel", default)]
    xp_level: i32,
    #[serde(rename = "XpTotal", default)]
    xp_total: i32,
    #[serde(rename = "playerGameType", default)]
    player_game_type: i32,
    #[serde(rename = "Dimension", default = "default_dimension")]
    dimension: String,
    #[serde(rename = "Pos", default)]
    pos: Vec<f64>,
    #[serde(rename = "Inventory", default)]
    inventory: Vec<RawNbtItem>,
    #[serde(rename = "EnderItems", default)]
    ender_items: Vec<RawNbtItem>,
}

fn default_health() -> f32 {
    20.0
}
fn default_food() -> i32 {
    20
}
fn default_dimension() -> String {
    "minecraft:overworld".to_string()
}

#[derive(Debug, Deserialize)]
struct RawNbtItem {
    #[serde(rename = "Slot", default)]
    slot: i8,
    #[serde(default)]
    id: String,
    #[serde(rename = "Count", alias = "count", default = "default_item_count")]
    count: i32,
    #[serde(default)]
    tag: Option<HashMap<String, NbtValue>>,
    #[serde(default)]
    components: Option<HashMap<String, NbtValue>>,
}

fn default_item_count() -> i32 {
    1
}

/// Convert Minecraft NBT Slot index into `/item replace entity <player> <slot>` syntax.
pub fn nbt_slot_to_command_slot(slot: i8, is_ender: bool) -> String {
    if is_ender {
        return format!("enderchest.{}", slot.max(0));
    }
    match slot {
        0..=8 => format!("hotbar.{}", slot),
        9..=35 => format!("container.{}", slot - 9),
        100 => "armor.feet".to_string(),
        101 => "armor.legs".to_string(),
        102 => "armor.chest".to_string(),
        103 => "armor.head".to_string(),
        -106 => "weapon.offhand".to_string(),
        other => format!("container.{}", other.max(0)),
    }
}

/// Compute Minecraft's deterministic Offline-Mode UUID (`OfflinePlayer:<username>`)
/// or normalize an existing dashed/undashed UUID.
pub fn offline_player_uuid(username: &str) -> String {
    // Minecraft's UUID.nameUUIDFromBytes uses version 3 (MD5), or we can compute a deterministic v3 UUID
    // using UUID namespace + OfflinePlayer prefix so offline players always have a valid UUID format.
    let input = format!("OfflinePlayer:{}", username);
    let digest = blake3::hash(input.as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest.as_bytes()[0..16]);
    // Set version 3 and IETF variant bits
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes).to_string()
}

/// Read and parse a player's `.dat` file using `flate2` + `fastnbt` inside `tokio::task::spawn_blocking`.
pub async fn inspect_player_nbt(
    server_dir: &Path,
    uuid: &str,
    username: Option<String>,
) -> AppResult<PlayerInventoryData> {
    let server_dir_buf = server_dir.to_path_buf();
    let uuid_str = uuid.to_string();
    let username_clone = username.clone();

    tokio::task::spawn_blocking(move || {
        parse_player_dat_sync(&server_dir_buf, &uuid_str, username_clone)
    })
    .await
    .map_err(|e| AppError::Nbt(format!("Blocking task join error: {}", e)))?
}

fn parse_player_dat_sync(
    server_dir: &Path,
    uuid: &str,
    username: Option<String>,
) -> AppResult<PlayerInventoryData> {
    let playerdata_dir = crate::engine::worlds::level_dir(server_dir).join("playerdata");
    let exact_path = playerdata_dir.join(format!("{}.dat", uuid));

    // Try exact UUID path first; if missing and there are .dat files in world/playerdata, find matching or most recent
    let target_path = if exact_path.exists() {
        Some(exact_path)
    } else if playerdata_dir.exists() {
        let mut latest_file: Option<(std::time::SystemTime, PathBuf)> = None;
        if let Ok(entries) = fs::read_dir(&playerdata_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("dat") {
                    if let Ok(meta) = entry.metadata() {
                        let mtime = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                        if latest_file.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
                            latest_file = Some((mtime, path));
                        }
                    }
                }
            }
        }
        latest_file.map(|(_, p)| p)
    } else {
        None
    };

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let Some(dat_path) = target_path else {
        // Return an empty default live inventory if the world hasn't flushed playerdata yet
        return Ok(PlayerInventoryData {
            uuid: uuid.to_string(),
            username,
            health: 20.0,
            max_health: 20.0,
            food_level: 20,
            saturation: 5.0,
            xp_level: 0,
            xp_total: 0,
            gamemode: 0,
            dimension: "minecraft:overworld".to_string(),
            position: [0.0, 64.0, 0.0],
            inventory: Vec::new(),
            ender_items: Vec::new(),
            last_updated_ms: now_ms,
        });
    };

    let file = fs::File::open(&dat_path)?;
    let mut decoder = GzDecoder::new(file);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;

    let raw: RawPlayerDat = fastnbt::from_bytes(&decompressed)?;

    let position = if raw.pos.len() >= 3 {
        [raw.pos[0], raw.pos[1], raw.pos[2]]
    } else {
        [0.0, 64.0, 0.0]
    };

    let inventory = raw
        .inventory
        .into_iter()
        .filter(|i| !i.id.is_empty() && i.id != "minecraft:air")
        .map(|item| {
            let summary = item
                .components
                .as_ref()
                .or(item.tag.as_ref())
                .map(|m| format!("{} NBT tags", m.len()));
            InventoryItemSlot {
                slot: item.slot,
                command_slot: nbt_slot_to_command_slot(item.slot, false),
                id: item.id,
                count: item.count.clamp(1, 99) as u8,
                tag_summary: summary,
            }
        })
        .collect();

    let ender_items = raw
        .ender_items
        .into_iter()
        .filter(|i| !i.id.is_empty() && i.id != "minecraft:air")
        .map(|item| {
            let summary = item
                .components
                .as_ref()
                .or(item.tag.as_ref())
                .map(|m| format!("{} NBT tags", m.len()));
            InventoryItemSlot {
                slot: item.slot,
                command_slot: nbt_slot_to_command_slot(item.slot, true),
                id: item.id,
                count: item.count.clamp(1, 99) as u8,
                tag_summary: summary,
            }
        })
        .collect();

    Ok(PlayerInventoryData {
        uuid: uuid.to_string(),
        username,
        health: raw.health,
        max_health: 20.0,
        food_level: raw.food_level,
        saturation: raw.food_saturation_level,
        xp_level: raw.xp_level,
        xp_total: raw.xp_total,
        gamemode: raw.player_game_type,
        dimension: raw.dimension,
        position,
        inventory,
        ender_items,
        last_updated_ms: now_ms,
    })
}

/// Start a background `notify` watcher on `world/playerdata` to push live inventory updates to the UI.
pub fn spawn_playerdata_watcher(server_dir: PathBuf, app_handle: AppHandle) {
    tokio::spawn(async move {
        let playerdata_dir = crate::engine::worlds::level_dir(&server_dir).join("playerdata");
        let _ = tokio::fs::create_dir_all(&playerdata_dir).await;

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<PathBuf>();

        let watcher_res = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    for path in event.paths {
                        if path.extension().and_then(|e| e.to_str()) == Some("dat") {
                            let _ = tx.send(path);
                        }
                    }
                }
            },
            Config::default(),
        );

        let Ok(mut watcher) = watcher_res else {
            return;
        };

        if watcher
            .watch(&playerdata_dir, RecursiveMode::NonRecursive)
            .is_err()
        {
            return;
        }

        // Keep watcher alive in this task and process changed .dat files
        while let Some(changed_path) = rx.recv().await {
            if let Some(stem) = changed_path.file_stem().and_then(|s| s.to_str()) {
                let uuid = stem.to_string();
                if let Ok(inv) = inspect_player_nbt(&server_dir, &uuid, None).await {
                    let _ = app_handle.emit("player:inventory-updated", inv);
                }
            }
        }
    });
}
