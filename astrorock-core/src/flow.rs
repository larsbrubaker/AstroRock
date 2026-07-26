//! # State transitions — the main-loop switch arms
//!
//! The transition half of `game.rs` (which owns the state and the
//! per-beat sim): the `STATE_PLAYING` gate/level-end/death logic and
//! the `STATE_INTERMISSION` iris + tally beat from `AstroRock.cpp`'s
//! `switch (GameState)`.

use crate::events::GameEvent;
use crate::game::{Game, Screen};
use crate::rect::Rect;

impl Game {
    /// The `STATE_PLAYING` arm: spawn gate, level end, death.
    pub(crate) fn playing_transitions(&mut self) {
        // `NeedToAddLocalPlayer` + `PressedContinue` -> `AddPlayer`.
        if self.need_add_player && self.enter_pressed {
            self.respawn();
            self.need_add_player = false;
        }

        // `NumBadGuys == 0` -> `SetStateIntermission`. Rocks don't
        // count — leftovers only zero the annihilation bonus.
        if self.enemies_alive() == 0 {
            let rocks_left = (self.rocks.num_big + self.rocks.num_med + self.rocks.num_lit) as i32;
            self.inter.begin(&mut self.stats, rocks_left);
            self.state = Screen::Intermission;
        }

        if self.local_player_dead {
            self.local_player_dead = false;
            if self.ship.num_ships == 0 {
                self.world.set_on_screen_rect(self.on_screen());
                self.game_over_pause = 0;
                self.state = Screen::GameOver;
            } else {
                self.need_add_player = true;
            }
        }
    }

    /// The `STATE_INTERMISSION` arm: the iris (sim still running),
    /// then the sliding tally counting the bonus into the score.
    pub(crate) fn intermission_beat(&mut self, clip: Rect) {
        let on_screen = self.on_screen();
        if self.inter.close_level > 0 {
            self.sim_beat(clip);
            self.inter.close_level -= 1;
            if self.inter.close_level > 0 {
                let iris = self.inter.shrink_rect(&on_screen);
                self.world.set_on_screen_rect(iris);
            } else {
                self.world.set_on_screen_rect(on_screen);
                self.level += 1;
                self.new_level();
                self.inter
                    .update_slide(on_screen.width(), on_screen.height());
            }
        } else if self.enter_pressed || self.inter.raising == 0 || self.inter.total_bonus == 0 {
            // Skip or done: bank the remainder and play on
            // (`ResetIntermisionInfo` on the way out).
            self.ship.add_score(self.inter.total_bonus.max(0) as u32);
            self.inter.total_bonus = 0;
            self.stats.reset(self.level as u32);
            self.stats.bad_guys_killed = self.enemies_alive() as i32;
            self.state = Screen::Playing;
        } else {
            self.inter
                .update_slide(on_screen.width(), on_screen.height());
            let (add, blip) = self.inter.count_step();
            if add > 0 {
                self.ship.add_score(add);
            }
            if blip {
                self.events.push(GameEvent::SfxBonus);
            }
        }
    }
}
