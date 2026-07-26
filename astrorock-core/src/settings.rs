//! # Settings — the modern `Astro.cfg`
//!
//! The original persisted a binary `Astro.cfg` (key scancodes, volumes,
//! stereo/mixing flags, start level, highest level reached, high
//! scores). The port stores JSON through a platform trait: a file on
//! native, `localStorage` on wasm. Missing/corrupt data falls back to
//! the shipped defaults, exactly like `LoadConfig`.

use agg_gui::event::Key;
use serde::{Deserialize, Serialize};

use crate::input::Bindings;

/// Platform persistence: shells hand one to the game at startup.
pub trait SettingsStore {
    fn load(&self) -> Option<String>;
    fn save(&self, json: &str);
}

/// Everything the game remembers between runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub key_left: String,
    pub key_right: String,
    pub key_thrust: String,
    pub key_fire: String,
    pub key_shield: String,
    pub key_bomb: String,
    pub start_level: u32,
    pub music_on: bool,
    pub sfx_on: bool,
    /// Slider fractions, 0.0 (silent) .. 1.0 (full) — the port of the
    /// dB ramps on `GlobalSetVolume`/`CStreamSoundSetVolume`.
    pub master_volume: f32,
    pub music_volume: f32,
    /// `GlobalHighScores` — 5 entries, name + score, sorted
    /// descending (`HSListInit(5,15)`; fresh slots are "EMPTY"/0).
    pub high_scores: Vec<(String, u32)>,
}

/// The 1997 fresh-install table.
pub fn default_high_scores() -> Vec<(String, u32)> {
    vec![("EMPTY".to_string(), 0); 5]
}

impl Default for Settings {
    fn default() -> Self {
        let b = Bindings::default();
        Self {
            key_left: key_to_string(&b.left),
            key_right: key_to_string(&b.right),
            key_thrust: key_to_string(&b.thrust),
            key_fire: key_to_string(&b.fire),
            key_shield: key_to_string(&b.shield),
            key_bomb: key_to_string(&b.bomb),
            start_level: 0,
            music_on: true,
            sfx_on: true,
            master_volume: 1.0,
            music_volume: 1.0,
            high_scores: default_high_scores(),
        }
    }
}

impl Settings {
    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("settings serialize")
    }

    pub fn bindings(&self) -> Bindings {
        let d = Bindings::default();
        Bindings {
            left: string_to_key(&self.key_left).unwrap_or(d.left),
            right: string_to_key(&self.key_right).unwrap_or(d.right),
            thrust: string_to_key(&self.key_thrust).unwrap_or(d.thrust),
            fire: string_to_key(&self.key_fire).unwrap_or(d.fire),
            shield: string_to_key(&self.key_shield).unwrap_or(d.shield),
            bomb: string_to_key(&self.key_bomb).unwrap_or(d.bomb),
        }
    }

    pub fn set_bindings(&mut self, b: &Bindings) {
        self.key_left = key_to_string(&b.left);
        self.key_right = key_to_string(&b.right);
        self.key_thrust = key_to_string(&b.thrust);
        self.key_fire = key_to_string(&b.fire);
        self.key_shield = key_to_string(&b.shield);
        self.key_bomb = key_to_string(&b.bomb);
    }
}

/// Stable string form for a bindable key. Single characters are the
/// character itself; named keys use a `@` prefix so `@Tab` can never
/// collide with a future layout's literal characters.
pub fn key_to_string(key: &Key) -> String {
    match key {
        Key::Char(' ') => "@Space".into(),
        Key::Char(c) => c.to_string(),
        Key::ArrowLeft => "@ArrowLeft".into(),
        Key::ArrowRight => "@ArrowRight".into(),
        Key::ArrowUp => "@ArrowUp".into(),
        Key::ArrowDown => "@ArrowDown".into(),
        Key::Tab => "@Tab".into(),
        Key::Backspace => "@Backspace".into(),
        Key::Delete => "@Delete".into(),
        Key::Insert => "@Insert".into(),
        Key::Home => "@Home".into(),
        Key::End => "@End".into(),
        Key::PageUp => "@PageUp".into(),
        Key::PageDown => "@PageDown".into(),
        Key::Enter => "@Enter".into(),
        Key::Escape => "@Escape".into(),
        Key::Other(s) => format!("@Other:{s}"),
    }
}

pub fn string_to_key(s: &str) -> Option<Key> {
    if let Some(name) = s.strip_prefix('@') {
        return Some(match name {
            "Space" => Key::Char(' '),
            "ArrowLeft" => Key::ArrowLeft,
            "ArrowRight" => Key::ArrowRight,
            "ArrowUp" => Key::ArrowUp,
            "ArrowDown" => Key::ArrowDown,
            "Tab" => Key::Tab,
            "Backspace" => Key::Backspace,
            "Delete" => Key::Delete,
            "Insert" => Key::Insert,
            "Home" => Key::Home,
            "End" => Key::End,
            "PageUp" => Key::PageUp,
            "PageDown" => Key::PageDown,
            "Enter" => Key::Enter,
            "Escape" => Key::Escape,
            other => {
                let name = other.strip_prefix("Other:")?;
                if name.is_empty() {
                    return None;
                }
                Key::Other(name.to_string())
            }
        });
    }
    let mut chars = s.chars();
    let c = chars.next()?;
    chars.next().is_none().then_some(Key::Char(c))
}

impl crate::game::Game {
    /// Attach the platform's settings store and apply what it holds
    /// (`LoadConfig` at startup; absent/corrupt data keeps defaults).
    pub fn set_settings_store(&mut self, store: Box<dyn SettingsStore>) {
        if let Some(s) = store.load().as_deref().and_then(Settings::from_json) {
            self.menu.bindings = s.bindings();
            self.menu.start_level = s.start_level.min(crate::menu::MAX_START_LEVEL);
            self.menu.master_volume = s.master_volume.clamp(0.0, 1.0);
            self.menu.music_volume = s.music_volume.clamp(0.0, 1.0);
            self.music_on = s.music_on;
            self.sfx_on = s.sfx_on;
            // The table is always exactly five entries, sorted.
            let mut scores = s.high_scores;
            scores.resize(5, ("EMPTY".to_string(), 0));
            scores.truncate(5);
            scores.sort_by_key(|e| std::cmp::Reverse(e.1));
            self.menu.high_scores = scores;
        }
        self.settings_store = Some(store);
    }

    /// `SaveConfig` — write the current state through the store.
    pub(crate) fn save_settings(&mut self) {
        let Some(store) = self.settings_store.as_deref() else {
            return;
        };
        let mut s = Settings::default();
        s.set_bindings(&self.menu.bindings);
        s.start_level = self.menu.start_level;
        s.master_volume = self.menu.master_volume;
        s.music_volume = self.menu.music_volume;
        s.music_on = self.music_on;
        s.sfx_on = self.sfx_on;
        s.high_scores = self.menu.high_scores.clone();
        store.save(&s.to_json());
    }

    /// Chrome-bar toggles — persisted like every other setting.
    pub fn toggle_music(&mut self) {
        self.music_on = !self.music_on;
        self.save_settings();
    }

    pub fn toggle_sfx(&mut self) {
        self.sfx_on = !self.sfx_on;
        self.save_settings();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Binding;

    #[test]
    fn settings_roundtrip_through_json() {
        let mut s = Settings::default();
        let mut b = s.bindings();
        b.assign(Binding::Fire, Key::Char('q'));
        b.assign(Binding::Shield, Key::ArrowDown);
        s.set_bindings(&b);
        s.start_level = 7;
        s.master_volume = 0.5;
        s.music_on = false;

        let back = Settings::from_json(&s.to_json()).unwrap();
        assert_eq!(back, s);
        let rb = back.bindings();
        assert_eq!(rb.lookup(&Key::Char('q')), Some(Binding::Fire));
        assert_eq!(rb.lookup(&Key::ArrowDown), Some(Binding::Shield));
    }

    #[test]
    fn corrupt_json_falls_back_to_defaults() {
        assert!(Settings::from_json("not json").is_none());
        // Unknown keys in stored bindings fall back per-slot.
        let s = Settings {
            key_fire: "@Other:".into(), // malformed
            ..Default::default()
        };
        assert_eq!(s.bindings().fire, Bindings::default().fire);
        // Partial JSON fills the rest with defaults (serde(default)).
        let partial = Settings::from_json(r#"{"start_level": 3}"#).unwrap();
        assert_eq!(partial.start_level, 3);
        assert!(partial.music_on);
        assert_eq!(partial.master_volume, 1.0);
    }

    #[test]
    fn every_bindable_key_roundtrips() {
        for key in [
            Key::Char('z'),
            Key::Char(' '),
            Key::Char('/'),
            Key::ArrowUp,
            Key::Tab,
            Key::PageDown,
            Key::Other("F5".into()),
        ] {
            assert_eq!(string_to_key(&key_to_string(&key)), Some(key));
        }
    }
}
