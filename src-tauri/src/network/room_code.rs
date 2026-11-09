use crate::error::{AppError, AppResult};
use base64::Engine;
use iroh::NodeAddr;
use pkarr::dns::rdata::TXT;
use pkarr::dns::Name;
use pkarr::{Client as PkarrClient, Keypair, SignedPacket};
use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::{IpAddr, SocketAddr, SocketAddrV4};
use std::path::Path;
use std::time::Duration;

const ADJECTIVES: &[&str] = &[
    "crimson", "obsidian", "emerald", "aether", "cobalt", "golden", "shadow", "frost", "solar",
    "lunar", "arcane", "mythic", "crystal", "velvet", "iron", "storm", "ember", "glacial",
    "astral", "verdant", "phantom", "radiant", "ancient", "thunder",
];

const NOUNS: &[&str] = &[
    "dragon", "phoenix", "beacon", "golem", "warden", "titan", "citadel", "nexus", "bastion",
    "falcon", "griffin", "kraken", "vortex", "forge", "portal", "prism", "horizon", "sentinel",
    "wyvern", "monolith", "sanctum", "comet", "valkyrie", "redstone",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomDhtRecord {
    pub room_code: String,
    pub pkarr_public_key: String,
    pub node_addr: NodeAddr,
    pub host_name: String,
    pub mc_version: String,
    pub loader: String,
    pub upnp_mapped: Option<SocketAddr>,
    pub published_at_ms: u64,
    /// Set when the host stops sharing, so the code is free again right away.
    #[serde(default)]
    pub closed: bool,
}

/// Hosts re-publish every 10 minutes; a record that is much older than that
/// belongs to a host that crashed or lost its connection.
const ROOM_STALE_MS: u64 = 25 * 60 * 1000;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn record_from_packet(packet: &SignedPacket) -> Option<RoomDhtRecord> {
    for rr in packet.all_resource_records() {
        if let pkarr::dns::rdata::RData::TXT(ref txt) = rr.rdata {
            if let Ok(txt_str) = String::try_from(txt.clone()) {
                if let Ok(decoded) =
                    base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(txt_str.trim())
                {
                    if let Ok(record) = serde_json::from_slice::<RoomDhtRecord>(&decoded) {
                        return Some(record);
                    }
                }
            }
        }
    }
    None
}

/// Make sure nobody else is hosting `room_code` right now.
///
/// A room code *is* the key its DHT record is signed with, so two hosts using the
/// same code would silently overwrite each other and friends would land in
/// whichever world published last. A record counts as taken when it belongs to a
/// different host, wasn't closed, and was refreshed recently. If the DHT can't be
/// reached we don't block hosting.
pub async fn ensure_room_code_free(room_code: &str, my_node_id: &str) -> AppResult<()> {
    let normalized = room_code.trim().to_lowercase();
    let Ok(client) = PkarrClient::builder().build() else {
        return Ok(());
    };
    let pubkey = derive_room_keypair(&normalized).public_key();
    let Ok(Some(packet)) =
        tokio::time::timeout(Duration::from_secs(4), client.resolve(&pubkey)).await
    else {
        return Ok(());
    };
    let Some(record) = record_from_packet(&packet) else {
        return Ok(());
    };

    let fresh = now_ms().saturating_sub(record.published_at_ms) < ROOM_STALE_MS;
    let someone_else = record.node_addr.node_id.to_string() != my_node_id;
    if !record.closed && fresh && someone_else {
        return Err(AppError::P2p(format!(
            "The code '{}' is already in use by another world. Pick a different code.",
            normalized
        )));
    }
    Ok(())
}

/// Free a room code when its host stops sharing. Does nothing if the record has
/// since been taken over by someone else (e.g. after a host handover).
pub async fn release_room(app_data_dir: &Path, room_code: &str, my_node_id: &str) {
    let normalized = room_code.trim().to_lowercase();
    let cache_file = app_data_dir.join("rooms").join(format!("{}.json", normalized));
    let _ = fs::remove_file(&cache_file);

    let keypair = derive_room_keypair(&normalized);
    let Ok(client) = PkarrClient::builder().build() else {
        return;
    };
    let Ok(Some(packet)) = tokio::time::timeout(
        Duration::from_secs(4),
        client.resolve(&keypair.public_key()),
    )
    .await
    else {
        return;
    };
    let Some(mut record) = record_from_packet(&packet) else {
        return;
    };
    if record.node_addr.node_id.to_string() != my_node_id || record.closed {
        return;
    }

    record.closed = true;
    record.published_at_ms = now_ms();
    let Ok(compact_json) = serde_json::to_vec(&record) else {
        return;
    };
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&compact_json);
    if let (Ok(name), Ok(txt)) = (Name::new("_aethermc"), TXT::try_from(encoded.as_str())) {
        if let Ok(signed) = SignedPacket::builder().txt(name, txt, 120).build(&keypair) {
            let _ = client.publish(&signed, None).await;
        }
    }
}

/// Generate a human-readable 3-word room code like `crimson-dragon-42`.
pub fn generate_human_room_code() -> String {
    let mut rng = rand::thread_rng();
    let adj = ADJECTIVES.choose(&mut rng).unwrap_or(&"crimson");
    let noun = NOUNS.choose(&mut rng).unwrap_or(&"dragon");
    let num: u16 = rng.gen_range(10..=99);
    format!("{}-{}-{}", adj, noun, num)
}

/// Deterministically derive an Ed25519 `pkarr::Keypair` from a human-readable Room Code.
pub fn derive_room_keypair(room_code: &str) -> Keypair {
    let normalized = room_code.trim().to_lowercase();
    let secret_bytes = blake3::derive_key(
        "aethermc-v1-mainline-dht-room-code-seed",
        normalized.as_bytes(),
    );
    Keypair::from_secret_key(&secret_bytes)
}

/// Attempt silent UPnP port mapping via `igd-next` as an optional fast-path.
pub async fn try_upnp_port_mapping(local_udp_port: u16) -> Option<SocketAddr> {
    tokio::task::spawn_blocking(move || -> Option<SocketAddr> {
        let search_opts = igd_next::SearchOptions {
            timeout: Some(Duration::from_secs(2)),
            ..Default::default()
        };
        let gateway = igd_next::search_gateway(search_opts).ok()?;
        let local_ip = local_ipv4_addr()?;
        let local_sa = SocketAddrV4::new(local_ip, local_udp_port);

        gateway
            .add_port(
                igd_next::PortMappingProtocol::UDP,
                local_udp_port,
                SocketAddr::V4(local_sa),
                3600,
                "Beacon P2P QUIC",
            )
            .ok()?;

        let ext_ip = gateway.get_external_ip().ok()?;
        Some(SocketAddr::new(ext_ip, local_udp_port))
    })
    .await
    .ok()
    .flatten()
}

fn local_ipv4_addr() -> Option< std::net::Ipv4Addr> {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    match sock.local_addr().ok()?.ip() {
        IpAddr::V4(v4) => Some(v4),
        IpAddr::V6(_) => None,
    }
}

/// Publish the Host's `iroh::NodeAddr` to the BitTorrent Mainline DHT via `pkarr`
/// and save a local cache copy in `app_data_dir/rooms/` for instant LAN/same-PC lookup.
pub async fn publish_room_to_dht(
    app_data_dir: &Path,
    room_code: &str,
    node_addr: &NodeAddr,
    host_name: &str,
    mc_version: &str,
    loader: &str,
    upnp_mapped: Option<SocketAddr>,
) -> AppResult<RoomDhtRecord> {
    let normalized_code = room_code.trim().to_lowercase();
    let keypair = derive_room_keypair(&normalized_code);
    let pkarr_public_key = keypair.public_key().to_z32();

    let now_ms = now_ms();

    let record = RoomDhtRecord {
        room_code: normalized_code.clone(),
        pkarr_public_key: pkarr_public_key.clone(),
        node_addr: node_addr.clone(),
        host_name: host_name.to_string(),
        mc_version: mc_version.to_string(),
        loader: loader.to_string(),
        upnp_mapped,
        published_at_ms: now_ms,
        closed: false,
    };

    // Save local fallback record so local/same-machine testing resolves in <1ms
    let rooms_dir = app_data_dir.join("rooms");
    let _ = fs::create_dir_all(&rooms_dir);
    let cache_file = rooms_dir.join(format!("{}.json", normalized_code));
    if let Ok(json_str) = serde_json::to_string_pretty(&record) {
        let _ = fs::write(&cache_file, json_str);
    }

    // Build compact payload for Mainline DHT (< 900 bytes to fit cleanly in a signed DNS packet)
    let compact_json = serde_json::to_vec(&record)?;
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&compact_json);

    let keypair_clone = keypair;
    tokio::spawn(async move {
        if let Ok(client) = PkarrClient::builder().build() {
            let mut packet_builder = SignedPacket::builder();
            if let (Ok(name), Ok(txt)) = (
                Name::new("_aethermc"),
                TXT::try_from(encoded.as_str()),
            ) {
                packet_builder = packet_builder.txt(name, txt, 120);
                if let Ok(signed_packet) = packet_builder.build(&keypair_clone) {
                    let _ = client.publish(&signed_packet, None).await;
                }
            }
        }
    });

    Ok(record)
}

/// Resolve a 3-word Room Code (e.g., `crimson-dragon-42`) from BitTorrent Mainline DHT via `pkarr`.
pub async fn resolve_room_from_dht(
    app_data_dir: &Path,
    room_code: &str,
) -> AppResult<RoomDhtRecord> {
    let normalized_code = room_code.trim().to_lowercase();

    // 1. Check local room cache first for instant zero-latency local/LAN development resolution
    let local_cache = app_data_dir
        .join("rooms")
        .join(format!("{}.json", normalized_code));
    if local_cache.exists() {
        if let Ok(raw) = fs::read_to_string(&local_cache) {
            if let Ok(record) = serde_json::from_str::<RoomDhtRecord>(&raw) {
                if !record.closed {
                    return Ok(record);
                }
            }
        }
    }

    // 2. Query BitTorrent Mainline DHT via pkarr
    let keypair = derive_room_keypair(&normalized_code);
    let pubkey = keypair.public_key();

    let client = PkarrClient::builder()
        .build()
        .map_err(|e| AppError::P2p(format!("Failed to initialize Pkarr DHT client: {}", e)))?;

    let signed_packet = tokio::time::timeout(
        Duration::from_secs(10),
        client.resolve(&pubkey),
    )
    .await
    .map_err(|_| {
        AppError::P2p(format!(
            "Timed out resolving room '{}' on Mainline DHT",
            normalized_code
        ))
    })?
    .ok_or_else(|| {
        AppError::P2p(format!(
            "Room '{}' not found on BitTorrent Mainline DHT",
            normalized_code
        ))
    })?;

    if let Some(record) = record_from_packet(&signed_packet) {
        if record.closed {
            return Err(AppError::P2p(format!(
                "The room '{}' was closed by its host.",
                normalized_code
            )));
        }
        return Ok(record);
    }

    Err(AppError::P2p(format!(
        "Resolved DHT packet for '{}', but no valid Beacon NodeAddr TXT record was present",
        normalized_code
    )))
}
