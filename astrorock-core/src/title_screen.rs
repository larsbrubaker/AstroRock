//! # The game widget — agg-gui shell around `game.rs`
//!
//! Owns the widget bounds, forwards input (keys via `Game::set_key`,
//! chrome-button clicks), and presents the composed 640x480 indexed
//! frame each paint: advance the sim, compose, palette-convert to
//! RGBA, and upload inside the themed chrome (chrome.rs).

use std::sync::Arc;

use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult, Key};
use agg_gui::geometry::{Rect as GuiRect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;

use crate::audio::AudioSink;
use crate::chrome;
use crate::game::{Game, SCREEN_H, SCREEN_W};

/// Font Awesome, for the chrome-button icons.
const ICON_FONT_BYTES: &[u8] = include_bytes!("../assets/fa.ttf");

pub struct TitleScreen {
    bounds: GuiRect,
    children: Vec<Box<dyn Widget>>,
    game: Game,
    rgba: Vec<u8>,
    icons: Arc<Font>,
    /// Chrome button hit rects in widget coords — recomputed each
    /// paint, checked on MouseDown.
    music_btn: GuiRect,
    sfx_btn: GuiRect,
    fullscreen_btn: GuiRect,
    /// Where the game surface landed last paint (widget coords) —
    /// translates mouse events into 640x480 game coordinates.
    game_rect: (f64, f64, f64, f64),
    /// Text font for the size dropdown's labels.
    text_font: Arc<Font>,
    /// Mobile virtual-gamepad rects from the last paint — polled
    /// against the active fingers each frame.
    touch_layout: Option<chrome::TouchLayout>,
    /// Gamepad buttons last frame — Start/Select fire on the edge.
    prev_pad_buttons: u32,
    /// Touch holds last frame — edges drive the arcade name entry.
    prev_touch: crate::touch_input::TouchHeld,
    /// The gear's S/M/L/XL dropdown is open.
    size_menu_open: bool,
}

impl TitleScreen {
    pub fn new() -> Self {
        Self::new_with_audio(None)
    }

    pub fn new_with_audio(audio: Option<Box<dyn AudioSink>>) -> Self {
        Self::new_with_platform(audio, None)
    }

    pub fn new_with_platform(
        audio: Option<Box<dyn AudioSink>>,
        settings: Option<Box<dyn crate::settings::SettingsStore>>,
    ) -> Self {
        let mut game = Game::new(audio);
        if let Some(store) = settings {
            game.set_settings_store(store);
        }
        Self {
            bounds: GuiRect::default(),
            children: Vec::new(),
            game,
            rgba: Vec::new(),
            icons: Arc::new(Font::from_slice(ICON_FONT_BYTES).expect("fa.ttf parses")),
            music_btn: GuiRect::default(),
            sfx_btn: GuiRect::default(),
            fullscreen_btn: GuiRect::default(),
            game_rect: (0.0, 0.0, 1.0, 1.0),
            text_font: crate::load_default_font(),
            touch_layout: None,
            prev_pad_buttons: 0,
            prev_touch: crate::touch_input::TouchHeld::default(),
            size_menu_open: false,
        }
    }

    /// Widget position -> 640x480 game-surface coordinates (widget Y
    /// is bottom-up; the game screen is top-down).
    fn to_game_coords(&self, x: f64, y: f64) -> (i32, i32) {
        let (dx, dy, dw, dh) = self.game_rect;
        let gx = (x - dx) / dw * SCREEN_W as f64;
        let gy = SCREEN_H as f64 - (y - dy) / dh * SCREEN_H as f64;
        (gx as i32, gy as i32)
    }
}

impl Default for TitleScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for TitleScreen {
    fn type_name(&self) -> &'static str {
        "TitleScreen"
    }

    fn bounds(&self) -> GuiRect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: GuiRect) {
        self.bounds = bounds;
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Box<dyn Widget>> {
        &mut self.children
    }

    fn is_focusable(&self) -> bool {
        // Held-key state needs KeyUp delivery, which only reaches the
        // focused widget. The app calls `focus_first()` at startup.
        true
    }

    fn layout(&mut self, available: Size) -> Size {
        available
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        // A real gamepad works everywhere (desktop included): the
        // left stick steers exactly like the touch pad (snap heading,
        // deflection past THRUST_FRAC = thrust), South fires,
        // East/shoulders shield, West bombs, Start = Enter,
        // Select/Back = Esc.
        let pad = agg_gui::gamepad::state();
        {
            use agg_gui::gamepad::buttons as gb;
            let now = pad.map(|p| p.buttons).unwrap_or(0);
            let edge = |bit: u32| now & bit != 0 && self.prev_pad_buttons & bit == 0;
            if edge(gb::START) {
                self.game.set_key(&Key::Enter, true);
                self.game.set_key(&Key::Enter, false);
            }
            if edge(gb::SELECT) {
                self.game.set_key(&Key::Escape, true);
                self.game.set_key(&Key::Escape, false);
            }
            self.prev_pad_buttons = now;
        }
        let pad_stick = pad.and_then(|p| {
            let mag = (p.left_x * p.left_x + p.left_y * p.left_y).sqrt();
            (mag >= 0.2).then_some((p.left_x, p.left_y))
        });
        let (pad_shield, pad_fire, pad_bomb) = pad
            .map(|p| {
                use agg_gui::gamepad::buttons as gb;
                (
                    p.pressed(gb::EAST) || p.pressed(gb::L1) || p.pressed(gb::R1),
                    p.pressed(gb::SOUTH),
                    p.pressed(gb::WEST),
                )
            })
            .unwrap_or((false, false, false));

        // Pad steering applies everywhere: snap heading + thrust
        // past THRUST_FRAC of full stick.
        let steer = pad_stick.map(|(x, y)| {
            (
                x * crate::joystick::MAX_TILT_DEG,
                y * crate::joystick::MAX_TILT_DEG,
            )
        });
        self.game.set_tilt(steer);
        let pad_thrust = steer
            .map(|(x, y)| {
                (x * x + y * y).sqrt()
                    >= crate::joystick::MAX_TILT_DEG * crate::joystick::THRUST_FRAC
            })
            .unwrap_or(false);

        // Mobile: plain hold buttons — rotate pair under the left
        // thumb, fire/thrust/shield under the right (tilt steering
        // retired; it never matched the 30 Hz ship feel).
        let touch_mode = agg_gui::input_profile::is_mobile_touch();
        let touch_ui = if touch_mode {
            let fingers = agg_gui::touch_points::active();
            let over = |r: &GuiRect| fingers.iter().any(|p| chrome::hit(r, p.pos.x, p.pos.y));
            let held = match &self.touch_layout {
                Some(t) => crate::touch_input::TouchHeld {
                    left: over(&t.left_btn),
                    right: over(&t.right_btn),
                    fire: over(&t.fire_btn) || pad_fire,
                    thrust: over(&t.thrust_btn) || pad_thrust,
                    shield: over(&t.shield_btn) || pad_shield,
                    bomb: pad_bomb,
                },
                None => crate::touch_input::TouchHeld::default(),
            };
            // Fresh press EDGES drive the arcade name entry.
            self.game.touch_edges(
                held.left && !self.prev_touch.left,
                held.right && !self.prev_touch.right,
                held.fire && !self.prev_touch.fire,
                held.thrust && !self.prev_touch.thrust,
            );
            self.prev_touch = held;
            self.game.set_touch(held);
            Some(chrome::TouchUi {
                left: held.left,
                right: held.right,
                fire: held.fire,
                thrust: held.thrust,
                shield: held.shield,
                size: self.game.touch_size,
                size_menu: self.size_menu_open,
            })
        } else {
            // Desktop: no touch chrome, but a connected pad still
            // fires through the same lane.
            self.game.set_touch(crate::touch_input::TouchHeld {
                left: false,
                right: false,
                shield: pad_shield,
                fire: pad_fire,
                thrust: pad_thrust,
                bomb: pad_bomb,
            });
            None
        };

        self.game.advance(self.game.now_ms());
        self.game.compose();
        // Keep the loop animating — content changes every frame, so the
        // full request_draw (animation.rs: "animation ticks … must call
        // request_draw").
        agg_gui::animation::request_draw();
        self.game
            .current_palette()
            .frame_to_rgba(self.game.screen(), &mut self.rgba);

        // Window chrome (chrome.rs): backdrop, rail/bar, buttons, and
        // the frame; it returns where the game surface goes.
        let (w, h) = (self.bounds.width, self.bounds.height);
        let layout = chrome::paint(
            ctx,
            w,
            h,
            self.game.music_on,
            self.game.sfx_on,
            touch_ui,
            &self.icons,
            &self.text_font,
        );
        self.music_btn = layout.music_btn;
        self.sfx_btn = layout.sfx_btn;
        self.fullscreen_btn = layout.fullscreen_btn;
        self.game_rect = layout.game;
        self.touch_layout = layout.touch;
        let (dx, dy, dw, dh) = layout.game;

        // A fresh Arc every frame, deliberately: the slice variant's
        // texture cache keys on pointer + head/tail bytes, and a reused
        // buffer whose corners stay black collides forever — the screen
        // freezes on the first uploaded frame while the sim runs at
        // 60fps. The Arc variant keys on pointer identity with proper
        // Weak-sentinel sweeping, so per-frame Arcs upload per frame.
        let frame_pixels = std::sync::Arc::new(std::mem::take(&mut self.rgba));
        ctx.draw_image_rgba_arc(
            &frame_pixels,
            SCREEN_W as u32,
            SCREEN_H as u32,
            dx,
            dy,
            dw,
            dh,
        );
    }

    fn on_event(&mut self, event: &Event) -> EventResult {
        match event {
            Event::KeyDown { key, modifiers } => {
                // Alt+Enter: the classic Windows fullscreen toggle.
                if *key == Key::Enter && modifiers.alt {
                    agg_gui::fullscreen::request_toggle();
                    return EventResult::Consumed;
                }
                if self.game.set_key(key, true) {
                    EventResult::Consumed
                } else {
                    EventResult::Ignored
                }
            }
            Event::KeyUp { key, .. } => {
                if self.game.set_key(key, false) {
                    EventResult::Consumed
                } else {
                    EventResult::Ignored
                }
            }
            Event::MouseDown { pos, .. } => {
                // The size gear + its dropdown come first: an open
                // dropdown eats the click either way.
                if let Some(t) = &self.touch_layout {
                    if chrome::hit(&t.gear_btn, pos.x, pos.y) {
                        self.size_menu_open = !self.size_menu_open;
                        return EventResult::Consumed;
                    }
                    if self.size_menu_open {
                        if let Some(opts) = &t.size_opts {
                            for (opt, preset) in opts.iter().zip(chrome::TouchSize::ALL) {
                                if chrome::hit(opt, pos.x, pos.y) {
                                    self.game.set_touch_size(preset);
                                    break;
                                }
                            }
                        }
                        self.size_menu_open = false;
                        return EventResult::Consumed;
                    }
                }
                if chrome::hit(&self.music_btn, pos.x, pos.y) {
                    self.game.toggle_music();
                    EventResult::Consumed
                } else if chrome::hit(&self.sfx_btn, pos.x, pos.y) {
                    self.game.toggle_sfx();
                    EventResult::Consumed
                } else if chrome::hit(&self.fullscreen_btn, pos.x, pos.y) {
                    agg_gui::fullscreen::request_toggle();
                    EventResult::Consumed
                } else if self
                    .touch_layout
                    .as_ref()
                    .is_some_and(|t| chrome::hit(&t.menu_btn, pos.x, pos.y))
                {
                    // The mobile menu button = the Esc key.
                    self.game.set_key(&Key::Escape, true);
                    self.game.set_key(&Key::Escape, false);
                    EventResult::Consumed
                } else {
                    let (gx, gy) = self.to_game_coords(pos.x, pos.y);
                    self.game.on_mouse_down(gx, gy);
                    EventResult::Consumed
                }
            }
            Event::MouseUp { pos, .. } => {
                let (gx, gy) = self.to_game_coords(pos.x, pos.y);
                self.game.on_mouse_up(gx, gy);
                EventResult::Consumed
            }
            Event::MouseMove { pos } => {
                let (gx, gy) = self.to_game_coords(pos.x, pos.y);
                self.game.on_mouse_move(gx, gy);
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_buttons_toggle_audio_flags() {
        use agg_gui::event::MouseButton;
        use agg_gui::geometry::Point;
        let mut t = TitleScreen::new();
        t.music_btn = GuiRect::new(10.0, 10.0, 90.0, 26.0);
        t.sfx_btn = GuiRect::new(110.0, 10.0, 90.0, 26.0);
        assert!(t.game.music_on && t.game.sfx_on);
        let click = |x: f64, y: f64| Event::MouseDown {
            pos: Point { x, y },
            button: MouseButton::Left,
            modifiers: Default::default(),
        };
        assert_eq!(t.on_event(&click(50.0, 20.0)), EventResult::Consumed);
        assert!(!t.game.music_on && t.game.sfx_on);
        assert_eq!(t.on_event(&click(150.0, 20.0)), EventResult::Consumed);
        assert!(!t.game.music_on && !t.game.sfx_on);
        // A click outside the chrome forwards into the game surface
        // (menu buttons live there now) without touching the toggles.
        assert_eq!(t.on_event(&click(400.0, 20.0)), EventResult::Consumed);
        assert!(!t.game.music_on && !t.game.sfx_on);
    }
}
