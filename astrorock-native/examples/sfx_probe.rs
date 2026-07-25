//! Diagnostic: verify every embedded SFX decodes to real samples and
//! that a detached sink actually consumes them in real time. Run with
//! `cargo run -p astrorock-native --example sfx_probe`.

use std::io::Cursor;
use std::time::{Duration, Instant};

use astrorock_core::audio::SfxId;
use rodio::source::Source;
use rodio::{Decoder, OutputStream, Sink};

fn main() {
    // 1) Data path: does each mp3 decode, and does the BUFFERED CLONE
    //    (the exact object the game plays) yield samples?
    for &id in SfxId::all() {
        match Decoder::new(Cursor::new(id.bytes())) {
            Ok(decoder) => {
                let rate = decoder.sample_rate();
                let channels = decoder.channels();
                let buffered = decoder.buffered();
                let n = buffered.clone().count();
                let secs = n as f64 / rate as f64 / channels as f64;
                println!(
                    "{:>8}: {n:>7} samples  {rate} Hz x{channels}  ({secs:.2}s)",
                    id.stem()
                );
            }
            Err(err) => println!("{:>8}: DECODE ERROR {err}", id.stem()),
        }
    }

    // 2) Device path: play one SFX exactly like RodioAudio::play and
    //    time it. Real playback takes ~= the clip duration; a broken
    //    path returns instantly.
    let Ok((_stream, handle)) = OutputStream::try_default() else {
        println!("no output device");
        return;
    };
    let buffered = Decoder::new(Cursor::new(SfxId::BigExplosion.bytes()))
        .expect("explo01 decodes")
        .buffered();
    // Prime the cache like a prior play would have.
    let expected = buffered.clone().count() as f64
        / buffered.clone().sample_rate() as f64
        / buffered.clone().channels() as f64;
    let sink = Sink::try_new(&handle).expect("sink");
    let start = Instant::now();
    sink.append(buffered.clone());
    sink.sleep_until_end();
    let took = start.elapsed();
    println!("playback of explo01: expected ~{expected:.2}s, took {took:.2?}");
    // Give the mixer a beat before dropping the stream.
    std::thread::sleep(Duration::from_millis(100));
}
