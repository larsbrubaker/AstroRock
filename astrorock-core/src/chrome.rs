//! # Window chrome — the themed surround for the game surface
//!
//! Modern presentation, not part of the 1997 look: a charcoal backdrop,
//! a control bar along the bottom with MUSIC / SOUND toggles, and a
//! hairline frame that makes the letterboxed 640x480 surface read as a
//! framed game screen instead of floating in dead black. Drawn entirely
//! through agg-gui vector calls (labels via the built-in GSV stroke
//! font), so native and wasm render identically — no platform UI.

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::geometry::Rect as GuiRect;

/// Height of the control bar under the game surface.
pub const BAR_H: f64 = 40.0;

/// Where everything landed this frame, in widget coords (bottom-left
/// origin, Y-up). Button rects are hit-tested on MouseDown.
pub struct ChromeLayout {
    /// Destination of the game surface: x, y, w, h.
    pub game: (f64, f64, f64, f64),
    pub music_btn: GuiRect,
    pub sfx_btn: GuiRect,
}

pub fn hit(r: &GuiRect, x: f64, y: f64) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}

/// Paint backdrop, bar, buttons, and the frame around the (still
/// unpainted) game rect; the caller blits the game image into
/// `layout.game` afterwards.
pub fn paint(ctx: &mut dyn DrawCtx, w: f64, h: f64, music_on: bool, sfx_on: bool) -> ChromeLayout {
    let backdrop = Color::from_rgb8(11, 13, 18);
    let bar_bg = Color::from_rgb8(24, 28, 36);
    let edge = Color::from_rgb8(56, 63, 79);

    ctx.set_fill_color(backdrop);
    ctx.begin_path();
    ctx.rect(0.0, 0.0, w, h);
    ctx.fill();

    // Bottom control bar with a 1px highlight along its top edge.
    ctx.set_fill_color(bar_bg);
    ctx.begin_path();
    ctx.rect(0.0, 0.0, w, BAR_H);
    ctx.fill();
    ctx.set_fill_color(edge);
    ctx.begin_path();
    ctx.rect(0.0, BAR_H - 1.0, w, 1.0);
    ctx.fill();

    // Aspect-fit the game surface in the area above the bar.
    let game_h = (h - BAR_H).max(1.0);
    let scale = (w / crate::game::SCREEN_W as f64).min(game_h / crate::game::SCREEN_H as f64);
    let dw = crate::game::SCREEN_W as f64 * scale;
    let dh = crate::game::SCREEN_H as f64 * scale;
    let dx = (w - dw) * 0.5;
    let dy = BAR_H + (game_h - dh) * 0.5;

    // Hairline frame around the game screen.
    ctx.set_fill_color(edge);
    ctx.begin_path();
    ctx.rect(dx - 2.0, dy - 2.0, dw + 4.0, 2.0);
    ctx.rect(dx - 2.0, dy + dh, dw + 4.0, 2.0);
    ctx.rect(dx - 2.0, dy - 2.0, 2.0, dh + 4.0);
    ctx.rect(dx + dw, dy - 2.0, 2.0, dh + 4.0);
    ctx.fill();

    let music_btn = GuiRect::new(12.0, 7.0, 92.0, BAR_H - 14.0);
    let sfx_btn = GuiRect::new(116.0, 7.0, 92.0, BAR_H - 14.0);
    for (rect, label, on) in [(music_btn, "MUSIC", music_on), (sfx_btn, "SOUND", sfx_on)] {
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
        // LED: lit green when on, dark socket when off.
        ctx.set_fill_color(if on {
            Color::from_rgb8(84, 220, 108)
        } else {
            Color::from_rgb8(52, 40, 40)
        });
        ctx.begin_path();
        ctx.rect(rect.x + 8.0, rect.y + rect.height / 2.0 - 3.0, 6.0, 6.0);
        ctx.fill();
        ctx.set_fill_color(if on {
            Color::from_rgb8(214, 222, 240)
        } else {
            Color::from_rgb8(130, 138, 155)
        });
        ctx.fill_text_gsv(label, rect.x + 22.0, rect.y + rect.height / 2.0 - 5.0, 11.0);
    }

    ChromeLayout {
        game: (dx, dy, dw, dh),
        music_btn,
        sfx_btn,
    }
}
