# AstroRock port — remaining work

This document tracks ONLY work that remains. Nothing listed here is done.
As items complete, delete them — in the same commit that finishes the
work. If this file ever describes finished work, that's a bug. Use
`git log` for history.

## Phase 1 — Asset pipeline (`astrorock-tools` crate)

- `.spr` (LBBSPR v4, format in `sequence.cpp`/`Sequence.hpp`: magic
  `LBBSPR`, version 4, little-endian longs, optional RLE + alpha block)
  → indexed PNG sheet + JSON sidecar per sprite.
- Interface BMPs (8-bit indexed, `BMPFileIO.cpp`) → indexed PNG.
- `ART/palettes/*.pal` (768-byte RGB) → commit under `assets/palettes/`.
- WAV → mp3 conversion script (ffmpeg): `SOUND/*.WAV` → `assets/sfx/`,
  `Music/Track02..08.wav` → `demo/public/assets/music/`.

## Phase 2 — Deterministic foundations

- `Fixed` (16.16) port of `Fixed.hpp` with MSVC-exact mul/div/truncation.
- `FixedTrig` tables (`ATBLSIZEBITS 6`) — sin/cos/atan bit-exact.
- Burgerlib RNG port from `rand.cpp`.
- 30 Hz `HeartBeat` accumulator (`ReadAndClear` semantics).
- Settings store trait (JSON; file on native, localStorage on wasm)
  replacing binary `Astro.cfg`.

## Phase 3 — Indexed compositor

- `Frame` (8-bit indexed) + `VirtualFrame` (2048×1024 wrapping world,
  `MovePointToCenter` camera).
- Blit modes: NORMAL, TRANSPARENT0 (+reverse), REMAP_SOURCE,
  REMAP_DEST_ON_1, COMBINE_64K translucency; RLE-compressed variants
  (`Blit.cpp`, `BlitCompressed.hpp`, `FrameCompress.hpp`).
- Palette: game palette, fades (`FadeBlit[16]`), remap table generation,
  translucency LUT build (`Palette.cpp`), gamma.
- Indexed → RGBA present through agg-gui each frame.
- Milestone: title bitmap + star field rendered native + wasm.

## Phase 4 — Sprite layer

- `FrameSequence` loader (PNG + JSON from Phase 1).
- `Sprite` port (`sprite.cpp`): CFixed position/delta, frame advance,
  rotation index, HP, timeout, `CollideOnBits` bit-level collision.
- `SpriteList` as Vec arena with `do_to_all` / collide callbacks.
- `GameSystem` trait replacing the `GameObj` function-pointer vtable.

## Phase 5 — Playable core

- `players.cpp`, `pship.cpp` (ship physics, guns, shield, bombs),
  `shots.cpp`, `thrust.cpp`, `rocks.cpp` (big/med/small splitting),
  `Explosion.cpp`, `spawnfx.cpp`.
- Game state machine from `AstroRock.cpp` (`STATE_*`), level/score/bonus,
  star field, stat bar, radar (`radar.cpp`).
- Milestone: playable single-player game.

## Phase 6 — Enemies + goodies

- `gloops.cpp`, `hk.cpp`, `bomber.cpp`, `SpikeBall.cpp`, `fastdeth.cpp`
  (each: `SpriteAI` + its extracted cfg), `goodies.cpp` powerups.

## Phase 7 — Audio

- `AudioPlatform` trait in core; rodio impl (native), web-sys
  AudioContext impl (wasm).
- Mixer policy port from `SoundWin95.cpp`: priorities
  (OPTIONAL/IMPORTANT/VITAL), max voices, multi-play copy pool,
  pan/volume/frequency, `PausedSoundPlayer`.
- Music: per-level mp3 tracks streamed/fetched (replaces the
  `Astro.Rck` PCM stream of `StreamSoundW95.cpp`).

## Phase 8 — Faithful bitmap UI

- Bitmap `Font` (`Font.cpp`, fonts are FrameSequences), `region.cpp` →
  `button.cpp` (2-state bitmap buttons) → `DragButton.cpp` (sliders).
- `StartScreen.cpp` (2018 lines): main menu, options, key/gamepad
  config, credits, help, high-score entry — original bitmaps, original
  look.
- `HighScore.cpp`, intermission/pause/game-over flows, `text.hpp`
  strings → `text.rs`.

## Phase 9 — Demo regression suite

- Demo `.dat` reader (`LoadADemo`/`SaveADemo` in `AstroRock.cpp`,
  30 Hz input stream + periodic checksums). Use the loose
  `demo/*.dat` (Apr 1997, matches final code) — the rez's rDemo00..28
  are older recordings from the Jan 1997 build and differ.
- Headless replay harness: run the sim, assert every checksum, for all
  27 shipped demos, in `cargo test`.
- Attract mode (demo playback from the title screen).

## Phase 10 — Polish + ship

- Gamma/fade options, windowed/fullscreen toggle.
- Pages deploy verified on desktop + phone; README hero screenshot.

## Deferred (not scheduled)

- Multiplayer: DirectPlay replaced by a modern transport. The
  determinism substrate (checksums, state serializers ported as part of
  demo support) is the prerequisite and is kept healthy by Phase 9.
