//! # Key bindings — who does what on the keyboard
//!
//! The original's shipped `Astro.cfg` defaults (StartScreen.cpp
//! `LoadConfig` fallback): Z/X rotate, N thrust, Space shield, M fire,
//! H bomb. Keys were remappable, so the classic alternates many hands
//! remember (arrows, `/` thrust, Shift fire, S shield, B bomb) are
//! bound too, until the Phase 8 key-config screen brings real
//! remapping backed by the settings store.

use agg_gui::event::Key;

/// The game actions a key can map to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding {
    Left,
    Right,
    Thrust,
    Fire,
    Shield,
    Bomb,
    Start,
    /// Escape — menu navigation / in-game options.
    Menu,
}

/// Held-key state, updated from KeyDown/KeyUp deliveries.
#[derive(Default, Clone, Copy)]
pub struct KeysHeld {
    pub left: bool,
    pub right: bool,
    pub thrust: bool,
    pub shield: bool,
    pub fire: bool,
    pub bomb: bool,
}

/// The action bound to `key`, if any.
pub fn binding(key: &Key) -> Option<Binding> {
    Some(match key {
        Key::Char('z') | Key::Char('Z') | Key::ArrowLeft => Binding::Left,
        Key::Char('x') | Key::Char('X') | Key::ArrowRight => Binding::Right,
        Key::Char('n') | Key::Char('N') | Key::Char('/') | Key::Char('?') | Key::ArrowUp => {
            Binding::Thrust
        }
        Key::Char('m') | Key::Char('M') => Binding::Fire,
        Key::Other(name) if name == "Shift" => Binding::Fire,
        Key::Char(' ') | Key::Char('s') | Key::Char('S') => Binding::Shield,
        Key::Char('h') | Key::Char('H') | Key::Char('b') | Key::Char('B') => Binding::Bomb,
        Key::Enter => Binding::Start,
        Key::Escape => Binding::Menu,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_match_the_shipped_defaults() {
        // Z/X rotate, N and / thrust, M and Shift fire, Space shield,
        // H bomb (Astro.cfg defaults + classic alternates).
        assert_eq!(binding(&Key::Char('z')), Some(Binding::Left));
        assert_eq!(binding(&Key::ArrowLeft), Some(Binding::Left));
        assert_eq!(binding(&Key::Char('x')), Some(Binding::Right));
        assert_eq!(binding(&Key::Char('n')), Some(Binding::Thrust));
        assert_eq!(binding(&Key::Char('/')), Some(Binding::Thrust));
        assert_eq!(binding(&Key::ArrowUp), Some(Binding::Thrust));
        assert_eq!(binding(&Key::Char('m')), Some(Binding::Fire));
        assert_eq!(binding(&Key::Other("Shift".into())), Some(Binding::Fire));
        assert_eq!(binding(&Key::Char(' ')), Some(Binding::Shield));
        assert_eq!(binding(&Key::Char('s')), Some(Binding::Shield));
        assert_eq!(binding(&Key::Char('h')), Some(Binding::Bomb));
        assert_eq!(binding(&Key::Char('b')), Some(Binding::Bomb));
        assert_eq!(binding(&Key::Enter), Some(Binding::Start));
        assert_eq!(binding(&Key::Escape), Some(Binding::Menu));
    }
}
