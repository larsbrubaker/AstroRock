//! # Title Screen (Phase 0 placeholder)
//!
//! Full-window widget that proves the render pipeline end to end on both
//! targets: a deterministic star field and the game title, all painted
//! through agg-gui's [`DrawCtx`]. Replaced by the real `StartScreen` port
//! in the UI phase; the star-field concept carries over (the original
//! game draws one behind the world too).

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult};
use agg_gui::geometry::{Rect, Size};
use agg_gui::text::Font;
use agg_gui::widget::Widget;

/// Number of background stars on the title screen.
const STAR_COUNT: u32 = 220;

pub struct TitleScreen {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    font: Arc<Font>,
}

impl TitleScreen {
    pub fn new(font: Arc<Font>) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            font,
        }
    }
}

/// Deterministic per-star pseudo-random value in `[0, 1)`.
///
/// A fixed integer hash (not `rand`) so the star field is identical on
/// every run and every platform — matching the project rule that anything
/// visual derives from deterministic state.
fn star_param(index: u32, salt: u32) -> f64 {
    let mut h = index
        .wrapping_mul(2654435761)
        .wrapping_add(salt.wrapping_mul(0x9E3779B9));
    h ^= h >> 16;
    h = h.wrapping_mul(0x8546_5549);
    h ^= h >> 13;
    (h as f64) / (u32::MAX as f64)
}

impl Widget for TitleScreen {
    fn type_name(&self) -> &'static str {
        "TitleScreen"
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Box<dyn Widget>> {
        &mut self.children
    }

    fn layout(&mut self, available: Size) -> Size {
        // Fill the window — the title screen owns the whole surface.
        available
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        let w = self.bounds.width;
        let h = self.bounds.height;

        // Space-black backdrop.
        ctx.set_fill_color(Color::from_rgb8(4, 4, 12));
        ctx.begin_path();
        ctx.rect(0.0, 0.0, w, h);
        ctx.fill();

        // Deterministic star field: three brightness tiers.
        for i in 0..STAR_COUNT {
            let x = star_param(i, 1) * w;
            let y = star_param(i, 2) * h;
            let tier = star_param(i, 3);
            let (radius, level) = if tier > 0.92 {
                (1.6, 235)
            } else if tier > 0.65 {
                (1.1, 170)
            } else {
                (0.7, 100)
            };
            ctx.set_fill_color(Color::from_rgb8(level, level, level));
            ctx.begin_path();
            ctx.circle(x, y, radius);
            ctx.fill();
        }

        // Title + status line, centered. DrawCtx text coordinates are
        // baseline-anchored in the widget's local Y-up space.
        ctx.set_font(Arc::clone(&self.font));

        let title = "ASTROROCK";
        let title_size = (w * 0.09).clamp(32.0, 96.0);
        ctx.set_font_size(title_size);
        let title_width = ctx.measure_text(title).map(|m| m.width).unwrap_or(0.0);
        ctx.set_fill_color(Color::from_rgb8(240, 240, 250));
        ctx.fill_text(title, (w - title_width) * 0.5, h * 0.55);

        let status = "Rust port under construction";
        let status_size = (title_size * 0.28).max(14.0);
        ctx.set_font_size(status_size);
        let status_width = ctx.measure_text(status).map(|m| m.width).unwrap_or(0.0);
        ctx.set_fill_color(Color::from_rgb8(140, 150, 180));
        ctx.fill_text(
            status,
            (w - status_width) * 0.5,
            h * 0.55 - title_size * 0.9,
        );
    }

    fn on_event(&mut self, _event: &Event) -> EventResult {
        EventResult::Ignored
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The star hash must be deterministic (same value for the same
    /// index/salt) and stay inside [0, 1) so stars land on-screen.
    #[test]
    fn star_param_is_deterministic_and_unit_range() {
        for i in 0..STAR_COUNT {
            for salt in 1..4 {
                let a = star_param(i, salt);
                let b = star_param(i, salt);
                assert_eq!(a, b, "star_param must be pure (index {i}, salt {salt})");
                assert!((0.0..1.0).contains(&a), "out of range: {a}");
            }
        }
    }

    /// Title screen claims the full window so the backdrop has no gaps.
    #[test]
    fn layout_fills_available_space() {
        let mut screen = TitleScreen::new(crate::load_default_font());
        let size = screen.layout(Size::new(640.0, 480.0));
        assert_eq!(size.width, 640.0);
        assert_eq!(size.height, 480.0);
    }
}
