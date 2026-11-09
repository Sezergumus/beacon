//! Settings of the app itself (not of any world), kept in `app_settings.json`.

use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use std::path::Path;

const FILE: &str = "app_settings.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AppSettings {
    /// Closing the window keeps a running world going from the system tray.
    pub close_to_tray: bool,
    /// The welcome tour has been seen (or skipped).
    pub onboarding_done: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            onboarding_done: false,
        }
    }
}

impl AppSettings {
    pub fn load(app_data_dir: &Path) -> Self {
        std::fs::read_to_string(app_data_dir.join(FILE))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
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
        assert!(s.onboarding_done && s.close_to_tray);
        let mut s2 = s.clone();
        s2.close_to_tray = false;
        s2.save(&dir).unwrap();
        assert_eq!(AppSettings::load(&dir), s2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
