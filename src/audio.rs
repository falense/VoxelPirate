use bevy::audio::{AudioSource, Volume};
use bevy::prelude::*;

const SAMPLE_RATE: u32 = 22_050;

/// Procedurally synthesized sound effects, built once at startup. No asset
/// files: each sound is a WAV rendered into memory.
#[derive(Resource)]
pub struct SoundBank {
    pub boom: Handle<AudioSource>,
    pub crunch: Handle<AudioSource>,
    pub splash: Handle<AudioSource>,
    pub ding: Handle<AudioSource>,
    pub fanfare: Handle<AudioSource>,
}

pub fn setup_sounds(mut commands: Commands, mut audio: ResMut<Assets<AudioSource>>) {
    commands.insert_resource(SoundBank {
        boom: audio.add(render_boom()),
        crunch: audio.add(render_crunch()),
        splash: audio.add(render_splash()),
        ding: audio.add(render_ding()),
        fanfare: audio.add(render_fanfare()),
    });
}

/// Fire-and-forget playback; the audio entity despawns when done.
pub fn play(commands: &mut Commands, source: &Handle<AudioSource>, volume: f32) {
    commands.spawn((
        AudioPlayer(source.clone()),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)),
    ));
}

/// Cannon shot: a deep sine thump under heavily low-passed noise.
fn render_boom() -> AudioSource {
    let mut rng = Rng(0x9E3779B9);
    let samples = render(0.6, |t, i| {
        let envelope = (-t * 7.0).exp();
        let rumble = (t * 55.0 * std::f32::consts::TAU).sin() * 0.6;
        let noise = rng.next_f32() * 0.5;
        let _ = i;
        (rumble + noise) * envelope
    });
    wav(low_pass(samples, 14))
}

/// Timbers shattering: a short bright noise burst.
fn render_crunch() -> AudioSource {
    let mut rng = Rng(0xDEADBEEF);
    let samples = render(0.28, |t, _| rng.next_f32() * (-t * 16.0).exp());
    wav(low_pass(samples, 3))
}

/// Cannonball splashdown: softer noise with a slower decay.
fn render_splash() -> AudioSource {
    let mut rng = Rng(0xC0FFEE11);
    let samples = render(0.45, |t, _| rng.next_f32() * (-t * 7.0).exp() * 0.6);
    wav(low_pass(samples, 6))
}

/// Salvage pickup: a two-partial bell blip.
fn render_ding() -> AudioSource {
    let samples = render(0.25, |t, _| {
        let envelope = (-t * 12.0).exp();
        ((t * 880.0 * std::f32::consts::TAU).sin() * 0.5
            + (t * 1320.0 * std::f32::consts::TAU).sin() * 0.25)
            * envelope
    });
    wav(samples)
}

/// Upgrade fanfare: an ascending major arpeggio.
fn render_fanfare() -> AudioSource {
    let notes = [523.25_f32, 659.25, 783.99];
    let samples = render(0.8, |t, _| {
        let step = (t / 0.22).min(2.99) as usize;
        let local = t - step as f32 * 0.22;
        let envelope = (-local * 6.0).exp().min(1.0);
        (local * notes[step.min(2)] * std::f32::consts::TAU).sin() * 0.5 * envelope
    });
    wav(samples)
}

fn render(seconds: f32, mut f: impl FnMut(f32, usize) -> f32) -> Vec<f32> {
    let count = (seconds * SAMPLE_RATE as f32) as usize;
    (0..count)
        .map(|i| f(i as f32 / SAMPLE_RATE as f32, i))
        .collect()
}

/// Box-filter low pass: averages a sliding window, which is plenty to take
/// the hiss off white noise.
fn low_pass(samples: Vec<f32>, window: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(samples.len());
    let mut sum = 0.0;
    for i in 0..samples.len() {
        sum += samples[i];
        if i >= window {
            sum -= samples[i - window];
        }
        out.push(sum / window as f32);
    }
    out
}

/// Pack mono f32 samples into an in-memory 16-bit WAV.
fn wav(samples: Vec<f32>) -> AudioSource {
    let data_len = (samples.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + samples.len() * 2);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes()); // PCM chunk size
    bytes.extend_from_slice(&1_u16.to_le_bytes()); // PCM format
    bytes.extend_from_slice(&1_u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    bytes.extend_from_slice(&2_u16.to_le_bytes()); // block align
    bytes.extend_from_slice(&16_u16.to_le_bytes()); // bits per sample
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        let quantized = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        bytes.extend_from_slice(&quantized.to_le_bytes());
    }
    AudioSource {
        bytes: bytes.into(),
    }
}

/// Xorshift noise source so sound synthesis needs no rand crate.
struct Rng(u32);

impl Rng {
    fn next_f32(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}
