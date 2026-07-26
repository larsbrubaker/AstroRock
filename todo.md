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
  mapping (screen.orientation fold-in), orientation lock while
  fullscreen, joystick rest-plane recalibration on pad release,
  thumb-override steering, FA glyphs f132/f05b/f135/f0c9 present in
  the embedded fa.ttf.

## Phase 9 — Demo regression suite (IN PROGRESS)

Landed: parser (`demo.rs` — the files carry NO recorded checksums;
CHECK_DEMO was compiled out in 1997), `init_demo`/`demo_beat`
playback, the full `CheckPlayField` port, and the golden-checksum
harness (`tests/demo_replay.rs`, bless with ASTROROCK_BLESS_DEMOS=1).
All 31 demos replay panic-free and deterministically; the current
golden pins OUR determinism only.

THE INSTRUMENTED C++ REFERENCE EXISTS AND RUNS (local-only, never
committed): `C:\Development\AstroRock-headless\` holds the headless
x86 `/arch:IA32 /fp:precise` build of the original sim (Burgerlib
stubbed, five platform files no-op'd, demos served from the dump-rez
payloads) plus a Rust per-beat probe. `build.ps1` reproduces it from
the read-only reference; reruns are byte-identical. CRand cross-check
is EXACT against rand.rs lock-ins. Notable: the reference's
`CPlayerShip::Check/GetStateData/SetStateData` infinitely recurse
(call themselves where `CSprite::` base calls were intended) — that
code was unexecutable in 1997, so demos carry no checksums and the
check-byte has no ground truth (rust = cpp + 36 mod 256, constant,
from differing readings of the broken method; SYNC is the reliable
signal). The shipped-game backup `C:\Development\Backups\2097-05`
referenced in CLAUDE.md does NOT exist on this machine.

STRATEGY PIVOT (Lars, 2026-07-26): shipped demos become CAPTURES,
not simulations. Capture a faithful C++ run per demo — per beat:
every visible sprite's (system, subtype, animation frame, x, y),
the sounds triggered, and the statbar/score line — and the Rust
side plays that back like a compressed video. Demos then survive
any future gameplay changes/improvements; a Rust recorder writing
the same format later makes NEW demos. Bit-exact 1997 parity stops
being a demo requirement (the determinism substrate stays healthy
for future net play via the our-determinism golden).

KNOWN BLOCKER: the rebuilt C++ replay's pilot ALSO dies — all three
traced demos ran exactly +30 beats past their recorded length (the
`!visible * 30` grace), so modern MSVC x87 codegen diverges from
MSVC 4.x too. A compiler-flag sweep (/Od, /fp:fast, /O1, x87
control-word variants) is testing whether some combo reproduces
1997 — the fitness signal is a demo finishing at EXACTLY its
recorded length with the ship alive. Fallback if none does: restore
the shipped-exe backup (missing from this machine) and capture from
the real binary via instrumentation.

- Faithful C++ run: finish the flag sweep; if green, extend the
  headless driver to dump the capture stream for all 27+ demos.
- Capture format: compact + versioned (delta/RLE per beat); decided
  when the first real dump exists.
- Rust capture player: renders sprites at recorded positions and
  fires recorded sounds — no sim; replaces sim-replay for the Demo
  button and the attract mode. Keep `tests/demo_replay.rs` (the
  our-determinism golden) as the sim regression suite.
- Rust capture recorder (later): record new/improved demos in the
  same format.
- The sync-divergence root cause (collision timing, first f32 diff
  around demo00 beats 340-356) is now OPTIONAL polish for the sim's
  own fidelity, not a demo blocker.

## Phase 10 — Polish + ship

- Gamma/fade options, windowed/fullscreen toggle.
- Pages deploy verified on desktop + phone; README hero screenshot.

## Tuning (unblocked once demos are captures — deliberate departures)

- Early-level rock speed: feels fast vs memory; velocity math is
  verified faithful, so any change is a modernization knob. The
  capture pivot removes the replay-validity constraint.
- Rock splits: children currently get fully random velocity exactly
  like the C++ (`NetRandAbout0`, no parent term) — Lars wants them to
  inherit some parent momentum. Also unblocked by the capture pivot.

## Deferred (not scheduled)

- Multiplayer: DirectPlay replaced by a modern transport. The
  determinism substrate (checksums, state serializers ported as part of
  demo support) is the prerequisite and is kept healthy by Phase 9.
