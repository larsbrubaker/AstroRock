//! # Wasm settings store — `localStorage`
//!
//! The modern `Astro.cfg` for the browser build: one JSON string
//! under a fixed key. Private-mode browsers that refuse storage just
//! run on defaults.

use astrorock_core::settings::SettingsStore;

const STORAGE_KEY: &str = "astrorock-settings";

pub struct LocalStorageSettings;

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

impl SettingsStore for LocalStorageSettings {
    fn load(&self) -> Option<String> {
        storage()?.get_item(STORAGE_KEY).ok()?
    }

    fn save(&self, json: &str) {
        if let Some(s) = storage() {
            let _ = s.set_item(STORAGE_KEY, json);
        }
    }
}
