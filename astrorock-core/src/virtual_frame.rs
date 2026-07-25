//! # Wrapping world coordinates — port of `VirtualFrame.cpp`
//!
//! `CVirtualFrame` has **no bitmap** ("A virtual frame class (it has no
//! bitmap)"): it is a coordinate transformer from torus world space
//! (2048x1024 in AstroRock) to the 640x480 back buffer, relative to a
//! camera at `cur_x`/`cur_y`. Each draw maps the world point to the
//! nearest wrap image around the screen center, then clips to
//! `on_screen_rect` and blits into the destination frame.

use crate::fixed_trig;
use crate::frame::{BlitMode, Frame};
use crate::rect::{frame_clip_rect, frame_clip_rects, Rect};

pub struct VirtualFrame {
    pub width: i32,
    pub height: i32,
    pub cur_x: i32,
    pub cur_y: i32,
    pub on_screen_rect: Rect,
}

impl VirtualFrame {
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            cur_x: 0,
            cur_y: 0,
            on_screen_rect: Rect::default(),
        }
    }

    pub fn set_on_screen_rect(&mut self, rect: Rect) {
        self.on_screen_rect = rect;
    }

    /// `GetPosRelCenter` — world point → offset from the screen center,
    /// folded to the nearest wrap image.
    pub fn pos_rel_center(&self, x: i32, y: i32) -> (i32, i32) {
        let mut x = x - self.cur_x - self.on_screen_rect.width() / 2;
        let mut y = y - self.cur_y - self.on_screen_rect.height() / 2;

        let half = self.width / 2;
        if x > half {
            x -= self.width;
        }
        if x < -half {
            x += self.width;
        }
        let half = self.height / 2;
        if y > half {
            y -= self.height;
        }
        if y < -half {
            y += self.height;
        }
        (x, y)
    }

    /// `GetMapToScreen` — world point → back-buffer coordinates.
    pub fn map_to_screen(&self, x: i32, y: i32) -> (i32, i32) {
        let (x, y) = self.pos_rel_center(x, y);
        (
            x + self.on_screen_rect.left + self.on_screen_rect.width() / 2,
            y + self.on_screen_rect.top + self.on_screen_rect.height() / 2,
        )
    }

    /// `CVirtualFrame::PSet` — plot a world-space pixel if visible.
    pub fn pset(&self, dest: &mut Frame, x: i32, y: i32, color: u8) {
        let (sx, sy) = self.map_to_screen(x, y);
        if self.on_screen_rect.pt_in_rect(sx, sy) {
            dest.pset(sx, sy, color);
        }
    }

    /// `CVirtualFrame::Blit` — blit `source[src_rect]` with its world
    /// destination at (`world_x`, `world_y`), clipped to the on-screen
    /// window of `dest`.
    pub fn blit(
        &self,
        dest: &mut Frame,
        source: &Frame,
        src_rect: &Rect,
        world_x: i32,
        world_y: i32,
        mode: BlitMode,
    ) {
        let (sx, sy) = self.map_to_screen(world_x, world_y);
        let mut src = *src_rect;
        let mut dst = Rect::new(sx, sy, sx + src_rect.width(), sy + src_rect.height());
        if frame_clip_rects(&self.on_screen_rect, &mut src, &mut dst) {
            // Inner blit clips again against the frame — harmless, and
            // matches the original's pScreen->Blit call.
            dest.blit(source, &src, dst.left, dst.top, mode);
        }
    }

    /// `CVirtualFrame::Erase` — clear a world-space rect to color 0.
    pub fn erase(&self, dest: &mut Frame, rect: &Rect) {
        let (sx, sy) = self.map_to_screen(rect.left, rect.top);
        let mut dst = Rect::new(sx, sy, sx + rect.width(), sy + rect.height());
        if frame_clip_rect(&self.on_screen_rect, &mut dst) {
            dest.erase(&dst);
        }
    }

    /// `Scroll` — move the camera, wrapping into [0, size).
    pub fn scroll(&mut self, dx: i32, dy: i32) {
        let mut dx = dx + self.cur_x;
        let mut dy = dy + self.cur_y;
        if dx < 0 {
            dx += self.width;
        }
        if dx >= self.width {
            dx -= self.width;
        }
        if dy < 0 {
            dy += self.height;
        }
        if dy >= self.height {
            dy -= self.height;
        }
        self.cur_x = dx;
        self.cur_y = dy;
    }

    /// `MovePointToCenter` — center the camera on a world point.
    pub fn move_point_to_center(&mut self, x: i32, y: i32) {
        self.cur_x = x - self.on_screen_rect.width() / 2;
        self.cur_y = y - self.on_screen_rect.height() / 2;
        self.scroll(0, 0);
    }

    /// `FindPnt1RelPnt2` — nearest wrap image of `pnt2` to `pnt1`.
    fn nearest_wrap(&self, p1: (i32, i32), p2: (i32, i32)) -> (i32, i32) {
        let d2 = |dx: i32, dy: i32| {
            let ex = dx + p2.0 - p1.0;
            let ey = dy + p2.1 - p1.1;
            ex * ex + ey * ey
        };
        let mut best = p2;
        let mut best_d = d2(0, 0);
        for wy in -1..=1 {
            for wx in -1..=1 {
                let d = d2(self.width * wx, self.height * wy);
                if d < best_d {
                    best_d = d;
                    best = (self.width * wx + p2.0, self.height * wy + p2.1);
                }
            }
        }
        best
    }

    /// `FindAngle` — angle from p1 to p2 accounting for wrapping.
    pub fn find_angle(&self, p1: (i32, i32), p2: (i32, i32)) -> f32 {
        let rel = self.nearest_wrap(p1, p2);
        fixed_trig::atan_d_relative(p1.0, p1.1, rel.0, rel.1)
    }

    /// `FindDist` — distance from p1 to p2 accounting for wrapping.
    pub fn find_dist(&self, p1: (i32, i32), p2: (i32, i32)) -> f32 {
        let rel = self.nearest_wrap(p1, p2);
        fixed_trig::distance(p1.0, p1.1, rel.0, rel.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> VirtualFrame {
        let mut v = VirtualFrame::new(2048, 1024);
        v.set_on_screen_rect(Rect::new(0, 0, 640, 480));
        v
    }

    #[test]
    fn centered_point_maps_to_screen_center() {
        let mut v = world();
        v.move_point_to_center(1000, 500);
        assert_eq!(v.map_to_screen(1000, 500), (320, 240));
        // A point 100 right of the camera target sits 100 right of center.
        assert_eq!(v.map_to_screen(1100, 500), (420, 240));
    }

    #[test]
    fn wrapping_picks_nearest_image() {
        let mut v = world();
        v.move_point_to_center(10, 10);
        // A point at the far right edge is really just left of the camera.
        let (x, _) = v.map_to_screen(2040, 10);
        assert_eq!(x, 320 - 18);
        // And straight-line distance honors the wrap.
        let d = v.find_dist((10, 10), (2040, 10));
        assert!((d - 18.0).abs() < 0.5, "wrapped dist = {d}");
    }

    #[test]
    fn scroll_wraps_camera() {
        let mut v = world();
        v.scroll(-10, -10);
        assert_eq!((v.cur_x, v.cur_y), (2038, 1014));
        v.scroll(20, 20);
        assert_eq!((v.cur_x, v.cur_y), (10, 10));
    }

    #[test]
    fn blit_lands_on_back_buffer() {
        let mut v = world();
        v.move_point_to_center(1000, 500);
        let mut screen = Frame::new(640, 480);
        let sprite = Frame::from_bits(2, 2, vec![5, 5, 5, 5]);
        v.blit(
            &mut screen,
            &sprite,
            &sprite.bounds(),
            1000,
            500,
            BlitMode::Transparent0,
        );
        assert_eq!(screen.get(320, 240), 5);
        assert_eq!(screen.get(321, 241), 5);
        assert_eq!(screen.get(322, 240), 0);
    }
}
