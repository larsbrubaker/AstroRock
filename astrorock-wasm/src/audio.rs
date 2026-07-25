//! # Wasm audio sink — WebAudio
//!
//! Implements [`AudioSink`] on the browser's `AudioContext`. The
//! embedded SFX mp3s (from `astrorock_core::audio`) are handed to
//! `decodeAudioData` at startup; decoding is async, so buffers fill in
//! over the first few hundred ms — `play` silently skips anything not
//! decoded yet (the title screen is quiet anyway).
//!
//! Browsers create `AudioContext`s suspended until a user gesture, so
//! every play/loop attempt nudges `resume()` — the first Enter press
//! that starts a game unlocks sound. Pan is accepted but not yet
//! spatialized (todo.md, same as native).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use astrorock_core::audio::{AudioSink, LoopKind, SfxId, MUSIC_TRACKS};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AudioBuffer, AudioBufferSourceNode, AudioContext, AudioContextState, GainNode, HtmlAudioElement,
};

/// Mix headroom: at full volume, music + a full-scale SFX sums past
/// 1.0 and the destination clamp squashes the music while the effect
/// plays. Same constants as the native sink.
const MUSIC_VOLUME: f64 = 0.45;
const SFX_VOLUME: f32 = 0.85;

type SfxBuffers = Rc<RefCell<HashMap<SfxId, AudioBuffer>>>;
type LoopBuffers = Rc<RefCell<HashMap<LoopKind, AudioBuffer>>>;

pub struct WebAudio {
    ctx: AudioContext,
    /// All SFX route through one gain node for mix headroom.
    sfx_gain: GainNode,
    sfx: SfxBuffers,
    loop_bufs: LoopBuffers,
    active: HashMap<LoopKind, AudioBufferSourceNode>,
    /// The soundtrack streams through a media element (tracks are
    /// megabytes — never decoded into AudioBuffers). `None` when the
    /// browser refused the element; the game just runs without music.
    music: Option<HtmlAudioElement>,
    /// Reusable no-op rejection handler: `play()` before the autoplay
    /// gate lifts rejects, and an unhandled rejection spams the
    /// console every frame.
    swallow: Closure<dyn FnMut(JsValue)>,
}

/// Relative to the page, staged by demo/sync-assets.ts.
fn music_src(track: usize) -> String {
    format!("music/{}.mp3", MUSIC_TRACKS[track])
}

impl WebAudio {
    /// `None` when the browser refuses an `AudioContext` entirely.
    pub fn new() -> Option<Self> {
        let ctx = AudioContext::new().ok()?;
        let sfx_gain = ctx.create_gain().ok()?;
        sfx_gain.gain().set_value(SFX_VOLUME);
        sfx_gain.connect_with_audio_node(&ctx.destination()).ok()?;
        let sfx: SfxBuffers = Rc::new(RefCell::new(HashMap::new()));
        let loop_bufs: LoopBuffers = Rc::new(RefCell::new(HashMap::new()));

        for &id in SfxId::all() {
            let ctx = ctx.clone();
            let sfx = sfx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match decode(&ctx, id.bytes()).await {
                    Ok(buf) => {
                        sfx.borrow_mut().insert(id, buf);
                    }
                    Err(err) => warn_decode(id.stem(), &err),
                }
            });
        }
        for &which in LoopKind::all() {
            let ctx = ctx.clone();
            let loop_bufs = loop_bufs.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match decode(&ctx, which.bytes()).await {
                    Ok(buf) => {
                        loop_bufs.borrow_mut().insert(which, buf);
                    }
                    Err(err) => warn_decode("loop", &err),
                }
            });
        }

        let music = HtmlAudioElement::new_with_src(&music_src(0)).ok();
        if let Some(el) = &music {
            el.set_volume(MUSIC_VOLUME);
            // Advance through MUSIC_TRACKS and wrap — the equivalent of
            // the original's one concatenated stream restarting itself.
            let track = Rc::new(Cell::new(0usize));
            let el2 = el.clone();
            let on_ended = Closure::<dyn FnMut()>::new(move || {
                let next = (track.get() + 1) % MUSIC_TRACKS.len();
                track.set(next);
                el2.set_src(&music_src(next));
                let _ = el2.play();
            });
            el.set_onended(Some(on_ended.as_ref().unchecked_ref()));
            // The element lives as long as the app — leak the handler.
            on_ended.forget();
        }

        Some(Self {
            ctx,
            sfx_gain,
            sfx,
            loop_bufs,
            active: HashMap::new(),
            music,
            swallow: Closure::new(|_| {}),
        })
    }

    /// Autoplay policy leaves the context suspended until a gesture;
    /// resuming is idempotent and cheap, so nudge it on every attempt.
    fn ensure_running(&self) {
        if self.ctx.state() == AudioContextState::Suspended {
            let _ = self.ctx.resume();
        }
    }

    fn start_source(&self, buf: &AudioBuffer, looping: bool) -> Option<AudioBufferSourceNode> {
        let node = self.ctx.create_buffer_source().ok()?;
        node.set_buffer(Some(buf));
        node.set_loop(looping);
        node.connect_with_audio_node(&self.sfx_gain).ok()?;
        node.start().ok()?;
        Some(node)
    }
}

impl AudioSink for WebAudio {
    fn play(&mut self, sfx: SfxId, _pan: i32) {
        self.ensure_running();
        if let Some(buf) = self.sfx.borrow().get(&sfx) {
            // One-shots are fire-and-forget: the node garbage-collects
            // itself once playback ends.
            self.start_source(buf, false);
        }
    }

    fn set_loop(&mut self, which: LoopKind, active: bool) {
        if !active {
            if let Some(node) = self.active.remove(&which) {
                // `stop` lives on the parent interface these days; the
                // AudioBufferSourceNode inherent copy is deprecated.
                let _ = web_sys::AudioScheduledSourceNode::stop(&node);
                let _ = node.disconnect();
            }
            return;
        }
        if self.active.contains_key(&which) {
            return;
        }
        self.ensure_running();
        // Called every frame while the state holds, so a buffer still
        // decoding simply starts a frame or two late.
        let started = self
            .loop_bufs
            .borrow()
            .get(&which)
            .and_then(|buf| self.start_source(buf, true));
        if let Some(node) = started {
            self.active.insert(which, node);
        }
    }

    fn set_music(&mut self, on: bool) {
        let Some(el) = &self.music else { return };
        if !on {
            if !el.paused() {
                el.pause().ok();
            }
            return;
        }
        self.ensure_running();
        // Media playback obeys the same user-activation rule as the
        // AudioContext — once the context runs, play() is allowed.
        if self.ctx.state() == AudioContextState::Running && el.paused() {
            if let Ok(promise) = el.play() {
                let _ = promise.catch(&self.swallow);
            }
        }
    }
}

async fn decode(ctx: &AudioContext, bytes: &[u8]) -> Result<AudioBuffer, JsValue> {
    // Uint8Array::from copies into a fresh, exactly-sized ArrayBuffer,
    // so handing the whole backing buffer to decodeAudioData is safe.
    let array = js_sys::Uint8Array::from(bytes).buffer();
    let promise = ctx.decode_audio_data(&array)?;
    JsFuture::from(promise).await?.dyn_into::<AudioBuffer>()
}

fn warn_decode(what: &str, err: &JsValue) {
    web_sys::console::warn_2(&format!("audio: decode {what} failed").into(), err);
}
