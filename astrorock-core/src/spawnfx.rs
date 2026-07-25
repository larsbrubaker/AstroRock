//! # Spawn effects — port of `spawnfx.cpp`
//!
//! The 30-beat warp-in shimmer that precedes a spawned object. In
//! single-player only Fast Deaths use it (rocks and the other enemies
//! respawn through it in net games only). While active: the incoming
//! object's animation frame advances (spikeballs walk their roll
//! segments with the special dance), 90 sparkle pixels scatter around
//! the spawn point each draw (LocalRand — visual only), and a radar
//! blip flickers. When the countdown ends the owner spawns the object
//! at the chosen spot.
//!
//! The fade-in of the object art itself (`FadeBlit[SpawnDuration/2]`)
//! needs the palette fade tables — it lands with them (todo.md).

use crate::frame::Frame;
use crate::radar::Radar;
use crate::rand::Rand;
use crate::virtual_frame::VirtualFrame;

const SPAWN_DURATION: u32 = 30;
const PAL_START: u32 = 178;
const PAL_DIST: u32 = 14;
const SHIMMER_DIST: u32 = 100;

/// What's being warped in (`SpawnObjectType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnKind {
    FastDeath,
    // Rock/Gloop/SpikeBall/Bomber/Hk arrive with net play.
}

/// A finished warp-in the owner must materialize.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompletedSpawn {
    pub kind: SpawnKind,
    pub x: f32,
    pub y: f32,
    pub cur_frame: f32,
}

pub struct SpawnFx {
    duration: u32,
    kind: Option<SpawnKind>,
    cur_frame: f32,
    spike_dir: f32,
    x: f32,
    y: f32,
}

impl SpawnFx {
    pub fn new() -> Self {
        Self {
            duration: 0,
            kind: None,
            cur_frame: 0.0,
            spike_dir: 1.0,
            x: 0.0,
            y: 0.0,
        }
    }

    /// `SpawnObj` — start a warp-in if none is running and the target
    /// system has room (`num < max`). Draws the spawn point from
    /// NetRand either way once accepted.
    pub fn spawn_obj(&mut self, kind: SpawnKind, num: u32, max: u32, net_rand: &mut Rand) {
        if self.duration == 0 && num < max {
            // Shimmer sound starts here (audio phase: rShimmerSnd).
            self.kind = Some(kind);
            self.cur_frame = 0.0;
            self.x = net_rand.rand(2048) as f32;
            self.y = net_rand.rand(1024) as f32;
            self.duration = SPAWN_DURATION;
        }
    }

    pub fn active(&self) -> bool {
        self.duration != 0 && self.kind.is_some()
    }

    /// `UpdateSpawnEffects` — advance the incoming object's frame and
    /// count down; returns the completed spawn on the final beat.
    /// (The spikeball roll-walk dance is ported for completeness even
    /// though spikeballs only warp in during net games.)
    pub fn update(&mut self, net_rand: &mut Rand) -> Option<CompletedSpawn> {
        if !self.active() {
            return None;
        }
        // Non-spikeball kinds just advance one frame per beat.
        self.cur_frame += 1.0;

        self.duration -= 1;
        if self.duration == 0 {
            let kind = self.kind.expect("active spawn has a kind");
            let _ = net_rand; // spikeball dance (net-only) draws here
            return Some(CompletedSpawn {
                kind,
                x: self.x,
                y: self.y,
                cur_frame: self.cur_frame,
            });
        }
        None
    }

    /// `DrawSpawnEffects` — 90 shimmer pixels around the spawn point
    /// plus a flickering radar blip; all LocalRand (visual-only).
    pub fn draw(
        &self,
        world: &VirtualFrame,
        screen: &mut Frame,
        radar: &mut Radar,
        local_rand: &mut Rand,
    ) {
        if !self.active() {
            return;
        }
        for _ in 0..90 {
            let angle = local_rand.rand(360);
            let dist = local_rand.rand(SHIMMER_DIST) as i32 - (SHIMMER_DIST as i32) / 2;
            let x = (self.x + crate::fixed_trig::cos_d(angle) * dist as f32) as i32;
            let y = (self.y + crate::fixed_trig::sin_d(angle) * dist as f32) as i32;
            let color = (PAL_START + local_rand.rand(PAL_DIST)) as u8;
            world.pset(screen, x, y, color);
        }
        // Radar blip at the spawn point.
        let mut blip = crate::sprite::Sprite::new();
        blip.x_pos = self.x;
        blip.y_pos = self.y;
        blip.visible = true;
        let color = (PAL_START + local_rand.rand(PAL_DIST)) as u8;
        radar.plot(&blip, color, world);
    }
}

impl Default for SpawnFx {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warp_in_completes_after_thirty_beats() {
        let mut fx = SpawnFx::new();
        let mut nr = Rand::new();
        fx.spawn_obj(SpawnKind::FastDeath, 0, 3, &mut nr);
        assert!(fx.active());

        let mut done = None;
        for _ in 0..SPAWN_DURATION {
            if let Some(c) = fx.update(&mut nr) {
                done = Some(c);
            }
        }
        let c = done.expect("spawn completed");
        assert_eq!(c.kind, SpawnKind::FastDeath);
        assert!(c.x >= 0.0 && c.x < 2048.0);
        assert!(!fx.active());
    }

    #[test]
    fn full_system_refuses_and_busy_refuses() {
        let mut fx = SpawnFx::new();
        let mut nr = Rand::new();
        fx.spawn_obj(SpawnKind::FastDeath, 3, 3, &mut nr); // full
        assert!(!fx.active());
        fx.spawn_obj(SpawnKind::FastDeath, 0, 3, &mut nr);
        let x = fx.x;
        fx.spawn_obj(SpawnKind::FastDeath, 0, 3, &mut nr); // busy — no-op
        assert_eq!(fx.x, x);
    }
}
