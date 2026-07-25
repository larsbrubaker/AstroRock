//! # 8-bit indexed frame + blit engine — port of `Frame.cpp`/`Blit.cpp`
//!
//! A `Frame` is a tightly packed byte buffer of palette indices
//! (scan width == width, matching `CFrame::Initialize`). The blit modes
//! are the uncompressed paths of the original:
//!
//! - `Normal` — copy all bytes
//! - `Transparent0` — skip source index 0 (`BlitBytesTransColor`)
//! - `Transparent0Reverse` — same, horizontally mirrored
//! - `RemapSource` — skip 0, write `remap[src]` (`BlitBytesTransCRemap`)
//! - `RemapDestOn1` — skip 0; source index 1 writes `remap[dest]`, any
//!   other index writes the source byte (`BlitBytesTransCRemapBC`)
//! - `Combine64K` — skip 0, write `table[(src << 8) + dest]`, the
//!   translucency path (`BlitBytesTransCRemapFCBC`); plus the reverse
//!
//! The original's RLE-compressed blit variants (`BlitCompressed*`) are
//! deliberately not ported: RLE was a 1997 CPU optimization with
//! identical pixel output, and the uncompressed loops are far beyond
//! fast enough for a 640x480 scene now.

use crate::rect::{frame_clip_rect, frame_clip_rects, Rect};

/// Which blit routine to use, mirroring `BlitTypes`. The remap/64K
/// variants carry their lookup table by reference at call time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlitMode<'a> {
    Normal,
    Transparent0,
    Transparent0Reverse,
    /// 256-entry table: `dest = remap[src]` where `src != 0`.
    RemapSource(&'a [u8; 256]),
    /// 256-entry table: source index 1 writes `remap[dest]`; any other
    /// non-zero source index writes the source byte.
    RemapDestOn1(&'a [u8; 256]),
    /// 64K table: `dest = table[(src << 8) + dest]` where `src != 0`.
    Combine64K(&'a [u8; 65536]),
    Combine64KReverse(&'a [u8; 65536]),
}

pub struct Frame {
    pub width: i32,
    pub height: i32,
    pub bits: Vec<u8>,
}

impl Frame {
    pub fn new(width: i32, height: i32) -> Self {
        assert!(width > 0 && height > 0, "degenerate frame {width}x{height}");
        Self {
            width,
            height,
            bits: vec![0; (width * height) as usize],
        }
    }

    pub fn from_bits(width: i32, height: i32, bits: Vec<u8>) -> Self {
        assert_eq!(bits.len(), (width * height) as usize);
        Self {
            width,
            height,
            bits,
        }
    }

    fn index(&self, x: i32, y: i32) -> usize {
        (y * self.width + x) as usize
    }

    /// `CFrame::PSet` (unclipped in the original's frame path; we clip
    /// because Rust will panic where C scribbled).
    pub fn pset(&mut self, x: i32, y: i32, color: u8) {
        if x >= 0 && y >= 0 && x < self.width && y < self.height {
            let i = self.index(x, y);
            self.bits[i] = color;
        }
    }

    pub fn get(&self, x: i32, y: i32) -> u8 {
        self.bits[self.index(x, y)]
    }

    /// `CFrame::FillBox` — clipped rectangle fill.
    pub fn fill_box(&mut self, rect: &Rect, color: u8) {
        let mut r = *rect;
        if !frame_clip_rect(&self.bounds(), &mut r) {
            return;
        }
        for y in r.top..r.bottom {
            let start = self.index(r.left, y);
            let end = self.index(r.right, y);
            self.bits[start..end].fill(color);
        }
    }

    /// `CFrame::Erase` — fill with color 0 (BLACK).
    pub fn erase(&mut self, rect: &Rect) {
        self.fill_box(rect, 0);
    }

    pub fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.width, self.height)
    }

    /// `CFrame::Blit` — clip to this frame then dispatch. `src_rect`
    /// selects the source region; `dst` gives the destination top-left
    /// (rect built exactly like the call sites do).
    pub fn blit(
        &mut self,
        source: &Frame,
        src_rect: &Rect,
        dst_x: i32,
        dst_y: i32,
        mode: BlitMode,
    ) {
        let mut src = *src_rect;
        let mut dst = Rect::new(
            dst_x,
            dst_y,
            dst_x + src_rect.width(),
            dst_y + src_rect.height(),
        );
        if frame_clip_rects(&self.bounds(), &mut src, &mut dst) {
            self.blit_fast(source, &src, &dst, mode);
        }
    }

    /// `CFrame::BlitFast` — no clipping; rects must already be valid.
    fn blit_fast(&mut self, source: &Frame, src: &Rect, dst: &Rect, mode: BlitMode) {
        debug_assert_eq!(src.width(), dst.width());
        debug_assert_eq!(src.height(), dst.height());
        let rows = dst.height();
        let cols = dst.width();

        for row in 0..rows {
            let src_start = source.index(src.left, src.top + row);
            let dst_start = self.index(dst.left, dst.top + row);
            match mode {
                BlitMode::Normal => {
                    let (s, d) = (src_start, dst_start);
                    self.bits[d..d + cols as usize]
                        .copy_from_slice(&source.bits[s..s + cols as usize]);
                }
                BlitMode::Transparent0 => {
                    for i in 0..cols as usize {
                        let c = source.bits[src_start + i];
                        if c != 0 {
                            self.bits[dst_start + i] = c;
                        }
                    }
                }
                BlitMode::Transparent0Reverse => {
                    // Mirrors the whole source row: the original
                    // asserts the src rect spans the full width in
                    // reverse mode, so column i reads from the far end.
                    for i in 0..cols as usize {
                        let c = source.bits[src_start + (cols as usize - 1 - i)];
                        if c != 0 {
                            self.bits[dst_start + i] = c;
                        }
                    }
                }
                BlitMode::RemapSource(remap) => {
                    for i in 0..cols as usize {
                        let c = source.bits[src_start + i];
                        if c != 0 {
                            self.bits[dst_start + i] = remap[c as usize];
                        }
                    }
                }
                BlitMode::RemapDestOn1(remap) => {
                    for i in 0..cols as usize {
                        let c = source.bits[src_start + i];
                        if c == 1 {
                            self.bits[dst_start + i] = remap[self.bits[dst_start + i] as usize];
                        } else if c != 0 {
                            self.bits[dst_start + i] = c;
                        }
                    }
                }
                BlitMode::Combine64K(table) => {
                    for i in 0..cols as usize {
                        let c = source.bits[src_start + i];
                        if c != 0 {
                            let d = self.bits[dst_start + i];
                            self.bits[dst_start + i] = table[((c as usize) << 8) + d as usize];
                        }
                    }
                }
                BlitMode::Combine64KReverse(table) => {
                    for i in 0..cols as usize {
                        let c = source.bits[src_start + (cols as usize - 1 - i)];
                        if c != 0 {
                            let d = self.bits[dst_start + i];
                            self.bits[dst_start + i] = table[((c as usize) << 8) + d as usize];
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_2x2() -> Frame {
        // [ 0 5 ]
        // [ 7 0 ]
        Frame::from_bits(2, 2, vec![0, 5, 7, 0])
    }

    #[test]
    fn normal_transparent_and_reverse_modes() {
        let src = source_2x2();

        let mut d = Frame::from_bits(2, 2, vec![9; 4]);
        d.blit(&src, &src.bounds(), 0, 0, BlitMode::Normal);
        assert_eq!(d.bits, vec![0, 5, 7, 0]);

        let mut d = Frame::from_bits(2, 2, vec![9; 4]);
        d.blit(&src, &src.bounds(), 0, 0, BlitMode::Transparent0);
        assert_eq!(d.bits, vec![9, 5, 7, 9]);

        let mut d = Frame::from_bits(2, 2, vec![9; 4]);
        d.blit(&src, &src.bounds(), 0, 0, BlitMode::Transparent0Reverse);
        assert_eq!(d.bits, vec![5, 9, 9, 7]);
    }

    #[test]
    fn remap_source_and_dest_on_1() {
        let mut remap = [0u8; 256];
        remap[5] = 50;
        remap[7] = 70;
        remap[9] = 90;

        let src = source_2x2();
        let mut d = Frame::from_bits(2, 2, vec![9; 4]);
        d.blit(&src, &src.bounds(), 0, 0, BlitMode::RemapSource(&remap));
        assert_eq!(d.bits, vec![9, 50, 70, 9]);

        // Source index 1 remaps the DEST pixel (shadow effect).
        let shadow_src = Frame::from_bits(2, 1, vec![1, 3]);
        let mut d = Frame::from_bits(2, 1, vec![9, 9]);
        d.blit(
            &shadow_src,
            &shadow_src.bounds(),
            0,
            0,
            BlitMode::RemapDestOn1(&remap),
        );
        assert_eq!(d.bits, vec![90, 3]);
    }

    #[test]
    fn combine_64k_indexes_src_high_dest_low() {
        let mut table = vec![0u8; 65536];
        table[(5 << 8) + 9] = 123;
        let table: &[u8; 65536] = table.as_slice().try_into().unwrap();

        let src = Frame::from_bits(1, 1, vec![5]);
        let mut d = Frame::from_bits(1, 1, vec![9]);
        d.blit(&src, &src.bounds(), 0, 0, BlitMode::Combine64K(table));
        assert_eq!(d.bits, vec![123]);
    }

    #[test]
    fn blit_clips_against_destination() {
        let src = Frame::from_bits(4, 1, vec![1, 2, 3, 4]);
        let mut d = Frame::from_bits(2, 1, vec![0, 0]);
        // Destination starts at -2: only the last two source pixels land.
        d.blit(&src, &src.bounds(), -2, 0, BlitMode::Normal);
        assert_eq!(d.bits, vec![3, 4]);
    }

    #[test]
    fn fill_box_clips() {
        let mut f = Frame::new(4, 4);
        f.fill_box(&Rect::new(2, 2, 10, 10), 7);
        assert_eq!(f.get(1, 1), 0);
        assert_eq!(f.get(2, 2), 7);
        assert_eq!(f.get(3, 3), 7);
    }
}
