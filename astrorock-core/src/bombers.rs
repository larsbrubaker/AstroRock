//! # Razor Bombers — port of `bomber.cpp`
//!
//! Five-slot enemy with per-bomber 5-bomb pools. Types (bomber.cfg
//! "level:count,type"): 1 = 150 pts / bomb-dist 500 / 200 HP / fire
//! 1-in-50 / bomb dmg 50 HP 30 / back 0.9 fwd 1.2 / turn 0.1 / thrust
//! 95 / bomb life 90; 2 = 450/400/400/30/90/60/1.1/1.6/0.1/75/90
//! (rBomber2Pal); 3 = 600/400/600/20/500/90/1.3/1.9/0.2/65/50
//! (rBomber3Pal); 4 = 1600/450/1200/7/30/190/1.1/2.1/0.5/35/25
//! (rBomber3Pal + LocalRand flicker).
//!
//! Behavior: bombers back away from their facing by default
//! (BACKACCELERATION) and only surge forward while the `Thrusting`
//! counter runs — which is set when the bomber takes non-lethal damage
//! (`BomberP1`). With a live target they turn toward it (same integer
//! math as the HK) and roll 1-in-N to bomb when inside BOMBERBOMBDIST.
//! Targetless bombers roll FindTarget, and on failure wander (1-in-90
//! roll picks straight/left/right). Their bombs are destructible
//! (`DamageBomb`) and keep flying after the last bomber dies.
//!
//! The art's hotspot is shifted down 3px at load (`OffsetHotSpot(0,
//! BOMBEROFFSET)`) so bombs appear to drop from the bay.

use std::rc::Rc;

use crate::bombs::Bombs;
use crate::events::Events;
use crate::explosion::Explosions;
use crate::fixed_trig;
use crate::frame::Frame;
use crate::rand::Rand;
use crate::rect::Rect;
use crate::rocks::MAX_LEVELS;
use crate::sequence;
use crate::sprite::{Sprite, SpriteBlit};
use crate::virtual_frame::VirtualFrame;

const MAX_BOMBERS: usize = 5;
const MAX_PLAYERS: u32 = 8;
/// `BOMBERCOLLIDEDAMAGE`.
pub const BOMBER_COLLIDE_DAMAGE: u32 = 100;
const BOMBER_OFFSET: i32 = 3;
const FRICTION: f32 = 0.85;
pub const BOMBER_RADAR_COLOR: u8 = 135;

const BOMBER_CFG: &str = include_str!("../../assets/config/bomber.cfg");

struct TypeStats {
    score: u32,
    bomb_dist: f32,
    hp: u32,
    fire_rand: u32,
    bomb_damage: u32,
    bomb_hp: u32,
    back_accel: f32,
    front_accel: f32,
    turn_speed: f32,
    thrust_duration: u32,
    bomb_duration: u32,
}

fn type_stats(bomber_type: u32) -> TypeStats {
    match bomber_type {
        2 => TypeStats {
            score: 450,
            bomb_dist: 400.0,
            hp: 400,
            fire_rand: 30,
            bomb_damage: 90,
            bomb_hp: 60,
            back_accel: 1.1,
            front_accel: 1.6,
            turn_speed: 0.1,
            thrust_duration: 75,
            bomb_duration: 90,
        },
        3 => TypeStats {
            score: 600,
            bomb_dist: 400.0,
            hp: 600,
            fire_rand: 20,
            bomb_damage: 500,
            bomb_hp: 90,
            back_accel: 1.3,
            front_accel: 1.9,
            turn_speed: 0.2,
            thrust_duration: 65,
            bomb_duration: 50,
        },
        4 => TypeStats {
            score: 1600,
            bomb_dist: 450.0,
            hp: 1200,
            fire_rand: 7,
            bomb_damage: 30,
            bomb_hp: 190,
            back_accel: 1.1,
            front_accel: 2.1,
            turn_speed: 0.5,
            thrust_duration: 35,
            bomb_duration: 25,
        },
        _ => TypeStats {
            score: 150,
            bomb_dist: 500.0,
            hp: 200,
            fire_rand: 50,
            bomb_damage: 50,
            bomb_hp: 30,
            back_accel: 0.9,
            front_accel: 1.2,
            turn_speed: 0.1,
            thrust_duration: 95,
            bomb_duration: 90,
        },
    }
}

pub struct Bombers {
    pool: Vec<Sprite>,
    pub bombs: Vec<Bombs>,
    targets: [bool; MAX_BOMBERS],
    thrusting: [u32; MAX_BOMBERS],
    pub num_bombers: u32,
    pub max_bombers: u32,
    cur_type: u32,
    score_per_kill: u32,
    bomb_dist: f32,
    hp: u32,
    fire_rand: u32,
    front_accel: f32,
    back_accel: f32,
    turn_speed: f32,
    thrust_duration: u32,
    level_num: [u32; MAX_LEVELS],
    level_type: [u32; MAX_LEVELS],
    remap2: Rc<[u8; 256]>,
    remap3: Rc<[u8; 256]>,
}

impl Bombers {
    /// `BombersInit` — pools + the 3px hotspot drop on the art.
    pub fn new() -> Self {
        let mut level_num = [0u32; MAX_LEVELS];
        let mut level_type = [0u32; MAX_LEVELS];
        for (i, line) in BOMBER_CFG.lines().take(MAX_LEVELS).enumerate() {
            let mut parts = line.split(&[':', ','][..]);
            let _level = parts.next();
            level_num[i] = parts
                .next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0)
                .min(MAX_BOMBERS as u32);
            level_type[i] = parts
                .next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0)
                .min(4);
        }

        // OffsetHotSpot(0, BOMBEROFFSET) — clone the shared sequence
        // and shift every frame's anchor down.
        let base = sequence::bomber();
        let mut shifted_frames: Vec<Frame> = Vec::with_capacity(base.frames.len());
        for f in &base.frames {
            let mut nf = Frame::from_bits(f.width, f.height, f.bits.clone());
            nf.hot_x = f.hot_x;
            nf.hot_y = f.hot_y + BOMBER_OFFSET;
            shifted_frames.push(nf);
        }
        let seq = Rc::new(crate::sequence::FrameSequence {
            frames: shifted_frames,
            num_frames: base.num_frames,
            num_rotations: base.num_rotations,
            original_bounds: base.original_bounds,
        });

        let bomb_seq = sequence::bomb();
        let pool: Vec<Sprite> = (0..MAX_BOMBERS)
            .map(|_| {
                let mut s = Sprite::new();
                s.set_sequence(seq.clone());
                s.visible = false;
                s
            })
            .collect();
        let bombs = (0..MAX_BOMBERS)
            .map(|_| Bombs::new(bomb_seq.clone(), 5))
            .collect();

        Self {
            pool,
            bombs,
            targets: [false; MAX_BOMBERS],
            thrusting: [0; MAX_BOMBERS],
            num_bombers: 0,
            max_bombers: 0,
            cur_type: 0,
            score_per_kill: 150,
            bomb_dist: 500.0,
            hp: 200,
            fire_rand: 50,
            front_accel: 1.2,
            back_accel: 0.9,
            turn_speed: 0.1,
            thrust_duration: 95,
            level_num,
            level_type,
            remap2: Rc::new(crate::assets::remap_table(crate::assets::BOMBER2_PAL)),
            remap3: Rc::new(crate::assets::remap_table(crate::assets::BOMBER3_PAL)),
        }
    }

    /// `SetBomberType`.
    fn set_type(&mut self, bomber_type: u32) {
        self.cur_type = bomber_type;
        let stats = type_stats(bomber_type);
        self.score_per_kill = stats.score;
        self.bomb_dist = stats.bomb_dist;
        self.hp = stats.hp;
        self.fire_rand = stats.fire_rand;
        self.back_accel = stats.back_accel;
        self.front_accel = stats.front_accel;
        self.turn_speed = stats.turn_speed;
        self.thrust_duration = stats.thrust_duration;
        let blit = match bomber_type {
            2 => SpriteBlit::RemapSource(self.remap2.clone()),
            3 | 4 => SpriteBlit::RemapSource(self.remap3.clone()),
            _ => SpriteBlit::Trans,
        };
        for i in 0..MAX_BOMBERS {
            self.pool[i].blit = blit.clone();
            self.bombs[i].config(stats.bomb_hp, stats.bomb_damage, stats.bomb_duration);
        }
    }

    /// `BombersReset(level)`.
    pub fn reset(&mut self, level: usize, net_rand: &mut Rand) {
        self.num_bombers = 0;
        let (max, ty) = if level < MAX_LEVELS {
            (self.level_num[level], self.level_type[level])
        } else {
            (MAX_BOMBERS as u32, 1)
        };
        self.max_bombers = max;
        self.set_type(ty);

        for i in 0..MAX_BOMBERS {
            let s = &mut self.pool[i];
            s.reset();
            if self.num_bombers < self.max_bombers {
                self.num_bombers += 1;
                let frames = s.sequence().expect("seq").num_frames;
                s.cur_frame = net_rand.rand(frames) as f32;
                s.hp = self.hp;
                s.visible = true;
                s.x_pos = net_rand.rand(2048) as f32;
                s.y_pos = net_rand.rand(1024) as f32;
            } else {
                s.visible = false;
            }
        }
        for i in 0..MAX_BOMBERS {
            self.targets[i] = false;
            self.thrusting[i] = 0;
            self.bombs[i].reset();
        }
    }

    /// `BombersUpdate`.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        clip: &Rect,
        net_rand: &mut Rand,
        world: &VirtualFrame,
        ship: &Sprite,
        explosions: &mut Explosions,
        events: &mut Events,
    ) {
        if self.max_bombers == 0 {
            return;
        }
        for i in 0..MAX_BOMBERS {
            if self.num_bombers != 0 && self.pool[i].visible {
                let cur_frame = self.pool[i].cur_frame as i32;
                let num_frames = self.pool[i].sequence().expect("seq").num_frames as i32;

                if self.targets[i] {
                    if ship.visible {
                        let s = &self.pool[i];
                        let angle = world.find_angle(
                            (s.x_pos as i32, s.y_pos as i32),
                            (ship.x_pos as i32, ship.y_pos as i32),
                        ) as i32;

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

                        if net_rand.rand(self.fire_rand) == 0 {
                            let s = &self.pool[i];
                            let dist = world.find_dist(
                                (s.x_pos as i32, s.y_pos as i32),
                                (ship.x_pos as i32, ship.y_pos as i32),
                            );
                            if dist < self.bomb_dist {
                                let who = &self.pool[i];
                                let mut shooter = Sprite::new();
                                shooter.x_pos = who.x_pos;
                                shooter.y_pos = who.y_pos;
                                shooter.x_delta = who.x_delta;
                                shooter.y_delta = who.y_delta;
                                shooter.cur_frame = who.cur_frame;
                                self.bombs[i].fire(&shooter, events);
                            }
                        }
                    } else {
                        // Target's ship gone — find a new one.
                        self.targets[i] = net_rand.rand(MAX_PLAYERS) == 0 && ship.visible;
                    }
                } else {
                    self.targets[i] = net_rand.rand(MAX_PLAYERS) == 0 && ship.visible;
                    if !self.targets[i] && net_rand.rand(90) == 0 {
                        // Wander: straight, left, or right.
                        let s = &mut self.pool[i];
                        match net_rand.rand(3) {
                            0 => s.frame_advance = 0.0,
                            1 => s.frame_advance = self.turn_speed,
                            _ => s.frame_advance = -self.turn_speed,
                        }
                    }
                }

                let facing = ((cur_frame * 360) / num_frames) as u32;
                let s = &mut self.pool[i];
                let (x_delta, y_delta) = if self.thrusting[i] != 0 {
                    self.thrusting[i] -= 1;
                    (
                        s.x_delta + fixed_trig::sin_d(facing) * self.front_accel,
                        s.y_delta - fixed_trig::cos_d(facing) * self.front_accel,
                    )
                } else {
                    (
                        s.x_delta - fixed_trig::sin_d(facing) * self.back_accel,
                        s.y_delta + fixed_trig::cos_d(facing) * self.back_accel,
                    )
                };
                s.x_delta = FRICTION * x_delta;
                s.y_delta = FRICTION * y_delta;
                let _ = s.update(clip, net_rand);
            }
            // Bombs keep flying after the last bomber dies.
            self.bombs[i].update(clip, net_rand, world, explosions, events);
        }
    }

    /// `BombersDraw` (+ type-4 LocalRand flicker).
    pub fn draw(&mut self, world: &VirtualFrame, screen: &mut Frame, local_rand: &mut Rand) {
        if self.cur_type == 4 {
            for i in 0..MAX_BOMBERS {
                self.pool[i].blit = match local_rand.rand(3) {
                    0 => SpriteBlit::Trans,
                    1 => SpriteBlit::RemapSource(self.remap2.clone()),
                    _ => SpriteBlit::RemapSource(self.remap3.clone()),
                };
            }
        }
        if self.num_bombers != 0 {
            for s in &self.pool {
                s.draw(world, screen);
            }
        }
        for bombs in &self.bombs {
            bombs.draw(world, screen);
        }
    }

    /// `BombersCheck`.
    pub fn check(&self) -> f32 {
        let mut sum = 0.0f32;
        if self.num_bombers != 0 {
            for s in &self.pool {
                sum += s.check(false);
            }
        }
        for bombs in &self.bombs {
            sum += bombs.check();
        }
        sum + self.num_bombers as f32
    }

    /// `BomberP1` — non-lethal hits trigger the forward-thrust lunge.
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
            self.num_bombers -= 1;
            explosions.explo_sprite(&mut self.pool[index], world, events);
            self.score_per_kill
        } else {
            sprite.hp -= damage;
            self.thrusting[index] = self.thrust_duration;
            explosions.play_shot_hit_at(sprite.x_pos, sprite.y_pos, world, events);
            0
        }
    }

    pub fn pool(&self) -> &[Sprite] {
        &self.pool
    }

    pub fn active(&self) -> bool {
        self.num_bombers != 0
    }

    pub fn engaged(&self) -> bool {
        self.max_bombers != 0
    }
}

impl Default for Bombers {
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

    fn first_bomber_level() -> usize {
        let b = Bombers::new();
        (0..MAX_LEVELS).find(|&i| b.level_num[i] > 0).unwrap()
    }

    #[test]
    fn cfg_has_bomber_levels_and_art_hotspot_shifted() {
        let b = Bombers::new();
        assert!(b.level_num.iter().any(|&n| n > 0));
        // The pool art's anchor sits 3px below the shared sheet's.
        let base = sequence::bomber();
        let shifted = b.pool[0].sequence().unwrap();
        assert_eq!(shifted.frames[0].hot_y, base.frames[0].hot_y + 3);
    }

    #[test]
    fn damaged_bomber_lunges_forward() {
        let mut b = Bombers::new();
        let mut nr = Rand::new();
        let w = world();
        let mut ex = Explosions::new();
        let mut ev = Events::new();
        b.reset(first_bomber_level(), &mut nr);

        let idx = b.pool.iter().position(|s| s.visible).unwrap();
        assert_eq!(b.thrusting[idx], 0);
        let score = b.damage(idx, 10, &w, &mut ex, &mut ev);
        assert_eq!(score, 0);
        assert_eq!(b.thrusting[idx], b.thrust_duration);

        // A lethal hit scores and hides.
        let score = b.damage(idx, 99999, &w, &mut ex, &mut ev);
        assert!(score > 0);
        assert!(!b.pool[idx].visible);
    }

    #[test]
    fn bomber_bombs_at_close_target() {
        let mut b = Bombers::new();
        let mut nr = Rand::new();
        let w = world();
        let mut ex = Explosions::new();
        let mut ev = Events::new();
        b.reset(first_bomber_level(), &mut nr);

        let mut ship = Sprite::new();
        ship.visible = true;
        let clip = Rect::new(0, 0, 2048, 1024);
        let mut bombed = false;
        for _ in 0..900 {
            ship.x_pos = b.pool[0].x_pos + 60.0;
            ship.y_pos = b.pool[0].y_pos;
            b.update(&clip, &mut nr, &w, &ship, &mut ex, &mut ev);
            if b.bombs[0].iter().any(|s| s.visible) {
                bombed = true;
                break;
            }
        }
        assert!(bombed, "bomber never dropped a bomb on an adjacent target");
    }
}
