//! # Level tally + bonuses — port of the intermission flow
//!
//! `AstroRock.cpp`: `ResetIntermisionInfo`, `SetUpIntermissionScreen`,
//! `UpdateIntermission`, `DrawIntermission`, and the
//! `STATE_INTERMISSION` arm of the main loop. When the last enemy dies
//! the view irises shut over `CLOSELEVELDURRATION` beats (the sim keeps
//! running), the next level resets behind it, and the tally window
//! slides down to count the bonus into the score at `TotalBonus/90`
//! per beat.
//!
//! Quirk preserved: the exit test fires the moment `TotalBonus` hits
//! zero — before `RaisingIntermission` ever decrements — so the
//! slide-up animation is vestigial 1997 code that never plays. Ported
//! as-is; Enter or Escape skips (banking the remainder), matching
//! `PressedContinue`.

use crate::assets;
use crate::font::{Font, Justify};
use crate::frame::{BlitMode, Frame};
use crate::rect::Rect;

/// `#define CLOSELEVELDURRATION 60`
pub const CLOSE_LEVEL_DURATION: i32 = 60;
/// `#define INTERMISSIONMOVEDURRATION 20`
const MOVE_DURATION: i32 = 20;
/// `#define INTERMISSIONDISTANCE 350`
const DISTANCE: i32 = 350;
/// `#define NUMBONUSADDSTEPS 90`
const NUM_BONUS_ADD_STEPS: i32 = 90;

/// The per-level running stats and bonus pots (`Global*` in
/// AstroRock.cpp; the zeroing rules live at the gameplay hooks).
pub struct LevelStats {
    /// Enemy count captured at level start (`NeedNumBadGuys`).
    pub bad_guys_killed: i32,
    pub shots_fired: i32,
    pub shots_hit: i32,
    pub lives_lost: i32,
    /// Scaled by hit percentage at tally time.
    pub deadeye: i32,
    /// Zeroed when the local player dies (`KillPlayer`).
    pub survival: i32,
    /// Zeroed at tally time if any rocks remain.
    pub annihilation: i32,
    /// Zeroed when the local player takes damage (`DamagePlayer`).
    pub untouched: i32,
    /// Zeroed any beat the shield is on (`PlayersUpdate`).
    pub no_shielding: i32,
}

impl LevelStats {
    pub fn new() -> Self {
        let mut stats = Self {
            bad_guys_killed: 0,
            shots_fired: 0,
            shots_hit: 0,
            lives_lost: 0,
            deadeye: 0,
            survival: 0,
            annihilation: 0,
            untouched: 0,
            no_shielding: 0,
        };
        stats.reset(0);
        stats
    }

    /// `ResetIntermisionInfo` — every pot starts at `200 + 50*level`.
    pub fn reset(&mut self, level: u32) {
        self.bad_guys_killed = 0;
        self.shots_fired = 0;
        self.shots_hit = 0;
        self.lives_lost = 0;
        let pot = 200 + 50 * level as i32;
        self.deadeye = pot;
        self.survival = pot;
        self.annihilation = pot;
        self.untouched = pot;
        self.no_shielding = pot;
    }
}

impl Default for LevelStats {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Intermission {
    tally_base: Frame,
    tally: Frame,
    font: Font,
    /// Iris-shut countdown; the sim keeps running while it ticks.
    pub close_level: i32,
    pub lowering: i32,
    pub raising: i32,
    pub total_bonus: i32,
    bonus_add: i32,
    bonus_beats: i32,
    /// Tally window position, updated by [`Self::update_slide`].
    pub x: i32,
    pub y: i32,
}

impl Intermission {
    pub fn new() -> Self {
        let tally_base = assets::frame_from_indexed_png(assets::TALLYWIN_PNG);
        let tally = assets::frame_from_indexed_png(assets::TALLYWIN_PNG);
        Self {
            tally_base,
            tally,
            font: Font::astro(),
            close_level: 0,
            lowering: 0,
            raising: 0,
            total_bonus: 0,
            bonus_add: 0,
            bonus_beats: 0,
            x: 0,
            y: 0,
        }
    }

    /// `SetUpIntermissionScreen` + `SetStateIntermission`: settle the
    /// pots, render the tally text, arm the counters.
    pub fn begin(&mut self, stats: &mut LevelStats, rocks_left: i32) {
        let percentage = if stats.shots_fired > 0 {
            stats.shots_hit = stats.shots_hit.min(stats.shots_fired);
            (stats.shots_hit as i64 * 10000 / stats.shots_fired as i64) as i32
        } else {
            0
        };
        stats.deadeye = (percentage as i64 * stats.deadeye as i64 / 10000) as i32;
        if rocks_left != 0 {
            stats.annihilation = 0;
        }
        self.total_bonus = stats.deadeye
            + stats.survival
            + stats.annihilation
            + stats.untouched
            + stats.no_shielding;
        self.bonus_add = self.total_bonus / NUM_BONUS_ADD_STEPS;
        self.bonus_beats = 0;

        self.render_tally(stats, percentage);

        if self.total_bonus > 0 {
            self.lowering = MOVE_DURATION;
            self.raising = MOVE_DURATION;
        } else {
            self.lowering = 0;
            self.raising = 0;
        }
        self.close_level = CLOSE_LEVEL_DURATION;
    }

    /// The 11 tally lines, C formats preserved (`%4d` pads with the
    /// astro font's real space glyph, unlike the digit fonts).
    fn render_tally(&mut self, stats: &LevelStats, percentage: i32) {
        self.tally = self.tally_base.clone();
        let tc = self.tally.width / 2;
        let (ta, space) = (15, 7);
        let mut tt = 47;

        let half = self.font.width_of(" ") * 12;
        self.tally
            .fill_box(&Rect::new(tc - half, tt, tc + half, tt + ta * 13 + 4), 0);
        tt += ta + space;

        let line = |tally: &mut Frame, tt: i32, text: String| {
            self.font.print(
                tally,
                tc,
                tt,
                &text,
                Justify::Center,
                BlitMode::Transparent0,
            );
        };
        line(
            &mut self.tally,
            tt,
            format!("Bad Guys Killed:    {:4}", stats.bad_guys_killed),
        );
        tt += ta;
        line(
            &mut self.tally,
            tt,
            format!("Shots Fired:        {:4}", stats.shots_fired),
        );
        tt += ta;
        line(
            &mut self.tally,
            tt,
            format!("Shots Hit:          {:4}", stats.shots_hit),
        );
        tt += ta;
        line(
            &mut self.tally,
            tt,
            format!(
                "Hit Percentage:   {:3}.{:02}",
                percentage / 100,
                percentage % 100
            ),
        );
        tt += ta;
        line(
            &mut self.tally,
            tt,
            format!("Lives Lost:         {:4}", stats.lives_lost),
        );
        tt += ta + space;
        line(
            &mut self.tally,
            tt,
            format!("Deadeye Bonus:      {:4}", stats.deadeye),
        );
        tt += ta;
        line(
            &mut self.tally,
            tt,
            format!("Survival Bonus:     {:4}", stats.survival),
        );
        tt += ta;
        line(
            &mut self.tally,
            tt,
            format!("Annhilation Bonus:  {:4}", stats.annihilation),
        );
        tt += ta;
        line(
            &mut self.tally,
            tt,
            format!("Untouched Bonus:    {:4}", stats.untouched),
        );
        tt += ta;
        line(
            &mut self.tally,
            tt,
            format!("No Shielding Bonus: {:4}", stats.no_shielding),
        );
        tt += ta + space;
        line(
            &mut self.tally,
            tt,
            format!("Total Bonus:        {:4}", self.total_bonus),
        );
    }

    /// `InterBox` — the iris rect for the current countdown value.
    pub fn shrink_rect(&self, on_screen: &Rect) -> Rect {
        let t = CLOSE_LEVEL_DURATION - self.close_level;
        let w = on_screen.width();
        let h = on_screen.height();
        Rect::new(
            on_screen.left + (w / 2 * t) / CLOSE_LEVEL_DURATION,
            on_screen.top + (h / 2 * t) / CLOSE_LEVEL_DURATION,
            on_screen.right - (w / 2 * t) / CLOSE_LEVEL_DURATION,
            on_screen.bottom - (h / 2 * t) / CLOSE_LEVEL_DURATION,
        )
    }

    /// `UpdateIntermission` — recenter, then slide.
    pub fn update_slide(&mut self, on_screen_w: i32, on_screen_h: i32) {
        self.x = on_screen_w / 2 - self.tally.width / 2;
        self.y = (on_screen_h / 2 - self.tally.height / 2) + 10;
        if self.lowering > 0 {
            self.y -= DISTANCE * self.lowering / MOVE_DURATION;
            self.lowering -= 1;
        } else if self.raising > 0 && self.total_bonus == 0 {
            self.y -= DISTANCE * (MOVE_DURATION - self.raising) / MOVE_DURATION;
            self.raising -= 1;
        }
    }

    /// One beat of bonus counting once the window is down. Returns the
    /// score to add and whether the bonus blip plays (every 2nd add).
    pub fn count_step(&mut self) -> (u32, bool) {
        if self.lowering > 0 || self.total_bonus <= 0 {
            return (0, false);
        }
        if self.total_bonus > self.bonus_add && self.bonus_add > 0 {
            self.bonus_beats += 1;
            self.total_bonus -= self.bonus_add;
            (self.bonus_add as u32, self.bonus_beats % 2 == 0)
        } else {
            let rest = self.total_bonus;
            self.total_bonus = 0;
            (rest as u32, false)
        }
    }

    /// `DrawIntermission`.
    pub fn draw(&self, screen: &mut Frame) {
        screen.blit(
            &self.tally,
            &self.tally.bounds(),
            self.x,
            self.y,
            BlitMode::Normal,
        );
    }
}

impl Default for Intermission {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pots_scale_with_level() {
        let mut s = LevelStats::new();
        assert_eq!(s.deadeye, 200);
        s.reset(4);
        assert_eq!(s.survival, 400);
        assert_eq!(s.untouched, 400);
    }

    #[test]
    fn begin_settles_pots_and_arms_counters() {
        let mut inter = Intermission::new();
        let mut s = LevelStats::new();
        s.shots_fired = 100;
        s.shots_hit = 50; // 50.00% -> deadeye halves
        inter.begin(&mut s, 3); // rocks left -> annihilation zeroed
        assert_eq!(s.deadeye, 100);
        assert_eq!(s.annihilation, 0);
        // deadeye 100 + survival 200 + annihilation 0 + untouched 200
        // + no-shielding 200.
        assert_eq!(inter.total_bonus, 700);
        assert_eq!(inter.close_level, CLOSE_LEVEL_DURATION);
        assert_eq!(inter.lowering, MOVE_DURATION);
    }

    #[test]
    fn count_step_drains_total_into_score() {
        let mut inter = Intermission::new();
        let mut s = LevelStats::new();
        s.shots_fired = 1;
        s.shots_hit = 1; // 100% hit keeps the full pots
        inter.begin(&mut s, 0);
        let total = inter.total_bonus as u32;
        inter.lowering = 0; // window already down
        let mut banked = 0u32;
        let mut steps = 0;
        while inter.total_bonus > 0 {
            let (add, _blip) = inter.count_step();
            banked += add;
            steps += 1;
            assert!(steps < 200, "bonus never drains");
        }
        assert_eq!(banked, total);
        assert_eq!(inter.count_step(), (0, false));
    }

    #[test]
    fn shrink_rect_irises_to_center() {
        let mut inter = Intermission::new();
        let on_screen = Rect::new(0, 0, 640, 384);
        inter.close_level = CLOSE_LEVEL_DURATION;
        assert_eq!(inter.shrink_rect(&on_screen), on_screen);
        inter.close_level = 1;
        let almost = inter.shrink_rect(&on_screen);
        assert!(almost.width() < 22 && almost.height() < 14);
        assert!(almost.width() > 0 && almost.height() > 0);
    }
}
