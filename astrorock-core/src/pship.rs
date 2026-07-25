//! # Player ship — port of `pship.cpp`
//!
//! `CPlayerShip`: the sprite plus three weapon tiers, bombs, shields,
//! thrust and the input-driven update. Physics constants are the
//! originals: ACCELERATION 1.2 along the facing (32 rotation frames),
//! FRICTION 0.98 per beat, rotation via FrameAdvance ±1.
//!
//! Sound loops (shield/thrust hum) are continuous states, not events —
//! they surface via `shield_on`/`thrusting` for the audio phase to
//! start/stop loops.

use crate::bombs::Bombs;
use crate::events::{Events, GameEvent};
use crate::explosion::Explosions;
use crate::fixed_trig;
use crate::frame::Frame;
use crate::rand::Rand;
use crate::rect::Rect;
use crate::sequence;
use crate::shots::{ShotTier, Shots};
use crate::sprite::Sprite;
use crate::thrust::Thrust;
use crate::virtual_frame::VirtualFrame;

const ACCELERATION: f32 = 1.2;
const MAX_SHIELDS: u32 = 168;
const START_HP: u32 = 100;
pub const MAX_HP: u32 = 168;
const MAX_SHIPS: u32 = 9;
const BOMB_DURATION: u32 = 90;
const START_SHIELD: u32 = 35;
const FRICTION: f32 = 0.98;

pub const NORMAL_SHOTS: usize = 0;
pub const POWER_SHOTS: usize = 1;
pub const SUPER_SHOTS: usize = 2;

/// Pressed-key snapshot for one update (`SetInputs`).
#[derive(Clone, Copy, Default)]
pub struct ShipInputs {
    pub left: bool,
    pub right: bool,
    pub thrust: bool,
    pub shield: bool,
    pub fire: bool,
    pub bomb: bool,
}

pub struct PlayerShip {
    pub sprite: Sprite,
    pub shots: [Shots; 3],
    pub cur_shots: usize,
    pub bombs: Bombs,
    pub score: u32,
    pub num_ships: u32,
    pub num_shields: u32,
    pub shield_on: bool,
    pub num_rapids: u32,
    pub num_spreads: u32,
    pub num_bombs: u32,
    switch_shots: bool,
    pub num_power_shots: u32,
    pub num_super_shots: u32,
    pub frag_count: u32,
    cur_thrust: usize,
    num_thrust: usize,
    pub thrusting: bool,
    firing: bool,
    bomb_away: bool,
    key_fire: bool,
    key_thrust: bool,
    key_shield: bool,
    key_bombs: bool,
    shield_sprite: Sprite,
}

impl PlayerShip {
    /// `CPlayerShip::CPlayerShip` — pools, weapon configs, defaults.
    pub fn new() -> Self {
        let mut shots = [
            Shots::new(sequence::shot01(), ShotTier::Normal, 15),
            Shots::new(sequence::shot02(), ShotTier::Power, 15),
            Shots::new(sequence::shot03(), ShotTier::Super, 15),
        ];
        shots[NORMAL_SHOTS].config(40);
        shots[POWER_SHOTS].config(192);
        shots[SUPER_SHOTS].config(512);

        let mut bombs = Bombs::new(sequence::bomb(), 5);
        // HP 0xFFFF (indestructible), damage 0xFFFF (touch it and die).
        bombs.config(0xFFFF, 0xFFFF, BOMB_DURATION);

        let mut sprite = Sprite::new();
        sprite.set_sequence(sequence::ship());
        sprite.frame_advance = 0.0;
        sprite.x_pos = 320.0;
        sprite.y_pos = 240.0;
        // `ShipBlit[0]` (players.cpp): the local player's hull draws
        // through the rPlrRedPal recolor; the stat bar's extra-ship
        // icons use the same table.
        sprite.blit = crate::sprite::SpriteBlit::RemapSource(std::rc::Rc::new(
            crate::assets::remap_table(crate::assets::PLRRED_PAL),
        ));

        let mut shield_sprite = Sprite::new();
        shield_sprite.set_sequence(sequence::shield());

        Self {
            sprite,
            shots,
            cur_shots: NORMAL_SHOTS,
            bombs,
            score: 0,
            num_ships: 0,
            num_shields: 0,
            shield_on: false,
            num_rapids: 0,
            num_spreads: 0,
            num_bombs: 0,
            switch_shots: false,
            num_power_shots: 0,
            num_super_shots: 0,
            frag_count: 0,
            cur_thrust: 0,
            num_thrust: crate::thrust::NUM_THRUSTS,
            thrusting: false,
            firing: false,
            bomb_away: false,
            key_fire: false,
            key_thrust: false,
            key_shield: false,
            key_bombs: false,
            shield_sprite,
        }
    }

    /// `CPlayerShipReset` — new game.
    pub fn reset(&mut self, ships: u32) {
        self.sprite.reset();
        self.key_fire = false;
        self.key_thrust = false;
        self.key_shield = false;
        self.key_bombs = false;
        self.new_ship();
        self.score = 0;
        self.frag_count = 0;
        self.num_ships = ships;
        self.sprite.frame_advance = 0.0;
    }

    /// `NewShip` — respawn state within a game.
    pub fn new_ship(&mut self) {
        self.sprite.hp = START_HP;
        self.num_shields = START_SHIELD;
        self.sprite.cur_frame = 0.0;
        self.sprite.x_delta = 0.0;
        self.sprite.y_delta = 0.0;
        self.num_rapids = 0;
        self.num_power_shots = 0;
        self.num_super_shots = 0;
        self.cur_shots = NORMAL_SHOTS;
        self.num_bombs = 0;
        self.num_spreads = 0;
    }

    /// `SetInputs` — rotation from left/right, latch the buttons.
    pub fn set_inputs(&mut self, inputs: ShipInputs) {
        self.sprite.frame_advance = if inputs.left && !inputs.right {
            -1.0
        } else if inputs.right && !inputs.left {
            1.0
        } else {
            0.0
        };
        self.key_fire = inputs.fire;
        self.key_thrust = inputs.thrust;
        self.key_shield = inputs.shield;
        self.key_bombs = inputs.bomb;
    }

    /// `CPlayerShip::Update` — one 30 Hz beat.
    pub fn update(
        &mut self,
        clip: &Rect,
        rand: &mut Rand,
        world: &VirtualFrame,
        explosions: &mut Explosions,
        events: &mut Events,
    ) {
        self.shots[self.cur_shots].update(clip, rand);
        self.bombs.update(clip, rand, world, explosions, events);

        if self.key_shield && self.num_shields != 0 {
            if self.sprite.visible {
                self.shield_on = true;
                self.num_shields -= 1;
            } else {
                self.shield_on = false;
            }
        } else {
            self.shield_on = false;
        }

        if self.key_thrust && self.sprite.visible {
            self.thrusting = true;
            let angle = ((self.sprite.cur_frame as i32 * 360) / 32) as u32;
            self.sprite.y_delta -= fixed_trig::cos_d(angle) * ACCELERATION;
            self.sprite.x_delta += fixed_trig::sin_d(angle) * ACCELERATION;
        } else {
            self.thrusting = false;
        }

        // f32 multiplication commutes bit-exactly, so `*=` matches the
        // original `XDelta = FRICTION * XDelta`.
        self.sprite.x_delta *= FRICTION;
        self.sprite.y_delta *= FRICTION;

        let _ = self.sprite.update(clip, rand);

        if self.switch_shots {
            // Can't fire while switching guns.
            if !self.shots[self.cur_shots].any_on_screen() {
                self.cur_shots = if self.num_super_shots != 0 {
                    SUPER_SHOTS
                } else if self.num_power_shots != 0 {
                    POWER_SHOTS
                } else {
                    NORMAL_SHOTS
                };
                self.switch_shots = false;
                events.push(GameEvent::SfxChangeGun);
            }
        } else {
            let mut num_fired = 0u32;
            if self.num_rapids != 0 && self.sprite.visible {
                if self.key_fire {
                    num_fired = self.fire_current(events) as u32;
                }
            } else if !self.firing && self.sprite.visible {
                if self.key_fire {
                    num_fired = self.fire_current(events) as u32;
                    self.firing = true;
                }
            } else if !self.key_fire {
                self.firing = false;
            }

            if num_fired != 0 {
                if self.num_spreads != 0 {
                    self.num_spreads -= 1;
                }
                if num_fired >= self.num_rapids {
                    self.num_rapids = 0;
                } else {
                    self.num_rapids -= num_fired;
                }
                if self.num_super_shots != 0 {
                    if num_fired >= self.num_super_shots {
                        self.num_super_shots = 0;
                        self.switch_shots = true;
                    } else {
                        self.num_super_shots -= num_fired;
                    }
                } else if self.num_power_shots != 0 {
                    if num_fired >= self.num_power_shots {
                        self.num_power_shots = 0;
                        self.switch_shots = true;
                    } else {
                        self.num_power_shots -= num_fired;
                    }
                }
            }
        }

        if self.key_bombs {
            if !self.bomb_away && self.sprite.visible && self.num_bombs != 0 {
                self.bombs.fire(&self.sprite, events);
                self.num_bombs -= 1;
                self.bomb_away = true;
            }
        } else {
            self.bomb_away = false;
        }
    }

    fn fire_current(&mut self, events: &mut Events) -> bool {
        self.shots[self.cur_shots].fire(&self.sprite, self.num_spreads != 0, events)
    }

    /// `CPlayerShip::Draw` — shots, bombs, shield halo, hull, thrust.
    pub fn draw(&mut self, world: &VirtualFrame, screen: &mut Frame, thrust: &mut Thrust) {
        self.shots[self.cur_shots].draw(world, screen);
        self.bombs.draw(world, screen);

        if self.sprite.visible {
            if self.shield_on {
                self.shield_sprite.x_pos = self.sprite.x_pos;
                self.shield_sprite.y_pos = self.sprite.y_pos;
                self.shield_sprite.cur_frame = self.sprite.cur_frame;
                self.shield_sprite.draw(world, screen);
            }

            self.sprite.draw(world, screen);

            if self.thrusting {
                self.cur_thrust += 1;
                if self.cur_thrust >= self.num_thrust {
                    self.cur_thrust = 0;
                }
                thrust.draw(
                    world,
                    screen,
                    self.sprite.cur_frame as i32,
                    self.cur_thrust,
                    self.sprite.x_pos as i32,
                    self.sprite.y_pos as i32,
                );
            }
        }
    }

    /// `CPlayerShip::Check` — base sprite (always included) plus the
    /// power-up counters, in the original accumulation order.
    pub fn check(&self) -> f32 {
        let mut sum = self.sprite.check(true);
        sum +=
            (self.cur_shots as u32 + self.frag_count + self.num_shields + self.num_rapids) as f32;
        sum += (self.num_spreads + self.num_bombs + self.num_power_shots + self.num_super_shots)
            as f32;
        sum
    }

    // Power-up mutators (`AddPowerShots` etc.) — capped at 999/limits.

    pub fn add_power_shots(&mut self, num: u32) {
        if self.num_power_shots == 0 {
            self.switch_shots = true;
        }
        self.num_power_shots = (self.num_power_shots + num).min(999);
    }

    pub fn add_super_shots(&mut self, num: u32) {
        if self.num_super_shots == 0 {
            self.switch_shots = true;
        }
        self.num_super_shots = (self.num_super_shots + num).min(999);
    }

    pub fn add_rapids(&mut self, num: u32) {
        self.num_rapids = (self.num_rapids + num).min(999);
    }

    pub fn add_hp(&mut self, num: u32) {
        self.sprite.hp = (self.sprite.hp + num).min(MAX_HP);
    }

    pub fn add_spreads(&mut self, num: u32) {
        self.num_spreads = (self.num_spreads + num).min(999);
    }

    pub fn add_bombs(&mut self, num: u32) {
        self.num_bombs = (self.num_bombs + num).min(999);
    }

    pub fn add_shields(&mut self, num: u32) {
        self.num_shields = (self.num_shields + num).min(MAX_SHIELDS);
    }

    pub fn add_score(&mut self, add: u32) {
        self.score += add;
    }

    pub fn add_ship(&mut self) {
        self.num_ships = (self.num_ships + 1).min(MAX_SHIPS);
    }

    pub fn remove_ship(&mut self) {
        self.num_ships = self.num_ships.saturating_sub(1);
    }
}

impl Default for PlayerShip {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (PlayerShip, VirtualFrame, Explosions, Events, Rand, Rect) {
        let mut world = VirtualFrame::new(2048, 1024);
        world.set_on_screen_rect(Rect::new(0, 0, 640, 480));
        (
            PlayerShip::new(),
            world,
            Explosions::new(),
            Events::new(),
            Rand::new(),
            Rect::new(0, 0, 2048, 1024),
        )
    }

    #[test]
    fn thrust_accelerates_along_facing_with_friction() {
        let (mut ship, world, mut ex, mut ev, mut rand, clip) = setup();
        ship.reset(3);
        ship.set_inputs(ShipInputs {
            thrust: true,
            ..Default::default()
        });
        ship.update(&clip, &mut rand, &world, &mut ex, &mut ev);
        // Facing up (frame 0): pure -y acceleration, then friction.
        assert_eq!(ship.sprite.x_delta, 0.98f32 * 0.0);
        let expected = 0.98f32 * (0.0 - fixed_trig::cos_d(0) * 1.2);
        assert_eq!(ship.sprite.y_delta.to_bits(), expected.to_bits());
        assert!(ship.thrusting);
    }

    #[test]
    fn fire_is_edge_triggered_without_rapids() {
        let (mut ship, world, mut ex, mut ev, mut rand, clip) = setup();
        ship.reset(3);
        ship.set_inputs(ShipInputs {
            fire: true,
            ..Default::default()
        });
        ship.update(&clip, &mut rand, &world, &mut ex, &mut ev);
        assert_eq!(
            ship.shots[NORMAL_SHOTS]
                .iter()
                .filter(|s| s.visible)
                .count(),
            1
        );
        // Held fire doesn't fire again…
        ship.update(&clip, &mut rand, &world, &mut ex, &mut ev);
        assert_eq!(
            ship.shots[NORMAL_SHOTS]
                .iter()
                .filter(|s| s.visible)
                .count(),
            1
        );
        // …until released and pressed again.
        ship.set_inputs(ShipInputs::default());
        ship.update(&clip, &mut rand, &world, &mut ex, &mut ev);
        ship.set_inputs(ShipInputs {
            fire: true,
            ..Default::default()
        });
        ship.update(&clip, &mut rand, &world, &mut ex, &mut ev);
        assert_eq!(
            ship.shots[NORMAL_SHOTS]
                .iter()
                .filter(|s| s.visible)
                .count(),
            2
        );
    }

    #[test]
    fn shields_drain_while_held() {
        let (mut ship, world, mut ex, mut ev, mut rand, clip) = setup();
        ship.reset(3);
        let start = ship.num_shields;
        ship.set_inputs(ShipInputs {
            shield: true,
            ..Default::default()
        });
        ship.update(&clip, &mut rand, &world, &mut ex, &mut ev);
        assert!(ship.shield_on);
        assert_eq!(ship.num_shields, start - 1);
    }

    #[test]
    fn power_shot_pickup_switches_gun_between_volleys() {
        let (mut ship, world, mut ex, mut ev, mut rand, clip) = setup();
        ship.reset(3);
        ship.add_power_shots(2);
        // Next quiet update performs the switch.
        ship.update(&clip, &mut rand, &world, &mut ex, &mut ev);
        assert_eq!(ship.cur_shots, POWER_SHOTS);
        assert!(ev.drain().any(|e| matches!(e, GameEvent::SfxChangeGun)));

        // Firing both power shots drops back to normal (switch pending).
        for _ in 0..2 {
            ship.set_inputs(ShipInputs {
                fire: true,
                ..Default::default()
            });
            ship.update(&clip, &mut rand, &world, &mut ex, &mut ev);
            ship.set_inputs(ShipInputs::default());
            ship.update(&clip, &mut rand, &world, &mut ex, &mut ev);
        }
        assert_eq!(ship.num_power_shots, 0);
        // Wait for shots to clear, then the switch back happens.
        for _ in 0..40 {
            ship.update(&clip, &mut rand, &world, &mut ex, &mut ev);
        }
        assert_eq!(ship.cur_shots, NORMAL_SHOTS);
    }

    #[test]
    fn bombs_are_edge_triggered_and_limited() {
        let (mut ship, world, mut ex, mut ev, mut rand, clip) = setup();
        ship.reset(3);
        ship.add_bombs(1);
        ship.set_inputs(ShipInputs {
            bomb: true,
            ..Default::default()
        });
        ship.update(&clip, &mut rand, &world, &mut ex, &mut ev);
        assert_eq!(ship.num_bombs, 0);
        assert_eq!(ship.bombs.iter().filter(|s| s.visible).count(), 1);
        // Held key + no bombs left: nothing more launches.
        ship.update(&clip, &mut rand, &world, &mut ex, &mut ev);
        assert_eq!(ship.bombs.iter().filter(|s| s.visible).count(), 1);
    }
}
