//! # Key bindings — who does what on the keyboard
//!
//! The original's shipped `Astro.cfg` defaults (StartScreen.cpp
//! `LoadConfig` fallback): Z/X rotate, N thrust, Space shield, M fire,
//! H bomb — remappable through the Config Controls screen exactly like
//! `STATE_CONFIG_KEYS`/`STATE_GETAKEY`. On top of the six primaries, a
//! fixed layer of classic alternates (arrows, `/` thrust, Shift fire,
//! S shield, B bomb) stays live unless one of those keys has been
//! claimed as a primary for a different action.

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

/// The six remappable actions, in the original's config-screen order
/// (left column top-to-bottom, then right column).
pub const REMAPPABLE: [Binding; 6] = [
    Binding::Left,
    Binding::Fire,
    Binding::Thrust,
    Binding::Right,
    Binding::Bomb,
    Binding::Shield,
];

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

/// The classic alternates — active unless their key is claimed as a
/// primary for a DIFFERENT action (rebinding wins, like the original's
/// single `KeyArray` scan honoring only the `Global*Key` slots).
const ALTERNATES: [(&Key, Binding); 8] = [
    (&Key::ArrowLeft, Binding::Left),
    (&Key::ArrowRight, Binding::Right),
    (&Key::Char('/'), Binding::Thrust),
    (&Key::Char('?'), Binding::Thrust),
    (&Key::ArrowUp, Binding::Thrust),
    (&Key::Char('s'), Binding::Shield),
    (&Key::Char('b'), Binding::Bomb),
    (&Key::Char(' '), Binding::Shield),
];

/// Case-insensitive key equality (Char keys arrive shifted).
fn key_matches(a: &Key, b: &Key) -> bool {
    match (a, b) {
        (Key::Char(x), Key::Char(y)) => {
            x.eq_ignore_ascii_case(y)
        }
        _ => a == b,
    }
}

/// The remappable bindings: one primary key per action
/// (`GlobalLeftKey` .. `GlobalBombKey`).
#[derive(Clone, Debug, PartialEq)]
pub struct Bindings {
    pub left: Key,
    pub right: Key,
    pub thrust: Key,
    pub fire: Key,
    pub shield: Key,
    pub bomb: Key,
}

impl Default for Bindings {
    /// The shipped `Astro.cfg` defaults.
    fn default() -> Self {
        Self {
            left: Key::Char('z'),
            right: Key::Char('x'),
            thrust: Key::Char('n'),
            fire: Key::Char('m'),
            shield: Key::Char(' '),
            bomb: Key::Char('h'),
        }
    }
}

impl Bindings {
    pub fn key_for(&self, action: Binding) -> &Key {
        match action {
            Binding::Left => &self.left,
            Binding::Right => &self.right,
            Binding::Thrust => &self.thrust,
            Binding::Fire => &self.fire,
            Binding::Shield => &self.shield,
            Binding::Bomb => &self.bomb,
            Binding::Start | Binding::Menu => unreachable!("not remappable"),
        }
    }

    fn key_for_mut(&mut self, action: Binding) -> &mut Key {
        match action {
            Binding::Left => &mut self.left,
            Binding::Right => &mut self.right,
            Binding::Thrust => &mut self.thrust,
            Binding::Fire => &mut self.fire,
            Binding::Shield => &mut self.shield,
            Binding::Bomb => &mut self.bomb,
            Binding::Start | Binding::Menu => unreachable!("not remappable"),
        }
    }

    /// The action bound to `key`, if any. Primaries first; the
    /// alternate layer only answers when its key isn't a primary of
    /// some other action. Enter/Escape are hardwired, exactly like
    /// `SC_RETURN`/`SC_ESCAPE` in the original.
    pub fn lookup(&self, key: &Key) -> Option<Binding> {
        match key {
            Key::Enter => return Some(Binding::Start),
            Key::Escape => return Some(Binding::Menu),
            Key::Other(name) if name == "Shift" => return Some(Binding::Fire),
            _ => {}
        }
        for action in REMAPPABLE {
            if key_matches(self.key_for(action), key) {
                return Some(action);
            }
        }
        for (alt, action) in ALTERNATES {
            if key_matches(alt, key) && !REMAPPABLE.iter().any(|&other| {
                other != action && key_matches(self.key_for(other), key)
            }) {
                return Some(action);
            }
        }
        None
    }

    /// `STATE_GETAKEY` assignment with the `CheckAndSwap` rule: if the
    /// new key is already the primary of another action, that action
    /// inherits this action's old key — no dead slots, ever.
    pub fn assign(&mut self, action: Binding, key: Key) {
        let key = match key {
            Key::Char(c) => Key::Char(c.to_ascii_lowercase()),
            k => k,
        };
        let old = self.key_for(action).clone();
        for other in REMAPPABLE {
            if other != action && key_matches(self.key_for(other), &key) {
                *self.key_for_mut(other) = old.clone();
            }
        }
        *self.key_for_mut(action) = key;
    }

    /// The config screen's `PrintKeyUsed` label for an action's key.
    pub fn key_name(&self, action: Binding) -> String {
        let key = self.key_for(action);
        let name = match key {
            Key::Char(' ') => "SPACE".to_string(),
            Key::Char(c) => c.to_uppercase().to_string(),
            Key::ArrowLeft => "LEFT".into(),
            Key::ArrowRight => "RIGHT".into(),
            Key::ArrowUp => "UP".into(),
            Key::ArrowDown => "DOWN".into(),
            Key::Tab => "TAB".into(),
            Key::Backspace => "BKSP".into(),
            Key::Delete => "DEL".into(),
            Key::Insert => "INS".into(),
            Key::Home => "HOME".into(),
            Key::End => "END".into(),
            Key::PageUp => "PGUP".into(),
            Key::PageDown => "PGDN".into(),
            Key::Enter => "ENTER".into(),
            Key::Escape => "ESC".into(),
            Key::Other(s) => s.to_uppercase(),
        };
        format!("'{name}'")
    }

    /// A key is usable as a primary if the platform can deliver it
    /// and it isn't one of the hardwired system keys.
    pub fn assignable(key: &Key) -> bool {
        !matches!(key, Key::Enter | Key::Escape)
            && !matches!(key, Key::Other(name) if name == "Shift")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_match_the_shipped_defaults() {
        // Z/X rotate, N and / thrust, M and Shift fire, Space shield,
        // H bomb (Astro.cfg defaults + classic alternates).
        let b = Bindings::default();
        assert_eq!(b.lookup(&Key::Char('z')), Some(Binding::Left));
        assert_eq!(b.lookup(&Key::Char('Z')), Some(Binding::Left));
        assert_eq!(b.lookup(&Key::ArrowLeft), Some(Binding::Left));
        assert_eq!(b.lookup(&Key::Char('x')), Some(Binding::Right));
        assert_eq!(b.lookup(&Key::Char('n')), Some(Binding::Thrust));
        assert_eq!(b.lookup(&Key::Char('/')), Some(Binding::Thrust));
        assert_eq!(b.lookup(&Key::ArrowUp), Some(Binding::Thrust));
        assert_eq!(b.lookup(&Key::Char('m')), Some(Binding::Fire));
        assert_eq!(b.lookup(&Key::Other("Shift".into())), Some(Binding::Fire));
        assert_eq!(b.lookup(&Key::Char(' ')), Some(Binding::Shield));
        assert_eq!(b.lookup(&Key::Char('s')), Some(Binding::Shield));
        assert_eq!(b.lookup(&Key::Char('h')), Some(Binding::Bomb));
        assert_eq!(b.lookup(&Key::Char('b')), Some(Binding::Bomb));
        assert_eq!(b.lookup(&Key::Enter), Some(Binding::Start));
        assert_eq!(b.lookup(&Key::Escape), Some(Binding::Menu));
    }

    #[test]
    fn assign_swaps_like_check_and_swap() {
        let mut b = Bindings::default();
        // Bind fire to Z (left's key): left inherits fire's old M.
        b.assign(Binding::Fire, Key::Char('z'));
        assert_eq!(b.lookup(&Key::Char('z')), Some(Binding::Fire));
        assert_eq!(b.lookup(&Key::Char('m')), Some(Binding::Left));
        assert_eq!(b.key_name(Binding::Fire), "'Z'");
        assert_eq!(b.key_name(Binding::Left), "'M'");
    }

    #[test]
    fn rebinding_shadows_the_alternate_layer() {
        let mut b = Bindings::default();
        // Claim S (default shield alternate) as the fire primary: it
        // must now fire, not shield.
        b.assign(Binding::Fire, Key::Char('s'));
        assert_eq!(b.lookup(&Key::Char('s')), Some(Binding::Fire));
        // Other alternates stay live.
        assert_eq!(b.lookup(&Key::Char('/')), Some(Binding::Thrust));
        // Uppercase capture normalizes.
        b.assign(Binding::Bomb, Key::Char('Q'));
        assert_eq!(b.lookup(&Key::Char('q')), Some(Binding::Bomb));
        assert_eq!(b.key_name(Binding::Bomb), "'Q'");
    }

    #[test]
    fn hardwired_keys_are_not_assignable() {
        assert!(!Bindings::assignable(&Key::Enter));
        assert!(!Bindings::assignable(&Key::Escape));
        assert!(!Bindings::assignable(&Key::Other("Shift".into())));
        assert!(Bindings::assignable(&Key::Char('q')));
        assert!(Bindings::assignable(&Key::ArrowDown));
    }
}
