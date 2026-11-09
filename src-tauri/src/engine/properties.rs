use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerProperties {
    /// Cracked/Offline Mode (`online-mode=false` when cracked/offline is enabled, `true` when Mojang auth is required)
    pub online_mode: bool,
    /// Keep Inventory gamerule (`gamerule keepInventory true/false`)
    pub keep_inventory: bool,
    /// `peaceful` | `easy` | `normal` | `hard`
    pub difficulty: String,
    /// Allow player vs player combat
    pub pvp: bool,
    /// Chunk render distance (4..32)
    pub view_distance: u32,
    /// Entity tick distance (4..32)
    pub simulation_distance: u32,
    /// Spawn protection radius (0..64)
    pub spawn_protection: u32,
    /// Whitelist enabled (`white-list`)
    pub white_list: bool,
    /// Server MOTD
    pub motd: String,
    /// Max players
    pub max_players: u32,
    /// Default gamemode (`survival` | `creative` | `adventure` | `spectator`)
    pub gamemode: String,
    /// Allow flight
    pub allow_flight: bool,
    /// Strictly bound local IP (127.0.0.1)
    pub server_ip: String,
    /// Strictly bound local port (25565)
    pub server_port: u16,
}

impl Default for ServerProperties {
    fn default() -> Self {
        Self {
            online_mode: false, // Default friendly for P2P home play; 1-click toggleable
            keep_inventory: true,
            difficulty: "normal".to_string(),
            pvp: true,
            view_distance: 10,
            simulation_distance: 8,
            spawn_protection: 0,
            white_list: false,
            motd: "§a[Beacon] §fZero-Cost Home Server".to_string(),
            max_players: 16,
            gamemode: "survival".to_string(),
            allow_flight: true,
            server_ip: "127.0.0.1".to_string(),
            server_port: 25565,
        }
    }
}

impl ServerProperties {
    pub fn load(server_dir: &Path) -> AppResult<Self> {
        let prop_path = server_dir.join("server.properties");
        let mut props = Self::default();

        if !prop_path.exists() {
            props.save(server_dir)?;
            return Ok(props);
        }

        let content = fs::read_to_string(&prop_path)?;
        let map = parse_properties_map(&content);

        if let Some(v) = map.get("online-mode") {
            props.online_mode = v.eq_ignore_ascii_case("true");
        }
        if let Some(v) = map.get("aethermc.keep-inventory") {
            props.keep_inventory = v.eq_ignore_ascii_case("true");
        }
        if let Some(v) = map.get("difficulty") {
            props.difficulty = v.clone();
        }
        if let Some(v) = map.get("pvp") {
            props.pvp = v.eq_ignore_ascii_case("true");
        }
        if let Some(v) = map.get("view-distance").and_then(|s| s.parse::<u32>().ok()) {
            props.view_distance = v.clamp(2, 32);
        }
        if let Some(v) = map
            .get("simulation-distance")
            .and_then(|s| s.parse::<u32>().ok())
        {
            props.simulation_distance = v.clamp(2, 32);
        }
        if let Some(v) = map
            .get("spawn-protection")
            .and_then(|s| s.parse::<u32>().ok())
        {
            props.spawn_protection = v;
        }
        if let Some(v) = map.get("white-list") {
            props.white_list = v.eq_ignore_ascii_case("true");
        }
        if let Some(v) = map.get("motd") {
            props.motd = v.clone();
        }
        if let Some(v) = map.get("max-players").and_then(|s| s.parse::<u32>().ok()) {
            props.max_players = v.max(1);
        }
        if let Some(v) = map.get("gamemode") {
            props.gamemode = v.clone();
        }
        if let Some(v) = map.get("allow-flight") {
            props.allow_flight = v.eq_ignore_ascii_case("true");
        }

        // Enforce strict localhost binding per Section 3.1.3
        props.server_ip = "127.0.0.1".to_string();
        props.server_port = 25565;

        Ok(props)
    }

    pub fn save(&self, server_dir: &Path) -> AppResult<()> {
        fs::create_dir_all(server_dir)?;
        let prop_path = server_dir.join("server.properties");

        let mut map = if prop_path.exists() {
            let existing = fs::read_to_string(&prop_path).unwrap_or_default();
            parse_properties_map(&existing)
        } else {
            BTreeMap::new()
        };

        // Strict localhost P2P ingress security
        map.insert("server-ip".to_string(), "127.0.0.1".to_string());
        map.insert("server-port".to_string(), "25565".to_string());
        map.insert("online-mode".to_string(), self.online_mode.to_string());
        map.insert(
            "aethermc.keep-inventory".to_string(),
            self.keep_inventory.to_string(),
        );
        map.insert("difficulty".to_string(), self.difficulty.clone());
        map.insert("pvp".to_string(), self.pvp.to_string());
        map.insert("view-distance".to_string(), self.view_distance.to_string());
        map.insert(
            "simulation-distance".to_string(),
            self.simulation_distance.to_string(),
        );
        map.insert(
            "spawn-protection".to_string(),
            self.spawn_protection.to_string(),
        );
        map.insert("white-list".to_string(), self.white_list.to_string());
        map.insert("enforce-whitelist".to_string(), self.white_list.to_string());
        map.insert("motd".to_string(), self.motd.clone());
        map.insert("max-players".to_string(), self.max_players.to_string());
        map.insert("gamemode".to_string(), self.gamemode.clone());
        map.insert("allow-flight".to_string(), self.allow_flight.to_string());
        map.insert("enable-command-block".to_string(), "true".to_string());
        map.insert("sync-chunk-writes".to_string(), "true".to_string());

        let mut out = String::from("#Minecraft server properties (Managed by AetherMC)\n");
        for (k, v) in map {
            out.push_str(&format!("{}={}\n", k, v));
        }

        fs::write(&prop_path, out)?;
        Ok(())
    }
}

fn parse_properties_map(content: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    map
}
