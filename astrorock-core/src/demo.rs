//! # Demo recordings — the determinism backbone (Phase 9)
//!
//! The loose `demo/*.dat` files (April 1997, recorded by the shipped
//! code, now committed under `assets/demos/`) are `SaveADemo`'s
//! format: `u32 NumDemoUpdates, u32 DemoStartLevel`, then one
//! `KeyFlags` byte per 30 Hz beat. The shipped builds had
//! `CHECK_DEMO` commented out, so there are no recorded checksums —
//! verification rests on the ending (a recording stopped the moment
//! the pilot died or pressed Enter, so a bit-exact replay must
//! reproduce that ending) plus golden checksums we record ourselves
//! (tests/demo_replay.rs) pinning the whole per-beat stream against
//! regressions on every platform.
//!
//! Playback ports `InitDemo` + the `STATE_PLAYINGDEMO` path of
//! `CheckControls`: inputs only refresh while the ship is visible
//! (stale flags otherwise), and no state transitions run — demos
//! never end a level; the death gate never spawns.

use crate::game::{Game, NUM_START_SHIPS};
use crate::input::KeysHeld;
use crate::pship::PlayerShip;

/// `FLAG*` — the recorded input bits.
pub const FLAG_TURN_LEFT: u8 = 1;
pub const FLAG_TURN_RIGHT: u8 = 2;
pub const FLAG_THRUST: u8 = 4;
pub const FLAG_SHIELD: u8 = 8;
pub const FLAG_FIRE: u8 = 16;
pub const FLAG_BOMBS: u8 = 32;
// FLAGSPAWN (64) and FLAGQUIT (128) are net-message bits — never in
// demo recordings.

/// `#define DEMORANDSEED 12`
const DEMO_RAND_SEED: u32 = 12;
/// `eFastDeath` (spawnfx.hpp) — the only kind that warps in solo.
pub(crate) const E_FAST_DEATH: u32 = 6;

/// A parsed `demo/*.dat`.
pub struct Demo {
    pub start_level: u32,
    pub key_flags: Vec<u8>,
}

impl Demo {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 8 {
            return Err(format!("demo too short: {} bytes", bytes.len()));
        }
        let count = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let start_level = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let rest = &bytes[8..];
        if rest.len() != count {
            return Err(format!(
                "demo body is {} bytes but the header says {count} updates",
                rest.len()
            ));
        }
        Ok(Self {
            start_level,
            key_flags: rest.to_vec(),
        })
    }
}

/// The recorded byte -> held keys (`SetInputs` from `KeyFlags`).
pub fn keys_from_flags(flags: u8) -> KeysHeld {
    KeysHeld {
        left: flags & FLAG_TURN_LEFT != 0,
        right: flags & FLAG_TURN_RIGHT != 0,
        thrust: flags & FLAG_THRUST != 0,
        shield: flags & FLAG_SHIELD != 0,
        fire: flags & FLAG_FIRE != 0,
        bomb: flags & FLAG_BOMBS != 0,
    }
}

impl Game {
    /// `InitDemo`: reseed NetRand with 12 (+12 warm-up `NetRand(10)`
    /// draws), reset the players (`InitPlayers` leaves
    /// `LocalPlayerIsDead = 1`), `ResetAll`, and `AddPlayer` — which,
    /// being dead, runs the full `NewShip`.
    pub fn init_demo(&mut self, start_level: u32) {
        self.net_rand.seed(DEMO_RAND_SEED);
        for _ in 0..DEMO_RAND_SEED {
            self.net_rand.rand(10);
        }
        self.level = start_level as usize;
        // `InitPlayers`.
        self.ship = PlayerShip::new();
        self.ship.reset(NUM_START_SHIPS);
        self.local_player_dead = true;
        self.need_add_player = false;
        self.reset_level();
        self.respawn();
        self.keys = KeysHeld::default();
    }

    /// One `STATE_PLAYINGDEMO` beat: refresh the held keys from the
    /// recording only while the ship is visible (`CheckControls`
    /// keeps the stale flags otherwise), then run `AdvanceFrames`.
    /// No transitions: demos never end levels or gate respawns.
    pub fn demo_beat(&mut self, flags: u8) {
        if self.ship.sprite.visible {
            self.keys = keys_from_flags(flags);
        }
        self.sim_beat(Self::clip());
        // Headless runs have no sink; don't let events pile up.
        if self.audio.is_none() {
            for _ in self.events.drain() {}
        }
    }

    /// `CheckPlayField` — the per-beat f32 checksum, truncated to a
    /// byte exactly like the C cast.
    pub fn check_play_field(&self) -> u8 {
        let mut sum = 0.0f32;
        sum += self.net_rand.sync() as f32;
        sum += self.level as f32;
        // GlobalNetSpawnRocks + GlobalNetSpawnBadGuys +
        // NetGetNumPlayers() are all 0 in single player (+0.0 exact).
        sum += self.spawnfx.check() as f32;
        sum += self.explosions.check();
        sum += self.speaker.sprite.check(false);
        sum += self.goodies.check();
        sum += self.rocks.check();
        sum += self.gloops.check();
        sum += self.spikeballs.check();
        sum += self.hks.check();
        sum += self.bombers.check();
        sum += self.fastdeaths.check();
        sum += self.check_players();
        (sum as i32) as u8
    }

    /// `PlayersCheck`: `PlayerList.Check()` counts only VISIBLE
    /// sprites (the seven ghost slots and a waiting-to-spawn ship add
    /// exactly 0.0); then per slot the six power-up counters while
    /// visible, and the three shot pools + bombs always (all zero for
    /// ghosts).
    fn check_players(&self) -> f32 {
        let mut sum = 0.0f32;
        if self.ship.sprite.visible {
            sum += self.ship.sprite.check(false);
        }
        if self.ship.sprite.visible {
            sum += self.ship.num_power_shots as f32;
            sum += self.ship.num_super_shots as f32;
            sum += self.ship.num_bombs as f32;
            sum += self.ship.num_spreads as f32;
            sum += self.ship.num_rapids as f32;
            sum += self.ship.num_shields as f32;
        }
        sum += self.ship.shots[crate::pship::NORMAL_SHOTS].check();
        sum += self.ship.shots[crate::pship::POWER_SHOTS].check();
        sum += self.ship.shots[crate::pship::SUPER_SHOTS].check();
        sum += self.ship.bombs.check();
        sum
    }

    /// Playback probes for the harness.
    pub fn ship_visible(&self) -> bool {
        self.ship.sprite.visible
    }
    pub fn score(&self) -> u32 {
        self.ship.score
    }
    pub fn rand_sync(&self) -> u32 {
        self.net_rand.sync()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rejects_bad_sizes() {
        assert!(Demo::parse(&[0u8; 4]).is_err());
        // Header says 4 updates, body has 3.
        let mut bytes = vec![4, 0, 0, 0, 0, 0, 0, 0];
        bytes.extend_from_slice(&[0, 0, 0]);
        assert!(Demo::parse(&bytes).is_err());
        bytes.push(0);
        let demo = Demo::parse(&bytes).expect("valid");
        assert_eq!(demo.key_flags.len(), 4);
        assert_eq!(demo.start_level, 0);
    }

    #[test]
    fn flags_map_to_keys() {
        let keys = keys_from_flags(FLAG_TURN_LEFT | FLAG_FIRE | FLAG_THRUST);
        assert!(keys.left && keys.fire && keys.thrust);
        assert!(!keys.right && !keys.shield && !keys.bomb);
    }

    #[test]
    fn init_demo_is_reproducible() {
        let mut a = Game::new(None);
        let mut b = Game::new(None);
        a.init_demo(3);
        b.init_demo(3);
        assert_eq!(a.check_play_field(), b.check_play_field());
        assert_eq!(a.rand_sync(), b.rand_sync());
        assert!(a.ship_visible(), "AddPlayer spawns the demo pilot");

        // And a few beats of identical inputs stay identical.
        for flags in [0u8, FLAG_THRUST, FLAG_THRUST | FLAG_FIRE, 0, FLAG_TURN_LEFT] {
            a.demo_beat(flags);
            b.demo_beat(flags);
        }
        assert_eq!(a.check_play_field(), b.check_play_field());
    }
}
