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

/// `PRESSENT.BMP` — the "Press Enter" prompt (the shareware teaser
/// screen is retired; the company, address, and phone number on it are
/// decades gone).
pub const PRESS_ENTER_PNG: &[u8] = include_bytes!("../../assets/interfac/pressent.png");

/// The four game fonts — fixed-width sheets sliced by `font.rs`.
/// `rCourierFont`, `rNumbersFont`, `rShotNumFont`, `rAstroTexFont`.
pub const COURIER_FONT_PNG: &[u8] = include_bytes!("../../assets/interfac/courier.png");
pub const NUMBERS_FONT_PNG: &[u8] = include_bytes!("../../assets/interfac/numbers.png");
pub const SHOTNUM_FONT_PNG: &[u8] = include_bytes!("../../assets/interfac/shotnum.png");
pub const ASTROTEX_FONT_PNG: &[u8] = include_bytes!("../../assets/interfac/astrotex.png");

/// Stat bar art (`DrawStats`): the bar itself plus meters, power-up
/// icons, and the extra-ship icon.
pub const STATBAR_PNG: &[u8] = include_bytes!("../../assets/interfac/statbar.png");
pub const HEALTH_PNG: &[u8] = include_bytes!("../../assets/interfac/health.png");
pub const HEALTHB_PNG: &[u8] = include_bytes!("../../assets/interfac/healthb.png");
/// `rShieldBmp` the METER FILL (not the shield halo sprite).
pub const SHIELD_STAT_PNG: &[u8] = include_bytes!("../../assets/interfac/shield.png");
pub const SHIELDB_PNG: &[u8] = include_bytes!("../../assets/interfac/shieldb.png");
pub const YGSTAT_PNG: &[u8] = include_bytes!("../../assets/interfac/ygstat.png");
pub const RGSTAT_PNG: &[u8] = include_bytes!("../../assets/interfac/rgstat.png");
pub const BOMBSTAT_PNG: &[u8] = include_bytes!("../../assets/interfac/bombstat.png");
pub const RAPDSTAT_PNG: &[u8] = include_bytes!("../../assets/interfac/rapdstat.png");
pub const SPRDSTAT_PNG: &[u8] = include_bytes!("../../assets/interfac/sprdstat.png");
pub const LIVES_PNG: &[u8] = include_bytes!("../../assets/interfac/lives.png");

/// `rPlrRedPal` — player 0's ship recolor (`ShipBlit[0]`,
/// `PlyrColorTable` in players.cpp).
pub const PLRRED_PAL: &[u8] = include_bytes!("../../assets/palettes/plrred.pal");

/// `rTallywinBmp` — the intermission tally window.
pub const TALLYWIN_PNG: &[u8] = include_bytes!("../../assets/interfac/tallywin.png");
/// `rEndgameBmp` — the GAME OVER overlay.
pub const ENDGAME_PNG: &[u8] = include_bytes!("../../assets/interfac/endgame.png");

/// `rStartBmp` — the start-screen backdrop, which carries ITS OWN
/// palette (`m_StartGameFrame.LoadPalette(rStartBmp)`).
pub const START_PNG: &[u8] = include_bytes!("../../assets/interfac/start.png");
/// `rReallyqBmp` — "Are you sure you want to quit?".
pub const REALLYQ_PNG: &[u8] = include_bytes!("../../assets/interfac/reallyq.png");
/// The showcase monitor's TV-static frames (`rStatic1Bmp`..3).
pub const STATIC1_PNG: &[u8] = include_bytes!("../../assets/interfac/static1.png");
pub const STATIC2_PNG: &[u8] = include_bytes!("../../assets/interfac/static2.png");
pub const STATIC3_PNG: &[u8] = include_bytes!("../../assets/interfac/static3.png");

/// Start-screen button art (up/down pairs).
pub const STRGM_PNG: [&[u8]; 2] = [
    include_bytes!("../../assets/interfac/strgm1.png"),
    include_bytes!("../../assets/interfac/strgm2.png"),
];
pub const NETR_PNG: [&[u8]; 2] = [
    include_bytes!("../../assets/interfac/netr1.png"),
    include_bytes!("../../assets/interfac/netr2.png"),
];
pub const VHIGH_PNG: [&[u8]; 2] = [
    include_bytes!("../../assets/interfac/vhigh1.png"),
    include_bytes!("../../assets/interfac/vhigh2.png"),
];
pub const CRED_PNG: [&[u8]; 2] = [
    include_bytes!("../../assets/interfac/cred1.png"),
    include_bytes!("../../assets/interfac/cred2.png"),
];
pub const CONFIG_PNG: [&[u8]; 2] = [
    include_bytes!("../../assets/interfac/config1.png"),
    include_bytes!("../../assets/interfac/config2.png"),
];
pub const HELP_PNG: [&[u8]; 2] = [
    include_bytes!("../../assets/interfac/help1.png"),
    include_bytes!("../../assets/interfac/help2.png"),
];
pub const QUIT_PNG: [&[u8]; 2] = [
    include_bytes!("../../assets/interfac/quit1.png"),
    include_bytes!("../../assets/interfac/quit2.png"),
];
pub const DEMO_PNG: [&[u8]; 2] = [
    include_bytes!("../../assets/interfac/demo1.png"),
    include_bytes!("../../assets/interfac/demo2.png"),
];
pub const DONE_PNG: [&[u8]; 2] = [
    include_bytes!("../../assets/interfac/done1.png"),
    include_bytes!("../../assets/interfac/done2.png"),
];
pub const BUTTONL_PNG: [&[u8]; 2] = [
    include_bytes!("../../assets/interfac/buttonl1.png"),
    include_bytes!("../../assets/interfac/buttonl2.png"),
];
pub const BUTTONR_PNG: [&[u8]; 2] = [
    include_bytes!("../../assets/interfac/buttonr1.png"),
    include_bytes!("../../assets/interfac/buttonr2.png"),
];
pub const CFGKEYS_PNG: [&[u8]; 2] = [
    include_bytes!("../../assets/interfac/cfgkeys1.png"),
    include_bytes!("../../assets/interfac/cfgkeys2.png"),
];
pub const CFGSND_PNG: [&[u8]; 2] = [
    include_bytes!("../../assets/interfac/cfgsnd1.png"),
    include_bytes!("../../assets/interfac/cfgsnd2.png"),
];
pub const YES_PNG: [&[u8]; 2] = [
    include_bytes!("../../assets/interfac/yes1.png"),
    include_bytes!("../../assets/interfac/yes2.png"),
];
pub const NO_PNG: [&[u8]; 2] = [
    include_bytes!("../../assets/interfac/no1.png"),
    include_bytes!("../../assets/interfac/no2.png"),
];

/// `rGloop2Pal` / `rGloop3Pal` — gloop tier recolor remap tables.
pub const GLOOP2_PAL: &[u8] = include_bytes!("../../assets/palettes/gloop2.pal");
pub const GLOOP3_PAL: &[u8] = include_bytes!("../../assets/palettes/gloop3.pal");

/// `rHk2Pal` / `rHk3Pal` — hunter-killer tier recolor remap tables.
pub const HK2_PAL: &[u8] = include_bytes!("../../assets/palettes/hk2.pal");
pub const HK3_PAL: &[u8] = include_bytes!("../../assets/palettes/hk3.pal");

/// `rBomber2Pal` / `rBomber3Pal` — bomber tier recolor remap tables.
pub const BOMBER2_PAL: &[u8] = include_bytes!("../../assets/palettes/bomber2.pal");
pub const BOMBER3_PAL: &[u8] = include_bytes!("../../assets/palettes/bomber3.pal");

/// `rSpikeBall2Pal` / `rSpikeBall3Pal` — spikeball tier remap tables.
pub const SPIKEBALL2_PAL: &[u8] = include_bytes!("../../assets/palettes/spkball2.pal");
pub const SPIKEBALL3_PAL: &[u8] = include_bytes!("../../assets/palettes/spkball3.pal");

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

/// The palette carried inside an indexed PNG (`LoadPalette(rXxxBmp)` —
/// screens like the start menu present through their own art's
/// palette). Entries past the PNG's PLTE stay black.
pub fn palette_from_indexed_png(data: &[u8]) -> Palette {
    let decoder = png::Decoder::new(data);
    let reader = decoder.read_info().expect("embedded png header");
    let plte = reader
        .info()
        .palette
        .as_ref()
        .expect("indexed png carries a palette");
    let mut rgb = [0u8; 768];
    let n = plte.len().min(768);
    rgb[..n].copy_from_slice(&plte[..n]);
    Palette { rgb }
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
    fn press_enter_decodes_to_indices() {
        let art = frame_from_indexed_png(PRESS_ENTER_PNG);
        assert!(art.width > 0 && art.height > 0);
        assert!(
            art.bits.iter().any(|&b| b != 0),
            "press-enter art should have non-transparent pixels"
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
