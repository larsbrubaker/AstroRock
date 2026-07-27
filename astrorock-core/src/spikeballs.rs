//! # Spike Balls — port of `SpikeBall.cpp`
//!
//! Five-slot enemy with a four-state attack cycle over its 70-frame
//! sheet (0–60 rolling in 20-frame segments, 60–69 opening):
//!
//! - **None** — rolls; every 20th frame either re-randomizes the roll
//!   segment/direction or (1-in-SPIKEOPENNETRAND when the roll is 0)
//!   starts opening; direction changes on a 1-in-CHANGEDIRNETRAND roll.
//! - **Open** — animates 60→69, then starts the charge (sound whine
//!   with a rising frequency ramp — audio phase) and the pause timer.
//! - **Charging** — frozen (deltas zeroed) until the pause runs out.
//! - **Close** — animates 69→60; on arrival detonates: six explosions
//!   scattered ±EXPLODIST, one-beat `DoBang` blast box dealing
//!   SPIKEBALLBANGDAMAGE, then rolls off with a fresh random delta.
//!
//! Closed spikeballs shrug off damage (`damage >>= 5`) and change
//! direction when hit. Their reset quirk: `SetVisAndMove` never calls
//! `Reset()`, so hidden slots keep stale state.

use std::rc::Rc;

use crate::events::Events;
use crate::explosion::Explosions;
use crate::frame::Frame;
use crate::rand::Rand;
use crate::rect::Rect;
use crate::rocks::MAX_LEVELS;
use crate::sequence;
use crate::sprite::{Sprite, SpriteBlit};
use crate::virtual_frame::VirtualFrame;

pub const MAX_SPIKEBALLS: usize = 5;
const NUM_SURROUND_EXPLOS: u32 = 6;
/// `SPIKEBALLCOLLIDEDAMAGE`.
pub const SPIKEBALL_COLLIDE_DAMAGE: u32 = 50;
pub const SPIKEBALL_RADAR_COLOR: u8 = 40;

const SPIKEBALL_CFG: &str = include_str!("../../assets/config/spikeball.cfg");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Attack {
    None,
    Charging,
    Open,
    Close,
}

struct TypeStats {
    hp: u32,
    attack_pause: u32,
    change_dir_rand: u32,
    slow: u32,
    fast: u32,
    open_rand: u32,
    explo_dist: u32,
    bang_damage: u32,
    score: u32,
}

fn type_stats(t: u32) -> TypeStats {
    match t {
        2 => TypeStats {
            hp: 400,
            attack_pause: 45,
            change_dir_rand: 64,
            slow: 4,
            fast: 16,
            open_rand: 8,
            explo_dist: 256,
            bang_damage: 75,
            score: 450,
        },
        3 => TypeStats {
            hp: 600,
            attack_pause: 30,
            change_dir_rand: 30,
            slow: 6,
            fast: 18,
            open_rand: 6,
            explo_dist: 256,
            bang_damage: 90,
            score: 600,
        },
        4 => TypeStats {
            hp: 1200,
            attack_pause: 25,
            change_dir_rand: 10,
            slow: 15,
            fast: 20,
            open_rand: 3,
            explo_dist: 400,
            bang_damage: 50,
            score: 1600,
        },
        _ => TypeStats {
            hp: 200,
            attack_pause: 60,
            change_dir_rand: 84,
            slow: 4,
            fast: 10,
            open_rand: 9,
            explo_dist: 128,
            bang_damage: 30,
            score: 150,
        },
    }
}

pub struct SpikeBalls {
    pool: Vec<Sprite>,
    attacking: [Attack; MAX_SPIKEBALLS],
    do_bang: [bool; MAX_SPIKEBALLS],
    attack_pause: [u32; MAX_SPIKEBALLS],
    /// The charge whine's frequency per ball (`pSoundCharging[i]`,
    /// audio-only — no RNG, no sim effect): 22050 at charge start,
    /// `(f >> 6) + f` per charging beat.
    charge_freq: [u32; MAX_SPIKEBALLS],
    pub num_spikeballs: u32,
    max_spikeballs: u32,
    cur_type: u32,
    hp: u32,
    attack_pause_delay: u32,
    change_dir_rand: u32,
    slow: u32,
    fast: u32,
    open_rand: u32,
    explo_dist: u32,
    pub bang_damage: u32,
    score_per_kill: u32,
    level_num: [u32; MAX_LEVELS],
    level_type: [u32; MAX_LEVELS],
    remap2: Rc<[u8; 256]>,
    remap3: Rc<[u8; 256]>,
}

impl SpikeBalls {
    /// `SpikeBallsInit`.
    pub fn new() -> Self {
        let mut level_num = [0u32; MAX_LEVELS];
        let mut level_type = [0u32; MAX_LEVELS];
        for (i, line) in SPIKEBALL_CFG.lines().take(MAX_LEVELS).enumerate() {
            let mut parts = line.split(&[':', ','][..]);
            let _level = parts.next();
            level_num[i] = parts
                .next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0)
                .min(MAX_SPIKEBALLS as u32);
            level_type[i] = parts
                .next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0)
                .min(4);
        }

        let seq = sequence::spkball();
        let pool = (0..MAX_SPIKEBALLS)
            .map(|_| {
                let mut s = Sprite::new();
                s.set_sequence(seq.clone());
                s.visible = false;
                s
            })
            .collect();

        Self {
            pool,
            attacking: [Attack::None; MAX_SPIKEBALLS],
            do_bang: [false; MAX_SPIKEBALLS],
            attack_pause: [0; MAX_SPIKEBALLS],
            charge_freq: [22050; MAX_SPIKEBALLS],
            num_spikeballs: 0,
            max_spikeballs: 0,
            cur_type: 0,
            hp: 200,
            attack_pause_delay: 60,
            change_dir_rand: 84,
            slow: 4,
            fast: 10,
            open_rand: 9,
            explo_dist: 128,
            bang_damage: 30,
            score_per_kill: 150,
            level_num,
            level_type,
            remap2: Rc::new(crate::assets::remap_table(crate::assets::SPIKEBALL2_PAL)),
            remap3: Rc::new(crate::assets::remap_table(crate::assets::SPIKEBALL3_PAL)),
        }
    }

    fn set_type(&mut self, t: u32) {
        self.cur_type = t;
        let stats = type_stats(t);
        self.hp = stats.hp;
        self.attack_pause_delay = stats.attack_pause;
        self.change_dir_rand = stats.change_dir_rand;
        self.slow = stats.slow;
        self.fast = stats.fast;
        self.open_rand = stats.open_rand;
        self.explo_dist = stats.explo_dist;
        self.bang_damage = stats.bang_damage;
        self.score_per_kill = stats.score;
        let blit = match t {
            2 => SpriteBlit::RemapSource(self.remap2.clone()),
            3 | 4 => SpriteBlit::RemapSource(self.remap3.clone()),
            _ => SpriteBlit::Trans,
        };
        for s in &mut self.pool {
            s.blit = blit.clone();
        }
    }

    /// `NetRandDelta` — three draws: speed pick, then both axes.
    fn net_rand_delta(slow: u32, fast: u32, sprite: &mut Sprite, net_rand: &mut Rand) {
        let speed = if net_rand.rand(2) != 0 { slow } else { fast };
        sprite.x_delta = net_rand.rand_about0(speed) as f32;
        sprite.y_delta = net_rand.rand_about0(speed) as f32;
    }

    /// `SpikeBallsReset(level)` — note: no `Reset()` on the sprites.
    pub fn reset(&mut self, level: usize, net_rand: &mut Rand) {
        self.num_spikeballs = 0;
        let (max, ty) = if level < MAX_LEVELS {
            (self.level_num[level], self.level_type[level])
        } else {
            (MAX_SPIKEBALLS as u32, 1)
        };
        self.max_spikeballs = max;
        self.set_type(ty);

        for i in 0..MAX_SPIKEBALLS {
            if self.num_spikeballs < self.max_spikeballs {
                self.num_spikeballs += 1;
                let s = &mut self.pool[i];
                s.hp = self.hp;
                s.visible = true;
                s.x_pos = net_rand.rand(2048) as f32;
                s.y_pos = net_rand.rand(1024) as f32;
                s.cur_frame = 0.0;
                s.frame_advance = 1.0;
                Self::net_rand_delta(self.slow, self.fast, &mut self.pool[i], net_rand);
            } else {
                self.pool[i].visible = false;
            }
        }
        for i in 0..MAX_SPIKEBALLS {
            self.attacking[i] = Attack::None;
            self.do_bang[i] = false;
            // `pSoundCharging[i]->Stop(); SetFrequency(22050)`.
            self.charge_freq[i] = 22050;
        }
    }

    /// The charge whine's playback rate for ball `i` — `Some` while
    /// it is charging (1.0 = the sample's native pitch, rising every
    /// beat), `None` otherwise (sink stops the loop).
    pub fn charge_rate(&self, i: usize) -> Option<f32> {
        if self.pool[i].visible && self.attacking[i] == Attack::Charging {
            Some(self.charge_freq[i] as f32 / 22050.0)
        } else {
            None
        }
    }

    /// `SpikeBallsUpdate` — the attack state machine.
    pub fn update(
        &mut self,
        clip: &Rect,
        net_rand: &mut Rand,
        world: &VirtualFrame,
        explosions: &mut Explosions,
        events: &mut Events,
    ) {
        if self.num_spikeballs == 0 {
            return;
        }
        for i in 0..MAX_SPIKEBALLS {
            if !self.pool[i].visible {
                continue;
            }
            self.do_bang[i] = false;
            let cur_frame = self.pool[i].cur_frame as i32;

            match self.attacking[i] {
                Attack::Open => {
                    if cur_frame == 69 {
                        self.pool[i].frame_advance = 0.0;
                        // `pSoundCharging[i]->Play(); SetFrequency(22050)`
                        // — the whine starts at native pitch.
                        self.charge_freq[i] = 22050;
                        self.attack_pause[i] = self.attack_pause_delay;
                        self.attacking[i] = Attack::Charging;
                    }
                }
                Attack::Charging => {
                    self.attack_pause[i] -= 1;
                    // `SetFrequency((f >> 6) + f)` — the rising ramp.
                    let f = self.charge_freq[i];
                    self.charge_freq[i] = (f >> 6) + f;
                    self.pool[i].x_delta = 0.0;
                    self.pool[i].y_delta = 0.0;
                    if self.attack_pause[i] == 0 {
                        self.pool[i].frame_advance = -1.0;
                        self.attacking[i] = Attack::Close;
                    }
                }
                Attack::Close => {
                    if cur_frame == 60 {
                        let (x, y) = (self.pool[i].x_pos as i32, self.pool[i].y_pos as i32);
                        for _ in 0..NUM_SURROUND_EXPLOS {
                            let dx = net_rand.rand_about0(self.explo_dist);
                            let dy = net_rand.rand_about0(self.explo_dist);
                            explosions.play_explo(x + dx, y + dy, world, events);
                        }
                        Self::net_rand_delta(self.slow, self.fast, &mut self.pool[i], net_rand);
                        self.attacking[i] = Attack::None;
                        self.do_bang[i] = true;
                    }
                }
                Attack::None => {
                    if cur_frame % 20 == 0 {
                        if net_rand.rand(self.open_rand) != 0 {
                            // Re-randomize the roll segment/direction.
                            let mut frame = (net_rand.rand(4) * 20) as i32;
                            if net_rand.rand(2) != 0 {
                                self.pool[i].frame_advance = -1.0;
                                if frame == 0 {
                                    frame = 60;
                                }
                            } else {
                                self.pool[i].frame_advance = 1.0;
                                if frame == 60 {
                                    frame = 0;
                                }
                            }
                            self.pool[i].cur_frame = frame as f32;
                        } else {
                            self.attacking[i] = Attack::Open;
                            self.pool[i].cur_frame = 60.0;
                            self.pool[i].frame_advance = 1.0;
                        }
                    }
                    if net_rand.rand(self.change_dir_rand) == 0 {
                        Self::net_rand_delta(self.slow, self.fast, &mut self.pool[i], net_rand);
                    }
                }
            }
            let _ = self.pool[i].update(clip, net_rand);
        }
    }

    /// `SpikeBallsDraw` (+ type-4 LocalRand flicker).
    pub fn draw(&mut self, world: &VirtualFrame, screen: &mut Frame, local_rand: &mut Rand) {
        if self.cur_type == 4 {
            for i in 0..MAX_SPIKEBALLS {
                self.pool[i].blit = match local_rand.rand(3) {
                    0 => SpriteBlit::Trans,
                    1 => SpriteBlit::RemapSource(self.remap2.clone()),
                    _ => SpriteBlit::RemapSource(self.remap3.clone()),
                };
            }
        }
        if self.num_spikeballs != 0 {
            for s in &self.pool {
                s.draw(world, screen);
            }
        }
    }

    /// `SpikeBallsCheck`.
    pub fn check(&self) -> f32 {
        let mut sum = 0.0f32;
        if self.num_spikeballs != 0 {
            for s in &self.pool {
                sum += s.check(false);
            }
        }
        sum + self.num_spikeballs as f32
    }

    /// The one-beat blast box for slot `i`, if it's banging.
    pub fn bang_rect(&self, i: usize) -> Option<Rect> {
        if !self.do_bang[i] {
            return None;
        }
        let half = (self.explo_dist / 2) as i32;
        let (x, y) = (self.pool[i].x_pos as i32, self.pool[i].y_pos as i32);
        Some(Rect::new(x - half, y - half, x + half, y + half))
    }

    pub fn slots(&self) -> usize {
        MAX_SPIKEBALLS
    }

    /// `SpikeBallP1` — closed balls take damage/32 and change course.
    pub fn damage(
        &mut self,
        index: usize,
        mut damage: u32,
        net_rand: &mut Rand,
        world: &VirtualFrame,
        explosions: &mut Explosions,
        events: &mut Events,
    ) -> u32 {
        if self.attacking[index] == Attack::None {
            damage >>= 5;
            Self::net_rand_delta(self.slow, self.fast, &mut self.pool[index], net_rand);
        }
        let sprite = &mut self.pool[index];
        if damage >= sprite.hp {
            sprite.hp = 0;
            self.num_spikeballs -= 1;
            explosions.explo_sprite(&mut self.pool[index], world, events);
            self.score_per_kill
        } else {
            sprite.hp -= damage;
            explosions.play_shot_hit_at(sprite.x_pos, sprite.y_pos, world, events);
            0
        }
    }

    pub fn pool(&self) -> &[Sprite] {
        &self.pool
    }

    pub fn active(&self) -> bool {
        self.num_spikeballs != 0
    }
}

impl Default for SpikeBalls {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> VirtualFrame {
        let mut w = VirtualFrame::new(2048, 1024);
        w.set_on_screen_rect(Rect::new(0, 0, 640, 480));
        w
    }

    fn first_level() -> usize {
        let s = SpikeBalls::new();
        (0..MAX_LEVELS).find(|&i| s.level_num[i] > 0).unwrap()
    }

    #[test]
    fn closed_balls_shrug_off_damage_and_swerve() {
        let mut sb = SpikeBalls::new();
        let mut nr = Rand::new();
        let w = world();
        let mut ex = Explosions::new();
        let mut ev = Events::new();
        sb.reset(first_level(), &mut nr);

        let idx = sb.pool.iter().position(|s| s.visible).unwrap();
        let hp = sb.pool[idx].hp;
        // A 40-damage shot against a closed ball chips only 40>>5 = 1.
        sb.damage(idx, 40, &mut nr, &w, &mut ex, &mut ev);
        assert_eq!(sb.pool[idx].hp, hp - 1);
    }

    #[test]
    fn attack_cycle_reaches_bang() {
        let mut sb = SpikeBalls::new();
        let mut nr = Rand::new();
        let w = world();
        let mut ex = Explosions::new();
        let mut ev = Events::new();
        sb.reset(first_level(), &mut nr);
        let clip = Rect::new(0, 0, 2048, 1024);

        let idx = sb.pool.iter().position(|s| s.visible).unwrap();
        let mut banged = false;
        for _ in 0..3000 {
            sb.update(&clip, &mut nr, &w, &mut ex, &mut ev);
            if sb.bang_rect(idx).is_some() {
                banged = true;
                break;
            }
        }
        assert!(banged, "spikeball never completed an attack cycle");
        // The blast box is centered on the ball.
        let r = sb.bang_rect(idx).unwrap();
        assert_eq!(r.width(), sb.explo_dist as i32);
        // And it cleared by the next beat.
        sb.update(&clip, &mut nr, &w, &mut ex, &mut ev);
        assert!(sb.bang_rect(idx).is_none());
    }
}
