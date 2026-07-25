//! # Audio dispatch — the port of the game's `CSound` call sites
//!
//! Core stays platform-free: gameplay emits [`GameEvent`]s and exposes
//! loop states (thrust/shield hum); this module turns them into calls
//! on an [`AudioSink`] the platform shells implement (rodio on native,
//! WebAudio on wasm). Sinks receive one-shot [`SfxId`]s with the
//! original's screen-relative pan, and start/stop [`LoopKind`]s.
//!
//! `GetGoody`'s sound pick is ported here: LocalRand(4) — 3-in-4 plays
//! the generic pickup jingle, otherwise the goody's voice line (the
//! original routed voices through `PausePlayerPlay` with a short
//! delay; the delay is a polish item, tracked in todo.md).

use crate::events::{Events, GameEvent};
use crate::goodies::GoodyKind;
use crate::rand::Rand;
use crate::shots::ShotTier;

/// One-shot effects, named after their `assets/sfx/*.mp3` files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SfxId {
    /// rExplo01Snd — the big kill explosion.
    BigExplosion,
    /// rExplo02Snd — shot hits and small explosions.
    MedExplosion,
    ShotNormal,
    ShotPower,
    ShotSuper,
    ShotHk,
    BombFire,
    ChangeGun,
    /// The intermission bonus blip (`rBonusSnd`).
    Bonus,
    /// The generic pickup jingle (`rGoodySnd`).
    Goody,
    /// Per-goody voice lines.
    VoiceBitchen,
    VoiceHose,
    VoiceStick,
    VoiceSugar,
    VoiceTKill,
    VoiceKickAss,
    VoiceAhYah,
    /// The spawn shimmer (`rShimmerSnd`).
    Shimmer,
}

impl SfxId {
    /// The `assets/sfx/` file stem for this effect.
    pub fn stem(self) -> &'static str {
        match self {
            SfxId::BigExplosion => "explo01",
            SfxId::MedExplosion => "explo02",
            SfxId::ShotNormal => "shot01",
            SfxId::ShotPower => "shot02",
            SfxId::ShotSuper => "shot03",
            SfxId::ShotHk => "shothk",
            SfxId::BombFire => "bomb",
            SfxId::ChangeGun => "gunchng",
            SfxId::Bonus => "bonus",
            SfxId::Goody => "goody",
            SfxId::VoiceBitchen => "bitchen",
            SfxId::VoiceHose => "hose",
            SfxId::VoiceStick => "stick",
            SfxId::VoiceSugar => "sugar",
            SfxId::VoiceTKill => "tkill",
            SfxId::VoiceKickAss => "kass",
            SfxId::VoiceAhYah => "ahyah",
            SfxId::Shimmer => "shimmer",
        }
    }

    /// The embedded mp3 for this effect — shared by every platform
    /// sink so the stem→bytes mapping exists exactly once.
    pub fn bytes(self) -> &'static [u8] {
        macro_rules! sfx {
            ($($variant:ident => $stem:literal),+ $(,)?) => {
                match self {
                    $(SfxId::$variant => include_bytes!(concat!(
                        "../../assets/sfx/", $stem, ".mp3"
                    )) as &'static [u8],)+
                }
            };
        }
        sfx! {
            BigExplosion => "explo01",
            MedExplosion => "explo02",
            ShotNormal => "shot01",
            ShotPower => "shot02",
            ShotSuper => "shot03",
            ShotHk => "shothk",
            BombFire => "bomb",
            ChangeGun => "gunchng",
            Bonus => "bonus",
            Goody => "goody",
            VoiceBitchen => "bitchen",
            VoiceHose => "hose",
            VoiceStick => "stick",
            VoiceSugar => "sugar",
            VoiceTKill => "tkill",
            VoiceKickAss => "kass",
            VoiceAhYah => "ahyah",
            Shimmer => "shimmer",
        }
    }

    /// Every effect a sink should preload.
    pub fn all() -> &'static [SfxId] {
        &[
            SfxId::BigExplosion,
            SfxId::MedExplosion,
            SfxId::ShotNormal,
            SfxId::ShotPower,
            SfxId::ShotSuper,
            SfxId::ShotHk,
            SfxId::BombFire,
            SfxId::ChangeGun,
            SfxId::Bonus,
            SfxId::Goody,
            SfxId::VoiceBitchen,
            SfxId::VoiceHose,
            SfxId::VoiceStick,
            SfxId::VoiceSugar,
            SfxId::VoiceTKill,
            SfxId::VoiceKickAss,
            SfxId::VoiceAhYah,
            SfxId::Shimmer,
        ]
    }
}

/// Continuous sounds driven by state, not events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoopKind {
    /// `pThrustSound` (thrust01), looping while thrusting.
    Thrust,
    /// `pShieldSound` (shield), looping while the shield holds.
    Shield,
}

impl LoopKind {
    /// The embedded mp3 for this loop.
    pub fn bytes(self) -> &'static [u8] {
        match self {
            LoopKind::Thrust => include_bytes!("../../assets/sfx/thrust01.mp3"),
            LoopKind::Shield => include_bytes!("../../assets/sfx/shield.mp3"),
        }
    }

    /// Every loop a sink should preload.
    pub fn all() -> &'static [LoopKind] {
        &[LoopKind::Thrust, LoopKind::Shield]
    }
}

/// The soundtrack, in play order. The original streamed one giant
/// concatenated PCM file (`Astro.Rck`) end to end and restarted it
/// whenever it ran out; playing these tracks in sequence and wrapping
/// reproduces that. NOT embedded (megabytes) — native reads
/// `assets/music/`, wasm fetches `music/` next to the page.
pub const MUSIC_TRACKS: &[&str] = &[
    "track02", "track03", "track04", "track05", "track06", "track07", "track08",
];

/// What the platform shells implement.
pub trait AudioSink {
    /// Play a one-shot. `pan` is the original's screen-relative x from
    /// `GetPosRelCenter` (−320..320 on screen; may exceed off-screen).
    fn play(&mut self, sfx: SfxId, pan: i32);
    /// Start/stop a loop (idempotent).
    fn set_loop(&mut self, which: LoopKind, active: bool);
    /// Called every frame with the desired music state. The sink owns
    /// sequencing through [`MUSIC_TRACKS`] (mirroring the original's
    /// "restart the stream when it stops" main-loop poll); `on` is the
    /// original's volume-above-minimum condition.
    fn set_music(&mut self, on: bool);
}

/// Drain a frame's events into the sink. `local_rand` drives the
/// goody sound pick exactly like `GetGoody` (visual/audio RNG — not
/// part of the synced stream).
pub fn dispatch(events: &mut Events, sink: &mut dyn AudioSink, local_rand: &mut Rand) {
    for event in events.drain() {
        match event {
            GameEvent::SfxBigExplosion { pan } => sink.play(SfxId::BigExplosion, pan),
            GameEvent::SfxMedExplosion { pan } => sink.play(SfxId::MedExplosion, pan),
            GameEvent::SfxShotFire { tier } => {
                let id = match tier {
                    ShotTier::Normal => SfxId::ShotNormal,
                    ShotTier::Power => SfxId::ShotPower,
                    ShotTier::Super => SfxId::ShotSuper,
                    ShotTier::Hk => SfxId::ShotHk,
                };
                sink.play(id, 0);
            }
            GameEvent::SfxBombFire => sink.play(SfxId::BombFire, 0),
            GameEvent::SfxChangeGun => sink.play(SfxId::ChangeGun, 0),
            GameEvent::SfxBonus => sink.play(SfxId::Bonus, 0),
            GameEvent::GoodyCollected { kind } => {
                if local_rand.rand(4) != 0 {
                    sink.play(SfxId::Goody, 0);
                } else {
                    let voice = match kind {
                        GoodyKind::Shield | GoodyKind::Health => SfxId::VoiceBitchen,
                        GoodyKind::Rapid => SfxId::VoiceHose,
                        GoodyKind::Gun1 => SfxId::VoiceStick,
                        GoodyKind::Gun2 => SfxId::VoiceSugar,
                        GoodyKind::Bomb => SfxId::VoiceTKill,
                        GoodyKind::Spread => SfxId::VoiceKickAss,
                        GoodyKind::Freeman => SfxId::VoiceAhYah,
                    };
                    sink.play(voice, 0);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recorder {
        plays: Vec<(SfxId, i32)>,
        loops: Vec<(LoopKind, bool)>,
        music: Vec<bool>,
    }

    impl AudioSink for Recorder {
        fn play(&mut self, sfx: SfxId, pan: i32) {
            self.plays.push((sfx, pan));
        }
        fn set_loop(&mut self, which: LoopKind, active: bool) {
            self.loops.push((which, active));
        }
        fn set_music(&mut self, on: bool) {
            self.music.push(on);
        }
    }

    #[test]
    fn events_map_to_effects_in_order() {
        let mut ev = Events::new();
        ev.push(GameEvent::SfxBigExplosion { pan: -100 });
        ev.push(GameEvent::SfxShotFire {
            tier: ShotTier::Super,
        });
        ev.push(GameEvent::SfxBombFire);

        let mut rec = Recorder::default();
        let mut lr = Rand::new();
        dispatch(&mut ev, &mut rec, &mut lr);
        assert_eq!(
            rec.plays,
            vec![
                (SfxId::BigExplosion, -100),
                (SfxId::ShotSuper, 0),
                (SfxId::BombFire, 0)
            ]
        );
        assert!(ev.is_empty());
    }

    #[test]
    fn goody_pick_is_jingle_or_voice() {
        let mut rec = Recorder::default();
        let mut lr = Rand::new();
        for _ in 0..40 {
            let mut ev = Events::new();
            ev.push(GameEvent::GoodyCollected {
                kind: GoodyKind::Rapid,
            });
            dispatch(&mut ev, &mut rec, &mut lr);
        }
        let jingles = rec.plays.iter().filter(|(s, _)| *s == SfxId::Goody).count();
        let voices = rec
            .plays
            .iter()
            .filter(|(s, _)| *s == SfxId::VoiceHose)
            .count();
        assert_eq!(jingles + voices, 40);
        assert!(jingles > voices, "3-in-4 jingle bias");
    }

    #[test]
    fn every_music_track_exists_on_disk() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/music");
        for stem in MUSIC_TRACKS {
            let path = dir.join(format!("{stem}.mp3"));
            assert!(path.is_file(), "missing music track {}", path.display());
        }
    }

    #[test]
    fn every_sfx_has_a_unique_stem() {
        let mut seen = std::collections::HashSet::new();
        for &id in SfxId::all() {
            assert!(seen.insert(id.stem()), "duplicate stem {}", id.stem());
        }
    }
}
