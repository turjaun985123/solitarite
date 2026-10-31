//! Synthesized, layered sound effects.
//!
//! A single worker thread owns the `rodio` output stream. Every effect is
//! rendered once into a PCM buffer at startup (card slides, snaps, flicks,
//! bells, a riffle shuffle and a victory arpeggio), then mixed concurrently
//! with `play_raw`, so quick sequences of moves overlap instead of queueing
//! behind each other. UI code only enqueues a tiny message; it never touches
//! audio hardware. If no output device exists, playback silently does nothing.
//!
//! Each play call applies a master volume and a small random pitch variation
//! so repeated actions do not sound mechanical.

use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::{
    OnceLock,
    mpsc::{self, SyncSender, TrySendError},
};
use std::thread;

use rodio::{OutputStream, Source, buffer::SamplesBuffer};

const SAMPLE_RATE: u32 = 44_100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sound {
    /// Stock card sliding over to the waste.
    Deal,
    /// Whole waste pile returning to the stock.
    Recycle,
    /// A dealt hand: riffle of many quick flicks.
    Shuffle,
    /// Card lands on the tableau.
    Place,
    /// Card lands on a foundation.
    Foundation,
    /// Face-down card turns over.
    Flip,
    /// Card lifted by the mouse.
    Pickup,
    /// A rejected move or a card returning home.
    Reject,
    Undo,
    Victory,
}

impl Sound {
    const ALL: [Sound; 10] = [
        Sound::Deal,
        Sound::Recycle,
        Sound::Shuffle,
        Sound::Place,
        Sound::Foundation,
        Sound::Flip,
        Sound::Pickup,
        Sound::Reject,
        Sound::Undo,
        Sound::Victory,
    ];

    /// How much pitch jitter feels natural for this effect (fraction of rate).
    fn jitter(self) -> f32 {
        match self {
            Sound::Victory | Sound::Foundation => 0.0,
            Sound::Shuffle | Sound::Recycle => 0.015,
            _ => 0.045,
        }
    }
}

struct Command {
    sound: Sound,
    volume: f32,
}

static AUDIO_TX: OnceLock<Option<SyncSender<Command>>> = OnceLock::new();

/// Queue a sound at `volume` (0.0–1.0) without blocking. Dropped when audio is
/// unavailable, the volume is zero, or the queue is full.
pub fn play(sound: Sound, volume: f32) {
    let volume = volume.clamp(0.0, 1.0);
    if volume <= 0.001 {
        return;
    }
    let sender = AUDIO_TX.get_or_init(start_worker);
    if let Some(sender) = sender {
        match sender.try_send(Command { sound, volume }) {
            Ok(()) | Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
        }
    }
}

/// Warm the worker up (builds all buffers and opens the device) off the UI
/// thread, so the very first card sound has no latency.
pub fn warm_up() {
    let _ = AUDIO_TX.get_or_init(start_worker);
}

fn start_worker() -> Option<SyncSender<Command>> {
    let (sender, receiver) = mpsc::sync_channel::<Command>(32);
    let worker = thread::Builder::new()
        .name("solitarite-audio".into())
        .spawn(move || {
            let buffers: HashMap<Sound, Vec<f32>> = Sound::ALL
                .iter()
                .map(|&sound| (sound, render(sound)))
                .collect();
            let Ok((_stream, handle)) = OutputStream::try_default() else {
                return;
            };
            let mut seed = 0x9e37_79b9u32;
            while let Ok(command) = receiver.recv() {
                let Some(samples) = buffers.get(&command.sound) else {
                    continue;
                };
                seed = lcg(seed);
                let unit = (seed >> 8) as f32 / (1u32 << 24) as f32; // 0..1
                let jitter = command.sound.jitter();
                let rate = (SAMPLE_RATE as f32 * (1.0 + (unit * 2.0 - 1.0) * jitter)) as u32;
                let source = SamplesBuffer::new(1, rate, samples.clone())
                    .amplify(command.volume * command.volume.sqrt());
                let _ = handle.play_raw(source);
            }
        });
    worker.ok().map(|_| sender)
}

// ---------------------------------------------------------------------------
// Synthesis
// ---------------------------------------------------------------------------

fn lcg(state: u32) -> u32 {
    state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223)
}

struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 = lcg(self.0);
        ((self.0 >> 8) as f32 / (1u32 << 23) as f32) - 1.0
    }
}

fn samples_for(seconds: f32) -> usize {
    (seconds * SAMPLE_RATE as f32) as usize
}

/// Add `src` into `dst` starting at `offset`, growing `dst` as needed.
fn mix_into(dst: &mut Vec<f32>, src: &[f32], offset: usize, gain: f32) {
    if dst.len() < offset + src.len() {
        dst.resize(offset + src.len(), 0.0);
    }
    for (i, s) in src.iter().enumerate() {
        dst[offset + i] += s * gain;
    }
}

/// Exponentially decaying sine with a sharp attack.
fn tone(freq: f32, seconds: f32, decay: f32, gain: f32) -> Vec<f32> {
    let n = samples_for(seconds);
    (0..n)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE as f32;
            let attack = (t / 0.002).min(1.0);
            (2.0 * PI * freq * t).sin() * (-t * decay).exp() * attack * gain
        })
        .collect()
}

/// A sine whose frequency glides from `from` to `to`.
fn sweep(from: f32, to: f32, seconds: f32, decay: f32, gain: f32) -> Vec<f32> {
    let n = samples_for(seconds);
    let mut phase = 0.0f32;
    (0..n)
        .map(|i| {
            let p = i as f32 / n as f32;
            let t = i as f32 / SAMPLE_RATE as f32;
            let freq = from + (to - from) * p;
            phase += 2.0 * PI * freq / SAMPLE_RATE as f32;
            let attack = (t / 0.002).min(1.0);
            phase.sin() * (-t * decay).exp() * attack * gain
        })
        .collect()
}

/// Band-limited noise "paper" texture: a one-pole low-pass whose cutoff
/// sweeps from `start_cut` to `end_cut` (0..1, larger is brighter) with a
/// shaped envelope. This is the core of every card slide/flick.
fn paper(seconds: f32, start_cut: f32, end_cut: f32, attack: f32, decay: f32, gain: f32, seed: u32) -> Vec<f32> {
    let n = samples_for(seconds);
    let mut noise = Noise(seed);
    let mut low = 0.0f32;
    let mut prev = 0.0f32;
    (0..n)
        .map(|i| {
            let p = i as f32 / n as f32;
            let t = i as f32 / SAMPLE_RATE as f32;
            let cut = start_cut + (end_cut - start_cut) * p;
            low += cut * (noise.next() - low);
            // High-pass the low-passed noise to remove rumble => "swish".
            let band = low - prev * 0.9;
            prev = low;
            let env = (t / attack.max(0.0005)).min(1.0) * (-t * decay).exp();
            band * env * gain
        })
        .collect()
}

fn soft_clip(buffer: &mut [f32]) {
    for s in buffer.iter_mut() {
        *s = s.tanh() * 0.95;
    }
}

fn fade_out(buffer: &mut [f32], seconds: f32) {
    let n = samples_for(seconds).min(buffer.len());
    let len = buffer.len();
    for i in 0..n {
        buffer[len - 1 - i] *= i as f32 / n as f32;
    }
}

fn render(sound: Sound) -> Vec<f32> {
    let mut out: Vec<f32> = Vec::new();
    match sound {
        Sound::Deal => {
            // Card sliding off the stock: airy swish with a soft tick at the end.
            mix_into(&mut out, &paper(0.16, 0.10, 0.42, 0.012, 14.0, 5.0, 11), 0, 0.55);
            mix_into(&mut out, &tone(1100.0, 0.03, 160.0, 0.5), samples_for(0.012), 0.10);
        }
        Sound::Recycle => {
            for k in 0..5 {
                let seed = 100 + k as u32 * 17;
                mix_into(
                    &mut out,
                    &paper(0.12, 0.12, 0.36, 0.008, 18.0, 5.0, seed),
                    samples_for(0.045 * k as f32),
                    0.40,
                );
            }
        }
        Sound::Shuffle => {
            // A riffle: many tiny flicks with rising density.
            let mut t = 0.0f32;
            let mut k = 0u32;
            while t < 0.62 {
                let seed = 300 + k * 31;
                mix_into(
                    &mut out,
                    &paper(0.05, 0.18, 0.5, 0.003, 60.0, 5.0, seed),
                    samples_for(t),
                    0.34,
                );
                t += 0.052 - 0.028 * (t / 0.62);
                k += 1;
            }
        }
        Sound::Place => {
            // Card snapping onto felt: low thump + crisp paper click.
            mix_into(&mut out, &sweep(190.0, 78.0, 0.11, 34.0, 0.9), 0, 0.85);
            mix_into(&mut out, &paper(0.045, 0.55, 0.30, 0.0008, 90.0, 6.0, 7), 0, 0.55);
            mix_into(&mut out, &tone(2400.0, 0.012, 380.0, 0.5), 0, 0.12);
        }
        Sound::Foundation => {
            // Snap plus a warm bell: the "success" voice of the game.
            mix_into(&mut out, &sweep(210.0, 90.0, 0.09, 40.0, 0.8), 0, 0.6);
            mix_into(&mut out, &paper(0.04, 0.55, 0.30, 0.0008, 100.0, 6.0, 9), 0, 0.4);
            for (i, (freq, gain)) in [(1318.5f32, 0.40f32), (1976.0, 0.20), (2637.0, 0.10)]
                .iter()
                .enumerate()
            {
                mix_into(
                    &mut out,
                    &tone(*freq, 0.55, 7.0 + i as f32 * 3.0, 1.0),
                    samples_for(0.02),
                    *gain,
                );
            }
        }
        Sound::Flip => {
            // Quick flick with a rising edge, like thumbing a card over.
            mix_into(&mut out, &paper(0.085, 0.14, 0.60, 0.004, 30.0, 5.0, 21), 0, 0.55);
            mix_into(&mut out, &sweep(520.0, 980.0, 0.05, 55.0, 0.5), 0, 0.12);
        }
        Sound::Pickup => {
            mix_into(&mut out, &paper(0.04, 0.30, 0.52, 0.002, 80.0, 5.0, 33), 0, 0.35);
            mix_into(&mut out, &tone(760.0, 0.035, 130.0, 0.5), 0, 0.10);
        }
        Sound::Reject => {
            mix_into(&mut out, &sweep(150.0, 92.0, 0.16, 20.0, 0.9), 0, 0.7);
            mix_into(&mut out, &paper(0.05, 0.30, 0.18, 0.001, 70.0, 5.0, 41), 0, 0.30);
        }
        Sound::Undo => {
            // Reversed swish: swells in, then taps out.
            let mut swish = paper(0.14, 0.42, 0.10, 0.002, 12.0, 5.0, 55);
            swish.reverse();
            mix_into(&mut out, &swish, 0, 0.5);
            mix_into(&mut out, &sweep(420.0, 310.0, 0.08, 40.0, 0.6), samples_for(0.10), 0.22);
        }
        Sound::Victory => {
            // Bright major arpeggio of bell tones with a shimmering tail.
            let notes = [523.25f32, 659.25, 783.99, 1046.5, 1318.5, 1568.0, 2093.0];
            for (i, freq) in notes.iter().enumerate() {
                let at = samples_for(0.085 * i as f32);
                mix_into(&mut out, &tone(*freq, 1.1, 4.2, 1.0), at, 0.30);
                mix_into(&mut out, &tone(freq * 2.0, 0.7, 7.0, 1.0), at, 0.12);
                mix_into(&mut out, &tone(freq * 3.0, 0.4, 12.0, 1.0), at, 0.05);
            }
            for k in 0..9 {
                let at = samples_for(0.55 + 0.07 * k as f32);
                let freq = 2093.0 * (1.0 + (k % 3) as f32 * 0.25);
                mix_into(&mut out, &tone(freq, 0.35, 11.0, 1.0), at, 0.06);
            }
        }
    }
    soft_clip(&mut out);
    fade_out(&mut out, 0.01);
    out
}

#[cfg(test)]
mod tests {
    use super::{Sound, render};

    #[test]
    fn every_sound_renders_audible_bounded_samples() {
        for sound in Sound::ALL {
            let samples = render(sound);
            assert!(samples.len() > 200, "{sound:?} is too short");
            let peak = samples.iter().fold(0.0f32, |peak, s| peak.max(s.abs()));
            assert!(peak > 0.02, "{sound:?} is nearly silent (peak {peak})");
            assert!(peak <= 1.0, "{sound:?} clips (peak {peak})");
            assert!(samples.iter().all(|s| s.is_finite()));
        }
    }
}
