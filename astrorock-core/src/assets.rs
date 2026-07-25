//! # Embedded converted assets
//!
//! The original loaded everything through Burgerlib's rez archive; this
//! port embeds the converted files from `assets/` directly in the
//! binary. Indexed PNGs decode back to [`Frame`]s of palette indices —
//! the game composites in palette space, colors only apply at
//! presentation via [`Palette`].

use crate::frame::Frame;
use crate::palette::Palette;

/// `rGamePalette` — the in-game master palette (extracted from the
/// shipped resource's BMP color table).
pub const GAME_PAL: &[u8] = include_bytes!("../../assets/palettes/game.pal");

/// `rTransRedPal` — 256-byte index remap table (stored in a 768-byte
/// .pal container; only the first 256 bytes are the table, matching
/// `CPalette::Load(..., PALETTE_REMAP)` which copies SIZE_REMAP bytes).
pub const TRANSRED_PAL: &[u8] = include_bytes!("../../assets/palettes/transred.pal");

/// `rTeaserBmp` — the ASTROROCK title art (310x294 indexed).
pub const TEASER_PNG: &[u8] = include_bytes!("../../assets/interfac/teaser.png");

/// Decode an 8-bit indexed PNG (as written by `astrorock-tools`) into a
/// [`Frame`] of palette indices. Panics on malformed embedded data —
/// these bytes ship inside the binary, so failure is a build defect.
pub fn frame_from_indexed_png(data: &[u8]) -> Frame {
    let mut decoder = png::Decoder::new(data);
    // Keep raw indices: no palette expansion, no tRNS-to-alpha.
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder.read_info().expect("embedded png header");
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("embedded png data");
    assert_eq!(
        info.color_type,
        png::ColorType::Indexed,
        "asset PNGs must stay indexed"
    );
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    buf.truncate((info.width * info.height) as usize);
    Frame::from_bits(info.width as i32, info.height as i32, buf)
}

/// The game master palette.
pub fn game_palette() -> Palette {
    Palette::from_pal_bytes(GAME_PAL).expect("game.pal is 768 bytes")
}

/// A 256-entry remap table from a `.pal` remap container.
pub fn remap_table(pal_bytes: &[u8]) -> [u8; 256] {
    let mut table = [0u8; 256];
    table.copy_from_slice(&pal_bytes[..256]);
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teaser_decodes_to_indices() {
        let teaser = frame_from_indexed_png(TEASER_PNG);
        assert_eq!((teaser.width, teaser.height), (310, 294));
        assert!(
            teaser.bits.iter().any(|&b| b != 0),
            "teaser should have non-transparent pixels"
        );
    }

    #[test]
    fn game_palette_has_white_at_15() {
        // Stars draw with index 15; the master palette keeps white there.
        assert_eq!(game_palette().color(15), (255, 255, 255));
        assert_eq!(game_palette().color(0), (0, 0, 0));
    }

    #[test]
    fn transred_is_a_remap_table() {
        let t = remap_table(TRANSRED_PAL);
        // Identity-ish in the low indices, red-shifted higher up — spot
        // check the shipped values.
        assert_eq!(t[1], 1);
        assert_eq!(t[0], 191);
    }
}
