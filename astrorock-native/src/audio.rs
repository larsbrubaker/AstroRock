//! # Native audio sink — rodio
//!
//! Implements [`AudioSink`] with rodio: every SFX mp3 (embedded by
//! `astrorock_core::audio`) is decoded once into a buffered source at
//! startup; one-shots spawn detached sinks, loops hold paused sinks
//! toggled by state. Pan is accepted but not yet spatialized (todo.md
//! — the original panned by screen x).

use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;

use astrorock_core::audio::{AudioSink, LoopKind, SfxId, MUSIC_TRACKS};
use rodio::source::{Buffered, Source};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

/// Mix headroom: at full volume, music + a full-scale SFX sums past
/// 1.0 and the output clamp squashes the music for the duration of
/// the effect (audible as the soundtrack "ducking"). Keeping the
/// steady-state music well under the ceiling leaves room for effects
/// to land on top without clipping.
const MUSIC_VOLUME: f32 = 0.45;
const SFX_VOLUME: f32 = 0.85;

type Sfx = Buffered<Decoder<Cursor<&'static [u8]>>>;

/// Where the music mp3s live — probed once, then remembered.
enum MusicDir {
    Unprobed,
    Found(PathBuf),
    /// Not found (or a track failed): run without music, silently.
    Missing,
}

pub struct RodioAudio {
    // Dropping the stream stops all audio — keep it alive.
    _stream: OutputStream,
    handle: OutputStreamHandle,
    buffers: HashMap<SfxId, Sfx>,
    loops: HashMap<LoopKind, Sink>,
    music: Option<Sink>,
    music_dir: MusicDir,
    /// Next index into [`MUSIC_TRACKS`].
    music_track: usize,
}

impl RodioAudio {
    /// `None` when no audio device is available (the game runs silent).
    pub fn new() -> Option<Self> {
        if std::env::var_os("ASTROROCK_AUDIO").is_some() {
            // Diagnostic: what does cpal think the default device is?
            use rodio::cpal::traits::{DeviceTrait, HostTrait};
            let host = rodio::cpal::default_host();
            match host.default_output_device() {
                Some(dev) => {
                    eprintln!(
                        "audio: default device = {:?}",
                        dev.name().unwrap_or_else(|_| "<unnamed>".into())
                    );
                    if let Ok(cfg) = dev.default_output_config() {
                        eprintln!("audio: default config = {cfg:?}");
                    }
                }
                None => eprintln!("audio: NO default output device"),
            }
        }
        let (stream, handle) = OutputStream::try_default().ok()?;
        if std::env::var_os("ASTROROCK_AUDIO").is_some() {
            // A 2s sine straight into the mixer: if this is inaudible,
            // the problem is below rodio (cpal/WASAPI), not our
            // decoders or sink plumbing.
            use rodio::source::SineWave;
            use std::time::Duration;
            let beep = SineWave::new(440.0)
                .take_duration(Duration::from_secs(2))
                .amplify(0.20);
            match handle.play_raw(beep.convert_samples()) {
                Ok(()) => eprintln!("audio: diagnostic beep queued"),
                Err(err) => eprintln!("audio: diagnostic beep FAILED: {err}"),
            }
        }
        let mut buffers = HashMap::new();
        for &id in SfxId::all() {
            match Decoder::new(Cursor::new(id.bytes())) {
                Ok(decoder) => {
                    buffers.insert(id, decoder.buffered());
                }
                Err(err) => {
                    eprintln!("audio: failed to decode {}: {err}", id.stem());
                }
            }
        }
        Some(Self {
            _stream: stream,
            handle,
            buffers,
            loops: HashMap::new(),
            music: None,
            music_dir: MusicDir::Unprobed,
            music_track: 0,
        })
    }

    /// `assets/music/` relative to the cwd (cargo run / cargo dev) or
    /// to the exe (`target/debug` layout, or shipped next to assets).
    fn probe_music_dir() -> Option<PathBuf> {
        let mut candidates = vec![PathBuf::from("assets/music")];
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join("assets/music"));
                candidates.push(dir.join("../../assets/music"));
            }
        }
        candidates.into_iter().find(|p| p.is_dir())
    }
}

impl AudioSink for RodioAudio {
    fn play(&mut self, sfx: SfxId, _pan: i32) {
        if let (Some(buf), Ok(sink)) = (self.buffers.get(&sfx), Sink::try_new(&self.handle)) {
            sink.set_volume(SFX_VOLUME);
            sink.append(buf.clone());
            sink.detach();
        }
    }

    fn set_loop(&mut self, which: LoopKind, active: bool) {
        let sink = self.loops.entry(which).or_insert_with(|| {
            let sink = Sink::try_new(&self.handle).expect("loop sink");
            sink.set_volume(SFX_VOLUME);
            if let Ok(decoder) = Decoder::new(Cursor::new(which.bytes())) {
                sink.append(decoder.buffered().repeat_infinite());
            }
            sink.pause();
            sink
        });
        if active {
            sink.play();
        } else {
            sink.pause();
        }
    }

    fn set_music(&mut self, on: bool) {
        if !on {
            if let Some(sink) = &self.music {
                sink.pause();
            }
            return;
        }
        if let MusicDir::Unprobed = self.music_dir {
            self.music_dir = match Self::probe_music_dir() {
                Some(dir) => MusicDir::Found(dir),
                None => {
                    eprintln!("audio: assets/music not found — running without music");
                    MusicDir::Missing
                }
            };
        }
        let MusicDir::Found(dir) = &self.music_dir else {
            return;
        };
        if self.music.is_none() {
            self.music = Sink::try_new(&self.handle).ok();
            if let Some(sink) = &self.music {
                sink.set_volume(MUSIC_VOLUME);
            }
        }
        let Some(sink) = self.music.as_ref() else {
            return;
        };
        // Keep one track playing and one queued so the wrap from
        // track08 back to track02 is seamless, like the original's
        // single concatenated stream.
        if sink.len() < 2 {
            let path = dir.join(format!("{}.mp3", MUSIC_TRACKS[self.music_track]));
            self.music_track = (self.music_track + 1) % MUSIC_TRACKS.len();
            match std::fs::read(&path).map(|bytes| Decoder::new(Cursor::new(bytes))) {
                Ok(Ok(decoder)) => {
                    if std::env::var_os("ASTROROCK_AUDIO").is_some() {
                        eprintln!(
                            "audio: queueing {} (sink len {}, vol {}, paused {})",
                            path.display(),
                            sink.len(),
                            sink.volume(),
                            sink.is_paused()
                        );
                    }
                    sink.append(decoder)
                }
                Ok(Err(err)) => {
                    eprintln!("audio: music decode {} failed: {err}", path.display());
                    self.music_dir = MusicDir::Missing;
                    return;
                }
                Err(err) => {
                    eprintln!("audio: music read {} failed: {err}", path.display());
                    self.music_dir = MusicDir::Missing;
                    return;
                }
            }
        }
        sink.play();
    }
}
