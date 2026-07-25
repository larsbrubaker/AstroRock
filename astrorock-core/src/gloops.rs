//! # Gloops — port of `gloops.cpp`
//!
//! Five-slot homing enemy. Levels set count + type (`gloops.cfg`,
//! "level:count,type"): type 1 = 300 HP / speed 2 / 100 pts, type 2 =
//! 600/3/300 (recolored via `rGloop2Pal`), type 3 = 1200/4/500
//! (`rGloop3Pal`), type 4 = 6000/8/900 and flickers between all three
//! palettes each draw using LocalRand (visual-only RNG).
//!
//! Homing: a gloop with a live target steers straight at it through the
//! wrap-aware `FindAngle`, at `-cos/-sin * speed` (the original's sign
//! convention). Targetless gloops call `FindTarget` every beat — which
//! draws `NetRand(MAXPLAYERS)` each time and only acquires when the
//! rolled slot holds a visible player (out-of-range rolls resolve to a
//! NULL target via `GetSprite`'s release-mode fallthrough).

use std::rc::Rc;

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

const MAX_GLOOPS: usize = 5;
/// `MAXPLAYERS` — FindTarget rolls over all 8 roster slots.
const MAX_PLAYERS: u32 = 8;
/// Damage a gloop deals on contact.
pub const GLOOP_COLLIDE_DAMAGE: u32 = 50;
const GLOOP_RADAR_COLOR: u8 = 104;

const GLOOPS_CFG: &str = include_str!("../../assets/config/gloops.cfg");

struct TypeStats {
    hp: u32,
    speed: f32,
    score: u32,
}

fn type_stats(gloop_type: u32) -> TypeStats {
    match gloop_type {
        2 => TypeStats {
            hp: 600,
            speed: 3.0,
            score: 300,
        },
        3 => TypeStats {
            hp: 1200,
            speed: 4.0,
            score: 500,
        },
        4 => TypeStats {
            hp: 6000,
            speed: 8.0,
            score: 900,
        },
        _ => TypeStats {
            hp: 300,
            speed: 2.0,
            score: 100,
        },
    }
}

pub struct Gloops {
    pool: Vec<Sprite>,
    /// `pTarget[i]` — in single-player either "tracking the local
    /// ship" or none.
    targets: [bool; MAX_GLOOPS],
    pub num_gloops: u32,
    max_gloops: u32,
    cur_type: u32,
    speed: f32,
    hp: u32,
    score_add_per_kill: u32,
    level_num: [u32; MAX_LEVELS],
    level_type: [u32; MAX_LEVELS],
    remap2: Rc<[u8; 256]>,
    remap3: Rc<[u8; 256]>,
}

impl Gloops {
    /// `GloopsInit`.
    pub fn new() -> Self {
        let mut level_num = [0u32; MAX_LEVELS];
        let mut level_type = [0u32; MAX_LEVELS];
        for (i, line) in GLOOPS_CFG.lines().take(MAX_LEVELS).enumerate() {
            // "01:02,01," — level : count , type
            let mut parts = line.split(&[':', ','][..]);
            let _level = parts.next();
            let count: u32 = parts
                .next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            let ty: u32 = parts
                .next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            level_num[i] = count.min(MAX_GLOOPS as u32);
            level_type[i] = ty.min(4);
        }

        let seq = sequence::gloop();
        let pool = (0..MAX_GLOOPS)
            .map(|_| {
                let mut s = Sprite::new();
                s.set_sequence(seq.clone());
                s.visible = false;
                s
            })
            .collect();

        let remap2: Rc<[u8; 256]> = Rc::new(crate::assets::remap_table(crate::assets::GLOOP2_PAL));
        let remap3: Rc<[u8; 256]> = Rc::new(crate::assets::remap_table(crate::assets::GLOOP3_PAL));

        Self {
            pool,
            targets: [false; MAX_GLOOPS],
            num_gloops: 0,
            max_gloops: 0,
            cur_type: 0,
            speed: 2.0,
            hp: 300,
            score_add_per_kill: 100,
            level_num,
            level_type,
            remap2,
            remap3,
        }
    }

    /// `SetGloopType` — stats + blit for every pool slot.
    fn set_type(&mut self, gloop_type: u32) {
        self.cur_type = gloop_type;
        let stats = type_stats(gloop_type);
        self.hp = stats.hp;
        self.speed = stats.speed;
        self.score_add_per_kill = stats.score;
        let blit = match gloop_type {
            2 => SpriteBlit::RemapSource(self.remap2.clone()),
            3 => SpriteBlit::RemapSource(self.remap3.clone()),
            _ => SpriteBlit::Trans,
        };
        for s in &mut self.pool {
            s.blit = blit.clone();
        }
    }

    /// `GloopsReset(level)`.
    pub fn reset(&mut self, level: usize, net_rand: &mut Rand) {
        self.num_gloops = 0;
        let (max, ty) = if level < MAX_LEVELS {
            (self.level_num[level], self.level_type[level])
        } else {
            (MAX_GLOOPS as u32, 1)
        };
        self.max_gloops = max;
        self.set_type(ty);

        // SetVisAndMove in list order — visible slots draw RNG, hidden
        // slots draw none (unlike rocks, which always draw one).
        for i in 0..MAX_GLOOPS {
            let s = &mut self.pool[i];
            s.reset();
            if self.num_gloops < self.max_gloops {
                self.num_gloops += 1;
                s.hp = self.hp;
                s.visible = true;
                let frames = s.sequence().expect("seq").num_frames;
                s.cur_frame = net_rand.rand(frames) as f32;
                s.x_pos = net_rand.rand(2048) as f32;
                s.y_pos = net_rand.rand(1024) as f32;
                s.frame_advance = if net_rand.rand(2) != 0 { 1.0 } else { -1.0 };
            } else {
                s.visible = false;
            }
            self.targets[i] = false;
        }
    }

    /// `GloopsUpdate` — retarget or steer, then move. Only visible
    /// gloops update at all.
    pub fn update(
        &mut self,
        clip: &Rect,
        net_rand: &mut Rand,
        world: &VirtualFrame,
        ship: &Sprite,
    ) {
        if self.num_gloops == 0 {
            return;
        }
        for i in 0..MAX_GLOOPS {
            if !self.pool[i].visible {
                continue;
            }
            if !self.targets[i] || !ship.visible {
                // FindTarget: one NetRand(MAXPLAYERS) draw per attempt;
                // only slot 0 (the local ship) is ever visible here.
                let roll = net_rand.rand(MAX_PLAYERS);
                self.targets[i] = roll == 0 && ship.visible;
            } else {
                let s = &self.pool[i];
                let angle = world.find_angle(
                    (s.x_pos as i32, s.y_pos as i32),
                    (ship.x_pos as i32, ship.y_pos as i32),
                ) as i32 as u32;
                let s = &mut self.pool[i];
                s.x_delta = -(fixed_trig::cos_d(angle) * self.speed);
                s.y_delta = -(fixed_trig::sin_d(angle) * self.speed);
            }
            let _ = self.pool[i].update(clip, net_rand);
        }
    }

    /// `GloopsDraw` — type 4 reshuffles palettes every draw with
    /// LocalRand (visual-only; does not touch NetRand).
    pub fn draw(&mut self, world: &VirtualFrame, screen: &mut Frame, local_rand: &mut Rand) {
        if self.cur_type == 4 {
            for i in 0..MAX_GLOOPS {
                self.pool[i].blit = match local_rand.rand(3) {
                    0 => SpriteBlit::Trans,
                    1 => SpriteBlit::RemapSource(self.remap2.clone()),
                    _ => SpriteBlit::RemapSource(self.remap3.clone()),
                };
            }
        }
        if self.num_gloops != 0 {
            for s in &self.pool {
                s.draw(world, screen);
            }
        }
    }

    /// Radar feed (`RadarDrawOn` with the shared gloop color).
    pub fn radar_sprites(&self) -> impl Iterator<Item = (&Sprite, u8)> {
        self.pool
            .iter()
            .map(|s| (s, GLOOP_RADAR_COLOR))
            .filter(|_| self.num_gloops != 0)
    }

    /// `GloopsCheck`.
    pub fn check(&self) -> f32 {
        let mut sum = 0.0f32;
        if self.num_gloops != 0 {
            for s in &self.pool {
                sum += s.check(false);
            }
        }
        sum + self.num_gloops as f32
    }

    /// `GloopP1` — damage/kill one gloop. Returns score added.
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
            self.num_gloops -= 1;
            explosions.explo_sprite(&mut self.pool[index], world, events);
            self.score_add_per_kill
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
        self.num_gloops != 0
    }
}

impl Default for Gloops {
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

    #[test]
    fn cfg_parses_count_and_type() {
        let g = Gloops::new();
        // gloops.cfg: 01:02,01  02:04,01  03:00,01  … 11:02,02.
        assert_eq!(g.level_num[0], 2);
        assert_eq!(g.level_type[0], 1);
        assert_eq!(g.level_num[2], 0);
        assert_eq!(g.level_num[10], 2);
        assert_eq!(g.level_type[10], 2);
    }

    #[test]
    fn reset_spawns_level_count_with_type_stats() {
        let mut g = Gloops::new();
        let mut nr = Rand::new();
        g.reset(0, &mut nr); // level 1: 2 gloops, type 1
        assert_eq!(g.num_gloops, 2);
        assert_eq!(g.pool.iter().filter(|s| s.visible).count(), 2);
        assert_eq!(g.pool[0].hp, 300);

        g.reset(10, &mut nr); // level 11: type 2
        assert_eq!(g.pool[0].hp, 600);
        assert!(matches!(g.pool[0].blit, SpriteBlit::RemapSource(_)));
    }

    #[test]
    fn homing_steers_toward_visible_ship_after_acquiring() {
        let mut g = Gloops::new();
        let mut nr = Rand::new();
        g.reset(0, &mut nr);
        let w = world();
        let mut ship = Sprite::new();
        ship.x_pos = 100.0;
        ship.y_pos = 500.0;
        ship.visible = true;

        // Park gloop 0 due right of the ship; run until it acquires
        // (FindTarget needs a 1-in-8 roll) and steers.
        g.pool[0].x_pos = 400.0;
        g.pool[0].y_pos = 500.0;
        let clip = Rect::new(0, 0, 2048, 1024);
        let mut steered = false;
        for _ in 0..200 {
            let (px, py) = (g.pool[0].x_pos, g.pool[0].y_pos);
            g.update(&clip, &mut nr, &w, &ship);
            if g.targets[0] && g.pool[0].x_pos < px && (g.pool[0].y_pos - py).abs() < 3.0 {
                steered = true;
                break;
            }
        }
        assert!(steered, "gloop never acquired and chased the ship");
    }

    #[test]
    fn damage_kills_and_scores() {
        let mut g = Gloops::new();
        let mut nr = Rand::new();
        g.reset(0, &mut nr);
        let w = world();
        let mut ex = Explosions::new();
        let mut ev = Events::new();

        let idx = g.pool.iter().position(|s| s.visible).unwrap();
        assert_eq!(g.damage(idx, 10, &w, &mut ex, &mut ev), 0);
        assert_eq!(g.pool[idx].hp, 290);
        assert_eq!(g.damage(idx, 9999, &w, &mut ex, &mut ev), 100);
        assert!(!g.pool[idx].visible);
        assert_eq!(g.num_gloops, 1);
    }
}
