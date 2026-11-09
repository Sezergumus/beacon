//! Which Minecraft versions each server type supports, and which Java they need.
//!
//! Lists come straight from the sources the servers are downloaded from, so new
//! releases show up without an app update:
//! - Paper:    PaperMC "Fill" API
//! - Fabric:   Fabric Meta (stable game versions)
//! - NeoForge: the NeoForge Maven (build numbers are mapped back to Minecraft versions)

use crate::engine::runner::ServerCore;
use crate::error::{AppError, AppResult};
use std::cmp::Ordering;
use std::collections::BTreeSet;

const PAPER_URL: &str = "https://fill.papermc.io/v3/projects/paper";
const FABRIC_URL: &str = "https://meta.fabricmc.net/v2/versions/game";
const NEOFORGE_URL: &str = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";

/// The oldest Java that runs `mc_version`, as one of the LTS builds Adoptium ships
/// (8, 17, 21, 25). A newer Java would start many old versions, but not reliably.
pub fn required_java_major(mc_version: &str) -> u8 {
    let nums: Vec<u32> = mc_version
        .split(|c| c == '.' || c == '-')
        .map_while(|p| p.parse().ok())
        .collect();
    match nums.as_slice() {
        // Mojang's year-based versions (26.1, 26.2, ...)
        [first, ..] if *first >= 26 => 25,
        [1, minor, rest @ ..] => {
            let patch = rest.first().copied().unwrap_or(0);
            match *minor {
                0..=16 => 8,
                17..=19 => 17,
                20 if patch < 5 => 17,
                _ => 21,
            }
        }
        _ => 21,
    }
}

fn version_numbers(v: &str) -> Vec<u32> {
    v.split('.').map_while(|p| p.parse().ok()).collect()
}

/// Newest first. Compares numerically, so `1.21.10 > 1.21.9` and `26.1 > 1.21.11`.
fn newest_first(a: &String, b: &String) -> Ordering {
    version_numbers(b).cmp(&version_numbers(a))
}

/// Plain releases only: no `-rc1`, `-pre2` or snapshot names.
fn is_release(v: &str) -> bool {
    !v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// `21.1.265` -> `1.21.1`, `21.0.167` -> `1.21`, `26.1.2.126` -> `26.1.2`, `26.2.0.93` -> `26.2`.
/// Pre-releases (`-beta`, `-alpha`, snapshots) return `None`.
pub fn neoforge_to_mc_version(build: &str) -> Option<String> {
    if build.contains('-') || build.contains('+') {
        return None;
    }
    let n = version_numbers(build);
    if n.len() != build.split('.').count() {
        return None;
    }
    match n.as_slice() {
        [major, minor, _patch, _build] if *major >= 26 => Some(if *_patch == 0 {
            format!("{}.{}", major, minor)
        } else {
            format!("{}.{}.{}", major, minor, _patch)
        }),
        [major, minor, _build] if *major < 26 && *major >= 20 => Some(if *minor == 0 {
            format!("1.{}", major)
        } else {
            format!("1.{}.{}", major, minor)
        }),
        _ => None,
    }
}

fn sorted_unique(versions: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut v: Vec<String> = versions
        .into_iter()
        .filter(|v| is_release(v))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    v.sort_by(newest_first);
    v
}

fn client() -> AppResult<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent("Beacon/0.1.0 (Minecraft server manager)")
        .timeout(std::time::Duration::from_secs(15))
        .build()?)
}

async fn get_json(url: &str) -> AppResult<serde_json::Value> {
    let resp = client()?.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(AppError::Network(format!(
            "Couldn't load the version list (HTTP {}). Check your internet connection.",
            resp.status()
        )));
    }
    Ok(resp.json().await?)
}

/// Every Minecraft release the given server type can run, newest first.
pub async fn list_versions(core: ServerCore) -> AppResult<Vec<String>> {
    let versions = match core {
        ServerCore::Paper => {
            let body = get_json(PAPER_URL).await?;
            // Grouped by minor version: {"1.21": ["1.21.4", ...], ...}
            let groups = body["versions"].as_object().cloned().unwrap_or_default();
            sorted_unique(
                groups
                    .values()
                    .filter_map(|g| g.as_array())
                    .flatten()
                    .filter_map(|v| v.as_str().map(String::from)),
            )
        }
        ServerCore::Fabric => {
            let body = get_json(FABRIC_URL).await?;
            sorted_unique(
                body.as_array()
                    .into_iter()
                    .flatten()
                    .filter(|v| v["stable"].as_bool().unwrap_or(false))
                    .filter_map(|v| v["version"].as_str().map(String::from)),
            )
        }
        ServerCore::Neoforge => {
            let body = get_json(NEOFORGE_URL).await?;
            sorted_unique(
                body["versions"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_str())
                    .filter_map(neoforge_to_mc_version),
            )
        }
    };

    if versions.is_empty() {
        return Err(AppError::Content(
            "The version list came back empty. Try again in a moment.".to_string(),
        ));
    }
    Ok(versions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_matches_the_minecraft_version() {
        assert_eq!(required_java_major("1.7.10"), 8);
        assert_eq!(required_java_major("1.12.2"), 8);
        assert_eq!(required_java_major("1.16.5"), 8);
        assert_eq!(required_java_major("1.17.1"), 17);
        assert_eq!(required_java_major("1.20.1"), 17);
        assert_eq!(required_java_major("1.20.4"), 17);
        assert_eq!(required_java_major("1.20.5"), 21);
        assert_eq!(required_java_major("1.20.6"), 21);
        assert_eq!(required_java_major("1.21.11"), 21);
        assert_eq!(required_java_major("26.1.2"), 25);
        assert_eq!(required_java_major("26.3"), 25);
    }

    #[test]
    fn versions_sort_numerically_and_drop_prereleases() {
        let list = sorted_unique(
            ["1.9.4", "1.21.9", "1.21.10", "26.1", "1.21.11-rc1", "1.21", "1.21.1", "1.21.1"]
                .iter()
                .map(|s| s.to_string()),
        );
        assert_eq!(list, ["26.1", "1.21.10", "1.21.9", "1.21.1", "1.21", "1.9.4"]);
    }

    #[test]
    fn neoforge_builds_map_back_to_minecraft_versions() {
        assert_eq!(neoforge_to_mc_version("21.1.265").as_deref(), Some("1.21.1"));
        assert_eq!(neoforge_to_mc_version("21.0.167").as_deref(), Some("1.21"));
        assert_eq!(neoforge_to_mc_version("20.4.237").as_deref(), Some("1.20.4"));
        assert_eq!(neoforge_to_mc_version("21.11.48").as_deref(), Some("1.21.11"));
        assert_eq!(neoforge_to_mc_version("26.1.2.126").as_deref(), Some("26.1.2"));
        assert_eq!(neoforge_to_mc_version("26.2.0.93").as_deref(), Some("26.2"));
        assert_eq!(neoforge_to_mc_version("21.1.300-beta"), None);
        assert_eq!(neoforge_to_mc_version("26.1.0.0-alpha.1+snapshot-1"), None);
        assert_eq!(neoforge_to_mc_version("0.25w14craftmine.3-beta"), None);
    }

    /// Hits the real servers: `cargo test --lib live_version_lists -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn live_version_lists() {
        for core in [ServerCore::Paper, ServerCore::Fabric, ServerCore::Neoforge] {
            let v = list_versions(core).await.expect("version list");
            println!("{:?}: {} versions, newest {:?}, oldest {:?}", core, v.len(), &v[..3], v.last());
            assert!(v.contains(&"1.21.1".to_string()));
        }
    }
}
