//! # Audio dispatch — the port of the game's `CSound` call sites
//!
//! Core stays platform-free: gameplay emits [`GameEvent`]s and exposes
//! loop states (thrust/shield hum); this module turns them into calls
//! on an [`AudioSink`] the platform shells implement (rodio on native,
//! WebAudio on wasm). Sinks receive one-shot [`SfxId`]s with the
//! original's screen-relative pan, and start/stop [`LoopKind`]s.
//!
//! Voice lines (goody pickups, hurt/carnage/death/new-ship banks) run
//! through [`VoicePlayer`] — the `PausedSoundPlayer` port: one pending
//! slot with the original delays, newer lines replacing older ones,
//! and playback cutting off whatever line still runs. All picks use
//! LocalRand exactly like the C++ trigger sites (audio-only RNG).

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
    /// New-ship lines (`pNewPlayerSounds`).
    VoiceLetsRock,
    VoicePartyTime,
    VoiceYah,
    /// Death lines (`pDeadPlayerSounds`).
    VoiceBumb,
    VoiceHarsh,
    VoiceDamn,
    VoiceLearned,
    /// Carnage lines (`pCarnagePlayerSounds`).
    VoiceTakeThat,
    VoiceBaby,
    VoiceForRock,
    VoiceWhoNext,
    VoiceBringOn,
    /// Hurt lines (`pHurtPlayerSounds`).
    VoiceMyTurn,
    VoiceOuch,
    VoicePaint,
    VoicePayback,
    /// The spawn shimmer (`rShimmerSnd`).
    Shimmer,
    /// Menu button click (`rClickedSnd`).
    Clicked,
    /// The showcase monitor's TV static (`rStaticSnd`).
    Static,
}

/// `pNewPlayerSounds` / `pDeadPlayerSounds` / `pCarnagePlayerSounds` /
/// `pHurtPlayerSounds`, in the original array orders (indexed by
/// LocalRand).
pub const NEW_SHIP_VOICES: [SfxId; 3] =
    [SfxId::VoiceLetsRock, SfxId::VoicePartyTime, SfxId::VoiceYah];
pub const DEAD_VOICES: [SfxId; 4] = [
    SfxId::VoiceBumb,
    SfxId::VoiceHarsh,
    SfxId::VoiceDamn,
    SfxId::VoiceLearned,
];
pub const CARNAGE_VOICES: [SfxId; 5] = [
    SfxId::VoiceTakeThat,
    SfxId::VoiceBaby,
    SfxId::VoiceForRock,
    SfxId::VoiceWhoNext,
    SfxId::VoiceBringOn,
];
pub const HURT_VOICES: [SfxId; 4] = [
    SfxId::VoiceMyTurn,
    SfxId::VoiceOuch,
    SfxId::VoicePaint,
    SfxId::VoicePayback,
];

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
            SfxId::VoiceLetsRock => "letsrock",
            SfxId::VoicePartyTime => "partytim",
            SfxId::VoiceYah => "yah",
            SfxId::VoiceBumb => "bumb",
            SfxId::VoiceHarsh => "harsh",
            SfxId::VoiceDamn => "damn",
            SfxId::VoiceLearned => "learned",
            SfxId::VoiceTakeThat => "takethat",
            SfxId::VoiceBaby => "baby",
            SfxId::VoiceForRock => "forrock",
            SfxId::VoiceWhoNext => "whonext",
            SfxId::VoiceBringOn => "bringon",
            SfxId::VoiceMyTurn => "myturn",
            SfxId::VoiceOuch => "ouch",
            SfxId::VoicePaint => "paint",
            SfxId::VoicePayback => "payback",
            SfxId::Shimmer => "shimmer",
            SfxId::Clicked => "clicked",
            SfxId::Static => "static",
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
            VoiceLetsRock => "letsrock",
            VoicePartyTime => "partytim",
            VoiceYah => "yah",
            VoiceBumb => "bumb",
            VoiceHarsh => "harsh",
            VoiceDamn => "damn",
            VoiceLearned => "learned",
            VoiceTakeThat => "takethat",
            VoiceBaby => "baby",
            VoiceForRock => "forrock",
            VoiceWhoNext => "whonext",
            VoiceBringOn => "bringon",
            VoiceMyTurn => "myturn",
            VoiceOuch => "ouch",
            VoicePaint => "paint",
            VoicePayback => "payback",
            Shimmer => "shimmer",
            Clicked => "clicked",
            Static => "static",
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
            SfxId::VoiceLetsRock,
            SfxId::VoicePartyTime,
            SfxId::VoiceYah,
            SfxId::VoiceBumb,
            SfxId::VoiceHarsh,
            SfxId::VoiceDamn,
            SfxId::VoiceLearned,
            SfxId::VoiceTakeThat,
            SfxId::VoiceBaby,
            SfxId::VoiceForRock,
            SfxId::VoiceWhoNext,
            SfxId::VoiceBringOn,
            SfxId::VoiceMyTurn,
            SfxId::VoiceOuch,
            SfxId::VoicePaint,
            SfxId::VoicePayback,
            SfxId::Shimmer,
            SfxId::Clicked,
            SfxId::Static,
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
    /// Music playback rate, called every pump. 1.0 is normal; the
    /// speaker gag (`SkipMusic`) drags it to [`MUSIC_SLOW_RATE`] —
    /// `CStreamSoundSetFrequency(Music, 7025)` against the stream's
    /// native 22050 Hz, pitch drop included. The game ramps the
    /// recovery, so sinks just apply whatever rate arrives.
    fn set_music_rate(&mut self, rate: f32);
    /// Play a voice line on the single voice channel, stopping any
    /// still-playing previous line ("don't play two at once" —
    /// `PausedSoundPlayer`).
    fn play_voice(&mut self, sfx: SfxId);
    /// The Config Sound sliders, 0.0..1.0 fractions applied on top of
    /// the built-in headroom (`GlobalSetVolume` for everything but the
    /// stream, `CStreamSoundSetVolume` for the music). Called every
    /// pump; sinks may early-out on unchanged values.
    fn set_volumes(&mut self, _master: f32, _music: f32) {}
}

/// `PausedSoundPlayer`: one pending voice slot with a countdown —
/// a newer request replaces whatever was waiting, and playback stops
/// the previous line. `take_due` ticks once per 30 Hz beat, exactly
/// where the original called `PausePlayerUpdate` (top of the
/// per-update loop).
#[derive(Default)]
pub struct VoicePlayer {
    pending: Option<SfxId>,
    pause: u32,
}

impl VoicePlayer {
    pub fn new() -> Self {
        Self::default()
    }

    /// `PausePlayerPlay`.
    pub fn play(&mut self, sfx: SfxId, pause: u32) {
        self.pending = Some(sfx);
        self.pause = pause;
    }

    /// `PausePlayerUpdate` — count the pending line down; returns it
    /// on the beat it comes due (the caller hands it to the sink).
    pub fn take_due(&mut self) -> Option<SfxId> {
        let sfx = self.pending?;
        if self.pause > 0 {
            self.pause -= 1;
            if self.pause == 0 {
                self.pending = None;
                return Some(sfx);
            }
        }
        None
    }
}

/// Voice delays, in 30 Hz beats (`PLAYHURTPAUSE`, `PLAYCARNAGEPAUSE`,
/// the KillPlayer/AddPlayer/GetGoody literals).
pub const HURT_PAUSE: u32 = 45;
pub const CARNAGE_PAUSE: u32 = 30;
pub const DEAD_PAUSE: u32 = 30;
pub const NEW_SHIP_PAUSE: u32 = 5;
pub const GOODY_VOICE_PAUSE: u32 = 5;

/// The `SkipMusic` playback rate: 7025 / 22050.
pub const MUSIC_SLOW_RATE: f32 = 7025.0 / 22050.0;
/// Recovery ramp per audio pump (~60/s): back to full speed in about
/// a second. The original popped straight back to 22050 Hz — the
/// spin-up is our one deliberate polish on the gag.
pub const MUSIC_RAMP_STEP: f32 = (1.0 - MUSIC_SLOW_RATE) / 60.0;

/// Drain a frame's events into the sink. `local_rand` drives the
/// voice picks exactly like the original's LocalRand calls at the
/// trigger sites (visual/audio RNG — not part of the synced stream);
/// voice lines go through `voice` (`PausedSoundPlayer`) with their
/// original delays.
pub fn dispatch(
    events: &mut Events,
    sink: &mut dyn AudioSink,
    local_rand: &mut Rand,
    voice: &mut VoicePlayer,
) {
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
            GameEvent::SfxShimmer => sink.play(SfxId::Shimmer, 0),
            GameEvent::SfxClicked => sink.play(SfxId::Clicked, 0),
            GameEvent::SfxStatic => sink.play(SfxId::Static, 0),
            GameEvent::GoodyCollected { kind } => {
                if local_rand.rand(4) != 0 {
                    sink.play(SfxId::Goody, 0);
                } else {
                    let line = match kind {
                        GoodyKind::Shield | GoodyKind::Health => SfxId::VoiceBitchen,
                        GoodyKind::Rapid => SfxId::VoiceHose,
                        GoodyKind::Gun1 => SfxId::VoiceStick,
                        GoodyKind::Gun2 => SfxId::VoiceSugar,
                        GoodyKind::Bomb => SfxId::VoiceTKill,
                        GoodyKind::Spread => SfxId::VoiceKickAss,
                        GoodyKind::Freeman => SfxId::VoiceAhYah,
                    };
                    voice.play(line, GOODY_VOICE_PAUSE);
                }
            }
            GameEvent::VoiceHurt => {
                voice.play(HURT_VOICES[local_rand.rand(4) as usize], HURT_PAUSE);
            }
            GameEvent::VoiceCarnage => {
                voice.play(CARNAGE_VOICES[local_rand.rand(5) as usize], CARNAGE_PAUSE);
            }
            GameEvent::VoiceDead => {
                voice.play(DEAD_VOICES[local_rand.rand(4) as usize], DEAD_PAUSE);
            }
            GameEvent::VoiceNewShip => {
                voice.play(NEW_SHIP_VOICES[local_rand.rand(3) as usize], NEW_SHIP_PAUSE);
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
        voices: Vec<SfxId>,
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
        fn set_music_rate(&mut self, _rate: f32) {}
        fn play_voice(&mut self, sfx: SfxId) {
            self.voices.push(sfx);
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
        let mut vp = VoicePlayer::new();
        dispatch(&mut ev, &mut rec, &mut lr, &mut vp);
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
        let mut vp = VoicePlayer::new();
        for _ in 0..40 {
            let mut ev = Events::new();
            ev.push(GameEvent::GoodyCollected {
                kind: GoodyKind::Rapid,
            });
            dispatch(&mut ev, &mut rec, &mut lr, &mut vp);
            // Flush the paused voice slot (5-beat delay).
            for _ in 0..GOODY_VOICE_PAUSE {
                if let Some(sfx) = vp.take_due() {
                    rec.play_voice(sfx);
                }
            }
        }
        let jingles = rec.plays.iter().filter(|(s, _)| *s == SfxId::Goody).count();
        let voices = rec
            .voices
            .iter()
            .filter(|&&s| s == SfxId::VoiceHose)
            .count();
        assert_eq!(jingles + voices, 40);
        assert!(jingles > voices, "3-in-4 jingle bias");
    }

    #[test]
    fn voice_player_delays_and_replaces() {
        let mut vp = VoicePlayer::new();
        vp.play(SfxId::VoiceOuch, 3);
        assert_eq!(vp.take_due(), None);
        assert_eq!(vp.take_due(), None);
        // A newer line replaces the pending one ("don't play two").
        vp.play(SfxId::VoiceBumb, 2);
        assert_eq!(vp.take_due(), None);
        assert_eq!(vp.take_due(), Some(SfxId::VoiceBumb));
        assert_eq!(vp.take_due(), None, "slot cleared after playing");
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
