//! # The bad-guy showcase monitor
//!
//! `SwitchBadGuy`/`UpdateBadGuyScreen`/`DrawBadGuyScreen` from
//! `StartScreen.cpp`: a random subject from 15 — the ship, five
//! enemies, a rock, and the eight goodies — switches every
//! `SWITCHBADGUYPAUSE` beats behind nine frames of TV static and
//! fades in through the `FadeBlit` tables.

use std::rc::Rc;

use crate::assets;
use crate::events::{Events, GameEvent};
use crate::frame::{BlitMode, Frame};
use crate::palette::{FadeBlits, NUM_FADES};
use crate::rand::Rand;
use crate::sequence::{self, FrameSequence};

/// `#define SWITCHBADGUYPAUSE 150`
pub(crate) const SWITCH_PAUSE: i32 = 150;
/// `#define NUMSTATICUPDATES 9`
pub(crate) const NUM_STATIC_UPDATES: i32 = 9;
/// `BADGUYX/Y` — direct screen coordinates.
const BADGUY_X: i32 = 320;
const BADGUY_Y: i32 = 150;
/// Spikeballs are showcase type 2 (they get the roll-walk dance).
const TYPE_SPIKEBALL: usize = 2;

pub(crate) struct Showcase {
    seqs: Vec<Rc<FrameSequence>>,
    statics: [Frame; 3],
    pub(crate) cur: usize,
    pub(crate) pause: i32,
    cur_frame: f32,
    spike_dir: f32,
    pub(crate) cur_fade: usize,
}

impl Showcase {
    pub(crate) fn new() -> Self {
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

    /// `SwitchBadGuy(type)` — the help pages pick their subject
    /// manually; the static burst plays like any other switch.
    pub(crate) fn switch_to(&mut self, subject: usize) {
        self.cur = subject % self.seqs.len();
        self.pause = 0;
    }

    /// One menu beat (`UpdateBadGuyScreen` + the switch check).
    /// `auto_switch` is off during the help pages (`ScreenState !=
    /// STATE_HELP` gate in StartScreenUpdate).
    pub(crate) fn update(&mut self, local_rand: &mut Rand, events: &mut Events, auto_switch: bool) {
        if auto_switch && self.pause >= SWITCH_PAUSE {
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
            && self.pause < NUM_FADES as i32 + NUM_STATIC_UPDATES
        {
            let fade = NUM_FADES as i32 + NUM_STATIC_UPDATES - self.pause;
            self.cur_fade = (fade.min(NUM_FADES as i32 - 1)) as usize;
        } else {
            self.cur_fade = 0;
        }
        self.pause += 1;
    }

    /// `DrawBadGuyScreen`.
    pub(crate) fn draw(&self, screen: &mut Frame, fades: &FadeBlits) {
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
