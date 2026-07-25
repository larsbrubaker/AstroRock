//! Dev tool: compose the current game screen headless and write it as
//! an RGBA PNG for visual inspection.
//!
//! ```text
//! cargo run -p astrorock-core --example dump_frame -- out.png
//! ```

use astrorock_core::assets;
use astrorock_core::title_screen::TitleScreen;

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "title_frame.png".to_string());

    let mut title = TitleScreen::new();
    title.compose();
    let screen = title.screen();
    let mut rgba = Vec::new();
    assets::game_palette().frame_to_rgba(screen, &mut rgba);

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
