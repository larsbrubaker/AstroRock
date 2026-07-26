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
use crate::audio::{self, AudioSink, LoopKind, VoicePlayer};
use crate::bombers::Bombers;
use crate::collide::{self, CollideCtx};
use crate::events::Events;
use crate::events::GameEvent;
use crate::explosion::Explosions;
use crate::fastdeaths::FastDeaths;
use crate::frame::Frame;
use crate::gloops::Gloops;
use crate::goodies::Goodies;
use crate::heartbeat::HeartBeat;
use crate::hks::Hks;
use crate::input::{Binding, KeysHeld};
use crate::intermission::{Intermission, LevelStats};
use crate::menu::Menu;
use crate::palette::{FadeBlits, Palette};
use crate::pship::{PlayerShip, ShipInputs};
use crate::radar::Radar;
use crate::rand::Rand;
use crate::rect::Rect;
use crate::rocks::Rocks;
use crate::settings::{Settings, SettingsStore};
use crate::spawnfx::{CompletedSpawn, SpawnFx, SpawnKind};
use crate::speaker::{self, Speaker};
use crate::spikeballs::SpikeBalls;
use crate::statbar::StatBar;
use crate::thrust::Thrust;
use crate::touch_input::TouchHeld;
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
pub(crate) const NUM_START_SHIPS: u32 = 3;
/// `#define GAMEOVERPAUSE 600` — beats before game over times out.
const GAME_OVER_PAUSE: i32 = 600;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Screen {
    /// The start screen (`STATE_STARTGAME` + menu.rs pages).
    Menu,
    Playing,
    /// Level cleared: iris shut, then the bonus tally.
    Intermission,
    GameOver,
    /// `STATE_PLAYINGDEMO` — a shipped recording plays back.
    Demo,
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
    /// The rate actually sent to the sink (ramps back up on release).
    music_rate: f32,
    /// `PausedSoundPlayer` — the delayed one-liner slot, ticked per
    /// beat; the due line waits here for the next audio pump.
    voice: VoicePlayer,
    due_voice: Option<crate::audio::SfxId>,
    /// `PrevScore` + `NumFramesLookScore` — the carnage-voice window.
    pub(crate) prev_score: u32,
    carnage_counter: u32,
    pub(crate) audio: Option<Box<dyn AudioSink>>,
    /// Chrome-bar toggles.
    pub music_on: bool,
    pub sfx_on: bool,
    /// The start screen (menu.rs).
    pub(crate) menu: Menu,
    /// Active demo playback: (embedded index, next beat).
    pub(crate) demo_run: Option<(usize, usize)>,
    /// `LastDemo` — don't repeat the same recording back to back.
    pub(crate) last_demo: usize,
    /// MSVC `rand()` (never srand'd — seed 1): burns the randomized
    /// LocalRand/NetRand draw counts at `START_ONEPLAYER`.
    pub(crate) crt_seed: u32,
    /// The modern `Astro.cfg` (file on native, localStorage on wasm).
    settings_store: Option<Box<dyn SettingsStore>>,
    /// Mobile virtual-gamepad buttons currently held (OR'd into the
    /// keyboard state while Playing) — see touch_input.rs.
    pub touch: TouchHeld,
    /// Tilt steering: the rotation frame (0..32) the tilt vector
    /// points at; the ship turns toward it at the normal key-rotate
    /// speed. `None` inside the dead zone.
    pub(crate) tilt_target: Option<f32>,
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
            state: Screen::Menu,
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
            music_rate: 1.0,
            voice: VoicePlayer::new(),
            due_voice: None,
            prev_score: 0,
            carnage_counter: 0,
            menu: Menu::new(),
            demo_run: None,
            last_demo: usize::MAX,
            crt_seed: 1,
            settings_store: None,
            touch: TouchHeld::default(),
            tilt_target: None,
        }
    }

    /// Attach the platform's settings store and apply what it holds
    /// (`LoadConfig` at startup; absent/corrupt data keeps defaults).
    pub fn set_settings_store(&mut self, store: Box<dyn SettingsStore>) {
        if let Some(s) = store.load().as_deref().and_then(Settings::from_json) {
            self.menu.bindings = s.bindings();
            self.menu.start_level = s.start_level.min(crate::menu::MAX_START_LEVEL);
            self.menu.master_volume = s.master_volume.clamp(0.0, 1.0);
            self.menu.music_volume = s.music_volume.clamp(0.0, 1.0);
            self.music_on = s.music_on;
            self.sfx_on = s.sfx_on;
        }
        self.settings_store = Some(store);
    }

    /// `SaveConfig` — write the current state through the store.
    fn save_settings(&mut self) {
        let Some(store) = self.settings_store.as_deref() else {
            return;
        };
        let mut s = Settings::default();
        s.set_bindings(&self.menu.bindings);
        s.start_level = self.menu.start_level;
        s.master_volume = self.menu.master_volume;
        s.music_volume = self.menu.music_volume;
        s.music_on = self.music_on;
        s.sfx_on = self.sfx_on;
        store.save(&s.to_json());
    }

    /// Chrome-bar toggles — persisted like every other setting.
    pub fn toggle_music(&mut self) {
        self.music_on = !self.music_on;
        self.save_settings();
    }

    pub fn toggle_sfx(&mut self) {
        self.sfx_on = !self.sfx_on;
        self.save_settings();
    }

    /// Milliseconds since construction — the widget's paint clock.
    pub fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    pub(crate) fn clip() -> Rect {
        Rect::new(0, 0, WORLD_W, WORLD_H)
    }

    /// `OnScreenRect` — the play field above the stat bar.
    pub(crate) fn on_screen(&self) -> Rect {
        Rect::new(0, 0, SCREEN_W, SCREEN_H - self.statbar.height())
    }

    /// The live enemy count (`NumBadGuys`) — rocks deliberately not
    /// included; leftover rocks only cost the annihilation bonus.
    pub(crate) fn enemies_alive(&self) -> u32 {
        self.gloops.num_gloops
            + self.hks.num_hks
            + self.bombers.num_bombers
            + self.spikeballs.num_spikeballs
            + self.fastdeaths.num_fast_deaths
    }

    /// `NewLevel` — reset the world, then wait for Enter to spawn
    /// (`NeedToAddLocalPlayer`).
    pub(crate) fn new_level(&mut self) {
        self.reset_level();
        self.need_add_player = true;
        // `NeedNumBadGuys`: the tally's "Bad Guys Killed" is the count
        // present at level start.
        self.stats.bad_guys_killed = self.enemies_alive() as i32;
    }

    /// `ResetAll` in the original's exact call order — the RNG draw
    /// order is part of the determinism contract: players first ("so
    /// no one finds them as targets"), then Rocks, Gloops, SpikeBalls,
    /// HKs, Bombers, FastDeaths, Goodies, Explosions, and the
    /// speaker's random position.
    pub(crate) fn reset_level(&mut self) {
        // `PlayersResetAll`: every one of the 8 player slots gets
        // `SetVisAndMove` — two NetRand draws each, occupied or not —
        // then its shots/bombs clear and it goes invisible. HP
        // survives the level change; the pre-rolled position is where
        // Enter will materialize the ship (the camera parks there, so
        // the player times the spawn).
        for slot in 0..8 {
            let x = self.net_rand.rand(WORLD_W as u32) as f32;
            let y = self.net_rand.rand(WORLD_H as u32) as f32;
            if slot == 0 {
                let hp = self.ship.sprite.hp;
                self.ship.sprite.reset();
                self.ship.sprite.x_pos = x;
                self.ship.sprite.y_pos = y;
                self.ship.sprite.hp = hp;
                let cur = self.ship.cur_shots;
                self.ship.shots[cur].reset();
                self.ship.bombs.reset();
                self.ship.sprite.visible = false;
            }
        }
        // `NumFramesLookScore = 0` at the end of PlayersResetAll.
        self.carnage_counter = 0;

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

    /// `AddPlayer` — materialize the ship WHERE IT ALREADY IS: the
    /// death spot, or the level's pre-rolled spawn point. No new
    /// position roll — the camera has been parked on the spot the
    /// whole time, so the player can wait for a safe moment.
    /// `NewShip` (stat reset) and the one-liner fire only when
    /// actually dead: surviving a level carries HP and every power-up
    /// across, exactly like `AddPlayer`'s `LocalPlayerIsDead` gate.
    pub(crate) fn respawn(&mut self) {
        if self.local_player_dead {
            self.events.push(GameEvent::VoiceNewShip);
            self.ship.new_ship();
            self.local_player_dead = false;
        }
        self.ship.sprite.cur_frame = 0.0;
        self.ship.sprite.x_delta = 0.0;
        self.ship.sprite.y_delta = 0.0;
        self.ship.sprite.visible = true;
    }

    /// Run the 30 Hz simulation up to `now_ms`.
    pub fn advance(&mut self, now_ms: u64) {
        let clip = Self::clip();
        let beats = self.heartbeat.read_and_clear(now_ms);
        for _ in 0..beats {
            // Top of the original per-update loop: `PausePlayerUpdate`
            // and the `ResetMusicFrequencyDelay` clock tick per BEAT.
            if let Some(sfx) = self.voice.take_due() {
                self.due_voice = Some(sfx);
            }
            if self.music_freq_delay > 0 {
                self.music_freq_delay -= 1;
                if self.music_freq_delay == 0 {
                    self.music_slow = false;
                }
            }

            match self.state {
                Screen::Menu => {
                    // `StartScreenUpdate(1)`: the showcase monitor
                    // animates per beat (LocalRand only — visual).
                    self.menu.beat(&mut self.local_rand, &mut self.events);
                    // `STATE_MAIN`: Enter starts a game (line 758ff);
                    // elsewhere it acts like Done. Clicks arrive
                    // through on_mouse_up.
                    if self.enter_pressed {
                        self.enter_pressed = false;
                        if self.menu.enter_starts() {
                            let level = self.menu.start_level;
                            self.start_game(level);
                            continue;
                        }
                        if let Some(action) = self.menu.on_enter() {
                            self.handle_menu_action(action);
                        }
                    }
                }
                Screen::Demo => {
                    let done = if let Some((which, beat)) = self.demo_run {
                        let demos = crate::demo::embedded_demos();
                        let demo =
                            crate::demo::Demo::parse(demos[which]).expect("embedded demo parses");
                        // `DemoUpdate >= NumDemoUpdates + (!visible * 30)`.
                        let grace = if self.ship.sprite.visible { 0 } else { 30 };
                        if beat < demo.key_flags.len() + grace {
                            let flags = demo.key_flags.get(beat).copied().unwrap_or(0);
                            self.demo_beat(flags);
                            self.demo_run = Some((which, beat + 1));
                            false
                        } else {
                            true
                        }
                    } else {
                        true
                    };
                    if done {
                        self.end_demo();
                    }
                }
                Screen::Playing => {
                    // The C++ switch arm: transitions first, then
                    // `AdvanceFrames` — even on the beat the state
                    // flips to intermission.
                    self.playing_transitions();
                    self.sim_beat(clip);
                }
                Screen::Intermission => self.intermission_beat(clip),
                Screen::GameOver => {
                    // Exit check, `GameOverPause++`, `AdvanceFrames` —
                    // in that order; Enter (or the 20s timeout)
                    // returns to the start screen.
                    if self.enter_pressed || self.game_over_pause >= GAME_OVER_PAUSE {
                        self.game_over_to_menu();
                    } else {
                        self.game_over_pause += 1;
                        self.sim_beat(clip);
                    }
                }
            }
            // An Enter press is a one-beat edge: whatever didn't
            // consume it this beat doesn't get it later (`FlushKeys`).
            self.enter_pressed = false;
        }
        // `SaveConfig`: persist once a change settles (never mid-drag).
        if self.menu.settings_dirty && self.menu.dragging.is_none() {
            self.menu.settings_dirty = false;
            self.save_settings();
        }
        self.pump_audio();
    }

    /// One beat of `UpdateAll` — runs while Playing, during the
    /// intermission iris, and under the GAME OVER overlay.
    pub(crate) fn sim_beat(&mut self, clip: Rect) {
        // Touch buttons and tilt steering merge into the key state —
        // but only for live play; demo playback feeds `keys` directly
        // and must never see local inputs.
        let (touch, tilt) = if self.state == Screen::Demo {
            (TouchHeld::default(), (false, false))
        } else {
            (self.touch, self.tilt_rotate())
        };
        self.ship.set_inputs(ShipInputs {
            left: self.keys.left || tilt.0,
            right: self.keys.right || tilt.1,
            thrust: self.keys.thrust || touch.thrust,
            shield: self.keys.shield || touch.shield,
            fire: self.keys.fire || touch.fire,
            bomb: self.keys.bomb,
        });

        // `AdvanceFrames` order: Explosions FIRST, then Rocks, then
        // spawn effects, Gloops, SpikeBalls, HKs, Bombers, FastDeaths.
        self.explosions.update(&clip, &mut self.net_rand);
        self.rocks.update(&clip, &mut self.net_rand);
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

        // `PlayersCollideObject` samples HP before each object pass
        // for the hurt voice; one bracket around the pass series has
        // the same audible outcome (the pending slot replaces).
        let hp_before = self.ship.sprite.hp;

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

        // The hurt voice: HP dropped this pass and it didn't kill you
        // (`if (ps->HP < temphp && !LocalPlayerIsDead)`).
        if self.ship.sprite.hp < hp_before && !self.local_player_dead {
            self.events.push(GameEvent::VoiceHurt);
        }
        // The carnage window: `NumFramesLookScore` ticks once per
        // PlayersCollideObject call — 7 object passes per beat; a
        // score jump of 200+ inside the window earns a one-liner.
        self.carnage_counter += 7;
        if self.carnage_counter > 20 {
            self.prev_score = self.ship.score;
            self.carnage_counter = 0;
        } else if self.ship.score >= self.prev_score + 200 {
            self.prev_score = self.ship.score;
            self.carnage_counter = 0;
            self.events.push(GameEvent::VoiceCarnage);
        }

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

        // The `LocalPlayerDead()` block sits INSIDE AdvanceFrames —
        // after the speaker pass, before Players.UpdateFunc — and
        // only acts while STATE_PLAYING.
        if self.local_player_dead && self.state == Screen::Playing {
            if self.ship.num_ships == 0 {
                self.world.set_on_screen_rect(self.on_screen());
                self.game_over_pause = 0;
                self.state = Screen::GameOver;
            } else if !self.need_add_player {
                self.need_add_player = true;
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
        // Turn the frame's events into sound; drain regardless so the
        // queue can't grow unbounded when running silent or muted.
        if let Some(sink) = self.audio.as_deref_mut() {
            if self.sfx_on {
                audio::dispatch(
                    &mut self.events,
                    sink,
                    &mut self.local_rand,
                    &mut self.voice,
                );
                // A line that came due this frame's beats plays now.
                if let Some(sfx) = self.due_voice.take() {
                    sink.play_voice(sfx);
                }
            } else {
                for _ in self.events.drain() {}
                self.due_voice = None;
            }
            // The Config Sound sliders scale the built-in headroom.
            sink.set_volumes(self.menu.master_volume, self.menu.music_volume);
            let playing = matches!(self.state, Screen::Playing | Screen::Demo);
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
            // The record grab is instant; the recovery spins back up.
            let target = if self.music_slow {
                audio::MUSIC_SLOW_RATE
            } else {
                1.0
            };
            if target < self.music_rate {
                self.music_rate = target;
            } else if self.music_rate < target {
                self.music_rate = (self.music_rate + audio::MUSIC_RAMP_STEP).min(target);
            }
            sink.set_music_rate(self.music_rate);
        } else {
            for _ in self.events.drain() {}
        }
    }

    /// The composed indexed back buffer (the widget's upload source,
    /// tests, and the `dump_frame` inspection example).
    pub fn screen(&self) -> &Frame {
        &self.screen
    }

    /// The palette to present through — the start screen shows with
    /// its own art's palette (`LoadPalette(rStartBmp)`).
    pub fn current_palette(&self) -> &Palette {
        if self.state == Screen::Menu {
            &self.menu.palette
        } else {
            &self.palette
        }
    }

    /// Route a key through the bindings table (input.rs).
    pub fn set_key(&mut self, key: &Key, down: bool) -> bool {
        // `MyKbhit()` — ANY key interrupts demo playback.
        if self.state == Screen::Demo {
            if down {
                self.end_demo();
            }
            return true;
        }
        // `STATE_GETAKEY`: the config screen's capture eats the next
        // press before the bindings see it.
        if self.state == Screen::Menu && down && self.menu.capture_key(key, &mut self.events) {
            return true;
        }
        match self.menu.bindings.lookup(key) {
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
            Some(Binding::Menu) => {
                if down {
                    match self.state {
                        // The requested flow: Esc pauses into options.
                        Screen::Playing => {
                            self.menu.show_options_from_game();
                            self.state = Screen::Menu;
                        }
                        Screen::Menu => {
                            if let Some(action) = self.menu.on_escape() {
                                self.handle_menu_action(action);
                            }
                        }
                        // Esc skips the tally / game over, like the
                        // original's `KeyArray[SC_ESCAPE]` checks.
                        Screen::Intermission | Screen::GameOver => {
                            self.enter_pressed = true;
                        }
                        Screen::Demo => unreachable!("handled above"),
                    }
                }
            }
            None => return false,
        }
        true
    }
}
