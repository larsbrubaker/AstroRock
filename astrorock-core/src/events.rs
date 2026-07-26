//! # Gameplay events
//!
//! The original gameplay code calls sound and cross-system functions
//! directly (`pSoundMedExplo->Play()`, `AddGoody(sprite)`). The port
//! decouples those side effects into an event queue the frame loop
//! drains: the audio phase turns Sfx events into playback, and systems
//! not yet ported consume theirs when they land. Emission order is
//! deterministic (it follows the original call order exactly).

use crate::goodies::GoodyKind;
use crate::shots::ShotTier;

/// One frame's side effects, in emission order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GameEvent {
    /// `pSoundBigExplo->SetPan(x); Play()` — pan is the screen-relative
    /// x from `GetPosRelCenter`.
    SfxBigExplosion { pan: i32 },
    /// `pSoundMedExplo` — shot hits and small explosions.
    SfxMedExplosion { pan: i32 },
    /// A shot pool fired (`pFireSound->Play()`, rShot01/02/03Snd).
    SfxShotFire { tier: ShotTier },
    /// A bomb launched (`rBombSnd`).
    SfxBombFire,
    /// Weapon switch completed (`pChangeGunSound`).
    SfxChangeGun,
    /// The intermission bonus blip (`pBonusSound`, rBonusSnd).
    SfxBonus,
    /// A warp-in started (`pShimmerSound`, rShimmerSnd).
    SfxShimmer,
    /// The local player took damage this pass (hurt voice bank).
    VoiceHurt,
    /// Score jumped ≥200 inside the look window (carnage bank).
    VoiceCarnage,
    /// The local player died (`KillPlayer` voice bank).
    VoiceDead,
    /// Respawned after death (`AddPlayer` new-ship bank).
    VoiceNewShip,
    /// `GetGoody` picked up (the pickup jingle / voice line).
    GoodyCollected { kind: GoodyKind },
}

#[derive(Default)]
pub struct Events {
    queue: Vec<GameEvent>,
}

impl Events {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, e: GameEvent) {
        self.queue.push(e);
    }

    /// Drain this frame's events in order.
    pub fn drain(&mut self) -> impl Iterator<Item = GameEvent> + '_ {
        self.queue.drain(..)
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drains_in_emission_order() {
        let mut ev = Events::new();
        ev.push(GameEvent::SfxMedExplosion { pan: -10 });
        ev.push(GameEvent::SfxBombFire);
        let all: Vec<_> = ev.drain().collect();
        assert_eq!(
            all,
            vec![
                GameEvent::SfxMedExplosion { pan: -10 },
                GameEvent::SfxBombFire
            ]
        );
        assert!(ev.is_empty());
    }
}
