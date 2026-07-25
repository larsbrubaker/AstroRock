//! # Bombs — port of `bombs.cpp`
//!
//! Razor bombs: same pool pattern as shots but slower (BOMBSPEED 6),
//! long-lived (duration set by `CBombsConfig`, 90 for players), and a
//! bomb that times out detonates with a shot-hit explosion where it
//! died. Player bombs are configured indestructible (HP 0xFFFF) and
//! lethal (damage 0xFFFF).

use std::rc::Rc;

use crate::events::{Events, GameEvent};
use crate::explosion::Explosions;
use crate::fixed_trig;
use crate::frame::Frame;
use crate::rand::Rand;
use crate::rect::Rect;
use crate::sequence::FrameSequence;
use crate::sprite::Sprite;
use crate::virtual_frame::VirtualFrame;

const BOMB_SPEED: i32 = 6;

pub struct Bombs {
    pool: Vec<Sprite>,
    counters: Vec<u32>,
    duration: u32,
    pub start_hp: u32,
    pub damage: u32,
}

impl Bombs {
    /// `CBombs::Load`.
    pub fn new(seq: Rc<FrameSequence>, max_bombs: usize) -> Self {
        let mut pool = Vec::with_capacity(max_bombs);
        for _ in 0..max_bombs {
            let mut s = Sprite::new();
            s.set_sequence(seq.clone());
            s.visible = false;
            pool.push(s);
        }
        Self {
            pool,
            counters: vec![0; max_bombs],
            duration: 0,
            start_hp: 0,
            damage: 0,
        }
    }

    /// `CBombsConfig`.
    pub fn config(&mut self, hp: u32, damage: u32, duration: u32) {
        self.start_hp = hp;
        self.damage = damage;
        self.duration = duration;
    }

    /// `CBombsReset`.
    pub fn reset(&mut self) {
        for (i, s) in self.pool.iter_mut().enumerate() {
            s.reset();
            s.visible = false;
            s.cur_frame = 0.0;
            s.hp = self.start_hp;
            self.counters[i] = 0;
        }
    }

    /// `CBomb::Update` per slot — a timed-out bomb detonates
    /// (`PlayShotHitAtSprite`) as it disappears.
    pub fn update(
        &mut self,
        clip: &Rect,
        rand: &mut Rand,
        world: &VirtualFrame,
        explosions: &mut Explosions,
        events: &mut Events,
    ) {
        for i in 0..self.pool.len() {
            if self.pool[i].visible && self.duration != 0 {
                self.counters[i] += 1;
                if self.counters[i] > self.duration {
                    self.counters[i] = 0;
                    self.pool[i].visible = false;
                    let (x, y) = (self.pool[i].x_pos, self.pool[i].y_pos);
                    explosions.play_shot_hit_at(x, y, world, events);
                    continue;
                }
            }
            let s = &mut self.pool[i];
            s.duration = 0;
            let _ = s.update(clip, rand);
        }
    }

    pub fn draw(&self, world: &VirtualFrame, screen: &mut Frame) {
        for s in &self.pool {
            s.draw(world, screen);
        }
    }

    pub fn check(&self) -> f32 {
        let mut sum = 0.0f32;
        for s in &self.pool {
            sum += s.check(false);
        }
        sum
    }

    pub fn iter(&self) -> impl Iterator<Item = &Sprite> {
        self.pool.iter()
    }

    /// `CBombsFire` — facing from the shooter's rotation frame.
    pub fn fire(&mut self, who: &Sprite, events: &mut Events) -> bool {
        let (who_x, who_y, who_xd, who_yd) = (who.x_pos, who.y_pos, who.x_delta, who.y_delta);
        let mut angle = (who.cur_frame * (360.0f32 / 32.0f32)) as i32;
        for i in 0..self.pool.len() {
            if !self.pool[i].visible {
                if angle < 0 {
                    angle += 360;
                }
                if angle >= 360 {
                    angle -= 360;
                }
                let mut yadd = -(fixed_trig::cos_d(angle as u32) * BOMB_SPEED as f32);
                let mut xadd = fixed_trig::sin_d(angle as u32) * BOMB_SPEED as f32;
                let s = &mut self.pool[i];
                s.x_pos = who_x + 1.3f32 * xadd;
                s.y_pos = who_y + 1.3f32 * yadd;
                yadd += who_yd;
                xadd += who_xd;
                s.x_delta = xadd;
                s.y_delta = yadd;
                self.counters[i] = 0;
                s.cur_frame = 0.0;
                s.visible = true;
                events.push(GameEvent::SfxBombFire);
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence;

    #[test]
    fn bomb_times_out_with_detonation() {
        let mut bombs = Bombs::new(sequence::bomb(), 5);
        bombs.config(0xFFFF, 0xFFFF, 3);
        let mut ev = Events::new();
        let mut rand = Rand::new();
        let mut world = VirtualFrame::new(2048, 1024);
        world.set_on_screen_rect(Rect::new(0, 0, 640, 480));
        let mut ex = Explosions::new();
        let clip = Rect::new(0, 0, 2048, 1024);

        let mut who = Sprite::new();
        who.x_pos = 1000.0;
        who.y_pos = 500.0;
        who.x_delta = 0.0;
        who.y_delta = 0.0;
        who.cur_frame = 0.0;
        assert!(bombs.fire(&who, &mut ev));
        assert_eq!(ev.drain().count(), 1); // bomb-away sound
        for _ in 0..4 {
            bombs.update(&clip, &mut rand, &world, &mut ex, &mut ev);
        }
        assert!(bombs.iter().all(|s| !s.visible));
        // Detonation queued a shot-hit explosion sound.
        assert!(ev
            .drain()
            .any(|e| matches!(e, GameEvent::SfxMedExplosion { .. })));
    }
}
