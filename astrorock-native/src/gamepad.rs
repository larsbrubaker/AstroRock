//! # Native gamepad polling — gilrs → `agg_gui::gamepad`
//!
//! The native half of the pad plumbing (the web shell polls the
//! Gamepad API): drain gilrs events once per frame, mirror the first
//! active pad into [`agg_gui::gamepad`], and let the game read it
//! exactly like the browser build does. Y is flipped into screen
//! convention (gilrs sticks are +up; the shared state is +down).

use agg_gui::gamepad::{buttons, GamepadState};
use gilrs::{Axis, Button, Gilrs};

pub struct GamepadPoller {
    gilrs: Option<Gilrs>,
}

impl GamepadPoller {
    pub fn new() -> Self {
        let gilrs = match Gilrs::new() {
            Ok(g) => Some(g),
            Err(err) => {
                eprintln!("gamepad: init failed ({err}) — running without");
                None
            }
        };
        Self { gilrs }
    }

    /// Once per frame: pump events, publish the first connected pad.
    pub fn poll(&mut self) {
        let Some(gilrs) = self.gilrs.as_mut() else {
            return;
        };
        // Draining keeps gilrs's cached state fresh.
        while gilrs.next_event().is_some() {}

        let Some((_, pad)) = gilrs.gamepads().next() else {
            agg_gui::gamepad::set_state(None);
            return;
        };
        let axis = |a: Axis| pad.axis_data(a).map(|d| d.value() as f64).unwrap_or(0.0);
        let mut mask = 0u32;
        let pairs = [
            (Button::South, buttons::SOUTH),
            (Button::East, buttons::EAST),
            (Button::West, buttons::WEST),
            (Button::North, buttons::NORTH),
            (Button::LeftTrigger, buttons::L1),
            (Button::RightTrigger, buttons::R1),
            (Button::Select, buttons::SELECT),
            (Button::Start, buttons::START),
            (Button::DPadUp, buttons::DPAD_UP),
            (Button::DPadDown, buttons::DPAD_DOWN),
            (Button::DPadLeft, buttons::DPAD_LEFT),
            (Button::DPadRight, buttons::DPAD_RIGHT),
        ];
        for (b, bit) in pairs {
            if pad.is_pressed(b) {
                mask |= bit;
            }
        }
        agg_gui::gamepad::set_state(Some(GamepadState {
            left_x: axis(Axis::LeftStickX),
            // gilrs: +up. Screen convention: +down.
            left_y: -axis(Axis::LeftStickY),
            buttons: mask,
        }));
    }
}
