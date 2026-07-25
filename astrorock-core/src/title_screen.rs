//! # Title screen — attract-mode composition
//!
//! Recreates `AstroRock.cpp`'s `DrawFrame` teaser state, now animated
//! at the original 30 Hz: the 50-star field plotted in world space,
//! level-1 rocks drifting and tumbling through the wrapping world, and
//! the ASTROROCK logo blitted centered with `RedBlit`
//! (`BLIT_TRANS_REMAP_BG` ≡ `BLIT_REMAP_DEST_ON_1` with the
//! `rTransRedPal` table — source index 1 tints the background). The
//! composed 640x480 indexed buffer converts through the game palette
//! and presents via agg-gui, aspect-fit with letterboxing.

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult};
use agg_gui::geometry::{Rect as GuiRect, Size};
use agg_gui::widget::Widget;
use web_time::Instant;

use crate::assets;
use crate::events::Events;
use crate::explosion::Explosions;
use crate::frame::{BlitMode, Frame};
use crate::heartbeat::HeartBeat;
use crate::palette::Palette;
use crate::rand::Rand;
use crate::rect::Rect;
use crate::rocks::Rocks;
use crate::virtual_frame::VirtualFrame;

/// `#define NUMSTARS 50`
const NUM_STARS: usize = 50;
/// Back-buffer size (`SetTo640X480X8`).
pub const SCREEN_W: i32 = 640;
pub const SCREEN_H: i32 = 480;
/// World size (`CVirtualFrame PlayScreen1(2048, 1024)`).
pub const WORLD_W: i32 = 2048;
pub const WORLD_H: i32 = 1024;

pub struct TitleScreen {
    bounds: GuiRect,
    children: Vec<Box<dyn Widget>>,
    screen: Frame,
    world: VirtualFrame,
    palette: Palette,
    teaser: Frame,
    transred: [u8; 256],
    stars: Vec<(i32, i32)>,
    rocks: Rocks,
    explosions: Explosions,
    events: Events,
    net_rand: Rand,
    heartbeat: HeartBeat,
    started: Instant,
    rgba: Vec<u8>,
}

impl TitleScreen {
    pub fn new() -> Self {
        // `STARInit`: NetRand(2048) x NetRand(1024); NetRand default-
        // constructs with seed 0, so the field matches the original's
        // first frame.
        let mut net_rand = Rand::new();
        let stars = (0..NUM_STARS)
            .map(|_| {
                let x = net_rand.rand(WORLD_W as u32) as i32;
                let y = net_rand.rand(WORLD_H as u32) as i32;
                (x, y)
            })
            .collect();

        let mut world = VirtualFrame::new(WORLD_W, WORLD_H);
        world.set_on_screen_rect(Rect::new(0, 0, SCREEN_W, SCREEN_H));
        world.move_point_to_center(WORLD_W / 2, WORLD_H / 2);

        let mut rocks = Rocks::new();
        rocks.reset(0, &mut net_rand);

        Self {
            bounds: GuiRect::default(),
            children: Vec::new(),
            screen: Frame::new(SCREEN_W, SCREEN_H),
            world,
            palette: assets::game_palette(),
            teaser: assets::frame_from_indexed_png(assets::TEASER_PNG),
            transred: assets::remap_table(assets::TRANSRED_PAL),
            stars,
            rocks,
            explosions: Explosions::new(),
            events: Events::new(),
            net_rand,
            heartbeat: HeartBeat::new(0),
            started: Instant::now(),
            rgba: Vec::new(),
        }
    }

    /// Run the 30 Hz simulation up to `now_ms`. Separate from `paint`
    /// so tests can step it headless.
    pub fn advance(&mut self, now_ms: u64) {
        let clip = Rect::new(0, 0, WORLD_W, WORLD_H);
        let beats = self.heartbeat.read_and_clear(now_ms);
        for _ in 0..beats {
            self.rocks.update(&clip, &mut self.net_rand);
            self.explosions.update(&clip, &mut self.net_rand);
        }
        // No audio sink yet — drain so the queue can't grow unbounded.
        for _ in self.events.drain() {}
    }

    /// Compose one frame into the indexed back buffer.
    pub fn compose(&mut self) {
        self.screen.erase(&Rect::new(0, 0, SCREEN_W, SCREEN_H));

        for &(x, y) in &self.stars {
            self.world.pset(&mut self.screen, x, y, 15);
        }

        self.rocks.draw(&self.world, &mut self.screen);
        self.explosions.draw(&self.world, &mut self.screen);

        // pScreen->Blit(&TeaserFrame, W/2 - tw/2, H/2 - th/2, &RedBlit)
        let teaser_bounds = self.teaser.bounds();
        self.screen.blit(
            &self.teaser,
            &teaser_bounds,
            SCREEN_W / 2 - self.teaser.width / 2,
            SCREEN_H / 2 - self.teaser.height / 2,
            BlitMode::RemapDestOn1(&self.transred),
        );
    }

    /// The composed indexed back buffer (tests + the `dump_frame`
    /// inspection example).
    pub fn screen(&self) -> &Frame {
        &self.screen
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

    fn layout(&mut self, available: Size) -> Size {
        available
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        let now_ms = self.started.elapsed().as_millis() as u64;
        self.advance(now_ms);
        self.compose();
        // Keep the attract loop animating.
        agg_gui::animation::request_draw_without_invalidation();
        self.palette.frame_to_rgba(&self.screen, &mut self.rgba);

        // Letterbox: aspect-fit the 640x480 game surface in the window.
        let (w, h) = (self.bounds.width, self.bounds.height);
        ctx.set_fill_color(Color::from_rgb8(0, 0, 0));
        ctx.begin_path();
        ctx.rect(0.0, 0.0, w, h);
        ctx.fill();

        let scale = (w / SCREEN_W as f64).min(h / SCREEN_H as f64);
        let dw = SCREEN_W as f64 * scale;
        let dh = SCREEN_H as f64 * scale;
        let dx = (w - dw) * 0.5;
        let dy = (h - dh) * 0.5;
        ctx.draw_image_rgba(&self.rgba, SCREEN_W as u32, SCREEN_H as u32, dx, dy, dw, dh);
    }

    fn on_event(&mut self, _event: &Event) -> EventResult {
        EventResult::Ignored
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composes_stars_and_centered_teaser() {
        let mut t = TitleScreen::new();
        t.compose();
        let screen = t.screen();

        // Star pixels (color 15) landed somewhere.
        let stars = screen.bits.iter().filter(|&&b| b == 15).count();
        assert!(stars > 0, "no stars plotted");
        assert!(
            stars <= NUM_STARS,
            "stars = {stars} (some may overlap/occlude)"
        );

        // The teaser occupies the center: some non-zero pixel inside
        // the centered rect that isn't from the star field alone.
        let cx = SCREEN_W / 2;
        let cy = SCREEN_H / 2;
        let mut non_zero = 0;
        for y in (cy - 100)..(cy + 100) {
            for x in (cx - 140)..(cx + 140) {
                if screen.get(x, y) != 0 {
                    non_zero += 1;
                }
            }
        }
        assert!(
            non_zero > 1000,
            "teaser not composed, center pixels = {non_zero}"
        );
    }

    #[test]
    fn star_field_is_deterministic() {
        let a = TitleScreen::new();
        let b = TitleScreen::new();
        assert_eq!(a.stars, b.stars);
        // Seed-0 NetRand first draws, from the locked RNG: x=NetRand(2048).
        let mut r = Rand::new();
        let x0 = r.rand(2048) as i32;
        let y0 = r.rand(1024) as i32;
        assert_eq!(a.stars[0], (x0, y0));
    }
}
