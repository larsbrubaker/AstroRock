//! # Native Shell for AstroRock
//!
//! Thinnest possible desktop shim: everything platform-generic (winit
//! window + event loop, wgpu surface, input forwarding, frame painting)
//! lives in `demo_wgpu::native_shell`. This file only names the window
//! and hands over the shared app built by `astrorock-core`.

use astrorock_core::{build_astrorock_app, load_default_font};

fn main() {
    let app = build_astrorock_app(load_default_font());

    demo_wgpu::native_shell::run(
        demo_wgpu::NativeShellConfig {
            title: "AstroRock",
            // The original game runs 640x480; give the dev window a little
            // headroom. The in-game surface stays 640x480 regardless.
            logical_size: (960.0, 720.0),
        },
        app,
        || {},
    );
}
