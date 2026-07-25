//! # WebAssembly Shell for AstroRock
//!
//! Thinnest possible browser shim: everything platform-generic (canvas
//! sizing, wgpu/WebGL2 surface, the rAF loop, DOM pointer / wheel /
//! keyboard listeners, DPR tracking) lives in `demo_wgpu::web_shell`.
//! This crate only boots the shared app built by `astrorock-core`.

#![cfg(target_arch = "wasm32")]

use astrorock_core::{build_astrorock_app, load_default_font};
use demo_wgpu::web_shell;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    web_shell::start(
        "astrorock-canvas",
        || build_astrorock_app(load_default_font()),
        // The 30 Hz simulation runs continuously — keep the rAF loop hot.
        || web_shell::mark_dirty(),
    );
}
