//! Settings of the app itself (not of any world), kept in `app_settings.json`.

use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use std::path::Path;

const FILE: &str = "app_settings.json";

/// What the window's close button does.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CloseAction {
    /// Close Beacon (an open world is saved and stopped first).
    Quit,
    /// Hide to the system tray while a world is open, otherwise close.
    TrayWhenRunning,
    /// Always hide to the system tray.
    TrayAlways,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AppSettings {
    pub close_action: CloseAction,
    /// The welcome tour has been seen (or skipped).
    pub onboarding_done: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            close_action: CloseAction::TrayWhenRunning,
            onboarding_done: false,
        }
    }
}

impl AppSettings {
    pub fn load(app_data_dir: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(app_data_dir.join(FILE)) else {
            return Self::default();
        };
        let mut settings: Self = serde_json::from_str(&text).unwrap_or_default();
        // Older versions had a plain on/off `close_to_tray`; keep the choice of those who turned it off.
        if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&text) {
            if raw.get("close_action").is_none()
                && raw.get("close_to_tray").and_then(|v| v.as_bool()) == Some(false)
            {
                settings.close_action = CloseAction::Quit;
            }
        }
        settings
    }

    pub fn save(&self, app_data_dir: &Path) -> AppResult<()> {
        std::fs::create_dir_all(app_data_dir)?;
        let tmp = app_data_dir.join(format!("{}.tmp", FILE));
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, app_data_dir.join(FILE))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_partial_file_uses_defaults() {
        let dir = std::env::temp_dir().join(format!("beacon-as-{}", uuid::Uuid::new_v4()));
        assert_eq!(AppSettings::load(&dir), AppSettings::default());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FILE), r#"{"onboarding_done":true}"#).unwrap();
        let s = AppSettings::load(&dir);
        assert!(s.onboarding_done && s.close_action == CloseAction::TrayWhenRunning);
        let mut s2 = s.clone();
        s2.close_action = CloseAction::TrayAlways;
        s2.save(&dir).unwrap();
        assert_eq!(AppSettings::load(&dir), s2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn old_close_to_tray_off_means_quit() {
        let dir = std::env::temp_dir().join(format!("beacon-as-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FILE), r#"{"close_to_tray":false}"#).unwrap();
        assert_eq!(AppSettings::load(&dir).close_action, CloseAction::Quit);
        std::fs::write(dir.join(FILE), r#"{"close_to_tray":true}"#).unwrap();
        assert_eq!(AppSettings::load(&dir).close_action, CloseAction::TrayWhenRunning);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
