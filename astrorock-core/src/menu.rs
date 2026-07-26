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

use std::rc::Rc;

use crate::assets;
use crate::events::{Events, GameEvent};
use crate::font::{Font, Justify};
use crate::frame::{BlitMode, Frame};
use crate::palette::{FadeBlits, Palette};
use crate::rand::Rand;
use crate::sequence::{self, FrameSequence};

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
    /// Abandon the running game (`STATE_REALLYENDGAME` Yes): the
    /// GAME OVER overlay plays out, then back to the menu.
    EndGame,
}

/// The bad-guy showcase monitor (`SwitchBadGuy`/`UpdateBadGuyScreen`/
/// `DrawBadGuyScreen`): a random subject from 15 — the ship, five
/// enemies, a rock, and the eight goodies — switches every
/// `SWITCHBADGUYPAUSE` beats behind nine frames of TV static and
/// fades in through the `FadeBlit` tables.
struct Showcase {
    seqs: Vec<Rc<FrameSequence>>,
    statics: [Frame; 3],
    cur: usize,
    pause: i32,
    cur_frame: f32,
    spike_dir: f32,
    cur_fade: usize,
}

/// `#define SWITCHBADGUYPAUSE 150`
const SWITCH_PAUSE: i32 = 150;
/// `#define NUMSTATICUPDATES 9`
const NUM_STATIC_UPDATES: i32 = 9;
/// `BADGUYX/Y` — direct screen coordinates.
const BADGUY_X: i32 = 320;
const BADGUY_Y: i32 = 150;
/// Spikeballs are showcase type 2 (they get the roll-walk dance).
const TYPE_SPIKEBALL: usize = 2;

impl Showcase {
    fn new() -> Self {
        // `SwitchBadGuy` type order, goodies in `GetType` order.
        let seqs: Vec<Rc<FrameSequence>> = vec![
            sequence::ship(),
            sequence::gloop(),
            sequence::spkball(),
            sequence::bomber(),
            sequence::hk(),
            sequence::fastdeth(),
            sequence::ast_big(),
            sequence::health(),
            sequence::pows(),
            sequence::rapid(),
            sequence::spred(),
            sequence::bombg(),
            sequence::gun01(),
            sequence::gun02(),
            sequence::one_up(),
        ];
        Self {
            seqs,
            statics: [
                assets::frame_from_indexed_png(assets::STATIC1_PNG),
                assets::frame_from_indexed_png(assets::STATIC2_PNG),
                assets::frame_from_indexed_png(assets::STATIC3_PNG),
            ],
            cur: 0,
            // Starts at the threshold: the first beat switches.
            pause: SWITCH_PAUSE,
            cur_frame: 0.0,
            spike_dir: 1.0,
            cur_fade: 0,
        }
    }

    /// One menu beat (`UpdateBadGuyScreen` + the switch check).
    fn update(&mut self, local_rand: &mut Rand, events: &mut Events) {
        if self.pause >= SWITCH_PAUSE {
            let n = self.seqs.len() as u32;
            let mut next = local_rand.rand(n) as usize;
            while next == self.cur {
                next = local_rand.rand(n) as usize;
            }
            self.cur = next;
            self.pause = 0;
            // `curframe` is a persistent static — it keeps counting
            // across switches (no reset in SwitchBadGuy).
        }

        if self.cur == TYPE_SPIKEBALL {
            // The roll-walk dance (LocalRand — visual only).
            self.cur_frame += self.spike_dir;
            if self.cur_frame > 60.0 {
                self.cur_frame = 0.0;
            }
            if self.cur_frame < 0.0 {
                self.cur_frame = 60.0;
            }
            if (self.cur_frame as i32) % 20 == 0 {
                self.cur_frame = (local_rand.rand(4) * 20) as f32;
                if local_rand.rand(2) != 0 {
                    self.spike_dir = -1.0;
                    if self.cur_frame == 0.0 {
                        self.cur_frame = 60.0;
                    }
                } else {
                    self.spike_dir = 1.0;
                    if self.cur_frame == 60.0 {
                        self.cur_frame = 0.0;
                    }
                }
            }
        } else {
            self.cur_frame += 1.0;
        }

        if self.pause == 0 {
            events.push(GameEvent::SfxStatic);
        }

        // Fade the fresh subject in from black behind the static.
        if self.pause >= NUM_STATIC_UPDATES - 1
            && self.pause < crate::palette::NUM_FADES as i32 + NUM_STATIC_UPDATES
        {
            let fade = crate::palette::NUM_FADES as i32 + NUM_STATIC_UPDATES - self.pause;
            self.cur_fade = (fade.min(crate::palette::NUM_FADES as i32 - 1)) as usize;
        } else {
            self.cur_fade = 0;
        }
        self.pause += 1;
    }

    /// `DrawBadGuyScreen`.
    fn draw(&self, screen: &mut Frame, fades: &FadeBlits) {
        let seq = &self.seqs[self.cur];
        let index = (self.cur_frame as u32 % seq.num_frames) as usize;
        let art = &seq.frames[index];
        let blit = fades.blit(self.cur_fade);
        screen.blit(
            art,
            &art.bounds(),
            BADGUY_X - art.hot_x,
            BADGUY_Y - art.hot_y,
            blit.to_mode(),
        );

        if self.pause < NUM_STATIC_UPDATES {
            let st = &self.statics[(self.pause % 3) as usize];
            screen.blit(st, &st.bounds(), 225, 118, BlitMode::Transparent0);
        }
    }
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

    fn contains_at(&self, at: (i32, i32), x: i32, y: i32) -> bool {
        x >= at.0 && x < at.0 + self.up.width && y >= at.1 && y < at.1 + self.up.height
    }

    fn draw_at(&self, screen: &mut Frame, at: (i32, i32), pressed: bool) {
        let art = if pressed { &self.down } else { &self.up };
        screen.blit(art, &art.bounds(), at.0, at.1, BlitMode::Transparent0);
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
    /// The bad-guy showcase monitor in the backdrop's top screen.
    showcase: Showcase,
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
            showcase: Showcase::new(),
        }
    }

    /// One 30 Hz menu beat (`StartScreenUpdate(isgameupdate=1)`): the
    /// showcase monitor animates on every page. (The original skips
    /// the SUBJECT SWITCH during `STATE_HELP` because help pages pick
    /// their subject manually — that gate arrives with the help
    /// pages.)
    pub fn beat(&mut self, local_rand: &mut Rand, events: &mut Events) {
        self.showcase.update(local_rand, events);
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
            // Opened mid-game, the config page grows a Quit button —
            // the way back to the original's `STATE_REALLYENDGAME`
            // "end this game?" confirm.
            Page::Config if self.from_game => &[
                ButtonId::LevelDown,
                ButtonId::LevelUp,
                ButtonId::CfgKeys,
                ButtonId::CfgSound,
                ButtonId::Done,
                ButtonId::Quit,
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
            b.contains_at(self.button_pos(id), x, y).then_some(id)
        });
    }

    pub fn on_mouse_up(&mut self, x: i32, y: i32, events: &mut Events) -> Option<MenuAction> {
        self.cursor = (x, y);
        let pressed = self.pressed.take()?;
        if !self
            .button(pressed)
            .contains_at(self.button_pos(pressed), x, y)
        {
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
            // From a running game the confirm ends THE GAME, not the
            // app (`STATE_REALLYENDGAME`: Y -> GAME OVER, N -> play).
            ButtonId::QuitYes if self.from_game => Some(MenuAction::EndGame),
            ButtonId::QuitYes => Some(MenuAction::Quit),
            ButtonId::QuitNo if self.from_game => Some(MenuAction::ResumeGame),
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

    /// Where a button sits on the CURRENT page. The 1997 coordinates
    /// are the default; the from-game config page (which has no 1997
    /// counterpart) rearranges its bottom row so Quit doesn't overlap
    /// Done — side by side in the main page's two columns.
    fn button_pos(&self, id: ButtonId) -> (i32, i32) {
        if self.from_game && self.page == Page::Config {
            match id {
                ButtonId::Done => return (136, 370),
                ButtonId::Quit => return (344, 370),
                _ => {}
            }
        }
        let b = self.button(id);
        (b.x, b.y)
    }

    /// Draw the whole page into the 640x480 screen. `fades` is the
    /// game-palette `FadeBlit` table — the original built it once at
    /// init and the start screen used it as-is.
    pub fn draw(&self, screen: &mut Frame, fades: &FadeBlits) {
        screen.blit(
            &self.backdrop,
            &self.backdrop.bounds(),
            0,
            0,
            BlitMode::Normal,
        );

        // `DrawBadGuyScreen` — right after the backdrop, every page.
        self.showcase.draw(screen, fades);

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
            let at = self.button_pos(id);
            let down = self.pressed == Some(id) && b.contains_at(at, cx, cy);
            b.draw_at(screen, at, down);
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
    fn showcase_switches_subjects_behind_static() {
        let mut menu = Menu::new();
        let mut rand = Rand::new();
        let mut ev = Events::new();

        // First beat: the pause starts at the threshold, so a subject
        // is picked (never the current one) and the static burst
        // begins with its sound.
        menu.beat(&mut rand, &mut ev);
        let first = menu.showcase.cur;
        assert_ne!(first, 0, "first pick must differ from the initial 0");
        assert!(ev.drain().any(|e| e == GameEvent::SfxStatic));
        assert_eq!(menu.showcase.pause, 1);

        // Static frames cover updates 0..NUMSTATICUPDATES; the fade
        // ramp runs while they clear, then settles at full (0).
        for _ in 0..NUM_STATIC_UPDATES + 3 {
            menu.beat(&mut rand, &mut ev);
        }
        assert!(menu.showcase.cur_fade > 0, "fade-in should be running");
        for _ in 0..crate::palette::NUM_FADES as i32 {
            menu.beat(&mut rand, &mut ev);
        }
        assert_eq!(menu.showcase.cur_fade, 0, "fade completes");

        // At SWITCHBADGUYPAUSE the subject changes again.
        while menu.showcase.pause < SWITCH_PAUSE {
            menu.beat(&mut rand, &mut ev);
        }
        menu.beat(&mut rand, &mut ev);
        assert_ne!(menu.showcase.cur, first, "subject never repeats");
    }

    #[test]
    fn from_game_quit_confirm_ends_the_game() {
        let mut menu = Menu::new();
        let mut ev = Events::new();
        menu.show_options_from_game();
        // The from-game config page grows a Quit button, rearranged
        // beside Done so the two 162-wide arts can't overlap.
        assert!(menu.page_buttons().contains(&ButtonId::Quit));
        let done = menu.button_pos(ButtonId::Done);
        let quit = menu.button_pos(ButtonId::Quit);
        assert!(
            done.0 + 162 <= quit.0 || quit.0 + 162 <= done.0 || (done.1 - quit.1).abs() >= 25,
            "Done {done:?} and Quit {quit:?} overlap"
        );

        menu.on_mouse_down(quit.0 + 5, quit.1 + 5);
        assert_eq!(menu.on_mouse_up(quit.0 + 5, quit.1 + 5, &mut ev), None);
        assert_eq!(menu.page, Page::ReallyQuit);

        // No resumes play; Yes abandons the game to GAME OVER.
        menu.on_mouse_down(350, 315); // QuitNo at (344, 310)
        assert_eq!(
            menu.on_mouse_up(350, 315, &mut ev),
            Some(MenuAction::ResumeGame)
        );
        menu.page = Page::ReallyQuit;
        menu.on_mouse_down(140, 315); // QuitYes at (136, 310)
        assert_eq!(
            menu.on_mouse_up(140, 315, &mut ev),
            Some(MenuAction::EndGame)
        );
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
