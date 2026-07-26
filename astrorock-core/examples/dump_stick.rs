//! Dev tool: render the tilt-joystick pad (SDF metaball) at a few
//! deflections and write PNGs for visual inspection.
//!
//! ```text
//! cargo run -p astrorock-core --example dump_stick -- out_dir
//! ```

use astrorock_core::joystick;

fn write_png(path: &std::path::Path, size: usize, rgba: &[u8]) {
    let file = std::fs::File::create(path).expect("create png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), size as u32, size as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .expect("png header")
        .write_image_data(rgba)
        .expect("png data");
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("out dir");
    const SIZE: usize = 192;
    for (name, pos, active) in [
        ("stick_rest", (0.0, 0.0), false),
        ("stick_mid", (0.55, -0.35), true),
        ("stick_edge", (1.0, 0.0), true),
    ] {
        let img = joystick::render(SIZE, pos, active);
        let path = dir.join(format!("{name}.png"));
        write_png(&path, SIZE, &img);
        println!("wrote {}", path.display());
    }
}
