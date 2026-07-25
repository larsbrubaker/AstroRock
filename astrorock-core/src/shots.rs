//! # Shots — port of `shots.cpp`
//!
//! A pool of `CShot` sprites (duration 30 updates, expiry hides like
//! `CExplo`). Firing picks the ship's facing from its rotation frame
//! (32 frames = 360°), launches at SHOTSPEED 9 plus the shooter's
//! momentum, and spawns 1.3 velocity-lengths ahead of the ship.

use std::rc::Rc;

use crate::events::{Events, GameEvent};
use crate::fixed_trig;
use crate::frame::Frame;
use crate::rand::Rand;
use crate::rect::Rect;
use crate::sequence::FrameSequence;
use crate::sprite::Sprite;
use crate::virtual_frame::VirtualFrame;

const SHOT_SPEED: i32 = 9;
const SHOT_DURATION: u32 = 30;

/// Which fire sound a pool triggers (`pFireSound`), carried on the
/// event so the audio phase can map tiers to rShot01/02/03Snd.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotTier {
    Normal,
    Power,
    Super,
}

pub struct Shots {
    pool: Vec<Sprite>,
    counters: Vec<u32>,
    pub damage: u32,
    tier: ShotTier,
}

impl Shots {
    /// `CShots::Load` — `maxshots` pool entries sharing one sequence.
    pub fn new(seq: Rc<FrameSequence>, tier: ShotTier, max_shots: usize) -> Self {
        let mut pool = Vec::with_capacity(max_shots);
        for _ in 0..max_shots {
            let mut s = Sprite::new();
            s.set_sequence(seq.clone());
            s.visible = false;
            pool.push(s);
        }
        Self {
            pool,
            counters: vec![0; max_shots],
            damage: 0,
            tier,
        }
    }

    /// `CShotsConfig`.
    pub fn config(&mut self, damage: u32) {
        self.damage = damage;
    }

    /// `CShotsReset`.
    pub fn reset(&mut self) {
        for (i, s) in self.pool.iter_mut().enumerate() {
            s.reset();
            s.cur_frame = 0.0;
            s.visible = false;
            self.counters[i] = 0;
        }
    }

    /// `CShot::Update` per slot — expiry hides, then normal update.
    pub fn update(&mut self, clip: &Rect, rand: &mut Rand) {
        for i in 0..self.pool.len() {
            let s = &mut self.pool[i];
            if s.visible {
                self.counters[i] += 1;
                if self.counters[i] > SHOT_DURATION {
                    self.counters[i] = 0;
                    s.visible = false;
                    continue;
                }
            }
            s.duration = 0;
            let _ = s.update(clip, rand);
        }
    }

    pub fn draw(&self, world: &VirtualFrame, screen: &mut Frame) {
        for s in &self.pool {
            s.draw(world, screen);
        }
    }

    /// `CShots::Check` (via the list walk).
    pub fn check(&self) -> f32 {
        let mut sum = 0.0f32;
        for s in &self.pool {
            sum += s.check(false);
        }
        sum
    }

    /// `CShotsNumOnScreen` — any shot visible?
    pub fn any_on_screen(&self) -> bool {
        self.pool.iter().any(|s| s.visible)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Sprite> {
        self.pool.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Sprite> {
        self.pool.iter_mut()
    }

    /// Hide shot `i` (collision handlers' `HideSprite`).
    pub fn hide(&mut self, index: usize) {
        self.counters[index] = 0;
        self.pool[index].cur_frame = 0.0;
        self.pool[index].visible = false;
    }

    /// `fire` — one bullet at `angle` degrees from the shooter's state.
    fn fire_one(&mut self, who: &Sprite, mut angle: i32, events: &mut Events) -> bool {
        let (who_x, who_y, who_xd, who_yd) = (who.x_pos, who.y_pos, who.x_delta, who.y_delta);
        for i in 0..self.pool.len() {
            if !self.pool[i].visible {
                if angle < 0 {
                    angle += 360;
                }
                if angle >= 360 {
                    angle -= 360;
                }
                let mut xadd = fixed_trig::sin_d(angle as u32) * SHOT_SPEED as f32;
                let mut yadd = -(fixed_trig::cos_d(angle as u32) * SHOT_SPEED as f32);
                let s = &mut self.pool[i];
                s.x_pos = who_x + 1.3f32 * xadd;
                s.y_pos = who_y + 1.3f32 * yadd;
                xadd += who_xd;
                yadd += who_yd;
                s.x_delta = xadd;
                s.y_delta = yadd;
                self.counters[i] = 0;
                s.cur_frame = 0.0;
                s.visible = true;
                events.push(GameEvent::SfxShotFire { tier: self.tier });
                return true;
            }
        }
        false
    }

    /// `CShotsFire` — main shot plus optional ±15° spread pair. The
    /// shooter's facing comes from its CurFrame (32 rotation frames).
    pub fn fire(&mut self, who: &Sprite, spread: bool, events: &mut Events) -> bool {
        let angle = (who.cur_frame * (360.0f32 / 32.0f32)) as i32;
        let mut result = self.fire_one(who, angle, events);
        if spread {
            result = self.fire_one(who, angle - 15, events);
            if result {
                result = self.fire_one(who, angle + 15, events);
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence;

    fn clip() -> Rect {
        Rect::new(0, 0, 2048, 1024)
    }

    fn shots() -> Shots {
        let mut s = Shots::new(sequence::shot01(), ShotTier::Normal, 15);
        s.config(40);
        s
    }

    fn shooter(x: f32, y: f32, xd: f32, yd: f32, frame: f32) -> Sprite {
        let mut s = Sprite::new();
        s.x_pos = x;
        s.y_pos = y;
        s.x_delta = xd;
        s.y_delta = yd;
        s.cur_frame = frame;
        s
    }

    #[test]
    fn fire_up_moves_negative_y_plus_momentum() {
        let mut sh = shots();
        let mut ev = Events::new();
        // Frame 0 = facing up (angle 0): sin 0, -cos = -1.
        assert!(sh.fire(&shooter(1000.0, 500.0, 2.0, 0.5, 0.0), false, &mut ev));
        let s = sh.iter().find(|s| s.visible).unwrap();
        assert_eq!(s.x_delta, 2.0); // 0*9 + shooter 2.0
        assert_eq!(s.y_delta, -9.0 + 0.5);
        assert!(s.y_pos < 500.0, "spawned ahead of the ship");
        assert_eq!(ev.drain().count(), 1);
    }

    #[test]
    fn spread_fires_three() {
        let mut sh = shots();
        let mut ev = Events::new();
        assert!(sh.fire(&shooter(1000.0, 500.0, 0.0, 0.0, 8.0), true, &mut ev));
        assert_eq!(sh.iter().filter(|s| s.visible).count(), 3);
    }

    #[test]
    fn shots_expire_after_duration() {
        let mut sh = shots();
        let mut ev = Events::new();
        let mut rand = Rand::new();
        sh.fire(&shooter(1000.0, 500.0, 0.0, 0.0, 0.0), false, &mut ev);
        assert!(sh.any_on_screen());
        for _ in 0..=SHOT_DURATION {
            sh.update(&clip(), &mut rand);
        }
        assert!(!sh.any_on_screen());
    }

    #[test]
    fn pool_exhausts_at_capacity() {
        let mut sh = shots();
        let mut ev = Events::new();
        let who = shooter(0.0, 0.0, 0.0, 0.0, 0.0);
        for _ in 0..15 {
            assert!(sh.fire(&who, false, &mut ev));
        }
        assert!(!sh.fire(&who, false, &mut ev));
    }
}
