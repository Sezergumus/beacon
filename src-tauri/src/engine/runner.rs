use crate::engine::jvm::{calculate_optimal_ram_mb, ensure_portable_jre, generate_jvm_flags};
use crate::engine::nbt_inspector::{offline_player_uuid, spawn_playerdata_watcher};
use crate::engine::properties::ServerProperties;
use crate::engine::versions::required_java_major;
use crate::error::{AppError, AppResult};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerCore {
    Paper,
    Fabric,
    Neoforge,
}

impl Default for ServerCore {
    fn default() -> Self {
        Self::Paper
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerStatus {
    Stopped,
    Provisioning,
    Starting,
    Running,
    Stopping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlinePlayer {
    pub username: String,
    pub uuid: String,
    pub is_op: bool,
    pub gamemode: String,
    pub ping_ms: u32,
    pub joined_at_ms: u64,
    pub coords: [f64; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerLogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerTelemetry {
    pub status: ServerStatus,
    pub core: ServerCore,
    pub mc_version: String,
    pub tps: f32,
    pub ram_used_mb: u64,
    pub ram_allocated_mb: u64,
    pub cpu_usage_percent: f32,
    pub uptime_secs: u64,
    pub online_players: Vec<OnlinePlayer>,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerLaunchConfig {
    pub core: ServerCore,
    pub mc_version: String,
    pub allocated_ram_mb: Option<u64>,
    pub java_major: Option<u8>,
    /// Which world to open. `None` reuses the last one (or makes a first world).
    #[serde(default)]
    pub world_id: Option<String>,
}

/// Default time between automatic saves of a running world.
pub const DEFAULT_AUTOSAVE_SECS: u64 = 600;

pub struct ServerRunner {
    pub app_data_dir: PathBuf,
    /// The world whose server folder everything (mods, settings, players) works on.
    active_world: std::sync::RwLock<Option<String>>,
    pub status: Arc<Mutex<ServerStatus>>,
    pub core: Arc<Mutex<ServerCore>>,
    pub mc_version: Arc<Mutex<String>>,
    pub allocated_ram_mb: Arc<Mutex<u64>>,
    pub child: Arc<Mutex<Option<Child>>>,
    pub stdin: Arc<Mutex<Option<ChildStdin>>>,
    pub child_pid: Arc<Mutex<Option<u32>>>,
    pub online_players: Arc<Mutex<HashMap<String, OnlinePlayer>>>,
    pub known_uuids: Arc<Mutex<HashMap<String, String>>>,
    pub current_tps: Arc<Mutex<f32>>,
    pub started_at: Arc<Mutex<Option<Instant>>>,
    pub logs: Arc<Mutex<Vec<ServerLogEntry>>>,
    /// How often a running world with players is saved to disk, in seconds. 0 = never
    /// (the game still autosaves on its own). Changed from Settings.
    pub autosave_secs: Arc<AtomicU64>,
}

impl ServerRunner {
    pub fn new(app_data_dir: PathBuf) -> Self {
        // Pick up the world used last time (if it still exists).
        let active_world = crate::engine::worlds::read_active_world(&app_data_dir);
        let server_dir = match &active_world {
            Some(id) => crate::engine::worlds::instance_dir(&app_data_dir, id),
            None => crate::engine::worlds::fallback_dir(&app_data_dir),
        };
        let _ = fs::create_dir_all(server_dir.join("mods"));
        let _ = fs::create_dir_all(server_dir.join("plugins"));

        let mut sys = System::new_all();
        sys.refresh_memory();
        let free_mb = (sys.available_memory() / (1024 * 1024)).max(4096);
        let optimal_ram = calculate_optimal_ram_mb(free_mb);

        Self {
            app_data_dir,
            active_world: std::sync::RwLock::new(active_world),
            status: Arc::new(Mutex::new(ServerStatus::Stopped)),
            core: Arc::new(Mutex::new(ServerCore::Paper)),
            mc_version: Arc::new(Mutex::new("1.21.1".to_string())),
            allocated_ram_mb: Arc::new(Mutex::new(optimal_ram)),
            child: Arc::new(Mutex::new(None)),
            stdin: Arc::new(Mutex::new(None)),
            child_pid: Arc::new(Mutex::new(None)),
            online_players: Arc::new(Mutex::new(HashMap::new())),
            known_uuids: Arc::new(Mutex::new(HashMap::new())),
            current_tps: Arc::new(Mutex::new(20.0)),
            started_at: Arc::new(Mutex::new(None)),
            logs: Arc::new(Mutex::new(Vec::new())),
            autosave_secs: Arc::new(AtomicU64::new(DEFAULT_AUTOSAVE_SECS)),
        }
    }

    /// The server folder of the active world (or a plain fallback folder while no world exists).
    pub fn server_dir(&self) -> PathBuf {
        match self.active_world.read().ok().and_then(|g| g.clone()) {
            Some(id) => crate::engine::worlds::instance_dir(&self.app_data_dir, &id),
            None => crate::engine::worlds::fallback_dir(&self.app_data_dir),
        }
    }

    pub fn active_world_id(&self) -> Option<String> {
        self.active_world.read().ok().and_then(|g| g.clone())
    }

    /// Make `id` the world that mods, settings and players refer to. Only the world
    /// the server isn't currently running can change, so callers check the status first.
    pub fn set_active_world(&self, id: Option<&str>) {
        if let Ok(mut guard) = self.active_world.write() {
            *guard = id.map(|s| s.to_string());
        }
        crate::engine::worlds::write_active_world(&self.app_data_dir, id);
        let dir = self.server_dir();
        let _ = fs::create_dir_all(dir.join("mods"));
        let _ = fs::create_dir_all(dir.join("plugins"));
    }

    pub async fn emit_log(&self, app_handle: Option<&AppHandle>, level: &str, message: impl Into<String>) {
        let msg = message.into();
        let entry = ServerLogEntry {
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
            level: level.to_string(),
            message: msg,
        };
        {
            let mut logs = self.logs.lock().await;
            logs.push(entry.clone());
            if logs.len() > 1000 {
                let drain_count = logs.len() - 1000;
                logs.drain(0..drain_count);
            }
        }
        if let Some(app) = app_handle {
            let _ = app.emit("server:log", entry);
        }
    }

    /// Provision the selected Server Core (PaperMC, Fabric + Lithium/FerriteCore, or NeoForge).
    ///
    /// Returns the JVM arguments that launch it (everything after the memory/GC flags).
    pub async fn ensure_server_core(
        &self,
        core: ServerCore,
        mc_version: &str,
        java_bin: &Path,
        app_handle: Option<&AppHandle>,
    ) -> AppResult<Vec<String>> {
        tokio::fs::create_dir_all(&self.server_dir()).await?;

        if core == ServerCore::Neoforge {
            return self.ensure_neoforge(mc_version, java_bin, app_handle).await;
        }

        let jar_name = match core {
            ServerCore::Paper => format!("paper-{}.jar", mc_version),
            _ => format!("fabric-server-{}.jar", mc_version),
        };
        let jar_path = self.server_dir().join(&jar_name);
        let jar_launch_args = |p: &Path| {
            vec![
                "-jar".to_string(),
                p.to_string_lossy().to_string(),
                "nogui".to_string(),
            ]
        };

        if jar_path.exists() && fs::metadata(&jar_path).map(|m| m.len() > 100_000).unwrap_or(false) {
            if core == ServerCore::Fabric {
                let _ = self.auto_install_fabric_optimizations(mc_version, app_handle).await;
            }
            return Ok(jar_launch_args(&jar_path));
        }

        let client = reqwest::Client::builder()
            .user_agent("Beacon/0.1.0 (Minecraft server manager)")
            .build()?;

        match core {
            ServerCore::Paper => {
                self.emit_log(
                    app_handle,
                    "INFO",
                    format!("Looking for the newest Paper build for Minecraft {}...", mc_version),
                )
                .await;

                // PaperMC "Fill" v3 API (the old api.papermc.io/v2 was shut down).
                let build_url = format!(
                    "https://fill.papermc.io/v3/projects/paper/versions/{}/builds/latest",
                    mc_version
                );
                let resp = client.get(&build_url).send().await?;
                if resp.status() == reqwest::StatusCode::NOT_FOUND {
                    return Err(AppError::Content(format!(
                        "Paper doesn't have a build for Minecraft {} yet. Try a different version.",
                        mc_version
                    )));
                }
                if !resp.status().is_success() {
                    return Err(AppError::Network(format!(
                        "Couldn't reach the Paper website (HTTP {}). Check your internet connection and try again.",
                        resp.status()
                    )));
                }
                let build: serde_json::Value = resp.json().await?;
                let download = &build["downloads"]["server:default"];
                let download_url = download["url"]
                    .as_str()
                    .ok_or_else(|| AppError::Content("Paper's download link is missing.".to_string()))?;
                let file_name = download["name"].as_str().unwrap_or("paper.jar");
                let expected_sha256 = download["checksums"]["sha256"].as_str().unwrap_or("");

                self.emit_log(
                    app_handle,
                    "INFO",
                    format!("Downloading {}...", file_name),
                )
                .await;

                download_with_sha256(&client, download_url, &jar_path, Some(expected_sha256)).await?;
            }
            ServerCore::Fabric => {
                self.emit_log(
                    app_handle,
                    "INFO",
                    format!("Fetching Fabric Server Launcher for Minecraft {}...", mc_version),
                )
                .await;

                // Query latest loader and installer versions from Fabric Meta v2
                let loaders: serde_json::Value = client
                    .get("https://meta.fabricmc.net/v2/versions/loader")
                    .send()
                    .await?
                    .json()
                    .await?;
                let loader_ver = loaders
                    .as_array()
                    .and_then(|a| a.iter().find(|v| v["stable"].as_bool().unwrap_or(false)).or(a.first()))
                    .and_then(|v| v["version"].as_str())
                    .unwrap_or("0.16.9");

                let installers: serde_json::Value = client
                    .get("https://meta.fabricmc.net/v2/versions/installer")
                    .send()
                    .await?
                    .json()
                    .await?;
                let installer_ver = installers
                    .as_array()
                    .and_then(|a| a.iter().find(|v| v["stable"].as_bool().unwrap_or(false)).or(a.first()))
                    .and_then(|v| v["version"].as_str())
                    .unwrap_or("1.0.1");

                let fabric_url = format!(
                    "https://meta.fabricmc.net/v2/versions/loader/{}/{}/{}/server/jar",
                    mc_version, loader_ver, installer_ver
                );

                download_with_sha256(&client, &fabric_url, &jar_path, None).await?;

                // Auto-install Lithium and FerriteCore by default to cut RAM usage by 40% (Section 3.2.2)
                let _ = self.auto_install_fabric_optimizations(mc_version, app_handle).await;
            }
            ServerCore::Neoforge => unreachable!("handled above"),
        }

        Ok(jar_launch_args(&jar_path))
    }

    /// Automatically install `Lithium` and `FerriteCore` from Modrinth API when Fabric is selected.
    async fn auto_install_fabric_optimizations(
        &self,
        mc_version: &str,
        app_handle: Option<&AppHandle>,
    ) -> AppResult<()> {
        let mods_dir = self.server_dir().join("mods");
        tokio::fs::create_dir_all(&mods_dir).await?;

        for slug in ["lithium", "ferrite-core", "fabric-api"] {
            let already_installed = fs::read_dir(&mods_dir)
                .ok()
                .map(|entries| {
                    entries.flatten().any(|e| {
                        e.file_name()
                            .to_string_lossy()
                            .to_lowercase()
                            .contains(&slug.replace('-', ""))
                            || e.file_name().to_string_lossy().to_lowercase().contains(slug)
                    })
                })
                .unwrap_or(false);

            if already_installed {
                continue;
            }

            self.emit_log(
                app_handle,
                "INFO",
                format!(
                    "Auto-installing optimization mod '{}' from Modrinth for Fabric {}...",
                    slug, mc_version
                ),
            )
            .await;

            if let Err(e) = crate::content::modrinth::install_modrinth_project(
                &self.server_dir(),
                slug,
                mc_version,
                "fabric",
                "mod",
            )
            .await
            {
                self.emit_log(
                    app_handle,
                    "WARN",
                    format!("Optional optimization mod '{}' skipped: {}", slug, e),
                )
                .await;
            }
        }
        Ok(())
    }

    /// Start the Minecraft Java Server on `127.0.0.1:25565`.
    pub async fn start_server(
        &self,
        config: ServerLaunchConfig,
        app_handle: AppHandle,
    ) -> AppResult<()> {
        {
            let current_status = *self.status.lock().await;
            if current_status != ServerStatus::Stopped {
                return Err(AppError::Process(format!(
                    "Server is already in {:?} state",
                    current_status
                )));
            }
            *self.status.lock().await = ServerStatus::Provisioning;
        }
        let _ = app_handle.emit("server:status", ServerStatus::Provisioning);

        // Every world has its own server folder and its own version/type. Open the
        // requested world and use *its* settings; the launch config only fills in a
        // world that has none yet (worlds made by older versions of the app).
        let world = (|| -> AppResult<crate::engine::worlds::WorldMeta> {
            let requested = config.world_id.clone().or_else(|| self.active_world_id());
            let mut world = crate::engine::worlds::resolve_world(
                &self.app_data_dir,
                requested.as_deref(),
                config.core,
                &config.mc_version,
            )?;
            if world.core.is_none() || world.mc_version.is_none() {
                let core = world.core.unwrap_or(config.core);
                let version = world.mc_version.clone().unwrap_or_else(|| config.mc_version.clone());
                crate::engine::worlds::update_world_config(&self.app_data_dir, &world.id, core, &version)?;
                world.core = Some(core);
                world.mc_version = Some(version);
            }
            Ok(world)
        })();
        let world = match world {
            Ok(w) => w,
            Err(e) => {
                *self.status.lock().await = ServerStatus::Stopped;
                let _ = app_handle.emit("server:status", ServerStatus::Stopped);
                self.emit_log(Some(&app_handle), "ERROR", format!("Startup failed: {}", e))
                    .await;
                return Err(e);
            }
        };
        self.set_active_world(Some(&world.id));
        let core = world.core.unwrap_or(config.core);
        let mc_version = world
            .mc_version
            .clone()
            .unwrap_or_else(|| config.mc_version.clone());

        let java_major = config
            .java_major
            .unwrap_or_else(|| required_java_major(&mc_version));

        let mut sys = System::new_all();
        sys.refresh_memory();
        let free_mb = (sys.available_memory() / (1024 * 1024)).max(3072);
        let ram_mb = config
            .allocated_ram_mb
            .unwrap_or_else(|| calculate_optimal_ram_mb(free_mb));

        *self.core.lock().await = core;
        *self.mc_version.lock().await = mc_version.clone();
        *self.allocated_ram_mb.lock().await = ram_mb;

        let result: AppResult<()> = async {
            self.emit_log(
                Some(&app_handle),
                "INFO",
                format!(
                    "Checking Portable Java {} runtime via Adoptium API...",
                    java_major
                ),
            )
            .await;

            let java_bin =
                ensure_portable_jre(&self.app_data_dir, java_major, Some(&app_handle)).await?;

            let launch_args = self
                .ensure_server_core(core, &mc_version, &java_bin, Some(&app_handle))
                .await?;

            // Every world shows an icon in the server list.
            crate::engine::worlds::ensure_icon(&self.app_data_dir, &world.id);

            // Auto-accept eula.txt
            let eula_path = self.server_dir().join("eula.txt");
            fs::write(
                &eula_path,
                "#By changing the setting below to TRUE you are indicating your agreement to our EULA (https://aka.ms/MinecraftEULA).\neula=true\n",
            )?;

            // Ensure server.properties binds strictly to 127.0.0.1:25565
            let props = ServerProperties::load(&self.server_dir())?;
            props.save(&self.server_dir())?;

            // The world lives in `world/` inside this world's own server folder.
            crate::engine::worlds::set_level_name(
                &self.server_dir(),
                crate::engine::worlds::LEGACY_WORLD_ID,
            )?;
            self.emit_log(
                Some(&app_handle),
                "INFO",
                format!("Opening world \"{}\"...", world.name),
            )
            .await;

            *self.status.lock().await = ServerStatus::Starting;
            let _ = app_handle.emit("server:status", ServerStatus::Starting);

            let mut jvm_args = generate_jvm_flags(ram_mb, java_major);
            jvm_args.extend(launch_args);

            self.emit_log(
                Some(&app_handle),
                "INFO",
                format!(
                    "Spawning {:?} {} with {} MB RAM on 127.0.0.1:25565...",
                    core, mc_version, ram_mb
                ),
            )
            .await;

            let mut cmd = Command::new(&java_bin);
            cmd.current_dir(&self.server_dir())
                .args(&jvm_args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            #[cfg(windows)]
            {
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }

            let mut child = cmd.spawn().map_err(|e| {
                AppError::Process(format!("Failed to spawn Java process: {}", e))
            })?;

            let pid = child.id();
            *self.child_pid.lock().await = pid;
            *self.started_at.lock().await = Some(Instant::now());

            let stdin = child.stdin.take();
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            *self.stdin.lock().await = stdin;
            *self.child.lock().await = Some(child);

            // Start playerdata file watcher for live NBT inventory updates
            spawn_playerdata_watcher(self.server_dir().clone(), app_handle.clone());

            // Stream stdout
            if let Some(out) = stdout {
                let status_arc = self.status.clone();
                let players_arc = self.online_players.clone();
                let uuids_arc = self.known_uuids.clone();
                let tps_arc = self.current_tps.clone();
                let logs_arc = self.logs.clone();
                let stdin_arc = self.stdin.clone();
                let server_dir_clone = self.server_dir().clone();
                let app_clone = app_handle.clone();

                tokio::spawn(async move {
                    let mut reader = BufReader::new(out).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        process_server_log_line(
                            &line,
                            "INFO",
                            &status_arc,
                            &players_arc,
                            &uuids_arc,
                            &tps_arc,
                            &logs_arc,
                            &stdin_arc,
                            &server_dir_clone,
                            &app_clone,
                        )
                        .await;
                    }
                });
            }

            // Stream stderr
            if let Some(err) = stderr {
                let status_arc = self.status.clone();
                let players_arc = self.online_players.clone();
                let uuids_arc = self.known_uuids.clone();
                let tps_arc = self.current_tps.clone();
                let logs_arc = self.logs.clone();
                let stdin_arc = self.stdin.clone();
                let server_dir_clone = self.server_dir().clone();
                let app_clone = app_handle.clone();

                tokio::spawn(async move {
                    let mut reader = BufReader::new(err).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        process_server_log_line(
                            &line,
                            "WARN",
                            &status_arc,
                            &players_arc,
                            &uuids_arc,
                            &tps_arc,
                            &logs_arc,
                            &stdin_arc,
                            &server_dir_clone,
                            &app_clone,
                        )
                        .await;
                    }
                });
            }

            // Spawn exit & telemetry monitor task
            let status_arc = self.status.clone();
            let child_arc = self.child.clone();
            let pid_arc = self.child_pid.clone();
            let players_arc = self.online_players.clone();
            let started_arc = self.started_at.clone();
            let core_arc = self.core.clone();
            let ver_arc = self.mc_version.clone();
            let ram_arc = self.allocated_ram_mb.clone();
            let tps_arc = self.current_tps.clone();
            let stdin_arc = self.stdin.clone();
            let autosave_arc = self.autosave_secs.clone();
            let app_clone = app_handle.clone();

            tokio::spawn(async move {
                let mut sys = System::new_all();
                let mut last_flush = Instant::now();

                loop {
                    tokio::time::sleep(Duration::from_secs(2)).await;

                    // Check if child has exited
                    let mut exited = false;
                    {
                        let mut child_guard = child_arc.lock().await;
                        if let Some(child_proc) = child_guard.as_mut() {
                            if let Ok(Some(_exit_status)) = child_proc.try_wait() {
                                exited = true;
                                *child_guard = None;
                            }
                        } else {
                            exited = true;
                        }
                    }

                    if exited {
                        *status_arc.lock().await = ServerStatus::Stopped;
                        *pid_arc.lock().await = None;
                        *started_arc.lock().await = None;
                        *stdin_arc.lock().await = None;
                        players_arc.lock().await.clear();
                        let _ = app_clone.emit("server:status", ServerStatus::Stopped);
                        break;
                    }

                    // Periodic `save-all flush` while players are online. The interval is a
                    // user setting; every flush shows up as a chat message for admins, so the
                    // default is deliberately sparse. Inventory/world views flush on demand.
                    let player_list: Vec<OnlinePlayer> =
                        players_arc.lock().await.values().cloned().collect();
                    let autosave = autosave_arc.load(Ordering::Relaxed);
                    if autosave > 0 && last_flush.elapsed() >= Duration::from_secs(autosave) {
                        last_flush = Instant::now();
                        if !player_list.is_empty() {
                            if let Some(stdin) = stdin_arc.lock().await.as_mut() {
                                let _ = stdin.write_all(b"save-all flush\n").await;
                                let _ = stdin.flush().await;
                            }
                        }
                    }

                    let mut ram_used_mb = 0;
                    let mut cpu_usage_percent = 0.0;
                    if let Some(pid_val) = *pid_arc.lock().await {
                        let sys_pid = Pid::from_u32(pid_val);
                        sys.refresh_processes(ProcessesToUpdate::Some(&[sys_pid]), true);
                        if let Some(proc_info) = sys.process(sys_pid) {
                            ram_used_mb = proc_info.memory() / (1024 * 1024);
                            // sysinfo reports 100% per core; scale to a 0-100% share of the whole CPU
                            cpu_usage_percent =
                                proc_info.cpu_usage() / sys.cpus().len().max(1) as f32;
                        }
                    }

                    let uptime_secs = started_arc
                        .lock()
                        .await
                        .map(|t| t.elapsed().as_secs())
                        .unwrap_or(0);

                    let telemetry = ServerTelemetry {
                        status: *status_arc.lock().await,
                        core: *core_arc.lock().await,
                        mc_version: ver_arc.lock().await.clone(),
                        tps: *tps_arc.lock().await,
                        ram_used_mb,
                        ram_allocated_mb: *ram_arc.lock().await,
                        cpu_usage_percent,
                        uptime_secs,
                        online_players: player_list,
                        pid: *pid_arc.lock().await,
                    };

                    let _ = app_clone.emit("server:telemetry", telemetry);
                }
            });

            Ok(())
        }
        .await;

        if let Err(ref e) = result {
            *self.status.lock().await = ServerStatus::Stopped;
            let _ = app_handle.emit("server:status", ServerStatus::Stopped);
            self.emit_log(Some(&app_handle), "ERROR", format!("Startup failed: {}", e))
                .await;
        }

        result
    }

    /// Send a raw command directly to the running JVM child process `stdin`.
    pub async fn send_command(&self, command: &str) -> AppResult<()> {
        let mut stdin_guard = self.stdin.lock().await;
        let stdin = stdin_guard
            .as_mut()
            .ok_or_else(|| AppError::Process("Server is not running".to_string()))?;

        let formatted = format!("{}\n", command.trim());
        stdin.write_all(formatted.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    /// Execute 1-Click Player Action Button commands per Section 3.4.3 & 3.4.4.
    pub async fn execute_player_action(
        &self,
        player: &str,
        action: &str,
        payload: Option<String>,
    ) -> AppResult<()> {
        match action {
            "op" => {
                self.send_command(&format!("op {}", player)).await?;
                if let Some(p) = self.online_players.lock().await.get_mut(player) {
                    p.is_op = true;
                }
            }
            "deop" => {
                self.send_command(&format!("deop {}", player)).await?;
                if let Some(p) = self.online_players.lock().await.get_mut(player) {
                    p.is_op = false;
                }
            }
            "heal_feed" => {
                self.send_command(&format!("effect give {} minecraft:instant_health 1 255", player))
                    .await?;
                self.send_command(&format!("effect give {} minecraft:saturation 1 255", player))
                    .await?;
            }
            "kill" => {
                self.send_command(&format!("kill {}", player)).await?;
            }
            "clear" => {
                self.send_command(&format!("clear {}", player)).await?;
                self.send_command("save-all flush").await?;
            }
            "gamemode" => {
                let mode = payload.unwrap_or_else(|| "survival".to_string());
                self.send_command(&format!("gamemode {} {}", mode, player))
                    .await?;
                if let Some(p) = self.online_players.lock().await.get_mut(player) {
                    p.gamemode = mode;
                }
            }
            "tp" => {
                let target = payload.ok_or_else(|| {
                    AppError::Process("Teleport target player or coordinates required".to_string())
                })?;
                self.send_command(&format!("tp {} {}", player, target))
                    .await?;
            }
            "kick" => {
                let reason = payload.unwrap_or_else(|| "Kicked by Host".to_string());
                self.send_command(&format!("kick {} {}", player, reason))
                    .await?;
            }
            "ban" => {
                let reason = payload.unwrap_or_else(|| "Banned by Host".to_string());
                self.send_command(&format!("ban {} {}", player, reason))
                    .await?;
            }
            "delete_slot" => {
                let slot = payload.ok_or_else(|| {
                    AppError::Process("Inventory slot name required".to_string())
                })?;
                self.send_command(&format!(
                    "item replace entity {} {} with minecraft:air",
                    player, slot
                ))
                .await?;
                self.send_command("save-all flush").await?;
            }
            "give_item" => {
                let item_spec = payload.ok_or_else(|| {
                    AppError::Process("Item ID and count required".to_string())
                })?;
                // item_spec format: "<item_id> <count>"
                self.send_command(&format!("give {} {}", player, item_spec))
                    .await?;
                self.send_command("save-all flush").await?;
            }
            _ => {
                return Err(AppError::Process(format!(
                    "Unknown player action: {}",
                    action
                )));
            }
        }
        Ok(())
    }

    /// Gracefully flush chunks and stop the Minecraft server (`save-all flush` -> `stop\n`).
    pub async fn stop_server(&self, app_handle: Option<&AppHandle>) -> AppResult<()> {
        {
            let current = *self.status.lock().await;
            if current == ServerStatus::Stopped || current == ServerStatus::Stopping {
                return Ok(());
            }
            *self.status.lock().await = ServerStatus::Stopping;
        }
        if let Some(app) = app_handle {
            let _ = app.emit("server:status", ServerStatus::Stopping);
        }

        self.emit_log(
            app_handle,
            "INFO",
            "Saving chunks and initiating graceful server shutdown...",
        )
        .await;

        let _ = self.send_command("save-all flush").await;
        tokio::time::sleep(Duration::from_millis(400)).await;
        let _ = self.send_command("stop").await;

        // Wait up to 12 seconds for clean exit before forcefully killing
        for _ in 0..24 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if *self.status.lock().await == ServerStatus::Stopped {
                return Ok(());
            }
        }

        if let Some(mut child) = self.child.lock().await.take() {
            let _ = child.kill().await;
        }
        *self.status.lock().await = ServerStatus::Stopped;
        if let Some(app) = app_handle {
            let _ = app.emit("server:status", ServerStatus::Stopped);
        }
        Ok(())
    }

    /// Synchronous emergency shutdown invoked by `RunEvent::ExitRequested` to guarantee zero orphan `java.exe`.
    pub fn blocking_shutdown_on_exit(&self) {
        if let Ok(mut stdin_guard) = self.stdin.try_lock() {
            if let Some(mut stdin) = stdin_guard.take() {
                let _ = tokio::runtime::Handle::try_current().map(|h| {
                    h.block_on(async {
                        let _ = stdin.write_all(b"save-all\nstop\n").await;
                        let _ = stdin.flush().await;
                    });
                });
            }
        }
        if let Ok(mut child_guard) = self.child.try_lock() {
            if let Some(mut child) = child_guard.take() {
                let _ = child.start_kill();
            }
        }
    }

    pub async fn get_telemetry(&self) -> ServerTelemetry {
        let players: Vec<OnlinePlayer> = self.online_players.lock().await.values().cloned().collect();
        let uptime_secs = self
            .started_at
            .lock()
            .await
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0);

        ServerTelemetry {
            status: *self.status.lock().await,
            core: *self.core.lock().await,
            mc_version: self.mc_version.lock().await.clone(),
            tps: *self.current_tps.lock().await,
            ram_used_mb: 0,
            ram_allocated_mb: *self.allocated_ram_mb.lock().await,
            cpu_usage_percent: 0.0,
            uptime_secs,
            online_players: players,
            pid: *self.child_pid.lock().await,
        }
    }
}

/// Removes ANSI escape sequences (`ESC [ ... <letter>`) from a log line.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&n) = chars.peek() {
                    chars.next();
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

async fn process_server_log_line(
    line: &str,
    default_level: &str,
    status_arc: &Arc<Mutex<ServerStatus>>,
    players_arc: &Arc<Mutex<HashMap<String, OnlinePlayer>>>,
    uuids_arc: &Arc<Mutex<HashMap<String, String>>>,
    tps_arc: &Arc<Mutex<f32>>,
    logs_arc: &Arc<Mutex<Vec<ServerLogEntry>>>,
    stdin_arc: &Arc<Mutex<Option<ChildStdin>>>,
    server_dir: &Path,
    app_handle: &AppHandle,
) {
    // Paper may emit ANSI colour codes (e.g. ESC[38;2;255;255;85m); strip them
    // so they don't leak into parsed values such as player names.
    let line = strip_ansi(line);
    let line = line.as_str();

    let level = if line.contains("WARN") {
        "WARN"
    } else if line.contains("ERROR") || line.contains("Exception") {
        "ERROR"
    } else {
        default_level
    };

    let entry = ServerLogEntry {
        timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        level: level.to_string(),
        message: line.to_string(),
    };

    {
        let mut logs = logs_arc.lock().await;
        logs.push(entry.clone());
        if logs.len() > 1000 {
            let drain = logs.len() - 1000;
            logs.drain(0..drain);
        }
    }
    let _ = app_handle.emit("server:log", entry);

    // 1. Detect server ready ("Done (...s)! For help, type \"help\"")
    if line.contains("Done (") && line.contains("For help, type") {
        *status_arc.lock().await = ServerStatus::Running;
        let _ = app_handle.emit("server:status", ServerStatus::Running);

        // Apply Keep Inventory gamerule from ServerProperties
        if let Ok(props) = ServerProperties::load(server_dir) {
            if let Some(stdin) = stdin_arc.lock().await.as_mut() {
                let cmd = format!("gamerule keepInventory {}\n", props.keep_inventory);
                let _ = stdin.write_all(cmd.as_bytes()).await;
                let _ = stdin.flush().await;
            }
        }
    }

    // 2. Parse UUID of player ("UUID of player <name> is <uuid>")
    if let Some(idx) = line.find("UUID of player ") {
        let rest = &line[idx + "UUID of player ".len()..];
        if let Some((name, uuid_part)) = rest.split_once(" is ") {
            let username = name.trim().to_string();
            let uuid = uuid_part.trim().to_string();
            uuids_arc.lock().await.insert(username, uuid);
        }
    }

    // 3. Parse player join ("logged in with entity id" or "joined the game")
    if line.contains("joined the game") {
        if let Some(before) = line.split(" joined the game").next() {
            if let Some(username_raw) = before.split_whitespace().last() {
                let username = username_raw.trim_matches(|c: char| !c.is_alphanumeric() && c != '_').to_string();
                if !username.is_empty() {
                    let uuid = uuids_arc
                        .lock()
                        .await
                        .get(&username)
                        .cloned()
                        .unwrap_or_else(|| offline_player_uuid(&username));
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;

                    let player = OnlinePlayer {
                        username: username.clone(),
                        uuid,
                        is_op: false,
                        gamemode: "survival".to_string(),
                        ping_ms: 18,
                        joined_at_ms: now_ms,
                        coords: [0.0, 64.0, 0.0],
                    };
                    players_arc.lock().await.insert(username, player);
                }
            }
        }
    }

    // 4. Parse player leave ("left the game")
    if line.contains("left the game") {
        if let Some(before) = line.split(" left the game").next() {
            if let Some(username_raw) = before.split_whitespace().last() {
                let username = username_raw.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
                players_arc.lock().await.remove(username);
            }
        }
    }

    // 5. Parse TPS warnings ("Can't keep up! Is the server overloaded?")
    if line.contains("Can't keep up!") {
        let mut tps = tps_arc.lock().await;
        *tps = (*tps - 2.5).max(8.0);
    } else {
        let mut tps = tps_arc.lock().await;
        if *tps < 20.0 {
            *tps = (*tps + 0.2).min(20.0);
        }
    }
}

pub(crate) async fn download_with_sha256(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    expected_sha256: Option<&str>,
) -> AppResult<()> {
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(AppError::Network(format!(
            "HTTP {} while downloading {}",
            resp.status(),
            url
        )));
    }

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let mut file = fs::File::create(dest)?;
    let mut hasher = Sha256::new();
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        hasher.update(&bytes);
        file.write_all(&bytes)?;
    }
    file.flush()?;

    if let Some(expected) = expected_sha256 {
        if !expected.is_empty() {
            let actual = hex::encode(hasher.finalize());
            if !actual.eq_ignore_ascii_case(expected) {
                let _ = fs::remove_file(dest);
                return Err(AppError::Content(format!(
                    "SHA-256 verification failed: expected {}, got {}",
                    expected, actual
                )));
            }
        }
    }

    Ok(())
}
