//! # The start screen — port of `StartScreen.cpp` (Phase 8, part 1)
//!
//! The original bitmap menu on the `start.png` backdrop, presented
//! through THAT art's own palette (`LoadPalette(rStartBmp)`). Buttons
//! are the shipped two-state bitmaps at their 1997 coordinates and
//! fire on release, exactly like `ButtonCheck`.
//!
//! Ported this pass: the main screen (Start Game, Net Rock*, View
//! High*, Credits, Config, Help*, Quit, Demo), the config screen
//! (start-level picker; key/sound sub-screens still to come*), the
//! credits page, and the really-quit confirm. Starred items draw but
//! don't act yet — their systems (net, high scores, help text,
//! remapping, sliders) arrive with the rest of Phase 8.
//!
//! Modern departure by request: Esc during play opens this menu's
//! config screen with the game frozen behind it (the original's Esc
//! went to a quit-confirm; options only lived on the start screen).

use crate::assets;
use crate::events::{Events, GameEvent};
use crate::font::{Font, Justify};
use crate::frame::{BlitMode, Frame};
use crate::palette::Palette;

/// `#define VERSION_NUMBER "V:1.0.2"` (+ the port's tag).
const VERSION: &str = "V:1.0.2 RUST";
/// Start-level picker bounds — the original gated the top by
/// `HighestLevelReached` from Astro.cfg; until the settings store
/// lands, the cap is the config table size.
const MAX_START_LEVEL: u32 = 49;

/// Which page is showing (`ScreenState`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Main,
    Config,
    Credits,
    ReallyQuit,
}

/// What a click asks the game to do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuAction {
    StartGame {
        level: u32,
    },
    PlayDemo,
    Quit,
    /// Close the menu and unfreeze the game (Esc-from-game flow).
    ResumeGame,
}

struct Button {
    up: Frame,
    down: Frame,
    x: i32,
    y: i32,
    id: ButtonId,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ButtonId {
    StartGame,
    NetRock,
    ViewHigh,
    Credits,
    Config,
    Help,
    Quit,
    Demo,
    Done,
    LevelDown,
    LevelUp,
    CfgKeys,
    CfgSound,
    QuitYes,
    QuitNo,
}

impl Button {
    fn new(art: [&[u8]; 2], x: i32, y: i32, id: ButtonId) -> Self {
        Self {
            up: assets::frame_from_indexed_png(art[0]),
            down: assets::frame_from_indexed_png(art[1]),
            x,
            y,
            id,
        }
    }

    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.up.width && y >= self.y && y < self.y + self.up.height
    }

    fn draw(&self, screen: &mut Frame, pressed: bool) {
        let art = if pressed { &self.down } else { &self.up };
        screen.blit(art, &art.bounds(), self.x, self.y, BlitMode::Transparent0);
    }
}

pub struct Menu {
    backdrop: Frame,
    pub palette: Palette,
    reallyq: Frame,
    font: Font,
    buttons: Vec<Button>,
    page: Page,
    /// Buttons fire on release over the same button (`ButtonCheck`).
    pressed: Option<ButtonId>,
    cursor: (i32, i32),
    /// `GlobalStartLevel` (persisted later with the settings store).
    pub start_level: u32,
    /// Set when Esc opened the menu mid-game: Done/Esc resumes.
    pub from_game: bool,
}

impl Menu {
    pub fn new() -> Self {
        // The shipped coordinates from StartScreenInit.
        let buttons = vec![
            Button::new(assets::STRGM_PNG, 245, 232, ButtonId::StartGame),
            Button::new(assets::NETR_PNG, 136, 272, ButtonId::NetRock),
            Button::new(assets::VHIGH_PNG, 136, 312, ButtonId::ViewHigh),
            Button::new(assets::CRED_PNG, 136, 352, ButtonId::Credits),
            Button::new(assets::CONFIG_PNG, 344, 272, ButtonId::Config),
            Button::new(assets::HELP_PNG, 344, 312, ButtonId::Help),
            Button::new(assets::QUIT_PNG, 344, 352, ButtonId::Quit),
            Button::new(assets::DEMO_PNG, 295, 390, ButtonId::Demo),
            Button::new(assets::DONE_PNG, 245, 370, ButtonId::Done),
            Button::new(assets::BUTTONL_PNG, 350, 254, ButtonId::LevelDown),
            Button::new(assets::BUTTONR_PNG, 400, 254, ButtonId::LevelUp),
            Button::new(assets::CFGKEYS_PNG, 245, 284, ButtonId::CfgKeys),
            Button::new(assets::CFGSND_PNG, 245, 324, ButtonId::CfgSound),
            Button::new(assets::YES_PNG, 136, 310, ButtonId::QuitYes),
            Button::new(assets::NO_PNG, 344, 310, ButtonId::QuitNo),
        ];
        Self {
            backdrop: assets::frame_from_indexed_png(assets::START_PNG),
            palette: assets::palette_from_indexed_png(assets::START_PNG),
            reallyq: assets::frame_from_indexed_png(assets::REALLYQ_PNG),
            font: Font::astro(),
            buttons,
            page: Page::Main,
            pressed: None,
            cursor: (-1, -1),
            start_level: 0,
            from_game: false,
        }
    }

    /// Which buttons live on the current page.
    fn page_buttons(&self) -> &'static [ButtonId] {
        match self.page {
            Page::Main => &[
                ButtonId::StartGame,
                ButtonId::NetRock,
                ButtonId::ViewHigh,
                ButtonId::Credits,
                ButtonId::Config,
                ButtonId::Help,
                ButtonId::Quit,
                ButtonId::Demo,
            ],
            Page::Config => &[
                ButtonId::LevelDown,
                ButtonId::LevelUp,
                ButtonId::CfgKeys,
                ButtonId::CfgSound,
                ButtonId::Done,
            ],
            Page::Credits => &[ButtonId::Done],
            Page::ReallyQuit => &[ButtonId::QuitYes, ButtonId::QuitNo],
        }
    }

    /// Boot / return-to-menu entry (`ScreenState = STATE_MAIN`).
    pub fn show_main(&mut self) {
        self.page = Page::Main;
        self.from_game = false;
        self.pressed = None;
    }

    /// Enter on the main page starts a game (`KeyArray[SC_RETURN]`
    /// in `STATE_MAIN`).
    pub fn enter_starts(&self) -> bool {
        self.page == Page::Main && !self.from_game
    }

    /// The Esc-from-game flow: straight to the config page.
    pub fn show_options_from_game(&mut self) {
        self.page = Page::Config;
        self.from_game = true;
        self.pressed = None;
    }

    pub fn on_mouse_move(&mut self, x: i32, y: i32) {
        self.cursor = (x, y);
    }

    pub fn on_mouse_down(&mut self, x: i32, y: i32) {
        self.cursor = (x, y);
        self.pressed = self.page_buttons().iter().find_map(|&id| {
            let b = self.button(id);
            b.contains(x, y).then_some(id)
        });
    }

    pub fn on_mouse_up(&mut self, x: i32, y: i32, events: &mut Events) -> Option<MenuAction> {
        self.cursor = (x, y);
        let pressed = self.pressed.take()?;
        if !self.button(pressed).contains(x, y) {
            return None; // released elsewhere — no fire (ButtonCheck)
        }
        events.push(GameEvent::SfxClicked);
        self.activate(pressed)
    }

    /// Esc inside the menu: main -> quit confirm, sub-pages -> back,
    /// Esc-opened-from-game -> resume.
    pub fn on_escape(&mut self) -> Option<MenuAction> {
        if self.from_game {
            return Some(MenuAction::ResumeGame);
        }
        match self.page {
            Page::Main => {
                self.page = Page::ReallyQuit;
                None
            }
            Page::ReallyQuit => {
                self.page = Page::Main;
                None
            }
            _ => {
                self.page = Page::Main;
                None
            }
        }
    }

    fn activate(&mut self, id: ButtonId) -> Option<MenuAction> {
        match id {
            ButtonId::StartGame => Some(MenuAction::StartGame {
                level: self.start_level,
            }),
            ButtonId::Demo => Some(MenuAction::PlayDemo),
            ButtonId::Config => {
                self.page = Page::Config;
                None
            }
            ButtonId::Credits => {
                self.page = Page::Credits;
                None
            }
            ButtonId::Quit => {
                self.page = Page::ReallyQuit;
                None
            }
            ButtonId::QuitYes => Some(MenuAction::Quit),
            ButtonId::QuitNo => {
                self.page = Page::Main;
                None
            }
            ButtonId::Done => {
                if self.from_game && self.page == Page::Config {
                    Some(MenuAction::ResumeGame)
                } else {
                    self.page = Page::Main;
                    None
                }
            }
            ButtonId::LevelDown => {
                self.start_level = self.start_level.saturating_sub(1);
                None
            }
            ButtonId::LevelUp => {
                self.start_level = (self.start_level + 1).min(MAX_START_LEVEL);
                None
            }
            // Systems still to come: net play, high scores, help
            // pages, key remapping, sound sliders.
            ButtonId::NetRock
            | ButtonId::ViewHigh
            | ButtonId::Help
            | ButtonId::CfgKeys
            | ButtonId::CfgSound => None,
        }
    }

    fn button(&self, id: ButtonId) -> &Button {
        self.buttons.iter().find(|b| b.id == id).expect("button")
    }

    /// Draw the whole page into the 640x480 screen.
    pub fn draw(&self, screen: &mut Frame) {
        screen.blit(
            &self.backdrop,
            &self.backdrop.bounds(),
            0,
            0,
            BlitMode::Normal,
        );

        match self.page {
            Page::Main => {}
            Page::Config => {
                // "Start Level   %d" at (STARTLEVELLEFT, START_LEVEL_TOP).
                let text = format!("Start Level   {}", self.start_level);
                self.font.print(
                    screen,
                    230,
                    254,
                    &text,
                    Justify::Left,
                    BlitMode::Transparent0,
                );
            }
            Page::Credits => {
                let lines = [
                    "Design & Programming:   Lars Brubaker ",
                    "         Art & Music:   Chad Max      ",
                    "  Explosions & Rocks:   Tony Bowren   ",
                    "        Lots of Help:   Scott Campbell",
                    "                 DOS:   Bill Heineman ",
                    "  Businesslike Stuff:   Steven Parsons",
                ];
                self.font.print(
                    screen,
                    320,
                    245 - 17,
                    VERSION,
                    Justify::Center,
                    BlitMode::Transparent0,
                );
                for (i, line) in lines.iter().enumerate() {
                    self.font.print(
                        screen,
                        320,
                        i as i32 * 17 + 245,
                        line,
                        Justify::Center,
                        BlitMode::Transparent0,
                    );
                }
            }
            Page::ReallyQuit => {
                screen.blit(
                    &self.reallyq,
                    &self.reallyq.bounds(),
                    320 - self.reallyq.width / 2,
                    240 - self.reallyq.height / 2,
                    BlitMode::Transparent0,
                );
            }
        }

        let (cx, cy) = self.cursor;
        for &id in self.page_buttons() {
            let b = self.button(id);
            let down = self.pressed == Some(id) && b.contains(cx, cy);
            b.draw(screen, down);
        }
    }
}

impl Default for Menu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_fires_on_release_over_the_same_button() {
        let mut menu = Menu::new();
        let mut ev = Events::new();
        // Start Game button lives at (245, 232).
        menu.on_mouse_down(250, 240);
        assert!(menu.pressed.is_some());
        // Release elsewhere: no action (ButtonCheck semantics).
        assert_eq!(menu.on_mouse_up(10, 10, &mut ev), None);

        menu.on_mouse_down(250, 240);
        let action = menu.on_mouse_up(251, 241, &mut ev);
        assert_eq!(action, Some(MenuAction::StartGame { level: 0 }));
        assert!(ev.drain().next().is_some(), "click sound queued");
    }

    #[test]
    fn config_page_picks_the_start_level() {
        let mut menu = Menu::new();
        let mut ev = Events::new();
        menu.on_mouse_down(350, 258); // config button? no — navigate first
        menu.page = Page::Config;
        menu.pressed = None;

        // LevelUp at (400, 254).
        menu.on_mouse_down(402, 258);
        menu.on_mouse_up(402, 258, &mut ev);
        assert_eq!(menu.start_level, 1);
        // LevelDown at (350, 254).
        menu.on_mouse_down(352, 258);
        menu.on_mouse_up(352, 258, &mut ev);
        assert_eq!(menu.start_level, 0);
        menu.on_mouse_down(352, 258);
        menu.on_mouse_up(352, 258, &mut ev);
        assert_eq!(menu.start_level, 0, "clamped at zero");
    }

    #[test]
    fn escape_walks_quit_confirm_and_resume() {
        let mut menu = Menu::new();
        assert_eq!(menu.on_escape(), None);
        assert_eq!(menu.page, Page::ReallyQuit);
        assert_eq!(menu.on_escape(), None);
        assert_eq!(menu.page, Page::Main);

        menu.show_options_from_game();
        assert_eq!(menu.on_escape(), Some(MenuAction::ResumeGame));
    }

    #[test]
    fn start_palette_differs_from_game_palette() {
        let menu = Menu::new();
        let game = crate::assets::game_palette();
        assert_ne!(
            menu.palette.rgb.to_vec(),
            game.rgb.to_vec(),
            "start.png should carry its own palette"
        );
    }
}
