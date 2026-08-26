//! # Native Shell for AstroRock
//!
//! Thinnest possible desktop shim: everything platform-generic (winit
//! window + event loop, wgpu surface, input forwarding, frame painting)
//! lives in the published `agg-gui-shell` crate. This file only names
//! the window, hands over the shared app built by `astrorock-core`, and
//! threads the app-specific glue (gamepad polling) through `ShellHost`.

mod audio;
mod gamepad;
mod settings;

use agg_gui_shell::{run, Frame, ShellConfig, ShellHost};
use astrorock_core::{build_astrorock_app_with_platform, load_default_font};

/// App-specific per-frame glue: gamepad polling rides the shell's
/// `on_frame` hook, exactly like the closure the old `native_shell`
/// wrapper called once per painted frame.
struct AstroRockHost {
    pads: gamepad::GamepadPoller,
}

impl ShellHost for AstroRockHost {
    fn on_frame(&mut self, _app: &mut agg_gui::App, _frame: &Frame) {
        self.pads.poll();
    }
}

fn main() {
    let sink =
        audio::RodioAudio::new().map(|a| Box::new(a) as Box<dyn astrorock_core::audio::AudioSink>);
    if sink.is_none() {
        eprintln!("audio: no output device — running silent");
    }
    let store =
        Box::new(settings::FileSettings::new()) as Box<dyn astrorock_core::settings::SettingsStore>;
    let app = build_astrorock_app_with_platform(load_default_font(), sink, Some(store));

    let config = ShellConfig::new("AstroRock")
        // The original game runs 640x480; give the dev window a little
        // headroom. The in-game surface stays 640x480 regardless.
        .with_logical_size(960.0, 720.0)
        .with_device_label("astrorock-native");

    if let Err(err) = run(config, move |_init| {
        Ok((
            app,
            AstroRockHost {
                pads: gamepad::GamepadPoller::new(),
            },
        ))
    }) {
        eprintln!("astrorock-native: {err}");
        std::process::exit(1);
    }
}
