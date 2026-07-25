//! # Rocks — port of `rocks.cpp`
//!
//! Fixed pools (10 big, 20 medium, 40 little) toggled by visibility.
//! Levels set how many big rocks spawn (`rocks.cfg`, recovered from the
//! shipped rez); big rocks split into two mediums, mediums into two
//! littles (spawned into invisible pool slots in list order), littles
//! drop a goody. RNG draws go through NetRand in the exact original
//! order — determinism hangs on it.
//!
//! Faithful quirk: `MedP1` gives spawned little rocks `MEDHP` (110),
//! not `LITHP` — little rocks from splits are tougher than `LitP1`'s
//! kill threshold assumes fresh ones would be.
//!
//! The collide entry points arrive with shots/players (their handler
//! call sites define the walker semantics).

use crate::events::Events;
use crate::frame::Frame;
use crate::goodies::Goodies;
use crate::rand::Rand;
use crate::rect::Rect;
use crate::sequence;
use crate::sprite::Sprite;
use crate::virtual_frame::VirtualFrame;

pub const MAX_LEVELS: usize = 40;
const MAX_BIG_ROCKS: usize = 10;

const BIG_HP: u32 = 220;
const MED_HP: u32 = 110;
// LITHP (50) is defined in the original but never assigned — little
// rocks only ever get MED_HP via the MedP1 quirk (see module docs).

const BIG_SCORE_ADD: u32 = 20;
const MED_SCORE_ADD: u32 = 40;
const LIT_SCORE_ADD: u32 = 50;

/// Recovered per-level big-rock counts.
const ROCKS_CFG: &str = include_str!("../../assets/config/rocks.cfg");

pub struct Rocks {
    big: Vec<Sprite>,
    med: Vec<Sprite>,
    lit: Vec<Sprite>,
    pub num_big: u32,
    pub num_med: u32,
    pub num_lit: u32,
    max_big: u32,
    level_num_rocks: [u32; MAX_LEVELS],
}

impl Rocks {
    /// `RocksInit` — parse the config, build the pools.
    pub fn new() -> Self {
        let mut level_num_rocks = [0u32; MAX_LEVELS];
        for (i, line) in ROCKS_CFG.lines().take(MAX_LEVELS).enumerate() {
            // "01:04," — level:count. sscanf("%u:%u") semantics.
            let mut parts = line.split(&[':', ','][..]);
            let _level = parts.next();
            let count: u32 = parts
                .next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            level_num_rocks[i] = count.min(MAX_BIG_ROCKS as u32);
        }

        let make_pool = |n: usize, seq: fn() -> std::rc::Rc<crate::sequence::FrameSequence>| {
            let shared = seq();
            (0..n)
                .map(|_| {
                    let mut s = Sprite::new();
                    s.set_sequence(shared.clone());
                    s
                })
                .collect::<Vec<_>>()
        };

        Self {
            big: make_pool(MAX_BIG_ROCKS, sequence::ast_big),
            med: make_pool(2 * MAX_BIG_ROCKS, sequence::ast_med),
            lit: make_pool(4 * MAX_BIG_ROCKS, sequence::ast_small),
            num_big: 0,
            num_med: 0,
            num_lit: 0,
            max_big: 0,
            level_num_rocks,
        }
    }

    /// `RocksReset(level)` — level is 0-based like the original index.
    pub fn reset(&mut self, level: usize, net_rand: &mut Rand) {
        self.num_big = 0;
        self.num_med = 0;
        self.num_lit = 0;
        self.max_big = if level < MAX_LEVELS {
            self.level_num_rocks[level]
        } else {
            MAX_BIG_ROCKS as u32
        };

        // SetVisAndMove over each pool, in list order (RNG order!).
        for i in 0..self.big.len() {
            let (width, height) = (2048u32, 1024u32);
            let s = &mut self.big[i];
            s.reset();
            s.frame_advance = if net_rand.rand(2) != 0 { -1.0 } else { 1.0 };
            s.hp = BIG_HP;
            if self.num_big < self.max_big {
                s.visible = true;
                s.x_pos = net_rand.rand(width) as f32;
                s.y_pos = net_rand.rand(height) as f32;
                s.x_delta = (net_rand.rand_about0(8) + 1) as f32;
                if s.x_delta == 0.0 {
                    s.x_delta = 1.0;
                }
                s.y_delta = net_rand.rand_about0(8) as f32;
                let frames = s.sequence().expect("seq").num_frames;
                s.cur_frame = net_rand.rand(frames) as f32;
                self.num_big += 1;
            } else {
                s.visible = false;
            }
        }
        for s in &mut self.med {
            s.reset();
            s.frame_advance = if net_rand.rand(2) != 0 { -1.0 } else { 1.0 };
            s.visible = false;
        }
        for s in &mut self.lit {
            s.reset();
            s.frame_advance = if net_rand.rand(2) != 0 { -1.0 } else { 1.0 };
            s.visible = false;
        }
    }

    /// `RocksUpdate`.
    pub fn update(&mut self, clip: &Rect, rand: &mut Rand) {
        for s in &mut self.big {
            let _ = s.update(clip, rand);
        }
        for s in &mut self.med {
            let _ = s.update(clip, rand);
        }
        for s in &mut self.lit {
            let _ = s.update(clip, rand);
        }
    }

    /// `RocksDraw` — returns total active rocks (level-clear signal).
    /// Radar plotting joins with the stat-bar/radar composition.
    pub fn draw(&self, world: &VirtualFrame, screen: &mut Frame) -> u32 {
        for s in &self.big {
            s.draw(world, screen);
        }
        for s in &self.med {
            s.draw(world, screen);
        }
        for s in &self.lit {
            s.draw(world, screen);
        }
        self.num_big + self.num_med + self.num_lit
    }

    /// `RocksCheck` — list checksums plus the live counters.
    pub fn check(&self) -> f32 {
        let mut sum = 0.0f32;
        for s in &self.big {
            sum += s.check(false);
        }
        for s in &self.med {
            sum += s.check(false);
        }
        for s in &self.lit {
            sum += s.check(false);
        }
        sum += (self.num_big + self.num_med + self.num_lit) as f32;
        sum
    }

    /// Iterate rocks for radar plotting: (sprite, radar color).
    pub fn radar_sprites(&self) -> impl Iterator<Item = (&Sprite, u8)> {
        self.big
            .iter()
            .map(|s| (s, 15u8))
            .chain(self.med.iter().map(|s| (s, 145u8)))
            .chain(self.lit.iter().map(|s| (s, 147u8)))
    }

    /// `BigP1` applied to big rock `index` — damage, split, score.
    /// Returns the score added (accumulated by the caller like
    /// `ScoreAdd`).
    pub fn damage_big(
        &mut self,
        index: usize,
        damage: u32,
        net_rand: &mut Rand,
        world: &VirtualFrame,
        explosions: &mut crate::explosion::Explosions,
        events: &mut Events,
    ) -> u32 {
        let sprite = &mut self.big[index];
        if damage >= sprite.hp {
            sprite.hp = 0;
            self.num_big -= 1;
            self.num_med += 2;
            let (x, y) = (sprite.x_pos, sprite.y_pos);

            let mut spawned = 0;
            let mut i = 0;
            while spawned < 2 {
                let p = &mut self.med[i];
                if !p.visible {
                    p.visible = true;
                    p.x_pos = x;
                    p.y_pos = y;
                    p.hp = MED_HP;
                    p.x_delta = (net_rand.rand_about0(4) + 1) as f32;
                    if p.x_delta == 0.0 {
                        p.x_delta = 1.0;
                    }
                    p.y_delta = net_rand.rand_about0(4) as f32;
                    let frames = p.sequence().expect("seq").num_frames;
                    p.cur_frame = net_rand.rand(frames) as f32;
                    p.frame_advance = if net_rand.rand(2) != 0 { -1.0 } else { 1.0 };
                    spawned += 1;
                }
                i += 1;
            }
            explosions.explo_sprite(&mut self.big[index], world, events);
            BIG_SCORE_ADD
        } else {
            sprite.hp -= damage;
            explosions.play_shot_hit_at(sprite.x_pos, sprite.y_pos, world, events);
            0
        }
    }

    /// `MedP1` — see module docs for the MED_HP-on-littles quirk.
    pub fn damage_med(
        &mut self,
        index: usize,
        damage: u32,
        net_rand: &mut Rand,
        world: &VirtualFrame,
        explosions: &mut crate::explosion::Explosions,
        events: &mut Events,
    ) -> u32 {
        let sprite = &mut self.med[index];
        if damage >= sprite.hp {
            sprite.hp = 0;
            self.num_med -= 1;
            self.num_lit += 2;
            let (x, y) = (sprite.x_pos, sprite.y_pos);

            let mut spawned = 0;
            let mut i = 0;
            while spawned < 2 {
                let p = &mut self.lit[i];
                if !p.visible {
                    p.visible = true;
                    p.x_pos = x;
                    p.y_pos = y;
                    p.hp = MED_HP; // original quirk (not LIT_HP)
                    p.x_delta = (net_rand.rand_about0(8) + 1) as f32;
                    if p.x_delta == 0.0 {
                        p.x_delta = 1.0;
                    }
                    p.y_delta = net_rand.rand_about0(8) as f32;
                    let frames = p.sequence().expect("seq").num_frames;
                    p.cur_frame = net_rand.rand(frames) as f32;
                    p.frame_advance = if net_rand.rand(2) != 0 { -1.0 } else { 1.0 };
                    spawned += 1;
                }
                i += 1;
            }
            explosions.explo_sprite(&mut self.med[index], world, events);
            MED_SCORE_ADD
        } else {
            sprite.hp -= damage;
            explosions.play_shot_hit_at(sprite.x_pos, sprite.y_pos, world, events);
            0
        }
    }

    /// `LitP1` — death drops a goody (`AddGoody` runs inline, before
    /// the explosion, exactly where the original draws its NetRands).
    #[allow(clippy::too_many_arguments)]
    pub fn damage_lit(
        &mut self,
        index: usize,
        damage: u32,
        goodies: &mut Goodies,
        net_rand: &mut Rand,
        world: &VirtualFrame,
        explosions: &mut crate::explosion::Explosions,
        events: &mut Events,
    ) -> u32 {
        let sprite = &mut self.lit[index];
        if damage >= sprite.hp {
            sprite.hp = 0;
            self.num_lit -= 1;
            let (x, y, xd, yd) = (sprite.x_pos, sprite.y_pos, sprite.x_delta, sprite.y_delta);
            goodies.add_goody(x, y, xd, yd, net_rand);
            explosions.explo_sprite(&mut self.lit[index], world, events);
            LIT_SCORE_ADD
        } else {
            sprite.hp -= damage;
            explosions.play_shot_hit_at(sprite.x_pos, sprite.y_pos, world, events);
            0
        }
    }

    pub fn big(&self) -> &[Sprite] {
        &self.big
    }

    pub fn med(&self) -> &[Sprite] {
        &self.med
    }

    pub fn lit(&self) -> &[Sprite] {
        &self.lit
    }
}

impl Default for Rocks {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explosion::Explosions;

    fn world() -> VirtualFrame {
        let mut w = VirtualFrame::new(2048, 1024);
        w.set_on_screen_rect(Rect::new(0, 0, 640, 480));
        w
    }

    #[test]
    fn cfg_parses_shipped_level_table() {
        let rocks = Rocks::new();
        // rocks.cfg: 01:04, 02:04, 03:04, 04:05 … 15:10, 22:03.
        assert_eq!(rocks.level_num_rocks[0], 4);
        assert_eq!(rocks.level_num_rocks[3], 5);
        assert_eq!(rocks.level_num_rocks[14], 10);
        assert_eq!(rocks.level_num_rocks[21], 3);
    }

    #[test]
    fn reset_spawns_level_count_of_big_rocks() {
        let mut rocks = Rocks::new();
        let mut nr = Rand::new();
        rocks.reset(0, &mut nr);
        assert_eq!(rocks.num_big, 4);
        assert_eq!(rocks.big.iter().filter(|s| s.visible).count(), 4);
        assert_eq!(rocks.med.iter().filter(|s| s.visible).count(), 0);
        // Every visible rock is inside the world and moving in x.
        for s in rocks.big.iter().filter(|s| s.visible) {
            assert!(s.x_pos >= 0.0 && s.x_pos < 2048.0);
            assert!(s.x_delta != 0.0);
        }
    }

    #[test]
    fn reset_is_deterministic_for_a_seed() {
        let mut a = Rocks::new();
        let mut b = Rocks::new();
        let mut ra = Rand::new();
        let mut rb = Rand::new();
        a.reset(2, &mut ra);
        b.reset(2, &mut rb);
        for (x, y) in a.big.iter().zip(b.big.iter()) {
            assert_eq!(x.x_pos.to_bits(), y.x_pos.to_bits());
            assert_eq!(x.cur_frame.to_bits(), y.cur_frame.to_bits());
        }
    }

    #[test]
    fn big_rock_splits_into_two_mediums() {
        let mut rocks = Rocks::new();
        let mut nr = Rand::new();
        let w = world();
        let mut ex = Explosions::new();
        let mut ev = Events::new();
        rocks.reset(0, &mut nr);

        let idx = rocks.big.iter().position(|s| s.visible).unwrap();
        let score = rocks.damage_big(idx, 9999, &mut nr, &w, &mut ex, &mut ev);
        assert_eq!(score, BIG_SCORE_ADD);
        assert_eq!(rocks.num_big, 3);
        assert_eq!(rocks.num_med, 2);
        assert_eq!(rocks.med.iter().filter(|s| s.visible).count(), 2);
        assert!(!rocks.big[idx].visible, "dead rock hidden by ExploSprite");

        // Partial damage only chips HP.
        let idx2 = rocks.big.iter().position(|s| s.visible).unwrap();
        let score = rocks.damage_big(idx2, 10, &mut nr, &w, &mut ex, &mut ev);
        assert_eq!(score, 0);
        assert_eq!(rocks.big[idx2].hp, BIG_HP - 10);
    }

    #[test]
    fn little_rock_death_rolls_for_a_goody() {
        let mut rocks = Rocks::new();
        let mut nr = Rand::new();
        let w = world();
        let mut ex = Explosions::new();
        let mut ev = Events::new();
        let mut goodies = Goodies::new();
        goodies.reset(0, &mut nr);
        rocks.reset(0, &mut nr);

        // Promote: kill a big, then a med, then a lit; repeat little
        // kills until the 40%% drop chance fires.
        let b = rocks.big.iter().position(|s| s.visible).unwrap();
        rocks.damage_big(b, 9999, &mut nr, &w, &mut ex, &mut ev);
        let m = rocks.med.iter().position(|s| s.visible).unwrap();
        rocks.damage_med(m, 9999, &mut nr, &w, &mut ex, &mut ev);
        let l = rocks.lit.iter().position(|s| s.visible).unwrap();
        let score = rocks.damage_lit(l, 9999, &mut goodies, &mut nr, &w, &mut ex, &mut ev);
        assert_eq!(score, LIT_SCORE_ADD);
    }
}
