//! # Radar — port of `radar.cpp`
//!
//! A 128x64 off-screen frame: sprites plot as single pixels (world
//! position relative to the camera, >> 4), the overlay art stamps the
//! center, the whole thing blits to the screen, then clears for the
//! next frame.

use crate::assets;
use crate::frame::{BlitMode, Frame};
use crate::rect::Rect;
use crate::sprite::Sprite;
use crate::virtual_frame::VirtualFrame;

const RAYOVER_PNG: &[u8] = include_bytes!("../../assets/interfac/rayover.png");

pub struct Radar {
    frame: Frame,
    overlay: Frame,
    over_x: i32,
    over_y: i32,
    xc: i32,
    yc: i32,
}

impl Radar {
    /// `RadarInit`.
    pub fn new() -> Self {
        let overlay = assets::frame_from_indexed_png(RAYOVER_PNG);
        let frame = Frame::new(128, 64);
        let over_x = (frame.width - overlay.width) / 2 + 1;
        let over_y = (frame.height - overlay.height) / 2 + 1;
        let xc = frame.width / 2;
        let yc = frame.height / 2;
        Self {
            frame,
            overlay,
            over_x,
            over_y,
            xc,
            yc,
        }
    }

    /// `RadarDrawOn` — plot a sprite as one pixel.
    pub fn plot(&mut self, sprite: &Sprite, color: u8, world: &VirtualFrame) {
        if sprite.visible {
            let (rx, ry) = world.pos_rel_center(sprite.x_pos as i32, sprite.y_pos as i32);
            self.frame
                .pset((rx >> 4) + self.xc, (ry >> 4) + self.yc, color);
        }
    }

    /// `RadarDraw` — stamp overlay, blit to screen at (x, y), clear.
    pub fn draw(&mut self, screen: &mut Frame, x: i32, y: i32) {
        let overlay_bounds = self.overlay.bounds();
        self.frame.blit(
            &self.overlay,
            &overlay_bounds,
            self.over_x,
            self.over_y,
            BlitMode::Transparent0,
        );
        let radar_bounds = self.frame.bounds();
        screen.blit(&self.frame, &radar_bounds, x, y, BlitMode::Normal);
        let all = Rect::new(0, 0, self.frame.width, self.frame.height);
        self.frame.erase(&all);
    }
}

impl Default for Radar {
    fn default() -> Self {
        Self::new()
    }
}

/// The `DrawPlayField` radar-collection pass: every live object plots
/// with its shipped blip color (rocks 15/145/147, gloops 104, plus the
/// per-enemy `*_RADAR_COLOR`s and the player at 160).
#[allow(clippy::too_many_arguments)]
pub fn plot_world(
    radar: &mut Radar,
    world: &VirtualFrame,
    rocks: &crate::rocks::Rocks,
    gloops: &crate::gloops::Gloops,
    hks: &crate::hks::Hks,
    bombers: &crate::bombers::Bombers,
    spikeballs: &crate::spikeballs::SpikeBalls,
    fastdeaths: &crate::fastdeaths::FastDeaths,
    ship: &Sprite,
) {
    for s in rocks.big() {
        radar.plot(s, 15, world);
    }
    for s in rocks.med() {
        radar.plot(s, 145, world);
    }
    for s in rocks.lit() {
        radar.plot(s, 147, world);
    }
    if gloops.active() {
        for s in gloops.pool() {
            radar.plot(s, 104, world);
        }
    }
    if hks.active() {
        for s in hks.pool() {
            radar.plot(s, crate::hks::HK_RADAR_COLOR, world);
        }
    }
    if bombers.active() {
        for s in bombers.pool() {
            radar.plot(s, crate::bombers::BOMBER_RADAR_COLOR, world);
        }
    }
    if spikeballs.active() {
        for s in spikeballs.pool() {
            radar.plot(s, crate::spikeballs::SPIKEBALL_RADAR_COLOR, world);
        }
    }
    for s in fastdeaths.pool() {
        radar.plot(s, crate::fastdeaths::FAST_DEATH_RADAR_COLOR, world);
    }
    radar.plot(ship, 160, world);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence;

    #[test]
    fn plots_relative_to_camera_and_clears_after_draw() {
        let mut world = VirtualFrame::new(2048, 1024);
        world.set_on_screen_rect(Rect::new(0, 0, 640, 480));
        world.move_point_to_center(1024, 512);

        let mut radar = Radar::new();
        let mut s = Sprite::new();
        s.set_sequence(sequence::ast_small());
        // 160 world px right of camera center -> 10 radar px right.
        s.x_pos = 1024.0 + 160.0;
        s.y_pos = 512.0;
        radar.plot(&s, 15, &world);
        assert_eq!(radar.frame.get(64 + 10, 32), 15);

        let mut screen = Frame::new(640, 480);
        radar.draw(&mut screen, 5, 5);
        // The plotted pixel arrived on screen (offset by radar origin).
        assert_eq!(screen.get(5 + 64 + 10, 5 + 32), 15);
        // And the working radar frame cleared for the next frame.
        assert_eq!(radar.frame.get(64 + 10, 32), 0);
    }
}
