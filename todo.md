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

- `players.cpp` remainder: `PlayersCollidePlayers` (deferred with
  net).
- Pause (`STATE_PAUSE`, Pause key + pause.png overlay, FastDeaths
  freeze while paused) and the Esc quit-confirm
  (`STATE_REALLYENDGAME`, reallyq.png, Y/N) — small states around
  the ported machine; land with Phase 8 menus. Ctrl+S/Ctrl+M mute
  keys covered by the chrome toggles until then.
- Known replay nuance for Phase 9: the original's level-end check
  reads `NumBadGuys` from the previous DRAW (render-paced); our
  live per-beat count is equivalent whenever rendering kept up with
  30 Hz (true on the dev machines that recorded the demos). If a
  demo diverges at a level boundary, this is the first suspect.

## Phase 7 — Audio (remaining)

- Spikeball charge whine: looping rSpikeBallChargeSnd with the
  per-beat rising frequency ramp ((f>>6)+f from 22050) — needs
  live-frequency support on `AudioSink` loops.
- Mixer policy from `SoundWin95.cpp` where it's audible: pan by screen
  x (`GetPosRelCenter`) on one-shots.
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

## Phase 9 — Demo regression suite (IN PROGRESS)

Landed: parser (`demo.rs` — the files carry NO recorded checksums;
CHECK_DEMO was compiled out in 1997), `init_demo`/`demo_beat`
playback, the full `CheckPlayField` port, and the golden-checksum
harness (`tests/demo_replay.rs`, bless with ASTROROCK_BLESS_DEMOS=1).
All 31 demos replay panic-free and deterministically; the current
golden pins OUR determinism only.

- ROOT CAUSE THE 1997 DESYNC: replays diverge from the recordings
  within ~100 beats (demo00: our pilot dies <100 beats into a
  1079-beat recording, score 0 — the recorded pilot survived to the
  end by definition of the stop condition). RNG primitive verified
  exact (count-then-early-out, warm-up counting). Bisect order:
  (1) init draw-count audit per reset (players 16, rocks 7*MAXBIG+
  5*visible, then gloops/spikeballs/hks/bombers/fastdeaths/goodies —
  each vs its cpp SetVisAndMove/Reset), (2) per-beat update draw
  audit in the same order, (3) sprite base Update (does CSprite::
  Update draw? our port takes rand — verify against sprite.cpp),
  (4) ship physics f32 shapes. A visual side-by-side with the
  shipped exe (backup runs demos in attract) would bisect init vs
  update instantly. `examples/demo_probe.rs` prints the timeline
  and dumps frames.
- Re-bless the golden once 1997 parity is confirmed; the difficulty
  audit (rock speed) is settled by the same event.
- Attract mode (demo playback from the title screen) — after parity.

## Phase 10 — Polish + ship

- Gamma/fade options, windowed/fullscreen toggle.
- Pages deploy verified on desktop + phone; README hero screenshot.

## Tuning (after Phase 9 proves the baseline — deliberate departures)

- Early-level rock speed: feels fast vs memory; velocity math is
  verified faithful, so any change is a modernization knob, decided
  once demos replay bit-exact.
- Rock splits: children currently get fully random velocity exactly
  like the C++ (`NetRandAbout0`, no parent term) — Lars wants them to
  inherit some parent momentum. Add as a tunable after the demo suite
  locks the faithful baseline (a toggle keeps replays valid).

## Deferred (not scheduled)

- Multiplayer: DirectPlay replaced by a modern transport. The
  determinism substrate (checksums, state serializers ported as part of
  demo support) is the prerequisite and is kept healthy by Phase 9.
