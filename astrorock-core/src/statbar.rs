//! # Stat bar — port of `DrawStats` + `printStat` (`AstroRock.cpp`)
//!
//! The bottom 96 rows of the 640x480 screen; the play field is the
//! 640x384 `OnScreenRect` above it. The original blitted the bar only
//! when dirty and erased stale digits (`printStat`'s else branch,
//! `CFont::Erase`); this port recomposes every frame, so the bar blit
//! doubles as the erase.
//!
//! Quirk preserved: the digit fonts (48..=57) have no space glyph, and
//! out-of-range chars fall back to cell 0 — the digit `0`. The
//! original prints `sprintf("%6d", Score)`, so its space padding
//! rendered as leading zeros ("000042"). Same formatting here.

use crate::assets;
use crate::font::{Font, Justify};
use crate::frame::{BlitMode, Frame};
use crate::pship::PlayerShip;
use crate::rect::Rect;

/// `HEALTHLEFT` / `SHIELDLEFT`; the tops are offsets from the bar top.
const HEALTH_LEFT: i32 = 38;
const HEALTH_TOP: i32 = 19;
const SHIELD_LEFT: i32 = 39;
const SHIELD_TOP: i32 = 65;

pub struct StatBar {
    bar: Frame,
    health: Frame,
    health_back: Frame,
    shield: Frame,
    shield_back: Frame,
    gun_yellow: Frame,
    gun_red: Frame,
    bomb: Frame,
    rapid: Frame,
    spread: Frame,
    lives: Frame,
    /// `pStatFont` (rShotNumFont) — power-up counters, BlitNormal.
    stat_font: Font,
    /// `pScoreFont` (rNumbersFont) — score + level, BlitTrans.
    score_font: Font,
    /// `ShipBlit[0]` — rPlrRedPal, recolors the extra-ship icons.
    player_remap: [u8; 256],
}

impl Default for StatBar {
    fn default() -> Self {
        Self::new()
    }
}

impl StatBar {
    pub fn new() -> Self {
        Self {
            bar: assets::frame_from_indexed_png(assets::STATBAR_PNG),
            health: assets::frame_from_indexed_png(assets::HEALTH_PNG),
            health_back: assets::frame_from_indexed_png(assets::HEALTHB_PNG),
            shield: assets::frame_from_indexed_png(assets::SHIELD_STAT_PNG),
            shield_back: assets::frame_from_indexed_png(assets::SHIELDB_PNG),
            gun_yellow: assets::frame_from_indexed_png(assets::YGSTAT_PNG),
            gun_red: assets::frame_from_indexed_png(assets::RGSTAT_PNG),
            bomb: assets::frame_from_indexed_png(assets::BOMBSTAT_PNG),
            rapid: assets::frame_from_indexed_png(assets::RAPDSTAT_PNG),
            spread: assets::frame_from_indexed_png(assets::SPRDSTAT_PNG),
            lives: assets::frame_from_indexed_png(assets::LIVES_PNG),
            stat_font: Font::stat(),
            score_font: Font::score(),
            player_remap: assets::remap_table(assets::PLRRED_PAL),
        }
    }

    /// The bar's height — the play field (`OnScreenRect`) is the
    /// screen minus this (`StatBarTop = pScreen->Height - Height`).
    pub fn height(&self) -> i32 {
        self.bar.height
    }

    /// `printStat`: a right-aligned 3-wide count plus its icon, only
    /// while the count is non-zero.
    #[allow(clippy::too_many_arguments)]
    fn print_stat(
        &self,
        screen: &mut Frame,
        top: i32,
        num: u32,
        x: i32,
        y: i32,
        icon: &Frame,
        x2: i32,
        y2: i32,
    ) {
        if num == 0 {
            return;
        }
        let text = format!("{num:3}");
        self.stat_font
            .print(screen, x, y + top, &text, Justify::Left, BlitMode::Normal);
        screen.blit(icon, &icon.bounds(), x2, y2 + top, BlitMode::Transparent0);
    }

    /// A meter: transparent backdrop, then the fill clipped to `value`
    /// pixels (`srcRect.right = min(Width, max(0, value))`).
    fn meter(&self, screen: &mut Frame, back: &Frame, fill: &Frame, value: i32, x: i32, y: i32) {
        screen.blit(back, &back.bounds(), x, y, BlitMode::Transparent0);
        let clipped = value.clamp(0, fill.width);
        if clipped > 0 {
            let src = Rect::new(0, 0, clipped, fill.height);
            screen.blit(fill, &src, x, y, BlitMode::Transparent0);
        }
    }

    /// `DrawStats` (the single-player parts; net names/frags are
    /// deferred with networking). The radar is drawn separately by the
    /// caller at (255, 395), after this, exactly like the original.
    pub fn draw(&self, screen: &mut Frame, ship: &PlayerShip, cur_level: u32) {
        let top = screen.height - self.bar.height;
        screen.blit(&self.bar, &self.bar.bounds(), 0, top, BlitMode::Normal);

        self.meter(
            screen,
            &self.health_back,
            &self.health,
            ship.sprite.hp as i32,
            HEALTH_LEFT,
            HEALTH_TOP + top,
        );

        self.print_stat(
            screen,
            top,
            ship.num_super_shots,
            444,
            33,
            &self.gun_yellow,
            438,
            43,
        );
        self.print_stat(
            screen,
            top,
            ship.num_power_shots,
            505,
            22,
            &self.gun_red,
            497,
            32,
        );
        self.print_stat(screen, top, ship.num_bombs, 565, 33, &self.bomb, 568, 43);
        self.print_stat(screen, top, ship.num_rapids, 479, 55, &self.rapid, 488, 66);
        self.print_stat(
            screen,
            top,
            ship.num_spreads,
            532,
            55,
            &self.spread,
            530,
            66,
        );

        self.meter(
            screen,
            &self.shield_back,
            &self.shield,
            ship.num_shields as i32,
            SHIELD_LEFT,
            SHIELD_TOP + top,
        );

        // Score and level sit at the TOP of the screen, not on the bar.
        self.score_font.print(
            screen,
            20,
            20,
            &format!("{:6}", ship.score),
            Justify::Left,
            BlitMode::Transparent0,
        );
        self.score_font.print(
            screen,
            520,
            20,
            &cur_level.to_string(),
            Justify::Left,
            BlitMode::Transparent0,
        );

        // Extra ships: NumShips-1 recolored icons from x=140.
        if ship.num_ships >= 2 {
            for i in 0..(ship.num_ships - 1) as i32 {
                screen.blit(
                    &self.lives,
                    &self.lives.bounds(),
                    140 + i * self.lives.width,
                    14,
                    BlitMode::RemapSource(&self.player_remap),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_matches_1997() {
        let sb = StatBar::new();
        // 640x96 bar -> StatBarTop 384, OnScreenRect 640x384.
        assert_eq!(sb.bar.width, 640);
        assert_eq!(sb.height(), 96);
        // MAX_HP fills the meter exactly.
        assert_eq!(sb.health.width, crate::pship::MAX_HP as i32);
    }

    #[test]
    fn draw_fills_the_bottom_and_counts_lives() {
        let sb = StatBar::new();
        let mut screen = Frame::new(640, 480);
        let mut ship = PlayerShip::new();
        ship.sprite.hp = 100;
        ship.score = 42;
        ship.num_ships = 3;
        ship.num_bombs = 2;
        sb.draw(&mut screen, &ship, 1);

        // The bar row is composed (bottom area no longer all black).
        let bar_row: Vec<u8> = (0..640).map(|x| screen.get(x, 470)).collect();
        assert!(bar_row.iter().any(|&b| b != 0), "stat bar did not draw");
        // Two extra-ship icons at y=14..39 starting at x=140.
        let lives_px: Vec<u8> = (140..190).map(|x| screen.get(x, 26)).collect();
        assert!(lives_px.iter().any(|&b| b != 0), "lives icons missing");
        // Score digits at the top-left ("000042" with the zero quirk).
        let score_px: Vec<u8> = (20..116).map(|x| screen.get(x, 28)).collect();
        assert!(score_px.iter().any(|&b| b != 0), "score digits missing");
    }

    #[test]
    fn zero_counts_draw_no_icons() {
        let sb = StatBar::new();
        let mut with = Frame::new(640, 480);
        let mut without = Frame::new(640, 480);
        let mut ship = PlayerShip::new();
        ship.num_bombs = 0;
        sb.draw(&mut without, &ship, 1);
        ship.num_bombs = 7;
        sb.draw(&mut with, &ship, 1);
        assert_ne!(with.bits, without.bits, "bomb count should change pixels");
    }
}
