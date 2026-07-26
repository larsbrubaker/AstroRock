//! # Mobile touch + tilt input (modern layer)
//!
//! The virtual gamepad for phones: on-screen shield/fire/thrust holds
//! (chrome.rs draws them, title_screen.rs polls the fingers) and tilt
//! steering. Tilt finds the lean angle and turns the ship toward it
//! at the NORMAL key-rotate speed — a way to reach a direction, never
//! a faster-than-keys rotate. Everything here merges into the same
//! `ShipInputs` the keyboard feeds, and demo playback bypasses it
//! entirely (`sim_beat` gates on the Demo state).

use crate::game::{Game, Screen};

/// Held state of the on-screen touch buttons.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct TouchHeld {
    pub shield: bool,
    pub fire: bool,
    pub thrust: bool,
}

impl Game {
    /// Update the virtual-gamepad hold state (widget, per frame). A
    /// fresh FIRE press also acts as Start/continue — the touch path
    /// through the press-Enter gates (spawn, tally skip, game over).
    pub fn set_touch(&mut self, touch: TouchHeld) {
        if touch.fire && !self.touch.fire && self.state != Screen::Menu {
            self.enter_pressed = true;
        }
        self.touch = touch;
    }

    /// Feed the tilt reading (screen-space lean, degrees). Inside the
    /// dead zone the ship keeps its heading; outside, the vector's
    /// angle becomes the rotation target: frame 0 points up, angles
    /// grow clockwise, 32 frames per turn — `atan2(sx, -sy)` in the
    /// game's screen coordinates (y down).
    pub fn set_tilt(&mut self, reading: Option<(f64, f64)>) {
        // Degrees of lean before tilt starts steering — mirrored by
        // the joystick pad's thin inner ring.
        const DEAD_ZONE: f64 = crate::joystick::DEAD_ZONE_DEG;
        self.tilt_target = reading.and_then(|(sx, sy)| {
            if (sx * sx + sy * sy).sqrt() < DEAD_ZONE {
                return None;
            }
            let angle = sx.atan2(-sy); // radians, 0 = up, clockwise
            let frame = (angle / std::f64::consts::TAU * 32.0 + 32.0) % 32.0;
            Some(frame as f32)
        });
    }

    /// Apply the tilt/joystick heading: the ship SNAPS to the stick
    /// direction (instant, by request — incremental chase felt
    /// confusing in play). Rounded to a whole rotation frame so the
    /// sprite and the shot direction agree.
    pub(crate) fn apply_tilt_heading(&mut self) {
        if let Some(target) = self.tilt_target {
            if self.ship.sprite.visible {
                self.ship.sprite.cur_frame = target.round() % 32.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilt_snaps_the_heading_instantly() {
        let mut g = Game::new(None);
        g.ship.sprite.visible = true;
        g.ship.sprite.cur_frame = 0.0; // pointing up

        // Lean hard right: heading snaps straight to east (frame 8).
        g.set_tilt(Some((30.0, 0.0)));
        g.apply_tilt_heading();
        assert_eq!(g.ship.sprite.cur_frame, 8.0);

        // Lean hard left: snaps to west (24) in ONE beat, no chase.
        g.set_tilt(Some((-30.0, 0.0)));
        g.apply_tilt_heading();
        assert_eq!(g.ship.sprite.cur_frame, 24.0);

        // Inside the dead zone: heading holds.
        g.set_tilt(Some((2.0, 3.0)));
        g.apply_tilt_heading();
        assert_eq!(g.ship.sprite.cur_frame, 24.0);
        g.set_tilt(None);
        g.apply_tilt_heading();
        assert_eq!(g.ship.sprite.cur_frame, 24.0);

        // An invisible (dead) ship never snaps.
        g.ship.sprite.visible = false;
        g.set_tilt(Some((30.0, 0.0)));
        g.apply_tilt_heading();
        assert_eq!(g.ship.sprite.cur_frame, 24.0);
    }

    #[test]
    fn fresh_fire_touch_acts_as_start() {
        let mut g = Game::new(None);
        // On the menu a fire-hold means nothing.
        g.set_touch(TouchHeld {
            fire: true,
            ..Default::default()
        });
        assert!(!g.enter_pressed);
        g.set_touch(TouchHeld::default());

        // In play, the press edge doubles as Enter (spawn gate).
        g.state = Screen::Playing;
        g.set_touch(TouchHeld {
            fire: true,
            ..Default::default()
        });
        assert!(g.enter_pressed);
        g.enter_pressed = false;
        // Holding does not re-trigger.
        g.set_touch(TouchHeld {
            fire: true,
            ..Default::default()
        });
        assert!(!g.enter_pressed);
    }
}
