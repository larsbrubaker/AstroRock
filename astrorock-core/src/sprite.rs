//! # Sprite — port of `CSprite` (`sprite.cpp`)
//!
//! Position/velocity/frame-index state in f32 (the shipped `CFixed`),
//! wrapping against the world-sized clip rect, per-update animation
//! advance, timeout lifetime, pixel-level collision, and the f32
//! checksum feeding demo/net sync.
//!
//! Two faithful quirks, preserved on purpose (demo checksums and
//! collision outcomes depend on them):
//!
//! - `Draw` computes a rotation-adjusted `RenderFrame` and then indexes
//!   with plain `TOINT(CurFrame)` anyway — rotations never affect which
//!   frame is drawn (all shipped sprites have `NumRotations == 1`).
//! - `CollideOnBits` misparenthesizes the second pixel test:
//!   `(*(pBits) + YTable[y] + (x + left)) != BLACK` adds the *first*
//!   pixel byte to the offsets instead of dereferencing at the offset,
//!   so "this" sprite's mask is effectively always solid. Collision is
//!   really "other frame has a non-black pixel in the overlap".

use std::rc::Rc;

use crate::frame::{BlitMode, Frame};
use crate::rand::Rand;
use crate::rect::Rect;
use crate::sequence::FrameSequence;
use crate::virtual_frame::VirtualFrame;

/// `CBlitType` for sprites — owns/shares its lookup tables. `to_mode`
/// borrows them as a [`BlitMode`] for the blit engine.
#[derive(Clone, Default)]
pub enum SpriteBlit {
    #[default]
    Trans,
    TransReverse,
    RemapSource(Rc<[u8; 256]>),
    RemapDestOn1(Rc<[u8; 256]>),
    Combine64K(Rc<[u8; 65536]>),
    Combine64KReverse(Rc<[u8; 65536]>),
}

impl SpriteBlit {
    pub fn to_mode(&self) -> BlitMode<'_> {
        match self {
            SpriteBlit::Trans => BlitMode::Transparent0,
            SpriteBlit::TransReverse => BlitMode::Transparent0Reverse,
            SpriteBlit::RemapSource(t) => BlitMode::RemapSource(t),
            SpriteBlit::RemapDestOn1(t) => BlitMode::RemapDestOn1(t),
            SpriteBlit::Combine64K(t) => BlitMode::Combine64K(t),
            SpriteBlit::Combine64KReverse(t) => BlitMode::Combine64KReverse(t),
        }
    }

    pub fn is_reverse(&self) -> bool {
        matches!(
            self,
            SpriteBlit::TransReverse | SpriteBlit::Combine64KReverse(_)
        )
    }
}

/// AI hook (`CSpriteAI`). Concrete behaviors arrive with the enemy
/// systems; the base class triggers every update and does nothing.
pub trait SpriteAi {
    /// `CSpriteAI::Update` — mutate the sprite each update. Returns
    /// whether the AI "triggered" (base class: always true).
    fn update(&mut self, sprite: &mut Sprite, rand: &mut Rand) -> bool;
}

/// What `Sprite::update` asks its owner to do afterwards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateResult {
    Live,
    /// Timeout expired — `m_pListIn->Destroy(this)`.
    Destroy,
}

#[derive(Default)]
pub struct Sprite {
    pub frame_advance: f32,
    pub cur_frame: f32,
    pub cur_rotation: u32,
    pub blit: SpriteBlit,
    pub hp: u32,
    pub x_delta: f32,
    pub y_delta: f32,
    pub x_pos: f32,
    pub y_pos: f32,
    pub visible: bool,
    /// Updates until self-destruction; 0 = never times out.
    pub duration: u32,
    num_moved: u32,
    pub ai: Option<Box<dyn SpriteAi>>,
    sequence: Option<Rc<FrameSequence>>,
}

impl Sprite {
    /// `CSprite::CSprite` + `Reset`.
    pub fn new() -> Self {
        Self {
            frame_advance: 1.0,
            cur_frame: 0.0,
            cur_rotation: 0,
            blit: SpriteBlit::Trans,
            hp: 0,
            x_delta: 1.0,
            y_delta: 1.0,
            x_pos: 0.0,
            y_pos: 0.0,
            visible: true,
            duration: 0,
            num_moved: 0,
            ai: None,
            sequence: None,
        }
    }

    /// `Reset` — position/delta/visibility back to defaults.
    pub fn reset(&mut self) {
        self.hp = 0;
        self.visible = true;
        self.frame_advance = 1.0;
        self.cur_frame = 0.0;
        self.x_pos = 0.0;
        self.y_pos = 0.0;
        self.x_delta = 1.0;
        self.y_delta = 1.0;
    }

    pub fn set_sequence(&mut self, seq: Rc<FrameSequence>) {
        self.sequence = Some(seq);
    }

    pub fn sequence(&self) -> Option<&Rc<FrameSequence>> {
        self.sequence.as_ref()
    }

    fn seq(&self) -> &FrameSequence {
        self.sequence.as_ref().expect("sprite has a sequence")
    }

    /// Current frame's art (by the original's `TOINT(CurFrame)` index).
    pub fn current_art(&self) -> &Frame {
        &self.seq().frames[self.cur_frame as i32 as usize]
    }

    /// `CSprite::Draw` through the wrapping world onto the back buffer.
    pub fn draw(&self, world: &VirtualFrame, screen: &mut Frame) {
        let seq = self.seq();
        if seq.num_frames == 0 || !self.visible {
            return;
        }
        let index = self.cur_frame as i32;
        if index < 0 || index as u32 >= seq.num_frames {
            return; // original logs "non-existant frame" and skips
        }
        let art = &seq.frames[index as usize];
        let (tlx, tly) =
            art.dest_top_left(self.x_pos as i32, self.y_pos as i32, self.blit.is_reverse());
        world.blit(screen, art, &art.bounds(), tlx, tly, self.blit.to_mode());
    }

    /// `CSprite::Erase` — clear the sprite's rect on the target.
    pub fn erase(&self, world: &VirtualFrame, screen: &mut Frame) {
        let seq = self.seq();
        if !self.visible || seq.num_frames == 0 {
            return;
        }
        let index = self.cur_frame as i32;
        if index < 0 || index as u32 >= seq.num_frames {
            return;
        }
        let art = &seq.frames[index as usize];
        let (tlx, tly) = art.dest_top_left(self.x_pos as i32, self.y_pos as i32, false);
        world.erase(
            screen,
            &Rect::new(tlx, tly, tlx + art.width, tly + art.height),
        );
    }

    /// `CSprite::Update` — AI, movement, wrap, animation. `clip` is
    /// `CSprite::SpriteClipRect` (world bounds during play).
    pub fn update(&mut self, clip: &Rect, rand: &mut Rand) -> UpdateResult {
        if !self.visible {
            return UpdateResult::Live;
        }

        if self.duration != 0 {
            self.num_moved += 1;
            if self.num_moved > self.duration {
                self.num_moved = 0;
                return UpdateResult::Destroy;
            }
        }

        // AI runs with the sprite borrowed mutably — take the chain out
        // for the call (the original just aliased through `this`).
        if let Some(mut ai) = self.ai.take() {
            ai.update(self, rand);
            if self.ai.is_none() {
                self.ai = Some(ai);
            }
        }

        self.x_pos += self.x_delta;
        self.y_pos += self.y_delta;

        // USE_WRAPPING is a mandatory define — right/bottom double as
        // the wrap modulus (left/top are 0 in play).
        while self.x_pos >= clip.right as f32 {
            self.x_pos -= clip.right as f32;
        }
        while self.x_pos < clip.left as f32 {
            self.x_pos += clip.right as f32;
        }
        while self.y_pos >= clip.bottom as f32 {
            self.y_pos -= clip.bottom as f32;
        }
        while self.y_pos < clip.top as f32 {
            self.y_pos += clip.bottom as f32;
        }

        let seq = self.sequence.as_ref().expect("sprite has a sequence");
        if seq.num_frames != 0 {
            let frames_per_rotation = seq.frames_per_rotation();
            self.cur_frame += self.frame_advance;
            if self.cur_frame as i32 >= frames_per_rotation as i32 {
                self.cur_frame -= frames_per_rotation as f32;
            }
            if (self.cur_frame as i32) < 0 {
                self.cur_frame += frames_per_rotation as f32;
            }
        }

        UpdateResult::Live
    }

    /// `CSprite::Check` — f32 checksum in the exact accumulation order.
    pub fn check(&self, include_non_visible: bool) -> f32 {
        let seq = self.seq();
        let mut checksum = 0.0f32;
        if seq.num_frames != 0 && (self.visible || include_non_visible) {
            checksum += self.hp as f32
                + self.frame_advance
                + self.cur_frame
                + self.x_pos
                + self.y_pos
                + self.x_delta
                + self.y_delta;
        }
        checksum
    }

    /// `Collide(CRect*)` — bounding box against a rect.
    pub fn collide_rect(&self, rect: &Rect) -> bool {
        if !self.visible {
            return false;
        }
        let art = self.current_art();
        let x = self.x_pos as i32 - art.hot_x;
        let y = self.y_pos as i32 - art.hot_y;
        let mine = Rect::new(x, y, x + art.width, y + art.height);
        intersect(&mine, rect).is_some()
    }

    /// `Collide(CSprite*)` — pixel-level against another sprite.
    pub fn collide_sprite(&self, other: &Sprite, clip: &Rect) -> bool {
        if !(self.visible && other.visible) {
            return false;
        }
        let other_art = other.current_art();
        self.collide_frame(
            other_art,
            other.x_pos as i32 - other_art.hot_x,
            other.y_pos as i32 - other_art.hot_y,
            clip,
        )
    }

    /// `Collide(CFrame*, xOff, yOff)` — pixel-level against a frame
    /// placed at (x_off, y_off), including the wrap-around retry.
    pub fn collide_frame(&self, frame: &Frame, x_off: i32, y_off: i32, clip: &Rect) -> bool {
        let seq = self.seq();
        let index = self.cur_frame as i32;
        if index < 0 || index as u32 >= seq.num_frames {
            return false;
        }
        if !self.visible {
            return false;
        }
        let art = &seq.frames[index as usize];
        let (hsx, hsy) = (art.hot_x, art.hot_y);

        let mut t_rect = Rect::new(0, 0, art.width, art.height);
        let mut s_rect = Rect::new(0, 0, frame.width, frame.height);
        t_rect.offset(self.x_pos as i32 - hsx, self.y_pos as i32 - hsy);
        s_rect.offset(x_off, y_off);

        if let Some(overlap) = intersect(&t_rect, &s_rect) {
            let mut t = overlap;
            t.offset(-(self.x_pos as i32) + hsx, -(self.y_pos as i32) + hsy);
            let mut s = overlap;
            s.offset(-x_off, -y_off);
            return self.collide_on_bits(art, &t, frame, &s);
        }

        // Wrap-around retry: if either rect hangs over the world edge,
        // shift ours by a world span toward the other and re-test.
        let inside = |r: &Rect| {
            r.top >= clip.top
                && r.bottom <= clip.bottom
                && r.left >= clip.left
                && r.right <= clip.right
        };
        if inside(&t_rect) && inside(&s_rect) {
            return false;
        }

        let width = clip.width();
        let height = clip.height();
        let mut wtv = 0;
        let mut wth = 0;

        if t_rect.top > s_rect.bottom || t_rect.bottom < s_rect.top {
            if t_rect.top > s_rect.bottom {
                t_rect.offset(0, -height);
                wtv = -height;
            } else {
                t_rect.offset(0, height);
                wtv = height;
            }
        }
        if t_rect.left > s_rect.right || t_rect.right < s_rect.left {
            if t_rect.left > s_rect.right {
                t_rect.offset(-width, 0);
                wth = -width;
            } else {
                t_rect.offset(width, 0);
                wth = width;
            }
        }

        if let Some(overlap) = intersect(&t_rect, &s_rect) {
            let mut t = overlap;
            t.offset(
                -(self.x_pos as i32) + hsx - wth,
                -(self.y_pos as i32) + hsy - wtv,
            );
            let mut s = overlap;
            s.offset(-x_off, -y_off);
            return self.collide_on_bits(art, &t, frame, &s);
        }
        false
    }

    /// `CollideOnBits` — bug-faithful pixel test (see module docs).
    fn collide_on_bits(&self, art: &Frame, t_rect: &Rect, frame: &Frame, s_rect: &Rect) -> bool {
        for y in 0..(t_rect.bottom - t_rect.top) {
            for x in 0..(t_rect.right - t_rect.left) {
                let other = frame.bits[((y + s_rect.top) * frame.width + x + s_rect.left) as usize];
                if other != 0 {
                    // Original: (*(pBits) + YTable[y + top] + (x + left))
                    // != BLACK — first byte PLUS offsets, not a pixel
                    // fetch. Zero only in a vanishing corner case.
                    let quirk =
                        art.bits[0] as i32 + (y + t_rect.top) * art.width + (x + t_rect.left);
                    if quirk != 0 {
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// `CRect::IntersectRect` — Some(overlap) when non-empty.
fn intersect(a: &Rect, b: &Rect) -> Option<Rect> {
    let r = Rect::new(
        a.left.max(b.left),
        a.top.max(b.top),
        a.right.min(b.right),
        a.bottom.min(b.bottom),
    );
    (r.left < r.right && r.top < r.bottom).then_some(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence;

    fn world_clip() -> Rect {
        Rect::new(0, 0, 2048, 1024)
    }

    fn ship_sprite() -> Sprite {
        let mut s = Sprite::new();
        s.set_sequence(sequence::ship());
        s
    }

    #[test]
    fn update_moves_wraps_and_animates() {
        let mut rand = Rand::new();
        let mut s = ship_sprite();
        s.x_pos = 2047.5;
        s.y_pos = 0.5;
        s.x_delta = 1.0;
        s.y_delta = -1.0;
        assert_eq!(s.update(&world_clip(), &mut rand), UpdateResult::Live);
        // Wrapped both axes.
        assert!(s.x_pos < 2048.0 && s.x_pos >= 0.0, "x = {}", s.x_pos);
        assert!(s.y_pos >= 1023.0, "y = {}", s.y_pos);
        // Frame advanced 0 -> 1.
        assert_eq!(s.cur_frame as i32, 1);
    }

    #[test]
    fn frame_animation_wraps_at_rotation_length() {
        let mut rand = Rand::new();
        let mut s = ship_sprite();
        s.cur_frame = 31.5;
        s.frame_advance = 1.0;
        s.update(&world_clip(), &mut rand);
        assert!((s.cur_frame as i32) < 32, "frame = {}", s.cur_frame);
    }

    #[test]
    fn timeout_destroys_after_duration() {
        let mut rand = Rand::new();
        let mut s = ship_sprite();
        s.duration = 2;
        assert_eq!(s.update(&world_clip(), &mut rand), UpdateResult::Live);
        assert_eq!(s.update(&world_clip(), &mut rand), UpdateResult::Live);
        assert_eq!(s.update(&world_clip(), &mut rand), UpdateResult::Destroy);
    }

    #[test]
    fn checksum_matches_field_sum_order() {
        let mut s = ship_sprite();
        s.hp = 3;
        s.frame_advance = 0.5;
        s.cur_frame = 2.0;
        s.x_pos = 100.25;
        s.y_pos = 200.5;
        s.x_delta = -1.5;
        s.y_delta = 0.75;
        let expected = 3.0f32 + 0.5 + 2.0 + 100.25 + 200.5 + (-1.5) + 0.75;
        assert_eq!(s.check(false).to_bits(), expected.to_bits());
        // Invisible sprites contribute nothing unless included.
        s.visible = false;
        assert_eq!(s.check(false), 0.0);
        assert_eq!(s.check(true).to_bits(), expected.to_bits());
    }

    #[test]
    fn sprites_collide_when_overlapping() {
        let clip = world_clip();
        let mut a = ship_sprite();
        let mut b = ship_sprite();
        a.x_pos = 500.0;
        a.y_pos = 500.0;
        b.x_pos = 500.0;
        b.y_pos = 500.0;
        assert!(a.collide_sprite(&b, &clip));
        b.x_pos = 800.0;
        assert!(!a.collide_sprite(&b, &clip));
    }

    #[test]
    fn wrapped_sprites_still_collide_across_the_seam() {
        let clip = world_clip();
        let mut a = ship_sprite();
        let mut b = ship_sprite();
        // a at the far right edge, b just past the left edge — their
        // wrap images overlap.
        a.x_pos = 2046.0;
        a.y_pos = 500.0;
        b.x_pos = 2.0;
        b.y_pos = 500.0;
        assert!(a.collide_sprite(&b, &clip), "seam collision missed");
    }

    #[test]
    fn rect_collision_uses_hotspot_anchor() {
        let mut s = ship_sprite();
        s.x_pos = 100.0;
        s.y_pos = 100.0;
        assert!(s.collide_rect(&Rect::new(95, 95, 105, 105)));
        assert!(!s.collide_rect(&Rect::new(300, 300, 310, 310)));
    }
}
