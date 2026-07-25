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

pub mod fixed_trig;
pub mod heartbeat;
pub mod rand;
mod title_screen;

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
pub fn build_astrorock_app(font: Arc<Font>) -> App {
    App::new(Box::new(TitleScreen::new(font)))
}
