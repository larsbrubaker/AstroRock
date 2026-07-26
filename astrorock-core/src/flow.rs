//! # State transitions — the main-loop switch arms
//!
//! The transition half of `game.rs` (which owns the state and the
//! per-beat sim): the `STATE_PLAYING` gate/level-end/death logic and
//! the `STATE_INTERMISSION` iris + tally beat from `AstroRock.cpp`'s
//! `switch (GameState)`.

use crate::events::GameEvent;
use crate::game::{Game, Screen, NUM_START_SHIPS};
use crate::menu::MenuAction;
use crate::pship::PlayerShip;
use crate::rect::Rect;

impl Game {
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

    /// `STATE_GAMEOVER` exit: back to the start screen — through the
    /// name-entry page when the run made the high-score table.
    pub(crate) fn game_over_to_menu(&mut self) {
        let score = self.ship.score;
        self.level = 0;
        self.reset_level();
        if score > 0 && self.menu.high_score_rank(score).is_some() {
            self.menu.start_high_score_entry(score);
        } else {
            self.menu.show_main();
        }
        self.state = Screen::Menu;
    }

    /// MSVC CRT `rand()` — `seed*214013+2531011`, top 15 bits. The
    /// original never called `srand`, so the sequence from seed 1 is
    /// part of the behavior.
    fn crt_rand(&mut self) -> u32 {
        self.crt_seed = self.crt_seed.wrapping_mul(214013).wrapping_add(2531011);
        (self.crt_seed >> 16) & 0x7FFF
    }

    /// `START_ONEPLAYER` + `NewGame`: burn a CRT-randomized count of
    /// LocalRand and NetRand draws (the original's game-to-game
    /// variety), then start at `level` (`GlobalStartLevel`).
    pub(crate) fn start_game(&mut self, level: u32) {
        let n = self.crt_rand() % 256;
        for _ in 0..n {
            self.local_rand.rand(256);
        }
        let n = self.crt_rand() % 256;
        for _ in 0..n {
            self.net_rand.rand(256);
        }
        self.level = level as usize;
        self.ship = PlayerShip::new();
        self.ship.reset(NUM_START_SHIPS);
        self.stats.reset(level);
        self.new_level();
        self.local_player_dead = false;
        self.game_over_pause = 0;
        self.state = Screen::Playing;
    }

    /// `StartDemoButton` + `LoadADemo`: a LocalRand pick that never
    /// repeats the previous recording.
    fn play_demo(&mut self) {
        let demos = crate::demo::embedded_demos();
        let mut pick = self.local_rand.rand(demos.len() as u32) as usize;
        while pick == self.last_demo {
            pick = self.local_rand.rand(demos.len() as u32) as usize;
        }
        self.last_demo = pick;
        let demo = crate::demo::Demo::parse(demos[pick]).expect("embedded demo parses");
        self.init_demo(demo.start_level);
        self.demo_run = Some((pick, 0));
        self.state = Screen::Demo;
    }

    /// Demo over or interrupted: back to the start screen
    /// (`CurLevel = 0; ResetAll(); StartScreenShow`).
    pub(crate) fn end_demo(&mut self) {
        self.demo_run = None;
        self.level = 0;
        self.reset_level();
        self.menu.show_main();
        self.state = Screen::Menu;
    }

    pub(crate) fn handle_menu_action(&mut self, action: MenuAction) {
        match action {
            MenuAction::StartGame { level } => self.start_game(level),
            MenuAction::PlayDemo => self.play_demo(),
            MenuAction::ResumeGame => self.state = Screen::Playing,
            // `STATE_REALLYENDGAME` Yes: the GAME OVER overlay plays
            // over the abandoned world, then back to the menu.
            MenuAction::EndGame => {
                self.world.set_on_screen_rect(self.on_screen());
                self.game_over_pause = 0;
                self.state = Screen::GameOver;
            }
            MenuAction::Quit => {
                #[cfg(not(target_arch = "wasm32"))]
                std::process::exit(0);
                #[cfg(target_arch = "wasm32")]
                self.menu.show_main();
            }
        }
    }

    /// Mouse input in 640x480 game-surface coordinates.
    pub fn on_mouse_move(&mut self, x: i32, y: i32) {
        if self.state == Screen::Menu {
            self.menu.on_mouse_move(x, y);
        }
    }

    pub fn on_mouse_down(&mut self, x: i32, y: i32) {
        match self.state {
            Screen::Menu => self.menu.on_mouse_down(x, y),
            // `MouseHasChanged()` interrupts a demo.
            Screen::Demo => self.end_demo(),
            _ => {}
        }
    }

    pub fn on_mouse_up(&mut self, x: i32, y: i32) {
        if self.state == Screen::Menu {
            if let Some(action) = self.menu.on_mouse_up(x, y, &mut self.events) {
                self.handle_menu_action(action);
            }
        }
    }
}

impl Game {
    /// The `STATE_PLAYING` switch arm, in the C++ order: level end
    /// first, (Esc quit-confirm arrives with Phase 8), then the spawn
    /// gate. The death check lives in `sim_beat` like the original's
    /// `LocalPlayerDead()` block inside `AdvanceFrames`.
    pub(crate) fn playing_transitions(&mut self) {
        // `NumBadGuys == 0` -> `SetStateIntermission`. Rocks don't
        // count — leftovers only zero the annihilation bonus.
        if self.enemies_alive() == 0 {
            let rocks_left = (self.rocks.num_big + self.rocks.num_med + self.rocks.num_lit) as i32;
            self.inter.begin(&mut self.stats, rocks_left);
            self.state = Screen::Intermission;
        }

        // `NeedToAddLocalPlayer` + `PressedContinue` -> `AddPlayer`,
        // gated by the post-death lockout (modern): a press during
        // the delay is ignored, not queued — the player must choose
        // the moment once they can see the field again.
        if self.respawn_delay > 0 {
            self.respawn_delay -= 1;
        } else if self.need_add_player && self.enter_pressed {
            self.respawn();
            self.need_add_player = false;
        }
    }

    /// The `STATE_INTERMISSION` arm: the iris (sim still running),
    /// then the sliding tally counting the bonus into the score.
    pub(crate) fn intermission_beat(&mut self, clip: Rect) {
        let on_screen = self.on_screen();
        if self.inter.close_level > 0 {
            // C++ order: countdown, iris/NewLevel, THEN AdvanceFrames
            // (the fresh level sims one beat before the tally shows).
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
            self.sim_beat(clip);
        } else if self.enter_pressed || self.inter.raising == 0 || self.inter.total_bonus == 0 {
            // Skip or done: bank the remainder and play on
            // (`ResetIntermisionInfo` on the way out).
            self.ship.add_score(self.inter.total_bonus.max(0) as u32);
            self.inter.total_bonus = 0;
            self.stats.reset(self.level as u32);
            self.stats.bad_guys_killed = self.enemies_alive() as i32;
            // `DontSayCarnage` — the banked bonus isn't a killstreak.
            self.prev_score = self.ship.score;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{NUM_START_SHIPS, SCREEN_H, SCREEN_W};
    use agg_gui::event::Key;

    /// Step the simulation n beats (33.4ms each).
    fn step(g: &mut Game, from_ms: u64, beats: u64) -> u64 {
        let target = from_ms + beats * 1000 / 30 + 2;
        g.advance(target);
        target
    }

    #[test]
    fn boot_composes_the_start_screen() {
        let mut g = Game::new(None);
        g.compose();
        let screen = g.screen();
        // The start.png backdrop fills the frame with plenty of art,
        // and the menu presents through its own palette.
        let non_zero = screen.bits.iter().filter(|&&b| b != 0).count();
        assert!(
            non_zero > 50_000,
            "start screen not composed: {non_zero} lit pixels"
        );
        assert_ne!(
            g.current_palette().rgb.to_vec(),
            crate::assets::game_palette().rgb.to_vec(),
            "menu should present through start.png's palette"
        );
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

        // The post-death lockout (modern): a player mashing fire dies
        // holding the button — presses during the delay are ignored,
        // not queued, so they can't bounce straight back onto the
        // rock that killed them.
        g.ship.sprite.x_pos = 10.0;
        g.ship.sprite.y_pos = 10.0;
        g.set_key(&Key::Enter, true);
        now = step(&mut g, now, 1);
        g.set_key(&Key::Enter, false);
        assert!(
            !g.ship.sprite.visible,
            "respawn must wait out the death lockout"
        );
        for _ in 0..crate::game::RESPAWN_DELAY {
            now = step(&mut g, now, 1);
        }
        assert!(!g.ship.sprite.visible, "ignored presses don't queue");

        // The dead ship keeps its position (`AddPlayer` never moves
        // it) — the killer rock is still parked there, so respawning
        // blind would die again. Drag the wreck somewhere quiet first,
        // like a player waiting for a safe moment.
        g.ship.sprite.x_pos = 10.0;
        g.ship.sprite.y_pos = 10.0;
        g.set_key(&Key::Enter, true);
        step(&mut g, now, 1);
        assert!(g.ship.sprite.visible);
        assert_eq!(g.ship.sprite.hp, 100);
        // And it spawned exactly where it was left.
        assert!(g.ship.sprite.x_pos < 30.0 && g.ship.sprite.y_pos < 30.0);
    }

    #[test]
    fn esc_quit_confirm_abandons_the_game_through_game_over() {
        let mut g = Game::new(None);
        let now = start_and_spawn(&mut g);

        // Esc mid-game: frozen into the options page.
        g.set_key(&Key::Escape, true);
        assert!(g.state == Screen::Menu && g.menu.from_game);
        g.set_key(&Key::Escape, false);

        // Quit -> confirm -> Yes: the GAME OVER overlay plays out.
        g.handle_menu_action(crate::menu::MenuAction::EndGame);
        assert!(g.state == Screen::GameOver);

        // Enter leaves GAME OVER for the start screen, reset clean.
        g.set_key(&Key::Enter, true);
        step(&mut g, now, 1);
        assert!(g.state == Screen::Menu);
        assert!(!g.menu.from_game, "back on the main page");
        assert!(g.menu.enter_starts(), "Enter can start a fresh game");
    }

    /// A won level means an untouchable pilot (modern): parking the
    /// ship inside a rock through the whole iris must not kill it.
    #[test]
    fn ship_is_indestructible_during_the_intermission() {
        let mut g = Game::new(None);
        let mut now = start_and_spawn(&mut g);
        g.inter.begin(&mut g.stats, 0);
        g.state = Screen::Intermission;
        let ships_before = g.ship.num_ships;

        // Ride the iris out (stop before NewLevel hides the ship for
        // the spawn gate), parked inside a rock the whole way.
        while g.inter.close_level > 1 {
            if let Some(idx) = g.rocks.big().iter().position(|s| s.visible) {
                g.ship.sprite.x_pos = g.rocks.big()[idx].x_pos;
                g.ship.sprite.y_pos = g.rocks.big()[idx].y_pos;
            }
            now = step(&mut g, now, 1);
            assert!(g.ship.sprite.visible, "ship must survive the iris");
        }
        assert_eq!(g.ship.num_ships, ships_before);
        assert_eq!(g.stats.lives_lost, 0);
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
