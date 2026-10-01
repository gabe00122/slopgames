//! Sound. Every effect and the music loop are synthesized into sample
//! buffers at startup. Effects in the world get quieter with distance from
//! the camera.

use crate::{Args, fx::Fx};
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
    Tick,
    Confirm,
    Back,
    Error,
    Order,
    Cast,
    Boom,
    BigBoom,
    Incoming,
    Heal,
    Rumble,
    Fork,
    Glitch,
    Summon,
    Sacrifice,
    Pickup,
    Grab,
    Bite,
    Slam,
    Zap,
    Blip,
    Pop,
    Crash,
    Compacted,
    Respawn,
    Build,
    Online,
    Alarm,
    Abort,
    Deprecated,
    Victory,
    Defeat,
}

const ALL: [Sound; 32] = [
    Sound::Tick,
    Sound::Confirm,
    Sound::Back,
    Sound::Error,
    Sound::Order,
    Sound::Cast,
    Sound::Boom,
    Sound::BigBoom,
    Sound::Incoming,
    Sound::Heal,
    Sound::Rumble,
    Sound::Fork,
    Sound::Glitch,
    Sound::Summon,
    Sound::Sacrifice,
    Sound::Pickup,
    Sound::Grab,
    Sound::Bite,
    Sound::Slam,
    Sound::Zap,
    Sound::Blip,
    Sound::Pop,
    Sound::Crash,
    Sound::Compacted,
    Sound::Respawn,
    Sound::Build,
    Sound::Online,
    Sound::Alarm,
    Sound::Abort,
    Sound::Deprecated,
    Sound::Victory,
    Sound::Defeat,
];

/// Request to play a sound, optionally at a place in the world.
#[derive(Message, Clone, Copy)]
pub struct Sfx {
    pub sound: Sound,
    pub pos: Option<Vec3>,
}

impl Sfx {
    pub fn at(sound: Sound, pos: Vec3) -> Self {
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

fn square(phase: f32) -> f32 {
    let x = phase * TAU;
    x.sin() + (3.0 * x).sin() / 3.0 + (5.0 * x).sin() / 5.0
}

fn saw(phase: f32) -> f32 {
    2.0 * phase.fract() - 1.0
}

fn tri(phase: f32) -> f32 {
    1.0 - 4.0 * (phase.fract() - 0.5).abs()
}

fn note(semitones_from_a4: f32) -> f32 {
    440.0 * 2f32.powf(semitones_from_a4 / 12.0)
}

/// Plucked notes: `(start, semitone, decay)`.
fn arp(notes: &[(f32, f32, f32)], secs: f32, vol: f32) -> Vec<f32> {
    fade(
        render(secs, |t| {
            notes
                .iter()
                .map(|&(at, semi, len)| {
                    let lt = t - at;
                    if lt < 0.0 {
                        0.0
                    } else {
                        square(lt * note(semi)) * pluck(lt, 0.004, len)
                    }
                })
                .sum::<f32>()
                * vol
        }),
        0.02,
    )
}

fn sweep(secs: f32, f0: f32, f1: f32, vol: f32, decay: f32) -> Vec<f32> {
    let mut phase = 0.0f32;
    fade(
        render(secs, |t| {
            let f = f0 + (f1 - f0) * (t / secs);
            phase += f / RATE as f32;
            square(phase) * vol * pluck(t, 0.005, decay)
        }),
        0.02,
    )
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

/// Bit-crushed random square blips: the sound of corrupted data.
fn glitch(secs: f32, vol: f32, seed: u32) -> Vec<f32> {
    let mut noise = Noise(seed);
    let mut f = 400.0;
    let mut phase = 0.0f32;
    let mut hold = 0usize;
    let step = (RATE as f32 * 0.025) as usize;
    let mut i = 0usize;
    fade(
        render(secs, |t| {
            if i.is_multiple_of(step) {
                f = 150.0 + (noise.next() * 0.5 + 0.5) * 1800.0;
                hold = (noise.next() * 4.0).abs() as usize;
            }
            i += 1;
            phase += f / RATE as f32;
            let s = if phase.fract() < 0.5 { 1.0 } else { -1.0 };
            let crushed = (s * 4.0f32).round() / 4.0;
            crushed * vol * (1.0 - t / secs) * if hold == 0 { 0.4 } else { 1.0 }
        }),
        0.02,
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
            lp += (s - lp) * 0.06;
            let env = (t / attack).min(1.0) * (1.0 - (t / secs).powi(3));
            lp * env * vol / semis.len() as f32
        }),
        0.1,
    )
}

fn effect(sound: Sound) -> Vec<f32> {
    match sound {
        Sound::Tick => arp(&[(0.0, 19.0, 0.02)], 0.06, 0.12),
        Sound::Confirm => arp(&[(0.0, 7.0, 0.06), (0.06, 14.0, 0.12)], 0.3, 0.2),
        Sound::Back => arp(&[(0.0, 7.0, 0.06), (0.06, 0.0, 0.12)], 0.3, 0.2),
        Sound::Error => mix(vec![
            sweep(0.18, 180.0, 150.0, 0.2, 0.12),
            delay(sweep(0.18, 180.0, 150.0, 0.2, 0.12), 0.1),
        ]),
        Sound::Order => arp(&[(0.0, 12.0, 0.04), (0.05, 7.0, 0.06)], 0.2, 0.15),
        Sound::Cast => mix(vec![
            sweep(0.25, 300.0, 900.0, 0.12, 0.1),
            whoosh(0.3, 0.1, 0.6, 0.2, 0.12, 3),
        ]),
        Sound::Boom => mix(vec![
            thud(0.6, 160.0, 45.0, 0.45, 0.18),
            whoosh(0.7, 0.5, 0.05, 0.3, 0.2, 7),
        ]),
        Sound::BigBoom => mix(vec![
            thud(1.6, 120.0, 28.0, 0.5, 0.5),
            whoosh(2.0, 0.6, 0.02, 0.36, 0.6, 9),
            delay(whoosh(1.2, 0.2, 0.02, 0.22, 0.4, 19), 0.15),
        ]),
        Sound::Incoming => {
            let mut phase = 0.0f32;
            fade(
                render(1.5, |t| {
                    let f = 1400.0 - 900.0 * (t / 1.5);
                    phase += f / RATE as f32;
                    (phase * TAU).sin() * 0.12 * (t / 1.5).powf(0.5)
                }),
                0.05,
            )
        }
        Sound::Heal => arp(
            &[
                (0.0, 12.0, 0.2),
                (0.08, 16.0, 0.2),
                (0.16, 19.0, 0.2),
                (0.24, 24.0, 0.35),
            ],
            0.8,
            0.12,
        ),
        Sound::Rumble => mix(vec![
            whoosh(1.2, 0.03, 0.01, 0.9, 0.6, 21),
            thud(1.0, 70.0, 35.0, 0.4, 0.5),
        ]),
        Sound::Fork => mix(vec![glitch(0.5, 0.12, 5), sweep(0.5, 200.0, 1200.0, 0.1, 0.3)]),
        Sound::Glitch => glitch(0.22, 0.1, 11),
        Sound::Summon => mix(vec![
            chord(&[-12.0, -5.0, 0.0, 3.0], 1.4, 0.35, 0.25),
            delay(
                arp(&[(0.0, 12.0, 0.1), (0.07, 15.0, 0.1), (0.14, 19.0, 0.2)], 0.6, 0.1),
                0.2,
            ),
            whoosh(1.0, 0.05, 0.4, 0.15, 0.4, 31),
        ]),
        Sound::Sacrifice => mix(vec![
            chord(&[-9.0, -2.0, 3.0], 1.2, 0.3, 0.1),
            whoosh(0.9, 0.4, 0.08, 0.25, 0.3, 37),
        ]),
        Sound::Pickup => arp(&[(0.0, 14.0, 0.05), (0.05, 19.0, 0.05), (0.1, 26.0, 0.12)], 0.35, 0.14),
        Sound::Grab => arp(&[(0.0, 5.0, 0.04), (0.04, 12.0, 0.06)], 0.15, 0.1),
        Sound::Bite => whoosh(0.1, 0.8, 0.3, 0.25, 0.03, 41),
        Sound::Slam => mix(vec![
            thud(0.5, 110.0, 40.0, 0.55, 0.15),
            whoosh(0.4, 0.3, 0.05, 0.3, 0.1, 43),
        ]),
        Sound::Zap => sweep(0.15, 1600.0, 500.0, 0.1, 0.05),
        Sound::Blip => sweep(0.1, 900.0, 1400.0, 0.08, 0.04),
        Sound::Pop => mix(vec![glitch(0.12, 0.1, 47), thud(0.2, 300.0, 120.0, 0.25, 0.05)]),
        Sound::Crash => mix(vec![glitch(0.35, 0.14, 53), whoosh(0.3, 0.6, 0.1, 0.2, 0.1, 59)]),
        Sound::Compacted => mix(vec![
            sweep(1.4, 900.0, 60.0, 0.16, 0.9),
            glitch(0.8, 0.12, 61),
            delay(thud(0.8, 90.0, 30.0, 0.5, 0.4), 0.3),
        ]),
        Sound::Respawn => mix(vec![
            sweep(0.9, 150.0, 1100.0, 0.12, 0.5),
            chord(&[0.0, 7.0, 12.0], 1.0, 0.25, 0.3),
        ]),
        Sound::Build => mix(vec![
            whoosh(3.0, 0.02, 0.08, 0.5, 2.0, 67),
            arp(&[(0.0, -12.0, 0.3), (0.3, -5.0, 0.3), (0.6, 0.0, 0.5)], 1.5, 0.12),
        ]),
        Sound::Online => arp(&[(0.0, 7.0, 0.08), (0.08, 12.0, 0.08), (0.16, 19.0, 0.2)], 0.6, 0.14),
        Sound::Alarm => {
            let mut phase = 0.0f32;
            fade(
                render(1.6, |t| {
                    let f = if (t * 4.0).fract() < 0.5 { 880.0 } else { 660.0 };
                    phase += f / RATE as f32;
                    square(phase) * 0.12 * (1.0 - t / 1.6)
                }),
                0.05,
            )
        }
        Sound::Abort => arp(&[(0.0, 5.0, 0.1), (0.12, 0.0, 0.1), (0.24, -7.0, 0.3)], 0.8, 0.16),
        Sound::Deprecated => mix(vec![
            thud(3.0, 100.0, 25.0, 0.55, 1.2),
            whoosh(3.5, 0.5, 0.01, 0.32, 1.2, 71),
            glitch(1.5, 0.12, 73),
            delay(chord(&[-24.0, -17.0, -12.0, -9.0], 3.0, 0.4, 0.5), 0.4),
        ]),
        Sound::Victory => mix(vec![
            arp(
                &[
                    (0.0, 0.0, 0.15),
                    (0.15, 4.0, 0.15),
                    (0.3, 7.0, 0.15),
                    (0.45, 12.0, 0.4),
                    (0.8, 7.0, 0.15),
                    (0.95, 12.0, 0.8),
                    (0.95, 16.0, 0.8),
                ],
                2.4,
                0.14,
            ),
            delay(chord(&[-12.0, -5.0, 0.0, 4.0], 1.6, 0.3, 0.2), 0.9),
        ]),
        Sound::Defeat => mix(vec![
            arp(
                &[(0.0, 3.0, 0.3), (0.3, 0.0, 0.3), (0.6, -4.0, 0.3), (0.9, -9.0, 1.0)],
                2.4,
                0.14,
            ),
            delay(chord(&[-21.0, -14.0, -9.0], 1.8, 0.3, 0.3), 0.9),
        ]),
    }
}

/// An ominous synth loop: A minor, F, D minor, E, two bars each.
fn music() -> Vec<f32> {
    const BEAT: f32 = 60.0 / 104.0;
    let bars: [[f32; 4]; 4] = [
        [-12.0, -9.0, -5.0, 0.0],
        [-16.0, -12.0, -9.0, -4.0],
        [-19.0, -14.0, -11.0, -7.0],
        [-17.0, -13.0, -10.0, -5.0],
    ];
    let roots = [-24.0, -28.0, -31.0, -29.0];
    let total = 32.0 * BEAT;
    let mut noise = Noise(83);
    let mut hat_lp = 0.0f32;
    let mut pad_phase = [0.0f32; 12];
    let mut pad_lp = 0.0f32;
    let mut bass_phase = 0.0f32;
    // Arpeggio pattern over the chord tones, sixteenths.
    let pattern = [0usize, 2, 3, 1, 2, 3, 0, 3, 1, 2, 3, 2, 0, 3, 2, 1];
    render(total, |t| {
        let beat = t / BEAT;
        let bar = (beat / 8.0) as usize % 4;
        let ch = &bars[bar];
        let mut s = 0.0;
        // Pad.
        let mut pad = 0.0;
        for (k, semi) in ch.iter().enumerate() {
            for d in 0..3 {
                let det = [-0.07, 0.0, 0.06][d];
                let p = &mut pad_phase[k * 3 + d];
                *p += note(semi + det) / RATE as f32;
                pad += saw(*p);
            }
        }
        pad_lp += (pad - pad_lp) * (0.025 + 0.015 * (t * 0.3).sin());
        let swell = 0.7 + 0.3 * ((beat / 8.0).fract() * std::f32::consts::PI).sin();
        s += pad_lp * 0.035 * swell;
        // Arpeggio, an octave up, quieter on the first bar of each pair.
        let six = beat * 4.0;
        let si = six as usize;
        let lt = six.fract() * BEAT / 4.0;
        let tone = ch[pattern[si % 16]] + 12.0;
        let accent = if si.is_multiple_of(4) { 1.0 } else { 0.65 };
        s += square(lt * note(tone)) * pluck(lt, 0.003, 0.09) * 0.05 * accent;
        // Bass: root, with a push before beat 3.
        let in_bar = beat % 4.0;
        let mut bass_env = 0.0;
        for (at, len) in [(0.0, 1.4), (1.5, 0.4), (2.0, 1.4), (3.5, 0.4)] {
            let bt = (in_bar - at) * BEAT;
            if bt >= 0.0 && bt < len * BEAT {
                bass_env = pluck(bt, 0.01, 0.35);
            }
        }
        bass_phase += note(roots[bar]) / RATE as f32;
        s += (tri(bass_phase) * 0.7 + (bass_phase * TAU).sin() * 0.5) * bass_env * 0.3;
        // Drums.
        let bt = (beat % 1.0) * BEAT;
        let which = beat as usize % 4;
        let n = noise.next();
        if which.is_multiple_of(2) {
            s += ((bt * (110.0 - 240.0 * bt).max(38.0)) * TAU).sin() * pluck(bt, 0.002, 0.09) * 0.42;
        } else {
            s += n * pluck(bt, 0.002, 0.06) * 0.12;
        }
        hat_lp += (n - hat_lp) * 0.55;
        let et = ((beat * 2.0) % 1.0) * BEAT * 0.5;
        s += (n - hat_lp) * pluck(et, 0.001, 0.02) * 0.05;
        s
    })
}

#[derive(Resource)]
pub struct Bank {
    waves: HashMap<Sound, Handle<Wave>>,
    music: Handle<Wave>,
}

#[derive(Component)]
pub struct Music;

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<Wave>()
            .add_message::<Sfx>()
            .add_systems(Startup, setup)
            .add_systems(Update, (play, music_toggle));
    }
}

fn setup(mut commands: Commands, mut waves: ResMut<Assets<Wave>>, args: Res<Args>) {
    let mut add = |v: Vec<f32>| waves.add(Wave { samples: v.into() });
    let bank = Bank {
        waves: ALL.iter().map(|&s| (s, add(effect(s)))).collect(),
        music: add(music()),
    };
    if !args.mute {
        commands.spawn((
            AudioPlayer(bank.music.clone()),
            PlaybackSettings {
                mode: PlaybackMode::Loop,
                volume: Volume::Linear(0.5),
                ..default()
            },
            Music,
        ));
    }
    commands.insert_resource(bank);
}

fn play(
    mut commands: Commands,
    args: Res<Args>,
    bank: Res<Bank>,
    fx: Res<Fx>,
    mut requests: MessageReader<Sfx>,
    mut played: Local<Vec<Sound>>,
) {
    played.clear();
    for req in requests.read() {
        if args.mute {
            continue;
        }
        // Several of the same effect in one frame play once, at the loudest.
        if played.contains(&req.sound) {
            continue;
        }
        let volume = match req.pos {
            None => 0.8,
            Some(p) => {
                let d = p.distance(fx.listener);
                let k = (1.0 - d / 140.0).clamp(0.0, 1.0);
                0.9 * k * k
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

fn music_toggle(keys: Res<ButtonInput<KeyCode>>, mut music: Query<&mut AudioSink, With<Music>>) {
    if keys.just_pressed(KeyCode::KeyM) {
        for mut sink in &mut music {
            sink.toggle_mute();
        }
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
    write("music", &music());
}
