# Beacon — Zero-Cost P2P Minecraft Server Manager

**Beacon** is an ultra-lightweight, zero-hosting-cost desktop application built with **Tauri v2 (Rust + React 19 / TypeScript / Tailwind CSS)** that enables any home user behind CGNAT or strict firewalls to host a fully optimized, modded or plugin-backed Minecraft Java Server with **one click**, and allows friends to join via a human-readable **Room Code** (`crimson-dragon-42`) without port forwarding, VPNs, or paid relays.

---

## Key Subsystems

1. **Zero-Config JVM Engine (`src-tauri/src/engine/jvm.rs` & `runner.rs`):**
   - Automatically detects OS/Arch and provisions Portable **Java 21** from the **Adoptium v3 API** (`app_data_dir/runtimes/java-21`).
   - Hardware-aware RAM allocation (`clamp(free_ram * 0.65, 2048MB, 10240MB)`) via `sysinfo`.
   - Injects **Generational ZGC** (`-XX:+UseZGC -XX:+ZGenerational`) when allocated RAM >= 6 GB on Java 21+, or **Aikar's G1GC Flags** otherwise.
   - Supports **PaperMC** (via PaperMC Fill v2 API), **Fabric** (auto-installs `Lithium` & `FerriteCore` from Modrinth API), and **NeoForge**.
   - Registers a graceful cleanup hook on `RunEvent::ExitRequested` (`save-all flush` -> `stop\n`) so orphan `java.exe` processes are never left running.

2. **Serverless Room Codes & TCP-over-QUIC P2P Bridge (`src-tauri/src/network/`):**
   - Derives a deterministic Ed25519 keypair from a 3-word Room Code and publishes the Host's `iroh::NodeAddr` to the **BitTorrent Mainline DHT** via `pkarr`.
   - Multiplexes raw game traffic over `/aethermc/mc-tunnel/1` (`tokio::io::copy_bidirectional` to `127.0.0.1:25565`) and JSON-RPC control/manifests/heartbeats over `/aethermc/control/1`.
   - Broadcasts `[MOTD]§a[P2P] Friend's Server[/MOTD][AD]<dynamic_port>[/AD]` to UDP multicast `224.0.2.60:4445` every 1.5s on the client side so the server appears automatically under **Multiplayer -> LAN Worlds**.

3. **Visual Server Control & Live NBT Inventory Inspector (`src-tauri/src/engine/properties.rs` & `nbt_inspector.rs`):**
   - Instant visual toggles for `Cracked/Offline Mode (online-mode)`, `Keep Inventory (gamerule)`, `Difficulty`, `PvP`, `View Distance`, `Simulation Distance`, `Spawn Protection`, and `Whitelist`.
   - 1-Click Player Action buttons (`OP/De-OP`, `Heal & Feed`, `Kill`, `Clear Inventory`, `Gamemode`, `Teleport`, `Kick`, `Ban`) piped directly to child `stdin`.
   - Watches `world/playerdata/<uuid>.dat` via `notify`, decompresses GZip NBT via `flate2` + `fastnbt`, and renders an interactive 36-slot + 4 armor + offhand grid with 1-click slot deletion and item spawning.

4. **Modrinth Integration & Automatic Pre-Join P2P Mod Sync (`src-tauri/src/content/`):**
   - Search and 1-click install `.mrpack` modpacks, Fabric/NeoForge mods, and Paper plugins with 12-stream bounded parallel downloads (`tokio::sync::Semaphore`) and SHA-512 verification.
   - Pre-Join Hook compares `ServerModManifest` on `/aethermc/control/1`, pulls standard mods from Modrinth CDN, and streams custom mods/overrides directly from the Host via Iroh P2P with BLAKE3 verification.

5. **Automatic Host Migration & Zstd Swarm Replicas (`src-tauri/src/network/migration.rs`):**
   - Packages `world/`, `mods/`, and `server.properties` into `.tar.zst` snapshots (`zstd` + `tar`) every 10 minutes and upon graceful shutdown.
   - Replicates snapshots to **Trusted Co-Hosts** in the background and supports both **1-Click Graceful Handover** and **15-Second Crash Failover**.

---

## Development & Build Commands

```bash
# Install frontend dependencies
npm install

# Run frontend production build (TypeScript + Vite)
npm run build

# Run Rust backend unit & integration tests
cargo test --manifest-path src-tauri/Cargo.toml

# Launch Beacon in Tauri desktop development mode
npm run tauri dev
```

---

## License

Beacon's source code is free software, licensed under the **GNU General Public License v3.0**
([LICENSE](LICENSE)). You may use, study, change and share it, but anything you distribute that is
based on it must also be released under GPL-3.0 with its source code.

The license covers the code only. The Minecraft item icons are not covered by it (see below).

## Legal

Beacon is a free, non-commercial fan project. It is **not an official Minecraft product and is not
approved by or associated with Mojang or Microsoft.** "Minecraft" is a trademark of Mojang AB / Microsoft.

The Minecraft item icons in `public/items/` are Mojang's artwork (some via the Minecraft Wiki, CC BY-NC-SA 3.0)
and are not covered by this project's own license. See [NOTICE.md](NOTICE.md) for sources and credits.
