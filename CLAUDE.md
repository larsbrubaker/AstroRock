# AstroRock — Rust port of the 1997 original

A faithful Rust port of AstroRock, the 1995–97 Win95 DirectDraw asteroids
game, rendered through agg-gui and shipped native + WebAssembly (GitHub
Pages). This file is the working charter: read it before writing code.

## The reference implementation (read-only, local-only)

The original C++ is NOT in this repo and is never published:

- `C:\Development\AstroRock` — original MSVC 4.x source (~31.5k effective
  lines), plus the restored built resources: `rezfile` (Burgerlib BRGR
  archive, 210 resources named by `REZFILE.hpp`), `Astro.Rck` (streamed
  music, headerless 22050 Hz mono 16-bit PCM), loose `ART/`, `SOUND/`,
  `Music/`, `demo/` asset trees.
- `C:\Development\Backups\2097-05\AstroRock` — the shipped-game backup.

Always read the actual C++ source when porting a function — never work
from memory of "how asteroids games work". When two files disagree, the
main source tree wins. The reference is read-only; never edit it.

Dead code in the reference — do not port: `Axis3D.cpp`, `Aviplay.cpp`,
`ApplyLight.hpp`. Deferred (not ported yet, keep the seams):
`net_w95.cpp`, `AstroDialog.cpp` (DirectPlay networking).

## The three pillars

**No stubs, no shortcuts.** Every function must be complete and
production-ready. No `todo!()`, no `unimplemented!()`, no partial
implementations. If dependencies aren't ready, implement them first.

**Determinism is a feature.** The game is a deterministic 30 Hz lockstep
simulation: demo playback (`demo/*.dat` = recorded inputs + periodic
checksums) and any future net play depend on bit-identical behavior.
Consequences:
- The shipped game compiles `CFixed` as `typedef float` (`USE_AS_FIXED
  0` in `Fixed.hpp`) — gameplay math is **f32**, not fixed-point. Port
  it as f32 with the exact C expression shapes (promotion to double and
  truncation points included); never reassociate or "simplify" float
  expressions — the rounding IS the behavior.
- `fixed_trig.rs` tables are generated with the `libm` crate (NOT std)
  so native and wasm produce identical bits; lock-in tests pin exact
  f32 bit patterns. The RNG (`rand.rs`) is wrapping-u32 bit-exact.
- Update order, RNG draw order, and per-object `Check()` checksums match
  the C++ exactly.
- Rust-recorded demos are deterministic across native + wasm. Whether
  the 1997 MSVC/x87 demo checksums reproduce is settled empirically in
  Phase 9 — if they diverge, root-cause before deciding anything.

**Test-first bug fixing.** 1) Write a failing test that reproduces the
bug. 2) Fix it. 3) Confirm the test passes. Never commit a bug fix that
isn't covered by a test. Once demo replay lands, the 27 shipped demos run
in `cargo test` as the whole-game regression suite.

## Porting workflow

Port phase-by-phase in complete, testable modules (this worked for
clipper2-rust, box2d-rust, and box3d-rust; function-by-function tracking
did not). One green module per commit. `todo.md` tracks ONLY remaining
work — as items complete, delete them in the same commit.

Before implementing any function:
1. Read the corresponding C++ source and identify everything it calls.
2. Verify the dependencies exist in Rust; if not, implement them first.
3. Port, keeping the C++ file/function names recognizable (e.g.
   `pship.cpp` → `src/pship.rs`).

When Rust diverges from the original: instrument both sides and diff
traces. Never guess at divergences from reading code.

### C++ → Rust patterns

| C++ | Rust |
|---|---|
| `CFixed` / `SPRITE_UNIT` | `f32` (the shipped `USE_AS_FIXED 0` config), C expression shapes preserved |
| intrusive `pNext/pPrev` sprite lists | `Vec` arenas + indices |
| `GameObj` C-function-pointer vtable | `GameSystem` trait |
| `LoadAResource(rXxx)` | direct load of converted assets (PNG/JSON/MP3/text) |
| 8-bit `CFrame` + blit modes | indexed `Frame` byte buffers, composed then converted to RGBA once per frame |
| DirectDraw/DirectSound/Win32 | agg-gui shells + `AudioPlatform` trait |
| `#define TEXT_*` (text.hpp) | `text.rs` string constants (already centralized) |

## Rendering architecture

All UI through agg-gui — the strongest invariant. Platform shells
(`astrorock-native`, `astrorock-wasm`) create the window/canvas, forward
input, and get out of the way: no widget construction, no mode decisions,
no user-facing strings, no HTML/CSS UI beyond the bare canvas.

The game composes into an 8-bit indexed 640×480 frame (world surface is a
wrapping 2048×1024 `VirtualFrame`), exactly like the original, then
converts through the active palette to RGBA and presents via agg-gui.
Palette tricks (fades, per-player remaps, translucency LUT) therefore
work exactly as in 1997.

## Assets

Converted once by `astrorock-tools` from the originals, committed to
`assets/`:
- `.spr` (LBBSPR v4, see `sequence.cpp`) → indexed PNG sheet + JSON
  (frames × rotations, hotspots, bounds)
- interface BMPs → indexed PNG; `.pal` → 768-byte RGB kept verbatim
- tuning configs (`Rocks.cfg` etc.) → extracted from `rezfile` as text
- SOUND/*.WAV → .mp3 (embedded); Music/*.wav → .mp3 (fetched at runtime,
  NOT embedded — they're megabytes)

The Burgerlib rez file is never read at runtime — it exists only as the
extraction source and as ground truth for what shipped.

## Local development uses agg-gui as a path dep — improve it as you go

`Cargo.toml` patches `agg-gui` to the sibling checkout `../agg-gui/`.
When AstroRock needs an agg-gui capability that doesn't exist, add it to
agg-gui itself (then Lars publishes a new version) — never a local
workaround. CI clones the sibling so the patch resolves there too.

## Build & test (Windows / PowerShell)

```powershell
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --package astrorock-core --package astrorock-native --package astrorock-wasm
cargo dev                          # hot-reload native shell (needs cargo-watch)
wasm-pack build astrorock-wasm --target web --out-dir ../demo/public/pkg --no-typescript
```

Run `scripts/pre-commit-check.ps1` before every commit.

## Forbidden patterns

- `todo!()`, `unimplemented!()`, `panic!()` for missing functionality
- Stub implementations; marking a phase complete while any test fails
- Weakening, `#[ignore]`-ing, or deleting tests to make them pass
- Guessing at divergences instead of instrumenting both sides
- Replacing fixed-point with float math anywhere in gameplay
- Files over 800 lines (`file_line_count` test enforces this — split into
  real modules, never compress to squeak under)
- Editing the C++ reference trees
