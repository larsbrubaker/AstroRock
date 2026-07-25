//! # Window chrome — the themed surround for the game surface
//!
//! Modern presentation, not part of the 1997 look. The game surface
//! gets as much of the window as possible: whenever an aspect-fit at
//! full height leaves enough horizontal slack, the controls live on a
//! side rail and the game runs top-to-bottom; only in narrow/portrait
//! windows do they drop to a bottom bar. Buttons are Font Awesome
//! glyphs (music, volume, expand/compress) drawn through agg-gui, so
//! native and wasm render identically — no platform UI.

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::geometry::Rect as GuiRect;
use agg_gui::text::Font;

/// Bottom control bar height (narrow/portrait fallback).
pub const BAR_H: f64 = 40.0;
/// Side rail width (preferred — lets the game fill the height).
pub const RAIL_W: f64 = 56.0;
const BTN: f64 = 40.0;

/// Font Awesome glyphs.
const FA_MUSIC: &str = "\u{f001}";
const FA_VOLUME: &str = "\u{f028}";
const FA_EXPAND: &str = "\u{f065}";
const FA_COMPRESS: &str = "\u{f066}";

/// Where everything landed this frame, in widget coords (bottom-left
/// origin, Y-up). Button rects are hit-tested on MouseDown.
pub struct ChromeLayout {
    /// Destination of the game surface: x, y, w, h.
    pub game: (f64, f64, f64, f64),
    pub music_btn: GuiRect,
    pub sfx_btn: GuiRect,
    pub fullscreen_btn: GuiRect,
}

pub fn hit(r: &GuiRect, x: f64, y: f64) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}

fn game_aspect() -> f64 {
    crate::game::SCREEN_W as f64 / crate::game::SCREEN_H as f64
}

/// One square icon button: plate, border, glyph; a red strike bar
/// marks the off state.
fn icon_button(ctx: &mut dyn DrawCtx, rect: &GuiRect, glyph: &str, on: bool, icons: &Arc<Font>) {
    let edge = Color::from_rgb8(56, 63, 79);
    ctx.set_fill_color(Color::from_rgb8(38, 44, 56));
    ctx.begin_path();
    ctx.rect(rect.x, rect.y, rect.width, rect.height);
    ctx.fill();
    ctx.set_fill_color(edge);
    ctx.begin_path();
    ctx.rect(rect.x, rect.y, rect.width, 1.0);
    ctx.rect(rect.x, rect.y + rect.height - 1.0, rect.width, 1.0);
    ctx.rect(rect.x, rect.y, 1.0, rect.height);
    ctx.rect(rect.x + rect.width - 1.0, rect.y, 1.0, rect.height);
    ctx.fill();

    ctx.set_fill_color(if on {
        Color::from_rgb8(214, 222, 240)
    } else {
        Color::from_rgb8(110, 118, 134)
    });
    ctx.set_font(icons.clone());
    ctx.set_font_size(18.0);
    // FA glyphs are near-square: eyeball-center in the plate.
    ctx.fill_text(
        glyph,
        rect.x + rect.width / 2.0 - 9.0,
        rect.y + rect.height / 2.0 - 8.0,
    );

    if !on {
        ctx.set_fill_color(Color::from_rgb8(196, 64, 64));
        ctx.begin_path();
        ctx.rect(
            rect.x + 6.0,
            rect.y + rect.height / 2.0 - 1.5,
            rect.width - 12.0,
            3.0,
        );
        ctx.fill();
    }
}

/// Paint backdrop, rail/bar, buttons, and the frame around the (still
/// unpainted) game rect; the caller blits the game image into
/// `layout.game` afterwards.
pub fn paint(
    ctx: &mut dyn DrawCtx,
    w: f64,
    h: f64,
    music_on: bool,
    sfx_on: bool,
    icons: &Arc<Font>,
) -> ChromeLayout {
    let backdrop = Color::from_rgb8(11, 13, 18);
    let panel_bg = Color::from_rgb8(24, 28, 36);
    let edge = Color::from_rgb8(56, 63, 79);

    ctx.set_fill_color(backdrop);
    ctx.begin_path();
    ctx.rect(0.0, 0.0, w, h);
    ctx.fill();

    // Side rail whenever full-height aspect-fit leaves the slack for
    // it; bottom bar otherwise (widget coords are bottom-left, Y-up).
    let rail = w - h * game_aspect() >= RAIL_W;
    let (game_w, game_h, rail_x, bar_h) = if rail {
        (w - RAIL_W, h, w - RAIL_W, 0.0)
    } else {
        (w, (h - BAR_H).max(1.0), w, BAR_H)
    };

    if rail {
        ctx.set_fill_color(panel_bg);
        ctx.begin_path();
        ctx.rect(rail_x, 0.0, RAIL_W, h);
        ctx.fill();
        ctx.set_fill_color(edge);
        ctx.begin_path();
        ctx.rect(rail_x, 0.0, 1.0, h);
        ctx.fill();
    } else {
        ctx.set_fill_color(panel_bg);
        ctx.begin_path();
        ctx.rect(0.0, 0.0, w, BAR_H);
        ctx.fill();
        ctx.set_fill_color(edge);
        ctx.begin_path();
        ctx.rect(0.0, BAR_H - 1.0, w, 1.0);
        ctx.fill();
    }

    // Aspect-fit the game surface in what's left.
    let scale = (game_w / crate::game::SCREEN_W as f64).min(game_h / crate::game::SCREEN_H as f64);
    let dw = crate::game::SCREEN_W as f64 * scale;
    let dh = crate::game::SCREEN_H as f64 * scale;
    let dx = (game_w - dw) * 0.5;
    let dy = bar_h + (game_h - dh) * 0.5;

    // Hairline frame around the game screen.
    ctx.set_fill_color(edge);
    ctx.begin_path();
    ctx.rect(dx - 2.0, dy - 2.0, dw + 4.0, 2.0);
    ctx.rect(dx - 2.0, dy + dh, dw + 4.0, 2.0);
    ctx.rect(dx - 2.0, dy - 2.0, 2.0, dh + 4.0);
    ctx.rect(dx + dw, dy - 2.0, 2.0, dh + 4.0);
    ctx.fill();

    // Buttons: stacked from the top of the rail, or left-to-right on
    // the bar.
    let (music_btn, sfx_btn, fullscreen_btn) = if rail {
        let bx = rail_x + (RAIL_W - BTN) / 2.0;
        let top = h - 12.0 - BTN;
        (
            GuiRect::new(bx, top, BTN, BTN),
            GuiRect::new(bx, top - (BTN + 10.0), BTN, BTN),
            GuiRect::new(bx, top - 2.0 * (BTN + 10.0), BTN, BTN),
        )
    } else {
        let by = (BAR_H - 30.0) / 2.0;
        (
            GuiRect::new(12.0, by, BTN, 30.0),
            GuiRect::new(12.0 + BTN + 10.0, by, BTN, 30.0),
            GuiRect::new(12.0 + 2.0 * (BTN + 10.0), by, BTN, 30.0),
        )
    };

    icon_button(ctx, &music_btn, FA_MUSIC, music_on, icons);
    icon_button(ctx, &sfx_btn, FA_VOLUME, sfx_on, icons);
    let fs_glyph = if agg_gui::fullscreen::is_active() {
        FA_COMPRESS
    } else {
        FA_EXPAND
    };
    icon_button(ctx, &fullscreen_btn, fs_glyph, true, icons);

    ChromeLayout {
        game: (dx, dy, dw, dh),
        music_btn,
        sfx_btn,
        fullscreen_btn,
    }
}
