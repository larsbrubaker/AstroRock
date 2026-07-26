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
const FA_SHIELD: &str = "\u{f132}";
const FA_CROSSHAIRS: &str = "\u{f05b}";
const FA_BARS: &str = "\u{f0c9}";

/// The mobile virtual-gamepad rects: the tilt joystick pad and
/// shield under the left thumb, fire under the right, and the menu
/// (Esc) tap target. There is no thrust button — full stick
/// deflection IS thrust (the dot merging into the ring shows it).
pub struct TouchLayout {
    pub stick: GuiRect,
    pub shield_btn: GuiRect,
    pub fire_btn: GuiRect,
    pub menu_btn: GuiRect,
}

/// Per-frame state the touch chrome renders from.
#[derive(Clone, Copy, Default)]
pub struct TouchUi {
    pub shield: bool,
    pub fire: bool,
    /// Dot position in steering units (length 1 = full deflection).
    pub stick_pos: (f64, f64),
    /// Steering engaged (outside the dead zone, or thumb on the pad).
    pub stick_active: bool,
}

/// Where everything landed this frame, in widget coords (bottom-left
/// origin, Y-up). Button rects are hit-tested on MouseDown.
pub struct ChromeLayout {
    /// Destination of the game surface: x, y, w, h.
    pub game: (f64, f64, f64, f64),
    pub music_btn: GuiRect,
    pub sfx_btn: GuiRect,
    pub fullscreen_btn: GuiRect,
    /// Present only in mobile-touch mode.
    pub touch: Option<TouchLayout>,
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

/// One big hold-target for the virtual gamepad: dark round-cornered
/// plate, thick border, large glyph; the border lights while held.
fn touch_button(ctx: &mut dyn DrawCtx, rect: &GuiRect, glyph: &str, held: bool, icons: &Arc<Font>) {
    ctx.set_fill_color(if held {
        Color::from_rgb8(52, 62, 84)
    } else {
        Color::from_rgb8(30, 35, 46)
    });
    ctx.begin_path();
    ctx.rect(rect.x, rect.y, rect.width, rect.height);
    ctx.fill();
    let edge = if held {
        Color::from_rgb8(140, 170, 230)
    } else {
        Color::from_rgb8(64, 72, 92)
    };
    ctx.set_fill_color(edge);
    ctx.begin_path();
    ctx.rect(rect.x, rect.y, rect.width, 2.0);
    ctx.rect(rect.x, rect.y + rect.height - 2.0, rect.width, 2.0);
    ctx.rect(rect.x, rect.y, 2.0, rect.height);
    ctx.rect(rect.x + rect.width - 2.0, rect.y, 2.0, rect.height);
    ctx.fill();
    ctx.set_fill_color(Color::from_rgb8(190, 200, 220));
    ctx.set_font(icons.clone());
    ctx.set_font_size(34.0);
    ctx.fill_text(
        glyph,
        rect.x + rect.width / 2.0 - 17.0,
        rect.y + rect.height / 2.0 - 15.0,
    );
}

/// Every rect the touch layout produces — pure geometry, unit-tested
/// for non-overlap across orientations. Left column: the tilt
/// joystick alone (release recalibrates the rest plane; deflection
/// past THRUST_FRAC = thrust). Right column, top to bottom: music +
/// sfx mutes, fullscreen + Esc, shield, fire.
pub(crate) struct TouchRects {
    pub game: (f64, f64, f64, f64),
    pub landscape: bool,
    pub col_w: f64,
    pub right_x: f64,
    pub zone_h: f64,
    pub stick: GuiRect,
    pub shield: GuiRect,
    pub fire: GuiRect,
    pub fs: GuiRect,
    pub menu: GuiRect,
    pub music: GuiRect,
    pub sfx: GuiRect,
}

/// Compute the touch layout. The playfield always wins: max HEIGHT
/// in landscape (columns take the leftover width), max WIDTH in
/// portrait (a bottom zone takes the leftover height) — down to a
/// floor where the game gives back just enough for usable thumbs.
/// Controls scale to their zone so nothing ever overlaps.
pub(crate) fn touch_rects(w: f64, h: f64) -> TouchRects {
    const PAD: f64 = 10.0;
    const GAP: f64 = 12.0;
    const MIN_COL: f64 = 58.0;
    const MIN_ZONE: f64 = 96.0;
    let landscape = w >= h;
    let aspect = game_aspect();
    let (dx, dy, dw, dh, col_w, left_x, right_x, zone_h) = if landscape {
        let dw = (h * aspect).min((w - 2.0 * MIN_COL).max(1.0));
        let dh = dw / aspect;
        let col_w = ((w - dw) / 2.0).max(1.0);
        (col_w, (h - dh) / 2.0, dw, dh, col_w, 0.0, col_w + dw, h)
    } else {
        let dh = (w / aspect).min((h - MIN_ZONE).max(1.0));
        let dw = dh * aspect;
        let zone_h = (h - dh).max(1.0);
        // Game at the top, controls below; two half-width columns.
        (
            (w - dw) / 2.0,
            h - dh,
            dw,
            dh,
            w / 2.0,
            0.0,
            w / 2.0,
            zone_h,
        )
    };

    // Button sizing: the right column stacks shield-over-fire plus
    // two small rows; the left column is ALL joystick — the left
    // thumb never has to leave it (shield and fire are never held
    // together, so both live under the right thumb).
    let small = 36.0_f64.min(((col_w - 3.0 * PAD) / 2.0).max(20.0));
    let big = (col_w - 2.0 * PAD)
        .min((zone_h - 2.0 * (small + GAP) - 2.0 * PAD - GAP) / 2.0)
        .clamp(30.0, 104.0);
    let stick_size = (col_w - 2.0 * PAD)
        .min(zone_h - 2.0 * PAD)
        .clamp(30.0, 200.0);

    // Left column: the joystick alone, bottom-anchored at the thumb.
    let lcx = left_x + col_w / 2.0;
    let stick = GuiRect::new(lcx - stick_size / 2.0, PAD, stick_size, stick_size);

    // Right column: fire at the bottom thumb spot, shield right
    // above it; the small buttons (mutes on top, then fullscreen +
    // Esc) anchor to the TOP of the zone so a firing thumb can't
    // graze them.
    let rcx = right_x + col_w / 2.0;
    let fire = GuiRect::new(rcx - big / 2.0, PAD, big, big);
    let shield = GuiRect::new(rcx - big / 2.0, PAD + big + GAP, big, big);
    let row_mutes = zone_h - PAD - small;
    let row_fs = row_mutes - GAP - small;
    let music = GuiRect::new(rcx - small - 4.0, row_mutes, small, small);
    let sfx = GuiRect::new(rcx + 4.0, row_mutes, small, small);
    let fs = GuiRect::new(rcx - small - 4.0, row_fs, small, small);
    let menu = GuiRect::new(rcx + 4.0, row_fs, small, small);

    TouchRects {
        game: (dx, dy, dw, dh),
        landscape,
        col_w,
        right_x,
        zone_h,
        stick,
        shield,
        fire,
        fs,
        menu,
        music,
        sfx,
    }
}

fn paint_touch(
    ctx: &mut dyn DrawCtx,
    w: f64,
    h: f64,
    music_on: bool,
    sfx_on: bool,
    ui: TouchUi,
    icons: &Arc<Font>,
) -> ChromeLayout {
    let panel_bg = Color::from_rgb8(24, 28, 36);
    let edge = Color::from_rgb8(56, 63, 79);
    let r = touch_rects(w, h);

    // Zone panels.
    ctx.set_fill_color(panel_bg);
    ctx.begin_path();
    if r.landscape {
        ctx.rect(0.0, 0.0, r.col_w, h);
        ctx.rect(r.right_x, 0.0, r.col_w, h);
    } else {
        ctx.rect(0.0, 0.0, w, r.zone_h);
    }
    ctx.fill();
    ctx.set_fill_color(edge);
    ctx.begin_path();
    if r.landscape {
        ctx.rect(r.col_w - 1.0, 0.0, 1.0, h);
        ctx.rect(r.right_x, 0.0, 1.0, h);
    } else {
        ctx.rect(0.0, r.zone_h - 1.0, w, 1.0);
    }
    ctx.fill();

    // The metaball joystick pad, rasterized at layout size.
    let stick_px = (r.stick.width.min(r.stick.height) as usize).max(16);
    let img = crate::joystick::render(stick_px, ui.stick_pos, ui.stick_active);
    ctx.draw_image_rgba_arc(
        &Arc::new(img),
        stick_px as u32,
        stick_px as u32,
        r.stick.x,
        r.stick.y,
        r.stick.width,
        r.stick.height,
    );

    touch_button(ctx, &r.shield, FA_SHIELD, ui.shield, icons);
    touch_button(ctx, &r.fire, FA_CROSSHAIRS, ui.fire, icons);
    icon_button(ctx, &r.music, FA_MUSIC, music_on, icons);
    icon_button(ctx, &r.sfx, FA_VOLUME, sfx_on, icons);
    icon_button(ctx, &r.menu, FA_BARS, true, icons);
    let fs_glyph = if agg_gui::fullscreen::is_active() {
        FA_COMPRESS
    } else {
        FA_EXPAND
    };
    icon_button(ctx, &r.fs, fs_glyph, true, icons);

    ChromeLayout {
        game: r.game,
        music_btn: r.music,
        sfx_btn: r.sfx,
        fullscreen_btn: r.fs,
        touch: Some(TouchLayout {
            stick: r.stick,
            shield_btn: r.shield,
            fire_btn: r.fire,
            menu_btn: r.menu,
        }),
    }
}

/// Paint backdrop, rail/bar, buttons, and the frame around the (still
/// unpainted) game rect; the caller blits the game image into
/// `layout.game` afterwards. `touch` enables the mobile
/// virtual-gamepad layout, lighting held buttons and positioning the
/// joystick dot.
pub fn paint(
    ctx: &mut dyn DrawCtx,
    w: f64,
    h: f64,
    music_on: bool,
    sfx_on: bool,
    touch: Option<TouchUi>,
    icons: &Arc<Font>,
) -> ChromeLayout {
    let backdrop = Color::from_rgb8(11, 13, 18);
    let panel_bg = Color::from_rgb8(24, 28, 36);
    let edge = Color::from_rgb8(56, 63, 79);

    ctx.set_fill_color(backdrop);
    ctx.begin_path();
    ctx.rect(0.0, 0.0, w, h);
    ctx.fill();

    if let Some(ui) = touch {
        return paint_touch(ctx, w, h, music_on, sfx_on, ui, icons);
    }

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
        touch: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlaps(a: &GuiRect, b: &GuiRect) -> bool {
        a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
    }

    /// Requirement: mobile controls never overlap each other or the
    /// game surface, across phone-ish and degenerate window sizes.
    #[test]
    fn touch_controls_never_overlap() {
        for (w, h) in [
            (844.0, 390.0), // landscape phone
            (390.0, 844.0), // portrait phone
            (932.0, 430.0),
            (360.0, 640.0),
            (640.0, 360.0),
            (320.0, 480.0), // small portrait
            (568.0, 320.0), // small landscape
        ] {
            let r = touch_rects(w, h);
            let rects = [
                ("stick", &r.stick),
                ("shield", &r.shield),
                ("fire", &r.fire),
                ("fs", &r.fs),
                ("menu", &r.menu),
                ("music", &r.music),
                ("sfx", &r.sfx),
            ];
            for i in 0..rects.len() {
                for j in i + 1..rects.len() {
                    assert!(
                        !overlaps(rects[i].1, rects[j].1),
                        "{}x{}: {} overlaps {}",
                        w,
                        h,
                        rects[i].0,
                        rects[j].0
                    );
                }
            }
            let game = GuiRect::new(r.game.0, r.game.1, r.game.2, r.game.3);
            for (name, rect) in rects {
                assert!(
                    !overlaps(rect, &game),
                    "{w}x{h}: {name} overlaps the game surface"
                );
                assert!(
                    rect.x >= -0.01
                        && rect.y >= -0.01
                        && rect.x + rect.width <= w + 0.01
                        && rect.y + rect.height <= h + 0.01,
                    "{w}x{h}: {name} leaves the window"
                );
            }
        }
    }

    /// Requirement: the playfield is maximal — full width in
    /// portrait, full height in landscape (until the control floor).
    #[test]
    fn playfield_maximizes_the_long_axis() {
        let l = touch_rects(844.0, 390.0);
        assert!(l.landscape);
        assert!((l.game.3 - 390.0).abs() < 0.01, "landscape: full height");

        let p = touch_rects(390.0, 844.0);
        assert!(!p.landscape);
        assert!((p.game.2 - 390.0).abs() < 0.01, "portrait: full width");
        // Game hugs the top of the window in portrait.
        assert!((p.game.1 + p.game.3 - 844.0).abs() < 0.01);

        // Narrow landscape: the game shrinks to keep the columns
        // usable, and stays centered.
        let n = touch_rects(700.0, 480.0);
        assert!(n.col_w >= 57.9, "columns keep their floor: {}", n.col_w);
    }
}
