//! Dev tool: compose the current game screen headless and write it as
//! an RGBA PNG for visual inspection.
//!
//! ```text
//! cargo run -p astrorock-core --example dump_frame -- out.png [beats] [esc]
//! ```
//!
//! The optional second argument advances the simulation that many
//! 30 Hz beats first (e.g. to catch the menu's showcase monitor
//! mid-animation). A third argument of `esc` starts a game and hits
//! Escape before those beats — dumping the from-game options page.

use agg_gui::event::Key;
use astrorock_core::game::Game;

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "title_frame.png".to_string());
    let beats: u64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let mode = std::env::args().nth(3).unwrap_or_default();

    let mut game = Game::new(None);
    let mut now = 0;
    if mode == "esc" {
        game.set_key(&Key::Enter, true);
        now = 40;
        game.advance(now);
        game.set_key(&Key::Enter, false);
        game.set_key(&Key::Escape, true);
        game.set_key(&Key::Escape, false);
    }
    let click = |g: &mut Game, x: i32, y: i32| {
        g.on_mouse_down(x, y);
        g.on_mouse_up(x, y);
    };
    if mode == "keys" || mode == "sound" {
        click(&mut game, 350, 280); // Configure (344, 272)
        if mode == "keys" {
            click(&mut game, 250, 290); // Config Controls (245, 284)
        } else {
            click(&mut game, 250, 330); // Config Sound (245, 324)
        }
    }
    if mode == "help" {
        click(&mut game, 350, 320); // Help (344, 312)
    }
    if mode == "high" {
        click(&mut game, 140, 320); // View High Scores (136, 312)
    }
    for _ in 0..beats {
        now += 34;
        game.advance(now);
    }
    game.compose();
    let mut rgba = Vec::new();
    game.current_palette()
        .frame_to_rgba(game.screen(), &mut rgba);
    let screen = game.screen();

    let file = std::fs::File::create(&path).expect("create output file");
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        screen.width as u32,
        screen.height as u32,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("png header");
    writer.write_image_data(&rgba).expect("png data");
    println!("wrote {path}");
}
