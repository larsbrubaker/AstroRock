//! # Native Shell for AstroRock
//!
//! Thinnest possible desktop shim: everything platform-generic (winit
//! window + event loop, wgpu surface, input forwarding, frame painting)
//! lives in `demo_wgpu::native_shell`. This file only names the window
//! and hands over the shared app built by `astrorock-core`.

mod audio;

use astrorock_core::{build_astrorock_app_with_audio, load_default_font};

fn main() {
    let sink =
        audio::RodioAudio::new().map(|a| Box::new(a) as Box<dyn astrorock_core::audio::AudioSink>);
    if sink.is_none() {
        eprintln!("audio: no output device — running silent");
    }
    let app = build_astrorock_app_with_audio(load_default_font(), sink);

    demo_wgpu::native_shell::run(
        demo_wgpu::NativeShellConfig {
            title: "AstroRock",
            // The original game runs 640x480; give the dev window a little
            // headroom. The in-game surface stays 640x480 regardless.
            logical_size: (960.0, 720.0),
        },
        app,
        {
            // Paint-cadence diagnostic: `ASTROROCK_FPS=1 cargo run` prints
            // frames-per-second once a second to stderr.
            let report = std::env::var_os("ASTROROCK_FPS").is_some();
            let mut frames = 0u32;
            let mut last = std::time::Instant::now();
            move || {
                if report {
                    frames += 1;
                    if last.elapsed().as_secs() >= 1 {
                        eprintln!("fps={frames}");
                        frames = 0;
                        last = std::time::Instant::now();
                    }
                }
            }
        },
    );
}
