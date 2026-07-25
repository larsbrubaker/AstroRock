//! # Game screen — attract mode + playable single-player loop
//!
//! The widget owning the 640x480 indexed pipeline. Two states:
//!
//! - **Attract**: star field, drifting level-1 rocks, and the original
//!   "Press Enter" art through `RedBlit`. (The shareware teaser/upsell
//!   screen is retired — Logicware's address and phone are long gone.)
//! - **Playing**: the ported `UpdateAll`/`DrawPlayField` core — ship
//!   physics and weapons, rocks with splitting, explosions, radar,
//!   camera following the ship, collisions in the original handler
//!   order (rock takes the collider's damage first, then the collider
//!   takes the rock class's damage).
//!
//! Keys (stand-ins for the original's remappable `Astro.cfg` bindings):
//! arrows rotate/thrust, Space fires, S shields, B bombs, Enter
//! starts/respawns.

use agg_gui::color::Color;
use agg_gui::draw_ctx::DrawCtx;
use agg_gui::event::{Event, EventResult, Key};
use agg_gui::geometry::{Rect as GuiRect, Size};
use agg_gui::widget::Widget;
use web_time::Instant;

use crate::assets;
use crate::bombers::{Bombers, BOMBER_RADAR_COLOR};
use crate::collide::{self, CollideCtx};
use crate::events::Events;
use crate::explosion::Explosions;
use crate::frame::{BlitMode, Frame};
use crate::gloops::Gloops;
use crate::heartbeat::HeartBeat;
use crate::hks::{Hks, HK_RADAR_COLOR};
use crate::palette::Palette;
use crate::pship::{PlayerShip, ShipInputs};
use crate::radar::Radar;
use crate::rand::Rand;
use crate::rect::Rect;
use crate::rocks::Rocks;
use crate::spikeballs::{SpikeBalls, SPIKEBALL_RADAR_COLOR};
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Attract,
    Playing,
}

#[derive(Default, Clone, Copy)]
struct KeysHeld {
    left: bool,
    right: bool,
    thrust: bool,
    shield: bool,
    fire: bool,
    bomb: bool,
}

pub struct TitleScreen {
    bounds: GuiRect,
    children: Vec<Box<dyn Widget>>,
    screen: Frame,
    world: VirtualFrame,
    palette: Palette,
    press_enter: Frame,
    transred: [u8; 256],
    stars: Vec<(i32, i32)>,
    rocks: Rocks,
    gloops: Gloops,
    hks: Hks,
    bombers: Bombers,
    spikeballs: SpikeBalls,
    explosions: Explosions,
    events: Events,
    net_rand: Rand,
    local_rand: Rand,
    heartbeat: HeartBeat,
    started: Instant,
    rgba: Vec<u8>,
    state: Screen,
    keys: KeysHeld,
    enter_pressed: bool,
    ship: PlayerShip,
    thrust: Thrust,
    radar: Radar,
    level: usize,
    local_player_dead: bool,
}

impl TitleScreen {
    pub fn new() -> Self {
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

        let mut world = VirtualFrame::new(WORLD_W, WORLD_H);
        world.set_on_screen_rect(Rect::new(0, 0, SCREEN_W, SCREEN_H));
        world.move_point_to_center(WORLD_W / 2, WORLD_H / 2);

        let mut rocks = Rocks::new();
        rocks.reset(0, &mut net_rand);

        Self {
            bounds: GuiRect::default(),
            children: Vec::new(),
            screen: Frame::new(SCREEN_W, SCREEN_H),
            world,
            palette: assets::game_palette(),
            press_enter: assets::frame_from_indexed_png(assets::PRESS_ENTER_PNG),
            transred: assets::remap_table(assets::TRANSRED_PAL),
            stars,
            rocks,
            gloops: Gloops::new(),
            hks: Hks::new(),
            bombers: Bombers::new(),
            spikeballs: SpikeBalls::new(),
            explosions: Explosions::new(),
            events: Events::new(),
            net_rand,
            local_rand: Rand::new(),
            heartbeat: HeartBeat::new(0),
            started: Instant::now(),
            rgba: Vec::new(),
            state: Screen::Attract,
            keys: KeysHeld::default(),
            enter_pressed: false,
            ship: PlayerShip::new(),
            thrust: Thrust::new(),
            radar: Radar::new(),
            level: 0,
            local_player_dead: false,
        }
    }

    fn clip() -> Rect {
        Rect::new(0, 0, WORLD_W, WORLD_H)
    }

    /// Start a fresh game (Enter from attract).
    fn start_game(&mut self) {
        self.net_rand.seed(0);
        self.level = 0;
        self.explosions.reset();
        self.reset_level();
        self.ship = PlayerShip::new();
        self.ship.reset(NUM_START_SHIPS);
        self.local_player_dead = false;
        self.state = Screen::Playing;
    }

    /// Per-level resets in the original's `ResetFunc` call order
    /// (RNG draw order is part of the determinism contract).
    fn reset_level(&mut self) {
        self.rocks.reset(self.level, &mut self.net_rand);
        self.gloops.reset(self.level, &mut self.net_rand);
        self.hks.reset(self.level, &mut self.net_rand);
        self.bombers.reset(self.level, &mut self.net_rand);
        self.spikeballs.reset(self.level, &mut self.net_rand);
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
                    self.gloops
                        .update(&clip, &mut self.net_rand, &self.world, &self.ship.sprite);
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
                    self.spikeballs.update(
                        &clip,
                        &mut self.net_rand,
                        &self.world,
                        &mut self.explosions,
                        &mut self.events,
                    );

                    // PlayersCollideObject order: Rocks first, then
                    // Gloops (then the rest as they're ported).
                    {
                        let mut ctx = CollideCtx {
                            world: &self.world,
                            explosions: &mut self.explosions,
                            events: &mut self.events,
                            net_rand: &mut self.net_rand,
                            clip,
                        };
                        if collide::player_vs_rocks(&mut self.ship, &mut self.rocks, &mut ctx) {
                            self.local_player_dead = true;
                        }
                        if collide::player_vs_gloops(&mut self.ship, &mut self.gloops, &mut ctx) {
                            self.local_player_dead = true;
                        }
                        if collide::player_vs_hks(&mut self.ship, &mut self.hks, &mut ctx) {
                            self.local_player_dead = true;
                        }
                        if collide::player_vs_bombers(&mut self.ship, &mut self.bombers, &mut ctx) {
                            self.local_player_dead = true;
                        }
                    }

                    self.ship.update(
                        &clip,
                        &mut self.net_rand,
                        &self.world,
                        &mut self.explosions,
                        &mut self.events,
                    );

                    // Level cleared — every rock AND every drawn enemy
                    // gone (the original ends the level when DrawFrame's
                    // active counts all hit zero).
                    if self.rocks.num_big + self.rocks.num_med + self.rocks.num_lit == 0
                        && self.gloops.num_gloops == 0
                        && self.hks.num_hks == 0
                        && self.bombers.num_bombers == 0
                        && self.spikeballs.num_spikeballs == 0
                    {
                        self.level += 1;
                        self.reset_level();
                    }

                    if self.local_player_dead {
                        if self.ship.num_ships == 0 {
                            // Game over — back to the teaser.
                            self.state = Screen::Attract;
                            self.rocks.reset(0, &mut self.net_rand);
                            self.explosions.reset();
                        } else if self.enter_pressed {
                            self.enter_pressed = false;
                            self.respawn();
                        }
                    }
                }
            }
        }
        // No audio sink yet — drain so the queue can't grow unbounded.
        for _ in self.events.drain() {}
    }

    /// Compose one frame into the indexed back buffer.
    pub fn compose(&mut self) {
        // `DrawPlayField`: camera follows the local player.
        if self.state == Screen::Playing {
            self.world
                .move_point_to_center(self.ship.sprite.x_pos as i32, self.ship.sprite.y_pos as i32);
        }

        self.screen.erase(&Rect::new(0, 0, SCREEN_W, SCREEN_H));

        for &(x, y) in &self.stars {
            self.world.pset(&mut self.screen, x, y, 15);
        }

        self.explosions.draw(&self.world, &mut self.screen);
        self.rocks.draw(&self.world, &mut self.screen);
        self.gloops
            .draw(&self.world, &mut self.screen, &mut self.local_rand);
        self.hks
            .draw(&self.world, &mut self.screen, &mut self.local_rand);
        self.bombers
            .draw(&self.world, &mut self.screen, &mut self.local_rand);
        self.spikeballs
            .draw(&self.world, &mut self.screen, &mut self.local_rand);

        match self.state {
            Screen::Attract => {
                // pScreen->Blit(&PressEnterFrame, centered, &RedBlit)
                let art_bounds = self.press_enter.bounds();
                self.screen.blit(
                    &self.press_enter,
                    &art_bounds,
                    SCREEN_W / 2 - self.press_enter.width / 2,
                    SCREEN_H / 2 - self.press_enter.height / 2,
                    BlitMode::RemapDestOn1(&self.transred),
                );
            }
            Screen::Playing => {
                self.ship
                    .draw(&self.world, &mut self.screen, &mut self.thrust);

                // Radar: rocks + player blips, bottom center for now
                // (the stat bar arrives with the UI phase).
                for i in 0..self.rocks.big().len() {
                    self.radar.plot(&self.rocks.big()[i], 15, &self.world);
                }
                for i in 0..self.rocks.med().len() {
                    self.radar.plot(&self.rocks.med()[i], 145, &self.world);
                }
                for i in 0..self.rocks.lit().len() {
                    self.radar.plot(&self.rocks.lit()[i], 147, &self.world);
                }
                if self.gloops.active() {
                    for i in 0..self.gloops.pool().len() {
                        self.radar.plot(&self.gloops.pool()[i], 104, &self.world);
                    }
                }
                if self.hks.active() {
                    for i in 0..self.hks.pool().len() {
                        self.radar
                            .plot(&self.hks.pool()[i], HK_RADAR_COLOR, &self.world);
                    }
                }
                if self.bombers.active() {
                    for i in 0..self.bombers.pool().len() {
                        self.radar
                            .plot(&self.bombers.pool()[i], BOMBER_RADAR_COLOR, &self.world);
                    }
                }
                if self.spikeballs.active() {
                    for i in 0..self.spikeballs.pool().len() {
                        self.radar.plot(
                            &self.spikeballs.pool()[i],
                            SPIKEBALL_RADAR_COLOR,
                            &self.world,
                        );
                    }
                }
                self.radar.plot(&self.ship.sprite, 160, &self.world);
                self.radar
                    .draw(&mut self.screen, SCREEN_W / 2 - 64, SCREEN_H - 66);
            }
        }
    }

    /// The composed indexed back buffer (tests + the `dump_frame`
    /// inspection example).
    pub fn screen(&self) -> &Frame {
        &self.screen
    }

    fn set_key(&mut self, key: &Key, down: bool) -> bool {
        match key {
            Key::ArrowLeft => self.keys.left = down,
            Key::ArrowRight => self.keys.right = down,
            Key::ArrowUp => self.keys.thrust = down,
            Key::Char(' ') => self.keys.fire = down,
            Key::Char('s') | Key::Char('S') => self.keys.shield = down,
            Key::Char('b') | Key::Char('B') => self.keys.bomb = down,
            Key::Enter => {
                if down {
                    self.enter_pressed = true;
                }
            }
            _ => return false,
        }
        true
    }
}

impl Default for TitleScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for TitleScreen {
    fn type_name(&self) -> &'static str {
        "TitleScreen"
    }

    fn bounds(&self) -> GuiRect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: GuiRect) {
        self.bounds = bounds;
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Box<dyn Widget>> {
        &mut self.children
    }

    fn is_focusable(&self) -> bool {
        // Held-key state needs KeyUp delivery, which only reaches the
        // focused widget. The app calls `focus_first()` at startup.
        true
    }

    fn layout(&mut self, available: Size) -> Size {
        available
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        let now_ms = self.started.elapsed().as_millis() as u64;
        self.advance(now_ms);
        self.compose();
        // Keep the loop animating — content changes every frame, so the
        // full request_draw (animation.rs: "animation ticks … must call
        // request_draw").
        agg_gui::animation::request_draw();
        self.palette.frame_to_rgba(&self.screen, &mut self.rgba);

        // Letterbox: aspect-fit the 640x480 game surface in the window.
        let (w, h) = (self.bounds.width, self.bounds.height);
        ctx.set_fill_color(Color::from_rgb8(0, 0, 0));
        ctx.begin_path();
        ctx.rect(0.0, 0.0, w, h);
        ctx.fill();

        let scale = (w / SCREEN_W as f64).min(h / SCREEN_H as f64);
        let dw = SCREEN_W as f64 * scale;
        let dh = SCREEN_H as f64 * scale;
        let dx = (w - dw) * 0.5;
        let dy = (h - dh) * 0.5;
        ctx.draw_image_rgba(&self.rgba, SCREEN_W as u32, SCREEN_H as u32, dx, dy, dw, dh);
    }

    fn on_event(&mut self, event: &Event) -> EventResult {
        match event {
            Event::KeyDown { key, .. } => {
                if self.set_key(key, true) {
                    EventResult::Consumed
                } else {
                    EventResult::Ignored
                }
            }
            Event::KeyUp { key, .. } => {
                if self.set_key(key, false) {
                    EventResult::Consumed
                } else {
                    EventResult::Ignored
                }
            }
            _ => EventResult::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Step the simulation n beats (33.4ms each).
    fn step(t: &mut TitleScreen, from_ms: u64, beats: u64) -> u64 {
        let target = from_ms + beats * 1000 / 30 + 2;
        t.advance(target);
        target
    }

    #[test]
    fn attract_composes_stars_rocks_and_teaser() {
        let mut t = TitleScreen::new();
        t.compose();
        let screen = t.screen();
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

    #[test]
    fn enter_starts_a_game_with_three_ships() {
        let mut t = TitleScreen::new();
        t.set_key(&Key::Enter, true);
        // One beat: start_game consumes it before any collision runs
        // (no spawn-protection effect yet — spawnfx is unported, so a
        // rock over the spawn point could kill instantly).
        let now = step(&mut t, 0, 1);
        assert!(t.state == Screen::Playing);
        assert_eq!(t.ship.num_ships, NUM_START_SHIPS);
        assert!(t.ship.sprite.visible);

        // The ship composes onto the screen at the camera center.
        t.compose();
        let _ = now;
        let mut ship_pixels = 0;
        for y in (SCREEN_H / 2 - 30)..(SCREEN_H / 2 + 30) {
            for x in (SCREEN_W / 2 - 30)..(SCREEN_W / 2 + 30) {
                if t.screen().get(x, y) != 0 {
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
        let mut t = TitleScreen::new();
        t.set_key(&Key::Enter, true);
        let mut now = step(&mut t, 0, 1); // start beat only — see above

        // Park the ship on top of the first visible big rock and fire
        // point-blank until it shatters into mediums.
        let idx = t.rocks.big().iter().position(|s| s.visible).unwrap();
        let (rx, ry) = (t.rocks.big()[idx].x_pos, t.rocks.big()[idx].y_pos);
        for _ in 0..60 {
            t.ship.sprite.x_pos = rx;
            t.ship.sprite.y_pos = ry;
            t.ship.sprite.x_delta = 0.0;
            t.ship.sprite.y_delta = 0.0;
            t.ship.sprite.hp = 9999; // survive the ram for this test
            t.set_key(&Key::Char(' '), true);
            now = step(&mut t, now, 1);
            t.set_key(&Key::Char(' '), false);
            now = step(&mut t, now, 1);
            if t.rocks.num_med > 0 {
                break;
            }
        }
        assert!(t.rocks.num_med > 0, "big rock never split");
        assert!(t.ship.score > 0, "no score awarded");
    }

    #[test]
    fn ramming_rocks_kills_the_ship_eventually() {
        let mut t = TitleScreen::new();
        t.set_key(&Key::Enter, true);
        let mut now = step(&mut t, 0, 1); // start beat only — see above
        let start_ships = t.ship.num_ships;
        assert_eq!(start_ships, NUM_START_SHIPS);

        for _ in 0..600 {
            if let Some(idx) = t.rocks.big().iter().position(|s| s.visible) {
                t.ship.sprite.x_pos = t.rocks.big()[idx].x_pos;
                t.ship.sprite.y_pos = t.rocks.big()[idx].y_pos;
            }
            now = step(&mut t, now, 1);
            if t.local_player_dead {
                break;
            }
        }
        assert!(t.local_player_dead, "ship survived 600 beats of ramming");
        assert_eq!(t.ship.num_ships, start_ships - 1);

        // Enter respawns with a fresh hull.
        t.set_key(&Key::Enter, true);
        step(&mut t, now, 2);
        assert!(t.ship.sprite.visible);
        assert_eq!(t.ship.sprite.hp, 100);
    }
}
