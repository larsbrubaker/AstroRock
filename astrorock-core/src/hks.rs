//! # Hunter-Killers — port of `hk.cpp`
//!
//! Five-slot enemy with per-HK 5-shot pools. Types (hks.cfg
//! "level:count,type"): 1 = 200 pts / fire 1-in-60 / accel 1.1 / turn
//! 0.15 / 200 HP / shot dmg 35; 2 = 400/10/1.2/0.25/300/5 (rHk2Pal);
//! 3 = 800/30/2.0/0.45/400/30 (rHk3Pal); 4 = 1800/5/2.0/1.0/400/20
//! (rHk3Pal + LocalRand flicker).
//!
//! AI per beat (visible HKs): with a visible target — 1-in-N fire roll
//! (inside 200px the HK zeroes its deltas while firing so the bullet
//! gets no momentum), then turn the facing frame toward the target the
//! short way at ±TURNSPEED. Without a target — a FindTarget roll.
//! Then: inside 200px (or with NO target, where dist stays 0) the HK
//! strafes — 1-in-64 roll to flip orbit direction, facing ±90° — and
//! accelerates along its facing with friction 0.85.
//!
//! Shot pools update, draw, and collide even when every HK is dead
//! ("there could be shots left on the screen").

use crate::events::Events;
use crate::explosion::Explosions;
use crate::fixed_trig;
use crate::frame::Frame;
use crate::rand::Rand;
use crate::rect::Rect;
use crate::rocks::MAX_LEVELS;
use crate::sequence;
use crate::shots::{ShotTier, Shots};
use crate::sprite::{Sprite, SpriteBlit};
use crate::virtual_frame::VirtualFrame;
use std::rc::Rc;

const MAX_HKS: usize = 5;
const MAX_PLAYERS: u32 = 8;
/// `HKCOLLIDEDAMAGE`.
pub const HK_COLLIDE_DAMAGE: u32 = 50;
const HK_STRAFE_DIST: f32 = 200.0;
const FRICTION: f32 = 0.85;
pub const HK_RADAR_COLOR: u8 = 198;

const HKS_CFG: &str = include_str!("../../assets/config/hks.cfg");

struct TypeStats {
    score: u32,
    fire_rand: u32,
    acceleration: f32,
    turn_speed: f32,
    hp: u32,
    shot_damage: u32,
}

fn type_stats(hk_type: u32) -> TypeStats {
    match hk_type {
        2 => TypeStats {
            score: 400,
            fire_rand: 10,
            acceleration: 1.2,
            turn_speed: 0.25,
            hp: 300,
            shot_damage: 5,
        },
        3 => TypeStats {
            score: 800,
            fire_rand: 30,
            acceleration: 2.0,
            turn_speed: 0.45,
            hp: 400,
            shot_damage: 30,
        },
        4 => TypeStats {
            score: 1800,
            fire_rand: 5,
            acceleration: 2.0,
            turn_speed: 1.0,
            hp: 400,
            shot_damage: 20,
        },
        _ => TypeStats {
            score: 200,
            fire_rand: 60,
            acceleration: 1.1,
            turn_speed: 0.15,
            hp: 200,
            shot_damage: 35,
        },
    }
}

pub struct Hks {
    pool: Vec<Sprite>,
    pub shots: Vec<Shots>,
    targets: [bool; MAX_HKS],
    strafe_dir: [bool; MAX_HKS],
    pub num_hks: u32,
    pub max_hks: u32,
    cur_type: u32,
    score_per_kill: u32,
    fire_rand: u32,
    acceleration: f32,
    turn_speed: f32,
    hp: u32,
    pub shot_damage: u32,
    level_num: [u32; MAX_LEVELS],
    level_type: [u32; MAX_LEVELS],
    remap2: Rc<[u8; 256]>,
    remap3: Rc<[u8; 256]>,
}

impl Hks {
    /// `HKsInit`.
    pub fn new() -> Self {
        let mut level_num = [0u32; MAX_LEVELS];
        let mut level_type = [0u32; MAX_LEVELS];
        for (i, line) in HKS_CFG.lines().take(MAX_LEVELS).enumerate() {
            let mut parts = line.split(&[':', ','][..]);
            let _level = parts.next();
            level_num[i] = parts
                .next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0)
                .min(MAX_HKS as u32);
            level_type[i] = parts
                .next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0)
                .min(4);
        }

        let seq = sequence::hk();
        let shot_seq = sequence::shothk();
        let pool: Vec<Sprite> = (0..MAX_HKS)
            .map(|_| {
                let mut s = Sprite::new();
                s.set_sequence(seq.clone());
                s.visible = false;
                s
            })
            .collect();
        let shots = (0..MAX_HKS)
            .map(|_| Shots::new(shot_seq.clone(), ShotTier::Hk, 5))
            .collect();

        Self {
            pool,
            shots,
            targets: [false; MAX_HKS],
            strafe_dir: [true; MAX_HKS],
            num_hks: 0,
            max_hks: 0,
            cur_type: 0,
            score_per_kill: 200,
            fire_rand: 60,
            acceleration: 1.1,
            turn_speed: 0.15,
            hp: 200,
            shot_damage: 35,
            level_num,
            level_type,
            remap2: Rc::new(crate::assets::remap_table(crate::assets::HK2_PAL)),
            remap3: Rc::new(crate::assets::remap_table(crate::assets::HK3_PAL)),
        }
    }

    /// `SetHKType`.
    fn set_type(&mut self, hk_type: u32) {
        self.cur_type = hk_type;
        let stats = type_stats(hk_type);
        self.score_per_kill = stats.score;
        self.fire_rand = stats.fire_rand;
        self.acceleration = stats.acceleration;
        self.turn_speed = stats.turn_speed;
        self.hp = stats.hp;
        self.shot_damage = stats.shot_damage;
        let blit = match hk_type {
            2 => SpriteBlit::RemapSource(self.remap2.clone()),
            3 | 4 => SpriteBlit::RemapSource(self.remap3.clone()),
            _ => SpriteBlit::Trans,
        };
        for i in 0..MAX_HKS {
            self.pool[i].blit = blit.clone();
            self.shots[i].config(stats.shot_damage);
        }
    }

    /// `HKsReset(level)`.
    pub fn reset(&mut self, level: usize, net_rand: &mut Rand) {
        self.num_hks = 0;
        let (max, ty) = if level < MAX_LEVELS {
            (self.level_num[level], self.level_type[level])
        } else {
            (MAX_HKS as u32, 1)
        };
        self.max_hks = max;
        self.set_type(ty);

        for i in 0..MAX_HKS {
            let s = &mut self.pool[i];
            s.reset();
            if self.num_hks < self.max_hks {
                self.num_hks += 1;
                s.hp = self.hp;
                s.visible = true;
                s.x_pos = net_rand.rand(2048) as f32;
                s.y_pos = net_rand.rand(1024) as f32;
                let frames = s.sequence().expect("seq").num_frames;
                s.cur_frame = net_rand.rand(frames) as f32;
                s.x_delta = 0.0;
                s.y_delta = 0.0;
            } else {
                s.visible = false;
            }
        }
        for i in 0..MAX_HKS {
            self.shots[i].reset();
            self.targets[i] = false;
            self.strafe_dir[i] = true;
        }
    }

    /// `HKsUpdate` — see module docs for the per-beat AI shape.
    pub fn update(
        &mut self,
        clip: &Rect,
        net_rand: &mut Rand,
        world: &VirtualFrame,
        ship: &Sprite,
        events: &mut Events,
    ) {
        if self.max_hks == 0 {
            return;
        }
        for i in 0..MAX_HKS {
            if self.num_hks != 0 && self.pool[i].visible {
                let cur_frame = self.pool[i].cur_frame as i32;
                let num_frames = self.pool[i].sequence().expect("seq").num_frames as i32;
                let mut dist = 0.0f32;

                if self.targets[i] {
                    if ship.visible {
                        let s = &self.pool[i];
                        let angle = world.find_angle(
                            (s.x_pos as i32, s.y_pos as i32),
                            (ship.x_pos as i32, ship.y_pos as i32),
                        ) as i32;
                        dist = world.find_dist(
                            (s.x_pos as i32, s.y_pos as i32),
                            (ship.x_pos as i32, ship.y_pos as i32),
                        );

                        if net_rand.rand(self.fire_rand) == 0 {
                            if dist < HK_STRAFE_DIST {
                                // Zero deltas while firing so the shot
                                // carries no momentum.
                                let (sx, sy) = (self.pool[i].x_delta, self.pool[i].y_delta);
                                self.pool[i].x_delta = 0.0;
                                self.pool[i].y_delta = 0.0;
                                let who = &self.pool[i];
                                let mut shooter = Sprite::new();
                                shooter.x_pos = who.x_pos;
                                shooter.y_pos = who.y_pos;
                                shooter.x_delta = 0.0;
                                shooter.y_delta = 0.0;
                                shooter.cur_frame = who.cur_frame;
                                self.shots[i].fire(&shooter, false, events);
                                self.pool[i].x_delta = sx;
                                self.pool[i].y_delta = sy;
                            } else {
                                let who = &self.pool[i];
                                let mut shooter = Sprite::new();
                                shooter.x_pos = who.x_pos;
                                shooter.y_pos = who.y_pos;
                                shooter.x_delta = who.x_delta;
                                shooter.y_delta = who.y_delta;
                                shooter.cur_frame = who.cur_frame;
                                self.shots[i].fire(&shooter, false, events);
                            }
                        }

                        // Turn the facing frame toward the target the
                        // short way (exact int math).
                        let mut want_frame = angle * num_frames / 360 - num_frames / 4;
                        if want_frame < 0 {
                            want_frame += num_frames;
                        }
                        let s = &mut self.pool[i];
                        if want_frame != cur_frame {
                            if want_frame < cur_frame {
                                if (cur_frame - want_frame) < num_frames / 2 {
                                    s.frame_advance = -self.turn_speed;
                                } else {
                                    s.frame_advance = self.turn_speed;
                                }
                            } else if (want_frame - cur_frame) < num_frames / 2 {
                                s.frame_advance = self.turn_speed;
                            } else {
                                s.frame_advance = -self.turn_speed;
                            }
                        } else {
                            s.frame_advance = 0.0;
                        }
                    } else {
                        self.targets[i] = net_rand.rand(MAX_PLAYERS) == 0 && ship.visible;
                    }
                } else {
                    self.targets[i] = net_rand.rand(MAX_PLAYERS) == 0 && ship.visible;
                }

                // Strafe/orbit: inside 200px, or with no target (dist
                // stays 0), the facing swings ±90° with a 1-in-64 flip.
                let mut pointing = cur_frame * 360 / num_frames;
                if dist < HK_STRAFE_DIST {
                    if net_rand.rand(64) == 0 {
                        self.strafe_dir[i] = !self.strafe_dir[i];
                    }
                    if self.strafe_dir[i] {
                        pointing += 90;
                        if pointing >= 360 {
                            pointing -= 360;
                        }
                    } else {
                        pointing -= 90;
                        if pointing < 0 {
                            pointing += 360;
                        }
                    }
                }

                let s = &mut self.pool[i];
                let x_delta = s.x_delta + fixed_trig::sin_d(pointing as u32) * self.acceleration;
                let y_delta = s.y_delta - fixed_trig::cos_d(pointing as u32) * self.acceleration;
                s.x_delta = FRICTION * x_delta;
                s.y_delta = FRICTION * y_delta;
                let _ = s.update(clip, net_rand);
            }
            // Shots keep flying even after their HK dies.
            self.shots[i].update(clip, net_rand);
        }
    }

    /// `HKsDraw` (+ the type-4 LocalRand palette flicker).
    pub fn draw(&mut self, world: &VirtualFrame, screen: &mut Frame, local_rand: &mut Rand) {
        if self.cur_type == 4 {
            for i in 0..MAX_HKS {
                self.pool[i].blit = match local_rand.rand(3) {
                    0 => SpriteBlit::Trans,
                    1 => SpriteBlit::RemapSource(self.remap2.clone()),
                    _ => SpriteBlit::RemapSource(self.remap3.clone()),
                };
            }
        }
        if self.num_hks != 0 {
            for s in &self.pool {
                s.draw(world, screen);
            }
        }
        for shots in &self.shots {
            shots.draw(world, screen);
        }
    }

    /// `HKsCheck`.
    pub fn check(&self) -> f32 {
        let mut sum = 0.0f32;
        if self.num_hks != 0 {
            for s in &self.pool {
                sum += s.check(false);
            }
        }
        for shots in &self.shots {
            sum += shots.check();
        }
        sum + self.num_hks as f32
    }

    /// `HKP1`.
    pub fn damage(
        &mut self,
        index: usize,
        damage: u32,
        world: &VirtualFrame,
        explosions: &mut Explosions,
        events: &mut Events,
    ) -> u32 {
        let sprite = &mut self.pool[index];
        if damage >= sprite.hp {
            sprite.hp = 0;
            self.num_hks -= 1;
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
        self.num_hks != 0
    }

    pub fn engaged(&self) -> bool {
        self.max_hks != 0
    }
}

impl Default for Hks {
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

    fn first_hk_level() -> usize {
        let h = Hks::new();
        (0..MAX_LEVELS).find(|&i| h.level_num[i] > 0).unwrap()
    }

    #[test]
    fn cfg_has_hk_levels() {
        let h = Hks::new();
        assert!(h.level_num.iter().any(|&n| n > 0), "no HK levels found");
    }

    #[test]
    fn reset_spawns_and_type_configures_shots() {
        let mut h = Hks::new();
        let mut nr = Rand::new();
        let lvl = first_hk_level();
        h.reset(lvl, &mut nr);
        assert!(h.num_hks > 0);
        assert_eq!(
            h.pool.iter().filter(|s| s.visible).count(),
            h.num_hks as usize
        );
        assert_eq!(h.shots[0].damage, h.shot_damage);
    }

    #[test]
    fn hk_eventually_fires_at_close_target() {
        let mut h = Hks::new();
        let mut nr = Rand::new();
        let w = world();
        let mut ev = Events::new();
        h.reset(first_hk_level(), &mut nr);

        let mut ship = Sprite::new();
        ship.visible = true;
        let clip = Rect::new(0, 0, 2048, 1024);
        // Park the ship right next to HK 0 and run beats until a shot
        // appears (needs a FindTarget roll then a fire roll).
        let mut fired = false;
        for _ in 0..600 {
            ship.x_pos = h.pool[0].x_pos + 50.0;
            ship.y_pos = h.pool[0].y_pos;
            h.update(&clip, &mut nr, &w, &ship, &mut ev);
            if h.shots[0].any_on_screen() {
                fired = true;
                break;
            }
        }
        assert!(fired, "HK never fired at an adjacent target");
    }

    #[test]
    fn shots_outlive_their_hk() {
        let mut h = Hks::new();
        let mut nr = Rand::new();
        let w = world();
        let mut ex = Explosions::new();
        let mut ev = Events::new();
        h.reset(first_hk_level(), &mut nr);

        // Force a shot into flight, then kill the HK.
        let mut shooter = Sprite::new();
        shooter.x_pos = 100.0;
        shooter.y_pos = 100.0;
        shooter.cur_frame = 0.0;
        h.shots[0].fire(&shooter, false, &mut ev);
        let score = h.damage(0, 99999, &w, &mut ex, &mut ev);
        assert!(score > 0);
        assert!(!h.pool[0].visible);
        // The shot still updates and draws.
        let clip = Rect::new(0, 0, 2048, 1024);
        h.update(&clip, &mut nr, &w, &Sprite::new(), &mut ev);
        assert!(h.shots[0].any_on_screen());
    }
}
