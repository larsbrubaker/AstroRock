//! # Frame composition — the port of `DrawFrame`/`DrawPlayField`
//!
//! The draw half of `game.rs` (which owns the state and the 30 Hz
//! update half): each frame erases the play field, draws the world
//! through the wrapping camera, then adds the per-state layer — stat
//! bar and radar, press-enter/game-over overlays through `RedBlit`,
//! the intermission iris box, or the sliding bonus tally.

use crate::frame::{BlitMode, Frame};
use crate::game::{Game, Screen};
use crate::radar;
use crate::rect::Rect;

impl Game {
    /// Draw the whole radar pass (`RadarDrawOn`s + `RadarDraw`).
    fn draw_radar(&mut self) {
        radar::plot_world(
            &mut self.radar,
            &self.world,
            &self.rocks,
            &self.gloops,
            &self.hks,
            &self.bombers,
            &self.spikeballs,
            &self.fastdeaths,
            &self.speaker.sprite,
            &self.ship.sprite,
        );
        self.radar.draw(&mut self.screen, 255, 395);
    }

    /// `pScreen->Blit(art, centered on OnScreenRect, &RedBlit)`.
    fn overlay_center(screen: &mut Frame, art: &Frame, on_screen: &Rect, transred: &[u8; 256]) {
        let bounds = art.bounds();
        screen.blit(
            art,
            &bounds,
            (on_screen.width() - art.width) / 2,
            (on_screen.height() - art.height) / 2,
            BlitMode::RemapDestOn1(transred),
        );
    }

    /// Compose one frame into the indexed back buffer (`DrawFrame`).
    pub fn compose(&mut self) {
        let iris = self.state == Screen::Intermission && self.inter.close_level > 0;
        let tally = self.state == Screen::Intermission && !iris;
        let on_screen = self.on_screen();

        // `DrawPlayField`: camera follows the local player.
        if self.state != Screen::Attract && !tally {
            self.world
                .move_point_to_center(self.ship.sprite.x_pos as i32, self.ship.sprite.y_pos as i32);
        }

        // `pScreen->Erase(&OnScreenRect)` — only the play field; the
        // stat bar recomposes over the bottom every frame.
        self.screen.erase(&on_screen);

        // The tally screen erases the field and draws only the window
        // ("a cheep hack so that I don't get a frame of the
        // intermission screen when there is no bonus to count").
        if !tally {
            for &(x, y) in &self.stars {
                self.world.pset(&mut self.screen, x, y, 15);
            }
            self.explosions.draw(&self.world, &mut self.screen);
            // `pSpeakerSprite->Draw` — between explosions and goodies.
            self.speaker.sprite.draw(&self.world, &mut self.screen);
            self.goodies.draw(&self.world, &mut self.screen);
            self.rocks.draw(&self.world, &mut self.screen);
            self.gloops
                .draw(&self.world, &mut self.screen, &mut self.local_rand);
            self.hks
                .draw(&self.world, &mut self.screen, &mut self.local_rand);
            self.bombers
                .draw(&self.world, &mut self.screen, &mut self.local_rand);
            self.spikeballs
                .draw(&self.world, &mut self.screen, &mut self.local_rand);
            self.fastdeaths.draw(&self.world, &mut self.screen);
            self.spawnfx.draw(
                &self.world,
                &mut self.screen,
                &mut self.radar,
                &mut self.local_rand,
                &self.fades,
            );
        }

        match self.state {
            Screen::Attract => {
                // The original attract is demo playback — the full game
                // view with the stat bar. Ours shows it with the idle
                // (zeroed) local player until Phase 9 demos land.
                self.statbar.draw(&mut self.screen, &self.ship, 0);
                Self::overlay_center(
                    &mut self.screen,
                    &self.press_enter,
                    &on_screen,
                    &self.transred,
                );
            }
            Screen::Playing => {
                self.ship
                    .draw(&self.world, &mut self.screen, &mut self.thrust);
                // `DrawStats` runs last in DrawPlayField; the radar
                // draws after it, onto the bar (`RadarDraw(255,395)`).
                self.statbar
                    .draw(&mut self.screen, &self.ship, self.level as u32);
                self.draw_radar();
                if self.need_add_player {
                    Self::overlay_center(
                        &mut self.screen,
                        &self.press_enter,
                        &on_screen,
                        &self.transred,
                    );
                }
            }
            Screen::Intermission => {
                if iris {
                    self.ship
                        .draw(&self.world, &mut self.screen, &mut self.thrust);
                    // `pScreen->Box(&InterBox, 184)`.
                    let rect = self.inter.shrink_rect(&on_screen);
                    self.screen.box_outline(&rect, 184);
                } else if self.inter.total_bonus > 0 || self.inter.raising > 0 {
                    self.inter.draw(&mut self.screen);
                }
                self.statbar
                    .draw(&mut self.screen, &self.ship, self.level as u32);
                self.draw_radar();
            }
            Screen::GameOver => {
                self.statbar
                    .draw(&mut self.screen, &self.ship, self.level as u32);
                self.draw_radar();
                Self::overlay_center(&mut self.screen, &self.endgame, &on_screen, &self.transred);
            }
        }
    }
}
