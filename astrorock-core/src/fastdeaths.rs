//! # Fast Deaths — port of `fastdeth.cpp`
//!
//! Three-slot relentless hunter. All slots start INVISIBLE — Fast
//! Deaths only enter play through the spawn shimmer: each level sets
//! `SpawnNetRand` from `fastdeth.cfg` (0 = never), `AttackPause`
//! counts that many beats down first, then every beat rolls 1-in-N to
//! start a warp-in.
//!
//! AI: constant 1.6 acceleration along the facing with friction 0.9
//! and turn speed 0.5 — they never stop coming. A 1-in-300 roll
//! re-targets even mid-chase. Shield contact is nerfed: damage in
//! [600, 2001) becomes 10 (bombs still vaporize them). They don't
//! count toward level clear (`FastDeathsDraw` returns 0).

use crate::events::Events;
use crate::explosion::Explosions;
use crate::fixed_trig;
use crate::frame::Frame;
use crate::rand::Rand;
use crate::rect::Rect;
use crate::rocks::MAX_LEVELS;
use crate::sequence;
use crate::spawnfx::{SpawnFx, SpawnKind};
use crate::sprite::Sprite;
use crate::virtual_frame::VirtualFrame;

const MAX_FAST_DEATHS: usize = 3;
const MAX_PLAYERS: u32 = 8;
/// `FASTDEATHCOLLIDEDAMAGE`.
pub const FAST_DEATH_COLLIDE_DAMAGE: u32 = 20;
const FAST_DEATH_SCORE: u32 = 350;
const FAST_DEATH_HP: u32 = 600;
const ACCELERATION: f32 = 1.6;
const TURN_SPEED: f32 = 0.5;
const FRICTION: f32 = 0.90;
pub const FAST_DEATH_RADAR_COLOR: u8 = 183;

const FASTDETH_CFG: &str = include_str!("../../assets/config/fastdeth.cfg");

pub struct FastDeaths {
    pool: Vec<Sprite>,
    targets: [bool; MAX_FAST_DEATHS],
    pub num_fast_deaths: u32,
    max_fast_deaths: u32,
    spawn_net_rand: i32,
    attack_pause: u32,
    level_spawn_rand: [u32; MAX_LEVELS],
}

impl FastDeaths {
    /// `FastDeathsInit`.
    pub fn new() -> Self {
        let mut level_spawn_rand = [0u32; MAX_LEVELS];
        for (i, line) in FASTDETH_CFG.lines().take(MAX_LEVELS).enumerate() {
            let mut parts = line.split(&[':', ','][..]);
            let _level = parts.next();
            level_spawn_rand[i] = parts
                .next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
        }

        let seq = sequence::fastdeth();
        let pool = (0..MAX_FAST_DEATHS)
            .map(|_| {
                let mut s = Sprite::new();
                s.set_sequence(seq.clone());
                s.visible = false;
                s
            })
            .collect();

        Self {
            pool,
            targets: [false; MAX_FAST_DEATHS],
            num_fast_deaths: 0,
            max_fast_deaths: 0,
            spawn_net_rand: 0,
            attack_pause: 0,
            level_spawn_rand,
        }
    }

    /// `FastDeathsReset(level)` — everyone hides; the spawn clock arms.
    pub fn reset(&mut self, level: usize, net_rand: &mut Rand) {
        self.num_fast_deaths = 0;
        self.max_fast_deaths = MAX_FAST_DEATHS as u32;

        let more = level as i32 - MAX_LEVELS as i32;
        let idx = level.min(MAX_LEVELS - 1);
        self.spawn_net_rand = if more <= MAX_LEVELS as i32 {
            self.level_spawn_rand[idx] as i32
        } else {
            (self.level_spawn_rand[idx] as i32 - more * 60).max(300)
        };

        for i in 0..MAX_FAST_DEATHS {
            let s = &mut self.pool[i];
            s.reset();
            // SetVisAndMove primes position/frame with NetRand draws
            // for the first `max` slots but hides EVERYONE.
            if (i as u32) < self.max_fast_deaths {
                s.hp = FAST_DEATH_HP;
                let frames = s.sequence().expect("seq").num_frames;
                s.cur_frame = net_rand.rand(frames) as f32;
                s.x_pos = net_rand.rand(2048) as f32;
                s.y_pos = net_rand.rand(1024) as f32;
            }
            s.visible = false;
            self.targets[i] = false;
        }

        self.attack_pause = self.spawn_net_rand.max(0) as u32;
    }

    /// `FastDeathsSpawnOne` — the shimmer finished; materialize.
    pub fn spawn_one(&mut self, x: f32, y: f32, cur_frame: f32) {
        if self.max_fast_deaths == 0 {
            return;
        }
        for i in 0..self.max_fast_deaths as usize {
            if !self.pool[i].visible {
                self.num_fast_deaths += 1;
                let s = &mut self.pool[i];
                s.hp = FAST_DEATH_HP;
                s.visible = true;
                s.x_pos = x;
                s.y_pos = y;
                s.x_delta = 0.0;
                s.y_delta = 0.0;
                s.cur_frame = cur_frame;
                s.frame_advance = 0.0;
                self.targets[i] = false;
                return;
            }
        }
    }

    /// `FastDeathsUpdate` — chase AI plus the spawn clock.
    pub fn update(
        &mut self,
        clip: &Rect,
        net_rand: &mut Rand,
        world: &VirtualFrame,
        ship: &Sprite,
        spawnfx: &mut SpawnFx,
    ) {
        for i in 0..MAX_FAST_DEATHS {
            if !self.pool[i].visible {
                continue;
            }
            let cur_frame = self.pool[i].cur_frame as i32;
            let num_frames = self.pool[i].sequence().expect("seq").num_frames as i32;

            // Every ~10 seconds pick a fresh target even mid-chase.
            if net_rand.rand(300) == 0 {
                self.targets[i] = net_rand.rand(MAX_PLAYERS) == 0 && ship.visible;
            }

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
                                s.frame_advance = -TURN_SPEED;
                            } else {
                                s.frame_advance = TURN_SPEED;
                            }
                        } else if (want_frame - cur_frame) < num_frames / 2 {
                            s.frame_advance = TURN_SPEED;
                        } else {
                            s.frame_advance = -TURN_SPEED;
                        }
                    } else {
                        s.frame_advance = 0.0;
                    }
                } else {
                    self.targets[i] = net_rand.rand(MAX_PLAYERS) == 0 && ship.visible;
                }
            } else {
                self.targets[i] = net_rand.rand(MAX_PLAYERS) == 0 && ship.visible;
                if !self.targets[i] && net_rand.rand(90) == 0 {
                    let s = &mut self.pool[i];
                    match net_rand.rand(3) {
                        0 => s.frame_advance = 0.0,
                        1 => s.frame_advance = TURN_SPEED,
                        _ => s.frame_advance = -TURN_SPEED,
                    }
                }
            }

            let facing = ((cur_frame * 360) / num_frames) as u32;
            let s = &mut self.pool[i];
            let y_delta = s.y_delta - fixed_trig::cos_d(facing) * ACCELERATION;
            let x_delta = s.x_delta + fixed_trig::sin_d(facing) * ACCELERATION;
            s.x_delta = FRICTION * x_delta;
            s.y_delta = FRICTION * y_delta;
            let _ = s.update(clip, net_rand);
        }

        // The spawn clock.
        if self.attack_pause != 0 {
            self.attack_pause -= 1;
        } else if self.spawn_net_rand > 0 && net_rand.rand(self.spawn_net_rand as u32) == 0 {
            spawnfx.spawn_obj(
                SpawnKind::FastDeath,
                self.num_fast_deaths,
                self.max_fast_deaths,
                net_rand,
            );
        }
    }

    pub fn draw(&self, world: &VirtualFrame, screen: &mut Frame) {
        for s in &self.pool {
            s.draw(world, screen);
        }
    }

    /// `FastDeathsCheck`.
    pub fn check(&self) -> f32 {
        let mut sum = 0.0f32;
        for s in &self.pool {
            sum += s.check(false);
        }
        sum + self.num_fast_deaths as f32
    }

    /// `FastDeathP1` — the shield-damage nerf, then kill/chip.
    pub fn damage(
        &mut self,
        index: usize,
        mut damage: u32,
        world: &VirtualFrame,
        explosions: &mut Explosions,
        events: &mut Events,
    ) -> u32 {
        if (600..2001).contains(&damage) {
            damage = 10;
        }
        let sprite = &mut self.pool[index];
        if damage >= sprite.hp {
            sprite.hp = 0;
            self.num_fast_deaths -= 1;
            explosions.explo_sprite(&mut self.pool[index], world, events);
            FAST_DEATH_SCORE
        } else {
            sprite.hp -= damage;
            explosions.play_shot_hit_at(sprite.x_pos, sprite.y_pos, world, events);
            0
        }
    }

    pub fn pool(&self) -> &[Sprite] {
        &self.pool
    }
}

impl Default for FastDeaths {
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

    fn spawning_level() -> usize {
        let f = FastDeaths::new();
        (0..MAX_LEVELS)
            .find(|&i| f.level_spawn_rand[i] > 0)
            .unwrap()
    }

    #[test]
    fn early_levels_never_spawn() {
        let f = FastDeaths::new();
        // fastdeth.cfg: levels 1-5 are 0 (never).
        assert_eq!(f.level_spawn_rand[0], 0);
        assert!(f.level_spawn_rand[5] > 0, "level 6 should arm the clock");
    }

    #[test]
    fn all_start_hidden_and_spawn_one_materializes() {
        let mut f = FastDeaths::new();
        let mut nr = Rand::new();
        f.reset(spawning_level(), &mut nr);
        assert_eq!(f.num_fast_deaths, 0);
        assert!(f.pool.iter().all(|s| !s.visible));

        f.spawn_one(700.0, 300.0, 5.0);
        assert_eq!(f.num_fast_deaths, 1);
        assert!(f.pool[0].visible);
        assert_eq!(f.pool[0].hp, FAST_DEATH_HP);
    }

    #[test]
    fn shield_damage_is_nerfed_but_bombs_kill() {
        let mut f = FastDeaths::new();
        let mut nr = Rand::new();
        let w = world();
        let mut ex = Explosions::new();
        let mut ev = Events::new();
        f.reset(spawning_level(), &mut nr);
        f.spawn_one(700.0, 300.0, 0.0);

        // Shield contact (1000) is treated as 10.
        f.damage(0, 1000, &w, &mut ex, &mut ev);
        assert_eq!(f.pool[0].hp, FAST_DEATH_HP - 10);
        // Bombs (0xFFFF) are outside the nerf window and kill.
        let score = f.damage(0, 0xFFFF, &w, &mut ex, &mut ev);
        assert_eq!(score, FAST_DEATH_SCORE);
        assert!(!f.pool[0].visible);
    }

    #[test]
    fn chases_once_targeted() {
        let mut f = FastDeaths::new();
        let mut nr = Rand::new();
        let w = world();
        let mut fx = SpawnFx::new();
        f.reset(spawning_level(), &mut nr);
        f.spawn_one(700.0, 300.0, 0.0);

        let mut ship = Sprite::new();
        ship.visible = true;
        ship.x_pos = 400.0;
        ship.y_pos = 300.0;
        let clip = Rect::new(0, 0, 2048, 1024);
        // Fast deaths accelerate constantly; once targeted they close
        // distance on a stationary ship.
        let mut closed_in = false;
        for _ in 0..900 {
            f.update(&clip, &mut nr, &w, &ship, &mut fx);
            let d = w.find_dist(
                (f.pool[0].x_pos as i32, f.pool[0].y_pos as i32),
                (ship.x_pos as i32, ship.y_pos as i32),
            );
            if d < 60.0 {
                closed_in = true;
                break;
            }
        }
        assert!(closed_in, "fast death never closed on the ship");
    }
}
