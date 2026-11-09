use crate::content::mod_sync::{
    build_server_mod_manifest, sync_client_mods_with_host, ServerModManifest,
};
use crate::engine::runner::{ServerCore, ServerRunner};
use crate::error::{AppError, AppResult};
use crate::network::fake_lan::FakeLanBroadcaster;
use crate::network::migration::{create_world_snapshot, promote_peer_to_host, SnapshotInfo};
use crate::network::room_code::{
    ensure_room_code_free, generate_human_room_code, publish_room_to_dht, release_room,
    resolve_room_from_dht, try_upnp_port_mapping,
};
use iroh::{Endpoint, NodeAddr};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, Notify};

pub const ALPN_MC_TUNNEL: &[u8] = b"/aethermc/mc-tunnel/1";
pub const ALPN_CONTROL: &[u8] = b"/aethermc/control/1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkRole {
    Idle,
    Host,
    Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedPeer {
    pub node_id: String,
    pub display_name: String,
    pub ping_ms: u32,
    pub is_trusted_cohost: bool,
    pub has_latest_snapshot: bool,
    pub connected_at_ms: u64,
    pub active_streams: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStateSnapshot {
    pub role: NetworkRole,
    pub node_id: String,
    pub room_code: Option<String>,
    pub relay_url: Option<String>,
    pub local_lan_port: Option<u16>,
    pub upnp_external_addr: Option<String>,
    pub connected_peers: Vec<ConnectedPeer>,
    pub latest_snapshot: Option<SnapshotInfo>,
    pub host_ping_ms: Option<u32>,
    pub is_trusted_cohost: bool,
    pub auto_failover_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum ControlRequest {
    Handshake {
        peer_node_id: String,
        display_name: String,
    },
    Heartbeat {
        peer_node_id: String,
        display_name: String,
        client_ping_ms: u32,
        has_snapshot_id: Option<String>,
    },
    GetModManifest,
    FetchBlob {
        relative_path: String,
    },
    FetchSnapshot {
        filename: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "data")]
pub enum ControlResponse {
    HandshakeAck {
        host_node_id: String,
        mc_version: String,
        loader: String,
        motd: String,
    },
    HeartbeatAck {
        server_time_ms: u64,
        is_trusted_cohost: bool,
        latest_snapshot: Option<SnapshotInfo>,
        handover_target_node_id: Option<String>,
    },
    ModManifest(ServerModManifest),
    BlobHeader {
        size_bytes: u64,
        blake3_hash: String,
    },
    Error(String),
}

pub struct P2pNetworkManager {
    pub app_data_dir: PathBuf,
    pub endpoint: Arc<Mutex<Option<Endpoint>>>,
    pub role: Arc<Mutex<NetworkRole>>,
    pub room_code: Arc<Mutex<Option<String>>>,
    pub local_lan_port: Arc<Mutex<Option<u16>>>,
    pub fake_lan: Arc<Mutex<Option<FakeLanBroadcaster>>>,
    pub upnp_addr: Arc<Mutex<Option<String>>>,
    pub peers: Arc<Mutex<HashMap<String, ConnectedPeer>>>,
    pub latest_snapshot: Arc<Mutex<Option<SnapshotInfo>>>,
    pub handover_target: Arc<Mutex<Option<String>>>,
    pub host_ping_ms: Arc<Mutex<Option<u32>>>,
    pub is_trusted_cohost: Arc<Mutex<bool>>,
    pub auto_failover_enabled: Arc<Mutex<bool>>,
    pub session_stop: Arc<Notify>,
}

impl P2pNetworkManager {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            app_data_dir,
            endpoint: Arc::new(Mutex::new(None)),
            role: Arc::new(Mutex::new(NetworkRole::Idle)),
            room_code: Arc::new(Mutex::new(None)),
            local_lan_port: Arc::new(Mutex::new(None)),
            fake_lan: Arc::new(Mutex::new(None)),
            upnp_addr: Arc::new(Mutex::new(None)),
            peers: Arc::new(Mutex::new(HashMap::new())),
            latest_snapshot: Arc::new(Mutex::new(None)),
            handover_target: Arc::new(Mutex::new(None)),
            host_ping_ms: Arc::new(Mutex::new(None)),
            is_trusted_cohost: Arc::new(Mutex::new(true)),
            auto_failover_enabled: Arc::new(Mutex::new(true)),
            session_stop: Arc::new(Notify::new()),
        }
    }

    /// Initialize or retrieve the `iroh::Endpoint` configured with `/aethermc/mc-tunnel/1` and `/aethermc/control/1`.
    pub async fn ensure_endpoint(&self) -> AppResult<Endpoint> {
        let mut guard = self.endpoint.lock().await;
        if let Some(ref ep) = *guard {
            return Ok(ep.clone());
        }

        let ep = Endpoint::builder()
            .alpns(vec![ALPN_MC_TUNNEL.to_vec(), ALPN_CONTROL.to_vec()])
            .discovery_n0()
            .discovery_dht()
            .bind()
            .await
            .map_err(|e| AppError::P2p(format!("Failed to bind Iroh QUIC Endpoint: {}", e)))?;

        *guard = Some(ep.clone());
        Ok(ep)
    }

    /// Start Hosting a P2P Session:
    /// 1. Initializes Iroh Endpoint & optional UPnP port mapping via `igd-next`.
    /// 2. Generates or uses a 3-word Room Code and publishes `NodeAddr` to Mainline DHT via `pkarr`.
    /// 3. Accepts incoming QUIC connections on `/aethermc/mc-tunnel/1` (piped to `127.0.0.1:25565`)
    ///    and `/aethermc/control/1` (Mod Manifests, Heartbeats, Snapshots).
    /// 4. Starts the 10-minute automatic `.tar.zst` world snapshot timer.
    pub async fn start_host_session(
        &self,
        custom_room_code: Option<String>,
        runner: Arc<ServerRunner>,
        app_handle: AppHandle,
    ) -> AppResult<NetworkStateSnapshot> {
        // Keep the old room's code until we know the new one, so hosting the same code again works.
        let previous_code = self.end_session().await;

        let ep = self.ensure_endpoint().await?;
        let _ = ep.home_relay().initialized().await;
        let mut node_addr = ep
            .node_addr()
            .await
            .map_err(|e| AppError::P2p(format!("Failed to resolve local NodeAddr: {}", e)))?;

        // Attempt optional UPnP fast-path on first direct UDP port
        let local_port = node_addr
            .direct_addresses
            .iter()
            .next()
            .map(|sa| sa.port())
            .unwrap_or(0);
        let upnp_mapped = if local_port > 0 {
            try_upnp_port_mapping(local_port).await
        } else {
            None
        };
        if let Some(ext_sa) = upnp_mapped {
            node_addr.direct_addresses.insert(ext_sa);
            *self.upnp_addr.lock().await = Some(ext_sa.to_string());
        }

        // Never share a code with another live host: the code is the DHT signing key,
        // so a second host would overwrite the first one's record.
        let my_node_id = ep.node_id().to_string();
        let code = match custom_room_code.filter(|s| !s.trim().is_empty()) {
            Some(custom) => {
                ensure_room_code_free(&custom, &my_node_id).await?;
                custom
            }
            None => {
                let mut picked = None;
                for _ in 0..6 {
                    let candidate = generate_human_room_code();
                    if ensure_room_code_free(&candidate, &my_node_id).await.is_ok() {
                        picked = Some(candidate);
                        break;
                    }
                }
                picked.ok_or_else(|| {
                    AppError::P2p("Couldn't find a free code. Please try again.".to_string())
                })?
            }
        };
        if let Some(prev) = previous_code {
            if prev.trim().to_lowercase() != code.trim().to_lowercase() {
                self.release_code(prev).await;
            }
        }

        let mc_version = runner.mc_version.lock().await.clone();
        let core = *runner.core.lock().await;
        let loader = match core {
            ServerCore::Paper => "paper",
            ServerCore::Fabric => "fabric",
            ServerCore::Neoforge => "neoforge",
        };

        publish_room_to_dht(
            &self.app_data_dir,
            &code,
            &node_addr,
            "AetherMC Host",
            &mc_version,
            loader,
            upnp_mapped,
        )
        .await?;

        *self.role.lock().await = NetworkRole::Host;
        *self.room_code.lock().await = Some(code.clone());
        *self.handover_target.lock().await = None;

        // Spawn Iroh incoming connection router loop
        let ep_clone = ep.clone();
        let stop_clone = self.session_stop.clone();
        let peers_arc = self.peers.clone();
        let snapshot_arc = self.latest_snapshot.clone();
        let handover_arc = self.handover_target.clone();
        let runner_clone = runner.clone();
        let app_clone = app_handle.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = stop_clone.notified() => {
                        break;
                    }
                    incoming_opt = ep_clone.accept() => {
                        let Some(incoming) = incoming_opt else {
                            break;
                        };
                        let peers_inner = peers_arc.clone();
                        let snap_inner = snapshot_arc.clone();
                        let handover_inner = handover_arc.clone();
                        let runner_inner = runner_clone.clone();
                        let ep_inner = ep_clone.clone();
                        let app_inner = app_clone.clone();

                        tokio::spawn(async move {
                            let Ok(connecting) = incoming.accept() else { return; };
                            let Ok(conn) = connecting.await else { return; };
                            let alpn = conn.alpn().unwrap_or_default();
                            let remote_node_id = conn
                                .remote_node_id()
                                .map(|id| id.to_string())
                                .unwrap_or_else(|_| "unknown-peer".to_string());

                            if alpn == ALPN_MC_TUNNEL {
                                handle_host_mc_tunnel_connection(conn, remote_node_id, peers_inner, app_inner).await;
                            } else if alpn == ALPN_CONTROL {
                                handle_host_control_connection(
                                    conn,
                                    ep_inner,
                                    runner_inner,
                                    peers_inner,
                                    snap_inner,
                                    handover_inner,
                                    app_inner,
                                ).await;
                            }
                        });
                    }
                }
            }
        });

        // Spawn 10-minute periodic Zstd World Snapshot + DHT refresh loop
        let stop_snap = self.session_stop.clone();
        let runner_snap = runner.clone();
        let snap_store = self.latest_snapshot.clone();
        let ep_snap = ep.clone();
        let app_snap = app_handle.clone();
        let app_data_snap = self.app_data_dir.clone();
        let room_code_snap = code.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(600));
            // Skip immediate tick at 0s
            interval.tick().await;
            loop {
                tokio::select! {
                    _ = stop_snap.notified() => break,
                    _ = interval.tick() => {
                        if let Ok(addr) = ep_snap.node_addr().await {
                            let mc_ver = runner_snap.mc_version.lock().await.clone();
                            let _ = publish_room_to_dht(
                                &app_data_snap,
                                &room_code_snap,
                                &addr,
                                "AetherMC Host",
                                &mc_ver,
                                "paper",
                                None,
                            ).await;
                            if let Ok(info) = create_world_snapshot(&runner_snap, Some(&addr), Some(&app_snap)).await {
                                *snap_store.lock().await = Some(info);
                            }
                        }
                    }
                }
            }
        });

        let state = self.get_state_snapshot().await;
        let _ = app_handle.emit("network:state", state.clone());
        Ok(state)
    }

    /// Join a Friend's Room as a Client/Peer:
    /// 1. Resolves the 3-word `room_code` from BitTorrent Mainline DHT via `pkarr`.
    /// 2. Connects to `/aethermc/control/1` to verify handshake & automatically sync required mods (Pre-Join Hook).
    /// 3. Binds a local TCP bridge on `127.0.0.1:<dynamic_port>` piping to `/aethermc/mc-tunnel/1`.
    /// 4. Starts the Fake LAN UDP multicast broadcaster on `224.0.2.60:4445` every 1.5s.
    /// 5. Starts the Host Migration heartbeat & replica sync monitor.
    pub async fn join_room_session(
        &self,
        room_code: String,
        display_name: String,
        runner: Arc<ServerRunner>,
        app_handle: AppHandle,
    ) -> AppResult<NetworkStateSnapshot> {
        let normalized_code = room_code.trim().to_lowercase();

        // Leaving our own room to join another frees our code, but joining our own
        // code (e.g. testing on one PC) must still be able to find the room.
        if let Some(hosted) = self.end_session().await {
            if hosted.trim().to_lowercase() != normalized_code {
                self.release_code(hosted).await;
            }
        }

        let ep = self.ensure_endpoint().await?;

        // 1. Resolve Host NodeAddr from Mainline DHT via pkarr
        let dht_record = resolve_room_from_dht(&self.app_data_dir, &normalized_code).await?;
        let host_addr = dht_record.node_addr.clone();

        // 2. Connect to `/aethermc/control/1` for Pre-Join Mod Sync & Handshake
        let control_conn = ep
            .connect(host_addr.clone(), ALPN_CONTROL)
            .await
            .map_err(|e| AppError::P2p(format!("Failed to connect to Host over QUIC: {}", e)))?;

        let local_node_id = ep.node_id().to_string();

        // Perform Handshake
        let _ack = send_control_rpc(
            &control_conn,
            &ControlRequest::Handshake {
                peer_node_id: local_node_id.clone(),
                display_name: display_name.clone(),
            },
        )
        .await?;

        // Fetch ServerModManifest and execute Pre-Join Client Mod Sync
        if let Ok(ControlResponse::ModManifest(manifest)) =
            send_control_rpc(&control_conn, &ControlRequest::GetModManifest).await
        {
            let client_profile_dir = self
                .app_data_dir
                .join("client_profiles")
                .join(&normalized_code);
            sync_client_mods_with_host(
                &manifest,
                &client_profile_dir,
                Some(&ep),
                Some(&host_addr),
                Some(&app_handle),
            )
            .await?;
        }

        // 3. Bind local TCP listener on `127.0.0.1:0` (dynamic port)
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| AppError::P2p(format!("Failed to bind local TCP bridge: {}", e)))?;
        let dynamic_port = listener.local_addr()?.port();

        // 4. Start Fake LAN UDP Multicast Broadcaster on 224.0.2.60:4445
        let motd = format!("§a[P2P] {} ({})", dht_record.host_name, normalized_code);
        let broadcaster = FakeLanBroadcaster::start(dynamic_port, Some(motd)).await?;

        *self.role.lock().await = NetworkRole::Client;
        *self.room_code.lock().await = Some(normalized_code.clone());
        *self.local_lan_port.lock().await = Some(dynamic_port);
        *self.fake_lan.lock().await = Some(broadcaster);

        // Spawn local TCP -> QUIC `/aethermc/mc-tunnel/1` bridge task
        let ep_tunnel = ep.clone();
        let host_addr_shared = Arc::new(Mutex::new(host_addr.clone()));
        let host_addr_for_tunnel = host_addr_shared.clone();
        let stop_tunnel = self.session_stop.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = stop_tunnel.notified() => break,
                    accept_res = listener.accept() => {
                        let Ok((mut tcp_stream, _)) = accept_res else { continue; };
                        let _ = tcp_stream.set_nodelay(true);
                        let ep_inner = ep_tunnel.clone();
                        let target_addr = host_addr_for_tunnel.lock().await.clone();

                        tokio::spawn(async move {
                            let Ok(quic_conn) = ep_inner.connect(target_addr, ALPN_MC_TUNNEL).await else {
                                return;
                            };
                            let Ok((mut quic_send, mut quic_recv)) = quic_conn.open_bi().await else {
                                return;
                            };
                            let mut quic_bi = QuicBiStream {
                                send: &mut quic_send,
                                recv: &mut quic_recv,
                            };
                            let _ = tokio::io::copy_bidirectional(&mut tcp_stream, &mut quic_bi).await;
                            let _ = quic_send.finish();
                        });
                    }
                }
            }
        });

        // 5. Spawn Heartbeat, Replica Sync & Automatic Host Migration Failover Loop
        let ep_hb = ep.clone();
        let stop_hb = self.session_stop.clone();
        let host_addr_hb = host_addr_shared.clone();
        let ping_arc = self.host_ping_ms.clone();
        let trusted_arc = self.is_trusted_cohost.clone();
        let failover_arc = self.auto_failover_enabled.clone();
        let snap_arc = self.latest_snapshot.clone();
        let role_arc = self.role.clone();
        let fake_lan_arc = self.fake_lan.clone();
        let app_data_hb = self.app_data_dir.clone();
        let runner_hb = runner.clone();
        let app_hb = app_handle.clone();
        let code_hb = normalized_code.clone();
        let name_hb = display_name.clone();
        let mc_ver_hb = dht_record.mc_version.clone();
        let loader_hb = dht_record.loader.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(3));
            let mut last_seen = Instant::now();
            let mut current_ping = 22u32;

            loop {
                tokio::select! {
                    _ = stop_hb.notified() => break,
                    _ = interval.tick() => {
                        let target = host_addr_hb.lock().await.clone();
                        let t0 = Instant::now();
                        let has_snap_id = snap_arc.lock().await.as_ref().map(|s| s.snapshot_id.clone());

                        let hb_result = async {
                            let conn = tokio::time::timeout(
                                Duration::from_secs(3),
                                ep_hb.connect(target.clone(), ALPN_CONTROL),
                            ).await.map_err(|_| AppError::P2p("Heartbeat connect timeout".to_string()))?
                            .map_err(|e| AppError::P2p(e.to_string()))?;

                            send_control_rpc(
                                &conn,
                                &ControlRequest::Heartbeat {
                                    peer_node_id: local_node_id.clone(),
                                    display_name: name_hb.clone(),
                                    client_ping_ms: current_ping,
                                    has_snapshot_id: has_snap_id.clone(),
                                },
                            ).await
                        }.await;

                        match hb_result {
                            Ok(ControlResponse::HeartbeatAck {
                                is_trusted_cohost,
                                latest_snapshot,
                                handover_target_node_id,
                                ..
                            }) => {
                                last_seen = Instant::now();
                                current_ping = (t0.elapsed().as_millis() as u32).max(1);
                                *ping_arc.lock().await = Some(current_ping);
                                *trusted_arc.lock().await = is_trusted_cohost;

                                // If Trusted Co-Host and Host has a newer snapshot, pull it silently in background
                                if is_trusted_cohost {
                                    if let Some(ref remote_snap) = latest_snapshot {
                                        let need_pull = snap_arc
                                            .lock()
                                            .await
                                            .as_ref()
                                            .map(|local| local.snapshot_id != remote_snap.snapshot_id)
                                            .unwrap_or(true);
                                        if need_pull {
                                            let dest_snap = app_data_hb.join("snapshots").join(&remote_snap.filename);
                                            if fetch_snapshot_from_host(
                                                &ep_hb,
                                                &target,
                                                &remote_snap.filename,
                                                &remote_snap.blake3_hash,
                                                &dest_snap,
                                            ).await.is_ok() {
                                                *snap_arc.lock().await = Some(remote_snap.clone());
                                                let _ = app_hb.emit("migration:snapshot-synced", remote_snap.clone());
                                            }
                                        }
                                    }
                                }

                                // Check Graceful Handover trigger targeting this Peer
                                if handover_target_node_id.as_deref() == Some(local_node_id.as_str()) {
                                    if let Some(local_addr) = ep_hb.node_addr().await.ok() {
                                        if let Some(fl) = fake_lan_arc.lock().await.take() {
                                            fl.stop();
                                        }
                                        *role_arc.lock().await = NetworkRole::Host;
                                        let snap_file = snap_arc
                                            .lock()
                                            .await
                                            .as_ref()
                                            .map(|s| app_data_hb.join("snapshots").join(&s.filename))
                                            .unwrap_or_else(|| app_data_hb.join("snapshots").join("latest.tar.zst"));
                                        let _ = promote_peer_to_host(
                                            &runner_hb,
                                            &snap_file,
                                            &code_hb,
                                            &local_addr,
                                            &mc_ver_hb,
                                            &loader_hb,
                                            app_hb.clone(),
                                        ).await;
                                        break;
                                    }
                                }
                            }
                            _ => {
                                // Check if Host migrated to a new Peer on the DHT or crashed (>15s timeout)
                                if last_seen.elapsed() > Duration::from_secs(15) {
                                    // First check if another Co-Host already updated the DHT record
                                    if let Ok(updated_record) = resolve_room_from_dht(&app_data_hb, &code_hb).await {
                                        if updated_record.node_addr.node_id != target.node_id {
                                            *host_addr_hb.lock().await = updated_record.node_addr;
                                            last_seen = Instant::now();
                                            continue;
                                        }
                                    }

                                    // Otherwise, if this Peer is a Trusted Co-Host with auto-failover enabled, take over!
                                    let is_trusted = *trusted_arc.lock().await;
                                    let auto_failover = *failover_arc.lock().await;
                                    if is_trusted && auto_failover {
                                        if let Ok(local_addr) = ep_hb.node_addr().await {
                                            if let Some(fl) = fake_lan_arc.lock().await.take() {
                                                fl.stop();
                                            }
                                            *role_arc.lock().await = NetworkRole::Host;
                                            let snap_file = snap_arc
                                                .lock()
                                                .await
                                                .as_ref()
                                                .map(|s| app_data_hb.join("snapshots").join(&s.filename))
                                                .unwrap_or_else(|| app_data_hb.join("snapshots").join("latest.tar.zst"));
                                            let _ = promote_peer_to_host(
                                                &runner_hb,
                                                &snap_file,
                                                &code_hb,
                                                &local_addr,
                                                &mc_ver_hb,
                                                &loader_hb,
                                                app_hb.clone(),
                                            ).await;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        let state = self.get_state_snapshot().await;
        let _ = app_handle.emit("network:state", state.clone());
        Ok(state)
    }

    /// Toggle whether a connected Peer is designated as a "Trusted Co-Host" for P2P World Replica sync.
    pub async fn set_peer_trusted_cohost(&self, node_id: &str, trusted: bool) -> AppResult<()> {
        if let Some(peer) = self.peers.lock().await.get_mut(node_id) {
            peer.is_trusted_cohost = trusted;
        }
        Ok(())
    }

    /// Trigger a Graceful Host Handover to the target Peer:
    /// 1. Creates a final `.tar.zst` delta snapshot of the world.
    /// 2. Sets `handover_target` so the target Peer pulls the snapshot and takes over the DHT record.
    pub async fn initiate_graceful_handover(
        &self,
        target_peer_node_id: String,
        runner: &ServerRunner,
        app_handle: &AppHandle,
    ) -> AppResult<SnapshotInfo> {
        let ep = self.ensure_endpoint().await?;
        let addr = ep.node_addr().await.ok();
        let snap = create_world_snapshot(runner, addr.as_ref(), Some(app_handle)).await?;
        *self.latest_snapshot.lock().await = Some(snap.clone());
        *self.handover_target.lock().await = Some(target_peer_node_id);
        Ok(snap)
    }

    /// Stop whatever session is running. A host that stops sharing frees its
    /// room code for others straight away.
    pub async fn stop_session(&self) {
        if let Some(code) = self.end_session().await {
            self.release_code(code).await;
        }
    }

    /// Mark a hosted room code as free on the DHT (in the background).
    async fn release_code(&self, code: String) {
        let Some(node_id) = self.endpoint.lock().await.as_ref().map(|ep| ep.node_id().to_string())
        else {
            return;
        };
        let app_data_dir = self.app_data_dir.clone();
        tokio::spawn(async move {
            release_room(&app_data_dir, &code, &node_id).await;
        });
    }

    /// Tear the session down. Returns the room code if this was a hosted room;
    /// the caller decides whether to release it (re-hosting the same code must not).
    async fn end_session(&self) -> Option<String> {
        self.session_stop.notify_waiters();
        if let Some(fl) = self.fake_lan.lock().await.take() {
            fl.stop();
        }
        let hosted_code = if *self.role.lock().await == NetworkRole::Host {
            self.room_code.lock().await.clone()
        } else {
            None
        };
        *self.role.lock().await = NetworkRole::Idle;
        *self.room_code.lock().await = None;
        *self.local_lan_port.lock().await = None;
        *self.host_ping_ms.lock().await = None;
        self.peers.lock().await.clear();
        hosted_code
    }

    pub async fn get_state_snapshot(&self) -> NetworkStateSnapshot {
        let (node_id, relay_url) = if let Some(ref ep) = *self.endpoint.lock().await {
            let nid = ep.node_id().to_string();
            let rurl = ep.home_relay().get().ok().flatten().map(|u| u.to_string());
            (nid, rurl)
        } else {
            ("uninitialized".to_string(), None)
        };

        let peers: Vec<ConnectedPeer> = self.peers.lock().await.values().cloned().collect();

        NetworkStateSnapshot {
            role: *self.role.lock().await,
            node_id,
            room_code: self.room_code.lock().await.clone(),
            relay_url,
            local_lan_port: *self.local_lan_port.lock().await,
            upnp_external_addr: self.upnp_addr.lock().await.clone(),
            connected_peers: peers,
            latest_snapshot: self.latest_snapshot.lock().await.clone(),
            host_ping_ms: *self.host_ping_ms.lock().await,
            is_trusted_cohost: *self.is_trusted_cohost.lock().await,
            auto_failover_enabled: *self.auto_failover_enabled.lock().await,
        }
    }
}

/// Wrapper combining `iroh::endpoint::SendStream` and `RecvStream` into a single `AsyncRead + AsyncWrite`
/// so `tokio::io::copy_bidirectional` can pipe TCP <-> QUIC with zero overhead.
pub struct QuicBiStream<'a> {
    pub send: &'a mut iroh::endpoint::SendStream,
    pub recv: &'a mut iroh::endpoint::RecvStream,
}

impl<'a> tokio::io::AsyncRead for QuicBiStream<'a> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut *self.recv).poll_read(cx, buf)
    }
}

impl<'a> tokio::io::AsyncWrite for QuicBiStream<'a> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut *self.send)
            .poll_write(cx, buf)
            .map_err(std::io::Error::other)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut *self.send)
            .poll_flush(cx)
            .map_err(std::io::Error::other)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut *self.send)
            .poll_shutdown(cx)
            .map_err(std::io::Error::other)
    }
}

/// Host Side Bridge: For every incoming QUIC bi-stream on `/aethermc/mc-tunnel/1`,
/// open a local `tokio::net::TcpStream` to `127.0.0.1:25565` and pipe data using `tokio::io::copy_bidirectional`.
async fn handle_host_mc_tunnel_connection(
    conn: iroh::endpoint::Connection,
    remote_node_id: String,
    peers: Arc<Mutex<HashMap<String, ConnectedPeer>>>,
    app_handle: AppHandle,
) {
    while let Ok((mut quic_send, mut quic_recv)) = conn.accept_bi().await {
        let peers_inner = peers.clone();
        let node_id_clone = remote_node_id.clone();
        let app_inner = app_handle.clone();

        tokio::spawn(async move {
            {
                let mut map = peers_inner.lock().await;
                if let Some(p) = map.get_mut(&node_id_clone) {
                    p.active_streams += 1;
                }
            }
            let _ = app_inner.emit("network:peer-updated", &node_id_clone);

            if let Ok(mut local_tcp) = TcpStream::connect("127.0.0.1:25565").await {
                let _ = local_tcp.set_nodelay(true);
                let mut quic_bi = QuicBiStream {
                    send: &mut quic_send,
                    recv: &mut quic_recv,
                };
                let _ = tokio::io::copy_bidirectional(&mut local_tcp, &mut quic_bi).await;
                let _ = quic_send.finish();
            }

            {
                let mut map = peers_inner.lock().await;
                if let Some(p) = map.get_mut(&node_id_clone) {
                    p.active_streams = p.active_streams.saturating_sub(1);
                }
            }
            let _ = app_inner.emit("network:peer-updated", &node_id_clone);
        });
    }
}

/// Host Side Control Handler on `/aethermc/control/1`:
/// Serves JSON-RPC requests for Handshake, ModManifest, Heartbeats, and P2P Blob/Snapshot transfers.
async fn handle_host_control_connection(
    conn: iroh::endpoint::Connection,
    endpoint: Endpoint,
    runner: Arc<ServerRunner>,
    peers: Arc<Mutex<HashMap<String, ConnectedPeer>>>,
    latest_snapshot: Arc<Mutex<Option<SnapshotInfo>>>,
    handover_target: Arc<Mutex<Option<String>>>,
    app_handle: AppHandle,
) {
    while let Ok((mut send, mut recv)) = conn.accept_bi().await {
        let ep_inner = endpoint.clone();
        let runner_inner = runner.clone();
        let peers_inner = peers.clone();
        let snap_inner = latest_snapshot.clone();
        let handover_inner = handover_target.clone();
        let app_inner = app_handle.clone();

        tokio::spawn(async move {
            let Ok(req_bytes) = recv.read_to_end(64 * 1024).await else {
                return;
            };
            let Ok(req) = serde_json::from_slice::<ControlRequest>(&req_bytes) else {
                return;
            };

            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            match req {
                ControlRequest::Handshake {
                    peer_node_id,
                    display_name,
                } => {
                    {
                        let mut map = peers_inner.lock().await;
                        let is_first_peer = map.is_empty();
                        map.entry(peer_node_id.clone())
                            .and_modify(|p| p.display_name = display_name.clone())
                            .or_insert(ConnectedPeer {
                                node_id: peer_node_id.clone(),
                                display_name,
                                ping_ms: 20,
                                is_trusted_cohost: is_first_peer, // First joining friend defaults to Trusted Co-Host
                                has_latest_snapshot: false,
                                connected_at_ms: now_ms,
                                active_streams: 0,
                            });
                    }
                    let _ = app_inner.emit("network:peer-updated", &peer_node_id);

                    let mc_version = runner_inner.mc_version.lock().await.clone();
                    let core = *runner_inner.core.lock().await;
                    let loader = match core {
                        ServerCore::Paper => "paper",
                        ServerCore::Fabric => "fabric",
                        ServerCore::Neoforge => "neoforge",
                    }
                    .to_string();

                    let resp = ControlResponse::HandshakeAck {
                        host_node_id: ep_inner.node_id().to_string(),
                        mc_version,
                        loader,
                        motd: "AetherMC P2P Server".to_string(),
                    };
                    if let Ok(bytes) = serde_json::to_vec(&resp) {
                        let _ = send.write_all(&bytes).await;
                        let _ = send.finish();
                    }
                }
                ControlRequest::Heartbeat {
                    peer_node_id,
                    display_name,
                    client_ping_ms,
                    has_snapshot_id,
                } => {
                    let latest_snap = snap_inner.lock().await.clone();
                    let is_trusted = {
                        let mut map = peers_inner.lock().await;
                        let has_latest = match (&has_snapshot_id, &latest_snap) {
                            (Some(cid), Some(snap)) => cid == &snap.snapshot_id,
                            _ => false,
                        };
                        let entry = map.entry(peer_node_id.clone()).or_insert(ConnectedPeer {
                            node_id: peer_node_id,
                            display_name,
                            ping_ms: client_ping_ms,
                            is_trusted_cohost: true,
                            has_latest_snapshot: has_latest,
                            connected_at_ms: now_ms,
                            active_streams: 0,
                        });
                        entry.ping_ms = client_ping_ms;
                        entry.has_latest_snapshot = has_latest;
                        entry.is_trusted_cohost
                    };

                    let resp = ControlResponse::HeartbeatAck {
                        server_time_ms: now_ms,
                        is_trusted_cohost: is_trusted,
                        latest_snapshot: latest_snap,
                        handover_target_node_id: handover_inner.lock().await.clone(),
                    };
                    if let Ok(bytes) = serde_json::to_vec(&resp) {
                        let _ = send.write_all(&bytes).await;
                        let _ = send.finish();
                    }
                }
                ControlRequest::GetModManifest => {
                    let mc_version = runner_inner.mc_version.lock().await.clone();
                    let core = *runner_inner.core.lock().await;
                    let loader = match core {
                        ServerCore::Paper => "vanilla",
                        ServerCore::Fabric => "fabric",
                        ServerCore::Neoforge => "neoforge",
                    };
                    let host_addr = ep_inner.node_addr().await.ok();
                    let manifest_res = build_server_mod_manifest(
                        &runner_inner.server_dir(),
                        &mc_version,
                        loader,
                        host_addr.as_ref(),
                    )
                    .await;

                    let resp = match manifest_res {
                        Ok(m) => ControlResponse::ModManifest(m),
                        Err(e) => ControlResponse::Error(e.to_string()),
                    };
                    if let Ok(bytes) = serde_json::to_vec(&resp) {
                        let _ = send.write_all(&bytes).await;
                        let _ = send.finish();
                    }
                }
                ControlRequest::FetchBlob { relative_path } => {
                    let rel = Path::new(&relative_path);
                    if rel.is_absolute()
                        || rel
                            .components()
                            .any(|c| matches!(c, std::path::Component::ParentDir))
                    {
                        return;
                    }
                    let target = runner_inner.server_dir().join(rel);
                    stream_file_over_quic(&target, &mut send).await;
                }
                ControlRequest::FetchSnapshot { filename } => {
                    let Some(safe_name) = Path::new(&filename).file_name() else {
                        return;
                    };
                    let target = runner_inner.app_data_dir.join("snapshots").join(safe_name);
                    stream_file_over_quic(&target, &mut send).await;
                }
            }
        });
    }
}

async fn stream_file_over_quic(path: &Path, send: &mut iroh::endpoint::SendStream) {
    if let Ok(bytes) = tokio::fs::read(path).await {
        let _ = send.write_all(&bytes).await;
        let _ = send.finish();
    }
}

pub async fn send_control_rpc(
    conn: &iroh::endpoint::Connection,
    req: &ControlRequest,
) -> AppResult<ControlResponse> {
    let (mut send, mut recv) = conn
        .open_bi()
        .await
        .map_err(|e| AppError::P2p(format!("Failed to open control bi-stream: {}", e)))?;

    let payload = serde_json::to_vec(req)?;
    send.write_all(&payload)
        .await
        .map_err(|e| AppError::P2p(e.to_string()))?;
    send.finish().map_err(|e| AppError::P2p(e.to_string()))?;

    let resp_bytes = recv
        .read_to_end(16 * 1024 * 1024)
        .await
        .map_err(|e| AppError::P2p(format!("Failed reading control response: {}", e)))?;

    let resp: ControlResponse = serde_json::from_slice(&resp_bytes)?;
    Ok(resp)
}

/// Pull a custom mod or config file directly from the Host over QUIC and verify its BLAKE3 hash.
pub async fn fetch_blob_from_host(
    endpoint: &Endpoint,
    host_addr: &NodeAddr,
    relative_path: &str,
    expected_blake3: &str,
    dest: &Path,
) -> AppResult<()> {
    let conn = endpoint
        .connect(host_addr.clone(), ALPN_CONTROL)
        .await
        .map_err(|e| AppError::P2p(format!("Blob transfer connect error: {}", e)))?;

    let (mut send, mut recv) = conn
        .open_bi()
        .await
        .map_err(|e| AppError::P2p(e.to_string()))?;

    let req = ControlRequest::FetchBlob {
        relative_path: relative_path.to_string(),
    };
    send.write_all(&serde_json::to_vec(&req)?)
        .await
        .map_err(|e| AppError::P2p(e.to_string()))?;
    send.finish().map_err(|e| AppError::P2p(e.to_string()))?;

    let bytes = recv
        .read_to_end(128 * 1024 * 1024)
        .await
        .map_err(|e| AppError::P2p(e.to_string()))?;

    let actual_b3 = blake3::hash(&bytes).to_hex().to_string();
    if !expected_blake3.is_empty() && !actual_b3.eq_ignore_ascii_case(expected_blake3) {
        return Err(AppError::P2p(format!(
            "BLAKE3 hash mismatch for {}: expected {}, got {}",
            relative_path, expected_blake3, actual_b3
        )));
    }

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    fs::write(dest, &bytes)?;
    Ok(())
}

/// Pull a `.tar.zst` world snapshot from the Host over QUIC and verify its BLAKE3 hash.
pub async fn fetch_snapshot_from_host(
    endpoint: &Endpoint,
    host_addr: &NodeAddr,
    filename: &str,
    expected_blake3: &str,
    dest: &Path,
) -> AppResult<()> {
    let conn = endpoint
        .connect(host_addr.clone(), ALPN_CONTROL)
        .await
        .map_err(|e| AppError::P2p(format!("Snapshot transfer connect error: {}", e)))?;

    let (mut send, mut recv) = conn
        .open_bi()
        .await
        .map_err(|e| AppError::P2p(e.to_string()))?;

    let req = ControlRequest::FetchSnapshot {
        filename: filename.to_string(),
    };
    send.write_all(&serde_json::to_vec(&req)?)
        .await
        .map_err(|e| AppError::P2p(e.to_string()))?;
    send.finish().map_err(|e| AppError::P2p(e.to_string()))?;

    let bytes = recv
        .read_to_end(512 * 1024 * 1024)
        .await
        .map_err(|e| AppError::P2p(e.to_string()))?;

    let actual_b3 = blake3::hash(&bytes).to_hex().to_string();
    if !expected_blake3.is_empty() && !actual_b3.eq_ignore_ascii_case(expected_blake3) {
        return Err(AppError::Migration(format!(
            "BLAKE3 verification failed for world snapshot {}",
            filename
        )));
    }

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    fs::write(dest, &bytes)?;
    Ok(())
}
