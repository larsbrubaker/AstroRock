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
}

impl TitleScreen {
    pub fn new() -> Self {
        Self::new_with_audio(None)
    }

    pub fn new_with_audio(audio: Option<Box<dyn AudioSink>>) -> Self {
        Self {
            bounds: GuiRect::default(),
            children: Vec::new(),
            game: Game::new(audio),
            rgba: Vec::new(),
            icons: Arc::new(Font::from_slice(ICON_FONT_BYTES).expect("fa.ttf parses")),
            music_btn: GuiRect::default(),
            sfx_btn: GuiRect::default(),
            fullscreen_btn: GuiRect::default(),
        }
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
        self.game.advance(self.game.now_ms());
        self.game.compose();
        // Keep the loop animating — content changes every frame, so the
        // full request_draw (animation.rs: "animation ticks … must call
        // request_draw").
        agg_gui::animation::request_draw();
        self.game
            .palette
            .frame_to_rgba(self.game.screen(), &mut self.rgba);

        // Window chrome (chrome.rs): backdrop, rail/bar, buttons, and
        // the frame; it returns where the game surface goes.
        let (w, h) = (self.bounds.width, self.bounds.height);
        let layout = chrome::paint(ctx, w, h, self.game.music_on, self.game.sfx_on, &self.icons);
        self.music_btn = layout.music_btn;
        self.sfx_btn = layout.sfx_btn;
        self.fullscreen_btn = layout.fullscreen_btn;
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
                    self.game.music_on = !self.game.music_on;
                    EventResult::Consumed
                } else if chrome::hit(&self.sfx_btn, pos.x, pos.y) {
                    self.game.sfx_on = !self.game.sfx_on;
                    EventResult::Consumed
                } else if chrome::hit(&self.fullscreen_btn, pos.x, pos.y) {
                    agg_gui::fullscreen::request_toggle();
                    EventResult::Consumed
                } else {
                    EventResult::Ignored
                }
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
        // A click outside both buttons is ignored.
        assert_eq!(t.on_event(&click(400.0, 20.0)), EventResult::Ignored);
    }
}
