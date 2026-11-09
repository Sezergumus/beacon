pub mod app_settings;
pub mod content;
pub mod engine;
pub mod error;
pub mod network;

use crate::app_settings::AppSettings;
use crate::content::modrinth::{
    get_modrinth_project, identify_content_hashes, install_modrinth_project, install_mrpack_archive,
    list_installed_content, remove_installed_content, search_modrinth, InstalledContentItem,
    ModrinthProjectDetails, ModrinthSearchHit,
};
use crate::engine::jvm::{detect_hardware_profile, ensure_portable_jre, HardwareProfile};
use crate::engine::nbt_inspector::{inspect_player_nbt, PlayerInventoryData};
use crate::engine::properties::ServerProperties;
use crate::engine::runner::{
    ServerCore, ServerLaunchConfig, ServerLogEntry, ServerRunner, ServerStatus, ServerTelemetry,
};
use crate::engine::versions;
use crate::engine::whitelist::{self, WhitelistEntry};
use crate::engine::worlds::{self, WorldEntry, WorldMeta};
use crate::error::{AppError, AppResult};
use crate::network::iroh_node::{NetworkStateSnapshot, P2pNetworkManager};
use crate::network::migration::{create_world_snapshot, SnapshotInfo};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, RunEvent, State, WindowEvent};

pub struct AppState {
    pub app_data_dir: PathBuf,
    pub runner: Arc<ServerRunner>,
    pub network: Arc<P2pNetworkManager>,
    pub settings: std::sync::RwLock<AppSettings>,
}

/// Bring the main window back from the tray (or from behind other windows).
fn show_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

/// Write any crash inside the app to `crash.log` next to the worlds, so a bug report
/// has something to attach. The normal panic message still goes to the console.
fn install_panic_log(app_data_dir: &std::path::Path) {
    let path = app_data_dir.join("crash.log");
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(
                f,
                "[{}] {}
{}
",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                info,
                std::backtrace::Backtrace::force_capture()
            );
        }
        default_hook(info);
    }));
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Beacon", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Beacon", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Beacon")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            // Exiting runs the shutdown hook that saves and stops a running world.
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

#[tauri::command]
async fn get_app_settings(state: State<'_, AppState>) -> AppResult<AppSettings> {
    Ok(state.settings.read().map(|s| s.clone()).unwrap_or_default())
}

#[tauri::command]
async fn save_app_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> AppResult<AppSettings> {
    settings.save(&state.app_data_dir)?;
    if let Ok(mut guard) = state.settings.write() {
        *guard = settings.clone();
    }
    Ok(settings)
}

#[tauri::command]
async fn get_hardware_profile(
    state: State<'_, AppState>,
    custom_ram_mb: Option<u64>,
    java_major: Option<u8>,
) -> AppResult<HardwareProfile> {
    Ok(detect_hardware_profile(
        &state.app_data_dir,
        custom_ram_mb,
        java_major.unwrap_or(21),
    ))
}

#[tauri::command]
async fn list_minecraft_versions(core: ServerCore) -> AppResult<Vec<String>> {
    versions::list_versions(core).await
}

#[tauri::command]
async fn provision_java_runtime(
    app: AppHandle,
    state: State<'_, AppState>,
    java_major: Option<u8>,
) -> AppResult<String> {
    let bin = ensure_portable_jre(&state.app_data_dir, java_major.unwrap_or(21), Some(&app)).await?;
    Ok(bin.to_string_lossy().to_string())
}

#[tauri::command]
async fn start_minecraft_server(
    app: AppHandle,
    state: State<'_, AppState>,
    config: ServerLaunchConfig,
) -> AppResult<()> {
    state.runner.start_server(config, app).await
}

#[tauri::command]
async fn stop_minecraft_server(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    // Create a graceful shutdown snapshot before stopping if server is running
    if *state.runner.status.lock().await == ServerStatus::Running {
        let addr = if let Some(ref ep) = *state.network.endpoint.lock().await {
            ep.node_addr().await.ok()
        } else {
            None
        };
        if let Ok(snap) = create_world_snapshot(&state.runner, addr.as_ref(), Some(&app)).await {
            *state.network.latest_snapshot.lock().await = Some(snap);
        }
    }
    state.runner.stop_server(Some(&app)).await
}

#[tauri::command]
async fn send_server_command(state: State<'_, AppState>, command: String) -> AppResult<()> {
    state.runner.send_command(&command).await
}

#[tauri::command]
async fn get_server_telemetry(state: State<'_, AppState>) -> AppResult<ServerTelemetry> {
    Ok(state.runner.get_telemetry().await)
}

#[tauri::command]
async fn get_server_logs(state: State<'_, AppState>) -> AppResult<Vec<ServerLogEntry>> {
    Ok(state.runner.logs.lock().await.clone())
}

#[tauri::command]
async fn get_server_properties(state: State<'_, AppState>) -> AppResult<ServerProperties> {
    ServerProperties::load(&state.runner.server_dir())
}

#[tauri::command]
async fn save_server_properties(
    state: State<'_, AppState>,
    props: ServerProperties,
) -> AppResult<ServerProperties> {
    props.save(&state.runner.server_dir())?;
    apply_properties_live(&state.runner, &props).await;
    ServerProperties::load(&state.runner.server_dir())
}

/// If the server is running, apply the settings that can change without a restart.
async fn apply_properties_live(runner: &ServerRunner, props: &ServerProperties) {
    if *runner.status.lock().await != ServerStatus::Running {
        return;
    }
    let _ = runner
        .send_command(&format!("gamerule keepInventory {}", props.keep_inventory))
        .await;
    let _ = runner
        .send_command(&format!("difficulty {}", props.difficulty))
        .await;
    let _ = runner
        .send_command(if props.white_list {
            "whitelist on"
        } else {
            "whitelist off"
        })
        .await;
}

/// Everything the world details window shows. The server only writes stats and
/// `level.dat` now and then, so a running world is saved first to get fresh numbers.
#[tauri::command]
async fn get_world_details(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<worlds::WorldDetails> {
    if state.runner.active_world_id().as_deref() == Some(id.as_str())
        && *state.runner.status.lock().await == ServerStatus::Running
    {
        let _ = state.runner.send_command("save-all flush").await;
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
    let app_data_dir = state.app_data_dir.clone();
    tokio::task::spawn_blocking(move || worlds::world_details(&app_data_dir, &id))
        .await
        .map_err(|e| AppError::Custom(format!("World scan failed: {}", e)))?
}

/// Save one world's settings, whether or not it is the one currently selected.
#[tauri::command]
async fn save_world_properties(
    state: State<'_, AppState>,
    id: String,
    props: ServerProperties,
) -> AppResult<ServerProperties> {
    let app_data_dir = state.app_data_dir.clone();
    let id_for_save = id.clone();
    let saved = tokio::task::spawn_blocking(move || {
        worlds::save_world_properties(&app_data_dir, &id_for_save, &props)
    })
    .await
    .map_err(|e| AppError::Custom(format!("Save failed: {}", e)))??;
    if state.runner.active_world_id().as_deref() == Some(id.as_str()) {
        apply_properties_live(&state.runner, &saved).await;
    }
    Ok(saved)
}

/// The server folder of an existing world, and whether the server is running it right now.
fn whitelist_target(state: &AppState, id: &str) -> AppResult<(PathBuf, bool)> {
    if !worlds::is_safe_id(id) || worlds::load_registry(&state.app_data_dir).iter().all(|w| w.id != id) {
        return Err(AppError::Custom("That world no longer exists.".to_string()));
    }
    let dir = worlds::instance_dir(&state.app_data_dir, id);
    let live = state.runner.active_world_id().as_deref() == Some(id);
    Ok((dir, live))
}

#[tauri::command]
async fn get_whitelist(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<Vec<WhitelistEntry>> {
    let (dir, _) = whitelist_target(&state, &id)?;
    Ok(whitelist::load(&dir))
}

/// Add a friend to a world's whitelist. A running world is told through its console (it
/// owns the file then); a stopped one has the file edited directly.
#[tauri::command]
async fn add_whitelist_player(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> AppResult<Vec<WhitelistEntry>> {
    let name = whitelist::validate_name(&name)?;
    let (dir, live) = whitelist_target(&state, &id)?;
    if live && *state.runner.status.lock().await == ServerStatus::Running {
        state
            .runner
            .send_command(&format!("whitelist add {}", name))
            .await?;
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
        let list = whitelist::load(&dir);
        if !whitelist::contains(&list, &name) {
            return Err(AppError::Custom(format!(
                "Couldn't add “{}”. If this world checks accounts, make sure the name is spelled exactly right.",
                name
            )));
        }
        return Ok(list);
    }
    let online_mode = ServerProperties::load(&dir)
        .map(|p| p.online_mode)
        .unwrap_or(false);
    whitelist::add_offline(&dir, &name, online_mode).await
}

#[tauri::command]
async fn remove_whitelist_player(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> AppResult<Vec<WhitelistEntry>> {
    let name = whitelist::validate_name(&name)?;
    let (dir, live) = whitelist_target(&state, &id)?;
    if live && *state.runner.status.lock().await == ServerStatus::Running {
        state
            .runner
            .send_command(&format!("whitelist remove {}", name))
            .await?;
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
        return Ok(whitelist::load(&dir));
    }
    whitelist::remove_offline(&dir, &name)
}

/// The starting settings new worlds are made with (the Settings page).
#[tauri::command]
async fn get_default_properties(state: State<'_, AppState>) -> AppResult<ServerProperties> {
    Ok(crate::engine::properties::load_defaults(&state.app_data_dir))
}

#[tauri::command]
async fn save_default_properties(
    state: State<'_, AppState>,
    props: ServerProperties,
) -> AppResult<ServerProperties> {
    crate::engine::properties::save_defaults(&state.app_data_dir, &props)
}

/// The server icon of a world, or (without an id) the one new worlds start with.
#[tauri::command]
async fn get_server_icon(state: State<'_, AppState>, id: Option<String>) -> AppResult<Option<String>> {
    worlds::read_icon(&state.app_data_dir, id.as_deref())
}

#[tauri::command]
async fn set_server_icon(
    state: State<'_, AppState>,
    id: Option<String>,
    png_base64: String,
) -> AppResult<()> {
    worlds::write_icon(&state.app_data_dir, id.as_deref(), &png_base64)
}

#[tauri::command]
async fn clear_server_icon(state: State<'_, AppState>, id: Option<String>) -> AppResult<()> {
    worlds::clear_icon(&state.app_data_dir, id.as_deref())
}

/// Show the folder where Beacon keeps its worlds and settings in the file explorer.
#[tauri::command]
async fn open_data_folder(state: State<'_, AppState>) -> AppResult<String> {
    let dir = state.app_data_dir.clone();
    std::fs::create_dir_all(&dir)?;
    #[cfg(target_os = "windows")]
    let opener = "explorer";
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let opener = "xdg-open";
    std::process::Command::new(opener)
        .arg(&dir)
        .spawn()
        .map_err(|e| AppError::Custom(format!("Couldn't open the folder: {}", e)))?;
    Ok(dir.to_string_lossy().to_string())
}

#[tauri::command]
async fn run_player_action(
    state: State<'_, AppState>,
    player: String,
    action: String,
    payload: Option<String>,
) -> AppResult<()> {
    state
        .runner
        .execute_player_action(&player, &action, payload)
        .await
}

#[tauri::command]
async fn inspect_player_inventory(
    state: State<'_, AppState>,
    uuid: String,
    username: Option<String>,
) -> AppResult<PlayerInventoryData> {
    if *state.runner.status.lock().await == ServerStatus::Running {
        let _ = state.runner.send_command("save-all flush").await;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    inspect_player_nbt(&state.runner.server_dir(), &uuid, username).await
}

#[tauri::command]
async fn search_modrinth_catalog(
    query: String,
    project_type: String,
    mc_version: Option<String>,
    loader: Option<String>,
) -> AppResult<Vec<ModrinthSearchHit>> {
    search_modrinth(
        &query,
        &project_type,
        mc_version.as_deref(),
        loader.as_deref(),
    )
    .await
}

#[tauri::command]
async fn get_modrinth_project_details(project_id: String) -> AppResult<ModrinthProjectDetails> {
    get_modrinth_project(&project_id).await
}

/// Maps file hashes (SHA-512) to Modrinth project ids, to tell which results are already installed.
#[tauri::command]
async fn identify_installed_content(
    hashes: Vec<String>,
) -> AppResult<std::collections::HashMap<String, String>> {
    identify_content_hashes(hashes).await
}

#[tauri::command]
async fn install_modrinth_item(
    state: State<'_, AppState>,
    project_id: String,
    mc_version: String,
    loader: String,
    project_type: String,
) -> AppResult<String> {
    install_modrinth_project(
        &state.runner.server_dir(),
        &project_id,
        &mc_version,
        &loader,
        &project_type,
    )
    .await
}

#[tauri::command]
async fn install_local_mrpack(
    app: AppHandle,
    state: State<'_, AppState>,
    mrpack_path: String,
) -> AppResult<String> {
    let path = PathBuf::from(&mrpack_path);
    let idx = install_mrpack_archive(&path, &state.runner.server_dir(), Some(&app)).await?;
    Ok(idx.name)
}

#[tauri::command]
async fn get_installed_content(
    state: State<'_, AppState>,
) -> AppResult<Vec<InstalledContentItem>> {
    list_installed_content(&state.runner.server_dir()).await
}

#[tauri::command]
async fn delete_installed_content(
    state: State<'_, AppState>,
    filename: String,
    kind: String,
) -> AppResult<()> {
    remove_installed_content(&state.runner.server_dir(), &filename, &kind)
}

#[tauri::command]
async fn list_worlds(state: State<'_, AppState>) -> AppResult<Vec<WorldEntry>> {
    let app_data_dir = state.app_data_dir.clone();
    let active = state.runner.active_world_id();
    let snapshots_dir = state.app_data_dir.join("snapshots");
    tokio::task::spawn_blocking(move || {
        worlds::list_worlds(&app_data_dir, &snapshots_dir, active.as_deref())
    })
    .await
    .map_err(|e| AppError::Custom(format!("World scan failed: {}", e)))
}

#[tauri::command]
async fn create_world(
    state: State<'_, AppState>,
    name: String,
    core: ServerCore,
    mc_version: String,
    options: Option<worlds::NewWorldOptions>,
) -> AppResult<WorldMeta> {
    worlds::create_world_with(
        &state.app_data_dir,
        &name,
        core,
        &mc_version,
        &options.unwrap_or_default(),
    )
}

/// How often (in minutes) a running world is saved to disk. 0 turns the extra saves off.
#[tauri::command]
async fn set_autosave_interval(state: State<'_, AppState>, minutes: u64) -> AppResult<()> {
    state
        .runner
        .autosave_secs
        .store(minutes.min(24 * 60) * 60, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
async fn rename_world(state: State<'_, AppState>, id: String, name: String) -> AppResult<()> {
    worlds::rename_world(&state.app_data_dir, &id, &name)
}

/// Switch which world the Mods / Settings / Players pages work on. Only possible
/// while the server is stopped, because a running server owns its world's folder.
#[tauri::command]
async fn set_active_world(state: State<'_, AppState>, id: String) -> AppResult<()> {
    if state.runner.active_world_id().as_deref() == Some(id.as_str()) {
        return Ok(());
    }
    if *state.runner.status.lock().await != ServerStatus::Stopped {
        return Err(AppError::Process(
            "Close your world first, then you can switch to another one.".to_string(),
        ));
    }
    if worlds::load_registry(&state.app_data_dir).iter().all(|w| w.id != id) {
        return Err(AppError::Custom("That world no longer exists.".to_string()));
    }
    state.runner.set_active_world(Some(&id));
    Ok(())
}

/// Change one world's server type and Minecraft version. Other worlds keep theirs.
#[tauri::command]
async fn set_world_config(
    state: State<'_, AppState>,
    id: String,
    core: ServerCore,
    mc_version: String,
) -> AppResult<()> {
    if state.runner.active_world_id().as_deref() == Some(id.as_str())
        && *state.runner.status.lock().await != ServerStatus::Stopped
    {
        return Err(AppError::Process(
            "Close your world first, then you can change its version.".to_string(),
        ));
    }
    worlds::update_world_config(&state.app_data_dir, &id, core, &mc_version)
}

#[tauri::command]
async fn delete_world(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let app_data_dir = state.app_data_dir.clone();
    let snapshots_dir = state.app_data_dir.join("snapshots");

    if !id.ends_with(".tar.zst") {
        // Only the world the server is using is locked; the others can go any time.
        let is_active = state.runner.active_world_id().as_deref() == Some(id.as_str());
        if is_active && *state.runner.status.lock().await != ServerStatus::Stopped {
            return Err(AppError::Process(
                "Close your world first, then you can delete it.".to_string(),
            ));
        }
        if is_active {
            state.runner.set_active_world(None);
        }
        tokio::task::spawn_blocking(move || worlds::delete_world(&app_data_dir, &id))
            .await
            .map_err(|e| AppError::Custom(format!("Delete failed: {}", e)))??;
    } else {
        let name = id.clone();
        tokio::task::spawn_blocking(move || worlds::delete_backup(&snapshots_dir, &name))
            .await
            .map_err(|e| AppError::Custom(format!("Delete failed: {}", e)))??;

        let mut latest = state.network.latest_snapshot.lock().await;
        if latest.as_ref().map(|s| s.filename == id).unwrap_or(false) {
            *latest = None;
        }
    }
    Ok(())
}

#[tauri::command]
async fn start_p2p_host(
    app: AppHandle,
    state: State<'_, AppState>,
    custom_room_code: Option<String>,
) -> AppResult<NetworkStateSnapshot> {
    state
        .network
        .start_host_session(custom_room_code, state.runner.clone(), app)
        .await
}

#[tauri::command]
async fn join_p2p_room(
    app: AppHandle,
    state: State<'_, AppState>,
    room_code: String,
    display_name: String,
) -> AppResult<NetworkStateSnapshot> {
    state
        .network
        .join_room_session(room_code, display_name, state.runner.clone(), app)
        .await
}

#[tauri::command]
async fn leave_p2p_session(state: State<'_, AppState>) -> AppResult<NetworkStateSnapshot> {
    state.network.stop_session().await;
    Ok(state.network.get_state_snapshot().await)
}

#[tauri::command]
async fn get_network_state(state: State<'_, AppState>) -> AppResult<NetworkStateSnapshot> {
    Ok(state.network.get_state_snapshot().await)
}

#[tauri::command]
async fn set_trusted_cohost(
    state: State<'_, AppState>,
    node_id: String,
    trusted: bool,
) -> AppResult<NetworkStateSnapshot> {
    state.network.set_peer_trusted_cohost(&node_id, trusted).await?;
    Ok(state.network.get_state_snapshot().await)
}

#[tauri::command]
async fn create_manual_snapshot(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<SnapshotInfo> {
    let addr = if let Some(ref ep) = *state.network.endpoint.lock().await {
        ep.node_addr().await.ok()
    } else {
        None
    };
    let snap = create_world_snapshot(&state.runner, addr.as_ref(), Some(&app)).await?;
    *state.network.latest_snapshot.lock().await = Some(snap.clone());
    Ok(snap)
}

#[tauri::command]
async fn trigger_host_migration(
    app: AppHandle,
    state: State<'_, AppState>,
    target_node_id: String,
) -> AppResult<SnapshotInfo> {
    state
        .network
        .initiate_graceful_handover(target_node_id, &state.runner, &app)
        .await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        // A second launch (e.g. the desktop icon while Beacon sits in the tray) just
        // brings the running window back instead of starting a second server manager.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let Some(state) = window.app_handle().try_state::<AppState>() else {
                    return;
                };
                let to_tray = state.settings.read().map(|s| s.close_to_tray).unwrap_or(false);
                // Only stay in the background while a world is up; otherwise just quit.
                let world_up = state
                    .runner
                    .status
                    .try_lock()
                    .map(|s| *s != ServerStatus::Stopped)
                    .unwrap_or(true);
                if to_tray && world_up {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")).join("AetherMC"));
            let _ = std::fs::create_dir_all(&app_data_dir);
            install_panic_log(&app_data_dir);

            // Worlds used to share one server folder; give each its own (runs once).
            worlds::migrate_legacy(&app_data_dir);

            let runner = Arc::new(ServerRunner::new(app_data_dir.clone()));
            let network = Arc::new(P2pNetworkManager::new(app_data_dir.clone()));

            let settings = std::sync::RwLock::new(AppSettings::load(&app_data_dir));
            app.manage(AppState {
                app_data_dir,
                runner,
                network,
                settings,
            });
            setup_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_settings,
            save_app_settings,
            get_hardware_profile,
            list_minecraft_versions,
            provision_java_runtime,
            start_minecraft_server,
            stop_minecraft_server,
            send_server_command,
            get_server_telemetry,
            get_server_logs,
            get_server_properties,
            save_server_properties,
            get_world_details,
            save_world_properties,
            get_whitelist,
            add_whitelist_player,
            remove_whitelist_player,
            get_default_properties,
            save_default_properties,
            get_server_icon,
            set_server_icon,
            clear_server_icon,
            open_data_folder,
            run_player_action,
            inspect_player_inventory,
            search_modrinth_catalog,
            get_modrinth_project_details,
            identify_installed_content,
            install_modrinth_item,
            install_local_mrpack,
            get_installed_content,
            delete_installed_content,
            list_worlds,
            create_world,
            rename_world,
            set_autosave_interval,
            set_active_world,
            set_world_config,
            delete_world,
            start_p2p_host,
            join_p2p_room,
            leave_p2p_session,
            get_network_state,
            set_trusted_cohost,
            create_manual_snapshot,
            trigger_host_migration,
        ])
        .build(tauri::generate_context!())
        .expect("error while building AetherMC Tauri application");

    app.run(|app_handle, event| {
        if let RunEvent::ExitRequested { .. } | RunEvent::Exit = event {
            if let Some(state) = app_handle.try_state::<AppState>() {
                state.runner.blocking_shutdown_on_exit();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use crate::engine::jvm::{calculate_optimal_ram_mb, generate_jvm_flags};
    use crate::engine::nbt_inspector::{nbt_slot_to_command_slot, offline_player_uuid};
    use crate::engine::properties::ServerProperties;
    use crate::engine::runner::ServerRunner;
    use crate::network::fake_lan::FakeLanBroadcaster;
    use crate::network::migration::{create_world_snapshot, extract_world_snapshot};
    use crate::network::room_code::{
        derive_room_keypair, generate_human_room_code, publish_room_to_dht, resolve_room_from_dht,
    };
    use iroh::Endpoint;

    #[test]
    fn test_optimal_ram_and_zgc_vs_aikar_flags() {
        // clamp(free_ram * 0.65, 2048, 10240)
        assert_eq!(calculate_optimal_ram_mb(2000), 2048);
        assert_eq!(calculate_optimal_ram_mb(10000), 6500);
        assert_eq!(calculate_optimal_ram_mb(32000), 10240);

        // >= 6GB (6144MB) and Java 21+ -> Generational ZGC
        let zgc_flags = generate_jvm_flags(6500, 21);
        assert!(zgc_flags.contains(&"-XX:+UseZGC".to_string()));
        assert!(zgc_flags.contains(&"-XX:+ZGenerational".to_string()));
        assert!(zgc_flags.contains(&"-XX:+AlwaysPreTouch".to_string()));
        assert!(zgc_flags.contains(&"-XX:+DisableExplicitGC".to_string()));

        // < 6GB -> Aikar's G1GC Flags
        let aikar_flags = generate_jvm_flags(4096, 21);
        assert!(aikar_flags.contains(&"-XX:+UseG1GC".to_string()));
        assert!(aikar_flags.contains(&"-XX:MaxGCPauseMillis=200".to_string()));
    }

    #[test]
    fn test_server_properties_strict_localhost_binding() {
        let temp_dir = std::env::temp_dir().join("aethermc_test_props");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let mut props = ServerProperties::load(&temp_dir).expect("load default properties");
        assert_eq!(props.server_ip, "127.0.0.1");
        assert_eq!(props.server_port, 25565);

        props.online_mode = false;
        props.keep_inventory = true;
        props.difficulty = "hard".to_string();
        props.view_distance = 14;
        props.save(&temp_dir).expect("save properties");

        let reloaded = ServerProperties::load(&temp_dir).expect("reload properties");
        assert!(!reloaded.online_mode);
        assert!(reloaded.keep_inventory);
        assert_eq!(reloaded.difficulty, "hard");
        assert_eq!(reloaded.view_distance, 14);
        assert_eq!(reloaded.server_ip, "127.0.0.1");
        assert_eq!(reloaded.server_port, 25565);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_nbt_slot_mapping_and_offline_uuid() {
        assert_eq!(nbt_slot_to_command_slot(0, false), "hotbar.0");
        assert_eq!(nbt_slot_to_command_slot(8, false), "hotbar.8");
        assert_eq!(nbt_slot_to_command_slot(9, false), "container.0");
        assert_eq!(nbt_slot_to_command_slot(35, false), "container.26");
        assert_eq!(nbt_slot_to_command_slot(103, false), "armor.head");
        assert_eq!(nbt_slot_to_command_slot(102, false), "armor.chest");
        assert_eq!(nbt_slot_to_command_slot(101, false), "armor.legs");
        assert_eq!(nbt_slot_to_command_slot(100, false), "armor.feet");
        assert_eq!(nbt_slot_to_command_slot(-106, false), "weapon.offhand");

        let u1 = offline_player_uuid("Steve");
        let u2 = offline_player_uuid("Steve");
        assert_eq!(u1, u2);
        assert_eq!(u1.len(), 36);
    }

    #[tokio::test]
    async fn test_room_code_dht_and_zstd_snapshot_roundtrip() {
        let temp_dir = std::env::temp_dir().join("aethermc_test_e2e");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let code = generate_human_room_code();
        assert_eq!(code.split('-').count(), 3);

        let kp1 = derive_room_keypair(&code);
        let kp2 = derive_room_keypair(&code.to_uppercase());
        assert_eq!(kp1.public_key(), kp2.public_key());

        let ep = Endpoint::builder().bind().await.expect("bind iroh endpoint");
        let node_addr = ep.node_addr().await.expect("node addr");

        let published = publish_room_to_dht(
            &temp_dir,
            &code,
            &node_addr,
            "TestHost",
            "1.21.1",
            "fabric",
            None,
        )
        .await
        .expect("publish room");

        let resolved = resolve_room_from_dht(&temp_dir, &code)
            .await
            .expect("resolve room");
        assert_eq!(resolved.node_addr.node_id, published.node_addr.node_id);

        // Test Fake LAN Broadcaster startup & teardown
        let lan = FakeLanBroadcaster::start(54321, None)
            .await
            .expect("start fake lan");
        assert_eq!(lan.local_port, 54321);
        lan.stop();

        // Test Zstd .tar.zst World Snapshot & Extraction
        let runner = ServerRunner::new(temp_dir.clone());
        std::fs::create_dir_all(runner.server_dir().join("world")).unwrap();
        std::fs::write(
            runner.server_dir().join("world").join("level.dat"),
            b"mock-level-dat-bytes",
        )
        .unwrap();

        let snap = create_world_snapshot(&runner, Some(&node_addr), None)
            .await
            .expect("create zstd snapshot");
        assert!(snap.size_bytes > 0);
        assert!(snap.blob_ticket.is_some());

        let restore_dir = temp_dir.join("restored_server");
        let archive_path = temp_dir.join("snapshots").join(&snap.filename);
        extract_world_snapshot(&archive_path, &restore_dir)
            .await
            .expect("extract zstd snapshot");

        let restored_level = std::fs::read(restore_dir.join("world").join("level.dat")).unwrap();
        assert_eq!(restored_level, b"mock-level-dat-bytes");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

