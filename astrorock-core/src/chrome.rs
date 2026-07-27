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
const FA_ROTATE_LEFT: &str = "\u{f0e2}";
const FA_ROTATE_RIGHT: &str = "\u{f01e}";
const FA_ROCKET: &str = "\u{f135}";
const FA_GEAR: &str = "\u{f013}";
const FA_BACK: &str = "\u{f060}";

/// Touch-button size presets — the gear dropdown's S/M/L/XL.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TouchSize {
    S,
    #[default]
    M,
    L,
    XL,
}

impl TouchSize {
    pub const ALL: [TouchSize; 4] = [TouchSize::S, TouchSize::M, TouchSize::L, TouchSize::XL];

    /// Drawn plate size in logical pixels.
    pub fn px(self) -> f64 {
        match self {
            TouchSize::S => 64.0,
            TouchSize::M => 92.0,
            TouchSize::L => 122.0,
            TouchSize::XL => 154.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TouchSize::S => "S",
            TouchSize::M => "M",
            TouchSize::L => "L",
            TouchSize::XL => "XL",
        }
    }

    pub fn from_label(s: &str) -> TouchSize {
        match s {
            "S" => TouchSize::S,
            "L" => TouchSize::L,
            "XL" => TouchSize::XL,
            _ => TouchSize::M,
        }
    }
}

/// The mobile virtual-gamepad HIT areas: rotate-left / rotate-right
/// under the left thumb; thrust + fire (with shield above fire)
/// under the right; plus the menu (Esc) tap target. The L/R/T/F
/// rects are much larger than the drawn plates — each fills its
/// corner zone out to the screen edges and the midline to its
/// neighbor, so a fat-fingered press near a button still lands.
pub struct TouchLayout {
    pub left_btn: GuiRect,
    pub right_btn: GuiRect,
    pub fire_btn: GuiRect,
    pub thrust_btn: GuiRect,
    pub shield_btn: GuiRect,
    pub menu_btn: GuiRect,
    /// The size-config gear at the top of the left side.
    pub gear_btn: GuiRect,
    /// The S/M/L/XL rows while the gear dropdown is open.
    pub size_opts: Option<[GuiRect; 4]>,
}

/// Per-frame hold state the touch chrome lights buttons from.
#[derive(Clone, Copy, Default)]
pub struct TouchUi {
    pub left: bool,
    pub right: bool,
    pub fire: bool,
    pub thrust: bool,
    pub shield: bool,
    /// Current plate-size preset (the gear dropdown edits it).
    pub size: TouchSize,
    /// The gear dropdown is open.
    pub size_menu: bool,
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
    // The glyph scales with the plate (FA glyphs are near-square).
    let glyph_px = (rect.height * 0.5).clamp(14.0, 28.0);
    ctx.set_font_size(glyph_px);
    ctx.fill_text(
        glyph,
        rect.x + rect.width / 2.0 - glyph_px / 2.0,
        rect.y + rect.height / 2.0 - glyph_px * 0.44,
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

/// One big hold-target for the virtual gamepad: a TRANSLUCENT plate
/// (it may sit over the playfield), thick border, large glyph; the
/// whole thing lights and firms up while held.
fn touch_button(ctx: &mut dyn DrawCtx, rect: &GuiRect, glyph: &str, held: bool, icons: &Arc<Font>) {
    ctx.set_fill_color(if held {
        Color::from_rgba8(58, 70, 96, 185)
    } else {
        Color::from_rgba8(30, 35, 46, 120)
    });
    ctx.begin_path();
    ctx.rect(rect.x, rect.y, rect.width, rect.height);
    ctx.fill();
    let edge = if held {
        Color::from_rgba8(140, 170, 230, 230)
    } else {
        Color::from_rgba8(120, 132, 158, 160)
    };
    ctx.set_fill_color(edge);
    ctx.begin_path();
    ctx.rect(rect.x, rect.y, rect.width, 2.0);
    ctx.rect(rect.x, rect.y + rect.height - 2.0, rect.width, 2.0);
    ctx.rect(rect.x, rect.y, 2.0, rect.height);
    ctx.rect(rect.x + rect.width - 2.0, rect.y, 2.0, rect.height);
    ctx.fill();
    ctx.set_fill_color(Color::from_rgba8(200, 210, 230, 220));
    ctx.set_font(icons.clone());
    // The glyph grows with the plate.
    let glyph_px = (rect.width * 0.38).clamp(20.0, 56.0);
    ctx.set_font_size(glyph_px);
    ctx.fill_text(
        glyph,
        rect.x + rect.width / 2.0 - glyph_px / 2.0,
        rect.y + rect.height / 2.0 - glyph_px * 0.44,
    );
}

/// Every rect the touch layout produces — pure geometry, unit-tested
/// for non-overlap across orientations. Left column: the rotate
/// pair `[L][R]` at the thumb. Right column, top to bottom: music +
/// sfx mutes, fullscreen + Esc, then shield above fire in the
/// `[T][F]` row.
pub(crate) struct TouchRects {
    pub game: (f64, f64, f64, f64),
    pub landscape: bool,
    pub col_w: f64,
    pub right_x: f64,
    pub zone_h: f64,
    /// Drawn plates.
    pub left: GuiRect,
    pub right: GuiRect,
    pub fire: GuiRect,
    pub thrust: GuiRect,
    pub shield: GuiRect,
    pub fs: GuiRect,
    pub menu: GuiRect,
    pub music: GuiRect,
    pub sfx: GuiRect,
    /// Expanded hit areas for the four hold buttons (the plates are
    /// just the visuals — the touch surface owns the corner).
    pub left_hit: GuiRect,
    pub right_hit: GuiRect,
    pub thrust_hit: GuiRect,
    pub fire_hit: GuiRect,
    /// The size-config gear + its dropdown rows.
    pub gear: GuiRect,
    pub size_opts: [GuiRect; 4],
}

/// Compute the touch layout. The playfield always wins: max HEIGHT
/// in landscape (columns take the leftover width), max WIDTH in
/// portrait (a bottom zone takes the leftover height). The
/// translucent plates size from the user's preset and may overlap
/// the playfield; controls never overlap EACH OTHER.
pub(crate) fn touch_rects(w: f64, h: f64, size: TouchSize) -> TouchRects {
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

    // Plate size comes from the user's S/M/L/XL preset. Plates are
    // TRANSLUCENT and may spill over the playfield, so the columns
    // no longer cap them — only the screen height does.
    // Small buttons sized for fingers, not mouse pointers.
    let small = 50.0_f64.min(((col_w - 3.0 * PAD) / 2.0).max(24.0));
    let pair_gap = 10.0;
    // Size from the preset, capped twice: the right-side stack
    // (fire + shield) must fit the zone height, and all four bottom
    // plates must fit across the window with clearance between the
    // clusters. (The small buttons live top-LEFT, out of both
    // constraints.)
    let stack_cap = (zone_h - 2.0 * PAD - GAP - 2.0) / 2.0;
    let row_cap = (w - 2.0 * PAD - 2.0 * pair_gap - 24.0) / 4.0;
    let pair = size.px().min(stack_cap).min(row_cap).max(30.0);

    // `[L][R]` rotate pair in the bottom-left corner (centered in
    // the column when it fits, corner-anchored when it spills).
    let lcx = left_x + col_w / 2.0;
    let total = 2.0 * pair + pair_gap;
    let lx0 = if total + 2.0 * PAD <= col_w {
        lcx - total / 2.0
    } else {
        left_x + PAD
    };
    let left = GuiRect::new(lx0, PAD, pair, pair);
    let right = GuiRect::new(lx0 + pair + pair_gap, PAD, pair, pair);

    // `[T][F]` row in the bottom-right corner (fire outboard at the
    // very corner), shield above fire — the stack owns the full zone
    // height now that the small buttons live elsewhere.
    let rcx = right_x + col_w / 2.0;
    let fx0 = if total + 2.0 * PAD <= col_w {
        rcx + total / 2.0 - pair
    } else {
        w - PAD - pair
    };
    let fire = GuiRect::new(fx0, PAD, pair, pair);
    let thrust = GuiRect::new(fx0 - pair_gap - pair, PAD, pair, pair);
    let shield = GuiRect::new(fire.x, PAD + pair + GAP, pair, pair);

    // All the small buttons form one TOP-LEFT row — far from the
    // firing thumb AND out of the stack's way: gear, back (Esc),
    // fullscreen, music, sfx. Translucent-era placement: the row may
    // sit over the playfield's top edge.
    let row_y = zone_h - PAD - small;
    let srow = |i: f64| GuiRect::new(left_x + PAD + i * (small + 8.0), row_y, small, small);
    let gear = srow(0.0);
    let menu = srow(1.0);
    let fs = srow(2.0);
    let music = srow(3.0);
    let sfx = srow(4.0);

    // The gear's dropdown rows open downward beneath it.
    let opt_h = 42.0;
    let opt_w = 76.0_f64.max(small);
    let mut size_opts = [GuiRect::default(); 4];
    for (i, slot) in size_opts.iter_mut().enumerate() {
        slot.x = gear.x;
        slot.y = gear.y - (i as f64 + 1.0) * (opt_h + 6.0);
        slot.width = opt_w;
        slot.height = opt_h;
    }

    // Expanded hit areas: from the screen bottom to a generous
    // overshoot above the plates, split at the pair midline, spilling
    // a margin past the outer plate edges. Fire stops just under the
    // shield plate; thrust just under the small rows.
    let overshoot = 44.0;
    let ext = 24.0;
    let mid_l = lx0 + pair + pair_gap / 2.0;
    let l_top = (PAD + pair + overshoot).min(h);
    let lr_edge = (lx0 + total + ext).min(w);
    let left_hit = GuiRect::new(0.0, 0.0, mid_l, l_top);
    let right_hit = GuiRect::new(mid_l, 0.0, lr_edge - mid_l, l_top);
    let mid_r = thrust.x + pair + pair_gap / 2.0;
    let t_left = (thrust.x - ext).max(lr_edge);
    let t_top = (PAD + pair + overshoot).min(h);
    let f_top = (PAD + pair + overshoot).min(shield.y - 2.0).min(h);
    let thrust_hit = GuiRect::new(t_left, 0.0, mid_r - t_left, t_top);
    let fire_hit = GuiRect::new(mid_r, 0.0, w - mid_r, f_top);

    TouchRects {
        game: (dx, dy, dw, dh),
        landscape,
        col_w,
        right_x,
        zone_h,
        left,
        right,
        fire,
        thrust,
        shield,
        fs,
        menu,
        music,
        sfx,
        left_hit,
        right_hit,
        thrust_hit,
        fire_hit,
        gear,
        size_opts,
    }
}

/// The touch BACKGROUND pass: zone panels only. The buttons draw in
/// [`paint_touch_overlay`] AFTER the caller blits the game image, so
/// the translucent plates sit ON TOP of the playfield.
fn paint_touch(ctx: &mut dyn DrawCtx, w: f64, h: f64, ui: TouchUi) -> ChromeLayout {
    let panel_bg = Color::from_rgb8(24, 28, 36);
    let edge = Color::from_rgb8(56, 63, 79);
    let r = touch_rects(w, h, ui.size);

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

    ChromeLayout {
        game: r.game,
        music_btn: r.music,
        sfx_btn: r.sfx,
        fullscreen_btn: r.fs,
        touch: Some(TouchLayout {
            left_btn: r.left_hit,
            right_btn: r.right_hit,
            fire_btn: r.fire_hit,
            thrust_btn: r.thrust_hit,
            shield_btn: r.shield,
            menu_btn: r.menu,
            gear_btn: r.gear,
            size_opts: ui.size_menu.then_some(r.size_opts),
        }),
    }
}

/// The touch OVERLAY pass — every control, drawn on top of the game
/// image the caller just blitted.
#[allow(clippy::too_many_arguments)]
pub fn paint_touch_overlay(
    ctx: &mut dyn DrawCtx,
    w: f64,
    h: f64,
    music_on: bool,
    sfx_on: bool,
    ui: TouchUi,
    icons: &Arc<Font>,
    text: &Arc<Font>,
) {
    let r = touch_rects(w, h, ui.size);

    touch_button(ctx, &r.left, FA_ROTATE_LEFT, ui.left, icons);
    touch_button(ctx, &r.right, FA_ROTATE_RIGHT, ui.right, icons);
    touch_button(ctx, &r.fire, FA_CROSSHAIRS, ui.fire, icons);
    touch_button(ctx, &r.thrust, FA_ROCKET, ui.thrust, icons);
    touch_button(ctx, &r.shield, FA_SHIELD, ui.shield, icons);
    icon_button(ctx, &r.music, FA_MUSIC, music_on, icons);
    icon_button(ctx, &r.sfx, FA_VOLUME, sfx_on, icons);
    // The back button (Esc): back through menus, options in play.
    icon_button(ctx, &r.menu, FA_BACK, true, icons);
    let fs_glyph = if agg_gui::fullscreen::is_active() {
        FA_COMPRESS
    } else {
        FA_EXPAND
    };
    icon_button(ctx, &r.fs, fs_glyph, true, icons);

    // The size gear + its S/M/L/XL dropdown.
    icon_button(ctx, &r.gear, FA_GEAR, true, icons);
    if ui.size_menu {
        for (opt, preset) in r.size_opts.iter().zip(TouchSize::ALL) {
            let current = preset == ui.size;
            ctx.set_fill_color(if current {
                Color::from_rgb8(58, 70, 96)
            } else {
                Color::from_rgb8(38, 44, 56)
            });
            ctx.begin_path();
            ctx.rect(opt.x, opt.y, opt.width, opt.height);
            ctx.fill();
            ctx.set_fill_color(Color::from_rgb8(120, 132, 158));
            ctx.begin_path();
            ctx.rect(opt.x, opt.y, opt.width, 1.0);
            ctx.rect(opt.x, opt.y + opt.height - 1.0, opt.width, 1.0);
            ctx.rect(opt.x, opt.y, 1.0, opt.height);
            ctx.rect(opt.x + opt.width - 1.0, opt.y, 1.0, opt.height);
            ctx.fill();
            ctx.set_fill_color(Color::from_rgb8(214, 222, 240));
            ctx.set_font(text.clone());
            ctx.set_font_size(20.0);
            let label = preset.label();
            let lw = label.len() as f64 * 11.0;
            ctx.fill_text(
                label,
                opt.x + (opt.width - lw) / 2.0,
                opt.y + opt.height / 2.0 - 9.0,
            );
        }
    }
}

/// Paint backdrop, rail/bar, buttons, and the frame around the (still
/// unpainted) game rect; the caller blits the game image into
/// `layout.game` afterwards. With `touch` set this pass draws ONLY
/// the zone panels — the controls come from [`paint_touch_overlay`]
/// after the game image, so translucent plates sit on top of it.
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
        return paint_touch(ctx, w, h, ui);
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

    /// Requirement: the touch controls never overlap EACH OTHER (the
    /// translucent plates may overlap the playfield by design),
    /// across sizes, orientations, and degenerate windows.
    #[test]
    fn touch_controls_never_overlap() {
        for size in TouchSize::ALL {
            for (w, h) in [
                (844.0, 390.0), // landscape phone
                (390.0, 844.0), // portrait phone
                (932.0, 430.0),
                (360.0, 640.0),
                (640.0, 360.0),
                (320.0, 480.0), // small portrait
                (568.0, 320.0), // small landscape
            ] {
                let r = touch_rects(w, h, size);
                // The HIT areas are the real interaction surfaces.
                let rects = [
                    ("left", &r.left_hit),
                    ("right", &r.right_hit),
                    ("fire", &r.fire_hit),
                    ("thrust", &r.thrust_hit),
                    ("shield", &r.shield),
                    ("fs", &r.fs),
                    ("menu", &r.menu),
                    ("music", &r.music),
                    ("sfx", &r.sfx),
                    ("gear", &r.gear),
                ];
                for i in 0..rects.len() {
                    for j in i + 1..rects.len() {
                        assert!(
                            !overlaps(rects[i].1, rects[j].1),
                            "{:?} {}x{}: {} overlaps {}",
                            size,
                            w,
                            h,
                            rects[i].0,
                            rects[j].0
                        );
                    }
                }
                for (name, rect) in rects {
                    assert!(
                        rect.x >= -0.01
                            && rect.y >= -0.01
                            && rect.x + rect.width <= w + 0.01
                            && rect.y + rect.height <= h + 0.01,
                        "{size:?} {w}x{h}: {name} leaves the window"
                    );
                }
            }
        }
    }

    /// Requirement: the playfield is maximal — full width in
    /// portrait, full height in landscape (until the control floor).
    #[test]
    fn playfield_maximizes_the_long_axis() {
        let l = touch_rects(844.0, 390.0, TouchSize::M);
        assert!(l.landscape);
        assert!((l.game.3 - 390.0).abs() < 0.01, "landscape: full height");

        let p = touch_rects(390.0, 844.0, TouchSize::M);
        assert!(!p.landscape);
        assert!((p.game.2 - 390.0).abs() < 0.01, "portrait: full width");
        // Game hugs the top of the window in portrait.
        assert!((p.game.1 + p.game.3 - 844.0).abs() < 0.01);

        // Narrow landscape: the game shrinks to keep the columns
        // usable, and stays centered.
        let n = touch_rects(700.0, 480.0, TouchSize::M);
        assert!(n.col_w >= 57.9, "columns keep their floor: {}", n.col_w);
    }
}
