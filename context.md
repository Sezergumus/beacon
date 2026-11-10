# Project Context & Architecture Specification: Zero-Cost P2P Minecraft Server Manager

## 1. System Overview & Core Philosophy

You are an expert Rust Systems Engineer and Full-Stack Tauri Developer building **AetherMC** (working title), an ultra-lightweight, zero-hosting-cost desktop application built with **Tauri v2 (Rust + React/TypeScript)**.

### Primary Goal

Enable any home user behind Carrier-Grade NAT (CGNAT) or strict firewalls to host a fully optimized, modded or plugin-backed Minecraft Java Server with **one click**, and allow friends to join using a human-readable **Room Code** without port forwarding, third-party VPNs (Hamachi/Radmin), or paid relay tunnels (Ngrok).

### Strict Architectural Constraints

1. **Zero Cloud/Hosting Cost ($0 Forever):** Do NOT use central backend servers, paid databases, or paid relay servers. All state discovery, NAT traversal, and file distribution must use free public infrastructure (BitTorrent Mainline DHT via `pkarr`, public STUN/iroh relays, Modrinth/Adoptium/PaperMC CDNs) and Peer-to-Peer (`iroh`) networking.
2. **Zero Virtualization Overhead:** Do NOT use Docker, WSL, or Electron. Manage the JVM directly as an OS-level child process via Rust (`tokio::process::Command`) inside a Tauri v2 shell (~25 MB idle RAM).
3. **Dual-Role Single Binary:** The exact same application acts as the **Host** (Server Manager + P2P Ingress Bridge) and the **Client/Peer** (Room Joiner + Auto Mod Syncer + Fake LAN Broadcaster).

---

## 2. Tech Stack & Key Dependencies

### Backend (`src-tauri` - Rust Edition 2021)

- **Framework:** `tauri` (v2), `serde`, `serde_json`, `tokio` (full features)
- **P2P Networking & NAT Traversal:**
  - `iroh` (QUIC-based P2P hole punching, ALPN multiplexing, encrypted streams)
  - `iroh-blobs` (BLAKE3 verified P2P file/modpack/world snapshot transfer)
  - `pkarr` (Public-Key Addressable Resource Records on Mainline DHT for serverless Room Code resolution)
  - `igd-next` (Optional silent UPnP port mapping fast-path)
- **Minecraft & NBT Parsing:**
  - `fastnbt` (Reading/parsing `level.dat` and `world/playerdata/<uuid>.dat` for Live Inventory)
  - `flate2` (GZip decompression for NBT files)
- **System, Compression & HTTP:**
  - `sysinfo` (Hardware RAM/CPU detection for automatic JVM flag tuning)
  - `zstd` (Ultra-fast world backup compression for Host Migration)
  - `reqwest` (Async streaming downloads with SHA-256/SHA-512 verification)
  - `zip` (Extracting portable JRE and `.mrpack` archives)
  - `notify` (File system watcher for live `playerdata` updates)

### Frontend (`src` - TypeScript + React)

- **UI & Styling:** React 19, TypeScript, Tailwind CSS, Shadcn UI, Lucide Icons
- **State Management:** Zustand (subscribed to Tauri IPC Events)
- **Communication:** `@tauri-apps/api/core` (`invoke`) and `@tauri-apps/api/event` (`listen`)

---

## 3. Core Subsystems & Technical Specifications

### 3.1. Serverless Room Codes & TCP-over-QUIC P2P Tunnel

Minecraft requires standard TCP sockets, while CGNAT hole punching requires UDP. We bridge them transparently:

1. **Serverless Room Code (`pkarr`):**
   - When the Host starts a session, generate a deterministic cryptographic keypair from a human-readable Room Code + session salt (or map a generated 3-word code like `crimson-dragon-42` via deterministic Ed25519 seed derivation) and publish the Host's `iroh::NodeAddr` (NodeId + relay URL + direct UDP socket addresses) to the BitTorrent Mainline DHT via `pkarr`.
2. **ALPN Protocols:**
   - `/aethermc/mc-tunnel/1`: Raw bidirectional TCP-over-QUIC game traffic stream.
   - `/aethermc/control/1`: JSON-RPC over QUIC for mod manifests, player verification, and Host Migration heartbeat.
3. **Host Side Bridge:**
   - Local Minecraft server binds strictly to `127.0.0.1:25565` (`online-mode` configurable).
   - For every incoming QUIC bi-stream on `/aethermc/mc-tunnel/1`, open a local `tokio::net::TcpStream` to `127.0.0.1:25565` and pipe data using `tokio::io::copy_bidirectional`.
4. **Client Side Bridge & Fake LAN Broadcaster:**
   - The joining friend enters `crimson-dragon-42`.
   - Rust resolves the `iroh::NodeAddr` from `pkarr`, establishes a direct P2P QUIC connection, and binds a local TCP listener on `127.0.0.1:<dynamic_port>`.
   - **Fake LAN Discovery:** Spawn a background UDP task broadcasting to multicast address `224.0.2.60:4445` every 1.5 seconds with payload:
     `[MOTD]§a[P2P] Friend's Server[/MOTD][AD]<dynamic_port>[/AD]`
   - Result: The server appears automatically in the friend's Minecraft Multiplayer menu under **"LAN Worlds"**: zero IP or port typing required.

---

### 3.2. Zero-Config JVM Engine & Hardware Auto-Optimization

Home users must never manually install Java or edit `.bat` files.

1. **Silent Portable JRE Provisioning:**
   - Check `app_data_dir/runtimes/java-21` (or Java 17 for legacy MC versions).
   - If missing, query the **Adoptium v3 API** (`https://api.adoptium.net/v3/binary/latest/21/ga/...`) for the user's OS/Arch and extract it silently.
2. **Core Selection:**
   - **PaperMC** (Default for Vanilla/Plugin servers, fetched via PaperMC Fill v2 API).
   - **Fabric** (Default for Modded, auto-installs `Lithium` and `FerriteCore` from Modrinth API by default to cut RAM usage by 40%).
   - **NeoForge** (For heavy modpacks).
3. **Smart JVM Flags (`sysinfo`):**
   - Detect available physical memory. Allocate `clamp(free_ram * 0.65, 2048MB, 10240MB)`.
   - If allocated RAM >= 6GB and Java 21+, inject **Generational ZGC**:
     `-XX:+UseZGC -XX:+ZGenerational -XX:+AlwaysPreTouch -XX:+DisableExplicitGC`
   - Otherwise, inject standard **Aikar's Flags**.

---

### 3.3. Modpack, Plugin Manager & Automatic P2P Mod Sync

Eliminate "Mod mismatch" errors when friends join a modded server.

1. **1-Click Modrinth Integration:**
   - Search and install `.mrpack` modpacks, Fabric/NeoForge mods, and Paper plugins directly via Modrinth REST API (`https://api.modrinth.com/v2/`).
   - Parse `modrinth.index.json` inside `.mrpack` archives and download all files concurrently using a bounded `tokio::sync::Semaphore` (e.g., 12 parallel streams) with SHA-512 verification.
2. **Automatic Client Mod Sync (Pre-Join Hook):**
   - When a Peer connects to `/aethermc/control/1`, the Host sends a `ServerModManifest` containing:
     - Minecraft Version & Loader (`fabric` / `neoforge` / `vanilla`)
     - List of required client-side/both-side mods (`filename`, `sha512`, `modrinth_download_url`, `iroh_blob_ticket`).
   - The Peer's app compares this manifest against the target Minecraft instance directory (or creates an isolated profile folder).
   - Standard mods are downloaded at full speed from Modrinth CDN; custom configs (`overrides/`) or non-CDN mods are pulled directly from the Host/Swarm via `iroh-blobs`.

---

### 3.4. Visual Server Control, Action Buttons & Live Inventory Inspector

Provide a full GUI dashboard so the Host never needs to memorize console commands.

1. **Visual `server.properties` Configurator:**
   - Instant UI switches for: `Cracked/Offline Mode (online-mode)`, `Keep Inventory (gamerule)`, `Difficulty`, `PvP`, `View Distance`, `Simulation Distance`, `Spawn Protection`, and `Whitelist`.
2. **Real-Time Telemetry & Interactive Console:**
   - Pipe child process `stdout` and `stderr` via async `BufReader` lines to the frontend (`server:log` event).
   - Parse join/leave events, TPS warnings, and player coordinates in real time.
3. **1-Click Player Action Buttons:**
   - Next to each online player card, provide instant action buttons that write directly to the JVM child process `stdin`:
     - **OP / De-OP:** `op <player>` / `deop <player>`
     - **Heal & Feed:** `effect give <player> instant_health 1 255` & `effect give <player> saturation 1 255`
     - **Kill / Clear Inventory:** `kill <player>` / `clear <player>`
     - **Gamemode Toggle:** `gamemode survival|creative|spectator <player>`
     - **Teleport:** `tp <player_a> <player_b>`
     - **Kick / Ban:** `kick <player>` / `ban <player>`
4. **Live Inventory Inspector & Visual Item Editor:**
   - Watch `world/playerdata/<uuid>.dat` using the `notify` crate + periodic `save-all` flush (or read live via an embedded lightweight bridge plugin/RCON socket).
   - Decompress the GZip `.dat` file in Rust and deserialize NBT tags (`Inventory`, `EnderItems`, `Health`, `foodLevel`, `XpLevel`) using `fastnbt`.
   - Render a visual 36-slot inventory grid + 4 armor slots + offhand in the React UI.
   - Allow the Host to click any slot to delete an item (`item replace entity <player> <slot> with air`) or search & give items (`give <player> <item> <count>`).

---

### 3.5. Automatic Host Migration & Distributed Swarm Backups

Ensure the world stays alive even if the original Host has to leave.

1. **Incremental `zstd` World Snapshots:**
   - Every 10 minutes (and upon graceful shutdown: `save-off` -> `save-all flush` -> `zstd` archive -> `save-on`), package the `world/`, `mods/`, and `server.properties` into a single `.tar.zst` bundle.
2. **P2P Replica Distribution (`iroh-blobs`):**
   - Share the snapshot's `BlobTicket` over `/aethermc/control/1` to connected Peers marked as "Trusted Co-Hosts".
   - Trusted Peers silently download the latest world snapshot in the background while playing.
3. **Seamless Host Handover (Migration):**
   - **Graceful Handover:** Host clicks "Transfer Host to [Peer]" or closes the app -> Final delta snapshot is pushed -> Target Peer automatically boots the local Paper/Fabric engine with the synced world, updates the `pkarr` DHT record for `crimson-dragon-42` with their own `NodeAddr`, and all other Peers automatically reconnect their QUIC tunnels to the new Host.
   - **Crash Failover:** If the Host's `/aethermc/control/1` heartbeat drops for >15 seconds, the highest-priority Trusted Peer prompts (or auto-triggers) local server startup from the latest synced `.tar.zst` snapshot and takes over the DHT record.

---

## 4. Recommended Project Directory Structure

```text
aethermc/
├── src/                          # React + TypeScript Frontend
│   ├── components/
│   │   ├── dashboard/            # TPS/RAM charts, Server Start/Stop, Room Code badge
│   │   ├── players/              # Online player cards, Quick Action buttons, Live Inventory Grid
│   │   ├── mods/                 # Modrinth browser, 1-click .mrpack & plugin installer
│   │   ├── network/              # P2P Peer list, Ping metrics, Host Migration controls
│   │   └── settings/             # Visual server.properties switches & JVM tuning
│   ├── stores/                   # Zustand state stores (useServerStore, useNetworkStore)
│   └── lib/                      # Tauri IPC wrappers & Minecraft item icon mappings
├── src-tauri/                    # Rust Backend
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs               # Entry point & graceful exit lifecycle hooks
│       ├── lib.rs                # Tauri command registrations & state initialization
│       ├── engine/               # JVM & Minecraft Process Management
│       │   ├── jvm.rs            # Adoptium API downloader & hardware-based flag generator
│       │   ├── runner.rs         # Child process spawner, stdin writer, stdout parser
│       │   ├── properties.rs     # Strongly-typed server.properties reader/writer
│       │   └── nbt_inspector.rs  # fastnbt parser for playerdata/<uuid>.dat live inventory
│       ├── network/              # Zero-Cost P2P & LAN Discovery
│       │   ├── iroh_node.rs      # Iroh Endpoint, ALPN handlers, TCP-over-QUIC bidirectional bridge
│       │   ├── room_code.rs      # Pkarr Mainline DHT publisher & resolver
│       │   ├── fake_lan.rs       # 224.0.2.60:4445 UDP multicast broadcaster
│       │   └── migration.rs      # Zstd snapshotter, iroh-blobs sync, Host takeover state machine
│       └── content/              # Modpacks, Mods & Plugins
│           ├── modrinth.rs       # Modrinth REST client & parallel .mrpack downloader
│           └── mod_sync.rs       # Client-Host manifest diffing & pre-join sync
```

---

## 5. Step-by-Step Implementation Roadmap

Follow these phases sequentially. Complete and verify each phase before moving to the next:

### Phase 1: Tauri v2 Skeleton & Zero-Config JVM Engine

- Initialize Tauri v2 project with React, TypeScript, and Tailwind CSS.
- Implement `engine/jvm.rs`: detect OS/Arch, fetch portable Java 21 from Adoptium API, and calculate optimal RAM + Generational ZGC / Aikar's Flags via `sysinfo`.
- Implement `engine/runner.rs`: download PaperMC latest build, auto-accept `eula.txt`, spawn the server on `127.0.0.1:25565`, stream `stdout` to React via Tauri events, and implement safe shutdown (`save-all` -> `stop`).

### Phase 2: Iroh P2P Tunnel, Room Codes & Fake LAN Broadcaster

- Implement `network/iroh_node.rs`: initialize `iroh::Endpoint` with `/aethermc/mc-tunnel/1` and `/aethermc/control/1` ALPNs.
- Build the TCP-over-QUIC bridge using `tokio::io::copy_bidirectional`.
- Implement `network/room_code.rs`: map 3-word room codes to `pkarr` Mainline DHT records so peers can resolve the Host's `NodeAddr` without a central server.
- Implement `network/fake_lan.rs`: broadcast `[MOTD]...[/MOTD][AD]...[/AD]` over UDP `224.0.2.60:4445` on the client side and verify the server appears in Minecraft's Multiplayer menu.

### Phase 3: Visual Dashboard, Player Action Buttons & Live Inventory

- Build `engine/properties.rs` and the React Settings UI for 1-click `server.properties` and `gamerule` toggles.
- Implement player tracking and 1-click `stdin` command wrappers (OP, Heal, Feed, Kill, Gamemode, Teleport).
- Implement `engine/nbt_inspector.rs` using `fastnbt` + `notify` file watcher on `world/playerdata/` and render the interactive 2D Player Inventory modal in React.

### Phase 4: Modrinth Integration & Automatic P2P Mod Sync

- Implement `content/modrinth.rs` for searching and parallel-downloading Paper plugins, Fabric mods, and `.mrpack` modpacks.
- Implement `content/mod_sync.rs`: exchange `ServerModManifest` over `/aethermc/control/1` when a peer joins a room and automatically download/verify missing mods before starting the Fake LAN broadcast.

### Phase 5: Zstd Snapshots & Automatic Host Migration

- Implement `network/migration.rs`: create `.tar.zst` snapshots of the world directory and replicate them to Trusted Peers via `iroh-blobs`.
- Implement the Host Handover protocol so a Peer can seamlessly spin up the replicated world and update the `pkarr` DHT room record if the original Host disconnects.

---

## 6. Coding Guidelines & Guardrails

- **Never Block the Async Runtime:** Use `tokio::task::spawn_blocking` for synchronous CPU-heavy operations like `zstd` compression, zip extraction, or `fastnbt` deserialization.
- **Graceful Cleanup is Mandatory:** Register handlers on Tauri's `RunEvent::ExitRequested` to ensure the child `java` process always receives `stop\n` and terminates cleanly, never leave orphan `java.exe` processes running in the background.
- **Error Handling:** Use `thiserror` and `anyhow` in Rust; return serializable `Result<T, AppError>` from all `#[tauri::command]` functions so the UI can display clear, non-technical toast notifications to home users.
