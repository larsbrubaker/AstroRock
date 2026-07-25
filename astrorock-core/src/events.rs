//! # Gameplay events
//!
//! The original gameplay code calls sound and cross-system functions
//! directly (`pSoundMedExplo->Play()`, `AddGoody(sprite)`). The port
//! decouples those side effects into an event queue the frame loop
//! drains: the audio phase turns Sfx events into playback, and systems
//! not yet ported consume theirs when they land. Emission order is
//! deterministic (it follows the original call order exactly).

/// One frame's side effects, in emission order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GameEvent {
    /// `pSoundBigExplo->SetPan(x); Play()` — pan is the screen-relative
    /// x from `GetPosRelCenter`.
    SfxBigExplosion { pan: i32 },
    /// `pSoundMedExplo` — shot hits and small explosions.
    SfxMedExplosion { pan: i32 },
    /// `AddGoody(sprite)` — a little rock died; goodies system decides.
    SpawnGoody { x: f32, y: f32 },
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
        ev.push(GameEvent::SpawnGoody { x: 1.0, y: 2.0 });
        let all: Vec<_> = ev.drain().collect();
        assert_eq!(
            all,
            vec![
                GameEvent::SfxMedExplosion { pan: -10 },
                GameEvent::SpawnGoody { x: 1.0, y: 2.0 }
            ]
        );
        assert!(ev.is_empty());
    }
}
