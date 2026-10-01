//! Sound. Every effect and every loop (rain, swamp chorus, music) is
//! synthesized into sample buffers at startup. Effects in the world get
//! quieter with distance from the camera; the music's tension layer swells
//! while hunters are on the bogwight's trail.

use crate::{
    Args,
    hunters::{Hunter, State},
    weather::Storm,
};
use bevy::{
    audio::{AddAudioSource, ChannelCount, PlaybackMode, SampleRate, Source, Volume},
    platform::collections::HashMap,
    prelude::*,
    reflect::TypePath,
};
use std::{f32::consts::TAU, sync::Arc, time::Duration};

const RATE: u32 = 44_100;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sound {
    Splash,
    BigSplash,
    Burst,
    Claw,
    Hit,
    Grab,
    Throw,
    Jump,
    Thud,
    Squelch,
    Boing,
    Hurt,
    Death,
    Scream,
    Gurgle,
    Whistle,
    Huh,
    Twang,
    Thunk,
    Creak,
    Swing,
    Snap,
    Glass,
    Sizzle,
    Capsize,
    Thunder,
    ThunderNear,
    Tick,
    Confirm,
    Back,
    Victory,
    Defeat,
}

const ALL: [Sound; 32] = [
    Sound::Splash,
    Sound::BigSplash,
    Sound::Burst,
    Sound::Claw,
    Sound::Hit,
    Sound::Grab,
    Sound::Throw,
    Sound::Jump,
    Sound::Thud,
    Sound::Squelch,
    Sound::Boing,
    Sound::Hurt,
    Sound::Death,
    Sound::Scream,
    Sound::Gurgle,
    Sound::Whistle,
    Sound::Huh,
    Sound::Twang,
    Sound::Thunk,
    Sound::Creak,
    Sound::Swing,
    Sound::Snap,
    Sound::Glass,
    Sound::Sizzle,
    Sound::Capsize,
    Sound::Thunder,
    Sound::ThunderNear,
    Sound::Tick,
    Sound::Confirm,
    Sound::Back,
    Sound::Victory,
    Sound::Defeat,
];

impl Sound {
    fn volume(self) -> f32 {
        match self {
            Sound::Squelch => 0.35,
            Sound::Huh | Sound::Creak => 0.6,
            Sound::Thunder | Sound::ThunderNear => 1.0,
            _ => 0.8,
        }
    }
}

/// Request to play a sound, optionally at a place in the world.
#[derive(Message, Clone, Copy)]
pub struct Sfx {
    pub sound: Sound,
    pub pos: Option<Vec2>,
}

impl Sfx {
    pub fn at(sound: Sound, pos: Vec2) -> Self {
        Sfx { sound, pos: Some(pos) }
    }

    pub fn ui(sound: Sound) -> Self {
        Sfx { sound, pos: None }
    }
}

/// A mono sample buffer.
#[derive(Asset, TypePath, Clone)]
pub struct Wave {
    samples: Arc<[f32]>,
}

pub struct WaveDecoder {
    samples: Arc<[f32]>,
    pos: usize,
}

impl Iterator for WaveDecoder {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let s = self.samples.get(self.pos).copied();
        self.pos += 1;
        s
    }
}

impl Source for WaveDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(1).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(RATE).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(self.samples.len() as f32 / RATE as f32))
    }
}

impl Decodable for Wave {
    type Decoder = WaveDecoder;
    fn decoder(&self) -> WaveDecoder {
        WaveDecoder {
            samples: self.samples.clone(),
            pos: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Synthesis
// ---------------------------------------------------------------------------

struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / 8_388_608.0 - 1.0
    }
    fn unit(&mut self) -> f32 {
        self.next() * 0.5 + 0.5
    }
}

fn render(secs: f32, mut f: impl FnMut(f32) -> f32) -> Vec<f32> {
    let n = (secs * RATE as f32) as usize;
    (0..n).map(|i| f(i as f32 / RATE as f32).clamp(-1.0, 1.0)).collect()
}

/// A quick rise then an exponential fall.
fn pluck(t: f32, attack: f32, decay: f32) -> f32 {
    if t < 0.0 {
        0.0
    } else {
        (t / attack).min(1.0) * (-t / decay).exp()
    }
}

/// Fades the last `tail` seconds to silence to avoid a click.
fn fade(mut v: Vec<f32>, tail: f32) -> Vec<f32> {
    let n = ((tail * RATE as f32) as usize).min(v.len());
    let len = v.len();
    for i in 0..n {
        v[len - 1 - i] *= i as f32 / n as f32;
    }
    v
}

fn saw(phase: f32) -> f32 {
    2.0 * phase.fract() - 1.0
}

fn note(semitones_from_a4: f32) -> f32 {
    440.0 * 2f32.powf(semitones_from_a4 / 12.0)
}

/// A two-pole state-variable filter; returns the band-pass output.
#[derive(Default)]
struct Svf {
    low: f32,
    band: f32,
}

impl Svf {
    fn band(&mut self, x: f32, freq: f32, q: f32) -> f32 {
        let f = 2.0 * (std::f32::consts::PI * freq / RATE as f32).sin();
        let high = x - self.low - q * self.band;
        self.band += f * high;
        self.low += f * self.band;
        self.band
    }
}

/// Noise through a low-pass whose cutoff moves from `c0` to `c1` (0..1).
fn whoosh(secs: f32, c0: f32, c1: f32, vol: f32, decay: f32, seed: u32) -> Vec<f32> {
    let mut noise = Noise(seed);
    let mut lp = 0.0f32;
    fade(
        render(secs, |t| {
            let c = c0 + (c1 - c0) * (t / secs);
            lp += (noise.next() - lp) * c;
            lp * vol * pluck(t, 0.01, decay) * 3.0
        }),
        0.03,
    )
}

/// A deep thump that drops in pitch.
fn thud(secs: f32, f0: f32, f1: f32, vol: f32, decay: f32) -> Vec<f32> {
    let mut phase = 0.0f32;
    fade(
        render(secs, |t| {
            let f = f1 + (f0 - f1) * (-t * 18.0).exp();
            phase += f / RATE as f32;
            (phase * TAU).sin() * vol * pluck(t, 0.002, decay)
        }),
        0.03,
    )
}

/// A sine sweep, like a bloop.
fn bloop(secs: f32, f0: f32, f1: f32, vol: f32, decay: f32) -> Vec<f32> {
    let mut phase = 0.0f32;
    fade(
        render(secs, |t| {
            let k = t / secs;
            let f = f0 * (f1 / f0).powf(k);
            phase += f / RATE as f32;
            (phase * TAU).sin() * vol * pluck(t, 0.003, decay)
        }),
        0.02,
    )
}

/// Random rising bubble blips.
fn bubbles(secs: f32, count: usize, vol: f32, seed: u32) -> Vec<f32> {
    let mut noise = Noise(seed);
    let blips: Vec<(f32, f32, f32)> = (0..count)
        .map(|_| {
            (
                noise.unit() * secs * 0.85,
                250.0 + noise.unit() * 700.0,
                0.03 + noise.unit() * 0.05,
            )
        })
        .collect();
    fade(
        render(secs, |t| {
            blips
                .iter()
                .map(|&(at, f, len)| {
                    let lt = t - at;
                    if lt < 0.0 || lt > len * 4.0 {
                        0.0
                    } else {
                        let ff = f * (1.0 + lt / len * 0.8);
                        (lt * ff * TAU).sin() * pluck(lt, 0.002, len)
                    }
                })
                .sum::<f32>()
                * vol
        }),
        0.03,
    )
}

/// A voice-like tone: a buzzy saw through two formant filters.
fn voice(secs: f32, f0: f32, f1: f32, formants: (f32, f32), vol: f32, vibrato: f32, seed: u32) -> Vec<f32> {
    let mut phase = 0.0f32;
    let mut a = Svf::default();
    let mut b = Svf::default();
    let mut noise = Noise(seed);
    fade(
        render(secs, |t| {
            let k = t / secs;
            let f = (f0 + (f1 - f0) * k) * (1.0 + (t * 6.5 * TAU).sin() * vibrato);
            phase += f / RATE as f32;
            let src = saw(phase) + noise.next() * 0.15;
            let out = a.band(src, formants.0, 0.25) + 0.6 * b.band(src, formants.1, 0.3);
            let env = (t / 0.04).min(1.0) * (1.0 - k).powf(0.7);
            out * vol * env
        }),
        0.05,
    )
}

/// Karplus-Strong plucked string.
fn string(secs: f32, freq: f32, vol: f32, seed: u32) -> Vec<f32> {
    let n = (RATE as f32 / freq) as usize;
    let mut noise = Noise(seed);
    let mut buf: Vec<f32> = (0..n).map(|_| noise.next()).collect();
    let mut i = 0;
    fade(
        render(secs, |_| {
            let j = (i + 1) % n;
            let s = buf[i];
            buf[i] = (buf[i] + buf[j]) * 0.5 * 0.994;
            i = j;
            s * vol
        }),
        0.05,
    )
}

fn mix(parts: Vec<Vec<f32>>) -> Vec<f32> {
    let n = parts.iter().map(|p| p.len()).max().unwrap_or(0);
    (0..n)
        .map(|i| {
            parts
                .iter()
                .map(|p| p.get(i).copied().unwrap_or(0.0))
                .sum::<f32>()
                .clamp(-1.0, 1.0)
        })
        .collect()
}

/// Offsets `v` by `secs` of silence.
fn delay(v: Vec<f32>, secs: f32) -> Vec<f32> {
    let mut out = vec![0.0; (secs * RATE as f32) as usize];
    out.extend(v);
    out
}

/// A long low roll of thunder.
fn rumble(secs: f32, vol: f32, seed: u32) -> Vec<f32> {
    let mut noise = Noise(seed);
    let mut lp = 0.0f32;
    let mut lp2 = 0.0f32;
    let mut slow = Noise(seed ^ 77);
    let mut amp = 0.5f32;
    let mut amp_target = 0.5f32;
    let mut i = 0usize;
    fade(
        render(secs, |t| {
            if i.is_multiple_of(2200) {
                amp_target = 0.3 + slow.unit() * 0.9;
            }
            i += 1;
            amp += (amp_target - amp) * 0.0006;
            lp += (noise.next() - lp) * 0.02;
            lp2 += (lp - lp2) * 0.05;
            let env = (t / 0.3).min(1.0) * (-t / (secs * 0.4)).exp();
            lp2 * amp * env * vol * 9.0
        }),
        0.3,
    )
}

fn effect(sound: Sound) -> Vec<f32> {
    match sound {
        Sound::Splash => mix(vec![
            whoosh(0.5, 0.5, 0.1, 0.3, 0.12, 3),
            bloop(0.12, 700.0, 220.0, 0.2, 0.05),
            thud(0.3, 120.0, 60.0, 0.25, 0.08),
            delay(bubbles(0.4, 5, 0.05, 4), 0.08),
        ]),
        Sound::BigSplash => mix(vec![
            whoosh(1.0, 0.45, 0.05, 0.3, 0.28, 5),
            thud(0.6, 90.0, 38.0, 0.45, 0.2),
            bloop(0.2, 500.0, 140.0, 0.25, 0.1),
            delay(whoosh(0.6, 0.3, 0.05, 0.2, 0.2, 6), 0.15),
            delay(bubbles(0.7, 10, 0.06, 7), 0.2),
        ]),
        Sound::Burst => mix(vec![whoosh(0.35, 0.08, 0.03, 0.5, 0.12, 9), bubbles(0.45, 9, 0.06, 10)]),
        Sound::Claw => mix(vec![
            whoosh(0.18, 0.9, 0.35, 0.3, 0.05, 11),
            delay(whoosh(0.1, 0.95, 0.6, 0.15, 0.03, 12), 0.03),
        ]),
        Sound::Hit => mix(vec![
            thud(0.25, 160.0, 60.0, 0.5, 0.07),
            whoosh(0.12, 0.7, 0.2, 0.3, 0.03, 13),
        ]),
        Sound::Grab => mix(vec![
            thud(0.2, 110.0, 70.0, 0.3, 0.06),
            whoosh(0.15, 0.3, 0.1, 0.2, 0.05, 14),
        ]),
        Sound::Throw => whoosh(0.35, 0.2, 0.6, 0.3, 0.14, 15),
        Sound::Jump => mix(vec![
            voice(0.18, 150.0, 110.0, (500.0, 900.0), 0.25, 0.0, 16),
            whoosh(0.15, 0.2, 0.1, 0.15, 0.05, 17),
        ]),
        Sound::Thud => mix(vec![
            thud(0.3, 100.0, 40.0, 0.5, 0.1),
            whoosh(0.2, 0.2, 0.05, 0.2, 0.06, 18),
        ]),
        Sound::Squelch => mix(vec![
            whoosh(0.1, 0.25, 0.1, 0.25, 0.03, 19),
            bloop(0.06, 180.0, 90.0, 0.12, 0.02),
        ]),
        Sound::Boing => {
            let mut phase = 0.0f32;
            fade(
                render(0.6, |t| {
                    let f = 180.0 + 260.0 * (1.0 - (-t * 9.0).exp()) + (t * 18.0 * TAU).sin() * 30.0 * (-t * 3.0).exp();
                    phase += f / RATE as f32;
                    (phase * TAU).sin() * 0.3 * pluck(t, 0.005, 0.25)
                }),
                0.05,
            )
        }
        Sound::Hurt => mix(vec![
            voice(0.4, 120.0, 80.0, (400.0, 800.0), 0.36, 0.03, 20),
            whoosh(0.2, 0.4, 0.1, 0.2, 0.05, 21),
        ]),
        Sound::Death => mix(vec![
            voice(1.6, 110.0, 45.0, (350.0, 700.0), 0.38, 0.04, 22),
            delay(bubbles(1.0, 14, 0.06, 23), 0.6),
        ]),
        Sound::Scream => voice(0.9, 420.0, 260.0, (850.0, 1250.0), 0.3, 0.035, 24),
        Sound::Gurgle => mix(vec![
            bubbles(1.3, 22, 0.08, 25),
            voice(0.8, 160.0, 90.0, (300.0, 600.0), 0.18, 0.08, 26),
        ]),
        Sound::Whistle => {
            let mut phase = 0.0f32;
            fade(
                render(0.75, |t| {
                    let f = if t < 0.3 {
                        2300.0 + t * 900.0
                    } else if t < 0.36 {
                        2000.0
                    } else {
                        2500.0 + (t * 40.0).sin() * 60.0
                    };
                    phase += f / RATE as f32;
                    let env = if (0.3..0.36).contains(&t) { 0.1 } else { 1.0 };
                    (phase * TAU).sin() * 0.38 * env * pluck(t, 0.01, 0.5)
                }),
                0.05,
            )
        }
        Sound::Huh => voice(0.32, 130.0, 175.0, (500.0, 1500.0), 0.35, 0.0, 27),
        Sound::Twang => mix(vec![
            string(0.5, 175.0, 0.35, 28),
            whoosh(0.05, 0.9, 0.5, 0.2, 0.01, 29),
        ]),
        Sound::Thunk => mix(vec![
            thud(0.18, 320.0, 150.0, 0.4, 0.04),
            whoosh(0.05, 0.8, 0.3, 0.2, 0.01, 30),
        ]),
        Sound::Creak => {
            let mut noise = Noise(31);
            let mut phase = 0.0f32;
            let mut f = Svf::default();
            fade(
                render(0.55, |t| {
                    let pitch = 70.0 + 30.0 * (t * 3.0).sin();
                    phase += pitch / RATE as f32;
                    let stick = if (phase * 3.0).fract() < 0.18 { 1.0 } else { 0.0 };
                    let x = (saw(phase) * 0.5 + noise.next() * 0.3) * stick;
                    f.band(x, 900.0, 0.4) * 0.5 * (t / 0.1).min(1.0) * (1.0 - t / 0.55)
                }),
                0.05,
            )
        }
        Sound::Swing => whoosh(0.3, 0.3, 0.7, 0.25, 0.1, 32),
        Sound::Snap => mix(vec![
            whoosh(0.06, 1.0, 0.8, 0.4, 0.01, 33),
            string(0.4, 90.0, 0.25, 34),
            thud(0.2, 200.0, 90.0, 0.3, 0.05),
        ]),
        Sound::Glass => {
            let mut noise = Noise(35);
            let tinks: Vec<(f32, f32)> = (0..12)
                .map(|_| (noise.unit() * 0.4, 2500.0 + noise.unit() * 4000.0))
                .collect();
            mix(vec![
                whoosh(0.3, 1.0, 0.7, 0.3, 0.06, 36),
                fade(
                    render(0.7, |t| {
                        tinks
                            .iter()
                            .map(|&(at, f)| ((t - at) * f * TAU).sin() * pluck(t - at, 0.001, 0.05))
                            .sum::<f32>()
                            * 0.06
                    }),
                    0.05,
                ),
            ])
        }
        Sound::Sizzle => {
            let mut noise = Noise(37);
            let mut lp = 0.0f32;
            fade(
                render(0.9, |t| {
                    let n = noise.next();
                    lp += (n - lp) * 0.3;
                    (n - lp) * 0.35 * pluck(t, 0.01, 0.35) * (0.7 + 0.3 * (t * 90.0).sin())
                }),
                0.05,
            )
        }
        Sound::Capsize => mix(vec![
            effect(Sound::BigSplash),
            delay(effect(Sound::Creak), 0.05),
            delay(thud(0.5, 80.0, 40.0, 0.4, 0.15), 0.1),
        ]),
        Sound::Thunder => rumble(4.5, 0.7, 38),
        Sound::ThunderNear => mix(vec![
            whoosh(0.25, 1.0, 0.4, 0.6, 0.06, 39),
            delay(whoosh(0.4, 0.8, 0.2, 0.4, 0.12, 40), 0.05),
            rumble(4.0, 0.8, 41),
        ]),
        Sound::Tick => bloop(0.05, 900.0, 700.0, 0.12, 0.02),
        Sound::Confirm => mix(vec![
            bloop(0.2, 300.0, 600.0, 0.18, 0.08),
            delay(bloop(0.25, 450.0, 900.0, 0.14, 0.1), 0.07),
        ]),
        Sound::Back => bloop(0.2, 500.0, 250.0, 0.15, 0.08),
        Sound::Victory => mix(vec![
            chord(&[-17.0, -10.0, -5.0, 2.0], 3.0, 0.35, 0.6),
            delay(bubbles(1.2, 12, 0.05, 42), 0.5),
            delay(voice(2.0, 220.0, 330.0, (600.0, 1100.0), 0.12, 0.02, 43), 0.3),
        ]),
        Sound::Defeat => mix(vec![chord(&[-29.0, -22.0, -18.0], 3.0, 0.4, 0.3), rumble(2.5, 0.5, 44)]),
    }
}

/// A soft choir-like chord: detuned saws through a low-pass, swelling in.
fn chord(semis: &[f32], secs: f32, vol: f32, attack: f32) -> Vec<f32> {
    let mut phases = vec![0.0f32; semis.len() * 3];
    let mut lp = 0.0f32;
    fade(
        render(secs, |t| {
            let mut s = 0.0;
            for (k, &semi) in semis.iter().enumerate() {
                for d in 0..3 {
                    let det = [-0.08, 0.0, 0.07][d];
                    let p = &mut phases[k * 3 + d];
                    *p += note(semi + det) / RATE as f32;
                    s += saw(*p);
                }
            }
            lp += (s - lp) * 0.05;
            let env = (t / attack).min(1.0) * (1.0 - (t / secs).powi(3));
            lp * env * vol / semis.len() as f32
        }),
        0.1,
    )
}

/// Crossfades the tail into the head so a buffer loops seamlessly.
fn make_loop(mut v: Vec<f32>, overlap: f32) -> Vec<f32> {
    let n = (overlap * RATE as f32) as usize;
    let len = v.len() - n;
    for i in 0..n {
        let k = i as f32 / n as f32;
        v[i] = v[i] * k + v[len + i] * (1.0 - k);
    }
    v.truncate(len);
    v
}

/// Rain on water and leaves: hiss plus a patter of drops.
fn rain_loop() -> Vec<f32> {
    let mut noise = Noise(51);
    let mut lp = 0.0f32;
    let mut lp2 = 0.0f32;
    let mut drops = Noise(52);
    let mut drip = 0.0f32;
    let mut drip_f = 0.0f32;
    let mut phase = 0.0f32;
    make_loop(
        render(8.5, |_| {
            let n = noise.next();
            lp += (n - lp) * 0.25;
            lp2 += (lp - lp2) * 0.08;
            if drops.unit() < 0.0009 {
                drip = 0.6 + drops.unit() * 0.4;
                drip_f = 1200.0 + drops.unit() * 3000.0;
            }
            drip *= 0.9985;
            phase += drip_f / RATE as f32;
            (lp - lp2) * 0.5 + lp2 * 0.3 + (phase * TAU).sin() * drip * drip * 0.05
        }),
        0.5,
    )
}

/// Frogs and insects.
fn swamp_loop() -> Vec<f32> {
    let secs = 12.5;
    let mut noise = Noise(61);
    let croaks: Vec<(f32, f32, u32)> = (0..16)
        .map(|_| {
            (
                noise.unit() * 12.0,
                90.0 + noise.unit() * 80.0,
                2 + (noise.unit() * 4.0) as u32,
            )
        })
        .collect();
    let chirps: Vec<(f32, f32)> = (0..30)
        .map(|_| (noise.unit() * 12.0, 3800.0 + noise.unit() * 1400.0))
        .collect();
    let mut f = Svf::default();
    make_loop(
        render(secs, |t| {
            let mut s = 0.0;
            for &(at, freq, pulses) in &croaks {
                let lt = t - at;
                let len = pulses as f32 * 0.07;
                if lt >= 0.0 && lt < len {
                    let pulse = ((lt / 0.07).fract() < 0.6) as i32 as f32;
                    s += saw(lt * freq) * pulse * 0.18;
                }
            }
            let croak = f.band(s, 450.0, 0.5);
            let mut ins = 0.0;
            for &(at, freq) in &chirps {
                let lt = t - at;
                if (0.0..0.35).contains(&lt) {
                    let am = ((lt * 38.0 * TAU).sin() * 0.5 + 0.5).powi(4);
                    ins += (lt * freq * TAU).sin() * am * 0.035;
                }
            }
            croak * 0.8 + ins
        }),
        0.5,
    )
}

/// A slow, dark drone in D with a heartbeat.
fn music_calm() -> Vec<f32> {
    let secs = 32.0;
    let chords: [[f32; 3]; 4] = [
        [-31.0, -24.0, -19.0],
        [-31.0, -26.0, -19.0],
        [-28.0, -24.0, -21.0],
        [-33.0, -24.0, -20.0],
    ];
    let mut phases = [0.0f32; 9];
    let mut lp = 0.0f32;
    let mut sub = 0.0f32;
    make_loop(
        render(secs + 1.0, |t| {
            let bar = ((t / 8.0) as usize) % 4;
            let blend = ((t % 8.0) / 8.0).min(1.0);
            let ch = chords[bar];
            let mut s = 0.0;
            for (k, semi) in ch.iter().enumerate() {
                for d in 0..3 {
                    let det = [-0.06, 0.0, 0.05][d];
                    let p = &mut phases[k * 3 + d];
                    *p += note(semi + det) / RATE as f32;
                    s += saw(*p);
                }
            }
            let cutoff = 0.012 + 0.008 * (t * 0.2).sin() + 0.004 * blend;
            lp += (s - lp) * cutoff;
            sub += note(-45.0) / RATE as f32;
            // Heartbeat: lub-dub every 1.5 s.
            let beat = t % 1.5;
            let heart = (beat * 55.0 * TAU).sin() * pluck(beat, 0.005, 0.07) * 0.5
                + ((beat - 0.22) * 48.0 * TAU).sin() * pluck(beat - 0.22, 0.005, 0.06) * 0.35;
            lp * 0.05 + (sub * TAU).sin() * 0.05 + heart * 0.45
        }),
        1.0,
    )
}

/// Pulsing low strings for when the hunt is on.
fn music_tense() -> Vec<f32> {
    let secs = 16.0;
    const BEAT: f32 = 60.0 / 112.0;
    let mut phases = [0.0f32; 6];
    let mut lp = 0.0f32;
    let mut noise = Noise(71);
    make_loop(
        render(secs + 0.5, |t| {
            let beat = t / BEAT;
            let eighth = (beat * 2.0).fract() * BEAT * 0.5;
            let bar = ((beat / 4.0) as usize) % 4;
            let root = [-31.0, -31.0, -29.0, -32.0][bar];
            let mut s = 0.0;
            for (k, semi) in [root, root + 7.0].iter().enumerate() {
                for d in 0..3 {
                    let det = [-0.1, 0.0, 0.1][d];
                    let p = &mut phases[k * 3 + d];
                    *p += note(semi + det) / RATE as f32;
                    s += saw(*p);
                }
            }
            lp += (s - lp) * 0.04;
            let pulse = pluck(eighth, 0.01, 0.12);
            let tick = if (beat as usize).is_multiple_of(2) {
                0.0
            } else {
                noise.next() * pluck(beat.fract() * BEAT, 0.001, 0.03) * 0.15
            };
            let drum = ((beat.fract() * BEAT) * 60.0 * TAU).sin() * pluck(beat.fract() * BEAT, 0.003, 0.12) * 0.5;
            lp * 0.07 * pulse + tick + drum * 0.5
        }),
        0.5,
    )
}

#[derive(Resource)]
pub struct Bank {
    waves: HashMap<Sound, Handle<Wave>>,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum Loop {
    Rain,
    Swamp,
    Calm,
    Tense,
}

#[derive(Resource, Default)]
pub struct Muted(pub bool);

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<Wave>()
            .add_message::<Sfx>()
            .init_resource::<Muted>()
            .add_systems(Startup, setup)
            .add_systems(Update, (play, loops));
    }
}

fn setup(mut commands: Commands, mut waves: ResMut<Assets<Wave>>, args: Res<Args>) {
    let mut add = |v: Vec<f32>| waves.add(Wave { samples: v.into() });
    let bank = Bank {
        waves: ALL.iter().map(|&s| (s, add(effect(s)))).collect(),
    };
    if !args.mute {
        for (kind, wave) in [
            (Loop::Rain, rain_loop()),
            (Loop::Swamp, swamp_loop()),
            (Loop::Calm, music_calm()),
            (Loop::Tense, music_tense()),
        ] {
            commands.spawn((
                AudioPlayer(add(wave)),
                PlaybackSettings {
                    mode: PlaybackMode::Loop,
                    volume: Volume::Linear(0.0),
                    ..default()
                },
                kind,
            ));
        }
    }
    commands.insert_resource(bank);
}

fn play(
    mut commands: Commands,
    args: Res<Args>,
    muted: Res<Muted>,
    bank: Res<Bank>,
    cam: Query<&Transform, With<Camera3d>>,
    mut requests: MessageReader<Sfx>,
    mut played: Local<Vec<Sound>>,
) {
    played.clear();
    let listener = cam.single().map_or(Vec2::ZERO, |c| c.translation.truncate());
    for req in requests.read() {
        if args.mute || muted.0 {
            continue;
        }
        // Several of the same effect in one frame play once.
        if played.contains(&req.sound) {
            continue;
        }
        let volume = req.sound.volume()
            * match req.pos {
                None => 1.0,
                Some(p) => {
                    let d = p.distance(listener);
                    let k = (1.0 - d / 38.0).clamp(0.0, 1.0);
                    k * k
                }
            };
        if volume < 0.02 {
            continue;
        }
        played.push(req.sound);
        commands.spawn((
            AudioPlayer(bank.waves[&req.sound].clone()),
            PlaybackSettings {
                mode: PlaybackMode::Despawn,
                volume: Volume::Linear(volume),
                ..default()
            },
        ));
    }
}

/// Loop volumes: rain with the storm, music tension with the hunt.
fn loops(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut muted: ResMut<Muted>,
    storm: Option<Res<Storm>>,
    stealth: Option<Res<crate::player::Stealth>>,
    hunters: Query<&Hunter>,
    mut sinks: Query<(&Loop, &mut AudioSink)>,
    mut tension: Local<f32>,
) {
    if keys.just_pressed(KeyCode::KeyM) {
        muted.0 = !muted.0;
    }
    let dt = time.delta_secs().min(0.1);
    // Only hunters who are after the bogwight around here count.
    let here = stealth.as_deref().map_or(Vec2::ZERO, |s| s.pos);
    let near = |h: &&Hunter| h.last_seen.distance(here) < 30.0;
    let hunted = hunters.iter().filter(near).any(|h| h.state == State::Hunt);
    let wary = hunters.iter().filter(near).any(|h| h.state == State::Suspicious);
    let target = if hunted {
        1.0
    } else if wary {
        0.35
    } else {
        0.0
    };
    let rate = if target > *tension { 1.5 } else { 0.25 };
    *tension += (target - *tension) * (1.0 - (-rate * dt).exp());
    let rain = storm.map_or(0.8, |s| s.rain);
    for (kind, mut sink) in &mut sinks {
        let v = if muted.0 {
            0.0
        } else {
            match kind {
                Loop::Rain => 0.22 + 0.2 * rain,
                Loop::Swamp => 0.3 * (1.0 - *tension * 0.7),
                Loop::Calm => 0.55 * (1.0 - *tension * 0.6),
                Loop::Tense => 0.5 * *tension,
            }
        };
        sink.set_volume(Volume::Linear(v));
    }
}

/// Writes every synthesized sound as a 16-bit mono WAV.
pub fn dump(dir: &str) {
    let write = |name: &str, samples: &[f32]| {
        let data: Vec<u8> = samples
            .iter()
            .flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
            .collect();
        let mut wav = Vec::with_capacity(44 + data.len());
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&RATE.to_le_bytes());
        wav.extend_from_slice(&(RATE * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(&data);
        let path = format!("{dir}/{name}.wav");
        match std::fs::write(&path, wav) {
            Ok(()) => println!("{path}: {:.2} s", samples.len() as f32 / RATE as f32),
            Err(e) => eprintln!("{path}: {e}"),
        }
    };
    let _ = std::fs::create_dir_all(dir);
    for sound in ALL {
        write(&format!("{sound:?}").to_lowercase(), &effect(sound));
    }
    write("rain", &rain_loop());
    write("swamp", &swamp_loop());
    write("music_calm", &music_calm());
    write("music_tense", &music_tense());
}
