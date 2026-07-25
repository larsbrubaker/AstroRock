//! # WebAssembly Shell for AstroRock
//!
//! Thinnest possible browser shim: everything platform-generic (canvas
//! sizing, wgpu/WebGL2 surface, the rAF loop, DOM pointer / wheel /
//! keyboard listeners, DPR tracking) lives in `demo_wgpu::web_shell`.
//! This crate only boots the shared app built by `astrorock-core`.

#![cfg(target_arch = "wasm32")]

mod audio;

use astrorock_core::{build_astrorock_app_with_audio, load_default_font};
use demo_wgpu::web_shell;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    web_shell::start(
        "astrorock-canvas",
        || {
            let sink = audio::WebAudio::new()
                .map(|a| Box::new(a) as Box<dyn astrorock_core::audio::AudioSink>);
            build_astrorock_app_with_audio(load_default_font(), sink)
        },
        // The 30 Hz simulation runs continuously — keep the rAF loop hot.
        || web_shell::mark_dirty(),
    );
}
