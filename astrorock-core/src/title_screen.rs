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
    /// Mobile virtual-gamepad rects from the last paint — polled
    /// against the active fingers each frame.
    touch_layout: Option<chrome::TouchLayout>,
    /// Tilt steering asked for once (the shell handles permission).
    tilt_requested: bool,
    /// The calibrated rest plane: raw tilt at the last stick release
    /// (or the first reading). Steering is measured from here.
    stick_baseline: Option<(f64, f64)>,
    /// The finger driving the joystick: captured when a NEW press
    /// lands on the pad, and kept — wherever it drags — until that
    /// same press lifts (its release recalibrates the rest plane).
    stick_finger: Option<u64>,
    /// Finger ids seen last frame (distinguishes a fresh press on
    /// the pad from a finger sliding in from elsewhere).
    prev_fingers: Vec<u64>,
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
            touch_layout: None,
            tilt_requested: false,
            stick_baseline: None,
            stick_finger: None,
            prev_fingers: Vec::new(),
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
        // Mobile: virtual gamepad + tilt steering, polled per frame.
        let touch_mode = agg_gui::input_profile::is_mobile_touch();
        let touch_ui = if touch_mode {
            if !self.tilt_requested {
                agg_gui::tilt::request_enable();
                self.tilt_requested = true;
            }
            let fingers = agg_gui::touch_points::active();

            // The joystick captures its finger: a NEW press landing
            // on the pad steers — wherever it drags — until that
            // press lifts. Fingers sliding in from elsewhere are
            // ignored.
            let was_captured = self.stick_finger.is_some();
            if let Some(id) = self.stick_finger {
                if !fingers.iter().any(|p| p.id == id) {
                    self.stick_finger = None;
                }
            }
            if self.stick_finger.is_none() {
                if let Some(t) = &self.touch_layout {
                    self.stick_finger = fingers
                        .iter()
                        .find(|p| {
                            !self.prev_fingers.contains(&p.id)
                                && chrome::hit(&t.stick, p.pos.x, p.pos.y)
                        })
                        .map(|p| p.id);
                }
            }
            self.prev_fingers = fingers.iter().map(|p| p.id).collect();

            // Thumb vector in pad units (may exceed 1 when dragged
            // past the rim — that's full deflection, i.e. thrust).
            // Widget Y-up flips to the game's screen-down axis.
            let thumb = self.stick_finger.and_then(|id| {
                let t = self.touch_layout.as_ref()?;
                let p = fingers.iter().find(|p| p.id == id)?;
                let r = (t.stick.width / 2.0).max(1.0);
                Some((
                    (p.pos.x - (t.stick.x + r)) / r,
                    -((p.pos.y - (t.stick.y + t.stick.height / 2.0)) / r),
                ))
            });

            // Buttons hold while any NON-stick finger covers them.
            let held = match &self.touch_layout {
                Some(t) => {
                    let over = |r: &GuiRect| {
                        fingers.iter().any(|p| {
                            Some(p.id) != self.stick_finger && chrome::hit(r, p.pos.x, p.pos.y)
                        })
                    };
                    (over(&t.shield_btn), over(&t.fire_btn))
                }
                None => (false, false),
            };

            // Rest-plane calibration: the first sensor reading zeroes
            // the stick, and releasing the pad re-zeroes it to
            // however the phone is held right now.
            let raw = agg_gui::tilt::reading();
            if let Some(raw) = raw {
                if self.stick_baseline.is_none() || (was_captured && self.stick_finger.is_none()) {
                    self.stick_baseline = Some(raw);
                }
            }

            // Steering, in degrees of lean: the thumb overrides tilt
            // (pad rim = full tilt deflection).
            let steer = match thumb {
                Some((vx, vy)) => Some((
                    vx * crate::joystick::MAX_TILT_DEG,
                    vy * crate::joystick::MAX_TILT_DEG,
                )),
                None => raw.map(|(x, y)| {
                    let b = self.stick_baseline.unwrap_or((0.0, 0.0));
                    (x - b.0, y - b.1)
                }),
            };
            self.game.set_tilt(steer);

            // No thrust button: driving the dot all the way to the
            // ring IS thrust — while turning too, since rotation and
            // thrust both read the same vector.
            let (pos, active, thrust) = match steer {
                Some((x, y)) => {
                    let mag = (x * x + y * y).sqrt();
                    (
                        (
                            x / crate::joystick::MAX_TILT_DEG,
                            y / crate::joystick::MAX_TILT_DEG,
                        ),
                        thumb.is_some() || mag >= crate::joystick::DEAD_ZONE_DEG,
                        mag >= crate::joystick::MAX_TILT_DEG * crate::joystick::THRUST_FRAC,
                    )
                }
                None => ((0.0, 0.0), false, false),
            };
            self.game.set_touch(crate::touch_input::TouchHeld {
                shield: held.0,
                fire: held.1,
                thrust,
            });
            Some(chrome::TouchUi {
                shield: held.0,
                fire: held.1,
                stick_pos: pos,
                stick_active: active,
            })
        } else {
            self.game.set_tilt(None);
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
