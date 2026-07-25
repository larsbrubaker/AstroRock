//! # Native audio sink — rodio
//!
//! Implements [`AudioSink`] with rodio: every SFX mp3 (embedded by
//! `astrorock_core::audio`) is decoded once into a buffered source at
//! startup; one-shots spawn detached sinks, loops hold paused sinks
//! toggled by state. Pan is accepted but not yet spatialized (todo.md
//! — the original panned by screen x).

use std::collections::HashMap;
use std::io::Cursor;

use astrorock_core::audio::{AudioSink, LoopKind, SfxId};
use rodio::source::{Buffered, Source};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

type Sfx = Buffered<Decoder<Cursor<&'static [u8]>>>;

pub struct RodioAudio {
    // Dropping the stream stops all audio — keep it alive.
    _stream: OutputStream,
    handle: OutputStreamHandle,
    buffers: HashMap<SfxId, Sfx>,
    loops: HashMap<LoopKind, Sink>,
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
        })
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
}
