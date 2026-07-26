//! # The demo replay regression suite (Phase 9)
//!
//! Replays every shipped `assets/demos/*.dat` headless and compares
//! against `tests/demo_golden.txt`: the per-beat `CheckPlayField`
//! stream digested with FNV-1a, the final NetRand sync counter, the
//! final checksum, whether the pilot ended dead, and the score.
//!
//! The 1997 files carry no recorded checksums (`CHECK_DEMO` was
//! compiled out), so the goldens are OUR blessed baseline: they pin
//! the whole deterministic core — every RNG draw, every f32 shape —
//! against regressions, identically on native and wasm. Bless with:
//!
//! ```text
//! $env:ASTROROCK_BLESS_DEMOS="1"; cargo test -p astrorock-core --test demo_replay
//! ```
//!
//! after VISUALLY confirming attract playback looks like a competent
//! 1997 pilot (a desynced replay flies into rocks within seconds).

use std::fmt::Write as _;
use std::path::PathBuf;

use astrorock_core::demo::Demo;
use astrorock_core::game::Game;

fn demos_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../assets/demos")
}

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/demo_golden.txt")
}

/// FNV-1a over the per-beat checksum bytes.
fn fnv1a(bytes: impl Iterator<Item = u8>) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

struct ReplayResult {
    name: String,
    updates: usize,
    start_level: u32,
    stream_fnv: u64,
    end_sync: u32,
    end_check: u8,
    ended_dead: bool,
    score: u32,
}

fn replay(name: &str, bytes: &[u8]) -> ReplayResult {
    let demo = Demo::parse(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
    let mut game = Game::new(None);
    game.init_demo(demo.start_level);

    let mut checks = Vec::with_capacity(demo.key_flags.len());
    for &flags in &demo.key_flags {
        game.demo_beat(flags);
        checks.push(game.check_play_field());
    }

    ReplayResult {
        name: name.to_string(),
        updates: demo.key_flags.len(),
        start_level: demo.start_level,
        stream_fnv: fnv1a(checks.iter().copied()),
        end_sync: game.rand_sync(),
        end_check: *checks.last().expect("demos are non-empty"),
        ended_dead: !game.ship_visible(),
        score: game.score(),
    }
}

fn result_line(r: &ReplayResult) -> String {
    let mut s = String::new();
    write!(
        s,
        "{} updates={} level={} fnv={:016x} sync={} check={} dead={} score={}",
        r.name,
        r.updates,
        r.start_level,
        r.stream_fnv,
        r.end_sync,
        r.end_check,
        r.ended_dead,
        r.score
    )
    .unwrap();
    s
}

#[test]
fn all_shipped_demos_replay_to_golden() {
    let mut entries: Vec<_> = std::fs::read_dir(demos_dir())
        .expect("assets/demos")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "dat"))
        .collect();
    entries.sort();
    assert!(entries.len() >= 30, "expected the shipped demo set");

    let mut lines = Vec::new();
    for path in &entries {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let bytes = std::fs::read(path).expect("read demo");
        let result = replay(&name, &bytes);
        lines.push(result_line(&result));
    }
    let actual = lines.join("\n") + "\n";

    if std::env::var_os("ASTROROCK_BLESS_DEMOS").is_some() {
        std::fs::write(golden_path(), &actual).expect("write golden");
        eprintln!("blessed {} demos into demo_golden.txt", entries.len());
        return;
    }

    let golden = std::fs::read_to_string(golden_path()).expect(
        "tests/demo_golden.txt missing - run once with ASTROROCK_BLESS_DEMOS=1 \
         after visually verifying attract playback",
    );
    let golden_lines: Vec<&str> = golden.lines().collect();
    let actual_lines: Vec<&str> = actual.lines().collect();
    assert_eq!(
        golden_lines.len(),
        actual_lines.len(),
        "demo count changed vs golden"
    );
    for (g, a) in golden_lines.iter().zip(actual_lines.iter()) {
        assert_eq!(g, a, "demo replay diverged from the blessed golden");
    }
}

#[test]
fn replays_are_deterministic_across_runs() {
    // The same demo replayed twice in one process must match exactly —
    // no hidden global state anywhere in the core.
    let path = demos_dir().join("demo00.dat");
    let bytes = std::fs::read(path).expect("demo00");
    let a = replay("demo00.dat", &bytes);
    let b = replay("demo00.dat", &bytes);
    assert_eq!(a.stream_fnv, b.stream_fnv);
    assert_eq!(a.end_sync, b.end_sync);
    assert_eq!(a.ended_dead, b.ended_dead);
}
