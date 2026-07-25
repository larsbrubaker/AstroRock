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
        let (stream, handle) = OutputStream::try_default().ok()?;
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
            sink.append(buf.clone());
            sink.detach();
        }
    }

    fn set_loop(&mut self, which: LoopKind, active: bool) {
        let sink = self.loops.entry(which).or_insert_with(|| {
            let sink = Sink::try_new(&self.handle).expect("loop sink");
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
                Ok(Ok(decoder)) => sink.append(decoder),
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
