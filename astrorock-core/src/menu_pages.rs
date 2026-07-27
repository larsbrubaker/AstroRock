//! # Help, High Scores, and Net Rock pages
//!
//! The `STATE_HELP` / `STATE_HIGHSCORES` / `STATE_NEWHIGHSCORE`
//! halves of `StartScreen.cpp`, plus the modern Net Rock placeholder
//! (`STATE_NONETPLAY`'s cousin). Help subjects share the showcase
//! monitor's `SwitchBadGuy` order; the text is `ppHelpText1`
//! verbatim.

use agg_gui::event::Key;

use crate::events::Events;
use crate::font::Justify;
use crate::frame::{BlitMode, Frame};
use crate::menu::{Menu, Page};

/// `NUMBADGUYTYPES` — one help entry per showcase subject.
pub(crate) const NUM_HELP_SUBJECTS: usize = 15;
/// `NUMHELPLINES`.
const HELP_LINES: usize = 4;
/// `MAXNAMEWIDTH`.
const MAX_NAME: usize = 15;
/// `HIGHSCORETOP` / `HIGHSCORESKIP`.
const HIGH_TOP: i32 = 250;
const HIGH_SKIP: i32 = 24;

/// The arcade-entry wheel: letters, digits, punctuation, and `<`
/// (rub out). FIRE on a character appends it; THRUST finishes.
const ARCADE_CHARS: &[char] = &[
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
    'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ' ', '.',
    '-', '!', '<',
];

/// `ppHelpText1`, verbatim — 4 lines per subject in showcase order.
#[rustfmt::skip]
const HELP_TEXT: [&str; NUM_HELP_SUBJECTS * HELP_LINES] = [
    "Fender-Astrocaster", "ANNIHILATOR 9000", "", "State Of The Art In Wanton Destruction.",
    "GLOOPS", "", "Their Only Mission Is Kamikazi.", "Kill Them Or Get Out Of The Way!",
    "SPIKEBALLS", "", "Vulnerable When Open, But Watch Out,", "They're Gonna Blow!",
    "RAZOR-BOMBERS", "", "Jittery Little Suckers.", "Watch Out For Those Razors.",
    "HUNTER-KILLERS", "", "They Swoop In, Circle Around,", "And Unload Their Guns.  Nuff Said.",
    "FASTDEATHS", "", "They're Fast And You'll Be Dead.", "Any Questions?",
    "ASTEROIDS", "", "Blast Them Up And Get", "The Goodies Inside.",
    "ARMOR POWER UP", "", "Musical Therapy.", "Grab These For More Armor.",
    "SHIELD POWER UP", "", "Bring 'Em On!", "Need Some Shields?",
    "RAPID-FIRE", "", "Prepare to Rock!", "Make Your Guns Blaze.",
    "SPREAD-FIRE", "", "Rain Some Abuse!", "Three Times The Pain.",
    "SAW BLADES", "", "Rip 'n Tear!", "Nothing Survives These, Man.",
    "PROTON ACCELERATOR", "", "The Equalizer.", "More Power is Good!",
    "ASTRO-SMASHER", "", "A REALLY Big Gun.", "One Shot, One Hit, One Kill.",
    "EXTRA SHIP", "", "One More Chance to Get Even.", "",
];

impl Menu {
    /// `STATE_HELP` draw: four centered lines for the subject the
    /// monitor is showing (buttons come from the shared page loop).
    pub(crate) fn draw_help(&self, screen: &mut Frame) {
        for i in 0..HELP_LINES {
            self.font.print(
                screen,
                320,
                i as i32 * 15 + 260,
                HELP_TEXT[self.cur_help * HELP_LINES + i],
                Justify::Center,
                BlitMode::Transparent0,
            );
        }
    }

    /// Arrow keys page the help subjects (`SC_LEFTARROW`/`RIGHTARROW`
    /// in the `STATE_HELP` update arm).
    pub fn help_arrow(&mut self, right: bool) {
        if right && self.cur_help < NUM_HELP_SUBJECTS - 1 {
            self.cur_help += 1;
            self.showcase.switch_to(self.cur_help);
        } else if !right && self.cur_help > 0 {
            self.cur_help -= 1;
            self.showcase.switch_to(self.cur_help);
        }
    }

    /// `STATE_HIGHSCORES` draw: `"%d. %-20s %8d"` centered.
    pub(crate) fn draw_high_scores(&self, screen: &mut Frame) {
        for (i, (name, score)) in self.high_scores.iter().enumerate() {
            let text = format!("{}. {:<20} {:>8}", i + 1, name, score);
            self.font.print(
                screen,
                320,
                i as i32 * HIGH_SKIP + HIGH_TOP,
                &text,
                Justify::Center,
                BlitMode::Transparent0,
            );
        }
    }

    /// `STATE_NEWHIGHSCORE` draw: rank line, the ask, and the name
    /// with the flashing `-name-` cursor.
    pub(crate) fn draw_new_high_score(&self, screen: &mut Frame) {
        let rank = self.high_score_rank(self.pending_score).unwrap_or(0) + 1;
        let lines = [
            format!("Your score is {} number {}.", self.pending_score, rank),
            "Give us something to remember you by.".to_string(),
        ];
        for (i, line) in lines.iter().enumerate() {
            self.font.print(
                screen,
                320,
                i as i32 * 15 + 260,
                line,
                Justify::Center,
                BlitMode::Transparent0,
            );
        }
        // `CursorFlashOn = !CursorFlashOn` per update. In arcade
        // (touch) mode the scrolling letter rides the end of the
        // name, blinking so it reads as a cursor, with a hint line.
        let shown = if self.arcade_active {
            if self.flash % 16 < 10 {
                format!("{}{}", self.new_high_text, ARCADE_CHARS[self.entry_char])
            } else {
                format!("{} ", self.new_high_text)
            }
        } else {
            self.new_high_text.clone()
        };
        let name = if self.flash % 2 == 0 {
            format!("-{shown}-")
        } else {
            format!(" {shown} ")
        };
        self.font.print(
            screen,
            320,
            3 * 15 + 260,
            &name,
            Justify::Center,
            BlitMode::Transparent0,
        );
        if self.arcade_active {
            self.font.print(
                screen,
                320,
                5 * 15 + 260,
                "TURN: pick letter   FIRE: enter   THRUST: done",
                Justify::Center,
                BlitMode::Transparent0,
            );
        }
    }

    /// The Net Rock placeholder (where `STATE_NONETPLAY` printed its
    /// "no network" line).
    pub(crate) fn draw_net_soon(&self, screen: &mut Frame) {
        self.font.print(
            screen,
            320,
            295,
            "Net Rock: coming soon.",
            Justify::Center,
            BlitMode::Transparent0,
        );
    }

    /// The rank this score would take, `None` when it misses the
    /// table (`HSListGetRank` / `HSListIsNewScore`).
    pub(crate) fn high_score_rank(&self, score: u32) -> Option<usize> {
        self.high_scores.iter().position(|&(_, s)| score > s)
    }

    /// Game over with a qualifying score: open the entry page.
    pub fn start_high_score_entry(&mut self, score: u32) {
        self.pending_score = score;
        self.new_high_text.clear();
        self.page = Page::NewHighScore;
        self.from_game = false;
        self.entry_char = 0;
        self.arcade_active = false;
    }

    /// Arcade-style entry for touch (no keyboard): L/R scroll the
    /// current letter, FIRE enters it ('<' rubs one out), THRUST
    /// finishes the name. Engaged by the first edge so keyboard
    /// users never see the scroll cursor.
    pub fn arcade_edges(
        &mut self,
        left: bool,
        right: bool,
        fire: bool,
        thrust: bool,
        events: &mut Events,
    ) {
        if self.page != Page::NewHighScore || !(left || right || fire || thrust) {
            return;
        }
        self.arcade_active = true;
        let n = ARCADE_CHARS.len();
        if left {
            self.entry_char = (self.entry_char + n - 1) % n;
        }
        if right {
            self.entry_char = (self.entry_char + 1) % n;
        }
        if fire {
            let c = ARCADE_CHARS[self.entry_char];
            if c == '<' {
                self.new_high_text.pop();
            } else if self.new_high_text.len() < MAX_NAME {
                self.new_high_text.push(c);
            }
            events.push(crate::events::GameEvent::SfxClicked);
        }
        if thrust {
            events.push(crate::events::GameEvent::SfxClicked);
            self.commit_high_score();
        }
    }

    /// `PressedContinue` on the entry page: add, sort, persist, show
    /// the table ("No Name" for an empty entry, like HSListAddScore).
    pub(crate) fn commit_high_score(&mut self) {
        let name = if self.new_high_text.is_empty() {
            "No Name".to_string()
        } else {
            self.new_high_text.clone()
        };
        let last = self.high_scores.len() - 1;
        self.high_scores[last] = (name, self.pending_score);
        self.high_scores.sort_by_key(|e| std::cmp::Reverse(e.1));
        self.settings_dirty = true;
        self.page = Page::HighScores;
    }

    /// `STATE_NEWHIGHSCORE` typing: printable ASCII appends (up to
    /// `MAXNAMEWIDTH`), Backspace deletes, Enter/Esc commit through
    /// `on_enter`/`on_escape`. Returns true when the key was eaten.
    pub fn high_score_key(&mut self, key: &Key, _events: &mut Events) -> bool {
        if self.page != Page::NewHighScore {
            return false;
        }
        match key {
            Key::Backspace => {
                self.new_high_text.pop();
                true
            }
            Key::Char(c) if (' '..='~').contains(c) => {
                if self.new_high_text.len() < MAX_NAME {
                    self.new_high_text.push(*c);
                }
                true
            }
            // Enter/Escape fall through to on_enter/on_escape.
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_score_entry_types_commits_and_sorts() {
        let mut menu = Menu::new();
        let mut ev = Events::new();
        assert_eq!(menu.high_score_rank(1), Some(0), "beats EMPTY/0");
        assert_eq!(menu.high_score_rank(0), None, "ties don't qualify");

        menu.start_high_score_entry(5000);
        assert_eq!(menu.page, Page::NewHighScore);
        for c in "LARS!".chars() {
            assert!(menu.high_score_key(&Key::Char(c), &mut ev));
        }
        assert!(menu.high_score_key(&Key::Backspace, &mut ev));
        assert_eq!(menu.new_high_text, "LARS");
        // Names cap at MAXNAMEWIDTH.
        for c in "XXXXXXXXXXXXXXXXXXXX".chars() {
            menu.high_score_key(&Key::Char(c), &mut ev);
        }
        assert_eq!(menu.new_high_text.len(), MAX_NAME);

        menu.commit_high_score();
        assert_eq!(menu.page, Page::HighScores);
        assert_eq!(menu.high_scores[0].1, 5000);
        assert!(menu.settings_dirty);

        // A second, lower score lands in rank 2.
        menu.start_high_score_entry(100);
        menu.high_score_key(&Key::Char('B'), &mut ev);
        menu.commit_high_score();
        assert_eq!(menu.high_scores[1], ("B".to_string(), 100));
        assert_eq!(menu.high_scores.len(), 5, "table stays five deep");
    }

    #[test]
    fn help_pages_and_arrows() {
        let mut menu = Menu::new();
        assert_eq!(menu.cur_help, 0);
        menu.help_arrow(false);
        assert_eq!(menu.cur_help, 0, "clamped at the first subject");
        for _ in 0..NUM_HELP_SUBJECTS + 3 {
            menu.help_arrow(true);
        }
        assert_eq!(menu.cur_help, NUM_HELP_SUBJECTS - 1, "clamped at last");
        // Every subject has its 4 lines.
        assert_eq!(HELP_TEXT.len(), NUM_HELP_SUBJECTS * HELP_LINES);
    }
}
