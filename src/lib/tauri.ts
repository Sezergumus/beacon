import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

export type ServerCore = "paper" | "fabric" | "neoforge";
export type ServerStatus = "stopped" | "provisioning" | "starting" | "running" | "stopping";
export type NetworkRole = "idle" | "host" | "client";

export interface HardwareProfile {
  total_ram_mb: number;
  free_ram_mb: number;
  recommended_ram_mb: number;
  cpu_cores: number;
  cpu_brand: string;
  os_name: string;
  arch: string;
  gc_profile: string;
  jvm_flags: string[];
  java_installed: boolean;
  java_path: string | null;
}

export interface OnlinePlayer {
  username: string;
  uuid: string;
  is_op: boolean;
  gamemode: string;
  ping_ms: number;
  joined_at_ms: number;
  coords: [number, number, number];
}

export interface ServerLogEntry {
  timestamp: string;
  level: string;
  message: string;
}

export interface ServerTelemetry {
  status: ServerStatus;
  core: ServerCore;
  mc_version: string;
  tps: number;
  ram_used_mb: number;
  ram_allocated_mb: number;
  cpu_usage_percent: number;
  uptime_secs: number;
  online_players: OnlinePlayer[];
  pid: number | null;
}

export interface ServerLaunchConfig {
  core: ServerCore;
  mc_version: string;
  allocated_ram_mb?: number;
  java_major?: number;
  /** Which world to open. Leave out to reuse the last one. */
  world_id?: string;
}

export interface ServerProperties {
  online_mode: boolean;
  keep_inventory: boolean;
  difficulty: string;
  pvp: boolean;
  view_distance: number;
  simulation_distance: number;
  spawn_protection: number;
  white_list: boolean;
  motd: string;
  max_players: number;
  gamemode: string;
  allow_flight: boolean;
  server_ip: string;
  server_port: number;
}

export interface InventoryItemSlot {
  slot: number;
  command_slot: string;
  id: string;
  count: number;
  tag_summary?: string | null;
}

export interface PlayerInventoryData {
  uuid: string;
  username?: string | null;
  health: number;
  max_health: number;
  food_level: number;
  saturation: number;
  xp_level: number;
  xp_total: number;
  gamemode: number;
  dimension: string;
  position: [number, number, number];
  inventory: InventoryItemSlot[];
  ender_items: InventoryItemSlot[];
  last_updated_ms: number;
}

/** Chosen when a world is made; Minecraft only reads these when it first generates the world. */
export interface NewWorldOptions {
  /** Any text or number. Empty means a random seed. */
  seed?: string;
  hardcore?: boolean;
}

export interface ModrinthSearchHit {
  project_id: string;
  slug: string;
  title: string;
  description: string;
  categories: string[];
  downloads: number;
  icon_url?: string | null;
  author: string;
  project_type: string;
  latest_version?: string | null;
}

export interface InstalledContentItem {
  filename: string;
  kind: string;
  size_bytes: number;
  sha512: string;
  blake3_hash: string;
}

export interface SnapshotInfo {
  snapshot_id: string;
  filename: string;
  size_bytes: number;
  blake3_hash: string;
  blob_ticket?: string | null;
  created_at_ms: number;
  mc_version: string;
  loader: string;
}

export interface WorldEntry {
  /** The world's id, or the file name for a backup */
  id: string;
  kind: "world" | "backup";
  name: string;
  size_bytes: number;
  modified_ms: number;
  /** The world the server used last (or will open next) */
  active: boolean;
  /** Server type this world runs on (worlds only) */
  core?: ServerCore | null;
  /** Minecraft version this world runs on (worlds only) */
  mc_version?: string | null;
  /** Mods / plugins installed in this world only */
  mod_count: number;
  plugin_count: number;
}

/** A mod or plugin file in one world's folder */
export interface ContentFile {
  filename: string;
  size_bytes: number;
}

/** Play time of one player in a world, from Minecraft's own stats */
export interface PlayerPlaytime {
  uuid: string;
  name: string | null;
  playtime_secs: number;
  last_seen_ms: number | null;
}

export interface WorldDetails {
  id: string;
  name: string;
  core: ServerCore | null;
  mc_version: string | null;
  created_ms: number;
  size_bytes: number;
  /** Empty until Minecraft has started this world once */
  last_played_ms: number | null;
  /** A string: 64-bit seeds don't fit in a JS number */
  seed: string | null;
  hardcore: boolean;
  /** Minecraft version that last saved the world */
  game_version: string | null;
  spawn: [number, number, number] | null;
  /** 24000 ticks = one in-game day */
  world_age_ticks: number | null;
  total_playtime_secs: number;
  players: PlayerPlaytime[];
  mods: ContentFile[];
  plugins: ContentFile[];
  properties: ServerProperties;
}

export interface WorldMeta {
  id: string;
  name: string;
  created_ms: number;
  core?: ServerCore | null;
  mc_version?: string | null;
}

export interface ConnectedPeer {
  node_id: string;
  display_name: string;
  ping_ms: number;
  is_trusted_cohost: boolean;
  has_latest_snapshot: boolean;
  connected_at_ms: number;
  active_streams: number;
}

export interface NetworkStateSnapshot {
  role: NetworkRole;
  node_id: string;
  room_code: string | null;
  relay_url: string | null;
  local_lan_port: number | null;
  upnp_external_addr?: string | null;
  connected_peers: ConnectedPeer[];
  latest_snapshot: SnapshotInfo | null;
  host_ping_ms: number | null;
  is_trusted_cohost: boolean;
  auto_failover_enabled: boolean;
}

export interface JvmProgressEvent {
  stage: string;
  downloaded_bytes: number;
  total_bytes: number;
  percent: number;
}

export interface ModpackProgressEvent {
  pack_name: string;
  completed_files: number;
  total_files: number;
  current_file: string;
}

export interface ModSyncStatusEvent {
  total_mods: number;
  synced_mods: number;
  current_mod: string;
  source: string;
  completed: boolean;
}

export interface MigrationEvent {
  stage: string;
  room_code: string;
  new_host_node_id: string;
  message: string;
}

export function formatAppError(err: unknown): string {
  if (!err) return "Unknown error";
  if (typeof err === "string") return err;
  if (typeof err === "object" && err !== null) {
    const maybeObj = err as Record<string, unknown>;
    if (typeof maybeObj.message === "string") {
      return `${maybeObj.kind ? `[${maybeObj.kind}] ` : ""}${maybeObj.message}`;
    }
  }
  return String(err);
}

export const tauriApi = {
  getHardwareProfile: (customRamMb?: number, javaMajor?: number) =>
    invoke<HardwareProfile>("get_hardware_profile", {
      customRamMb: customRamMb ?? null,
      javaMajor: javaMajor ?? 21,
    }),

  setAutosaveInterval: (minutes: number) =>
    invoke<void>("set_autosave_interval", { minutes }),

  listMinecraftVersions: (core: ServerCore) =>
    invoke<string[]>("list_minecraft_versions", { core }),

  provisionJavaRuntime: (javaMajor?: number) =>
    invoke<string>("provision_java_runtime", { javaMajor: javaMajor ?? 21 }),

  startMinecraftServer: (config: ServerLaunchConfig) =>
    invoke<void>("start_minecraft_server", { config }),

  stopMinecraftServer: () => invoke<void>("stop_minecraft_server"),

  sendServerCommand: (command: string) =>
    invoke<void>("send_server_command", { command }),

  getServerTelemetry: () => invoke<ServerTelemetry>("get_server_telemetry"),

  getServerLogs: () => invoke<ServerLogEntry[]>("get_server_logs"),

  getServerProperties: () => invoke<ServerProperties>("get_server_properties"),

  saveServerProperties: (props: ServerProperties) =>
    invoke<ServerProperties>("save_server_properties", { props }),

  runPlayerAction: (player: string, action: string, payload?: string) =>
    invoke<void>("run_player_action", {
      player,
      action,
      payload: payload ?? null,
    }),

  inspectPlayerInventory: (uuid: string, username?: string) =>
    invoke<PlayerInventoryData>("inspect_player_inventory", {
      uuid,
      username: username ?? null,
    }),

  searchModrinthCatalog: (
    query: string,
    projectType: string,
    mcVersion?: string,
    loader?: string
  ) =>
    invoke<ModrinthSearchHit[]>("search_modrinth_catalog", {
      query,
      projectType,
      mcVersion: mcVersion ?? null,
      loader: loader ?? null,
    }),

  installModrinthItem: (
    projectId: string,
    mcVersion: string,
    loader: string,
    projectType: string
  ) =>
    invoke<string>("install_modrinth_item", {
      projectId,
      mcVersion,
      loader,
      projectType,
    }),

  installLocalMrpack: (mrpackPath: string) =>
    invoke<string>("install_local_mrpack", { mrpackPath }),

  getInstalledContent: () =>
    invoke<InstalledContentItem[]>("get_installed_content"),

  deleteInstalledContent: (filename: string, kind: string) =>
    invoke<void>("delete_installed_content", { filename, kind }),

  listWorlds: () => invoke<WorldEntry[]>("list_worlds"),

  createWorld: (
    name: string,
    core: ServerCore,
    mcVersion: string,
    options?: NewWorldOptions
  ) => invoke<WorldMeta>("create_world", { name, core, mcVersion, options: options ?? null }),

  /** Make this world the one the Mods / Settings / Players pages work on (server must be stopped). */
  setActiveWorld: (id: string) => invoke<void>("set_active_world", { id }),

  /** Change one world's server type and Minecraft version; other worlds keep theirs. */
  setWorldConfig: (id: string, core: ServerCore, mcVersion: string) =>
    invoke<void>("set_world_config", { id, core, mcVersion }),

  renameWorld: (id: string, name: string) =>
    invoke<void>("rename_world", { id, name }),

  deleteWorld: (id: string) => invoke<void>("delete_world", { id }),

  getWorldDetails: (id: string) => invoke<WorldDetails>("get_world_details", { id }),

  saveWorldProperties: (id: string, props: ServerProperties) =>
    invoke<ServerProperties>("save_world_properties", { id, props }),

  startP2pHost: (customRoomCode?: string) =>
    invoke<NetworkStateSnapshot>("start_p2p_host", {
      customRoomCode: customRoomCode ?? null,
    }),

  joinP2pRoom: (roomCode: string, displayName: string) =>
    invoke<NetworkStateSnapshot>("join_p2p_room", { roomCode, displayName }),

  leaveP2pSession: () => invoke<NetworkStateSnapshot>("leave_p2p_session"),

  getNetworkState: () => invoke<NetworkStateSnapshot>("get_network_state"),

  setTrustedCohost: (nodeId: string, trusted: boolean) =>
    invoke<NetworkStateSnapshot>("set_trusted_cohost", { nodeId, trusted }),

  createManualSnapshot: () => invoke<SnapshotInfo>("create_manual_snapshot"),

  triggerHostMigration: (targetNodeId: string) =>
    invoke<SnapshotInfo>("trigger_host_migration", { targetNodeId }),

  onEvent: <T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> =>
    listen<T>(event, (e) => handler(e.payload)),
};
