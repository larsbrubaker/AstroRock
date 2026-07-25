# AstroRock port — remaining work

This document tracks ONLY work that remains. Nothing listed here is done.
As items complete, delete them — in the same commit that finishes the
work. If this file ever describes finished work, that's a bug. Use
`git log` for history.

## Phase 3 — Indexed compositor (remaining)

- Screen fades between states (gamma ramp / `ScreenFadePalette`) and
  the runtime translucency LUT build (`MakeTranslucentLookup`) —
  land with the first consumers (state transitions; COMBINE_64K users
  like shields/explosions).

## Phase 5 — Playable core (remaining)

- `players.cpp` remainder: hurt/carnage/new-ship voice lines via
  `PausePlayerPlay` + LocalRand timing; `PlayersCollidePlayers`
  (deferred with net).
- Exact `UpdateAll` call order pass — align update/collide sequence
  with `AstroRock.cpp` line-for-line before demo replay (Phase 9
  depends on it).
- Pause (`STATE_PAUSE`, Pause key + pause.png overlay) and the Esc
  quit-confirm (`STATE_REALLYENDGAME`, reallyq.png, Y/N) — small
  states around the ported machine; land with Phase 8 menus.

## Phase 7 — Audio (remaining)

- Spikeball charge whine: looping rSpikeBallChargeSnd with the
  per-beat rising frequency ramp ((f>>6)+f from 22050) — needs
  live-frequency support on `AudioSink` loops.
- Mixer policy from `SoundWin95.cpp` where it's audible: pan by screen
  x (`GetPosRelCenter`) on one-shots, `PausedSoundPlayer` delay for
  goody voice lines.
- Volume/mute controls (M key, sliders) — land with Phase 8 options
  UI and the settings store.

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
