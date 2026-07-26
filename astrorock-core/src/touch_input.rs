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

    /// The rotate keys the tilt target implies this beat: turn toward
    /// the target the short way at normal key speed; stop inside half
    /// a frame so the heading doesn't oscillate.
    pub(crate) fn tilt_rotate(&self) -> (bool, bool) {
        let Some(target) = self.tilt_target else {
            return (false, false);
        };
        let mut diff = target - self.ship.sprite.cur_frame;
        while diff > 16.0 {
            diff -= 32.0;
        }
        while diff < -16.0 {
            diff += 32.0;
        }
        if diff > 0.5 {
            (false, true)
        } else if diff < -0.5 {
            (true, false)
        } else {
            (false, false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilt_steers_toward_the_lean_at_key_speed() {
        let mut g = Game::new(None);
        g.ship.sprite.cur_frame = 0.0; // pointing up

        // Lean hard right: target frame 8 (east) — rotate right.
        g.set_tilt(Some((30.0, 0.0)));
        assert_eq!(g.tilt_rotate(), (false, true));

        // Lean hard left: target 24 (west) — the short way is left.
        g.set_tilt(Some((-30.0, 0.0)));
        assert_eq!(g.tilt_rotate(), (true, false));

        // Already on target: no keys — no oscillation.
        g.ship.sprite.cur_frame = 8.0;
        g.set_tilt(Some((30.0, 0.0)));
        assert_eq!(g.tilt_rotate(), (false, false));

        // Wrap: from frame 30, east (8) is 10 steps right, not 22
        // left.
        g.ship.sprite.cur_frame = 30.0;
        assert_eq!(g.tilt_rotate(), (false, true));

        // Inside the dead zone: no target at all.
        g.set_tilt(Some((2.0, 3.0)));
        assert_eq!(g.tilt_rotate(), (false, false));
        g.set_tilt(None);
        assert_eq!(g.tilt_rotate(), (false, false));
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
