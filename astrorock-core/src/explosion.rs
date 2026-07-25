//! # Explosions — port of `Explosion.cpp`
//!
//! Fixed pool of 15 explosion sprites: slot 0 uses the big `bgexp`
//! sequence, slots 1..15 the regular `explo` sequence. `CExplo`
//! overrides the timeout to go *invisible* instead of destroying (the
//! pool is permanent), resetting its animation on re-activation.
//!
//! Faithful quirk: `ExploSprite`'s free-slot scan increments twice
//! (`i++` in the body and `++i` in the condition), so it only ever
//! considers even slots 0, 2, 4, … 14.

use crate::events::{Events, GameEvent};
use crate::frame::Frame;
use crate::rand::Rand;
use crate::rect::Rect;
use crate::sequence;
use crate::sprite::Sprite;
use crate::virtual_frame::VirtualFrame;

const MAX_EXPLOSIONS: usize = 15;

pub struct Explosions {
    pool: Vec<Sprite>,
    /// Per-slot animation lifetime (`SetDuration(NumFrames - 1)`).
    durations: Vec<u32>,
    counters: Vec<u32>,
}

impl Explosions {
    /// `ExplosionsInit` — build the pool (slot 0 big, rest regular).
    pub fn new() -> Self {
        let big = sequence::bg_explo();
        let small = sequence::explo();
        let mut pool = Vec::with_capacity(MAX_EXPLOSIONS);
        let mut durations = Vec::with_capacity(MAX_EXPLOSIONS);
        for i in 0..MAX_EXPLOSIONS {
            let mut s = Sprite::new();
            let seq = if i == 0 { big.clone() } else { small.clone() };
            durations.push(seq.num_frames - 1);
            s.set_sequence(seq);
            s.visible = false;
            pool.push(s);
        }
        Self {
            pool,
            durations,
            counters: vec![0; MAX_EXPLOSIONS],
        }
    }

    /// `CExplo::SetVisible` — restart animation state on (de)activation.
    fn set_visible(&mut self, slot: usize, vis: bool) {
        self.counters[slot] = 0;
        self.pool[slot].cur_frame = 0.0;
        self.pool[slot].visible = vis;
    }

    /// `ExplosionsReset`.
    pub fn reset(&mut self) {
        for i in 0..MAX_EXPLOSIONS {
            self.pool[i].reset();
            self.pool[i].cur_frame = 0.0;
            self.pool[i].visible = false;
            self.counters[i] = 0;
        }
    }

    /// `ExplosionsUpdate` — `CExplo::Update` per slot: lifetime check
    /// (expiry hides, never destroys), then the normal sprite update
    /// with the timeout suppressed.
    pub fn update(&mut self, clip: &Rect, rand: &mut Rand) {
        for i in 0..MAX_EXPLOSIONS {
            let s = &mut self.pool[i];
            if s.visible {
                self.counters[i] += 1;
                if self.counters[i] > self.durations[i] {
                    self.counters[i] = 0;
                    s.visible = false;
                    continue;
                }
            }
            s.duration = 0; // pool sprites never self-destroy
            let _ = s.update(clip, rand);
        }
    }

    /// `ExplosionsDraw` (radar plotting arrives with the radar caller).
    pub fn draw(&self, world: &VirtualFrame, screen: &mut Frame) {
        for s in &self.pool {
            s.draw(world, screen);
        }
    }

    /// `ExplosionsCheck`.
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

    /// `PlayExplo(x, y)` — medium explosion at a point (skips slot 0).
    pub fn play_explo(&mut self, x: i32, y: i32, world: &VirtualFrame, events: &mut Events) {
        let (pan, _) = world.pos_rel_center(x, 0);
        events.push(GameEvent::SfxMedExplosion { pan });
        for i in 1..MAX_EXPLOSIONS {
            if !self.pool[i].visible {
                self.set_visible(i, true);
                self.pool[i].x_pos = x as f32;
                self.pool[i].y_pos = y as f32;
                return;
            }
        }
    }

    /// `PlayShotHitAtSprite` — medium explosion at a sprite (skips 0).
    pub fn play_shot_hit_at(
        &mut self,
        x_pos: f32,
        y_pos: f32,
        world: &VirtualFrame,
        events: &mut Events,
    ) {
        let (pan, _) = world.pos_rel_center(x_pos as i32, 0);
        events.push(GameEvent::SfxMedExplosion { pan });
        for i in 1..MAX_EXPLOSIONS {
            if !self.pool[i].visible {
                self.set_visible(i, true);
                self.pool[i].x_pos = x_pos;
                self.pool[i].y_pos = y_pos;
                return;
            }
        }
    }

    /// `ExploSprite` — hide the victim, big-explosion sound, claim a
    /// slot with the original's even-slots-only double-increment scan.
    pub fn explo_sprite(&mut self, victim: &mut Sprite, world: &VirtualFrame, events: &mut Events) {
        victim.visible = false;
        let (pan, _) = world.pos_rel_center(victim.x_pos as i32, 0);
        events.push(GameEvent::SfxBigExplosion { pan });

        let mut i = 0usize;
        loop {
            if !self.pool[i].visible {
                self.set_visible(i, true);
                self.pool[i].x_pos = victim.x_pos;
                self.pool[i].y_pos = victim.y_pos;
                return;
            }
            i += 1; // body `i++` …
            i += 1; // … and the loop's `++i`
            if i >= MAX_EXPLOSIONS {
                return;
            }
        }
    }
}

impl Default for Explosions {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Explosions, VirtualFrame, Events, Rand) {
        let mut world = VirtualFrame::new(2048, 1024);
        world.set_on_screen_rect(Rect::new(0, 0, 640, 480));
        (Explosions::new(), world, Events::new(), Rand::new())
    }

    #[test]
    fn explosion_animates_then_hides() {
        let (mut ex, world, mut events, mut rand) = setup();
        ex.play_explo(100, 100, &world, &mut events);
        assert!(ex.pool[1].visible);
        let clip = Rect::new(0, 0, 2048, 1024);
        // Runs for its duration, then hides itself.
        for _ in 0..=ex.durations[1] {
            ex.update(&clip, &mut rand);
        }
        assert!(!ex.pool[1].visible);
        assert_eq!(
            events.drain().next(),
            Some(GameEvent::SfxMedExplosion { pan: 100 - 320 })
        );
    }

    #[test]
    fn explo_sprite_hides_victim_and_scans_even_slots() {
        let (mut ex, world, mut events, _) = setup();
        // Occupy slot 0 so the scan must skip to slot 2 (1 is skipped
        // by the double increment).
        ex.set_visible(0, true);
        ex.set_visible(1, false);

        let mut victim = Sprite::new();
        victim.set_sequence(sequence::ast_big());
        victim.x_pos = 500.0;
        victim.y_pos = 300.0;
        ex.explo_sprite(&mut victim, &world, &mut events);

        assert!(!victim.visible);
        assert!(
            !ex.pool[1].visible,
            "slot 1 must be skipped (even-only scan)"
        );
        assert!(ex.pool[2].visible, "slot 2 should be claimed");
        assert_eq!(
            events.drain().next(),
            Some(GameEvent::SfxBigExplosion { pan: 500 - 320 })
        );
    }
}
