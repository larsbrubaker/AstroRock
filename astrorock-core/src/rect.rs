//! # Rectangles and blit clipping — port of `Rect.hpp` + `Frame.cpp`'s
//! `FrameClipRects`/`FrameClipRect`
//!
//! Half-open rects (`right`/`bottom` exclusive), i32 like the original.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }

    pub fn pt_in_rect(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    /// Offset the whole rect.
    pub fn offset(&mut self, dx: i32, dy: i32) {
        self.left += dx;
        self.top += dy;
        self.right += dx;
        self.bottom += dy;
    }
}

/// `FrameClipRects` — clip `dst` to `bound`, adjusting `src` by the same
/// amounts. Returns false when nothing is left to draw.
pub fn frame_clip_rects(bound: &Rect, src: &mut Rect, dst: &mut Rect) -> bool {
    if dst.top < bound.top {
        src.top += bound.top - dst.top;
        dst.top = bound.top;
        if dst.top >= dst.bottom {
            return false;
        }
    }
    if dst.bottom > bound.bottom {
        src.bottom -= dst.bottom - bound.bottom;
        dst.bottom = bound.bottom;
        if dst.bottom <= dst.top {
            return false;
        }
    }
    if dst.left < bound.left {
        src.left += bound.left - dst.left;
        dst.left = bound.left;
        if dst.left >= dst.right {
            return false;
        }
    }
    if dst.right > bound.right {
        src.right -= dst.right - bound.right;
        dst.right = bound.right;
        if dst.right <= dst.left {
            return false;
        }
    }
    true
}

/// `FrameClipRect` — clip a single rect to `bound`.
pub fn frame_clip_rect(bound: &Rect, rect: &mut Rect) -> bool {
    if rect.top < bound.top {
        rect.top = bound.top;
        if rect.top >= rect.bottom {
            return false;
        }
    }
    if rect.bottom > bound.bottom {
        rect.bottom = bound.bottom;
        if rect.bottom <= rect.top {
            return false;
        }
    }
    if rect.left < bound.left {
        rect.left = bound.left;
        if rect.left >= rect.right {
            return false;
        }
    }
    if rect.right > bound.right {
        rect.right = bound.right;
        if rect.right <= rect.left {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_adjusts_src_in_lockstep() {
        let bound = Rect::new(0, 0, 640, 480);
        let mut src = Rect::new(0, 0, 40, 30);
        let mut dst = Rect::new(-10, -5, 30, 25);
        assert!(frame_clip_rects(&bound, &mut src, &mut dst));
        assert_eq!(dst, Rect::new(0, 0, 30, 25));
        assert_eq!(src, Rect::new(10, 5, 40, 30));
    }

    #[test]
    fn clip_right_bottom_shrinks_src() {
        let bound = Rect::new(0, 0, 100, 100);
        let mut src = Rect::new(0, 0, 40, 30);
        let mut dst = Rect::new(80, 90, 120, 120);
        assert!(frame_clip_rects(&bound, &mut src, &mut dst));
        assert_eq!(dst, Rect::new(80, 90, 100, 100));
        assert_eq!(src, Rect::new(0, 0, 20, 10));
    }

    #[test]
    fn fully_outside_returns_false() {
        let bound = Rect::new(0, 0, 100, 100);
        let mut src = Rect::new(0, 0, 10, 10);
        let mut dst = Rect::new(200, 0, 210, 10);
        assert!(!frame_clip_rects(&bound, &mut src, &mut dst));
        let mut r = Rect::new(-20, -20, -10, -10);
        assert!(!frame_clip_rect(&bound, &mut r));
    }
}
