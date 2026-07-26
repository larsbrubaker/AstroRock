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
  freeze while paused) — lands with the Phase 8 pause overlay.
  Ctrl+S/Ctrl+M mute keys covered by the chrome toggles until then.
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

## Phase 8 — Faithful bitmap UI (IN PROGRESS)

Landed (menu.rs): boot into the shipped start screen (start.png with
its own palette), 2-state bitmap buttons at the 1997 coordinates
firing on release with the click sound, Enter-starts, the config
page's start-level picker, credits, really-quit confirm, the Demo
button playing an embedded recording, the bad-guy showcase monitor
(subjects cycling behind TV static with the FadeBlit fade-in and
static sound), and (modern, by request) Esc in-game pausing into the
config page — which grows a Quit button routing through the
`STATE_REALLYENDGAME` confirm to GAME OVER and back to the menu.

Also landed: Config Controls (`STATE_CONFIG_KEYS`/`STATE_GETAKEY`
with CheckAndSwap), Config Sound (DragButton volume sliders), the
JSON settings store (file native / localStorage wasm), and the
mobile virtual gamepad (touch holds + tilt steering, touch_input.rs,
backed by agg-gui touch/tilt plumbing).

- Help pages (`ppHelpText` from text.hpp) behind the Help button —
  including the showcase's manual subject picker (`SwitchBadGuy` gate
  during `STATE_HELP`, HelpLeft/HelpRight buttons).
- High scores: `HighScore.cpp` list + entry + View High button;
  HighestLevelReached gating the start-level picker (store fields
  exist).
- Joystick/gamepad config (the 1997 joy half of Config Controls).
- Pause overlay (`STATE_PAUSE`, pause.png, Pause key, FastDeaths
  freeze) — Esc-options covers most of the need already.
- Attract: auto-play a demo after idle time on the main screen
  (original `STATE_MAIN` timeout), once demo parity is proven.
- Verify mobile touch/tilt on a real device: iOS sensor-permission
  prompt (first tap), landscape/portrait zone layout, tilt axis
  mapping (screen.orientation fold-in), FA glyphs f132/f05b/f135
  present in the embedded fa.ttf.

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
- THE INSTRUMENTED C++ REFERENCE (the root-cause tool): a headless
  x86 `/arch:IA32` build of the original sim with Burgerlib stubbed
  (~200-line surface: typedefs, KeyArray, LoadAResource fread-by-id
  from the dump-rez payloads) and the five platform files
  (HalWin95/SoundWin95/StreamSoundW95/ScreenDD/Ddwindow) replaced by
  no-ops, driving the game's own demo loop and printing per-beat
  sync + CheckPlayField. `astrorock-tools dump-rez` landed; the
  build scaffold lives in the session scratchpad (`cppbuild/`) —
  fold the stubs + build scripts into a local-only `cppref/` dir
  once the trace runs (never commit copied game sources).
  NOTE: the shipped-game backup `C:\Development\Backups\2097-05`
  referenced in CLAUDE.md does NOT exist on this machine — no exe to
  run side-by-side; the headless build is the only reference path.
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
