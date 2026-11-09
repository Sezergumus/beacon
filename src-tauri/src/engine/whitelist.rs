//! Reading and editing a world's `whitelist.json`.
//!
//! While a world is running the server owns the file (it rewrites it on every change), so the
//! commands in `lib.rs` talk to the server then. These helpers are for a stopped world, and for
//! reading the list back in both cases.

use crate::error::{AppError, AppResult};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const FILE: &str = "whitelist.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WhitelistEntry {
    pub uuid: String,
    pub name: String,
}

/// A Minecraft username: 3-16 letters, digits or underscores. Checked before anything is
/// sent to the server console, so a name can never smuggle in a second command.
pub fn validate_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    let ok = (3..=16).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok {
        Ok(name.to_string())
    } else {
        Err(AppError::Custom(
            "A player name has 3 to 16 letters, numbers or underscores.".to_string(),
        ))
    }
}

pub fn load(server_dir: &Path) -> Vec<WhitelistEntry> {
    let Ok(text) = fs::read_to_string(server_dir.join(FILE)) else {
        return Vec::new();
    };
    let mut list: Vec<WhitelistEntry> = serde_json::from_str(&text).unwrap_or_default();
    list.sort_by_key(|e| e.name.to_lowercase());
    list
}

fn save(server_dir: &Path, list: &[WhitelistEntry]) -> AppResult<()> {
    fs::create_dir_all(server_dir)?;
    let json = serde_json::to_string_pretty(list)?;
    let tmp = server_dir.join(format!("{}.tmp", FILE));
    fs::write(&tmp, json)?;
    fs::rename(&tmp, server_dir.join(FILE))?;
    Ok(())
}

pub fn contains(list: &[WhitelistEntry], name: &str) -> bool {
    list.iter().any(|e| e.name.eq_ignore_ascii_case(name))
}

/// The UUID a server in offline mode gives a player (Java's `UUID.nameUUIDFromBytes`).
pub fn offline_uuid(name: &str) -> String {
    let mut bytes: [u8; 16] = Md5::digest(format!("OfflinePlayer:{}", name).as_bytes()).into();
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes).to_string()
}

#[derive(Deserialize)]
struct MojangProfile {
    id: String,
    name: String,
}

/// Ask Mojang for the real account behind a name (for worlds that check accounts).
async fn lookup_online(name: &str) -> AppResult<WhitelistEntry> {
    let client = reqwest::Client::builder()
        .user_agent("Beacon/0.1.0")
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    let resp = client
        .get(format!(
            "https://api.mojang.com/users/profiles/minecraft/{}",
            name
        ))
        .send()
        .await
        .map_err(|_| {
            AppError::Network("Couldn't reach Mojang to look that player up. Check your internet.".to_string())
        })?;
    let status = resp.status();
    if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::NO_CONTENT {
        return Err(AppError::Custom(format!(
            "No Minecraft account is called “{}”. Check the spelling.",
            name
        )));
    }
    if !status.is_success() {
        return Err(AppError::Network(format!(
            "Mojang couldn't look that player up right now ({}). Try again in a minute.",
            status
        )));
    }
    let profile: MojangProfile = resp.json().await?;
    let id = profile.id.replace('-', "");
    let uuid = uuid::Uuid::parse_str(&id)
        .map_err(|_| AppError::Custom("Mojang sent back an unexpected answer.".to_string()))?
        .to_string();
    Ok(WhitelistEntry {
        uuid,
        name: profile.name,
    })
}

/// Add a player while the world is stopped.
pub async fn add_offline(
    server_dir: &Path,
    name: &str,
    online_mode: bool,
) -> AppResult<Vec<WhitelistEntry>> {
    let name = validate_name(name)?;
    let mut list = load(server_dir);
    if contains(&list, &name) {
        return Ok(list);
    }
    let entry = if online_mode {
        lookup_online(&name).await?
    } else {
        WhitelistEntry {
            uuid: offline_uuid(&name),
            name,
        }
    };
    // The real account name may differ in case from what was typed.
    if contains(&list, &entry.name) {
        return Ok(list);
    }
    list.push(entry);
    list.sort_by_key(|e| e.name.to_lowercase());
    save(server_dir, &list)?;
    Ok(list)
}

/// Remove a player while the world is stopped.
pub fn remove_offline(server_dir: &Path, name: &str) -> AppResult<Vec<WhitelistEntry>> {
    let mut list = load(server_dir);
    list.retain(|e| !e.name.eq_ignore_ascii_case(name));
    save(server_dir, &list)?;
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_java() {
        // Known value: UUID.nameUUIDFromBytes("OfflinePlayer:Notch".getBytes())
        assert_eq!(offline_uuid("Notch"), "b50ad385-829d-3141-a216-7e7d7539ba7f");
    }

    #[test]
    fn rejects_bad_names() {
        assert!(validate_name("Steve_01").is_ok());
        assert!(validate_name("  Alex ").is_ok());
        assert!(validate_name("ab").is_err());
        assert!(validate_name("way_too_long_name_x").is_err());
        assert!(validate_name("a b").is_err());
        assert!(validate_name("x\nstop").is_err());
    }

    #[test]
    fn add_remove_roundtrip() {
        let dir = std::env::temp_dir().join(format!("beacon-wl-{}", uuid::Uuid::new_v4()));
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let list = rt.block_on(add_offline(&dir, "Steve", false)).unwrap();
        assert_eq!(list.len(), 1);
        // Adding again with different case doesn't duplicate.
        let list = rt.block_on(add_offline(&dir, "steve", false)).unwrap();
        assert_eq!(list.len(), 1);
        let list = remove_offline(&dir, "STEVE").unwrap();
        assert!(list.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
