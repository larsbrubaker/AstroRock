//! # Goodies — port of `goodies.cpp`
//!
//! Eight five-slot powerup pools. A dying little rock rolls
//! `NetRand(100)` against the level's cumulative cutoff table
//! (`goodies.cfg`: shields, rapid, health, gunone, guntwo, freeman,
//! bombs, spread — a roll past the last cutoff drops nothing) and the
//! goody inherits the rock's position and velocity.
//!
//! Collection ignores shields and damage entirely (`GoodiesCollide*`
//! hardcodes its own handlers): the goody re-hides with a fresh random
//! frame and pays out NetRand-sized amounts. Order discipline matters —
//! the spawn chain, the collide walk, the draw order, and the reset
//! order each differ in the original and are kept verbatim.

use std::rc::Rc;

use crate::events::{Events, GameEvent};
use crate::frame::Frame;
use crate::pship::PlayerShip;
use crate::rand::Rand;
use crate::rect::Rect;
use crate::rocks::MAX_LEVELS;
use crate::sequence::{self, FrameSequence};
use crate::sprite::Sprite;
use crate::virtual_frame::VirtualFrame;

const POOL: usize = 5;
/// Pool indices, in the *storage* order (init order).
const SHIELD: usize = 0;
const RAPID: usize = 1;
const HEALTH: usize = 2;
const GUN1: usize = 3;
const GUN2: usize = 4;
const SPREAD: usize = 5;
const BOMB: usize = 6;
const FREEMAN: usize = 7;

/// What a collected goody was (for the pickup sound).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoodyKind {
    Shield,
    Rapid,
    Health,
    Gun1,
    Gun2,
    Freeman,
    Spread,
    Bomb,
}

const GOODIES_CFG: &str = include_str!("../../assets/config/goodies.cfg");

pub struct Goodies {
    pools: Vec<Vec<Sprite>>,
    cutoffs: [[u32; 8]; MAX_LEVELS],
    /// shields, rapid, health, gun1, gun2, freeman, bombs, spread.
    cur: [u32; 8],
}

impl Goodies {
    /// `GoodiesInit` — the cfg has a human column-header line first.
    pub fn new() -> Self {
        let mut cutoffs = [[0u32; 8]; MAX_LEVELS];
        for (i, line) in GOODIES_CFG.lines().skip(1).take(MAX_LEVELS).enumerate() {
            let mut parts = line.split(&[':', ','][..]);
            let _level = parts.next();
            for slot in cutoffs[i].iter_mut() {
                *slot = parts
                    .next()
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0);
            }
        }

        let arts: [Rc<FrameSequence>; 8] = [
            sequence::pows(),   // shields
            sequence::rapid(),  // rapid fire
            sequence::health(), // health
            sequence::gun01(),  // power shots
            sequence::gun02(),  // super shots
            sequence::spred(),  // spread fire
            sequence::bombg(),  // razor bombs
            sequence::one_up(), // free man
        ];
        let pools = arts
            .into_iter()
            .map(|seq| {
                (0..POOL)
                    .map(|_| {
                        let mut s = Sprite::new();
                        s.set_sequence(seq.clone());
                        s.visible = false;
                        s
                    })
                    .collect()
            })
            .collect();

        Self {
            pools,
            cutoffs,
            cur: [10, 20, 30, 40, 50, 60, 70, 80],
        }
    }

    /// `SetVisAndMove` — hide with a fresh random frame (one draw).
    fn hide_with_random_frame(sprite: &mut Sprite, net_rand: &mut Rand) {
        sprite.reset();
        let frames = sprite.sequence().expect("seq").num_frames;
        sprite.cur_frame = net_rand.rand(frames) as f32;
        sprite.visible = false;
    }

    /// `GoodiesReset(level)` — cutoffs, then hide every pool in the
    /// original reset order (shield, rapid, health, gun1, gun2,
    /// spread, freeman, bombs).
    pub fn reset(&mut self, level: usize, net_rand: &mut Rand) {
        self.cur = if level < MAX_LEVELS {
            self.cutoffs[level]
        } else {
            [10, 20, 30, 40, 50, 60, 70, 80]
        };
        for &pool in &[SHIELD, RAPID, HEALTH, GUN1, GUN2, SPREAD, FREEMAN, BOMB] {
            for i in 0..POOL {
                Self::hide_with_random_frame(&mut self.pools[pool][i], net_rand);
            }
        }
    }

    /// `AddGoody` — the drop roll from a dying little rock. Chain order:
    /// shield, rapid, health, gun1, gun2, freeman, bomb, spread; a roll
    /// beyond the spread cutoff drops nothing.
    pub fn add_goody(&mut self, x: f32, y: f32, x_delta: f32, y_delta: f32, net_rand: &mut Rand) {
        let chance = net_rand.rand(100);
        let (cur, pool) = if chance < self.cur[0] {
            (0, SHIELD)
        } else if chance < self.cur[1] {
            (1, RAPID)
        } else if chance < self.cur[2] {
            (2, HEALTH)
        } else if chance < self.cur[3] {
            (3, GUN1)
        } else if chance < self.cur[4] {
            (4, GUN2)
        } else if chance < self.cur[5] {
            (5, FREEMAN)
        } else if chance < self.cur[6] {
            (6, BOMB)
        } else if chance < self.cur[7] {
            (7, SPREAD)
        } else {
            return;
        };
        let _ = cur;
        for i in 0..POOL {
            if !self.pools[pool][i].visible {
                let s = &mut self.pools[pool][i];
                s.visible = true;
                s.x_pos = x;
                s.y_pos = y;
                let frames = s.sequence().expect("seq").num_frames;
                s.cur_frame = net_rand.rand(frames) as f32;
                s.x_delta = x_delta;
                s.y_delta = y_delta;
                return;
            }
        }
    }

    /// `GoodiesUpdate` — shield, rapid, health, gun1, gun2, freeman,
    /// spread, bombs.
    pub fn update(&mut self, clip: &Rect, net_rand: &mut Rand) {
        for &pool in &[SHIELD, RAPID, HEALTH, GUN1, GUN2, FREEMAN, SPREAD, BOMB] {
            for s in &mut self.pools[pool] {
                let _ = s.update(clip, net_rand);
            }
        }
    }

    /// `GoodiesDraw` — shield, rapid, health, gun1, gun2, spread,
    /// freeman, bombs. No radar blips.
    pub fn draw(&self, world: &VirtualFrame, screen: &mut Frame) {
        for &pool in &[SHIELD, RAPID, HEALTH, GUN1, GUN2, SPREAD, FREEMAN, BOMB] {
            for s in &self.pools[pool] {
                s.draw(world, screen);
            }
        }
    }

    /// `GoodiesCheck` — same order as draw.
    pub fn check(&self) -> f32 {
        let mut sum = 0.0f32;
        for &pool in &[SHIELD, RAPID, HEALTH, GUN1, GUN2, SPREAD, FREEMAN, BOMB] {
            for s in &self.pools[pool] {
                sum += s.check(false);
            }
        }
        sum
    }

    /// `GoodiesCollideSprite` — pickup walk in the original collide
    /// order (shield, rapid, health, gun1, gun2, BOMBS, freeman,
    /// spread). Shields and damage are irrelevant; the goody re-hides
    /// (one frame draw) and pays out (its own draws).
    pub fn collide_with_player(
        &mut self,
        ship: &mut PlayerShip,
        clip: &Rect,
        net_rand: &mut Rand,
        events: &mut Events,
    ) {
        if !ship.sprite.visible {
            return;
        }
        const ORDER: [(usize, GoodyKind); 8] = [
            (SHIELD, GoodyKind::Shield),
            (RAPID, GoodyKind::Rapid),
            (HEALTH, GoodyKind::Health),
            (GUN1, GoodyKind::Gun1),
            (GUN2, GoodyKind::Gun2),
            (BOMB, GoodyKind::Bomb),
            (FREEMAN, GoodyKind::Freeman),
            (SPREAD, GoodyKind::Spread),
        ];
        for (pool, kind) in ORDER {
            for i in 0..POOL {
                let hit = {
                    let g = &self.pools[pool][i];
                    g.visible && g.collide_sprite(&ship.sprite, clip)
                };
                if hit {
                    Self::hide_with_random_frame(&mut self.pools[pool][i], net_rand);
                    Self::get_goody(ship, kind, net_rand, events);
                }
            }
        }
    }

    /// `GetGoody` — payout amounts draw NetRand in a fixed order.
    fn get_goody(ship: &mut PlayerShip, kind: GoodyKind, net_rand: &mut Rand, events: &mut Events) {
        match kind {
            GoodyKind::Shield => {
                let amount = 15 + net_rand.rand(8) + net_rand.rand(8) + net_rand.rand(8);
                ship.add_shields(amount);
            }
            GoodyKind::Rapid => {
                let amount = 30 + net_rand.rand(32) + net_rand.rand(32);
                ship.add_rapids(amount);
            }
            GoodyKind::Health => {
                let amount = 15 + net_rand.rand(8) + net_rand.rand(8) + net_rand.rand(8);
                ship.add_hp(amount);
            }
            GoodyKind::Gun1 => {
                let amount = 15 + net_rand.rand(8) + net_rand.rand(8);
                ship.add_power_shots(amount);
            }
            GoodyKind::Gun2 => {
                let amount = 15 + net_rand.rand(8) + net_rand.rand(8);
                ship.add_super_shots(amount);
            }
            GoodyKind::Freeman => {
                ship.add_ship();
            }
            GoodyKind::Spread => {
                let amount = 30 + net_rand.rand(32) + net_rand.rand(32);
                ship.add_spreads(amount);
            }
            GoodyKind::Bomb => {
                let amount = 4 + net_rand.rand(8);
                ship.add_bombs(amount);
            }
        }
        events.push(GameEvent::GoodyCollected { kind });
    }
}

impl Default for Goodies {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip() -> Rect {
        Rect::new(0, 0, 2048, 1024)
    }

    #[test]
    fn cfg_parses_cumulative_cutoffs() {
        let g = Goodies::new();
        // goodies.cfg level 1: 20,20,40,40,40,40,40,40.
        assert_eq!(g.cutoffs[0], [20, 20, 40, 40, 40, 40, 40, 40]);
    }

    #[test]
    fn drops_spawn_with_inherited_motion_or_nothing() {
        let mut g = Goodies::new();
        let mut nr = Rand::new();
        g.reset(0, &mut nr);

        // Roll many drops; level 1 gives ~40% drop rate, all shields
        // or health (rapid's cutoff equals shield's).
        for _ in 0..50 {
            g.add_goody(700.0, 300.0, 2.5, -1.5, &mut nr);
        }
        let shields = g.pools[SHIELD].iter().filter(|s| s.visible).count();
        let healths = g.pools[HEALTH].iter().filter(|s| s.visible).count();
        assert!(shields + healths > 0, "no goodies dropped in 50 rolls");
        assert_eq!(
            g.pools[RAPID].iter().filter(|s| s.visible).count(),
            0,
            "rapid can't drop on level 1 (cutoff equals shield's)"
        );
        let s = g.pools[SHIELD]
            .iter()
            .chain(g.pools[HEALTH].iter())
            .find(|s| s.visible)
            .unwrap();
        assert_eq!((s.x_delta, s.y_delta), (2.5, -1.5), "inherits velocity");
    }

    #[test]
    fn pickup_pays_out_and_rehides() {
        let mut g = Goodies::new();
        let mut nr = Rand::new();
        let mut ev = Events::new();
        g.reset(0, &mut nr);

        let mut ship = PlayerShip::new();
        ship.reset(3);
        ship.sprite.x_pos = 700.0;
        ship.sprite.y_pos = 300.0;

        // Place a shield goody on the ship.
        g.pools[SHIELD][0].visible = true;
        g.pools[SHIELD][0].x_pos = 700.0;
        g.pools[SHIELD][0].y_pos = 300.0;

        let before = ship.num_shields;
        g.collide_with_player(&mut ship, &clip(), &mut nr, &mut ev);
        assert!(!g.pools[SHIELD][0].visible, "goody re-hidden");
        assert!(
            ship.num_shields >= before + 15,
            "shield payout at least the base 15"
        );
        assert!(ev.drain().any(|e| matches!(
            e,
            GameEvent::GoodyCollected {
                kind: GoodyKind::Shield
            }
        )));
    }

    #[test]
    fn freeman_grants_a_ship() {
        let mut g = Goodies::new();
        let mut nr = Rand::new();
        let mut ev = Events::new();
        g.reset(0, &mut nr);
        let mut ship = PlayerShip::new();
        ship.reset(3);
        ship.sprite.x_pos = 700.0;
        ship.sprite.y_pos = 300.0;
        g.pools[FREEMAN][0].visible = true;
        g.pools[FREEMAN][0].x_pos = 700.0;
        g.pools[FREEMAN][0].y_pos = 300.0;
        g.collide_with_player(&mut ship, &clip(), &mut nr, &mut ev);
        assert_eq!(ship.num_ships, 4);
    }
}
