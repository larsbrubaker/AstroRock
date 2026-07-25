# AstroRock port — remaining work

This document tracks ONLY work that remains. Nothing listed here is done.
As items complete, delete them — in the same commit that finishes the
work. If this file ever describes finished work, that's a bug. Use
`git log` for history.

## Phase 3 — Indexed compositor (remaining)

- Palette fades (`FadeBlit[16]`), gamma, and the runtime translucency
  LUT build (`MakeTranslucentLookup` + `FindClosestColor` in
  `Palette.cpp`) — land with the first consumers (screen fades between
  states; COMBINE_64K users like shields/explosions).

## Phase 5 — Playable core (remaining)

- `spawnfx.cpp` — the enemy respawn shimmer (fade-in via `FadeBlit[16]`
  + LocalRand sparkles). Needs the palette fade tables; port together.
  (Note: player spawns have no protection in the original either — a
  rock on the spawn point kills you there too.)
- `players.cpp` remainder: the full `UpdateAll` ordering (speaker
  sprite, hurt/carnage voice timing via LocalRand, untouched/survival
  bonus flags), `PlayersCollidePlayers` (deferred with net), stat bar
  (`printStat`, lives/health/shield readouts — needs bitmap fonts from
  Phase 8; radar is placed placeholder-center-bottom until then).
- Intermission/level-advance flow and score bonuses from
  `AstroRock.cpp` (currently levels hard-cut to the next reset).
- Exact `UpdateAll` call order pass — align update/collide sequence
  with `AstroRock.cpp` line-for-line before demo replay (Phase 9
  depends on it).

## Phase 7 — Audio

- Spikeball charge whine: looping rSpikeBallChargeSnd with the
  per-beat rising frequency ramp ((f>>6)+f from 22050) — needs loop +
  live-frequency support in the AudioPlatform trait.

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
- Settings store trait (JSON; file on native, localStorage on wasm)
  replacing binary `Astro.cfg` — key bindings, volumes, high scores.

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
