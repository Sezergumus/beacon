use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

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

/// Longest welcome message line, in characters (colour codes count).
const MAX_MOTD_LINE_CHARS: usize = 120;

impl ServerProperties {
    /// Check the values that must come from a fixed list or that would break the
    /// file. The settings come straight from the window, so nothing is trusted.
    pub fn validate(&self) -> AppResult<()> {
        if !["survival", "creative", "adventure", "spectator"].contains(&self.gamemode.as_str()) {
            return Err(AppError::Custom("That isn't a valid game mode.".to_string()));
        }
        if !["peaceful", "easy", "normal", "hard"].contains(&self.difficulty.as_str()) {
            return Err(AppError::Custom("That isn't a valid difficulty.".to_string()));
        }
        // The welcome message may have two lines; any other control character would
        // start a new line (a new setting) in the file.
        if self.motd.chars().any(|c| c.is_control() && c != '\n') {
            return Err(AppError::Custom(
                "The welcome message can only contain text and one line break.".to_string(),
            ));
        }
        let lines: Vec<&str> = self.motd.split('\n').collect();
        if lines.len() > 2 {
            return Err(AppError::Custom(
                "The welcome message can have two lines at most.".to_string(),
            ));
        }
        if lines.iter().any(|l| l.chars().count() > MAX_MOTD_LINE_CHARS) {
            return Err(AppError::Custom("That welcome message is too long.".to_string()));
        }
        Ok(())
    }

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
            props.motd = unescape_property(v);
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
        map.insert("motd".to_string(), escape_property(&self.motd));
        map.insert("max-players".to_string(), self.max_players.to_string());
        map.insert("gamemode".to_string(), self.gamemode.clone());
        map.insert("allow-flight".to_string(), self.allow_flight.to_string());
        map.insert("enable-command-block".to_string(), "true".to_string());
        map.insert("sync-chunk-writes".to_string(), "true".to_string());

        let mut out = String::from("#Minecraft server properties (Managed by Beacon)\n");
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

/// Write a value the way Java `.properties` files expect it: line breaks and
/// backslashes escaped, anything outside ASCII (like the `§` colour sign) as `\uXXXX`.
fn escape_property(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\\' => out.push_str("\\\\"),
            c if c.is_ascii() => out.push(c),
            c => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{:04x}", unit));
                }
            }
        }
    }
    out
}

/// The reverse of [`escape_property`]. Text that was never escaped (files written by
/// older versions, with a raw `§`) comes back unchanged.
fn unescape_property(value: &str) -> String {
    fn push(units: &mut Vec<u16>, c: char) {
        let mut buf = [0u16; 2];
        units.extend_from_slice(c.encode_utf16(&mut buf));
    }
    let mut units: Vec<u16> = Vec::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            push(&mut units, c);
            continue;
        }
        match chars.next() {
            Some('n') => units.push(b'\n' as u16),
            Some('t') => units.push(b'\t' as u16),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                match u16::from_str_radix(&hex, 16) {
                    Ok(unit) if hex.len() == 4 => units.push(unit),
                    _ => {
                        push(&mut units, 'u');
                        hex.chars().for_each(|h| push(&mut units, h));
                    }
                }
            }
            Some(other) => push(&mut units, other),
            None => push(&mut units, '\\'),
        }
    }
    String::from_utf16_lossy(&units)
}

/// Where the starting settings for new worlds live.
fn defaults_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("default-settings.json")
}

/// The settings every new world starts with (what the Settings page edits).
pub fn load_defaults(app_data_dir: &Path) -> ServerProperties {
    fs::read_to_string(defaults_path(app_data_dir))
        .ok()
        .and_then(|raw| serde_json::from_str::<ServerProperties>(&raw).ok())
        .filter(|p| p.validate().is_ok())
        .map(|mut p| {
            p.server_ip = "127.0.0.1".to_string();
            p.server_port = 25565;
            p
        })
        .unwrap_or_default()
}

pub fn save_defaults(app_data_dir: &Path, props: &ServerProperties) -> AppResult<ServerProperties> {
    props.validate()?;
    let mut props = props.clone();
    props.view_distance = props.view_distance.clamp(2, 32);
    props.simulation_distance = props.simulation_distance.clamp(2, 32);
    props.spawn_protection = props.spawn_protection.min(64);
    props.max_players = props.max_players.clamp(1, 1000);
    props.server_ip = "127.0.0.1".to_string();
    props.server_port = 25565;
    fs::create_dir_all(app_data_dir)?;
    fs::write(defaults_path(app_data_dir), serde_json::to_string_pretty(&props)?)?;
    Ok(props)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("aethermc_props_{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn motd_survives_a_save_and_load() {
        let dir = temp("motd");
        let mut props = ServerProperties::default();
        props.motd = "§aHello §lworld\nSecond \\ line ✓".to_string();
        props.save(&dir).unwrap();
        let raw = fs::read_to_string(dir.join("server.properties")).unwrap();
        // One physical line of plain ASCII, as Minecraft expects.
        let line = raw.lines().find(|l| l.starts_with("motd=")).unwrap();
        assert!(line.is_ascii());
        assert!(line.contains("\\u00a7a") && line.contains("\\n"));
        assert_eq!(ServerProperties::load(&dir).unwrap().motd, props.motd);
    }

    #[test]
    fn old_files_with_a_raw_colour_sign_still_load() {
        assert_eq!(unescape_property("§a[Beacon] §fHome"), "§a[Beacon] §fHome");
    }

    #[test]
    fn motd_rules() {
        let mut props = ServerProperties::default();
        props.motd = "one\ntwo".to_string();
        assert!(props.validate().is_ok());
        props.motd = "one\ntwo\nthree".to_string();
        assert!(props.validate().is_err());
        props.motd = "bad\rline".to_string();
        assert!(props.validate().is_err());
    }

    #[test]
    fn defaults_round_trip_and_fall_back() {
        let dir = temp("defaults");
        assert_eq!(load_defaults(&dir).difficulty, "normal");
        let mut props = ServerProperties::default();
        props.difficulty = "hard".to_string();
        props.motd = "Hi\nthere".to_string();
        save_defaults(&dir, &props).unwrap();
        let back = load_defaults(&dir);
        assert_eq!(back.difficulty, "hard");
        assert_eq!(back.motd, "Hi\nthere");
        props.gamemode = "nonsense".to_string();
        assert!(save_defaults(&dir, &props).is_err());
    }
}
