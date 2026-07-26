//! # Native settings store — the modern `Astro.cfg`
//!
//! JSON in `%APPDATA%\AstroRock\settings.json` (falling back to the
//! working directory when APPDATA is unset, e.g. odd CI shells).

use std::path::PathBuf;

use astrorock_core::settings::SettingsStore;

pub struct FileSettings {
    path: PathBuf,
}

impl FileSettings {
    pub fn new() -> Self {
        let dir = std::env::var_os("APPDATA")
            .map(|base| PathBuf::from(base).join("AstroRock"))
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            path: dir.join("settings.json"),
        }
    }
}

impl SettingsStore for FileSettings {
    fn load(&self) -> Option<String> {
        std::fs::read_to_string(&self.path).ok()
    }

    fn save(&self, json: &str) {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(err) = std::fs::write(&self.path, json) {
            eprintln!("settings: save failed ({}): {err}", self.path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_through_a_temp_file() {
        let dir = std::env::temp_dir().join(format!("astrorock-settings-{}", std::process::id()));
        let store = FileSettings {
            path: dir.join("settings.json"),
        };
        assert!(store.load().is_none());
        store.save("{\"start_level\": 5}");
        assert_eq!(store.load().as_deref(), Some("{\"start_level\": 5}"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
