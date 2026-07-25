//! # Bitmap fonts — port of `Font.cpp`
//!
//! Only the fixed-width path is ported: all four fonts the game loads
//! (`Courier`, `pScoreFont`, `pStatFont`, `pAstroFont`) pass
//! `fixedwidth = 1`, slicing one sheet into equal-width cells
//! (`CFrameSequence::Initialize(frame, nFrames, 0, 0)`). The
//! proportional loader, `BGColor` fill, `PrintMultiLine`, and the
//! per-call fixed-advance override have no call sites in the shipped
//! game and are not ported. `CFont::Erase` (one stat-bar dirty-rect
//! call) is also skipped — this port recomposes every frame.
//!
//! Instead of the original's mutable `pBlitType` field (players.cpp
//! swaps it around a Print to recolor text per player), `print` takes
//! the [`BlitMode`] directly.

use crate::assets;
use crate::frame::{BlitMode, Frame};
use crate::rect::Rect;

/// `JUSTIFICATION`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Justify {
    Left,
    Center,
    Right,
}

pub struct Font {
    sheet: Frame,
    cell_width: i32,
    first_ascii: u8,
    last_ascii: u8,
}

impl Font {
    /// `CFont::Load(..., fixedwidth = 1)`: the sheet divides into
    /// `last - first + 1` equal cells.
    pub fn load_fixed(png: &[u8], first_ascii: u8, last_ascii: u8) -> Self {
        let sheet = assets::frame_from_indexed_png(png);
        let cells = (last_ascii - first_ascii) as i32 + 1;
        let cell_width = sheet.width / cells;
        assert!(
            cell_width > 0 && sheet.width % cells == 0,
            "font sheet width {} does not divide into {cells} cells",
            sheet.width
        );
        Self {
            sheet,
            cell_width,
            first_ascii,
            last_ascii,
        }
    }

    /// `rCourierFont` — 9x13 cells, ASCII 32..=126.
    pub fn courier() -> Self {
        Self::load_fixed(assets::COURIER_FONT_PNG, 32, 126)
    }

    /// `rAstroTexFont` (`pAstroFont`) — 10x13 textured caps, 32..=90.
    pub fn astro() -> Self {
        Self::load_fixed(assets::ASTROTEX_FONT_PNG, 32, 90)
    }

    /// `rNumbersFont` (`pScoreFont`) — 16x16 score digits, 48..=57.
    pub fn score() -> Self {
        Self::load_fixed(assets::NUMBERS_FONT_PNG, 48, 57)
    }

    /// `rShotNumFont` (`pStatFont`) — 8x9 stat digits, 48..=57.
    pub fn stat() -> Self {
        Self::load_fixed(assets::SHOTNUM_FONT_PNG, 48, 57)
    }

    pub fn height(&self) -> i32 {
        self.sheet.height
    }

    pub fn cell_width(&self) -> i32 {
        self.cell_width
    }

    /// `GetWidth(char *)` — fixed cells, so length x cell width.
    pub fn width_of(&self, text: &str) -> i32 {
        text.len() as i32 * self.cell_width
    }

    /// The cell index for a byte, with the original's quirks: fonts
    /// without lowercase (`LastASCII < 96`) uppercase their input, and
    /// the unsigned `curf - FirstASCII` underflow catches everything
    /// out of range with cell 0 (the space).
    fn glyph(&self, c: u8) -> i32 {
        let c = if self.last_ascii < 96 {
            c.to_ascii_uppercase()
        } else {
            c
        };
        let idx = c.wrapping_sub(self.first_ascii);
        if idx > self.last_ascii - self.first_ascii {
            0
        } else {
            idx as i32
        }
    }

    /// `CFont::Print`.
    pub fn print(
        &self,
        dest: &mut Frame,
        x: i32,
        y: i32,
        text: &str,
        justify: Justify,
        mode: BlitMode,
    ) {
        if text.is_empty() {
            return;
        }
        let mut x = match justify {
            Justify::Left => x,
            Justify::Center => x - self.width_of(text) / 2,
            Justify::Right => x - self.width_of(text),
        };
        for &b in text.as_bytes() {
            let g = self.glyph(b);
            let src = Rect::new(
                g * self.cell_width,
                0,
                (g + 1) * self.cell_width,
                self.sheet.height,
            );
            dest.blit(&self.sheet, &src, x, y, mode);
            x += self.cell_width;
        }
    }

    /// `CFont::Print(..., int num, ...)` (`longToAscii` + Print).
    pub fn print_num(
        &self,
        dest: &mut Frame,
        x: i32,
        y: i32,
        num: i64,
        justify: Justify,
        mode: BlitMode,
    ) {
        self.print(dest, x, y, &num.to_string(), justify, mode);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_four_fonts_slice_cleanly() {
        // Cell sizes pinned against the shipped sheets and the C++
        // Load calls (AstroRock.cpp 546..577).
        let courier = Font::courier();
        assert_eq!((courier.cell_width(), courier.height()), (9, 13));
        let astro = Font::astro();
        assert_eq!((astro.cell_width(), astro.height()), (10, 13));
        let score = Font::score();
        assert_eq!((score.cell_width(), score.height()), (16, 16));
        let stat = Font::stat();
        assert_eq!((stat.cell_width(), stat.height()), (8, 9));
    }

    #[test]
    fn glyph_mapping_quirks() {
        let astro = Font::astro();
        // No lowercase cells: 'a' upper-cases to 'A' = cell 33.
        assert_eq!(astro.glyph(b'a'), 33);
        assert_eq!(astro.glyph(b'A'), 33);
        // Out of range (above and below) falls back to cell 0.
        assert_eq!(astro.glyph(b'{'), 0);
        assert_eq!(astro.glyph(7), 0);
        // Courier has lowercase and keeps it.
        let courier = Font::courier();
        assert_eq!(courier.glyph(b'a'), (b'a' - 32) as i32);
    }

    #[test]
    fn print_renders_and_justifies() {
        let font = Font::score();
        let mut left = Frame::new(64, 16);
        font.print(&mut left, 0, 0, "42", Justify::Left, BlitMode::Normal);
        // Right-justified against the frame edge lands at x=32 —
        // shifted, not clipped.
        let mut right = Frame::new(64, 16);
        font.print(&mut right, 64, 0, "42", Justify::Right, BlitMode::Normal);
        assert!(left.bits.iter().any(|&b| b != 0), "digits drew nothing");
        assert_ne!(left.bits, right.bits);
        assert_eq!(left.bits[..32], right.bits[32..64]);
        // Centered at 16 == left at 0 for a 32px-wide string.
        let mut centered = Frame::new(64, 16);
        font.print(
            &mut centered,
            16,
            0,
            "42",
            Justify::Center,
            BlitMode::Normal,
        );
        assert_eq!(left.bits, centered.bits);
    }

    #[test]
    fn print_num_matches_print_of_digits() {
        let font = Font::stat();
        let mut via_num = Frame::new(40, 9);
        font.print_num(&mut via_num, 0, 0, 307, Justify::Left, BlitMode::Normal);
        let mut via_str = Frame::new(40, 9);
        font.print(&mut via_str, 0, 0, "307", Justify::Left, BlitMode::Normal);
        assert_eq!(via_num.bits, via_str.bits);
    }
}
