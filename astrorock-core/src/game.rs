//! # The game — states, 30 Hz simulation, and frame composition
//!
//! The platform-free heart of the port (`AstroRock.cpp`'s main loop),
//! owned by the `TitleScreen` widget in `title_screen.rs`. States:
//!
//! - **Attract**: star field, drifting level-1 rocks, and the original
//!   "Press Enter" art through `RedBlit`. (The shareware teaser/upsell
//!   screen is retired — Logicware's address and phone are long gone.)
//! - **Playing**: the ported `UpdateAll`/`DrawPlayField` core, with the
//!   `NeedToAddLocalPlayer` press-Enter spawn gate.
//! - **Intermission**: the level-clear iris plus the bonus tally
//!   (intermission.rs).
//! - **GameOver**: the endgame overlay over the still-running world.
//!
//! Keys route through `input.rs` (the shipped `Astro.cfg` defaults plus
//! classic alternates); Enter starts/spawns/skips.

use agg_gui::event::Key;
use web_time::Instant;

use crate::assets;
use crate::audio::{self, AudioSink, LoopKind};
use crate::bombers::Bombers;
use crate::collide::{self, CollideCtx};
use crate::events::{Events, GameEvent};
use crate::explosion::Explosions;
use crate::fastdeaths::FastDeaths;
use crate::frame::Frame;
use crate::gloops::Gloops;
use crate::goodies::Goodies;
use crate::heartbeat::HeartBeat;
use crate::hks::Hks;
use crate::input::{self, Binding, KeysHeld};
use crate::intermission::{Intermission, LevelStats};
use crate::palette::{FadeBlits, Palette};
use crate::pship::{PlayerShip, ShipInputs};
use crate::radar::Radar;
use crate::rand::Rand;
use crate::rect::Rect;
use crate::rocks::Rocks;
use crate::spawnfx::{CompletedSpawn, SpawnFx, SpawnKind};
use crate::speaker::{self, Speaker};
use crate::spikeballs::SpikeBalls;
use crate::statbar::StatBar;
use crate::thrust::Thrust;
use crate::virtual_frame::VirtualFrame;

/// `#define NUMSTARS 50`
const NUM_STARS: usize = 50;
/// Back-buffer size (`SetTo640X480X8`).
pub const SCREEN_W: i32 = 640;
pub const SCREEN_H: i32 = 480;
/// World size (`CVirtualFrame PlayScreen1(2048, 1024)`).
pub const WORLD_W: i32 = 2048;
pub const WORLD_H: i32 = 1024;

/// `#define NUMSTARTSHIPS 3`
const NUM_START_SHIPS: u32 = 3;
/// `#define GAMEOVERPAUSE 600` — beats before game over times out.
const GAME_OVER_PAUSE: i32 = 600;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Screen {
    Attract,
    Playing,
    /// Level cleared: iris shut, then the bonus tally.
    Intermission,
    GameOver,
}

pub struct Game {
    pub(crate) screen: Frame,
    pub(crate) world: VirtualFrame,
    pub palette: Palette,
    /// `FadeBlit[NUMFADES]` — built once from the game palette.
    pub(crate) fades: FadeBlits,
    pub(crate) press_enter: Frame,
    pub(crate) transred: [u8; 256],
    pub(crate) stars: Vec<(i32, i32)>,
    pub(crate) rocks: Rocks,
    pub(crate) gloops: Gloops,
    pub(crate) hks: Hks,
    pub(crate) bombers: Bombers,
    pub(crate) spikeballs: SpikeBalls,
    pub(crate) fastdeaths: FastDeaths,
    pub(crate) spawnfx: SpawnFx,
    pub(crate) speaker: Speaker,
    pub(crate) goodies: Goodies,
    pub(crate) explosions: Explosions,
    pub(crate) events: Events,
    pub(crate) net_rand: Rand,
    pub(crate) local_rand: Rand,
    pub(crate) heartbeat: HeartBeat,
    pub(crate) started: Instant,
    pub(crate) state: Screen,
    pub(crate) keys: KeysHeld,
    pub(crate) enter_pressed: bool,
    pub(crate) ship: PlayerShip,
    pub(crate) thrust: Thrust,
    pub(crate) radar: Radar,
    pub(crate) statbar: StatBar,
    pub(crate) stats: LevelStats,
    pub(crate) inter: Intermission,
    pub(crate) endgame: Frame,
    pub(crate) level: usize,
    pub(crate) local_player_dead: bool,
    /// `NeedToAddLocalPlayer` — press Enter to spawn.
    pub(crate) need_add_player: bool,
    pub(crate) game_over_pause: i32,
    /// `ResetMusicFrequencyDelay` + whether the slowdown engaged.
    music_freq_delay: i32,
    music_slow: bool,
    pub(crate) audio: Option<Box<dyn AudioSink>>,
    /// Chrome-bar toggles.
    pub music_on: bool,
    pub sfx_on: bool,
}

impl Game {
    pub fn new(audio: Option<Box<dyn AudioSink>>) -> Self {
        // `STARInit`: NetRand(2048) x NetRand(1024); NetRand default-
        // constructs with seed 0, so the field matches the original's
        // first frame.
        let mut net_rand = Rand::new();
        let stars = (0..NUM_STARS)
            .map(|_| {
                let x = net_rand.rand(WORLD_W as u32) as i32;
                let y = net_rand.rand(WORLD_H as u32) as i32;
                (x, y)
            })
            .collect();

        // `OnScreenRect`: the play field is the screen above the stat
        // bar (640x384), set once the bar art is loaded — exactly like
        // `StatBarTop = pScreen->Height - StatBarFrame.Height`.
        let statbar = StatBar::new();
        let mut world = VirtualFrame::new(WORLD_W, WORLD_H);
        world.set_on_screen_rect(Rect::new(0, 0, SCREEN_W, SCREEN_H - statbar.height()));
        world.move_point_to_center(WORLD_W / 2, WORLD_H / 2);

        let mut rocks = Rocks::new();
        rocks.reset(0, &mut net_rand);

        // Attract-mode cosmetics: give the speaker a random drift
        // start too (start_game reseeds, so the contract is unhurt).
        let mut speaker = Speaker::new();
        speaker.reset(WORLD_W as u32, WORLD_H as u32, &mut net_rand);

        let palette = assets::game_palette();
        let fades = FadeBlits::new(&palette);

        Self {
            screen: Frame::new(SCREEN_W, SCREEN_H),
            world,
            palette,
            fades,
            press_enter: assets::frame_from_indexed_png(assets::PRESS_ENTER_PNG),
            transred: assets::remap_table(assets::TRANSRED_PAL),
            stars,
            rocks,
            gloops: Gloops::new(),
            hks: Hks::new(),
            bombers: Bombers::new(),
            spikeballs: SpikeBalls::new(),
            fastdeaths: FastDeaths::new(),
            spawnfx: SpawnFx::new(),
            speaker,
            goodies: Goodies::new(),
            explosions: Explosions::new(),
            events: Events::new(),
            net_rand,
            local_rand: Rand::new(),
            heartbeat: HeartBeat::new(0),
            started: Instant::now(),
            state: Screen::Attract,
            keys: KeysHeld::default(),
            enter_pressed: false,
            ship: PlayerShip::new(),
            thrust: Thrust::new(),
            radar: Radar::new(),
            statbar,
            stats: LevelStats::new(),
            inter: Intermission::new(),
            endgame: assets::frame_from_indexed_png(assets::ENDGAME_PNG),
            level: 0,
            local_player_dead: false,
            need_add_player: false,
            game_over_pause: 0,
            audio,
            music_on: true,
            sfx_on: true,
            music_freq_delay: 0,
            music_slow: false,
        }
    }

    /// Milliseconds since construction — the widget's paint clock.
    pub fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn clip() -> Rect {
        Rect::new(0, 0, WORLD_W, WORLD_H)
    }

    /// `OnScreenRect` — the play field above the stat bar.
    pub(crate) fn on_screen(&self) -> Rect {
        Rect::new(0, 0, SCREEN_W, SCREEN_H - self.statbar.height())
    }

    /// The live enemy count (`NumBadGuys`) — rocks deliberately not
    /// included; leftover rocks only cost the annihilation bonus.
    fn enemies_alive(&self) -> u32 {
        self.gloops.num_gloops
            + self.hks.num_hks
            + self.bombers.num_bombers
            + self.spikeballs.num_spikeballs
            + self.fastdeaths.num_fast_deaths
    }

    /// Start a fresh game (Enter from attract) — `NewGame`.
    fn start_game(&mut self) {
        self.net_rand.seed(0);
        self.level = 0;
        self.ship = PlayerShip::new();
        self.ship.reset(NUM_START_SHIPS);
        self.stats.reset(0);
        self.new_level();
        self.local_player_dead = false;
        self.game_over_pause = 0;
        self.state = Screen::Playing;
    }

    /// `NewLevel` — reset the world, then wait for Enter to spawn
    /// (`NeedToAddLocalPlayer`).
    fn new_level(&mut self) {
        self.reset_level();
        self.ship.sprite.visible = false;
        self.need_add_player = true;
        // `NeedNumBadGuys`: the tally's "Bad Guys Killed" is the count
        // present at level start.
        self.stats.bad_guys_killed = self.enemies_alive() as i32;
    }

    /// `ResetAll` in the original's exact call order — the RNG draw
    /// order is part of the determinism contract: Rocks, Gloops,
    /// SpikeBalls, HKs, Bombers, FastDeaths, Goodies, Explosions,
    /// then the speaker's random position.
    fn reset_level(&mut self) {
        self.rocks.reset(self.level, &mut self.net_rand);
        self.gloops.reset(self.level, &mut self.net_rand);
        self.spikeballs.reset(self.level, &mut self.net_rand);
        self.hks.reset(self.level, &mut self.net_rand);
        self.bombers.reset(self.level, &mut self.net_rand);
        self.fastdeaths.reset(self.level, &mut self.net_rand);
        self.goodies.reset(self.level, &mut self.net_rand);
        self.explosions.reset();
        self.spawnfx = SpawnFx::new();
        self.speaker
            .reset(WORLD_W as u32, WORLD_H as u32, &mut self.net_rand);
    }

    /// Respawn after death, lives permitting (Enter while dead).
    fn respawn(&mut self) {
        self.ship.new_ship();
        self.ship.sprite.x_pos = self.net_rand.rand(WORLD_W as u32) as f32;
        self.ship.sprite.y_pos = self.net_rand.rand(WORLD_H as u32) as f32;
        self.ship.sprite.visible = true;
        self.local_player_dead = false;
    }

    /// Run the 30 Hz simulation up to `now_ms`.
    pub fn advance(&mut self, now_ms: u64) {
        let clip = Self::clip();
        let beats = self.heartbeat.read_and_clear(now_ms);
        for _ in 0..beats {
            match self.state {
                Screen::Attract => {
                    if self.enter_pressed {
                        self.enter_pressed = false;
                        self.start_game();
                        continue;
                    }
                    self.rocks.update(&clip, &mut self.net_rand);
                    self.explosions.update(&clip, &mut self.net_rand);
                }
                Screen::Playing => {
                    self.sim_beat(clip);
                    self.playing_transitions();
                }
                Screen::Intermission => self.intermission_beat(clip),
                Screen::GameOver => {
                    // `AdvanceFrames` keeps the world running under
                    // the GAME OVER overlay; Enter (or the 20s pause)
                    // returns to attract.
                    self.sim_beat(clip);
                    self.game_over_pause += 1;
                    if self.enter_pressed || self.game_over_pause >= GAME_OVER_PAUSE {
                        self.level = 0;
                        self.reset_level();
                        self.state = Screen::Attract;
                    }
                }
            }
            // An Enter press is a one-beat edge: whatever didn't
            // consume it this beat doesn't get it later (`FlushKeys`).
            self.enter_pressed = false;
        }
        self.pump_audio();
    }

    /// One beat of `UpdateAll` — runs while Playing, during the
    /// intermission iris, and under the GAME OVER overlay.
    fn sim_beat(&mut self, clip: Rect) {
        self.ship.set_inputs(ShipInputs {
            left: self.keys.left,
            right: self.keys.right,
            thrust: self.keys.thrust,
            shield: self.keys.shield,
            fire: self.keys.fire,
            bomb: self.keys.bomb,
        });

        self.rocks.update(&clip, &mut self.net_rand);
        self.explosions.update(&clip, &mut self.net_rand);
        // UpdateAll order: spawn effects, Gloops, SpikeBalls, HKs,
        // Bombers, FastDeaths.
        if let Some(CompletedSpawn {
            kind: SpawnKind::FastDeath,
            x,
            y,
            cur_frame,
        }) = self.spawnfx.update(&mut self.net_rand)
        {
            self.fastdeaths.spawn_one(x, y, cur_frame);
        }
        self.gloops
            .update(&clip, &mut self.net_rand, &self.world, &self.ship.sprite);
        self.spikeballs.update(
            &clip,
            &mut self.net_rand,
            &self.world,
            &mut self.explosions,
            &mut self.events,
        );
        self.hks.update(
            &clip,
            &mut self.net_rand,
            &self.world,
            &self.ship.sprite,
            &mut self.events,
        );
        self.bombers.update(
            &clip,
            &mut self.net_rand,
            &self.world,
            &self.ship.sprite,
            &mut self.explosions,
            &mut self.events,
        );
        self.fastdeaths.update(
            &clip,
            &mut self.net_rand,
            &self.world,
            &self.ship.sprite,
            &mut self.spawnfx,
            &mut self.events,
        );
        self.goodies.update(&clip, &mut self.net_rand);
        // `pSpeakerSprite->Update()` — right before the collide pass.
        self.speaker.update(&clip, &mut self.net_rand);

        // PlayersCollideObject order: Rocks, Gloops, SpikeBalls, HKs,
        // Bombers, FastDeaths, then Goodies.
        {
            let mut ctx = CollideCtx {
                world: &self.world,
                explosions: &mut self.explosions,
                events: &mut self.events,
                net_rand: &mut self.net_rand,
                goodies: &mut self.goodies,
                stats: &mut self.stats,
                clip,
            };
            if collide::player_vs_rocks(&mut self.ship, &mut self.rocks, &mut ctx) {
                self.local_player_dead = true;
            }
            if collide::player_vs_gloops(&mut self.ship, &mut self.gloops, &mut ctx) {
                self.local_player_dead = true;
            }
            if collide::player_vs_spikeballs(&mut self.ship, &mut self.spikeballs, &mut ctx) {
                self.local_player_dead = true;
            }
            if collide::player_vs_hks(&mut self.ship, &mut self.hks, &mut ctx) {
                self.local_player_dead = true;
            }
            if collide::player_vs_bombers(&mut self.ship, &mut self.bombers, &mut ctx) {
                self.local_player_dead = true;
            }
            if collide::player_vs_fastdeaths(&mut self.ship, &mut self.fastdeaths, &mut ctx) {
                self.local_player_dead = true;
            }
        }
        // PlayersCollideObject(&Goodies, 0) — pickups last.
        self.goodies.collide_with_player(
            &mut self.ship,
            &clip,
            &mut self.net_rand,
            &mut self.events,
        );

        // The speaker pass: everything dies on its grille; the player
        // just gets bumped — and any touch is a `SkipMusic`.
        {
            let mut ctx = CollideCtx {
                world: &self.world,
                explosions: &mut self.explosions,
                events: &mut self.events,
                net_rand: &mut self.net_rand,
                goodies: &mut self.goodies,
                stats: &mut self.stats,
                clip,
            };
            speaker::speaker_vs_world(
                &self.speaker,
                &mut self.rocks,
                &mut self.gloops,
                &mut self.spikeballs,
                &mut self.hks,
                &mut self.bombers,
                &mut self.fastdeaths,
                &mut ctx,
            );
            let (touched, died) =
                speaker::player_vs_speaker(&mut self.ship, &self.speaker, &mut ctx);
            if touched {
                // `SkipMusic`: only sustained contact engages the
                // slowdown; every touch restarts the 90-count.
                if self.music_freq_delay != 0 {
                    self.music_slow = true;
                }
                self.music_freq_delay = 90;
            }
            if died {
                self.local_player_dead = true;
            }
        }

        self.ship.update(
            &clip,
            &mut self.net_rand,
            &self.world,
            &mut self.explosions,
            &mut self.events,
            &mut self.stats,
        );

        // `PlayersUpdate`: any beat with the shield up forfeits the
        // no-shielding bonus.
        if self.ship.shield_on {
            self.stats.no_shielding = 0;
        }
    }

    /// Drain the beat's events into the platform sink.
    fn pump_audio(&mut self) {
        // `ResetMusicFrequencyDelay` counts down in the original's
        // render loop (framerate-paced, like this pump); at zero the
        // stream returns to 22050 Hz.
        if self.music_freq_delay > 0 {
            self.music_freq_delay -= 1;
            if self.music_freq_delay == 0 {
                self.music_slow = false;
            }
        }
        // Turn the frame's events into sound; drain regardless so the
        // queue can't grow unbounded when running silent or muted.
        if let Some(sink) = self.audio.as_deref_mut() {
            if self.sfx_on {
                audio::dispatch(&mut self.events, sink, &mut self.local_rand);
            } else {
                for _ in self.events.drain() {}
            }
            let playing = self.state == Screen::Playing;
            let alive = playing && self.ship.sprite.visible;
            sink.set_loop(
                LoopKind::Thrust,
                self.sfx_on && alive && self.ship.thrusting,
            );
            sink.set_loop(
                LoopKind::Shield,
                self.sfx_on && alive && self.ship.shield_on,
            );
            // The original starts the music stream at init and restarts
            // it from the main loop forever — attract mode included.
            sink.set_music(self.music_on);
            sink.set_music_slow(self.music_slow);
        } else {
            for _ in self.events.drain() {}
        }
    }

    /// The `STATE_PLAYING` arm: spawn gate, level end, death.
    fn playing_transitions(&mut self) {
        // `NeedToAddLocalPlayer` + `PressedContinue` -> `AddPlayer`.
        if self.need_add_player && self.enter_pressed {
            self.respawn();
            self.need_add_player = false;
        }

        // `NumBadGuys == 0` -> `SetStateIntermission`. Rocks don't
        // count — leftovers only zero the annihilation bonus.
        if self.enemies_alive() == 0 {
            let rocks_left = (self.rocks.num_big + self.rocks.num_med + self.rocks.num_lit) as i32;
            self.inter.begin(&mut self.stats, rocks_left);
            self.state = Screen::Intermission;
        }

        if self.local_player_dead {
            self.local_player_dead = false;
            if self.ship.num_ships == 0 {
                self.world.set_on_screen_rect(self.on_screen());
                self.game_over_pause = 0;
                self.state = Screen::GameOver;
            } else {
                self.need_add_player = true;
            }
        }
    }

    /// The `STATE_INTERMISSION` arm: the iris (sim still running),
    /// then the sliding tally counting the bonus into the score.
    fn intermission_beat(&mut self, clip: Rect) {
        let on_screen = self.on_screen();
        if self.inter.close_level > 0 {
            self.sim_beat(clip);
            self.inter.close_level -= 1;
            if self.inter.close_level > 0 {
                let iris = self.inter.shrink_rect(&on_screen);
                self.world.set_on_screen_rect(iris);
            } else {
                self.world.set_on_screen_rect(on_screen);
                self.level += 1;
                self.new_level();
                self.inter
                    .update_slide(on_screen.width(), on_screen.height());
            }
        } else if self.enter_pressed || self.inter.raising == 0 || self.inter.total_bonus == 0 {
            // Skip or done: bank the remainder and play on
            // (`ResetIntermisionInfo` on the way out).
            self.ship.add_score(self.inter.total_bonus.max(0) as u32);
            self.inter.total_bonus = 0;
            self.stats.reset(self.level as u32);
            self.stats.bad_guys_killed = self.enemies_alive() as i32;
            self.state = Screen::Playing;
        } else {
            self.inter
                .update_slide(on_screen.width(), on_screen.height());
            let (add, blip) = self.inter.count_step();
            if add > 0 {
                self.ship.add_score(add);
            }
            if blip {
                self.events.push(GameEvent::SfxBonus);
            }
        }
    }

    /// The composed indexed back buffer (the widget's upload source,
    /// tests, and the `dump_frame` inspection example).
    pub fn screen(&self) -> &Frame {
        &self.screen
    }

    /// Route a key through the bindings table (input.rs).
    pub fn set_key(&mut self, key: &Key, down: bool) -> bool {
        match input::binding(key) {
            Some(Binding::Left) => self.keys.left = down,
            Some(Binding::Right) => self.keys.right = down,
            Some(Binding::Thrust) => self.keys.thrust = down,
            Some(Binding::Fire) => self.keys.fire = down,
            Some(Binding::Shield) => self.keys.shield = down,
            Some(Binding::Bomb) => self.keys.bomb = down,
            Some(Binding::Start) => {
                if down {
                    self.enter_pressed = true;
                }
            }
            None => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Step the simulation n beats (33.4ms each).
    fn step(g: &mut Game, from_ms: u64, beats: u64) -> u64 {
        let target = from_ms + beats * 1000 / 30 + 2;
        g.advance(target);
        target
    }

    #[test]
    fn attract_composes_stars_rocks_and_teaser() {
        let mut g = Game::new(None);
        g.compose();
        let screen = g.screen();
        let stars = screen.bits.iter().filter(|&&b| b == 15).count();
        assert!(stars > 0, "no stars plotted");

        let cx = SCREEN_W / 2;
        let cy = SCREEN_H / 2;
        let mut non_zero = 0;
        for y in (cy - 100)..(cy + 100) {
            for x in (cx - 140)..(cx + 140) {
                if screen.get(x, y) != 0 {
                    non_zero += 1;
                }
            }
        }
        assert!(non_zero > 1000, "teaser not composed: {non_zero}");
    }

    /// Enter twice: attract -> game, then through the
    /// `NeedToAddLocalPlayer` gate to spawn the ship.
    fn start_and_spawn(g: &mut Game) -> u64 {
        g.set_key(&Key::Enter, true);
        let now = step(g, 0, 1);
        g.set_key(&Key::Enter, false);
        g.set_key(&Key::Enter, true);
        let now = step(g, now, 1);
        g.set_key(&Key::Enter, false);
        now
    }

    #[test]
    fn enter_starts_a_game_with_three_ships() {
        let mut g = Game::new(None);
        g.set_key(&Key::Enter, true);
        let now = step(&mut g, 0, 1);
        assert!(g.state == Screen::Playing);
        assert_eq!(g.ship.num_ships, NUM_START_SHIPS);
        // The press-enter spawn gate (`NeedToAddLocalPlayer`).
        assert!(!g.ship.sprite.visible);
        assert!(g.need_add_player);
        g.set_key(&Key::Enter, false);
        g.set_key(&Key::Enter, true);
        let _ = step(&mut g, now, 1);
        assert!(g.ship.sprite.visible);

        // The ship composes onto the screen at the play-field center.
        g.compose();
        let cy = (SCREEN_H - g.statbar.height()) / 2;
        let mut ship_pixels = 0;
        for y in (cy - 30)..(cy + 30) {
            for x in (SCREEN_W / 2 - 30)..(SCREEN_W / 2 + 30) {
                if g.screen().get(x, y) != 0 {
                    ship_pixels += 1;
                }
            }
        }
        assert!(
            ship_pixels > 50,
            "ship not visible at center: {ship_pixels}"
        );
    }

    #[test]
    fn firing_can_break_rocks_and_score() {
        let mut g = Game::new(None);
        let mut now = start_and_spawn(&mut g);

        // Park the ship on top of the first visible big rock and fire
        // point-blank until it shatters into mediums.
        let idx = g.rocks.big().iter().position(|s| s.visible).unwrap();
        let (rx, ry) = (g.rocks.big()[idx].x_pos, g.rocks.big()[idx].y_pos);
        for _ in 0..60 {
            g.ship.sprite.x_pos = rx;
            g.ship.sprite.y_pos = ry;
            g.ship.sprite.x_delta = 0.0;
            g.ship.sprite.y_delta = 0.0;
            g.ship.sprite.hp = 9999; // survive the ram for this test
            g.set_key(&Key::Char('m'), true);
            now = step(&mut g, now, 1);
            g.set_key(&Key::Char('m'), false);
            now = step(&mut g, now, 1);
            if g.rocks.num_med > 0 {
                break;
            }
        }
        assert!(g.rocks.num_med > 0, "big rock never split");
        assert!(g.ship.score > 0, "no score awarded");
        // Hits were tallied for the intermission stats.
        assert!(g.stats.shots_fired > 0);
        assert!(g.stats.shots_hit > 0);
    }

    #[test]
    fn ramming_rocks_kills_the_ship_eventually() {
        let mut g = Game::new(None);
        let mut now = start_and_spawn(&mut g);
        let start_ships = g.ship.num_ships;
        assert_eq!(start_ships, NUM_START_SHIPS);

        for _ in 0..600 {
            if let Some(idx) = g.rocks.big().iter().position(|s| s.visible) {
                g.ship.sprite.x_pos = g.rocks.big()[idx].x_pos;
                g.ship.sprite.y_pos = g.rocks.big()[idx].y_pos;
            }
            now = step(&mut g, now, 1);
            // Death re-arms the spawn gate (`NeedToAddLocalPlayer`).
            if g.need_add_player {
                break;
            }
        }
        assert!(g.need_add_player, "ship survived 600 beats of ramming");
        assert_eq!(g.ship.num_ships, start_ships - 1);
        // Dying zeroed the survival bonus and counted the life.
        assert_eq!(g.stats.survival, 0);
        assert_eq!(g.stats.lives_lost, 1);

        // Enter respawns with a fresh hull.
        g.set_key(&Key::Enter, true);
        step(&mut g, now, 2);
        assert!(g.ship.sprite.visible);
        assert_eq!(g.ship.sprite.hp, 100);
    }

    #[test]
    fn intermission_irises_advances_the_level_and_pays_the_bonus() {
        let mut g = Game::new(None);
        let mut now = start_and_spawn(&mut g);
        let level_before = g.level;
        let score_before = g.ship.score;

        // Enter the intermission as if the last enemy just died.
        g.inter.begin(&mut g.stats, 0);
        g.state = Screen::Intermission;
        assert_eq!(
            g.inter.close_level,
            crate::intermission::CLOSE_LEVEL_DURATION
        );

        // Ride the iris out one beat at a time (the heartbeat caps
        // beats per read): the next level resets behind it.
        for _ in 0..crate::intermission::CLOSE_LEVEL_DURATION + 2 {
            now = step(&mut g, now, 1);
        }
        assert_eq!(g.level, level_before + 1);
        assert!(g.need_add_player, "next level should re-arm the spawn gate");

        // The tally slides down, counts the bonus into the score, and
        // play resumes.
        let mut guard = 0;
        while g.state == Screen::Intermission {
            now = step(&mut g, now, 1);
            guard += 1;
            assert!(guard < 400, "intermission never ended");
        }
        assert!(g.state == Screen::Playing);
        assert!(g.ship.score > score_before, "bonus never reached the score");
        // `ResetIntermisionInfo` re-primed the pots for the new level.
        assert_eq!(g.stats.survival, 200 + 50 * g.level as i32);
    }
}
