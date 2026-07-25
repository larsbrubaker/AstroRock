//! # AstroRock Core
//!
//! Target-agnostic game core for AstroRock, a Rust port of the 1995–97
//! Win95 DirectDraw asteroids game. Every visible pixel renders through
//! agg-gui's [`DrawCtx`] — the native and WASM shells in sibling crates
//! only own the OS window/canvas, the event loop, and platform services.
//!
//! The crate is `wasm32`-clean: no `tokio`, no `winit`, no direct `wgpu`
//! calls.
//!
//! Current state: Phase 0 scaffold — a title screen proving the full
//! native + web pipeline. The game systems land phase by phase; see
//! `todo.md` at the workspace root.

pub mod assets;
pub mod bombers;
pub mod bombs;
pub mod collide;
pub mod events;
pub mod explosion;
pub mod fixed_trig;
pub mod frame;
pub mod gloops;
pub mod heartbeat;
pub mod hks;
pub mod palette;
pub mod pship;
pub mod radar;
pub mod rand;
pub mod rect;
pub mod rocks;
pub mod sequence;
pub mod shots;
pub mod sprite;
pub mod sprite_list;
pub mod thrust;
pub mod title_screen;
pub mod virtual_frame;

use std::sync::Arc;

use agg_gui::text::Font;
use agg_gui::App;

use crate::title_screen::TitleScreen;

/// CascadiaCode bundled into the binary.
///
/// Native + WASM shells pull this via [`load_default_font`] so both targets
/// render the same glyphs without filesystem access (agg-gui's text stack
/// needs a parsed `Font` before the first paint). The original game's
/// bitmap fonts arrive with the UI port; this font covers development
/// overlays and pre-port screens.
pub const DEFAULT_FONT_BYTES: &[u8] = include_bytes!("../assets/CascadiaCode.ttf");

/// Load the default font (CascadiaCode) as an `Arc<Font>`.
pub fn load_default_font() -> Arc<Font> {
    Arc::new(Font::from_slice(DEFAULT_FONT_BYTES).expect("astrorock default font"))
}

/// Build the shared AstroRock widget tree. Both the native and WASM shells
/// call this and forward platform input into the returned [`App`].
///
/// The font isn't consumed yet — the game surface is pure indexed
/// composition; the bitmap-font UI phase will take it (and until then
/// shells keep passing it so the signature is stable).
pub fn build_astrorock_app(_font: Arc<Font>) -> App {
    let mut app = App::new(Box::new(TitleScreen::new()));
    // Held-key tracking needs KeyUp delivery, which only reaches the
    // focused widget — focus the game surface from the first frame.
    app.focus_first();
    app
}
