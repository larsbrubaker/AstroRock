//! # Config Controls + Config Sound pages
//!
//! The `STATE_CONFIG_KEYS`, `STATE_GETAKEY`, and `STATE_CONFIG_SOUND`
//! halves of `StartScreen.cpp`: the six key-row buttons with their
//! `PrintKeyUsed` labels, the "press the key" prompt, and the two
//! `DragButton` volume sliders on the groove art.

use crate::font::Justify;
use crate::frame::{BlitMode, Frame};
use crate::input::Binding;
use crate::menu::Menu;

/// `#define CONFIGTOP 242` / `CONFIGSKIP 30`.
const CONFIG_TOP: i32 = 242;
const CONFIG_SKIP: i32 = 30;
/// `CFIG_KEY_LEFTCOL 136` / `CFIG_KEY_RIGHTCOL 344`, labels at +90.
const KEY_LEFT_COL: i32 = 136;
const KEY_RIGHT_COL: i32 = 344;
/// `SLIDERMUSICTOP 236` / `SLIDERMASTERTOP 276`.
const SLIDER_MUSIC_TOP: i32 = 236;
const SLIDER_MASTER_TOP: i32 = 276;
/// The `STATE_GETAKEY` prompt line (`3 * HIGHSCORESKIP +
/// HIGHSCORETOP` = 3 * 24 + 250).
const GET_KEY_PROMPT_Y: i32 = 322;

/// Which slider a drag owns (`DragMasterVolume`/`DragMusicVolume`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SliderId {
    Master,
    Music,
}

impl SliderId {
    fn top(self) -> i32 {
        match self {
            SliderId::Master => SLIDER_MASTER_TOP,
            SliderId::Music => SLIDER_MUSIC_TOP,
        }
    }
}

impl Menu {
    /// `SLIDERLEFT = pScreen->Width/2 - GroovFrame.Width/2`.
    fn slider_left(&self) -> i32 {
        320 - self.grove.width / 2
    }

    /// `SLIDERACTIVEWIDTH` — the tab's travel range.
    fn slider_active(&self) -> i32 {
        self.grove.width - self.drag_tab.width
    }

    fn volume_of(&self, id: SliderId) -> f32 {
        match id {
            SliderId::Master => self.master_volume,
            SliderId::Music => self.music_volume,
        }
    }

    /// The tab's screen x for a slider's current volume (full volume
    /// parks at the right end, like `DragButtonMoveTo` at init).
    fn tab_x(&self, id: SliderId) -> i32 {
        self.slider_left() + (self.slider_active() as f32 * self.volume_of(id)) as i32
    }

    fn set_volume_from_x(&mut self, id: SliderId, x: i32) {
        let left = self.slider_left();
        let active = self.slider_active().max(1);
        let tab_x = (x - self.drag_tab.width / 2 - left).clamp(0, active);
        let f = tab_x as f32 / active as f32;
        match id {
            SliderId::Master => self.master_volume = f,
            SliderId::Music => self.music_volume = f,
        }
    }

    /// A press anywhere on a groove grabs that slider's tab
    /// (`DragButton` region check) and jumps it to the cursor.
    pub(crate) fn slider_mouse_down(&mut self, x: i32, y: i32) -> bool {
        if self.page != crate::menu::Page::ConfigSound {
            return false;
        }
        let left = self.slider_left();
        let right = left + self.grove.width;
        for id in [SliderId::Music, SliderId::Master] {
            let top = id.top();
            if x >= left && x < right && y >= top && y < top + self.drag_tab.height {
                self.dragging = Some(id);
                self.set_volume_from_x(id, x);
                return true;
            }
        }
        false
    }

    pub(crate) fn slider_mouse_move(&mut self, x: i32) {
        if let Some(id) = self.dragging {
            self.set_volume_from_x(id, x);
        }
    }

    /// Release ends the drag; the changed volume persists
    /// (`SaveConfig` runs via the dirty flag).
    pub(crate) fn slider_mouse_up(&mut self) -> bool {
        if self.dragging.take().is_some() {
            self.settings_dirty = true;
            true
        } else {
            false
        }
    }

    /// `STATE_CONFIG_KEYS` draw: `PrintKeyUsed` beside each row
    /// (buttons themselves come from the shared page loop).
    pub(crate) fn draw_config_keys(&self, screen: &mut Frame) {
        // Left column rows 1..3: left, fire, thrust; right column:
        // right, blade (bomb), shield — the original's exact order.
        let rows = [
            (KEY_LEFT_COL, 1, Binding::Left),
            (KEY_RIGHT_COL, 1, Binding::Right),
            (KEY_LEFT_COL, 2, Binding::Fire),
            (KEY_RIGHT_COL, 2, Binding::Bomb),
            (KEY_LEFT_COL, 3, Binding::Thrust),
            (KEY_RIGHT_COL, 3, Binding::Shield),
        ];
        for (col, row, action) in rows {
            self.font.print(
                screen,
                col + 90,
                row * CONFIG_SKIP + CONFIG_TOP + 6,
                &self.bindings.key_name(action),
                Justify::Left,
                BlitMode::Transparent0,
            );
        }
    }

    /// `STATE_GETAKEY` draw: the centered prompt (bomb is called
    /// "blade" — the weapon's real name).
    pub(crate) fn draw_get_key(&self, screen: &mut Frame, action: Binding) {
        let name = match action {
            Binding::Left => "left",
            Binding::Right => "right",
            Binding::Thrust => "thrust",
            Binding::Fire => "fire",
            Binding::Shield => "shield",
            Binding::Bomb => "blade",
            Binding::Start | Binding::Menu => unreachable!("not remappable"),
        };
        let text = format!("Press the key you want to use for {name}.");
        self.font.print(
            screen,
            320,
            GET_KEY_PROMPT_Y,
            &text,
            Justify::Center,
            BlitMode::Transparent0,
        );
    }

    /// `STATE_CONFIG_SOUND` draw: two labeled grooves with their drag
    /// tabs. (The 1997 stereo/mixing toggles were DirectSound knobs —
    /// nothing to configure on the modern mixer.)
    pub(crate) fn draw_config_sound(&self, screen: &mut Frame) {
        let half_tab = self.drag_tab.height / 2;
        for (id, label) in [
            (SliderId::Music, "Music Volume"),
            (SliderId::Master, "Master Volume"),
        ] {
            let top = id.top();
            screen.blit(
                &self.grove,
                &self.grove.bounds(),
                self.slider_left(),
                top + half_tab - self.grove.height / 2,
                BlitMode::Normal,
            );
            screen.blit(
                &self.drag_tab,
                &self.drag_tab.bounds(),
                self.tab_x(id),
                top,
                BlitMode::Transparent0,
            );
            self.font.print(
                screen,
                320,
                top + half_tab * 2,
                label,
                Justify::Center,
                BlitMode::Transparent0,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Events;
    use crate::menu::Page;
    use agg_gui::event::Key;

    #[test]
    fn config_keys_capture_binds_and_swaps() {
        let mut menu = Menu::new();
        let mut ev = Events::new();
        menu.page = Page::ConfigKeys;

        // Click the FIRE row (left column, row 2: 136, 302).
        menu.on_mouse_down(140, 306);
        menu.on_mouse_up(140, 306, &mut ev);
        assert_eq!(menu.page, Page::GetKey(Binding::Fire));

        // The next keypress binds; Z was left's key, so left inherits
        // fire's old M (CheckAndSwap).
        assert!(menu.capture_key(&Key::Char('z'), &mut ev));
        assert_eq!(menu.page, Page::ConfigKeys);
        assert!(menu.settings_dirty);
        assert_eq!(menu.bindings.lookup(&Key::Char('z')), Some(Binding::Fire));
        assert_eq!(menu.bindings.lookup(&Key::Char('m')), Some(Binding::Left));

        // Escape cancels a capture without touching bindings.
        menu.page = Page::GetKey(Binding::Shield);
        let before = menu.bindings.clone();
        assert!(menu.capture_key(&Key::Escape, &mut ev));
        assert_eq!(menu.page, Page::ConfigKeys);
        assert_eq!(menu.bindings, before);
    }

    #[test]
    fn sliders_drag_the_volumes() {
        let mut menu = Menu::new();
        menu.page = Page::ConfigSound;
        assert_eq!(menu.master_volume, 1.0);

        // Grab the master groove at its horizontal middle.
        let mid_x = 320;
        let master_y = SLIDER_MASTER_TOP + 2;
        assert!(menu.slider_mouse_down(mid_x, master_y));
        assert!(menu.dragging.is_some());
        assert!(
            (menu.master_volume - 0.5).abs() < 0.1,
            "middle of groove should be about half volume: {}",
            menu.master_volume
        );

        // Drag right to the end: full volume; music untouched.
        menu.slider_mouse_move(640);
        assert_eq!(menu.master_volume, 1.0);
        assert_eq!(menu.music_volume, 1.0);
        menu.slider_mouse_move(0);
        assert_eq!(menu.master_volume, 0.0);
        assert!(menu.slider_mouse_up());
        assert!(menu.settings_dirty);

        // Off the grooves: no grab.
        assert!(!menu.slider_mouse_down(320, 400));
    }
}
