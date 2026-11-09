//! Automatic NeoForge server provisioning.
//!
//! NeoForge has no single runnable server jar. Instead we download the official
//! installer, run it once with `--installServer`, and then launch the JVM with the
//! generated `@libraries/.../{win,unix}_args.txt` argument file.

use crate::engine::jvm::JvmProgressEvent;
use crate::engine::runner::{download_with_sha256, ServerRunner};
use crate::error::{AppError, AppResult};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

const VERSIONS_URL: &str = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";

/// `1.21.1` -> `21.1.`, `1.21` -> `21.0.`, `1.20.4` -> `20.4.`,
/// and for the year-based versions `26.1.2` -> `26.1.2.`, `26.2` -> `26.2.0.`
pub fn neoforge_prefix(mc_version: &str) -> Option<String> {
    let nums = mc_version
        .split('.')
        .map(|p| p.parse::<u32>().ok())
        .collect::<Option<Vec<_>>>()?;
    match nums.as_slice() {
        [1, major, rest @ ..] if rest.len() <= 1 => {
            Some(format!("{}.{}.", major, rest.first().copied().unwrap_or(0)))
        }
        [year, minor, rest @ ..] if *year >= 26 && rest.len() <= 1 => Some(format!(
            "{}.{}.{}.",
            year,
            minor,
            rest.first().copied().unwrap_or(0)
        )),
        _ => None,
    }
}

/// Pick the newest release (preferring non-beta) for a Minecraft version.
pub fn pick_neoforge_version(all: &[String], mc_version: &str) -> Option<String> {
    let prefix = neoforge_prefix(mc_version)?;
    let build_number = |v: &str| -> u32 {
        v[prefix.len()..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or(0)
    };
    let candidates: Vec<&String> = all.iter().filter(|v| v.starts_with(&prefix)).collect();
    let stable: Vec<&&String> = candidates.iter().filter(|v| !v.contains("beta") && !v.contains("alpha") && !v.contains("snapshot")).collect();
    if !stable.is_empty() {
        stable.into_iter().max_by_key(|v| build_number(v)).map(|v| (*v).clone())
    } else {
        candidates.into_iter().max_by_key(|v| build_number(v)).cloned()
    }
}

fn args_file_rel(version: &str) -> String {
    let name = if cfg!(windows) { "win_args.txt" } else { "unix_args.txt" };
    format!("libraries/net/neoforged/neoforge/{}/{}", version, name)
}

fn launch_args(version: &str) -> Vec<String> {
    vec![format!("@{}", args_file_rel(version)), "nogui".to_string()]
}

impl ServerRunner {
    fn neoforge_marker(&self, mc_version: &str) -> PathBuf {
        self.server_dir().join(format!("neoforge-{}.version", mc_version))
    }

    fn emit_install_progress(&self, app: Option<&AppHandle>, stage: &str, percent: f32) {
        if let Some(app) = app {
            let _ = app.emit(
                "jvm:download-progress",
                JvmProgressEvent {
                    stage: stage.to_string(),
                    downloaded_bytes: 0,
                    total_bytes: 0,
                    percent,
                },
            );
        }
    }

    /// Make sure NeoForge is installed for `mc_version` and return the JVM launch arguments.
    pub async fn ensure_neoforge(
        &self,
        mc_version: &str,
        java_bin: &Path,
        app_handle: Option<&AppHandle>,
    ) -> AppResult<Vec<String>> {
        // Already installed? (marker file remembers which NeoForge version we used)
        if let Ok(v) = std::fs::read_to_string(self.neoforge_marker(mc_version)) {
            let v = v.trim().to_string();
            if !v.is_empty() && self.server_dir().join(args_file_rel(&v)).is_file() {
                return Ok(launch_args(&v));
            }
        }

        if neoforge_prefix(mc_version).is_none() {
            return Err(AppError::Content(format!(
                "NeoForge doesn't support Minecraft {}.",
                mc_version
            )));
        }

        let client = reqwest::Client::builder()
            .user_agent("Beacon/0.1.0 (Minecraft server manager)")
            .build()?;

        self.emit_log(app_handle, "INFO", format!("Looking for NeoForge for Minecraft {}...", mc_version))
            .await;
        self.emit_install_progress(app_handle, "Finding the right NeoForge version…", 5.0);

        let resp = client.get(VERSIONS_URL).send().await?;
        if !resp.status().is_success() {
            return Err(AppError::Network(format!(
                "Couldn't reach the NeoForge website (HTTP {}). Check your internet connection and try again.",
                resp.status()
            )));
        }
        let body: serde_json::Value = resp.json().await?;
        let all: Vec<String> = body["versions"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let version = pick_neoforge_version(&all, mc_version).ok_or_else(|| {
            AppError::Content(format!(
                "NeoForge isn't available for Minecraft {}. Try a newer version (1.20.2 or later), or pick Fabric.",
                mc_version
            ))
        })?;

        let installer = self.server_dir().join(format!("neoforge-{}-installer.jar", version));
        let url = format!(
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/{v}/neoforge-{v}-installer.jar",
            v = version
        );
        self.emit_log(app_handle, "INFO", format!("Downloading NeoForge {} installer...", version))
            .await;
        self.emit_install_progress(app_handle, &format!("Downloading NeoForge {}…", version), 20.0);
        download_with_sha256(&client, &url, &installer, None).await?;

        self.emit_log(
            app_handle,
            "INFO",
            "Installing NeoForge (first time only, about a minute)...",
        )
        .await;
        self.emit_install_progress(app_handle, "Installing NeoForge (first time only)…", 45.0);

        let mut cmd = Command::new(java_bin);
        cmd.current_dir(&self.server_dir())
            .arg("-jar")
            .arg(&installer)
            .arg("--installServer")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::Process(format!("Couldn't run the NeoForge installer: {}", e)))?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        for stream in [
            stdout.map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>),
            stderr.map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>),
        ]
        .into_iter()
        .flatten()
        {
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stream).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx.send(line);
                }
            });
        }
        drop(tx);

        let mut tail: Vec<String> = Vec::new();
        while let Some(line) = rx.recv().await {
            if line.trim().is_empty() {
                continue;
            }
            tail.push(line.clone());
            if tail.len() > 8 {
                tail.remove(0);
            }
            self.emit_log(app_handle, "INFO", line).await;
        }
        let status = child.wait().await?;

        let _ = std::fs::remove_file(&installer);
        let _ = std::fs::remove_file(self.server_dir().join(format!("neoforge-{}-installer.jar.log", version)));

        if !status.success() || !self.server_dir().join(args_file_rel(&version)).is_file() {
            return Err(AppError::Process(format!(
                "The NeoForge installer didn't finish. {}",
                tail.last().cloned().unwrap_or_default()
            )));
        }

        std::fs::write(self.neoforge_marker(mc_version), &version)?;
        self.emit_install_progress(app_handle, "NeoForge is ready", 95.0);
        self.emit_log(app_handle, "INFO", format!("NeoForge {} installed.", version))
            .await;
        Ok(launch_args(&version))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_newest_stable_build() {
        let all: Vec<String> = ["21.1.9", "21.1.172", "21.1.265", "21.1.300-beta", "21.4.50", "20.4.237", "20.4.0-beta"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(pick_neoforge_version(&all, "1.21.1").as_deref(), Some("21.1.265"));
        assert_eq!(pick_neoforge_version(&all, "1.21.4").as_deref(), Some("21.4.50"));
        assert_eq!(pick_neoforge_version(&all, "1.20.4").as_deref(), Some("20.4.237"));
        assert_eq!(pick_neoforge_version(&all, "1.20.1"), None);

        let new: Vec<String> = ["26.1.2.125", "26.1.2.126", "26.2.0.93", "26.2.0.94-beta", "26.1.0.0-alpha.1+snapshot-1"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(neoforge_prefix("26.1.2").as_deref(), Some("26.1.2."));
        assert_eq!(neoforge_prefix("26.2").as_deref(), Some("26.2.0."));
        assert_eq!(pick_neoforge_version(&new, "26.1.2").as_deref(), Some("26.1.2.126"));
        assert_eq!(pick_neoforge_version(&new, "26.2").as_deref(), Some("26.2.0.93"));
    }
}
