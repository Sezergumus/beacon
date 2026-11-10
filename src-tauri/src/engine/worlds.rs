use crate::engine::properties::ServerProperties;
use crate::engine::runner::ServerCore;
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Every world's own server folder keeps its Minecraft world in a folder with this
/// name (`level-name` in its `server.properties`), plus `_nether` / `_the_end`.
/// It's also the folder name worlds made before per-world servers used by default.
pub const LEGACY_WORLD_ID: &str = "world";

const MAX_NAME_CHARS: usize = 40;

/// One world the player created. Each world owns a complete server folder
/// (`instances/<id>/`) with its own server jar, mods, plugins and settings, so
/// changing the version or installing a modpack never touches another world.
/// `name` is only a label, so renaming never touches the files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldMeta {
    pub id: String,
    pub name: String,
    pub created_ms: u64,
    /// Server type (Paper / Fabric / NeoForge) this world runs on.
    #[serde(default)]
    pub core: Option<ServerCore>,
    /// Minecraft version this world runs on.
    #[serde(default)]
    pub mc_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldEntry {
    /// The world id, or the file name for a backup.
    pub id: String,
    /// `"world"` or `"backup"`.
    pub kind: String,
    pub name: String,
    pub size_bytes: u64,
    pub modified_ms: u64,
    /// True for the world the server uses (or will use on its next start).
    pub active: bool,
    pub core: Option<ServerCore>,
    pub mc_version: Option<String>,
    /// Number of installed mods / plugins in this world.
    pub mod_count: u32,
    pub plugin_count: u32,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += dir_size(&p);
            } else if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

fn count_jars(dir: &Path) -> u32 {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().to_lowercase().ends_with(".jar"))
                .count() as u32
        })
        .unwrap_or(0)
}

fn modified_ms(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
}

/// Parent folder of every world's server folder.
pub fn instances_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("instances")
}

/// The complete server folder that belongs to one world.
pub fn instance_dir(app_data_dir: &Path, id: &str) -> PathBuf {
    instances_dir(app_data_dir).join(id)
}

/// Used only while no world exists at all (e.g. a brand-new install).
pub fn fallback_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("server")
}

fn active_world_file(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("active_world.txt")
}

/// The world that was selected last time, if it still exists.
pub fn read_active_world(app_data_dir: &Path) -> Option<String> {
    let id = fs::read_to_string(active_world_file(app_data_dir)).ok()?;
    let id = id.trim().to_string();
    (is_safe_id(&id) && instance_dir(app_data_dir, &id).is_dir()).then_some(id)
}

pub fn write_active_world(app_data_dir: &Path, id: Option<&str>) {
    let path = active_world_file(app_data_dir);
    match id {
        Some(id) => {
            let _ = fs::write(path, id);
        }
        None => {
            let _ = fs::remove_file(path);
        }
    }
}

fn world_dirs(instance: &Path) -> [PathBuf; 3] {
    [
        instance.join(LEGACY_WORLD_ID),
        instance.join(format!("{}_nether", LEGACY_WORLD_ID)),
        instance.join(format!("{}_the_end", LEGACY_WORLD_ID)),
    ]
}

/// Folder holding the world's `level.dat`, `playerdata/`, ...
pub fn level_dir(server_dir: &Path) -> PathBuf {
    server_dir.join(level_name(server_dir))
}

/// The Minecraft world folder a server folder is set up to run, from `server.properties`.
pub fn level_name(server_dir: &Path) -> String {
    let props = fs::read_to_string(server_dir.join("server.properties")).unwrap_or_default();
    props
        .lines()
        .filter_map(|l| l.trim().strip_prefix("level-name="))
        .map(|v| v.trim().to_string())
        .find(|v| is_safe_id(v))
        .unwrap_or_else(|| LEGACY_WORLD_ID.to_string())
}

/// Point `server.properties` at a world folder. Everything else in the file is kept.
pub fn set_level_name(server_dir: &Path, id: &str) -> AppResult<()> {
    if !is_safe_id(id) {
        return Err(AppError::Custom("That isn't a valid world.".to_string()));
    }
    set_property(server_dir, "level-name", id)
}

/// Set one `key=value` line in `server.properties`. Everything else in the file is kept.
/// `value` must be a single line, since it is written into the file as-is.
fn set_property(server_dir: &Path, key: &str, value: &str) -> AppResult<()> {
    if value.chars().any(|c| c.is_control()) {
        return Err(AppError::Custom("That value can't contain line breaks.".to_string()));
    }
    let path = server_dir.join("server.properties");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let prefix = format!("{}=", key);
    let mut out = String::new();
    let mut replaced = false;
    for line in existing.lines() {
        if line.trim().starts_with(&prefix) {
            if !replaced {
                out.push_str(&format!("{}{}\n", prefix, value));
                replaced = true;
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !replaced {
        out.push_str(&format!("{}{}\n", prefix, value));
    }
    fs::create_dir_all(server_dir)?;
    fs::write(path, out)?;
    Ok(())
}

/// Ids end up in folder names, so only plain `[A-Za-z0-9_-]` is accepted.
pub(crate) fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn is_safe_version(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 32
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

fn clean_name(name: &str) -> AppResult<String> {
    let name: String = name
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    if name.is_empty() {
        return Err(AppError::Custom("Give your world a name first.".to_string()));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(AppError::Custom(format!(
            "That name is too long (max {} letters).",
            MAX_NAME_CHARS
        )));
    }
    Ok(name)
}

fn registry_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("worlds.json")
}

fn save_registry(app_data_dir: &Path, worlds: &[WorldMeta]) -> AppResult<()> {
    let json = serde_json::to_string_pretty(worlds)?;
    fs::write(registry_path(app_data_dir), json)?;
    Ok(())
}

/// All worlds, oldest first.
pub fn load_registry(app_data_dir: &Path) -> Vec<WorldMeta> {
    fs::read_to_string(registry_path(app_data_dir))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

const MAX_SEED_CHARS: usize = 64;

/// Things that can only be chosen while a world is being made: Minecraft reads them
/// when it generates the world on first start and ignores them afterwards.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NewWorldOptions {
    /// Any text or number. Empty / missing means a random seed.
    #[serde(default)]
    pub seed: Option<String>,
    /// One life: players become spectators when they die. Locks the difficulty to hard.
    #[serde(default)]
    pub hardcore: bool,
}

fn clean_seed(seed: Option<&str>) -> AppResult<Option<String>> {
    let seed = seed.map(str::trim).filter(|s| !s.is_empty());
    match seed {
        None => Ok(None),
        Some(s) if s.chars().any(|c| c.is_control()) => {
            Err(AppError::Custom("The seed can't contain line breaks.".to_string()))
        }
        Some(s) if s.chars().count() > MAX_SEED_CHARS => Err(AppError::Custom(format!(
            "That seed is too long (max {} characters).",
            MAX_SEED_CHARS
        ))),
        Some(s) => Ok(Some(s.to_string())),
    }
}

/// Make a new world with its own (still empty) server folder. Minecraft generates
/// the world itself the first time it's started.
pub fn create_world(
    app_data_dir: &Path,
    name: &str,
    core: ServerCore,
    mc_version: &str,
) -> AppResult<WorldMeta> {
    create_world_with(app_data_dir, name, core, mc_version, &NewWorldOptions::default())
}

/// Like [`create_world`], with a chosen seed and/or hardcore mode.
pub fn create_world_with(
    app_data_dir: &Path,
    name: &str,
    core: ServerCore,
    mc_version: &str,
    options: &NewWorldOptions,
) -> AppResult<WorldMeta> {
    let name = clean_name(name)?;
    let seed = clean_seed(options.seed.as_deref())?;
    if !is_safe_version(mc_version) {
        return Err(AppError::Custom(
            "That isn't a valid Minecraft version.".to_string(),
        ));
    }
    let mut worlds = load_registry(app_data_dir);

    let created_ms = now_ms();
    let mut id = format!("w{}", created_ms);
    // Two worlds made in the same millisecond must not share a folder.
    while worlds.iter().any(|w| w.id == id) {
        id.push('x');
    }

    let inst = instance_dir(app_data_dir, &id);
    fs::create_dir_all(inst.join("mods"))?;
    fs::create_dir_all(inst.join("plugins"))?;
    // New worlds start from the settings and icon chosen on the Settings page.
    let mut props = crate::engine::properties::load_defaults(app_data_dir);
    props.save(&inst)?;
    ensure_icon(app_data_dir, &id);
    set_level_name(&inst, LEGACY_WORLD_ID)?;
    if let Some(seed) = &seed {
        set_property(&inst, "level-seed", seed)?;
    }
    if options.hardcore {
        // Hardcore is always played on hard; keep the settings page in line with that.
        props.difficulty = "hard".to_string();
        props.save(&inst)?;
        set_property(&inst, "hardcore", "true")?;
    }

    let meta = WorldMeta {
        id,
        name,
        created_ms,
        core: Some(core),
        mc_version: Some(mc_version.to_string()),
    };
    worlds.push(meta.clone());
    save_registry(app_data_dir, &worlds)?;
    Ok(meta)
}

pub fn rename_world(app_data_dir: &Path, id: &str, name: &str) -> AppResult<()> {
    let name = clean_name(name)?;
    let mut worlds = load_registry(app_data_dir);
    let world = worlds
        .iter_mut()
        .find(|w| w.id == id)
        .ok_or_else(|| AppError::Custom("That world no longer exists.".to_string()))?;
    world.name = name;
    save_registry(app_data_dir, &worlds)
}

/// Change the server type / Minecraft version of one world. Other worlds are untouched.
pub fn update_world_config(
    app_data_dir: &Path,
    id: &str,
    core: ServerCore,
    mc_version: &str,
) -> AppResult<()> {
    if !is_safe_version(mc_version) {
        return Err(AppError::Custom(
            "That isn't a valid Minecraft version.".to_string(),
        ));
    }
    let mut worlds = load_registry(app_data_dir);
    let world = worlds
        .iter_mut()
        .find(|w| w.id == id)
        .ok_or_else(|| AppError::Custom("That world no longer exists.".to_string()))?;
    world.core = Some(core);
    world.mc_version = Some(mc_version.to_string());
    save_registry(app_data_dir, &worlds)
}

/// The world to launch: the requested one, else the first one, and if there are
/// none at all, a fresh "My world" with the given defaults.
pub fn resolve_world(
    app_data_dir: &Path,
    requested: Option<&str>,
    default_core: ServerCore,
    default_version: &str,
) -> AppResult<WorldMeta> {
    let worlds = load_registry(app_data_dir);

    if let Some(id) = requested {
        return worlds
            .into_iter()
            .find(|w| w.id == id)
            .ok_or_else(|| AppError::Custom("That world no longer exists.".to_string()));
    }
    if let Some(w) = worlds.into_iter().next() {
        return Ok(w);
    }
    create_world(app_data_dir, "My world", default_core, default_version)
}

/// The live worlds plus every `.tar.zst` backup, newest first within each group.
pub fn list_worlds(
    app_data_dir: &Path,
    snapshots_dir: &Path,
    active_id: Option<&str>,
) -> Vec<WorldEntry> {
    let mut out = Vec::new();

    let mut worlds = load_registry(app_data_dir);
    worlds.sort_by(|a, b| b.created_ms.cmp(&a.created_ms));
    for w in worlds {
        let inst = instance_dir(app_data_dir, &w.id);
        let dirs = world_dirs(&inst);
        let size = dirs.iter().map(|d| dir_size(d)).sum();
        let played = modified_ms(&dirs[0].join("level.dat")).unwrap_or(w.created_ms);
        out.push(WorldEntry {
            active: Some(w.id.as_str()) == active_id,
            mod_count: count_jars(&inst.join("mods")),
            plugin_count: count_jars(&inst.join("plugins")),
            core: w.core,
            mc_version: w.mc_version.clone(),
            id: w.id,
            kind: "world".to_string(),
            name: w.name,
            size_bytes: size,
            modified_ms: played,
        });
    }

    let mut backups: Vec<WorldEntry> = Vec::new();
    if let Ok(entries) = fs::read_dir(snapshots_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".tar.zst") {
                continue;
            }
            let path = entry.path();
            backups.push(WorldEntry {
                id: name.clone(),
                kind: "backup".to_string(),
                name,
                size_bytes: fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
                modified_ms: modified_ms(&path).unwrap_or(0),
                active: false,
                core: None,
                mc_version: None,
                mod_count: 0,
                plugin_count: 0,
            });
        }
    }
    backups.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms));
    out.extend(backups);
    out
}

/// Remove a world's server folder and forget it. The server must not be using it.
pub fn delete_world(app_data_dir: &Path, id: &str) -> AppResult<()> {
    let mut worlds = load_registry(app_data_dir);
    if !is_safe_id(id) || !worlds.iter().any(|w| w.id == id) {
        return Err(AppError::Custom("That world no longer exists.".to_string()));
    }
    let inst = instance_dir(app_data_dir, id);
    if inst.exists() {
        fs::remove_dir_all(&inst)?;
    }
    worlds.retain(|w| w.id != id);
    save_registry(app_data_dir, &worlds)?;
    if read_active_world(app_data_dir).is_none() {
        write_active_world(app_data_dir, None);
    }
    Ok(())
}

/// Remove one backup archive. Only plain `*.tar.zst` file names are accepted.
pub fn delete_backup(snapshots_dir: &Path, filename: &str) -> AppResult<()> {
    let valid = filename.ends_with(".tar.zst")
        && !filename.contains('/')
        && !filename.contains('\\')
        && !filename.contains("..");
    if !valid {
        return Err(AppError::Custom("That isn't a valid backup.".to_string()));
    }
    let path = snapshots_dir.join(filename);
    if !path.is_file() {
        return Err(AppError::Custom("That backup no longer exists.".to_string()));
    }
    fs::remove_file(path)?;
    Ok(())
}

/* ---------- Details of one world (for the details window) ---------- */

/// A mod / plugin file sitting in a world's `mods/` or `plugins/` folder.
#[derive(Debug, Clone, Serialize)]
pub struct ContentFile {
    pub filename: String,
    pub size_bytes: u64,
}

/// How long one player has played in a world, from Minecraft's own stats file.
#[derive(Debug, Clone, Serialize)]
pub struct PlayerPlaytime {
    pub uuid: String,
    pub name: Option<String>,
    pub playtime_secs: u64,
    pub last_seen_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldDetails {
    pub id: String,
    pub name: String,
    pub core: Option<ServerCore>,
    pub mc_version: Option<String>,
    pub created_ms: u64,
    pub size_bytes: u64,
    /// From `level.dat`; empty until Minecraft has started this world once.
    pub last_played_ms: Option<u64>,
    /// A string, because a 64-bit seed doesn't fit in a JavaScript number.
    pub seed: Option<String>,
    pub hardcore: bool,
    /// Minecraft version that last saved this world.
    pub game_version: Option<String>,
    pub spawn: Option<[i32; 3]>,
    /// In-game ticks since the world was created (24000 ticks = one in-game day).
    pub world_age_ticks: Option<u64>,
    pub total_playtime_secs: u64,
    pub players: Vec<PlayerPlaytime>,
    pub mods: Vec<ContentFile>,
    pub plugins: Vec<ContentFile>,
    pub properties: ServerProperties,
}

#[derive(Debug, Default)]
struct LevelInfo {
    seed: Option<String>,
    hardcore: bool,
    game_version: Option<String>,
    spawn: Option<[i32; 3]>,
    age_ticks: Option<u64>,
    last_played_ms: Option<u64>,
}

fn nbt_get<'a>(value: &'a fastnbt::Value, key: &str) -> Option<&'a fastnbt::Value> {
    match value {
        fastnbt::Value::Compound(map) => map.get(key),
        _ => None,
    }
}

fn nbt_int(value: &fastnbt::Value) -> Option<i64> {
    match value {
        fastnbt::Value::Byte(v) => Some(*v as i64),
        fastnbt::Value::Short(v) => Some(*v as i64),
        fastnbt::Value::Int(v) => Some(*v as i64),
        fastnbt::Value::Long(v) => Some(*v),
        _ => None,
    }
}

/// Reads what it can from `level.dat`. Anything missing or unreadable stays empty,
/// because a world that was never started simply has no such file yet.
fn read_level_info(level: &Path) -> LevelInfo {
    use std::io::Read;

    let mut info = LevelInfo::default();
    let Ok(file) = fs::File::open(level.join("level.dat")) else {
        return info;
    };
    let mut bytes = Vec::new();
    if flate2::read::GzDecoder::new(file).read_to_end(&mut bytes).is_err() {
        return info;
    }
    let Ok(root) = fastnbt::from_bytes::<fastnbt::Value>(&bytes) else {
        return info;
    };
    let Some(data) = nbt_get(&root, "Data") else {
        return info;
    };

    // 1.16+ keeps the seed under WorldGenSettings, older worlds use RandomSeed.
    info.seed = nbt_get(data, "WorldGenSettings")
        .and_then(|w| nbt_get(w, "seed"))
        .or_else(|| nbt_get(data, "RandomSeed"))
        .and_then(nbt_int)
        .map(|s| s.to_string());
    info.hardcore = nbt_get(data, "hardcore").and_then(nbt_int).unwrap_or(0) != 0;
    info.game_version = nbt_get(data, "Version")
        .and_then(|v| nbt_get(v, "Name"))
        .and_then(|n| match n {
            fastnbt::Value::String(s) => Some(s.clone()),
            _ => None,
        });
    info.age_ticks = nbt_get(data, "Time").and_then(nbt_int).map(|t| t.max(0) as u64);
    info.last_played_ms = nbt_get(data, "LastPlayed")
        .and_then(nbt_int)
        .filter(|t| *t > 0)
        .map(|t| t as u64);
    let coord = |key: &str| nbt_get(data, key).and_then(nbt_int).map(|v| v as i32);
    if let (Some(x), Some(y), Some(z)) = (coord("SpawnX"), coord("SpawnY"), coord("SpawnZ")) {
        info.spawn = Some([x, y, z]);
    }
    info
}

/// Play time per player from `<level>/stats/<uuid>.json`, longest first. Minecraft
/// counts ticks (20 per second); 1.12 and older called the counter `play_one_minute`.
fn read_playtimes(instance: &Path, level: &Path) -> Vec<PlayerPlaytime> {
    let names: std::collections::HashMap<String, String> =
        fs::read_to_string(instance.join("usercache.json"))
            .ok()
            .and_then(|raw| serde_json::from_str::<Vec<serde_json::Value>>(&raw).ok())
            .unwrap_or_default()
            .iter()
            .filter_map(|e| {
                let uuid = e.get("uuid")?.as_str()?.to_lowercase();
                let name = e.get("name")?.as_str()?.to_string();
                Some((uuid, name))
            })
            .collect();

    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(level.join("stats")) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(uuid) = path.file_stem().and_then(|s| s.to_str()).map(str::to_lowercase) else {
            continue;
        };
        let Some(json) = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        else {
            continue;
        };
        let custom = &json["stats"]["minecraft:custom"];
        let ticks = custom["minecraft:play_time"]
            .as_u64()
            .or_else(|| custom["minecraft:play_one_minute"].as_u64())
            .unwrap_or(0);
        let last_seen_ms = modified_ms(&level.join("playerdata").join(format!("{}.dat", uuid)))
            .or_else(|| modified_ms(&path));
        out.push(PlayerPlaytime {
            name: names.get(&uuid).cloned(),
            uuid,
            playtime_secs: ticks / 20,
            last_seen_ms,
        });
    }
    out.sort_by(|a, b| b.playtime_secs.cmp(&a.playtime_secs));
    out
}

fn list_jar_files(dir: &Path) -> Vec<ContentFile> {
    let mut out: Vec<ContentFile> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| {
                    let filename = e.file_name().to_string_lossy().to_string();
                    filename.to_lowercase().ends_with(".jar").then(|| ContentFile {
                        size_bytes: e.metadata().map(|m| m.len()).unwrap_or(0),
                        filename,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|f| f.filename.to_lowercase());
    out
}

/// Everything the details window shows for one world, read straight from the files
/// Minecraft and this app already keep in the world's folder.
pub fn world_details(app_data_dir: &Path, id: &str) -> AppResult<WorldDetails> {
    let meta = load_registry(app_data_dir)
        .into_iter()
        .find(|w| w.id == id && is_safe_id(&w.id))
        .ok_or_else(|| AppError::Custom("That world no longer exists.".to_string()))?;
    let inst = instance_dir(app_data_dir, &meta.id);
    let level = level_dir(&inst);
    let level_info = read_level_info(&level);
    let players = read_playtimes(&inst, &level);

    Ok(WorldDetails {
        size_bytes: world_dirs(&inst).iter().map(|d| dir_size(d)).sum(),
        last_played_ms: level_info
            .last_played_ms
            .or_else(|| modified_ms(&level.join("level.dat"))),
        seed: level_info.seed,
        hardcore: level_info.hardcore,
        game_version: level_info.game_version,
        spawn: level_info.spawn,
        world_age_ticks: level_info.age_ticks,
        total_playtime_secs: players.iter().map(|p| p.playtime_secs).sum(),
        players,
        mods: list_jar_files(&inst.join("mods")),
        plugins: list_jar_files(&inst.join("plugins")),
        properties: ServerProperties::load(&inst)?,
        id: meta.id,
        name: meta.name,
        core: meta.core,
        mc_version: meta.mc_version,
        created_ms: meta.created_ms,
    })
}

/// Save one world's `server.properties`. Checks the values that must come from a
/// fixed list, since this takes input straight from the window.
pub fn save_world_properties(
    app_data_dir: &Path,
    id: &str,
    props: &ServerProperties,
) -> AppResult<ServerProperties> {
    if !is_safe_id(id) || load_registry(app_data_dir).iter().all(|w| w.id != id) {
        return Err(AppError::Custom("That world no longer exists.".to_string()));
    }
    props.validate()?;
    let inst = instance_dir(app_data_dir, id);
    props.save(&inst)?;
    ServerProperties::load(&inst)
}

/* ---------- Server icon ---------- */

/// Minecraft shows `server-icon.png` (exactly 64x64) in the multiplayer list.
const ICON_FILE: &str = "server-icon.png";
const ICON_SIZE: u32 = 64;
const MAX_ICON_BYTES: usize = 64 * 1024;

/// The app's own icon (64x64): what a world shows until the player picks another picture.
const APP_ICON: &[u8] = include_bytes!("../../icons/64x64.png");

/// The icon file of one world, or (with no id) the one new worlds start with.
fn icon_path(app_data_dir: &Path, id: Option<&str>) -> AppResult<PathBuf> {
    match id {
        None => Ok(app_data_dir.join("default-server-icon.png")),
        Some(id) => {
            if !is_safe_id(id) || load_registry(app_data_dir).iter().all(|w| w.id != id) {
                return Err(AppError::Custom("That world no longer exists.".to_string()));
            }
            Ok(instance_dir(app_data_dir, id).join(ICON_FILE))
        }
    }
}

/// The icon new worlds start with: the one chosen on the Settings page, else the app's.
fn default_icon_bytes(app_data_dir: &Path) -> Vec<u8> {
    fs::read(app_data_dir.join("default-server-icon.png")).unwrap_or_else(|_| APP_ICON.to_vec())
}

/// Give a world the default icon if it has none yet (older worlds, or a fresh one).
pub fn ensure_icon(app_data_dir: &Path, id: &str) {
    let path = instance_dir(app_data_dir, id).join(ICON_FILE);
    if !path.exists() {
        let _ = fs::write(path, default_icon_bytes(app_data_dir));
    }
}

/// The icon as a `data:` URL the window can show. A world without one shows the default.
pub fn read_icon(app_data_dir: &Path, id: Option<&str>) -> AppResult<Option<String>> {
    use base64::Engine;
    let path = icon_path(app_data_dir, id)?;
    let bytes = fs::read(&path).unwrap_or_else(|_| default_icon_bytes(app_data_dir));
    Ok(Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )))
}

/// Save a PNG sent from the window. It must already be 64x64: the window crops and
/// scales the picture the player chose, so anything else is refused.
pub fn write_icon(app_data_dir: &Path, id: Option<&str>, png_base64: &str) -> AppResult<()> {
    use base64::Engine;
    let path = icon_path(app_data_dir, id)?;
    let data = png_base64
        .split_once("base64,")
        .map(|(_, rest)| rest)
        .unwrap_or(png_base64);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|_| AppError::Custom("That picture couldn't be read.".to_string()))?;
    if bytes.len() > MAX_ICON_BYTES {
        return Err(AppError::Custom("That picture is too big.".to_string()));
    }
    // PNG signature, then the IHDR chunk with the width and height.
    let is_png = bytes.len() >= 24
        && bytes[..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        && &bytes[12..16] == b"IHDR";
    if !is_png {
        return Err(AppError::Custom("The icon has to be a PNG picture.".to_string()));
    }
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    if width != ICON_SIZE || height != ICON_SIZE {
        return Err(AppError::Custom(format!(
            "The icon has to be {0}x{0} pixels.",
            ICON_SIZE
        )));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    Ok(())
}

/// Back to the default: for a world that is the starting icon, for the starting icon
/// itself it is the app's own.
pub fn clear_icon(app_data_dir: &Path, id: Option<&str>) -> AppResult<()> {
    let path = icon_path(app_data_dir, id)?;
    match id {
        None => match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        },
        Some(_) => {
            fs::write(path, default_icon_bytes(app_data_dir))?;
            Ok(())
        }
    }
}

/* ---------- One-time move from the old shared server folder ---------- */

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}

fn move_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    if fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    copy_dir(src, dst)?;
    fs::remove_dir_all(src)
}

/// Best guess of what the old shared server ran, from the newest server jar in it.
fn detect_legacy_config(legacy: &Path) -> (Option<ServerCore>, Option<String>) {
    let mut best: Option<(u64, ServerCore, String)> = None;
    if let Ok(entries) = fs::read_dir(legacy) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let found = if let Some(v) = name
                .strip_prefix("paper-")
                .and_then(|n| n.strip_suffix(".jar"))
            {
                Some((ServerCore::Paper, v.to_string()))
            } else if let Some(v) = name
                .strip_prefix("fabric-server-")
                .and_then(|n| n.strip_suffix(".jar"))
            {
                Some((ServerCore::Fabric, v.to_string()))
            } else if let Some(v) = name
                .strip_prefix("neoforge-")
                .and_then(|n| n.strip_suffix(".version"))
            {
                Some((ServerCore::Neoforge, v.to_string()))
            } else {
                None
            };
            if let Some((core, version)) = found {
                if !is_safe_version(&version) {
                    continue;
                }
                let when = modified_ms(&entry.path()).unwrap_or(0);
                if best.as_ref().map(|b| when >= b.0).unwrap_or(true) {
                    best = Some((when, core, version));
                }
            }
        }
    }
    match best {
        Some((_, core, version)) => (Some(core), Some(version)),
        None => (None, None),
    }
}

/// Worlds used to share one server folder (one jar, one `mods/`, one `plugins/`).
/// Give every world its own folder, carrying over its save and a copy of the shared
/// mods, plugins and settings. Runs once; the old folder is kept as `server_old`.
pub fn migrate_legacy(app_data_dir: &Path) {
    let marker = instances_dir(app_data_dir).join(".migrated");
    if marker.exists() {
        return;
    }
    let _ = fs::create_dir_all(instances_dir(app_data_dir));
    let legacy = fallback_dir(app_data_dir);
    if legacy.is_dir() {
        if let Err(e) = migrate_legacy_inner(app_data_dir, &legacy) {
            eprintln!("World migration failed, will retry next start: {}", e);
            return;
        }
        let _ = fs::rename(&legacy, app_data_dir.join("server_old"));
    }
    let _ = fs::write(marker, "1");
}

fn migrate_legacy_inner(app_data_dir: &Path, legacy: &Path) -> AppResult<()> {
    let mut worlds = load_registry(app_data_dir);

    // A world made before multi-world support lives in plain `world/`.
    let legacy_level = legacy.join(LEGACY_WORLD_ID).join("level.dat");
    if legacy_level.exists() && !worlds.iter().any(|w| w.id == LEGACY_WORLD_ID) {
        worlds.insert(
            0,
            WorldMeta {
                id: LEGACY_WORLD_ID.to_string(),
                name: "Your world".to_string(),
                created_ms: modified_ms(&legacy_level).unwrap_or_else(now_ms),
                core: None,
                mc_version: None,
            },
        );
    }

    let (core, version) = detect_legacy_config(legacy);

    for w in worlds.iter_mut() {
        if !is_safe_id(&w.id) {
            continue;
        }
        let inst = instance_dir(app_data_dir, &w.id);
        if inst.join("server.properties").exists() {
            continue; // already moved by an earlier, interrupted run
        }
        fs::create_dir_all(&inst)?;

        for (suffix, target) in [
            ("", LEGACY_WORLD_ID.to_string()),
            ("_nether", format!("{}_nether", LEGACY_WORLD_ID)),
            ("_the_end", format!("{}_the_end", LEGACY_WORLD_ID)),
        ] {
            let src = legacy.join(format!("{}{}", w.id, suffix));
            let dst = inst.join(&target);
            if src.is_dir() && !dst.exists() {
                move_dir(&src, &dst)?;
            }
        }

        for dir in ["mods", "plugins", "config"] {
            let src = legacy.join(dir);
            if src.is_dir() {
                copy_dir(&src, &inst.join(dir))?;
            }
        }
        for file in [
            "ops.json",
            "whitelist.json",
            "banned-players.json",
            "banned-ips.json",
            "usercache.json",
            "eula.txt",
        ] {
            let src = legacy.join(file);
            if src.is_file() {
                fs::copy(&src, inst.join(file))?;
            }
        }
        // Server jars are big: share them on disk when possible, copy otherwise.
        if let Ok(entries) = fs::read_dir(legacy) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".jar")
                    && (name.starts_with("paper-") || name.starts_with("fabric-server-"))
                {
                    let dst = inst.join(&name);
                    if fs::hard_link(entry.path(), &dst).is_err() {
                        let _ = fs::copy(entry.path(), &dst);
                    }
                }
            }
        }

        fs::create_dir_all(inst.join("mods"))?;
        fs::create_dir_all(inst.join("plugins"))?;
        if legacy.join("server.properties").is_file() {
            fs::copy(
                legacy.join("server.properties"),
                inst.join("server.properties"),
            )?;
        } else {
            ServerProperties::load(&inst)?;
        }
        set_level_name(&inst, LEGACY_WORLD_ID)?;

        if w.core.is_none() || w.mc_version.is_none() {
            w.core = core;
            w.mc_version = version.clone();
        }
    }

    save_registry(app_data_dir, &worlds)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("aethermc_worlds_{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A valid 64x64 PNG header is all the validation looks at.
    fn png_header(w: u32, h: u32) -> String {
        use base64::Engine;
        let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
        b.extend_from_slice(b"IHDR");
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&h.to_be_bytes());
        base64::engine::general_purpose::STANDARD.encode(b)
    }

    #[test]
    fn icons_are_checked_and_stored_per_world() {
        let app = temp("icons");
        let a = create_world(&app, "A", ServerCore::Paper, "1.21.1").unwrap();
        // Until a picture is chosen the world carries the app's own icon.
        assert_eq!(fs::read(instance_dir(&app, &a.id).join(ICON_FILE)).unwrap(), APP_ICON);
        assert!(write_icon(&app, Some(&a.id), &png_header(32, 32)).is_err());
        assert!(write_icon(&app, Some(&a.id), "bm90IGEgcG5n").is_err());
        assert!(write_icon(&app, Some("../evil"), &png_header(64, 64)).is_err());
        write_icon(&app, Some(&a.id), &png_header(64, 64)).unwrap();
        assert!(instance_dir(&app, &a.id).join(ICON_FILE).exists());
        assert!(read_icon(&app, Some(&a.id)).unwrap().unwrap().starts_with("data:image/png;base64,"));
        clear_icon(&app, Some(&a.id)).unwrap();
        assert_eq!(fs::read(instance_dir(&app, &a.id).join(ICON_FILE)).unwrap(), APP_ICON);
    }

    #[test]
    fn new_worlds_start_from_the_defaults() {
        let app = temp("defaults_on_create");
        let mut defaults = ServerProperties::default();
        defaults.difficulty = "hard".to_string();
        defaults.motd = "Welcome
friends".to_string();
        crate::engine::properties::save_defaults(&app, &defaults).unwrap();
        write_icon(&app, None, &png_header(64, 64)).unwrap();

        let w = create_world(&app, "Fresh", ServerCore::Paper, "1.21.1").unwrap();
        let props = ServerProperties::load(&instance_dir(&app, &w.id)).unwrap();
        assert_eq!(props.difficulty, "hard");
        assert_eq!(props.motd, "Welcome
friends");
        assert!(instance_dir(&app, &w.id).join(ICON_FILE).exists());
        assert_ne!(fs::read(instance_dir(&app, &w.id).join(ICON_FILE)).unwrap(), APP_ICON);
    }

    #[test]
    fn worlds_have_their_own_config_and_folders() {
        let app = temp("flow");

        let a = create_world(&app, "  Survival   with friends ", ServerCore::Paper, "1.21.1").unwrap();
        let b = create_world(&app, "Modded", ServerCore::Fabric, "1.20.1").unwrap();
        assert_eq!(a.name, "Survival with friends");
        assert_ne!(a.id, b.id);
        assert!(create_world(&app, "   ", ServerCore::Paper, "1.21.1").is_err());
        assert!(create_world(&app, &"x".repeat(41), ServerCore::Paper, "1.21.1").is_err());
        assert!(create_world(&app, "Bad", ServerCore::Paper, "../1").is_err());

        // Each world has its own server folder, mods and plugins.
        let (ia, ib) = (instance_dir(&app, &a.id), instance_dir(&app, &b.id));
        assert_ne!(ia, ib);
        assert!(ia.join("mods").is_dir() && ib.join("plugins").is_dir());
        assert_eq!(level_name(&ia), LEGACY_WORLD_ID);

        // Installing something into one world does not show up in the other.
        fs::write(ib.join("mods").join("lithium.jar"), b"x").unwrap();
        fs::create_dir_all(ia.join("world")).unwrap();
        fs::write(ia.join("world").join("level.dat"), b"x").unwrap();
        let listed = list_worlds(&app, &app.join("snapshots"), Some(&b.id));
        let la = listed.iter().find(|w| w.id == a.id).unwrap();
        let lb = listed.iter().find(|w| w.id == b.id).unwrap();
        assert_eq!((la.mod_count, lb.mod_count), (0, 1));
        assert!(lb.active && !la.active);
        assert_eq!(la.core, Some(ServerCore::Paper));
        assert_eq!(lb.mc_version.as_deref(), Some("1.20.1"));

        // Changing one world's version leaves the other alone.
        update_world_config(&app, &b.id, ServerCore::Neoforge, "1.21.1").unwrap();
        let regs = load_registry(&app);
        let ra = regs.iter().find(|w| w.id == a.id).unwrap();
        let rb = regs.iter().find(|w| w.id == b.id).unwrap();
        assert_eq!(ra.core, Some(ServerCore::Paper));
        assert_eq!(ra.mc_version.as_deref(), Some("1.21.1"));
        assert_eq!(rb.core, Some(ServerCore::Neoforge));
        assert!(update_world_config(&app, "missing", ServerCore::Paper, "1.21.1").is_err());

        rename_world(&app, &a.id, "Hardcore").unwrap();
        assert!(rename_world(&app, "missing", "Nope").is_err());
        assert_eq!(
            resolve_world(&app, Some(&a.id), ServerCore::Paper, "1.21.1").unwrap().name,
            "Hardcore"
        );

        delete_world(&app, &a.id).unwrap();
        assert!(!ia.exists());
        assert!(ib.exists());
        assert!(delete_world(&app, &a.id).is_err());
        let _ = fs::remove_dir_all(&app);
    }

    #[test]
    fn new_world_can_have_a_seed_and_be_hardcore() {
        let app = temp("options");

        let plain = create_world(&app, "Plain", ServerCore::Paper, "1.21.1").unwrap();
        let props = fs::read_to_string(instance_dir(&app, &plain.id).join("server.properties")).unwrap();
        assert!(!props.contains("level-seed") && !props.contains("hardcore"));

        let opts = NewWorldOptions { seed: Some("  glacier 42 ".to_string()), hardcore: true };
        let w = create_world_with(&app, "One life", ServerCore::Paper, "1.21.1", &opts).unwrap();
        let inst = instance_dir(&app, &w.id);
        let props = fs::read_to_string(inst.join("server.properties")).unwrap();
        assert!(props.contains("level-seed=glacier 42\n"));
        assert!(props.contains("hardcore=true\n"));
        assert!(props.contains("difficulty=hard\n"));
        assert!(props.contains("level-name=world\n"));

        // Saving the world's settings later must not drop either of them.
        let saved = ServerProperties::load(&inst).unwrap();
        saved.save(&inst).unwrap();
        let props = fs::read_to_string(inst.join("server.properties")).unwrap();
        assert!(props.contains("level-seed=glacier 42\n") && props.contains("hardcore=true\n"));

        // A blank seed means random; bad seeds are refused before anything is made.
        let blank = NewWorldOptions { seed: Some("   ".to_string()), hardcore: false };
        let w = create_world_with(&app, "Blank", ServerCore::Paper, "1.21.1", &blank).unwrap();
        let props = fs::read_to_string(instance_dir(&app, &w.id).join("server.properties")).unwrap();
        assert!(!props.contains("level-seed"));
        let before = load_registry(&app).len();
        for bad in ["a\nhardcore=true".to_string(), "x".repeat(MAX_SEED_CHARS + 1)] {
            let o = NewWorldOptions { seed: Some(bad), hardcore: false };
            assert!(create_world_with(&app, "Bad", ServerCore::Paper, "1.21.1", &o).is_err());
        }
        assert_eq!(load_registry(&app).len(), before);
        let _ = fs::remove_dir_all(&app);
    }

    #[test]
    fn old_shared_server_is_split_into_worlds() {
        let app = temp("legacy");
        let server = fallback_dir(&app);
        fs::create_dir_all(server.join("world")).unwrap();
        fs::write(server.join("world").join("level.dat"), b"old").unwrap();
        fs::create_dir_all(server.join("mods")).unwrap();
        fs::write(server.join("mods").join("lithium.jar"), b"x").unwrap();
        fs::write(server.join("paper-1.20.4.jar"), b"jar").unwrap();
        fs::write(server.join("server.properties"), "motd=hi\nlevel-name=world\n").unwrap();

        migrate_legacy(&app);

        let worlds = load_registry(&app);
        assert_eq!(worlds.len(), 1);
        assert_eq!(worlds[0].id, LEGACY_WORLD_ID);
        assert_eq!(worlds[0].core, Some(ServerCore::Paper));
        assert_eq!(worlds[0].mc_version.as_deref(), Some("1.20.4"));
        let inst = instance_dir(&app, LEGACY_WORLD_ID);
        assert_eq!(fs::read(inst.join("world").join("level.dat")).unwrap(), b"old");
        assert!(inst.join("mods").join("lithium.jar").is_file());
        assert!(inst.join("paper-1.20.4.jar").is_file());
        assert!(fs::read_to_string(inst.join("server.properties")).unwrap().contains("motd=hi"));
        assert!(!server.exists() && app.join("server_old").is_dir());

        // Running it again changes nothing.
        migrate_legacy(&app);
        assert_eq!(load_registry(&app).len(), 1);
        let _ = fs::remove_dir_all(&app);
    }

    fn write_level_dat(level: &Path, data: fastnbt::Value) {
        use flate2::write::GzEncoder;
        use std::collections::HashMap;
        use std::io::Write;
        let root = fastnbt::Value::Compound(HashMap::from([("Data".to_string(), data)]));
        let bytes = fastnbt::to_bytes(&root).unwrap();
        fs::create_dir_all(level).unwrap();
        let mut enc = GzEncoder::new(
            fs::File::create(level.join("level.dat")).unwrap(),
            flate2::Compression::default(),
        );
        enc.write_all(&bytes).unwrap();
        enc.finish().unwrap();
    }

    #[test]
    fn details_of_a_world_that_was_never_started_are_empty() {
        let app = temp("details_new");
        let w = create_world(&app, "Fresh", ServerCore::Paper, "1.21.1").unwrap();

        let d = world_details(&app, &w.id).unwrap();
        assert_eq!(d.name, "Fresh");
        assert!(d.seed.is_none() && d.last_played_ms.is_none() && d.spawn.is_none());
        assert_eq!(d.total_playtime_secs, 0);
        assert!(d.players.is_empty() && d.mods.is_empty() && d.plugins.is_empty());

        assert!(world_details(&app, "../evil").is_err());
        assert!(world_details(&app, "nope").is_err());
        let _ = fs::remove_dir_all(&app);
    }

    #[test]
    fn details_read_level_dat_stats_and_jars() {
        use fastnbt::Value;
        use std::collections::HashMap;

        let app = temp("details_played");
        let w = create_world(&app, "Played", ServerCore::Fabric, "1.21.1").unwrap();
        let inst = instance_dir(&app, &w.id);
        let level = inst.join(LEGACY_WORLD_ID);

        write_level_dat(
            &level,
            Value::Compound(HashMap::from([
                (
                    "WorldGenSettings".to_string(),
                    Value::Compound(HashMap::from([("seed".to_string(), Value::Long(-4172144997902289642))])),
                ),
                ("hardcore".to_string(), Value::Byte(1)),
                ("Time".to_string(), Value::Long(48000)),
                ("LastPlayed".to_string(), Value::Long(1_700_000_000_000)),
                ("SpawnX".to_string(), Value::Int(10)),
                ("SpawnY".to_string(), Value::Int(64)),
                ("SpawnZ".to_string(), Value::Int(-5)),
                (
                    "Version".to_string(),
                    Value::Compound(HashMap::from([("Name".to_string(), Value::String("1.21.1".to_string()))])),
                ),
            ])),
        );

        fs::create_dir_all(level.join("stats")).unwrap();
        let (a, b, c) = (
            "11111111-1111-1111-1111-111111111111",
            "22222222-2222-2222-2222-222222222222",
            "33333333-3333-3333-3333-333333333333",
        );
        fs::write(
            level.join("stats").join(format!("{}.json", a)),
            r#"{"stats":{"minecraft:custom":{"minecraft:play_time":72000}},"DataVersion":3955}"#,
        )
        .unwrap();
        // Old name of the same counter.
        fs::write(
            level.join("stats").join(format!("{}.json", b)),
            r#"{"stat.playOneMinute":1,"stats":{"minecraft:custom":{"minecraft:play_one_minute":24000}}}"#,
        )
        .unwrap();
        // Broken file must be skipped, not fail the whole window.
        fs::write(level.join("stats").join(format!("{}.json", c)), "{not json").unwrap();
        fs::write(
            inst.join("usercache.json"),
            format!(r#"[{{"name":"Steve","uuid":"{}","expiresOn":"x"}}]"#, a),
        )
        .unwrap();
        fs::write(inst.join("mods").join("Lithium.jar"), b"12345").unwrap();
        fs::write(inst.join("mods").join("notes.txt"), b"x").unwrap();

        let d = world_details(&app, &w.id).unwrap();
        assert_eq!(d.seed.as_deref(), Some("-4172144997902289642"));
        assert!(d.hardcore);
        assert_eq!(d.world_age_ticks, Some(48000));
        assert_eq!(d.last_played_ms, Some(1_700_000_000_000));
        assert_eq!(d.spawn, Some([10, 64, -5]));
        assert_eq!(d.game_version.as_deref(), Some("1.21.1"));

        assert_eq!(d.players.len(), 2);
        assert_eq!(d.players[0].name.as_deref(), Some("Steve"));
        assert_eq!(d.players[0].playtime_secs, 3600);
        assert_eq!(d.players[1].playtime_secs, 1200);
        assert_eq!(d.total_playtime_secs, 4800);

        assert_eq!(d.mods.len(), 1);
        assert_eq!(d.mods[0].filename, "Lithium.jar");
        assert_eq!(d.mods[0].size_bytes, 5);
        let _ = fs::remove_dir_all(&app);
    }

    #[test]
    fn old_worlds_keep_their_seed_in_random_seed() {
        use fastnbt::Value;
        use std::collections::HashMap;

        let app = temp("details_old_seed");
        let w = create_world(&app, "Old", ServerCore::Paper, "1.12.2").unwrap();
        write_level_dat(
            &instance_dir(&app, &w.id).join(LEGACY_WORLD_ID),
            Value::Compound(HashMap::from([("RandomSeed".to_string(), Value::Long(42))])),
        );
        assert_eq!(world_details(&app, &w.id).unwrap().seed.as_deref(), Some("42"));
        let _ = fs::remove_dir_all(&app);
    }

    #[test]
    fn saving_world_properties_checks_values_and_stays_in_that_world() {
        let app = temp("details_save");
        let a = create_world(&app, "A", ServerCore::Paper, "1.21.1").unwrap();
        let b = create_world(&app, "B", ServerCore::Paper, "1.21.1").unwrap();

        let mut props = world_details(&app, &a.id).unwrap().properties;
        props.difficulty = "hard".to_string();
        props.gamemode = "creative".to_string();
        let saved = save_world_properties(&app, &a.id, &props).unwrap();
        assert_eq!(saved.difficulty, "hard");
        assert_eq!(world_details(&app, &a.id).unwrap().properties.gamemode, "creative");
        // The other world is untouched, and level-name survives the save.
        assert_eq!(world_details(&app, &b.id).unwrap().properties.difficulty, "normal");
        assert_eq!(level_name(&instance_dir(&app, &a.id)), LEGACY_WORLD_ID);

        let mut bad = props.clone();
        bad.gamemode = "godmode".to_string();
        assert!(save_world_properties(&app, &a.id, &bad).is_err());
        let mut bad = props.clone();
        bad.motd = "hi\rlevel-name=evil".to_string();
        assert!(save_world_properties(&app, &a.id, &bad).is_err());
        // A line break is a real second MOTD line now; it is escaped, so it can't
        // start a new setting in the file.
        let mut two_lines = props.clone();
        two_lines.motd = "hi\nlevel-name=evil".to_string();
        let saved = save_world_properties(&app, &a.id, &two_lines).unwrap();
        assert_eq!(saved.motd, "hi\nlevel-name=evil");
        assert_eq!(level_name(&instance_dir(&app, &a.id)), LEGACY_WORLD_ID);
        assert!(save_world_properties(&app, "../x", &props).is_err());
        let _ = fs::remove_dir_all(&app);
    }
}
