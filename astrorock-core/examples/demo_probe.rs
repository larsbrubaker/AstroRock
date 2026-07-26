//! Diagnostic: replay one demo and print a timeline — score, sync,
//! stats — plus dump frames as PNGs to eyeball whether the recorded
//! pilot is visibly tracking the world (synced) or flailing
//! (diverged). `cargo run -p astrorock-core --example demo_probe --
//! demo00.dat [dump_beat...]`

use astrorock_core::demo::Demo;
use astrorock_core::game::Game;

fn main() {
    let name = std::env::args().nth(1).unwrap_or("demo00.dat".into());
    let dump_beats: Vec<usize> = std::env::args()
        .skip(2)
        .filter_map(|a| a.parse().ok())
        .collect();

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../assets/demos")
        .join(&name);
    let bytes = std::fs::read(&path).expect("read demo");
    let demo = Demo::parse(&bytes).expect("parse");
    println!(
        "{name}: {} updates, start level {}",
        demo.key_flags.len(),
        demo.start_level
    );

    let mut game = Game::new(None);
    game.init_demo(demo.start_level);
    println!(
        "after init: sync={} check={}",
        game.rand_sync(),
        game.check_play_field()
    );

    let mut fired_beats = 0usize;
    let mut was_visible = true;
    for (i, &flags) in demo.key_flags.iter().enumerate() {
        if flags & astrorock_core::demo::FLAG_FIRE != 0 {
            fired_beats += 1;
        }
        game.demo_beat(flags);
        if game.ship_visible() != was_visible {
            was_visible = game.ship_visible();
            println!(
                "beat {i:5}: ship {} (sync={})",
                if was_visible { "SPAWNED" } else { "DIED" },
                game.rand_sync(),
            );
        }
        if i % 100 == 0 || i + 1 == demo.key_flags.len() {
            println!(
                "beat {i:5}: sync={:6} check={:3} score={:5} visible={} fire_beats={}",
                game.rand_sync(),
                game.check_play_field(),
                game.score(),
                game.ship_visible(),
                fired_beats,
            );
        }
        if dump_beats.contains(&i) {
            let mut rgba = Vec::new();
            game.compose();
            game.palette.frame_to_rgba(game.screen(), &mut rgba);
            let out = format!("demo_probe_{i}.png");
            let file = std::fs::File::create(&out).expect("create png");
            let mut enc = png::Encoder::new(std::io::BufWriter::new(file), 640, 480);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header()
                .expect("hdr")
                .write_image_data(&rgba)
                .expect("data");
            println!("wrote {out}");
        }
    }
}
